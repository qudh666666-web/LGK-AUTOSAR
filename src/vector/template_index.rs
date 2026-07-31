use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use walkdir::WalkDir;
use xmltree::{Element, XMLNode};

use crate::project::SessionConfig;
use crate::vector::module_index::ModuleIndex;
use crate::vector::search::{child_text, descendants};

#[derive(Debug)]
pub struct TemplateIndex {
    pub module: String,
    pub definition_ref: String,
    pub path: PathBuf,
    pub root: Element,
}

impl TemplateIndex {
    pub fn load(config: &SessionConfig, module: &str) -> Result<Self> {
        let modules = ModuleIndex::load(config)?;
        let module_info = modules.find(module)?;
        let definition_ref = read_module_definition_ref(&module_info.config_path, module)?;
        let short_name = definition_ref
            .rsplit('/')
            .find(|part| !part.is_empty())
            .ok_or_else(|| anyhow::anyhow!("invalid module definition ref: {definition_ref}"))?;

        let mut candidates = Vec::new();
        for entry in WalkDir::new(&config.tool_path)
            .follow_links(false)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().is_file())
        {
            let path = entry.path();
            if !path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|value| value.eq_ignore_ascii_case("arxml"))
            {
                continue;
            }
            let Ok(raw) = fs::read(path) else {
                continue;
            };
            let text = String::from_utf8_lossy(&raw);
            if !text.contains("<ECUC-MODULE-DEF") || !text.contains(short_name) {
                continue;
            }
            let Ok(root) = Element::parse(Cursor::new(&raw)) else {
                continue;
            };
            if module_definition_paths(&root)
                .iter()
                .any(|candidate| candidate == &definition_ref)
            {
                candidates.push((path.to_path_buf(), root));
            }
        }

        match candidates.len() {
            0 => bail!("no template arxml found for module definition: {definition_ref}"),
            1 => {
                let (path, root) = candidates.pop().expect("length checked");
                Ok(Self {
                    module: module.to_string(),
                    definition_ref,
                    path,
                    root,
                })
            }
            _ => bail!(
                "found multiple template arxml files for {}: {}",
                definition_ref,
                candidates
                    .iter()
                    .map(|(path, _)| path.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    pub fn module_definition(&self) -> Result<&Element> {
        let mut module_defs = Vec::new();
        descendants(&self.root, "ECUC-MODULE-DEF", &mut module_defs);
        module_defs
            .into_iter()
            .find(|element| {
                child_text(element, "SHORT-NAME")
                    .is_some_and(|name| self.definition_ref.ends_with(&format!("/{name}")))
            })
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "module definition {} not found in {}",
                    self.definition_ref,
                    self.path.display()
                )
            })
    }
}

pub fn read_module_definition_ref(config_path: &Path, module: &str) -> Result<String> {
    let root = Element::parse(
        fs::File::open(config_path)
            .with_context(|| format!("cannot open module config: {}", config_path.display()))?,
    )
    .with_context(|| {
        format!(
            "failed to parse module config arxml: {}",
            config_path.display()
        )
    })?;
    let mut values = Vec::new();
    descendants(&root, "ECUC-MODULE-CONFIGURATION-VALUES", &mut values);
    let mut candidates = values
        .into_iter()
        .filter_map(|value| {
            let name = child_text(value, "SHORT-NAME")?;
            let definition_ref = child_text(value, "DEFINITION-REF")?;
            Some((name, definition_ref))
        })
        .collect::<Vec<_>>();
    if let Some((_, definition_ref)) = candidates
        .iter()
        .find(|(name, _)| name.eq_ignore_ascii_case(module))
    {
        return Ok(definition_ref.clone());
    }
    if let Some((_, definition_ref)) = candidates.iter().find(|(_, definition_ref)| {
        definition_ref
            .rsplit('/')
            .find(|part| !part.is_empty())
            .is_some_and(|name| name.eq_ignore_ascii_case(module))
    }) {
        return Ok(definition_ref.clone());
    }
    if candidates.len() == 1 {
        return Ok(candidates.pop().expect("length checked").1);
    }
    bail!(
        "cannot identify module {module} in {} from definition refs: {}",
        config_path.display(),
        candidates
            .iter()
            .map(|(_, definition_ref)| definition_ref.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn module_definition_paths(root: &Element) -> Vec<String> {
    let mut output = Vec::new();
    collect_definition_paths(root, &mut Vec::new(), &mut output);
    output
}

fn collect_definition_paths(
    element: &Element,
    packages: &mut Vec<String>,
    output: &mut Vec<String>,
) {
    let is_package = element.name == "AR-PACKAGE";
    if is_package {
        if let Some(name) = child_text(element, "SHORT-NAME") {
            packages.push(name);
        }
    }

    if element.name == "ECUC-MODULE-DEF" {
        if let Some(name) = child_text(element, "SHORT-NAME") {
            output.push(format!("/{}/{}", packages.join("/"), name));
        }
    }

    for node in &element.children {
        if let XMLNode::Element(child) = node {
            collect_definition_paths(child, packages, output);
        }
    }

    if is_package && !packages.is_empty() {
        packages.pop();
    }
}
