//! The subcommands. Each returns an [`Outcome`] — what to print and how to exit — instead of printing,
//! so the commands are tested without capturing the terminal.

pub mod bug_report;
pub mod convert;
pub mod explain;
pub mod inspect;
pub mod plan;
pub mod preview;
pub mod profiles;
pub mod testsheet;

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use sha2::{Digest, Sha256};
use stitchcraft_core::{Code, Diagnostic, Fix, Severity};
use stitchcraft_formats::Decoded;
use stitchcraft_plan::palette::Palette;
use stitchcraft_plan::{FormatId, StitchPlan};

use crate::files;

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
    /// StitchCraft found a bug in itself; a bug-report bundle was written where it could be.
    Bug = 4,
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

    /// A file could not be read or written.
    fn io(verb: &str, path: &Path, error: &std::io::Error) -> Self {
        Outcome { stdout: String::new(), stderr: format!("stitch: cannot {verb} {}: {error}\n", path.display()), status: Status::Io }
    }

    /// Errors in the design: `stderr` (warnings found so far) and the diagnostics, and nothing written. A
    /// failed internal check (`SC-E0009`) is a bug in StitchCraft, not in the design, and ends with
    /// [`Status::Bug`].
    pub fn refuse(stderr: String, diagnostics: &[Diagnostic]) -> Self {
        let bug = diagnostics.iter().any(|d| d.code == Code::InternalCheckFailed);
        let status = if bug { Status::Bug } else { Status::DesignErrors };
        Outcome { stdout: String::new(), stderr: stderr + &render_diagnostics(diagnostics) + "nothing was written\n", status }
    }
}

/// The bytes of the machine file at `path` and what they say, or the outcome that reports why they
/// cannot be read (`SC-E0603` for a file that is not a machine file StitchCraft reads).
pub fn read_machine_file(path: &Path) -> Result<(Vec<u8>, Decoded), Outcome> {
    let bytes = files::read_capped(path).map_err(|e| Outcome::io("read", path, &e))?;
    match stitchcraft_formats::decode(&bytes) {
        Ok(decoded) => Ok((bytes, decoded)),
        Err(e) => Err(Outcome { stdout: String::new(), stderr: render_diagnostics(&[e.diagnostic()]), status: Status::DesignErrors }),
    }
}

/// The readers' warnings about `decoded`, one `warning:` line each.
pub fn reader_warnings(decoded: &Decoded) -> String {
    decoded.warnings.iter().map(|w| format!("warning: {w}\n")).collect()
}

/// `SC-W0604` when `decoded` stores no thread colours, so its threads are placeholders.
pub fn unknown_colors(decoded: &Decoded) -> Option<Diagnostic> {
    decoded.palette.is_none().then(|| {
        Diagnostic::new(Code::ThreadColorsUnknown, format!("{} stores no thread colours; every thread is a black placeholder.", decoded.format))
    })
}

/// Writes `bytes` to `path` in one step ([`files::write_atomically`]).
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), Outcome> {
    files::write_atomically(path, bytes).map_err(|e| Outcome::io("write", path, &e))
}

/// The format `--format` names, or else the one `output`'s extension names.
pub fn output_format(flag: Option<crate::cli::Format>, output: &Path) -> Result<FormatId, Outcome> {
    let extension = output.extension().and_then(|e| e.to_str()).and_then(FormatId::from_extension);
    flag.map(FormatId::from)
        .or(extension)
        .ok_or_else(|| Outcome::usage(format!("cannot tell the format of `{}`: name it .pes or .dst, or use --format", output.display())))
}

/// A written machine file, as report lines (`  file      …`, `  sha256    …`): the checksum is how a
/// sew-out report names exactly the bytes that were sewn.
pub fn describe_file(out: &mut String, output: &Path, format: FormatId, bytes: &[u8]) {
    let _ = writeln!(out, "  file      {} ({}, {} bytes)", output.display(), format.name(), bytes.len());
    let _ = writeln!(out, "  sha256    {}", hex(&Sha256::digest(bytes)));
}

/// `bytes` in lower-case hexadecimal.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::with_capacity(bytes.len() * 2), |mut s, b| {
        let _ = write!(s, "{b:02x}");
        s
    })
}

/// The plan's size and counts, as report lines (`  size      …`, `  stitches  …`).
pub fn describe_plan(out: &mut String, plan: &StitchPlan) {
    if let Some(b) = plan.bounds() {
        let _ = writeln!(out, "  size      {:.1} × {:.1} mm", b.width(), b.height());
    }
    let _ = writeln!(out, "  stitches  {}", counts(plan));
}

/// "274 stitches, 7 jumps, 7 trims, 0 colour changes, 1 stop": the plan's counts, as people say them.
pub fn counts(plan: &StitchPlan) -> String {
    let stats = plan.stats();
    [(stats.stitches, "stitch", "stitches"), (stats.jumps, "jump", "jumps"), (stats.trims, "trim", "trims")]
        .into_iter()
        .chain([(stats.color_changes, "colour change", "colour changes"), (stats.stops, "stop", "stops")])
        .map(|(n, one, many)| format!("{n} {}", if n == 1 { one } else { many }))
        .collect::<Vec<_>>()
        .join(", ")
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

/// A diagnostic's first line: `warning SC-W0307 (svg:patch:fill): …`, the element named when there is one.
pub fn headline(d: &Diagnostic) -> String {
    match &d.element {
        Some(element) => format!("{} {} ({element}): {}", d.severity().label(), d.code, d.message),
        None => d.to_string(),
    }
}

/// Diagnostics as people read them: one line each, then the fix indented.
pub fn render_diagnostics(diagnostics: &[Diagnostic]) -> String {
    let mut out = String::new();
    for d in diagnostics {
        out.push_str(&headline(d));
        out.push('\n');
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
