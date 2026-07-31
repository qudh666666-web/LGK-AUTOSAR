pub mod cli;
pub mod host;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::project::SessionConfig;

pub const DEFAULT_HOST_PORT: u16 = 32483;

#[derive(Debug, Serialize, Deserialize)]
pub struct HostRequest {
    pub raw_text: String,
    pub cfg: SessionConfig,
    pub launch_key: String,
    pub shutdown: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostResponse {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

pub fn token_file() -> PathBuf {
    let base = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(PathBuf::from))
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    base.join(".autosar-ecuc-bridge").join("host.token")
}
