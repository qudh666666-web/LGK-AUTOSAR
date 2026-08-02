use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::path::Path;

use anyhow::{Context, Result};

use crate::app::{HostRequest, HostResponse};
use crate::daemon::commands::CommandDispatcher;
use crate::project::SessionConfig;

// Resident Host 只服务本机的一个 DaVinci 工程会话。
// 这样连续请求可复用 DaVinci，避免每次查询或生成都冷启动。
pub fn run(port: u16, token_path: &Path) -> Result<()> {
    let token = fs::read_to_string(token_path)
        .with_context(|| format!("read token file: {}", token_path.display()))?;
    let token = token.trim().to_string();
    let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
    let listener =
        TcpListener::bind(address).with_context(|| format!("bind resident server: {address}"))?;
    // 第一个有效请求绑定工程；会话期间拒绝切换工程，防止串项目。
    let mut active_config: Option<SessionConfig> = None;
    // Dispatcher 持有惰性创建的 DaVinciClient。
    let mut dispatcher = Some(CommandDispatcher::new());

    // 每条 TCP 连接只承载一个 JSON 请求和一个 JSON 响应。
    for incoming in listener.incoming() {
        let mut stream = match incoming {
            Ok(stream) => stream,
            Err(_) => continue,
        };
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        let request = match read_request(&mut stream) {
            Ok(request) => request,
            Err(error) => {
                let _ = write_response(
                    &mut stream,
                    HostResponse {
                        exit_code: 2,
                        stdout: String::new(),
                        stderr: format!("invalid resident request: {error}"),
                    },
                );
                continue;
            }
        };
        // 端口只绑定 127.0.0.1，Token 再限制为包装器创建的本地会话。
        if request.launch_key != token {
            write_response(
                &mut stream,
                HostResponse {
                    exit_code: 3,
                    stdout: String::new(),
                    stderr: "invalid launch key".to_string(),
                },
            )?;
            continue;
        }
        if let Some(active) = &active_config {
            if active != &request.cfg {
                write_response(
                    &mut stream,
                    HostResponse {
                        exit_code: 4,
                        stdout: String::new(),
                        stderr: format!(
                            "resident host is already bound to another project: active={}, requested={}",
                            active.project_path.display(),
                            request.cfg.project_path.display()
                        ),
                    },
                )?;
                continue;
            }
        } else {
            active_config = Some(request.cfg.clone());
        }

        // shutdown 会先让 Dispatcher 正常退出 DaVinci，再结束 Host 主循环。
        if request.shutdown {
            let result = dispatcher.take().expect("dispatcher available").shutdown();
            let response = match result {
                Ok(()) => HostResponse {
                    exit_code: 0,
                    stdout: serde_json::json!({"status": "shutdown"}).to_string(),
                    stderr: String::new(),
                },
                Err(error) => HostResponse {
                    exit_code: 1,
                    stdout: String::new(),
                    stderr: error.to_string(),
                },
            };
            write_response(&mut stream, response)?;
            break;
        }

        // 普通请求交给命令分发器：本地查询不会启动 DaVinci，
        // 只有生成/校验类请求才会按需创建 DaVinciClient。
        let response = match dispatcher
            .as_mut()
            .expect("dispatcher available")
            .dispatch_batch(&request.cfg, &request.raw_text)
        {
            Ok(value) => HostResponse {
                exit_code: 0,
                stdout: serde_json::to_string_pretty(&value)?,
                stderr: String::new(),
            },
            Err(error) => HostResponse {
                exit_code: 1,
                stdout: String::new(),
                stderr: error.to_string(),
            },
        };
        write_response(&mut stream, response)?;
    }
    Ok(())
}

fn read_request(stream: &mut TcpStream) -> Result<HostRequest> {
    let mut line = String::new();
    BufReader::new(stream).read_line(&mut line)?;
    serde_json::from_str(line.trim()).context("parse resident request")
}

fn write_response(stream: &mut TcpStream, response: HostResponse) -> Result<()> {
    serde_json::to_writer(&mut *stream, &response)?;
    writeln!(stream)?;
    stream.flush()?;
    Ok(())
}
