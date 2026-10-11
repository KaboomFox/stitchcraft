//! Generators (pipeline stage 3): one module per stitch type, each turning a normalized shape and its
//! typed parameters into stitches.
//!
//! Every generator is pure: its stitches depend only on the shape, its parameters, its neighbours, its
//! seed and its budget. It charges the budget in every loop, and it reports what it changed or left out
//! with a coded diagnostic (`docs/src/design/engine-pipeline.md` › Generate). Each generator is a
//! function, and [`crate::generate`] sends each element to its own.
//!
//! Elements are generated in sewing order, and each sees its [`Neighbours`]: where the elements before it
//! left the needle, and what the next element offers to end near, its [`Approach`]
//! (`docs/src/design/adr/0014-generators-see-their-neighbours.md`). A satin column and a tatami fill
//! start and end by them; the other stitch types do not yet.

pub mod manual;
pub mod passes;
pub mod running;
pub mod satin;
pub mod tatami;

use stitchcraft_core::{Code, Diagnostic, Point};
use stitchcraft_params::{ChoiceOption, StitchType};

/// What the next element offers the one before it to end near (Ink/Stitch's "next stitch").
#[derive(Clone, Debug, PartialEq)]
pub enum Approach {
    /// It starts at this point: a stroke's first point, or a satin column's first rail's start when it
    /// does not start at its nearest point.
    Point(Point),
    /// It starts at the point of these polylines nearest the needle: a satin column's rails as they are
    /// sewn, swapped and turned, or a fill's rings, part by part and each outline before its holes, in the
    /// order the element before measures them.
    Shape(Vec<Vec<Point>>),
}

/// An element's neighbours in sewing order (`REQ-GEN-003`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Neighbours {
    /// The last needle point of the elements before it that sew any, whatever their thread.
    pub needle: Option<Point>,
    /// What the next element in the design offers to end near.
    pub next: Option<Approach>,
}

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

/// A stitch type as a method a settings window offers: its id and its name.
pub(crate) const fn method(stitch_type: StitchType) -> ChoiceOption {
    ChoiceOption { id: stitch_type.id(), label: stitch_type.name() }
}

/// `value` millimetres for a message: at most two decimals, without trailing zeros.
pub(crate) fn mm(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::Family;

    use super::*;

    #[test]
    fn each_method_leads_back_to_its_stitch_type() {
        // `method` builds the lists at compile time; here it runs, and each option is its type's id and name.
        for (family, methods) in
            [(Family::Stroke, crate::generate::STROKE_METHODS), (Family::Satin, satin::SATIN_METHODS), (Family::Fill, crate::generate::FILL_METHODS)]
        {
            for option in methods {
                assert_eq!(StitchType::from_id(family, option.id).map(method), Some(*option), "{option:?}");
            }
        }
    }
}
