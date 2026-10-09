//! `cargo xtask prose [--base REF]`: the prose lint, on the Markdown lines a branch adds.
//!
//! Text written quickly, by people or by language models, drifts toward the same habits: em dashes,
//! "not X, but Y", stacked hedges, a mind given to software. They make docs longer without making them
//! clearer, and an agent reading the docs as context pays for every word. Vale runs two styles over the
//! docs (`.vale.ini`): `ai-tells`, vendored from vale-ai-tells (MIT), and the house rules in
//! `.vale/styles/StitchCraft` (`docs/src/contributing/writing-style.md`).
//!
//! Only lines added since the merge base with `REF` (default `origin/main`) are held to the rules,
//! uncommitted and untracked files included, so the gate checks what a branch writes while older pages
//! are rewritten in their own time. Vale is an optional tool: skipped locally without it, required in
//! CI (`cargo xtask ci`).

use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

use serde::Deserialize;

use crate::util::{self, Findings};

/// How to install the Vale version CI uses: built from its tagged source, checked by Go's checksum
/// database. `.github/workflows/ci.yml` runs this command, as a test checks.
pub const INSTALL: &str = "go install github.com/vale-cli/vale/v3/cmd/vale@v3.23.0";

/// The base a branch is compared with when no `--base` is given.
const DEFAULT_BASE: &str = "origin/main";

/// Directories whose Markdown is not prose to lint: the vendored rules.
const SKIP: &[&str] = &[".vale/"];

/// Whether Vale is installed.
pub fn available() -> bool {
    util::tool_available("vale", &["--version"])
}

/// One alert, as `vale --output=JSON` reports it.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "PascalCase")]
struct Alert {
    line: usize,
    check: String,
    message: String,
    severity: String,
}

/// `cargo xtask prose`.
pub fn run(args: &[String]) -> Result<(), String> {
    let base = match args {
        [] => DEFAULT_BASE.to_string(),
        [flag, base] if flag == "--base" => base.clone(),
        _ => return Err("usage: cargo xtask prose [--base REF]".to_string()),
    };
    let root = util::root();
    let since = util::git(&root, &["merge-base", &base, "HEAD"])?.trim().to_string();
    let mut added = added_lines(&util::git(&root, &["diff", "-U0", "--no-color", "--no-ext-diff", &since, "--", "*.md"])?);
    for path in util::git(&root, &["ls-files", "--others", "--exclude-standard", "--", "*.md"])?.lines() {
        let lines = util::read(&util::root().join(path))?.lines().count();
        added.insert(path.to_string(), (1..=lines).collect());
    }
    added.retain(|path, lines| !lines.is_empty() && !SKIP.iter().any(|skip| path.starts_with(skip)));
    let mut findings = Findings::default();
    if added.is_empty() {
        return findings.finish("prose", &format!("no Markdown lines added since {base}"));
    }
    let files: Vec<&str> = added.keys().map(String::as_str).collect();
    let output = Command::new("vale")
        .args(["--output=JSON", "--no-exit"])
        .args(&files)
        .current_dir(util::root())
        .output()
        .map_err(|e| format!("vale: {e}"))?;
    if !output.status.success() {
        return Err(format!("vale failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    let alerts = parse(&String::from_utf8_lossy(&output.stdout))?;
    for (path, line, alert) in on_added_lines(&alerts, &added) {
        findings.error(format!("{path}:{line}: {} ({}): {}", alert.check, alert.severity, alert.message));
    }
    let count: usize = added.values().map(BTreeSet::len).sum();
    findings.finish("prose", &format!("{count} added lines in {} Markdown files read clean", added.len()))
}

/// The lines each file gains in a `git diff -U0`, by path.
fn added_lines(diff: &str) -> BTreeMap<String, BTreeSet<usize>> {
    let mut added: BTreeMap<String, BTreeSet<usize>> = BTreeMap::new();
    let mut file = None;
    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ ") {
            // `/dev/null` for a deleted file; `b/path` otherwise.
            file = path.strip_prefix("b/").map(str::to_string);
        } else if let (Some(file), Some(hunk)) = (&file, line.strip_prefix("@@ ")) {
            // `@@ -a,b +c,d @@`: d lines from c, one when d is left out.
            let new = hunk.split_whitespace().find_map(|part| part.strip_prefix('+')).unwrap_or_default();
            let (start, count) = new.split_once(',').unwrap_or((new, "1"));
            if let (Ok(start), Ok(count)) = (start.parse::<usize>(), count.parse::<usize>()) {
                added.entry(file.clone()).or_default().extend(start..start + count);
            }
        }
    }
    added
}

/// Vale's JSON report: alerts by file.
fn parse(json: &str) -> Result<BTreeMap<String, Vec<Alert>>, String> {
    if json.trim().is_empty() {
        return Ok(BTreeMap::new());
    }
    serde_json::from_str(json).map_err(|e| format!("vale's report: {e}"))
}

/// The alerts on added lines, in file and line order.
fn on_added_lines<'a>(alerts: &'a BTreeMap<String, Vec<Alert>>, added: &BTreeMap<String, BTreeSet<usize>>) -> Vec<(&'a str, usize, &'a Alert)> {
    let mut kept = Vec::new();
    for (path, list) in alerts {
        let Some(lines) = added.get(path.trim_start_matches("./")) else { continue };
        for alert in list.iter().filter(|a| lines.contains(&a.line)) {
            kept.push((path.as_str(), alert.line, alert));
        }
    }
    kept.sort_by_key(|(path, line, _)| (*path, *line));
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ci_installs_the_vale_this_module_names() {
        let workflow = util::read(&util::root().join(".github/workflows/ci.yml")).unwrap();
        assert!(workflow.contains(INSTALL), "ci.yml must run `{INSTALL}`");
    }

    #[test]
    fn added_lines_come_from_the_new_side_of_each_hunk() {
        let diff = "\
diff --git a/README.md b/README.md
--- a/README.md
+++ b/README.md
@@ -3 +3 @@ intro
-old
+new
@@ -10,0 +11,2 @@
+one
+two
@@ -20,3 +22,0 @@
diff --git a/gone.md b/gone.md
--- a/gone.md
+++ /dev/null
@@ -1,2 +0,0 @@
diff --git a/docs/new.md b/docs/new.md
--- /dev/null
+++ b/docs/new.md
@@ -0,0 +1,3 @@
";
        let added = added_lines(diff);
        assert_eq!(added.get("README.md"), Some(&BTreeSet::from([3, 11, 12])));
        assert_eq!(added.get("docs/new.md"), Some(&BTreeSet::from([1, 2, 3])));
        assert!(!added.contains_key("gone.md"));
    }

    #[test]
    fn only_alerts_on_added_lines_count() {
        let json = r#"{"README.md": [
            {"Line": 3, "Check": "ai-tells.EmDashUsage", "Message": "em dash", "Severity": "error", "Span": [1, 2], "Match": "—"},
            {"Line": 4, "Check": "ai-tells.EmDashUsage", "Message": "em dash", "Severity": "error", "Span": [1, 2], "Match": "—"}
        ], "docs/old.md": [
            {"Line": 1, "Check": "StitchCraft.Names", "Message": "name", "Severity": "error", "Span": [1, 2], "Match": "x"}
        ]}"#;
        let alerts = parse(json).unwrap();
        let added = BTreeMap::from([("README.md".to_string(), BTreeSet::from([3, 11]))]);
        let kept = on_added_lines(&alerts, &added);
        assert_eq!(kept.len(), 1);
        assert_eq!((kept[0].0, kept[0].1, kept[0].2.check.as_str()), ("README.md", 3, "ai-tells.EmDashUsage"));
        assert!(parse("").unwrap().is_empty());
        assert!(parse("not json").is_err());
    }
}
