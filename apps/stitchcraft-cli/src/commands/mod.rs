//! The subcommands. Each returns an [`Outcome`] — what to print and how to exit — instead of printing,
//! so the commands are tested without capturing the terminal.

pub mod explain;
pub mod profiles;
pub mod testsheet;

use std::process::ExitCode;

use stitchcraft_core::{Diagnostic, Fix, Severity};

/// What a command wants to say, and how the program ends.
#[derive(Debug)]
pub struct Outcome {
    /// For standard output: results.
    pub stdout: String,
    /// For standard error: diagnostics and errors.
    pub stderr: String,
    /// The exit status.
    pub status: Status,
}

/// Exit statuses, as documented in `stitch --help` and the command-line reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    /// Done; warnings may have been printed.
    Done = 0,
    /// The design has errors; nothing was written.
    DesignErrors = 1,
    /// The command line was wrong.
    Usage = 2,
    /// A file could not be read or written.
    Io = 3,
}

impl From<Status> for ExitCode {
    fn from(status: Status) -> ExitCode {
        ExitCode::from(status as u8)
    }
}

impl Outcome {
    /// Success with `stdout`.
    pub fn done(stdout: String) -> Self {
        Outcome { stdout, stderr: String::new(), status: Status::Done }
    }

    /// A usage error.
    pub fn usage(message: impl AsRef<str>) -> Self {
        Outcome { stdout: String::new(), stderr: format!("stitch: {}\n", message.as_ref()), status: Status::Usage }
    }
}

/// Diagnostics as people read them: one line each, then the fix indented.
pub fn render_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    for d in diagnostics {
        out.push_str(&format!("{d}\n"));
        if let Some(fix) = &d.fix {
            let label = if matches!(fix, Fix::Apply(_)) { "fix" } else { "hint" };
            out.push_str(&format!("  {label}: {}\n", fix.describe()));
        }
        if d.severity() == Severity::Error {
            out.push_str(&format!("  more: stitch explain {}\n", d.code));
        }
    }
    out
}
