//! `stitch`: the StitchCraft command-line tool.
//!
//! The only part of StitchCraft that touches files and the terminal. Subcommands arrive with their roadmap
//! milestones (`docs/src/plan/roadmap.md`). Every subcommand returns an outcome instead of printing, so
//! `main` is the one place that writes to the terminal: arguments that are not valid Unicode are reported
//! (by `clap`) instead of panicking, and a closed stdout (`stitch … | head`) ends the program quietly.
#![forbid(unsafe_code)]

use std::io::{ErrorKind, Write};
use std::process::ExitCode;

use clap::Parser;
use stitchcraft_cli::cli::{Cli, Command};
use stitchcraft_cli::commands::{self, Outcome, Status};

fn run(cli: &Cli) -> Outcome {
    match &cli.command {
        Command::Plan(args) => commands::plan::run(args),
        Command::Testsheet(args) => commands::testsheet::run(args),
        Command::Inspect(args) => commands::inspect::run(args),
        Command::Preview(args) => commands::preview::run(args),
        Command::Convert(args) => commands::convert::run(args),
        Command::Profiles => commands::profiles::run(),
        Command::Explain { code } => commands::explain::run(code),
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // `--help` and `--version` arrive here too, as "errors" that print to stdout and succeed.
            let status = if e.use_stderr() { Status::Usage } else { Status::Done };
            let text = e.render().ansi().to_string();
            let stream: &mut dyn Write = if e.use_stderr() { &mut std::io::stderr() } else { &mut std::io::stdout() };
            return write_or_quit(stream, &text, status.into());
        }
    };
    let outcome = run(&cli);
    let code = write_or_quit(&mut std::io::stdout(), &outcome.stdout, outcome.status.into());
    write_or_quit(&mut std::io::stderr(), &outcome.stderr, code)
}

/// Writes `text`; a closed pipe ends the program quietly with success, any other write error is a failure.
fn write_or_quit(out: &mut dyn Write, text: &str, code: ExitCode) -> ExitCode {
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => code,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("stitch").chain(args.iter().copied()))
    }

    #[test]
    fn the_grammar_is_consistent() {
        use clap::CommandFactory;
        Cli::command().debug_assert();
    }

    #[test]
    fn help_and_version_are_not_errors() {
        for flag in ["--help", "--version"] {
            let e = parse(&[flag]).unwrap_err();
            assert!(!e.use_stderr(), "{flag}");
        }
    }

    #[test]
    fn missing_arguments_are_usage_errors() {
        assert!(parse(&[]).unwrap_err().use_stderr());
        assert!(parse(&["testsheet", "TS-01"]).unwrap_err().use_stderr(), "profile and output are required");
        assert!(parse(&["testsheet", "--list"]).is_ok());
        assert!(parse(&["sew-it"]).unwrap_err().use_stderr());
    }

    #[test]
    fn subcommands_dispatch() {
        let out = run(&parse(&["explain", "SC-E0701"]).unwrap());
        assert_eq!(out.status, Status::Done);
        assert!(out.stdout.starts_with("SC-E0701: Design does not fit the hoop"));
        assert_eq!(run(&parse(&["profiles"]).unwrap()).status, Status::Done);
    }
}
