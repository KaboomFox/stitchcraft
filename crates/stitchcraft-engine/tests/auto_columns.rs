//! Parts of automatic areas as wide as a satin column (`REQ-AUTO-016` to `REQ-AUTO-018`): a part that isn't
//! narrow, and is nowhere wider than 7 mm, sews as satin columns along its centre lines, between its own
//! outlines, each drawn with its rails and rungs and with the settings of its width; a column that ends under
//! another sews first, longer at that end; and the areas sewn before the columns reach under them.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::{self, sin_cos};
use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_engine::auto::{CONTOUR_FROM, Digitized, Stitches, ZIGZAG_ABOVE, digitize};
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generators::passes::BEAN_STITCH_REPEATS;
use stitchcraft_engine::generators::pull::PULL_COMPENSATION;
use stitchcraft_engine::generators::satin::{
    CENTER_WALK_UNDERLAY, CONTOUR_UNDERLAY, END_AT_NEAREST_POINT, PUSH_COMPENSATION, SATIN_COLUMN, ZIGZAG_UNDERLAY,
};
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::normalize::satin::{Pairing, Shape as Read, recognize};
use stitchcraft_engine::testsheets::find;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, messages, planned, polylines, rules, widened};

/// What the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Digitized {
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// A red fill `id` of the outline `outline`.
fn shape(id: &str, outline: &[(f64, f64)]) -> Element {
    fill(id, &[outline], &RED, &[])
}

/// A rectangle's corners, from `(x0, y0)` to `(x1, y1)`.
fn rectangle((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Vec<(f64, f64)> {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// The underlays `element` turns on, of the centre walk, the contour and the zigzag.
fn underlays(element: &Element) -> Vec<&'static str> {
    [CENTER_WALK_UNDERLAY, CONTOUR_UNDERLAY, ZIGZAG_UNDERLAY].into_iter().filter(|key| setting(element, key) == Some("true")).collect()
}

/// The rails and rungs the satin column `element` is read back with.
fn read(element: &Element) -> ([Vec<Point>; 2], Vec<[Point; 2]>) {
    let Shape::Stroke { path, .. } = &element.shape else { panic!("`{}` is not a stroke: {:?}", element.id, element.shape) };
    let found = recognize(path, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(found.warnings.is_empty(), "{:?}", found.warnings);
    let Ok(Read::Rails(satin)) = found.shape else { panic!("{:?}", found.shape) };
    let Pairing::Rungs(rungs) = satin.pairing else { panic!("{:?}", satin.pairing) };
    (satin.rails, rungs.into_iter().map(|rung| rung.points).collect())
}

/// The area the fill `element` sews, in square millimetres.
fn area(element: &Element) -> f64 {
    let Shape::Fill { path, rule } = &element.shape else { panic!("`{}` is not a fill: {:?}", element.id, element.shape) };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

#[test]
fn req_auto_016_a_band_sews_as_one_satin_column_between_its_sides() {
    // A band 3 mm wide and 20 mm long: one satin column, which keeps the band's name, read back with the
    // band's long sides as its rails and rungs straight across it, and settings that say so.
    let band = shape("band", &rectangle((0.0, 0.0), (20.0, 3.0)));
    let out = made(&band);
    assert_eq!(out.stitches, Some(Stitches::Satin));
    let [column] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    assert_eq!(column.id.as_str(), "band");
    assert!(!column.automatic);
    let Shape::Stroke { width, .. } = column.shape else { panic!() };
    assert!((width.get() - 3.0).abs() < 1e-6, "{width:?}");
    let (rails, rungs) = read(column);
    for (rail, y) in rails.iter().zip(if rails[0][0].y() == 0.0 { [0.0, 3.0] } else { [3.0, 0.0] }) {
        assert!(rail.iter().all(|q| q.y() == y), "{rail:?}");
        assert_eq!((rail[0].x() - rail.last().unwrap().x()).abs(), 20.0, "{rail:?}");
    }
    assert!(!rungs.is_empty() && rungs.iter().all(|[a, b]| (a.x() - b.x()).abs() < 1e-6), "{rungs:?}");
    assert_eq!(setting(column, SATIN_COLUMN), Some("true"));
    // The column paints the band to its neighbours.
    assert!((out.parts[0].as_ref().unwrap().area() - 60.0).abs() < 1e-6);
    // Planned, it sews and says so.
    let outcome = planned(vec![automatic(band)]);
    assert!(outcome.plan.is_some());
    let said = messages(&outcome);
    assert!(said.contains(&"info SC-I0901: 1 shape is digitized automatically: it sews as a satin column.".to_string()), "{said:?}");
}

#[test]
fn req_auto_016_the_bound_is_the_widest_satin_column_to_the_micrometre() {
    // 7 mm wide, the widest satin column: shrunk by 3.5 mm taken half a micrometre wider, nothing of it is
    // left, so it sews satin columns. 10 micrometres wider is filled.
    let band = |width: f64| shape("band", &rectangle((0.0, 0.0), (30.0, width)));
    assert_eq!(made(&band(7.0)).stitches, Some(Stitches::Satin));
    assert_eq!(made(&band(7.01)).stitches, Some(Stitches::Fill));
}

#[test]
fn req_auto_016_a_column_that_ends_under_another_sews_first_and_reaches_under_it() {
    // A T: a bar 3 mm wide, and a stem 3 mm wide below it. The stem's column ends under the bar's: it sews
    // first, as `t`, and is 0.4 mm longer at that end by its push compensation; the bar's sews after it.
    let t = shape("t", &[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (11.5, 3.0), (11.5, 15.0), (8.5, 15.0), (8.5, 3.0), (0.0, 3.0)]);
    let out = made(&t);
    let ids: Vec<&str> = out.elements.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["t", "t#2"]);
    let (stem, bar) = (&out.elements[0], &out.elements[1]);
    let (stem_rails, _) = read(stem);
    assert!(stem_rails.iter().flatten().all(|q| q.y() >= 3.0 - 1e-9), "{stem_rails:?}");
    let under = format!("{}", -rules().auto_overlap_mm.get());
    let push = setting(stem, PUSH_COMPENSATION).unwrap();
    assert!(push == format!("{under} 0") || push == format!("0 {under}"), "{push}");
    assert_eq!(setting(bar, PUSH_COMPENSATION), None);
    // The stem's end under the bar is the bar's side, from the stem's one side to its other.
    let ends: Vec<Point> = stem_rails.iter().flat_map(|rail| [rail[0], *rail.last().unwrap()]).filter(|q| q.y() < 3.0 + 1e-9).collect();
    assert_eq!(ends.len(), 2, "{stem_rails:?}");
    assert!(ends.iter().any(|q| q.x() == 8.5) && ends.iter().any(|q| q.x() == 11.5), "{ends:?}");
}

#[test]
fn req_auto_016_a_v_sews_as_2_columns_one_ending_under_the_other() {
    // A V whose arms, 3 mm wide, meet in a flat end: its line turns by more than 60° there, and the arms sew
    // as 2 satin columns, the one that ends under the other first, 0.4 mm longer at that end.
    let v = shape("v", &[(0.0, 15.0), (3.0, 15.0), (6.0, 3.0), (9.0, 15.0), (12.0, 15.0), (7.5, 0.0), (4.5, 0.0)]);
    let out = made(&v);
    assert_eq!(out.stitches, Some(Stitches::Satin));
    let [first, second] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    let under = format!("{}", -rules().auto_overlap_mm.get());
    let push = setting(first, PUSH_COMPENSATION).unwrap();
    assert!(push == format!("{under} 0") || push == format!("0 {under}"), "{push}");
    assert_eq!((setting(first, SATIN_COLUMN), setting(second, SATIN_COLUMN)), (Some("true"), Some("true")));
    assert_eq!(setting(second, PUSH_COMPENSATION), None);
}

#[test]
fn req_auto_016_a_column_too_narrow_for_satin_runs_along_its_line() {
    // A T whose bar is 4 mm wide and whose stem is 1.5 mm wide, in a design whose narrowest satin stroke is
    // 2 mm: the stem's column runs along its line as a triple run, as a stroke that narrow would, and the
    // bar's sews as a satin column.
    let t = shape("t", &[(0.0, 0.0), (20.0, 0.0), (20.0, 4.0), (10.75, 4.0), (10.75, 16.0), (9.25, 16.0), (9.25, 4.0), (0.0, 4.0)]);
    let settings = DesignSettings { min_satin_stroke_width: Mm::new(2.0).unwrap(), ..DesignSettings::default() };
    let out = digitize(&automatic(t), &settings, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(out.stitches, Some(Stitches::Satin));
    let [stem, bar] = out.elements.as_slice() else { panic!("{:?}", out.elements) };
    let Shape::Stroke { width, .. } = stem.shape else { panic!("{:?}", stem.shape) };
    assert!((width.get() - 1.5).abs() < 0.1, "{width:?}");
    assert_eq!((setting(stem, SATIN_COLUMN), setting(stem, BEAN_STITCH_REPEATS)), (None, Some("1")));
    assert_eq!(setting(bar, SATIN_COLUMN), Some("true"));
}

#[test]
fn req_auto_016_a_letters_legs_sew_alike() {
    // TS-13B's letter A 6 mm tall, its legs 1.08 mm wide: wider than the narrowest satin stroke, 1 mm, so both
    // sew as satin columns, as its crossbar does. The line along one leg ran on into the corner at its foot,
    // where it measures nothing, and its width, averaged along it, came out under 1 mm: that leg sewed as a
    // line beside the other's column, whose line took in the junction at the top. Which leg did turned with
    // the drawing (`tests/transformed.rs`). Measured without its piece into the corner, each leg is a column.
    let design = find("TS-13B").unwrap().design().unwrap().unwrap();
    let a = design.elements().iter().find(|element| element.id.as_str() == "ts13b:auto:word-6:a").unwrap();
    let out = digitize(a, &design.settings, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(out.elements.len(), 3, "{:?}", out.elements);
    for column in &out.elements {
        assert_eq!(setting(column, SATIN_COLUMN), Some("true"), "{}", column.id);
    }
}

#[test]
fn req_auto_016_the_areas_sewn_before_a_column_reach_under_it() {
    // 2 red squares, and a blue band 3 mm wide between them, as a trace whose colours abut draws them. The
    // band sews as a satin column, after both squares, which it borders, and each square reaches 0.4 mm
    // under it.
    let red = |id: &str, from: (f64, f64), to: (f64, f64)| automatic(shape(id, &rectangle(from, to)));
    let blue = automatic(fill("band", &[&rectangle((10.0, 0.0), (13.0, 10.0))], &BLUE, &[]));
    let design = stitchcraft_engine::design::Design::new(
        vec![red("left", (0.0, 0.0), (10.0, 10.0)), blue, red("right", (13.0, 0.0), (23.0, 10.0))],
        DesignSettings::default(),
    )
    .unwrap();
    let (out, _) = stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT);
    let ids: Vec<&str> = out.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, ["left", "right", "band"]);
    assert_eq!(setting(&out[2], SATIN_COLUMN), Some("true"));
    assert!((area(&out[0]) - 104.0).abs() < 0.05 && (area(&out[1]) - 104.0).abs() < 0.05, "{} {}", area(&out[0]), area(&out[1]));
}

#[test]
fn req_auto_017_a_curved_columns_stitches_lie_square_to_it() {
    // Half a ring 3 mm wide, its middle 10.5 mm from its centre: each stitch across the whole of it runs
    // along the radius through its middle, to within 10°.
    let centre = (20.0, 20.0);
    let ring = |r: f64, from: f64, to: f64| -> Vec<(f64, f64)> {
        (0..=36)
            .map(|i| {
                let (sin, cos) = sin_cos(from + (to - from) * f64::from(i) / 36.0);
                (centre.0 + r * cos, centre.1 + r * sin)
            })
            .collect()
    };
    let mut outline = ring(12.0, 0.0, std::f64::consts::PI);
    outline.extend(ring(9.0, std::f64::consts::PI, 0.0));
    let half = shape("half", &outline);
    assert_eq!(made(&half).stitches, Some(Stitches::Satin));
    let plan = planned(vec![automatic(half)]).plan.unwrap();
    let tops: Vec<Point> = plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect();
    let radius = |q: Point| math::hypot(q.x() - centre.0, q.y() - centre.1);
    let mut across = 0;
    for pair in tops.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        // A stitch across the whole column, from past one rail to past the other: not one of the contour
        // underlay's, along a rail, nor one shortened on the inside of the curve.
        if (radius(a) - radius(b)).abs() < 3.0 {
            continue;
        }
        across += 1;
        let middle = a.lerp(b, 0.5);
        let (rx, ry) = (middle.x() - centre.0, middle.y() - centre.1);
        let (sx, sy) = (b.x() - a.x(), b.y() - a.y());
        let cosine = (rx * sx + ry * sy).abs() / (math::hypot(rx, ry) * math::hypot(sx, sy));
        assert!(cosine >= math::cos(math::to_radians(10.0)), "{a:?} {b:?}: {}°", math::to_degrees(math::acos(cosine.min(1.0))));
    }
    assert!(across > 50, "{across}");
}

#[test]
fn req_auto_018_a_column_has_the_underlay_and_settings_of_its_width() {
    // Compared to the micrometre: a centre walk below 2 mm, a contour from 2 mm to 4 mm, and a contour and a
    // zigzag wider, for a stroke's column and for a part's.
    let stroke = |width: f64| widened(along("line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), &RED, &[]), width);
    let band = |width: f64| shape("band", &rectangle((0.0, 0.0), (30.0, width)));
    let walk: &[&str] = &[CENTER_WALK_UNDERLAY];
    let contour: &[&str] = &[CONTOUR_UNDERLAY];
    let both: &[&str] = &[CONTOUR_UNDERLAY, ZIGZAG_UNDERLAY];
    let (from, above) = (CONTOUR_FROM.get(), ZIGZAG_ABOVE.get());
    for (width, expected) in
        [(1.5, walk), (from - 0.001, walk), (from, contour), (3.0, contour), (above, contour), (above + 0.001, both), (5.0, both)]
    {
        for made in [made(&stroke(width)), made(&band(width))] {
            assert_eq!(made.stitches, Some(Stitches::Satin), "{width} mm");
            let column = &made.elements[0];
            assert_eq!(underlays(column), expected, "{width} mm: {}", column.id);
            // Pull compensation of the overlap an area reaches under its neighbours with, and no end near the
            // next shape.
            assert_eq!(setting(column, PULL_COMPENSATION), Some(rules().auto_overlap_mm.get().to_string().as_str()));
            assert_eq!(setting(column, END_AT_NEAREST_POINT), Some("false"));
        }
    }
}
