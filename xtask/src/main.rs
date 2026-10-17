//! `cargo xtask`: StitchCraft's repository automation.
//!
//! Every rule the project enforces runs from here — locally with `cargo xtask ci`, and in GitHub Actions
//! on every pull request — so the rules hold without anyone having to remember them
//! (`docs/src/design/guardrails.md`). Each subcommand is a module with its own tests.
//!
//! The grammar below declares every subcommand and option once: `clap` parses it, writes `--help` from it,
//! and refuses an argument it does not declare with exit status 2 before anything runs, so a misspelt
//! `--check` fails instead of running the command's other mode. `cargo xtask docs --check` checks each
//! `cargo xtask <name>` the docs and workflows mention against it ([`subcommands`]).
#![forbid(unsafe_code)]
// A command-line tool reports on the terminal, and reads and writes the repository's files, which
// `clippy.toml` keeps out of library crates.
#![allow(clippy::print_stdout, clippy::print_stderr, clippy::disallowed_methods)]

mod api;
mod ci;
mod cleanroom;
mod compat;
mod conformance;
mod contract_page;
mod coverage;
mod design_map;
mod deviations;
mod docs;
mod docs_audit;
mod examples;
mod figures;
mod filesize;
mod join;
mod layers;
mod markdown;
mod mutants;
mod param_pages;
mod perf;
mod prose;
mod reference_pages;
mod shots;
mod unsafe_audit;
mod util;
mod wasm;
mod window;

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

/// `cargo xtask`.
#[derive(Parser)]
#[command(name = "cargo xtask", bin_name = "cargo xtask", about = "StitchCraft repository automation", disable_version_flag = true)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run every gate and print a summary
    Ci {
        /// Leave out these gates, by name (`--skip api,prose`)
        #[arg(long, value_name = "GATE", value_delimiter = ',')]
        skip: Vec<String>,
    },
    /// Check that crates depend only on lower layers
    Layers,
    /// Regenerate the generated pages, or check the docs
    #[command(args_conflicts_with_subcommands = true)]
    Docs {
        /// Verify the docs without writing
        #[arg(long)]
        check: bool,
        #[command(subcommand)]
        pages: Option<Docs>,
    },
    /// Lint the Markdown lines a branch adds with Vale (house style and ai-tells)
    Prose {
        /// The branch is what changed since its merge base with REF
        #[arg(long, value_name = "REF", default_value = prose::DEFAULT_BASE)]
        base: String,
    },
    /// Run the conformance suite and write the report
    Conformance {
        /// Check requirements and cases without running them
        #[arg(long)]
        check: bool,
        /// Run only the cases whose id, test name or requirements contain TEXT
        #[arg(long, value_name = "TEXT")]
        filter: Option<String>,
        /// Rewrite the golden files of one data case
        #[arg(long, value_name = "CASE")]
        bless: Option<String>,
    },
    /// No GPL licence text or pasted Python source anywhere in the repository
    Cleanroom,
    /// Unsafe only in the plug-in ABI shim, always with SAFETY comments
    UnsafeAudit,
    /// Rust files warn above 800 lines and fail above 1,500
    Filesize,
    /// Library crates and the plug-in build for wasm32-unknown-unknown
    Wasm {
        /// Instead, make the golden files with `stitch` built as WebAssembly (WASI, run by wasmtime), byte
        /// for byte
        #[arg(long)]
        goldens: bool,
    },
    /// VectorCraft compatibility
    Compat {
        #[command(subcommand)]
        command: Compat,
    },
    /// Regenerate the documentation images
    Shots {
        /// Compare them with the committed ones without writing
        #[arg(long)]
        check: bool,
    },
    /// Write the library crates' public API snapshots
    Api {
        /// Compare them with the committed ones without writing
        #[arg(long)]
        check: bool,
    },
    /// Line coverage per crate against the floors in conformance/coverage.toml
    Coverage {
        /// Raise the floors to today's coverage
        #[arg(long)]
        record: bool,
    },
    /// Add up cargo-mutants results against conformance/mutation.toml
    Mutants {
        /// Lower the recorded counts to today's
        #[arg(long)]
        record: bool,
        /// Fail for any mutant of the changed lines that no test notices
        #[arg(long, conflicts_with = "record")]
        changed: bool,
        /// cargo-mutants output directories, each holding a mutants.out/
        #[arg(required = true)]
        dirs: Vec<PathBuf>,
    },
    /// Plan the hoop's whole field: instructions against conformance/perf.toml, time against NFR-PERF-1
    Perf {
        /// Lower the record to today's count
        #[arg(long)]
        record: bool,
    },
    /// Download the pinned real-world corpora for Ink/Stitch differential testing (planned: M8)
    Corpus,
}

#[derive(Subcommand)]
enum Docs {
    /// Name the pages that describe each file
    For {
        /// Answer a Claude Code PostToolUse hook, which names the edited file on standard input
        #[arg(long, conflicts_with = "paths")]
        hook: bool,
        /// Files, by path from the repository root
        #[arg(required_unless_present = "hook")]
        paths: Vec<String>,
    },
}

#[derive(Subcommand)]
enum Compat {
    /// The VectorCraft versions each track names today
    Discover {
        /// As JSON, for the workflows
        #[arg(long)]
        json: bool,
    },
    /// Build the plug-in's modules for wasm32 and load them into VectorCraft's host at a version
    Contract {
        /// The VectorCraft tag or commit
        #[arg(long = "ref", value_name = "TAG|SHA")]
        reference: String,
    },
    /// Level B: the scenarios of compat/scenarios with a VectorCraft release's vectorcraft-cli, or another
    E2e {
        /// The release, whose Linux archive is fetched and checked by its SHA-256
        #[arg(long = "ref", value_name = "TAG", required_unless_present = "cli")]
        reference: Option<String>,
        /// A vectorcraft-cli to run instead, such as one built from a branch
        #[arg(long, value_name = "FILE")]
        cli: Option<PathBuf>,
    },
    /// Both levels on one track, with its result in target/compat/results for the report
    Track {
        /// stable, prerelease, release-branch or main
        track: String,
        /// The VectorCraft tag or commit
        #[arg(long = "ref", value_name = "TAG|SHA")]
        reference: String,
        /// The vectorcraft-cli to run, for a branch; a release's is fetched
        #[arg(long, value_name = "FILE")]
        cli: Option<PathBuf>,
    },
    /// VectorCraft's window without a screen: screenshots alike, and xa11y driving it (Linux)
    Window {
        /// The app to run, instead of the pinned stable release's `bin/vectorcraft`
        #[arg(long, value_name = "FILE")]
        app: Option<PathBuf>,
    },
    /// Join this folder to a VectorCraft checkout's workspace
    Join {
        /// Keep StitchCraft's own workspace inside VectorCraft's
        #[arg(long)]
        nested: bool,
        /// The VectorCraft checkout
        vectorcraft: PathBuf,
    },
    /// What a scheduled run does with the tracks' results: issues, and the adoption of a new release (JSON)
    Report {
        /// The folder of the tracks' results
        #[arg(long, value_name = "DIR")]
        results: PathBuf,
        /// What `compat discover --json` said; empty when discovery failed
        #[arg(long, value_name = "JSON")]
        discovered: String,
        /// How the move job ended
        #[arg(long = "move", value_name = "RESULT", value_parser = compat::report::JOB_RESULTS)]
        moved: String,
        /// The day of the run, YYYY-MM-DD in UTC
        #[arg(long)]
        date: String,
        /// The run's address, for the issues and the pull request
        #[arg(long, value_name = "URL")]
        run_url: String,
        /// Write the adoption: the pins, the contract crate's tag and the record
        #[arg(long)]
        write: bool,
    },
    /// File one of the issues `compat report` decides, in the repository GITHUB_REPOSITORY names (gh)
    Issue {
        /// The issue, as `compat report` prints it
        #[arg(value_name = "JSON")]
        issue: String,
    },
}

/// The subcommands' names, against which `cargo xtask docs --check` checks every `cargo xtask <name>`.
pub fn subcommands() -> Vec<String> {
    Cli::command().get_subcommands().map(|command| command.get_name().to_string()).collect()
}

fn run(command: Command) -> Result<(), String> {
    match command {
        Command::Ci { skip } => ci::run(&skip),
        Command::Layers => layers::run(),
        Command::Docs { pages: Some(Docs::For { hook, paths }), .. } => design_map::run_for(hook, &paths),
        Command::Docs { check, pages: None } => docs::run(check),
        Command::Prose { base } => prose::run(&base),
        Command::Conformance { check, filter, bless } => conformance::run(check, filter.as_deref(), bless.as_deref()),
        Command::Cleanroom => cleanroom::run(),
        Command::UnsafeAudit => unsafe_audit::run(),
        Command::Filesize => filesize::run(),
        Command::Wasm { goldens: false } => wasm::run(),
        Command::Wasm { goldens: true } => wasm::goldens(),
        Command::Compat { command: Compat::Discover { json } } => compat::discover(json),
        Command::Compat { command: Compat::Contract { reference } } => compat::contract(&reference),
        Command::Compat { command: Compat::Window { app } } => window::run(app.as_deref()),
        Command::Compat { command: Compat::Join { nested, vectorcraft } } => join::run(nested, &vectorcraft),
        Command::Compat { command: Compat::E2e { reference, cli } } => compat::e2e(reference.as_deref(), cli.as_deref()),
        Command::Compat { command: Compat::Track { track, reference, cli } } => compat::track(&track, &reference, cli.as_deref()),
        Command::Compat { command: Compat::Report { results, discovered, moved, date, run_url, write } } => {
            compat::report::run(&results, &discovered, &moved, &date, &run_url, write)
        }
        Command::Compat { command: Compat::Issue { issue } } => compat::report::file(&issue),
        Command::Shots { check } => shots::run(check),
        Command::Api { check } => api::run(check),
        Command::Coverage { record } => coverage::run(record),
        Command::Mutants { record, changed, dirs } => mutants::run(record, changed, &dirs),
        Command::Perf { record } => perf::run(record),
        Command::Corpus => Err("`corpus` arrives with roadmap step M8".to_string()),
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // Help prints to standard output and succeeds; anything else is a usage error, status 2.
            let _ = e.print();
            return ExitCode::from(u8::try_from(e.exit_code()).unwrap_or(2));
        }
    };
    let Some(command) = cli.command else {
        let _ = Cli::command().print_help();
        return ExitCode::SUCCESS;
    };
    let name = std::env::args().nth(1).unwrap_or_default();
    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("xtask: {e}");
            if std::env::var_os("GITHUB_ACTIONS").is_some() {
                println!("{}", util::annotation("error", &format!("cargo xtask {name}"), &e));
            }
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grammar_is_consistent() {
        Cli::command().debug_assert();
    }

    #[test]
    fn the_workflows_invocations_parse() {
        for args in [
            &["docs", "--check"][..],
            &["docs", "for", "--hook"],
            &["docs", "for", "crates/stitchcraft-core/src/lib.rs"],
            &["conformance", "--check"],
            &["conformance", "--filter", "ts-01", "--bless", "ts-01"],
            &["compat", "discover", "--json"],
            &["compat", "contract", "--ref", "v0.7.0"],
            &["compat", "e2e", "--ref", "v0.12.0"],
            &["compat", "e2e", "--cli", "vectorcraft/target/release/vectorcraft-cli"],
            &["compat", "track", "stable", "--ref", "v0.12.0"],
            &["compat", "track", "main", "--ref", "f7de3d6", "--cli", "vectorcraft/target/release/vectorcraft-cli"],
            &[
                "compat",
                "report",
                "--results",
                "results",
                "--discovered",
                "{}",
                "--move",
                "success",
                "--date",
                "2026-10-18",
                "--run-url",
                "u",
                "--write",
            ],
            &["compat", "report", "--results", "r", "--discovered", "", "--move", "skipped", "--date", "d", "--run-url", "u"],
            &["compat", "issue", "{}"],
            &["compat", "join", "--nested", ".."],
            &["compat", "window"],
            &["compat", "window", "--app", "vectorcraft/target/release/vectorcraft"],
            &["mutants", "--changed", "a", "b"],
            &["prose", "--base", "origin/main"],
            &["ci", "--skip", "api,prose"],
            &["wasm", "--goldens"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("xtask").chain(args.iter().copied())).is_ok(), "{args:?}");
        }
        for args in [
            &["docs", "--check", "for", "x"][..],
            &["docs", "for"],
            &["mutants"],
            &["mutants", "--record", "--changed", "a"],
            &["compat", "e2e"],
            &["compat", "report", "--results", "r", "--discovered", "{}", "--move", "timed-out", "--date", "d", "--run-url", "u"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("xtask").chain(args.iter().copied())).is_err(), "{args:?}");
        }
        assert!(subcommands().iter().any(|name| name == "unsafe-audit") && subcommands().iter().any(|name| name == "corpus"));
    }
}
