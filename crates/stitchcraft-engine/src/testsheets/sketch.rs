//! A small drawing helper for test sheets: running-stitch lines, moves between parts, commands.
//!
//! Test sheets are drawn in code, not authored as SVG, so they exist before the SVG reader does and their
//! geometry is exact. Every line is sewn with stitches of at most [`STITCH_LEN`], spaced evenly so a line
//! of whole millimetres has equal stitches; every move between parts is an explicit jump followed by the
//! first stitch at its end, optionally preceded by a trim. That is all M1's sheets need; real stitch
//! types arrive with the engine in M3.

use stitchcraft_core::{Budget, ElementId, ElementIdError, Exhausted, Meter, Point, UnitError};
use stitchcraft_plan::{ElementRef, PlanBuilder, PlanError, Provenance, Role, StitchPlan, Thread};

/// The longest running stitch on a test sheet: a common running-stitch length, well inside every profile.
pub const STITCH_LEN: f64 = 2.5;

/// Why a test sheet could not be drawn (a bug in the sheet: built-in sheets are tested).
#[derive(Clone, Debug, PartialEq, thiserror::Error)]
pub enum SheetError {
    /// A coordinate was not finite.
    #[error(transparent)]
    Unit(#[from] UnitError),
    /// An element id was invalid.
    #[error(transparent)]
    ElementId(#[from] ElementIdError),
    /// The plan could not be built.
    #[error(transparent)]
    Plan(#[from] PlanError),
    /// A Brother PEC palette index that does not exist.
    #[error("no Brother PEC thread has index {0}")]
    UnknownThread(u8),
    /// The sheet needs more stitches than one design may have: its geometry is wrong.
    #[error(transparent)]
    Budget(#[from] Exhausted),
    /// The engine had something to say about a sheet drawn as a design, so it is not the sheet its checks
    /// describe (`designed`); the diagnostics, as people read them.
    #[error("the engine says: {0}")]
    Said(String),
}

/// Draws a test sheet into a plan.
pub(crate) struct Sketch {
    builder: PlanBuilder,
    element: Option<ElementRef>,
    sheet: &'static str,
    /// Every stitch is charged, so wrong geometry fails fast instead of exhausting memory (mutation
    /// testing found that a sign error made lines grow without bound).
    meter: Meter,
}

impl Sketch {
    /// A sheet whose element ids start with `sheet` (`ts01:cross`), sewn first with `thread`.
    pub fn new(sheet: &'static str, thread: Thread) -> Self {
        Sketch { builder: PlanBuilder::new(thread), element: None, sheet, meter: Budget::DEFAULT.meter() }
    }

    /// Starts the part named `name`; stitches until the next part belong to it.
    pub fn part(&mut self, name: &str) -> Result<(), SheetError> {
        let id = ElementId::new(format!("{}:{name}", self.sheet))?;
        self.element = Some(self.builder.element(id)?);
        Ok(())
    }

    /// Moves to `at` without sewing — after a trim when `trim` — and puts the needle down there.
    pub fn move_to(&mut self, at: (f64, f64), trim: bool) -> Result<(), SheetError> {
        let at = point(at)?;
        self.meter.charge_stitches(1)?;
        if trim {
            self.builder.trim(self.element);
        }
        self.builder.jump(at, self.provenance(Role::Travel));
        self.builder.stitch(at, self.provenance(Role::Top));
        Ok(())
    }

    /// Sews a straight line from the needle to `to`, in equal stitches of at most [`STITCH_LEN`]. A line
    /// to where the needle already is sews nothing.
    pub fn line_to(&mut self, to: (f64, f64)) -> Result<(), SheetError> {
        let from = self.builder.needle();
        let (dx, dy) = (to.0 - from.x(), to.1 - from.y());
        let length = from.distance(point(to)?);
        if length == 0.0 {
            return Ok(());
        }
        let steps = (length / STITCH_LEN).ceil().max(1.0);
        // A whole number of at least 1; `as` saturates, and the budget refuses anything large.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let count = steps as u32;
        self.meter.charge_stitches(count)?;
        for i in 1..=count {
            let t = f64::from(i) / steps;
            self.builder.stitch(point((from.x() + dx * t, from.y() + dy * t))?, self.provenance(Role::Top));
        }
        Ok(())
    }

    /// Sews through each point in turn.
    pub fn polyline(&mut self, points: &[(f64, f64)]) -> Result<(), SheetError> {
        points.iter().try_for_each(|p| self.line_to(*p))
    }

    /// Cuts the thread where the needle is.
    pub fn trim(&mut self) {
        self.builder.trim(self.element);
    }

    /// Pauses the machine where the needle is.
    pub fn stop(&mut self) {
        self.builder.stop(self.element);
    }

    /// Continues with `thread` (the machine stops for a thread change).
    pub fn change_thread(&mut self, thread: Thread) {
        self.builder.change_thread(thread);
    }

    /// The finished plan.
    pub fn finish(self) -> StitchPlan {
        self.builder.finish()
    }

    fn provenance(&self, role: Role) -> Provenance {
        Provenance { element: self.element, role }
    }
}

/// A point from literal coordinates.
fn point((x, y): (f64, f64)) -> Result<Point, UnitError> {
    Point::new(x, y)
}

/// An upright letter "F" whose stem stands on `base` (bottom of the stem) and is `height` tall. The F is
/// asymmetric in both axes, so a mirrored or rotated sew-out is obvious.
pub(crate) fn letter_f(sketch: &mut Sketch, base: (f64, f64), height: f64, trim: bool) -> Result<(), SheetError> {
    let (x, y) = base;
    // The middle bar sits halfway, so the stem's second pass reuses the first pass's needle holes.
    let (top, middle) = (y - height, y - height * 0.5);
    sketch.move_to(base, trim)?;
    sketch.polyline(&[(x, top), (x + height * 0.6, top), (x, top), (x, middle), (x + height * 0.4, middle), (x, middle)])
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::{Rgb, StitchKind};

    use super::*;

    fn sketch() -> Sketch {
        Sketch::new("test", Thread::new(Rgb::new(0, 0, 0)))
    }

    #[test]
    fn lines_are_sewn_in_equal_stitches_of_at_most_stitch_len() {
        let mut s = sketch();
        s.move_to((0.0, 0.0), false).unwrap();
        s.line_to((10.0, 0.0)).unwrap();
        s.line_to((10.0, 0.0)).unwrap();
        let sewn: Vec<f64> = s.finish().stitches().filter(|st| st.kind == StitchKind::Normal).map(|st| st.at.x()).collect();
        assert_eq!(sewn, [0.0, 2.5, 5.0, 7.5, 10.0], "a line to where the needle is sews nothing");
    }

    #[test]
    fn runaway_geometry_fails_fast_instead_of_exhausting_memory() {
        let mut s = sketch();
        s.move_to((0.0, 0.0), false).unwrap();
        // 6 km of line would be 2.4 million stitches, more than one design may have: refused before
        // a single one is made.
        assert!(matches!(s.line_to((6_000_000.0, 0.0)), Err(SheetError::Budget(Exhausted::Stitches))));
        assert_eq!(s.finish().stats().stitches, 1);
    }
}
