//! `cargo xtask ci`: every gate, in one command, with a summary.
//!
//! Runs all steps even when one fails, so a contributor sees every problem at once. Steps that need an
//! optional tool (the wasm target, cargo-public-api, `cargo-deny`, `typos`, `mdbook`) are skipped locally
//! when the tool is missing and required in CI (`CI` set), where the workflow installs them. Coverage is
//! not a step here: it runs the whole suite again, instrumented, so CI gives it a job of its own.
//!
//! Cargo steps name this repository's packages, so inside VectorCraft's workspace (ADR-0011) they build,
//! lint and test StitchCraft only. There, `cargo-deny` is skipped, because it judges a whole workspace
//! against one policy and VectorCraft's workspace answers to VectorCraft's, and optional tools stay
//! optional: the move rehearsal checks that the code moves, while StitchCraft's own CI runs every tool.

use crate::util;
use crate::{api, cleanroom, conformance, docs, filesize, layers, shots, unsafe_audit, wasm};

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

fn tool_step(program: &'static str, args: &'static [&'static str]) -> Box<dyn Fn() -> Outcome> {
    Box::new(move || {
        let mut cmd = std::process::Command::new(program);
        cmd.args(args);
        outcome(util::run(cmd, &format!("{program} {}", args.join(" "))))
    })
}

/// `cargo xtask ci`.
pub fn run() -> Result<(), String> {
    let packages = util::packages()?;
    let guest = packages.guest;
    let doc_args = packages.args();
    let deny: Box<dyn Fn() -> Outcome> = if guest {
        Box::new(|| Outcome::Skipped("inside another workspace, whose own dependency policy applies"))
    } else {
        optional(|| util::tool_available("cargo-deny", &["--version"]), "cargo install cargo-deny", guest, tool_step("cargo-deny", &["check"]))
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
        ("conformance", Box::new(|| outcome(conformance::run(&[])))),
        ("wasm", optional(wasm::target_installed, "rustup target add wasm32-unknown-unknown", guest, Box::new(|| outcome(wasm::run())))),
        ("api", optional(api::available, api::INSTALL, guest, Box::new(|| outcome(api::run(true))))),
        ("deny", deny),
        ("typos", optional(|| util::tool_available("typos", &["--version"]), "cargo install typos-cli", guest, tool_step("typos", &[]))),
        ("book", optional(|| util::tool_available("mdbook", &["--version"]), "cargo install mdbook", guest, tool_step("mdbook", &["build", "docs"]))),
    ];
    let mut summary = Vec::new();
    for (name, step) in &steps {
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
