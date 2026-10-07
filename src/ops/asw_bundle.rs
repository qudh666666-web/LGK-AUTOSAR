//! Typed, single-file ASW authoring. This is a saved-model operation, not a
//! DaVinci in-memory mutation or a persisted undo journal.
use super::{collect_objects, direct_child_text, ModelIndex, ModelObject};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Component as PathComponent, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use serde_json::{json, Value};
use xmltree::{Element, EmitterConfig, XMLNode};

use crate::project::{normalize_canonical_path, SessionConfig};

const MARKER: &str = "<!-- LGK-AUTOSAR ASW bundle v1 -->";
const MAX_BYTES: usize = 256 * 1024;
const MAX_OBJECTS: usize = 512;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    func: String,
    file: PathBuf,
    expected: Value,
    #[serde(default)]
    bundle: Option<Bundle>,
    #[serde(default)]
    delete: bool,
    #[serde(default)]
    preview: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    package: String,
    #[serde(default)]
    types: Vec<DataType>,
    #[serde(default)]
    interfaces: Vec<Interface>,
    #[serde(default)]
    components: Vec<Swc>,
    #[serde(default)]
    compositions: Vec<Composition>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum DataType {
    #[serde(rename = "alias")]
    Alias { name: String, type_ref: String },
    #[serde(rename = "scalar")]
    Scalar { name: String, base_type_ref: String },
    #[serde(rename = "array")]
    Array {
        name: String,
        type_ref: String,
        length: u32,
    },
    #[serde(rename = "record")]
    Record {
        name: String,
        fields: Vec<DataElement>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DataElement {
    name: String,
    type_ref: String,
}

#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Interface {
    #[serde(rename = "sender_receiver")]
    SenderReceiver {
        name: String,
        data_elements: Vec<DataElement>,
    },
    #[serde(rename = "client_server")]
    ClientServer {
        name: String,
        operations: Vec<Operation>,
    },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Operation {
    name: String,
    #[serde(default)]
    arguments: Vec<Argument>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Argument {
    name: String,
    type_ref: String,
    direction: ArgumentDirection,
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
enum ArgumentDirection {
    In,
    Out,
    Inout,
}

#[derive(Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum Direction {
    Provide,
    Require,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Port {
    name: String,
    direction: Direction,
    interface_ref: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Swc {
    name: String,
    #[serde(default)]
    ports: Vec<Port>,
    #[serde(default)]
    runnables: Vec<Runnable>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Runnable {
    name: String,
    symbol: String,
    #[serde(default)]
    period_seconds: Option<f64>,
    #[serde(default)]
    reads: Vec<Access>,
    #[serde(default)]
    writes: Vec<Access>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Access {
    port: String,
    data_element: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Composition {
    name: String,
    instances: Vec<Instance>,
    #[serde(default)]
    ports: Vec<Port>,
    #[serde(default)]
    connections: Vec<Connection>,
    #[serde(default)]
    delegations: Vec<Delegation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Instance {
    name: String,
    type_ref: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Endpoint {
    instance: String,
    port: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Connection {
    name: String,
    provider: Endpoint,
    requester: Endpoint,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delegation {
    name: String,
    inner: Endpoint,
    outer_port: String,
}

struct Prepared {
    file: PathBuf,
    old: Option<Vec<u8>>,
    new: Option<Vec<u8>>,
    objects: usize,
    package: String,
    preview: bool,
}

pub fn validate_bundle(config: &SessionConfig, request: &Value) -> Result<()> {
    prepare(config, request).map(|_| ())
}

pub fn write_bundle(config: &SessionConfig, request: &Value) -> Result<Value> {
    let prepared = prepare(config, request)?;
    let changed = prepared.old != prepared.new;
    if changed && !prepared.preview {
        match (&prepared.old, &prepared.new) {
            (None, Some(new)) => create_checked(&prepared.file, new)?,
            (Some(old), Some(new)) => {
                super::super::edit_file::replace_checked(&prepared.file, old, new)?
            }
            (Some(old), None) => {
                if fs::read(&prepared.file)? != *old {
                    bail!("ASW file changed before deletion");
                }
                fs::remove_file(&prepared.file)?;
            }
            (None, None) => unreachable!(),
        }
    }
    Ok(json!({"file": prepared.file, "package": prepared.package,
        "preview": prepared.preview, "changed": changed, "applied": changed && !prepared.preview,
        "deleted": prepared.new.is_none() && !prepared.preview,
        "operation": if prepared.new.is_none() {"delete"} else if prepared.old.is_none() {"create"} else {"update"},
        "objects": prepared.objects, "bytes_before": prepared.old.as_ref().map_or(0, Vec::len),
        "bytes_after": prepared.new.as_ref().map_or(0, Vec::len),
        "write_path": "saved_arxml", "davinci_validated": false,
        "cross_request_undo_available": false}))
}

fn prepare(config: &SessionConfig, value: &Value) -> Result<Prepared> {
    if value.get("expected").is_none() {
        bail!("expected is required: null for create or complete saved content for update/delete");
    }
    if serde_json::to_vec(value)?.len() > 2 * MAX_BYTES {
        bail!("ASW request exceeds 512 KiB");
    }
    let request: Request =
        serde_json::from_value(value.clone()).context("invalid ASW bundle request")?;
    if request.func != "write_asw_bundle" {
        bail!("func must be write_asw_bundle");
    }
    let file = target_file(config, &request.file)?;
    let old = match &request.expected {
        Value::Null => {
            if fs::symlink_metadata(&file).is_ok() {
                bail!("ASW create requires an absent file; never overwrites");
            }
            None
        }
        Value::String(expected) => {
            if expected.len() > MAX_BYTES {
                bail!("expected ASW file exceeds 256 KiB");
            }
            if fs::metadata(&file)?.len() > MAX_BYTES as u64 {
                bail!("saved ASW bundle exceeds 256 KiB");
            }
            let actual = fs::read(&file)?;
            if actual != expected.as_bytes() {
                bail!("ASW file does not match expected; re-read before writing");
            }
            if !expected.starts_with("<?xml") || !expected.contains(MARKER) {
                bail!("updates/deletion only support LGK-authored ASW bundle files");
            }
            Some(actual)
        }
        _ => bail!(
            "expected must be null for create or the complete saved UTF-8 file for update/delete"
        ),
    };
    if request.delete == request.bundle.is_some() {
        bail!("supply bundle for write or delete=true without bundle");
    }
    if request.delete && old.is_none() {
        bail!("deletion requires complete expected saved content");
    }
    let mut old_objects = Vec::new();
    let mut old_subtrees = BTreeMap::new();
    let old_package = if let Some(bytes) = &old {
        let root = Element::parse(bytes.as_slice()).context("invalid saved ASW bundle")?;
        let packages = root
            .get_child("AR-PACKAGES")
            .context("saved bundle lacks AR-PACKAGES")?;
        let roots = packages
            .children
            .iter()
            .filter_map(|node| {
                if let XMLNode::Element(e) = node {
                    Some(e)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if root.name != "AUTOSAR"
            || root.namespace.as_deref() != Some("http://autosar.org/schema/r4.0")
            || roots.len() != 1
            || roots[0].name != "AR-PACKAGE"
        {
            bail!("saved ASW bundle must contain one package");
        }
        collect_objects(&root, "", &[], None, &mut old_objects);
        collect_subtrees(&root, "", &mut old_subtrees);
        Some(direct_child_text(roots[0], "SHORT-NAME").context("saved package has no name")?)
    } else {
        None
    };
    let index = ModelIndex::load(config, &json!({"scope":"all"}))?;
    let relative = file
        .strip_prefix(&config.project_path)?
        .to_string_lossy()
        .replace('\\', "/");
    let existing = index
        .objects
        .iter()
        .filter(|object| object.file != relative)
        .collect::<Vec<_>>();
    let mut new_subtrees = BTreeMap::new();
    let (package, new, objects) = if let Some(bundle) = &request.bundle {
        identifier(&bundle.package)?;
        if old_package
            .as_ref()
            .is_some_and(|old| old != &bundle.package)
        {
            bail!("ASW bundle package cannot be renamed");
        }
        let prefix = format!("/{}", bundle.package);
        if existing
            .iter()
            .any(|o| o.path == prefix || o.path.starts_with(&(prefix.clone() + "/")))
        {
            bail!("ASW bundle package already has definitions in another saved file");
        }
        let element = build(bundle, &existing)?;
        collect_subtrees(&element, "", &mut new_subtrees);
        let mut objects = Vec::new();
        collect_objects(&element, &relative, &[], None, &mut objects);
        if objects.len() > MAX_OBJECTS {
            bail!("ASW bundle exceeds 512 named objects");
        }
        let mut paths = BTreeSet::new();
        for object in &objects {
            identifier(&object.short_name)?;
            if !paths.insert(object.path.clone()) {
                bail!("duplicate ASW path: {}", object.path);
            }
        }
        validate_references(&objects, &existing)?;
        validate_type_cycles(&objects, &existing)?;
        let mut output = Vec::new();
        element.write_with_config(
            &mut output,
            EmitterConfig::new()
                .perform_indent(true)
                .write_document_declaration(false),
        )?;
        let body = String::from_utf8(output)?;
        let content = format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n{MARKER}\n<AUTOSAR xmlns=\"http://autosar.org/schema/r4.0\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:schemaLocation=\"http://autosar.org/schema/r4.0 AUTOSAR_00044.xsd\">\n  <AR-PACKAGES>\n{body}\n  </AR-PACKAGES>\n</AUTOSAR>\n");
        if content.len() > MAX_BYTES {
            bail!("ASW bundle exceeds 256 KiB");
        }
        Element::parse(content.as_bytes()).context("generated ASW XML did not round-trip")?;
        (bundle.package.clone(), Some(content.into_bytes()), objects)
    } else {
        (old_package.context("no saved package")?, None, Vec::new())
    };
    // No external object may retain a reference to a removed or retyped object.
    let old_paths = old_objects
        .iter()
        .map(|o| o.path.as_str())
        .collect::<BTreeSet<_>>();
    for source in &existing {
        for reference in &source.refs {
            if reference.role != "CONTAINS"
                && old_paths.contains(reference.target.as_str())
                && !objects.iter().any(|o| {
                    o.path == reference.target
                        && reference.dest.as_ref().is_none_or(|dest| dest == &o.kind)
                })
            {
                bail!(
                    "external ASW reference would become dangling: {} -> {}",
                    source.path,
                    reference.target
                );
            }
            if reference.role != "CONTAINS"
                && old_paths.contains(reference.target.as_str())
                && old_subtrees.get(&reference.target) != new_subtrees.get(&reference.target)
            {
                bail!("externally referenced ASW object cannot be changed by a bundle rewrite: {} -> {}", source.path, reference.target);
            }
        }
    }
    Ok(Prepared {
        file,
        old,
        new,
        objects: objects.len(),
        package,
        preview: request.preview,
    })
}

fn collect_subtrees(element: &Element, parent: &str, output: &mut BTreeMap<String, Value>) {
    let path = direct_child_text(element, "SHORT-NAME")
        .map_or_else(|| parent.to_owned(), |name| format!("{parent}/{name}"));
    if direct_child_text(element, "SHORT-NAME").is_some() {
        output.insert(path.clone(), semantic_element(element));
    }
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            collect_subtrees(child, &path, output);
        }
    }
}

fn semantic_element(element: &Element) -> Value {
    let attributes = element.attributes.iter().collect::<BTreeMap<_, _>>();
    let children = element
        .children
        .iter()
        .filter_map(|node| match node {
            XMLNode::Element(child) => Some(semantic_element(child)),
            XMLNode::Text(value) | XMLNode::CData(value) if !value.trim().is_empty() => {
                Some(json!(value.trim()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    json!({"kind":element.name,"attributes":attributes,"children":children})
}

fn target_file(config: &SessionConfig, raw: &Path) -> Result<PathBuf> {
    if !raw.is_absolute()
        || !raw
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("arxml"))
    {
        bail!(
            "file must be an absolute .arxml path in a DPA-registered application component folder"
        );
    }
    if raw
        .components()
        .any(|part| matches!(part, PathComponent::ParentDir))
    {
        bail!("ASW path must not contain parent traversal");
    }
    let parent = normalize_canonical_path(
        raw.parent()
            .context("ASW file parent missing")?
            .canonicalize()?,
    );
    if !registered_roots(config, true)?
        .iter()
        .any(|root| parent.starts_with(root))
    {
        bail!("file must be in a project-contained DPA ApplicationComponentFolder; an unregistered Developer file is not a DaVinci input");
    }
    for protected in [
        "ECUC",
        "System",
        "ServiceComponents",
        "AUTOSAR",
        "InternalBehavior",
    ] {
        if parent.starts_with(config.project_path.join("Config").join(protected)) {
            bail!("ASW bundles cannot be written in a protected configuration folder");
        }
    }
    let file = parent.join(raw.file_name().context("ASW file name missing")?);
    if let Ok(meta) = fs::symlink_metadata(&file) {
        if meta.file_type().is_symlink() || !meta.is_file() {
            bail!("ASW target cannot be a link or directory");
        }
        let canonical = normalize_canonical_path(file.canonicalize()?);
        if canonical != file {
            bail!("ASW target must not resolve through a link");
        }
    }
    Ok(file)
}

pub(super) fn input_roots(config: &SessionConfig) -> Result<Vec<PathBuf>> {
    registered_roots(config, false)
}

fn registered_roots(config: &SessionConfig, writing: bool) -> Result<Vec<PathBuf>> {
    let root = Element::parse(fs::File::open(config.dpa_file()?)?)?;
    let mut paths = Vec::new();
    if let Some(folders) = root
        .get_child("Folders")
        .and_then(|f| f.get_child("ApplicationComponentFolders"))
    {
        for node in &folders.children {
            if let XMLNode::Element(folder) = node {
                if folder.name != "ApplicationComponentFolder" {
                    continue;
                }
                if let Some(value) = folder.get_text() {
                    let value = value.trim().replace('\\', "/");
                    if value.is_empty() {
                        if writing {
                            bail!("ASW write requires accessible project-contained application inputs");
                        }
                        continue;
                    }
                    let path = config.project_path.join(value);
                    // Queries and writes are confined to this project. Missing
                    // or external application inputs are not scanned here.
                    if let Ok(canonical) = path.canonicalize() {
                        let canonical = normalize_canonical_path(canonical);
                        if canonical.is_dir() && canonical.starts_with(&config.project_path) {
                            paths.push(canonical);
                            continue;
                        }
                    }
                    if writing {
                        bail!("ASW write cannot check dependencies in an external or inaccessible ApplicationComponentFolder");
                    }
                }
            }
        }
    }
    paths.sort();
    paths.dedup();
    Ok(paths)
}

fn create_checked(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = path.with_extension(format!(
        "lgk-autosar-{}-{}.tmp",
        std::process::id(),
        rand::random::<u64>()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        // Atomic no-clobber publication on the same filesystem. A destination
        // created after preflight is preserved; no truncating fallback exists.
        fs::hard_link(&temporary, path)
            .context("ASW file publication failed; destination was not overwritten")?;
        Ok(())
    })();
    fs::remove_file(&temporary).ok();
    result
}

fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || !value.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
        || !value.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        bail!("invalid AUTOSAR/C identifier: {value}");
    }
    Ok(())
}

fn child(parent: &mut Element, child: Element) {
    parent.children.push(XMLNode::Element(child));
}
fn text(tag: &str, value: &str) -> Element {
    let mut element = Element::new(tag);
    element.children.push(XMLNode::Text(value.to_owned()));
    element
}
fn named(tag: &str, name: &str) -> Result<Element> {
    identifier(name)?;
    let mut element = Element::new(tag);
    child(&mut element, text("SHORT-NAME", name));
    Ok(element)
}
fn reference(tag: &str, dest: &str, value: &str) -> Result<Element> {
    if !value.starts_with('/')
        || value.len() > 4096
        || value
            .split('/')
            .skip(1)
            .any(|part| identifier(part).is_err())
    {
        bail!("reference must be an absolute AUTOSAR path: {value}");
    }
    let mut element = text(tag, value);
    element.attributes.insert("DEST".into(), dest.into());
    Ok(element)
}
fn props(role: &str, kind: &str, target: &str) -> Result<Element> {
    let mut props = Element::new("SW-DATA-DEF-PROPS");
    let mut variants = Element::new("SW-DATA-DEF-PROPS-VARIANTS");
    let mut conditional = Element::new("SW-DATA-DEF-PROPS-CONDITIONAL");
    child(&mut conditional, reference(role, kind, target)?);
    child(&mut variants, conditional);
    child(&mut props, variants);
    Ok(props)
}
fn type_element(name: &str, target: &str) -> Result<Element> {
    let mut element = named("IMPLEMENTATION-DATA-TYPE-ELEMENT", name)?;
    child(&mut element, text("CATEGORY", "TYPE_REFERENCE"));
    child(
        &mut element,
        props(
            "IMPLEMENTATION-DATA-TYPE-REF",
            "IMPLEMENTATION-DATA-TYPE",
            target,
        )?,
    );
    Ok(element)
}

fn build(bundle: &Bundle, existing: &[&ModelObject]) -> Result<Element> {
    let total = bundle.types.len()
        + bundle.interfaces.len()
        + bundle.components.len()
        + bundle.compositions.len();
    if total == 0 || total > 128 {
        bail!("ASW bundle requires 1..128 top-level elements");
    }
    let prefix = format!("/{}", bundle.package);
    let mut package = named("AR-PACKAGE", &bundle.package)?;
    let mut elements = Element::new("ELEMENTS");
    for datatype in &bundle.types {
        let (name, category) = match datatype {
            DataType::Alias { name, .. } => (name, "TYPE_REFERENCE"),
            DataType::Scalar { name, .. } => (name, "VALUE"),
            DataType::Array { name, .. } => (name, "ARRAY"),
            DataType::Record { name, .. } => (name, "STRUCTURE"),
        };
        let mut element = named("IMPLEMENTATION-DATA-TYPE", name)?;
        child(&mut element, text("CATEGORY", category));
        match datatype {
            DataType::Alias { type_ref, .. } => child(
                &mut element,
                props(
                    "IMPLEMENTATION-DATA-TYPE-REF",
                    "IMPLEMENTATION-DATA-TYPE",
                    type_ref,
                )?,
            ),
            DataType::Scalar { base_type_ref, .. } => child(
                &mut element,
                props("BASE-TYPE-REF", "SW-BASE-TYPE", base_type_ref)?,
            ),
            DataType::Array {
                type_ref, length, ..
            } => {
                if *length == 0 || *length > 65535 {
                    bail!("array length must be 1..65535");
                }
                let mut sub = Element::new("SUB-ELEMENTS");
                let mut item = named("IMPLEMENTATION-DATA-TYPE-ELEMENT", "Element")?;
                child(&mut item, text("CATEGORY", "TYPE_REFERENCE"));
                child(&mut item, text("ARRAY-SIZE", &length.to_string()));
                child(&mut item, text("ARRAY-SIZE-SEMANTICS", "FIXED-SIZE"));
                child(
                    &mut item,
                    props(
                        "IMPLEMENTATION-DATA-TYPE-REF",
                        "IMPLEMENTATION-DATA-TYPE",
                        type_ref,
                    )?,
                );
                child(&mut sub, item);
                child(&mut element, sub);
            }
            DataType::Record { fields, .. } => {
                if fields.is_empty() || fields.len() > 64 {
                    bail!("record requires 1..64 fields");
                }
                let mut sub = Element::new("SUB-ELEMENTS");
                for field in fields {
                    child(&mut sub, type_element(&field.name, &field.type_ref)?);
                }
                child(&mut element, sub);
            }
        }
        child(&mut elements, element);
    }
    for interface in &bundle.interfaces {
        let mut element = match interface {
            Interface::SenderReceiver {
                name,
                data_elements,
            } => {
                if data_elements.is_empty() || data_elements.len() > 64 {
                    bail!("SR interface requires 1..64 data elements");
                }
                let mut interface = named("SENDER-RECEIVER-INTERFACE", name)?;
                child(&mut interface, text("IS-SERVICE", "false"));
                let mut data = Element::new("DATA-ELEMENTS");
                for item in data_elements {
                    let mut prototype = named("VARIABLE-DATA-PROTOTYPE", &item.name)?;
                    child(
                        &mut prototype,
                        reference("TYPE-TREF", "IMPLEMENTATION-DATA-TYPE", &item.type_ref)?,
                    );
                    child(&mut data, prototype);
                }
                child(&mut interface, data);
                interface
            }
            Interface::ClientServer { name, operations } => {
                if operations.is_empty() || operations.len() > 64 {
                    bail!("CS interface requires 1..64 operations");
                }
                let mut interface = named("CLIENT-SERVER-INTERFACE", name)?;
                child(&mut interface, text("IS-SERVICE", "false"));
                let mut list = Element::new("OPERATIONS");
                for operation in operations {
                    if operation.arguments.len() > 32 {
                        bail!("CS operation exceeds 32 arguments");
                    }
                    let mut op = named("CLIENT-SERVER-OPERATION", &operation.name)?;
                    if !operation.arguments.is_empty() {
                        let mut arguments = Element::new("ARGUMENTS");
                        for argument in &operation.arguments {
                            let mut arg = named("ARGUMENT-DATA-PROTOTYPE", &argument.name)?;
                            child(
                                &mut arg,
                                reference(
                                    "TYPE-TREF",
                                    "IMPLEMENTATION-DATA-TYPE",
                                    &argument.type_ref,
                                )?,
                            );
                            child(
                                &mut arg,
                                text(
                                    "DIRECTION",
                                    match argument.direction {
                                        ArgumentDirection::In => "IN",
                                        ArgumentDirection::Out => "OUT",
                                        ArgumentDirection::Inout => "INOUT",
                                    },
                                ),
                            );
                            child(
                                &mut arg,
                                text("SERVER-ARGUMENT-IMPL-POLICY", "USE-ARGUMENT-TYPE"),
                            );
                            child(&mut arguments, arg);
                        }
                        child(&mut op, arguments);
                    }
                    child(&mut list, op);
                }
                child(&mut interface, list);
                interface
            }
        };
        // Keep owned references discoverable without interpreting arbitrary XML.
        element.namespace = None;
        child(&mut elements, element);
    }
    let interface_kinds = bundle
        .interfaces
        .iter()
        .map(|interface| match interface {
            Interface::SenderReceiver { name, .. } => {
                (format!("{prefix}/{name}"), "SENDER-RECEIVER-INTERFACE")
            }
            Interface::ClientServer { name, .. } => {
                (format!("{prefix}/{name}"), "CLIENT-SERVER-INTERFACE")
            }
        })
        .collect::<BTreeMap<_, _>>();
    for swc in &bundle.components {
        if swc.runnables.len() > 64 {
            bail!("SWC exceeds 64 runnables");
        }
        let mut component = named("APPLICATION-SW-COMPONENT-TYPE", &swc.name)?;
        add_ports(&mut component, &swc.ports, &interface_kinds, existing)?;
        if !swc.runnables.is_empty() {
            let behavior_name = "InternalBehavior";
            let behavior_path = format!("{prefix}/{}/{behavior_name}", swc.name);
            let mut behaviors = Element::new("INTERNAL-BEHAVIORS");
            let mut behavior = named("SWC-INTERNAL-BEHAVIOR", behavior_name)?;
            let mut events = Element::new("EVENTS");
            let mut runnables = Element::new("RUNNABLES");
            for runnable in &swc.runnables {
                identifier(&runnable.symbol)?;
                if let Some(period) = runnable.period_seconds {
                    if !period.is_finite() || period <= 0.0 || period > 86400.0 {
                        bail!("timing event period_seconds must be finite and in (0,86400]");
                    }
                    let mut event = named("TIMING-EVENT", &format!("Timer_{}", runnable.name))?;
                    child(
                        &mut event,
                        reference(
                            "START-ON-EVENT-REF",
                            "RUNNABLE-ENTITY",
                            &format!("{behavior_path}/{}", runnable.name),
                        )?,
                    );
                    child(&mut event, text("PERIOD", &period.to_string()));
                    child(&mut events, event);
                }
                let mut run = named("RUNNABLE-ENTITY", &runnable.name)?;
                child(&mut run, text("MINIMUM-START-INTERVAL", "0"));
                child(&mut run, text("CAN-BE-INVOKED-CONCURRENTLY", "false"));
                add_accesses(
                    &mut run,
                    "DATA-READ-ACCESSS",
                    Direction::Require,
                    &runnable.reads,
                    swc,
                    bundle,
                    &prefix,
                )?;
                add_accesses(
                    &mut run,
                    "DATA-WRITE-ACCESSS",
                    Direction::Provide,
                    &runnable.writes,
                    swc,
                    bundle,
                    &prefix,
                )?;
                child(&mut run, text("SYMBOL", &runnable.symbol));
                child(&mut runnables, run);
            }
            if !events.children.is_empty() {
                child(&mut behavior, events);
            }
            child(&mut behavior, runnables);
            child(
                &mut behavior,
                text("SUPPORTS-MULTIPLE-INSTANTIATION", "false"),
            );
            child(&mut behaviors, behavior);
            child(&mut component, behaviors);
        }
        child(&mut elements, component);
    }
    for composition in &bundle.compositions {
        if composition.instances.is_empty()
            || composition.instances.len() > 128
            || composition.connections.len() + composition.delegations.len() > 128
        {
            bail!("composition exceeds instance/connector bounds or has no instances");
        }
        let mut component = named("COMPOSITION-SW-COMPONENT-TYPE", &composition.name)?;
        add_ports(
            &mut component,
            &composition.ports,
            &interface_kinds,
            existing,
        )?;
        let mut instances = Element::new("COMPONENTS");
        for instance in &composition.instances {
            // Nested compositions and foreign component types need additional
            // instance/type expansion and are deliberately rejected here.
            if !bundle
                .components
                .iter()
                .any(|swc| format!("{prefix}/{}", swc.name) == instance.type_ref)
            {
                bail!("composition instances must reference application SWCs in this bundle");
            }
            let mut prototype = named("SW-COMPONENT-PROTOTYPE", &instance.name)?;
            child(
                &mut prototype,
                reference(
                    "TYPE-TREF",
                    "APPLICATION-SW-COMPONENT-TYPE",
                    &instance.type_ref,
                )?,
            );
            child(&mut instances, prototype);
        }
        child(&mut component, instances);
        let mut connectors = Element::new("CONNECTORS");
        for connection in &composition.connections {
            let (provider_instance, provider) =
                endpoint(&connection.provider, composition, bundle, &prefix)?;
            let (requester_instance, requester) =
                endpoint(&connection.requester, composition, bundle, &prefix)?;
            if provider.direction != Direction::Provide
                || requester.direction != Direction::Require
                || provider.interface_ref != requester.interface_ref
            {
                bail!("assembly requires matching interface and P -> R direction");
            }
            let mut connector = named("ASSEMBLY-SW-CONNECTOR", &connection.name)?;
            let mut p = Element::new("PROVIDER-IREF");
            child(
                &mut p,
                reference(
                    "CONTEXT-COMPONENT-REF",
                    "SW-COMPONENT-PROTOTYPE",
                    &format!("{prefix}/{}/{}", composition.name, provider_instance.name),
                )?,
            );
            child(
                &mut p,
                reference(
                    "TARGET-P-PORT-REF",
                    "P-PORT-PROTOTYPE",
                    &format!("{}/{}", provider_instance.type_ref, provider.name),
                )?,
            );
            let mut r = Element::new("REQUESTER-IREF");
            child(
                &mut r,
                reference(
                    "CONTEXT-COMPONENT-REF",
                    "SW-COMPONENT-PROTOTYPE",
                    &format!("{prefix}/{}/{}", composition.name, requester_instance.name),
                )?,
            );
            child(
                &mut r,
                reference(
                    "TARGET-R-PORT-REF",
                    "R-PORT-PROTOTYPE",
                    &format!("{}/{}", requester_instance.type_ref, requester.name),
                )?,
            );
            child(&mut connector, p);
            child(&mut connector, r);
            child(&mut connectors, connector);
        }
        for delegation in &composition.delegations {
            let (instance, inner) = endpoint(&delegation.inner, composition, bundle, &prefix)?;
            let outer = composition
                .ports
                .iter()
                .find(|p| p.name == delegation.outer_port)
                .context("delegation outer port not found")?;
            if inner.direction != outer.direction || inner.interface_ref != outer.interface_ref {
                bail!("delegation requires matching directions and interface");
            }
            let mut connector = named("DELEGATION-SW-CONNECTOR", &delegation.name)?;
            let mut iref = Element::new("INNER-PORT-IREF");
            let (wrapper, role, dest) = match inner.direction {
                Direction::Provide => (
                    "P-PORT-IN-COMPOSITION-INSTANCE-REF",
                    "TARGET-P-PORT-REF",
                    "P-PORT-PROTOTYPE",
                ),
                Direction::Require => (
                    "R-PORT-IN-COMPOSITION-INSTANCE-REF",
                    "TARGET-R-PORT-REF",
                    "R-PORT-PROTOTYPE",
                ),
            };
            let mut context = Element::new(wrapper);
            child(
                &mut context,
                reference(
                    "CONTEXT-COMPONENT-REF",
                    "SW-COMPONENT-PROTOTYPE",
                    &format!("{prefix}/{}/{}", composition.name, instance.name),
                )?,
            );
            child(
                &mut context,
                reference(role, dest, &format!("{}/{}", instance.type_ref, inner.name))?,
            );
            child(&mut iref, context);
            child(&mut connector, iref);
            child(
                &mut connector,
                reference(
                    "OUTER-PORT-REF",
                    dest,
                    &format!("{prefix}/{}/{}", composition.name, outer.name),
                )?,
            );
            child(&mut connectors, connector);
        }
        if !connectors.children.is_empty() {
            child(&mut component, connectors);
        }
        child(&mut elements, component);
    }
    child(&mut package, elements);
    Ok(package)
}

fn add_ports(
    parent: &mut Element,
    ports: &[Port],
    kinds: &BTreeMap<String, &str>,
    existing: &[&ModelObject],
) -> Result<()> {
    if ports.len() > 64 {
        bail!("component exceeds 64 ports");
    }
    let mut container = Element::new("PORTS");
    for port in ports {
        let kind = kinds
            .get(&port.interface_ref)
            .copied()
            .or_else(|| {
                existing
                    .iter()
                    .find(|o| {
                        o.path == port.interface_ref
                            && matches!(
                                o.kind.as_str(),
                                "SENDER-RECEIVER-INTERFACE" | "CLIENT-SERVER-INTERFACE"
                            )
                    })
                    .map(|o| o.kind.as_str())
            })
            .context("port interface not found with supported SR/CS kind")?;
        let (tag, role) = match port.direction {
            Direction::Provide => ("P-PORT-PROTOTYPE", "PROVIDED-INTERFACE-TREF"),
            Direction::Require => ("R-PORT-PROTOTYPE", "REQUIRED-INTERFACE-TREF"),
        };
        let mut prototype = named(tag, &port.name)?;
        child(&mut prototype, reference(role, kind, &port.interface_ref)?);
        child(&mut container, prototype);
    }
    if !container.children.is_empty() {
        child(parent, container);
    }
    Ok(())
}

fn add_accesses(
    run: &mut Element,
    tag: &str,
    direction: Direction,
    accesses: &[Access],
    swc: &Swc,
    bundle: &Bundle,
    prefix: &str,
) -> Result<()> {
    if accesses.len() > 64 {
        bail!("runnable exceeds 64 accesses per direction");
    }
    let mut container = Element::new(tag);
    for (ordinal, access) in accesses.iter().enumerate() {
        let port = swc
            .ports
            .iter()
            .find(|p| p.name == access.port && p.direction == direction)
            .context("runnable access port missing or has wrong direction")?;
        if !bundle.interfaces.iter().any(|i| match i {
            Interface::SenderReceiver {
                name,
                data_elements,
            } => {
                format!("{prefix}/{name}") == port.interface_ref
                    && data_elements.iter().any(|d| d.name == access.data_element)
            }
            _ => false,
        }) {
            bail!("runnable accesses require a data element in an SR interface of this bundle");
        }
        let mut variable = named("VARIABLE-ACCESS", &format!("Access_{ordinal}"))?;
        // Read and write access names share the runnable namespace.
        variable.children[0] = XMLNode::Element(text(
            "SHORT-NAME",
            &format!(
                "{}_{}",
                if direction == Direction::Require {
                    "Read"
                } else {
                    "Write"
                },
                ordinal
            ),
        ));
        let mut accessed = Element::new("ACCESSED-VARIABLE");
        let mut iref = Element::new("AUTOSAR-VARIABLE-IREF");
        child(
            &mut iref,
            reference(
                "PORT-PROTOTYPE-REF",
                if direction == Direction::Require {
                    "R-PORT-PROTOTYPE"
                } else {
                    "P-PORT-PROTOTYPE"
                },
                &format!("{prefix}/{}/{}", swc.name, port.name),
            )?,
        );
        child(
            &mut iref,
            reference(
                "TARGET-DATA-PROTOTYPE-REF",
                "VARIABLE-DATA-PROTOTYPE",
                &format!("{}/{}", port.interface_ref, access.data_element),
            )?,
        );
        child(&mut accessed, iref);
        child(&mut variable, accessed);
        child(&mut container, variable);
    }
    if !container.children.is_empty() {
        child(run, container);
    }
    Ok(())
}

fn endpoint<'a>(
    endpoint: &Endpoint,
    composition: &'a Composition,
    bundle: &'a Bundle,
    prefix: &str,
) -> Result<(&'a Instance, &'a Port)> {
    let instance = composition
        .instances
        .iter()
        .find(|i| i.name == endpoint.instance)
        .context("connector instance not found")?;
    let component = bundle
        .components
        .iter()
        .find(|c| format!("{prefix}/{}", c.name) == instance.type_ref)
        .context("connector component type not found")?;
    let port = component
        .ports
        .iter()
        .find(|p| p.name == endpoint.port)
        .context("connector port not found on instance type")?;
    Ok((instance, port))
}

fn validate_references(objects: &[ModelObject], existing: &[&ModelObject]) -> Result<()> {
    for object in objects {
        for reference in object.refs.iter().filter(|r| r.role != "CONTAINS") {
            let targets = objects
                .iter()
                .chain(existing.iter().copied())
                .filter(|o| o.path == reference.target)
                .collect::<Vec<_>>();
            if targets.len() != 1
                || !reference
                    .dest
                    .as_ref()
                    .is_some_and(|dest| targets[0].kind == *dest)
            {
                bail!(
                    "ASW reference is unresolved or has incompatible DEST: {} {} -> {}",
                    object.path,
                    reference.role,
                    reference.target
                );
            }
        }
    }
    Ok(())
}

fn validate_type_cycles(objects: &[ModelObject], existing: &[&ModelObject]) -> Result<()> {
    let types = objects
        .iter()
        .chain(existing.iter().copied())
        .filter(|o| {
            matches!(
                o.kind.as_str(),
                "IMPLEMENTATION-DATA-TYPE" | "IMPLEMENTATION-DATA-TYPE-ELEMENT"
            )
        })
        .map(|o| (o.path.as_str(), o))
        .collect::<BTreeMap<_, _>>();
    fn visit<'a>(
        path: &'a str,
        types: &BTreeMap<&'a str, &'a ModelObject>,
        active: &mut BTreeSet<&'a str>,
        done: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if done.contains(path) {
            return Ok(());
        }
        if active.len() >= MAX_OBJECTS {
            bail!("ASW implementation type dependency exceeds 512 levels");
        }
        if !active.insert(path) {
            bail!("cyclic ASW implementation type dependency: {path}");
        }
        for reference in &types[path].refs {
            if matches!(
                reference.role.as_str(),
                "CONTAINS" | "IMPLEMENTATION-DATA-TYPE-REF"
            ) && types.contains_key(reference.target.as_str())
            {
                visit(reference.target.as_str(), types, active, done)?;
            }
        }
        active.remove(path);
        done.insert(path);
        Ok(())
    }
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    for object in objects
        .iter()
        .filter(|object| types.contains_key(object.path.as_str()))
    {
        visit(object.path.as_str(), &types, &mut active, &mut done)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn publication_preserves_a_destination_created_after_preflight() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("Bundle.arxml");
        fs::write(&path, b"concurrent writer").unwrap();
        assert!(create_checked(&path, b"new ASW").is_err());
        assert_eq!(fs::read(path).unwrap(), b"concurrent writer");
        assert_eq!(fs::read_dir(root.path()).unwrap().count(), 1);
    }
}
