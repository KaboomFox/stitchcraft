//! The E-stitch's conformance cases (`REQ-SAT-020`, `REQ-SAT-021`): a satin column sewn as a spine along its
//! first rail, with a spike across the column at each pair, as Ink/Stitch sews its E stitch.
//!
//! The cases sew straight columns along x, the first rail at y = 0 and the second at the column's width,
//! with no short stitches. Each spike goes straight up from the spine and back down.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::Point;
use stitchcraft_engine::generators::{Approach, Neighbours};
use stitchcraft_testkit::designs::p;
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, ladder, sewn_between, sewn_satin};

/// The needle points of a column `length` long and `width` wide, sewn as an E-stitch with `params` and no
/// short stitches, which gives no warning.
fn e_stitch(length: f64, width: f64, params: &[(&str, &str)]) -> Vec<(f64, f64)> {
    let mut params = params.to_vec();
    params.push(NO_SHORT_STITCHES);
    sewn(length, width, &params)
}

/// The needle points of a column `length` long and `width` wide, sewn as an E-stitch with `params`, which
/// gives no warning.
fn sewn(length: f64, width: f64, params: &[(&str, &str)]) -> Vec<(f64, f64)> {
    let mut params = params.to_vec();
    params.push(("satin_method", "e_stitch"));
    let (points, warnings) = sewn_satin(&ladder(length, width, &[]), &params);
    assert!(warnings.is_empty(), "{warnings:?}");
    xy(&points)
}

fn xy(points: &[Point]) -> Vec<(f64, f64)> {
    points.iter().map(|q| (q.x(), q.y())).collect()
}

/// Whether `got` are the points `want`, but for rounding.
fn near(got: &[(f64, f64)], want: &[(f64, f64)]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(g, w)| (g.0 - w.0).abs() < 1e-9 && (g.1 - w.1).abs() < 1e-9)
}

/// A spike at `x` across a column `width` wide, split at `splits` from the spine: up, then back down.
fn spike(x: f64, width: f64, splits: &[f64]) -> Vec<(f64, f64)> {
    let up = splits.iter().map(|&y| (x, y));
    let down = splits.iter().rev().map(|&y| (x, y));
    [(x, 0.0)].into_iter().chain(up).chain([(x, width)]).chain(down).chain([(x, 0.0)]).collect()
}

/// The spine's splits from `from` to `to` along the first rail, at `at` from `from`.
fn spine(from: f64, at: &[f64]) -> Vec<(f64, f64)> {
    at.iter().map(|&d| (from + d, 0.0)).collect()
}

#[test]
fn req_sat_020_each_pair_is_a_spike_from_the_first_rail_and_back() {
    // Pairs every 2 mm along a column 10 mm long and 4 mm wide: a spike at each, and the spine between them
    // in one stitch.
    let points = e_stitch(10.0, 4.0, &[("zigzag_spacing_mm", "2")]);
    let want: Vec<(f64, f64)> = (0..=5).flat_map(|i| spike(2.0 * f64::from(i), 4.0, &[])).collect();
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_020_a_spike_splits_as_a_stitch_across_and_comes_back_through_its_splits() {
    // Spikes 7 mm long with a longest stitch of 3 mm: 3 equal parts, up and back down.
    let third = 7.0 / 3.0;
    let points = e_stitch(4.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3")]);
    let want: Vec<(f64, f64)> = [0.0, 2.0, 4.0].into_iter().flat_map(|x| spike(x, 7.0, &[third, 2.0 * third])).collect();
    assert!(near(&points, &want), "{points:?}");
    // Splits moved at random come back down through the same needle holes.
    let jittered = e_stitch(4.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3"), ("random_split_jitter_percent", "40")]);
    assert_eq!(jittered.len(), want.len());
    for one in jittered.chunks(7) {
        let back: Vec<(f64, f64)> = one.iter().rev().copied().collect();
        assert_eq!(one, back.as_slice(), "the way back is the way up reversed");
    }
    assert!(!near(&jittered, &want), "the splits moved");
}

#[test]
fn req_sat_020_spikes_split_by_the_staggered_method_take_the_rows_of_stitches_across() {
    // Spikes 7 mm long with a longest stitch of 3 mm, split by the staggered method. The n-th spike, from
    // 0, is a satin column's stitch across 2n + 1: its splits start a quarter of 3 mm from the spine, then
    // three quarters, and on in turn.
    let points = e_stitch(6.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3"), ("split_method", "staggered")]);
    let (quarter, three_quarters) = (&[0.75, 3.75, 6.75][..], &[2.25, 5.25][..]);
    let want: Vec<(f64, f64)> =
        [(0.0, quarter), (2.0, three_quarters), (4.0, quarter), (6.0, three_quarters)].into_iter().flat_map(|(x, at)| spike(x, 7.0, at)).collect();
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_020_swapped_rails_put_the_spine_on_the_other_rail() {
    let points = e_stitch(4.0, 3.0, &[("zigzag_spacing_mm", "2"), ("swap_satin_rails", "true")]);
    let want: Vec<(f64, f64)> = [0.0, 2.0, 4.0].into_iter().flat_map(|x| [(x, 3.0), (x, 0.0), (x, 3.0)]).collect();
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_020_after_an_odd_centre_walk_the_spikes_run_from_the_column_s_end() {
    let points = e_stitch(4.0, 3.0, &[("zigzag_spacing_mm", "2"), ("center_walk_underlay", "true"), ("center_walk_underlay_repeats", "1")]);
    // The top stitches come last, their first spike at the column's end and their last at its start.
    let top = &points[points.len() - 9..];
    let want: Vec<(f64, f64)> = [4.0, 2.0, 0.0].into_iter().flat_map(|x| spike(x, 3.0, &[])).collect();
    assert!(near(top, &want), "{points:?}");
}

#[test]
fn req_sat_021_where_the_spacing_is_longer_than_the_longest_stitch_the_spine_splits() {
    // Spikes 5 mm apart on a column 1 mm wide, with a longest stitch of 2 mm. The spikes are not split.
    let column = |method: &str, extra: &[(&str, &str)]| {
        let mut params = vec![("zigzag_spacing_mm", "5"), ("max_stitch_length_mm", "2"), ("split_method", method)];
        params.extend_from_slice(extra);
        e_stitch(10.0, 1.0, &params)
    };
    let with = |splits: &[f64]| -> Vec<(f64, f64)> {
        [spike(0.0, 1.0, &[]), spine(0.0, splits), spike(5.0, 1.0, &[]), spine(5.0, splits), spike(10.0, 1.0, &[])].concat()
    };
    // By default, into the fewest equal parts: 3 of 5/3 mm. Jitter and random phase leave the spine alone.
    let thirds = with(&[5.0 / 3.0, 10.0 / 3.0]);
    for extra in [&[][..], &[("random_split_jitter_percent", "40")], &[("random_split_phase", "true")]] {
        let points = column("default", extra);
        assert!(near(&points, &thirds), "{extra:?}: {points:?}");
    }
    // Simple and staggered splits fall at whole multiples of the longest stitch from the spine stitch's start.
    for method in ["simple", "staggered"] {
        let points = column(method, &[]);
        assert!(near(&points, &with(&[2.0, 4.0])), "{method}: {points:?}");
    }
}

#[test]
fn req_sat_021_the_spine_splits_from_a_spike_s_inset_point_to_the_next_pair_s_point() {
    // Pairs 3 mm apart on a column 4 mm wide with a longest stitch of 2 mm. Their points crowd closer than
    // 5 mm, and every other one is inset 15 % of the width, 0.6 mm. A spine stitch splits as one from the
    // spike's inset point to the next pair's point before its inset: halfway between those.
    let points = sewn(12.0, 4.0, &[("zigzag_spacing_mm", "3"), ("max_stitch_length_mm", "2"), ("short_stitch_distance_mm", "5")]);
    let inset = |x: f64, by: f64| vec![(x, by), (x, 2.0), (x, 4.0 - by), (x, 2.0), (x, by)];
    let want = [
        inset(0.0, 0.0),
        vec![(1.5, 0.0)],
        inset(3.0, 0.6),
        vec![(4.5, 0.3)],
        inset(6.0, 0.0),
        vec![(7.5, 0.0)],
        inset(9.0, 0.6),
        vec![(10.5, 0.3)],
        inset(12.0, 0.0),
    ]
    .concat();
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_021_the_spacing_as_set_decides_whether_the_spine_splits() {
    // A spacing of 2 mm and a longest stitch of 2 mm: random spacing makes some spine stitches longer than
    // 2 mm, and none of them splits. Each needle point is a spike's.
    let points = e_stitch(20.0, 1.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "2"), ("random_zigzag_spacing_percent", "30")]);
    let spikes: Vec<&[(f64, f64)]> = points.chunks(3).collect();
    assert!(spikes.iter().all(|s| s.len() == 3 && s[0] == s[2] && s[0].1.abs() < 1e-9 && (s[1].1 - 1.0).abs() < 1e-9), "{points:?}");
    let longest = spikes.windows(2).map(|w| w[1][0].0 - w[0][2].0).fold(0.0, f64::max);
    assert!(longest > 2.0, "some spine stitch is longer than the longest stitch: {longest}");
}

#[test]
fn req_sat_020_between_neighbours_the_end_cuts_the_spikes_as_a_satin_s_stitches() {
    // A needle to the left of the column and an element above its middle: the column starts on the line
    // between its rails nearest the needle, and ends on its outline nearest the element. The end cuts the
    // top stitches at the pair nearest it, and every spike's tip is still sewn once, on one side of the cut.
    let neighbours = Neighbours { needle: Some(p(-3.0, 2.0)), next: Some(Approach::Point(p(5.0, 7.0))) };
    let params = [("satin_method", "e_stitch"), ("zigzag_spacing_mm", "2"), ("pull_compensation_mm", "0.2"), NO_SHORT_STITCHES];
    let (points, warnings) = sewn_between(&ladder(10.0, 4.0, &[]), &params, &neighbours);
    assert!(warnings.is_empty(), "{warnings:?}");
    let points = xy(&points);
    assert!(near(&points[..1], &[(0.0, 2.0)]) && near(&points[points.len() - 1..], &[(5.0, 4.2)]), "{points:?}");
    for x in [0.0, 2.0, 4.0, 6.0, 8.0, 10.0] {
        let tips = points.iter().filter(|&&(px, py)| (px - x).abs() < 1e-9 && (py - 4.2).abs() < 1e-9).count();
        assert_eq!(tips, 1, "the spike at {x} mm: {points:?}");
    }
}
