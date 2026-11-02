//! `REQ-SVG-011`: a shape is cut to its clip paths as a viewer clips it, each a clip of its design elements
//! (`REQ-GEN-005`, which `crates/stitchcraft-engine/tests/clips.rs` sews). These files are drawn 1 user unit
//! to the millimetre, so a clip's coordinates are its millimetres.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::{svg, warnings};
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::design::{Clip, Element, FillRule, Path, Segment};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, StitchKind};

/// A file of `body`, 100 mm square, drawn in millimetres.
fn file(body: &str) -> String {
    let head = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="100mm" height="100mm" viewBox="0 0 100 100">"#;
    format!("{head}{body}</svg>")
}

/// The point of `segment`, from `from`, at `t`.
fn on(from: Point, segment: Segment, t: f64) -> (f64, f64) {
    let u = 1.0 - t;
    let mix = |ps: &[(Point, f64)]| (ps.iter().map(|(p, w)| p.x() * w).sum(), ps.iter().map(|(p, w)| p.y() * w).sum());
    match segment {
        Segment::Line(end) => mix(&[(from, u), (end, t)]),
        Segment::Quad(c, end) => mix(&[(from, u * u), (c, 2.0 * u * t), (end, t * t)]),
        Segment::Cubic(c1, c2, end) => mix(&[(from, u * u * u), (c1, 3.0 * u * u * t), (c2, 3.0 * u * t * t), (end, t * t * t)]),
    }
}

/// The box round what `path` draws, curves sampled finely, to a hundredth of a millimetre: [x0, y0, x1, y1].
fn bounds(path: &Path) -> [f64; 4] {
    let mut points = Vec::new();
    for subpath in &path.subpaths {
        let mut from = subpath.start;
        points.push((from.x(), from.y()));
        for &segment in &subpath.segments {
            points.extend((1..=64).map(|i| on(from, segment, f64::from(i) / 64.0)));
            from = segment.end();
        }
    }
    let round = |v: f64| (v * 100.0).round() / 100.0 + 0.0;
    let (xs, ys) = (points.iter().map(|p| p.0), points.iter().map(|p| p.1));
    [xs.clone().fold(f64::MAX, f64::min), ys.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.fold(f64::MIN, f64::max)].map(round)
}

/// The box round each shape of each clip of `element`.
fn boxes(element: &Element) -> Vec<Vec<[f64; 4]>> {
    element.clips.iter().map(|clip| clip.shapes.iter().map(|shape| bounds(&shape.path)).collect()).collect()
}

/// The rule of each shape of each clip of `element`.
fn rules(element: &Element) -> Vec<Vec<FillRule>> {
    element.clips.iter().map(|clip| clip.shapes.iter().map(|shape| shape.rule).collect()).collect()
}

#[test]
fn req_svg_011_a_shape_is_cut_to_the_clip_path_it_names() {
    // As an attribute, in its style, quoted, and by a style sheet's rule; `none` names none.
    let read = svg(file(
        r#"<style>.c { clip-path: url('#c') }</style>
        <defs><clipPath id="c"><rect x="10" y="20" width="30" height="40"/></clipPath></defs>
        <rect id="a" width="100" height="100" clip-path="url(#c)"/>
        <rect id="s" width="100" height="100" style="clip-path:url(&quot;#c&quot;)"/>
        <rect id="k" class="c" width="100" height="100"/>
        <rect id="none" width="100" height="100" clip-path="none" stroke="red"/>"#,
    ));
    assert!(warnings(&read).is_empty(), "{:?}", warnings(&read));
    let elements = read.design.elements();
    for element in &elements[..3] {
        assert_eq!(boxes(element), [[[10.0, 20.0, 40.0, 60.0]]], "{}", element.id);
        assert_eq!(rules(element), [[FillRule::NonZero]]);
    }
    // Its fill and its stroke alike.
    assert!(elements[3..].iter().all(|element| element.clips.is_empty()) && elements.len() == 5);
}

#[test]
fn req_svg_011_the_clip_paths_of_the_groups_links_and_switches_round_a_shape_cut_it_too() {
    // Each in the user space of the element that names it, here moved 10 mm right by the group, and the
    // shape's own last: the switch's, the link's, the group's, then the shape's.
    let read = svg(file(
        r#"<defs><clipPath id="c"><rect width="50" height="50"/></clipPath><clipPath id="d"><rect x="20" y="20" width="50" height="50"/></clipPath></defs>
        <g clip-path="url(#c)" transform="translate(10 0)"><a clip-path="url(#d)"><switch clip-path="url(#c)">
          <rect id="r" width="100" height="100" clip-path="url(#d)"/>
        </switch></a></g>
        <rect id="free" width="100" height="100"/>"#,
    ));
    let elements = read.design.elements();
    let (c, d) = ([10.0, 0.0, 60.0, 50.0], [30.0, 20.0, 80.0, 70.0]);
    assert_eq!(boxes(&elements[0]), [[c], [d], [c], [d]]);
    assert!(elements[1].clips.is_empty(), "a shape after the group isn't in it");
}

#[test]
fn req_svg_011_a_clip_path_is_placed_in_the_user_space_of_the_element_it_cuts() {
    // The shape's own transform, then the clip path's, then each child's. The transform of a group the
    // clip path lies in doesn't move it.
    let read = svg(file(
        r#"<defs><g transform="translate(50 50)"><clipPath id="c" transform="translate(5 0)"><rect transform="translate(0 7)" width="10" height="10"/></clipPath></g></defs>
        <rect id="r" transform="translate(20 0)" width="100" height="100" clip-path="url(#c)"/>"#,
    ));
    assert_eq!(boxes(&read.design.elements()[0]), [[[25.0, 7.0, 35.0, 17.0]]]);
}

#[test]
fn req_svg_011_object_bounding_box_units_are_shares_of_the_shape_s_box() {
    // Half the rectangle's width and all of its height. A group's box isn't measured, so its clip path is
    // named and not applied.
    let read = svg(file(
        r#"<defs><clipPath id="c" clipPathUnits="objectBoundingBox"><rect width="0.5" height="1"/></clipPath></defs>
        <rect id="r" x="10" y="20" width="40" height="20" clip-path="url(#c)"/>
        <g id="g" clip-path="url(#c)"><rect width="10" height="10"/></g>"#,
    ));
    let elements = read.design.elements();
    assert_eq!(boxes(&elements[0]), [[[10.0, 20.0, 30.0, 40.0]]]);
    assert!(elements[1].clips.is_empty());
    assert_eq!(
        warnings(&read),
        ["warning SC-W0802: The clip path `c` of `g` is in units of the box round what `g` holds, which StitchCraft doesn't measure; it is not applied."]
    );
}

#[test]
fn req_svg_011_a_clip_path_is_the_area_of_its_children_as_a_viewer_takes_them_in() {
    // A path, a circle, a use of a rectangle at its x and y, and a group's polygon, as Inkscape draws it, each
    // by its clip-rule, which inherits from the clip path. Not a child that isn't displayed or visible, nor a
    // line, nor text, which is named; nor what a shape in a clip path is clipped to, which is named too.
    let read = svg(file(
        r##"<defs><rect id="used" width="5" height="5"/>
        <clipPath id="c" clip-rule="evenodd">
          <path d="M 0 0 H 10 V 10 H 0 Z" clip-rule="nonzero"/>
          <circle cx="50" cy="50" r="10" fill="none"/>
          <use xlink:href="#used" x="70" y="70"/>
          <g transform="translate(0 80)"><polygon points="0,0 10,0 10,10" clip-path="url(#c)"/></g>
          <rect display="none" width="99" height="99"/>
          <rect visibility="hidden" width="99" height="99"/>
          <line x1="0" y1="0" x2="99" y2="99"/>
          <text>A</text>
        </clipPath></defs>
        <rect id="r" width="100" height="100" clip-path="url(#c)"/>"##,
    ));
    let element = &read.design.elements()[0];
    assert_eq!(boxes(element), [[[0.0, 0.0, 10.0, 10.0], [40.0, 40.0, 60.0, 60.0], [70.0, 70.0, 75.0, 75.0], [0.0, 80.0, 10.0, 90.0]]]);
    assert_eq!(rules(element), [[FillRule::NonZero, FillRule::EvenOdd, FillRule::EvenOdd, FillRule::EvenOdd]]);
    assert_eq!(
        warnings(&read),
        [
            "warning SC-W0802: A shape in the clip path `c` of `r` is clipped itself, which is not applied: all of it clips.",
            "warning SC-W0802: The clip path `c` of `r` holds text, which StitchCraft doesn't read; it adds nothing to the clip.",
        ]
    );
}

#[test]
fn req_svg_011_a_clip_path_s_own_clip_path_cuts_it_in_turn_and_a_reference_to_nothing_clips_nothing() {
    // c's own clip path, d, is a clip of the shape too. A loop of clip paths is named and cut no further. A
    // reference to nothing clips nothing, as in a viewer, and so does one to an element that isn't a clip
    // path. A CSS shape is named.
    let read = svg(file(
        r#"<defs>
          <clipPath id="c" clip-path="url(#d)"><rect width="50" height="50"/></clipPath><clipPath id="d"><rect x="25" width="50" height="50"/></clipPath>
          <clipPath id="x" clip-path="url(#y)"><rect width="10" height="10"/></clipPath><clipPath id="y" clip-path="url(#x)"><rect width="20" height="20"/></clipPath>
          <rect id="plain" width="5" height="5"/>
        </defs>
        <rect id="r" width="100" height="100" clip-path="url(#c)"/>
        <rect id="loop" width="100" height="100" clip-path="url(#x)"/>
        <rect id="missing" width="100" height="100" clip-path="url(#nothing)"/>
        <rect id="rect" width="100" height="100" clip-path="url(#plain)"/>
        <rect id="css" width="100" height="100" clip-path="circle(50%)"/>"#,
    ));
    let elements = read.design.elements();
    assert_eq!(boxes(&elements[0]), [[[0.0, 0.0, 50.0, 50.0]], [[25.0, 0.0, 75.0, 50.0]]]);
    assert_eq!(boxes(&elements[1]), [[[0.0, 0.0, 10.0, 10.0]], [[0.0, 0.0, 20.0, 20.0]]]);
    assert!(elements[2..].iter().all(|element| element.clips.is_empty()));
    assert_eq!(
        warnings(&read),
        [
            "warning SC-W0802: The clip paths of `loop` refer to each other in a loop, at `x`; it is cut no further.",
            "warning SC-W0802: The clip path of `css`, \"circle(50%)\", is not a reference to a clip path; it is not applied.",
        ]
    );
}

#[test]
fn req_svg_011_an_empty_clip_path_leaves_nothing_and_the_root_s_clip_path_is_named() {
    // A viewer shows nothing of a shape whose clip path has no area: it sews nothing, and the engine says so.
    let read = svg(r#"<svg xmlns="http://www.w3.org/2000/svg" clip-path="url(#c)"><defs><clipPath id="c"/></defs><rect id="r" width="10" height="10" clip-path="url(#c)"/></svg>"#);
    assert_eq!(read.design.elements()[0].clips, [Clip::default()]);
    assert_eq!(warnings(&read), ["warning SC-W0802: The root `<svg>` element's clip path is not applied."]);
    let planned = stitchcraft_engine::plan(&read.design, REFERENCE, &Budget::DEFAULT);
    assert!(planned.diagnostics.iter().any(|d| d.to_string().contains("SC-W0013")), "{:?}", planned.diagnostics);
}

#[test]
fn req_svg_011_a_clipped_fill_sews_inside_its_clip_path() {
    // The square's right half, from y 5 to 15: what a viewer shows of it.
    let read = svg(file(
        r#"<defs><clipPath id="c"><rect x="0" y="5" width="30" height="10"/></clipPath></defs>
        <rect id="r" x="-10" y="0" width="20" height="20" fill="red" clip-path="url(#c)"/>"#,
    ));
    let planned = stitchcraft_engine::plan(&read.design, REFERENCE, &Budget::DEFAULT);
    let plan = planned.plan.unwrap();
    let sewn: Vec<Point> = plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock).map(|s| s.at).collect();
    // The plan is measured from the design's middle, the middle of its fill as drawn here: (0, 10).
    let (xs, ys) = (sewn.iter().map(|p| p.x()), sewn.iter().map(|p| p.y()));
    let [x0, x1] = [xs.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max)];
    let [y0, y1] = [ys.clone().fold(f64::MAX, f64::min), ys.fold(f64::MIN, f64::max)];
    assert!(x1 - x0 <= 10.1 && y1 - y0 <= 10.1, "{:?}", [x0, y0, x1, y1]);
}
