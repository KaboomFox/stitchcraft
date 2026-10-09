//! `cargo xtask`: StitchCraft's repository automation.
//!
//! Every rule the project enforces runs from here — locally with `cargo xtask ci`, and in GitHub Actions
//! on every pull request — so the rules hold without anyone having to remember them
//! (`docs/src/design/guardrails.md`). Each subcommand is a module with its own tests.
#![forbid(unsafe_code)]
// A command-line tool reports on the terminal.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod api;
mod ci;
mod cleanroom;
mod compat;
mod conformance;
mod contract_page;
mod coverage;
mod deviations;
mod docs;
mod filesize;
mod layers;
mod markdown;
mod mutants;
mod param_pages;
mod reference_pages;
mod shots;
mod unsafe_audit;
mod util;
mod wasm;

use std::process::ExitCode;

/// Whether a subcommand works today or arrives with a roadmap step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Implemented.
    Ready,
    /// Arrives with the given roadmap step; documents may already describe it.
    Planned(&'static str),
}

/// Every subcommand: name, status, one-line help. `cargo xtask docs --check` fails when a document mentions
/// `cargo xtask <name>` for a name that is not listed here.
pub const SUBCOMMANDS: &[(&str, Status, &str)] = &[
    ("ci", Status::Ready, "run every gate and print a summary"),
    ("layers", Status::Ready, "check that crates depend only on lower layers"),
    ("docs", Status::Ready, "regenerate generated pages; --check verifies docs without writing"),
    ("conformance", Status::Ready, "run the conformance suite and write the report; --check, --filter TEXT, --bless CASE"),
    ("cleanroom", Status::Ready, "no GPL licence text or Ink/Stitch source paths anywhere in the repository"),
    ("unsafe-audit", Status::Ready, "unsafe only in the plug-in ABI shim, always with SAFETY comments"),
    ("filesize", Status::Ready, "Rust files warn above 800 lines and fail above 1,500"),
    ("wasm", Status::Ready, "library crates and the plug-in build for wasm32-unknown-unknown"),
    ("compat", Status::Ready, "VectorCraft compatibility: `discover`, `contract --ref R --wasm FILE` (report: M6.6)"),
    ("shots", Status::Ready, "regenerate the documentation images; --check compares them with the committed ones"),
    ("api", Status::Ready, "write the library crates' public API snapshots; --check compares them"),
    ("coverage", Status::Ready, "line coverage per crate against the floors in conformance/coverage.toml; --record raises floors"),
    ("mutants", Status::Ready, "add up cargo-mutants results (DIR…) against conformance/mutation.toml; --record lowers counts"),
    ("corpus", Status::Planned("M8"), "download the pinned real-world corpora (Ink/Stitch differential testing)"),
];

fn usage() -> String {
    let mut text = String::from("cargo xtask — StitchCraft repository automation\n\nUSAGE: cargo xtask <command> [options]\n\n");
    for (name, status, help) in SUBCOMMANDS {
        let tag = match status {
            Status::Ready => String::new(),
            Status::Planned(step) => format!(" (planned: {step})"),
        };
        text.push_str(&format!("  {name:<14}{help}{tag}\n"));
    }
    text
}

fn main() -> ExitCode {
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(a) => args.push(a),
            Err(raw) => {
                eprintln!("xtask: argument is not valid Unicode: {}", raw.to_string_lossy());
                return ExitCode::from(2);
            }
        }
    }
    let Some((command, rest)) = args.split_first() else {
        print!("{}", usage());
        return ExitCode::SUCCESS;
    };
    let flag = |f: &str| rest.iter().any(|a| a == f);
    let result = match command.as_str() {
        "ci" => ci::run(),
        "layers" => layers::run(),
        "docs" => docs::run(flag("--check")),
        "conformance" => conformance::run(rest),
        "cleanroom" => cleanroom::run(),
        "unsafe-audit" => unsafe_audit::run(),
        "filesize" => filesize::run(),
        "wasm" => wasm::run(),
        "compat" => compat::run(rest),
        "shots" => shots::run(flag("--check")),
        "api" => api::run(flag("--check")),
        "coverage" => coverage::run(flag("--record")),
        "mutants" => mutants::run(rest),
        "-h" | "--help" | "help" => {
            print!("{}", usage());
            Ok(())
        }
        other => match SUBCOMMANDS.iter().find(|(name, _, _)| *name == other) {
            Some((_, Status::Planned(step), _)) => Err(format!("`{other}` arrives with roadmap step {step}")),
            _ => Err(format!("unknown command `{other}`\n\n{}", usage())),
        },
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            if std::env::var_os("GITHUB_ACTIONS").is_some() {
                println!("{}", util::annotation("error", &format!("cargo xtask {command}"), &e));
            }
            ExitCode::FAILURE
        }
    }
}
