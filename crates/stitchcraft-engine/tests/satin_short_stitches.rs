//! Short stitches' conformance cases (`REQ-SAT-009`): where a satin column's needle points crowd on a rail,
//! on the inside of a tight curve, some move in along their stitches, as in Ink/Stitch.
//!
//! Each case sews a column with short stitches and without, and checks the difference against the rule:
//! on each rail, a point closer than the distance to the last point left in place moves in by the next
//! inset, in turn, and any other point stays and is the one to measure from.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::Point;
use stitchcraft_testkit::designs::{RED, along, messages, p, planned, polylines};
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, across, ladder, quarter_ring, sewn_satin};

/// `plain`'s pairs as the rule insets them: on each rail, a point closer than `distance` to the last
/// point left in place moves toward the other end of its stitch by the next of `insets`, as a fraction
/// of the stitch's length.
fn ruled(plain: &[[Point; 2]], distance: f64, insets: &[f64]) -> Vec<[Point; 2]> {
    let mut rails = [(None::<Point>, 0_usize); 2];
    plain
        .iter()
        .map(|&pair| {
            let mut sewn = pair;
            for (side, (last, next)) in rails.iter_mut().enumerate() {
                let (point, other) = (pair[side], pair[1 - side]);
                if *next >= insets.len() {
                    *next = 0;
                }
                match last {
                    Some(at) if point.distance(*at) < distance => {
                        let t = insets[*next];
                        sewn[side] = p(point.x() + (other.x() - point.x()) * t, point.y() + (other.y() - point.y()) * t);
                        *next += 1;
                    }
                    _ => (*last, *next) = (Some(point), 0),
                }
            }
            sewn
        })
        .collect()
}

/// Whether `pairs` are `want`, to within rounding.
fn same(pairs: &[[Point; 2]], want: &[[Point; 2]]) -> bool {
    pairs.len() == want.len() && pairs.iter().zip(want).all(|(a, b)| a[0].distance(b[0]) < 1e-9 && a[1].distance(b[1]) < 1e-9)
}

#[test]
fn req_sat_009_on_a_tight_curve_crowded_inner_points_move_in() {
    // A quarter ring 4 mm wide around a 2 mm hole: the inner rail's points are a third as far apart as
    // the outer rail's, 0.13 mm, and every other one moves in by 15 % of its stitch, as by default.
    let ring = quarter_ring(6.0, 2.0);
    let plain = across(&sewn_satin(&ring, &[NO_SHORT_STITCHES]).0);
    let (points, warnings) = sewn_satin(&ring, &[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let sewn = across(&points);
    assert!(same(&sewn, &ruled(&plain, 0.25, &[0.15])), "{sewn:?}");
    let moved = sewn.iter().zip(&plain).filter(|(a, b)| a[1] != b[1]).count();
    assert!(moved * 3 > sewn.len(), "about every other one: {moved} of {}", sewn.len());
    // The outer rail's points are 0.4 mm apart and stay, all but the end pair's, which can be closer.
    let (_, inside) = sewn.split_last().unwrap();
    assert!(inside.iter().zip(&plain).all(|(a, b)| a[0] == b[0]), "{sewn:?}");
}

#[test]
fn req_sat_009_points_that_crowd_one_after_another_take_the_insets_in_turn() {
    // Around a 2 mm hole, 8 mm wide: the inner points are 0.08 mm apart, so 3 crowd after each one left
    // in place, and they take 10 %, 30 % and 10 %.
    let ring = quarter_ring(10.0, 2.0);
    let plain = across(&sewn_satin(&ring, &[NO_SHORT_STITCHES]).0);
    let sewn = across(&sewn_satin(&ring, &[("short_stitch_inset", "10 30")]).0);
    assert!(same(&sewn, &ruled(&plain, 0.25, &[0.1, 0.3])), "{sewn:?}");
    // A longer distance moves more of them.
    let far = across(&sewn_satin(&ring, &[("short_stitch_distance_mm", "0.5")]).0);
    assert!(same(&far, &ruled(&plain, 0.5, &[0.15])), "{far:?}");
}

#[test]
fn req_sat_009_points_far_enough_apart_and_a_distance_of_0_move_none() {
    // A straight column's points are 0.4 mm apart, more than the 0.25 mm distance.
    let straight = sewn_satin(&ladder(10.0, 4.0, &[]), &[]).0;
    assert_eq!(straight, sewn_satin(&ladder(10.0, 4.0, &[]), &[NO_SHORT_STITCHES]).0);
    let ring = quarter_ring(6.0, 2.0);
    assert_eq!(sewn_satin(&ring, &[("short_stitch_distance_mm", "0")]).0, sewn_satin(&ring, &[NO_SHORT_STITCHES]).0);
}

#[test]
fn req_sat_009_insets_are_taken_from_the_compensated_stitch() {
    // With 0.5 mm of pull compensation the stitches are 5 mm long, and a crowded point moves 15 % of that.
    let ring = quarter_ring(6.0, 2.0);
    let pulled = [("pull_compensation_mm", "0.5")];
    let plain = across(&sewn_satin(&ring, &[pulled[0], NO_SHORT_STITCHES]).0);
    let sewn = across(&sewn_satin(&ring, &pulled).0);
    assert!(same(&sewn, &ruled(&plain, 0.25, &[0.15])), "{sewn:?}");
}

#[test]
fn req_sat_009_ends_both_inset_by_half_meet_in_the_middle() {
    // A pointed column, whose points crowd within 1 mm: where both ends of a stitch move in by half of
    // it, they meet in the middle. The first pair, at the tip, is one point and stays.
    let lens = polylines(&[&[(0.0, 0.0), (5.0, -2.0), (10.0, 0.0)], &[(0.0, 0.0), (5.0, 2.0), (10.0, 0.0)]]);
    let plain = across(&sewn_satin(&lens, &[NO_SHORT_STITCHES]).0);
    let sewn = across(&sewn_satin(&lens, &[("short_stitch_inset", "50"), ("short_stitch_distance_mm", "1")]).0);
    assert!(same(&sewn, &ruled(&plain, 1.0, &[0.5])), "{sewn:?}");
    assert_eq!(sewn[0], plain[0], "the tip");
    let met = sewn.iter().filter(|[a, b]| a.distance(*b) < 1e-9 && a.x() > 0.0).count();
    assert!(met > 2, "{sewn:?}");
}

#[test]
fn req_sat_009_satins_get_short_stitches_by_default_as_in_ink_stitch() {
    let column = |params: &[(&str, &str)]| {
        let outcome = planned(vec![along("ring", quarter_ring(6.0, 2.0), &RED, params)]);
        let said = messages(&outcome);
        outcome.plan.unwrap_or_else(|| panic!("{said:?}")).stitches().map(|stitch| stitch.at).collect::<Vec<_>>()
    };
    assert_ne!(column(&[("satin_column", "true")]), column(&[("satin_column", "true"), NO_SHORT_STITCHES]));
}
