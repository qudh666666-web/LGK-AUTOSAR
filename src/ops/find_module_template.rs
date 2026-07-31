use anyhow::Result;
use serde_json::{json, Value};

use crate::project::SessionConfig;
use crate::vector::param_definition_index::ParamDefinitionIndex;
use crate::vector::template_index::TemplateIndex;

pub fn execute(config: &SessionConfig, request: &Value) -> Result<Value> {
    let module = request
        .get("module")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("module is required"))?;
    let template = TemplateIndex::load(config, module)?;
    let definitions = ParamDefinitionIndex::load(&template)?;

    let containers = definitions
        .all()
        .iter()
        .filter(|item| item.group == "containers")
        .count();
    let parameters = definitions
        .all()
        .iter()
        .filter(|item| item.group == "parameters")
        .count();
    let references = definitions
        .all()
        .iter()
        .filter(|item| item.group == "references")
        .count();

    Ok(json!({
        "module": template.module,
        "definition_ref": template.definition_ref,
        "template_path": template.path,
        "counts": {
            "containers": containers,
            "parameters": parameters,
            "references": references,
        },
        "definitions": definitions.all(),
    }))
}
