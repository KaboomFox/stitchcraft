//! Satin columns of 2 closed rails, rings (`REQ-SAT-019`, `DEV-SAT-009`), through the engine's satin
//! stitch. The second ring starts at its point nearest the first ring's start before the rails are turned,
//! so the stitches go straight across wherever each ring was drawn from and whichever way it turns.
//! VectorCraft's Make Compound Path, from 0.10.0, turns the front of 2 rings round, which starts it at
//! what was its last corner: a quarter of the way round a circle.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, and this extends it to the
// helpers here).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::f64::consts::SQRT_2;

use stitchcraft_core::Point;
use stitchcraft_testkit::designs::{p, polylines, rings};
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, across, sewn_satin};

/// A square ring's outer edge, 40 mm a side, from its top-left corner clockwise on screen (y down).
const OUTER: &[(f64, f64)] = &[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)];
/// Its inner edge, 4 mm in, drawn as the outer one is.
const INNER: &[(f64, f64)] = &[(4.0, 4.0), (36.0, 4.0), (36.0, 36.0), (4.0, 36.0)];
/// The inner edge as VectorCraft turns the front path of a compound path round: its corners in the
/// opposite order, from its last.
const TURNED: &[(f64, f64)] = &[(4.0, 36.0), (36.0, 36.0), (36.0, 4.0), (4.0, 4.0)];

/// How far a stitch straight across the ring goes at most: across a corner, from one edge's corner to
/// the other's.
const ACROSS_A_CORNER: f64 = 4.0 * SQRT_2;

/// The longest of the stitches across the column whose needle points are `points`.
fn longest_across(points: &[Point]) -> f64 {
    across(points).iter().map(|[a, b]| a.distance(*b)).fold(0.0, f64::max)
}

/// `ring` drawn back to its start: its corners and its first corner again.
fn drawn_back(ring: &[(f64, f64)]) -> Vec<(f64, f64)> {
    ring.iter().chain(ring.first()).copied().collect()
}

#[test]
fn req_sat_019_a_ring_turned_round_from_another_corner_sews_straight_across() {
    // Without rungs, the rings' corners pair up: each of the outer ring's with the inner one's beside it.
    let (points, warnings) = sewn_satin(&rings(&[OUTER, TURNED]), &[NO_SHORT_STITCHES]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let longest = longest_across(&points);
    assert!(longest <= ACROSS_A_CORNER + 1e-9, "a stitch {longest} mm long crosses the ring");
    assert_eq!(points[0], p(0.0, 0.0), "the column starts where its first ring does");
}

#[test]
fn req_sat_019_rings_closed_after_a_segment_back_to_their_start_sew_straight_across() {
    // VectorCraft closes a ring with a segment back to its start, and `Z`: the start is its last node
    // twice, and turned round, its first twice. Both rings closed so, or only one: the same nodes pair up.
    let (outer, turned) = (drawn_back(OUTER), drawn_back(TURNED));
    for pair in [[&outer[..], &turned], [&outer, TURNED], [OUTER, &turned]] {
        let (points, warnings) = sewn_satin(&rings(&pair), &[NO_SHORT_STITCHES]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let longest = longest_across(&points);
        assert!(longest <= ACROSS_A_CORNER + 1e-9, "a stitch {longest} mm long crosses the ring");
    }
}

#[test]
fn req_sat_019_rungs_cut_both_rings_from_the_same_place() {
    // A rung across the middle of each side. Each ring is cut at the rungs in order from its start, so 2
    // rings that start apart pair the wrong parts.
    let mut path = rings(&[OUTER, TURNED]);
    let rungs: &[&[(f64, f64)]] =
        &[&[(20.0, -1.0), (20.0, 5.0)], &[(35.0, 20.0), (41.0, 20.0)], &[(20.0, 35.0), (20.0, 41.0)], &[(-1.0, 20.0), (5.0, 20.0)]];
    path.subpaths.extend(polylines(rungs).subpaths);
    let (points, warnings) = sewn_satin(&path, &[NO_SHORT_STITCHES]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let longest = longest_across(&points);
    assert!(longest <= ACROSS_A_CORNER + 1e-9, "a stitch {longest} mm long crosses the ring");
}

#[test]
fn req_sat_019_rings_drawn_alike_sew_from_where_they_start() {
    let (points, warnings) = sewn_satin(&rings(&[OUTER, INNER]), &[NO_SHORT_STITCHES]);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(across(&points)[0], [p(0.0, 0.0), p(4.0, 4.0)]);
    assert!(longest_across(&points) <= ACROSS_A_CORNER + 1e-9);
}

#[test]
fn req_sat_019_only_2_rings_start_together() {
    // A ring and a rail that stops short of its start: the rail keeps its start, a corner along from the
    // ring's, and its stitches start there.
    let mut path = rings(&[OUTER]);
    path.subpaths.extend(polylines(&[TURNED]).subpaths);
    let (points, _) = sewn_satin(&path, &[NO_SHORT_STITCHES, ("reverse_rails", "none")]);
    assert_eq!(across(&points)[0], [p(0.0, 0.0), p(4.0, 36.0)]);
    // Drawn back to its start without `Z`, the rail is a ring, and starts at its corner nearest the other
    // ring's start.
    let mut path = rings(&[OUTER]);
    path.subpaths.extend(polylines(&[&drawn_back(TURNED)]).subpaths);
    let (points, _) = sewn_satin(&path, &[NO_SHORT_STITCHES, ("reverse_rails", "none")]);
    assert_eq!(across(&points)[0], [p(0.0, 0.0), p(4.0, 4.0)]);
}
