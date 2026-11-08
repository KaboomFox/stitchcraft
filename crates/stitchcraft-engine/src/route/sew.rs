//! The elements a route sews, made from its steps (`docs/src/design/routing.md` › Auto-satin, steps 5 to
//! 9), as Ink/Stitch makes them (read at `d59c9ab`).
//!
//! 1. **Each step** along a piece sews it, the way the step goes, and a step along a jump jumps.
//! 2. **A column's piece passed twice** is sewn as a running stitch along its stretch of the centre line
//!    the first time, under the satin that the last time sews over it. A stroke's piece is sewn each time.
//! 3. **Pieces in a row** are one element: a column's pieces that follow on along it the same way, and
//!    running stitches along one element, unless the second goes back the way the first came.
//! 4. **Elements.** A column's pieces are cut from it ([`Cuttable::piece`]). A column's piece keeps its
//!    settings, but an odd number of centre walk repeats is lowered by 1, to no fewer than 2, so that it
//!    sews from its start. A run under a column takes the column's running stitch length, or else its centre
//!    walk's or its contour's, its shortest jump, its bean stitch and its `underlays_first`, where it sets
//!    them, and a run along a stroke the stroke's. A route's pieces and runs that set `underlays_first` so
//!    sew one run of underlays first, unless a trim parts them (`REQ-ASM-009`). Each is the element's thread
//!    and stroke. A column's piece keeps the column's clips
//!    too, which leave a satin column whole, and a run, a path of its own, keeps none. Neither keeps the
//!    element's start and end positions or its target point: the route places its ends.
//! 5. **Trims.** A jump longer than [`TRIMMED`] outside the elements it joins ([`trim::outside`]) trims after
//!    the element before it, when the route trims. The runs right after a trim are left out, and the last
//!    element trims after it too.
//! 6. **Names.** AutoSatin and a number for a column's piece, AutoSatin Running Stitch and a number for a
//!    run, counting every element. With Preserve order a column's piece keeps its element's name, when the
//!    element has one, and isn't counted.

use std::collections::BTreeSet;

use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};
use stitchcraft_params::ParamSet;

use crate::auto::numbered;
use crate::common::{MIN_JUMP_STITCH_LENGTH, TRIM_AFTER, UNDERLAYS_FIRST};
use crate::design::{Element, Hints, Path, Segment, Shape, Subpath};
use crate::generators::passes::BEAN_STITCH_REPEATS;
use crate::normalize::satin::SAME_POINT;
use crate::route::graph::{Graph, Link, Step};
use crate::route::read::{Kind, Routable};
use crate::route::trim::{self, TRIMMED};
use crate::route::{Options, Piece};

/// The key of a running stitch's length.
pub(super) const RUN_LENGTH: &str = "running_stitch_length_mm";

/// The keys of the lengths a run under a column takes its stitch length from, the first the column sets.
const RUN_LENGTHS: [&str; 3] = [RUN_LENGTH, "center_walk_underlay_stitch_length_mm", "contour_underlay_stitch_length_mm"];

/// The key of a satin column's centre walk repeats.
const CENTRE_WALK_REPEATS: &str = "center_walk_underlay_repeats";

/// What a route does next.
#[derive(Clone, Debug, PartialEq)]
enum Op {
    /// Sews the routed element `routable`'s pieces `first` to `last` as satin, back from the last to the
    /// first when `back`.
    Satin { routable: usize, first: usize, last: usize, back: bool },
    /// Sews a running stitch along `points` of the routed element `routable`: `under` a column's satin.
    Run { routable: usize, points: Vec<Point>, under: bool },
    /// Jumps from one place to another.
    Jump { from: usize, to: usize },
}

/// A piece made, and whether it is a column's piece, which a trim doesn't leave out.
struct Made {
    piece: Piece,
    satin: bool,
}

/// The elements `steps` sew, the route along `graph`'s links, of the routed elements `routables` of
/// `elements`, as `options` say; what is said of them goes to `said`. Cutting the pieces charges `meter`.
pub(super) fn pieces(
    (steps, graph): (&[Step], &Graph),
    (routables, elements): (&[Routable], &[Element]),
    options: &Options,
    said: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Piece>, Exhausted> {
    let ops = merged(under(ops(steps, graph, routables, meter)?, routables, meter)?);
    let mut made: Vec<Made> = Vec::new();
    let mut trims = BTreeSet::new();
    for op in ops {
        meter.charge(1)?;
        match op {
            Op::Satin { routable, first, last, back } => {
                let Some(routed) = routables.get(routable) else { continue };
                let piece = satin(routed, elements, [first, last], back, made.len() + 1, said, meter)?;
                made.extend(piece.map(|piece| Made { piece, satin: true }));
            }
            Op::Run { routable, points, under } => {
                let Some(routed) = routables.get(routable) else { continue };
                made.extend(run(routed, elements, &points, under, made.len() + 1).map(|piece| Made { piece, satin: false }));
            }
            Op::Jump { from, to } => {
                if !made.is_empty() && options.trim && long(graph, routables, [from, to], meter)? {
                    trims.insert(made.len() - 1);
                }
            }
        }
    }
    named(&mut made, elements, options.preserve_order);
    let made = if options.trim { trimmed(made, &trims) } else { made };
    Ok(made.into_iter().map(|made| made.piece).collect())
}

/// The operations `steps` make, one for each step: a column's piece sewn as satin the way the step goes, a
/// stroke's as a run, and a jump.
fn ops(steps: &[Step], graph: &Graph, routables: &[Routable], meter: &mut Meter) -> Result<Vec<Op>, Exhausted> {
    let mut ops = Vec::with_capacity(steps.len());
    for step in steps {
        meter.charge(1)?;
        let Some(link) = graph.link(step.link) else { continue };
        let Link::Piece { element, index, ends } = link else {
            ops.push(Op::Jump { from: step.from, to: step.to });
            continue;
        };
        let back = step.from != ends[0];
        let Some(routed) = routables.get(element) else { continue };
        ops.push(if routed.is_column() {
            Op::Satin { routable: element, first: index, last: index, back }
        } else {
            let mut points = routed.line.clone();
            if back {
                points.reverse();
            }
            Op::Run { routable: element, points, under: false }
        });
    }
    Ok(ops)
}

/// `ops`, with each column's piece sewn before its last time a run along its stretch of the centre line,
/// under the satin.
fn under(mut ops: Vec<Op>, routables: &[Routable], meter: &mut Meter) -> Result<Vec<Op>, Exhausted> {
    let mut seen = BTreeSet::new();
    for op in ops.iter_mut().rev() {
        meter.charge(1)?;
        let Op::Satin { routable, first, back, .. } = *op else { continue };
        if seen.insert((routable, first)) {
            continue;
        }
        let Some(routed) = routables.get(routable) else { continue };
        *op = Op::Run { routable, points: routed.stretch(first, first, back, meter)?, under: true };
    }
    Ok(ops)
}

/// `ops` with each run of operations in a row that make one element merged: a column's pieces that follow
/// on along it the same way, and runs along one element, unless the second goes back the way the first came.
fn merged(ops: Vec<Op>) -> Vec<Op> {
    let mut out: Vec<Op> = Vec::with_capacity(ops.len());
    for op in ops {
        match (out.last_mut(), op) {
            (Some(Op::Satin { routable, first, last, back }), Op::Satin { routable: next, first: from, last: to, back: next_back })
                if *routable == next && *back == next_back && if *back { to + 1 == *first } else { *last + 1 == from } =>
            {
                *first = (*first).min(from);
                *last = (*last).max(to);
            }
            (Some(Op::Run { routable, points, under }), Op::Run { routable: next, points: more, under: next_under })
                if *routable == next && !(points.first() == more.last() && points.last() == more.first()) =>
            {
                *under = *under && next_under;
                let joint = points.last().copied();
                points.extend(more.into_iter().skip_while(|point| Some(*point) == joint));
            }
            (_, op) => out.push(op),
        }
    }
    out
}

/// The piece of the column `routed` from its piece `first` to its piece `last`, both included, sewn back
/// when `back`: cut from it, with its settings, its id numbered `number`. `None` when it sews nothing.
/// `SC-W1003` goes to `said` when its rails and rungs would be read back otherwise.
fn satin(
    routed: &Routable,
    elements: &[Element],
    [first, last]: [usize; 2],
    back: bool,
    number: usize,
    said: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Piece>, Exhausted> {
    let (Kind::Column(column), Some(element)) = (&routed.kind, elements.get(routed.element)) else { return Ok(None) };
    let Shape::Stroke { width, join, cap, .. } = &element.shape else { return Ok(None) };
    let (cuttable, satin) = column.as_ref();
    let start = if first > 0 { cuttable.cut_at(routed.fraction(first), meter)? } else { Some([0.0; 2]) };
    let end = if last + 1 < routed.pieces { cuttable.cut_at(routed.fraction(last + 1), meter)? } else { Some(cuttable.lengths()) };
    let (Some(start), Some(end), Some(id)) = (start, end, numbered(&element.id, number)) else { return Ok(None) };
    let push = satin.push_compensation_mm.map(|mm| mm.get());
    let Some(cut) = cuttable.piece(start, end, back, push, meter)? else { return Ok(None) };
    if !cut.reads_back {
        let message = "A piece of this satin column, cut for the route, would be read back with other rails or rungs, so it may sew otherwise \
                       than the column does there.";
        let fix = "Draw the column with more rungs where it turns sharply.";
        said.push(Diagnostic::new(Code::RoutePieceReadOtherwise, message).with_element(element.id.clone()).with_fix(Fix::Hint(fix.to_string())));
    }
    let mut params = element.params.clone();
    let repeats = satin.center_walk_underlay_repeats;
    if repeats % 2 == 1 {
        params.set(CENTRE_WALK_REPEATS, repeats.saturating_sub(1).max(2).to_string());
    }
    let shape = Shape::Stroke { path: cut.path, width: *width, join: *join, cap: *cap };
    let piece = Element { id, name: None, shape, params, automatic: false, hints: Hints::default(), ..element.clone() };
    Ok(Some(Piece { element: piece, from: routed.element, under: false }))
}

/// A running stitch along `points` of the element `routed` comes from, `under` a column's satin, its id
/// numbered `number`, a point that repeats the one before to within a millionth of a millimetre left out, as
/// Ink/Stitch rounds a run's points; none without points.
pub(super) fn run(routed: &Routable, elements: &[Element], points: &[Point], under: bool, number: usize) -> Option<Piece> {
    let element = elements.get(routed.element)?;
    let Shape::Stroke { width, join, cap, .. } = &element.shape else { return None };
    let mut params = ParamSet::new();
    if let Some(length) = RUN_LENGTHS.iter().find_map(|key| element.params.get(key)) {
        params.set(RUN_LENGTH, length);
    }
    for key in [MIN_JUMP_STITCH_LENGTH, BEAN_STITCH_REPEATS, UNDERLAYS_FIRST] {
        if let Some(value) = element.params.get(key) {
            params.set(key, value);
        }
    }
    let mut kept: Vec<Point> = Vec::with_capacity(points.len());
    for &point in points {
        if kept.last().is_none_or(|last| last.distance(point) >= SAME_POINT) {
            kept.push(point);
        }
    }
    let (&start, rest) = kept.split_first()?;
    let path = Path { subpaths: vec![Subpath { start, segments: rest.iter().map(|&point| Segment::Line(point)).collect(), closed: false }] };
    let shape = Shape::Stroke { path, width: *width, join: *join, cap: *cap };
    let id = numbered(&element.id, number)?;
    let run = Element { id, name: None, shape, params, automatic: false, hints: Hints::default(), clips: Vec::new(), ..element.clone() };
    Some(Piece { element: run, from: routed.element, under })
}

/// Whether the jump between the places `ends` of `graph` runs longer than [`TRIMMED`] outside the routed
/// elements whose pieces end there.
fn long(graph: &Graph, routables: &[Routable], ends: [usize; 2], meter: &mut Meter) -> Result<bool, Exhausted> {
    let [Some(from), Some(to)] = ends.map(|place| graph.places().get(place)) else { return Ok(false) };
    let (Some(a), Some(b)) = (routables.get(from.element), routables.get(to.element)) else { return Ok(false) };
    Ok(trim::outside(from.at, to.at, [&a.outline, &b.outline], meter)? > TRIMMED)
}

/// Names `made`: AutoSatin and a number for a column's piece, AutoSatin Running Stitch and a number for a
/// run, counting from 1. With `preserve_order`, a column's piece whose element has a name keeps it, and
/// isn't counted.
fn named(made: &mut [Made], elements: &[Element], preserve_order: bool) {
    let mut number = 1;
    for Made { piece, satin } in made {
        let kept = elements.get(piece.from).and_then(|element| element.name.clone()).filter(|_| preserve_order && *satin);
        piece.element.name = Some(kept.unwrap_or_else(|| {
            let name = if *satin { format!("AutoSatin {number}") } else { format!("AutoSatin Running Stitch {number}") };
            number += 1;
            name
        }));
    }
}

/// `made` trimmed after each of `trims` and after the last, without the runs right after a trim, which
/// a trim makes needless.
fn trimmed(made: Vec<Made>, trims: &BTreeSet<usize>) -> Vec<Made> {
    let count = made.len();
    let mut kept: Vec<Made> = Vec::with_capacity(count);
    let mut just_trimmed = false;
    for (k, mut made) in made.into_iter().enumerate() {
        if just_trimmed && !made.satin {
            continue;
        }
        just_trimmed = trims.contains(&k);
        if just_trimmed || k + 1 == count {
            made.piece.element.params.set(TRIM_AFTER, "true");
        }
        kept.push(made);
    }
    kept
}

#[cfg(test)]
mod tests;
