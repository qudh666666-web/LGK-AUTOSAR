use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Serialize;
use xmltree::{Element, XMLNode};

use crate::project::SessionConfig;

// DPA 中一个 Module 条目与它实际 ECUC 文件之间的映射。
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct ModuleInfo {
    pub module: String,
    pub name: String,
    pub config_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ModuleIndex {
    modules: Vec<ModuleInfo>,
}

impl ModuleIndex {
    pub fn load(config: &SessionConfig) -> Result<Self> {
        // DPA 只负责回答“模块配置文件在哪”；它不是 BSWMD 参数模板。
        let dpa_path = config.dpa_file()?;
        let root = Element::parse(
            File::open(&dpa_path)
                .with_context(|| format!("cannot open dpa file: {}", dpa_path.display()))?,
        )
        .with_context(|| format!("failed to parse dpa file: {}", dpa_path.display()))?;
        let splitter = child(&root, "EcucSplitter")
            .ok_or_else(|| anyhow::anyhow!("EcucSplitter not found in {}", dpa_path.display()))?;

        let mut modules = Vec::new();
        // 一个 Splitter 可能包含多个 Module，它们共享同一份 ECUC ARXML 文件。
        for node in &splitter.children {
            let XMLNode::Element(splitter_entry) = node else {
                continue;
            };
            if splitter_entry.name != "Splitter" {
                continue;
            }
            let Some(file) = splitter_entry.attributes.get("File") else {
                continue;
            };
            for module in child_elements(splitter_entry, "Module") {
                let Some(name) = module.attributes.get("Name") else {
                    continue;
                };
                modules.push(ModuleInfo {
                    module: name.clone(),
                    name: name.clone(),
                    config_path: normalize_relative(&config.project_path, file),
                });
            }
        }

        if modules.is_empty() {
            bail!(
                "no modules found under EcucSplitter/Splitter/Module in {}",
                dpa_path.display()
            );
        }
        modules.sort_by(|left, right| left.module.cmp(&right.module));
        Ok(Self { modules })
    }

    pub fn all(&self) -> &[ModuleInfo] {
        &self.modules
    }

    pub fn find(&self, module: &str) -> Result<&ModuleInfo> {
        // 对外部请求使用大小写无关匹配，避免 Com/com 造成无意义失败。
        self.modules
            .iter()
            .find(|item| item.module.eq_ignore_ascii_case(module))
            .ok_or_else(|| {
                let query = module.to_ascii_lowercase();
                let mut related: Vec<_> = self.modules.iter().filter(|item| {
                    let name = item.module.to_ascii_lowercase();
                    name.contains(&query) || query.contains(&name)
                }).collect();
                // Prefer the closest prefix (e.g. a driver package -> CanTrcv,
                // before Can). Never substitute a suggestion in a mutation.
                related.sort_by(|a, b| b.module.len().cmp(&a.module.len()).then(a.module.cmp(&b.module)));
                let source: Vec<_> = if related.is_empty() { self.modules.iter().collect() } else { related };
                let candidates: Vec<_> = source.iter().take(8).map(|item| &item.module).collect();
                crate::diagnostic::failure(
                    "MODULE_NOT_FOUND", "module not found in current project", "module",
                    serde_json::json!({"requested": module, "candidates": candidates,
                        "candidates_truncated": source.len() > 8,
                        "available_count": self.modules.len(),
                        "next": "Use a listed module, or find_module with module=all to inspect all names"}),
                )
            })
    }
}

fn child<'a>(element: &'a Element, name: &str) -> Option<&'a Element> {
    child_elements(element, name).into_iter().next()
}

fn child_elements<'a>(element: &'a Element, name: &str) -> Vec<&'a Element> {
    element
        .children
        .iter()
        .filter_map(|node| match node {
            XMLNode::Element(child) if child.name == name => Some(child),
            _ => None,
        })
        .collect()
}

fn normalize_relative(project: &Path, raw: &str) -> PathBuf {
    let normalized = raw.replace('/', "\\");
    let relative = normalized
        .strip_prefix(".\\")
        .unwrap_or(&normalized)
        .to_string();
    project.join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn suggestions_are_bounded_and_prefer_specific_project_names() {
        let mut names = vec!["Can".to_owned(), "CanTrcv".to_owned()];
        names.extend((0..12).map(|i| format!("VendorModule{i}")));
        let index = ModuleIndex {
            modules: names
                .into_iter()
                .map(|module| ModuleInfo {
                    name: module.clone(),
                    module,
                    config_path: PathBuf::from("unused.arxml"),
                })
                .collect(),
        };
        let error = index.find("CanTrcv_Package").unwrap_err();
        let value: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
        assert_eq!(
            value["details"]["candidates"],
            serde_json::json!(["CanTrcv", "Can"])
        );
        let error = index.find("Unrelated\"\n").unwrap_err();
        let value: serde_json::Value = serde_json::from_str(&error.to_string()).unwrap();
        assert_eq!(value["details"]["candidates"].as_array().unwrap().len(), 8);
        assert_eq!(value["details"]["candidates_truncated"], true);
        assert_eq!(value["details"]["available_count"], 14);
    }
}
