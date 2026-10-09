//! `cargo xtask ci`: every gate, in one command, with a summary.
//!
//! Runs all steps even when one fails, so a contributor sees every problem at once. Steps that need an
//! optional tool (the wasm target, `cargo-deny`, `typos`, `mdbook`) are skipped locally when the tool is
//! missing and required in CI (`CI` set), where the workflow installs them.

use crate::util;
use crate::{cleanroom, conformance, docs, filesize, layers, shots, unsafe_audit, wasm};

/// How one step ended.
enum Outcome {
    Passed,
    Failed(String),
    Skipped(&'static str),
}

type Step = (&'static str, Box<dyn Fn() -> Outcome>);

fn cargo_step(args: &'static [&'static str]) -> Box<dyn Fn() -> Outcome> {
    Box::new(move || {
        let mut cmd = util::cargo();
        cmd.args(args);
        outcome(util::run(cmd, &format!("cargo {}", args.join(" "))))
    })
}

fn outcome(result: Result<(), String>) -> Outcome {
    match result {
        Ok(()) => Outcome::Passed,
        Err(e) => Outcome::Failed(e),
    }
}

/// A step that needs an optional tool: skipped locally without it, failed in CI without it.
fn optional(available: fn() -> bool, missing: &'static str, step: Box<dyn Fn() -> Outcome>) -> Box<dyn Fn() -> Outcome> {
    Box::new(move || {
        if available() {
            step()
        } else if util::in_ci() {
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
    let steps: Vec<Step> = vec![
        ("fmt", cargo_step(&["fmt", "--all", "--", "--check"])),
        ("clippy", cargo_step(&["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"])),
        ("test", cargo_step(&["test", "--workspace", "--locked"])),
        ("layers", Box::new(|| outcome(layers::run()))),
        ("filesize", Box::new(|| outcome(filesize::run()))),
        ("cleanroom", Box::new(|| outcome(cleanroom::run()))),
        ("unsafe-audit", Box::new(|| outcome(unsafe_audit::run()))),
        ("docs", Box::new(|| outcome(docs::run(true)))),
        ("shots", Box::new(|| outcome(shots::run(true)))),
        ("conformance", Box::new(|| outcome(conformance::run(&[])))),
        ("wasm", optional(wasm::target_installed, "rustup target add wasm32-unknown-unknown", Box::new(|| outcome(wasm::run())))),
        ("deny", optional(|| util::tool_available("cargo-deny", &["--version"]), "cargo install cargo-deny", tool_step("cargo-deny", &["check"]))),
        ("typos", optional(|| util::tool_available("typos", &["--version"]), "cargo install typos-cli", tool_step("typos", &[]))),
        ("book", optional(|| util::tool_available("mdbook", &["--version"]), "cargo install mdbook", tool_step("mdbook", &["build", "docs"]))),
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
