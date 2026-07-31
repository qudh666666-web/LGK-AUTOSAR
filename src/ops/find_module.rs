use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::project::SessionConfig;
use crate::vector::module_index::ModuleIndex;
use crate::vector::template_index::read_module_definition_ref;

pub fn execute(config: &SessionConfig, request: &Value) -> Result<Value> {
    let module = request
        .get("module")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("module is required"))?;
    let index = ModuleIndex::load(config)?;
    if module.eq_ignore_ascii_case("all") {
        return Ok(serde_json::to_value(index.all())?);
    }
    if module.contains('/') || module.contains('\\') {
        bail!("module must be a short module name");
    }
    let found = index.find(module)?;
    let definition_ref = read_module_definition_ref(&found.config_path, &found.module)?;
    Ok(json!({
        "module": found.module,
        "name": found.name,
        "configPath": found.config_path,
        "definition_ref": definition_ref,
    }))
}
