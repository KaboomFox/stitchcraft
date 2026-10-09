//! Generators (pipeline stage 3): one module per stitch type, each turning a normalized shape and its
//! typed parameters into stitches.
//!
//! Every generator is pure: its stitches depend only on the shape, its parameters, its hints, its seed
//! and its budget. It charges the budget in every loop, and it reports what it changed or left out with a
//! coded diagnostic (`docs/src/design/engine-pipeline.md` › Generate). Each generator is a function, and
//! [`crate::generate`] sends each element to its own.

pub mod manual;
pub mod passes;
pub mod running;
pub mod satin;

use stitchcraft_core::{Code, Diagnostic, Point};

/// The stitches of one stroke.
#[derive(Clone, Debug, PartialEq)]
pub struct Stitched {
    /// The needle points of each piece of the stroke that is stitched, in drawing order, its repeats and
    /// bean stitch included. Each run starts where its piece starts, and ends where its last pass does.
    pub runs: Vec<Vec<Point>>,
    /// What was changed or left out. They name no element: the caller adds it.
    pub warnings: Vec<Diagnostic>,
}

/// Why a part of a stroke is not stitched.
#[derive(Clone, Copy, Debug)]
pub(crate) enum TooSmall {
    /// It is one point.
    Point,
    /// It is this long, shorter than the shortest stitch.
    Short(f64),
    /// It is this long, but lies all within the shortest stitch of its ends.
    Curled(f64),
}

/// `SC-W0401` for a part of a stroke that is not stitched, with the shortest stitch `min`.
pub(crate) fn too_small(why: TooSmall, min: f64) -> Diagnostic {
    let message = match why {
        TooSmall::Point => "A part of the stroke is a single point, so it is not stitched.".to_string(),
        TooSmall::Short(length) => {
            format!("A part of the stroke is {} mm long, shorter than the shortest stitch ({} mm), so it is not stitched.", mm(length), mm(min))
        }
        TooSmall::Curled(length) => format!(
            "A part of the stroke is {} mm long, but all of it lies within the shortest stitch ({} mm) of its ends, so it is not stitched.",
            mm(length),
            mm(min)
        ),
    };
    Diagnostic::new(Code::StrokeTooSmall, message)
}

/// `value` millimetres for a message: at most two decimals, without trailing zeros.
pub(crate) fn mm(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}
