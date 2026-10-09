//! `REQ-SVG-001`: what an SVG file draws becomes the design exactly. Every element a viewer shows is
//! there, in paint order and in its colours, at its position to within a micrometre; what a viewer hides
//! is not.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

mod common;

use common::{all_at, at, ends, fixture, ids, svg, warnings};
use stitchcraft_core::{Budget, Code, math};
use stitchcraft_engine::design::{FillRule, Segment, Shape};
use stitchcraft_plan::Rgb;

#[test]
fn req_svg_001_an_inkscape_document_in_millimetres() {
    let svg = svg(fixture("inkscape-mm.svg"));
    assert_eq!(ids(&svg), ["svg:rect1:fill", "svg:rect1:stroke", "svg:circle1:fill", "svg:path1:stroke", "svg:ellipse1:fill"]);
    let e = svg.design.elements();
    // The rectangle, moved by its layer's translate(10,5): a fill and a stroke along the same outline.
    assert_eq!(e[0].shape, Shape::Fill { path: e[1].shape.path().clone(), rule: FillRule::NonZero });
    assert!(matches!(e[1].shape, Shape::Stroke(_)));
    all_at(&ends(&e[0])[0], &[(10.0, 5.0), (40.0, 5.0), (40.0, 25.0), (10.0, 25.0)]);
    assert!(e[0].shape.path().subpaths[0].closed);
    assert_eq!((e[0].thread.color, e[1].thread.color), (Rgb::new(255, 0, 0), Rgb::new(0, 0, 255)));
    assert_eq!((e[0].name.as_deref(), e[2].name.as_deref()), (Some("Red box"), None));
    // The circle: centre (60, 15), radius 8, a quarter turn at a time from its right, every point on it.
    let circle = &ends(&e[2])[0];
    assert_eq!(circle.len(), 13);
    for (i, (x, y)) in [(68.0, 15.0), (60.0, 23.0), (52.0, 15.0), (60.0, 7.0), (68.0, 15.0)].into_iter().enumerate() {
        at(circle[3 * i], x, y);
    }
    for p in circle {
        assert!((math::hypot(p.x() - 60.0, p.y() - 15.0) - 8.0).abs() <= common::UM, "{p:?} is off the circle");
    }
    assert_eq!(e[2].thread.color, Rgb::new(0, 255, 0));
    // The path: the curve's control points exactly, the arc ending exactly where it says, closed.
    let path = &e[3].shape.path().subpaths[0];
    at(path.start, 10.0, 35.0);
    let Segment::Cubic(c1, c2, end) = path.segments[0] else { panic!("{:?}", path.segments[0]) };
    all_at(&[c1, c2, end], &[(20.0, 30.0), (30.0, 40.0), (40.0, 35.0)]);
    at(path.segments.last().unwrap().end(), 60.0, 45.0);
    assert!(path.closed);
    assert_eq!(e[3].thread.color, Rgb::new(0, 0, 0));
    // The gradient is stitched in its first colour, and says so; the hidden layer is not there.
    assert_eq!(e[4].thread.color, Rgb::new(0x00, 0xaa, 0x00));
    assert_eq!(warnings(&svg), ["warning SC-W0802: The fill of `ellipse1` is a gradient; it is stitched in the gradient's first colour, #00aa00."]);
}

#[test]
fn req_svg_001_pixels_nested_transforms_and_basic_shapes() {
    let svg = svg(fixture("px-document.svg"));
    // A line has no inside to fill, so only its stroke is there.
    assert_eq!(ids(&svg), ["svg:line:stroke", "svg:triangle:fill", "svg:zigzag:stroke"]);
    assert_eq!(warnings(&svg), Vec::<String>::new());
    let e = svg.design.elements();
    // Without a viewBox a user unit is a CSS pixel, 1/96 inch. The inner transform applies first:
    // (0, 0) moves to (96, 0), and the quarter turn takes that to (0, 96) px, (0, 25.4) mm.
    all_at(&ends(&e[0])[0], &[(0.0, 25.4), (0.0, 50.8)]);
    assert_eq!(e[0].thread.color, Rgb::new(0x80, 0x00, 0x80));
    all_at(&ends(&e[1])[0], &[(0.0, 0.0), (25.4, 0.0), (25.4, 25.4)]);
    assert!(e[1].shape.path().subpaths[0].closed);
    assert_eq!(e[1].thread.color, Rgb::new(0xff, 0x88, 0x00));
    // `currentColor` is the element's own `color`.
    all_at(&ends(&e[2])[0], &[(0.0, 0.0), (12.7, 12.7), (25.4, 0.0)]);
    assert!(!e[2].shape.path().subpaths[0].closed);
    assert_eq!(e[2].thread.color, Rgb::new(0x00, 0x80, 0x80));
}

#[test]
fn req_svg_001_viewbox_scale_and_aspect_ratio() {
    let line = |attributes: &str| {
        let svg = svg(format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 200 100" {attributes}><path d="M 0 0 L 200 100" stroke="red" fill="none"/></svg>"#
        ));
        ends(&svg.design.elements()[0])[0].clone()
    };
    // A 200 × 100 viewBox in a 100 mm square: halved to fit, and centred vertically.
    all_at(&line(""), &[(0.0, 25.0), (100.0, 75.0)]);
    all_at(&line(r#"preserveAspectRatio="none""#), &[(0.0, 0.0), (100.0, 100.0)]);
    all_at(&line(r#"preserveAspectRatio="xMaxYMax slice""#), &[(-100.0, 0.0), (100.0, 100.0)]);
    // Units inside the drawing are CSS units of its user space: 1in is 96 user units, here 96 mm.
    let svg = svg(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><rect x="1in" width="10%" height="5" /></svg>"#,
    );
    all_at(&ends(&svg.design.elements()[0])[0], &[(96.0, 0.0), (106.0, 0.0), (106.0, 5.0), (96.0, 5.0)]);
}

#[test]
fn req_svg_001_colours_and_paint_order() {
    let svg = svg(r##"<svg xmlns="http://www.w3.org/2000/svg" color="#123456">
          <g fill="#f00" stroke="currentColor">
            <rect id="a" width="1" height="1"/>
            <rect id="b" width="1" height="1" fill="rgb(0, 128, 255)" stroke="hsl(120, 100%, 50%)" paint-order="stroke"/>
            <rect id="c" width="1" height="1" style="fill:darkorange;stroke:none" fill="blue"/>
          </g>
        </svg>"##);
    assert_eq!(ids(&svg), ["svg:a:fill", "svg:a:stroke", "svg:b:stroke", "svg:b:fill", "svg:c:fill"]);
    let colours: Vec<_> = svg.design.elements().iter().map(|e| e.thread.color.to_string()).collect();
    assert_eq!(colours, ["#ff0000", "#123456", "#00ff00", "#0080ff", "#ff8c00"]);
}

#[test]
fn req_svg_001_what_a_viewer_hides_is_left_out_quietly() {
    let svg = svg(r#"<svg xmlns="http://www.w3.org/2000/svg">
          <defs><rect id="in-defs" width="1" height="1"/></defs>
          <rect id="none" width="1" height="1" display="none"/>
          <g style="display:none"><rect id="in-hidden-group" width="1" height="1"/><text>hidden</text></g>
          <g visibility="hidden"><rect id="hidden" width="1" height="1"/><rect id="shown" width="1" height="1" visibility="visible"/></g>
          <g opacity="0"><rect id="transparent" width="1" height="1" visibility="visible"/></g>
          <rect id="clear-fill" width="1" height="1" fill-opacity="0" stroke="red"/>
          <rect id="no-paint" width="1" height="1" fill="none"/>
          <switch>
            <text requiredExtensions="http://example.org/extension">not chosen</text>
            <rect id="chosen" width="1" height="1"/>
            <rect id="not-chosen" width="1" height="1"/>
          </switch>
          <text id="hidden-text" visibility="hidden">invisible</text>
        </svg>"#);
    assert_eq!(ids(&svg), ["svg:shown:fill", "svg:clear-fill:stroke", "svg:chosen:fill"]);
    assert_eq!(warnings(&svg), Vec::<String>::new());
}

#[test]
fn req_svg_001_ids_name_the_svg_elements() {
    let svg = svg(r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape">
          <path d="M 0 0 L 1 1" stroke="red" fill="none"/>
          <path id="twin" d="M 0 0 L 1 1" stroke="red" fill="none" inkscape:label=" Left eye "/>
          <path d="M 0 0 L 1 1" stroke="red" fill="none"/>
          <path id="twin" d="M 0 0 L 1 1" stroke="red" fill="none"/>
          <path id="path@1" d="M 0 0 L 1 1" stroke="red" fill="none"/>
          <path id="LONG" d="M 0 0 L 1 1" stroke="red" fill="none"/>
        </svg>"#
        .replace("LONG", &"x".repeat(300)));
    // Without an id (or with one too long for an element id), the n-th element of its kind; a repeated
    // id gets a suffix.
    let expected = ["svg:path@1:stroke", "svg:twin:stroke", "svg:path@3:stroke", "svg:twin~2:stroke", "svg:path@1~2:stroke", "svg:path@6:stroke"];
    assert_eq!(ids(&svg), expected);
    let names: Vec<_> = svg.design.elements().iter().map(|e| e.name.as_deref()).collect();
    assert_eq!(names, [None, Some("Left eye"), None, None, None, None]);
}

#[test]
fn req_svg_001_paint_servers_become_colours() {
    let svg = svg(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">
          <defs>
            <linearGradient id="stops" color="#00ff00"><stop offset="0" style="stop-color:currentColor"/><stop offset="1" stop-color="red"/></linearGradient>
            <radialGradient id="via" href="#stops"/>
            <linearGradient id="to-pattern" xlink:href="#dots"/>
            <linearGradient id="empty"/>
            <linearGradient id="loop-a" xlink:href="#loop-b"/><linearGradient id="loop-b" xlink:href="#loop-a"/>
            <pattern id="dots" width="2" height="2"/>
            <linearGradient id="odd-stop"><stop offset="0" stop-color="not-a-colour"/></linearGradient>
            <linearGradient id="first"><stop offset="0" stop-color="#ff0000"/></linearGradient>
            <linearGradient id="first"><stop offset="0" stop-color="#0000ff"/></linearGradient>
          </defs>
          <rect id="a" width="1" height="1" fill="url(#via)"/>
          <rect id="b" width="1" height="1" fill="url(#to-pattern) blue"/>
          <rect id="c" width="1" height="1" fill="url(#empty)"/>
          <rect id="d" width="1" height="1" fill="url(#loop-a)"/>
          <rect id="e" width="1" height="1" fill="url(#odd-stop)"/>
          <rect id="f" width="1" height="1" fill="url(#missing) green"/>
          <rect id="g" width="1" height="1" fill="url(#missing)"/>
          <rect id="h" width="1" height="1" fill="url(#dots)"/>
          <rect id="i" width="1" height="1" fill="none" stroke="url(#stops)"/>
          <rect id="j" width="1" height="1" fill="url(#first)"/>
        </svg>"##);
    // A gradient's first colour (currentColor there is the gradient's own `color`); a gradient without
    // stops, an `href` to something else or a loop paints nothing, as in a viewer; so does a reference
    // to nothing, unless it has a fallback; the first element with an id is the one used.
    assert_eq!(ids(&svg), ["svg:a:fill", "svg:e:fill", "svg:f:fill", "svg:i:stroke", "svg:j:fill"]);
    let colours: Vec<_> = svg.design.elements().iter().map(|e| e.thread.color.to_string()).collect();
    assert_eq!(colours, ["#00ff00", "#000000", "#008000", "#00ff00", "#ff0000"]);
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: The fill of `a` is a gradient; it is stitched in the gradient's first colour, #00ff00.",
            "warning SC-W0802: The fill of `e` is a gradient; it is stitched in the gradient's first colour, #000000.",
            "warning SC-W0802: The fill of `h` is a `<pattern>`, which is not stitched; the fill is left out.",
            "warning SC-W0802: The stroke of `i` is a gradient; it is stitched in the gradient's first colour, #00ff00.",
            "warning SC-W0802: The fill of `j` is a gradient; it is stitched in the gradient's first colour, #ff0000.",
        ]
    );
}

#[test]
fn req_svg_001_basic_shape_attributes() {
    let svg = svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">
          <rect id="one-radius" x="10" y="10" width="20" height="10" rx="2"/>
          <ellipse id="one-ellipse-radius" cx="50" cy="50" ry="5"/>
          <circle id="relative" cx="50" cy="50" r="10%"/>
          <polyline id="odd" points="0,0 10,0 10" stroke="red" fill="none"/>
          <rect id="no-size" width="10"/>
          <circle id="no-radius" r="-1"/>
        </svg>"#);
    assert_eq!(ids(&svg), ["svg:one-radius:fill", "svg:one-ellipse-radius:fill", "svg:relative:fill", "svg:odd:stroke"]);
    let e = svg.design.elements();
    // One radius given: the other is the same, so the corners are round, not square.
    let rounded = &ends(&e[0])[0];
    assert_eq!(rounded.len(), 17);
    at(rounded[0], 12.0, 10.0);
    // An ellipse with one radius is a circle of that radius (SVG 2's `auto`).
    at(ends(&e[1])[0][0], 55.0, 50.0);
    // A percentage radius is of the viewport's diagonal over √2: 10 % of 100 here.
    at(ends(&e[2])[0][0], 60.0, 50.0);
    // A point list that runs out mid-point keeps the points before it.
    all_at(&ends(&e[3])[0], &[(0.0, 0.0), (10.0, 0.0)]);
    assert_eq!(svg.warnings.len(), 2, "no-size and no-radius draw nothing: {:?}", warnings(&svg));
}

#[test]
fn req_svg_001_percentages_without_a_viewbox() {
    // Of the root's size in pixels, when it has one…
    let sized = svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="200" height="100"><rect width="50%" height="50%"/></svg>"#);
    all_at(&ends(&sized.design.elements()[0])[0], &[(0.0, 0.0), (100.0 * MM, 0.0), (100.0 * MM, 50.0 * MM), (0.0, 50.0 * MM)]);
    // …else of a viewer's default size, 300 × 150 pixels.
    let relative = svg(r#"<svg xmlns="http://www.w3.org/2000/svg" width="50%"><rect width="50%" height="10%"/></svg>"#);
    all_at(&ends(&relative.design.elements()[0])[0], &[(0.0, 0.0), (150.0 * MM, 0.0), (150.0 * MM, 15.0 * MM), (0.0, 15.0 * MM)]);
}

#[test]
fn req_svg_001_radii_follow_the_specification() {
    let svg = svg(r#"<svg xmlns="http://www.w3.org/2000/svg">
          <rect id="square" width="10" height="10" rx="0" ry="2"/>
          <rect id="negative" width="10" height="10" rx="-2" ry="1"/>
          <ellipse id="zero" cx="5" cy="5" rx="0" ry="5"/>
          <ellipse id="flat" cx="5" cy="5" rx="5" ry="0"/>
          <circle id="dot" cx="5" cy="5" r="0"/>
        </svg>"#);
    // A radius of zero means square corners; a negative one is as if missing, so the other one is used.
    assert_eq!(ids(&svg), ["svg:square:fill", "svg:negative:fill"]);
    let segments = |i: usize| svg.design.elements()[i].shape.path().subpaths[0].segments.len();
    assert_eq!((segments(0), segments(1)), (3, 16));
    // An ellipse or circle with a zero radius draws nothing.
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0804: `zero` draws nothing: its size or its data is empty. It is left out.",
            "warning SC-W0804: `flat` draws nothing: its size or its data is empty. It is left out.",
            "warning SC-W0804: `dot` draws nothing: its size or its data is empty. It is left out.",
        ]
    );
}

#[test]
fn req_svg_001_only_svg_elements_draw_and_gradient_stops_are_stops() {
    let svg = svg(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:x="http://example.org/x">
          <x:rect width="1" height="1"/>
          <linearGradient id="described"><desc>first comes a description</desc><stop stop-color="blue"/></linearGradient>
          <pattern id="odd"><stop stop-color="lime"/></pattern>
          <linearGradient id="to-odd" href="#odd"/>
          <rect id="a" width="1" height="1" fill="url(#described)"/>
          <rect id="b" width="1" height="1" fill="url(#to-odd)"/>
        </svg>"##);
    // The rect in another namespace is not SVG's; a stop is found past other children, and only in
    // gradients.
    assert_eq!(ids(&svg), ["svg:a:fill"]);
    assert_eq!(svg.design.elements()[0].thread.color, Rgb::new(0, 0, 255));
}

/// Millimetres per CSS pixel.
const MM: f64 = 25.4 / 96.0;

#[test]
fn reading_is_bounded_by_the_budget() {
    let file = format!(r#"<svg xmlns="http://www.w3.org/2000/svg">{}</svg>"#, r#"<path d="M 0 0 L 1 1" stroke="red"/>"#.repeat(100));
    let d = stitchcraft_svg::read(file.as_bytes(), &Budget { max_stitches: 1, max_work: 50 }).unwrap_err();
    assert_eq!(d.code, Code::BudgetExhausted);
    assert_eq!(d.to_string(), "error SC-E0004: Reading the SVG file needed more than the work budget of 50 units.");
    assert!(stitchcraft_svg::read(file.as_bytes(), &Budget { max_stitches: 1, max_work: 500 }).is_ok());
    // Exactly: one unit per XML node, and one more per 8 bytes of a point list (here 15 bytes: 2).
    let exact = |file: &str, units: u64| {
        let budget = |max_work| Budget { max_stitches: 1, max_work };
        assert!(stitchcraft_svg::read(file.as_bytes(), &budget(units)).is_ok(), "{units} units are enough for {file}");
        assert!(stitchcraft_svg::read(file.as_bytes(), &budget(units - 1)).is_err(), "{} units are not enough for {file}", units - 1);
    };
    exact(r#"<svg xmlns="http://www.w3.org/2000/svg"><rect width="1" height="1"/><rect width="1" height="1"/></svg>"#, 2);
    exact(r#"<svg xmlns="http://www.w3.org/2000/svg"><polyline points="0,0 1,1 2,2 3,3" stroke="red" fill="none"/></svg>"#, 3);
}
