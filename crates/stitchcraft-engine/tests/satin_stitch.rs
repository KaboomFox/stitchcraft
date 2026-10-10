//! Satin stitches' conformance cases (`REQ-SAT-002`) and `SC-W0210`: where the needle points go along a
//! satin column, through `generators::satin::satin_stitch` and the engine's entry point. The cases follow
//! how Ink/Stitch places them, so a file sews the same in both: straight and curved columns, rails drawn
//! against each other, swapped rails, rungs drawn in any order, and columns without rungs, whose nodes pair.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, and this extends it to the
// helpers here).
#![allow(clippy::unwrap_used, clippy::panic)]

use std::f64::consts::FRAC_PI_2;

use proptest::prelude::*;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_engine::generators::satin::{SatinParams, satin_stitch};
use stitchcraft_engine::normalize::satin::{Shape, recognize};
use stitchcraft_params::ParamSet;
use stitchcraft_testkit::designs::{RED, along, messages, p, planned as sewn, polylines, shape_of};
use stitchcraft_testkit::satins::{NO_SHORT_STITCHES, across, gaps, ladder, quarter_ring, satin_lengths, sewn_satin};

#[test]
fn req_sat_002_a_straight_column_is_sewn_rail_to_rail_at_the_spacing() {
    let (points, warnings) = sewn_satin(&ladder(10.0, 4.0, &[]), &[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let pairs = across(&points);
    assert_eq!(pairs.len(), 26, "every 0.4 mm from 0 to 10");
    for (k, [a, b]) in pairs.iter().enumerate() {
        let x = 0.4 * k as f64;
        assert!(a.distance(p(x, 0.0)) < 1e-9 && b.distance(p(x, 4.0)) < 1e-9, "pair {k}: {a:?} {b:?}");
    }
    // A wider spacing, and an end that is not on the spacing: the last pair is at the end.
    let pairs = across(&sewn_satin(&ladder(10.0, 4.0, &[]), &[("zigzag_spacing_mm", "3")]).0);
    let x: Vec<f64> = pairs.iter().map(|[a, _]| a.x()).collect();
    assert_eq!(x.len(), 5, "{x:?}");
    assert!(x.iter().zip([0.0, 3.0, 6.0, 9.0, 10.0]).all(|(x, want)| (x - want).abs() < 1e-9), "{x:?}");
}

#[test]
fn req_sat_002_rungs_cut_the_column_and_the_spacing_runs_on_across_them() {
    // Rungs at 2.5 mm and 7.3 mm, drawn in either order: the pairs stay on the 0.4 mm grid.
    for rungs in [[2.5, 7.3], [7.3, 2.5]] {
        let pairs = across(&sewn_satin(&ladder(10.0, 4.0, &rungs), &[]).0);
        assert_eq!(pairs.len(), 26, "{rungs:?}");
        assert!(gaps(&pairs).iter().all(|gap| (gap - 0.4).abs() < 1e-9), "{rungs:?}: {:?}", gaps(&pairs));
    }
}

#[test]
fn req_sat_002_on_a_curve_the_outer_edge_is_sewn_at_the_spacing() {
    // A quarter ring, 4 mm wide: the outer rail of radius 20, the inner of 16.
    let (points, warnings) = sewn_satin(&quarter_ring(20.0, 16.0), &[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let pairs = across(&points);
    let gaps = gaps(&pairs);
    let (last, inside) = gaps.split_last().unwrap();
    assert!(inside.iter().all(|gap| (gap - 0.4).abs() <= 0.02), "{inside:?}");
    assert!(*last <= 0.42, "the end pair, at most a spacing on: {last}");
    // The outer edge is about a quarter of a 20 mm circle long: 79 steps of 0.4 mm.
    assert!((pairs.len() as f64 - (20.0 * FRAC_PI_2 / 0.4 + 1.0)).abs() <= 1.0, "{}", pairs.len());
}

#[test]
fn req_sat_002_rails_drawn_against_each_other_are_turned_to_run_together() {
    let both_ways = polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(10.0, 4.0), (0.0, 4.0)]]);
    let straight = |points: &[Point]| across(points).iter().all(|[a, b]| (a.x() - b.x()).abs() < 1e-9);
    // Automatic: the second rail turns, and every stitch goes straight across.
    let (points, _) = sewn_satin(&both_ways, &[]);
    assert!(straight(&points) && points[0] == p(0.0, 0.0), "{:?}", &points[..4]);
    // Not turned, the stitches cross the column from one end to the other.
    let (points, _) = sewn_satin(&both_ways, &[("reverse_rails", "none")]);
    assert_eq!((points[0], points[1]), (p(0.0, 0.0), p(10.0, 4.0)));
    // Turned as asked: the second rail, or the first, which sews from its end.
    assert!(straight(&sewn_satin(&both_ways, &[("reverse_rails", "second")]).0));
    let (points, _) = sewn_satin(&both_ways, &[("reverse_rails", "first")]);
    assert!(straight(&points) && points[0] == p(10.0, 0.0), "{:?}", &points[..4]);
    // Rails drawn the same way, both turned: sewn from the other end.
    let (points, _) = sewn_satin(&ladder(10.0, 4.0, &[]), &[("reverse_rails", "both")]);
    assert!(straight(&points) && points[0] == p(10.0, 0.0), "{:?}", &points[..4]);
}

#[test]
fn req_sat_002_swapped_rails_start_on_the_second() {
    // With a rung, and without, whose nodes then pair up.
    for rungs in [&[5.0][..], &[]] {
        let (points, _) = sewn_satin(&ladder(10.0, 4.0, rungs), &[("swap_satin_rails", "true")]);
        assert_eq!((points[0], points[1]), (p(0.0, 4.0), p(0.0, 0.0)), "{rungs:?}");
        assert!(points[2].distance(p(0.4, 4.0)) < 1e-9, "{rungs:?}: {:?}", points[2]);
    }
}

#[test]
fn req_sat_002_a_pointed_column_starts_and_ends_at_its_tips() {
    // A lens, its rails meeting at both ends, with 3 rungs: the first pair is the tip, a pair of no
    // length, and the next is measured from its point. The last pair is within 0.1 mm of the far tip,
    // which then gets none.
    let (top, bottom) = (&[(0.0, 0.0), (10.0, 3.0), (20.0, 0.0)][..], &[(0.0, 0.0), (10.0, -3.0), (20.0, 0.0)][..]);
    let lens = polylines(&[top, bottom, &[(5.0, -3.0), (5.0, 3.0)], &[(10.0, -4.0), (10.0, 4.0)], &[(15.0, -3.0), (15.0, 3.0)]]);
    let (points, warnings) = sewn_satin(&lens, &[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let pairs = across(&points);
    assert_eq!(pairs[0], [p(0.0, 0.0), p(0.0, 0.0)]);
    let [a, b] = *pairs.last().unwrap();
    assert!(20.0 - a.x() < 0.1 && a.x() == b.x(), "{a:?} {b:?}");
    let second = pairs[1];
    assert!((second[0].distance(p(0.0, 0.0)) - 0.4).abs() < 0.02 || (second[1].distance(p(0.0, 0.0)) - 0.4).abs() < 0.02, "{second:?}");
    assert!(pairs.iter().all(|[a, b]| a.y() >= 0.0 && b.y() <= 0.0), "top rail first");
}

#[test]
fn req_sat_002_a_column_without_rungs_pairs_its_rails_nodes() {
    // The lower rail's node at 5 mm goes with the upper rail's at 15 mm: the stitches slant to it, and
    // straighten after it.
    let slanted = polylines(&[&[(0.0, 0.0), (5.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (15.0, 4.0), (20.0, 4.0)]]);
    let (points, warnings) = sewn_satin(&slanted, &[NO_SHORT_STITCHES]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let pairs = across(&points);
    let before: Vec<&[Point; 2]> = pairs.iter().filter(|[a, _]| a.x() < 5.0 - 1e-9).collect();
    assert!(before.len() > 2 && before.iter().all(|[a, b]| (b.x() - 3.0 * a.x()).abs() < 1e-9), "{before:?}");
    let after: Vec<&[Point; 2]> = pairs.iter().filter(|[a, _]| a.x() > 5.0 + 1e-9).collect();
    assert!(after.iter().all(|[a, b]| (b.x() - (15.0 + (a.x() - 5.0) / 3.0)).abs() < 1e-9), "{after:?}");
}

#[test]
fn req_sat_002_rails_of_2_nodes_are_cut_a_fifth_of_a_css_pixel_from_their_starts() {
    // A trapezoid: rails 10 mm and 6 mm long. Each is cut 0.2 px (0.053 mm) from its start, and past the
    // cut the pairs are at equal fractions of what is left of each rail.
    let cut = 0.2 * MM_PER_SVG_PX;
    let (points, _) = sewn_satin(&polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(2.0, 4.0), (8.0, 4.0)]]), &[NO_SHORT_STITCHES]);
    let pairs = across(&points);
    assert!(pairs.len() > 10);
    for [a, b] in &pairs[1..] {
        let (on_a, on_b) = ((a.x() - cut) / (10.0 - cut), (b.x() - 2.0 - cut) / (6.0 - cut));
        assert!((on_a - on_b).abs() < 1e-9, "{a:?} {b:?}");
    }
}

#[test]
fn req_sat_002_where_the_rails_converge_a_pair_moves_to_lie_at_the_spacing() {
    // A V: the rails close in, so a step along them gains less than the spacing across the column, and
    // each pair moves on until it lies a spacing past the one before.
    let v = polylines(&[&[(-5.0, 10.0), (0.0, 0.0)], &[(5.0, 10.0), (0.0, 0.0)]]);
    let mut meter = Budget::DEFAULT.meter();
    let Ok(Shape::Rails(satin)) = recognize(&v, &mut meter).unwrap().shape else { panic!() };
    let params = SatinParams::from_set(&ParamSet::default()).unwrap().params;
    let mut meter = Budget::DEFAULT.meter();
    let lengths = satin_lengths(&ParamSet::default());
    let pairs = across(&satin_stitch(&satin, &params, lengths, &Neighbours::default(), &mut SplitMix64::new(0), &mut meter).unwrap().runs[0]);
    let gaps = gaps(&pairs);
    let inside = &gaps[..gaps.len() - 1];
    assert!(inside.iter().all(|gap| (gap - 0.4).abs() <= 0.02), "within 5 %: {gaps:?}");
    assert!(inside[1..].iter().all(|gap| (gap - 0.4).abs() < 1e-6), "each moved to the spacing: {gaps:?}");
    // The work pins how many places the pairs were tried at: a unit per point measured and per place.
    assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 68);
}

#[test]
fn req_sat_002_a_pair_that_moves_stops_at_its_section_s_end() {
    // The first 9.985 mm narrow from 6 mm to 2, and the rest runs straight, with a rung between. Pairs
    // step 0.371 mm along the lower rail and move to 0.4, until 9.6 mm: from there a step stays inside the
    // section, and the move to 0.4 would leave it. It stops at the rung instead, 0.385 mm on, which is
    // within 5 % of the spacing.
    let narrowing =
        polylines(&[&[(0.0, 0.0), (9.985, 0.0), (19.985, 0.0)], &[(0.0, 6.0), (9.985, 2.0), (19.985, 2.0)], &[(9.985, -1.0), (9.985, 3.0)]]);
    let (points, warnings) = sewn_satin(&narrowing, &[]);
    assert!(warnings.is_empty(), "{warnings:?}");
    let x: Vec<f64> = across(&points).iter().map(|[a, _]| a.x()).collect();
    assert!(x.iter().any(|x| (x - 9.985).abs() < 1e-9), "a pair on the rung: {x:?}");
    assert!(x.iter().any(|x| (x - 10.385).abs() < 1e-6), "and the spacing on from it: {x:?}");
}

#[test]
fn diag_sc_w0210_rails_without_rungs_with_different_numbers_of_nodes_are_named() {
    let uneven = polylines(&[&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (15.0, 4.0), (20.0, 4.0)]]);
    let (points, warnings) = sewn_satin(&uneven, &[]);
    assert_eq!(
        warnings,
        ["warning SC-W0210: This satin column has no rungs, and its rails have 4 and 3 nodes, so they pair up only as far as the rail \
          with fewer goes."]
    );
    // The lower rail's node at 5 mm pairs with the upper's at 15 mm, and its node at 10 mm with none.
    let at_cut = across(&points).into_iter().find(|[a, _]| a.x() > 5.0 - 1e-9).unwrap();
    assert!(at_cut[1].x() >= 15.0 - 1e-9, "{at_cut:?}");
}

#[test]
fn req_sat_002_a_satin_column_is_planned_with_locks_at_both_ends() {
    let column = along("satin", ladder(4.0, 2.0, &[1.0, 2.0, 3.0]), &RED, &[("satin_column", "true")]);
    let outcome = sewn(vec![column]);
    assert!(outcome.diagnostics.is_empty(), "{:?}", messages(&outcome));
    assert_eq!(shape_of(&outcome), "J L4 S22 L4", "11 pairs, every 0.4 mm from 0 to 4");
}

#[test]
fn req_sat_002_satin_stitches_are_charged_to_the_budget() {
    let path = ladder(10.0, 4.0, &[5.0]);
    let mut meter = Budget::DEFAULT.meter();
    let Ok(Shape::Rails(satin)) = recognize(&path, &mut meter).unwrap().shape else { panic!() };
    let (params, lengths) = (SatinParams::from_set(&ParamSet::default()).unwrap().params, satin_lengths(&ParamSet::default()));
    assert!(
        satin_stitch(
            &satin,
            &params,
            lengths,
            &Neighbours::default(),
            &mut SplitMix64::new(0),
            &mut Budget { max_stitches: 10, max_work: 20 }.meter()
        )
        .is_err()
    );
    assert!(satin_stitch(&satin, &params, lengths, &Neighbours::default(), &mut SplitMix64::new(0), &mut Budget::DEFAULT.meter()).is_ok());
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    #[test]
    fn req_sat_002_parallel_rails_are_sewn_at_the_spacing_whatever_cuts_them(
        length in 1.0..40.0_f64,
        width in 0.5..10.0_f64,
        spacing in 0.2..3.0_f64,
        // Rungs at distinct places inside the column, and not exactly 2 of them: 2 rungs across rails
        // that do not meet leave rails and rungs alike (`SC-W0202`).
        rungs in prop::collection::btree_set(1..100_u32, 0..5).prop_filter("2 rungs are a #", |rungs| rungs.len() != 2),
    ) {
        let rungs: Vec<f64> = rungs.iter().map(|&r| f64::from(r) / 100.0 * length).collect();
        let (points, warnings) = sewn_satin(&ladder(length, width, &rungs), &[("zigzag_spacing_mm", &spacing.to_string()), NO_SHORT_STITCHES]);
        prop_assert!(warnings.is_empty(), "{:?}", warnings);
        let pairs = across(&points);
        prop_assert!(
            pairs.iter().all(|[a, b]| a.y() == 0.0 && (b.y() - width).abs() < 1e-9 && (a.x() - b.x()).abs() < 1e-9),
            "rail to rail, straight across"
        );
        prop_assert_eq!(pairs.first().map(|[a, _]| a.x()), Some(0.0));
        let end = pairs.last().map_or(0.0, |[a, _]| a.x());
        prop_assert!(length - end < 0.1 + 1e-9, "the column ends at its end: {} of {}", end, length);
        let gaps = gaps(&pairs);
        if let Some((last, inside)) = gaps.split_last() {
            prop_assert!(inside.iter().all(|gap| (gap - spacing).abs() < 1e-6), "{:?}", gaps);
            prop_assert!(*last < spacing + 1e-6 && *last > 0.0, "{:?}", gaps);
        }
    }
}
