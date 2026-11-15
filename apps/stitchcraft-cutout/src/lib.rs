//! The subject cut-out of `stitch plan --subject` and `stitch cutout`: a photo's subject cut out before
//! VectorCraft's command line traces it (ADR-0019, `docs/src/design/image-to-embroidery.md` › Subject
//! cut-out).
//!
//! **Why it exists.** A photo's background traces into shapes of its own, turf, a fence or tyres behind a
//! dog, and the background rule leaves out one shape at most. Image Trace leaves out the pixels whose alpha
//! is under 128, so a picture whose background is transparent traces as its subject alone. This crate makes
//! that picture: the photo, cropped to its subject, with everything else transparent.
//!
//! **How.** MobileSAM cuts the subject out of a box. Its encoder takes the picture once, and its decoder takes
//! the box and any points the user marks ([`Marks`]). U²-Net-p finds the box when the user gives none, round
//! the piece of its map above a half with the most of the map in it ([`salient`]). tract runs the 3 models
//! from their ONNX files on one thread ([`run`]), which [`pins`] fetches and checks.
//!
//! **The steps apart from the runs.** [`cut_out`] does every step but running the models, which it asks of
//! a [`Models`]: [`run::Tract`] runs them, and tests give it the outputs the models gave for the test photos,
//! saved as fixtures (ADR-0019, decision 8). So the box, the prompt, the alpha, the crop and the file are
//! tested without the models, and a job of its own runs the models and checks what they give.
//!
//! **Why an app's crate.** The library crates of layers L0 to L3 don't read a picture or run a model, and
//! only `stitch` depends on this one (`cargo xtask layers`). The models' results may differ in their last
//! bits from one processor to another, so a plan is reproducible from the document VectorCraft traced, not
//! from the photo (ADR-0019, decision 7).
//!
//! **Never crashing.** Every loop over the picture charges the budget, and every failure is a coded
//! diagnostic: a picture the cut-out can't read (`SC-E1203`), models it can't fetch or that aren't the
//! ones pinned (`SC-E1201`), a model tract can't run (`SC-E1204`) or that has a random operator
//! (`SC-E1205`), and no subject found (`SC-E1202`). A cut-out made says what it kept (`SC-I1206`).
//!
//! **The `models` feature**, on by default, holds the cut-out itself: `image` and tract, and the steps that
//! use them. Without it the crate holds the marks, the pins and the budget alone, which the command line's
//! grammar and its bundles read, so that `stitch` can be built without the cut-out (its `cutout` feature).
#![forbid(unsafe_code)]

#[cfg(feature = "models")]
pub mod cut;
pub mod marks;
#[cfg(feature = "models")]
pub mod picture;
pub mod pins;
#[cfg(feature = "models")]
pub mod run;
#[cfg(feature = "models")]
pub mod salient;
#[cfg(feature = "models")]
pub mod segment;
pub mod work;

#[cfg(feature = "models")]
use stitchcraft_core::Diagnostic;

#[cfg(feature = "models")]
pub use cut::CutOut;
pub use marks::{Marks, PixelBox, Point};
#[cfg(feature = "models")]
pub use picture::Picture;
#[cfg(feature = "models")]
pub use segment::{Embedding, LowRes, Prompt};
pub use work::Work;

/// What runs the 3 models: [`run::Tract`], or a stand-in that gives the outputs saved for a test photo.
#[cfg(feature = "models")]
pub trait Models {
    /// U²-Net-p's map of `input` ([`salient::input`]): 320 × 320 values, row by row.
    fn salient(&mut self, input: &[f32]) -> Result<Vec<f32>, Diagnostic>;
    /// MobileSAM's encoder's embedding of `input` ([`segment::encoder_input`]).
    fn embed(&mut self, input: &[f32]) -> Result<Embedding, Diagnostic>;
    /// MobileSAM's decoder's map for `prompt` on `embedding`, of a picture `width` × `height` px.
    fn decode(&mut self, embedding: &Embedding, prompt: &Prompt, width: u32, height: u32) -> Result<LowRes, Diagnostic>;
}

/// Where the subject's box came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxFrom {
    /// `--subject-box` gave it.
    Given,
    /// U²-Net-p found it.
    Found,
}

/// The subject of `picture` cut out with `marks`, the models run by `models`, every loop charging `work`:
/// the box, given or else found by U²-Net-p, MobileSAM's map for the box and the points, and the picture
/// with that alpha, cropped round what it keeps. `SC-E1202` when U²-Net-p finds no subject or MobileSAM
/// keeps no pixel.
#[cfg(feature = "models")]
pub fn cut_out(picture: &Picture, marks: &Marks, models: &mut dyn Models, work: &mut Work) -> Result<CutOut, Diagnostic> {
    let (subject, from) = match marks.subject_box {
        Some(given) => (given, BoxFrom::Given),
        None => {
            let input = salient::input(picture, work)?;
            let map = models.salient(&input)?;
            let found = salient::subject_box(&map, picture.width(), picture.height(), work)?;
            (found.ok_or_else(|| salient::not_found(picture))?, BoxFrom::Found)
        }
    };
    let frame = segment::Frame::of(picture.width(), picture.height());
    let input = segment::encoder_input(picture, &frame, work)?;
    let embedding = models.embed(&input)?;
    let prompt = Prompt::new(&subject, marks, &frame);
    let low = models.decode(&embedding, &prompt, picture.width(), picture.height())?;
    let alpha = segment::alpha(&low, &frame, work)?;
    CutOut::new(picture, &alpha, subject, from, work)
}
