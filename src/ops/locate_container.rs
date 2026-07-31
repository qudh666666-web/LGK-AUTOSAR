use std::fs;

use anyhow::{Context, Result};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};

use crate::project::SessionConfig;
use crate::vector::module_index::ModuleIndex;

#[derive(Debug)]
struct Frame {
    start_line: usize,
    short_name: Option<String>,
    definition_ref: Option<String>,
}

#[derive(Debug, Serialize)]
struct Match {
    short_name: String,
    definition_ref: String,
    container_path: String,
    start_line: usize,
    end_line: usize,
}

pub fn execute(config: &SessionConfig, request: &Value) -> Result<Value> {
    let module = required_string(request, "module")?;
    let definition_ref = required_string(request, "definition_ref")?;
    let short_name_regex = request
        .get("short_name_regex")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim();
    let patterns = compile_patterns(short_name_regex)?;

    let modules = ModuleIndex::load(config)?;
    let module_info = modules.find(module)?;
    let raw = fs::read_to_string(&module_info.config_path).with_context(|| {
        format!(
            "failed to read config file: {}",
            module_info.config_path.display()
        )
    })?;
    let short_name = Regex::new(r"<SHORT-NAME>([^<]+)</SHORT-NAME>")?;
    let definition = Regex::new(r#"<DEFINITION-REF(?:\s+[^>]*)?>([^<]+)</DEFINITION-REF>"#)?;

    let mut stack: Vec<Frame> = Vec::new();
    let mut matches = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let line_number = index + 1;
        if line.contains("<ECUC-CONTAINER-VALUE") {
            stack.push(Frame {
                start_line: line_number,
                short_name: None,
                definition_ref: None,
            });
        }

        if let Some(frame) = stack.last_mut() {
            if frame.short_name.is_none() {
                frame.short_name = short_name
                    .captures(line)
                    .and_then(|captures| captures.get(1))
                    .map(|value| value.as_str().trim().to_string());
            }
            if frame.definition_ref.is_none() {
                frame.definition_ref = definition
                    .captures(line)
                    .and_then(|captures| captures.get(1))
                    .map(|value| value.as_str().trim().to_string());
            }
        }

        if line.contains("</ECUC-CONTAINER-VALUE>") {
            let Some(frame) = stack.pop() else {
                continue;
            };
            let Some(found_ref) = frame.definition_ref else {
                continue;
            };
            let found_name = frame.short_name.unwrap_or_default();
            if found_ref != definition_ref || !matches_name(&patterns, &found_name) {
                continue;
            }
            let mut path = stack
                .iter()
                .filter_map(|parent| parent.short_name.as_deref())
                .map(str::to_string)
                .collect::<Vec<_>>();
            path.push(found_name.clone());
            matches.push(Match {
                short_name: found_name,
                definition_ref: found_ref,
                container_path: path.join("/"),
                start_line: frame.start_line,
                end_line: line_number,
            });
        }
    }

    Ok(json!({
        "module": module,
        "configPath": module_info.config_path,
        "definition_ref": definition_ref,
        "count": matches.len(),
        "containers": matches,
    }))
}

fn required_string<'a>(request: &'a Value, name: &str) -> Result<&'a str> {
    request
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required"))
}

fn compile_patterns(raw: &str) -> Result<Vec<Regex>> {
    if raw.is_empty() {
        return Ok(Vec::new());
    }
    raw.split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| Regex::new(part).with_context(|| format!("invalid short_name_regex '{part}'")))
        .collect()
}

fn matches_name(patterns: &[Regex], name: &str) -> bool {
    patterns.is_empty() || patterns.iter().any(|pattern| pattern.is_match(name))
}
