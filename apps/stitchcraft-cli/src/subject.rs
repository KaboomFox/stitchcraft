//! A picture's subject cut out before VectorCraft's command line traces it: `stitch plan --subject` and
//! `stitch cutout` (`REQ-CLI-014` to `REQ-CLI-018`; ADR-0019, `docs/src/design/image-to-embroidery.md` ›
//! Subject cut-out).
//!
//! **The command line's part.** `stitchcraft-cutout` cuts the subject out. Here the picture's bytes are
//! decoded, the marks checked against its size, which are usage errors when they don't fit it, the models
//! fetched and checked against their pins, and the cut-out made. What the cut-out kept is said with the rest
//! (`SC-I1206`), and a failure stops the command with its coded diagnostic, with nothing written but the
//! report.
//!
//! **What is recorded.** A report and a bug-report bundle of a picture cut out keep the models' SHA-256
//! sums, the box and where it came from, and the points ([`Record`]): what reproduces the cut-out where the
//! same models run, though the plan is reproducible from the document VectorCraft traced alone, which the
//! bundle holds (`REQ-CLI-018`).
//!
//! **The models come from a [`Provider`].** `stitch` fetches the pinned ones ([`fetched`]), and the tests give
//! stand-ins, tiny models with the real ones' inputs and outputs, so that every step here runs without them.
//!
//! **A build may leave the cut-out out**, without `stitch`'s `cutout` feature, as the WASI build that makes
//! the golden files does. Its grammar is the same, and `--subject` and `stitch cutout` stop at once with
//! `SC-E1207`, which says so ([`left_out`]).

use serde::{Deserialize, Serialize};
use stitchcraft_core::{Code, Diagnostic, Fix};
#[cfg(feature = "cutout")]
use stitchcraft_cutout::pins::{self, Pins};
#[cfg(feature = "cutout")]
use stitchcraft_cutout::run::Tract;
use stitchcraft_cutout::{Marks, Work};

/// What a [`Provider`] gives: the models a cut-out runs, and their SHA-256 sums, by file.
#[cfg(feature = "cutout")]
pub type Provided = (Box<dyn stitchcraft_cutout::Models>, Vec<(String, String)>);
/// What a [`Provider`] gives: a build without the cut-out has no models to give.
#[cfg(not(feature = "cutout"))]
pub type Provided = std::convert::Infallible;
/// Where the models a cut-out runs come from: [`fetched`], or a test's stand-ins.
pub type Provider = dyn Fn(&mut Work) -> Result<Provided, Diagnostic>;

/// The pinned models, fetched into their folder when they aren't there, and checked: `SC-E1201` when they
/// can't be, `SC-E1204` and `SC-E1205` when tract refuses one.
#[cfg(feature = "cutout")]
pub fn fetched(work: &mut Work) -> Result<Provided, Diagnostic> {
    let pins = Pins::pinned()?;
    let ready = pins::ready(&pins, pins::folder().as_deref())?;
    let sums = pins.all().iter().map(|pin| (pin.file.clone(), pin.sha256.clone())).collect();
    Ok((Box::new(Tract::load(&ready, work)?), sums))
}

/// The pinned models, which a build without the cut-out doesn't run: `SC-E1207`.
#[cfg(not(feature = "cutout"))]
pub fn fetched(_: &mut Work) -> Result<Provided, Diagnostic> {
    Err(left_out())
}

/// `SC-E1207`: this `stitch` was built without the cut-out.
pub fn left_out() -> Diagnostic {
    Diagnostic::new(
        Code::CutoutLeftOut,
        "This stitch was built without the subject cut-out, its `cutout` feature, so it can't cut a picture's subject out.",
    )
    .with_fix(Fix::Hint("Plan the picture without --subject, or use a stitch built with its default features.".to_string()))
}

/// Why a cut-out wasn't made.
#[derive(Debug)]
pub enum Stopped {
    /// A mark doesn't fit the picture: a usage error, which says why.
    Usage(String),
    /// A coded diagnostic: the picture can't be read, the models aren't ready or can't run, or no subject was
    /// found.
    Said(Diagnostic),
}

impl From<Diagnostic> for Stopped {
    fn from(said: Diagnostic) -> Stopped {
        Stopped::Said(said)
    }
}

/// A subject cut out: what `stitch` writes and says of it, and its record.
#[derive(Debug)]
pub struct Subject {
    /// The cut-out, a PNG file.
    pub png: Vec<u8>,
    /// Its width and height, cropped, in pixels.
    pub cropped: [u32; 2],
    /// The picture's width and height, in pixels.
    pub size: [u32; 2],
    /// What it kept (`SC-I1206`).
    pub said: Diagnostic,
    /// What a report and a bundle keep of it.
    pub record: Record,
}

/// The subject of the picture `bytes` cut out with `marks`, the models given by `models`.
#[cfg(feature = "cutout")]
pub fn cut(bytes: &[u8], marks: &Marks, models: &Provider) -> Result<Subject, Stopped> {
    let mut work = Work::new(stitchcraft_core::Budget::DEFAULT);
    let picture = stitchcraft_cutout::Picture::decode(bytes, &mut work)?;
    marks.check(picture.width(), picture.height()).map_err(Stopped::Usage)?;
    let (mut models, sums) = models(&mut work)?;
    let cut = stitchcraft_cutout::cut_out(&picture, marks, models.as_mut(), &mut work)?;
    let record = Record::of(&cut, marks, sums);
    Ok(Subject { cropped: cut.cropped(), size: cut.size, said: cut.said(), record, png: cut.png })
}

/// No subject cut out: this build leaves the cut-out out (`SC-E1207`).
#[cfg(not(feature = "cutout"))]
pub fn cut(_: &[u8], _: &Marks, _: &Provider) -> Result<Subject, Stopped> {
    Err(Stopped::Said(left_out()))
}

/// What a report and a bug-report bundle keep of a cut-out (`REQ-CLI-018`).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    /// Each model's file and its SHA-256, in the order they run.
    pub models: Vec<ModelSum>,
    /// The box the subject was cut out of, in the picture's pixels: left, top, right and bottom edges.
    pub subject_box: [f64; 4],
    /// Who gave the box: `subject-box`, or `u2netp`.
    pub box_from: String,
    /// The points of the subject, each across and down.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subject_points: Vec<[f64; 2]>,
    /// The points of what isn't the subject.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_subject: Vec<[f64; 2]>,
    /// The cut-out's crop of the picture, in its whole pixels: left, top, right and bottom edges.
    pub crop: [u32; 4],
}

/// A model's file and its SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelSum {
    /// Its file's name.
    pub file: String,
    /// Its SHA-256, in lower-case hexadecimal.
    pub sha256: String,
}

impl Record {
    /// The record of `cut`, made with `marks` by the models whose sums are `sums`.
    #[cfg(feature = "cutout")]
    fn of(cut: &stitchcraft_cutout::CutOut, marks: &Marks, sums: Vec<(String, String)>) -> Record {
        let point = |point: &stitchcraft_cutout::Point| [point.x, point.y];
        let b = cut.subject;
        Record {
            models: sums.into_iter().map(|(file, sha256)| ModelSum { file, sha256 }).collect(),
            subject_box: [b.x0, b.y0, b.x1, b.y1],
            box_from: match cut.from {
                stitchcraft_cutout::BoxFrom::Given => "subject-box",
                stitchcraft_cutout::BoxFrom::Found => "u2netp",
            }
            .to_string(),
            subject_points: marks.subject.iter().map(point).collect(),
            not_subject: marks.not_subject.iter().map(point).collect(),
            crop: cut.crop,
        }
    }
}

// 2 attributes, as clippy reads `cfg(test)` alone as test code.
#[cfg(test)]
#[cfg(feature = "cutout")]
mod tests;

#[cfg(test)]
mod left_out {
    use super::*;

    #[test]
    fn diag_sc_e1207_a_build_without_the_cut_out_says_so() {
        let said = left_out();
        assert_eq!(
            said.to_string(),
            "error SC-E1207: This stitch was built without the subject cut-out, its `cutout` feature, so it can't cut a picture's subject out."
        );
        assert_eq!(
            said.fix.map(|fix| fix.describe()).as_deref(),
            Some("Plan the picture without --subject, or use a stitch built with its default features.")
        );
        // Built without it, `stitch` cuts nothing out, and says so at once.
        #[cfg(not(feature = "cutout"))]
        {
            assert!(matches!(cut(b"", &Marks::default(), &fetched), Err(Stopped::Said(said)) if said == left_out()));
            assert_eq!(fetched(&mut Work::new(stitchcraft_core::Budget::DEFAULT)).err(), Some(left_out()));
        }
    }
}
