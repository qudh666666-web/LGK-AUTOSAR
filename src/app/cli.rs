use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{bail, Context, Result};
use rand::RngCore;
use serde_json::Value;

use crate::app::{token_file, HostRequest, HostResponse, DEFAULT_HOST_PORT};
use crate::project::SessionConfig;

// --start-host 使用的入口：创建 Token，必要时启动后台 Host，并等待监听就绪。
pub fn ensure_host() -> Result<()> {
    let token_path = token_file();
    let _ = load_or_create_token(&token_path)?;
    if host_is_listening() {
        return Ok(());
    }
    start_host(&token_path)?;
    wait_for_host_listener()
}

pub fn run(project_directory: &Path, raw_request: &str) -> Result<String> {
    // CLI 不处理 ECUC 逻辑；它把已校验的工程配置、请求和 Token 一起转发给 Host。
    let config = SessionConfig::load(project_directory)?;
    let shutdown = is_shutdown_request(raw_request)?;
    let token_path = token_file();
    let launch_key = load_or_create_token(&token_path)?;

    match send_request(&config, raw_request, &launch_key, shutdown) {
        Ok(response) => response_text(response),
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

fn wait_for_host_listener() -> Result<()> {
    // 后台进程创建和端口监听是两个时刻，因此轮询直到 Host 真正可连接。
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if host_is_listening() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(100));
    }
    bail!("resident host did not become ready")
}

fn host_is_listening() -> bool {
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_HOST_PORT);
    TcpStream::connect_timeout(&address, Duration::from_millis(250)).is_ok()
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
    config: &SessionConfig,
    raw: &str,
    launch_key: &str,
    shutdown: bool,
) -> Result<HostResponse> {
    // HostRequest 是本机 TCP 上传递的外层信封；raw_text 才是用户原始 JSON 请求。
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), DEFAULT_HOST_PORT);
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(1))
        .with_context(|| format!("resident host is not running at {address}"))?;
    stream.set_read_timeout(Some(Duration::from_secs(3600)))?;
    let request = HostRequest {
        raw_text: raw.to_string(),
        cfg: config.clone(),
        launch_key: launch_key.to_string(),
        shutdown,
    };
    serde_json::to_writer(&mut stream, &request)?;
    writeln!(stream)?;
    stream.flush()?;

    let mut response = String::new();
    BufReader::new(stream).read_line(&mut response)?;
    if response.trim().is_empty() {
        bail!("resident host returned an empty response");
    }
    serde_json::from_str(response.trim()).context("invalid resident host response")
}

fn response_text(response: HostResponse) -> Result<String> {
    if response.exit_code != 0 {
        bail!("{}", response.stderr);
    }
    Ok(response.stdout)
}

fn load_or_create_token(path: &Path) -> Result<String> {
    // Token 保存在 EXE 旁边，CLI 与 Host 通过同一值确认彼此属于同一个本地安装。
    if let Ok(value) = fs::read_to_string(path) {
        let value = value.trim();
        if value.len() >= 32 {
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
            Ok(value.trim().to_string())
        }
        Err(error) => Err(error.into()),
    }
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
    let mut command = Command::new(&executable);
    command
        .arg("--port")
        .arg(DEFAULT_HOST_PORT.to_string())
        .arg("--token-file")
        .arg(token_path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    detach_resident_host(&mut command);
    command
        .spawn()
        .with_context(|| format!("start {}", executable.display()))?;
    Ok(())
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
fn detach_resident_host(command: &mut Command) {
    use std::os::windows::process::CommandExt;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    command.creation_flags(CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn detach_resident_host(_command: &mut Command) {}
