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

use stitchcraft_core::{Budget, Code, ElementId, Mm, Point, UnitError};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{StitchPlan, Thread};

use super::Cloned;
use super::pictures::Picture;
use super::sketch::SheetError;
use crate::common::{STOP_AFTER, TRIM_AFTER};
use crate::design::{Cap, Clip, Design, DesignSettings, Element, FillRule, Guide, Hints, Join, Path, Segment, Shape, Subpath, Turn};
use crate::generators::satin::{END_AT_NEAREST_POINT, START_AT_NEAREST_POINT};
use crate::generators::tatami::ANGLE;
use crate::route::{Route, route};

/// The settings that sew a sheet's satin column as drawn, from its start to its end whatever is around it,
/// and trim after it, so the columns stand apart.
pub(super) const AS_DRAWN: [(&str, &str); 3] = [(START_AT_NEAREST_POINT, "false"), (END_AT_NEAREST_POINT, "false"), (TRIM_AFTER, "true")];

/// A test sheet as a design: its elements, in sewing order, and the design's settings, and the elements its
/// file draws as clones of others.
pub(super) struct Drawing {
    sheet: &'static str,
    elements: Vec<Element>,
    settings: DesignSettings,
    clones: Vec<Cloned>,
}

impl Drawing {
    /// A sheet whose element ids start with `sheet` (`ts03:circle-0.1`), with the design settings' defaults.
    pub fn new(sheet: &'static str) -> Self {
        Drawing { sheet, elements: Vec::new(), settings: DesignSettings::default(), clones: Vec::new() }
    }

    /// A clone named `name` of the fill drawn as `of`, which the sheet's file draws as a `<use>` of it under
    /// `map` (`REQ-SVG-012`), SVG's `matrix(a b c d e f)` in millimetres: the fill's shape and start and end
    /// positions moved by the map, and its parameters, but its `angle`, which the clone turns to `angle`
    /// (`REQ-SVG-013`). The design holds it as an element of its own, which `stitch testsheet` sews, and the
    /// SVG reader reads the clone back as that element. `SheetError::Said` when there is no fill `of`.
    pub fn clone_of(&mut self, name: &str, of: &str, map: [f64; 6], angle: &str) -> Result<(), SheetError> {
        let of = ElementId::new(format!("{}:{of}", self.sheet))?;
        let Some(source) = self.elements.iter().find(|e| e.id == of) else {
            return Err(SheetError::Said(format!("{of} is not drawn before its clone")));
        };
        let Shape::Fill { path, rule } = &source.shape else { return Err(SheetError::Said(format!("{of} is not a fill"))) };
        let [a, b, c, d, e, f] = map;
        let moved = |p: Point| Point::new(a * p.x() + c * p.y() + e, b * p.x() + d * p.y() + f);
        let shape = Shape::Fill { path: path.map(moved)?, rule: *rule };
        let mut params = source.params.clone();
        params.set(ANGLE, angle);
        let id = ElementId::new(format!("{}:{name}", self.sheet))?;
        let element = Element { id: id.clone(), shape, params, hints: source.hints.map(moved)?, ..source.clone() };
        self.elements.push(element);
        self.clones.push(Cloned { element: id, of, map });
        Ok(())
    }

    /// The elements the sheet's file draws as clones.
    pub fn clones(self) -> Vec<Cloned> {
        self.clones
    }

    /// The design's origin, which goes to the hoop's centre, and the stop position the frame moves to before
    /// each stop (`REQ-ASM-005`), as Ink/Stitch's commands give them.
    pub fn origin_and_stop_position(&mut self, origin: (f64, f64), stop_position: (f64, f64)) -> Result<(), SheetError> {
        self.settings.origin = Some(point(origin)?);
        self.settings.stop_position = Some(point(stop_position)?);
        Ok(())
    }

    /// The turn the design is written at, about its origin (`REQ-ASM-005`), as Ink/Stitch's
    /// `rotate_on_export` gives it.
    pub fn turned(&mut self, turn: Turn) {
        self.settings.rotate_on_export = turn;
    }

    /// A stroke through `points`, named `name`, sewn with `thread` and the Ink/Stitch `params`.
    pub fn polyline(&mut self, name: &str, points: &[(f64, f64)], thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let Some((&first, rest)) = points.split_first() else { return Ok(()) };
        self.stroke(name, vec![line(first, rest)?], thread, params)
    }

    /// A stroke along `path`, named `name`, sewn with `thread` and the Ink/Stitch `params`, drawn as a
    /// polyline's stroke is.
    pub fn along(&mut self, name: &str, path: Subpath, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        self.stroke(name, vec![path], thread, params)
    }

    /// A stroke of several `subpaths`, as a ripple stitch's rails and rungs are, named `name`, sewn with
    /// `thread` and the Ink/Stitch `params`.
    pub fn strokes(&mut self, name: &str, subpaths: Vec<Subpath>, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        self.stroke(name, subpaths, thread, params)
    }

    /// A stroke along `path`, `width` mm wide, named `name`, sewn with `thread` and the Ink/Stitch `params`:
    /// a zigzag stroke sews across its path as wide as it is drawn.
    pub fn wide(&mut self, name: &str, path: Subpath, width: f64, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let shape = Shape::Stroke { path: Path { subpaths: vec![path] }, width: Mm::new(width)?, join: Join::UNSET, cap: Cap::Butt };
        self.add(name, shape, thread, params)
    }

    /// A satin column named `name` between 2 rails, each from its start to its end, sewn with `thread` and
    /// the Ink/Stitch `params`. The first rail is the one a satin column starts on, and an E-stitch's spine.
    pub fn satin_between(&mut self, name: &str, rails: [Subpath; 2], thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let params = [params, &[("satin_column", "true")]].concat();
        self.stroke(name, rails.into(), thread, &params)
    }

    /// A satin column named `name` drawn with `subpaths`, its rails and rungs, sewn with `thread` and the
    /// Ink/Stitch `params`.
    pub fn satin_drawn(&mut self, name: &str, subpaths: Vec<Subpath>, thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let params = [params, &[("satin_column", "true")]].concat();
        self.stroke(name, subpaths, thread, &params)
    }

    /// The elements of `drawing`, routed as `how` says, in the design's settings, after the elements drawn so
    /// far: the route's pieces, each named by its id. A route that says anything is not the route the sheet
    /// shows, and fails, saying what it said.
    pub fn routed(&mut self, drawing: &Drawing, how: &Route) -> Result<(), SheetError> {
        let routed = route(&drawing.elements, how, &self.settings, &Budget::DEFAULT);
        if !routed.diagnostics.is_empty() || routed.pieces.is_empty() {
            return Err(SheetError::Said(routed.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")));
        }
        self.elements.extend(routed.pieces.into_iter().map(|piece| piece.element));
        Ok(())
    }

    /// A satin column named `name` between 2 straight rails, each from its first point to its second, sewn
    /// with `thread` and the Ink/Stitch `params`.
    pub fn satin(&mut self, name: &str, rails: [[(f64, f64); 2]; 2], thread: &Thread, params: &[(&str, &str)]) -> Result<(), SheetError> {
        let [[a, b], [c, d]] = rails;
        self.satin_between(name, [line(a, &[b])?, line(c, &[d])?], thread, params)
    }

    /// A satin column named `name` round `centre`, between a circle of radius `outer`, its first rail, and one
    /// of radius `inner`, each round from `start`, sewn with `thread` and the Ink/Stitch `params`.
    pub fn ring(
        &mut self,
        name: &str,
        (centre, start): ((f64, f64), Start),
        (outer, inner): (f64, f64),
        thread: &Thread,
        params: &[(&str, &str)],
    ) -> Result<(), SheetError> {
        self.satin_between(name, [round_from(centre, outer, start)?, round_from(centre, inner, start)?], thread, params)
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

    /// The shapes of `picture`, each named after the picture with `half` before it
    /// (`ts13a:auto:badge-abutting:red`): automatic, as the picture draws them, when `automatic`, and
    /// otherwise shapes without settings, which sew as drawn, in paint order.
    pub fn picture(&mut self, half: &str, picture: Picture, automatic: bool) -> Result<(), SheetError> {
        for element in picture.elements {
            let id = ElementId::new(format!("{}:{half}:{}:{}", self.sheet, picture.name, element.id))?;
            self.elements.push(Element { id, automatic, ..element });
        }
        Ok(())
    }

    /// A stop after the last element drawn so far: what is drawn before it sews before it, and what is drawn
    /// after it sews after it, whatever order automatic digitizing chooses (`REQ-AUTO-010`).
    pub fn stop(&mut self) {
        if let Some(last) = self.elements.last_mut() {
            last.params.set(STOP_AFTER, "true");
        }
    }

    /// Start and end positions for the last element drawn so far, as Ink/Stitch's start and end commands
    /// give them (`REQ-GEN-001`); none before the first.
    pub fn positions(&mut self, start: (f64, f64), end: (f64, f64)) -> Result<(), SheetError> {
        if let Some(last) = self.elements.last_mut() {
            last.hints = Hints { start: Some(point(start)?), end: Some(point(end)?), ..Hints::default() };
        }
        Ok(())
    }

    /// A target point for the last element drawn so far, as Ink/Stitch's target point command gives it: the
    /// point its ripple stitch shrinks towards (`REQ-RIP-002`); none before the first.
    pub fn target(&mut self, at: (f64, f64)) -> Result<(), SheetError> {
        if let Some(last) = self.elements.last_mut() {
            last.hints.target = Some(point(at)?);
        }
        Ok(())
    }

    /// A guide line for the last element drawn so far, as Ink/Stitch's guide-line marker on a path grouped with
    /// it gives it: `subpaths`, a stroke `width` mm wide, with the Ink/Stitch `params`. Its ripple stitch's
    /// copies follow it (`REQ-RIP-012`), or lie between its rails when it is a satin column or has several
    /// subpaths (`REQ-RIP-013`). None before the first.
    pub fn guide(&mut self, subpaths: Vec<Subpath>, width: f64, params: &[(&str, &str)]) -> Result<(), SheetError> {
        if let Some(last) = self.elements.last_mut() {
            let params = params.iter().copied().collect();
            last.hints.guide = Some(Guide { path: Path { subpaths }, width: Mm::new(width)?, join: Join::UNSET, params });
        }
        Ok(())
    }

    /// An anchor line for the last element drawn so far, from `from` to `to`, as Ink/Stitch's anchor-line
    /// marker gives it: where a satin guide places each copy's ends (`REQ-RIP-014`). None before the first.
    pub fn anchor(&mut self, from: (f64, f64), to: (f64, f64)) -> Result<(), SheetError> {
        if let Some(last) = self.elements.last_mut() {
            last.hints.anchor = Some([point(from)?, point(to)?]);
        }
        Ok(())
    }

    /// A clip for the last element drawn so far, as a clip path round it cuts it (`REQ-GEN-005`): the area its
    /// closed `rings` bound by the nonzero rule.
    pub fn clip(&mut self, rings: Vec<Subpath>) {
        if let Some(last) = self.elements.last_mut() {
            last.clips.push(Clip::of(Path { subpaths: rings }, FillRule::NonZero));
        }
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
            hints: Hints::default(),
            clips: Vec::new(),
        });
        Ok(())
    }

    /// The sheet as a design with its settings, as it is planned; or why it is none.
    pub fn design(self) -> Result<Design, SheetError> {
        Design::new(self.elements, self.settings).map_err(|d| SheetError::Said(d.to_string()))
    }

    /// The sheet planned for the reference machine, which says exactly the diagnostics of `codes`, in their
    /// order; otherwise what the engine said about it.
    pub fn plan_saying(self, codes: &[Code]) -> Result<StitchPlan, SheetError> {
        let design = self.design()?;
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

/// The open polyline through `points`.
pub(super) fn open(points: &[(f64, f64)]) -> Result<Subpath, UnitError> {
    let Some((&first, rest)) = points.split_first() else { return Ok(Subpath { start: Point::ORIGIN, segments: Vec::new(), closed: false }) };
    line(first, rest)
}

/// The control points of a quarter circle drawn as a cubic curve lie this share of its radius from its ends,
/// 4/3 (√2 − 1).
pub(super) const KAPPA: f64 = 4.0 / 3.0 * (SQRT_2 - 1.0);

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
    let (x, y, r, k) = (centre.0, centre.1, radius, KAPPA * radius);
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

/// Which way a hook runs from its start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Way {
    /// East, then round to the south the way a clock's hands turn.
    East,
    /// West, then round to the south the other way: a hook running east, mirrored.
    West,
}

impl Way {
    /// The sign of the way along x: 1 east and -1 west.
    pub(super) const fn sign(self) -> f64 {
        match self {
            Way::East => 1.0,
            Way::West => -1.0,
        }
    }

    /// The other way.
    pub(super) const fn back(self) -> Way {
        match self {
            Way::East => Way::West,
            Way::West => Way::East,
        }
    }
}

/// A hook: `straight` mm from `start` the `way` it runs, then a quarter circle of `radius` round to the south,
/// so a sample sews straight and then curved. With `aside`, the hook lies that far towards the middle of its
/// turn, as a satin column's rail lies beside its middle: south of the straight, and inside the curve.
pub(super) fn hook((x, y): (f64, f64), way: Way, (straight, radius): (f64, f64), aside: f64) -> Result<Subpath, UnitError> {
    // The quarter circle round (x ± straight, y + radius), from its top to its side, as `round_from` draws a
    // quarter, mirrored for a hook running west.
    let (s, r) = (way.sign(), radius - aside);
    let (cx, cy, k) = (x + s * straight, y + radius, KAPPA * r);
    let turn = Segment::Cubic(point((cx + s * k, cy - r))?, point((cx + s * r, cy - k))?, point((cx + s * r, cy))?);
    Ok(Subpath { start: point((x, y + aside))?, segments: vec![Segment::Line(point((cx, cy - r))?), turn], closed: false })
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
        let Err(SheetError::Said(said)) = d.plan_saying(&[]) else { panic!("drawn despite what the engine said") };
        assert!(said.starts_with("error SC-E0101: "), "{said}");
        assert!(Drawing::new("empty").plan_saying(&[]).is_err(), "nothing to stitch");
        // Two elements with one id: the design itself is refused.
        let mut twice = Drawing::new("twice");
        twice.polyline("a", &[(0.0, 0.0), (10.0, 0.0)], &thread, &[]).unwrap();
        twice.polyline("a", &[(0.0, 5.0), (10.0, 5.0)], &thread, &[]).unwrap();
        let Err(SheetError::Said(said)) = twice.plan_saying(&[]) else { panic!("drawn with one id twice") };
        assert!(said.contains("Two elements of the design have the id `twice:a`."), "{said}");
    }

    #[test]
    fn a_stop_positions_hints_or_a_clip_before_any_element_change_nothing() {
        // Each is for the last element drawn so far, and there is none yet.
        let mut d = Drawing::new("none");
        d.stop();
        assert!(d.positions((0.0, 0.0), (1.0, 1.0)).is_ok());
        assert!(d.target((1.0, 0.0)).is_ok());
        assert!(d.guide(vec![open(&[(0.0, 0.0), (1.0, 0.0)]).unwrap()], 0.5, &[]).is_ok());
        assert!(d.anchor((0.0, 0.0), (0.0, 1.0)).is_ok());
        d.clip(vec![polygon(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0)]).unwrap()]);
        assert!(d.elements.is_empty());
    }

    #[test]
    fn a_clone_is_of_a_fill_drawn_before_it() {
        let thread = Thread::new(Rgb::new(0, 0, 0));
        let mut d = Drawing::new("test");
        d.polyline("line", &[(0.0, 0.0), (10.0, 0.0)], &thread, &[]).unwrap();
        let identity = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let said = |result: Result<(), SheetError>| result.unwrap_err().to_string();
        assert_eq!(said(d.clone_of("copy", "fill", identity, "0")), "the engine says: test:fill is not drawn before its clone");
        assert_eq!(said(d.clone_of("copy", "line", identity, "0")), "the engine says: test:line is not a fill");
        // A fill moved by the map, its hints with it, its angle given.
        d.fill("fill", vec![polygon(&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0)]).unwrap()], &thread, &[("row_spacing_mm", "0.3")]).unwrap();
        d.positions((4.0, 0.0), (0.0, 0.0)).unwrap();
        d.clone_of("copy", "fill", [0.0, 1.0, -1.0, 0.0, 10.0, 0.0], "-90").unwrap();
        let clone = d.elements.last().unwrap();
        let ring = &clone.shape.path().subpaths[0];
        assert_eq!((ring.start, ring.segments[1].end()), (point((10.0, 0.0)).unwrap(), point((8.0, 4.0)).unwrap()));
        assert_eq!((clone.hints.start, clone.hints.end), (Some(point((10.0, 4.0)).unwrap()), Some(point((10.0, 0.0)).unwrap())));
        assert_eq!(clone.params.iter().collect::<Vec<_>>(), [("angle", "-90"), ("row_spacing_mm", "0.3")]);
        assert_eq!(d.clones().iter().map(|c| (c.element.as_str(), c.of.as_str(), c.map[4])).collect::<Vec<_>>(), [("test:copy", "test:fill", 10.0)]);
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
        let Err(SheetError::Said(said)) = parts().plan_saying(&[]) else { panic!("drawn despite the warning") };
        assert!(said.starts_with("warning SC-W0307: "), "{said}");
        assert!(parts().plan_saying(&[Code::FillPartsApart]).is_ok());
        assert!(parts().plan_saying(&[Code::FillPartsApart; 2]).is_err(), "said once, not twice");
    }

    #[test]
    fn a_route_that_says_anything_is_not_drawn() {
        let how = Route { tool: crate::route::Tool::Satins, options: crate::route::Tool::Satins.defaults(), start: None, end: None };
        let Err(SheetError::Said(said)) = Drawing::new("routed").routed(&Drawing::new("nothing"), &how) else { panic!("routed nothing") };
        assert!(said.starts_with("error SC-E1001: "), "{said}");
        // Nor is a route that sews its column but says that it leaves a fill as it is.
        let thread = Thread::new(Rgb::new(0, 0, 0));
        let mut some = Drawing::new("some");
        some.satin("column", [[(0.0, 0.0), (10.0, 0.0)], [(0.0, 3.0), (10.0, 3.0)]], &thread, &[]).unwrap();
        some.fill("fill", vec![polygon(&[(0.0, 10.0), (10.0, 10.0), (10.0, 20.0)]).unwrap()], &thread, &[]).unwrap();
        assert_eq!(route(&some.elements, &how, &DesignSettings::default(), &Budget::DEFAULT).pieces.len(), 1);
        let Err(SheetError::Said(said)) = Drawing::new("routed").routed(&some, &how) else { panic!("routed despite what it said") };
        assert_eq!(said, "info SC-I1002: This element is left out of the route and kept as it is: it is a fill, which a route doesn't take.");
    }
}
