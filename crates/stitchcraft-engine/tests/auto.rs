//! Automatic digitizing (`REQ-AUTO-001` to `REQ-AUTO-004`): a shape without settings sews by its width. An
//! area is a tatami fill; a line no wider than the design's narrowest satin stroke a running stitch, a line
//! up to 7 mm wide a satin column as wide as the line, its caps included, and a wider line the tatami fill
//! of the area it paints. `SC-I0901` counts the shapes digitized, by their stitches.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Mm, Point, math};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::auto::{SATIN_MAX_WIDTH, Stitches, digitize};
use stitchcraft_engine::design::{Cap, DesignSettings, Element, FillRule, Join, Path, Segment, Shape, Subpath};
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::normalize::stroke::SHAPE_TOLERANCE;
use stitchcraft_params::ParamSet;
use stitchcraft_testkit::designs::{RED, along, automatic, capped, fill, messages, p, planned, polylines, widened};

/// The needle points the design sews, in order.
fn needle_points(outcome: &PlanOutcome) -> Vec<Point> {
    outcome.plan.as_ref().unwrap_or_else(|| panic!("no plan: {:?}", messages(outcome))).stitches().map(|s| s.at).collect()
}

/// A line along `points`, drawn `width` millimetres wide with no settings.
fn drawn(id: &str, points: &[(f64, f64)], width: f64) -> Element {
    widened(along(id, polylines(&[points]), &RED, &[]), width)
}

/// `element` with the settings `params` added.
fn with(element: Element, params: &[(&str, &str)]) -> Element {
    let mut set: ParamSet = element.params.clone();
    for (key, value) in params {
        set.set(*key, *value);
    }
    Element { params: set, ..element }
}

/// The ordinary elements the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Vec<Element> {
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// `element`, a stroke, turning its corners with `join`.
fn joined(element: &Element, join: Join) -> Element {
    let Shape::Stroke { path, width, cap, .. } = element.shape.clone() else { panic!("`{}` is not a stroke", element.id) };
    Element { shape: Shape::Stroke { path, width, join, cap }, ..element.clone() }
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

/// The stitches of a stroke `width` millimetres wide, in a design with the default settings.
fn stitches(width: f64) -> Stitches {
    Stitches::of_stroke(Mm::new(width).unwrap(), &DesignSettings::default())
}

#[test]
fn req_auto_002_an_area_fills_and_a_line_up_to_the_narrowest_satin_runs() {
    // An automatic area sews as the same area without settings does: a tatami fill.
    let square = fill("square", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]], &RED, &[]);
    assert_eq!(needle_points(&planned(vec![automatic(square.clone())])), needle_points(&planned(vec![square.clone()])));
    assert_eq!(made(&square), vec![square]);
    // A line no wider than 1 mm, the narrowest satin stroke unless the design sets it, runs: as the same line
    // without settings does, exactly 1 mm included.
    for width in [0.5, 1.0] {
        let line = drawn("line", &[(0.0, 0.0), (20.0, 0.0), (20.0, 15.0)], width);
        assert_eq!(stitches(width), Stitches::Running, "{width} mm");
        assert_eq!(needle_points(&planned(vec![automatic(line.clone())])), needle_points(&planned(vec![line])), "{width} mm");
    }
    // The design's own narrowest satin stroke moves the bound.
    let settings = DesignSettings { min_satin_stroke_width: Mm::new(2.0).unwrap(), ..DesignSettings::default() };
    assert_eq!(Stitches::of_stroke(Mm::new(1.5).unwrap(), &settings), Stitches::Running);
    assert_eq!(Stitches::of_stroke(Mm::new(2.5).unwrap(), &settings), Stitches::Satin);
}

#[test]
fn req_auto_003_a_line_up_to_7_mm_is_a_satin_column_as_wide_as_it_is_drawn() {
    assert_eq!(SATIN_MAX_WIDTH, Mm::new(7.0).unwrap());
    assert_eq!(
        (stitches(1.01), stitches(3.0), stitches(7.0), stitches(7.01)),
        (Stitches::Satin, Stitches::Satin, Stitches::Satin, Stitches::FilledStroke)
    );
    assert!(Stitches::Satin.across() && Stitches::FilledStroke.across() && !Stitches::Running.across() && !Stitches::Fill.across());
    // A line 3 mm wide sews as the satin column drawn as that line, keeping its other settings.
    let line = with(drawn("line", &[(0.0, 0.0), (20.0, 0.0), (20.0, 15.0)], 3.0), &[("trim_after", "true")]);
    let column = with(line.clone(), &[("satin_column", "true")]);
    assert_eq!(made(&line), vec![column.clone()]);
    let sewn = needle_points(&planned(vec![automatic(line)]));
    assert_eq!(sewn, needle_points(&planned(vec![column])));
    // As wide as drawn: its stitches cross the line from one edge to the other, 1.5 mm each side of it.
    let off_the_first_leg = sewn.iter().filter(|q| q.x() < 15.0).map(|q| q.y().abs()).fold(0.0, f64::max);
    assert!((off_the_first_leg - 1.5).abs() < 0.05, "{off_the_first_leg}");
}

#[test]
fn req_auto_003_a_line_of_several_subpaths_is_a_column_for_each() {
    let two = widened(along("two", polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 10.0), (20.0, 10.0)]]), &RED, &[]), 3.0);
    let columns = made(&two);
    let ids: Vec<&str> = columns.iter().map(|c| c.id.as_str()).collect();
    assert_eq!(ids, ["two", "two#2"]);
    let apart = [
        with(widened(along("two", polylines(&[&[(0.0, 0.0), (20.0, 0.0)]]), &RED, &[]), 3.0), &[("satin_column", "true")]),
        with(widened(along("two#2", polylines(&[&[(0.0, 10.0), (20.0, 10.0)]]), &RED, &[]), 3.0), &[("satin_column", "true")]),
    ];
    assert_eq!(columns, apart);
    assert_eq!(needle_points(&planned(vec![automatic(two)])), needle_points(&planned(apart.to_vec())));
}

#[test]
fn req_auto_003_a_column_s_number_steps_aside_for_an_element_that_has_it() {
    let two = widened(along("two", polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 10.0), (20.0, 10.0)]]), &RED, &[]), 3.0);
    let other = drawn("two#2", &[(0.0, 20.0), (20.0, 20.0)], 0.5);
    let outcome = planned(vec![automatic(two), other]);
    let ids: Vec<&str> = outcome.plan.as_ref().unwrap().elements.iter().map(|e| e.as_str()).collect();
    assert_eq!(ids, ["two", "two#2~2", "two#2"]);
    // A stroke of no subpath is one column, as the same column set by hand.
    let none = widened(along("none", Path::default(), &RED, &[]), 3.0);
    assert_eq!(made(&none), vec![with(none, &[("satin_column", "true")])]);
}

#[test]
fn req_auto_003_round_and_square_caps_reach_half_the_width_past_an_open_end() {
    let line = drawn("line", &[(0.0, 0.0), (20.0, 0.0)], 3.0);
    for cap in [Cap::Round, Cap::Square] {
        let [column] = made(&capped(line.clone(), cap)).try_into().unwrap();
        let Shape::Stroke { path, .. } = &column.shape else { panic!("{:?}", column.shape) };
        let reach = Subpath {
            start: p(-1.5, 0.0),
            segments: vec![Segment::Line(p(0.0, 0.0)), Segment::Line(p(20.0, 0.0)), Segment::Line(p(21.5, 0.0))],
            closed: false,
        };
        assert_eq!(path, &Path { subpaths: vec![reach] }, "{cap:?}");
        let xs: Vec<f64> = needle_points(&planned(vec![automatic(capped(line.clone(), cap))])).iter().map(|q| q.x()).collect();
        let (least, most) = (xs.iter().copied().fold(f64::MAX, f64::min), xs.iter().copied().fold(f64::MIN, f64::max));
        assert!((least + 1.5).abs() < 0.05 && (most - 21.5).abs() < 0.05, "{cap:?}: {least} to {most}");
    }
    // A butt cap ends at the path's end, and a closed line has no end.
    assert_eq!(made(&line), vec![with(line.clone(), &[("satin_column", "true")])]);
    let ring = widened(
        along(
            "ring",
            Path { subpaths: vec![Subpath { closed: true, ..polylines(&[&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)]]).subpaths[0].clone() }] },
            &RED,
            &[],
        ),
        3.0,
    );
    let round = capped(ring, Cap::Round);
    assert_eq!(made(&round), vec![with(round, &[("satin_column", "true")])]);
    // Where the path starts with a curve, the cap reaches on along the curve's tangent there.
    let curve = Path {
        subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0))], closed: false }],
    };
    let [column] = made(&capped(widened(along("curve", curve, &RED, &[]), 2.0), Cap::Square)).try_into().unwrap();
    let Shape::Stroke { path, .. } = &column.shape else { panic!("{:?}", column.shape) };
    assert_eq!(path.subpaths[0].start, p(-1.0, 0.0));
    assert_eq!(path.subpaths[0].segments.last(), Some(&Segment::Line(p(10.0, 11.0))));
}

#[test]
fn req_auto_004_a_line_wider_than_7_mm_fills_the_area_it_paints() {
    // 10 mm wide and 30 mm long: a rectangle, longer by the caps that reach past its ends. Arcs are drawn
    // with chords within the shapes' tolerance of them, which leave out less than that times their length.
    let straight = drawn("wide", &[(0.0, 0.0), (30.0, 0.0)], 10.0);
    let (half_disc, half_circle) = (math::to_radians(90.0) * 25.0, math::to_radians(180.0) * 5.0);
    let near = |found: f64, expected: f64, arcs: f64| found <= expected + 1e-6 && found >= expected - SHAPE_TOLERANCE * arcs - 1e-6;
    for (cap, expected, arcs) in [(Cap::Butt, 300.0, 0.0), (Cap::Square, 400.0, 0.0), (Cap::Round, 300.0 + 2.0 * half_disc, 2.0 * half_circle)] {
        let [filled] = made(&capped(straight.clone(), cap)).try_into().unwrap();
        assert_eq!((filled.id.as_str(), filled.params.clone(), filled.automatic), ("wide", ParamSet::new(), false));
        assert!(matches!(filled.shape, Shape::Fill { rule: FillRule::NonZero, .. }), "{:?}", filled.shape);
        assert!(near(area(&filled), expected, arcs), "{cap:?}: {} mm², not {expected}", area(&filled));
    }
    // An L of 2 legs 20 mm long: their rectangles, and the corner the join fills outside them.
    let corner = drawn("corner", &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)], 10.0);
    let joins = [(Join::Miter { limit: 4.0 }, 400.0, 0.0), (Join::Bevel, 387.5, 0.0), (Join::Round, 375.0 + half_disc / 2.0, half_circle / 2.0)];
    for (join, expected, arcs) in joins {
        let [filled] = made(&joined(&corner, join)).try_into().unwrap();
        assert!(near(area(&filled), expected, arcs), "{join:?}: {} mm², not {expected}", area(&filled));
    }
    // A miter longer than its limit is cut square: a bevel.
    let limited = joined(&corner, Join::Miter { limit: 1.2 });
    assert!(near(area(&made(&limited)[0]), 387.5, 0.0), "{}", area(&made(&limited)[0]));
    // Planned, it sews as a fill, with nothing wrong said of it.
    let outcome = planned(vec![automatic(straight)]);
    assert!(!needle_points(&outcome).is_empty());
    assert!(messages(&outcome).iter().all(|m| m.starts_with("info SC-I0901")), "{:?}", messages(&outcome));
}

#[test]
fn req_auto_004_an_element_that_needs_more_work_than_the_budget_is_left_out() {
    // A wiggly line 10 mm wide needs more work than a budget of 2,000 units allows to find its area.
    let wiggle: Vec<(f64, f64)> = (0..200).map(|i| (f64::from(i), if i % 2 == 0 { 0.0 } else { 3.0 })).collect();
    let wide = automatic(drawn("wide", &wiggle, 10.0));
    let thin = automatic(drawn("thin", &[(0.0, 30.0), (20.0, 30.0)], 0.5));
    let design = stitchcraft_engine::design::Design::new(vec![wide, thin], DesignSettings::default()).unwrap();
    let outcome = stitchcraft_engine::plan(&design, stitchcraft_plan::profiles::REFERENCE, &Budget { max_work: 2_000, ..Budget::DEFAULT });
    let said = messages(&outcome);
    let spent = |d: &stitchcraft_core::Diagnostic| d.code == Code::BudgetExhausted && d.element.as_ref().map(|e| e.as_str()) == Some("wide");
    assert!(outcome.diagnostics.iter().any(spent), "{said:?}");
    assert!(outcome.plan.is_some_and(|plan| plan.elements.iter().all(|e| e.as_str() == "thin")), "{said:?}");
}

#[test]
fn req_auto_001_only_automatic_elements_are_digitized() {
    // The same wide line, not automatic: a running stitch, as without the switch.
    let line = drawn("line", &[(0.0, 0.0), (20.0, 0.0)], 3.0);
    let outcome = planned(vec![line.clone()]);
    assert!(messages(&outcome).is_empty(), "{:?}", messages(&outcome));
    let run = needle_points(&outcome);
    assert!(run.iter().all(|q| q.y() == 0.0), "{run:?}");
    assert_ne!(needle_points(&planned(vec![automatic(line)])), run);
}

#[test]
fn diag_sc_i0901_counts_the_shapes_digitized_by_their_stitches() {
    let area = automatic(fill("area", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]], &RED, &[]));
    let thin = automatic(drawn("thin", &[(0.0, 20.0), (20.0, 20.0)], 0.5));
    let column = automatic(drawn("column", &[(0.0, 30.0), (20.0, 30.0)], 3.0));
    let wide = automatic(drawn("wide", &[(0.0, 45.0), (20.0, 45.0)], 10.0));
    let set = drawn("set", &[(0.0, 60.0), (20.0, 60.0)], 3.0);
    let said = messages(&planned(vec![area, thin, column.clone(), wide, set]));
    assert_eq!(said, ["info SC-I0901: 4 shapes are digitized automatically: 1 sews as a running stitch, 1 as a satin column and 2 as tatami fills."]);
    assert_eq!(messages(&planned(vec![column])), ["info SC-I0901: 1 shape is digitized automatically: it sews as a satin column."]);
    let two = (0..2).map(|i| automatic(drawn(&format!("c{i}"), &[(0.0, f64::from(i) * 10.0), (20.0, f64::from(i) * 10.0)], 3.0))).collect();
    assert_eq!(messages(&planned(two)), ["info SC-I0901: 2 shapes are digitized automatically: both sew as satin columns."]);
    assert_eq!(Code::AutoDigitized.id(), "SC-I0901");
}
