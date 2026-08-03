pub mod cli;
pub mod host;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::project::SessionConfig;

// CLI 和 Host 共用的固定本机入口；DaVinci 内 Groovy 使用随机端口。
pub const DEFAULT_HOST_PORT: u16 = 32483;

// CLI 发给 Host 的本地 TCP 信封：业务请求、已校验配置和 Token 一起传递。
#[derive(Debug, Serialize, Deserialize)]
pub struct HostRequest {
    pub raw_text: String,
    pub cfg: SessionConfig,
    pub launch_key: String,
    pub shutdown: bool,
}

// Host 总是把成功输出和失败信息分开返回，CLI 再决定是否以非零退出。
#[derive(Debug, Serialize, Deserialize)]
pub struct HostResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn token_file() -> PathBuf {
    // Token 放在 EXE 目录旁边，使包装器、CLI 和 Host 能够稳定共享它。
    let base = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".lgk-vector").join("host.token")
}
