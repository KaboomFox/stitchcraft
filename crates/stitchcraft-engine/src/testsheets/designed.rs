//! Test sheets drawn as designs and planned by the engine (from MC-2): their stitches come from the same
//! generators, locks and plan assembly as any design's, so sewing them tests those — where the M1 sheets,
//! drawn stitch by stitch (`sketch`), test the machine and the file formats.
//!
//! A sheet is a list of elements — polylines and circles, each with Ink/Stitch parameters — planned like
//! any design: for the reference machine's profile, the machine the checkpoints run on, with the design's
//! origin at the middle of its stitches. A sheet the engine has anything to say about is not the sheet
//! its checks describe, so drawing one fails on any diagnostic.

use std::f64::consts::SQRT_2;

use stitchcraft_core::{Budget, ElementId, Point, UnitError};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{StitchPlan, Thread};

use super::sketch::SheetError;
use crate::design::{Design, DesignSettings, Element, Path, Segment, Shape, Subpath};

/// A test sheet as a design: its elements, in sewing order.
pub(super) struct Drawing {
    sheet: &'static str,
    elements: Vec<Element>,
}

impl Drawing {
    /// A sheet whose element ids start with `sheet` (`ts03:circle-0.1`).
    pub fn new(sheet: &'static str) -> Self {
        Drawing { sheet, elements: Vec::new() }
    }

    /// A stroke through `points`, named `name`, sewn with `thread` and the Ink/Stitch `params`.
    pub fn polyline(&mut self, name: &str, points: &[(f64, f64)], thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let Some((&first, rest)) = points.split_first() else { return Ok(()) };
        let segments = rest.iter().map(|&p| point(p).map(Segment::Line)).collect::<Result<_, _>>()?;
        self.stroke(name, Subpath { start: point(first)?, segments, closed: false }, thread, params)
    }

    /// A circle of `radius` round `centre`: four cubic curves from its right-hand point, back to it.
    pub fn circle(&mut self, name: &str, centre: (f64, f64), radius: f64, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        // The control points of a quarter circle: 4/3 (√2 − 1) of the radius from its ends.
        let (x, y, r, k) = (centre.0, centre.1, radius, 4.0 / 3.0 * (SQRT_2 - 1.0) * radius);
        let quarter = |c1, c2, end| Ok::<_, UnitError>(Segment::Cubic(point(c1)?, point(c2)?, point(end)?));
        let segments = vec![
            quarter((x + r, y + k), (x + k, y + r), (x, y + r))?,
            quarter((x - k, y + r), (x - r, y + k), (x - r, y))?,
            quarter((x - r, y - k), (x - k, y - r), (x, y - r))?,
            quarter((x + k, y - r), (x + r, y - k), (x + r, y))?,
        ];
        self.stroke(name, Subpath { start: point((x + r, y))?, segments, closed: true }, thread, params)
    }

    fn stroke(&mut self, name: &str, subpath: Subpath, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        self.elements.push(Element {
            id: ElementId::new(format!("{}:{name}", self.sheet))?,
            name: None,
            shape: Shape::Stroke(Path { subpaths: vec![subpath] }),
            thread: thread.clone(),
            params: params.iter().copied().collect::<ParamSet>(),
        });
        Ok(())
    }

    /// The sheet planned for the reference machine, or what the engine said about it.
    pub fn plan(self) -> Result<StitchPlan, SheetError> {
        let design = Design::new(self.elements, DesignSettings::default()).map_err(|d| SheetError::Said(d.to_string()))?;
        let outcome = crate::plan(&design, REFERENCE, &Budget::DEFAULT);
        match outcome.plan {
            Some(plan) if outcome.diagnostics.is_empty() => Ok(plan),
            _ => Err(SheetError::Said(outcome.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join(" "))),
        }
    }
}

/// A point from literal coordinates.
fn point((x, y): (f64, f64)) -> Result<Point, UnitError> {
    Point::new(x, y)
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::{Rgb, Thread};

    use super::*;

    #[test]
    fn a_sheet_the_engine_says_anything_about_does_not_draw() {
        let thread = Thread::new(Rgb::new(0, 0, 0));
        let mut d = Drawing::new("test");
        d.polyline("fine", &[(0.0, 0.0), (10.0, 0.0)], &thread, &[]).unwrap();
        d.polyline("unreadable", &[(0.0, 5.0), (10.0, 5.0)], &thread, &[("running_stitch_length_mm", "long")]).unwrap();
        let Err(SheetError::Said(said)) = d.plan() else { panic!("drawn despite what the engine said") };
        assert!(said.starts_with("error SC-E0101: "), "{said}");
        assert!(Drawing::new("empty").plan().is_err(), "nothing to stitch");
        // Two elements with one id: the design itself is refused.
        let mut twice = Drawing::new("twice");
        twice.polyline("a", &[(0.0, 0.0), (10.0, 0.0)], &thread, &[]).unwrap();
        twice.polyline("a", &[(0.0, 5.0), (10.0, 5.0)], &thread, &[]).unwrap();
        let Err(SheetError::Said(said)) = twice.plan() else { panic!("drawn with one id twice") };
        assert!(said.contains("Two elements of the design have the id `twice:a`."), "{said}");
    }
}
