//! Test sheets drawn as designs and planned by the engine (from MC-2): their stitches come from the same
//! generators, locks and plan assembly as any design's, so sewing them tests those — where the M1 sheets,
//! drawn stitch by stitch (`sketch`), test the machine and the file formats.
//!
//! A sheet is a list of elements — polylines, circles, satin columns and fills, each with Ink/Stitch
//! parameters — planned like any design: for the reference machine's profile, the machine the checkpoints
//! run on, with the design's origin at the middle of its stitches. A sheet the engine says anything
//! unexpected about is not the sheet its checks describe, so drawing one fails on any diagnostic but those
//! the sheet asks for, as a fill in parts asks for the warning that it is.

use std::f64::consts::SQRT_2;

use stitchcraft_core::{Budget, Code, ElementId, Point, UnitError};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{StitchPlan, Thread};

use super::sketch::SheetError;
use crate::design::{Design, DesignSettings, Element, FillRule, Path, Segment, Shape, Subpath};

/// The settings that sew a sheet's satin column as drawn, from its start to its end whatever is around it,
/// and trim after it, so the columns stand apart.
pub(super) const AS_DRAWN: [(&str, &str); 3] = [("start_at_nearest_point", "false"), ("end_at_nearest_point", "false"), ("trim_after", "true")];

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
        self.stroke(name, vec![line(first, rest)?], thread, params)
    }

    /// A satin column named `name` between 2 straight rails, each from its first point to its second, sewn
    /// with `thread` and the Ink/Stitch `params`.
    pub fn satin(&mut self, name: &str, rails: [[(f64, f64); 2]; 2], thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let [[a, b], [c, d]] = rails;
        let params = [params, &[("satin_column", "true")]].concat();
        self.stroke(name, vec![line(a, &[b])?, line(c, &[d])?], thread, &params)
    }

    /// A circle of `radius` round `centre`, sewn along from `start` with `thread` and the Ink/Stitch `params`.
    pub fn circle(
        &mut self,
        name: &str,
        centre: (f64, f64),
        radius: f64,
        start: Start,
        thread: &Thread,
        params: &[(&str, &str)],
    ) -> Result<(), SheetError> {
        self.add(name, Shape::stroke(Path { subpaths: vec![round_from(centre, radius, start)?] }), thread, params)
    }

    /// A fill named `name` of the area its closed `rings` bound by the even-odd rule, so a ring inside
    /// another is a hole whichever way it runs, sewn with `thread` and the Ink/Stitch `params`.
    pub fn fill(&mut self, name: &str, rings: Vec<Subpath>, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        self.add(name, Shape::Fill { path: Path { subpaths: rings }, rule: FillRule::EvenOdd }, thread, params)
    }

    fn stroke(&mut self, name: &str, subpaths: Vec<Subpath>, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        self.add(name, Shape::stroke(Path { subpaths }), thread, params)
    }

    fn add(&mut self, name: &str, shape: Shape, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        self.elements.push(Element {
            id: ElementId::new(format!("{}:{name}", self.sheet))?,
            name: None,
            shape,
            thread: thread.clone(),
            params: params.iter().copied().collect::<ParamSet>(),
            automatic: false,
        });
        Ok(())
    }

    /// The sheet planned for the reference machine, or what the engine said about it.
    pub fn plan(self) -> Result<StitchPlan, SheetError> {
        self.plan_saying(&[])
    }

    /// The sheet planned for the reference machine, which says exactly the diagnostics of `codes`, in their
    /// order; otherwise what the engine said about it.
    pub fn plan_saying(self, codes: &[Code]) -> Result<StitchPlan, SheetError> {
        let design = Design::new(self.elements, DesignSettings::default()).map_err(|d| SheetError::Said(d.to_string()))?;
        let outcome = crate::plan(&design, REFERENCE, &Budget::DEFAULT);
        let said: Vec<Code> = outcome.diagnostics.iter().map(|d| d.code).collect();
        match outcome.plan {
            Some(plan) if said == codes => Ok(plan),
            _ => Err(SheetError::Said(outcome.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join(" "))),
        }
    }
}

/// The rails of a satin column across `left` to `right` and `top` to `bottom`, each running the way the column
/// is sewn: `down` or up, its first rail the left one (`from_left`) or the right one. A column starts on its
/// first rail, so a sheet starts each one on the side the needle comes from: the reference machine leaves a
/// jump's thread on the fabric (MC-1), and a column that started on its far side would sew it down along its
/// end. Sewn up or from the right, a column's zigzag is its mirror image, the same stitches to the eye.
pub(super) fn rails((left, right): (f64, f64), (top, bottom): (f64, f64), down: bool, from_left: bool) -> [[(f64, f64); 2]; 2] {
    let (from, to) = if down { (top, bottom) } else { (bottom, top) };
    let (first, second) = if from_left { (left, right) } else { (right, left) };
    [[(first, from), (first, to)], [(second, from), (second, to)]]
}

/// The closed ring through `points`, back to the first.
pub(super) fn polygon(points: &[(f64, f64)]) -> Result<Subpath, UnitError> {
    let Some((&first, rest)) = points.split_first() else { return Ok(Subpath { start: Point::ORIGIN, segments: Vec::new(), closed: true }) };
    Ok(Subpath { closed: true, ..line(first, rest)? })
}

/// Where a circle starts and ends: a point where one of its axes crosses it. A circle runs clockwise as seen
/// from the front, the drawing's y axis pointing down: from its right-hand point down to its bottom.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Start {
    /// Its right-hand point, at 3 o'clock, where an SVG circle starts.
    Right,
    /// Its left-hand point, at 9 o'clock.
    Left,
    /// Its top, at 12 o'clock.
    Top,
}

/// A circle of `radius` round `centre`: four cubic curves from its right-hand point, back to it.
pub(super) fn round(centre: (f64, f64), radius: f64) -> Result<Subpath, UnitError> {
    round_from(centre, radius, Start::Right)
}

/// A circle of `radius` round `centre`: four cubic curves from `start`, clockwise, back to it. A sheet starts
/// a circle where the jump to it arrives, since the reference machine does not cut the jump's thread, and a
/// circle that crossed it would sew it down.
pub(super) fn round_from(centre: (f64, f64), radius: f64, start: Start) -> Result<Subpath, UnitError> {
    // The control points of a quarter circle: 4/3 (√2 − 1) of the radius from its ends.
    let (x, y, r, k) = (centre.0, centre.1, radius, 4.0 / 3.0 * (SQRT_2 - 1.0) * radius);
    // The quarters clockwise, each to the next point from the one before: to the bottom, the left, the top and
    // the right.
    let quarters = [
        [(x + r, y + k), (x + k, y + r), (x, y + r)],
        [(x - k, y + r), (x - r, y + k), (x - r, y)],
        [(x - r, y - k), (x - k, y - r), (x, y - r)],
        [(x + k, y - r), (x + r, y - k), (x + r, y)],
    ];
    let first = match start {
        Start::Right => 0,
        Start::Left => 2,
        Start::Top => 3,
    };
    let mut segments = Vec::with_capacity(quarters.len());
    // The circle closes where it starts: at the end of its last quarter.
    let mut end = (x, y);
    for &[c1, c2, to] in quarters.iter().cycle().skip(first).take(quarters.len()) {
        segments.push(Segment::Cubic(point(c1)?, point(c2)?, point(to)?));
        end = to;
    }
    Ok(Subpath { start: point(end)?, segments, closed: true })
}

/// A point from literal coordinates.
fn point((x, y): (f64, f64)) -> Result<Point, UnitError> {
    Point::new(x, y)
}

/// The open polyline from `first` through `rest`.
fn line(first: (f64, f64), rest: &[(f64, f64)]) -> Result<Subpath, UnitError> {
    let segments = rest.iter().map(|&p| point(p).map(Segment::Line)).collect::<Result<_, _>>()?;
    Ok(Subpath { start: point(first)?, segments, closed: false })
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

    #[test]
    fn rings_close_back_to_their_first_point_and_circles_start_on_their_right() {
        let ring = polygon(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]).unwrap();
        assert_eq!((ring.closed, ring.start, ring.segments.len()), (true, point((0.0, 0.0)).unwrap(), 2));
        let circle = round((1.0, 2.0), 3.0).unwrap();
        assert_eq!((circle.closed, circle.start, circle.segments.len()), (true, point((4.0, 2.0)).unwrap(), 4));
    }

    #[test]
    fn a_circle_starts_where_asked_and_runs_clockwise_through_the_same_curves() {
        let ends = |start| {
            let circle = round_from((1.0, 2.0), 3.0, start).unwrap();
            let ends: Vec<Point> = circle.segments.iter().map(|s| if let Segment::Cubic(_, _, end) = s { *end } else { panic!("{s:?}") }).collect();
            (circle.start, ends)
        };
        let [right, bottom, left, top] = [(4.0, 2.0), (1.0, 5.0), (-2.0, 2.0), (1.0, -1.0)].map(|p| point(p).unwrap());
        assert_eq!(ends(Start::Right), (right, vec![bottom, left, top, right]));
        assert_eq!(ends(Start::Left), (left, vec![top, right, bottom, left]));
        assert_eq!(ends(Start::Top), (top, vec![right, bottom, left, top]));
        // The same curves, whichever point it starts from.
        let curves = |start| round_from((1.0, 2.0), 3.0, start).unwrap().segments;
        let mut turned = curves(Start::Right);
        turned.rotate_left(2);
        assert_eq!(curves(Start::Left), turned);
    }

    #[test]
    fn a_sheet_draws_with_exactly_the_diagnostics_it_asks_for() {
        // One fill of 2 squares apart, which the engine sews in 2 parts and warns of (SC-W0307).
        let parts = || {
            let thread = Thread::new(Rgb::new(0, 0, 0));
            let mut d = Drawing::new("parts");
            let squares = vec![
                polygon(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)]).unwrap(),
                polygon(&[(20.0, 0.0), (20.0, 10.0), (30.0, 10.0), (30.0, 0.0)]).unwrap(),
            ];
            d.fill("fill", squares, &thread, &[]).unwrap();
            d
        };
        let Err(SheetError::Said(said)) = parts().plan() else { panic!("drawn despite the warning") };
        assert!(said.starts_with("warning SC-W0307: "), "{said}");
        assert!(parts().plan_saying(&[Code::FillPartsApart]).is_ok());
        assert!(parts().plan_saying(&[Code::FillPartsApart; 2]).is_err(), "said once, not twice");
    }
}
