//! Bounded, local acceptance of the report written by one generation request.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{bail, Context, Result};
use regex::Regex;
use serde_json::{json, Value};
use walkdir::WalkDir;
use xmltree::Element;

use crate::project::SessionConfig;

type Stamp = (u64, SystemTime);
pub struct ReportSnapshot {
    directory: PathBuf,
    before: BTreeMap<PathBuf, Stamp>,
}

impl ReportSnapshot {
    pub fn capture(config: &SessionConfig) -> Result<Self> {
        let dpa = Element::parse(fs::File::open(config.dpa_file()?)?)?;
        let logs = dpa
            .get_child("Folders")
            .and_then(|folders| folders.get_child("Logs"))
            .and_then(Element::get_text)
            .ok_or_else(|| {
                anyhow::anyhow!("DPA Folders/Logs is missing; cannot locate generation evidence")
            })?;
        let logs = logs.trim().replace('\\', "/");
        if logs.is_empty() {
            bail!("DPA Folders/Logs is empty");
        }
        let directory = config.project_path.join(logs);
        let before = report_stamps(&directory)?;
        Ok(Self { directory, before })
    }

    pub fn verify(&self, definition_ref: Option<&str>) -> Result<Value> {
        let after = report_stamps(&self.directory)?;
        let changed: Vec<_> = after
            .iter()
            .filter(|(path, stamp)| self.before.get(*path) != Some(*stamp))
            .collect();
        if changed.len() != 1 {
            bail!("generation report verification failed: expected one fresh report, found {}; logs={}; generation was not retried", changed.len(), self.directory.display());
        }
        let (path, (size, _)) = changed[0];
        if *size > 32 * 1024 * 1024 {
            bail!("generation report exceeds 32 MiB: {}", path.display());
        }
        let html = fs::read_to_string(path)
            .with_context(|| format!("read generation report: {}", path.display()))?;
        let summary = parse_report(&html, definition_ref)
            .with_context(|| format!("generation report verification failed: {}", path.display()));
        // Host renders Display rather than the anyhow chain, so retain the reason.
        let summary = summary.map_err(|error| anyhow::anyhow!("{error:#}"))?;
        Ok(
            json!({"path": path, "validation_errors": 0, "fatal_errors": 0, "generators_verified": summary}),
        )
    }
}

fn report_stamps(directory: &Path) -> Result<BTreeMap<PathBuf, Stamp>> {
    let mut result = BTreeMap::new();
    if !directory.exists() {
        return Ok(result);
    }
    // DaVinci writes Logs/GenerationReport_<time>/GenerationReport.html.
    // Do not walk the SIP or generated source trees, or read historical reports.
    for (count, entry) in WalkDir::new(directory)
        .max_depth(2)
        .follow_links(false)
        .into_iter()
        .enumerate()
    {
        if count >= 8192 {
            bail!(
                "too many log entries for bounded report discovery: {}",
                directory.display()
            );
        }
        let entry = entry?;
        if entry.file_type().is_file()
            && entry
                .file_name()
                .to_string_lossy()
                .eq_ignore_ascii_case("GenerationReport.html")
        {
            let metadata = entry.metadata()?;
            result.insert(entry.into_path(), (metadata.len(), metadata.modified()?));
        }
    }
    Ok(result)
}

fn plain(html: &str) -> Result<String> {
    let text = Regex::new(r"(?s)<[^>]*>")?.replace_all(html, " ");
    Ok(text
        .replace("&nbsp;", " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" "))
}

fn parse_report(html: &str, definition_ref: Option<&str>) -> Result<usize> {
    let body_start = Regex::new(r"(?i)<body\b[^>]*>")?
        .find(html)
        .ok_or_else(|| anyhow::anyhow!("unsupported report: missing body"))?
        .end();
    let body = &html[body_start..];
    let text = plain(body)?;
    let counts = Regex::new(r"\bValidation Results\s+Fatal Errors\s+(\d+)\s+Errors\s+(\d+)\b")?
        .captures(&text)
        .ok_or_else(|| anyhow::anyhow!("unsupported report: missing validation totals"))?;
    if counts[1].parse::<u64>()? != 0 || counts[2].parse::<u64>()? != 0 {
        let details = validation_error_details(body)?;
        bail!(
            "validation has {} fatal error(s) and {} error(s){}",
            &counts[1],
            &counts[2],
            details.map_or_else(String::new, |value| format!("; {value}"))
        );
    }
    if !Regex::new(
        r"\bGeneration Results\s+Execution Result\s*:?\s+(?:SUCCESS|SUCCESSFUL|WARNING)\b",
    )?
    .is_match(&text)
    {
        bail!("generation execution result is missing or unsuccessful");
    }
    let lower = text.to_ascii_lowercase();
    if lower.contains("not started") || lower.contains("configuration contains errors") {
        bail!("report contains a skipped generator or configuration error");
    }
    let marker = Regex::new(
        r#"(?is)<span\b[^>]*class=['"]([^'"]*\bicon-genres-[^'"]*)['"][^>]*>[^<]*</span>"#,
    )?;
    let sections: Vec<_> = marker.captures_iter(body).collect();
    let phase = Regex::new(
        r#"(?is)<span\b[^>]*class=['"][^'"]*\bicon-state-successful\b[^'"]*['"][^>]*>\s*GENERATION\s*</span>"#,
    )?;
    let mut verified = 0;
    for (index, section) in sections.iter().enumerate() {
        let start = section.get(0).unwrap().start();
        let end = sections
            .get(index + 1)
            .map_or(body.len(), |next| next.get(0).unwrap().start());
        let block = &body[start..end];
        if let Some(definition) = definition_ref {
            // Display/package names can differ. Match the exact real definition.
            let block_text = plain(block)?;
            let expected = format!(r"\bDefinition:\s+{}(?:\s|$)", regex::escape(definition));
            if !Regex::new(&expected)?.is_match(&block_text) {
                continue;
            }
        }
        if !section[1].split_whitespace().any(|class| {
            matches!(
                class,
                "icon-genres-internal-success" | "icon-genres-external-success"
            )
        }) || !phase.is_match(block)
        {
            bail!("selected generator lacks successful execution or GENERATION phase");
        }
        verified += 1;
    }
    if verified == 0 {
        bail!("no successful generator matches the requested definition and GENERATION phase");
    }
    Ok(verified)
}

fn validation_error_details(body: &str) -> Result<Option<String>> {
    let end = body.find("Generation Results").unwrap_or(body.len());
    let section = &body[..end];
    let spans = Regex::new(
        r#"(?is)<span\b[^>]*class=['"]([^'"]*)['"][^>]*>([^<]*)</span>"#,
    )?;
    let code = Regex::new(r"(?i)^[A-Z][A-Z0-9_]{1,15}\d{3,6}\b")?;
    let mut issues: Vec<(String, Vec<String>)> = Vec::new();
    let mut fallback = Vec::new();
    for span in spans.captures_iter(section) {
        let classes = span[1].split_whitespace().collect::<Vec<_>>();
        let value = plain(&span[2])?;
        if value.is_empty() {
            continue;
        }
        if classes.contains(&"icon-error") {
            if code.is_match(&value) {
                if issues.len() >= 4 {
                    break;
                }
                issues.push((truncate_chars(&value, 160), Vec::new()));
            } else if let Some((_, details)) = issues.last_mut() {
                if details.len() < 5 {
                    details.push(truncate_chars(&value, 160));
                }
            } else if fallback.len() < 4 {
                fallback.push(truncate_chars(&value, 160));
            }
        } else if (classes.contains(&"icon-parameter") || classes.contains(&"icon-container"))
            && value.starts_with('/')
        {
            if let Some((_, details)) = issues.last_mut() {
                if details.len() < 5 {
                    details.push(truncate_chars(&value, 180));
                }
            }
        }
    }
    let details = if issues.is_empty() {
        fallback.join("; ")
    } else {
        issues
            .into_iter()
            .map(|(label, details)| format!("{label}: {}", details.join("; ")))
            .collect::<Vec<_>>()
            .join(" | ")
    };
    Ok((!details.is_empty()).then_some(details))
}

fn truncate_chars(value: &str, limit: usize) -> String {
    value.chars().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    // Synthetic layout only; no vendor report or customer content is included.
    fn report() -> String {
        "<html><body><div>Validation Results</div><table><td>Fatal Errors</td><td>0</td><td>Errors</td><td>0</td></table><div>Generation Results</div><td>Execution Result</td><td>SUCCESS</td><span class='icon-genres-internal-success elem'>DifferentDisplayName</span><span class='icon-state-successful elem'>GENERATION</span><span>Definition: /OtherVendor/Example</span></body></html>".to_string()
    }

    #[test]
    fn validates_real_definition_instead_of_display_name() {
        assert_eq!(
            parse_report(&report(), Some("/OtherVendor/Example")).unwrap(),
            1
        );
        assert_eq!(parse_report(&report(), None).unwrap(), 1);
        assert!(parse_report(&report(), Some("/OtherVendor/Exam")).is_err());
        assert!(parse_report(&report().replace("SUCCESS", "SUCCESSFUL"), None).is_ok());
        assert!(parse_report(&report().replace("SUCCESS", "WARNING"), None).is_ok());
    }

    #[test]
    fn rejects_errors_missing_phases_and_unknown_formats() {
        for invalid in [
            report().replace("<td>Errors</td><td>0", "<td>Errors</td><td>1"),
            report().replace("<td>Fatal Errors</td><td>0", "<td>Fatal Errors</td><td>1"),
            report().replace("SUCCESS", "ERROR"),
            report().replace("GENERATION", "VALIDATION"),
            report().replace("icon-state-successful", "icon-state-failed"),
            report().replace("internal-success", "internal-error"),
            report().replace("</body>", "generator not started</body>"),
            "unrecognized".to_string(),
        ] {
            assert!(parse_report(&invalid, None).is_err(), "{invalid}");
        }
    }

    #[test]
    fn reports_bounded_validation_codes_and_targets() {
        let invalid = report().replace(
            "<div>Generation Results</div>",
            "<span class='icon-error elem'>CAN03000 (Invalid hardware layout)</span><span class='icon-error elem'>Set to derivative specific value.</span><span class='icon-parameter elem'>/ActiveEcuC/Can/CanConfigSet/CtrlA[0:CanChannelCanObjectStartIndex]</span><div>Generation Results</div>",
        ).replace("<td>Errors</td><td>0", "<td>Errors</td><td>1");
        let error = parse_report(&invalid, None).unwrap_err().to_string();
        assert!(error.contains("CAN03000"));
        assert!(error.contains("CtrlA[0:CanChannelCanObjectStartIndex]"));
        assert!(error.contains("validation has 0 fatal error(s) and 1 error(s)"));
    }

    #[test]
    fn cannot_borrow_success_from_another_generator() {
        let html = report().replace("<span class='icon-state-successful", "<span class='icon-genres-internal-success'>Another</span><span class='icon-state-successful");
        assert!(parse_report(&html, None).is_err());
        let html = report().replace("<span class='icon-state-successful", "<span>Definition: /OtherVendor/Target</span><span class='icon-genres-internal-success'>Another</span><span class='icon-state-successful");
        assert!(parse_report(&html, Some("/OtherVendor/Target")).is_err());
    }

    #[test]
    fn only_one_new_or_rewritten_report_is_accepted() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("GenerationReport.html");
        fs::write(&path, report()).unwrap();
        let snapshot = ReportSnapshot {
            directory: root.path().to_path_buf(),
            before: report_stamps(root.path()).unwrap(),
        };
        assert!(snapshot.verify(None).is_err());
        fs::write(&path, format!("{} ", report())).unwrap();
        assert!(snapshot.verify(None).is_ok());
        fs::create_dir(root.path().join("GenerationReport_other")).unwrap();
        fs::write(
            root.path()
                .join("GenerationReport_other/GenerationReport.html"),
            report(),
        )
        .unwrap();
        assert!(snapshot.verify(None).is_err());
    }

    #[test]
    #[ignore = "optional local read-only report check; no vendor fixtures are shipped"]
    fn private_report_check() {
        let path = std::env::var("LGK_AUTOSAR_TEST_REPORT").expect("report path");
        let definition = std::env::var("LGK_AUTOSAR_TEST_DEFINITION").ok();
        let html = fs::read_to_string(path).unwrap();
        let start = std::time::Instant::now();
        let result = parse_report(&html, definition.as_deref());
        eprintln!("report parse: {:?}; elapsed={:?}", result, start.elapsed());
        let expect_pass = std::env::var("LGK_AUTOSAR_TEST_EXPECT_PASS").unwrap() == "true";
        assert_eq!(result.is_ok(), expect_pass);
    }
}
