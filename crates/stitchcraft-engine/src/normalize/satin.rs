//! A satin column's path as its two rails and its rungs (`docs/src/design/algorithms/satin.md` ›
//! Recognizing rails and rungs).
//!
//! A satin column is drawn as one path of several subpaths: two rails, the column's edges, and any number
//! of rungs, short lines across both rails that say which point of one rail goes with which point of the
//! other. The path does not say which subpath is which, so the roles come from where the subpaths meet:
//! a rung meets the two rails, and a rail meets every rung. The rule is Ink/Stitch's, so that a file sews
//! the same in both:
//!
//! - With three subpaths (one rung), the rails are the two that meet exactly one other subpath.
//! - With four or more, the rails are the two that meet more than two others.
//! - When that does not give exactly two, the two longest subpaths are the rails, and `SC-W0202` says so.
//!   Two rails that do not meet, with exactly two rungs, always come to this: each of the four meets two
//!   others, as the strokes of a `#` do. Subpaths shorter than [`MIN_RAIL`] are never rails by the first
//!   two rules.
//!
//! Rails may meet each other, as the two sides of a pointed column do at its tips. A path of two subpaths
//! is two rails without rungs, whose nodes pair up instead, and a path of one is the column's centre line
//! (Ink/Stitch's simple satin, whose width is the stroke's).
//!
//! Each rung gives a pair of points, one on each rail, where it crosses them. Where a rung misses a rail,
//! the point of that rail nearest the rung stands in (`SC-W0203`), as in Ink/Stitch. A rung that crosses
//! a rail more than once is left out (`SC-W0207`): it does not say which crossing is meant.
//!
//! Each subpath is flattened to a polyline within [`TOLERANCE`], and two subpaths meet where they share a
//! point, crossing or touching. A point where segments meet end to end is found twice and counted once, so
//! a rung that ends exactly on a rail meets it once. Work is charged to the meter: one unit per pair of
//! segments looked at.

use std::collections::BTreeMap;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::design::{Path, Subpath};
use crate::normalize::stroke::{self, nearest_on_segment, segments};

/// How closely the polylines follow the subpaths, in millimetres: far finer than a stitch, so subpaths
/// meet where the drawing shows them meeting.
pub const TOLERANCE: f64 = 0.01;

/// The shortest subpath the meeting rule takes as a rail, in millimetres: a tenth of a CSS pixel,
/// Ink/Stitch's limit.
pub const MIN_RAIL: f64 = 0.1 * MM_PER_SVG_PX;

/// Two points closer than this, in millimetres, are one point.
const SAME_POINT: f64 = 1e-6;

/// How far past either end of a segment, as a fraction of its length, a point still lies on it. Where two
/// segments end at one node, as the rails of a pointed column do at its tip, rounding can put the
/// computed meeting a hair beyond the end of either.
const ON_SEGMENT: f64 = 1e-9;

/// A satin column's rails, and what pairs their points.
#[derive(Clone, Debug, PartialEq)]
pub struct Satin {
    /// The two rails as polylines, in the order the path draws them.
    pub rails: [Vec<Point>; 2],
    /// What says which point of one rail goes with which point of the other.
    pub pairing: Pairing,
}

/// What pairs the points of a satin column's rails.
#[derive(Clone, Debug, PartialEq)]
pub enum Pairing {
    /// The rungs, in the order the path draws them: the point on the first rail and the point on the
    /// second rail that each says go together. Where a rung misses a rail, it is the point of the rail
    /// nearest the rung. Empty when every rung drawn was left out.
    Rungs(Vec<[Point; 2]>),
    /// The path draws no rungs, and the rails' nodes pair up instead, as in Ink/Stitch: each rail's start,
    /// the end of each of its segments and, when it is closed, its start again.
    Nodes([Vec<Point>; 2]),
}

/// What a satin column's path is.
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// Rails and rungs.
    Rails(Satin),
    /// One subpath: the column's centre line, its width the stroke's (Ink/Stitch's simple satin), from which
    /// its rails and rungs are made (`normalize::centre_line`).
    CentreLine {
        /// The line, flattened.
        line: Vec<Point>,
        /// Whether the path ends by closing its last subpath (`Z`), which Ink/Stitch takes as a closed line.
        closed: bool,
    },
}

/// What recognizing a satin column gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Recognition {
    /// What the path is, or why it is not a satin column (`SC-E0201`).
    pub shape: Result<Shape, Diagnostic>,
    /// What was taken by length, stood in for or left out (`SC-W0202`, `SC-W0203`, `SC-W0205`,
    /// `SC-W0207`).
    pub warnings: Vec<Diagnostic>,
}

/// One subpath, flattened.
struct Line {
    /// Its number in the path, from 1.
    number: usize,
    points: Vec<Point>,
    /// Its nodes, as `Pairing::Nodes` has them.
    nodes: Vec<Point>,
    length: f64,
    /// Its bounding box: the least and the greatest x and y.
    bounds: [f64; 4],
}

/// `path`, a satin column's, as its rails and rungs, or why it cannot be one.
pub fn recognize(path: &Path, meter: &mut Meter) -> Result<Recognition, Exhausted> {
    let flat = stroke::flatten(path, TOLERANCE, meter)?;
    let mut warnings = Vec::new();
    let mut lines = Vec::new();
    for (index, (subpath, piece)) in path.subpaths.iter().zip(flat.pieces).enumerate() {
        let number = index + 1;
        if piece.points.len() < 2 {
            let message = format!("Subpath {number} of this satin column is one point, so it is left out.");
            warnings.push(Diagnostic::new(Code::SatinSubpathPoint, message));
            continue;
        }
        let length = piece.points.windows(2).map(|pair| pair[0].distance(pair[1])).sum();
        lines.push(Line { number, bounds: bounds(&piece.points), points: piece.points, nodes: nodes(subpath), length });
    }
    let shape = match lines.as_slice() {
        [] => Err(Diagnostic::new(Code::SatinWithoutRails, "This satin column has no subpath longer than a point, so it has no rails.")
            .with_fix(Fix::Hint("Draw the column as two rails, its edges, with rungs across both.".to_string()))),
        [one] => Ok(Shape::CentreLine { line: one.points.clone(), closed: path.subpaths.last().is_some_and(|s| s.closed) }),
        [a, b] => {
            Ok(Shape::Rails(Satin { rails: [a.points.clone(), b.points.clone()], pairing: Pairing::Nodes([a.nodes.clone(), b.nodes.clone()]) }))
        }
        _ => Ok(Shape::Rails(rails_and_rungs(&lines, &mut warnings, meter)?)),
    };
    Ok(Recognition { shape, warnings })
}

/// The rails and rungs of three or more subpaths.
fn rails_and_rungs(lines: &[Line], warnings: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<Satin, Exhausted> {
    // Where each pair of subpaths meets, by their positions in `lines`, the lesser first.
    let mut shared: BTreeMap<(usize, usize), Vec<Point>> = BTreeMap::new();
    for (i, a) in lines.iter().enumerate() {
        for (j, b) in lines.iter().enumerate().skip(i + 1) {
            shared.insert((i, j), meeting(a, b, meter)?);
        }
    }
    let at = |i: usize, j: usize| shared.get(&(i.min(j), i.max(j))).map_or(&[][..], Vec::as_slice);
    let meets = |i: usize| (0..lines.len()).filter(|&j| j != i && !at(i, j).is_empty()).count();
    let rule = |i: usize| if lines.len() == 3 { meets(i) == 1 } else { meets(i) > 2 };
    let mut rails: Vec<usize> = (0..lines.len()).filter(|&i| rule(i) && length(lines, i) > MIN_RAIL).collect();
    if rails.len() != 2 {
        // The two longest; of equally long ones, the first drawn.
        let mut by_length: Vec<usize> = (0..lines.len()).collect();
        by_length.sort_by(|&i, &j| length(lines, j).total_cmp(&length(lines, i)));
        rails = by_length.into_iter().take(2).collect();
        rails.sort_unstable();
        let [a, b] = [0, 1].map(|k| line(lines, rails.get(k).copied().unwrap_or_default()).number);
        warnings.push(
            Diagnostic::new(
                Code::SatinRailsByLength,
                format!(
                    "Where the subpaths of this satin column meet does not say which are its rails, so its two longest, \
                     subpaths {a} and {b}, are taken as the rails."
                ),
            )
            .with_fix(Fix::Hint("If they are not the rails, use three rungs or more, each crossing both rails once.".to_string())),
        );
    }
    let (first, second) = (rails.first().copied().unwrap_or_default(), rails.get(1).copied().unwrap_or_default());
    let [rail_a, rail_b] = [line(lines, first), line(lines, second)];
    let mut rungs = Vec::new();
    for (k, rung) in lines.iter().enumerate().filter(|(k, _)| *k != first && *k != second) {
        let n = rung.number;
        match (at(k, first), at(k, second)) {
            (&[a], &[b]) => rungs.push([a, b]),
            (a, b) if a.len() > 1 || b.len() > 1 => {
                let rail = if a.len() > 1 { rail_a.number } else { rail_b.number };
                warnings.push(Diagnostic::new(
                    Code::SatinRungAmbiguous,
                    format!(
                        "Subpath {n} of this satin column, a rung, crosses the rail that is subpath {rail} more than once, so it does \
                         not say which points go together. It is left out."
                    ),
                ));
            }
            (a, b) => {
                let message = match (a.is_empty(), b.is_empty()) {
                    (true, true) => format!(
                        "Subpath {n} of this satin column, a rung, reaches neither rail (subpaths {} and {}), so the points of the \
                         rails nearest the rung are used.",
                        rail_a.number, rail_b.number
                    ),
                    (true, false) => missed(n, rail_a.number),
                    _ => missed(n, rail_b.number),
                };
                warnings.push(Diagnostic::new(Code::SatinRungDangling, message));
                let on_a = a.first().copied().map_or_else(|| nearest(rail_a, rung, meter), Ok)?;
                let on_b = b.first().copied().map_or_else(|| nearest(rail_b, rung, meter), Ok)?;
                rungs.push([on_a, on_b]);
            }
        }
    }
    Ok(Satin { rails: [rail_a.points.clone(), rail_b.points.clone()], pairing: Pairing::Rungs(rungs) })
}

/// `SC-W0203`'s message for the rung `rung`, which misses the rail `rail` (both subpath numbers).
fn missed(rung: usize, rail: usize) -> String {
    format!(
        "Subpath {rung} of this satin column, a rung, does not reach the rail that is subpath {rail}, so the point of that rail \
         nearest the rung is used."
    )
}

/// The length of the subpath at `i` in `lines`.
fn length(lines: &[Line], i: usize) -> f64 {
    line(lines, i).length
}

/// The subpath at `i` in `lines`, which has it.
fn line(lines: &[Line], i: usize) -> &Line {
    lines.get(i).unwrap_or(&EMPTY)
}

/// A subpath with no points, for a position that is not in the list.
static EMPTY: Line = Line { number: 0, points: Vec::new(), nodes: Vec::new(), length: 0.0, bounds: [0.0; 4] };

/// The nodes of `subpath`, as `Pairing::Nodes` has them.
fn nodes(subpath: &Subpath) -> Vec<Point> {
    let ends = subpath.segments.iter().map(|segment| segment.end());
    std::iter::once(subpath.start).chain(ends).chain(subpath.closed.then_some(subpath.start)).collect()
}

/// The points where `a` and `b` meet, each once.
fn meeting(a: &Line, b: &Line, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut found: Vec<Point> = Vec::new();
    if !overlap(a.bounds, b.bounds) {
        return Ok(found);
    }
    for (p1, p2) in segments(&a.points) {
        for (q1, q2) in segments(&b.points) {
            meter.charge(1)?;
            shared(p1, p2, q1, q2).into_iter().for_each(|point| add_once(&mut found, point));
        }
    }
    Ok(found)
}

/// Adds `point` to `found` unless one there is the same point: where segments meet end to end, both find
/// the point they share.
fn add_once(found: &mut Vec<Point>, point: Point) {
    if !found.iter().any(|f| f.distance(point) < SAME_POINT) {
        found.push(point);
    }
}

/// The point of `rail` nearest `rung`, which it does not meet.
fn nearest(rail: &Line, rung: &Line, meter: &mut Meter) -> Result<Point, Exhausted> {
    let mut best: Option<(f64, Point)> = None;
    for (p1, p2) in segments(&rung.points) {
        for (q1, q2) in segments(&rail.points) {
            meter.charge(1)?;
            // Segments that do not meet are nearest at an end of one of them.
            let ends = [(p1, nearest_on_segment(p1, q1, q2)), (p2, nearest_on_segment(p2, q1, q2))];
            let others = [(nearest_on_segment(q1, p1, p2), q1), (nearest_on_segment(q2, p1, p2), q2)];
            for (from, on) in ends.into_iter().chain(others) {
                let distance = from.distance(on);
                if best.is_none_or(|(d, _)| distance < d) {
                    best = Some((distance, on));
                }
            }
        }
    }
    Ok(best.map_or(Point::ORIGIN, |(_, on)| on))
}

/// The points the segments `p1`–`p2` and `q1`–`q2` share, ends included: none, the one where they cross
/// or touch, or the two ends of the part they share when they lie on one line.
fn shared(p1: Point, p2: Point, q1: Point, q2: Point) -> Vec<Point> {
    let (r, s) = ((p2.x() - p1.x(), p2.y() - p1.y()), (q2.x() - q1.x(), q2.y() - q1.y()));
    let qp = (q1.x() - p1.x(), q1.y() - p1.y());
    let cross = |u: (f64, f64), v: (f64, f64)| u.0 * v.1 - u.1 * v.0;
    let denominator = cross(r, s);
    if denominator != 0.0 {
        let (t, u) = (cross(qp, s) / denominator, cross(qp, r) / denominator);
        let on = |f: f64| (-ON_SEGMENT..=1.0 + ON_SEGMENT).contains(&f);
        return if on(t) && on(u) { vec![p1.lerp(p2, t)] } else { Vec::new() };
    }
    if cross(qp, r) != 0.0 {
        return Vec::new();
    }
    // On one line: the part of `q` within `p`, as fractions of `p`.
    let rr = r.0 * r.0 + r.1 * r.1;
    let along = |q: Point| ((q.x() - p1.x()) * r.0 + (q.y() - p1.y()) * r.1) / rr;
    let (t0, t1) = (along(q1), along(q2));
    let (start, end) = (t0.min(t1).max(0.0), t0.max(t1).min(1.0));
    if start > end { Vec::new() } else { vec![p1.lerp(p2, start), p1.lerp(p2, end)] }
}

/// The least and greatest x and y of `points`.
fn bounds(points: &[Point]) -> [f64; 4] {
    points.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |[x0, y0, x1, y1], p| {
        [x0.min(p.x()), y0.min(p.y()), x1.max(p.x()), y1.max(p.y())]
    })
}

/// Whether two bounding boxes share a point.
fn overlap(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn segments_share_their_crossing_their_touching_ends_or_their_overlap() {
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 2.0), p(0.0, 2.0), p(2.0, 0.0)), [p(1.0, 1.0)], "crossing");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0), p(1.0, 3.0)), [p(1.0, 0.0)], "one ends on the other");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(2.0, 0.0), p(2.0, 3.0)), [p(2.0, 0.0)], "end to end");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 1.0), p(1.0, 3.0)), Vec::<Point>::new(), "short of it");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(3.0, -1.0), p(3.0, 1.0)), Vec::<Point>::new(), "beyond its end");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(0.0, 1.0), p(2.0, 1.0)), Vec::<Point>::new(), "parallel");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(1.0, 0.0), p(3.0, 0.0)), [p(1.0, 0.0), p(2.0, 0.0)], "overlapping");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(3.0, 0.0), p(1.5, 0.0)), [p(1.5, 0.0), p(2.0, 0.0)], "overlapping backwards");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(3.0, 0.0), p(4.0, 0.0)), Vec::<Point>::new(), "on one line, apart");
        assert_eq!(shared(p(0.0, 0.0), p(2.0, 0.0), p(2.0, 0.0), p(4.0, 0.0)), [p(2.0, 0.0), p(2.0, 0.0)], "on one line, end to end");
    }

    #[test]
    fn segments_on_one_slanted_line_share_the_part_they_overlap() {
        // Along (3, 4) from (1, 1): the second segment starts halfway along the first and goes on past it.
        assert_eq!(shared(p(1.0, 1.0), p(4.0, 5.0), p(2.5, 3.0), p(7.0, 9.0)), [p(2.5, 3.0), p(4.0, 5.0)]);
        assert_eq!(shared(p(1.0, 1.0), p(4.0, 5.0), p(-2.0, -3.0), p(2.5, 3.0)), [p(1.0, 1.0), p(2.5, 3.0)]);
    }

    #[test]
    fn points_closer_than_a_millionth_of_a_millimetre_are_one() {
        let mut found = vec![p(0.0, 0.0)];
        add_once(&mut found, p(0.5e-6, 0.0));
        assert_eq!(found.len(), 1);
        add_once(&mut found, p(SAME_POINT, 0.0));
        assert_eq!(found, [p(0.0, 0.0), p(SAME_POINT, 0.0)], "exactly that far apart is two points");
    }

    #[test]
    fn a_meeting_a_hair_past_either_end_is_at_that_end() {
        // A crossing 1e-10 of the segment's length past its end is rounding, and lies on its end; 1e-8 past
        // is beyond it.
        let across = |x: f64| shared(p(0.0, 0.0), p(1.0, 0.0), p(x, -1.0), p(x, 1.0));
        assert_eq!(across(1.0 + 1e-10), [p(1.0, 0.0)]);
        assert_eq!(across(-1e-10), [p(0.0, 0.0)]);
        assert_eq!(across(1.0 + 1e-8), Vec::<Point>::new());
        assert_eq!(across(-1e-8), Vec::<Point>::new());
        // The same past the ends of the other segment.
        let reaching = |y: f64| shared(p(0.0, 0.0), p(1.0, 0.0), p(0.5, y - 1.0), p(0.5, y));
        assert_eq!(reaching(-1e-10), [p(0.5, 0.0)]);
        assert_eq!(reaching(-1e-8), Vec::<Point>::new());
        assert_eq!(reaching(1.0 + 1e-10), [p(0.5, 0.0)]);
        assert_eq!(reaching(1.0 + 1e-8), Vec::<Point>::new());
    }

    #[test]
    fn rails_that_end_at_one_node_meet_there_whatever_the_rounding() {
        // Both segments end at the node, the case whose arithmetic rounds; coordinates that are not
        // whole numbers, as after a transform. Without the allowance past the ends, some of these miss.
        let tip = p(20.0 * 0.264_583, 1.0 / 3.0);
        for (a, b) in [(p(0.1, 7.3), p(0.7, -5.9)), (p(-3.3, 0.37), p(1.11, 2.9)), (p(4.4, 4.1), p(0.3, -0.1))] {
            let found = shared(a, tip, b, tip);
            assert!(matches!(found.as_slice(), [point] if point.distance(tip) < 1e-12), "{a:?} {b:?}: {found:?}");
        }
    }

    #[test]
    fn boxes_overlap_when_they_share_a_point() {
        let unit = [0.0, 0.0, 1.0, 1.0];
        assert!(overlap(unit, [1.0, 1.0, 2.0, 2.0]), "a corner");
        assert!(overlap(unit, [0.5, -1.0, 0.6, 2.0]), "across");
        assert!(!overlap(unit, [1.5, 0.0, 2.0, 1.0]), "to the right");
        assert!(!overlap(unit, [-2.0, 0.0, -1.0, 1.0]), "to the left");
        assert!(!overlap(unit, [0.0, 1.5, 1.0, 2.0]), "below");
        assert!(!overlap(unit, [0.0, -2.0, 1.0, -1.0]), "above");
        assert_eq!(bounds(&[p(1.0, 5.0), p(-2.0, 3.0), p(0.0, 7.0)]), [-2.0, 3.0, 1.0, 7.0]);
    }

    #[test]
    fn a_subpath_s_nodes_are_its_start_and_its_segments_ends() {
        use crate::design::Segment;
        let open = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(10.0, 0.0)), Segment::Line(p(10.0, 0.0))], closed: false };
        assert_eq!(nodes(&open), [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.0)], "as drawn, a segment that does not move too");
        let curve = Segment::Cubic(p(12.0, 2.0), p(12.0, 8.0), p(10.0, 10.0));
        let closed = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(10.0, 0.0)), curve], closed: true };
        assert_eq!(nodes(&closed), [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 0.0)], "closed: back to the start");
    }

    #[test]
    fn a_position_out_of_the_list_is_an_empty_subpath() {
        assert!(line(&[], 3).points.is_empty());
        assert_eq!(length(&[], 3), 0.0);
    }

    #[test]
    fn the_nearest_point_of_a_rail_with_no_segments_is_the_origin() {
        let rung =
            Line { number: 1, points: vec![p(1.0, 1.0), p(2.0, 2.0)], nodes: Vec::new(), length: 2.0_f64.sqrt(), bounds: [1.0, 1.0, 2.0, 2.0] };
        assert_eq!(nearest(&EMPTY, &rung, &mut stitchcraft_core::Budget::DEFAULT.meter()).unwrap(), Point::ORIGIN);
    }
}
