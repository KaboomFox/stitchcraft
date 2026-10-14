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
//! Cargo steps name this repository's packages, so inside VectorCraft's workspace (ADR-0011) they build,
//! lint and test StitchCraft only. There, `cargo-deny` is skipped, because it judges a whole workspace
//! against one policy and VectorCraft's workspace answers to VectorCraft's, and optional tools stay
//! optional: the move rehearsal checks that the code moves, while StitchCraft's own CI runs every tool.

use crate::util;
use crate::{api, cleanroom, conformance, docs, filesize, layers, prose, shots, unsafe_audit, wasm};

/// How one step ended.
enum Outcome {
    Passed,
    Failed(String),
    Skipped(&'static str),
}

type Step = (&'static str, Box<dyn Fn() -> Outcome>);

/// `cargo <command> -p … <rest>`: a Cargo step over this repository's packages.
fn cargo_step(command: &'static str, packages: &util::Packages, rest: &'static [&'static str]) -> Box<dyn Fn() -> Outcome> {
    let package_args = packages.args();
    Box::new(move || {
        let mut cmd = util::cargo();
        cmd.arg(command).args(&package_args).args(rest);
        outcome(util::run(cmd, &format!("cargo {command} {}", rest.join(" "))))
    })
}

fn outcome(result: Result<(), String>) -> Outcome {
    match result {
        Ok(()) => Outcome::Passed,
        Err(e) => Outcome::Failed(e),
    }
}

/// A step that needs an optional tool: skipped without it, except in StitchCraft's own CI, which fails.
fn optional(available: fn() -> bool, missing: &'static str, guest: bool, step: Box<dyn Fn() -> Outcome>) -> Box<dyn Fn() -> Outcome> {
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

/// cargo-deny's runs: the workspace, and the fuzz targets, a workspace of their own (`fuzz/`) with
/// dependencies of their own. The compatibility probe (`compat/vc-contract`) is not judged here. Its
/// dependencies are VectorCraft's, at whichever version a track tests, and VectorCraft's policy applies
/// to them, as it does inside VectorCraft's workspace.
const DENY: &[&[&str]] = &[&["check"], &["--manifest-path", "fuzz/Cargo.toml", "check"]];

/// An error naming the first of `skip` that is none of `gates`.
fn unknown_gate(skip: &[String], gates: &[&str]) -> Result<(), String> {
    match skip.iter().find(|gate| !gates.contains(&gate.as_str())) {
        Some(unknown) => Err(format!("there is no gate `{unknown}` to skip (the gates: {})", gates.join(", "))),
        None => Ok(()),
    }
}

/// `cargo xtask ci`, without the gates named in `skip`.
pub fn run(skip: &[String]) -> Result<(), String> {
    let packages = util::packages()?;
    let guest = packages.guest;
    let doc_args = packages.args();
    let deny: Box<dyn Fn() -> Outcome> = if guest {
        Box::new(|| Outcome::Skipped("inside another workspace, whose own dependency policy applies"))
    } else {
        optional(|| util::tool_available("cargo-deny", &["--version"]), "cargo install cargo-deny", guest, tool_step("cargo-deny", DENY))
    };
    let steps: Vec<Step> = vec![
        ("fmt", cargo_step("fmt", &packages, &["--", "--check"])),
        ("clippy", cargo_step("clippy", &packages, &["--all-targets", "--locked", "--", "-D", "warnings"])),
        ("test", cargo_step("test", &packages, &["--locked"])),
        (
            "rustdoc",
            Box::new(move || {
                // Broken, ambiguous or private intra-doc links are documentation bugs.
                let mut cmd = util::cargo();
                cmd.arg("doc").args(&doc_args).args(["--no-deps", "--locked"]).env("RUSTDOCFLAGS", "-D warnings");
                outcome(util::run(cmd, "cargo doc (warnings denied)"))
            }),
        ),
        ("layers", Box::new(|| outcome(layers::run()))),
        ("filesize", Box::new(|| outcome(filesize::run()))),
        ("cleanroom", Box::new(|| outcome(cleanroom::run()))),
        ("unsafe-audit", Box::new(|| outcome(unsafe_audit::run()))),
        ("docs", Box::new(|| outcome(docs::run(true)))),
        ("shots", Box::new(|| outcome(shots::run(true)))),
        ("conformance", Box::new(|| outcome(conformance::run(false, None, None)))),
        ("wasm", optional(wasm::target_installed, "rustup target add wasm32-unknown-unknown", guest, Box::new(|| outcome(wasm::run())))),
        ("api", optional(api::available, api::INSTALL, guest, Box::new(|| outcome(api::run(true))))),
        ("deny", deny),
        ("typos", optional(|| util::tool_available("typos", &["--version"]), "cargo install typos-cli", guest, tool_step("typos", &[&[]]))),
        ("prose", optional(prose::available, prose::INSTALL, guest, Box::new(|| outcome(prose::run(prose::DEFAULT_BASE))))),
        (
            "book",
            optional(|| util::tool_available("mdbook", &["--version"]), "cargo install mdbook", guest, tool_step("mdbook", &[&["build", "docs"]])),
        ),
    ];
    unknown_gate(skip, &steps.iter().map(|(name, _)| *name).collect::<Vec<_>>())?;
    let mut summary = Vec::new();
    for (name, step) in &steps {
        if skip.iter().any(|gate| gate.as_str() == *name) {
            summary.push((*name, Outcome::Skipped("--skip")));
            continue;
        }
        eprintln!("\n=== ci: {name} ===");
        let result = step();
        summary.push((*name, result));
    }
    println!("\nCI summary:");
    let mut failed = 0;
    for (name, result) in &summary {
        match result {
            Outcome::Passed => println!("  ok    {name}"),
            Outcome::Skipped(why) => println!("  skip  {name} ({why})"),
            Outcome::Failed(e) => {
                failed += 1;
                println!("  FAIL  {name}: {e}");
                if std::env::var_os("GITHUB_ACTIONS").is_some() {
                    println!("{}", util::annotation("error", "cargo xtask ci", &format!("{name} failed: {e}")));
                }
            }
        }
    }
    if failed == 0 { Ok(()) } else { Err(format!("{failed} of {} gates failed", summary.len())) }
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
}
