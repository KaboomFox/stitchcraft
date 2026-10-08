//! `stitch`: the StitchCraft command-line tool.
//!
//! The only part of StitchCraft that touches files and the terminal. Subcommands arrive with their roadmap
//! milestones (`docs/src/plan/roadmap.md`); this bootstrap answers `--help` and `--version` and shows the
//! conventions every subcommand follows: arguments that are not valid Unicode are reported instead of
//! panicking, and a closed stdout ends the program quietly.
#![forbid(unsafe_code)]

use std::io::{ErrorKind, Write};
use std::process::ExitCode;

const USAGE: &str = "\
stitch — StitchCraft machine embroidery

USAGE:
  stitch --help        Show this help
  stitch --version     Show the version

Coming with the roadmap (docs/src/plan/roadmap.md):
  stitch testsheet TS-xx --profile brother-200x200 -o file.pes    (M1)
  stitch inspect FILE                                              (M1)
  stitch plan design.svg -o design.pes --preview design.png        (M3)
  stitch export design.vectorcraft -o design.pes                   (M6)
";

/// What the program wants to say and how it ends.
struct Outcome {
    stdout: String,
    stderr: String,
    code: ExitCode,
}

fn run(args: &[String]) -> Outcome {
    let ok = |text: String| Outcome { stdout: text, stderr: String::new(), code: ExitCode::SUCCESS };
    match args.first().map(String::as_str) {
        None | Some("-h" | "--help" | "help") => ok(USAGE.to_string()),
        Some("-V" | "--version") => ok(format!("stitch {}\n", env!("CARGO_PKG_VERSION"))),
        Some(other) => Outcome { stdout: String::new(), stderr: format!("stitch: unknown command `{other}`\n\n{USAGE}"), code: ExitCode::from(2) },
    }
}

fn main() -> ExitCode {
    let mut args = Vec::new();
    for arg in std::env::args_os().skip(1) {
        match arg.into_string() {
            Ok(arg) => args.push(arg),
            Err(raw) => {
                let message = format!("stitch: argument is not valid Unicode: {}\n", raw.to_string_lossy());
                return write_or_quit(&mut std::io::stderr(), &message, ExitCode::from(2));
            }
        }
    }
    let outcome = run(&args);
    let code = write_or_quit(&mut std::io::stdout(), &outcome.stdout, outcome.code);
    write_or_quit(&mut std::io::stderr(), &outcome.stderr, code)
}

/// Writes `text`; a closed pipe ends the program quietly with success, any other write error is a failure.
fn write_or_quit(out: &mut impl Write, text: &str, code: ExitCode) -> ExitCode {
    match out.write_all(text.as_bytes()).and_then(|()| out.flush()) {
        Ok(()) => code,
        Err(e) if e.kind() == ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(_) => ExitCode::FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_string()).collect()
    }

    #[test]
    fn help_and_version_succeed() {
        assert!(run(&args(&[])).stdout.contains("USAGE"));
        assert!(run(&args(&["--help"])).stdout.contains("USAGE"));
        assert!(run(&args(&["--version"])).stdout.starts_with("stitch "));
    }

    #[test]
    fn unknown_commands_are_errors_with_usage() {
        let out = run(&args(&["sew-it"]));
        assert!(out.stdout.is_empty());
        assert!(out.stderr.contains("unknown command `sew-it`") && out.stderr.contains("USAGE"));
    }
}
