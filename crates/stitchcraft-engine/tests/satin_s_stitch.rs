//! The S-stitch's conformance cases (`REQ-SAT-024`, `REQ-SAT-025`): a satin column sewn once across at each
//! pair, every other pair the other way, the needle going on from each stitch to the next along a rail, as
//! Ink/Stitch sews its S stitch.
//!
//! The cases sew straight columns along x, the first rail at y = 0 and the second at the column's width,
//! with no short stitches unless a case says so. The first stitch goes down from the second rail, the next
//! up from the first, and on in turn.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_engine::generators::{Approach, Neighbours};
use stitchcraft_testkit::designs::p;
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, ladder, near, sewn_between, sewn_ladder, xy};

/// The needle points of a column `length` long and `width` wide, sewn as an S-stitch with `params` and no
/// short stitches, which gives no warning.
fn s_stitch(length: f64, width: f64, params: &[(&str, &str)]) -> Vec<(f64, f64)> {
    let mut params = params.to_vec();
    params.push(NO_SHORT_STITCHES);
    sewn_ladder(length, width, "s_stitch", &params)
}

/// The stitch across a column `width` wide at `x`, split at `splits` up from the first rail, sewn as the
/// pair numbered `n` is: down from the second rail when `n` is even, up from the first when it is odd.
fn across(n: u32, x: f64, width: f64, splits: &[f64]) -> Vec<(f64, f64)> {
    let mut stitch: Vec<(f64, f64)> = [0.0].iter().chain(splits).chain(&[width]).map(|&y| (x, y)).collect();
    if n.is_multiple_of(2) {
        stitch.reverse();
    }
    stitch
}

/// The stitches across at `xs` in turn, each split at `splits`.
fn column(xs: &[f64], width: f64, splits: &[f64]) -> Vec<(f64, f64)> {
    xs.iter().zip(0..).flat_map(|(&x, n)| across(n, x, width, splits)).collect()
}

#[test]
fn req_sat_024_each_pair_is_sewn_once_across_every_other_one_from_the_second_rail() {
    // Pairs every 2 mm along a column 10 mm long and 4 mm wide: down at 0, up at 2, down at 4 and on, each
    // stitch going on to the next along the rail it ended on.
    let points = s_stitch(10.0, 4.0, &[("zigzag_spacing_mm", "2")]);
    let want = column(&[0.0, 2.0, 4.0, 6.0, 8.0, 10.0], 4.0, &[]);
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_024_a_stitch_splits_as_one_from_the_first_rail_and_is_sewn_its_pair_s_way() {
    // Stitches 7 mm long with a longest stitch of 3 mm: 3 equal parts, down and up in turn.
    let third = 7.0 / 3.0;
    let even = column(&[0.0, 2.0, 4.0], 7.0, &[third, 2.0 * third]);
    let points = s_stitch(4.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3")]);
    assert!(near(&points, &even), "{points:?}");
    // Splits moved at random stay on their stitch, in order along the way it is sewn.
    let jittered = s_stitch(4.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3"), ("random_split_jitter_percent", "40")]);
    assert_eq!(jittered.len(), even.len());
    for (stitch, n) in jittered.chunks(4).zip(0_u32..) {
        let x = 2.0 * f64::from(n);
        let ys: Vec<f64> = stitch.iter().map(|&(_, y)| y).collect();
        let on_its_way = ys.windows(2).all(|w| if n.is_multiple_of(2) { w[0] > w[1] } else { w[0] < w[1] });
        assert!(stitch.iter().all(|&(px, _)| (px - x).abs() < 1e-9) && on_its_way, "stitch {n}: {stitch:?}");
    }
    assert!(!near(&jittered, &even), "the splits moved");
}

#[test]
fn req_sat_024_stitches_split_by_the_staggered_method_take_their_pair_s_number() {
    // Stitches 7 mm long with a longest stitch of 3 mm, split by the staggered method into its 4 staggers.
    // The n-th stitch, from 0, is the column's stitch n: its splits start n quarters of 3 mm from the
    // first rail, whichever way it is sewn.
    let points = s_stitch(6.0, 7.0, &[("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3"), ("split_method", "staggered")]);
    let splits: [&[f64]; 4] = [&[3.0, 6.0], &[0.75, 3.75, 6.75], &[1.5, 4.5], &[2.25, 5.25]];
    let want: Vec<(f64, f64)> = splits.iter().zip(0_u32..).flat_map(|(at, n)| across(n, 2.0 * f64::from(n), 7.0, at)).collect();
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_024_a_random_phase_splits_a_stitch_from_the_first_rail_whichever_way_it_is_sewn() {
    // A random phase starts a stitch's splits a random share of the longest stitch from its first rail's
    // inset point. The first stitch of an S-stitch and a satin column's first stitch across draw the same
    // numbers, so they split alike, measured from the first rail, the S-stitch's sewn the other way.
    let params = [("zigzag_spacing_mm", "2"), ("max_stitch_length_mm", "3"), ("random_split_phase", "true"), NO_SHORT_STITCHES];
    let first =
        |method: &str| -> Vec<(f64, f64)> { sewn_ladder(4.0, 7.0, method, &params).into_iter().take_while(|&(x, _)| x.abs() < 1e-9).collect() };
    let (mut s, satin) = (first("s_stitch"), first("satin_column"));
    assert!(s.len() > 2 && near(&s[..1], &[(0.0, 7.0)]), "split, down from the second rail: {s:?}");
    s.reverse();
    assert!(near(&s, &satin), "{s:?} against {satin:?}");
}

#[test]
fn req_sat_024_swapped_rails_start_the_column_on_the_rail_drawn_first() {
    // The rail drawn second, at y = 3, is the first rail now, so the first stitch goes up to it from the
    // rail drawn first.
    let points = s_stitch(4.0, 3.0, &[("zigzag_spacing_mm", "2"), ("swap_satin_rails", "true")]);
    let want = [(0.0, 0.0), (0.0, 3.0), (2.0, 3.0), (2.0, 0.0), (4.0, 0.0), (4.0, 3.0)];
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_024_after_an_odd_centre_walk_the_stitches_run_from_the_column_s_end() {
    let points = s_stitch(4.0, 3.0, &[("zigzag_spacing_mm", "2"), ("center_walk_underlay", "true"), ("center_walk_underlay_repeats", "1")]);
    // The top stitches come last, sewn backwards, from the end of the last pair's stitch to the start of the
    // first's.
    let top = &points[points.len() - 6..];
    let mut want = column(&[0.0, 2.0, 4.0], 3.0, &[]);
    want.reverse();
    assert!(near(top, &want), "{points:?}");
}

#[test]
fn req_sat_025_a_step_along_a_rail_is_one_stitch_however_long() {
    // Pairs 5 mm apart on a column 1 mm wide, with a longest stitch of 2 mm: each step along a rail is 5 mm
    // long, and is sewn in one stitch by every split method. The stitches across are too short to split.
    let unsplit = column(&[0.0, 5.0, 10.0], 1.0, &[]);
    for method in
        [&[("split_method", "default")][..], &[("split_method", "simple")], &[("split_method", "staggered")], &[("random_split_phase", "true")]]
    {
        let mut params = vec![("zigzag_spacing_mm", "5"), ("max_stitch_length_mm", "2")];
        params.extend_from_slice(method);
        let points = s_stitch(10.0, 1.0, &params);
        assert!(near(&points, &unsplit), "{method:?}: {points:?}");
    }
}

#[test]
fn req_sat_025_a_step_goes_from_the_inset_point_a_stitch_ends_on_to_the_next_one() {
    // Pairs 3 mm apart on a column 4 mm wide with a longest stitch of 2 mm. Their points crowd closer than
    // 5 mm, and every other pair is inset 15 % of the width, 0.6 mm, on both rails. Each stitch across splits
    // halfway between its pair's points, and each step goes from inset point to inset point, unsplit.
    let points = sewn_ladder(12.0, 4.0, "s_stitch", &[("zigzag_spacing_mm", "3"), ("max_stitch_length_mm", "2"), ("short_stitch_distance_mm", "5")]);
    let inset = |n: u32, x: f64, by: f64| {
        let mut stitch = vec![(x, by), (x, 2.0), (x, 4.0 - by)];
        if n.is_multiple_of(2) {
            stitch.reverse();
        }
        stitch
    };
    let want = [inset(0, 0.0, 0.0), inset(1, 3.0, 0.6), inset(2, 6.0, 0.0), inset(3, 9.0, 0.6), inset(4, 12.0, 0.0)].concat();
    assert!(near(&points, &want), "{points:?}");
}

#[test]
fn req_sat_024_between_neighbours_the_end_cuts_the_stitches_as_a_satin_s() {
    // A needle to the left of the column and an element above its middle: the column starts on the line
    // between its rails nearest the needle, and ends on its outline nearest the element. The end cuts the
    // top stitches at the pair nearest it, 4 mm along, whose stitch lies along the cut. They are cut at the
    // cut's point on the first rail, 0.2 mm from the stitch's point there, within the shortest stitch of
    // it, so at that point: both halves end there, and every other point of the pairs is sewn once, on one
    // side of the cut or the other.
    let neighbours = Neighbours { needle: Some(p(-3.0, 2.0)), next: Some(Approach::Point(p(5.0, 7.0))) };
    let params = [("satin_method", "s_stitch"), ("zigzag_spacing_mm", "2"), ("pull_compensation_mm", "0.2"), NO_SHORT_STITCHES];
    let (points, warnings) = sewn_between(&ladder(10.0, 4.0, &[]), &params, &neighbours);
    assert!(warnings.is_empty(), "{warnings:?}");
    let points = xy(&points);
    assert!(near(&points[..1], &[(0.0, 2.0)]) && near(&points[points.len() - 1..], &[(5.0, 4.2)]), "{points:?}");
    for x in [0.0, 2.0, 4.0, 6.0, 8.0, 10.0] {
        for y in [-0.2, 4.2] {
            let sewn = points.iter().filter(|&&(px, py)| (px - x).abs() < 1e-9 && (py - y).abs() < 1e-9).count();
            let at_the_cut = if (x, y) == (4.0, -0.2) { 2 } else { 1 };
            assert_eq!(sewn, at_the_cut, "the pair's point at ({x}, {y}): {points:?}");
        }
    }
}
