use anyhow::{bail, Result};
use serde_json::{json, Value};

use crate::daemon::client::DaVinciClient;
use crate::ops;
use crate::project::SessionConfig;
use crate::vector::module_index::ModuleIndex;
use crate::vector::template_index::read_module_definition_ref;

pub struct CommandDispatcher {
    davinci: Option<DaVinciClient>,
}

impl CommandDispatcher {
    pub fn new() -> Self {
        Self { davinci: None }
    }

    pub fn dispatch_batch(&mut self, config: &SessionConfig, raw: &str) -> Result<Value> {
        let parsed: Value =
            serde_json::from_str(raw).map_err(|error| anyhow::anyhow!("invalid JSON: {error}"))?;
        match &parsed {
            Value::Array(items) => {
                if items.is_empty() {
                    bail!("request array must not be empty");
                }
                let results = items
                    .iter()
                    .map(|item| self.dispatch_one(config, item))
                    .collect::<Result<Vec<_>>>()?;
                Ok(Value::Array(results))
            }
            Value::Object(_) => self.dispatch_one(config, &parsed),
            _ => bail!("request must be a JSON object or array"),
        }
    }

    pub fn shutdown(mut self) -> Result<()> {
        if let Some(client) = self.davinci.take() {
            let _ = client.shutdown();
        }
        Ok(())
    }

    fn dispatch_one(&mut self, config: &SessionConfig, request: &Value) -> Result<Value> {
        let func = request
            .get("func")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("func is required"))?;
        match func {
            "find_module" | "find_bsw_module" => ops::find_module::execute(config, request),
            "find_module_template" | "get_bsw_module_template" => {
                ops::find_module_template::execute(config, request)
            }
            "get_param_definition" | "get_bsw_param_definition" => {
                ops::get_param_definition::execute(config, request)
            }
            "locate_container" => ops::locate_container::execute(config, request),
            "edit_file" => ops::edit_file::execute(config, request),
            "get_errors_list" => {
                let module = optional_module(request);
                self.davinci(config)?.list_errors(module)
            }
            "auto_solve_errors" => {
                if request.get("confirmed").and_then(Value::as_bool) != Some(true) {
                    bail!(
                        "auto_solve_errors requires confirmed=true after reviewing a fresh error list"
                    );
                }
                let module = optional_module(request);
                let targets = request.get("targets").and_then(Value::as_str);
                let message = self.davinci(config)?.solve_errors(module, targets)?;
                Ok(json!({"module": module, "message": message}))
            }
            "generate_code" => {
                let module = optional_module(request);
                let definition_ref = if module.eq_ignore_ascii_case("all") {
                    None
                } else {
                    let modules = ModuleIndex::load(config)?;
                    let module_info = modules.find(module)?;
                    Some(read_module_definition_ref(
                        &module_info.config_path,
                        &module_info.module,
                    )?)
                };
                let message = self
                    .davinci(config)?
                    .generate(module, definition_ref.as_deref())?;
                Ok(json!({
                    "module": module,
                    "definition_ref": definition_ref,
                    "message": message
                }))
            }
            "shutdown_host" => {
                bail!("shutdown_host is handled by the resident host protocol")
            }
            _ => bail!("unsupported func: {func}"),
        }
    }

    fn davinci(&mut self, config: &SessionConfig) -> Result<&DaVinciClient> {
        if self.davinci.is_none() {
            self.davinci = Some(DaVinciClient::start(config)?);
        }
        Ok(self.davinci.as_ref().expect("initialized"))
    }
}

fn optional_module(request: &Value) -> &str {
    request
        .get("module")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("all")
}
