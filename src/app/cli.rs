use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use rand::RngCore;
use serde_json::Value;

use crate::app::{
    token_file, HostRequest, HostResponse, DEFAULT_HOST_PORT, DEFAULT_HOST_PROBE_PORT,
    HOST_PROTOCOL_VERSION,
};
use crate::project::SessionConfig;

const HOST_REQUEST_TIMEOUT: Duration = Duration::from_secs(175);

// --start-host 使用的入口：创建 Token，必要时启动后台 Host，并等待监听就绪。
pub fn ensure_host() -> Result<()> {
    let token_path = token_file();
    let launch_key = load_or_create_token(&token_path)?;
    match probe_host(&launch_key) {
        Ok(ProbeState::Ready) => return Ok(()),
        Ok(ProbeState::Busy) => {
            bail!("resident host is busy with another request; retry after that request completes")
        }
        Ok(ProbeState::Stopping) => {
            bail!("resident host is shutting down; retry after it exits")
        }
        Err(error) if host_is_listening() => {
            bail!(
                "port {DEFAULT_HOST_PORT} is occupied by an incompatible, stale, or foreign resident host: {error}"
            )
        }
        Err(_) => {}
    }
    start_host(&token_path)?;
    wait_for_host_listener(&launch_key)
}

pub fn run(project_directory: &Path, raw_request: &str) -> Result<String> {
    // 同一安装目录一次只允许一个 CLI 业务请求。它与 Host 的 busy
    // 状态共同阻止第二个写/生成请求排队后在调用方超时之外继续执行。
    let _request_lock = acquire_request_lock()?;
    // CLI 不处理 ECUC 逻辑；它把已校验的工程配置、请求和 Token 一起转发给 Host。
    let shutdown = is_shutdown_request(raw_request)?;
    let token_path = token_file();
    let launch_key = load_or_create_token(&token_path)?;
    let config = if shutdown {
        None
    } else {
        Some(SessionConfig::load(project_directory)?)
    };

    match send_request(config.as_ref(), raw_request, &launch_key, shutdown) {
        Ok(response) => {
            let result = response_text(response);
            if shutdown {
                // The Host acknowledges shutdown just before its listener
                // threads and process finish.  Wait for both ports to close so
                // callers can safely move/delete the project directory or
                // start a matching replacement immediately after this returns.
                wait_for_host_shutdown()?;
            }
            result
        }
        Err(first_error) if host_is_listening() => bail!(
            "port {DEFAULT_HOST_PORT} is occupied by an incompatible, stale, or foreign resident host; stop that process normally before retrying: {first_error}"
        ),
        Err(first_error) if shutdown => Ok(serde_json::json!({
            "status": "not_running",
            "message": first_error.to_string(),
        })
        .to_string()),
        Err(first_error) => bail!(
            "resident host is not running; run --start-host before the request: {first_error}"
        ),
    }
}

pub fn doctor(project_directory: &Path, raw_request: &str) -> Result<String> {
    let config = SessionConfig::load(project_directory)?;
    let functions =
        crate::daemon::commands::CommandDispatcher::validate_batch(&config, raw_request)?;
    // Doctor is intentionally a static preflight. It resolves every path and
    // local prerequisite but does not claim that the proprietary command can
    // start, acquire the DPA, or complete generation.
    let project_file = config.dpa_file()?;
    let davinci_command = crate::daemon::client::resolve_davinci_command(&config)?;
    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "valid": true,
        "preflight": "static",
        "davinci_executed": false,
        "version": env!("CARGO_PKG_VERSION"),
        "project_path": config.project_path,
        "tool_path": config.tool_path,
        "project_file": project_file,
        "davinci_command_path": davinci_command,
        "functions": functions,
    }))?)
}

fn wait_for_host_listener(launch_key: &str) -> Result<()> {
    // 后台进程创建和端口监听是两个时刻，因此轮询直到 Host 真正可连接。
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        match probe_host(launch_key) {
            Ok(ProbeState::Ready) => return Ok(()),
            Ok(ProbeState::Busy) => {
                bail!("new resident host unexpectedly reported busy during startup")
            }
            Ok(ProbeState::Stopping) => {
                bail!("new resident host unexpectedly reported stopping during startup")
            }
            Err(_) => {}
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("resident host did not become ready")
}

fn host_is_listening() -> bool {
    port_is_listening(DEFAULT_HOST_PORT)
}

fn port_is_listening(port: u16) -> bool {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
}

fn wait_for_host_shutdown() -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if !port_is_listening(DEFAULT_HOST_PORT) && !port_is_listening(DEFAULT_HOST_PROBE_PORT) {
            // Port closure slightly precedes final Windows process teardown.
            thread::sleep(Duration::from_millis(50));
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    bail!(
        "resident host acknowledged shutdown but ports {DEFAULT_HOST_PORT}/{DEFAULT_HOST_PROBE_PORT} did not close"
    )
}

fn is_shutdown_request(raw: &str) -> Result<bool> {
    // shutdown 必须独立发送，避免同一个批次中一半请求已执行、一半被提前关闭。
    let value: Value = serde_json::from_str(raw).context("request is not valid JSON")?;
    match value {
        Value::Object(map) => Ok(map
            .get("func")
            .and_then(Value::as_str)
            .is_some_and(|func| func == "shutdown_host")),
        Value::Array(items) => {
            if items.iter().any(|item| {
                item.get("func")
                    .and_then(Value::as_str)
                    .is_some_and(|func| func == "shutdown_host")
            }) {
                bail!("shutdown_host must be a standalone request");
            }
            Ok(false)
        }
        _ => bail!("request must be a JSON object or array"),
    }
}

fn send_request(
    config: Option<&SessionConfig>,
    raw: &str,
    launch_key: &str,
    shutdown: bool,
) -> Result<HostResponse> {
    send_host_request(
        HostRequest {
            protocol_version: HOST_PROTOCOL_VERSION,
            raw_text: raw.to_string(),
            cfg: config.cloned(),
            launch_key: launch_key.to_string(),
            shutdown,
            probe: false,
        },
        HOST_REQUEST_TIMEOUT,
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProbeState {
    Ready,
    Busy,
    Stopping,
}

fn probe_host(launch_key: &str) -> Result<ProbeState> {
    let response = send_probe_request(
        HostRequest {
            protocol_version: HOST_PROTOCOL_VERSION,
            raw_text: String::new(),
            cfg: None,
            launch_key: launch_key.to_string(),
            shutdown: false,
            probe: true,
        },
        Duration::from_secs(2),
    )?;
    let raw = response_text(response)?;
    let probe: Value = serde_json::from_str(&raw).context("invalid resident host probe")?;
    let version = probe.get("version").and_then(Value::as_str).unwrap_or("");
    let build_id = probe.get("build_id").and_then(Value::as_str).unwrap_or("");
    if version != env!("CARGO_PKG_VERSION") || build_id != crate::app::BUILD_ID {
        bail!(
            "resident host identity mismatch: expected version {} build {}, response={raw}",
            env!("CARGO_PKG_VERSION"),
            crate::app::BUILD_ID
        );
    }
    match probe.get("status").and_then(Value::as_str) {
        Some("ready") => Ok(ProbeState::Ready),
        Some("busy") => Ok(ProbeState::Busy),
        Some("stopping") => Ok(ProbeState::Stopping),
        _ => bail!("resident host returned an unknown activity state: {raw}"),
    }
}

fn send_host_request(request: HostRequest, timeout: Duration) -> Result<HostResponse> {
    // HostRequest 是本机 TCP 上传递的外层信封；raw_text 才是用户原始 JSON 请求。
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_HOST_PORT);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .with_context(|| format!("resident host is not running at {address}"))?;
    // On Windows a socket created by connect_timeout can briefly retain its
    // nonblocking state.  Force blocking I/O before the newline-framed JSON
    // exchange so WSAEWOULDBLOCK (10035) is never misreported as a bad Host.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(timeout))?;
    serde_json::to_writer(&mut stream, &request)?;
    writeln!(stream)?;
    stream.flush()?;

    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .context("LGK-Vector request exceeded the 3-minute operation budget")?;
    if response.trim().is_empty() {
        bail!("resident host returned an empty response");
    }
    serde_json::from_str(response.trim()).context("invalid resident host response")
}

fn send_probe_request(request: HostRequest, timeout: Duration) -> Result<HostResponse> {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_HOST_PROBE_PORT);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(500))
        .with_context(|| format!("resident probe is not running at {address}"))?;
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(timeout))?;
    serde_json::to_writer(&mut stream, &request)?;
    writeln!(stream)?;
    stream.flush()?;
    let mut response = String::new();
    BufReader::new(stream)
        .read_line(&mut response)
        .context("resident probe did not answer within 2 seconds")?;
    if response.trim().is_empty() {
        bail!("resident probe returned an empty response");
    }
    serde_json::from_str(response.trim()).context("invalid resident probe response")
}

fn response_text(response: HostResponse) -> Result<String> {
    if response.protocol_version != HOST_PROTOCOL_VERSION {
        bail!(
            "resident host protocol mismatch: CLI={}, Host={}",
            HOST_PROTOCOL_VERSION,
            response.protocol_version
        );
    }
    if response.exit_code != 0 {
        bail!("{}", response.stderr);
    }
    Ok(response.stdout)
}

fn load_or_create_token(path: &Path) -> Result<String> {
    // Token 保存在 EXE 旁边，CLI 与 Host 通过同一值确认彼此属于同一个本地安装。
    if let Ok(value) = fs::read_to_string(path) {
        let value = value.trim();
        if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(value.to_string());
        }
    }
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("token file has no parent directory"))?;
    fs::create_dir_all(parent)?;
    let mut bytes = [0_u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let token = bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    match OpenOptions::new().write(true).create_new(true).open(path) {
        Ok(mut file) => {
            writeln!(file, "{token}")?;
            file.sync_all()?;
            Ok(token)
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            let value = fs::read_to_string(path)?;
            let value = value.trim();
            if value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
                return Ok(value.to_string());
            }
            let mut file = OpenOptions::new().write(true).truncate(true).open(path)?;
            writeln!(file, "{token}")?;
            file.sync_all()?;
            Ok(token)
        }
        Err(error) => Err(error.into()),
    }
}

fn acquire_request_lock() -> Result<fs::File> {
    let path = token_file()
        .parent()
        .ok_or_else(|| anyhow::anyhow!("request lock has no parent directory"))?
        .join("request.lock");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    open_request_lock(&path).with_context(|| {
        "another LGK-Vector request is already running from this installation; wait for it to finish"
    })
}

#[cfg(windows)]
fn open_request_lock(path: &Path) -> std::io::Result<fs::File> {
    use std::os::windows::fs::OpenOptionsExt;
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .share_mode(0)
        .open(path)
}

#[cfg(not(windows))]
fn open_request_lock(path: &Path) -> std::io::Result<fs::File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

fn start_host(token_path: &Path) -> Result<()> {
    // Host EXE 与 CLI EXE 必须同目录发布，避免调用到另一个版本的后台程序。
    let current = std::env::current_exe()?;
    let directory = current
        .parent()
        .ok_or_else(|| anyhow::anyhow!("current executable has no parent directory"))?;
    let executable = host_executable(directory);
    if !executable.is_file() {
        bail!(
            "resident host executable is missing: {}",
            executable.display()
        );
    }
    spawn_resident_host(&executable, token_path)
        .with_context(|| format!("start {}", executable.display()))
}

fn host_executable(directory: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        directory.join("lgk-vector-host.exe")
    }
    #[cfg(not(windows))]
    {
        directory.join("lgk-vector-host")
    }
}

#[cfg(windows)]
fn spawn_resident_host(executable: &Path, token_path: &Path) -> Result<()> {
    use std::ffi::c_void;

    type Handle = *mut c_void;

    #[repr(C)]
    struct StartupInfoW {
        cb: u32,
        reserved: *mut u16,
        desktop: *mut u16,
        title: *mut u16,
        x: u32,
        y: u32,
        x_size: u32,
        y_size: u32,
        x_count_chars: u32,
        y_count_chars: u32,
        fill_attribute: u32,
        flags: u32,
        show_window: u16,
        reserved2: u16,
        cb_reserved2: *mut u8,
        standard_input: Handle,
        standard_output: Handle,
        standard_error: Handle,
    }

    #[repr(C)]
    struct ProcessInformation {
        process: Handle,
        thread: Handle,
        process_id: u32,
        thread_id: u32,
    }

    const STARTF_USESTDHANDLES: u32 = 0x0000_0100;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn CreateProcessW(
            application_name: *const u16,
            command_line: *mut u16,
            process_attributes: *const c_void,
            thread_attributes: *const c_void,
            inherit_handles: i32,
            creation_flags: u32,
            environment: *const c_void,
            current_directory: *const u16,
            startup_info: *mut StartupInfoW,
            process_information: *mut ProcessInformation,
        ) -> i32;
        fn CloseHandle(handle: Handle) -> i32;
        fn GetLastError() -> u32;
    }

    // bInheritHandles 必须为 FALSE。Rust 标准库的 spawn 无法关闭句柄继承：
    // 调用链上任何标记为可继承的管道（PowerShell 捕获管道、bash/MSYS 管道、
    // .NET 重定向管道）都会随子进程进入常驻 Host。Host 永不退出，调用方的
    // 管道就永远等不到 EOF，整条 shell 管道在所有前台进程退出后假死
    // （2026-08 Git Bash 管道事故；v0.3.0 只清理了 CLI 自身三个标准流句柄，
    // 覆盖不到这些更外层的继承来源）。因此这里绕过标准库直接 CreateProcessW：
    // Host 不继承任何句柄，三个标准流显式为空，配合 DETACHED_PROCESS |
    // CREATE_NEW_PROCESS_GROUP 与调用方控制台/进程组彻底分离。
    let mut command_line = windows_command_line(&[
        executable,
        Path::new("--port"),
        Path::new(&DEFAULT_HOST_PORT.to_string()),
        Path::new("--token-file"),
        token_path,
    ]);
    let mut startup_info = StartupInfoW {
        cb: std::mem::size_of::<StartupInfoW>() as u32,
        reserved: std::ptr::null_mut(),
        desktop: std::ptr::null_mut(),
        title: std::ptr::null_mut(),
        x: 0,
        y: 0,
        x_size: 0,
        y_size: 0,
        x_count_chars: 0,
        y_count_chars: 0,
        fill_attribute: 0,
        // Host 是无控制台的常驻进程，不读写任何标准流。
        flags: STARTF_USESTDHANDLES,
        show_window: 0,
        reserved2: 0,
        cb_reserved2: std::ptr::null_mut(),
        standard_input: std::ptr::null_mut(),
        standard_output: std::ptr::null_mut(),
        standard_error: std::ptr::null_mut(),
    };
    let mut process_information = ProcessInformation {
        process: std::ptr::null_mut(),
        thread: std::ptr::null_mut(),
        process_id: 0,
        thread_id: 0,
    };
    let created = unsafe {
        CreateProcessW(
            std::ptr::null(),
            command_line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            // FALSE：Host 不继承 CLI 进程句柄表中的任何句柄。
            0,
            DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP,
            std::ptr::null(),
            std::ptr::null(),
            &mut startup_info,
            &mut process_information,
        )
    };
    if created == 0 {
        let code = unsafe { GetLastError() };
        bail!(
            "CreateProcessW failed for the resident host: {}",
            std::io::Error::from_raw_os_error(code as i32)
        );
    }
    // SAFETY: CreateProcessW 成功后句柄必然有效，且本进程不再使用它们。
    unsafe {
        CloseHandle(process_information.process);
        CloseHandle(process_information.thread);
    }
    Ok(())
}

#[cfg(windows)]
fn windows_command_line(arguments: &[&Path]) -> Vec<u16> {
    let mut command_line = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        if index > 0 {
            command_line.push(u16::from(b' '));
        }
        push_windows_argument(&mut command_line, argument);
    }
    command_line.push(0);
    command_line
}

#[cfg(windows)]
fn push_windows_argument(command_line: &mut Vec<u16>, argument: &Path) {
    use std::os::windows::ffi::OsStrExt;

    let wide: Vec<u16> = argument.as_os_str().encode_wide().collect();
    let needs_quotes = wide.is_empty()
        || wide.iter().any(|&character| {
            character == u16::from(b' ')
                || character == u16::from(b'\t')
                || character == u16::from(b'"')
        });
    if !needs_quotes {
        command_line.extend_from_slice(&wide);
        return;
    }
    // MSVCRT argv 规则：整体加引号；引号前的反斜杠翻倍并转义引号，结尾
    // 反斜杠翻倍，其余反斜杠保持原样。
    command_line.push(u16::from(b'"'));
    let mut backslashes = 0usize;
    for &character in &wide {
        if character == u16::from(b'\\') {
            backslashes += 1;
            continue;
        }
        let escaped_backslashes = if character == u16::from(b'"') {
            backslashes * 2 + 1
        } else {
            backslashes
        };
        for _ in 0..escaped_backslashes {
            command_line.push(u16::from(b'\\'));
        }
        command_line.push(character);
        backslashes = 0;
    }
    for _ in 0..(backslashes * 2) {
        command_line.push(u16::from(b'\\'));
    }
    command_line.push(u16::from(b'"'));
}

#[cfg(not(windows))]
fn spawn_resident_host(executable: &Path, token_path: &Path) -> Result<()> {
    use std::process::{Command, Stdio};

    let mut command = Command::new(executable);
    command
        .arg("--port")
        .arg(DEFAULT_HOST_PORT.to_string())
        .arg("--token-file")
        .arg(token_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    command
        .spawn()
        .with_context(|| format!("start {}", executable.display()))?;
    Ok(())
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;

    fn decode(line: &[u16]) -> String {
        String::from_utf16(&line[..line.len() - 1]).expect("command line is valid UTF-16")
    }

    #[test]
    fn windows_command_line_quotes_paths_with_spaces() {
        let line = windows_command_line(&[
            Path::new(r"C:\Tools\LGK Vector\lgk-vector-host.exe"),
            Path::new("--port"),
            Path::new("32483"),
            Path::new("--token-file"),
            Path::new(r"C:\Users\lgk user\.lgk-vector\host.token"),
        ]);
        assert_eq!(
            decode(&line),
            "\"C:\\Tools\\LGK Vector\\lgk-vector-host.exe\" --port 32483 --token-file \"C:\\Users\\lgk user\\.lgk-vector\\host.token\""
        );
    }

    #[test]
    fn windows_command_line_doubles_trailing_backslashes() {
        let line = windows_command_line(&[Path::new(r"C:\tools\host dir\")]);
        assert_eq!(decode(&line), "\"C:\\tools\\host dir\\\\\"");
    }
}
