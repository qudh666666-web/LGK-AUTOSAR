use std::fs;
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};

use crate::project::{normalize_canonical_path, SessionConfig};

const MAX_CHECKS: usize = 64;
const MAX_PATTERNS_PER_KIND: usize = 128;
const MAX_FILE_SIZE: u64 = 64 * 1024 * 1024;

pub fn execute(config: &SessionConfig, request: &Value) -> Result<Value> {
    let root_text = required_string(request, "root")?;
    let requested_root = Path::new(root_text);
    if !requested_root.is_absolute() {
        bail!("root must be an absolute project directory");
    }
    let root =
        normalize_canonical_path(requested_root.canonicalize().with_context(|| {
            format!("verification root not found: {}", requested_root.display())
        })?);
    if !root.is_dir() {
        bail!("verification root is not a directory: {}", root.display());
    }
    if !config.project_path.starts_with(&root) {
        bail!(
            "verification root must contain the configured DaVinci project: root={}, project={}",
            root.display(),
            config.project_path.display()
        );
    }

    let checks = request
        .get("checks")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("checks must be a non-empty array"))?;
    if checks.is_empty() {
        bail!("checks must be a non-empty array");
    }
    if checks.len() > MAX_CHECKS {
        bail!("checks may contain at most {MAX_CHECKS} items");
    }
    let enforce = optional_bool(request, "enforce")?.unwrap_or(true);

    let mut results = Vec::with_capacity(checks.len());
    let mut failed_paths = Vec::new();
    for check in checks {
        let result = verify_one(&root, check)?;
        if result.get("passed").and_then(Value::as_bool) != Some(true) {
            failed_paths.push(
                result
                    .get("path")
                    .and_then(Value::as_str)
                    .unwrap_or("<unknown>")
                    .to_string(),
            );
        }
        results.push(result);
    }

    let passed = failed_paths.is_empty();
    let response = json!({
        "root": root,
        "passed": passed,
        "checks": results
    });
    if !passed && enforce {
        bail!(
            "delivery verification failed for: {}; rerun with enforce=false to inspect structured details",
            failed_paths.join(", ")
        );
    }
    Ok(response)
}

fn verify_one(root: &Path, check: &Value) -> Result<Value> {
    let relative_text = required_string(check, "path")?;
    let path = resolve_relative_file(root, relative_text)?;
    let required = optional_string_array(check, "must_contain")?;
    let forbidden = optional_string_array(check, "must_not_contain")?;
    let same_as_text = optional_string(check, "same_as")?;
    let same_as_path = same_as_text
        .map(|value| resolve_relative_file(root, value))
        .transpose()?;

    let content = read_optional_file(&path)?;
    let comparison = same_as_path
        .as_deref()
        .map(read_optional_file)
        .transpose()?
        .flatten();
    let exists = content.is_some();
    let same_as_exists = same_as_path.as_ref().map(|_| comparison.is_some());
    let synchronized = same_as_path.as_ref().map(|_| {
        content
            .as_ref()
            .zip(comparison.as_ref())
            .is_some_and(|(left, right)| left == right)
    });

    let missing_required = required
        .iter()
        .filter(|pattern| {
            content
                .as_ref()
                .is_none_or(|bytes| !contains_bytes(bytes, pattern.as_bytes()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let forbidden_found = forbidden
        .iter()
        .filter(|pattern| {
            content
                .as_ref()
                .is_some_and(|bytes| contains_bytes(bytes, pattern.as_bytes()))
        })
        .cloned()
        .collect::<Vec<_>>();
    let passed = exists
        && synchronized.unwrap_or(true)
        && missing_required.is_empty()
        && forbidden_found.is_empty();

    Ok(json!({
        "path": relative_text,
        "exists": exists,
        "same_as": same_as_text,
        "same_as_exists": same_as_exists,
        "synchronized": synchronized,
        "missing_required": missing_required,
        "forbidden_found": forbidden_found,
        "passed": passed
    }))
}

fn resolve_relative_file(root: &Path, value: &str) -> Result<PathBuf> {
    let relative = Path::new(value);
    if relative.is_absolute() || value.trim().is_empty() {
        bail!("verification paths must be non-empty paths relative to root: {value}");
    }
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_) | Component::CurDir))
    {
        bail!("verification paths cannot contain parent, root, or prefix components: {value}");
    }
    let joined = root.join(relative);
    if joined.exists() {
        let canonical = normalize_canonical_path(
            joined
                .canonicalize()
                .with_context(|| format!("cannot resolve verification path: {value}"))?,
        );
        if !canonical.starts_with(root) {
            bail!("verification path escapes root through a link: {value}");
        }
        return Ok(canonical);
    }
    Ok(joined)
}

fn read_optional_file(path: &Path) -> Result<Option<Vec<u8>>> {
    let metadata = match fs::metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot inspect {}", path.display()))
        }
    };
    if !metadata.is_file() {
        bail!("verification path is not a file: {}", path.display());
    }
    if metadata.len() > MAX_FILE_SIZE {
        bail!(
            "verification file exceeds {} bytes: {}",
            MAX_FILE_SIZE,
            path.display()
        );
    }
    fs::read(path)
        .with_context(|| format!("cannot read verification file: {}", path.display()))
        .map(Some)
}

fn contains_bytes(content: &[u8], pattern: &[u8]) -> bool {
    !pattern.is_empty()
        && content
            .windows(pattern.len())
            .any(|window| window == pattern)
}

fn required_string<'a>(value: &'a Value, name: &str) -> Result<&'a str> {
    value
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} must be a non-empty string"))
}

fn optional_string<'a>(value: &'a Value, name: &str) -> Result<Option<&'a str>> {
    value
        .get(name)
        .map(|item| {
            item.as_str()
                .map(str::trim)
                .filter(|item| !item.is_empty())
                .ok_or_else(|| anyhow::anyhow!("{name} must be a non-empty string"))
        })
        .transpose()
}

fn optional_bool(value: &Value, name: &str) -> Result<Option<bool>> {
    value
        .get(name)
        .map(|item| {
            item.as_bool()
                .ok_or_else(|| anyhow::anyhow!("{name} must be a boolean"))
        })
        .transpose()
}

fn optional_string_array(value: &Value, name: &str) -> Result<Vec<String>> {
    let Some(items) = value.get(name) else {
        return Ok(Vec::new());
    };
    let items = items
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("{name} must be an array of non-empty strings"))?;
    if items.len() > MAX_PATTERNS_PER_KIND {
        bail!("{name} may contain at most {MAX_PATTERNS_PER_KIND} strings");
    }
    items
        .iter()
        .map(|item| {
            item.as_str()
                .filter(|item| !item.is_empty())
                .map(str::to_string)
                .ok_or_else(|| anyhow::anyhow!("{name} must contain only non-empty strings"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::contains_bytes;

    #[test]
    fn byte_search_handles_exact_non_utf8_safe_content() {
        assert!(contains_bytes(b"abc\0def", b"c\0d"));
        assert!(!contains_bytes(b"abc", b""));
    }
}
