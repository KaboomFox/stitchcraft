//! Reading an SVG file with `--auto` (`REQ-AUTO-001` to `REQ-AUTO-005`): a shape with no `inkstitch:`
//! attribute is an automatic element, a stroke's `stroke-linecap` is read, as it inherits, and an automatic
//! line follows the drawing: its width as drawn, to the micrometre, its corners as SVG joins them, and a
//! transform that stretches it unevenly named.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::{Budget, Code, Mm};
use stitchcraft_engine::auto::Stitches;
use stitchcraft_engine::design::{Cap, Element, Join, Reading, Shape};
use stitchcraft_svg::{Svg, read_with};

/// A document of `shapes`, a millimetre to the user unit, with Ink/Stitch's namespace declared.
fn document(shapes: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">{shapes}</svg>"#
    )
}

/// `document` read with `--auto` or without.
fn read(document: &str, auto: bool) -> Svg {
    read_with(document.as_bytes(), &Budget::DEFAULT, Reading { auto, ..Reading::default() }).unwrap()
}

/// Each element's id, and whether it is automatic.
fn automatic(svg: &Svg) -> Vec<(&str, bool)> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.automatic)).collect()
}

#[test]
fn req_auto_001_a_shape_without_inkstitch_attributes_is_automatic() {
    let shapes = document(
        r#"<path id="plain" d="M 0 0 L 50 0" stroke="red" stroke-width="3" fill="none"/>
           <path id="set" d="M 0 10 L 50 10" stroke="red" stroke-width="3" fill="none" inkstitch:satin_column="true"/>
           <rect id="area" x="0" y="20" width="10" height="10" fill="blue" stroke="red"/>"#,
    );
    let with = read(&shapes, true);
    assert_eq!(automatic(&with), [("svg:plain:stroke", true), ("svg:set:stroke", false), ("svg:area:fill", true), ("svg:area:stroke", true)]);
    // Without the switch, nothing is, and the design is otherwise the same, but for the corners of the
    // automatic lines, joined as SVG draws them (`REQ-AUTO-004`), where Ink/Stitch reads a miter limited at 5.
    let without = read(&shapes, false);
    assert!(without.design.elements().iter().all(|e| !e.automatic));
    let unmarked: Vec<Element> = with
        .design
        .elements()
        .iter()
        .map(|e| match &e.shape {
            Shape::Stroke { path, width, join, cap } if e.automatic => {
                assert_eq!(*join, Join::Miter { limit: 4.0 }, "{}", e.id);
                Element { automatic: false, shape: Shape::Stroke { path: path.clone(), width: *width, join: Join::UNSET, cap: *cap }, ..e.clone() }
            }
            _ => Element { automatic: false, ..e.clone() },
        })
        .collect();
    assert_eq!(unmarked, without.design.elements());
}

#[test]
fn req_auto_003_stroke_linecap_is_read_as_it_inherits() {
    let shapes = document(
        r#"<g fill="none"><path id="butt" d="M 0 0 L 50 0" stroke="red"/>
           <path id="round" d="M 0 10 L 50 10" stroke="red" stroke-linecap="round"/>
           <path id="square" d="M 0 20 L 50 20" stroke="red" style="stroke-linecap:SQUARE"/>
           <g stroke-linecap="round"><path id="inherited" d="M 0 30 L 50 30" stroke="red"/>
             <path id="unread" d="M 0 40 L 50 40" stroke="red" stroke-linecap="pointy"/></g></g>"#,
    );
    let svg = read(&shapes, true);
    let caps: Vec<(String, Cap)> = svg
        .design
        .elements()
        .iter()
        .map(|e| match e.shape {
            Shape::Stroke { cap, .. } => (e.id.to_string(), cap),
            Shape::Fill { .. } => panic!("{e:?}"),
        })
        .collect();
    // A value that does not read is ignored, as a viewer ignores it: the cap inherited stays.
    let expected = [("butt", Cap::Butt), ("round", Cap::Round), ("square", Cap::Square), ("inherited", Cap::Round), ("unread", Cap::Round)];
    assert_eq!(caps, expected.map(|(id, cap)| (format!("svg:{id}:stroke"), cap)));
}

/// Each stroke's id and join, read with `--auto`.
fn joins(svg: &Svg) -> Vec<(String, Join)> {
    svg.design
        .elements()
        .iter()
        .filter_map(|e| match e.shape {
            Shape::Stroke { join, .. } => Some((e.id.to_string(), join)),
            Shape::Fill { .. } => None,
        })
        .collect()
}

#[test]
fn req_auto_004_an_automatic_line_s_corners_are_joined_as_svg_draws_them() {
    // SVG's initial join is a miter, limited by `stroke-miterlimit`, 4 unless set; SVG 2's joins are
    // miters too. A shape with settings keeps Ink/Stitch's reading of a join not set: a miter limited at 5.
    let corner = |id: &str, extra: &str| format!(r#"<path id="{id}" d="M 0 0 L 40 0 L 13 13" stroke="red" stroke-width="8" fill="none" {extra}/>"#);
    let shapes = document(
        &[
            corner("unset", ""),
            corner("limited", r#"stroke-miterlimit="10""#),
            corner("clip", r#"stroke-linejoin="miter-clip""#),
            corner("round", r#"stroke-linejoin="round""#),
            corner("set", r#"inkstitch:satin_column="true""#),
        ]
        .concat(),
    );
    let expected = [
        ("unset", Join::Miter { limit: 4.0 }),
        ("limited", Join::Miter { limit: 10.0 }),
        ("clip", Join::Miter { limit: 4.0 }),
        ("round", Join::Round),
        ("set", Join::UNSET),
    ];
    assert_eq!(joins(&read(&shapes, true)), expected.map(|(id, join)| (format!("svg:{id}:stroke"), join)));
    // Without the switch, every join is read as Ink/Stitch reads it: a miter limit applies only to a join
    // set to `miter`.
    let unset = Join::UNSET;
    assert_eq!(joins(&read(&shapes, false)).iter().map(|(_, join)| *join).collect::<Vec<_>>(), [unset, unset, unset, Join::Round, unset]);
}

#[test]
fn req_auto_002_a_width_in_css_pixels_is_measured_to_the_micrometre() {
    // 7 mm and 1 mm, written in CSS pixels, read a rounding error wider: they sew as 7 mm and 1 mm do.
    let shapes = r#"<svg xmlns="http://www.w3.org/2000/svg" width="378" height="378">
        <path id="seven" d="M 40 100 L 300 100" fill="none" stroke="black" stroke-width="26.45669291338583"/>
        <path id="one" d="M 40 200 L 300 200" fill="none" stroke="black" stroke-width="3.7795275590551185"/></svg>"#;
    let svg = read(shapes, true);
    let widths: Vec<f64> = svg.design.elements().iter().map(|e| if let Shape::Stroke { width, .. } = e.shape { width.get() } else { 0.0 }).collect();
    assert_eq!(widths, [7.0_f64.next_up(), 1.0_f64.next_up()], "a rounding error wider");
    let rules = stitchcraft_testkit::designs::rules();
    let stitches: Vec<Stitches> = widths.iter().map(|&width| Stitches::of_stroke(Mm::new(width).unwrap(), &svg.design.settings, &rules)).collect();
    assert_eq!(stitches, [Stitches::Satin, Stitches::Running]);
}

#[test]
fn req_auto_005_a_line_its_transforms_stretch_unevenly_is_named() {
    // Scaled 3 times down the page, a line 2 user units wide along x is drawn 6 mm wide and sewn 4 mm wide:
    // as its transforms stretch it on average. So is a skewed one. Not a line that runs, one scaled evenly,
    // one with settings, nor any without the switch.
    let line = |id: &str, width: &str, extra: &str| {
        format!(r#"<path id="{id}" d="M 10 10 L 60 10" stroke="red" stroke-width="{width}" fill="none" {extra}/>"#)
    };
    let shapes = document(
        &[
            line("band", "2", r#"transform="scale(1,3)""#),
            line("wide", "6", r#"transform="scale(1,3)""#),
            line("skewed", "2", r#"transform="skewX(30)""#),
            line("thin", "0.25", r#"transform="scale(1,3)""#),
            line("even", "2", r#"transform="rotate(30) scale(2)""#),
            line("set", "2", r#"transform="scale(1,3)" inkstitch:satin_column="true""#),
        ]
        .concat(),
    );
    let named = |what: &str, id: &str| {
        format!(
            "warning SC-W0802: The {what} of `{id}` is as wide as its stroke all along, as wide as its transforms stretch it on average, \
             where they stretch it more one way than the other and draw it wider one way."
        )
    };
    let said = |auto: bool| {
        let read = read(&shapes, auto);
        read.warnings.iter().filter(|w| w.code == Code::SvgFeatureIgnored && w.element.is_some()).map(ToString::to_string).collect::<Vec<_>>()
    };
    assert_eq!(said(true), [named("satin column", "band"), named("fill", "wide"), named("satin column", "skewed")]);
    assert_eq!(said(false), Vec::<String>::new());
    let svg = read(&shapes, true);
    let band = svg.warnings.iter().find(|w| w.element.as_ref().map(|e| e.as_str()) == Some("svg:band")).unwrap();
    assert_eq!(
        band.fix.as_ref().map(|f| f.describe()),
        Some("Scale the line evenly, or make a shape of its stroke (in Inkscape: Path › Stroke to Path).".to_string())
    );
}
