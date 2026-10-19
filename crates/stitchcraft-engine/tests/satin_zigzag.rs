//! The zigzag satin's conformance cases (`REQ-SAT-026` to `REQ-SAT-028`): an open zigzag between a satin
//! column's rails, a point every half spacing on each rail in turn, over a contour underlay that stops where
//! it does, as Ink/Stitch sews its zigzag satin.
//!
//! The cases sew straight columns along x, the first rail at y = 0 and the second at the column's width,
//! with no short stitches unless a case says so. With a zigzag spacing of 2 mm the pairs lie 1 mm apart,
//! and the needle goes from (0, 0) on the first rail to (1, width) on the second, down to (2, 0), and on.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::hypot;
use stitchcraft_engine::generators::{Approach, Neighbours};
use stitchcraft_testkit::designs::{p, polylines};
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, ladder, near, sewn_between, sewn_ladder, sewn_satin, xy};

/// The needle points of a column `length` long and `width` wide, sewn as a zigzag satin with `params` and
/// no short stitches, which gives no warning.
fn zigzag(length: f64, width: f64, params: &[(&str, &str)]) -> Vec<(f64, f64)> {
    let mut params = params.to_vec();
    params.push(NO_SHORT_STITCHES);
    sewn_ladder(length, width, "zigzag", &params)
}

/// The points `at` millimetres along the stitch from `from` to `to`, in that order.
fn along(from: (f64, f64), to: (f64, f64), at: &[f64]) -> Vec<(f64, f64)> {
    let length = hypot(to.0 - from.0, to.1 - from.1);
    at.iter().map(|d| (from.0 + (to.0 - from.0) * d / length, from.1 + (to.1 - from.1) * d / length)).collect()
}

#[test]
fn req_sat_026_points_every_half_spacing_go_from_rail_to_rail_in_turn() {
    // A column 9 mm long has 10 pairs 1 mm apart: 5 stitches across, each to the next pair's point on the
    // second rail, and 4 back. One 10 mm long has 11, and the point left over, at (10, 0), is left out.
    let want = [(0.0, 0.0), (1.0, 4.0), (2.0, 0.0), (3.0, 4.0), (4.0, 0.0), (5.0, 4.0), (6.0, 0.0), (7.0, 4.0), (8.0, 0.0), (9.0, 4.0)];
    for length in [9.0, 10.0] {
        let points = zigzag(length, 4.0, &[("zigzag_spacing_mm", "2")]);
        assert!(near(&points, &want), "{length} mm: {points:?}");
    }
}

#[test]
fn req_sat_026_crowded_points_are_inset_along_their_own_stitch() {
    // Pairs 1.5 mm apart on a column 12 mm long and 4 mm wide: each rail's points lie 3 mm apart, closer
    // than 5 mm, so every other one is inset by 15 % of its stitch, which goes 1.5 mm along for its 4 mm
    // across. Each inset point moves that share of the way to its stitch's other end.
    let points = sewn_ladder(12.0, 4.0, "zigzag", &[("zigzag_spacing_mm", "3"), ("short_stitch_distance_mm", "5")]);
    let want = [(0.0, 0.0), (1.5, 4.0), (3.225, 0.6), (4.275, 3.4), (6.0, 0.0), (7.5, 4.0), (9.225, 0.6), (10.275, 3.4)];
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_027_a_stitch_splits_into_equal_parts_whichever_way_it_goes() {
    // Stitches 7.07 mm long with a longest stitch of 3 mm: 3 equal parts each, across and back.
    let points = zigzag(4.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3")]);
    let third = 7.0 / 3.0;
    let want = [
        (0.0, 0.0),
        (1.0 / 3.0, third),
        (2.0 / 3.0, 2.0 * third),
        (1.0, 7.0),
        (4.0 / 3.0, 2.0 * third),
        (5.0 / 3.0, third),
        (2.0, 0.0),
        (7.0 / 3.0, third),
        (8.0 / 3.0, 2.0 * third),
        (3.0, 7.0),
    ];
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_027_a_stitch_back_splits_into_its_own_fewest_parts() {
    // The second rail is 6 mm long, the first 10 mm: the stitches lean one way at the start and the other
    // at the end, and a stitch back is longer or shorter than the stitch across before it. With a longest
    // stitch of 1.5 mm each splits into its own fewest equal parts, which a satin column's stitch back
    // would take from the stitch across.
    let tapered = polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(2.0, 2.0), (8.0, 2.0)]]);
    let params = [("satin_method", "zigzag"), ("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "1.5"), NO_SHORT_STITCHES];
    let (points, warnings) = sewn_satin(&tapered, &params);
    assert!(warnings.is_empty(), "{warnings:?}");
    let points = xy(&points);
    // The stitches run from rail to rail: their ends are the points on the rails, and between them their
    // splits.
    let on_rail = |&(_, y): &(f64, f64)| y.abs() < 1e-9 || (y - 2.0).abs() < 1e-9;
    let ends: Vec<usize> = (0..points.len()).filter(|&i| on_rail(&points[i])).collect();
    let parts = |from: usize, to: usize| {
        let ((ax, ay), (bx, by)) = (points[from], points[to]);
        (hypot(bx - ax, by - ay) / 1.5).ceil().max(1.0)
    };
    let mut differ = false;
    for (n, pair) in ends.windows(2).enumerate() {
        let (from, to) = (pair[0], pair[1]);
        assert_eq!(
            f64::from(u32::try_from(to - from).unwrap()),
            parts(from, to),
            "stitch {n}, from {:?} to {:?}: {points:?}",
            points[from],
            points[to]
        );
        // A stitch back, from the second rail, beside the stitch across before it.
        if n > 0 && points[from].1 > 1.0 {
            differ |= parts(from, to) != parts(ends[n - 1], from);
        }
    }
    assert!(ends.len() > 8 && differ, "some stitch back splits otherwise than the stitch across before it: {points:?}");
}

#[test]
fn req_sat_027_stitches_split_by_the_staggered_method_take_the_column_s_numbers() {
    // Stitches 7.07 mm long with a longest stitch of 3 mm, split by the staggered method into its 4
    // staggers. The stitch across the first pair is the column's stitch 1, the stitch back to the second is
    // 2, the one across it 3: their splits start 1, 2 and 3 quarters of 3 mm from the first rail.
    let points = zigzag(4.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3"), ("split_method", "staggered")]);
    let mut want = vec![(0.0, 0.0)];
    want.extend(along((0.0, 0.0), (1.0, 7.0), &[0.75, 3.75, 6.75]));
    want.push((1.0, 7.0));
    let mut back = along((2.0, 0.0), (1.0, 7.0), &[1.5, 4.5]);
    back.reverse();
    want.extend(back);
    want.push((2.0, 0.0));
    want.extend(along((2.0, 0.0), (3.0, 7.0), &[2.25, 5.25]));
    want.push((3.0, 7.0));
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_026_after_an_odd_centre_walk_the_stitches_run_from_the_column_s_end() {
    let points = zigzag(4.0, 3.0, &[("zigzag_spacing_mm", "2"), ("center_walk_underlay", "true"), ("center_walk_underlay_repeats", "1")]);
    // The top stitches come last, sewn backwards, from the last pair's point on the second rail to the
    // first pair's on the first.
    let top = &points[points.len() - 4..];
    assert!(near(top, &[(3.0, 3.0), (2.0, 0.0), (1.0, 3.0), (0.0, 0.0)]), "{points:?}");
}

#[test]
fn req_sat_026_between_neighbours_each_point_is_sewn_once_but_the_one_at_the_cut() {
    // A needle to the left of the column and an element above it, 4 mm along: the column starts on the
    // line between its rails nearest the needle, and ends on its outline nearest the element, at (4, 4.2).
    // The end cuts the top stitches where they come nearest the pair 4 mm along, 0.195 mm from the zigzag's
    // point there, within the shortest stitch of it, so at that point. Both halves end there, and every
    // other point of the zigzag is sewn once. The point left over is not sewn at all.
    let neighbours = Neighbours { needle: Some(p(-3.0, 2.0)), next: Some(Approach::Point(p(4.0, 7.0))) };
    let params = [("satin_method", "zigzag"), ("zigzag_spacing_mm", "2"), ("pull_compensation_mm", "0.2"), NO_SHORT_STITCHES];
    let (points, warnings) = sewn_between(&ladder(10.0, 4.0, &[]), &params, &neighbours);
    assert!(warnings.is_empty(), "{warnings:?}");
    let points = xy(&points);
    assert!(near(&points[..1], &[(0.0, 2.0)]) && near(&points[points.len() - 3..], &[(4.0, -0.2), (4.0, 2.0), (4.0, 4.2)]), "{points:?}");
    let sewn = |x: f64, y: f64| points.iter().filter(|&&(px, py)| (px - x).abs() < 1e-9 && (py - y).abs() < 1e-9).count();
    for k in 0..5 {
        let x = 2.0 * f64::from(k);
        let at_the_cut = if k == 2 { 2 } else { 1 };
        assert_eq!((sewn(x, -0.2), sewn(x + 1.0, 4.2)), (at_the_cut, 1), "the points at {x} and {}: {points:?}", x + 1.0);
    }
    assert_eq!(sewn(10.0, -0.2), 0, "{points:?}");
}

#[test]
fn req_sat_026_the_column_ends_on_its_outline_through_the_pairs_at_the_full_spacing() {
    // A column 10.25 mm long: its pairs at the full spacing end with one at its end, 0.25 mm past the last
    // of the others, and the outline through them reaches it. The pairs at half the spacing, which the
    // zigzag sews, end 0.05 mm short of it, too near for a pair of their own. The element after the column
    // lies beyond its end, and the column ends at the end's first rail point.
    let neighbours = Neighbours { needle: None, next: Some(Approach::Point(p(20.0, -1.0))) };
    let (points, warnings) = sewn_between(&ladder(10.25, 4.0, &[]), &[("satin_method", "zigzag")], &neighbours);
    assert!(warnings.is_empty(), "{warnings:?}");
    let points = xy(&points);
    assert!(near(&points[points.len() - 1..], &[(10.25, 0.0)]), "{points:?}");
}

#[test]
fn req_sat_028_the_contour_underlay_stops_where_the_zigzag_does() {
    // Pairs 1 mm apart on a column 10 mm long and 4 mm wide: the zigzag's last point on the first rail is
    // at x = 8, its first and last on the second rail at x = 1 and x = 9. The contour's sides, 0.4 mm in
    // from the rails, are cut where those points meet them, then stop 0.4 mm short of each end: the first
    // from 0.4 to 7.6, the second from 1.4 to 8.6. After an odd centre walk the sides are sewn the other way
    // and stop at the same places (`DEV-SAT-011`).
    for walk in [&[][..], &[("center_walk_underlay", "true"), ("center_walk_underlay_repeats", "1")]] {
        let mut params = vec![("zigzag_spacing_mm", "2"), ("contour_underlay", "true")];
        params.extend_from_slice(walk);
        let points = zigzag(10.0, 4.0, &params);
        let side = |y: f64| -> (f64, f64) {
            let xs: Vec<f64> = points.iter().filter(|&&(_, py)| (py - y).abs() < 1e-9).map(|&(x, _)| x).collect();
            (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max))
        };
        let (first, second) = (side(0.4), side(3.6));
        assert!((first.0 - 0.4).abs() < 1e-9 && (first.1 - 7.6).abs() < 1e-9, "{walk:?}: first side {first:?}: {points:?}");
        assert!((second.0 - 1.4).abs() < 1e-9 && (second.1 - 8.6).abs() < 1e-9, "{walk:?}: second side {second:?}: {points:?}");
    }
}

#[test]
fn diag_sc_w0215_a_zigzag_satin_too_short_for_a_stitch_is_left_out() {
    // 0.05 mm long, the column has one pair at half the spacing: one point of the zigzag, and no stitch.
    let (points, warnings) = sewn_satin(&ladder(0.05, 4.0, &[]), &[("satin_method", "zigzag")]);
    assert_eq!(warnings, ["warning SC-W0215: This satin column is too short for a stitch across it, so it is not stitched."]);
    assert!(points.is_empty(), "{points:?}");
}

#[test]
fn req_sat_018_a_zigzag_satin_too_short_for_a_stitch_across_sews_no_underlay() {
    // The same column with a zigzag underlay, whose one pair would sew a stitch 3.6 mm across under no top
    // stitch: the column is left out, its underlay with it (`DEV-SAT-008`).
    let params = [("satin_method", "zigzag"), ("zigzag_underlay", "true")];
    let (points, warnings) = sewn_satin(&ladder(0.05, 4.0, &[]), &params);
    assert_eq!(warnings, ["warning SC-W0215: This satin column is too short for a stitch across it, so it is not stitched."]);
    assert!(points.is_empty(), "{points:?}");
}
