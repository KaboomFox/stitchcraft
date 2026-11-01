//! The conformance report: the requirement × case matrix and the diagnostic codes with their `diag_`
//! cases, as Markdown (the pull request's job summary) and as JSON (for the docs).

use std::fmt::Write as _;
use std::path::Path;

use serde_json::json;
use stitchcraft_core::Code;

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
    /// Its cases did not run here, for want of a tool, and none ran and failed.
    Skipped,
}

impl Verdict {
    fn mark(self) -> &'static str {
        match self {
            Verdict::Pass => "✅",
            Verdict::Fail => "❌",
            Verdict::Untested => "⚪",
            Verdict::Skipped => "⏭",
        }
    }

    /// Its name in `report.json`.
    fn name(self) -> &'static str {
        match self {
            Verdict::Pass => "pass",
            Verdict::Fail => "fail",
            Verdict::Untested => "untested",
            Verdict::Skipped => "skipped",
        }
    }

    /// Whether an active requirement or a registered code may stand so: it passed, or, outside CI, its cases
    /// were skipped as optional tools are (`docs/src/design/conformance.md`).
    pub fn acceptable(self, in_ci: bool) -> bool {
        self == Verdict::Pass || (self == Verdict::Skipped && !in_ci)
    }
}

/// The verdict for `id` (a requirement or a diagnostic code) from the cases that cover it.
fn verdict(id: &str, outcomes: &[Outcome]) -> Verdict {
    let covering: Vec<&Outcome> = outcomes.iter().filter(|o| o.requirements.iter().any(|r| r == id)).collect();
    let ran: Vec<&&Outcome> = covering.iter().filter(|o| o.skipped.is_none()).collect();
    if ran.is_empty() {
        if covering.is_empty() { Verdict::Untested } else { Verdict::Skipped }
    } else if ran.iter().all(|o| o.passed()) {
        Verdict::Pass
    } else {
        Verdict::Fail
    }
}

/// The verdict for each requirement, in file order.
pub fn verdicts<'a>(requirements: &'a [Requirement], outcomes: &[Outcome]) -> Vec<(&'a Requirement, Verdict)> {
    requirements.iter().map(|r| (r, verdict(&r.id, outcomes))).collect()
}

/// The verdict for each registered diagnostic code, in registry order.
pub fn code_verdicts(outcomes: &[Outcome]) -> Vec<(Code, Verdict)> {
    Code::ALL.iter().map(|code| (*code, verdict(code.id(), outcomes))).collect()
}

/// `✅ case` for each case covering `id`.
fn case_list(id: &str, outcomes: &[Outcome]) -> String {
    let cases: Vec<String> = outcomes
        .iter()
        .filter(|o| o.requirements.iter().any(|r| r == id))
        .map(|o| {
            let mark = if o.skipped.is_some() {
                "⏭"
            } else if o.passed() {
                "✅"
            } else {
                "❌"
            };
            format!("{mark} `{}`", o.case)
        })
        .collect();
    if cases.is_empty() { "—".to_string() } else { cases.join("<br>") }
}

/// Writes `report.md` and `report.json` into `dir` and returns the Markdown.
pub fn write(dir: &Path, requirements: &[Requirement], outcomes: &[Outcome], commit: &str) -> Result<String, String> {
    let verdicts = verdicts(requirements, outcomes);
    let codes = code_verdicts(outcomes);
    let markdown = markdown(&verdicts, &codes, outcomes, commit);
    let json = json!({
        "commit": commit,
        "requirements": verdicts.iter().map(|(r, v)| json!({
            "id": r.id, "level": r.level, "status": r.status, "milestone": r.milestone,
            "result": v.name(),
            "cases": outcomes.iter().filter(|o| o.requirements.contains(&r.id)).map(|o| o.case.clone()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "diagnostics": codes.iter().map(|(code, v)| json!({
            "code": code.id(),
            "result": v.name(),
            "cases": outcomes.iter().filter(|o| o.requirements.iter().any(|r| r == code.id())).map(|o| o.case.clone()).collect::<Vec<_>>(),
        })).collect::<Vec<_>>(),
        "cases": outcomes.iter().map(|o| json!({
            "case": o.case, "kind": o.kind, "file": o.file, "requirements": o.requirements,
            "passed": o.passed(), "failures": o.failures, "skipped": o.skipped,
        })).collect::<Vec<_>>(),
    });
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let write = |name: &str, text: String| std::fs::write(dir.join(name), text).map_err(|e| format!("{name}: {e}"));
    write("report.md", markdown.clone())?;
    write("report.json", serde_json::to_string_pretty(&json).map_err(|e| e.to_string())? + "\n")?;
    Ok(markdown)
}

fn markdown(verdicts: &[(&Requirement, Verdict)], codes: &[(Code, Verdict)], outcomes: &[Outcome], commit: &str) -> String {
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
        let status = if r.status == "planned" { format!("planned ({})", r.milestone) } else { r.status.clone() };
        let _ = writeln!(out, "| {} | `{}` | {} | {status} | {} |", v.mark(), r.id, r.level, case_list(&r.id, outcomes));
    }
    if !untested.is_empty() {
        let _ = writeln!(out, "\n<details><summary>{} requirements planned for later milestones, not tested yet</summary>\n", untested.len());
        for (r, _) in untested {
            let _ = writeln!(out, "- `{}` ({}, {}): {}", r.id, r.level, r.milestone, r.statement);
        }
        let _ = writeln!(out, "\n</details>");
    }
    let green = codes.iter().filter(|(_, v)| *v == Verdict::Pass).count();
    let _ = writeln!(out, "\n## Diagnostic codes\n\n**{green} of {} codes produced and rendered by a `diag_` case**\n", codes.len());
    let _ = writeln!(out, "| | Code | Title | Cases |\n|---|---|---|---|");
    for (code, v) in codes {
        let _ = writeln!(out, "| {} | `{}` | {} | {} |", v.mark(), code.id(), code.title(), case_list(code.id(), outcomes));
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
            notes: Vec::new(),
            details: Vec::new(),
        }
    }

    #[test]
    fn a_requirement_whose_cases_were_all_skipped_is_skipped_and_fails_only_in_ci() {
        let skipped = Outcome { skipped: Some("pyembroidery is not installed".into()), ..outcome("oracle", &["REQ-A-001", "REQ-A-002"], &[]) };
        let outcomes = [skipped, outcome("ok_case", &["REQ-A-002"], &[])];
        let requirements = [req("REQ-A-001", "active"), req("REQ-A-002", "active"), req("REQ-A-003", "active")];
        let v = verdicts(&requirements, &outcomes);
        assert_eq!(v.iter().map(|(_, v)| *v).collect::<Vec<_>>(), [Verdict::Skipped, Verdict::Pass, Verdict::Untested]);
        assert!(Verdict::Skipped.acceptable(false) && !Verdict::Skipped.acceptable(true));
        assert!(Verdict::Pass.acceptable(true) && !Verdict::Untested.acceptable(false) && !Verdict::Fail.acceptable(false));
    }

    #[test]
    fn verdicts_and_failures_first() {
        let requirements = vec![req("REQ-A-001", "active"), req("REQ-A-002", "active"), req("REQ-A-003", "planned")];
        let outcomes = vec![outcome("ok_case", &["REQ-A-001"], &[]), outcome("bad_case", &["REQ-A-002"], &["boom"])];
        let v = verdicts(&requirements, &outcomes);
        assert_eq!(v.iter().map(|(_, v)| *v).collect::<Vec<_>>(), vec![Verdict::Pass, Verdict::Fail, Verdict::Untested]);
        let md = markdown(&v, &[], &outcomes, "abc");
        assert!(md.contains("**1 of 2 active requirements green** · 2 cases, 1 failed"));
        assert!(md.contains("## Failures\n\n- **bad_case** (rust, REQ-A-002) — f.rs\n  - boom"));
        let table = md.split("## Requirements").nth(1).unwrap();
        assert!(table.find("REQ-A-002").unwrap() < table.find("REQ-A-001").unwrap(), "failures come first");
        assert!(md.contains("<details><summary>1 requirements planned for later milestones, not tested yet</summary>\n\n- `REQ-A-003` (L0, M1): s"));
    }

    #[test]
    fn every_code_gets_a_row_with_its_cases() {
        let outcomes = vec![outcome("diag_sc_e0701_wide", &["SC-E0701"], &[]), outcome("diag_sc_w0702_big", &["SC-W0702"], &["boom"])];
        let codes = code_verdicts(&outcomes);
        assert_eq!(codes.len(), Code::ALL.len());
        let of = |id: &str| codes.iter().find(|(c, _)| c.id() == id).map(|(_, v)| *v);
        assert_eq!((of("SC-E0701"), of("SC-W0702"), of("SC-E0010")), (Some(Verdict::Pass), Some(Verdict::Fail), Some(Verdict::Untested)));
        let md = markdown(&[], &codes, &outcomes, "abc");
        assert!(md.contains(&format!("**1 of {} codes produced and rendered by a `diag_` case**", Code::ALL.len())));
        assert!(md.contains("| ✅ | `SC-E0701` | Design does not fit the hoop | ✅ `diag_sc_e0701_wide` |"));
        assert!(md.contains("| ⚪ | `SC-E0010` | Nothing to stitch | — |"));
    }
}
