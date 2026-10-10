//! `REQ-SVG-004`: a stroke's width and join, as Ink/Stitch reads them. A satin column drawn as one path
//! takes its width and its corners from them.
//!
//! The width is `stroke-width`, inherited, 1 user unit when nothing sets it, and scaled by the average of
//! how far the shape's transform stretches the x and y axes. The join is `stroke-linejoin`: round and bevel
//! as they say, a miter limited by `stroke-miterlimit` (4 when unset) when it says miter, and otherwise a
//! miter limited at 5.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{svg, warnings};
use stitchcraft_core::units::MM_PER_SVG_PX as MM_PER_PX;
use stitchcraft_engine::design::{Join, Shape};

/// A document in CSS pixels, 200 × 100 user units, holding `body`.
fn px_document(body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100">{body}</svg>"#)
}

/// The width in millimetres and the join of the strokes of `document`, in order.
fn strokes(document: &str) -> Vec<(f64, Join)> {
    let svg = svg(document);
    svg.design
        .elements()
        .iter()
        .filter_map(|e| match &e.shape {
            Shape::Stroke { width, join, .. } => Some((width.get(), *join)),
            Shape::Fill { .. } => None,
        })
        .collect()
}

/// The width of the first stroke of `document`, in CSS pixels.
fn width_px(document: &str) -> f64 {
    strokes(document)[0].0 / MM_PER_PX
}

const LINE: &str = r#"d="M 10 10 L 90 10""#;

#[test]
fn req_svg_004_a_stroke_is_1_user_unit_wide_unless_it_says_otherwise() {
    let w = width_px(&px_document(&format!(r#"<path {LINE} stroke="black" fill="none"/>"#)));
    assert!((w - 1.0).abs() < 1e-12, "{w}");
    // As an attribute, in the style, with units: millimetres are 96 to the inch of user space.
    let w = width_px(&px_document(&format!(r#"<path {LINE} stroke="black" stroke-width="3" fill="none"/>"#)));
    assert!((w - 3.0).abs() < 1e-12, "{w}");
    let [(mm, _)] = strokes(&px_document(&format!(r#"<path {LINE} style="stroke:black;stroke-width:2mm;fill:none"/>"#)))[..] else { panic!() };
    assert!((mm - 2.0).abs() < 1e-9, "{mm}");
    // A percentage is of the viewport's diagonal over √2, here √((200² + 100²) / 2) user units.
    let w = width_px(&px_document(&format!(r#"<path {LINE} stroke="black" stroke-width="10%" fill="none"/>"#)));
    assert!((w - 0.1 * 25_000.0_f64.sqrt()).abs() < 1e-9, "{w}");
}

#[test]
fn req_svg_004_the_width_is_inherited_and_scaled_by_the_transforms() {
    let inside = |group: &str| width_px(&px_document(&format!(r#"<g {group}><path {LINE} stroke="black" fill="none"/></g>"#)));
    assert!((inside(r#"stroke-width="3""#) - 3.0).abs() < 1e-12);
    assert!((inside(r#"style="stroke-width:3""#) - 3.0).abs() < 1e-12);
    // The x axis stretched 2 times and the y axis 4 times: 3 times on average.
    assert!((inside(r#"transform="scale(2, 4)""#) - 3.0).abs() < 1e-12);
    // Turning changes nothing, and a skew stretches the y axis to √2.
    assert!((inside(r#"transform="rotate(30) scale(2)""#) - 2.0).abs() < 1e-12);
    assert!((inside(r#"transform="skewX(45)""#) - (1.0 + 2.0_f64.sqrt()) / 2.0).abs() < 1e-12);
    // A stroke that a viewer leaves unscaled is scaled too, as Ink/Stitch scales it.
    let unscaled = format!(r#"<g transform="scale(2)"><path {LINE} stroke="black" fill="none" style="vector-effect:non-scaling-stroke"/></g>"#);
    assert!((width_px(&px_document(&unscaled)) - 2.0).abs() < 1e-12);
    // An inherited width is applied in the shape's own user space, scaled by the shape's transform.
    let w = width_px(&px_document(&format!(r#"<g stroke-width="2"><path {LINE} transform="scale(3)" stroke="black" fill="none"/></g>"#)));
    assert!((w - 6.0).abs() < 1e-12, "{w}");
    // A viewBox that makes a user unit a millimetre makes the width millimetres.
    let document = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 100 50"><path {LINE} stroke="black" stroke-width="0.5" fill="none"/></svg>"#
    );
    let [(mm, _)] = strokes(&document)[..] else { panic!() };
    assert!((mm - 0.5).abs() < 1e-12, "{mm}");
}

#[test]
fn req_svg_004_a_width_that_does_not_read_is_left_for_the_inherited_one() {
    for bad in ["thick", "-2"] {
        let document = px_document(&format!(r#"<g stroke-width="3"><path id="p" {LINE} stroke="black" stroke-width="{bad}" fill="none"/></g>"#));
        assert!((width_px(&document) - 3.0).abs() < 1e-12, "{bad}");
        let message =
            format!("warning SC-W0804: The stroke-width of `p`, \"{bad}\", is not a length of 0 or more; it is ignored, as a viewer ignores it.");
        assert_eq!(warnings(&svg(&document)), [message]);
    }
}

#[test]
fn req_svg_004_joins_as_ink_stitch_reads_them() {
    let join = |style: &str| strokes(&px_document(&format!(r#"<path {LINE} stroke="black" fill="none" style="{style}"/>"#)))[0].1;
    assert_eq!(join(""), Join::Miter { limit: 5.0 }, "unset: a miter limited at 5");
    assert_eq!(join("stroke-linejoin:miter"), Join::Miter { limit: 4.0 }, "SVG's limit, 4");
    assert_eq!(join("stroke-linejoin:miter;stroke-miterlimit:10"), Join::Miter { limit: 10.0 });
    assert_eq!(join("stroke-miterlimit:10"), Join::Miter { limit: 5.0 }, "a limit without the join changes nothing");
    assert_eq!(join("stroke-linejoin:round"), Join::Round);
    assert_eq!(join("stroke-linejoin:bevel"), Join::Bevel);
    for other in ["miter-clip", "arcs"] {
        assert_eq!(join(&format!("stroke-linejoin:{other}")), Join::Miter { limit: 5.0 }, "{other}");
    }
    // Keywords in any case, as a viewer reads them (`DEV-SVG-001`).
    assert_eq!(join("stroke-linejoin:ROUND"), Join::Round);
    // Both are inherited.
    let document = px_document(&format!(r#"<g stroke-linejoin="miter" stroke-miterlimit="7"><path {LINE} stroke="black" fill="none"/></g>"#));
    assert_eq!(strokes(&document)[0].1, Join::Miter { limit: 7.0 });
    let document = px_document(&format!(r#"<g style="stroke-linejoin:round"><path {LINE} stroke="black" fill="none"/></g>"#));
    assert_eq!(strokes(&document)[0].1, Join::Round);
}

#[test]
fn req_svg_004_a_limit_that_does_not_read_is_left_for_the_inherited_one() {
    // SVG's limit is a number of at least 1.
    for bad in ["sharp", "0.5"] {
        let document = px_document(&format!(
            r#"<g stroke-miterlimit="6"><path id="p" {LINE} stroke="black" fill="none" stroke-linejoin="miter" stroke-miterlimit="{bad}"/></g>"#
        ));
        assert_eq!(strokes(&document)[0].1, Join::Miter { limit: 6.0 }, "{bad}");
        let message = format!(
            "warning SC-W0804: The stroke-miterlimit of `p`, \"{bad}\", is not a number of 1 or more; it is ignored, as a viewer ignores it."
        );
        assert_eq!(warnings(&svg(&document)), [message]);
    }
}

#[test]
fn req_svg_004_a_join_that_does_not_read_is_left_for_the_inherited_one() {
    let document =
        px_document(&format!(r#"<g stroke-linejoin="round"><path id="p" {LINE} stroke="black" fill="none" stroke-linejoin="pointy"/></g>"#));
    assert_eq!(strokes(&document)[0].1, Join::Round);
    let message = "warning SC-W0804: The stroke-linejoin of `p`, \"pointy\", is not a join SVG names; it is ignored, as a viewer ignores it.";
    assert_eq!(warnings(&svg(&document)), [message]);
    // What a hidden element declares goes unsaid, as its paints do.
    let hidden = px_document(&format!(r#"<path {LINE} stroke="black" stroke-width="thick" stroke-linejoin="pointy" visibility="hidden"/>"#));
    assert_eq!(warnings(&svg(&hidden)), [] as [String; 0]);
}

#[test]
fn req_svg_004_a_stroke_too_wide_to_measure_is_left_out() {
    // 1e308 inches is a number; in millimetres it is not. The fill is still sewn.
    let document = px_document(r#"<path id="p" d="M 10 10 L 90 10 L 90 50 Z" stroke="black" stroke-width="1e308in" fill="red"/>"#);
    let svg = svg(&document);
    assert!(matches!(svg.design.elements(), [only] if matches!(only.shape, Shape::Fill { .. })));
    assert_eq!(warnings(&svg), ["warning SC-W0804: The stroke of `p` cannot be stitched: it is too wide to measure. It is left out."]);
}
