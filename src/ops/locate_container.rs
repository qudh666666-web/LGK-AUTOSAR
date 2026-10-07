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
    let (limit, offset) = validate_request(request)?;
    let module = super::required_module(request)?;
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
    // Parse container boundaries and identifying child tags as a token stream over
    // the whole document. DaVinci and vendor tools may wrap XML tags across lines,
    // so a line-by-line regex silently misses otherwise valid containers.
    let tokens = Regex::new(
        r#"(?s)(?P<open><ECUC-CONTAINER-VALUE(?:\s+[^>]*)?>)|(?P<close></ECUC-CONTAINER-VALUE\s*>)|(?P<short><SHORT-NAME(?:\s+[^>]*)?>\s*(?P<short_value>[^<]+?)\s*</SHORT-NAME\s*>)|(?P<definition><DEFINITION-REF(?:\s+[^>]*)?>\s*(?P<definition_value>[^<]+?)\s*</DEFINITION-REF\s*>)"#,
    )?;
    let line_starts = std::iter::once(0)
        .chain(raw.match_indices('\n').map(|(index, _)| index + 1))
        .collect::<Vec<_>>();

    let mut stack: Vec<Frame> = Vec::new();
    let mut matches = Vec::new();
    for captures in tokens.captures_iter(&raw) {
        let token = captures.get(0).expect("token capture");
        let line_number = line_number_at(&line_starts, token.start());
        if captures.name("open").is_some() {
            stack.push(Frame {
                start_line: line_number,
                short_name: None,
                definition_ref: None,
            });
        }

        if let Some(frame) = stack.last_mut() {
            if frame.short_name.is_none() {
                frame.short_name = captures
                    .name("short_value")
                    .map(|value| value.as_str().trim().to_string());
            }
            if frame.definition_ref.is_none() {
                frame.definition_ref = captures
                    .name("definition_value")
                    .map(|value| value.as_str().trim().to_string());
            }
        }

        if captures.name("close").is_some() {
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

    let count = matches.len();
    let page = matches
        .into_iter()
        .skip(offset)
        .take(limit)
        .collect::<Vec<_>>();
    let next = offset.saturating_add(page.len());
    Ok(json!({
        "module": module,
        "configPath": module_info.config_path,
        "definition_ref": definition_ref,
        "count": count,
        "containers": page,
        "limit": limit,
        "offset": offset,
        "truncated": next < count,
        "next_offset": if next < count { Some(next) } else { None },
    }))
}

/// Small, locally available contract; no project or Host is needed to read it.
pub fn contract() -> Value {
    json!({
        "func": "locate_container",
        "required": ["module", "definition_ref"],
        "optional": {"module_name": "alias for module", "short_name_regex": "string; comma-separated regex alternatives", "limit": "integer 1..256, default 32", "offset": "nonnegative integer, default 0"},
        "example": {"func": "locate_container", "module": "Com", "definition_ref": "/MICROSAR/Com/ComConfig/ComSignal", "short_name_regex": "^VehicleSpeed$", "limit": 8},
        "notes": "Use the real definition_ref from find_module_template. query and path are not filters. count is the full match count; next_offset continues the page."
    })
}

fn validate_request(request: &Value) -> Result<(usize, usize)> {
    let mut issues = Vec::new();
    for field in ["module", "definition_ref"] {
        let value = if field == "module" {
            request.get(field).or_else(|| request.get("module_name"))
        } else {
            request.get(field)
        };
        if !value
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty())
        {
            issues.push(json!({"field": field, "reason": "nonempty string required"}));
        }
    }
    if let Some(fields) = request.as_object() {
        for field in fields.keys() {
            if ![
                "func",
                "module",
                "module_name",
                "definition_ref",
                "short_name_regex",
                "limit",
                "offset",
            ]
            .contains(&field.as_str())
            {
                issues.push(json!({"field": field, "reason": "unsupported field"}));
            }
        }
    }
    if let Some(value) = request.get("short_name_regex") {
        if !value.is_string() {
            issues.push(json!({"field": "short_name_regex", "reason": "string required"}));
        }
    }
    let limit = request.get("limit").map_or(Some(32), Value::as_u64);
    let offset = request.get("offset").map_or(Some(0), Value::as_u64);
    if !limit.is_some_and(|n| (1..=256).contains(&n)) {
        issues.push(json!({"field": "limit", "reason": "integer 1..256 required"}));
    }
    if !offset.is_some_and(|n| usize::try_from(n).is_ok()) {
        issues.push(
            json!({"field": "offset", "reason": "nonnegative platform-sized integer required"}),
        );
    }
    if !issues.is_empty() {
        let total = issues.len();
        issues.truncate(8);
        return Err(crate::diagnostic::failure(
            "INVALID_REQUEST",
            "invalid locate_container request; fix fields together",
            "request",
            json!({"issues": issues, "issue_count": total, "contract": contract()}),
        ));
    }
    Ok((limit.unwrap() as usize, offset.unwrap() as usize))
}

fn line_number_at(line_starts: &[usize], byte_offset: usize) -> usize {
    line_starts.partition_point(|start| *start <= byte_offset)
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
