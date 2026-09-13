//! Bounded, vendor-neutral index and reference tracing for AUTOSAR ARXML models.
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde::Serialize;
use serde_json::{json, Value};
use walkdir::WalkDir;
use xmltree::{Element, XMLNode};

use crate::project::SessionConfig;
const DEFAULT_RESULT_LIMIT: usize = 32;
const MAX_RESULT_LIMIT: usize = 256;
const DEFAULT_NODE_LIMIT: usize = 64;
const MAX_NODE_LIMIT: usize = 256;
const DEFAULT_EDGE_LIMIT: usize = 128;
const MAX_EDGE_LIMIT: usize = 512;
const MAX_REFS_PER_OBJECT: usize = 16;
const MAX_FILES: usize = 512;
const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_BYTES: u64 = 256 * 1024 * 1024;

#[derive(Clone, Debug, Serialize)]
struct ModelRef {
    role: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dest: Option<String>,
}

#[derive(Clone, Debug)]
struct ModelObject {
    path: String,
    short_name: String,
    kind: String,
    file: String,
    refs: Vec<ModelRef>,
}

#[derive(Debug)]
struct ModelIndex {
    objects: Vec<ModelObject>,
    by_path: BTreeMap<String, Vec<usize>>,
    files_scanned: usize,
    bytes_scanned: u64,
    elapsed_ms: u128,
}

#[derive(Serialize)]
struct ObjectView<'a> {
    path: &'a str,
    short_name: &'a str,
    kind: &'a str,
    file: &'a str,
    references: &'a [ModelRef],
    #[serde(skip_serializing_if = "is_zero")]
    references_omitted: usize,
}

#[derive(Clone, Debug, Serialize)]
struct TraceNode {
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    short_name: Option<String>,
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<String>,
    resolved: bool,
}

#[derive(Clone, Debug, Serialize)]
struct TraceEdge {
    source: String,
    role: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dest: Option<String>,
    resolved: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Direction {
    Outgoing,
    Incoming,
    Both,
}

pub fn inspect(config: &SessionConfig, request: &Value) -> Result<Value> {
    let index = ModelIndex::load(config, request)?;
    let kinds = string_set(request, "kinds")?;
    let name_regex = request
        .get("name_regex")
        .and_then(Value::as_str)
        .map(|raw| Regex::new(raw).with_context(|| format!("invalid name_regex: {raw}")))
        .transpose()?;
    let path_prefix = optional_string(request, "path_prefix");
    let references_to = optional_string(request, "references_to");
    let include_references = request
        .get("include_references")
        .and_then(Value::as_bool)
        .unwrap_or(true);
    let limit = bounded_usize(request, "limit", DEFAULT_RESULT_LIMIT, MAX_RESULT_LIMIT)?;

    let mut matches = index
        .objects
        .iter()
        .filter(|object| {
            kinds
                .as_ref()
                .is_none_or(|wanted| wanted.contains(&object.kind))
                && name_regex
                    .as_ref()
                    .is_none_or(|pattern| pattern.is_match(&object.short_name))
                && path_prefix.is_none_or(|prefix| object.path.starts_with(prefix))
                && references_to.is_none_or(|target| {
                    object.refs.iter().any(|reference| {
                        reference.target == target || reference.target.ends_with(target)
                    })
                })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| left.path.cmp(&right.path).then(left.kind.cmp(&right.kind)));
    let total = matches.len();
    let objects = matches
        .into_iter()
        .take(limit)
        .map(|object| {
            let ref_count = if include_references {
                object.refs.len().min(MAX_REFS_PER_OBJECT)
            } else {
                0
            };
            ObjectView {
                path: &object.path,
                short_name: &object.short_name,
                kind: &object.kind,
                file: &object.file,
                references: &object.refs[..ref_count],
                references_omitted: object.refs.len() - ref_count,
            }
        })
        .collect::<Vec<_>>();
    Ok(json!({
        "scope": scope_name(request)?,
        "files_scanned": index.files_scanned,
        "bytes_scanned": index.bytes_scanned,
        "objects_indexed": index.objects.len(),
        "total": total,
        "objects": objects,
        "truncated": total > limit,
        "limit": limit,
        "elapsed_ms": index.elapsed_ms,
    }))
}

pub fn trace(config: &SessionConfig, request: &Value) -> Result<Value> {
    let index = ModelIndex::load(config, request)?;
    let start = required_string(request, "start")?;
    let direction = match optional_string(request, "direction").unwrap_or("both") {
        "outgoing" => Direction::Outgoing,
        "incoming" => Direction::Incoming,
        "both" => Direction::Both,
        _ => bail!("direction must be outgoing, incoming, or both"),
    };
    let depth = bounded_usize(request, "depth", 3, 6)?;
    let node_limit = bounded_usize(request, "node_limit", DEFAULT_NODE_LIMIT, MAX_NODE_LIMIT)?;
    let edge_limit = bounded_usize(request, "edge_limit", DEFAULT_EDGE_LIMIT, MAX_EDGE_LIMIT)?;
    let start_index = resolve_start(&index, start)?;
    let start_path = index.objects[start_index].path.clone();
    let mut nodes = BTreeMap::<String, TraceNode>::new();
    let mut edges = BTreeMap::<(String, String, String), TraceEdge>::new();
    let mut queue = VecDeque::from([(start_path.clone(), 0usize)]);
    let mut expanded = BTreeSet::new();
    let mut truncated = false;

    while let Some((path, level)) = queue.pop_front() {
        add_node(&index, &mut nodes, &path, None);
        if level >= depth || !expanded.insert(path.clone()) {
            continue;
        }
        if direction != Direction::Incoming {
            if let Some(indices) = index.by_path.get(&path) {
                for object_index in indices {
                    for reference in &index.objects[*object_index].refs {
                        if edges.len() >= edge_limit || nodes.len() >= node_limit {
                            truncated = true;
                            break;
                        }
                        let resolved = index.by_path.contains_key(&reference.target);
                        let edge = TraceEdge {
                            source: path.clone(),
                            role: reference.role.clone(),
                            target: reference.target.clone(),
                            dest: reference.dest.clone(),
                            resolved,
                        };
                        edges.insert(
                            (edge.source.clone(), edge.role.clone(), edge.target.clone()),
                            edge,
                        );
                        add_node(
                            &index,
                            &mut nodes,
                            &reference.target,
                            reference.dest.as_deref(),
                        );
                        if resolved {
                            queue.push_back((reference.target.clone(), level + 1));
                        }
                    }
                }
            }
        }
        if direction != Direction::Outgoing {
            for object in &index.objects {
                for reference in object.refs.iter().filter(|item| item.target == path) {
                    if edges.len() >= edge_limit || nodes.len() >= node_limit {
                        truncated = true;
                        break;
                    }
                    let edge = TraceEdge {
                        source: object.path.clone(),
                        role: reference.role.clone(),
                        target: path.clone(),
                        dest: reference.dest.clone(),
                        resolved: true,
                    };
                    edges.insert(
                        (edge.source.clone(), edge.role.clone(), edge.target.clone()),
                        edge,
                    );
                    add_node(&index, &mut nodes, &object.path, None);
                    queue.push_back((object.path.clone(), level + 1));
                }
            }
        }
        if truncated {
            break;
        }
    }

    Ok(json!({
        "scope": scope_name(request)?,
        "start": start_path,
        "direction": match direction { Direction::Outgoing => "outgoing",
            Direction::Incoming => "incoming", Direction::Both => "both" },
        "depth": depth,
        "files_scanned": index.files_scanned,
        "objects_indexed": index.objects.len(),
        "nodes": nodes.into_values().collect::<Vec<_>>(),
        "edges": edges.into_values().collect::<Vec<_>>(),
        "truncated": truncated,
        "node_limit": node_limit,
        "edge_limit": edge_limit,
        "elapsed_ms": index.elapsed_ms,
    }))
}

impl ModelIndex {
    fn load(config: &SessionConfig, request: &Value) -> Result<Self> {
        let started = Instant::now();
        let roots = scope_roots(config, request)?;
        let mut files = Vec::new();
        for root in roots {
            if !root.exists() {
                continue;
            }
            for entry in WalkDir::new(&root).follow_links(false) {
                let entry =
                    entry.with_context(|| format!("scan model root: {}", root.display()))?;
                if entry.file_type().is_symlink() {
                    bail!(
                        "AUTOSAR model scope contains a link: {}",
                        entry.path().display()
                    );
                }
                if entry.file_type().is_file()
                    && entry
                        .path()
                        .extension()
                        .and_then(|value| value.to_str())
                        .is_some_and(|value| value.eq_ignore_ascii_case("arxml"))
                {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
        files.sort();
        files.dedup();
        if files.len() > MAX_FILES {
            bail!("AUTOSAR model scope exceeds {MAX_FILES} ARXML files; use a narrower scope");
        }
        let mut objects = Vec::new();
        let mut total_bytes = 0u64;
        for file in &files {
            let size = fs::metadata(file)?.len();
            if size > MAX_FILE_SIZE {
                bail!("AUTOSAR model file exceeds 64 MiB: {}", file.display());
            }
            total_bytes = total_bytes.saturating_add(size);
            if total_bytes > MAX_TOTAL_BYTES {
                bail!("AUTOSAR model scope exceeds 256 MiB; use a narrower scope");
            }
            let root = Element::parse(
                fs::File::open(file)
                    .with_context(|| format!("open AUTOSAR model: {}", file.display()))?,
            )
            .with_context(|| format!("parse AUTOSAR model: {}", file.display()))?;
            let relative = file
                .strip_prefix(&config.project_path)
                .unwrap_or(file)
                .to_string_lossy()
                .replace('\\', "/");
            collect_objects(&root, &relative, &[], None, &mut objects);
        }
        objects.sort_by(|left, right| left.path.cmp(&right.path).then(left.kind.cmp(&right.kind)));
        let mut by_path = BTreeMap::<String, Vec<usize>>::new();
        for (index, object) in objects.iter().enumerate() {
            by_path.entry(object.path.clone()).or_default().push(index);
        }
        Ok(Self {
            objects,
            by_path,
            files_scanned: files.len(),
            bytes_scanned: total_bytes,
            elapsed_ms: started.elapsed().as_millis(),
        })
    }
}

fn collect_objects(
    element: &Element,
    file: &str,
    parent_path: &[String],
    parent_object: Option<usize>,
    objects: &mut Vec<ModelObject>,
) {
    let short_name = direct_child_text(element, "SHORT-NAME");
    let mut current_path = parent_path.to_vec();
    if let Some(name) = &short_name {
        current_path.push(name.clone());
        let refs = collect_owned_refs(element);
        let current_index = objects.len();
        objects.push(ModelObject {
            path: format!("/{}", current_path.join("/")),
            short_name: name.clone(),
            kind: element.name.clone(),
            file: file.to_string(),
            refs,
        });
        if let Some(parent_index) = parent_object {
            // Mapping/prototype objects often carry their own SHORT-NAME. Preserve
            // that semantic boundary, but connect it to its owning model object.
            // AR-PACKAGE containment is omitted because it adds navigation noise.
            if objects[parent_index].kind != "AR-PACKAGE" {
                let target = objects[current_index].path.clone();
                let dest = objects[current_index].kind.clone();
                objects[parent_index].refs.push(ModelRef {
                    role: "CONTAINS".to_string(),
                    target,
                    dest: Some(dest),
                });
            }
        }
        for node in &element.children {
            if let XMLNode::Element(child) = node {
                collect_objects(child, file, &current_path, Some(current_index), objects);
            }
        }
        return;
    }
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            collect_objects(child, file, &current_path, parent_object, objects);
        }
    }
}

fn collect_owned_refs(element: &Element) -> Vec<ModelRef> {
    fn visit(element: &Element, root: bool, refs: &mut Vec<ModelRef>) {
        if !root && direct_child_text(element, "SHORT-NAME").is_some() {
            return;
        }
        if element.name.ends_with("-REF") {
            if let Some(target) = element.get_text().map(|text| text.trim().to_string()) {
                if !target.is_empty() {
                    refs.push(ModelRef {
                        role: element.name.clone(),
                        target,
                        dest: element.attributes.get("DEST").cloned(),
                    });
                }
            }
            return;
        }
        for node in &element.children {
            if let XMLNode::Element(child) = node {
                visit(child, false, refs);
            }
        }
    }
    let mut refs = Vec::new();
    visit(element, true, &mut refs);
    refs.sort_by(|left, right| {
        left.role
            .cmp(&right.role)
            .then(left.target.cmp(&right.target))
    });
    refs.dedup_by(|left, right| {
        left.role == right.role && left.target == right.target && left.dest == right.dest
    });
    refs
}

fn direct_child_text(element: &Element, name: &str) -> Option<String> {
    element.children.iter().find_map(|node| match node {
        XMLNode::Element(child) if child.name == name => {
            child.get_text().map(|text| text.trim().to_string())
        }
        _ => None,
    })
}

fn scope_roots(config: &SessionConfig, request: &Value) -> Result<Vec<PathBuf>> {
    let config_root = config.project_path.join("Config");
    match scope_name(request)? {
        "system" => Ok(vec![config_root.join("System")]),
        "developer" => Ok(vec![config_root.join("Developer")]),
        "model" => Ok(vec![
            config_root.join("System"),
            config_root.join("Developer"),
        ]),
        "all" => Ok(vec![config_root]),
        _ => unreachable!(),
    }
}

fn scope_name(request: &Value) -> Result<&str> {
    let scope = optional_string(request, "scope").unwrap_or("model");
    if matches!(scope, "system" | "developer" | "model" | "all") {
        Ok(scope)
    } else {
        bail!("scope must be system, developer, model, or all")
    }
}

fn resolve_start(index: &ModelIndex, start: &str) -> Result<usize> {
    if let Some(indices) = index.by_path.get(start) {
        return unique_start(index, start, indices);
    }
    let matches = index
        .objects
        .iter()
        .enumerate()
        .filter(|(_, object)| object.short_name == start || object.path.ends_with(start))
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    unique_start(index, start, &matches)
}

fn unique_start(index: &ModelIndex, start: &str, matches: &[usize]) -> Result<usize> {
    match matches {
        [] => bail!("AUTOSAR model start was not found: {start}"),
        [found] => Ok(*found),
        _ => {
            let first = &index.objects[matches[0]];
            if matches.iter().all(|item| {
                let candidate = &index.objects[*item];
                candidate.path == first.path && candidate.kind == first.kind
            }) {
                // Communication/SystemExtract commonly repeat the same semantic
                // object. Trace all matching definitions through by_path while
                // using one representative as the start node.
                return Ok(matches[0]);
            }
            let candidates = matches
                .iter()
                .take(8)
                .map(|item| &index.objects[*item].path)
                .collect::<Vec<_>>();
            bail!(
                "AUTOSAR model start is ambiguous: {start}; candidates={}",
                serde_json::to_string(&candidates)?
            )
        }
    }
}

fn add_node(
    index: &ModelIndex,
    nodes: &mut BTreeMap<String, TraceNode>,
    path: &str,
    unresolved_dest: Option<&str>,
) {
    nodes.entry(path.to_string()).or_insert_with(|| {
        index
            .by_path
            .get(path)
            .and_then(|indices| indices.first())
            .map(|object_index| {
                let object = &index.objects[*object_index];
                TraceNode {
                    path: object.path.clone(),
                    short_name: Some(object.short_name.clone()),
                    kind: object.kind.clone(),
                    file: Some(object.file.clone()),
                    resolved: true,
                }
            })
            .unwrap_or_else(|| TraceNode {
                path: path.to_string(),
                short_name: path.rsplit('/').next().map(str::to_string),
                kind: unresolved_dest.unwrap_or("UNRESOLVED").to_string(),
                file: None,
                resolved: false,
            })
    });
}

fn string_set(request: &Value, name: &str) -> Result<Option<BTreeSet<String>>> {
    let Some(value) = request.get(name) else {
        return Ok(None);
    };
    let values = match value {
        Value::String(raw) => raw.split(',').map(str::trim).collect::<Vec<_>>(),
        Value::Array(items) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(str::trim)
                    .ok_or_else(|| anyhow::anyhow!("{name} array must contain only strings"))
            })
            .collect::<Result<Vec<_>>>()?,
        _ => bail!("{name} must be a comma-separated string or string array"),
    };
    let set = values
        .into_iter()
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<BTreeSet<_>>();
    Ok(Some(set))
}

fn bounded_usize(request: &Value, name: &str, default: usize, maximum: usize) -> Result<usize> {
    request
        .get(name)
        .map(|value| {
            value
                .as_u64()
                .and_then(|value| usize::try_from(value).ok())
                .filter(|value| (1..=maximum).contains(value))
                .ok_or_else(|| anyhow::anyhow!("{name} must be an integer from 1 to {maximum}"))
        })
        .transpose()
        .map(|value| value.unwrap_or(default))
}

fn optional_string<'a>(request: &'a Value, name: &str) -> Option<&'a str> {
    request
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_string<'a>(request: &'a Value, name: &str) -> Result<&'a str> {
    optional_string(request, name).ok_or_else(|| anyhow::anyhow!("{name} is required"))
}

fn is_zero(value: &usize) -> bool {
    *value == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owned_references_stop_at_nested_identifiables() {
        let root = Element::parse(
            br#"<ROOT><SHORT-NAME>Parent</SHORT-NAME><A-REF DEST="A">/A</A-REF>
                <CHILD><SHORT-NAME>Child</SHORT-NAME><B-REF DEST="B">/B</B-REF></CHILD></ROOT>"#
                .as_slice(),
        )
        .unwrap();
        let refs = collect_owned_refs(&root);
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].target, "/A");
    }

    #[test]
    fn named_children_are_connected_to_non_package_owners() {
        let root = Element::parse(
            br#"<CAN-FRAME><SHORT-NAME>Frame</SHORT-NAME><MAPPINGS>
                <PDU-TO-FRAME-MAPPING><SHORT-NAME>Map</SHORT-NAME>
                <PDU-REF DEST="I-SIGNAL-I-PDU">/Pdus/Pdu</PDU-REF>
                </PDU-TO-FRAME-MAPPING></MAPPINGS></CAN-FRAME>"#
                .as_slice(),
        )
        .unwrap();
        let mut objects = Vec::new();
        collect_objects(&root, "Config/System/Test.arxml", &[], None, &mut objects);
        assert_eq!(objects.len(), 2);
        assert!(objects[0]
            .refs
            .iter()
            .any(|item| item.role == "CONTAINS" && item.target == "/Frame/Map"));
        assert!(objects[1]
            .refs
            .iter()
            .any(|item| item.role == "PDU-REF" && item.target == "/Pdus/Pdu"));
    }

    #[test]
    fn duplicate_extract_definitions_resolve_as_one_semantic_start() {
        let objects = vec![
            ModelObject {
                path: "/Frames/Frame".to_string(),
                short_name: "Frame".to_string(),
                kind: "CAN-FRAME".to_string(),
                file: "Communication.arxml".to_string(),
                refs: Vec::new(),
            },
            ModelObject {
                path: "/Frames/Frame".to_string(),
                short_name: "Frame".to_string(),
                kind: "CAN-FRAME".to_string(),
                file: "SystemExtract.arxml".to_string(),
                refs: Vec::new(),
            },
        ];
        let index = ModelIndex {
            objects,
            by_path: BTreeMap::from([("/Frames/Frame".to_string(), vec![0, 1])]),
            files_scanned: 2,
            bytes_scanned: 0,
            elapsed_ms: 0,
        };
        assert_eq!(resolve_start(&index, "/Frames/Frame").unwrap(), 0);
    }
}
