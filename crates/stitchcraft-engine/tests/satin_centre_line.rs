//! A satin column drawn as one path, its centre line (`REQ-SAT-015`). Its stroke's width says how wide the
//! column is. A column no wider than the design's `min_satin_stroke_width_mm`, 1 mm unless the design sets
//! it, is too narrow to stitch across, and is sewn as a stroke instead, as Ink/Stitch sews it.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_engine::design::{DesignSettings, Element, Join, Shape};
use stitchcraft_engine::generate::approach;
use stitchcraft_engine::generators::Approach;
use stitchcraft_testkit::designs::{RED, along, line, messages, p, planned, planned_with, polylines, shape_of, widened};

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
    // With the usual limit it is a column, sewn along its centre line from M4.9.
    let outcome = planned(vec![drawn("wide", 1.5, &satin), ok()]);
    assert_eq!(
        messages(&outcome),
        ["warning SC-W0011: This element is a satin column drawn as its centre line, which this version of StitchCraft does not sew \
          yet, so it is skipped."]
    );
}

#[test]
fn req_sat_015_only_a_path_of_one_subpath_is_a_centre_line_by_its_width() {
    // A second subpath that is a point is left out, and the column is a centre line however narrow, as in
    // Ink/Stitch, which counts the subpaths drawn.
    let element = along("dotted", polylines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(5.0, 5.0)]]), &RED, &[("satin_column", "true")]);
    let outcome = planned(vec![element, ok()]);
    assert_eq!(
        messages(&outcome),
        [
            "warning SC-W0205: Subpath 2 of this satin column is one point, so it is left out.",
            "warning SC-W0011: This element is a satin column drawn as its centre line, which this version of StitchCraft does not \
             sew yet, so it is skipped."
        ]
    );
}

#[test]
fn req_sat_015_a_narrow_column_offers_its_first_point_as_a_stroke_does() {
    let budget = Budget::DEFAULT;
    let narrow = drawn("narrow", 0.5, &[("satin_column", "true")]);
    assert_eq!(approach(&narrow, &DesignSettings::default(), &budget), Some(Approach::Point(p(0.0, 0.0))));
    // A wider one offers nothing until it is sewn (M4.9).
    assert_eq!(approach(&drawn("wide", 3.0, &[("satin_column", "true")]), &DesignSettings::default(), &budget), None);
    // The join plays no part in how narrow a column is.
    let Shape::Stroke { path, .. } = drawn("round", 0.5, &[]).shape else { panic!("a stroke") };
    let round = Element { shape: Shape::Stroke { path, width: Mm::new(0.5).unwrap(), join: Join::Round }, ..narrow };
    assert_eq!(approach(&round, &DesignSettings::default(), &budget), Some(Approach::Point(p(0.0, 0.0))));
}
