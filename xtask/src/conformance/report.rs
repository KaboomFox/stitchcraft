//! The conformance report: the requirement × case matrix as Markdown (the pull request's job summary),
//! the same as JSON (for the docs), and output hashes (for the cross-platform determinism job).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::Path;

use serde_json::json;

use super::cases::Requirement;
use super::runner::Outcome;

/// How a requirement stands after a run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Every case covering it passed.
    Pass,
    /// A case covering it failed.
    Fail,
    /// No case covers it.
    Untested,
}

impl Verdict {
    fn mark(self) -> &'static str {
        match self {
            Verdict::Pass => "✅",
            Verdict::Fail => "❌",
            Verdict::Untested => "⚪",
        }
    }
}

/// The verdict for each requirement, in file order.
pub fn verdicts<'a>(requirements: &'a [Requirement], outcomes: &[Outcome]) -> Vec<(&'a Requirement, Verdict)> {
    requirements
        .iter()
        .map(|r| {
            let ran: Vec<&Outcome> = outcomes.iter().filter(|o| o.requirements.contains(&r.id) && o.skipped.is_none()).collect();
            let verdict = if ran.is_empty() {
                Verdict::Untested
            } else if ran.iter().all(|o| o.passed()) {
                Verdict::Pass
            } else {
                Verdict::Fail
            };
            (r, verdict)
        })
        .collect()
}

/// Writes `report.md`, `report.json` and `hashes.json` into `dir` and returns the Markdown.
pub fn write(dir: &Path, requirements: &[Requirement], outcomes: &[Outcome], commit: &str) -> Result<String, String> {
    let verdicts = verdicts(requirements, outcomes);
    let markdown = markdown(&verdicts, outcomes, commit);
    let json = json!({
        "commit": commit,
        "requirements": verdicts.iter().map(|(r, v)| json!({
            "id": r.id, "level": r.level, "status": r.status, "milestone": r.milestone,
            "result": match v { Verdict::Pass => "pass", Verdict::Fail => "fail", Verdict::Untested => "untested" },
            "cases": outcomes.iter().filter(|o| o.requirements.contains(&r.id)).map(|o| o.case.clone()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "cases": outcomes.iter().map(|o| json!({
            "case": o.case, "kind": o.kind, "file": o.file, "requirements": o.requirements,
            "passed": o.passed(), "failures": o.failures, "skipped": o.skipped,
        })).collect::<Vec<_>>(),
    });
    let hashes: BTreeMap<String, String> =
        outcomes.iter().flat_map(|o| o.outputs.iter().map(move |(name, hash)| (format!("{}/{name}", o.case), hash.clone()))).collect();
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let write = |name: &str, text: String| std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"));
    write("report.md", markdown.clone())?;
    write("report.json", serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n")?;
    write("hashes.json", serde_json::to_string_pretty(&hashes).map_err(|e| e.to_string())? + "\n")?;
    Ok(markdown)
}

fn markdown(verdicts: &[(&Requirement, Verdict)], outcomes: &[Outcome], commit: &str) -> String {
    let count = |want: Verdict, active_only: bool| verdicts.iter().filter(|(r, v)| *v == want && (!active_only || r.status == "active")).count();
    let active = verdicts.iter().filter(|(r, _)| r.status == "active").count();
    let failed: Vec<&Outcome> = outcomes.iter().filter(|o| o.failed()).collect();
    let mut out = String::new();
    let _ = writeln!(out, "# Conformance report\n");
    let _ = writeln!(
        out,
        "**{} of {active} active requirements green** · {} cases, {} failed · {} requirements planned · commit `{commit}`\n",
        count(Verdict::Pass, true),
        outcomes.len(),
        failed.len(),
        verdicts.iter().filter(|(r, _)| r.status == "planned").count(),
    );
    if !failed.is_empty() {
        let _ = writeln!(out, "## Failures\n");
        for o in &failed {
            let _ = writeln!(out, "- **{}** ({}, {}) — {}", o.case, o.kind, o.requirements.join(", "), o.file);
            for f in &o.failures {
                let _ = writeln!(out, "  - {f}");
            }
        }
        out.push('\n');
    }
    let _ = writeln!(out, "## Requirements\n");
    let _ = writeln!(out, "| | Requirement | Level | Status | Cases |");
    let _ = writeln!(out, "|---|---|---|---|---|");
    // Failures first, then the rest in file order; requirements nobody tests yet are folded away.
    let mut rows: Vec<&(&Requirement, Verdict)> = verdicts.iter().collect();
    rows.sort_by_key(|(_, v)| *v != Verdict::Fail);
    let (tested, untested): (Vec<_>, Vec<_>) = rows.into_iter().partition(|(r, v)| *v != Verdict::Untested || r.status == "active");
    for (r, v) in tested {
        let cases: Vec<String> = outcomes
            .iter()
            .filter(|o| o.requirements.contains(&r.id))
            .map(|o| {
                format!(
                    "{} `{}`",
                    if o.skipped.is_some() {
                        "⏭"
                    } else if o.passed() {
                        "✅"
                    } else {
                        "❌"
                    },
                    o.case
                )
            })
            .collect();
        let status = if r.status == "planned" { format!("planned ({})", r.milestone) } else { r.status.clone() };
        let _ = writeln!(
            out,
            "| {} | `{}` | {} | {status} | {} |",
            v.mark(),
            r.id,
            r.level,
            if cases.is_empty() { "—".to_string() } else { cases.join("<br>") }
        );
    }
    if !untested.is_empty() {
        let _ = writeln!(out, "\n<details><summary>{} requirements planned for later milestones, not tested yet</summary>\n", untested.len());
        for (r, _) in untested {
            let _ = writeln!(out, "- `{}` ({}, {}): {}", r.id, r.level, r.milestone, r.statement);
        }
        let _ = writeln!(out, "\n</details>");
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req(id: &str, status: &str) -> Requirement {
        Requirement {
            id: id.into(),
            area: "a".into(),
            level: "L0".into(),
            statement: "s".into(),
            milestone: "M1".into(),
            status: status.into(),
            rationale: None,
        }
    }

    fn outcome(case: &str, requirements: &[&str], failures: &[&str]) -> Outcome {
        Outcome {
            case: case.into(),
            kind: "rust",
            file: "f.rs".into(),
            requirements: requirements.iter().map(|s| (*s).to_string()).collect(),
            failures: failures.iter().map(|s| (*s).to_string()).collect(),
            outputs: Vec::new(),
            skipped: None,
        }
    }

    #[test]
    fn verdicts_and_failures_first() {
        let requirements = vec![req("REQ-A-001", "active"), req("REQ-A-002", "active"), req("REQ-A-003", "planned")];
        let outcomes = vec![outcome("ok_case", &["REQ-A-001"], &[]), outcome("bad_case", &["REQ-A-002"], &["boom"])];
        let v = verdicts(&requirements, &outcomes);
        assert_eq!(v.iter().map(|(_, v)| *v).collect::<Vec<_>>(), vec![Verdict::Pass, Verdict::Fail, Verdict::Untested]);
        let md = markdown(&v, &outcomes, "abc");
        assert!(md.contains("**1 of 2 active requirements green** · 2 cases, 1 failed"));
        assert!(md.contains("## Failures\n\n- **bad_case** (rust, REQ-A-002) — f.rs\n  - boom"));
        let table = md.split("## Requirements").nth(1).unwrap();
        assert!(table.find("REQ-A-002").unwrap() < table.find("REQ-A-001").unwrap(), "failures come first");
        assert!(md.contains("<details><summary>1 requirements planned for later milestones, not tested yet</summary>\n\n- `REQ-A-003` (L0, M1): s"));
    }
}
