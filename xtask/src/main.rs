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
    Wasm,
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
    /// Load the built plug-in into VectorCraft's host at a version
    Contract {
        /// The VectorCraft tag or commit
        #[arg(long = "ref", value_name = "TAG|SHA")]
        reference: String,
        /// The plug-in's module
        #[arg(long, value_name = "FILE")]
        wasm: PathBuf,
    },
    /// Join this folder to a VectorCraft checkout's workspace
    Join {
        /// Keep StitchCraft's own workspace inside VectorCraft's
        #[arg(long)]
        nested: bool,
        /// The VectorCraft checkout
        vectorcraft: PathBuf,
    },
    /// The compatibility report (planned: M6.6)
    Report,
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
        Command::Wasm => wasm::run(),
        Command::Compat { command: Compat::Discover { json } } => compat::discover(json),
        Command::Compat { command: Compat::Contract { reference, wasm } } => compat::contract(&reference, &wasm),
        Command::Compat { command: Compat::Join { nested, vectorcraft } } => join::run(nested, &vectorcraft),
        Command::Compat { command: Compat::Report } => Err("`compat report` arrives with roadmap step M6.6".to_string()),
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
            &["compat", "contract", "--ref", "v0.7.0", "--wasm", "plugin.wasm"],
            &["compat", "join", "--nested", ".."],
            &["mutants", "--changed", "a", "b"],
            &["prose", "--base", "origin/main"],
            &["ci", "--skip", "api,prose"],
        ] {
            assert!(Cli::try_parse_from(std::iter::once("xtask").chain(args.iter().copied())).is_ok(), "{args:?}");
        }
        for args in [&["docs", "--check", "for", "x"][..], &["docs", "for"], &["mutants"], &["mutants", "--record", "--changed", "a"]] {
            assert!(Cli::try_parse_from(std::iter::once("xtask").chain(args.iter().copied())).is_err(), "{args:?}");
        }
        assert!(subcommands().iter().any(|name| name == "unsafe-audit") && subcommands().iter().any(|name| name == "corpus"));
    }
}
