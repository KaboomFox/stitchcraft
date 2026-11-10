//! Generators (pipeline stage 3): one module per stitch type, each turning a normalized shape and its
//! typed parameters into stitches.
//!
//! Every generator is pure: its stitches depend only on the shape, its parameters, its neighbours, its
//! seed and its budget. It charges the budget in every loop, and it reports what it changed or left out
//! with a coded diagnostic (`docs/src/design/engine-pipeline.md` › Generate). Each generator is a
//! function, and [`crate::generate`] sends each element to its own.
//!
//! The stitch budget bounds what a generator holds (`REQ-GEN-004`): it spends a stitch for each needle
//! point as it makes it (`made`), checks that the points it lays out before sewing them fit in the
//! stitches left ([`Meter::room`]), and settles at the needle points it gives (`settled`). So a few
//! settings that multiply needle points, such as the smallest spacing with repeats and bean stitch, stop
//! at the design's limit instead of filling memory first.
//!
//! Elements are generated in sewing order, and each sees its [`Neighbours`]: where the elements before it
//! left the needle, and what the next element offers to end near, its [`Approach`]
//! (`docs/src/design/adr/0014-generators-see-their-neighbours.md`). A satin column and the fills start
//! and end by them, but for a contour fill's end, which lies where its rings end; a stroke starts and ends
//! where it is drawn. An element's start and end positions take their place for those that use them, as
//! the generate step passes them on (`REQ-GEN-001`). The fills and the satin column end near the next
//! stitch, found as Ink/Stitch's geometry library finds it, by the rules of the `next` module.
//!
//! A generator sews its runs in its element's thread, unless it gives them threads of their own
//! ([`Stitched::colours`], `REQ-ASM-006`), as a tartan fill sews each colour of its sett and a linear gradient
//! fill each colour of its gradient, both as Ink/Stitch's do.
//! Plan assembly changes thread inside the element where they change, and trims after each colour block
//! when the element's `trim_after` is on (`REQ-ASM-007`).
//!
//! An element in a run of underlays first is sewn as 2 [`Layer`]s, its underlay and its top, each with
//! neighbours of its own (`crate::underlays_first`, ADR-0018). A satin column and the fills with an
//! underlay sew each layer from one reading of the element: the satin column's `Column`, and a fill's
//! settings and area, its underlay over every part (`tatami::sew::underlay_sewn`) and its top over every
//! part without it.

pub mod circular;
pub mod contour;
pub mod cross;
pub mod gradient;
pub mod guided;
pub mod manual;
pub mod meander;
mod next;
pub mod passes;
pub mod pull;
pub mod ripple;
pub mod running;
pub mod satin;
pub(crate) mod smoothing;
pub mod tartan;
pub mod tatami;
pub mod zigzag;

pub(crate) use next::{Own, covers};
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};
use stitchcraft_params::{ChoiceOption, StitchType};
use stitchcraft_plan::Thread;

use crate::normalize::region::Polygon;

/// What the next element offers the one before it to end near. The element before ends near its next
/// stitch, the point of its own shape nearest this, as Ink/Stitch finds it (the `next` module).
#[derive(Clone, Debug, PartialEq)]
pub enum Approach {
    /// It starts at this point: a stroke's first point, or a satin column's first rail's start when it
    /// does not start at its nearest point.
    Point(Point),
    /// It starts at the point of these polylines nearest the needle: a satin column's rails as they are
    /// sewn, swapped and turned.
    Shape(Vec<Vec<Point>>),
    /// It starts at the point of this area nearest the needle: a tatami fill's parts.
    Area(Vec<Polygon>),
}

/// Which of an element's stitches are sewn: all of them, or one of the 2 layers an element of a run of
/// underlays first is sewn as (`REQ-ASM-009`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layer {
    /// Its underlay and its top stitches, as every element outside a run is sewn.
    #[default]
    Whole,
    /// Its underlay alone, which its run sews before any top stitch.
    Underlay,
    /// Its top stitches alone, which its run sews after every underlay.
    Top,
}

/// An element's neighbours in sewing order (`REQ-GEN-003`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Neighbours {
    /// The last needle point of the elements before it that sew any, whatever their thread.
    pub needle: Option<Point>,
    /// What the next element in the design offers to end near.
    pub next: Option<Approach>,
}

/// The stitches of one element, as its generator sews them.
#[derive(Clone, Debug, PartialEq)]
pub struct Stitched {
    /// The needle points of each piece of the element that is stitched, in sewing order, a stroke's
    /// repeats and bean stitch included. Each run starts where its piece starts, and ends where its last
    /// pass does.
    pub runs: Vec<Vec<Point>>,
    /// The colour of each run, by its place: empty when every run is sewn in the element's thread, as every
    /// stitch type sewn so far sews them (`REQ-ASM-006`). A run without a colour is sewn in the element's
    /// thread and ends no colour block.
    pub colours: Vec<Colour>,
    /// What was changed or left out. They name no element: the caller adds it.
    pub warnings: Vec<Diagnostic>,
}

/// The thread a run of an element sewn in several threads is sewn in, and whether it ends one of the
/// element's colour blocks (`REQ-ASM-006`, `REQ-ASM-007`).
///
/// A colour block is what the generator sews in one thread before it moves on, as Ink/Stitch's tartan and
/// gradient fills sew each colour of each part as one group (read at `d59c9ab`). Assembly trims after each
/// one when the element's `trim_after` is on, as Ink/Stitch's plan trims after each, and not after the
/// runs that end none, such as a tartan's underlay.
#[derive(Clone, Debug, PartialEq)]
pub struct Colour {
    /// The run's thread; `None` for its element's.
    pub thread: Option<Thread>,
    /// Whether the run ends a colour block.
    pub ends_block: bool,
}

/// Spends a stitch of `meter` for each of `count` needle points a generator makes. A count past `u32` is
/// past any budget.
pub(crate) fn made(meter: &mut Meter, count: usize) -> Result<(), Exhausted> {
    meter.charge_stitches(u32::try_from(count).unwrap_or(u32::MAX))
}

/// What `generator` sews, the stitches it spent settled at the needle points it gives
/// ([`Meter::settle_stitches`]): it spent one for each needle point as it made it, and dropped or copied
/// some of them on the way.
pub(crate) fn settled(meter: &mut Meter, generator: impl FnOnce(&mut Meter) -> Result<Stitched, Exhausted>) -> Result<Stitched, Exhausted> {
    let left = meter.stitches_left();
    let stitched = generator(meter)?;
    meter.settle_stitches(left, stitched.runs.iter().map(Vec::len).sum())?;
    Ok(stitched)
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
    ChoiceOption::new(stitch_type.id(), stitch_type.name())
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
