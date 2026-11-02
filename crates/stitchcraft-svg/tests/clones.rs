//! `REQ-SVG-012` and `REQ-SVG-013`: a clone (`<use>`) sews what it shows where it shows it, as Ink/Stitch sews
//! a clone, and turns the angles of the fills it shows with it. These files are drawn 1 user unit to the
//! millimetre.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

mod common;

use common::{all_at, ends, ids, svg, warnings};
use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::design::{Element, Hints, Reading};
use stitchcraft_svg::{Svg, read, read_with};

/// A drawing 100 mm square, in millimetres, with the namespaces Ink/Stitch's files declare and its current
/// file version, around `body`.
fn drawing(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:inkstitch="http://inkstitch.org/namespace"
             width="100mm" height="100mm" viewBox="0 0 100 100">
             <metadata><inkstitch:inkstitch_svg_version>4</inkstitch:inkstitch_svg_version></metadata>{body}</svg>"#
    )
}

/// The symbols of the commands the tests tie to shapes.
const SYMBOLS: &str = r#"<defs><symbol id="inkstitch_trim"><circle r="1"/></symbol><symbol id="inkstitch_stop"><circle r="1"/></symbol>
    <symbol id="inkstitch_starting_point"><circle r="1"/></symbol><symbol id="inkstitch_ending_point"><circle r="1"/></symbol>
    <symbol id="inkstitch_ignore_object"><circle r="1"/></symbol></defs>"#;

/// Ink/Stitch's command `name`, its symbol at (`x`, `y`), tied to `target` by a connector, both in a group as
/// its Attach Commands draws them; `id` names the symbol's `<use>`.
fn command(id: &str, name: &str, (x, y): (f64, f64), target: &str) -> String {
    format!(
        r##"<g><path d="M {x},{y} {x},{y}" style="fill:none;stroke:#000" inkscape:connection-start="#{id}" inkscape:connection-end="#{target}"/><use id="{id}" xlink:href="#inkstitch_{name}" x="{x}" y="{y}"/></g>"##
    )
}

/// The element with the id `id`.
fn element<'s>(svg: &'s Svg, id: &str) -> &'s Element {
    svg.design.elements().iter().find(|e| e.id.as_str() == id).unwrap_or_else(|| panic!("no {id} in {:?}", ids(svg)))
}

/// Each element's id and the value it gives `key`.
fn setting<'s>(svg: &'s Svg, key: &str) -> Vec<(&'s str, Option<&'s str>)> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.params.get(key))).collect()
}

/// A point in millimetres.
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// `hints`, each point to the micrometre, as (x, y) pairs.
fn placed(hints: Hints) -> [Option<(f64, f64)>; 3] {
    [hints.start, hints.end, hints.target].map(|at| at.map(|q| ((q.x() * 1000.0).round() / 1000.0, (q.y() * 1000.0).round() / 1000.0)))
}

#[test]
fn req_svg_012_a_clone_sews_what_it_shows_in_its_place_in_the_document_s_order() {
    let file = svg(drawing(
        r##"<rect id="square" x="10" y="10" width="20" height="10" fill="red" inkstitch:row_spacing_mm="0.3"/>
            <circle id="dot" cx="80" cy="80" r="2" fill="blue"/>
            <use id="clone1" xlink:href="#square" x="5" y="40" transform="translate(50 0)"/>
            <path id="last" d="M 0 90 L 10 90" stroke="green" fill="none"/>"##,
    ));
    assert_eq!(ids(&file), ["svg:square:fill", "svg:dot:fill", "svg:clone1/square:fill", "svg:last:stroke"]);
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
    // Under the use's transform, after its x and y.
    let clone = element(&file, "svg:clone1/square:fill");
    all_at(&ends(clone)[0], &[(65.0, 50.0), (85.0, 50.0), (85.0, 60.0), (65.0, 60.0)]);
    // With the source's colour and settings, its angle turned with it, here by nothing.
    assert_eq!(clone.thread.color.to_string(), "#ff0000");
    assert_eq!(clone.params.iter().collect::<Vec<_>>(), [("angle", "0"), ("row_spacing_mm", "0.3")]);
    assert_eq!(element(&file, "svg:square:fill").params.get("angle"), None, "the source keeps its own");
}

#[test]
fn req_svg_012_a_clone_of_a_group_sews_each_shape_it_holds_in_order_under_every_transform() {
    let file = svg(drawing(
        r##"<defs><g id="pair" transform="translate(0 5)"><rect id="a" width="10" height="10" fill="red"/>
              <g transform="scale(2)"><rect id="b" x="10" width="5" height="5" fill="blue"/></g></g></defs>
            <use id="c" xlink:href="#pair" transform="translate(30 20)"/>"##,
    ));
    // What `<defs>` holds is drawn only where a clone shows it.
    assert_eq!(ids(&file), ["svg:c/a:fill", "svg:c/b:fill"]);
    all_at(&ends(element(&file, "svg:c/a:fill"))[0], &[(30.0, 25.0), (40.0, 25.0), (40.0, 35.0), (30.0, 35.0)]);
    all_at(&ends(element(&file, "svg:c/b:fill"))[0], &[(50.0, 25.0), (60.0, 25.0), (60.0, 35.0), (50.0, 35.0)]);
}

#[test]
fn req_svg_012_a_clone_s_content_inherits_from_it_as_in_a_viewer() {
    let file = svg(drawing(
        r##"<style>#styled { fill: #00ff00 } .classy { fill: #0000ff }</style>
            <defs><rect id="bare" width="10" height="10"/><rect id="own" width="10" height="10" fill="#ff0000"/>
              <rect id="styled" width="10" height="10"/><rect id="classed" class="classy" width="10" height="10"/>
              <rect id="seen" width="10" height="10" visibility="visible"/></defs>
            <use id="u1" xlink:href="#bare" x="10" fill="#ff00ff"/>
            <use id="u2" xlink:href="#own" x="30" style="fill:#ff00ff"/>
            <use id="u3" xlink:href="#styled" x="50" fill="#ff00ff"/>
            <use id="u4" xlink:href="#classed" x="70" style="fill:#ff00ff"/>
            <g fill="#ffff00"><use id="u5" xlink:href="#bare" x="10" y="30"/></g>
            <use id="u6" xlink:href="#bare" y="50" style="display:none"/>
            <use id="u7" xlink:href="#bare" y="50" opacity="0"/>
            <use id="u8" xlink:href="#bare" y="70" visibility="hidden"/>
            <use id="u9" xlink:href="#seen" y="70" visibility="hidden"/>"##,
    ));
    // The use's presentation attribute reaches what it shows, which the source's own declarations, a style
    // sheet's rules among them, override; a hidden clone shows what makes itself visible.
    let colours: Vec<(&str, String)> = file.design.elements().iter().map(|e| (e.id.as_str(), e.thread.color.to_string())).collect();
    let expected = [
        ("svg:u1/bare:fill", "#ff00ff"),
        ("svg:u2/own:fill", "#ff0000"),
        ("svg:u3/styled:fill", "#00ff00"),
        ("svg:u4/classed:fill", "#0000ff"),
        ("svg:u5/bare:fill", "#ffff00"),
        ("svg:u9/seen:fill", "#000000"),
    ];
    assert_eq!(colours, expected.map(|(id, colour)| (id, colour.to_string())));
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
}

#[test]
fn req_svg_012_a_clone_of_a_clone_draws_both_maps() {
    let file = svg(drawing(
        r##"<rect id="square" width="10" height="10" fill="red"/>
            <use id="inner" xlink:href="#square" x="20"/>
            <use id="outer" xlink:href="#inner" y="30" transform="scale(2)"/>"##,
    ));
    assert_eq!(ids(&file), ["svg:square:fill", "svg:inner/square:fill", "svg:outer/inner/square:fill"]);
    all_at(&ends(element(&file, "svg:inner/square:fill"))[0], &[(20.0, 0.0), (30.0, 0.0), (30.0, 10.0), (20.0, 10.0)]);
    all_at(&ends(element(&file, "svg:outer/inner/square:fill"))[0], &[(40.0, 60.0), (60.0, 60.0), (60.0, 80.0), (40.0, 80.0)]);
}

#[test]
fn req_svg_012_a_symbol_is_fitted_to_the_viewport_its_clone_gives_it() {
    let file = svg(drawing(
        r##"<defs><symbol id="sym" viewBox="0 0 10 10"><rect width="10" height="10" fill="red"/></symbol>
              <symbol id="wide" viewBox="0 0 10 5" preserveAspectRatio="xMinYMid meet"><rect width="10" height="5" fill="blue"/></symbol>
              <symbol id="sized" viewBox="0 0 10 10" width="20" height="20"><rect width="10" height="10" fill="green"/></symbol>
              <symbol id="plain"><rect x="-2" width="10" height="10"/></symbol></defs>
            <use id="u1" xlink:href="#sym" x="10" y="10" width="40" height="40"/>
            <use id="u2" xlink:href="#wide" x="60" y="10" width="20" height="20"/>
            <use id="u3" xlink:href="#sized" x="10" y="60"/>
            <use id="u4" xlink:href="#plain" x="60" y="60" width="5" height="5"/>
            <use id="u5" xlink:href="#sym"/>"##,
    ));
    let corners = |id: &str| ends(element(&file, id))[0].clone();
    // Fitted to the use's size; to the symbol's own, by SVG 2; to the page's without either; and, without a
    // viewBox, not scaled, nor cut to the use's size, as Inkscape draws it.
    all_at(&corners("svg:u1/rect@1:fill"), &[(10.0, 10.0), (50.0, 10.0), (50.0, 50.0), (10.0, 50.0)]);
    all_at(&corners("svg:u2/rect@1:fill"), &[(60.0, 15.0), (80.0, 15.0), (80.0, 25.0), (60.0, 25.0)]);
    all_at(&corners("svg:u3/rect@1:fill"), &[(10.0, 60.0), (30.0, 60.0), (30.0, 80.0), (10.0, 80.0)]);
    all_at(&corners("svg:u4/rect@1:fill"), &[(58.0, 60.0), (68.0, 60.0), (68.0, 70.0), (58.0, 70.0)]);
    all_at(&corners("svg:u5/rect@1:fill"), &[(0.0, 0.0), (100.0, 0.0), (100.0, 100.0), (0.0, 100.0)]);
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
}

#[test]
fn diag_sc_w0804_a_symbol_s_viewport_that_doesnt_read_is_named() {
    let file = svg(drawing(
        r##"<defs><symbol id="bad" viewBox="0 0 ten 10"><rect width="10" height="10"/></symbol>
              <symbol id="flat" viewBox="0 0 0 10"><rect width="10" height="10"/></symbol>
              <symbol id="odd" viewBox="0 0 10 10" preserveAspectRatio="sideways"><rect width="10" height="10"/></symbol></defs>
            <use id="u1" xlink:href="#bad" x="10"/>
            <use id="u2" xlink:href="#flat"/>
            <use id="u3" xlink:href="#odd" width="20" height="20"/>
            <use id="u4" xlink:href="#odd" width="20" height="0"/>
            <use id="u5" xlink:href="#odd" width="wide" height="20"/>"##,
    ));
    assert_eq!(
        warnings(&file),
        [
            "warning SC-W0804: The viewBox of `bad`, which `u1` shows, \"0 0 ten 10\", is not 4 numbers of an area; it is ignored.",
            "warning SC-W0804: The viewBox of `flat`, which `u2` shows, \"0 0 0 10\", is not 4 numbers of an area; it is ignored.",
            "warning SC-W0804: The preserveAspectRatio of `odd`, which `u3` shows, \"sideways\", is not an aspect ratio SVG names; it is \
             ignored, as a viewer ignores it.",
            "warning SC-W0804: The preserveAspectRatio of `odd`, which `u4` shows, \"sideways\", is not an aspect ratio SVG names; it is \
             ignored, as a viewer ignores it.",
            "warning SC-W0804: `u4` gives the symbol `odd` a viewport 20 by 0, which a viewer draws nothing in; it is left out.",
            "warning SC-W0804: The preserveAspectRatio of `odd`, which `u5` shows, \"sideways\", is not an aspect ratio SVG names; it is \
             ignored, as a viewer ignores it.",
            "warning SC-W0804: The width of `u5`, \"wide\", is not a length; it is ignored, as a viewer ignores it.",
        ]
    );
    // What doesn't read is ignored: unscaled, scaled to fit, and fitted to the page across.
    assert_eq!(ids(&file), ["svg:u1/rect@1:fill", "svg:u2/rect@1:fill", "svg:u3/rect@1:fill", "svg:u5/rect@1:fill"]);
    all_at(&ends(element(&file, "svg:u1/rect@1:fill"))[0], &[(10.0, 0.0), (20.0, 0.0), (20.0, 10.0), (10.0, 10.0)]);
    all_at(&ends(element(&file, "svg:u3/rect@1:fill"))[0], &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]);
    all_at(&ends(element(&file, "svg:u5/rect@1:fill"))[0], &[(40.0, 0.0), (60.0, 0.0), (60.0, 20.0), (40.0, 20.0)]);
}

#[test]
fn diag_sc_w0804_a_clone_that_shows_itself_is_left_out() {
    let file = svg(drawing(
        r##"<rect id="square" width="10" height="10" fill="red"/>
            <use id="itself" xlink:href="#itself"/>
            <g id="holder"><circle cx="50" cy="50" r="5" fill="blue"/><use id="inside" xlink:href="#holder" x="20"/></g>
            <use id="ping" xlink:href="#pong"/><use id="pong" xlink:href="#ping"/>
            <use id="fine" xlink:href="#square" x="20"/>"##,
    ));
    let left_out = |id: &str| {
        format!("warning SC-W0804: `{id}` shows itself, directly or through other clones, which a viewer doesn't draw, so it is left out.")
    };
    assert_eq!(warnings(&file), ["itself", "inside", "ping", "pong"].map(left_out));
    assert_eq!(ids(&file), ["svg:square:fill", "svg:circle@1:fill", "svg:fine/square:fill"]);
}

#[test]
fn req_svg_012_a_loop_is_only_through_clones() {
    // A link that isn't a clone, a `<use>` outside SVG's namespace and a command's symbol draw nothing they
    // name, so they make no loop; a clone shown twice is followed once.
    let file = svg(drawing(&format!(
        r##"{SYMBOLS}<defs><g id="g"><a xlink:href="#u"><rect width="5" height="5" fill="red"/></a>
              <x:use xmlns:x="http://example.org/x" xlink:href="#u"/><use xlink:href="#inkstitch_trim"/>
              <use xlink:href="#dot"/><use xlink:href="#dot" x="10"/></g>
              <circle id="dot" cx="50" cy="50" r="2" fill="blue"/></defs>
            <use id="u" xlink:href="#g"/>"##
    )));
    assert_eq!(ids(&file), ["svg:u/rect@1:fill", "svg:u/use@2/dot:fill", "svg:u/use@3/dot:fill"]);
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
    // A command's symbol whose drawing shows the clone isn't followed either.
    let file = svg(drawing(
        r##"<defs><symbol id="inkstitch_trim"><use xlink:href="#u"/></symbol>
              <g id="g"><use xlink:href="#inkstitch_trim"/><rect width="5" height="5" fill="red"/></g></defs>
            <use id="u" xlink:href="#g"/>"##,
    ));
    assert_eq!(ids(&file), ["svg:u/rect@1:fill"]);
}

#[test]
fn diag_sc_w0802_a_clone_inside_more_than_16_clones_is_left_out() {
    let mut body = String::from(r##"<rect id="square" width="10" height="10" fill="red"/><use id="u0" xlink:href="#square"/>"##);
    for k in 1..18 {
        body.push_str(&format!(r##"<use id="u{k}" xlink:href="#u{}"/>"##, k - 1));
    }
    let file = svg(drawing(&body));
    // Each clone is drawn, through all the clones it shows, but the 2 that show 17 or more.
    assert_eq!(file.design.elements().len(), 17);
    // The 17th clone down is named after all those round it.
    let chain = |outer: u32| (outer - 16..=outer).rev().map(|i| format!("u{i}")).collect::<Vec<_>>().join("/");
    let note = |outer: u32| format!("warning SC-W0802: `{}` is a clone inside more than 16 clones, and what it shows is left out.", chain(outer));
    assert_eq!(warnings(&file), [note(16), note(17)]);
    assert!(file.warnings.iter().all(|w| w.code == Code::SvgFeatureIgnored));
}

#[test]
fn req_svg_012_a_link_to_nothing_shows_nothing() {
    let file = svg(drawing(
        r##"<defs><linearGradient id="grad"><stop offset="0" stop-color="red"/></linearGradient></defs>
            <x:rect xmlns:x="http://example.org/x" id="foreign" width="10" height="10"/>
            <use xlink:href="#nothing"/><use xlink:href="square"/><use/><use xlink:href="#grad"/><use xlink:href="#foreign"/>
            <use href=" #square " x="20"/><rect id="square" width="10" height="10"/>"##,
    ));
    // Nor does a viewer draw any of them, so they are left out without a word; a link with spaces round it is
    // read as one without.
    assert_eq!(ids(&file), ["svg:use@6/square:fill", "svg:square:fill"]);
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
}
