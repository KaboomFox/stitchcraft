//! `stitch expand DESIGN -o expanded.vectorcraft`: an automatic design written as the ordinary shapes automatic
//! digitizing makes of it, each with its own StitchCraft effect, which a person edits one by one
//! (`REQ-CLI-025`, roadmap step M13.26; `docs/src/design/image-to-embroidery.md` › Expand).
//!
//! **Why the command line.** A VectorCraft filter isn't given the effects of the groups round a path, where
//! the Auto effect usually is, and digitizing a design takes an export's work, more than a filter gets. So
//! this command writes the document, and the person opens it in VectorCraft.
//!
//! **What it reads.** A design as `stitch plan --auto` reads it, every shape without settings automatic, with
//! the same `--set`, `--style`, `--text`, `--keep` and `--guide` ([`plan::read_design`]). A picture is traced
//! first, as `stitch plan` traces it ([`trace`]). The engine expands the design
//! ([`stitchcraft_engine::auto::expand`]).
//!
//! **What it writes.** A `.vectorcraft` output is the document given, or the one a picture was traced into,
//! with the traced shapes hidden and locked under a new group of the parts
//! ([`stitchcraft_vectorcraft::expanded`]). An `.svg` output is the parts alone, with Ink/Stitch's
//! attributes ([`stitchcraft_svg::written`]). Nothing is written when anything said is an error, as `stitch
//! plan` writes nothing then.

use std::fmt::Write as _;
use std::path::Path;

use stitchcraft_core::{Budget, Code, Diagnostic, Severity};
use stitchcraft_engine::design::Reading;
use stitchcraft_svg::written::{self, Options};

use super::plan::{self, Source};
use super::{Outcome, Status, asked_profile, distinct, render_diagnostics, write_files};
use crate::cli::ExpandArgs;
use crate::files::{self, Scratch, Unwritten};
use crate::trace;

/// What `stitch expand` writes, by the output's extension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Output {
    /// A VectorCraft document.
    VectorCraft,
    /// An SVG file with Ink/Stitch's attributes.
    Svg,
}

/// What `output` is, by its extension; the usage error when it is neither.
fn output_of(output: &Path) -> Result<Output, Outcome> {
    let extension = output.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
    if stitchcraft_vectorcraft::EXTENSIONS.iter().any(|known| extension.eq_ignore_ascii_case(known)) {
        Ok(Output::VectorCraft)
    } else if extension.eq_ignore_ascii_case("svg") {
        Ok(Output::Svg)
    } else {
        Err(Outcome::usage(format!("cannot tell what to write as `{}`: name it .vectorcraft or .svg", output.display())))
    }
}

/// Runs `stitch expand`.
pub fn run(args: &ExpandArgs) -> Outcome {
    let asked = match asked_profile(args.profile.as_deref()) {
        Ok(asked) => asked,
        Err(outcome) => return outcome,
    };
    let output = match output_of(&args.output) {
        Ok(output) => output,
        Err(outcome) => return outcome,
    };
    let picture = trace::is_picture(&args.design);
    if !picture && args.tracing.given() {
        return Outcome::usage("--vectorcraft-cli, --size, --colours and --traced are for a picture, a PNG or JPEG file");
    }
    if output == Output::VectorCraft && !picture && Source::of(&args.design) == Source::Svg {
        return Outcome::usage("a VectorCraft document is written from a VectorCraft document or a picture: name the output .svg");
    }
    let writes: Vec<&Path> = [Some(&args.output), args.tracing.traced.as_ref()].into_iter().flatten().map(|path| path.as_path()).collect();
    if let Err(outcome) = distinct(&[&args.design], &writes) {
        return outcome;
    }
    let bytes = match files::read_capped(&args.design) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", args.design.display()), status: Status::Io };
        }
    };
    let tracing_for = asked.unwrap_or(stitchcraft_plan::profiles::REFERENCE);
    let (source, document) = if picture {
        match traced(args, tracing_for, &bytes) {
            Ok(document) => (Source::VectorCraft, document),
            Err(outcome) => return outcome,
        }
    } else {
        (Source::of(&args.design), bytes)
    };
    let profile = plan::machine_for(asked, source, &document);
    let reading = Reading { auto: true, ..Reading::from(&args.reading) };
    let (design, mut said) = match plan::read_design(source, &document, (reading, &args.reading.rules())) {
        Ok(read) => read,
        Err(unreadable) => return Outcome::refuse(String::new(), &[unreadable]),
    };
    let budget = Budget::DEFAULT;
    let (expanded, digitized) = match stitchcraft_engine::auto::expand(&design, profile, &budget) {
        Ok(expanded) => expanded,
        Err(bug) => return Outcome::refuse(String::new(), &[bug]),
    };
    said.extend(digitized);
    let written = match output {
        Output::VectorCraft => stitchcraft_vectorcraft::expanded::written(&document, &design, &expanded, profile, &budget).map(|written| {
            said.extend(written.warnings);
            written.document
        }),
        Output::Svg => written::svg(expanded.elements(), &expanded.settings, &Options::EXPANDED)
            .map(String::into_bytes)
            .map_err(|why| Diagnostic::new(Code::InternalCheckFailed, format!("The expanded design can't be written as SVG: {why}."))),
    };
    let written = match written {
        Ok(written) => written,
        Err(said_now) => {
            said.push(said_now);
            return Outcome::refuse(String::new(), &said);
        }
    };
    if said.iter().any(|said| said.severity() == Severity::Error) {
        return Outcome::refuse(String::new(), &said);
    }
    let rendered = render_diagnostics(&said);
    let mut files: Vec<(&Path, &[u8])> = vec![(&args.output, &written)];
    if let (Some(traced), true) = (&args.tracing.traced, picture) {
        files.push((traced, &document));
    }
    if let Err(unwritten) = write_files(&files, &rendered) {
        return unwritten;
    }
    let mut out = String::new();
    let _ = writeln!(out, "{} · expanded into {} parts → {}", args.design.display(), expanded.elements().len(), args.output.display());
    Outcome { stdout: out, stderr: rendered, status: Status::Done }
}

/// The document VectorCraft's command line traces the picture `bytes` into, for `profile`, as `args` ask
/// ([`trace`]); or the outcome that says why it doesn't.
fn traced(args: &ExpandArgs, profile: &stitchcraft_plan::MachineProfile, bytes: &[u8]) -> Result<Vec<u8>, Outcome> {
    let tracing = &args.tracing;
    let name = args.design.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
    let fitting = if tracing.fewer_points { trace::Fitting::Fewest } else { trace::Fitting::Defaults };
    let steps = trace::steps(&name, bytes, profile, tracing.size, tracing.colours.unwrap_or(trace::COLOURS), fitting);
    let scratch = Scratch::beside(&args.output)
        .map_err(|error| Outcome::unwritten("", &Unwritten { path: args.output.clone(), error, written: Vec::new() }))?;
    trace::traced(tracing.vectorcraft_cli.as_deref(), &name, &steps, &scratch).map_err(|untraced| Outcome::refuse(String::new(), &[untraced]))
}
