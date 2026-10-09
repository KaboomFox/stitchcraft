//! The subcommands. Each returns an [`Outcome`] — what to print and how to exit — instead of printing,
//! so the commands are tested without capturing the terminal.

pub mod explain;
pub mod inspect;
pub mod profiles;
pub mod testsheet;

use std::fmt::Write as _;
use std::process::ExitCode;

use stitchcraft_core::{Diagnostic, Fix, Severity};
use stitchcraft_plan::StitchPlan;
use stitchcraft_plan::palette::Palette;

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
    /// The design or file has errors; nothing was written.
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

/// The plan's size and counts, as report lines (`  size      …`, `  stitches  …`).
pub fn describe_plan(out: &mut String, plan: &StitchPlan) {
    let stats = plan.stats();
    if let Some(b) = plan.bounds() {
        let _ = writeln!(out, "  size      {:.1} × {:.1} mm", b.width(), b.height());
    }
    let counts = [(stats.stitches, "stitch", "stitches"), (stats.jumps, "jump", "jumps"), (stats.trims, "trim", "trims")]
        .into_iter()
        .chain([(stats.color_changes, "colour change", "colour changes"), (stats.stops, "stop", "stops")])
        .map(|(n, one, many)| format!("{n} {}", if n == 1 { one } else { many }))
        .collect::<Vec<_>>()
        .join(", ");
    let _ = writeln!(out, "  stitches  {counts}");
}

/// The threads the machine asks for, one line each, with the palette entry a machine shows for each
/// colour when the format stores palette indices.
pub fn describe_threads(out: &mut String, plan: &StitchPlan, palette: Option<&Palette>) {
    for (i, entry) in plan.color_entries().iter().enumerate() {
        let thread = entry.thread;
        let name = thread.name.as_deref().unwrap_or("unnamed");
        let mut line = format!("{}. {name} ({})", i + 1, thread.color);
        if let (Some(palette), Some(matched)) = (palette, palette.and_then(|p| p.nearest(thread.color))) {
            let _ = write!(line, ", shown as {} {} \"{}\"", palette.name, matched.index, matched.name);
        }
        if entry.stop {
            line.push_str(" — a stop: keep the same thread");
        }
        let _ = writeln!(out, "  {:<10}{line}", if i == 0 { "threads" } else { "" });
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
