//! `stitch cutout PICTURE -o FILE`: a picture's subject cut out, as `stitch plan --subject` cuts it out
//! before VectorCraft traces it, written alone as a PNG file (`REQ-CLI-014`, ADR-0019 decision 2).
//!
//! **Why.** A VectorCraft filter can't read or replace a placed picture, so in VectorCraft's own flow the
//! user places the cut-out instead of the photo and traces it with Image Trace, as the tutorial traces any
//! picture. The marks are those `stitch plan` takes, and what was kept is said as `stitch plan` says it
//! (`SC-I1206`), so a cut-out corrected here is the one `stitch plan` makes with the same marks.
//!
//! **What fails.** A mark that doesn't fit the picture is a usage error. A picture that can't be read,
//! models that aren't ready or can't run, and no subject found are their coded diagnostics, and nothing is
//! written.

use std::fmt::Write as _;
use std::path::Path;

use super::{Outcome, Status, distinct, render_diagnostics, write_files};
use crate::cli::CutoutArgs;
use crate::files;
use crate::subject::{self, Provider, Stopped, Subject};

/// Runs `stitch cutout` with the pinned models.
pub fn run(args: &CutoutArgs) -> Outcome {
    run_with(args, &subject::fetched)
}

/// Runs `stitch cutout`, its models given by `models`.
pub fn run_with(args: &CutoutArgs, models: &Provider) -> Outcome {
    if let Err(outcome) = distinct(&[&args.picture], &[&args.output]) {
        return outcome;
    }
    let bytes = match files::read_capped(&args.picture) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", args.picture.display()), status: Status::Io };
        }
    };
    let made = match subject::cut(&bytes, &args.marks.marks(), models) {
        Ok(made) => made,
        Err(Stopped::Usage(why)) => return Outcome::usage(why),
        Err(Stopped::Said(said)) => return Outcome::refuse(String::new(), &[said]),
    };
    let said = render_diagnostics(std::slice::from_ref(&made.said));
    if let Err(unwritten) = write_files(&[(args.output.as_path(), made.png.as_slice())], &said) {
        return unwritten;
    }
    Outcome { stdout: summary(&args.picture, &args.output, &made), stderr: said, status: Status::Done }
}

/// What `stitch cutout` prints of the subject `made`, cut out of `picture` and written to `output`.
fn summary(picture: &Path, output: &Path, made: &Subject) -> String {
    let mut out = String::new();
    let ([width, height], [left, top, right, bottom], [across, down]) = (made.cropped, made.record.crop, made.size);
    let _ = writeln!(out, "{}", picture.display());
    let _ = writeln!(out, "  cutout    {} ({width} × {height} pixels)", output.display());
    let _ = writeln!(out, "  crop      {left},{top},{right},{bottom} of the picture's {across} × {down} pixels");
    out
}
