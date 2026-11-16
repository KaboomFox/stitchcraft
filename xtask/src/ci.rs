//! `cargo xtask ci`: every gate, in one command, with a summary.
//!
//! Runs all steps even when one fails, so a contributor sees every problem at once. Steps that need an
//! optional tool (the wasm target, cargo-public-api, `cargo-deny`, `typos`, Vale, `mdbook`) are skipped locally
//! when the tool is missing and required in CI (`CI` set), where the workflow installs them. Coverage is
//! not a step here: it runs the whole suite again, instrumented, so CI gives it a job of its own.
//!
//! `--skip` leaves out gates by name, for a job that runs the gates on another compiler than the pinned
//! one: the API snapshots are the pinned toolchain's rustdoc output, and the prose lint has nothing to do
//! with the compiler. A name that is no gate is an error.
//!
//! Every gate belongs to one group, and `--group` runs the gates of one group alone. CI runs each group as a
//! job of its own, side by side (`.github/workflows/ci.yml`), so a round waits for the slowest group and not
//! for the sum of all gates. The list of gates and their groups is here and nowhere else: a test fails when
//! the workflow's jobs and the groups differ, so no gate can be left without a job that runs it. The summary
//! gives each gate's time, as a notice too in GitHub Actions, where a log is harder to reach.
//!
//! Cargo steps name this repository's packages, so inside VectorCraft's workspace (ADR-0011) they build,
//! lint and test StitchCraft only. There, `cargo-deny` is skipped, because it judges a whole workspace
//! against one policy and VectorCraft's workspace answers to VectorCraft's, and optional tools stay
//! optional: the move rehearsal checks that the code moves, while StitchCraft's own CI runs every tool.

use crate::util;
use crate::{api, book, cleanroom, conformance, docs, filesize, layers, prose, shots, unsafe_audit, wasm};

/// How one step ended.
pub(crate) enum Outcome {
    Passed,
    Failed(String),
    Skipped(&'static str),
}

/// A gate: its name, the group whose job runs it in CI, and what it runs.
type Step = (&'static str, &'static str, Box<dyn Fn() -> Outcome>);

/// The groups of gates. Each is a job of `.github/workflows/ci.yml`, which names them in its matrix.
/// - `lint` reads the code without running it: the compiler's and Clippy's verdicts, rustdoc, the API
///   snapshots, and the scans of the sources and the dependencies.
/// - `test` runs the tests.
/// - `conformance` runs the conformance suite and builds for WebAssembly.
/// - `docs` checks the documents, their pictures, their prose and the book.
const LINT: &str = "lint";
const TEST: &str = "test";
const CONFORMANCE: &str = "conformance";
const DOCS: &str = "docs";

/// `cargo <command> -p … <rest>`: a Cargo step over this repository's packages.
fn cargo_step(command: &'static str, packages: &util::Packages, rest: &'static [&'static str]) -> Box<dyn Fn() -> Outcome> {
    let package_args = packages.args();
    Box::new(move || {
        let mut cmd = util::cargo();
        cmd.arg(command).args(&package_args).args(rest);
        outcome(util::run(cmd, &format!("cargo {command} {}", rest.join(" "))))
    })
}

pub(crate) fn outcome(result: Result<(), String>) -> Outcome {
    match result {
        Ok(()) => Outcome::Passed,
        Err(e) => Outcome::Failed(e),
    }
}

/// A step that needs an optional tool: skipped without it, except in StitchCraft's own CI, which fails.
pub(crate) fn optional(available: fn() -> bool, missing: &'static str, guest: bool, step: Box<dyn Fn() -> Outcome>) -> Box<dyn Fn() -> Outcome> {
    Box::new(move || {
        if available() {
            step()
        } else if util::in_ci() && !guest {
            Outcome::Failed(format!("required in CI: {missing}"))
        } else {
            Outcome::Skipped(missing)
        }
    })
}

/// `program` run from the repository's root once with each of `runs`' arguments; the first run that fails
/// fails the step.
fn tool_step(program: &'static str, runs: &'static [&'static [&'static str]]) -> Box<dyn Fn() -> Outcome> {
    Box::new(move || {
        for args in runs {
            let mut cmd = std::process::Command::new(program);
            cmd.args(*args).current_dir(util::root());
            if let Err(e) = util::run(cmd, &format!("{program} {}", args.join(" "))) {
                return Outcome::Failed(e);
            }
        }
        Outcome::Passed
    })
}

/// cargo-deny's runs: the workspace, and the workspaces of their own with dependencies of their own: the
/// fuzz targets (`fuzz/`) and the window probe (`compat/vc-window`), whose xa11y we chose. The contract
/// tests (`compat/vc-contract`) are not judged here. Their dependencies are VectorCraft's, at whichever
/// version a track tests, and VectorCraft's policy applies to them, as it does inside VectorCraft's
/// workspace.
const DENY: &[&[&str]] =
    &[&["check"], &["--manifest-path", "fuzz/Cargo.toml", "check"], &["--manifest-path", "compat/vc-window/Cargo.toml", "check"]];

/// An error naming the first of `skip` that is none of `gates`.
fn unknown_gate(skip: &[String], gates: &[&str]) -> Result<(), String> {
    match skip.iter().find(|gate| !gates.contains(&gate.as_str())) {
        Some(unknown) => Err(format!("there is no gate `{unknown}` to skip (the gates: {})", gates.join(", "))),
        None => Ok(()),
    }
}

/// An error when `group` is the group of none of `steps`.
fn unknown_group(group: Option<&str>, steps: &[Step]) -> Result<(), String> {
    let groups: std::collections::BTreeSet<&str> = steps.iter().map(|(_, group, _)| *group).collect();
    match group {
        Some(unknown) if !groups.contains(unknown) => {
            Err(format!("there is no group `{unknown}` of gates (the groups: {})", groups.into_iter().collect::<Vec<_>>().join(", ")))
        }
        _ => Ok(()),
    }
}

/// Every gate, in the order `cargo xtask ci` runs them, with its group.
fn steps(packages: &util::Packages) -> Vec<Step> {
    let guest = packages.guest;
    let doc_args = packages.args();
    let deny: Box<dyn Fn() -> Outcome> = if guest {
        Box::new(|| Outcome::Skipped("inside another workspace, whose own dependency policy applies"))
    } else {
        optional(|| util::tool_available("cargo-deny", &["--version"]), "cargo install cargo-deny", guest, tool_step("cargo-deny", DENY))
    };
    vec![
        ("fmt", LINT, cargo_step("fmt", packages, &["--", "--check"])),
        ("clippy", LINT, cargo_step("clippy", packages, &["--all-targets", "--locked", "--", "-D", "warnings"])),
        ("test", TEST, cargo_step("test", packages, &["--locked"])),
        (
            "rustdoc",
            LINT,
            Box::new(move || {
                // Broken, ambiguous or private intra-doc links are documentation bugs.
                let mut cmd = util::cargo();
                cmd.arg("doc").args(&doc_args).args(["--no-deps", "--locked"]).env("RUSTDOCFLAGS", "-D warnings");
                outcome(util::run(cmd, "cargo doc (warnings denied)"))
            }),
        ),
        ("layers", LINT, Box::new(|| outcome(layers::run()))),
        ("filesize", LINT, Box::new(|| outcome(filesize::run()))),
        ("cleanroom", LINT, Box::new(|| outcome(cleanroom::run()))),
        ("unsafe-audit", LINT, Box::new(|| outcome(unsafe_audit::run()))),
        ("docs", DOCS, Box::new(|| outcome(docs::run(true)))),
        ("shots", DOCS, Box::new(|| outcome(shots::run(true, false)))),
        ("conformance", CONFORMANCE, Box::new(|| outcome(conformance::run(false, None, None, false, &conformance::Glyphs::default())))),
        ("wasm", CONFORMANCE, optional(wasm::target_installed, "rustup target add wasm32-unknown-unknown", guest, Box::new(|| outcome(wasm::run())))),
        ("wasm-goldens", CONFORMANCE, optional(wasm::wasi_ready, wasm::WASI_INSTALL, guest, Box::new(|| outcome(wasm::goldens())))),
        ("api", LINT, optional(api::available, api::INSTALL, guest, Box::new(|| outcome(api::run(true))))),
        ("deny", LINT, deny),
        ("typos", LINT, optional(|| util::tool_available("typos", &["--version"]), "cargo install typos-cli", guest, tool_step("typos", &[&[]]))),
        ("prose", DOCS, optional(prose::available, prose::INSTALL, guest, Box::new(|| outcome(prose::run(prose::DEFAULT_BASE))))),
        (
            "book",
            DOCS,
            optional(|| util::tool_available("mdbook", &["--version"]), "cargo install mdbook", guest, Box::new(|| outcome(book::run(false)))),
        ),
    ]
}

/// `cargo xtask ci`, without the gates named in `skip`, and with the gates of `group` alone when one is given.
pub fn run(skip: &[String], group: Option<&str>) -> Result<(), String> {
    let packages = util::packages()?;
    let steps = steps(&packages);
    unknown_gate(skip, &steps.iter().map(|(name, _, _)| *name).collect::<Vec<_>>())?;
    unknown_group(group, &steps)?;
    let title = group.map_or_else(|| "cargo xtask ci".to_string(), |group| format!("cargo xtask ci --group {group}"));
    let steps: Vec<Check> =
        steps.into_iter().filter(|(_, of, _)| group.is_none_or(|group| group == *of)).map(|(name, _, step)| (name, step)).collect();
    run_checks(&title, &steps, skip)
}

/// A named check and what it runs.
pub(crate) type Check = (&'static str, Box<dyn Fn() -> Outcome>);

/// Runs `checks` one after another, all of them even when one fails, without those named in `skip`, and
/// prints a summary with each one's time. `title` names the command in GitHub's annotations.
pub(crate) fn run_checks(title: &str, checks: &[Check], skip: &[String]) -> Result<(), String> {
    let mut summary = Vec::new();
    for (name, step) in checks {
        if skip.iter().any(|gate| gate.as_str() == *name) {
            summary.push((*name, Outcome::Skipped("--skip"), 0));
            continue;
        }
        eprintln!("\n=== {name} ===");
        // A gate's time is for the summary alone, and no result depends on it.
        #[allow(clippy::disallowed_types)]
        let started = std::time::Instant::now();
        let result = step();
        summary.push((*name, result, started.elapsed().as_secs()));
    }
    println!("\nSummary of `{title}`:");
    let mut failed = 0;
    for (name, result, seconds) in &summary {
        match result {
            Outcome::Passed => println!("  ok    {name} ({seconds} s)"),
            Outcome::Skipped(why) => println!("  skip  {name} ({why})"),
            Outcome::Failed(e) => {
                failed += 1;
                println!("  FAIL  {name} ({seconds} s): {e}");
                if std::env::var_os("GITHUB_ACTIONS").is_some() {
                    println!("{}", util::annotation("error", title, &format!("{name} failed: {e}")));
                }
            }
        }
    }
    if std::env::var_os("GITHUB_ACTIONS").is_some() {
        println!("{}", util::annotation("notice", title, &times(&summary)));
    }
    if failed == 0 { Ok(()) } else { Err(format!("{failed} of {} gates failed", summary.len())) }
}

/// Each gate that ran with the seconds it took, in one line.
fn times(summary: &[(&str, Outcome, u64)]) -> String {
    let ran: Vec<String> = summary
        .iter()
        .filter(|(_, result, _)| !matches!(result, Outcome::Skipped(_)))
        .map(|(name, _, seconds)| format!("{name} {seconds} s"))
        .collect();
    format!("the gates' times: {}", ran.join(", "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_gate_to_skip_must_be_a_gate() {
        let skip = |names: &[&str]| unknown_gate(&names.iter().map(|n| (*n).to_string()).collect::<Vec<_>>(), &["api", "prose"]);
        assert_eq!(skip(&[]), Ok(()));
        assert_eq!(skip(&["prose", "api"]), Ok(()));
        assert_eq!(skip(&["api", "apo"]), Err("there is no gate `apo` to skip (the gates: api, prose)".to_string()));
    }

    /// The groups the matrix of `ci.yml`'s `groups` job names, in its `group: [a, b]` line.
    fn workflow_groups(workflow: &str) -> Vec<String> {
        let list = workflow.lines().filter_map(|line| line.trim().strip_prefix("group: [")).filter_map(|rest| rest.strip_suffix(']'));
        list.flat_map(|names| names.split(',')).map(|name| name.trim().to_string()).collect()
    }

    #[test]
    fn every_gate_has_a_group_and_every_group_a_job_in_ci() {
        let steps = steps(&util::packages().unwrap());
        let names: Vec<&str> = steps.iter().map(|(name, _, _)| *name).collect();
        assert_eq!(names.iter().collect::<std::collections::BTreeSet<_>>().len(), names.len(), "a gate is named once");
        let mut groups: Vec<&str> = steps.iter().map(|(_, group, _)| *group).collect();
        groups.sort_unstable();
        groups.dedup();
        assert_eq!(groups, [CONFORMANCE, DOCS, LINT, TEST]);
        // The workflow's matrix names each group once and no other, and its job runs that group's gates.
        let workflow = util::read(&util::root().join(".github/workflows/ci.yml")).unwrap();
        let mut jobs = workflow_groups(&workflow);
        jobs.sort_unstable();
        assert_eq!(jobs, groups, "ci.yml's matrix of groups");
        assert_eq!(workflow.matches("cargo xtask ci --group \"$GROUP\"").count(), 1);
        assert_eq!(workflow.matches("GROUP: ${{ matrix.group }}").count(), 1);
        // The check main's ruleset requires waits for every group and fails unless all of them passed.
        assert!(workflow.contains("    name: gates (cargo xtask ci)\n    needs: groups\n    if: ${{ always() }}\n"));
        assert!(workflow.contains("test \"$RESULT\" = success"));
        assert!(workflow.contains("RESULT: ${{ needs.groups.result }}"));
    }

    #[test]
    fn a_group_to_run_must_be_a_group() {
        let step = |name, group| -> Step { (name, group, Box::new(|| Outcome::Passed)) };
        let steps = [step("fmt", LINT), step("test", TEST)];
        assert_eq!(unknown_group(None, &steps), Ok(()));
        assert_eq!(unknown_group(Some("test"), &steps), Ok(()));
        assert_eq!(unknown_group(Some("docs"), &steps), Err("there is no group `docs` of gates (the groups: lint, test)".to_string()));
        assert_eq!(workflow_groups("  matrix:\n    group: [lint, test]\n    os: [a]\n"), ["lint", "test"]);
    }

    #[test]
    fn the_notice_gives_the_time_of_each_gate_that_ran() {
        let summary = [("fmt", Outcome::Passed, 3), ("api", Outcome::Skipped("--skip"), 0), ("test", Outcome::Failed("x".to_string()), 61)];
        assert_eq!(times(&summary), "the gates' times: fmt 3 s, test 61 s");
    }
}
