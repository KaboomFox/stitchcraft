//! Automatic digitizing (`REQ-AUTO-001` to `REQ-AUTO-004`, `REQ-AUTO-006`): a shape without settings sews by
//! its width. An area is a tatami fill; a line no wider than the design's narrowest satin stroke a running
//! stitch, a line up to 7 mm wide a satin column as wide as the line, its caps included, and a wider line the
//! tatami fill of the area it paints. A part a satin column cannot follow is filled, and no shape keeps the
//! design from planning. `SC-I0901` counts the shapes digitized, by their stitches.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Mm, Point, math};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::auto::{SATIN_MAX_WIDTH, Stitches, digitize, digitized};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, FillRule, Join, Path, Segment, Shape, Subpath};
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
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap().elements
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
    // A stroke of no subpath paints nothing, and is left as it is: it sews as it would without the switch.
    let none = widened(along("none", Path::default(), &RED, &[]), 3.0);
    assert_eq!(made(&none), vec![none]);
}

#[test]
fn req_auto_003_a_line_whose_columns_need_more_work_to_name_than_is_left_is_left_out() {
    // Columns `two` and `two#2`, beside elements that have `two#2` and the next 2 names a clash steps aside
    // to: naming the second column costs 3 units of work, on top of the work of digitizing the line.
    let two = automatic(widened(along("two", polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 10.0), (20.0, 10.0)]]), &RED, &[]), 3.0));
    let mut meter = Budget::DEFAULT.meter();
    digitize(&two, &DesignSettings::default(), &mut meter).unwrap();
    let work = Budget::DEFAULT.max_work - meter.work_left();
    let others = ["two#2", "two#2~2", "two#2~3"].map(|id| drawn(id, &[(0.0, 20.0), (20.0, 20.0)], 0.5));
    let design = Design::new([vec![two], others.to_vec()].concat(), DesignSettings::default()).unwrap();
    // What digitizing says. With so little work, the order can't find what the columns cover either, and
    // says so with codes of its own (REQ-AUTO-012).
    let with_work = |extra: u64| {
        let (elements, said) = digitized(&design, &Budget { max_work: work + extra, ..Budget::DEFAULT });
        let digitizing = said.iter().filter(|d| ![Code::AutoUnder, Code::AutoOrder].contains(&d.code)).map(ToString::to_string);
        (elements.iter().map(|e| e.id.to_string()).collect::<Vec<_>>(), digitizing.collect::<Vec<_>>())
    };
    let ids = |ids: &[&str]| ids.iter().map(ToString::to_string).collect::<Vec<_>>();
    assert_eq!(
        with_work(3),
        (
            ids(&["two", "two#2~4", "two#2", "two#2~2", "two#2~3"]),
            ids(&["info SC-I0901: 1 shape is digitized automatically: it sews as a satin column."])
        )
    );
    // A unit less, and the line is left out whole, as a line that needs more work to digitize is.
    assert_eq!(
        with_work(2),
        (
            ids(&["two#2", "two#2~2", "two#2~3"]),
            vec![format!("error SC-E0004: Planning this element needed more than the work budget of {} units, so it was skipped.", work + 2)]
        )
    );
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
    // Where the path starts or ends with a curve, the cap reaches on along the curve's tangent there.
    let curve = Path {
        subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0))], closed: false }],
    };
    let [column] = made(&capped(widened(along("curve", curve, &RED, &[]), 3.0), Cap::Square)).try_into().unwrap();
    let Shape::Stroke { path, .. } = &column.shape else { panic!("{:?}", column.shape) };
    assert_eq!(path.subpaths[0].start, p(-1.5, 0.0));
    assert_eq!(path.subpaths[0].segments.last(), Some(&Segment::Line(p(10.0, 11.5))));
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
    let design = Design::new(vec![wide, thin], DesignSettings::default()).unwrap();
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
    let three = (0..3).map(|i| automatic(drawn(&format!("c{i}"), &[(0.0, f64::from(i) * 10.0), (20.0, f64::from(i) * 10.0)], 3.0))).collect();
    assert_eq!(messages(&planned(three)), ["info SC-I0901: 3 shapes are digitized automatically: all sew as satin columns."]);
    assert_eq!(Code::AutoDigitized.id(), "SC-I0901");
}

/// A circle of radius `r` around the origin, as 4 cubics.
fn circle(r: f64) -> Path {
    let k = 0.552_284_749_830_793_4 * r;
    let segments = vec![
        Segment::Cubic(p(r, k), p(k, r), p(0.0, r)),
        Segment::Cubic(p(-k, r), p(-r, k), p(-r, 0.0)),
        Segment::Cubic(p(-r, -k), p(-k, -r), p(0.0, -r)),
        Segment::Cubic(p(k, -r), p(r, -k), p(r, 0.0)),
    ];
    Path { subpaths: vec![Subpath { start: p(r, 0.0), segments, closed: true }] }
}

/// A line along `points` and a dot at (10, 20), drawn 3 mm wide with `cap`.
fn dotted(points: &[(f64, f64)], cap: Cap) -> Element {
    let mut path = polylines(&[points]);
    path.subpaths.push(Subpath { start: p(10.0, 20.0), segments: vec![Segment::Line(p(10.0, 20.0))], closed: false });
    capped(widened(along("d", path, &RED, &[]), 3.0), cap)
}

/// The error codes `outcome` says.
fn errors(outcome: &PlanOutcome) -> Vec<Code> {
    outcome.diagnostics.iter().filter(|d| d.severity() == stitchcraft_core::Severity::Error).map(|d| d.code).collect()
}

#[test]
fn req_auto_002_widths_are_compared_to_the_micrometre() {
    // 7 mm and 1 mm written in CSS pixels read a rounding error wider: they sew as 7 mm and 1 mm do.
    let (seven, one) = (7.0_f64.next_up(), 1.0_f64.next_up());
    assert_eq!((stitches(seven), stitches(one)), (Stitches::Satin, Stitches::Running));
    // A micrometre more is wider.
    assert_eq!((stitches(7.001), stitches(1.001)), (Stitches::FilledStroke, Stitches::Satin));
}

#[test]
fn req_auto_003_a_trim_or_stop_after_a_line_follows_its_last_part() {
    let two = || polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 10.0), (20.0, 10.0)]]);
    for (key, symbol) in [("trim_after", "T"), ("stop_after", "P")] {
        let line = widened(along("two", two(), &RED, &[(key, "true")]), 3.0);
        let set: Vec<Option<&str>> =
            made(&line).iter().map(|part| part.params.get(key)).collect::<Vec<_>>().into_iter().map(|v| v.map(|_| "true")).collect();
        assert_eq!(set, [None, Some("true")], "{key}");
        let shape = stitchcraft_testkit::designs::shape_of(&planned(vec![automatic(line)]));
        assert_eq!(shape.split(' ').filter(|word| *word == symbol).count(), 1, "{key}: {shape}");
        assert!(shape.ends_with(symbol), "{key}: {shape}");
    }
}

#[test]
fn req_auto_003_a_line_a_satin_column_cannot_follow_is_filled() {
    // A ring 2 mm across drawn 3 mm wide, as a traced dot is; a hairpin 1 mm across; and a line shorter than
    // a CSS pixel, 6 mm wide, so that it paints more than a speck (REQ-AUTO-008). The ring paints the disc
    // 5 mm across.
    let ring = widened(along("ring", circle(1.0), &RED, &[]), 3.0);
    let [filled] = made(&ring).try_into().unwrap();
    let disc = std::f64::consts::PI * 2.5 * 2.5;
    assert!(area(&filled) <= disc && area(&filled) >= disc - SHAPE_TOLERANCE * 5.0 * std::f64::consts::PI, "{}", area(&filled));
    let hairpin = capped(drawn("hairpin", &[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], 3.0), Cap::Round);
    let short = drawn("short", &[(0.0, 0.0), (0.2, 0.0)], 6.0);
    for line in [ring, hairpin, short] {
        let [part] = made(&line).try_into().unwrap();
        assert!(matches!(part.shape, Shape::Fill { rule: FillRule::NonZero, .. }) && part.id == line.id, "{part:?}");
        let outcome = planned(vec![automatic(line.clone())]);
        assert_eq!(errors(&outcome), [], "{}: {:?}", line.id, messages(&outcome));
        let said = messages(&outcome);
        assert_eq!(
            said[..2],
            ["info SC-I0901: 1 shape is digitized automatically: it sews as a tatami fill.".to_string(), filled_message()],
            "{}",
            line.id
        );
        assert!(outcome.diagnostics[1].element.as_ref() == Some(&line.id));
    }
    // The hairpin sews as wide as it is drawn, its round caps included.
    let hairpin = capped(drawn("hairpin", &[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], 3.0), Cap::Round);
    let sewn = needle_points(&planned(vec![automatic(hairpin)]));
    let (xs, ys): (Vec<f64>, Vec<f64>) = sewn.iter().map(|q| (q.x(), q.y())).unzip();
    let extent = |values: &[f64]| values.iter().fold((f64::MAX, f64::MIN), |(lo, hi), &v| (lo.min(v), hi.max(v)));
    let ((left, right), (top, bottom)) = (extent(&xs), extent(&ys));
    assert!(left > -1.6 && right < 11.6 && top > -1.6 && bottom < 2.6 && right - left > 12.0 && bottom - top > 3.5, "{left}…{right}, {top}…{bottom}");
}

/// What `SC-I0902` says of a line a satin column cannot follow.
fn filled_message() -> String {
    "info SC-I0902: A satin column cannot follow this part of the line, which turns more tightly than the line is wide or is shorter than \
     a CSS pixel, so the area it paints is filled."
        .to_string()
}

#[test]
fn req_auto_003_a_dot_sews_as_its_cap_draws_it() {
    let line = [(0.0, 0.0), (20.0, 0.0)];
    for (cap, dot) in [(Cap::Round, std::f64::consts::PI * 1.5 * 1.5), (Cap::Square, 9.0)] {
        let [column, filled] = made(&dotted(&line, cap)).try_into().unwrap();
        assert_eq!((column.id.as_str(), column.params.get("satin_column")), ("d", Some("true")));
        assert_eq!((filled.id.as_str(), filled.params.get("satin_column")), ("d#2", None));
        assert!(area(&filled) <= dot && area(&filled) >= dot - SHAPE_TOLERANCE * 3.0 * std::f64::consts::PI, "{cap:?}: {}", area(&filled));
        let outcome = planned(vec![automatic(dotted(&line, cap))]);
        assert_eq!(errors(&outcome), [], "{cap:?}: {:?}", messages(&outcome));
        let said = &outcome.diagnostics[1];
        assert_eq!(
            (said.to_string(), said.element.as_ref().map(|e| e.as_str())),
            (
                "info SC-I0902: This part of the line is a dot, which its cap draws and a satin column cannot sew, so the dot is filled.".to_string(),
                Some("d#2")
            )
        );
    }
    // A butt cap draws no dot: it is left out, as the same line without the switch leaves it out.
    assert_eq!(made(&dotted(&line, Cap::Butt)).iter().map(|part| part.id.as_str()).collect::<Vec<_>>(), ["d"]);
    let outcome = planned(vec![automatic(dotted(&line, Cap::Butt))]);
    assert_eq!(
        messages(&outcome),
        [
            "info SC-I0901: 1 shape is digitized automatically: it sews as a satin column.",
            "warning SC-W0401: A part of the stroke is a single point, so it is not stitched."
        ]
    );
    // A line of nothing but dots that draw nothing sews as it does without the switch.
    let dots =
        capped(widened(along("dots", Path { subpaths: vec![dotted(&line, Cap::Butt).shape.path().subpaths[1].clone()] }, &RED, &[]), 3.0), Cap::Butt);
    assert_eq!(made(&dots), vec![dots.clone()]);
    assert_eq!(
        messages(&planned(vec![automatic(dots.clone()), drawn("other", &line, 0.5)])),
        messages(&planned(vec![dots, drawn("other", &line, 0.5)]))
    );
}

#[test]
fn req_auto_004_parts_too_small_to_sew_are_named() {
    // 10 mm wide with butt caps: a line 20 mm long, and far from it one 0.01 mm long, which paints 0.1 mm², a
    // speck that shares no edge with another area (REQ-AUTO-008).
    let line = widened(along("w", polylines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 40.0), (0.01, 40.0)]]), &RED, &[]), 10.0);
    assert_eq!(
        messages(&planned(vec![automatic(line)])),
        [
            "info SC-I0901: 1 shape is digitized automatically: it sews as a tatami fill.",
            "info SC-I0905: A part of this area, of 0.1 mm², is too small to sew and shares no edge with an automatic area, so it is left out."
        ]
    );
}

#[test]
fn req_auto_004_a_line_whose_area_reaches_past_10_m_is_left_out() {
    let huge = automatic(drawn("huge", &[(0.0, 0.0), (10.0, 0.0)], 140_000.0));
    let thin = automatic(drawn("thin", &[(0.0, 30.0), (20.0, 30.0)], 0.5));
    // A dot that a line as wide as a satin column draws at the edge, which its round cap paints past it.
    let edge = Path { subpaths: vec![Subpath { start: p(9_999.0, 0.0), segments: vec![Segment::Line(p(9_999.0, 0.0))], closed: false }] };
    let edge = automatic(capped(widened(along("edge", edge, &RED, &[]), 3.0), Cap::Round));
    let outcome = planned(vec![huge, thin, edge]);
    let far = |width: &str| {
        format!(
            "warning SC-W0903: This line is {width} mm wide, and the area it paints would reach more than 10 m from the origin, beyond \
             what machine files can record, so it is left out."
        )
    };
    assert_eq!(
        messages(&outcome),
        ["info SC-I0901: 1 shape is digitized automatically: it sews as a running stitch.".to_string(), far("140000"), far("3")]
    );
    assert!(outcome.plan.is_some_and(|plan| plan.elements.iter().all(|e| e.as_str() == "thin")));
}

#[test]
fn diag_sc_w0903_a_line_too_wide_to_fill_is_left_out() {
    let outcome = planned(vec![automatic(drawn("huge", &[(0.0, 0.0), (10.0, 0.0)], 30_000.0)), drawn("other", &[(0.0, 5.0), (5.0, 5.0)], 0.5)]);
    let said = outcome.diagnostics.iter().find(|d| d.code == Code::AutoAreaTooLarge).unwrap();
    assert_eq!(
        (said.to_string(), said.fix.as_ref().map(|f| f.describe())),
        (
            "warning SC-W0903: This line is 30000 mm wide, and the area it paints would reach more than 10 m from the origin, beyond what \
             machine files can record, so it is left out."
                .to_string(),
            Some("Check the line's stroke width.".to_string())
        )
    );
    assert_eq!((Code::AutoAreaTooLarge.id(), said.element.as_ref().map(|e| e.as_str())), ("SC-W0903", Some("huge")));
}

#[test]
fn diag_sc_i0902_a_line_a_satin_column_cannot_follow_is_filled() {
    let outcome = planned(vec![automatic(widened(along("ring", circle(1.0), &RED, &[]), 3.0))]);
    let said = outcome.diagnostics.iter().find(|d| d.code == Code::AutoSatinFilled).unwrap();
    assert_eq!(
        (said.to_string(), said.fix.as_ref().map(|f| f.describe())),
        (filled_message(), Some("To sew it as a satin column, draw the line with gentler turns or narrower.".to_string()))
    );
    assert_eq!(Code::AutoSatinFilled.id(), "SC-I0902");
}

#[test]
fn req_auto_004_a_long_wide_line_fills_within_the_budget() {
    // A wavy line 10 mm wide running a metre down the page, as cubics: its area's edges all overlap
    // across the page, which is the most work for the fill's area code.
    let mut segments = Vec::new();
    for i in 0..100 {
        let y = f64::from(i) * 10.0;
        segments.push(Segment::Cubic(p(4.0, y + 2.5), p(4.0, y + 2.5), p(0.0, y + 5.0)));
        segments.push(Segment::Cubic(p(-4.0, y + 7.5), p(-4.0, y + 7.5), p(0.0, y + 10.0)));
    }
    let wave = widened(along("wave", Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: false }] }, &RED, &[]), 10.0);
    let [filled] = made(&wave).try_into().unwrap();
    assert!(matches!(filled.shape, Shape::Fill { .. }));
}

proptest::proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(48))]

    #[test]
    fn req_auto_006_digitizing_makes_no_error_of_its_own(
        points in proptest::collection::vec((-8i32..=8, -8i32..=8), 1..6),
        closed in proptest::bool::ANY,
        tenths in 2u32..=120,
        style in 0usize..12,
    ) {
        // A line of up to 5 points on a half-millimetre grid, its first point repeated when it has one: a dot.
        let mut points: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (f64::from(x) / 2.0, f64::from(y) / 2.0)).collect();
        if points.len() == 1 {
            points.push(points[0]);
        }
        let path = Path { subpaths: vec![Subpath { closed, ..polylines(&[&points]).subpaths[0].clone() }] };
        let joins = [Join::Bevel, Join::Round, Join::Miter { limit: 4.0 }, Join::Miter { limit: 10.0 }];
        let caps = [Cap::Butt, Cap::Round, Cap::Square];
        let line = capped(joined(&widened(along("line", path, &RED, &[]), f64::from(tenths) / 10.0), joins[style % 4]), caps[style / 4]);
        let other = drawn("other", &[(0.0, 30.0), (20.0, 30.0)], 0.5);
        let plain = errors(&planned(vec![line.clone(), other.clone()]));
        let auto = planned(vec![automatic(line), other]);
        let new: Vec<Code> = errors(&auto).into_iter().filter(|code| !plain.contains(code) && *code != Code::BudgetExhausted).collect();
        proptest::prop_assert!(new.is_empty(), "{:?}", messages(&auto));
    }
}
