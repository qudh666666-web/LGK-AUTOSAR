//! Compact semantic diff for two ECUC ARXML snapshots inside one project.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use serde_json::{json, Value};
use xmltree::{Element, XMLNode};

use crate::project::SessionConfig;
use crate::vector::search::child_text;

const DEFAULT_LIMIT: usize = 32;
const MAX_LIMIT: usize = 256;
const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_VALUE_CHARS: usize = 512;

#[derive(Clone, Debug, Serialize)]
struct EcucValue {
    value: String,
    kind: &'static str,
}

#[derive(Debug, Serialize)]
struct Change {
    op: &'static str,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    old: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    new: Option<String>,
    #[serde(skip_serializing_if = "is_false")]
    old_truncated: bool,
    #[serde(skip_serializing_if = "is_false")]
    new_truncated: bool,
    kind: &'static str,
}

pub fn execute(config: &SessionConfig, request: &Value) -> Result<Value> {
    let module = super::required_module(request)?;
    if module.eq_ignore_ascii_case("all") {
        bail!("diff_ecuc requires one concrete module");
    }
    let left = project_file(config, required_string(request, "left")?)?;
    let right = project_file(config, required_string(request, "right")?)?;
    let prefix = request
        .get("path_prefix")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let limit = request
        .get("limit")
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| (1..=MAX_LIMIT).contains(value))
                .ok_or_else(|| anyhow::anyhow!("limit must be an integer from 1 to {MAX_LIMIT}"))
        })
        .transpose()?
        .unwrap_or(DEFAULT_LIMIT);

    let left_values = load_values(&left, module)?;
    let right_values = load_values(&right, module)?;
    let mut changes = Vec::new();
    for (path, old) in &left_values {
        if prefix.is_some_and(|prefix| !path.starts_with(prefix)) {
            continue;
        }
        match right_values.get(path) {
            None => {
                let (old_value, old_truncated) = bounded_value(&old.value);
                changes.push(Change {
                    op: "delete",
                    path: path.clone(),
                    old: Some(old_value),
                    new: None,
                    old_truncated,
                    new_truncated: false,
                    kind: old.kind,
                });
            }
            Some(new) if old.value != new.value || old.kind != new.kind => {
                let (old_value, old_truncated) = bounded_value(&old.value);
                let (new_value, new_truncated) = bounded_value(&new.value);
                changes.push(Change {
                    op: "modify",
                    path: path.clone(),
                    old: Some(old_value),
                    new: Some(new_value),
                    old_truncated,
                    new_truncated,
                    kind: new.kind,
                });
            }
            Some(_) => {}
        }
    }
    for (path, new) in &right_values {
        if !left_values.contains_key(path) && prefix.is_none_or(|prefix| path.starts_with(prefix)) {
            let (new_value, new_truncated) = bounded_value(&new.value);
            changes.push(Change {
                op: "add",
                path: path.clone(),
                old: None,
                new: Some(new_value),
                old_truncated: false,
                new_truncated,
                kind: new.kind,
            });
        }
    }
    changes.sort_by(|a, b| a.path.cmp(&b.path).then(a.op.cmp(b.op)));
    let counts = json!({
        "add": changes.iter().filter(|change| change.op == "add").count(),
        "modify": changes.iter().filter(|change| change.op == "modify").count(),
        "delete": changes.iter().filter(|change| change.op == "delete").count(),
    });
    let total = changes.len();
    changes.truncate(limit);
    Ok(json!({
        "module": module,
        "left": left,
        "right": right,
        "path_prefix": prefix,
        "total": total,
        "counts": counts,
        "changes": changes,
        "truncated": total > limit,
        "limit": limit,
    }))
}

fn bounded_value(value: &str) -> (String, bool) {
    if value.chars().count() <= MAX_VALUE_CHARS {
        return (value.to_string(), false);
    }
    (value.chars().take(MAX_VALUE_CHARS).collect(), true)
}

fn is_false(value: &bool) -> bool {
    !*value
}

fn required_string<'a>(request: &'a Value, key: &str) -> Result<&'a str> {
    request
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} must be a non-empty path"))
}

fn project_file(config: &SessionConfig, value: &str) -> Result<PathBuf> {
    let path = Path::new(value);
    let joined = if path.is_absolute() {
        path.to_path_buf()
    } else {
        config.project_path.join(path)
    };
    let path = config.ensure_project_file(&joined)?;
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("arxml"))
    {
        bail!(
            "diff_ecuc inputs must have an .arxml extension: {}",
            path.display()
        );
    }
    if fs::metadata(&path)?.len() > MAX_FILE_SIZE {
        bail!("diff_ecuc input exceeds 64 MiB: {}", path.display());
    }
    Ok(path)
}

fn load_values(path: &Path, module: &str) -> Result<BTreeMap<String, EcucValue>> {
    let root = Element::parse(
        fs::File::open(path).with_context(|| format!("cannot open ARXML: {}", path.display()))?,
    )
    .with_context(|| format!("cannot parse ARXML: {}", path.display()))?;
    let module_root = find_module(&root, module)
        .ok_or_else(|| anyhow::anyhow!("module {module} was not found in {}", path.display()))?;
    let actual_module = child_text(module_root, "SHORT-NAME").unwrap_or_else(|| module.to_string());
    let mut values = BTreeMap::new();
    walk(module_root, &actual_module, &mut values, path)?;
    Ok(values)
}

fn find_module<'a>(element: &'a Element, module: &str) -> Option<&'a Element> {
    if element.name == "ECUC-MODULE-CONFIGURATION-VALUES"
        && child_text(element, "SHORT-NAME").is_some_and(|name| name.eq_ignore_ascii_case(module))
    {
        return Some(element);
    }
    element.children.iter().find_map(|node| match node {
        XMLNode::Element(child) => find_module(child, module),
        _ => None,
    })
}

fn walk(
    element: &Element,
    container_path: &str,
    values: &mut BTreeMap<String, EcucValue>,
    source: &Path,
) -> Result<()> {
    for node in &element.children {
        let XMLNode::Element(child) = node else {
            continue;
        };
        if child.name == "ECUC-CONTAINER-VALUE" {
            let Some(name) = child_text(child, "SHORT-NAME") else {
                continue;
            };
            walk(child, &format!("{container_path}/{name}"), values, source)?;
        } else if child.name.ends_with("-PARAM-VALUE") {
            insert_leaf(child, container_path, "parameter", "VALUE", values, source)?;
        } else if child.name == "ECUC-REFERENCE-VALUE" {
            insert_leaf(
                child,
                container_path,
                "reference",
                "VALUE-REF",
                values,
                source,
            )?;
        } else {
            walk(child, container_path, values, source)?;
        }
    }
    Ok(())
}

fn insert_leaf(
    element: &Element,
    container_path: &str,
    kind: &'static str,
    value_tag: &str,
    values: &mut BTreeMap<String, EcucValue>,
    source: &Path,
) -> Result<()> {
    let Some(definition) = child_text(element, "DEFINITION-REF") else {
        return Ok(());
    };
    let Some(name) = definition.rsplit('/').find(|part| !part.is_empty()) else {
        return Ok(());
    };
    let value = child_text(element, value_tag).unwrap_or_default();
    let path = format!("{container_path}/{name}");
    if values
        .insert(path.clone(), EcucValue { value, kind })
        .is_some()
    {
        return Err(crate::diagnostic::failure(
            "ECUC_DIFF_AMBIGUOUS_PATH",
            "duplicate semantic path cannot be compared safely",
            "left/right",
            json!({"path": path, "source": source}),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_values_by_characters_without_splitting_utf8() {
        let input = "中".repeat(MAX_VALUE_CHARS + 1);
        let (value, truncated) = bounded_value(&input);
        assert!(truncated);
        assert_eq!(value.chars().count(), MAX_VALUE_CHARS);
        assert!(value.chars().all(|character| character == '中'));
    }
}
