//! Underlays' conformance cases (`REQ-SAT-004`, `REQ-SAT-010` to `REQ-SAT-012`) and `SC-W0206`: a
//! satin column's centre walk, contour and zigzag, sewn before its top stitches as Ink/Stitch sews them.
//!
//! Most cases sew a straight column 10 mm long and 6 mm wide along x, between rails at y = 0 and y = 6.
//! Its top stitches are those of the column sewn without underlays, so each case takes them off the end
//! of the run and looks at what comes before: the underlays, and the needle's travel between them and on
//! to the top stitches.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::Point;
use stitchcraft_testkit::designs::{RED, along, messages, p, planned, shape_of};
use stitchcraft_testkit::satins::{ladder, quarter_ring, sewn_satin};

/// The column 10 mm long and 6 mm wide, sewn with `params`, and its warnings.
fn column(params: &[(&str, &str)]) -> (Vec<Point>, Vec<String>) {
    sewn_satin(&ladder(10.0, 6.0, &[]), params)
}

/// The needle points before the top stitches of the column sewn with `params`: its underlays, and the
/// travel between them. The run must end with the top stitches as they are without underlays, `turned`
/// round when an odd centre walk ends at the column's end.
fn underlays(params: &[(&str, &str)], turned: bool) -> Vec<Point> {
    let (sewn, warnings) = column(params);
    assert!(warnings.is_empty(), "{warnings:?}");
    let without: Vec<(&str, &str)> = params.iter().copied().filter(|(key, _)| !key.ends_with("_underlay")).collect();
    let (mut top, _) = column(&without);
    if turned {
        top.reverse();
    }
    assert!(sewn.len() >= top.len() && sewn.ends_with(&top), "the top stitches come last, as they are without underlays");
    sewn[..sewn.len() - top.len()].to_vec()
}

/// Whether `got` are the points `want`, to within rounding.
fn same(got: &[Point], want: &[(f64, f64)]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(got, &(x, y))| got.distance(p(x, y)) < 1e-9)
}

/// Whether `got` are the numbers `want`, to within rounding.
fn close(got: &[f64], want: &[f64]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-9)
}

/// The points that split the line from `a` to `b` into `parts` equal parts, as the needle travels it.
fn travel(a: (f64, f64), b: (f64, f64), parts: u32) -> Vec<(f64, f64)> {
    (1..parts).map(|k| f64::from(k) / f64::from(parts)).map(|t| (a.0 + (b.0 - a.0) * t, a.1 + (b.1 - a.1) * t)).collect()
}

/// The contour underlay of the column with its default insets: the first rail's side, inset 0.4 mm, from
/// 0.4 mm past the start to 0.4 mm short of the end, in 3 mm stitches spread over the whole column (2.5
/// mm), then the second rail's side back, with the travel between them.
fn contour() -> Vec<(f64, f64)> {
    let first = [(0.4, 0.4), (2.5, 0.4), (5.0, 0.4), (7.5, 0.4), (9.6, 0.4)];
    let second = [(9.6, 5.6), (7.5, 5.6), (5.0, 5.6), (2.5, 5.6), (0.4, 5.6)];
    [&first[..], &travel((9.6, 0.4), (9.6, 5.6), 3), &second[..]].concat()
}

#[test]
fn req_sat_004_without_underlays_a_column_is_its_top_stitches() {
    assert!(underlays(&[], false).is_empty());
    for underlay in ["center_walk_underlay", "contour_underlay", "zigzag_underlay"] {
        assert!(underlays(&[(underlay, "false")], false).is_empty(), "{underlay}");
    }
}

#[test]
fn req_sat_004_underlays_come_before_the_top_stitches_and_leave_them_as_they_were() {
    for walk in ["false", "true"] {
        for contour in ["false", "true"] {
            for zigzag in ["false", "true"] {
                let on = [("center_walk_underlay", walk), ("contour_underlay", contour), ("zigzag_underlay", zigzag)];
                let under = underlays(&on, false);
                assert_eq!(under.is_empty(), on.iter().all(|(_, value)| *value == "false"), "{on:?}");
            }
        }
    }
}

#[test]
fn req_sat_004_underlays_are_planned_in_one_run_with_the_top_stitches() {
    let all = [("satin_column", "true"), ("center_walk_underlay", "true"), ("contour_underlay", "true"), ("zigzag_underlay", "true")];
    let outcome = planned(vec![along("satin", ladder(10.0, 6.0, &[]), &RED, &all)]);
    assert!(outcome.diagnostics.is_empty(), "{:?}", messages(&outcome));
    // 44 needle points of underlay and travel, and 26 pairs of top stitches, with a lock at each end.
    assert_eq!(shape_of(&outcome), "J L4 S96 L4");
}

#[test]
fn req_sat_004_the_centre_walk_comes_first_then_the_contour_then_the_zigzag() {
    let all = [("center_walk_underlay", "true"), ("contour_underlay", "true"), ("zigzag_underlay", "true")];
    let under = underlays(&all, false);
    // Each underlay's points lie on lines of their own: the walk's in the middle, the contour's 0.4 mm in
    // from the rails, the zigzag's 0.2 mm in.
    let on = |ys: &[f64]| -> Vec<usize> { (0..under.len()).filter(|&i| ys.iter().any(|y| (under[i].y() - y).abs() < 1e-9)).collect() };
    let (walk, contour, zigzag) = (on(&[3.0]), on(&[0.4, 5.6]), on(&[0.2, 5.8]));
    assert!(!walk.is_empty() && !contour.is_empty() && !zigzag.is_empty(), "{under:?}");
    assert!(walk.last() < contour.first() && contour.last() < zigzag.first(), "{under:?}");
}

#[test]
fn req_sat_004_the_needle_travels_straight_on_in_equal_stitches_no_longer_than_the_running_stitch_length() {
    // The contour ends 0.4 mm from the start on the second rail's side, 5.4148 mm from where the zigzag
    // starts, 0.2 mm in on the first rail's side. The zigzag ends on the second rail's side, 5.8 mm from
    // where the top stitches start.
    let both = [("contour_underlay", "true"), ("zigzag_underlay", "true")];
    for (length, parts) in [("2.5", 3), ("1", 6)] {
        let under = underlays(&[both[0], both[1], ("running_stitch_length_mm", length)], false);
        let after_contour = under.iter().position(|q| q.distance(p(0.4, 5.6)) < 1e-9).unwrap();
        let between = &under[after_contour + 1..after_contour + parts as usize];
        assert!(same(between, &travel((0.4, 5.6), (0.0, 0.2), parts)), "{length} mm: {between:?}");
        assert!(under[after_contour + parts as usize].distance(p(0.0, 0.2)) < 1e-9);
        let last = &under[under.len() - (parts as usize - 1)..];
        assert!(same(last, &travel((0.0, 5.8), (0.0, 0.0), parts)), "{length} mm: {last:?}");
    }
}

#[test]
fn req_sat_004_an_odd_centre_walk_ends_at_the_column_s_end_and_turns_the_rest_round() {
    for repeats in ["1", "3"] {
        let all =
            [("center_walk_underlay", "true"), ("center_walk_underlay_repeats", repeats), ("contour_underlay", "true"), ("zigzag_underlay", "true")];
        let under = underlays(&all, true);
        let walk: Vec<Point> = under.iter().copied().take_while(|q| (q.y() - 3.0).abs() < 1e-9).collect();
        assert_eq!(walk.last(), Some(&p(10.0, 3.0)), "{repeats}: the walk ends at the end");
        // The contour goes back along the first rail's side and on along the second's.
        let contour: Vec<Point> = under.iter().copied().filter(|q| (q.y() - 0.4).abs() < 1e-9 || (q.y() - 5.6).abs() < 1e-9).collect();
        let want = [(9.6, 0.4), (7.5, 0.4), (5.0, 0.4), (2.5, 0.4), (0.4, 0.4), (0.4, 5.6), (2.5, 5.6), (5.0, 5.6), (7.5, 5.6), (9.6, 5.6)];
        assert!(same(&contour, &want), "{repeats}: {contour:?}");
        // The zigzag starts at the end and comes back to it.
        let zigzag: Vec<Point> = under.iter().copied().filter(|q| (q.y() - 0.2).abs() < 1e-9 || (q.y() - 5.8).abs() < 1e-9).collect();
        assert_eq!((zigzag.first().map(|q| q.x()), zigzag.last().map(|q| q.x())), (Some(10.0), Some(10.0)), "{repeats}: {zigzag:?}");
    }
}

#[test]
fn req_sat_010_the_centre_walk_runs_along_the_line_at_its_position_there_and_back() {
    // 3 mm stitches spread evenly over 10 mm: 4 of 2.5 mm, there and back.
    let walk = |y: f64| -> Vec<(f64, f64)> { [0.0, 2.5, 5.0, 7.5, 10.0, 7.5, 5.0, 2.5, 0.0].iter().map(|&x| (x, y)).collect() };
    let under = underlays(&[("center_walk_underlay", "true")], false);
    // From where the walk ends, the needle goes 3 mm straight down to the top stitches' first point.
    assert!(same(&under, &[walk(3.0), travel((0.0, 3.0), (0.0, 0.0), 2)].concat()), "{under:?}");
    for (position, y) in [("25", 1.5), ("0", 0.0), ("100", 6.0)] {
        let under = underlays(&[("center_walk_underlay", "true"), ("center_walk_underlay_position", position)], false);
        let walked: Vec<Point> = under.iter().copied().take(9).collect();
        assert!(same(&walked, &walk(y)), "{position} %: {under:?}");
    }
}

#[test]
fn req_sat_010_its_stitches_are_spread_evenly_at_most_its_length_and_sewn_its_repeats() {
    // Where along the column the walk's needle points are, sewn with `length` and `repeats`.
    let walked = |length: &str, repeats: &str| -> Vec<f64> {
        let params = [("center_walk_underlay", "true"), ("center_walk_underlay_stitch_length_mm", length), ("center_walk_underlay_repeats", repeats)];
        let under = underlays(&params, repeats != "2");
        under.iter().filter(|q| (q.y() - 3.0).abs() < 1e-9).map(|q| q.x()).collect()
    };
    assert!(close(&walked("5", "2"), &[0.0, 5.0, 10.0, 5.0, 0.0]));
    // 4 mm stitches: 3 of a third of the column.
    let (third, two_thirds) = (10.0 / 3.0, 20.0 / 3.0);
    assert!(close(&walked("4", "2"), &[0.0, third, two_thirds, 10.0, two_thirds, third, 0.0]));
    assert!(close(&walked("5", "1"), &[0.0, 5.0, 10.0]));
    assert!(close(&walked("5", "3"), &[0.0, 5.0, 10.0, 5.0, 0.0, 5.0, 10.0]));
}

/// The centre walk of a quarter ring with rails of radius 10 and 6 mm, sewn once with `params`.
fn ring_walk(params: &[(&str, &str)]) -> Vec<Point> {
    let mut all = vec![("center_walk_underlay", "true"), ("center_walk_underlay_repeats", "1")];
    all.extend_from_slice(params);
    let (sewn, warnings) = sewn_satin(&quarter_ring(10.0, 6.0), &all);
    assert!(warnings.is_empty(), "{warnings:?}");
    sewn.into_iter().take_while(|q| (q.distance(Point::ORIGIN) - 8.0).abs() < 0.3).collect()
}

#[test]
fn req_sat_010_on_a_curve_the_centre_walk_keeps_within_its_tolerance() {
    // The walk follows the circle of radius 8 mm halfway between the rails, which are cubics within a
    // hundredth of a millimetre of circles. Each stitch's middle strays from it by at most the tolerance.
    let strays = |walk: &[Point]| walk.windows(2).map(|w| (w[0].lerp(w[1], 0.5).distance(Point::ORIGIN) - 8.0).abs()).fold(0.0, f64::max);
    let (loose, tight) = (ring_walk(&[]), ring_walk(&[("center_walk_underlay_stitch_tolerance_mm", "0.05")]));
    assert!(strays(&loose) <= 0.2 + 1e-2 && strays(&tight) <= 0.05 + 1e-2, "{} {}", strays(&loose), strays(&tight));
    assert!(tight.len() > loose.len(), "{} {}", tight.len(), loose.len());
    // DEV-SAT-004: the satin's running stitch tolerance is not the walk's, as it is in Ink/Stitch.
    assert_eq!(ring_walk(&[("running_stitch_tolerance_mm", "0.05")]), loose);
}

#[test]
fn req_sat_011_the_contour_runs_along_each_rail_inset_and_stops_short_of_the_column_s_ends() {
    let under = underlays(&[("contour_underlay", "true")], false);
    assert!(same(&under, &[contour(), travel((0.4, 5.6), (0.0, 0.0), 3)].concat()), "{under:?}");
}

#[test]
fn req_sat_011_insets_take_a_share_of_the_width_and_may_differ_by_rail() {
    // 0.4 mm in on the first rail's side and 1 mm on the second's. Each side stops short of the start by
    // the first rail's inset and of the end by the second's.
    let under = underlays(&[("contour_underlay", "true"), ("contour_underlay_inset_mm", "0.4 1")], false);
    let first: Vec<Point> = under.iter().copied().filter(|q| (q.y() - 0.4).abs() < 1e-9).collect();
    let second: Vec<Point> = under.iter().copied().filter(|q| (q.y() - 5.0).abs() < 1e-9).collect();
    assert!(same(&first, &[(0.4, 0.4), (2.5, 0.4), (5.0, 0.4), (7.5, 0.4), (9.0, 0.4)]), "{first:?}");
    assert!(same(&second, &[(9.0, 5.0), (7.5, 5.0), (5.0, 5.0), (2.5, 5.0), (0.4, 5.0)]), "{second:?}");
    // 10 % of the 6 mm width, 0.6 mm, and no length: the sides run to the column's ends.
    let under = underlays(&[("contour_underlay", "true"), ("contour_underlay_inset_mm", "0"), ("contour_underlay_inset_percent", "10")], false);
    let first: Vec<Point> = under.iter().copied().filter(|q| (q.y() - 0.6).abs() < 1e-9).collect();
    assert!(same(&first, &[(0.0, 0.6), (2.5, 0.6), (5.0, 0.6), (7.5, 0.6), (10.0, 0.6)]), "{first:?}");
    assert_eq!(under.iter().filter(|q| (q.y() - 5.4).abs() < 1e-9).count(), 5, "{under:?}");
    // With the rails swapped, the first rail's side is the one at y = 6.
    let under = underlays(&[("contour_underlay", "true"), ("swap_satin_rails", "true")], false);
    assert!((under[0].y() - 5.6).abs() < 1e-9, "{under:?}");
}

#[test]
fn diag_sc_w0206_a_contour_side_too_short_to_stop_short_of_both_ends_keeps_its_length() {
    // 0.92 mm long: stopping 0.4 mm short of each end would leave 0.12 mm, less than half a CSS pixel.
    let short = |inset: &str| sewn_satin(&ladder(0.92, 6.0, &[]), &[("contour_underlay", "true"), ("contour_underlay_inset_mm", inset)]);
    let (sewn, warnings) = short("0.4");
    assert!(same(&sewn[..2], &[(0.0, 0.4), (0.92, 0.4)]), "{sewn:?}");
    assert_eq!(
        warnings,
        [
            "warning SC-W0206: This satin column's contour underlay is too short to stop 0.4 mm short of the column's start and 0.4 mm short of its end, so it keeps its length."
        ]
    );
    // 0.3 mm leaves 0.32 mm.
    let (sewn, warnings) = short("0.3");
    assert!(same(&sewn[..2], &[(0.3, 0.3), (0.62, 0.3)]) && warnings.is_empty(), "{sewn:?} {warnings:?}");
}

#[test]
fn req_sat_012_the_zigzag_goes_through_one_end_of_each_pair_and_back_through_the_others() {
    // Pairs every 1.5 mm, half the 3 mm spacing, and at the end, 0.2 mm in: half the contour's inset.
    let there = [(0.0, 0.2), (1.5, 5.8), (3.0, 0.2), (4.5, 5.8), (6.0, 0.2), (7.5, 5.8), (9.0, 0.2), (10.0, 5.8)];
    let back = [(10.0, 0.2), (9.0, 5.8), (7.5, 0.2), (6.0, 5.8), (4.5, 0.2), (3.0, 5.8), (1.5, 0.2), (0.0, 5.8)];
    let under = underlays(&[("zigzag_underlay", "true")], false);
    let want = [&there[..], &travel((10.0, 5.8), (10.0, 0.2), 3), &back[..], &travel((0.0, 5.8), (0.0, 0.0), 3)].concat();
    assert!(same(&under, &want), "{under:?}");
}

/// How far in from the first rail the zigzag's first point is, with `params`.
fn zigzag_inset(params: &[(&str, &str)]) -> f64 {
    underlays(&[&[("zigzag_underlay", "true")], params].concat(), false)[0].y()
}

#[test]
fn req_sat_012_its_insets_are_half_the_contour_s_unless_set() {
    let near = |got: f64, want: f64| (got - want).abs() < 1e-9;
    assert!(near(zigzag_inset(&[("contour_underlay_inset_mm", "1")]), 0.5));
    // 5 % of the 6 mm width, half the contour's 10 %, and half its 0.4 mm.
    assert!(near(zigzag_inset(&[("contour_underlay_inset_percent", "10")]), 0.5));
    assert!(near(zigzag_inset(&[("zigzag_underlay_inset_mm", "0.3")]), 0.3));
    assert!(near(zigzag_inset(&[("zigzag_underlay_inset_mm", "0")]), 0.0), "0 is an inset, not empty");
    assert!(near(zigzag_inset(&[("contour_underlay_inset_percent", "10"), ("zigzag_underlay_inset_percent", "0")]), 0.2));
    // One value for each rail: the second rail's points are 1 mm in.
    let under = underlays(&[("zigzag_underlay", "true"), ("zigzag_underlay_inset_mm", "0.2 1")], false);
    assert!((under[1].y() - 5.0).abs() < 1e-9, "{under:?}");
}

#[test]
fn req_sat_012_its_spacing_sets_how_far_apart_a_rail_s_points_are() {
    // Pairs every 2 mm, so each rail's points are 4 mm apart.
    let under = underlays(&[("zigzag_underlay", "true"), ("zigzag_underlay_spacing_mm", "4")], false);
    assert!(same(&under[..6], &[(0.0, 0.2), (2.0, 5.8), (4.0, 0.2), (6.0, 5.8), (8.0, 0.2), (10.0, 5.8)]), "{under:?}");
}

#[test]
fn req_sat_012_stitches_longer_than_its_longest_are_split_into_equal_parts() {
    // The 5.8 mm stitches split into 3 parts of at most 2 mm. The travel to the top stitches is the
    // running stitch's, 2.5 mm.
    let plain = underlays(&[("zigzag_underlay", "true")], false);
    let split = underlays(&[("zigzag_underlay", "true"), ("zigzag_underlay_max_stitch_length_mm", "2")], false);
    let longest = |points: &[Point]| points.windows(2).map(|w| w[0].distance(w[1])).fold(0.0, f64::max);
    assert!(longest(&split) <= 2.0 + 1e-9 && longest(&plain) > 5.0, "{split:?}");
    let first = [&[(0.0, 0.2)][..], &travel((0.0, 0.2), (1.5, 5.8), 3), &[(1.5, 5.8)]].concat();
    assert!(same(&split[..4], &first), "{split:?}");
    // Each of the 2 passes' 7 stitches splits into 3, and the travels between and after them are as
    // before.
    assert_eq!(split.len(), plain.len() + 2 * 7 * 2, "{} {}", split.len(), plain.len());
}
