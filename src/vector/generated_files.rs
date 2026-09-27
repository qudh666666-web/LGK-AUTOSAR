//! Content-based inventory of files touched by one verified generation request.
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use walkdir::WalkDir;
use xmltree::Element;

use crate::project::{normalize_canonical_path, SessionConfig};

const MAX_FILES: usize = 8192;
const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;
const MAX_TOTAL_SIZE: u64 = 512 * 1024 * 1024;
const MAX_RESULTS: usize = 128;

#[derive(Clone, Copy, PartialEq, Eq)]
struct Fingerprint(u64, u64);

pub struct GeneratedSnapshot {
    directory: PathBuf,
    before: BTreeMap<PathBuf, Fingerprint>,
    delivery: Option<Delivery>,
}

struct Delivery {
    root: PathBuf,
    directory: PathBuf,
}

impl GeneratedSnapshot {
    pub fn validate(config: &SessionConfig, request: &Value) -> Result<()> {
        source_directory(config)?;
        delivery_directory(config, request)?;
        Ok(())
    }

    pub fn capture(config: &SessionConfig, request: &Value) -> Result<Self> {
        let directory = source_directory(config)?;
        let delivery = delivery_directory(config, request)?;
        let before = inventory(&directory)?;
        Ok(Self {
            directory,
            before,
            delivery,
        })
    }

    pub fn diff(&self) -> Result<Value> {
        let after = inventory(&self.directory)?;
        let paths = self
            .before
            .keys()
            .chain(after.keys())
            .collect::<BTreeSet<_>>();
        let mut changed_count = 0usize;
        let mut needs_sync_count = 0usize;
        let mut files = Vec::new();
        for path in paths {
            let old = self.before.get(path);
            let new = after.get(path);
            if old == new {
                continue;
            }
            changed_count += 1;
            let state = match (old, new) {
                (None, Some(_)) => "added",
                (Some(_), None) => "deleted",
                _ => "modified",
            };
            let relative = path.strip_prefix(&self.directory)?;
            let relative_text = relative.to_string_lossy().replace('\\', "/");
            let mut item = json!({"path": relative_text, "change": state});
            if let Some(delivery) = &self.delivery {
                let target = delivery.directory.join(relative);
                ensure_destination(&delivery.root, &target)?;
                let synchronized = match new {
                    Some(source) => target.is_file() && fingerprint(&target)? == *source,
                    None => !target.exists(),
                };
                if !synchronized {
                    needs_sync_count += 1;
                }
                item["needs_sync"] = json!(!synchronized);
                item["destination"] = json!(target);
            }
            if files.len() < MAX_RESULTS {
                files.push(item);
            }
        }
        Ok(json!({
            "source_directory": self.directory,
            "changed_count": changed_count,
            "files": files,
            "truncated": changed_count > MAX_RESULTS,
            "delivery_directory": self.delivery.as_ref().map(|delivery| &delivery.directory),
            "needs_sync_count": self.delivery.as_ref().map(|_| needs_sync_count),
        }))
    }
}

fn source_directory(config: &SessionConfig) -> Result<PathBuf> {
    let dpa = Element::parse(File::open(config.dpa_file()?)?)?;
    let raw = dpa
        .get_child("Folders")
        .and_then(|folders| folders.get_child("GenData"))
        .and_then(Element::get_text)
        .ok_or_else(|| anyhow::anyhow!("DPA Folders/GenData is missing"))?;
    let raw = raw.trim().replace('\\', "/");
    if raw.is_empty() {
        bail!("DPA Folders/GenData is empty");
    }
    let parent = config
        .project_path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("project path has no parent"))?;
    let directory = normalize_canonical_path(
        config
            .project_path
            .join(raw)
            .canonicalize()
            .context("DPA GenData directory not found")?,
    );
    if !directory.is_dir() || !directory.starts_with(parent) {
        bail!("DPA GenData must be a directory within the configured project parent");
    }
    Ok(directory)
}

fn delivery_directory(config: &SessionConfig, request: &Value) -> Result<Option<Delivery>> {
    let Some(delivery) = request.get("delivery") else {
        return Ok(None);
    };
    let root = delivery
        .get("root")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("delivery.root must be an absolute project directory"))?;
    let root_path = Path::new(root);
    if !root_path.is_absolute() {
        bail!("delivery.root must be absolute");
    }
    let root = normalize_canonical_path(root_path.canonicalize()?);
    if !root.is_dir() || !config.project_path.starts_with(&root) {
        bail!("delivery.root must contain the DaVinci Cfg directory");
    }
    let relative = delivery
        .get("directory")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("delivery.directory must be relative to delivery.root"))?;
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        bail!("delivery.directory must be a non-empty relative path without traversal");
    }
    let directory = normalize_canonical_path(
        root.join(relative)
            .canonicalize()
            .context("delivery.directory must already exist")?,
    );
    if !directory.is_dir() || !directory.starts_with(&root) {
        bail!("delivery.directory must remain inside delivery.root");
    }
    Ok(Some(Delivery { root, directory }))
}

fn ensure_destination(root: &Path, target: &Path) -> Result<()> {
    let mut existing = target;
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid delivery path"))?;
    }
    let canonical = normalize_canonical_path(existing.canonicalize()?);
    if !canonical.starts_with(root) {
        bail!(
            "delivery target resolves outside delivery.root: {}",
            target.display()
        );
    }
    Ok(())
}

fn inventory(directory: &Path) -> Result<BTreeMap<PathBuf, Fingerprint>> {
    let mut result = BTreeMap::new();
    let mut total = 0u64;
    for entry in WalkDir::new(directory).follow_links(false).into_iter() {
        let entry = entry?;
        if entry.file_type().is_symlink() {
            bail!(
                "generated directory contains a symlink: {}",
                entry.path().display()
            );
        }
        if !entry.file_type().is_file() {
            continue;
        }
        if result.len() >= MAX_FILES {
            bail!("generated directory exceeds {MAX_FILES} files");
        }
        let size = entry.metadata()?.len();
        total = total
            .checked_add(size)
            .ok_or_else(|| anyhow::anyhow!("generated size overflow"))?;
        if size > MAX_FILE_SIZE || total > MAX_TOTAL_SIZE {
            bail!("generated files exceed bounded inventory limits");
        }
        let fingerprint = fingerprint(entry.path())?;
        result.insert(entry.into_path(), fingerprint);
    }
    Ok(result)
}

fn fingerprint(path: &Path) -> Result<Fingerprint> {
    let mut file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > MAX_FILE_SIZE {
        bail!("file exceeds inventory limit: {}", path.display());
    }
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        buffer[..count].hash(&mut hasher);
    }
    Ok(Fingerprint(size, hasher.finish()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn content_changes_and_delivery_status_are_reported() {
        let root = tempfile::tempdir().unwrap();
        let root_path = normalize_canonical_path(root.path().canonicalize().unwrap());
        let source = root_path.join("GenData");
        let target = root_path.join("Delivery");
        fs::create_dir(&source).unwrap();
        fs::create_dir(&target).unwrap();
        fs::write(source.join("Com.c"), b"old").unwrap();
        fs::write(target.join("Com.c"), b"old").unwrap();
        let snapshot = GeneratedSnapshot {
            before: inventory(&source).unwrap(),
            directory: source.clone(),
            delivery: Some(Delivery {
                root: root_path,
                directory: target,
            }),
        };
        fs::write(source.join("Com.c"), b"new").unwrap();
        let diff = snapshot.diff().unwrap();
        assert_eq!(diff["changed_count"], 1);
        assert_eq!(diff["needs_sync_count"], 1);
        assert_eq!(diff["files"][0]["path"], "Com.c");
        assert_eq!(diff["files"][0]["change"], "modified");
    }
}
