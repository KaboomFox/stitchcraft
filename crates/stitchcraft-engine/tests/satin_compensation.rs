//! Satin compensation's conformance cases: pull compensation (`REQ-SAT-001`), push compensation
//! (`REQ-SAT-008`, `SC-W0211`), values for each side (`REQ-SAT-006`) and random widths and spacing
//! (`REQ-SAT-007`), through `generators::satin::satin_stitch` and the engine's entry point.
//!
//! Most cases sew a straight column along x, whose stitches go straight across it, so where each end
//! should be after compensation can be read off the axes.

// Test code may unwrap, panic and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use stitchcraft_core::Point;
use stitchcraft_testkit::designs::{RED, along, p, planned, polylines};
use stitchcraft_testkit::satins::{across, gaps, ladder, sewn_satin};

/// The pairs of a straight column 10 mm long and 4 mm wide, sewn with `params`, and its warnings.
fn column(params: &[(&str, &str)]) -> (Vec<[Point; 2]>, Vec<String>) {
    let (points, warnings) = sewn_satin(&ladder(10.0, 4.0, &[]), params);
    (across(&points), warnings)
}

/// Where the ends of `pair` go when each moves out along it by `by`: the first, then the second.
fn moved_out([a, b]: [Point; 2], [by_a, by_b]: [f64; 2]) -> [Point; 2] {
    let (x, y) = ((a.x() - b.x()) / a.distance(b), (a.y() - b.y()) / a.distance(b));
    [p(a.x() + x * by_a, a.y() + y * by_a), p(b.x() - x * by_b, b.y() - y * by_b)]
}

#[test]
fn req_sat_001_pull_compensation_moves_each_end_out_along_its_stitch() {
    let (plain, _) = column(&[]);
    // 0.2 mm, 10 % of the 4 mm width, and both.
    let cases: [(&[(&str, &str)], f64); 3] = [
        (&[("pull_compensation_mm", "0.2")], 0.2),
        (&[("pull_compensation_percent", "10")], 0.4),
        (&[("pull_compensation_mm", "0.2"), ("pull_compensation_percent", "10")], 0.6),
    ];
    for (params, out) in cases {
        let (pairs, warnings) = column(params);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(pairs.len(), plain.len(), "the same stitches, {params:?}");
        for ([a, b], [plain_a, _]) in pairs.iter().zip(&plain) {
            assert!(a.distance(p(plain_a.x(), -out)) < 1e-9 && b.distance(p(plain_a.x(), 4.0 + out)) < 1e-9, "{params:?}: {a:?} {b:?}");
        }
    }
}

#[test]
fn req_sat_001_on_a_slanted_column_each_end_moves_along_its_own_stitch() {
    // A trapezoid: rails 10 mm and 6 mm long, so the stitches slant, each its own way and width.
    let trapezoid = polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(2.0, 4.0), (8.0, 4.0)]]);
    let plain = across(&sewn_satin(&trapezoid, &[]).0);
    let (points, _) = sewn_satin(&trapezoid, &[("pull_compensation_mm", "0.3"), ("pull_compensation_percent", "10")]);
    let pulled = across(&points);
    assert_eq!(pulled.len(), plain.len());
    for (pair, before) in pulled.iter().zip(&plain) {
        let out = 0.3 + 0.1 * before[0].distance(before[1]);
        let [a, b] = moved_out(*before, [out, out]);
        assert!(pair[0].distance(a) < 0.01 && pair[1].distance(b) < 0.01, "{pair:?} {before:?}");
    }
}

#[test]
fn req_sat_001_ends_moved_in_past_each_other_meet_instead() {
    // 3 mm in on each side of a 4 mm column: they meet in the middle, each having moved 2 mm.
    let (pairs, _) = column(&[("pull_compensation_mm", "-3")]);
    assert!(pairs.iter().all(|[a, b]| (a.y() - 2.0).abs() < 1e-9 && (b.y() - 2.0).abs() < 1e-9), "{pairs:?}");
    // 3 mm in on the first side and 1 mm on the second: exactly the width, so they meet 3 mm in.
    let (pairs, _) = column(&[("pull_compensation_mm", "-3 -1")]);
    assert!(pairs.iter().all(|[a, b]| (a.y() - 3.0).abs() < 1e-9 && (b.y() - 3.0).abs() < 1e-9), "{pairs:?}");
    // A percentage meets the same way: 75 % in on each side is 1.5 widths in all.
    let (pairs, _) = column(&[("pull_compensation_percent", "-75")]);
    assert!(pairs.iter().all(|[a, b]| (a.y() - 2.0).abs() < 1e-9 && (b.y() - 2.0).abs() < 1e-9), "{pairs:?}");
}

#[test]
fn req_sat_006_a_value_for_each_side_changes_only_that_side() {
    let (plain, _) = column(&[]);
    for (key, value) in [
        ("pull_compensation_mm", "0 0.5"),
        ("pull_compensation_percent", "0 10"),
        ("random_width_increase_percent", "0 20"),
        ("random_width_decrease_percent", "0 20"),
    ] {
        let (pairs, _) = column(&[(key, value)]);
        assert!(pairs.iter().zip(&plain).all(|([a, _], [plain_a, _])| a == plain_a), "{key}: the first rail's ends stay on it");
        assert!(pairs.iter().zip(&plain).any(|([_, b], [_, plain_b])| b != plain_b), "{key}: the second rail's move");
        // And the other way round.
        let flipped: String = value.split(' ').rev().collect::<Vec<_>>().join(" ");
        let (pairs, _) = column(&[(key, &flipped)]);
        assert!(pairs.iter().zip(&plain).all(|([_, b], [_, plain_b])| b == plain_b), "{key} {flipped}: the second rail's stay");
    }
}

#[test]
fn req_sat_006_push_compensation_for_each_end_changes_only_that_end() {
    let x = |params: &[(&str, &str)]| -> Vec<f64> { column(params).0.iter().map(|[a, _]| a.x()).collect() };
    let start = x(&[("push_compensation_mm", "1 0")]);
    assert_eq!((start.first().copied(), start.last().copied()), (Some(1.0), Some(10.0)), "{start:?}");
    let end = x(&[("push_compensation_mm", "0 1")]);
    assert_eq!((end.first().copied(), end.last().copied()), (Some(0.0), Some(9.0)), "{end:?}");
}

#[test]
fn req_sat_008_push_compensation_shortens_the_column_at_both_ends() {
    let (pairs, warnings) = column(&[("push_compensation_mm", "1")]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let x: Vec<f64> = pairs.iter().map(|[a, _]| a.x()).collect();
    assert_eq!(x.len(), 21, "every 0.4 mm from 1 to 9: {x:?}");
    assert!(x.iter().enumerate().all(|(k, x)| (x - (1.0 + 0.4 * k as f64)).abs() < 1e-9), "{x:?}");
    // Negative: longer, along the rails' end segments.
    let x: Vec<f64> = column(&[("push_compensation_mm", "-1")]).0.iter().map(|[a, _]| a.x()).collect();
    assert_eq!(x.len(), 31, "every 0.4 mm from -1 to 11: {x:?}");
    assert!(x.iter().enumerate().all(|(k, x)| (x - (-1.0 + 0.4 * k as f64)).abs() < 1e-9), "{x:?}");
}

#[test]
fn req_sat_008_a_rung_in_a_part_taken_off_leaves_its_section_out() {
    // A rung 0.5 mm from the start, inside the 1 mm push compensation takes off: it cuts the shortened
    // rails at their starts, and the column sews as if it were not there.
    let pushed = [("push_compensation_mm", "1")];
    let with_rung = across(&sewn_satin(&ladder(10.0, 4.0, &[0.5]), &pushed).0);
    assert_eq!(with_rung, column(&pushed).0);
}

#[test]
fn diag_sc_w0211_push_compensation_longer_than_a_rail_is_named_and_left_out() {
    // 1 mm long: taking 0.5 mm off each end would leave nothing.
    let (points, warnings) = sewn_satin(&ladder(1.0, 4.0, &[]), &[("push_compensation_mm", "0.5")]);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("SC-W0211") && warnings[0].contains("0.5 mm at its start and 0.5 mm at its end"), "{warnings:?}");
    let x: Vec<f64> = across(&points).iter().map(|[a, _]| a.x()).collect();
    assert_eq!((x.first().copied(), x.last().copied()), (Some(0.0), Some(1.0)), "the column keeps its length: {x:?}");
    // One rail too short for it is enough: here the second, 1 mm long beside a first of 10 mm.
    let uneven = polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(4.5, 4.0), (5.5, 4.0)]]);
    let (_, warnings) = sewn_satin(&uneven, &[("push_compensation_mm", "0.45")]);
    assert!(warnings.len() == 1 && warnings[0].contains("SC-W0211"), "{warnings:?}");
}

#[test]
fn req_sat_007_random_widths_and_spacing_are_the_seed_s_and_stay_in_range() {
    let random = [("random_width_increase_percent", "20"), ("random_width_decrease_percent", "10"), ("random_zigzag_spacing_percent", "20")];
    let (pairs, warnings) = column(&random);
    assert!(warnings.is_empty(), "{warnings:?}");
    assert_eq!(pairs, column(&random).0, "the same stitches every time");
    // On each side, from 10 % of the 4 mm width in to 20 % out.
    assert!(pairs.iter().all(|[a, b]| (-0.8..=0.4).contains(&a.y()) && (3.6..=4.8).contains(&b.y())), "{pairs:?}");
    assert!(pairs.windows(2).any(|w| w[0][0].y() != w[1][0].y()), "the widths vary: {pairs:?}");
    // Each step from 0.32 to 0.48 mm, the last perhaps shorter, to the column's end.
    let steps = gaps(&pairs);
    let inside = &steps[..steps.len() - 1];
    assert!(inside.iter().all(|step| (0.32 - 1e-9..=0.48 + 1e-9).contains(step)), "{steps:?}");
    assert!(inside.windows(2).any(|w| (w[0] - w[1]).abs() > 1e-6), "the steps vary: {steps:?}");
    // Another seed, other stitches.
    let seeded = |seed: &str| column(&[random[0], random[1], random[2], ("random_seed", seed)]).0;
    assert_ne!(seeded("1"), seeded("2"));
    assert_eq!(seeded("1"), seeded("1"));
    assert_ne!(seeded("1"), pairs, "an empty seed is the element's own");
}

#[test]
fn req_sat_007_each_element_varies_its_own_way() {
    // The same column twice, as 2 elements: each id gives its own random widths.
    let column = |id: &str| {
        let params = [("satin_column", "true"), ("random_width_increase_percent", "30")];
        let outcome = planned(vec![along(id, ladder(10.0, 4.0, &[]), &RED, &params)]);
        outcome.plan.unwrap().stitches().map(|stitch| (stitch.at.x(), stitch.at.y())).collect::<Vec<_>>()
    };
    assert_eq!(column("left"), column("left"));
    assert_ne!(column("left"), column("right"));
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    /// However they are set, compensation and random widths move each end along its stitch within the
    /// range the parameters give, and random spacing keeps each step within its range.
    #[test]
    fn req_sat_001_and_007_every_end_stays_within_what_its_parameters_allow(
        pull in -1.0..1.0f64,
        percent in -20.0..20.0f64,
        decrease in 0.0..50.0f64,
        increase in 0.0..50.0f64,
        jitter in 0.0..50.0f64,
        seed in 0u64..1000,
    ) {
        let text = |v: f64| format!("{v}");
        let params = [
            ("pull_compensation_mm", text(pull)),
            ("pull_compensation_percent", text(percent)),
            ("random_width_decrease_percent", text(decrease)),
            ("random_width_increase_percent", text(increase)),
            ("random_zigzag_spacing_percent", text(jitter)),
            ("random_seed", seed.to_string()),
        ];
        let params: Vec<(&str, &str)> = params.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let (pairs, warnings) = column(&params);
        prop_assert!(warnings.is_empty(), "{:?}", warnings);
        prop_assert_eq!(&pairs, &column(&params).0);
        // How far out each end may go: the length, plus the share of the 4 mm width, less the decrease
        // or plus the increase; ends never cross.
        let least = pull + 4.0 * (percent - decrease) / 100.0;
        let most = pull + 4.0 * (percent + increase) / 100.0;
        for [a, b] in &pairs {
            prop_assert!(a.x() == b.x(), "straight across: {:?} {:?}", a, b);
            prop_assert!(a.y() <= b.y() + 1e-9, "never crossed: {:?} {:?}", a, b);
            if least > -2.0 {
                prop_assert!((-most - 1e-9..=-least + 1e-9).contains(&a.y()), "{} to {}: {:?}", least, most, a);
                prop_assert!((4.0 + least - 1e-9..=4.0 + most + 1e-9).contains(&b.y()), "{} to {}: {:?}", least, most, b);
            }
        }
        // The steps along the column, read where the stitches cross the first rail (ends that met have no
        // direction to measure across): each between the zigzag spacing less and plus the jitter. The
        // first may have moved toward its spacing, and the last ends the column.
        let steps: Vec<f64> = pairs.windows(2).map(|w| w[1][0].x() - w[0][0].x()).collect();
        let factor = jitter / 100.0;
        for step in steps.get(1..steps.len() - 1).unwrap_or_default() {
            prop_assert!(*step >= 0.4 * (1.0 - factor) - 1e-9 && *step <= 0.4 * (1.0 + factor) + 1e-9, "{:?}", steps);
        }
    }
}
