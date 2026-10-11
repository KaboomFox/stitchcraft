//! A satin column drawn as one path, its centre line. Its stroke's width says how wide the column is
//! (`REQ-SAT-015`). A column no wider than the design's `min_satin_stroke_width_mm`, 1 mm unless the design
//! sets it, is too narrow to stitch across, and is sewn as a stroke instead, as Ink/Stitch sews it. A wider
//! one is sewn between 2 rails, its line offset by half its width to each side with the stroke's join
//! (`REQ-SAT-016`), and rungs placed as Ink/Stitch places them (`REQ-SAT-017`).
//!
//! The line's points are rounded to a ten-thousandth of a CSS pixel before its rails are made, as
//! Ink/Stitch rounds them, so the rails and rungs are compared within a ten-thousandth of a millimetre.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Mm, Point, math};
use stitchcraft_engine::design::{DesignSettings, Element, Join, Path, Segment, Shape, Subpath};
use stitchcraft_engine::generate::approach;
use stitchcraft_engine::generators::Approach;
use stitchcraft_engine::generators::satin::rails;
use stitchcraft_engine::normalize::satin::{Pairing, Satin, Shape as SatinShape, recognize};
use stitchcraft_testkit::designs::{RED, along, line, messages, p, planned, planned_with, polylines, shape_of, widened};

/// How near a point made from a centre line is to the one expected.
const NEAR: f64 = 1e-4;

/// A line 10 mm long along x, drawn as a stroke `width` millimetres wide, with `params`.
fn drawn(id: &str, width: f64, params: &[(&str, &str)]) -> Element {
    widened(along(id, polylines(&[&[(0.0, 0.0), (10.0, 0.0)]]), &RED, params), width)
}

/// A plain stroke beside the column, so the design has something to sew when the column is skipped.
fn ok() -> Element {
    line("ok", (0.0, 8.0), 10.0, &RED, &[])
}

/// The needle points the design sews, in order.
fn needle_points(outcome: &stitchcraft_engine::PlanOutcome) -> Vec<Point> {
    outcome.plan.as_ref().unwrap().stitches().map(|s| s.at).collect()
}

/// A path of one subpath through `points`, closed with Z.
fn closed(points: &[(f64, f64)]) -> Path {
    let (&(x, y), rest) = points.split_first().unwrap();
    Path { subpaths: vec![Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: true }] }
}

/// The rails and rungs made from `path`, a satin column drawn as one path `width` mm wide turning its
/// corners with `join`, and what was said of it.
fn made(path: &Path, width: f64, join: Join) -> (Option<Satin>, Vec<String>) {
    let mut diagnostics = Vec::new();
    let found = rails(path, Mm::new(width).unwrap(), join, &mut diagnostics, &mut Budget::DEFAULT.meter()).unwrap();
    (found, diagnostics.iter().map(ToString::to_string).collect())
}

/// Whether `found` are the points `expected`, each within [`NEAR`].
fn near(found: &[Point], expected: &[(f64, f64)]) -> bool {
    found.len() == expected.len() && found.iter().zip(expected).all(|(a, &(x, y))| a.distance(p(x, y)) < NEAR)
}

/// Whether `pairing` is the rungs `expected`, each the points it pairs on the first and second rail.
fn rungs_near(pairing: &Pairing, expected: &[[(f64, f64); 2]]) -> bool {
    let Pairing::Rungs(rungs) = pairing else { return false };
    rungs.len() == expected.len() && rungs.iter().zip(expected).all(|(found, wanted)| near(found, wanted))
}

#[test]
fn req_sat_015_a_column_too_narrow_to_stitch_across_is_sewn_as_a_stroke() {
    let satin = [("satin_column", "true")];
    // 0.5 mm wide: the stroke's stitches, the same as with satin_column off.
    let narrow = planned(vec![drawn("narrow", 0.5, &satin)]);
    let stroke = planned(vec![drawn("narrow", 0.5, &[])]);
    assert_eq!(needle_points(&narrow), needle_points(&stroke));
    // Exactly the limit is no wider than it.
    let edge = planned(vec![drawn("edge", 1.0, &satin)]);
    assert_eq!(needle_points(&edge), needle_points(&planned(vec![drawn("edge", 1.0, &[])])));
    // Sewn by its stroke settings: here a running stitch of 2 mm.
    let two = [("satin_column", "true"), ("running_stitch_length_mm", "2")];
    assert_eq!(shape_of(&planned(vec![drawn("two", 0.5, &two)])), "J L4 S6 L4");
}

#[test]
fn diag_sc_w0212_the_width_and_the_limit_are_named() {
    let narrow = planned(vec![drawn("narrow", 0.5, &[("satin_column", "true")])]);
    assert_eq!(
        messages(&narrow),
        ["warning SC-W0212: This satin column is drawn as one path, and its stroke, 0.5 mm wide, is no wider than the design's \
          limit of 1 mm, so it is sewn as a stroke."]
    );
    // A stroke says nothing of its width.
    assert_eq!(messages(&planned(vec![drawn("stroke", 0.5, &[])])), [] as [String; 0]);
}

#[test]
fn req_sat_015_the_design_says_how_narrow() {
    let satin = [("satin_column", "true")];
    let wider = DesignSettings { min_satin_stroke_width: Mm::new(2.0).unwrap(), origin: Some(p(0.0, 0.0)), ..DesignSettings::default() };
    let outcome = planned_with(vec![drawn("wide", 1.5, &satin), ok()], wider);
    assert_eq!(
        messages(&outcome),
        ["warning SC-W0212: This satin column is drawn as one path, and its stroke, 1.5 mm wide, is no wider than the design's \
          limit of 2 mm, so it is sewn as a stroke."]
    );
    // With the usual limit it is a column, sewn between rails made from its line.
    let outcome = planned(vec![drawn("wide", 1.5, &satin), ok()]);
    assert_eq!(messages(&outcome), [] as [String; 0]);
    assert_ne!(needle_points(&outcome), needle_points(&planned(vec![drawn("wide", 1.5, &[]), ok()])));
}

#[test]
fn req_sat_015_only_a_path_of_one_subpath_is_a_centre_line_by_its_width() {
    // A second subpath that is a point is left out, and the column is a centre line however narrow, as in
    // Ink/Stitch, which counts the subpaths drawn: here its rails are the stroke's 1 CSS pixel apart, too
    // close for the machine to sew across.
    let element = along("dotted", polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(5.0, 5.0)]]), &RED, &[("satin_column", "true")]);
    let outcome = planned(vec![element, ok()]);
    assert_eq!(
        messages(&outcome),
        [
            "warning SC-W0205: Subpath 2 of this satin column is one point, so it is left out.",
            "info SC-I0504: 26 needle points less than the shortest stitch (0.3 mm) from the one before were left out."
        ]
    );
    let alone = planned(vec![along("alone", polylines(&[&[(0.0, 0.0), (10.0, 0.0)]]), &RED, &[("satin_column", "true")]), ok()]);
    assert_ne!(needle_points(&outcome), needle_points(&alone), "the column, not the stroke a lone line that narrow is");
}

#[test]
fn req_sat_015_a_narrow_column_offers_its_first_point_as_a_stroke_does() {
    let budget = Budget::DEFAULT;
    let narrow = drawn("narrow", 0.5, &[("satin_column", "true")]);
    assert_eq!(approach(&narrow, &DesignSettings::default(), &budget), Some(Approach::Point(p(0.0, 0.0))));
    // The join plays no part in how narrow a column is.
    let Shape::Stroke { path, .. } = drawn("round", 0.5, &[]).shape else { panic!("a stroke") };
    let round = Element { shape: Shape::Stroke { path, width: Mm::new(0.5).unwrap(), join: Join::Round }, ..narrow };
    assert_eq!(approach(&round, &DesignSettings::default(), &budget), Some(Approach::Point(p(0.0, 0.0))));
}

#[test]
fn req_sat_016_a_wider_column_offers_its_rails_as_a_drawn_one_does() {
    // It starts at its nearest point unless set otherwise, so it offers its rails as they are sewn.
    let wide = drawn("wide", 3.0, &[("satin_column", "true")]);
    let Some(Approach::Shape(lines)) = approach(&wide, &DesignSettings::default(), &Budget::DEFAULT) else { panic!("its rails") };
    assert!(lines.len() == 2 && near(&lines[0], &[(0.0, -1.5), (10.0, -1.5)]) && near(&lines[1], &[(0.0, 1.5), (10.0, 1.5)]), "{lines:?}");
}

#[test]
fn req_sat_016_the_rails_are_the_line_offset_by_half_the_width_to_each_side() {
    // The first rail lies to the right of the line as it runs in y-up axes, the second to its left.
    let (satin, said) = made(&polylines(&[&[(0.0, 0.0), (10.0, 0.0)]]), 3.0, Join::UNSET);
    let satin = satin.unwrap();
    assert!(near(&satin.rails[0], &[(0.0, -1.5), (10.0, -1.5)]) && near(&satin.rails[1], &[(0.0, 1.5), (10.0, 1.5)]), "{satin:?}");
    assert_eq!(said, [] as [String; 0]);
}

#[test]
fn req_sat_016_the_rails_turn_their_corners_with_the_stroke_s_join() {
    let corner = polylines(&[&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0)]]);
    let rails_with = |join| made(&corner, 4.0, join).0.unwrap().rails;
    // Inside the corner the offsets meet, whatever the join.
    for join in [Join::UNSET, Join::Bevel, Join::Round, Join::Miter { limit: 1.0 }] {
        assert!(near(&rails_with(join)[1], &[(0.0, 2.0), (38.0, 2.0), (38.0, 40.0)]), "{join:?}");
    }
    // Outside it, the mitre's point lies 2.83 mm from the corner, within 5 half widths.
    assert!(near(&rails_with(Join::UNSET)[0], &[(0.0, -2.0), (42.0, -2.0), (42.0, 40.0)]));
    assert!(near(&rails_with(Join::Bevel)[0], &[(0.0, -2.0), (40.0, -2.0), (42.0, 0.0), (42.0, 40.0)]));
    // A limit of 1 cuts the mitre square, 1 half width from the corner.
    assert!(near(&rails_with(Join::Miter { limit: 1.0 })[0], &[(0.0, -2.0), (40.828_427, -2.0), (42.0, -0.828_427), (42.0, 40.0)]));
    // Round: a quarter circle about the corner, in 16 steps.
    let round = &rails_with(Join::Round)[0];
    assert!(near(&round[..2], &[(0.0, -2.0), (40.0, -2.0)]) && near(&round[17..], &[(42.0, 0.0), (42.0, 40.0)]), "{round:?}");
    assert!(round.len() == 19 && round[1..18].iter().all(|q| (q.distance(p(40.0, 0.0)) - 2.0).abs() < NEAR), "{round:?}");
}

#[test]
fn req_sat_016_a_column_drawn_as_one_path_sews_as_its_rails_drawn() {
    let satin = [("satin_column", "true")];
    let one = planned(vec![drawn("one", 3.0, &satin)]);
    let rails = polylines(&[&[(0.0, -1.5), (10.0, -1.5)], &[(0.0, 1.5), (10.0, 1.5)], &[(5.0, -1.8), (5.0, 1.8)]]);
    let two = planned(vec![along("two", rails, &RED, &satin)]);
    let (one, two) = (needle_points(&one), needle_points(&two));
    assert!(one.len() == two.len() && one.iter().zip(&two).all(|(a, b)| a.distance(*b) < NEAR), "{one:?}\n{two:?}");
}

#[test]
fn req_sat_017_rungs_go_beside_sharp_corners_and_at_nodes() {
    // A line of 2 nodes: one rung, in its middle, pairing the points where it crosses the rails.
    let straight = made(&polylines(&[&[(0.0, 0.0), (10.0, 0.0)]]), 3.0, Join::UNSET).0.unwrap();
    assert!(rungs_near(&straight.pairing, &[[(5.0, -1.5), (5.0, 1.5)]]), "{:?}", straight.pairing);
    // Round a corner: 5 hundredths of the line's length before it and after it. The rung at the corner's
    // node crosses only the outer rail, and is left out.
    let corner = made(&polylines(&[&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0)]]), 4.0, Join::UNSET).0.unwrap();
    assert!(rungs_near(&corner.pairing, &[[(36.0, -2.0), (36.0, 2.0)], [(42.0, 4.0), (38.0, 4.0)]]), "{:?}", corner.pairing);
    // Nodes less than 1 mm apart get one rung.
    let close = made(&polylines(&[&[(0.0, 0.0), (5.0, 0.0), (5.5, 0.0), (10.0, 0.0)]]), 3.0, Join::UNSET).0.unwrap();
    assert!(rungs_near(&close.pairing, &[[(5.0, -1.5), (5.0, 1.5)]]), "{:?}", close.pairing);
}

#[test]
fn req_sat_017_the_rails_made_stay_the_rails() {
    // A dash 3.3 mm long and 5 mm wide, with 2 nodes inside: 2 rungs, each 6 mm long, longer than the
    // rails.
    let dash = [(0.0, 0.0), (1.1, 0.0), (2.2, 0.0), (3.3, 0.0)];
    let satin = made(&polylines(&[&dash]), 5.0, Join::UNSET).0.unwrap();
    assert!(near(&satin.rails[0], &[(0.0, -2.5), (3.3, -2.5)]) && near(&satin.rails[1], &[(0.0, 2.5), (3.3, 2.5)]), "{satin:?}");
    assert!(rungs_near(&satin.pairing, &[[(1.1, -2.5), (1.1, 2.5)], [(2.2, -2.5), (2.2, 2.5)]]), "{:?}", satin.pairing);
    // Drawn as subpaths, the same rails and rungs are told apart by where they meet and how long they are,
    // and the rungs are taken for the rails (DEV-SAT-007).
    let subpaths = polylines(&[&[(0.0, -2.5), (3.3, -2.5)], &[(0.0, 2.5), (3.3, 2.5)], &[(1.1, 3.0), (1.1, -3.0)], &[(2.2, 3.0), (2.2, -3.0)]]);
    let found = recognize(&subpaths, &mut Budget::DEFAULT.meter()).unwrap();
    let Ok(SatinShape::Rails(drawn)) = found.shape else { panic!("rails") };
    assert!(near(&drawn.rails[0], &[(1.1, 3.0), (1.1, -3.0)]), "{:?}", drawn.rails);
    assert_eq!(found.warnings.iter().map(|w| w.code).collect::<Vec<_>>(), [Code::SatinRailsByLength]);
}

#[test]
fn req_sat_017_a_closed_line_starts_away_from_where_it_crosses_itself() {
    // A bow tie crosses itself at the middle of its first segment. Closed with Z, it starts on its second
    // segment, from (10, 10) to (10, 0): rolled to start at that segment's middle, and then, ending where
    // it starts, halfway on to (10, 0).
    let bow = [(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0)];
    let rails = made(&closed(&bow), 2.0, Join::UNSET).0.unwrap().rails;
    assert!(near(&rails[0][..1], &[(9.0, 2.5)]) && near(&rails[1][..1], &[(11.0, 2.5)]), "{rails:?}");
    // Drawn back to its start without Z, it is not closed, and starts at the middle of its first segment.
    let open = polylines(&[&[(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0), (0.0, 0.0)]]);
    let rails = made(&open, 2.0, Join::UNSET).0.unwrap().rails;
    let side = std::f64::consts::FRAC_1_SQRT_2;
    assert!(near(&rails[0][..1], &[(5.0 + side, 5.0 - side)]) && near(&rails[1][..1], &[(5.0 - side, 5.0 + side)]), "{rails:?}");
}

#[test]
fn req_sat_017_a_line_that_crosses_itself_is_made_in_halves() {
    // Each half by length is made on its own, and the halves are joined where they meet, at (20, 10), with
    // a rung pairing the second half's first points. The other rungs near the X's sharp corners cross only
    // one rail.
    let x = polylines(&[&[(0.0, 0.0), (20.0, 20.0), (20.0, 0.0), (0.0, 20.0)]]);
    let (satin, said) = made(&x, 2.0, Join::UNSET);
    let satin = satin.unwrap();
    // Each corner of a rail is where the offsets of the segments either side of it meet.
    let (r, s) = (std::f64::consts::FRAC_1_SQRT_2, std::f64::consts::SQRT_2);
    assert!(near(&satin.rails[0], &[(r, -r), (19.0, 19.0 - s), (19.0, 10.0), (19.0, 1.0 + s), (r, 20.0 + r)]), "{:?}", satin.rails);
    assert!(near(&satin.rails[1], &[(-r, r), (21.0, 21.0 + s), (21.0, 10.0), (21.0, -1.0 - s), (-r, 20.0 - r)]), "{:?}", satin.rails);
    assert!(rungs_near(&satin.pairing, &[[(19.0, 10.0), (21.0, 10.0)]]), "{:?}", satin.pairing);
    assert_eq!(said, [] as [String; 0]);
    // A line that goes straight back along itself is not cut: each offset turns round its tip in one
    // curve, as shapely's does, and the column folds back on itself.
    let back = made(&polylines(&[&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0)]]), 2.0, Join::UNSET).0.unwrap();
    assert!(near(&back.rails[0], &[(0.0, -1.0), (5.0, -1.0), (10.0, -1.0), (10.0, 1.0), (5.0, 1.0)]), "{:?}", back.rails);
    assert!(near(&back.rails[1], &[(0.0, 1.0), (5.0, 1.0), (10.0, 1.0), (10.0, -1.0), (5.0, -1.0)]), "{:?}", back.rails);
}

/// A line along x from 0 to `end`, with a loop of radius 0.3 mm at each of `loops`: up and round
/// counter-clockwise, 12 segments to a turn.
fn looped_line(loops: &[f64], end: f64) -> Path {
    let mut points = vec![(0.0, 0.0)];
    for &x in loops {
        points.extend((0..=12).map(|i| {
            let angle = -std::f64::consts::FRAC_PI_2 + std::f64::consts::TAU * f64::from(i) / 12.0;
            (x + 0.3 * math::cos(angle), 0.3 + 0.3 * math::sin(angle))
        }));
    }
    points.push((end, 0.0));
    polylines(&[&points])
}

#[test]
fn diag_sc_w0213_parts_left_out_are_counted() {
    // A U 1 mm across, 21 mm long, then a line 22 mm long back across its first leg. The line crosses
    // itself, so it is cut in half: the U and the first 0.5 mm of the line, whose offset inside the bend
    // vanishes, and the rest of the line, which is the column.
    let bent = polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0), (13.2, -16.6)]]);
    let (satin, said) = made(&bent, 2.0, Join::UNSET);
    assert_eq!(
        said,
        ["warning SC-W0213: 1 part of this satin column drawn as one path crosses itself or turns more tightly than the column is \
          wide, so it is left out."]
    );
    let satin = satin.unwrap();
    assert!(near(&satin.rails[0], &[(-0.5, 0.0), (12.4, -17.2)]) && near(&satin.rails[1], &[(1.1, 1.2), (14.0, -16.0)]), "{satin:?}");
    assert!(rungs_near(&satin.pairing, &[[(5.95, -8.6), (7.55, -7.4)]]), "{:?}", satin.pairing);
    // Loops 0.6 mm across in a column 3 mm wide: the line is cut down to arcs of them, too tight for an
    // offset inside, at 2 places.
    let loops = looped_line(&[10.0, 20.0], 30.0);
    assert_eq!(
        made(&loops, 3.0, Join::UNSET).1,
        ["warning SC-W0213: 2 parts of this satin column drawn as one path cross themselves or turn more tightly than the column \
          is wide, so they are left out."]
    );
    // The rest is sewn.
    let outcome = planned(vec![widened(along("loops", loops, &RED, &[("satin_column", "true")]), 3.0)]);
    assert!(shape_of(&outcome).starts_with("J L4 S"), "{}", shape_of(&outcome));
}

#[test]
fn diag_sc_e0214_a_line_too_short_or_too_tight_for_its_width_has_no_rails() {
    let message = "error SC-E0214: This satin column is drawn as one path, and its rails cannot be made from it: the line is too \
                   short or turns too tightly for the stroke's width.";
    // A U 1 mm across, in a column 2 mm wide: the offset inside it vanishes.
    let tight = polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)]]);
    assert_eq!(made(&tight, 2.0, Join::UNSET), (None, vec![message.to_string()]));
    // A line shorter than a CSS pixel.
    let short = polylines(&[&[(0.0, 0.0), (0.2, 0.0)]]);
    assert_eq!(made(&short, 3.0, Join::UNSET), (None, vec![message.to_string()]));
    // The design goes on without it.
    let outcome = planned(vec![widened(along("tight", tight, &RED, &[("satin_column", "true")]), 2.0), ok()]);
    assert_eq!(messages(&outcome), [message]);
    assert_eq!(shape_of(&outcome), shape_of(&planned(vec![ok()])));
}
