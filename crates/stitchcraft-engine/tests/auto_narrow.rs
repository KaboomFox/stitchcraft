//! Narrow parts of automatic areas (`REQ-AUTO-013` to `REQ-AUTO-015`): a part narrower than the design's
//! narrowest satin stroke everywhere sews as a running stitch along its centre line, tripled from 0.5 mm
//! wide as a stroke is, after the rest of its area, which stays a fill with its specks; and the areas sewn
//! before it reach under the part, as under a satin column.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Mm, Point};
use stitchcraft_engine::auto::{Digitized, Stitches, digitize, digitized};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::generators::passes::BEAN_STITCH_REPEATS;
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, messages, p, planned, polylines, widened};

/// What the automatic `element` sews as, in a design with `settings`.
fn made_with(element: &Element, settings: &DesignSettings) -> Digitized {
    digitize(&automatic(element.clone()), settings, &mut Budget::DEFAULT.meter()).unwrap()
}

/// What the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Digitized {
    made_with(element, &DesignSettings::default())
}

/// A rectangle's corners, from `(x0, y0)` to `(x1, y1)`.
fn rectangle((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Vec<(f64, f64)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A red fill `id` of the rings `rings`, with the settings `params`.
fn shape(id: &str, rings: &[Vec<(f64, f64)>], params: &[(&str, &str)]) -> Element {
    let rings: Vec<&[(f64, f64)]> = rings.iter().map(Vec::as_slice).collect();
    fill(id, &rings, &RED, params)
}

/// The points of the stroke `element`'s path.
fn points(element: &Element) -> Vec<Point> {
    let Shape::Stroke { .. } = &element.shape else { panic!("`{}` is not a stroke: {:?}", element.id, element.shape) };
    element.shape.path().points().collect()
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

/// The elements `elements` are sewn as, in a design with the default settings, and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

#[test]
fn req_auto_013_a_narrow_area_runs_along_its_centre_line() {
    // An L 0.6 mm wide: one running stitch along its middle, from near one end round the corner to near the
    // other, tripled since it is 0.5 mm wide or wider.
    let l = shape("l", &[vec![(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (19.4, 10.0), (19.4, 0.6), (0.0, 0.6)]], &[]);
    let out = made(&l);
    assert_eq!(out.stitches, Some(Stitches::Running));
    assert_eq!(out.elements.len(), 1, "{:?}", out.elements);
    let line = &out.elements[0];
    assert_eq!(line.id, l.id);
    assert!(!line.automatic);
    assert_eq!(setting(line, BEAN_STITCH_REPEATS), Some("1"));
    // Each end of the line is the middle of a chord that crosses the end's corner, as much as a quarter of
    // the width off the middle.
    let middle = |q: Point| distance_to_segment(q, p(0.0, 0.3), p(19.7, 0.3)).min(distance_to_segment(q, p(19.7, 0.3), p(19.7, 10.0)));
    let along = points(line);
    assert!(along.iter().all(|&q| middle(q) < 0.15 + 1e-9), "{along:?}");
    assert!(along[1..along.len() - 1].iter().all(|&q| middle(q) < 0.1), "{along:?}");
    let ends = [along[0], *along.last().unwrap()];
    assert!(ends.iter().any(|q| q.x() < 0.7) && ends.iter().any(|q| q.y() > 9.3), "{ends:?}");
    // The line paints the part, to its neighbours.
    assert_eq!(out.parts.len(), 1);
    assert!((out.parts[0].as_ref().unwrap().area() - (20.0 * 0.6 + 9.4 * 0.6)).abs() < 1e-6);
    // Planned, it sews and says so.
    let outcome = planned(vec![automatic(l)]);
    assert!(outcome.plan.is_some());
    assert!(
        messages(&outcome).contains(&"info SC-I0901: 1 shape is digitized automatically: it sews as a running stitch.".to_string()),
        "{:?}",
        messages(&outcome)
    );
}

#[test]
fn req_auto_013_the_bound_is_the_designs_narrowest_satin_stroke_to_the_micrometre() {
    let strip = |width: f64| shape("strip", &[rectangle((0.0, 0.0), (20.0, width))], &[]);
    // 1 mm wide, the narrowest satin stroke unless the design sets it, runs; 2 micrometres wider is a satin
    // column (REQ-AUTO-016).
    assert_eq!(made(&strip(1.0)).stitches, Some(Stitches::Running));
    assert_eq!(made(&strip(1.002)).stitches, Some(Stitches::Satin));
    // The design's own narrowest satin stroke moves the bound.
    let settings = DesignSettings { min_satin_stroke_width: Mm::new(2.0).unwrap(), ..DesignSettings::default() };
    assert_eq!(made_with(&strip(1.5), &settings).stitches, Some(Stitches::Running));
    assert_eq!(made_with(&strip(2.5), &settings).stitches, Some(Stitches::Satin));
    // A ring narrow everywhere runs round, its hole kept empty: a closed line.
    let ring = shape("ring", &[rectangle((0.0, 0.0), (10.0, 10.0)), rectangle((0.5, 9.5), (9.5, 0.5))], &[]);
    let out = made(&ring);
    assert_eq!(out.stitches, Some(Stitches::Running));
    let Shape::Stroke { path, .. } = &out.elements[0].shape else { panic!() };
    assert_eq!(path.subpaths.len(), 1);
    assert!(path.subpaths[0].closed);
}

#[test]
fn req_auto_013_broad_parts_and_specks_stay_filled_and_the_narrow_ones_run_after_them() {
    // A square, a strip 0.6 mm wide and a speck of 0.75 mm², all one shape with a trim after it.
    let square = rectangle((0.0, 0.0), (10.0, 10.0));
    let strip = rectangle((20.0, 0.0), (40.0, 0.6));
    let speck = rectangle((50.0, 0.0), (50.5, 1.5));
    let parts = shape("parts", &[square, strip, speck], &[("trim_after", "true")]);
    let out = made(&parts);
    assert_eq!(out.stitches, Some(Stitches::Fill));
    let ids: Vec<&str> = out.elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["parts", "parts#2"]);
    // The fill keeps the square and the speck, which the neighbours merge or leave out.
    assert!((area(&out.elements[0]) - 100.75).abs() < 1e-6, "{}", area(&out.elements[0]));
    assert!(matches!(out.elements[1].shape, Shape::Stroke { .. }));
    assert_eq!(out.parts.iter().map(Option::is_some).collect::<Vec<_>>(), [false, true]);
    // The trim follows the last part.
    assert_eq!(setting(&out.elements[0], "trim_after"), None);
    assert_eq!(setting(&out.elements[1], "trim_after"), Some("true"));
    // An area all of whose parts are narrow runs, each part a line of its own, the first with its name.
    let two = shape("two", &[rectangle((0.0, 0.0), (20.0, 0.6)), rectangle((0.0, 5.0), (20.0, 5.4))], &[]);
    let out = made(&two);
    assert_eq!(out.stitches, Some(Stitches::Running));
    assert_eq!(out.elements.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["two", "two#2"]);
    // The narrow parts' widths: 0.6 mm triples, 0.4 mm doesn't.
    assert_eq!(setting(&out.elements[0], BEAN_STITCH_REPEATS), Some("1"));
    assert_eq!(setting(&out.elements[1], BEAN_STITCH_REPEATS), None);
}

#[test]
fn req_auto_013_the_areas_sewn_before_a_narrow_part_reach_under_it() {
    // 2 red squares, and a blue strip 0.6 mm wide between them, as a trace whose colours abut draws them. The
    // strip runs, after both squares, which it borders, and each square reaches 0.4 mm under it.
    let red = |id: &str, from: (f64, f64), to: (f64, f64)| automatic(shape(id, &[rectangle(from, to)], &[]));
    let blue = automatic(fill("strip", &[&rectangle((10.0, 0.0), (10.6, 10.0))], &BLUE, &[]));
    let (out, said) = sewn(vec![red("left", (0.0, 0.0), (10.0, 10.0)), blue, red("right", (10.6, 0.0), (20.6, 10.0))]);
    let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["left", "right", "strip"]);
    assert!(matches!(out[2].shape, Shape::Stroke { .. }));
    // Each reaches into the strip grown by twice the shapes' tolerance, which adds a little past its ends.
    assert!((area(&out[0]) - 104.0).abs() < 0.05, "{}", area(&out[0]));
    assert!((area(&out[1]) - 104.0).abs() < 0.05, "{}", area(&out[1]));
    assert!(area(&out[0]) > 103.99 && area(&out[1]) > 103.99);
    let said: Vec<String> = said.iter().map(ToString::to_string).collect();
    assert!(said.contains(&"info SC-I0904: 2 areas reach 0.4 mm under the shapes sewn after them that they touch.".to_string()), "{said:?}");
}

#[test]
fn req_auto_013_a_narrow_part_that_the_areas_sewn_after_it_cover_whole_sews_nothing() {
    // A strip 1 mm wide under a square drawn over it: left out, as an area covered whole is. One that runs out
    // from under the square sews whole.
    let cover = || automatic(fill("cover", &[&rectangle((0.0, 0.0), (20.0, 20.0))], &BLUE, &[]));
    let hidden = automatic(shape("hidden", &[rectangle((0.1, 2.0), (1.1, 5.0))], &[]));
    let (out, said) = sewn(vec![hidden, cover()]);
    assert_eq!(out.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["cover"]);
    let said: Vec<String> = said.iter().map(ToString::to_string).collect();
    assert!(said.contains(&"info SC-I0904: This area is covered by the areas sewn after it, so it sews nothing.".to_string()), "{said:?}");
    let showing = automatic(shape("showing", &[rectangle((5.0, 15.0), (5.6, 30.0))], &[]));
    let (out, _) = sewn(vec![showing, cover()]);
    assert_eq!(out.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(), ["showing", "cover"]);
}

#[test]
fn req_auto_015_a_running_stitch_is_tripled_from_half_a_millimetre() {
    // Automatic strokes: from 0.5 mm, to the micrometre.
    let stroke = |width: f64| widened(along("line", polylines(&[&[(0.0, 0.0), (20.0, 0.0)]]), &RED, &[]), width);
    for (width, tripled) in [(0.3, false), (0.4994, false), (0.4996, true), (0.5, true), (1.0, true)] {
        let out = made(&stroke(width));
        assert_eq!(out.stitches, Some(Stitches::Running), "{width} mm");
        assert_eq!(setting(&out.elements[0], BEAN_STITCH_REPEATS).is_some(), tripled, "{width} mm");
    }
    // Narrow parts, by the circles that fit inside them centred on their centre lines: a strip's width.
    for (width, tripled) in [(0.45, false), (0.55, true)] {
        let out = made(&shape("strip", &[rectangle((0.0, 0.0), (20.0, width))], &[]));
        assert_eq!(setting(&out.elements[0], BEAN_STITCH_REPEATS).is_some(), tripled, "{width} mm");
        let Shape::Stroke { width: measured, .. } = out.elements[0].shape else { panic!() };
        assert!((measured.get() - width).abs() < 0.01, "{measured:?} for {width} mm");
    }
}

#[test]
fn req_auto_013_finding_the_centre_line_is_the_areas_work() {
    let l = automatic(shape("l", &[vec![(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (19.4, 10.0), (19.4, 0.6), (0.0, 0.6)]], &[]));
    let mut small = Budget { max_stitches: 1_000, max_work: 2_000 }.meter();
    assert_eq!(digitize(&l, &DesignSettings::default(), &mut small), Err(Exhausted::Work));
    assert_eq!(Code::AutoDigitized.id(), "SC-I0901");
}
