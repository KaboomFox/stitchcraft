//! Satin column recognition's conformance cases (`REQ-SAT-005`) and its diagnostics (`SC-E0201`,
//! `SC-W0202`, `SC-W0203`, `SC-W0205`, `SC-W0207`), through `normalize::satin::recognize` and the
//! engine's entry point. The cases follow how Ink/Stitch tells rails from rungs, so a file sews the same
//! in both: pointed columns, the two-rung `#`, columns wider than they are long, and rungs that miss a
//! rail or cross one twice.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, and this extends it to the
// helpers here).
#![allow(clippy::unwrap_used, clippy::panic)]

use proptest::prelude::*;
use stitchcraft_core::{Budget, Code, ElementId, Point};
use stitchcraft_engine::normalize::satin::{Recognition, Satin, Shape, recognize};
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_testkit::designs::{RED, along, line, messages, p, planned as sewn, polylines, shape_of};

/// `parts` recognized as a satin column, with the default budget.
fn recognized(parts: &[&[(f64, f64)]]) -> Recognition {
    recognize(&polylines(parts), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The rails and rungs `parts` are recognized as, and the warnings, as people read them.
fn ladder(parts: &[&[(f64, f64)]]) -> (Satin, Vec<String>) {
    let Recognition { shape, warnings } = recognized(parts);
    let Ok(Shape::Rails(satin)) = shape else { panic!("not rails: {shape:?}") };
    (satin, warnings.iter().map(ToString::to_string).collect())
}

fn points(list: &[(f64, f64)]) -> Vec<Point> {
    list.iter().map(|&(x, y)| p(x, y)).collect()
}

/// Asserts that `found` are the rungs `expected`, but for rounding: a crossing is computed along the
/// rung or along the rail, whichever is drawn first, so it may be a hair off either.
#[track_caller]
fn assert_rungs(found: &[[Point; 2]], expected: &[[(f64, f64); 2]]) {
    let near = |a: Point, (x, y): (f64, f64)| a.distance(p(x, y)) < 1e-12;
    let same = found.len() == expected.len() && found.iter().zip(expected).all(|(f, e)| near(f[0], e[0]) && near(f[1], e[1]));
    assert!(same, "rungs {found:?}, expected {expected:?}");
}

/// Two rails 20 mm long and 4 mm apart, at y = 0 and y = 4.
const LOWER: &[(f64, f64)] = &[(0.0, 0.0), (20.0, 0.0)];
const UPPER: &[(f64, f64)] = &[(0.0, 4.0), (20.0, 4.0)];

#[test]
fn req_sat_005_the_rails_are_the_two_subpaths_that_meet_the_most_others() {
    // Three rungs across two rails, drawn rung, rail, rung, rail, rung; the second rung drawn backwards.
    let (satin, warnings) = ladder(&[&[(5.0, -1.0), (5.0, 5.0)], LOWER, &[(10.0, 5.0), (10.0, -1.0)], UPPER, &[(15.0, -1.0), (15.0, 5.0)]]);
    assert_eq!(satin.rails, [points(LOWER), points(UPPER)], "in the order they are drawn");
    assert_rungs(&satin.rungs, &[[(5.0, 0.0), (5.0, 4.0)], [(10.0, 0.0), (10.0, 4.0)], [(15.0, 0.0), (15.0, 4.0)]]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn req_sat_005_with_one_rung_the_rails_are_the_two_that_meet_only_one_other() {
    // A column wider than it is long: its rung is its longest subpath, and still a rung.
    let (left, right) = (&[(0.0, 0.0), (3.0, 0.0)][..], &[(0.0, 10.0), (3.0, 10.0)][..]);
    let (satin, warnings) = ladder(&[left, &[(1.5, -1.0), (1.5, 11.0)], right]);
    assert_eq!(satin.rails, [points(left), points(right)]);
    assert_rungs(&satin.rungs, &[[(1.5, 0.0), (1.5, 10.0)]]);
    assert!(warnings.is_empty(), "{warnings:?}");
}

#[test]
fn req_sat_005_rails_may_meet_as_a_pointed_column_s_do_at_its_tips() {
    let (top, bottom) = (&[(0.0, 0.0), (10.0, 3.0), (20.0, 0.0)][..], &[(0.0, 0.0), (10.0, -3.0), (20.0, 0.0)][..]);
    // Two rungs: each rail meets the other rail and both rungs.
    let (satin, warnings) = ladder(&[top, &[(5.0, -3.0), (5.0, 3.0)], bottom, &[(15.0, 3.0), (15.0, -3.0)]]);
    assert_eq!(satin.rails, [points(top), points(bottom)]);
    assert_rungs(&satin.rungs, &[[(5.0, 1.5), (5.0, -1.5)], [(15.0, 1.5), (15.0, -1.5)]]);
    assert!(warnings.is_empty(), "{warnings:?}");
    // One rung: each of the three meets the other two, so the two longest are taken, the rails here.
    let (satin, warnings) = ladder(&[top, bottom, &[(5.0, -3.0), (5.0, 3.0)]]);
    assert_eq!(satin.rails, [points(top), points(bottom)]);
    assert_rungs(&satin.rungs, &[[(5.0, 1.5), (5.0, -1.5)]]);
    assert_eq!(
        warnings,
        ["warning SC-W0202: Where the subpaths of this satin column meet does not say which are its rails, so its two longest, \
          subpaths 1 and 2, are taken as the rails."]
    );
}

#[test]
fn diag_sc_w0202_two_rungs_across_rails_that_do_not_meet_are_settled_by_length() {
    // A `#`: each of the four subpaths meets two others, and the two longest are the rails.
    let (satin, warnings) = ladder(&[&[(2.0, -1.0), (2.0, 5.0)], LOWER, UPPER, &[(18.0, -1.0), (18.0, 5.0)]]);
    assert_eq!(satin.rails, [points(LOWER), points(UPPER)]);
    assert_rungs(&satin.rungs, &[[(2.0, 0.0), (2.0, 4.0)], [(18.0, 0.0), (18.0, 4.0)]]);
    assert_eq!(
        warnings,
        ["warning SC-W0202: Where the subpaths of this satin column meet does not say which are its rails, so its two longest, \
          subpaths 2 and 3, are taken as the rails."]
    );
    // Wider than it is long, the same drawing has rungs longer than its rails, and the rungs are taken:
    // the ambiguity the warning names, as in Ink/Stitch. A third rung settles it.
    let (left, right) = (&[(0.0, 0.0), (4.0, 0.0)][..], &[(0.0, 10.0), (4.0, 10.0)][..]);
    let (first, second, third) = (&[(1.0, -1.0), (1.0, 11.0)][..], &[(3.0, -1.0), (3.0, 11.0)][..], &[(2.0, -1.0), (2.0, 11.0)][..]);
    let (satin, warnings) = ladder(&[left, right, first, second]);
    assert_eq!((satin.rails, warnings.len()), ([points(first), points(second)], 1));
    let (satin, warnings) = ladder(&[left, right, first, second, third]);
    assert_eq!((satin.rails, warnings.len()), ([points(left), points(right)], 0));
    // In a square `#`, all four are equally long, and the first two drawn are taken, as in Ink/Stitch.
    let (across, down) = (&[(2.0, -1.0), (2.0, 5.0)][..], &[(-1.0, 0.0), (5.0, 0.0)][..]);
    let (satin, _) = ladder(&[across, down, &[(-1.0, 4.0), (5.0, 4.0)], &[(4.0, -1.0), (4.0, 5.0)]]);
    assert_eq!(satin.rails, [points(across), points(down)]);
}

#[test]
fn diag_sc_w0203_a_rung_that_misses_a_rail_joins_the_point_of_it_nearest_the_rung() {
    let (satin, warnings) = ladder(&[
        LOWER,
        UPPER,
        &[(2.0, -1.0), (2.0, 5.0)],
        &[(10.0, -1.0), (10.0, 3.0)],
        &[(12.0, 1.0), (12.0, 5.0)],
        &[(8.0, 1.0), (7.0, 3.0)],
        &[(18.0, -1.0), (18.0, 5.0)],
    ]);
    assert_eq!(satin.rails, [points(LOWER), points(UPPER)]);
    assert_rungs(
        &satin.rungs,
        &[[(2.0, 0.0), (2.0, 4.0)], [(10.0, 0.0), (10.0, 4.0)], [(12.0, 0.0), (12.0, 4.0)], [(8.0, 0.0), (7.0, 4.0)], [(18.0, 0.0), (18.0, 4.0)]],
    );
    assert_eq!(
        warnings,
        [
            "warning SC-W0203: Subpath 4 of this satin column, a rung, does not reach the rail that is subpath 2, so the point of \
             that rail nearest the rung is used.",
            "warning SC-W0203: Subpath 5 of this satin column, a rung, does not reach the rail that is subpath 1, so the point of \
             that rail nearest the rung is used.",
            "warning SC-W0203: Subpath 6 of this satin column, a rung, reaches neither rail (subpaths 1 and 2), so the points of the \
             rails nearest the rung are used.",
        ]
    );
    // The nearest point may be a rail's end, and the rung's middle may be nearest it.
    let (satin, _) =
        ladder(&[LOWER, UPPER, &[(1.0, -1.0), (1.0, 5.0)], &[(2.0, -1.0), (2.0, 5.0)], &[(21.0, 1.0), (23.0, 3.0)], &[(-1.0, 3.0), (-1.0, 5.0)]]);
    assert_rungs(&satin.rungs[2..], &[[(20.0, 0.0), (20.0, 4.0)], [(0.0, 0.0), (0.0, 4.0)]]);
}

#[test]
fn diag_sc_w0205_a_subpath_of_one_point_is_left_out() {
    // A stray node, and a line that does not move, drawn between the rails.
    let Recognition { shape, warnings } = recognized(&[LOWER, &[(7.0, 2.0)], &[(9.0, 2.0), (9.0, 2.0)], UPPER]);
    assert_eq!(shape, Ok(Shape::Rails(Satin { rails: [points(LOWER), points(UPPER)], rungs: Vec::new() })));
    let shown: Vec<String> = warnings.iter().map(ToString::to_string).collect();
    assert_eq!(
        shown,
        [
            "warning SC-W0205: Subpath 2 of this satin column is one point, so it is left out.",
            "warning SC-W0205: Subpath 3 of this satin column is one point, so it is left out.",
        ]
    );
}

#[test]
fn diag_sc_w0207_a_rung_that_crosses_a_rail_more_than_once_is_left_out() {
    let (satin, warnings) = ladder(&[
        LOWER,
        UPPER,
        &[(2.0, -1.0), (2.0, 5.0)],
        &[(4.0, 1.0), (5.0, -1.0), (6.0, 1.0), (6.0, 5.0)],
        &[(14.0, 3.0), (15.0, 5.0), (16.0, 3.0), (16.0, -1.0)],
        &[(9.0, 2.0), (9.0, 4.0), (11.0, 4.0), (11.0, -1.0)],
        &[(18.0, -1.0), (18.0, 5.0)],
    ]);
    assert_rungs(&satin.rungs, &[[(2.0, 0.0), (2.0, 4.0)], [(18.0, 0.0), (18.0, 4.0)]]);
    let tail = "more than once, so it does not say which points go together. It is left out.";
    assert_eq!(
        warnings,
        [
            format!("warning SC-W0207: Subpath 4 of this satin column, a rung, crosses the rail that is subpath 1 {tail}"),
            format!("warning SC-W0207: Subpath 5 of this satin column, a rung, crosses the rail that is subpath 2 {tail}"),
            format!("warning SC-W0207: Subpath 6 of this satin column, a rung, crosses the rail that is subpath 2 {tail}"),
        ],
        "the last runs along a rail for a while, which is more than one point"
    );
}

#[test]
fn diag_sc_e0201_a_satin_column_with_no_subpath_longer_than_a_point_is_not_sewn() {
    let Recognition { shape, warnings } = recognized(&[&[(3.0, 3.0)]]);
    assert_eq!(shape.unwrap_err().to_string(), "error SC-E0201: This satin column has no subpath longer than a point, so it has no rails.");
    assert_eq!(warnings.len(), 1, "the point is named too");
    assert_eq!(recognized(&[]).shape.unwrap_err().code, Code::SatinWithoutRails, "a path of no subpaths");
    // Through the engine: the element is named, and the rest of the design is sewn.
    let dot = along("dot", polylines(&[&[(3.0, 3.0)]]), &RED, &[("satin_column", "true")]);
    let outcome = sewn(vec![dot, line("ok", (0.0, 5.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&outcome),
        [
            "warning SC-W0205: Subpath 1 of this satin column is one point, so it is left out.",
            "error SC-E0201: This satin column has no subpath longer than a point, so it has no rails."
        ]
    );
    assert!(outcome.diagnostics.iter().all(|d| d.element.as_ref().map(ElementId::as_str) == Some("dot")));
    assert_eq!(shape_of(&outcome), "J L4 S5 L4");
}

#[test]
fn diag_sc_w0011_a_satin_column_is_recognized_then_skipped_until_satin_stitches_are_sewn() {
    let path = polylines(&[
        LOWER,
        UPPER,
        &[(5.0, -1.0), (5.0, 5.0)],
        &[(10.0, -1.0), (10.0, 3.0)],
        &[(15.0, -1.0), (15.0, 5.0)],
        &[(16.0, -1.0), (16.0, 5.0)],
    ]);
    // On, the path is a satin column whatever its stroke method says, and what recognition finds is named.
    let satin = along("satin", path.clone(), &RED, &[("satin_column", "true"), ("stroke_method", "manual_stitch")]);
    let outcome = sewn(vec![satin, line("ok", (0.0, 8.0), 10.0, &RED, &[])]);
    assert_eq!(
        messages(&outcome),
        [
            "warning SC-W0203: Subpath 4 of this satin column, a rung, does not reach the rail that is subpath 2, so the point of \
             that rail nearest the rung is used.",
            "warning SC-W0011: This element is a satin column, and this version of StitchCraft does not sew satin columns yet, so it \
             is skipped."
        ]
    );
    assert_eq!(shape_of(&outcome), "J L4 S5 L4");
    // A path of one subpath is the column's centre line.
    let centre = along("centre", polylines(&[LOWER]), &RED, &[("satin_column", "true")]);
    assert_eq!(
        messages(&sewn(vec![centre, line("ok", (0.0, 8.0), 10.0, &RED, &[])])),
        ["warning SC-W0011: This element is a satin column drawn as its centre line, which this version of StitchCraft does not sew \
          yet, so it is skipped."]
    );
    // Off, the same path is a stroke, sewn subpath by subpath.
    let stroke = sewn(vec![along("stroke", path, &RED, &[("satin_column", "false")])]);
    assert!(stroke.diagnostics.is_empty(), "{:?}", messages(&stroke));
    assert_eq!(shape_of(&stroke), "J L4 S9 L4 J L4 S9 L4 J L4 S4 L4 J L4 S3 L4 J L4 S4 L4 J L4 S4 L4");
    // A setting that does not read is an error, and the element is skipped.
    let unread = sewn(vec![along("x", polylines(&[LOWER]), &RED, &[("satin_column", "maybe")]), line("ok", (0.0, 8.0), 10.0, &RED, &[])]);
    assert_eq!(unread.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::ParamInvalid]);
}

#[test]
fn req_sat_005_recognition_is_charged_to_the_budget() {
    let path = polylines(&[LOWER, UPPER, &[(5.0, -1.0), (5.0, 5.0)], &[(10.0, -1.0), (10.0, 3.0)], &[(15.0, -1.0), (15.0, 5.0)]]);
    let tight = Budget { max_stitches: 10, max_work: 5 };
    assert!(recognize(&path, &mut tight.meter()).is_err());
    let roomy = Budget { max_stitches: 10, max_work: 10_000 };
    assert!(recognize(&path, &mut roomy.meter()).is_ok());
}

/// The distance from `point` to the polyline `line`.
fn distance(point: Point, line: &[Point]) -> f64 {
    line.windows(2).map(|pair| distance_to_segment(point, pair[0], pair[1])).fold(f64::INFINITY, f64::min)
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    #[test]
    fn req_sat_005_ladders_are_recognized_in_any_drawing_order(
        order in prop::collection::vec((any::<u16>(), any::<bool>()), 5..10),
    ) {
        // Two rails, then rungs 2 mm apart from x = 2, drawn in the order of their keys, some backwards.
        let rungs: Vec<[(f64, f64); 2]> = (0..order.len() - 2)
            .map(|k| {
                let x = 2.0 + 2.0 * k as f64;
                if order[k + 2].1 { [(x, 5.0), (x, -1.0)] } else { [(x, -1.0), (x, 5.0)] }
            })
            .collect();
        let long = [(0.0, 0.0), (2.0 * order.len() as f64, 0.0)];
        let wide = [(0.0, 4.0), (2.0 * order.len() as f64, 4.0)];
        let mut parts: Vec<(u16, &[(f64, f64)])> = vec![(order[0].0, &long[..]), (order[1].0, &wide[..])];
        parts.extend(rungs.iter().enumerate().map(|(k, rung)| (order[k + 2].0, &rung[..])));
        parts.sort_by_key(|&(key, _)| key);
        let drawn: Vec<&[(f64, f64)]> = parts.iter().map(|&(_, part)| part).collect();
        let (satin, warnings) = ladder(&drawn);
        prop_assert!(warnings.is_empty(), "{:?}", warnings);
        let lower_first = drawn.iter().position(|part| part[0] == long[0]) < drawn.iter().position(|part| part[0] == wide[0]);
        let expected = if lower_first { [points(&long), points(&wide)] } else { [points(&wide), points(&long)] };
        prop_assert_eq!(&satin.rails, &expected);
        prop_assert_eq!(satin.rungs.len(), rungs.len());
        for [on_first, on_second] in &satin.rungs {
            prop_assert!(distance(*on_first, &satin.rails[0]) < 1e-9 && distance(*on_second, &satin.rails[1]) < 1e-9);
            prop_assert!((on_first.x() - on_second.x()).abs() < 1e-9, "a rung joins the points straight across");
        }
    }

    #[test]
    fn req_sat_005_any_drawing_is_recognized_without_a_crash(
        parts in prop::collection::vec(prop::collection::vec((-10.0..10.0_f64, -10.0..10.0_f64), 1..5), 0..7),
    ) {
        let drawn: Vec<&[(f64, f64)]> = parts.iter().map(Vec::as_slice).collect();
        let Recognition { shape, warnings } = recognized(&drawn);
        let usable = parts.iter().filter(|part| part.len() > 1).count();
        let count = |code: Code| warnings.iter().filter(|w| w.code == code).count();
        prop_assert_eq!(count(Code::SatinSubpathPoint), parts.len() - usable);
        match shape {
            Err(error) => prop_assert_eq!((error.code, usable), (Code::SatinWithoutRails, 0)),
            Ok(Shape::CentreLine) => prop_assert_eq!(usable, 1),
            Ok(Shape::Rails(satin)) => {
                prop_assert!(usable >= 2);
                let lines: Vec<Vec<Point>> = parts.iter().map(|part| points(part)).collect();
                prop_assert!(satin.rails.iter().all(|rail| lines.contains(rail)), "each rail is a subpath as drawn");
                prop_assert_eq!(satin.rungs.len() + count(Code::SatinRungAmbiguous), usable - 2, "every other subpath is a rung or left out");
                // On the rail, but for rounding: a crossing is computed along the rung or the rail.
                for [on_first, on_second] in &satin.rungs {
                    prop_assert!(distance(*on_first, &satin.rails[0]) < 1e-6, "{:?} {:?}", on_first, satin.rails[0]);
                    prop_assert!(distance(*on_second, &satin.rails[1]) < 1e-6, "{:?} {:?}", on_second, satin.rails[1]);
                }
            }
        }
    }
}
