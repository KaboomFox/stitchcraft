//! Reading an SVG file with `--auto` (`REQ-AUTO-001`, `REQ-AUTO-003`): a shape with no `inkstitch:`
//! attribute is an automatic element, and a stroke's `stroke-linecap` is read, as it inherits.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Cap, Element, Reading, Shape};
use stitchcraft_svg::{Svg, read_with};

/// A document of `shapes`, a millimetre to the user unit, with Ink/Stitch's namespace declared.
fn document(shapes: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">{shapes}</svg>"#
    )
}

/// `document` read with `--auto` or without.
fn read(document: &str, auto: bool) -> Svg {
    read_with(document.as_bytes(), &Budget::DEFAULT, Reading { auto }).unwrap()
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
    // Without the switch, nothing is, and the design is otherwise the same.
    let without = read(&shapes, false);
    assert!(without.design.elements().iter().all(|e| !e.automatic));
    let unmarked: Vec<Element> = with.design.elements().iter().map(|e| Element { automatic: false, ..e.clone() }).collect();
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
