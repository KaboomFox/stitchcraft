//! Custom locks drawn as SVG paths (`REQ-LCK-005`, `REQ-LCK-006`), sewn as Ink/Stitch sews them.
//!
//! Design: `docs/src/design/algorithms/locks.md` › Drawn as a path. A path draws the lock in millimetres.
//! Each of its commands ends at a node, and the needle sews straight from node to node: a curve's or an
//! arc's end is a node, and a closepath's node is where its subpath started. The last segment gives the
//! direction. It starts at the last node but one, the anchor, which goes where the stitching starts or
//! ends, and the path is turned about the anchor until its last segment runs along the first stitch into
//! the stitching. A tie-in sews the nodes before the anchor in the order they are drawn, and a tie-off the
//! same lock backwards. Ink/Stitch's custom paths were read at `d59c9ab` (`lib/stitch_plan/lock_stitch.py`)
//! and run as a black box. Nothing of them is copied (ADR-0012).
//!
//! **Why the engine reads SVG's path syntax.** The setting's text is the lock in every host, in a
//! VectorCraft document's effect as in an SVG file's attribute, so the engine reads it, as it reads a
//! lock's numbers. It reads it with `svgtypes`' path-data parser, which the SVG adapter reads a path's `d`
//! with, so one parser reads SVG's path syntax for both. The engine uses the parser alone.
//!
//! **Untrusted text.** Reading stops at the first error, as an SVG viewer stops, and one node past
//! [`MOST_NODES`]. Every command costs a unit of work. What cannot be sewn is said (`SC-W0503`), and a path
//! that cannot be sewn at all leaves the half stitch to be sewn instead.

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, math};
use stitchcraft_plan::invariants::LOCK_MIN_STITCH;
use svgtypes::{PathParser, PathSegment};

use super::{End, LARGEST_SCALE_PERCENT, unusable};
use crate::design::WORKING_LIMIT_MM;
use crate::generators::mm;

/// The most nodes a lock drawn as a path may have. Ink/Stitch's own drawn locks have 9 at most, so 100
/// leave room for any lock, and keep a path from sewing a design of its own as a lock.
pub(crate) const MOST_NODES: usize = 100;

/// A path's nodes, read from its text: where each of its commands ends, in millimetres as drawn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Nodes {
    /// The nodes read, in order: at most one more than [`MOST_NODES`], which is enough to know there are
    /// too many.
    pub nodes: Vec<(f64, f64)>,
    /// The parser's words for the error that ended the reading, if one did.
    pub error: Option<String>,
}

/// `text`'s nodes, read as SVG reads path data, up to the first error. Every command costs a unit of
/// `meter`.
pub(crate) fn read(text: &str, meter: &mut Meter) -> Result<Nodes, Exhausted> {
    let (mut nodes, mut error) = (Vec::new(), None);
    // Where the pen is, and where its subpath started, which a closepath goes back to.
    let (mut pen, mut start) = ((0.0, 0.0), (0.0, 0.0));
    for item in PathParser::from(text) {
        meter.charge(1)?;
        match item {
            Ok(segment) => {
                pen = end_of(segment, pen, start);
                if matches!(segment, PathSegment::MoveTo { .. }) {
                    start = pen;
                }
                nodes.push(pen);
                if nodes.len() > MOST_NODES {
                    break;
                }
            }
            Err(e) => {
                error = Some(e.to_string());
                break;
            }
        }
    }
    Ok(Nodes { nodes, error })
}

/// Where `segment` ends, drawn from `pen` in a subpath that started at `start`.
fn end_of(segment: PathSegment, pen: (f64, f64), start: (f64, f64)) -> (f64, f64) {
    let at = |abs: bool, x: f64, y: f64| if abs { (x, y) } else { (pen.0 + x, pen.1 + y) };
    match segment {
        PathSegment::MoveTo { abs, x, y }
        | PathSegment::LineTo { abs, x, y }
        | PathSegment::CurveTo { abs, x, y, .. }
        | PathSegment::SmoothCurveTo { abs, x, y, .. }
        | PathSegment::Quadratic { abs, x, y, .. }
        | PathSegment::SmoothQuadratic { abs, x, y }
        | PathSegment::EllipticalArc { abs, x, y, .. } => at(abs, x, y),
        PathSegment::HorizontalLineTo { abs, x } => (if abs { x } else { pen.0 + x }, pen.1),
        PathSegment::VerticalLineTo { abs, y } => (pen.0, if abs { y } else { pen.1 + y }),
        PathSegment::ClosePath { .. } => start,
    }
}

/// The lock `read` draws, sewn at `end`: its needle points in the frame at the anchor, in millimetres at
/// 100 %, in the order the needle sews them, the anchor among them, last at a tie-in and first at a tie-off.
/// `None` when the path cannot be sewn, and the half stitch is sewn instead. `warnings` says why, and what
/// was left out.
pub(super) fn placed(read: &Nodes, end: End, warnings: &mut Vec<Diagnostic>) -> Option<Vec<(f64, f64)>> {
    let name = end.name();
    if let Some(error) = &read.error {
        warnings.push(Diagnostic::new(
            Code::CustomLockUnusable,
            format!("The custom {name} lock's path has an error ({error}), so the rest of it is left out."),
        ));
    }
    let nodes = read.nodes.as_slice();
    if nodes.len() > MOST_NODES {
        warnings.push(unusable(end, &format!("is a path of more than {MOST_NODES} nodes")));
        return None;
    }
    let (before, anchor, toward) = match nodes {
        [before @ .., anchor, toward] if !before.is_empty() => (before, *anchor, *toward),
        _ => {
            warnings.push(unusable(end, "is a path of fewer than 3 nodes"));
            return None;
        }
    };
    // Every node within the data model's reach of the anchor, along each axis: then every coordinate below is
    // finite, however far the path lies from its own origin.
    let near = |v: f64| v.abs() <= WORKING_LIMIT_MM;
    if !nodes.iter().all(|&(x, y)| near(x - anchor.0) && near(y - anchor.1)) {
        warnings.push(unusable(end, "is a path that reaches farther than 10 m from its last node but one"));
        return None;
    }
    let (dx, dy) = (toward.0 - anchor.0, toward.1 - anchor.1);
    let length = math::hypot(dx, dy);
    if at_least(0.0, length) {
        warnings.push(unusable(end, "is a path whose last segment, which would give its direction, has no length"));
        return None;
    }
    // The frame: x along the last segment, y a quarter turn from it, as the lock's frame turns.
    let (ux, uy) = (dx / length, dy / length);
    let framed = |&(x, y): &(f64, f64)| {
        let (vx, vy) = (x - anchor.0, y - anchor.1);
        (vx * ux + vy * uy, vy * ux - vx * uy)
    };
    // A tie-off's order: from the anchor, through the nodes before it backwards. A node where the one sewn
    // before it is would sew in place.
    let mut points: Vec<(f64, f64)> = std::iter::once((0.0, 0.0)).chain(before.iter().rev().map(framed)).collect();
    let drawn = points.len();
    points.dedup_by(|later, kept| at_least(0.0, math::hypot(later.0 - kept.0, later.1 - kept.1)));
    match drawn - points.len() {
        0 => {}
        1 => warnings.push(Diagnostic::new(
            Code::CustomLockUnusable,
            format!("A node of the custom {name} lock's path is where the node before it is, so it is left out."),
        )),
        n => warnings.push(Diagnostic::new(
            Code::CustomLockUnusable,
            format!("{n} nodes of the custom {name} lock's path are where the nodes before them are, so they are left out."),
        )),
    }
    if points.len() < 2 {
        warnings.push(unusable(end, "is a path with no stitch to sew"));
        return None;
    }
    // Enlarged, a drawn lock's shortest stitch is 0.2 mm, but no further than the largest scale.
    let shortest = points.iter().zip(points.iter().skip(1)).map(|(a, b)| math::hypot(b.0 - a.0, b.1 - a.1)).fold(f64::INFINITY, f64::min);
    let largest = shortest * LARGEST_SCALE_PERCENT / 100.0;
    if !at_least(largest, LOCK_MIN_STITCH.get()) {
        let why = format!(
            "is a path with a stitch of {} mm even at {} %, the largest scale, shorter than {} mm",
            mm(largest),
            mm(LARGEST_SCALE_PERCENT),
            mm(LOCK_MIN_STITCH.get())
        );
        warnings.push(unusable(end, &why));
        return None;
    }
    if end == End::Start {
        points.reverse();
    }
    Some(points)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn nodes(text: &str) -> Nodes {
        read(text, &mut Budget::DEFAULT.meter()).unwrap()
    }

    /// The nodes of `text`, which reads without an error.
    fn read_whole(text: &str) -> Vec<(f64, f64)> {
        let read = nodes(text);
        assert_eq!(read.error, None, "{text}");
        read.nodes
    }

    /// `text`'s lock at `end`, and what was said of it.
    fn lock(text: &str, end: End) -> (Option<Vec<(f64, f64)>>, Vec<String>) {
        let mut warnings = Vec::new();
        let points = placed(&nodes(text), end, &mut warnings);
        assert!(warnings.iter().all(|w| w.code == Code::CustomLockUnusable && w.at.is_none()), "{warnings:?}");
        (points, warnings.iter().map(|w| w.message.clone()).collect())
    }

    /// `points` are `want`, each to within a nanometre.
    fn near(points: &[(f64, f64)], want: &[(f64, f64)]) {
        assert_eq!(points.len(), want.len(), "{points:?}");
        assert!(points.iter().zip(want).all(|(a, b)| (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9), "{points:?}");
    }

    #[test]
    fn every_command_ends_at_a_node() {
        // Absolute, each from (1, 1): a line, across, down, a curve, a smooth curve, a quadratic one, a smooth
        // one and an arc, each ending where its last pair says.
        let absolute = "M 1,1 L 2,1 H 3 V 2 C 9,9 9,9 4,2 S 9,9 5,2 Q 9,9 6,2 T 7,2 A 1,1 0 0 1 8,2";
        assert_eq!(
            read_whole(absolute),
            [(1.0, 1.0), (2.0, 1.0), (3.0, 1.0), (3.0, 2.0), (4.0, 2.0), (5.0, 2.0), (6.0, 2.0), (7.0, 2.0), (8.0, 2.0)]
        );
        // The same relative to the node before, the first move relative to (0, 0).
        let relative = "m 1,1 l 1,0 h 1 v 1 c 9,9 9,9 1,0 s 9,9 1,0 q 9,9 1,0 t 1,0 a 1,1 0 0 1 1,0";
        assert_eq!(read_whole(relative), read_whole(absolute));
        // Pairs after a move are lines, and arc flags may be written together, as SVG says.
        assert_eq!(read_whole("m 1,2 3,4 5,6"), [(1.0, 2.0), (4.0, 6.0), (9.0, 12.0)]);
        assert_eq!(read_whole("M 0,0 A 1 1 0 01 2,0"), [(0.0, 0.0), (2.0, 0.0)]);
        // Across and down keep the other coordinate of the node before, whatever it is.
        assert_eq!(read_whole("M 2,3 h 4 v 5 H 1 V 2"), [(2.0, 3.0), (6.0, 3.0), (6.0, 8.0), (1.0, 8.0), (1.0, 2.0)]);
    }

    #[test]
    fn a_closepath_ends_where_its_subpath_started_and_the_next_command_starts_there() {
        assert_eq!(read_whole("M 1,1 L 3,1 L 3,3 z l 1,0"), [(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 1.0), (2.0, 1.0)]);
        // A move starts a subpath, and a closepath goes back to the latest one.
        assert_eq!(read_whole("M 1,1 L 2,2 m 3,0 l 0,1 Z"), [(1.0, 1.0), (2.0, 2.0), (5.0, 2.0), (5.0, 3.0), (5.0, 2.0)]);
    }

    #[test]
    fn reading_stops_at_the_first_error() {
        let read = nodes("M 0,0 L 1,1 X 2,2");
        assert_eq!((read.nodes, read.error.as_deref()), (vec![(0.0, 0.0), (1.0, 1.0)], Some("unexpected data at position 13")));
        // A path starts with a move, and a number after a closepath is an error.
        assert_eq!(nodes("L 1,1 2,2"), Nodes { nodes: vec![], error: Some("unexpected data at position 1".to_string()) });
        assert_eq!(nodes("M 0,0 Z 1,1").error.as_deref(), Some("unexpected data at position 9"));
        // So is a number too large to be finite.
        assert_eq!(nodes("M 0,0 L 1e999,0").error.as_deref(), Some("invalid number at position 9"));
    }

    #[test]
    fn reading_stops_one_node_past_the_most_and_the_budget_bounds_it() {
        let path = |count: usize| format!("M 0,0{}", " h 1".repeat(count - 1));
        assert_eq!(nodes(&path(MOST_NODES)).nodes.len(), MOST_NODES);
        // 200 nodes cost 101 units of work: one past the most is enough to know there are too many.
        let many = read(&path(200), &mut Budget { max_stitches: 1, max_work: 101 }.meter()).unwrap();
        assert_eq!((many.nodes.len(), many.error), (MOST_NODES + 1, None));
        assert_eq!(read(&path(3), &mut Budget { max_stitches: 1, max_work: 2 }.meter()), Err(Exhausted::Work));
    }

    #[test]
    fn a_tie_in_sews_the_nodes_before_the_anchor_and_a_tie_off_the_same_backwards() {
        // The anchor is (1, 0), and the last segment runs along x.
        let square = "M 0,0 L 0,1 L 1,1 L 1,0 L 2,0";
        let (start, said) = lock(square, End::Start);
        near(&start.unwrap(), &[(-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
        assert!(said.is_empty());
        let (end, _) = lock(square, End::End);
        near(&end.unwrap(), &[(0.0, 0.0), (0.0, 1.0), (-1.0, 1.0), (-1.0, 0.0)]);
    }

    #[test]
    fn the_last_segment_turns_the_path_into_the_frame() {
        // Its last segment runs down: down is the frame's x axis, and right a quarter turn back from it.
        let (points, _) = lock("M 0,0 L 1,0 L 1,1 L 0,1 L 0,2", End::Start);
        near(&points.unwrap(), &[(-1.0, 0.0), (-1.0, -1.0), (0.0, -1.0), (0.0, 0.0)]);
        // Slanted, 3-4-5: a node 5 mm back along the last segment, and one 5 mm to its side.
        let (points, _) = lock("M -3,-4 L 4,-3 L 0,0 L 3,4", End::Start);
        near(&points.unwrap(), &[(-5.0, 0.0), (0.0, -5.0), (0.0, 0.0)]);
        // Far from the path's own origin, only the distances count.
        let (points, _) = lock("M 6000,6000 L 6001,6000 L 6002,6000", End::End);
        near(&points.unwrap(), &[(0.0, 0.0), (-1.0, 0.0)]);
    }

    #[test]
    fn a_path_of_3_to_100_nodes_is_sewn() {
        let path = |count: usize| format!("M 0,0{}", " h 1".repeat(count - 1));
        let failed = |text: &str| lock(text, End::Start).0.is_none();
        assert!(!failed(&path(3)) && !failed(&path(MOST_NODES)));
        assert_eq!(
            lock(&path(2), End::Start),
            (None, vec!["The custom start lock is a path of fewer than 3 nodes, so the half stitch is sewn instead.".into()])
        );
        assert_eq!(
            lock(&path(MOST_NODES + 1), End::End),
            (None, vec!["The custom end lock is a path of more than 100 nodes, so the half stitch is sewn instead.".into()])
        );
        assert!(failed("M 0,0") && failed(""));
    }

    #[test]
    fn a_path_with_an_error_sews_what_comes_before_it() {
        let (points, said) = lock("M 0,0 L 0,1 L 1,1 L 1,0 L 2,0 L", End::Start);
        near(&points.unwrap(), &[(-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
        assert_eq!(said, ["The custom start lock's path has an error (unexpected end of stream), so the rest of it is left out."]);
        // Nothing before the error: too few nodes, said too.
        let (points, said) = lock("1\t-1", End::End);
        assert_eq!(points, None);
        assert_eq!(
            said,
            [
                "The custom end lock's path has an error (unexpected data at position 1), so the rest of it is left out.",
                "The custom end lock is a path of fewer than 3 nodes, so the half stitch is sewn instead."
            ]
        );
    }

    #[test]
    fn a_path_within_10_m_of_its_anchor_is_sewn() {
        let reach = |x: f64| lock(&format!("M {x},0 L 0,0 L 1,0"), End::Start);
        assert_eq!(reach(-10_000.0).0.map(|points| points.len()), Some(2));
        assert_eq!(reach(10_000.0).0.map(|points| points.len()), Some(2));
        let farther =
            "The custom start lock is a path that reaches farther than 10 m from its last node but one, so the half stitch is sewn instead.";
        assert_eq!(reach(10_000.001), (None, vec![farther.to_string()]));
        assert_eq!(reach(-10_000.001), (None, vec![farther.to_string()]));
        // Along y too, and the last node counts.
        assert_eq!(lock("M 0,10000.001 L 0,0 L 1,0", End::Start).1, [farther]);
        assert_eq!(lock("M 0,-10000 L 0,0 L 1,0", End::Start).1, Vec::<String>::new());
        assert_eq!(lock("M 1,0 L 0,0 L 0,10000.001", End::Start).1, [farther]);
        // Nodes beyond any finite distance are farther too.
        assert_eq!(lock("M 1,0 L 0,0 l 1e308,0 l 1e308,0", End::Start).0, None);
    }

    #[test]
    fn the_last_segment_needs_a_length_for_the_direction() {
        let no_direction =
            "The custom start lock is a path whose last segment, which would give its direction, has no length, so the half stitch is sewn instead.";
        assert_eq!(lock("M 0,1 L 1,0 L 1,0", End::Start), (None, vec![no_direction.to_string()]));
        // Half a billionth of a millimetre is no length, and 2 billionths are.
        assert_eq!(lock("M 0,1 L 1,0 l 0.0000000005,0", End::Start).0, None);
        let (points, _) = lock("M 0,1 L 1,0 l 0,0.000000002", End::Start);
        near(&points.unwrap(), &[(1.0, 1.0), (0.0, 0.0)]);
    }

    #[test]
    fn a_node_where_the_node_before_it_is_is_left_out() {
        let (points, said) = lock("M 0,0 L 0,1 L 0,1 L 1,1 L 1,0 L 2,0", End::Start);
        near(&points.unwrap(), &[(-1.0, 0.0), (-1.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
        assert_eq!(said, ["A node of the custom start lock's path is where the node before it is, so it is left out."]);
        // Within a billionth of a millimetre, and at the anchor too.
        let (points, said) = lock("M 0,1 L 0,1.0000000005 L 1,1 L 1,0 L 1,0 L 2,0", End::End);
        near(&points.unwrap(), &[(0.0, 0.0), (0.0, 1.0), (-1.0, 1.0)]);
        assert_eq!(said, ["2 nodes of the custom end lock's path are where the nodes before them are, so they are left out."]);
        // A little farther apart, they are 2 nodes, too near each other to sew.
        assert_eq!(
            lock("M 0,1 L 0,1.000000002 L 1,1 L 1,0 L 2,0", End::End).1,
            [
                "The custom end lock is a path with a stitch of 0 mm even at 500 %, the largest scale, shorter than 0.2 mm, so the half stitch is sewn instead."
            ]
        );
        // With none left before the anchor, there is no stitch to sew.
        assert_eq!(
            lock("M 1,0 L 1,0 L 2,0", End::Start).1,
            [
                "A node of the custom start lock's path is where the node before it is, so it is left out.",
                "The custom start lock is a path with no stitch to sew, so the half stitch is sewn instead."
            ]
        );
    }

    #[test]
    fn a_path_needs_stitches_of_0_2_mm_at_500_percent() {
        // 0.04 mm is 0.2 mm at 500 %, the largest scale, and a stitch shorter than that is too short to sew.
        assert_eq!(lock("M 0,0 L 0.04,0 L 1,0 L 2,0", End::Start).0.map(|points| points.len()), Some(3));
        assert_eq!(lock("M 0,0 L 1,0 L 1.04,0 L 2,0", End::End).0.map(|points| points.len()), Some(3));
        let short = "The custom start lock is a path with a stitch of 0.19 mm even at 500 %, the largest scale, shorter than 0.2 mm, so the half stitch is sewn instead.";
        assert_eq!(lock("M 0,0 L 0.038,0 L 1,0 L 2,0", End::Start), (None, vec![short.to_string()]));
        // The stitch into the anchor counts.
        assert_eq!(lock("M 0,0 L 0.962,0 L 1,0 L 2,0", End::Start).1, [short]);
    }
}
