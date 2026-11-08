//! `REQ-SVG-014`: a linear gradient fill's gradient is read as a viewer places it, for the fill to sew its
//! colours (`REQ-FILL-GRD-001`). These files are drawn 1 user unit to the millimetre, so a gradient's
//! coordinates are its millimetres.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::{at, svg, warnings};
use stitchcraft_engine::design::{Gradient, Shape};
use stitchcraft_plan::Rgb;
use stitchcraft_svg::Svg;

const RED: Rgb = Rgb::new(255, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);

/// A file of `defs` and `body`, 100 mm square, drawn in millimetres, saved by Ink/Stitch.
fn file(defs: &str, body: &str) -> String {
    let head = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;
    let version = "<metadata><inkstitch:inkstitch_svg_version>4</inkstitch:inkstitch_svg_version></metadata>";
    format!("{head}{version}<defs>{defs}</defs>{body}</svg>")
}

/// A rectangle `id`, a linear gradient fill painted with the paint server `server`.
fn graded(id: &str, server: &str, rect: &str) -> String {
    format!(r#"<rect id="{id}" {rect} fill="url(#{server})" inkstitch:fill_method="linear_gradient_fill"/>"#)
}

/// The gradient of the fill of the element made of `id`.
fn gradient_of(svg: &Svg, id: &str) -> Option<Gradient> {
    let element = svg.design.elements().iter().find(|e| e.id.as_str() == format!("svg:{id}:fill")).unwrap();
    match &element.shape {
        Shape::Fill { gradient, .. } => gradient.clone(),
        Shape::Stroke { .. } => None,
    }
}

/// Each stop's offset, colour and opacity.
fn stops(gradient: &Gradient) -> Vec<(f64, Rgb, f64)> {
    gradient.stops.iter().map(|stop| (stop.offset, stop.colour, stop.opacity)).collect()
}

/// Red to blue, as `id`, with `attributes`.
fn red_blue(id: &str, attributes: &str) -> String {
    format!(r#"<linearGradient id="{id}" {attributes}><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>"#)
}

#[test]
fn req_svg_014_a_linear_gradient_fill_reads_its_gradient_where_the_drawing_shows_it() {
    // In user units, as Inkscape writes it: from (0, 0) to (20, 0), in the fill's thread red, the first
    // stop's colour, and no warning.
    let defs = red_blue("user", r#"gradientUnits="userSpaceOnUse" x1="0" y1="0" x2="20" y2="0""#);
    let read = svg(file(&defs, &graded("a", "user", r#"width="20" height="20""#)));
    assert!(warnings(&read).is_empty(), "{:?}", warnings(&read));
    let gradient = gradient_of(&read, "a").unwrap();
    at(gradient.start, 0.0, 0.0);
    at(gradient.end, 20.0, 0.0);
    assert_eq!(stops(&gradient), [(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    assert_eq!(read.design.elements()[0].thread.color, RED);
    // The element's transform and the gradient's own move it with the drawing: turned a quarter, then
    // moved 5 mm right and 10 mm down.
    let defs = red_blue("turned", r#"gradientUnits="userSpaceOnUse" x2="20" gradientTransform="rotate(90)""#);
    let body = format!(r#"<g transform="translate(5 10)">{}</g>"#, graded("b", "turned", r#"width="20" height="20""#));
    let gradient = gradient_of(&svg(file(&defs, &body)), "b").unwrap();
    at(gradient.start, 5.0, 10.0);
    at(gradient.end, 5.0, 30.0);
}

#[test]
fn req_svg_014_without_units_a_gradient_lies_in_shares_of_the_shape_s_box() {
    // SVG's default: the box of the rectangle from (10, 20), 40 wide and 10 high. Left out, the vector runs
    // along the box's top from its left side to its right; a percentage is a share too.
    let defs = format!(
        "{}{}{}",
        red_blue("plain", ""),
        red_blue("shares", r#"x1="0.5" y1="0" x2="50%" y2="100%""#),
        red_blue("turned", r#"gradientTransform="rotate(90)""#)
    );
    let rect = r#"x="10" y="20" width="40" height="10""#;
    let body = [graded("plain", "plain", rect), graded("shares", "shares", rect), graded("turned", "turned", rect)].concat();
    let read = svg(file(&defs, &body));
    let plain = gradient_of(&read, "plain").unwrap();
    at(plain.start, 10.0, 20.0);
    at(plain.end, 50.0, 20.0);
    let shares = gradient_of(&read, "shares").unwrap();
    at(shares.start, 30.0, 20.0);
    at(shares.end, 30.0, 30.0);
    // Its own transform turns it in the box's shares: down the box's left side.
    let turned = gradient_of(&read, "turned").unwrap();
    at(turned.start, 10.0, 20.0);
    at(turned.end, 10.0, 30.0);
    // A box with no height paints nothing in a viewer: no gradient to sew.
    let flat =
        svg(file(&red_blue("flat", ""), r#"<path id="flat" d="M 0 5 H 20 H 0 Z" fill="url(#flat)" inkstitch:fill_method="linear_gradient_fill"/>"#));
    assert_eq!(gradient_of(&flat, "flat"), None);
}

#[test]
fn req_svg_014_a_gradient_takes_what_it_doesn_t_say_from_the_gradient_it_names() {
    // Its stops from the first gradient that has any, its vector from the nearest linear gradient that says
    // it, and its units and transform from the nearest that says them, a radial gradient's included.
    let defs = r##"<linearGradient id="colours"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
        <radialGradient id="units" xlink:href="#colours" gradientUnits="userSpaceOnUse" gradientTransform="translate(0 5)" cx="7"/>
        <linearGradient id="vector" xlink:href="#units" x2="30"/>
        <linearGradient id="named" href="#vector"/>"##;
    let read = svg(file(defs, &graded("a", "named", r#"width="40" height="40""#)));
    let gradient = gradient_of(&read, "a").unwrap();
    assert_eq!(stops(&gradient), [(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    at(gradient.start, 0.0, 5.0);
    at(gradient.end, 30.0, 5.0);
    // A loop of names ends where it comes back.
    let defs = r##"<linearGradient id="loop-a" xlink:href="#loop-b" gradientUnits="userSpaceOnUse" x2="10"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
        <linearGradient id="loop-b" xlink:href="#loop-a"/>"##;
    let gradient = gradient_of(&svg(file(defs, &graded("b", "loop-b", r#"width="10" height="10""#))), "b").unwrap();
    at(gradient.end, 10.0, 0.0);
}

#[test]
fn req_svg_014_each_stop_is_read_as_a_viewer_reads_it() {
    // Offsets as numbers or percentages, kept from 0 to 1 and never before the stop before; opacity the
    // stop's times its colour's alpha; currentColor the nearest `color`; a colour that doesn't read black.
    let defs = r##"<linearGradient id="g" gradientUnits="userSpaceOnUse" x2="10" color="#00ff00">
        <stop offset="-1" stop-color="red" stop-opacity="0.5"/>
        <stop offset="60%" style="stop-color: rgba(0, 0, 255, 0.5); stop-opacity: 50%"/>
        <stop offset="0.4" stop-color="currentColor"/>
        <stop offset="2" stop-color="no-colour" stop-opacity="0"/>
        <text>not a stop</text>
      </linearGradient>"##;
    let gradient = gradient_of(&svg(file(defs, &graded("a", "g", r#"width="10" height="10""#))), "a").unwrap();
    let read = stops(&gradient);
    assert_eq!(read.iter().map(|s| (s.0, s.1)).collect::<Vec<_>>(), [(0.0, RED), (0.6, BLUE), (0.6, Rgb::new(0, 255, 0)), (1.0, Rgb::new(0, 0, 0))]);
    let opacities: Vec<f64> = read.iter().map(|s| s.2).collect();
    assert!((opacities[0] - 0.5).abs() < 1e-12 && (opacities[1] - 0.5 * 128.0 / 255.0).abs() < 1e-3, "{opacities:?}");
    assert_eq!(&opacities[2..], [1.0, 0.0]);
    // A gradient of one stop is no gradient: the fill sews its colour, as its rows fade.
    let one = r#"<linearGradient id="one" gradientUnits="userSpaceOnUse" x2="10"><stop offset="0" stop-color="red"/></linearGradient>"#;
    let read = svg(file(one, &graded("b", "one", r#"width="10" height="10""#)));
    assert_eq!((gradient_of(&read, "b"), read.design.elements()[0].thread.color), (None, RED));
}

#[test]
fn req_svg_014_only_a_linear_gradient_fill_sews_its_gradient_and_others_are_said() {
    // A tatami fill sews the first colour, which it says; a radial gradient is no linear gradient.
    let defs = format!(
        "{}{}",
        red_blue("linear", ""),
        r#"<radialGradient id="radial"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></radialGradient>"#
    );
    let body = format!(
        r#"<rect id="tatami" width="10" height="10" fill="url(#linear)"/>{}"#,
        graded("radial", "radial", r#"x="20" width="10" height="10""#)
    );
    let read = svg(file(&defs, &body));
    assert_eq!(
        warnings(&read),
        [
            "warning SC-W0802: The fill of `tatami` is a gradient; it is stitched in the gradient's first colour, #ff0000.",
            "warning SC-W0802: The fill of `radial` is a gradient; it is stitched in the gradient's first colour, #ff0000."
        ]
    );
    assert_eq!((gradient_of(&read, "tatami"), gradient_of(&read, "radial")), (None, None));
}

#[test]
fn req_svg_014_a_part_of_a_gradient_that_can_t_be_used_is_said_and_left_as_a_viewer_leaves_it() {
    let defs = format!(
        "{}{}{}",
        red_blue("odd-length", r#"gradientUnits="userSpaceOnUse" x2="ten""#),
        red_blue("odd-transform", r#"gradientUnits="userSpaceOnUse" x2="10" gradientTransform="rotate(""#),
        red_blue("far", r#"gradientUnits="userSpaceOnUse" x2="20000000""#)
    );
    let rect = r#"width="10" height="10""#;
    let body = [graded("a", "odd-length", rect), graded("b", "odd-transform", rect), graded("c", "far", rect)].concat();
    let read = svg(file(&defs, &body));
    let said = warnings(&read);
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(said[0].starts_with("warning SC-W0804") && said[0].contains("x2 of the gradient `odd-length`") && said[0].contains("100%"), "{said:?}");
    assert!(said[1].starts_with("warning SC-W0804") && said[1].contains("transform of the gradient `odd-transform`"), "{said:?}");
    assert!(said[2].starts_with("warning SC-W0804") && said[2].contains("more than 10 m"), "{said:?}");
    // The length left as its default: across the user space to 100 %, its width; the transform left out.
    at(gradient_of(&read, "a").unwrap().end, 100.0, 0.0);
    at(gradient_of(&read, "b").unwrap().end, 10.0, 0.0);
    assert_eq!(gradient_of(&read, "c"), None);
}
