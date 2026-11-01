//! `stitch`: the StitchCraft command-line tool.
//!
//! The only part of StitchCraft that touches files and the terminal. Subcommands arrive with their roadmap
//! milestones (`docs/src/plan/roadmap.md`). Every subcommand returns an outcome instead of printing, so
//! `main` is the one place that writes to the terminal: arguments that are not valid Unicode are reported
//! (by `clap`) instead of panicking, and a panic — a bug in StitchCraft — is reported with exit status 4
//! (`commands::bug_report::guarded`).
//!
//! A closed standard output (`stitch … | head`) ends the writing quietly, and the command's own exit status
//! stands: what it did, it did. Any other error writing standard output or standard error is said, and
//! ends with exit status 3 (`REQ-CLI-009`). Help and usage errors are coloured only on a terminal, and not
//! when `NO_COLOR` is set or `TERM` is `dumb` (`REQ-CLI-010`).
#![forbid(unsafe_code)]
// The binary of an app: it reads its environment (`NO_COLOR`, `TERM`).
#![allow(clippy::disallowed_methods)]

use std::io::{ErrorKind, IsTerminal, Write};
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
        Command::Palette(args) => commands::palette::run(args),
        Command::Explain { code } => commands::explain::run(code),
        Command::BugReport(args) => commands::bug_report::run(args),
    }
}

fn main() -> ExitCode {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(e) => {
            // `--help` and `--version` arrive here too, as "errors" that print to stdout and succeed.
            let status = if e.use_stderr() { Status::Usage } else { Status::Done };
            let terminal = if e.use_stderr() { std::io::stderr().is_terminal() } else { std::io::stdout().is_terminal() };
            let rendered = e.render();
            let colour = coloured(terminal, std::env::var_os("NO_COLOR").as_deref(), std::env::var_os("TERM").as_deref());
            let text = if colour { rendered.ansi().to_string() } else { rendered.to_string() };
            let stream: &mut dyn Write = if e.use_stderr() { &mut std::io::stderr() } else { &mut std::io::stdout() };
            return written(stream, &text, status).into();
        }
    };
    // A panic is a bug in StitchCraft (the libraries deny them): reported, with a bundle for `plan`.
    let outcome = commands::bug_report::guarded(&cli.command, || run(&cli));
    let status = written(&mut std::io::stdout(), &outcome.stdout, outcome.status);
    written(&mut std::io::stderr(), &outcome.stderr, status).into()
}

/// Whether to colour what goes to a stream that is a `terminal` or not, with the environment's `NO_COLOR`
/// and `TERM`: on a terminal, unless `NO_COLOR` is set to anything (<https://no-color.org>) or the terminal
/// is `dumb`.
fn coloured(terminal: bool, no_color: Option<&std::ffi::OsStr>, term: Option<&std::ffi::OsStr>) -> bool {
    terminal && no_color.is_none_or(|value| value.is_empty()) && term.is_none_or(|term| term != "dumb")
}

/// Writes `text` to `out`, and the status the program ends with: `status`, also when `out` is a closed pipe,
/// or [`Status::Io`] after another error, which is said on standard error.
fn written(out: &mut dyn Write, text: &str, status: Status) -> Status {
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => status,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => status,
        Err(e) => {
            let _ = writeln!(std::io::stderr(), "stitch: cannot write the output: {e}");
            Status::Io
        }
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

    /// A stream that refuses every write with `kind`.
    struct Refusing(ErrorKind);

    impl Write for Refusing {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::from(self.0))
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn req_cli_009_a_closed_pipe_keeps_the_status_and_other_errors_end_with_3() {
        for status in [Status::Done, Status::DesignErrors, Status::Bug] {
            assert_eq!(written(&mut Refusing(ErrorKind::BrokenPipe), "text", status), status);
            assert_eq!(written(&mut Refusing(ErrorKind::StorageFull), "text", status), Status::Io);
            assert_eq!(written(&mut Vec::new(), "text", status), status);
        }
    }

    #[test]
    fn req_cli_010_colour_only_on_a_terminal_without_no_color_or_a_dumb_terminal() {
        use std::ffi::OsStr;
        assert!(coloured(true, None, Some(OsStr::new("xterm-256color"))));
        assert!(coloured(true, Some(OsStr::new("")), None), "NO_COLOR set to nothing does not count");
        assert!(!coloured(false, None, None), "a pipe or a file");
        assert!(!coloured(true, Some(OsStr::new("1")), None));
        assert!(!coloured(true, None, Some(OsStr::new("dumb"))));
    }

    #[test]
    fn req_vc_008_export_is_another_name_for_plan() {
        let Command::Plan(args) = parse(&["export", "patch.vectorcraft", "-o", "patch.pes"]).unwrap().command else {
            panic!("`stitch export` is `stitch plan`")
        };
        assert_eq!(args.design, std::path::PathBuf::from("patch.vectorcraft"));
    }

    #[test]
    fn req_cli_013_a_size_and_a_colour_count_out_of_range_are_usage_errors() {
        let plan = |option: &str, value: &str| parse(&["plan", "badge.png", "-o", "badge.pes", option, value]);
        for (option, value) in [("--size", "0"), ("--size", "-5"), ("--size", "big"), ("--size", "inf"), ("--colours", "1"), ("--colours", "257")] {
            assert!(plan(option, value).unwrap_err().use_stderr(), "{option} {value}");
        }
        let Command::Plan(args) = parse(&["plan", "badge.png", "-o", "badge.pes", "--size", "80.5", "--colours", "256"]).unwrap().command else {
            panic!("`stitch plan`")
        };
        assert_eq!((args.tracing.size.map(stitchcraft_core::Mm::get), args.tracing.colours), (Some(80.5), Some(256)));
        assert!(args.tracing.given());
        let Command::Plan(args) = parse(&["plan", "badge.png", "-o", "badge.pes"]).unwrap().command else { panic!("`stitch plan`") };
        assert!(!args.tracing.given());
    }

    #[test]
    fn the_colours_help_says_the_default_a_trace_takes() {
        use clap::CommandFactory;
        let mut cli = Cli::command();
        let help = cli
            .find_subcommand_mut("plan")
            .and_then(|plan| plan.get_arguments().find(|arg| arg.get_id() == "colours").and_then(|arg| arg.get_help()).map(ToString::to_string));
        let default = format!("by default {}", stitchcraft_cli::trace::COLOURS);
        assert!(help.as_deref().is_some_and(|help| help.contains(&default)), "{help:?}");
    }

    #[test]
    fn subcommands_dispatch() {
        let out = run(&parse(&["explain", "SC-E0701"]).unwrap());
        assert_eq!(out.status, Status::Done);
        assert!(out.stdout.starts_with("SC-E0701: Design does not fit the hoop"));
        assert_eq!(run(&parse(&["profiles"]).unwrap()).status, Status::Done);
    }
}
