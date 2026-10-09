//! `REQ-SVG-002`: any path data, degenerate arcs included, is read the way the SVG specification says
//! (implementation notes F.5 and F.6), and never fails the adapter: what cannot be used is reported and
//! left out, and everything else is read.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

mod common;

use common::{all_at, at, ends, fixture, ids, svg, warnings};
use proptest::prelude::*;
use stitchcraft_core::{Code, Point};
use stitchcraft_engine::design::{Segment, WORKING_LIMIT_MM};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

#[test]
fn req_svg_002_degenerate_geometry_follows_the_specification() {
    let svg = svg(fixture("degenerate.svg"));
    assert_eq!(ids(&svg), ["svg:zero-radius:stroke", "svg:same-ends:stroke", "svg:tiny-radii:stroke", "svg:broken:stroke"]);
    let e = svg.design.elements();
    // A zero radius draws a straight line.
    assert_eq!(e[0].shape.path().subpaths[0].segments, [Segment::Line(p(20.0, 10.0))]);
    // An arc that ends where it starts draws nothing; the line after it is drawn.
    assert_eq!(e[1].shape.path().subpaths[0].segments, [Segment::Line(p(20.0, 20.0))]);
    // Radii too small for the end points grow until they fit: here a half circle of radius 10, in six
    // pieces, through (20, 20) at the top, ending exactly at its end point.
    let half = &ends(&e[2])[0];
    assert_eq!(half.len(), 7);
    all_at(&[half[0], half[3], half[6]], &[(10.0, 30.0), (20.0, 20.0), (30.0, 30.0)]);
    assert_eq!(half[6], p(30.0, 30.0));
    // An error ends the path; what came before it is stitched.
    all_at(&ends(&e[3])[0], &[(10.0, 40.0), (20.0, 40.0)]);
    assert!(svg.warnings.iter().all(|w| w.code == Code::SvgGeometryUnusable), "{:?}", warnings(&svg));
    assert_eq!(svg.warnings.len(), 5, "broken, empty, flat, far and dot: {:?}", warnings(&svg));
}

/// A number as path data may write it: plain, signed, fractional, exponents, extremes, nonsense.
fn number() -> impl Strategy<Value = String> {
    prop_oneof![
        4 => (-200.0..200.0_f64).prop_map(|v| format!("{v}")),
        2 => (-200_i32..200).prop_map(|v| v.to_string()),
        1 => prop::sample::select(vec!["0", "-0", ".5", "-.5e-3", "1e308", "-1e308", "1e-320", "1e400", "1.", "+3", "0.0.1", "1e", "-", "inf", "NaN"])
            .prop_map(str::to_string),
        1 => any::<f64>().prop_map(|v| format!("{v:e}")),
    ]
}

/// One command with its arguments; arcs get flags, which must be 0 or 1 (and sometimes are not).
fn command() -> impl Strategy<Value = String> {
    let letter = prop::sample::select("MmLlHhVvCcSsQqTtZzAa".chars().collect::<Vec<_>>());
    let flag = prop::sample::select(vec!["0", "1", "0", "1", "2"]);
    let separator = prop::sample::select(vec![" ", ",", " , ", ""]);
    (letter, prop::collection::vec(number(), 0..8), flag.clone(), flag, separator).prop_map(|(letter, numbers, f1, f2, sep)| {
        let mut args = numbers;
        if letter.eq_ignore_ascii_case(&'a') && args.len() >= 5 {
            args[3] = f1.to_string();
            args[4] = f2.to_string();
        }
        format!("{letter}{sep}{}", args.join(sep))
    })
}

fn path_data() -> impl Strategy<Value = String> {
    prop::collection::vec(command(), 0..12).prop_map(|commands| commands.join(" "))
}

fn transform() -> impl Strategy<Value = String> {
    prop_oneof![
        Just(String::new()),
        number().prop_map(|a| format!("rotate({a})")),
        (number(), number()).prop_map(|(a, b)| format!("scale({a} {b})")),
        (number(), number()).prop_map(|(a, b)| format!("translate({a},{b}) skewX({a})")),
        prop::collection::vec(number(), 6).prop_map(|m| format!("matrix({})", m.join(" "))),
    ]
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(512))]

    /// Whatever the path data and transform, the file is read: geometry that cannot be used is a warning
    /// (`SC-W0804`), and everything in the design lies within reach of a machine file.
    #[test]
    fn req_svg_002_any_path_data_reads_without_failing(data in path_data(), transform in transform()) {
        let file = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100mm" height="100mm"><path transform="{transform}" d="{data}" fill="red" stroke="blue"/></svg>"#
        );
        let svg = stitchcraft_svg::read(file.as_bytes(), &stitchcraft_core::Budget::DEFAULT);
        prop_assert!(svg.is_ok(), "{:?}", svg.err());
        let svg = svg.unwrap();
        for warning in &svg.warnings {
            prop_assert_eq!(warning.code, Code::SvgGeometryUnusable, "{}", warning);
        }
        for element in svg.design.elements() {
            for point in element.shape.path().points() {
                prop_assert!(point.x().abs() <= WORKING_LIMIT_MM && point.y().abs() <= WORKING_LIMIT_MM, "{:?}", point);
            }
        }
    }
}

#[test]
fn req_svg_002_huge_numbers_are_reported_not_failed_on() {
    // A straight line 1e308 user units long, and a half ellipse with radii of 1e308 between two points
    // almost on top of each other: both really do reach far beyond 10 m, so both are left out.
    let svg = svg(r#"<svg xmlns="http://www.w3.org/2000/svg">
          <path id="huge" d="M 0 0 L 1e308 0" stroke="red" fill="none"/>
          <path id="arc" d="M 0 0 A 1e308 1e308 0 1 1 1e-300 0" stroke="red" fill="none"/>
          <path id="fine" d="M 0 0 L 96 96" stroke="red" fill="none"/>
        </svg>"#);
    assert_eq!(ids(&svg), ["svg:fine:stroke"]);
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0804: `huge` cannot be stitched: it lies more than 10 m from the document's origin. It is left out.",
            "warning SC-W0804: `arc` cannot be stitched: it lies more than 10 m from the document's origin. It is left out.",
        ]
    );
    at(ends(&svg.design.elements()[0])[0][1], 25.4, 25.4);
}

#[test]
fn req_svg_002_a_transform_that_cannot_be_used_is_reported() {
    let svg = svg(r#"<svg xmlns="http://www.w3.org/2000/svg"><g id="group" transform="rotate(45"><path d="M 0 0 L 1 1" stroke="red"/></g></svg>"#);
    assert_eq!(ids(&svg), Vec::<&str>::new());
    assert_eq!(
        warnings(&svg),
        ["warning SC-W0804: The transform of `group` cannot be used (unexpected end of stream), so it is left out with everything inside it."]
    );
}
