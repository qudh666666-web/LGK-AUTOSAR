//! Compact JSON details inside the existing nonzero-exit error channel.
//! Inspired by structured field errors in claude-autosar; independently
//! implemented for LGK-Vector's existing Rust/Host/PowerShell protocol.
use serde_json::{json, Value};

pub(crate) fn failure(code: &str, error: &str, field: &str, details: Value) -> anyhow::Error {
    anyhow::anyhow!(json!({
        "success": false,
        "code": code,
        "error": error,
        "field": field,
        "details": details
    })
    .to_string())
}
