//! Semantically target one existing ECUC parameter or reference value.
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use xmltree::{Element, XMLNode};

use crate::project::SessionConfig;
use crate::vector::module_index::ModuleIndex;
use crate::vector::search::{child, child_text};

const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_REQUEST_VALUE_CHARS: usize = 4096;
const MAX_RESPONSE_VALUE_CHARS: usize = 512;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FrameKind {
    Module,
    Container,
}

#[derive(Debug)]
struct Frame {
    kind: FrameKind,
    start: usize,
    short_name: Option<String>,
}

#[derive(Clone, Debug)]
struct ContainerSpan {
    module: String,
    path: String,
    start: usize,
    end: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ValueKind {
    Parameter,
    Reference,
}

impl ValueKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Parameter => "parameter",
            Self::Reference => "reference",
        }
    }
}

#[derive(Debug)]
struct LocatedValue {
    start: usize,
    end: usize,
    current: String,
    definition_ref: String,
    kind: ValueKind,
    semantic_path: String,
}

struct SemanticValue {
    current: String,
    definition_ref: String,
}

struct PreparedEdit {
    path: PathBuf,
    original: Vec<u8>,
    updated: Vec<u8>,
    current: String,
    replacement: String,
    definition_ref: String,
    kind: ValueKind,
    semantic_path: String,
    changed: bool,
}

pub fn validate(config: &SessionConfig, request: &Value) -> Result<()> {
    prepare(config, request).map(|_| ())
}

pub fn execute(config: &SessionConfig, request: &Value) -> Result<Value> {
    let prepared = prepare(config, request)?;
    if prepared.changed {
        super::edit_file::replace_checked(&prepared.path, &prepared.original, &prepared.updated)?;
    }
    let (old, old_truncated) = bounded(&prepared.current);
    let (new, new_truncated) = bounded(&prepared.replacement);
    let mut response = json!({
        "changed": prepared.changed,
        "path": prepared.semantic_path,
        "config_path": prepared.path,
        "definition_ref": prepared.definition_ref,
        "kind": prepared.kind.as_str(),
        "old": old,
        "new": new,
    });
    if old_truncated {
        response["old_truncated"] = json!(true);
    }
    if new_truncated {
        response["new_truncated"] = json!(true);
    }
    Ok(response)
}

fn prepare(config: &SessionConfig, request: &Value) -> Result<PreparedEdit> {
    let requested_module = super::required_module(request)?;
    if requested_module.eq_ignore_ascii_case("all") {
        bail!("set_ecuc_value requires one concrete module");
    }
    let container_path = required_trimmed(request, "container_path")?;
    let (kind, target_name) = requested_target(request)?;
    let expected = required_value(request, "expected")?;
    let replacement = required_value(request, "value")?;
    if replacement.chars().count() > MAX_REQUEST_VALUE_CHARS {
        bail!("value must not exceed {MAX_REQUEST_VALUE_CHARS} Unicode characters");
    }

    let modules = ModuleIndex::load(config)?;
    let module = modules.find(requested_module)?;
    let path = config.ensure_project_file(&module.config_path)?;
    ensure_arxml_size(&path)?;
    let original = fs::read(&path)
        .with_context(|| format!("failed to read ECUC configuration: {}", path.display()))?;
    let bom_len = usize::from(original.starts_with(&[0xEF, 0xBB, 0xBF])) * 3;
    let text = std::str::from_utf8(&original[bom_len..])
        .with_context(|| format!("ECUC configuration is not UTF-8: {}", path.display()))?;
    let semantic = semantic_value(text, &module.module, container_path, kind, target_name)?;
    let located = locate_value(text, &module.module, container_path, kind, target_name)?;
    if located.current != semantic.current || located.definition_ref != semantic.definition_ref {
        bail!("raw XML location does not match the parsed semantic target; no edit was attempted");
    }
    if semantic.current != expected {
        let (current, truncated) = bounded(&semantic.current);
        let (expected_value, expected_truncated) = bounded(expected);
        return Err(crate::diagnostic::failure(
            "ECUC_VALUE_PRECONDITION_FAILED",
            "the saved ECUC value does not match expected",
            "expected",
            json!({
                "path": located.semantic_path,
                "expected": expected_value,
                "expected_truncated": expected_truncated,
                "current": current,
                "current_truncated": truncated,
                "next": "Inspect the saved value and submit a new explicit expected value"
            }),
        ));
    }

    let changed = semantic.current != replacement;
    let mut updated = original.clone();
    if changed {
        let escaped = escape_xml_text(replacement);
        updated.splice(
            (bom_len + located.start)..(bom_len + located.end),
            escaped.bytes(),
        );
        let updated_text = std::str::from_utf8(&updated[bom_len..])?;
        let verified = semantic_value(
            updated_text,
            &module.module,
            container_path,
            kind,
            target_name,
        )?;
        if verified.current != replacement || verified.definition_ref != semantic.definition_ref {
            bail!("semantic edit verification failed before replacement");
        }
    }

    Ok(PreparedEdit {
        path,
        original,
        updated,
        current: semantic.current,
        replacement: replacement.to_string(),
        definition_ref: located.definition_ref,
        kind: located.kind,
        semantic_path: format!("{}/{}", module.module, located.semantic_path),
        changed,
    })
}

fn semantic_value(
    text: &str,
    module: &str,
    requested_container_path: &str,
    kind: ValueKind,
    target_name: &str,
) -> Result<SemanticValue> {
    let root = Element::parse(text.as_bytes()).context("invalid ECUC XML")?;
    let mut modules = Vec::new();
    collect_modules(&root, module, &mut modules);
    let module_root = match modules.as_slice() {
        [] => bail!("module {module} was not found in its configured ARXML"),
        [module_root] => *module_root,
        _ => bail!("module {module} occurs more than once in its configured ARXML"),
    };
    let normalized = normalize_container_path(requested_container_path, module);
    let parts = normalized
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        bail!("container_path must identify a container below the module");
    }
    let mut parents = vec![module_root];
    for (depth, part) in parts.iter().enumerate() {
        let mut next = Vec::new();
        for parent in parents {
            let group_name = if depth == 0 {
                "CONTAINERS"
            } else {
                "SUB-CONTAINERS"
            };
            let Some(group) = child(parent, group_name) else {
                continue;
            };
            next.extend(group.children.iter().filter_map(|node| match node {
                XMLNode::Element(container)
                    if container.name == "ECUC-CONTAINER-VALUE"
                        && child_text(container, "SHORT-NAME").as_deref() == Some(*part) =>
                {
                    Some(container)
                }
                _ => None,
            }));
        }
        parents = next;
    }
    let container = match parents.as_slice() {
        [] => {
            return Err(crate::diagnostic::failure(
                "ECUC_CONTAINER_NOT_FOUND",
                "container_path was not found in the selected module",
                "container_path",
                json!({"module": module, "container_path": requested_container_path}),
            ));
        }
        [container] => *container,
        _ => {
            return Err(crate::diagnostic::failure(
                "ECUC_VALUE_AMBIGUOUS",
                "container_path matches more than one saved container",
                "container_path",
                json!({"module": module, "container_path": requested_container_path,
                    "matches": parents.len()}),
            ));
        }
    };
    let group_name = match kind {
        ValueKind::Parameter => "PARAMETER-VALUES",
        ValueKind::Reference => "REFERENCE-VALUES",
    };
    let mut matches = child(container, group_name)
        .into_iter()
        .flat_map(|group| group.children.iter())
        .filter_map(|node| match node {
            XMLNode::Element(value)
                if (kind == ValueKind::Parameter && value.name.ends_with("-PARAM-VALUE"))
                    || (kind == ValueKind::Reference && value.name == "ECUC-REFERENCE-VALUE") =>
            {
                let definition_ref = child_text(value, "DEFINITION-REF")?;
                if !definition_matches(&definition_ref, target_name) {
                    return None;
                }
                let value_tag = if kind == ValueKind::Parameter {
                    "VALUE"
                } else {
                    "VALUE-REF"
                };
                Some(SemanticValue {
                    current: child_text(value, value_tag).unwrap_or_default(),
                    definition_ref,
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Err(crate::diagnostic::failure(
            "ECUC_VALUE_NOT_FOUND",
            "the requested value was not found directly in the container",
            kind.as_str(),
            json!({"module": module, "container_path": requested_container_path,
                "name": target_name, "kind": kind.as_str()}),
        )),
        1 => Ok(matches.remove(0)),
        count => Err(crate::diagnostic::failure(
            "ECUC_VALUE_AMBIGUOUS",
            "the requested value occurs more than once in the container",
            kind.as_str(),
            json!({"module": module, "container_path": requested_container_path,
                "name": target_name, "kind": kind.as_str(), "matches": count}),
        )),
    }
}

fn collect_modules<'a>(element: &'a Element, module: &str, found: &mut Vec<&'a Element>) {
    if element.name == "ECUC-MODULE-CONFIGURATION-VALUES"
        && child_text(element, "SHORT-NAME").is_some_and(|name| name.eq_ignore_ascii_case(module))
    {
        found.push(element);
        return;
    }
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            collect_modules(child, module, found);
        }
    }
}

fn locate_value(
    text: &str,
    module: &str,
    requested_container_path: &str,
    kind: ValueKind,
    target_name: &str,
) -> Result<LocatedValue> {
    let containers = collect_container_spans(text)?;
    let normalized_path = normalize_container_path(requested_container_path, module);
    if normalized_path.is_empty() {
        bail!("container_path must identify a container below the module");
    }
    let matching_containers = containers
        .iter()
        .filter(|span| span.module.eq_ignore_ascii_case(module) && span.path == normalized_path)
        .collect::<Vec<_>>();
    let container = match matching_containers.as_slice() {
        [] => {
            return Err(crate::diagnostic::failure(
                "ECUC_CONTAINER_NOT_FOUND",
                "container_path was not found in the selected module",
                "container_path",
                json!({"module": module, "container_path": requested_container_path}),
            ));
        }
        [container] => *container,
        _ => {
            return Err(crate::diagnostic::failure(
                "ECUC_VALUE_AMBIGUOUS",
                "container_path matches more than one saved container",
                "container_path",
                json!({"module": module, "container_path": requested_container_path,
                    "matches": matching_containers.len()}),
            ));
        }
    };

    let mut matches = collect_leaf_values(text, &containers, kind)?
        .into_iter()
        .filter(|candidate| {
            candidate.0.start == container.start
                && candidate.0.end == container.end
                && definition_matches(&candidate.1.definition_ref, target_name)
        })
        .map(|(_, value)| value)
        .collect::<Vec<_>>();
    match matches.len() {
        0 => Err(crate::diagnostic::failure(
            "ECUC_VALUE_NOT_FOUND",
            "the requested value was not found directly in the container",
            kind.as_str(),
            json!({"module": module, "container_path": requested_container_path,
                "name": target_name, "kind": kind.as_str()}),
        )),
        1 => Ok(matches.remove(0)),
        count => Err(crate::diagnostic::failure(
            "ECUC_VALUE_AMBIGUOUS",
            "the requested value occurs more than once in the container",
            kind.as_str(),
            json!({"module": module, "container_path": requested_container_path,
                "name": target_name, "kind": kind.as_str(), "matches": count}),
        )),
    }
}

fn collect_container_spans(text: &str) -> Result<Vec<ContainerSpan>> {
    let tokens = Regex::new(
        r#"(?s)(?P<module_open><ECUC-MODULE-CONFIGURATION-VALUES(?:\s+[^>]*)?>)|(?P<module_close></ECUC-MODULE-CONFIGURATION-VALUES\s*>)|(?P<container_open><ECUC-CONTAINER-VALUE(?:\s+[^>]*)?>)|(?P<container_close></ECUC-CONTAINER-VALUE\s*>)|(?P<short><SHORT-NAME(?:\s+[^>]*)?>\s*(?P<short_value>[^<]+?)\s*</SHORT-NAME\s*>)"#,
    )?;
    let mut stack = Vec::<Frame>::new();
    let mut spans = Vec::new();
    for captures in tokens.captures_iter(text) {
        let token = captures.get(0).expect("structural token");
        if captures.name("module_open").is_some() {
            stack.push(Frame {
                kind: FrameKind::Module,
                start: token.start(),
                short_name: None,
            });
            continue;
        }
        if captures.name("container_open").is_some() {
            stack.push(Frame {
                kind: FrameKind::Container,
                start: token.start(),
                short_name: None,
            });
            continue;
        }
        if let Some(short_name) = captures.name("short_value") {
            if let Some(frame) = stack.last_mut() {
                if frame.short_name.is_none() {
                    frame.short_name = Some(decode_xml_text(short_name.as_str().trim())?);
                }
            }
            continue;
        }
        let closing_kind = if captures.name("container_close").is_some() {
            Some(FrameKind::Container)
        } else if captures.name("module_close").is_some() {
            Some(FrameKind::Module)
        } else {
            None
        };
        let Some(closing_kind) = closing_kind else {
            continue;
        };
        let Some(frame) = stack.pop() else {
            bail!("unexpected ECUC closing tag while locating semantic value");
        };
        if frame.kind != closing_kind {
            bail!("unexpected ECUC nesting while locating semantic value");
        }
        if closing_kind == FrameKind::Container {
            let module = stack
                .iter()
                .rev()
                .find(|parent| parent.kind == FrameKind::Module)
                .and_then(|parent| parent.short_name.clone())
                .unwrap_or_default();
            let mut names = stack
                .iter()
                .filter(|parent| parent.kind == FrameKind::Container)
                .filter_map(|parent| parent.short_name.clone())
                .collect::<Vec<_>>();
            names.push(frame.short_name.unwrap_or_default());
            spans.push(ContainerSpan {
                module,
                path: names.join("/"),
                start: frame.start,
                end: token.end(),
            });
        }
    }
    Ok(spans)
}

fn collect_leaf_values<'a>(
    text: &'a str,
    containers: &'a [ContainerSpan],
    requested_kind: ValueKind,
) -> Result<Vec<(&'a ContainerSpan, LocatedValue)>> {
    let (leaf_pattern, value_pattern) = match requested_kind {
        ValueKind::Parameter => (
            r#"(?s)<ECUC-[A-Z0-9-]+-PARAM-VALUE(?:\s+[^>]*)?>(?P<body>.*?)</ECUC-[A-Z0-9-]+-PARAM-VALUE\s*>"#,
            r#"(?s)<VALUE(?:\s+[^>]*)?>(?P<value>[^<]*)</VALUE\s*>"#,
        ),
        ValueKind::Reference => (
            r#"(?s)<ECUC-REFERENCE-VALUE(?:\s+[^>]*)?>(?P<body>.*?)</ECUC-REFERENCE-VALUE\s*>"#,
            r#"(?s)<VALUE-REF(?:\s+[^>]*)?>(?P<value>[^<]*)</VALUE-REF\s*>"#,
        ),
    };
    let leaves = Regex::new(leaf_pattern)?;
    let definitions = Regex::new(
        r#"(?s)<DEFINITION-REF(?:\s+[^>]*)?>(?P<definition>[^<]*)</DEFINITION-REF\s*>"#,
    )?;
    let values = Regex::new(value_pattern)?;
    let mut found = Vec::new();
    for leaf in leaves.captures_iter(text) {
        let whole = leaf.get(0).expect("leaf token");
        let body = leaf.name("body").expect("leaf body");
        let Some(definition) = definitions.captures(body.as_str()) else {
            continue;
        };
        let Some(value) = values.captures(body.as_str()) else {
            continue;
        };
        let raw_value = value.name("value").expect("value text");
        let trimmed = raw_value.as_str().trim();
        let leading = raw_value.as_str().len() - raw_value.as_str().trim_start().len();
        let start = body.start() + raw_value.start() + leading;
        let end = start + trimmed.len();
        let Some(container) = containers
            .iter()
            .filter(|span| span.start <= whole.start() && span.end >= whole.end())
            .min_by_key(|span| span.end - span.start)
        else {
            continue;
        };
        let definition_ref = decode_xml_text(
            definition
                .name("definition")
                .expect("definition text")
                .as_str()
                .trim(),
        )?;
        let name = definition_ref
            .rsplit('/')
            .find(|part| !part.is_empty())
            .unwrap_or_default();
        found.push((
            container,
            LocatedValue {
                start,
                end,
                current: decode_xml_text(trimmed)?,
                semantic_path: format!("{}/{name}", container.path),
                definition_ref,
                kind: requested_kind,
            },
        ));
    }
    Ok(found)
}

fn requested_target(request: &Value) -> Result<(ValueKind, &str)> {
    let parameter = optional_trimmed(request, "parameter");
    let reference = optional_trimmed(request, "reference");
    match (parameter, reference) {
        (Some(name), None) => Ok((ValueKind::Parameter, name)),
        (None, Some(name)) => Ok((ValueKind::Reference, name)),
        (None, None) => bail!("exactly one of parameter or reference is required"),
        (Some(_), Some(_)) => bail!("parameter and reference cannot both be specified"),
    }
}

fn normalize_container_path(path: &str, module: &str) -> String {
    let mut parts = path
        .trim_matches('/')
        .split('/')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts
        .first()
        .is_some_and(|first| first.eq_ignore_ascii_case(module))
    {
        parts.remove(0);
    }
    parts.join("/")
}

fn definition_matches(definition_ref: &str, requested: &str) -> bool {
    if requested.contains('/') {
        definition_ref == requested
    } else {
        definition_ref.rsplit('/').find(|part| !part.is_empty()) == Some(requested)
    }
}

fn required_trimmed<'a>(request: &'a Value, name: &str) -> Result<&'a str> {
    optional_trimmed(request, name).ok_or_else(|| anyhow::anyhow!("{name} is required"))
}

fn optional_trimmed<'a>(request: &'a Value, name: &str) -> Option<&'a str> {
    request
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_value<'a>(request: &'a Value, name: &str) -> Result<&'a str> {
    request
        .get(name)
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("{name} must be a string"))
}

fn ensure_arxml_size(path: &Path) -> Result<()> {
    if !path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("arxml"))
    {
        bail!("set_ecuc_value requires an .arxml module configuration");
    }
    if fs::metadata(path)?.len() > MAX_FILE_SIZE {
        bail!("set_ecuc_value input exceeds 64 MiB: {}", path.display());
    }
    Ok(())
}

fn decode_xml_text(raw: &str) -> Result<String> {
    let wrapped = format!("<LGK>{raw}</LGK>");
    let element = Element::parse(wrapped.as_bytes()).context("invalid XML text value")?;
    Ok(element
        .get_text()
        .map_or_else(String::new, |text| text.into_owned()))
}

fn escape_xml_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn bounded(value: &str) -> (String, bool) {
    if value.chars().count() <= MAX_RESPONSE_VALUE_CHARS {
        (value.to_string(), false)
    } else {
        (value.chars().take(MAX_RESPONSE_VALUE_CHARS).collect(), true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_accepts_module_prefix_and_xml_text_round_trips() {
        assert_eq!(normalize_container_path("/Com/A/B", "com"), "A/B");
        let source = "A & <B>";
        assert_eq!(decode_xml_text(&escape_xml_text(source)).unwrap(), source);
    }
}
