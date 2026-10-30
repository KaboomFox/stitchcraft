//! Automatic areas wider than a satin column in places (`REQ-AUTO-019`): their broad parts are filled, and
//! their narrow parts sew over the fill as lines and columns, cut square where they meet the broad parts,
//! and the fill reaches under them.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Exhausted, Point};
use stitchcraft_engine::auto::{Digitized, Stitches, digitize, digitized};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::generators::passes::BEAN_STITCH_REPEATS;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::normalize::satin::{Shape as Read, recognize};
use stitchcraft_testkit::designs::{RED, automatic, fill};

/// What the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Digitized {
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// A red fill `id` of the outline `outline`.
fn shape(id: &str, outline: &[(f64, f64)]) -> Element {
    fill(id, &[outline], &RED, &[])
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// The rails the satin column `element` is read back with.
fn rails(element: &Element) -> [Vec<Point>; 2] {
    let Shape::Stroke { path, .. } = &element.shape else { panic!("`{}` is not a stroke: {:?}", element.id, element.shape) };
    let Ok(Read::Rails(satin)) = recognize(path, &mut Budget::DEFAULT.meter()).unwrap().shape else { panic!("{path:?}") };
    satin.rails
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

/// A square 20 mm a side with a tab `width` mm wide and 12 mm long on its right side, halfway up.
fn flag(width: f64) -> Element {
    let (low, high) = (10.0 - width / 2.0, 10.0 + width / 2.0);
    shape("flag", &[(0.0, 0.0), (20.0, 0.0), (20.0, low), (32.0, low), (32.0, high), (20.0, high), (20.0, 20.0), (0.0, 20.0)])
}

#[test]
fn req_auto_019_a_shapes_narrow_part_sews_as_a_column_over_the_fill_of_its_broad_part() {
    // A square with a tab 3 mm wide: the square is filled, and keeps the shape's name, and the tab sews after
    // it as a satin column, `#2`, from the square's side to the tab's end, between the tab's sides.
    let out = made(&flag(3.0));
    assert_eq!(out.stitches, Some(Stitches::Fill));
    let ids: Vec<&str> = out.elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["flag", "flag#2"]);
    let (square, tab) = (&out.elements[0], &out.elements[1]);
    assert!((area(square) - 400.0).abs() < 0.1, "{}", area(square));
    assert_eq!(setting(tab, SATIN_COLUMN), Some("true"));
    for rail in rails(tab) {
        assert!(rail.iter().all(|q| q.y() == rail[0].y()) && [8.5, 11.5].contains(&rail[0].y()), "{rail:?}");
        let xs: Vec<f64> = [rail[0], *rail.last().unwrap()].iter().map(|q| q.x()).collect();
        assert!(xs.iter().any(|&x| (x - 20.0).abs() < 0.01) && xs.contains(&32.0), "{rail:?}");
    }
    // The column paints the tab to its neighbours.
    assert!((out.parts[1].as_ref().unwrap().area() - 36.0).abs() < 0.1);
}

#[test]
fn req_auto_019_a_narrow_part_too_narrow_for_satin_runs_along_its_centre_line() {
    // A square with a tab 0.8 mm wide: the tab is narrow, and runs along its middle after the square's fill,
    // tripled, as a line 0.8 mm wide is.
    let out = made(&flag(0.8));
    let [square, tab] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    assert!(matches!(square.shape, Shape::Fill { .. }) && (area(square) - 400.0).abs() < 0.1, "{:?}", square.shape);
    let Shape::Stroke { path, .. } = &tab.shape else { panic!("{:?}", tab.shape) };
    assert!(path.subpaths.len() == 1 && setting(tab, BEAN_STITCH_REPEATS) == Some("1") && setting(tab, SATIN_COLUMN).is_none());
}

#[test]
fn req_auto_019_the_fill_reaches_under_the_columns_sewn_over_it() {
    // Digitized with its neighbours, the square reaches 0.4 mm under the tab's column, 3 mm wide: 1.2 mm²
    // more.
    let design = Design::new(vec![automatic(flag(3.0))], DesignSettings::default()).unwrap();
    let (out, _) = digitized(&design, &Budget::DEFAULT);
    let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["flag", "flag#2"]);
    assert!((area(&out[0]) - 401.2).abs() < 0.1, "{}", area(&out[0]));
}

#[test]
fn req_auto_019_a_shape_without_narrow_parts_is_one_fill_as_drawn() {
    // A square with a tab 4 mm long: the tab is a stub too short for a column, and the shape is filled whole,
    // as it is drawn.
    let stub = shape("stub", &[(0.0, 0.0), (20.0, 0.0), (20.0, 8.5), (24.0, 8.5), (24.0, 11.5), (20.0, 11.5), (20.0, 20.0), (0.0, 20.0)]);
    let out = made(&stub);
    assert_eq!(out.stitches, Some(Stitches::Fill));
    let [whole] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    assert_eq!(whole.shape, stub.shape);
}

#[test]
fn req_auto_019_splitting_a_part_is_the_areas_work() {
    // The opening that finds the broad parts, and the columns of the narrow ones, cost the work of
    // digitizing the shape: a budget too small for them stops it.
    let small = Budget { max_stitches: 1_000, max_work: 2_000 };
    assert_eq!(digitize(&automatic(flag(3.0)), &DesignSettings::default(), &mut small.meter()), Err(Exhausted::Work));
}

#[test]
fn req_auto_019_a_tapering_narrow_part_ends_flat_along_its_cut() {
    // A star of 5 points, 30 mm across, its middle broad: the middle is filled, and each point sews as a
    // satin column from its tip to its cut, where its rails end at the cut's 2 ends, 6.6 mm apart, not at a
    // point.
    let star: Vec<(f64, f64)> = (0..10)
        .map(|i| {
            let r = if i % 2 == 0 { 15.0 } else { 6.0 };
            let (sin, cos) = sin_cos(-std::f64::consts::FRAC_PI_2 + std::f64::consts::PI * f64::from(i) / 5.0);
            (r * cos, r * sin)
        })
        .collect();
    let out = made(&shape("star", &star));
    let [fill, points @ ..] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    assert!(matches!(fill.shape, Shape::Fill { .. }) && points.len() == 5, "{:?}", out.elements);
    for point in points {
        let [one, other] = rails(point);
        let apart = |at: fn(&Vec<Point>) -> Point| at(&one).distance(at(&other));
        let (start, end) = (apart(|rail| rail[0]), apart(|rail| *rail.last().unwrap()));
        assert!(start.max(end) > 6.5 && start.min(end) < 0.5, "{start} {end}");
    }
}
