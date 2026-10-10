//! Split stitches' conformance cases (`REQ-SAT-003`): a satin column's stitches longer than the longest
//! stitch, split as `split_method` says, as Ink/Stitch splits them.
//!
//! Most cases sew a straight column 7 mm wide along x. Its stitches across go straight up from y = 0 to
//! y = 7, and its stitches back slant down to the next pair, so each stitch's needle points between its
//! ends are its splits.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::Point;
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, ladder, quarter_ring, sewn_satin};

/// The needle points of a column 10 mm long and 7 mm wide, sewn with `params` and no short stitches.
fn column(params: &[(&str, &str)]) -> Vec<Point> {
    let mut params = params.to_vec();
    params.push(NO_SHORT_STITCHES);
    let (points, warnings) = sewn_satin(&ladder(10.0, 7.0, &[]), &params);
    assert!(warnings.is_empty(), "{warnings:?}");
    points
}

/// The stitches of `points`, each as its needle points from one rail to the other: its ends, and its
/// splits between them, in sewing order. The stitches across are the even ones, from the first.
fn stitches(points: &[Point]) -> Vec<Vec<Point>> {
    let on_rail = |p: &Point| p.y().abs() < 1e-9 || (p.y() - 7.0).abs() < 1e-9;
    let mut stitches = Vec::new();
    let mut current = vec![points[0]];
    for point in &points[1..] {
        current.push(*point);
        if on_rail(point) {
            stitches.push(std::mem::replace(&mut current, vec![*point]));
        }
    }
    stitches
}

/// How far each of `stitch`'s splits is from its start, along it.
fn splits(stitch: &[Point]) -> Vec<f64> {
    stitch[1..stitch.len() - 1].iter().map(|p| p.distance(stitch[0])).collect()
}

/// The length of `stitch`, from its first needle point to its last.
fn length(stitch: &[Point]) -> f64 {
    stitch[0].distance(stitch[stitch.len() - 1])
}

/// Whether `got` are `want`, to within `tolerance`.
fn near(got: &[f64], want: &[f64], tolerance: f64) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(got, want)| (got - want).abs() <= tolerance)
}

#[test]
fn req_sat_003_without_a_longest_stitch_no_stitch_is_split() {
    let points = column(&[]);
    assert!(stitches(&points).iter().all(|stitch| stitch.len() == 2), "{points:?}");
}

#[test]
fn req_sat_003_by_default_a_stitch_splits_into_the_fewest_equal_parts() {
    // 7 mm stitches with a longest stitch of 3 mm: 3 equal parts. The stitches back, a little longer, are
    // split into as many parts as the stitch across before them.
    for stitch in stitches(&column(&[("max_stitch_length_mm", "3")])) {
        let third = length(&stitch) / 3.0;
        assert!(near(&splits(&stitch), &[third, 2.0 * third], 1e-9), "{stitch:?}");
    }
    // A longest stitch of 3.6 mm: 2 parts. Of 7.5 mm: none.
    assert!(stitches(&column(&[("max_stitch_length_mm", "3.6")])).iter().all(|stitch| splits(stitch).len() == 1));
    assert!(stitches(&column(&[("max_stitch_length_mm", "7.5")])).iter().all(|stitch| stitch.len() == 2));
    // Of 7.01 mm: the stitches across are shorter and not split. The stitches back, 7.011 mm, are longer,
    // and split into as many parts as the stitch across before them: 1, so not split either.
    assert!(stitches(&column(&[("max_stitch_length_mm", "7.01")])).iter().all(|stitch| stitch.len() == 2));
}

#[test]
fn req_sat_003_jitter_moves_each_split_at_random_within_its_share() {
    // 25 % of a part either way, drawn again for every split, the same for the same seed.
    let jittered = [("max_stitch_length_mm", "3"), ("random_split_jitter_percent", "25")];
    let sewn = column(&jittered);
    assert_eq!(sewn, column(&jittered));
    let (mut back, mut on) = (0, 0);
    for stitch in stitches(&sewn) {
        let (parts, third) = (splits(&stitch), length(&stitch) / 3.0);
        assert!(near(&parts, &[third, 2.0 * third], 0.25 * third + 1e-9), "{parts:?}");
        for (at, even) in parts.iter().zip([third, 2.0 * third]) {
            back += usize::from(at - even < -1e-6);
            on += usize::from(at - even > 1e-6);
        }
    }
    assert!(back > 20 && on > 20, "the splits move both ways: {back} back, {on} on");
    assert_ne!(sewn, column(&[jittered[0], jittered[1], ("random_seed", "2")]), "another seed, other splits");
}

#[test]
fn req_sat_003_a_random_phase_starts_the_splits_at_random_and_spaces_them_by_the_longest_stitch() {
    let phased = [("max_stitch_length_mm", "3"), ("random_split_phase", "true")];
    let sewn = column(&phased);
    assert_eq!(sewn, column(&phased));
    let mut starts = Vec::new();
    for stitch in stitches(&sewn) {
        let (parts, long) = (splits(&stitch), length(&stitch));
        // The first within 3 mm of the start, or 3 mm on from one within the shortest stitch of it, which
        // is left out. The rest 3 mm apart, all before the end.
        assert!(!parts.is_empty() && parts[0] < 3.3 && *parts.last().unwrap() < long, "{parts:?}");
        assert!(parts.windows(2).all(|w| (w[1] - w[0] - 3.0).abs() < 1e-9), "{parts:?}");
        // Of 2 or more, none within the shortest stitch (0.3 mm) of an end.
        assert!(parts.len() == 1 || (parts[0] > 0.3 && long - parts.last().unwrap() > 0.3), "{parts:?}");
        starts.push(parts[0]);
    }
    assert!(starts.windows(2).any(|w| (w[0] - w[1]).abs() > 0.1), "the phase varies: {starts:?}");
}

#[test]
fn req_sat_003_a_random_phase_splits_only_stitches_longer_than_the_shortest_to_split() {
    // A column 2 mm wide: its stitches are shorter than the 3 mm longest stitch, and not split, unless the
    // shortest stitch to split is under 2 mm.
    let narrow = |params: &[(&str, &str)]| {
        let mut params = params.to_vec();
        params.extend([("max_stitch_length_mm", "3"), ("random_split_phase", "true"), NO_SHORT_STITCHES]);
        sewn_satin(&ladder(10.0, 2.0, &[]), &params).0
    };
    assert_eq!(narrow(&[]), sewn_satin(&ladder(10.0, 2.0, &[]), &[NO_SHORT_STITCHES]).0);
    let split = narrow(&[("min_random_split_length_mm", "1.5")]);
    assert!(split.iter().any(|p| p.y() > 1e-9 && p.y() < 2.0 - 1e-9), "some split");
}

/// The splits a staggered stitch has, from its start: whole multiples of `every` from `offset`, measured
/// from its start across and from its end back, strictly inside it.
fn staggered(stitch: &[Point], across: bool, offset: f64, every: f64) -> Vec<f64> {
    let long = length(stitch);
    let mut at: Vec<f64> = (0..10).map(|n| offset + every * f64::from(n)).filter(|at| *at > 0.0 && *at < long).collect();
    if !across {
        at = at.iter().rev().map(|at| long - at).collect();
    }
    at
}

#[test]
fn req_sat_003_simple_splits_at_whole_multiples_of_the_longest_stitch() {
    // Across, 3 and 6 mm from the first rail. Back, 3 and 6 mm from the next pair's first point, the
    // stitch's end, so the splits line up in rows.
    for (k, stitch) in stitches(&column(&[("split_method", "simple"), ("max_stitch_length_mm", "3")])).iter().enumerate() {
        assert!(near(&splits(stitch), &staggered(stitch, k % 2 == 0, 0.0, 3.0), 1e-9), "stitch {k}: {stitch:?}");
    }
}

#[test]
fn req_sat_003_staggered_splits_move_along_from_one_stitch_to_the_next() {
    // 4 staggers of a 3 mm longest stitch: stitch k of the column starts its splits (k + 1) / 4 of 3 mm
    // on, and every 4 stitches they come back.
    let sewn = stitches(&column(&[("split_method", "staggered"), ("max_stitch_length_mm", "3")]));
    for (k, stitch) in sewn.iter().enumerate() {
        let offset = 3.0 * ((k + 1) % 4) as f64 / 4.0;
        assert!(near(&splits(stitch), &staggered(stitch, k % 2 == 0, offset, 3.0), 1e-9), "stitch {k}: {stitch:?}");
    }
    // 2.5 staggers, a fraction: back where they started after 5 stitches.
    let fractional = stitches(&column(&[("split_method", "staggered"), ("max_stitch_length_mm", "3"), ("split_staggers", "2.5")]));
    for (k, stitch) in fractional.iter().enumerate() {
        let offset = 3.0 * ((k + 1) as f64 / 2.5).rem_euclid(1.0);
        assert!(near(&splits(stitch), &staggered(stitch, k % 2 == 0, offset, 3.0), 1e-9), "stitch {k}: {stitch:?}");
    }
}

#[test]
fn req_sat_003_with_even_splits_a_short_stitch_s_inset_is_at_most_a_third_of_the_longest_stitch() {
    // A tight ring 4 mm wide: its inner points crowd, and insets of 50 % would move them 2 mm. Its stitches
    // split into 3 parts at a longest stitch of 1.5 mm, which holds the insets to 0.5 mm.
    let (points, _) = sewn_satin(&quarter_ring(6.0, 2.0), &[("short_stitch_inset", "50"), ("max_stitch_length_mm", "1.5")]);
    // Every third needle point ends a stitch: the outer rail's on radius 6, the inner's from 2 to 2.5. The
    // rails are cubics flattened within a tenth of a CSS pixel (0.026 mm), and those cubics lie within a
    // thousandth of a millimetre of circles this small.
    let inner: Vec<f64> = points.iter().step_by(3).map(|p| p.distance(Point::ORIGIN)).filter(|r| *r < 4.0).collect();
    assert!(inner.iter().all(|r| (2.0 - 0.03..=2.5 + 0.03).contains(r)), "{inner:?}");
    assert!(inner.iter().filter(|r| (*r - 2.5).abs() < 1e-2).count() > 5, "inset the most: {inner:?}");
}
