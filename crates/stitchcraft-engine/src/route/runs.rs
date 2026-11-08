//! The elements an auto-run route sews, made from its steps (`docs/src/design/routing.md` › Auto-run, steps
//! 4 to 7), as Ink/Stitch's Auto-Route Running Stitch makes them (read at `d59c9ab`), each with the settings
//! of the strokes it runs along.
//!
//! 1. **Each step** along a piece runs along it, the way the step goes, and a step along a jump jumps.
//! 2. **A piece passed twice** is under the others the first time, and on top the last time. Each piece is
//!    passed once or twice: along the way from the start to the end once, and off it out and back.
//! 3. **Pieces in a row** are one element, until a jump comes, the route goes from on top to under or back,
//!    a piece on top comes from another stroke, or a piece under from a stroke whose run under the others
//!    sews otherwise: another thread, another stitch length or tolerance, a manual stitch or not.
//! 4. **Elements.** One on top is a copy of its stroke, its path the pieces', its settings the stroke's,
//!    but an even number of repeats is lowered by 1, so that it ends at its far end. One under the others
//!    is a running stitch along its pieces with the stroke's running stitch length and tolerance, or a
//!    manual stitch where the stroke is one, without repeats or bean stitch. Each is the stroke's thread and
//!    stroke, and neither keeps the stroke's clips, positions or target point: the route places its ends.
//!    Ink/Stitch gives an element on top the settings of the stroke of the element under the others that
//!    follows it, and an element under the others along several strokes the last one's (`DEV-ROUTE-006`).
//! 5. **Trims.** With Trim jump stitches, a jump longer than [`RUN_TRIMMED`] trims the thread after the
//!    element before it, and the elements under the others right after a trim are left out, as the needle
//!    no longer needs them. Of 2 jumps in a row, Ink/Stitch looks only at the first (`DEV-ROUTE-009`).
//! 6. **Names.** AutoRun and a number for an element on top, AutoRun Underpath and a number for one under
//!    the others, counting every element made, from 1.

use std::collections::BTreeSet;

use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_params::{ParamSet, StitchType};

use crate::auto::numbered;
use crate::common::TRIM_AFTER;
use crate::design::{Element, Hints, Path, Segment, Shape, Subpath};
use crate::normalize::satin::SAME_POINT;
use crate::route::graph::{Graph, Link, Step};
use crate::route::lines::Stroke;
use crate::route::sew::RUN_LENGTH;
use crate::route::{Options, Piece};

/// How long a jump of an auto-run route may be without a trim, in millimetres: 0.75 mm, Ink/Stitch's. A
/// longer one trims the thread when the route trims.
pub const RUN_TRIMMED: f64 = 0.75;

/// The key of a stroke's repeats.
const REPEATS: &str = "repeats";

/// The key of a running stitch's tolerance.
const TOLERANCE: &str = "running_stitch_tolerance_mm";

/// The key of a stroke's method.
const STROKE_METHOD: &str = "stroke_method";

/// An element being made: the stroke whose settings it takes, by its place among those routed, whether it
/// runs under the others, and its points.
#[derive(Clone, Debug, PartialEq)]
struct Made {
    stroke: usize,
    under: bool,
    points: Vec<Point>,
}

/// The elements `steps` sew, the route along `graph`'s links, of the routed `strokes` of `elements`, as
/// `options` say. Each step looked at charges `meter` a unit.
pub(super) fn pieces(
    (steps, graph): (&[Step], &Graph),
    (strokes, elements): (&[Stroke], &[Element]),
    options: &Options,
    meter: &mut Meter,
) -> Result<Vec<Piece>, Exhausted> {
    let mut seen = BTreeSet::new();
    let mut under = vec![false; steps.len()];
    for (k, step) in steps.iter().enumerate().rev() {
        meter.charge(1)?;
        if let Some(slot) = under.get_mut(k) {
            *slot = !seen.insert(step.link);
        }
    }
    let mut made: Vec<Made> = Vec::new();
    let mut trims = BTreeSet::new();
    let mut making: Option<Made> = None;
    let mut just_trimmed = false;
    for (step, &under) in steps.iter().zip(&under) {
        meter.charge(1)?;
        let Some(link) = graph.link(step.link) else { continue };
        let Link::Piece { element: stroke, index, ends } = link else {
            made.extend(making.take());
            if options.trim && !made.is_empty() && long(graph, step) {
                trims.insert(made.len() - 1);
                just_trimmed = true;
            }
            continue;
        };
        if just_trimmed && under {
            continue;
        }
        just_trimmed = false;
        let Some(&[a, b]) = strokes.get(stroke).and_then(|routed| routed.pieces.get(index)) else { continue };
        let (from, to) = if step.from == ends[0] { (a, b) } else { (b, a) };
        let goes_on = making.as_ref().is_some_and(|being| being.under == under && same(being.stroke, stroke, under, (strokes, elements)));
        if !goes_on {
            made.extend(making.replace(Made { stroke, under, points: vec![from] }));
        }
        if let Some(being) = making.as_mut() {
            being.points.push(to);
            being.stroke = stroke;
        }
    }
    made.extend(making);
    let mut pieces = Vec::with_capacity(made.len());
    for (k, made) in made.iter().enumerate() {
        meter.charge(1)?;
        pieces.extend(element(made, (strokes, elements), pieces.len() + 1, trims.contains(&k)));
    }
    Ok(pieces)
}

/// Whether the jump `step` takes through `graph` is longer than [`RUN_TRIMMED`].
fn long(graph: &Graph, step: &Step) -> bool {
    let places = graph.places();
    places.get(step.from).zip(places.get(step.to)).is_some_and(|(from, to)| from.at.distance(to.at) > RUN_TRIMMED)
}

/// Whether a piece of the stroke `next` goes on the element being made of the stroke `being`, `under` the
/// others or on top: on top, when they are one stroke; under, when their runs under the others sew alike.
fn same(being: usize, next: usize, under: bool, (strokes, elements): (&[Stroke], &[Element])) -> bool {
    if being == next {
        return true;
    }
    let sewn = |stroke: usize| {
        let routed = strokes.get(stroke)?;
        let element = elements.get(routed.element)?;
        Some((&element.thread, underneath(element, routed.stitch_type)))
    };
    under && sewn(being).is_some() && sewn(being) == sewn(next)
}

/// The settings of a run under the others along `element`, a stroke sewn as `stitch_type`: its running
/// stitch length and tolerance, where it sets them, and its method where it is a manual stitch.
fn underneath(element: &Element, stitch_type: StitchType) -> ParamSet {
    let mut params = ParamSet::new();
    for key in [RUN_LENGTH, TOLERANCE] {
        if let Some(value) = element.params.get(key) {
            params.set(key, value);
        }
    }
    if stitch_type == StitchType::ManualStitch {
        params.set(STROKE_METHOD, stitch_type.id());
    }
    params
}

/// The element `made` is, of the routed `strokes` of `elements`, its id numbered `number`, and trimmed
/// after when `trimmed`: a copy of its stroke on top, or a run under the others, along its points, a point
/// that repeats the one before to within a millionth of a millimetre left out. `None` without 2 points.
fn element(made: &Made, (strokes, elements): (&[Stroke], &[Element]), number: usize, trimmed: bool) -> Option<Piece> {
    let routed = strokes.get(made.stroke)?;
    let element = elements.get(routed.element)?;
    let Shape::Stroke { width, join, cap, .. } = &element.shape else { return None };
    let mut kept: Vec<Point> = Vec::with_capacity(made.points.len());
    for &point in &made.points {
        if kept.last().is_none_or(|last| last.distance(point) >= SAME_POINT) {
            kept.push(point);
        }
    }
    let (&start, rest) = kept.split_first().filter(|(_, rest)| !rest.is_empty())?;
    let path = Path { subpaths: vec![Subpath { start, segments: rest.iter().map(|&point| Segment::Line(point)).collect(), closed: false }] };
    let mut params = if made.under { underneath(element, routed.stitch_type) } else { element.params.clone() };
    if !made.under && routed.repeats % 2 == 0 {
        params.set(REPEATS, routed.repeats.saturating_sub(1).max(1).to_string());
    }
    if trimmed {
        params.set(TRIM_AFTER, "true");
    }
    let name = if made.under { format!("AutoRun Underpath {number}") } else { format!("AutoRun {number}") };
    let run = Element {
        id: numbered(&element.id, number)?,
        name: Some(name),
        shape: Shape::Stroke { path, width: *width, join: *join, cap: *cap },
        thread: element.thread.clone(),
        params,
        automatic: false,
        hints: Hints::default(),
        clips: Vec::new(),
    };
    Some(Piece { element: run, from: routed.element, under: made.under })
}

#[cfg(test)]
mod tests;
