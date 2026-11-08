//! `REQ-RIP-012` to `REQ-RIP-014` and `REQ-SVG-003`: Ink/Stitch's guide lines and anchor lines, read for the
//! shapes grouped with them, as Ink/Stitch at `d59c9ab` finds them (`src/document/guides.rs`).

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

mod common;

use common::{ids, svg, warnings};
use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_engine::design::{Guide, Join, Path, Segment, Subpath};
use stitchcraft_svg::{Svg, read};

/// A point in millimetres.
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A drawing in millimetres with the namespaces Ink/Stitch's files declare, around `body`.
fn drawing(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
             xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">
             <metadata><inkstitch:inkstitch_svg_version>4</inkstitch:inkstitch_svg_version></metadata>{body}</svg>"#
    )
}

/// A ripple stitch `id` from (0, 0) to (4, 0).
fn ripple(id: &str) -> String {
    format!(r#"<path id="{id}" d="M 0 0 L 4 0" style="fill:none;stroke:red" inkstitch:stroke_method="ripple_stitch"/>"#)
}

/// A helper `id`, Ink/Stitch's `marker` (`guide-line` or `anchor-line`), drawn as `d`, with more `style` and
/// attributes `more`.
fn helper(id: &str, marker: &str, d: &str, style: &str, more: &str) -> String {
    format!(r#"<path id="{id}" d="{d}" style="fill:none;stroke:#000;{style}marker-start:url(#inkstitch-{marker}-marker)" {more}/>"#)
}

/// The polyline through `points`, open.
fn line(points: &[(f64, f64)]) -> Subpath {
    Subpath { start: p(points[0].0, points[0].1), segments: points[1..].iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: false }
}

/// An element's id, its guide and its anchor line.
type Steered<'s> = (&'s str, Option<Guide>, Option<[Point; 2]>);

/// Each element's id, its guide and its anchor line.
fn steered(svg: &Svg) -> Vec<Steered<'_>> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.hints.guide.clone(), e.hints.anchor)).collect()
}

#[test]
fn req_rip_012_a_guide_line_steers_the_shapes_of_its_group() {
    let file = svg(drawing(&format!(
        r#"<g transform="translate(10 0)">{}{}</g>{}"#,
        ripple("ripple"),
        helper("guide", "guide-line", "M 0 0 L 0 20", "stroke-width:0.5;stroke-linejoin:round;", r#"inkstitch:stroke_method="manual_stitch""#),
        ripple("alone"),
    )));
    // The guide line is not stitched, and needs no word: it steers the ripple of its group, under the group's
    // transform, and not the one outside it.
    assert_eq!(ids(&file), ["svg:ripple:stroke", "svg:alone:stroke"]);
    assert_eq!(warnings(&file), Vec::<String>::new());
    let guide = Guide {
        path: Path { subpaths: vec![line(&[(10.0, 0.0), (10.0, 20.0)])] },
        width: Mm::new(0.5).unwrap(),
        join: Join::Round,
        params: [("stroke_method", "manual_stitch")].into_iter().collect(),
    };
    assert_eq!(steered(&file), [("svg:ripple:stroke", Some(guide), None), ("svg:alone:stroke", None, None)]);
}

#[test]
fn req_rip_013_a_satin_guide_comes_before_a_guide_line_and_an_anchor_line_is_never_a_satin() {
    let file = svg(drawing(&format!(
        "<g>{}{}{}{}{}{}</g>",
        helper("line", "guide-line", "M 0 0 L 0 20", "", ""),
        helper("satin", "guide-line", "M 0 0 L 20 0 M 0 4 L 20 4", "", r#"inkstitch:satin_column="True""#),
        helper("satin-anchor", "anchor-line", "M 9 9 L 9 1", "", r#"inkstitch:satin_column="True""#),
        helper("anchor", "anchor-line", "M 1 -1 L 1 1 M 5 5 L 6 6", "", ""),
        helper("anchor-2", "anchor-line", "M 2 -1 L 2 1", "", ""),
        ripple("ripple"),
    )));
    let [(_, Some(guide), anchor)] = &steered(&file)[..] else { panic!("{:?}", steered(&file)) };
    // The satin guide, its rails, and the first anchor line that isn't a satin column: its first subpath's
    // ends.
    assert_eq!(guide.path, Path { subpaths: vec![line(&[(0.0, 0.0), (20.0, 0.0)]), line(&[(0.0, 4.0), (20.0, 4.0)])] });
    assert_eq!(guide.params.get("satin_column"), Some("True"));
    assert_eq!(*anchor, Some([p(1.0, -1.0), p(1.0, 1.0)]));
    // Without a satin guide, the first guide line.
    let file = svg(drawing(&format!(
        "<g>{}{}{}</g>",
        helper("first", "guide-line", "M 0 0 L 0 20", "", ""),
        helper("second", "guide-line", "M 0 0 L 20 0", "", ""),
        ripple("ripple")
    )));
    assert_eq!(steered(&file)[0].1.as_ref().map(|guide| &guide.path), Some(&Path { subpaths: vec![line(&[(0.0, 0.0), (0.0, 20.0)])] }));
}

#[test]
fn req_rip_012_a_hidden_helper_steers_and_one_without_a_stroke_or_a_usable_path_does_not() {
    // Hidden with display: none, under its own transform, its stroke as wide as the transform makes it.
    let file = svg(drawing(&format!(
        "<g>{}{}{}{}</g>",
        helper("unstroked", "guide-line", "M 0 0 L 0 5", "stroke:none;", ""),
        helper("flat", "guide-line", "M 0 0 L 0 5", "", r#"transform="scale(0)""#),
        helper("hidden", "guide-line", "M 0 0 L 0 5", "display:none;", r#"transform="scale(2)""#),
        ripple("ripple"),
    )));
    let [(_, Some(guide), None)] = &steered(&file)[..] else { panic!("{:?}", steered(&file)) };
    assert_eq!((&guide.path, guide.width), (&Path { subpaths: vec![line(&[(0.0, 0.0), (0.0, 10.0)])] }, Mm::new(2.0).unwrap()));
    // An anchor line that closes ends where it starts.
    let file = svg(drawing(&format!("<g>{}{}</g>", helper("ring", "anchor-line", "M 1 1 L 3 1 L 3 3 Z", "", ""), ripple("ripple"))));
    assert_eq!(steered(&file)[0].2, Some([p(1.0, 1.0), p(1.0, 1.0)]));
    // A helper with no path, or no point, steers nothing.
    let file = svg(drawing(&format!("<g>{}{}</g>", helper("empty", "guide-line", "", "", ""), ripple("ripple"))));
    assert_eq!(steered(&file)[0].1, None);
}

#[test]
fn req_rip_012_helpers_steer_only_the_shapes_beside_them_in_their_group() {
    // A guide line in an outer group doesn't steer a shape in a group inside it, and 2 shapes beside one
    // share it.
    let file = svg(drawing(&format!(
        "<g>{}<g>{}</g></g><g>{}{}{}</g>",
        helper("outer", "guide-line", "M 0 0 L 0 20", "", ""),
        ripple("inner"),
        ripple("a"),
        helper("shared", "guide-line", "M 0 0 L 20 0", "", ""),
        ripple("b"),
    )));
    let guided: Vec<(&str, bool)> = steered(&file).into_iter().map(|(id, guide, _)| (id, guide.is_some())).collect();
    assert_eq!(guided, [("svg:inner:stroke", false), ("svg:a:stroke", true), ("svg:b:stroke", true)]);
}

#[test]
fn req_rip_012_a_group_is_looked_through_for_helpers_once() {
    // Its children are looked at once for guide and anchor lines, at a unit of work each, whatever the number
    // of its shapes: 40 shapes in a group cost little more than the same shapes alone, and not 40 looks of 40.
    let cost = |text: &str| -> u64 {
        let fits = |work: u64| read(text.as_bytes(), &Budget { max_stitches: 1, max_work: work }).is_ok();
        let (mut low, mut high) = (0, 1 << 20);
        assert!(fits(high));
        while high - low > 1 {
            let middle = (low + high) / 2;
            if fits(middle) { high = middle } else { low = middle }
        }
        high
    };
    let shapes: String = (0..40).map(|k| format!(r#"<rect x="{k}" width="0.5" height="0.5"/>"#)).collect();
    let (alone, grouped) = (cost(&drawing(&shapes)), cost(&drawing(&format!("<g>{shapes}</g>"))));
    assert!(grouped > alone && grouped <= alone + 2 * 40, "{alone} {grouped}");
}
