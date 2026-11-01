//! Automatic shapes beside each other (`REQ-AUTO-007` to `REQ-AUTO-009`): an area reaches 0.4 mm under the
//! shapes sewn after it that it touches, and is cut away under the areas sewn after it that cover it; a speck
//! merges into the area it shares the longest edge with; and the picture's background is left out.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::normalize::region::{self, Region};
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, messages, planned, polylines, rings, widened};

/// An automatic rectangle `id` from `(x0, y0)` to `(x1, y1)`.
fn rect(id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Element {
    automatic(fill(id, &[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]], &RED, &[]))
}

/// An automatic rectangle `id` from `(x0, y0)` to `(x1, y1)` with a rectangular hole from `(a0, b0)` to
/// `(a1, b1)`, drawn the other way round, so that the nonzero rule leaves it empty.
fn holed(id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64), (a0, b0): (f64, f64), (a1, b1): (f64, f64)) -> Element {
    automatic(fill(id, &[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)], &[(a0, b0), (a0, b1), (a1, b1), (a1, b0)]], &BLUE, &[]))
}

/// A line along `points` with no settings, `width` millimetres wide.
fn line(id: &str, points: &[(f64, f64)], width: f64) -> Element {
    automatic(widened(along(id, polylines(&[points]), &RED, &[]), width))
}

/// The elements `elements` are sewn as, in a design with the default settings, and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

/// The element named `id` among `elements`.
fn named<'e>(elements: &'e [Element], id: &str) -> &'e Element {
    elements.iter().find(|element| element.id.as_str() == id).unwrap_or_else(|| panic!("no `{id}` among {:?}", ids(elements)))
}

/// The ids of `elements`, in order.
fn ids(elements: &[Element]) -> Vec<&str> {
    elements.iter().map(|element| element.id.as_str()).collect()
}

/// The area the fill `element` sews.
fn region_of(element: &Element) -> Region {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    region_of(element).area()
}

/// The box around the area the fill `element` sews: least x and y, then greatest.
fn bounds(element: &Element) -> [f64; 4] {
    let region = region_of(element);
    let points = || region.parts.iter().flat_map(|part| part.outline.iter());
    let least = |of: fn(&stitchcraft_core::Point) -> f64| points().map(of).fold(f64::INFINITY, f64::min);
    let most = |of: fn(&stitchcraft_core::Point) -> f64| points().map(of).fold(f64::NEG_INFINITY, f64::max);
    [least(|p| p.x()), least(|p| p.y()), most(|p| p.x()), most(|p| p.y())]
}

/// What is said with `code`, as a user reads it.
fn said(diagnostics: &[Diagnostic], code: Code) -> Vec<String> {
    diagnostics.iter().filter(|d| d.code == code).map(ToString::to_string).collect()
}

/// Whether `got` is within `slack` of `want`.
fn near(got: f64, want: f64, slack: f64) -> bool {
    (got - want).abs() <= slack
}

/// Twice the shapes' tolerance, how far apart 2 shapes may be and still touch: an area that reaches under
/// a shape may reach that much past the shape's edge where the edge ends, give or take rounding.
const TOUCHING: f64 = stitchcraft_engine::auto::TOUCHING + 1e-9;

/// How much more than the strip along an edge 2 areas share an area may gain reaching under the other:
/// past each end of the edge, a strip [`TOUCHING`] wide, as long as the overlap and [`TOUCHING`] together.
const PAST_ENDS: f64 = 2.0 * (0.4 + TOUCHING) * TOUCHING;

#[test]
fn req_auto_007_an_area_reaches_0_4_mm_under_the_area_sewn_after_it_that_it_touches() {
    let (out, diagnostics) =
        sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0)), rect("right", (10.0, 0.0), (20.0, 10.0)), rect("apart", (21.0, 0.0), (31.0, 10.0))]);
    assert_eq!(ids(&out), ["left", "right", "apart"]);
    // `left` grows 0.4 mm into `right` along the edge they share, and nowhere else.
    let left = named(&out, "left");
    let [x0, y0, x1, y1] = bounds(left);
    assert!(near(x1, 10.4, 1e-6) && near(x0, 0.0, 1e-9), "{:?}", bounds(left));
    assert!(near(y0, 0.0, TOUCHING) && near(y1, 10.0, TOUCHING), "{:?}", bounds(left));
    assert!(near(area(left), 104.0, PAST_ENDS), "{}", area(left));
    // `right` touches no shape sewn after it, 1 mm from `apart`, and the last is sewn last: both as drawn.
    for (id, element) in [("right", rect("right", (10.0, 0.0), (20.0, 10.0))), ("apart", rect("apart", (21.0, 0.0), (31.0, 10.0)))] {
        assert_eq!(named(&out, id).shape, element.shape, "{id}");
    }
    assert_eq!(said(&diagnostics, Code::AutoUnder), ["info SC-I0904: 1 area reaches 0.4 mm under the shapes sewn after it that it touches."]);
}

#[test]
fn req_auto_007_areas_reach_under_the_shape_sewn_after_them_both() {
    // 2 squares, one on the other, and a bar beside both, sewn after them: the lower square reaches under the
    // upper one and the bar, and the upper square under the bar. Where the squares meet beside the bar, each
    // reaches round its corner under the bar, a quarter of a circle as wide as the overlap.
    let (out, diagnostics) =
        sewn(vec![rect("low", (0.0, 0.0), (10.0, 10.0)), rect("high", (0.0, 10.0), (10.0, 20.0)), rect("bar", (10.0, 0.0), (20.0, 20.0))]);
    let corner = std::f64::consts::PI * 0.4 * 0.4 / 4.0;
    assert!(near(area(named(&out, "low")), 108.0 + corner, 2.0 * PAST_ENDS), "{}", area(named(&out, "low")));
    assert!(near(area(named(&out, "high")), 104.0 + corner, PAST_ENDS), "{}", area(named(&out, "high")));
    let [x0, y0, x1, y1] = bounds(named(&out, "low"));
    assert!(near(x0, 0.0, TOUCHING) && near(y0, 0.0, TOUCHING) && near(x1, 10.4, 1e-6) && near(y1, 10.4, 1e-6), "{:?}", bounds(named(&out, "low")));
    assert_eq!(said(&diagnostics, Code::AutoUnder), ["info SC-I0904: 2 areas reach 0.4 mm under the shapes sewn after them that they touch."]);
}

#[test]
fn req_auto_007_an_area_reaches_under_no_shape_that_comes_near_without_touching() {
    // 0.2 mm apart, the squares don't touch: the band round the first that lies under the second would be a
    // part of its own, and is left out.
    let (out, diagnostics) = sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0)), rect("right", (10.2, 0.0), (20.0, 10.0))]);
    assert_eq!(named(&out, "left").shape, rect("left", (0.0, 0.0), (10.0, 10.0)).shape);
    assert!(said(&diagnostics, Code::AutoUnder).is_empty());
    // 0.05 mm apart, within twice the shapes' tolerance, they touch, and the gap closes.
    let (out, _) = sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0)), rect("right", (10.05, 0.0), (20.0, 10.0))]);
    let left = named(&out, "left");
    assert_eq!(region_of(left).parts.len(), 1);
    assert!(near(bounds(left)[2], 10.4, 1e-6), "{:?}", bounds(left));
}

#[test]
fn req_auto_007_an_area_reaches_under_a_shape_only_where_they_touch() {
    // An L sewn after the square touches its top edge from x = 8 to 10, then runs down its right side 0.2 mm
    // from it, too far to touch.
    let ell = automatic(fill("ell", &[&[(8.0, 10.0), (10.2, 10.0), (10.2, 0.0), (20.0, 0.0), (20.0, 12.0), (8.0, 12.0)]], &BLUE, &[]));
    let (out, _) = sewn(vec![rect("square", (0.0, 0.0), (10.0, 10.0)), ell]);
    // The square reaches 0.4 mm under the L along the 2 mm they share and round the corner there, and no
    // further down its right side than the overlap from that corner.
    let square = named(&out, "square");
    assert!(near(area(square), 100.0 + 2.0 * 0.4, 0.4), "{}", area(square));
    let region = region_of(square);
    let beside =
        region.parts.iter().flat_map(|part| part.outline.iter()).filter(|p| p.x() > 10.0 + 1e-9).map(|p| p.y()).fold(f64::INFINITY, f64::min);
    assert!(beside > 10.0 - 0.4 - TOUCHING, "{beside}");
}

#[test]
fn req_auto_007_an_area_covered_by_an_area_sewn_after_it_is_cut_away_0_4_mm_inside_its_edge() {
    // A shape away from the others, so that the lowest is no background (REQ-AUTO-009).
    let far = || rect("far", (40.0, 0.0), (45.0, 5.0));
    let over = || rect("over", (10.0, 10.0), (20.0, 20.0));
    // Traced with its colours stacked, the lower area runs on under the upper one.
    let (stacked, stacked_said) = sewn(vec![rect("under", (0.0, 0.0), (30.0, 30.0)), over(), far()]);
    // Traced with its colours abutting, the lower area has a hole where the upper one is, 10 mm from its
    // edges, wider than a satin column: so it has no narrow parts (REQ-AUTO-019).
    let (abutting, abutting_said) = sewn(vec![holed("under", (0.0, 0.0), (30.0, 30.0), (10.0, 10.0), (20.0, 20.0)), over(), far()]);
    // Either way, the lower area is sewn up to 0.4 mm inside the upper one's edge: 900 mm² less a 9.2 mm
    // square.
    for out in [&stacked, &abutting] {
        assert!(near(area(named(out, "under")), 900.0 - 9.2 * 9.2, 0.05), "{}", area(named(out, "under")));
        assert_eq!(named(out, "over").shape, over().shape);
    }
    assert_eq!(said(&stacked_said, Code::AutoUnder), ["info SC-I0904: 1 area is cut away under the areas sewn after it, 0.4 mm inside their edges."]);
    assert_eq!(said(&abutting_said, Code::AutoUnder), ["info SC-I0904: 1 area reaches 0.4 mm under the shapes sewn after it that it touches."]);
    // So the stacked trace sews the stitches of the abutting one.
    let stitches = |elements: Vec<Element>| planned(elements).plan.unwrap().stitches().count();
    let stacked_stitches = stitches(vec![rect("under", (0.0, 0.0), (30.0, 30.0)), over(), far()]);
    let abutting_stitches = stitches(vec![holed("under", (0.0, 0.0), (30.0, 30.0), (10.0, 10.0), (20.0, 20.0)), over(), far()]);
    let apart = stacked_stitches.abs_diff(abutting_stitches) as f64 / abutting_stitches as f64;
    assert!(apart < 0.02, "stacked {stacked_stitches}, abutting {abutting_stitches}");
}

#[test]
fn req_auto_007_an_area_covered_whole_by_an_area_sewn_after_it_sews_nothing() {
    // It stops the machine after it, which it can't where it sews nothing: the stop is left out, as after
    // any element that sews nothing, and of the shapes digitized the 2 that sew are counted, `far` narrow
    // enough for a satin column.
    let hidden = automatic(fill("hidden", &[&[(5.0, 5.0), (8.0, 5.0), (8.0, 8.0), (5.0, 8.0)]], &RED, &[("stop_after", "true")]));
    let (out, diagnostics) = sewn(vec![hidden, rect("cover", (0.0, 0.0), (20.0, 20.0)), rect("far", (30.0, 0.0), (35.0, 5.0))]);
    assert_eq!(ids(&out), ["cover", "far"]);
    let hidden = diagnostics.iter().find(|d| d.code == Code::AutoUnder).unwrap();
    assert_eq!(hidden.to_string(), "info SC-I0904: This area is covered by the areas sewn after it, so it sews nothing.");
    assert_eq!(hidden.element.as_ref().map(|id| id.as_str()), Some("hidden"));
    let stop = diagnostics.iter().find(|d| d.code == Code::TrimOrStopLeftOut).unwrap();
    assert_eq!(stop.to_string(), "warning SC-W0505: This element sews no stitch, so the stop after it is left out.");
    assert_eq!(stop.element.as_ref().map(|id| id.as_str()), Some("hidden"));
    assert_eq!(
        said(&diagnostics, Code::AutoDigitized),
        ["info SC-I0901: 2 shapes are digitized automatically: 1 sews as a satin column and 1 as a tatami fill."]
    );
}

#[test]
fn req_auto_007_an_area_covered_whole_sews_nothing_however_near_the_edge_of_what_covers_it() {
    // 60 mm² under the cover's left edge: cut 0.4 mm inside that edge, it would leave a sliver 0.3 mm wide
    // that the cover sews over. It is 7.5 mm wide, wider than a satin column, which a part as narrow as that
    // sews as (REQ-AUTO-016).
    let (out, diagnostics) =
        sewn(vec![rect("hidden", (0.1, 2.0), (7.6, 10.0)), rect("cover", (0.0, 0.0), (20.0, 20.0)), rect("far", (30.0, 0.0), (35.0, 5.0))]);
    assert_eq!(ids(&out), ["cover", "far"]);
    assert_eq!(said(&diagnostics, Code::AutoUnder), ["info SC-I0904: This area is covered by the areas sewn after it, so it sews nothing."]);
    // A part of an area that lies under the cover whole is left out, and the rest is cut away as usual.
    let (out, diagnostics) = sewn(vec![
        automatic(fill(
            "parts",
            &[&[(0.1, 2.0), (7.6, 2.0), (7.6, 10.0), (0.1, 10.0)], &[(-10.0, 0.0), (-1.0, 0.0), (-1.0, 10.0), (-10.0, 10.0)]],
            &RED,
            &[],
        )),
        rect("cover", (0.0, 0.0), (20.0, 20.0)),
        rect("far", (30.0, 0.0), (35.0, 5.0)),
    ]);
    assert!(near(area(named(&out, "parts")), 90.0, 1e-9), "{}", area(named(&out, "parts")));
    // The band round the part left out, under the cover's edge, touches nothing left of the area: it
    // reaches under nothing.
    assert_eq!(said(&diagnostics, Code::AutoUnder), ["info SC-I0904: 1 area is cut away under the areas sewn after it, 0.4 mm inside their edges."]);
}

#[test]
fn req_auto_007_an_area_that_bounds_nothing_is_left_to_its_fill() {
    // Drawn along a line, a fill bounds no area: it is sewn as drawn, and its fill says that it sews nothing.
    let flat = || automatic(fill("flat", &[&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)]], &RED, &[]));
    let square = || rect("square", (0.0, 5.0), (10.0, 15.0));
    let (out, diagnostics) = sewn(vec![flat(), square()]);
    assert_eq!(ids(&out), ["flat", "square"]);
    assert!(said(&diagnostics, Code::AutoUnder).is_empty(), "{:?}", said(&diagnostics, Code::AutoUnder));
    let said = messages(&planned(vec![flat(), square()]));
    assert!(said.iter().any(|m| m == "warning SC-W0303: This fill bounds no area under its fill rule, so it sews nothing."), "{said:?}");
}

#[test]
fn req_auto_007_a_column_or_a_line_sewn_after_an_area_goes_over_it() {
    // A satin column 3 mm wide across the square, and a running line 0.5 mm wide across it too.
    let column = line("column", &[(-5.0, 5.0), (15.0, 5.0)], 3.0);
    let running = line("running", &[(-5.0, 2.0), (15.0, 2.0)], 0.5);
    let (out, _) = sewn(vec![rect("square", (0.0, 0.0), (10.0, 10.0)), column, running]);
    // The square is sewn whole, and reaches 0.4 mm under the column where the column crosses its edges, on
    // both sides: 2 strips 0.4 mm by 3 mm. A running line is too narrow to reach under.
    let square = named(&out, "square");
    assert!(near(area(square), 100.0 + 2.0 * 0.4 * 3.0, 4.0 * 0.4 * TOUCHING), "{}", area(square));
    let [x0, y0, x1, y1] = bounds(square);
    assert!(near(x0, -0.4, 1e-6) && near(x1, 10.4, 1e-6) && near(y0, 0.0, 1e-9) && near(y1, 10.0, 1e-9), "{:?}", bounds(square));
}

#[test]
fn req_auto_007_an_area_reaches_under_a_satin_column_whichever_way_its_rails_are_drawn() {
    // A satin column with settings along the square's top edge, 3 mm wide, its second rail drawn the way
    // the first is and drawn back: it is sewn alike, its rails turned to run the same way.
    for second in [[(0.0, 13.0), (10.0, 13.0)], [(10.0, 13.0), (0.0, 13.0)]] {
        let column = along("column", polylines(&[&[(0.0, 10.0), (10.0, 10.0)], &second]), &BLUE, &[("satin_column", "true")]);
        let (out, _) = sewn(vec![rect("square", (0.0, 0.0), (10.0, 10.0)), column]);
        let square = named(&out, "square");
        assert!(near(area(square), 104.0, PAST_ENDS), "{second:?}: {}", area(square));
    }
}

#[test]
fn req_auto_007_shapes_with_settings_are_sewn_as_drawn() {
    let set = |element: Element| Element { automatic: false, ..element };
    // A shape with settings doesn't reach under the shapes after it...
    let (out, _) = sewn(vec![set(rect("left", (0.0, 0.0), (10.0, 10.0))), rect("right", (10.0, 0.0), (20.0, 10.0))]);
    assert_eq!(area(named(&out, "left")), 100.0);
    // ...but an automatic area reaches under one sewn after it.
    let (out, _) = sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0)), set(rect("right", (10.0, 0.0), (20.0, 10.0)))]);
    assert!(near(area(named(&out, "left")), 104.0, PAST_ENDS), "{}", area(named(&out, "left")));
    assert_eq!(area(named(&out, "right")), 100.0);
}

#[test]
fn req_auto_008_a_speck_in_a_hole_merges_into_the_area_around_it() {
    let (out, diagnostics) = sewn(vec![
        holed("big", (0.0, 0.0), (20.0, 20.0), (9.55, 9.55), (10.45, 10.45)),
        rect("speck", (9.55, 9.55), (10.45, 10.45)),
        rect("far", (30.0, 0.0), (35.0, 5.0)),
    ]);
    assert_eq!(ids(&out), ["big", "far"]);
    assert!(near(area(named(&out, "big")), 400.0, 1e-9), "no hole shows where the speck was: {}", area(named(&out, "big")));
    let speck = diagnostics.iter().find(|d| d.code == Code::AutoSpeck).unwrap();
    assert_eq!(
        (speck.to_string(), speck.element.as_ref().map(|id| id.as_str().to_string())),
        (
            "info SC-I0905: This area of 0.81 mm² is too small to sew, so it is merged into `big`, which it shares the longest edge with."
                .to_string(),
            Some("speck".to_string())
        )
    );
}

#[test]
fn req_auto_008_a_speck_merges_into_the_area_it_shares_the_longest_edge_with() {
    // The speck shares 0.8 mm of edge with `left` and 1 mm with `right`.
    let (out, diagnostics) =
        sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0)), rect("right", (10.0, 1.0), (20.0, 10.0)), rect("speck", (10.0, 0.2), (11.0, 1.0))]);
    assert_eq!(ids(&out), ["left", "right"]);
    assert!(near(area(named(&out, "right")), 90.0 + 0.8, 1e-9), "{}", area(named(&out, "right")));
    assert_eq!(
        said(&diagnostics, Code::AutoSpeck),
        ["info SC-I0905: This area of 0.8 mm² is too small to sew, so it is merged into `right`, which it shares the longest edge with."]
    );
}

#[test]
fn req_auto_008_a_part_too_small_to_sew_rejoins_the_rest_of_its_area_when_it_shares_the_longest_edge_with_it() {
    // A part 0.02 mm from the square it was traced with, nearer than twice the shapes' tolerance: the gap
    // closes.
    let (out, diagnostics) = sewn(vec![
        automatic(fill(
            "parts",
            &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[(10.02, 0.0), (10.52, 0.0), (10.52, 1.0), (10.02, 1.0)]],
            &RED,
            &[],
        )),
        rect("far", (30.0, 0.0), (35.0, 5.0)),
    ]);
    assert_eq!(ids(&out), ["parts", "far"]);
    let parts = named(&out, "parts");
    assert_eq!(region_of(parts).parts.len(), 1);
    assert!(area(parts) > 100.5 && near(bounds(parts)[2], 10.52, TOUCHING), "{} {:?}", area(parts), bounds(parts));
    assert_eq!(
        said(&diagnostics, Code::AutoSpeck),
        [
            "info SC-I0905: A part of this area, of 0.5 mm², is too small to sew, so it is joined to the rest of this area, which it shares the longest edge with."
        ]
    );
}

#[test]
fn req_auto_008_a_speck_that_shares_no_edge_is_left_out() {
    let (out, diagnostics) = sewn(vec![
        rect("square", (0.0, 0.0), (10.0, 10.0)),
        rect("lone", (30.0, 30.0), (30.5, 30.5)),
        // A part of an area that small, away from every other area.
        automatic(fill(
            "parts",
            &[&[(0.0, 20.0), (10.0, 20.0), (10.0, 30.0), (0.0, 30.0)], &[(12.0, 20.0), (12.8, 20.0), (12.8, 20.8), (12.0, 20.8)]],
            &RED,
            &[],
        )),
        // An area of exactly 1 mm² is no speck, and one a little smaller is named under 1 mm².
        rect("square-mm", (40.0, 0.0), (41.0, 1.0)),
        rect("nearly", (40.0, 5.0), (40.996, 6.0)),
        // A speck in a hole that touches none of its edges.
        holed("ring", (50.0, 0.0), (70.0, 20.0), (58.0, 8.0), (62.0, 12.0)),
        rect("inside", (59.75, 9.75), (60.25, 10.25)),
    ]);
    assert_eq!(ids(&out), ["square", "parts", "square-mm", "ring"]);
    assert!(near(area(named(&out, "parts")), 100.0, 1e-9), "{}", area(named(&out, "parts")));
    assert_eq!(
        said(&diagnostics, Code::AutoSpeck),
        [
            "info SC-I0905: This area of 0.25 mm² is too small to sew and shares no edge with an automatic area, so it is left out.",
            "info SC-I0905: A part of this area, of 0.64 mm², is too small to sew and shares no edge with an automatic area, so it is left out.",
            "info SC-I0905: This area of 0.99 mm² is too small to sew and shares no edge with an automatic area, so it is left out.",
            "info SC-I0905: This area of 0.25 mm² is too small to sew and shares no edge with an automatic area, so it is left out.",
        ]
    );
    // A speck with settings is sewn as they say.
    let (out, diagnostics) =
        sewn(vec![rect("square", (0.0, 0.0), (10.0, 10.0)), Element { automatic: false, ..rect("dot", (30.0, 30.0), (30.5, 30.5)) }]);
    assert_eq!(ids(&out), ["square", "dot"]);
    assert!(said(&diagnostics, Code::AutoSpeck).is_empty());
}

#[test]
fn req_auto_009_the_lowest_area_that_covers_the_design_is_the_background_and_is_left_out() {
    let logo = || rect("logo", (5.0, 5.0), (15.0, 15.0));
    let message = "warning SC-W0906: This area covers the whole design under the other shapes, so it is left out as the picture's background.";
    // Stacked, the background is a whole rectangle; abutting, it has a hole where the logo is.
    for background in [rect("background", (0.0, 0.0), (30.0, 20.0)), holed("background", (0.0, 0.0), (30.0, 20.0), (5.0, 5.0), (15.0, 15.0))] {
        let (out, diagnostics) = sewn(vec![background, logo()]);
        assert_eq!(ids(&out), ["logo"]);
        assert_eq!(named(&out, "logo").shape, logo().shape);
        assert_eq!(said(&diagnostics, Code::AutoBackground), [message]);
    }
    // It covers the box around the design's paths to within 0.5 mm of each side, here a line's.
    let corners = || line("corners", &[(0.0, 0.0), (30.0, 20.0)], 0.5);
    let (out, _) = sewn(vec![rect("background", (0.4, 0.4), (29.6, 19.6)), corners()]);
    assert_eq!(ids(&out), ["corners"]);
    let (out, diagnostics) = sewn(vec![rect("background", (0.6, 0.4), (29.6, 19.6)), corners()]);
    assert_eq!(ids(&out), ["background", "corners"], "0.6 mm from the left side");
    assert!(said(&diagnostics, Code::AutoBackground).is_empty());
}

#[test]
fn req_auto_009_a_background_with_settings_or_alone_or_not_lowest_is_sewn() {
    let background = || rect("background", (0.0, 0.0), (30.0, 20.0));
    let logo = || rect("logo", (5.0, 5.0), (15.0, 15.0));
    for (case, elements) in [
        ("with settings", vec![Element { automatic: false, ..background() }, logo()]),
        ("alone", vec![background()]),
        ("not the lowest", vec![logo(), background()]),
    ] {
        let (out, diagnostics) = sewn(elements);
        assert!(ids(&out).contains(&"background"), "{case}");
        assert!(said(&diagnostics, Code::AutoBackground).is_empty(), "{case}");
    }
}

#[test]
fn req_auto_009_a_background_notched_where_a_shape_touches_the_pictures_edge_is_left_out() {
    let message = "warning SC-W0906: This area covers the whole design under the other shapes, so it is left out as the picture's background.";
    // A logo touches the left side, and an abutting trace cuts the background there. Inside the margin, the
    // notch runs 10 mm along the side and 29 mm along the background.
    let notched = || {
        let outline = [(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0), (0.0, 15.0), (10.0, 15.0), (10.0, 5.0), (0.0, 5.0)];
        automatic(fill("background", &[&outline], &BLUE, &[]))
    };
    let (out, diagnostics) = sewn(vec![notched(), rect("logo", (0.0, 5.0), (10.0, 15.0))]);
    assert_eq!(ids(&out), ["logo"]);
    assert_eq!(said(&diagnostics, Code::AutoBackground), [message]);
    // A logo that fills it but for a gap narrower than shapes still meet across, as rounding leaves between
    // edges drawn as one, fills it; one with a wider gap doesn't.
    for (gap, background) in [(0.5 * TOUCHING, false), (2.0 * TOUCHING, true)] {
        let (out, _) = sewn(vec![notched(), rect("logo", (0.0, 5.0 + gap), (10.0 - gap, 15.0 - gap))]);
        assert_eq!(ids(&out).contains(&"background"), background, "a gap of {gap} mm: {:?}", ids(&out));
    }
    // A logo in a corner runs as far along the box's sides as along the background: 17 mm each.
    let cornered = automatic(fill("background", &[&[(10.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0), (0.0, 8.0), (10.0, 8.0)]], &BLUE, &[]));
    let (out, _) = sewn(vec![cornered, rect("logo", (0.0, 0.0), (10.0, 8.0))]);
    assert_eq!(ids(&out), ["logo"]);
    // A notch that no area drawn on the shape fills is part of the shape, which sews.
    let (out, diagnostics) = sewn(vec![notched(), rect("logo", (15.0, 5.0), (25.0, 15.0))]);
    assert!(ids(&out).contains(&"background"), "{:?}", ids(&out));
    assert!(said(&diagnostics, Code::AutoBackground).is_empty());
}

#[test]
fn req_auto_009_a_background_whose_side_leans_across_the_margins_edge_is_left_out() {
    // A border line half a millimetre past the background's left side sets the box, so that side lies on
    // the margin's edge, and leans 40 nm over 20 mm across it. The sliver it leaves inside the margin runs
    // along the box's side a little further than along the background, and has no area: it is no corner of
    // the picture. A logo fills the notch in the right side.
    let lean = 4e-8;
    let background = [(0.5, 0.0), (0.5 + lean, 20.0), (30.0, 20.0), (30.0, 15.0), (20.0, 15.0), (20.0, 5.0), (30.0, 5.0), (30.0, 0.0)];
    let elements = vec![
        automatic(fill("background", &[&background], &BLUE, &[])),
        rect("logo", (20.0, 5.0), (30.0, 15.0)),
        line("border", &[(0.0, 0.0), (0.0, 20.0)], 0.35),
    ];
    let (out, diagnostics) = sewn(elements);
    assert_eq!(ids(&out), ["logo", "border"]);
    assert_eq!(said(&diagnostics, Code::AutoBackground).len(), 1);
}

#[test]
fn req_auto_009_a_lowest_shape_that_leaves_the_corners_to_the_shapes_in_them_is_sewn() {
    // An octagon, a rectangle with its corners cut 4 mm along each side, reaches the box's sides, and an
    // abutting trace makes each corner a shape of its own. Inside the margin, a corner runs 6 mm along the
    // sides and 4.2 mm along the octagon: the octagon is part of the picture, as a disc or a rounded square
    // is.
    let octagon = [(4.0, 0.0), (26.0, 0.0), (30.0, 4.0), (30.0, 16.0), (26.0, 20.0), (4.0, 20.0), (0.0, 16.0), (0.0, 4.0)];
    let corner = |id: &str, corner: [(f64, f64); 3]| automatic(fill(id, &[&corner], &RED, &[]));
    let elements = vec![
        automatic(fill("badge", &[&octagon], &BLUE, &[])),
        corner("top left", [(0.0, 0.0), (4.0, 0.0), (0.0, 4.0)]),
        corner("top right", [(30.0, 0.0), (30.0, 4.0), (26.0, 0.0)]),
        corner("bottom right", [(30.0, 20.0), (26.0, 20.0), (30.0, 16.0)]),
        corner("bottom left", [(0.0, 20.0), (0.0, 16.0), (4.0, 20.0)]),
    ];
    let (out, diagnostics) = sewn(elements);
    assert!(ids(&out).contains(&"badge"), "{:?}", ids(&out));
    assert!(said(&diagnostics, Code::AutoBackground).is_empty());
}

#[test]
fn req_auto_009_a_design_of_one_area_with_its_own_stroke_sews_it() {
    // A shape with a fill and a stroke is 2 elements on one path, as the readers make it: its stroke is no
    // shape drawn on its fill.
    let outline: &[(f64, f64)] = &[(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0)];
    let patch = || automatic(fill("patch:fill", &[outline], &BLUE, &[]));
    let stroke = || automatic(widened(along("patch:stroke", rings(&[outline]), &RED, &[]), 0.35));
    let (out, diagnostics) = sewn(vec![patch(), stroke()]);
    assert!(ids(&out).contains(&"patch:fill"), "{:?}", ids(&out));
    assert!(said(&diagnostics, Code::AutoBackground).is_empty());
    // A shape on a path of its own drawn on it makes it the background, and its stroke sews.
    let (out, diagnostics) = sewn(vec![patch(), stroke(), rect("logo", (5.0, 5.0), (15.0, 15.0))]);
    assert!(!ids(&out).contains(&"patch:fill") && ids(&out).contains(&"patch:stroke"), "{:?}", ids(&out));
    assert_eq!(said(&diagnostics, Code::AutoBackground).len(), 1);
}

#[test]
fn req_auto_009_a_background_that_needs_more_work_to_find_than_the_budget_allows_is_sewn_and_said_to_be() {
    let design =
        Design::new(vec![rect("background", (0.0, 0.0), (30.0, 20.0)), rect("logo", (5.0, 5.0), (15.0, 15.0))], DesignSettings::default()).unwrap();
    let with = |work: u64| {
        let (out, diagnostics) = digitized(&design, &Budget { max_work: work, ..Budget::DEFAULT });
        let digitized = ids(&out).iter().map(ToString::to_string).collect::<Vec<_>>();
        let exhausted: Vec<String> = diagnostics
            .iter()
            .filter(|d| d.code == Code::BudgetExhausted)
            .filter_map(|d| d.element.as_ref().map(|id| id.as_str().to_string()))
            .collect();
        (digitized, exhausted, said(&diagnostics, Code::AutoBackground))
    };
    let found = |work: u64| with(work).2.iter().any(|m| m.contains("left out as the picture's background"));
    // The least work that finds the background.
    let (mut low, mut high) = (0, 1 << 24);
    assert!(found(high));
    while high - low > 1 {
        let middle = (low + high) / 2;
        if found(middle) { high = middle } else { low = middle }
    }
    // With a unit less, it is sewn, and said to be. So little work is too little to digitize it as well,
    // its opening included (REQ-AUTO-019), which is said of it too: the element isn't dropped as a background.
    let (ids, exhausted, said) = with(low);
    let named = |list: &[String]| list.iter().any(|id| id == "background");
    assert!(named(&ids) || named(&exhausted), "{ids:?} {exhausted:?}");
    assert_eq!(
        said,
        [format!(
            "warning SC-W0906: Finding whether this area is the picture's background needed more than the work budget of {low} units, so it is sewn."
        )]
    );
}

#[test]
fn diag_sc_i0904_areas_reach_under_the_shapes_sewn_after_them_or_are_cut_away() {
    // 2 areas reach under the next, and the lowest of 3 stacked areas is cut away under the 2 above it, each
    // wider than a satin column, which a part as narrow as that sews as (REQ-AUTO-016).
    let (_, diagnostics) = sewn(vec![
        rect("a", (0.0, 0.0), (10.0, 10.0)),
        rect("b", (10.0, 0.0), (20.0, 10.0)),
        rect("c", (20.0, 0.0), (30.0, 10.0)),
        rect("under", (0.0, 20.0), (30.0, 40.0)),
        rect("over1", (4.0, 25.0), (12.0, 35.0)),
        rect("over2", (18.0, 25.0), (26.0, 35.0)),
    ]);
    let [summary] = said(&diagnostics, Code::AutoUnder).try_into().unwrap();
    assert_eq!(
        summary,
        "info SC-I0904: 2 areas reach 0.4 mm under the shapes sewn after them that they touch. 1 area is cut away under the areas sewn after it, \
         0.4 mm inside their edges."
    );
    assert_eq!(Code::AutoUnder.id(), "SC-I0904");
}

#[test]
fn diag_sc_i0904_names_the_areas_in_the_order_they_are_sewn() {
    let (_, diagnostics) = sewn(vec![
        rect("first", (2.0, 2.0), (4.0, 4.0)),
        rect("second", (6.0, 6.0), (8.0, 8.0)),
        rect("cover", (0.0, 0.0), (20.0, 20.0)),
        rect("far", (30.0, 0.0), (35.0, 5.0)),
    ]);
    let named: Vec<&str> =
        diagnostics.iter().filter(|d| d.code == Code::AutoUnder).filter_map(|d| d.element.as_ref().map(|id| id.as_str())).collect();
    assert_eq!(named, ["first", "second"]);
}

#[test]
fn diag_sc_i0905_a_speck_is_merged_or_left_out() {
    // With a shape away from the others, the square is no background (REQ-AUTO-009).
    let (_, diagnostics) =
        sewn(vec![rect("square", (0.0, 0.0), (10.0, 10.0)), rect("speck", (10.0, 0.0), (10.5, 0.5)), rect("far", (20.0, 0.0), (25.0, 5.0))]);
    let speck = diagnostics.iter().find(|d| d.code == Code::AutoSpeck).unwrap();
    assert_eq!(
        (speck.to_string(), speck.fix.as_ref().map(|f| f.describe())),
        (
            "info SC-I0905: This area of 0.25 mm² is too small to sew, so it is merged into `square`, which it shares the longest edge with."
                .to_string(),
            Some("To sew it as it is drawn, give it settings.".to_string())
        )
    );
    assert_eq!(Code::AutoSpeck.id(), "SC-I0905");
}

#[test]
fn diag_sc_w0906_the_background_is_left_out() {
    let (_, diagnostics) = sewn(vec![rect("background", (0.0, 0.0), (30.0, 20.0)), rect("logo", (5.0, 5.0), (15.0, 15.0))]);
    let background = diagnostics.iter().find(|d| d.code == Code::AutoBackground).unwrap();
    assert_eq!(
        (background.to_string(), background.element.as_ref().map(|id| id.as_str().to_string()), background.fix.as_ref().map(|f| f.describe())),
        (
            "warning SC-W0906: This area covers the whole design under the other shapes, so it is left out as the picture's background.".to_string(),
            Some("background".to_string()),
            Some("To sew it, give it settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it.".to_string())
        )
    );
    assert_eq!(Code::AutoBackground.id(), "SC-W0906");
    // The rest of the design still plans.
    let outcome = planned(vec![rect("background", (0.0, 0.0), (30.0, 20.0)), rect("logo", (5.0, 5.0), (15.0, 15.0))]);
    assert!(outcome.plan.is_some(), "{:?}", messages(&outcome));
}
