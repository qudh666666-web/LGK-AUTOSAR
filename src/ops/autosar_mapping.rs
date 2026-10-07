//! Read-only evidence of saved mapping rows; no scheduling validity inference.
use super::*;
use crate::vector::module_index::ModuleIndex;

#[derive(Debug)]
pub(super) struct MappingRow {
    category: &'static str,
    kind: String,
    owner_path: String,
    file: String,
    xml_location: String,
    refs: Vec<ModelRef>,
    reference_locations: Vec<String>,
    instance_context: Vec<ModelRef>,
    required_roles: Vec<Vec<&'static str>>,
    fields: BTreeMap<String, String>,
}

pub fn inspect_mapping(config: &SessionConfig, request: &Value) -> Result<Value> {
    let started = Instant::now();
    let category = match request.get("category") {
        None => "all",
        Some(Value::String(value))
            if matches!(value.as_str(), "all" | "ports" | "data" | "tasks") =>
        {
            value
        }
        _ => bail!("category must be all, ports, data, or tasks"),
    };
    let limit = bounded_usize(request, "limit", 8, 32)?;
    let offset = request
        .get("offset")
        .map_or(Some(0), Value::as_u64)
        .filter(|n| usize::try_from(*n).is_ok())
        .ok_or_else(|| anyhow::anyhow!("offset must be a nonnegative platform-sized integer"))?
        as usize;
    let issues_only = match request.get("issues_only") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => bail!("issues_only must be a boolean"),
    };
    let summary_only = match request.get("summary_only") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => bail!("summary_only must be a boolean"),
    };
    if request.get("path_prefix").is_some_and(|v| !v.is_string()) {
        bail!("path_prefix must be a string");
    }
    let prefix = optional_string(request, "path_prefix");
    let mut index = ModelIndex::load_with_mappings(config, request, category != "tasks")?;
    let ecuc = if matches!(category, "tasks" | "all") {
        load_task_rows(config, &mut index)?
    } else {
        json!({"rte_available":false,"os_available":false,"scanned":false})
    };
    let mut counts = BTreeMap::<&str, usize>::new();
    let mut without_task = 0usize;
    let mut missing_fields = 0usize;
    let mut unresolved = 0usize;
    let mut mismatched = 0usize;
    let mut selected = Vec::new();
    for row in &index.mappings {
        if category != "all" && row.category != category
            || prefix.is_some_and(|p| !row.owner_path.starts_with(p))
        {
            continue;
        }
        *counts.entry(row.category).or_default() += 1;
        let missing = missing_roles(row);
        let task_missing =
            row.category == "tasks" && missing.iter().any(|role| role.contains("MappedToTaskRef"));
        without_task += usize::from(task_missing);
        missing_fields += usize::from(!missing.is_empty());
        let absent = row
            .refs
            .iter()
            .filter(|r| !index.by_path.contains_key(&r.target))
            .count();
        let mismatch = row
            .refs
            .iter()
            .filter(|r| resolution(&index, r) == "dest_mismatch")
            .count();
        unresolved += absent;
        mismatched += mismatch;
        if !issues_only || !missing.is_empty() || absent > 0 || mismatch > 0 {
            selected.push(row);
        }
    }
    let count = selected.len();
    let mut returned = Vec::new();
    if !summary_only {
        for row in selected.into_iter().skip(offset).take(limit) {
            let missing = missing_roles(row);
            let refs = row.refs.iter().zip(&row.reference_locations).take(16).map(|(r,location)| json!({"role":r.role,"target":r.target,"dest":r.dest,"xml_location":location,
                "resolution":resolution(&index,r),"definitions_in_scope":index.by_path.get(&r.target).map_or(0,Vec::len)})).collect::<Vec<_>>();
            returned.push(json!({"category":row.category,"kind":row.kind,"owner_path":row.owner_path,
                "file":row.file,"xml_location":row.xml_location,"references":refs,
                "references_omitted":row.refs.len().saturating_sub(16),"missing_roles":missing,"instance_context":row.instance_context,
                "fields":row.fields,"task_assignment":if row.category!="tasks" {Value::Null} else if missing.iter().any(|r| r.contains("MappedToTaskRef")) {json!("no_task_reference_requires_review")} else {json!("task_reference_present")}}));
        }
    }
    let next = offset.saturating_add(returned.len());
    let result = json!({"scope":scope_name(request)?,"category":category,"summary_only":summary_only,
        "model_files_scanned":index.files_scanned,"model_bytes_scanned":index.bytes_scanned,"ecuc":ecuc,
        "by_category":counts,"rows_with_missing_roles":missing_fields,"rows_without_task_reference":without_task,
        "unresolved_references_in_scope":unresolved,"dest_mismatches_in_scope":mismatched,
        "count":count,"rows":returned,"limit":limit,"offset":offset,
        "truncated":!summary_only && next<count,"next_offset":if !summary_only && next<count {Some(next)} else {None},
        "elapsed_ms":started.elapsed().as_millis(),
        "interpretation":"Saved mapping rows only; no endpoint instance expansion or DaVinci validation. Missing tasks may use direct calls. References outside this scope are not configuration errors."});
    if serde_json::to_vec(&result)?.len() > 64 * 1024 {
        bail!("mapping response exceeds 64 KiB; narrow path_prefix or limit, or use summary_only:true");
    }
    Ok(result)
}

fn resolution(index: &ModelIndex, reference: &ModelRef) -> &'static str {
    match index.by_path.get(&reference.target) {
        None => "unresolved_in_scope",
        Some(objects)
            if reference
                .dest
                .as_ref()
                .is_some_and(|dest| !objects.iter().any(|n| index.objects[*n].kind == *dest)) =>
        {
            "dest_mismatch"
        }
        Some(_) => "present_in_scope",
    }
}

fn missing_roles(row: &MappingRow) -> Vec<String> {
    row.required_roles
        .iter()
        .filter(|roles| !row.refs.iter().any(|r| roles.contains(&r.role.as_str())))
        .map(|roles| roles.join("|"))
        .collect()
}

pub(super) fn collect_model_rows(
    element: &Element,
    file: &str,
    owner: &str,
    location: &str,
    rows: &mut Vec<MappingRow>,
) {
    let current = direct_child_text(element, "SHORT-NAME")
        .map_or_else(|| owner.to_string(), |name| format!("{owner}/{name}"));
    let roles = match element.name.as_str() {
        "ASSEMBLY-SW-CONNECTOR" => Some((
            "ports",
            vec![vec!["TARGET-P-PORT-REF"], vec!["TARGET-R-PORT-REF"]],
        )),
        "DELEGATION-SW-CONNECTOR" => Some((
            "ports",
            vec![
                vec!["OUTER-PORT-REF"],
                vec!["TARGET-P-PORT-REF", "TARGET-R-PORT-REF"],
            ],
        )),
        "SENDER-RECEIVER-TO-SIGNAL-MAPPING" => Some((
            "data",
            vec![vec!["TARGET-DATA-PROTOTYPE-REF"], vec!["SYSTEM-SIGNAL-REF"]],
        )),
        "SENDER-RECEIVER-TO-SIGNAL-GROUP-MAPPING" => Some((
            "data",
            vec![vec!["TARGET-DATA-PROTOTYPE-REF"], vec!["SIGNAL-GROUP-REF"]],
        )),
        _ => None,
    };
    if let Some((category, required_roles)) = roles {
        let mut refs = Vec::new();
        let mut reference_locations = Vec::new();
        collect_located_refs(element, location, true, &mut refs, &mut reference_locations);
        rows.push(MappingRow {
            category,
            kind: element.name.clone(),
            owner_path: current.clone(),
            file: file.to_string(),
            xml_location: location.to_string(),
            refs,
            reference_locations,
            instance_context: Vec::new(),
            required_roles,
            fields: BTreeMap::new(),
        });
    }
    let mut ordinals = BTreeMap::<&str, usize>::new();
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            let ordinal = ordinals.entry(&child.name).or_default();
            *ordinal += 1;
            collect_model_rows(
                child,
                file,
                &current,
                &format!("{location}/{}[{ordinal}]", child.name),
                rows,
            );
        }
    }
}

fn collect_located_refs(
    element: &Element,
    location: &str,
    root: bool,
    refs: &mut Vec<ModelRef>,
    locations: &mut Vec<String>,
) {
    if !root && direct_child_text(element, "SHORT-NAME").is_some() {
        return;
    }
    if (element.name.ends_with("-REF") || element.name.ends_with("-TREF"))
        && !element
            .children
            .iter()
            .any(|n| matches!(n, XMLNode::Element(_)))
    {
        if let Some(target) = element
            .get_text()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
        {
            refs.push(ModelRef {
                role: element.name.clone(),
                target,
                dest: element.attributes.get("DEST").cloned(),
            });
            locations.push(location.to_string());
        }
        return;
    }
    let mut ordinals = BTreeMap::<&str, usize>::new();
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            let ordinal = ordinals.entry(&child.name).or_default();
            *ordinal += 1;
            collect_located_refs(
                child,
                &format!("{location}/{}[{ordinal}]", child.name),
                false,
                refs,
                locations,
            );
        }
    }
}

fn load_task_rows(config: &SessionConfig, index: &mut ModelIndex) -> Result<Value> {
    let project_root = fs::canonicalize(&config.project_path)?;
    let modules = ModuleIndex::load(config)?;
    let rte = modules
        .all()
        .iter()
        .any(|m| m.module.eq_ignore_ascii_case("Rte"));
    let os = modules
        .all()
        .iter()
        .any(|m| m.module.eq_ignore_ascii_case("Os"));
    let mut files = BTreeSet::new();
    let mut bytes = 0u64;
    for module in modules
        .all()
        .iter()
        .filter(|m| m.module.eq_ignore_ascii_case("Rte") || m.module.eq_ignore_ascii_case("Os"))
    {
        let file =
            fs::canonicalize(&module.config_path).context("canonicalize Rte/Os configuration")?;
        if !file.starts_with(&project_root) {
            bail!("Rte/Os configuration must be within the selected project");
        }
        if !files.insert(file.clone()) {
            continue;
        }
        let size = fs::metadata(&file)?.len();
        if size > MAX_FILE_SIZE {
            bail!("Rte/Os mapping file exceeds 64 MiB");
        }
        bytes += size;
        if bytes + index.bytes_scanned > MAX_TOTAL_BYTES {
            bail!("mapping scope exceeds 256 MiB");
        }
        let root =
            Element::parse(fs::File::open(&file)?).context("parse Rte/Os mapping configuration")?;
        let relative = file
            .strip_prefix(&project_root)?
            .to_string_lossy()
            .replace('\\', "/");
        if !index.objects.iter().any(|object| object.file == relative) {
            collect_objects(&root, &relative, &[], None, &mut index.objects);
        }
        collect_task_rows(
            &root,
            &relative,
            "",
            "/AUTOSAR[1]",
            &[],
            &mut index.mappings,
        );
    }
    index.by_path.clear();
    for (n, object) in index.objects.iter().enumerate() {
        index
            .by_path
            .entry(object.path.clone())
            .or_default()
            .push(n);
    }
    Ok(
        json!({"rte_available":rte,"os_available":os,"scanned":true,"files_scanned":files.len(),"bytes_scanned":bytes}),
    )
}

fn collect_task_rows(
    element: &Element,
    file: &str,
    owner: &str,
    location: &str,
    context: &[ModelRef],
    rows: &mut Vec<MappingRow>,
) {
    let current = direct_child_text(element, "SHORT-NAME")
        .map_or_else(|| owner.to_string(), |name| format!("{owner}/{name}"));
    let mut owned_context = Vec::new();
    let mut context = context;
    if element.name == "ECUC-CONTAINER-VALUE" {
        let definition = direct_child_text(element, "DEFINITION-REF").unwrap_or_default();
        let kind = definition.rsplit('/').next().unwrap_or_default();
        if matches!(kind, "RteSwComponentInstance" | "RteBswModuleInstance") {
            if let Some(XMLNode::Element(values)) = element
                .children
                .iter()
                .find(|n| matches!(n,XMLNode::Element(e) if e.name=="REFERENCE-VALUES"))
            {
                for node in &values.children {
                    if let XMLNode::Element(value) = node {
                        let definition =
                            direct_child_text(value, "DEFINITION-REF").unwrap_or_default();
                        if let Some(XMLNode::Element(target)) = value
                            .children
                            .iter()
                            .find(|n| matches!(n,XMLNode::Element(e) if e.name=="VALUE-REF"))
                        {
                            if let Some(text) = target
                                .get_text()
                                .map(|t| t.trim().to_string())
                                .filter(|t| !t.is_empty())
                            {
                                owned_context.push(ModelRef {
                                    role: definition
                                        .rsplit('/')
                                        .next()
                                        .unwrap_or_default()
                                        .to_string(),
                                    target: text,
                                    dest: target.attributes.get("DEST").cloned(),
                                });
                            }
                        }
                    }
                }
            }
            context = &owned_context;
        }
        if matches!(kind, "RteEventToTaskMapping" | "RteBswEventToTaskMapping") {
            let bsw = kind == "RteBswEventToTaskMapping";
            let mut row = MappingRow {
                category: "tasks",
                kind: kind.to_string(),
                owner_path: current.clone(),
                file: file.to_string(),
                xml_location: location.to_string(),
                refs: Vec::new(),
                reference_locations: Vec::new(),
                instance_context: context.to_vec(),
                required_roles: if bsw {
                    vec![vec!["RteBswEventRef"], vec!["RteBswMappedToTaskRef"]]
                } else {
                    vec![vec!["RteEventRef"], vec!["RteMappedToTaskRef"]]
                },
                fields: BTreeMap::new(),
            };
            for group in ["PARAMETER-VALUES", "REFERENCE-VALUES"] {
                if let Some(XMLNode::Element(values)) = element
                    .children
                    .iter()
                    .find(|n| matches!(n,XMLNode::Element(e) if e.name==group))
                {
                    let mut ordinals = BTreeMap::<&str, usize>::new();
                    for node in &values.children {
                        if let XMLNode::Element(value) = node {
                            let ordinal = ordinals.entry(&value.name).or_default();
                            *ordinal += 1;
                            let definition =
                                direct_child_text(value, "DEFINITION-REF").unwrap_or_default();
                            let role = definition.rsplit('/').next().unwrap_or_default();
                            if let Some(XMLNode::Element(target)) = value
                                .children
                                .iter()
                                .find(|n| matches!(n,XMLNode::Element(e) if e.name=="VALUE-REF"))
                            {
                                if let Some(text) = target
                                    .get_text()
                                    .map(|t| t.trim().to_string())
                                    .filter(|t| !t.is_empty())
                                {
                                    row.refs.push(ModelRef {
                                        role: role.to_string(),
                                        target: text,
                                        dest: target.attributes.get("DEST").cloned(),
                                    });
                                    row.reference_locations.push(format!(
                                        "{location}/{group}[1]/{}[{ordinal}]/VALUE-REF[1]",
                                        value.name
                                    ));
                                }
                            } else if role.contains("PositionInTask")
                                || role.contains("ActivationOffset")
                                || role.contains("ImmediateRestart")
                                || role == "RteUsedOsActivation"
                            {
                                if let Some(text) = direct_child_text(value, "VALUE") {
                                    row.fields.insert(role.to_string(), text);
                                }
                            }
                        }
                    }
                }
            }
            rows.push(row);
        }
    }
    let mut ordinals = BTreeMap::<&str, usize>::new();
    for node in &element.children {
        if let XMLNode::Element(child) = node {
            let ordinal = ordinals.entry(&child.name).or_default();
            *ordinal += 1;
            collect_task_rows(
                child,
                file,
                &current,
                &format!("{location}/{}[{ordinal}]", child.name),
                context,
                rows,
            );
        }
    }
}
