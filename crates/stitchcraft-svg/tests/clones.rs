//! `REQ-SVG-012` and `REQ-SVG-013`: a clone (`<use>`) sews what it shows where it shows it, as Ink/Stitch sews
//! a clone, and turns the angles of the fills it shows with it. These files are drawn 1 user unit to the
//! millimetre.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

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
fn placed(hints: &Hints) -> [Option<(f64, f64)>; 3] {
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
fn req_svg_012_context_fill_and_context_stroke_paint_with_the_clone_s_fill_and_stroke() {
    let file = svg(drawing(
        r##"<defs><g id="motif" color="#ffff00"><rect id="r" width="10" height="10" fill="context-stroke" stroke="context-fill"/>
              <circle id="c" cx="20" cy="5" r="5" style="fill:context-fill"/></g></defs>
            <rect id="alone" y="60" width="10" height="10" fill="context-fill" stroke="context-stroke"/>
            <use id="u1" xlink:href="#motif" fill="currentColor" color="#00ff00" stroke="#0000ff"/>
            <use id="u2" xlink:href="#motif" y="20"/>
            <use id="u3" xlink:href="#u2" y="40" fill="#ff0000"/>
            <use id="u4" xlink:href="#u1" y="60" fill="#ff0000"/>"##,
    ));
    // Outside a clone they paint nothing; a clone's currentColor is the clone's colour, and a clone in a
    // clone gives the paints it has, its own or those it inherits.
    let colours: Vec<(&str, String)> = file.design.elements().iter().map(|e| (e.id.as_str(), e.thread.color.to_string())).collect();
    let expected = [
        ("svg:u1/r:fill", "#0000ff"),
        ("svg:u1/r:stroke", "#00ff00"),
        ("svg:u1/c:fill", "#00ff00"),
        ("svg:u1/c:stroke", "#0000ff"),
        ("svg:u2/r:stroke", "#000000"),
        ("svg:u2/c:fill", "#000000"),
        ("svg:u3/u2/r:stroke", "#ff0000"),
        ("svg:u3/u2/c:fill", "#ff0000"),
        ("svg:u4/u1/r:fill", "#0000ff"),
        ("svg:u4/u1/r:stroke", "#00ff00"),
        ("svg:u4/u1/c:fill", "#00ff00"),
        ("svg:u4/u1/c:stroke", "#0000ff"),
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
              <symbol id="odd" viewBox="0 0 10 10" preserveAspectRatio="sideways"><rect width="10" height="10"/></symbol>
              <symbol id="sized" viewBox="0 0 10 10" width="big"><rect width="10" height="10"/></symbol></defs>
            <use id="u1" xlink:href="#bad" x="10"/>
            <use id="u2" xlink:href="#flat"/>
            <use id="u3" xlink:href="#odd" width="20" height="20"/>
            <use id="u4" xlink:href="#odd" width="20" height="0"/>
            <use id="u5" xlink:href="#odd" width="wide" height="20"/>
            <use id="u6" xlink:href="#sized" height="20"/>
            <use id="u7" xlink:href="#sized" width="0" height="20"/>"##,
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
            "warning SC-W0804: The width of `sized`, which `u6` shows, \"big\", is not a length; it is ignored, as a viewer ignores it.",
            "warning SC-W0804: `u7` gives the symbol `sized` a viewport 0 by 20, which a viewer draws nothing in; it is left out.",
        ]
    );
    // What doesn't read is ignored: unscaled, scaled to fit, and fitted to the page across.
    assert_eq!(ids(&file), ["svg:u1/rect@1:fill", "svg:u2/rect@1:fill", "svg:u3/rect@1:fill", "svg:u5/rect@1:fill", "svg:u6/rect@1:fill"]);
    all_at(&ends(element(&file, "svg:u1/rect@1:fill"))[0], &[(10.0, 0.0), (20.0, 0.0), (20.0, 10.0), (10.0, 10.0)]);
    all_at(&ends(element(&file, "svg:u3/rect@1:fill"))[0], &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)]);
    for clone in ["u5", "u6"] {
        all_at(&ends(element(&file, &format!("svg:{clone}/rect@1:fill")))[0], &[(40.0, 0.0), (60.0, 0.0), (60.0, 20.0), (40.0, 20.0)]);
    }
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

#[test]
fn req_svg_012_a_clone_s_clip_path_cuts_what_it_shows() {
    let file = svg(drawing(
        r##"<defs><clipPath id="left" clipPathUnits="userSpaceOnUse"><rect width="5" height="100"/></clipPath>
              <clipPath id="top" clipPathUnits="userSpaceOnUse"><rect width="100" height="5"/></clipPath>
              <rect id="square" width="10" height="10" fill="red" clip-path="url(#top)"/></defs>
            <use id="c" xlink:href="#square" x="20" clip-path="url(#left)"/>"##,
    ));
    // The source's own clip path, in its user space, which the clone moves, then the clone's, in its own.
    let clips = &element(&file, "svg:c/square:fill").clips;
    let corners: Vec<Vec<Point>> = clips.iter().map(|clip| clip.points().collect()).collect();
    assert_eq!(
        corners,
        [vec![p(20.0, 0.0), p(120.0, 0.0), p(120.0, 5.0), p(20.0, 5.0)], vec![p(0.0, 0.0), p(5.0, 0.0), p(5.0, 100.0), p(0.0, 100.0)]]
    );
}

#[test]
fn req_svg_012_commands_tied_to_its_source_go_with_a_clone() {
    let file = svg(drawing(&format!(
        r##"{SYMBOLS}<rect id="square" x="10" y="10" width="20" height="10" fill="red"/>{}{}{}
            <use id="c" xlink:href="#square" transform="rotate(90 50 50)"/>"##,
        command("s1", "starting_point", (31.0, 10.0), "square"),
        command("e1", "ending_point", (9.0, 20.0), "square"),
        command("t1", "trim", (35.0, 5.0), "square"),
    )));
    assert_eq!(ids(&file), ["svg:square:fill", "svg:c/square:fill"]);
    // Each at its symbol's place, which the clone turns about (50, 50).
    assert_eq!(placed(&element(&file, "svg:square:fill").hints), [Some((31.0, 10.0)), Some((9.0, 20.0)), None]);
    assert_eq!(placed(&element(&file, "svg:c/square:fill").hints), [Some((90.0, 31.0)), Some((80.0, 9.0)), None]);
    assert_eq!(setting(&file, "trim_after"), [("svg:square:fill", Some("true")), ("svg:c/square:fill", Some("true"))]);
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
}

#[test]
fn req_svg_012_a_clone_of_a_group_copies_the_commands_inside_it() {
    // In the group: `a` with its start command, as Attach Commands puts it beside it, and `b` with one whose
    // symbol lies outside the group; `c` with one from outside the group, which stays behind with it.
    let file = svg(drawing(&format!(
        r##"{SYMBOLS}<use id="s2" xlink:href="#inkstitch_starting_point" x="15" y="35"/>
            <g id="g"><rect id="a" width="10" height="10" fill="red"/>{}
              <rect id="b" y="20" width="10" height="10" fill="red"/>
              <path d="M 15,35 10,30" inkscape:connection-start="#s2" inkscape:connection-end="#b"/>
              <rect id="c" y="40" width="10" height="10" fill="red"/></g>{}
            <use id="u" xlink:href="#g" x="50"/>"##,
        command("s1", "starting_point", (11.0, 0.0), "a"),
        command("s3", "starting_point", (0.0, 51.0), "c"),
    )));
    let starts: Vec<(&str, Option<(f64, f64)>)> = file.design.elements().iter().map(|e| (e.id.as_str(), placed(&e.hints)[0])).collect();
    assert_eq!(
        starts,
        [
            ("svg:a:fill", Some((11.0, 0.0))),
            ("svg:b:fill", Some((15.0, 35.0))),
            ("svg:c:fill", Some((0.0, 51.0))),
            ("svg:u/a:fill", Some((61.0, 0.0))),
            ("svg:u/b:fill", Some((15.0, 35.0))),
            ("svg:u/c:fill", None),
        ]
    );
}

#[test]
fn req_svg_012_an_object_left_out_is_left_out_of_its_clones_too() {
    let file = svg(drawing(&format!(
        r##"{SYMBOLS}<rect id="ignored" width="10" height="10" fill="red"/>{}
            <rect id="set" x="20" width="10" height="10" fill="red" inkstitch:ignore_object="yes"/>
            <rect id="kept" x="40" width="10" height="10" fill="red"/>
            <use id="u1" xlink:href="#ignored" y="20"/><use id="u2" xlink:href="#set" y="20"/>
            <use id="u3" xlink:href="#kept" y="20" inkstitch:ignore_object="true"/>{}"##,
        command("i1", "ignore_object", (11.0, 0.0), "ignored"),
        command("i2", "ignore_object", (61.0, 20.0), "u3"),
    )));
    assert_eq!(ids(&file), ["svg:kept:fill"]);
    let codes: Vec<Code> = file.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, [Code::SvgObjectIgnored; 5]);
    assert_eq!(
        warnings(&file)[2..],
        [
            "info SC-I0805: `u1/ignored` is left out: an Ink/Stitch `ignore_object` command is attached to it.",
            "info SC-I0805: `u2/set` is left out: its Ink/Stitch setting `ignore_object` is on.",
            "info SC-I0805: `u3` is left out, with all it shows: an Ink/Stitch `ignore_object` command is attached to it.",
        ]
    );
}

#[test]
fn req_svg_012_a_clone_s_own_trim_stop_and_shortest_lengths_apply_to_what_it_sews() {
    let file = svg(drawing(&format!(
        r##"{SYMBOLS}<defs><g id="pair"><rect id="a" width="5" height="5" fill="red" inkstitch:min_jump_stitch_length_mm="5"/>
              <rect id="b" x="10" width="5" height="5" fill="red"/></g></defs>
            <use id="c1" xlink:href="#pair" inkstitch:trim_after="yes" inkstitch:min_jump_stitch_length_mm="2" inkstitch:min_stitch_length_mm="0.5"/>
            <use id="c2" xlink:href="#pair" y="20" inkstitch:stop_after="true" inkstitch:trim_after="no"/>
            <use id="c3" xlink:href="#pair" y="40"/>{}{}{}
            <use id="c4" xlink:href="#pair" y="80"/>{}
            <use id="none" xlink:href="#pair" y="60" inkstitch:clone="no"/>{}"##,
        command("t3", "trim", (20.0, 40.0), "c3"),
        command("p3", "stop", (20.0, 45.0), "c3"),
        command("s3", "starting_point", (20.0, 50.0), "c3"),
        command("t5", "trim", (20.0, 80.0), "c4"),
        command("t4", "trim", (20.0, 60.0), "none"),
    )));
    let has = |key: &str| setting(&file, key).into_iter().filter_map(|(id, value)| Some(format!("{id}={}", value?))).collect::<Vec<_>>();
    assert_eq!(has("min_jump_stitch_length_mm"), ["svg:c1/a:fill=2", "svg:c1/b:fill=2", "svg:c2/a:fill=5", "svg:c3/a:fill=5", "svg:c4/a:fill=5"]);
    assert_eq!(has("min_stitch_length_mm"), ["svg:c1/a:fill=0.5", "svg:c1/b:fill=0.5"]);
    // A command applies as what it is: a trim alone trims, and stops nothing.
    assert_eq!(has("trim_after"), ["svg:c1/b:fill=true", "svg:c3/b:fill=true", "svg:c4/b:fill=true"]);
    assert_eq!(has("stop_after"), ["svg:c2/b:fill=true", "svg:c3/b:fill=true"]);
    // Of the commands tied to a clone Ink/Stitch applies a trim and a stop, and to one that sews nothing none.
    assert_eq!(
        warnings(&file),
        [
            "info SC-I0805: `none` is left out, with all it shows: its Ink/Stitch setting `clone` is off.",
            "warning SC-W0802: `c3` has an Ink/Stitch `starting_point` command, which is not applied: `c3` is a clone, and of the commands \
             tied to a clone Ink/Stitch applies only a trim and a stop.",
            "warning SC-W0802: `none` has an Ink/Stitch `trim` command, which is not applied: `none` is not a shape that is stitched.",
        ]
    );
}

#[test]
fn diag_sc_w0802_a_command_s_symbol_that_a_clone_shows_is_named_as_the_document_names_it() {
    // The origin's symbol is the document's second `<use>`, and the first in the group the clone shows: the
    // note names it as the document numbers it, though the clone walks the group after it.
    let file = svg(drawing(
        r##"<defs><symbol id="inkstitch_origin"><circle r="1"/></symbol></defs>
            <use id="first" xlink:href="#square" y="20"/>
            <g id="motif"><rect id="square" width="10" height="10" fill="red"/><use xlink:href="#inkstitch_origin" x="far"/></g>
            <use id="clone1" xlink:href="#motif" x="20"/>"##,
    ));
    assert_eq!(ids(&file), ["svg:first/square:fill", "svg:square:fill", "svg:clone1/square:fill"]);
    assert_eq!(
        warnings(&file),
        ["warning SC-W0802: `use@2` is Ink/Stitch's `origin` command, whose place doesn't read: a coordinate or a transform is not a \
             number, or it lies more than 10 m from the origin. It is left out."]
    );
}

#[test]
fn diag_sc_i0805_a_clone_whose_clone_setting_is_off_sews_nothing() {
    let file = svg(drawing(
        r##"<defs><rect id="square" width="10" height="10" fill="red"/></defs>
            <use id="on" xlink:href="#square"/><use id="blank" xlink:href="#square" inkstitch:clone=" "/>
            <use id="yes" xlink:href="#square" inkstitch:clone="Yes"/><use id="no" xlink:href="#square" inkstitch:clone="false"/>
            <use id="maybe" xlink:href="#square" inkstitch:clone="maybe"/>"##,
    ));
    // Off in any of Ink/Stitch's words but its words for yes, as it reads it; a blank one is the default, on.
    assert_eq!(ids(&file), ["svg:on/square:fill", "svg:blank/square:fill", "svg:yes/square:fill"]);
    let off = |id: &str| format!("info SC-I0805: `{id}` is left out, with all it shows: its Ink/Stitch setting `clone` is off.");
    assert_eq!(warnings(&file), [off("no"), off("maybe")]);
}

#[test]
fn diag_sc_w0802_a_clone_s_settings_that_change_nothing_are_named() {
    let file = svg(drawing(
        r##"<rect id="square" width="10" height="10" fill="red"/>
            <use id="c" xlink:href="#square" x="20" inkstitch:fill_method="contour_fill" inkstitch:angle="10" inkstitch:row_spacing_mm="1"/>"##,
    ));
    let named = &file.warnings[0];
    assert_eq!(
        named.to_string(),
        "warning SC-W0802: The clone `c` has Ink/Stitch settings, `fill_method`, `row_spacing_mm`, which Ink/Stitch doesn't read from a \
         clone: they change nothing."
    );
    assert_eq!(named.fix.as_ref().map(|f| f.describe()).as_deref(), Some("Set them on what the clone shows."));
    assert_eq!(file.warnings.len(), 1);
    assert_eq!(setting(&file, "fill_method"), [("svg:square:fill", None), ("svg:c/square:fill", None)]);
}

#[test]
fn diag_sc_w0804_a_clone_s_place_that_doesnt_read_is_named() {
    let file = svg(drawing(
        r##"<g transform="bogus"><rect id="lost" width="10" height="10" fill="red" inkstitch:angle="30"/></g>
            <g transform="scale(0)"><rect id="flat" width="10" height="10" fill="red"/></g>
            <use id="c1" xlink:href="#lost" x="ten" y="20" transform="rotate(45)"/>
            <use id="c2" xlink:href="#flat" x="50"/>"##,
    ));
    // A viewer draws each clone, though its source isn't drawn where it is; Ink/Stitch's angle can't be
    // turned without the source's place.
    assert_eq!(ids(&file), ["svg:c1/lost:fill", "svg:c2/flat:fill"]);
    assert_eq!(setting(&file, "angle"), [("svg:c1/lost:fill", Some("30")), ("svg:c2/flat:fill", Some("0"))]);
    let unread = |id: &str, source: &str| {
        format!(
            "warning SC-W0804: `{id}` shows `{source}`, whose place in the drawing doesn't read: a transform round it, or round `{id}`, cannot \
             be used or flattens it. The angles of its fills don't turn with the clone, and the commands tied to it stay where they are."
        )
    };
    assert_eq!(
        warnings(&file)[2..],
        [
            "warning SC-W0804: The x of `c1`, \"ten\", is not a length; 0 is used, as a viewer uses it.".to_string(),
            unread("c1", "lost"),
            unread("c2", "flat"),
        ]
    );
    all_at(&ends(element(&file, "svg:c2/flat:fill"))[0], &[(50.0, 0.0), (60.0, 0.0), (60.0, 10.0), (50.0, 10.0)]);
}

#[test]
fn req_svg_013_a_clone_turns_the_angles_of_its_fills_with_it() {
    let file = svg(drawing(
        r##"<defs><rect id="flat" width="10" height="10" fill="red"/>
              <rect id="thirty" width="10" height="10" fill="red" inkstitch:angle="30"/>
              <path id="line" d="M 0 50 L 10 50" stroke="blue" fill="none" inkstitch:angle="30"/>
              <rect id="steep" width="10" height="10" fill="red" inkstitch:angle="steep"/></defs>
            <g transform="rotate(90)"><rect id="inner" width="10" height="10" fill="red" inkstitch:angle="30"/></g>
            <use id="r45" xlink:href="#flat" transform="rotate(45)"/>
            <use id="r45b" xlink:href="#thirty" transform="rotate(45)"/>
            <use id="mirror" xlink:href="#thirty" transform="scale(-1 1)"/>
            <use id="tall" xlink:href="#thirty" transform="scale(1 2)"/>
            <use id="moved" xlink:href="#flat" x="5"/>
            <use id="stroke" xlink:href="#line" transform="rotate(45)"/>
            <use id="parent" xlink:href="#inner" x="60"/>
            <use id="unread" xlink:href="#steep" transform="rotate(45)"/>"##,
    ));
    assert_eq!(
        setting(&file, "angle"),
        [
            ("svg:inner:fill", Some("30")),
            ("svg:r45/flat:fill", Some("-45")),
            ("svg:r45b/thirty:fill", Some("-15")),
            ("svg:mirror/thirty:fill", Some("150")),
            ("svg:tall/thirty:fill", Some("49.106605")),
            ("svg:moved/flat:fill", Some("0")),
            ("svg:stroke/line:stroke", Some("30")),
            ("svg:parent/inner:fill", Some("120")),
            ("svg:unread/steep:fill", Some("steep")),
        ]
    );
    assert!(warnings(&file).is_empty(), "{:?}", warnings(&file));
}

#[test]
fn req_svg_013_a_clone_s_own_angle_and_flip_angle() {
    let file = svg(drawing(
        r##"<defs><rect id="thirty" width="10" height="10" fill="red" inkstitch:angle="30"/></defs>
            <use id="own" xlink:href="#thirty" transform="rotate(45)" inkstitch:angle="10"/>
            <use id="flip" xlink:href="#thirty" transform="rotate(45)" inkstitch:flip_angle="true"/>
            <use id="both" xlink:href="#thirty" inkstitch:angle="10.12345678" inkstitch:flip_angle="yes"/>
            <use id="bad" xlink:href="#thirty" transform="rotate(45)" inkstitch:angle="steep"/>
            <use id="blank" xlink:href="#thirty" inkstitch:angle="  "/>
            <use id="units" xlink:href="#thirty" inkstitch:angle="45deg" inkstitch:flip_angle="no"/>
            <g id="holder"><use id="inner" xlink:href="#thirty" inkstitch:angle="20"/></g>
            <use id="outer" xlink:href="#holder" transform="rotate(90)" inkstitch:flip_angle="1"/>"##,
    ));
    // Each clone in its own place, the innermost first: the inner's 20 turned a quarter right, -70, reversed.
    assert_eq!(
        setting(&file, "angle"),
        [
            ("svg:own/thirty:fill", Some("10")),
            ("svg:flip/thirty:fill", Some("15")),
            ("svg:both/thirty:fill", Some("-10.123457")),
            ("svg:bad/thirty:fill", Some("-15")),
            ("svg:blank/thirty:fill", Some("30")),
            ("svg:units/thirty:fill", Some("45")),
            ("svg:inner/thirty:fill", Some("20")),
            ("svg:outer/inner/thirty:fill", Some("70")),
        ]
    );
    assert_eq!(
        warnings(&file),
        ["warning SC-W0802: The Ink/Stitch setting `angle` of `bad`, \"steep\", is not an angle; the clone turns the angles of its fills \
          with it, as Ink/Stitch reads it."]
    );
}

#[test]
fn req_svg_013_a_symbol_s_fills_turn_from_the_clone_s_parent() {
    let file = svg(drawing(
        r##"<defs><g transform="rotate(30)"><symbol id="sym"><rect width="10" height="10" fill="red"/></symbol></g></defs>
            <g transform="translate(10 10)"><use id="u" xlink:href="#sym" transform="rotate(90)"/></g>"##,
    ));
    // Ink/Stitch takes a symbol's content from where the clone is, so the symbol's own surroundings don't
    // turn it: a quarter turn right, and not a sixth.
    assert_eq!(setting(&file, "angle"), [("svg:u/rect@1:fill", Some("-90"))]);
}

#[test]
fn req_svg_013_an_automatic_shape_keeps_the_angle_digitizing_chooses() {
    let file = read_with(
        drawing(
            r##"<defs><rect id="plain" width="10" height="10" fill="red"/></defs>
                <use id="turned" xlink:href="#plain" transform="rotate(45)"/>
                <use id="angled" xlink:href="#plain" inkstitch:angle="10"/><use id="flipped" xlink:href="#plain" inkstitch:flip_angle="yes"/>"##,
        )
        .as_bytes(),
        &Budget::DEFAULT,
        Reading { auto: true, ..Reading::default() },
    )
    .unwrap();
    assert!(file.design.elements().iter().all(|e| e.automatic && e.params.get("angle").is_none()));
    let named = |id: &str| {
        format!(
            "warning SC-W0802: `{id}/plain` is digitized automatically, which chooses its stitches: the Ink/Stitch settings `angle` and \
             `flip_angle` of the clones that show it are not applied."
        )
    };
    assert_eq!(warnings(&file), [named("angled"), named("flipped")]);
}

#[test]
fn req_svg_012_reading_a_clone_costs_work_and_a_clone_bomb_runs_out_of_it() {
    // Each level shows the one below it twice: 2^20 copies of a square.
    let mut body = String::from(r##"<defs><rect id="l0" width="1" height="1" fill="red"/>"##);
    for k in 1..=20 {
        body.push_str(&format!(r##"<g id="l{k}"><use xlink:href="#l{0}"/><use xlink:href="#l{0}" x="1"/></g>"##, k - 1));
    }
    body.push_str(r##"</defs><use xlink:href="#l20"/>"##);
    let refused = read(drawing(&body).as_bytes(), &Budget { max_stitches: 1, max_work: 1_000_000 }).unwrap_err();
    assert_eq!(refused.code, Code::BudgetExhausted);
    // Drawing a clone costs what drawing its source does, and more: following its clones first. Only the
    // document's own reading, and the look at the group's 2 children for Ink/Stitch's guide lines, which the
    // clone takes from its source, are paid once.
    let cost = |text: &str| (1..100_000).find(|&work| read(text.as_bytes(), &Budget { max_stitches: 1, max_work: work }).is_ok()).unwrap();
    let alone = cost(&drawing(r##"<g id="g"><rect width="1" height="1"/><rect x="2" width="1" height="1"/></g>"##));
    let cloned = cost(&drawing(r##"<g id="g"><rect width="1" height="1"/><rect x="2" width="1" height="1"/></g><use xlink:href="#g"/>"##));
    assert!(cloned > 2 * alone - 12, "{alone} {cloned}");
}

#[test]
fn req_svg_012_ink_stitch_s_update_of_an_older_file_counts_each_shape_once() {
    let file = svg(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
          xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">
          <metadata><inkstitch:inkstitch_svg_version>3</inkstitch:inkstitch_svg_version></metadata>
          <rect id="square" width="10" height="10" fill="red" inkstitch:fill_method="tatami_fill"/>
          <use id="c" xlink:href="#square" x="20"/></svg>"##);
    // The clone sews its source as updated, and the note counts the shape once, as Ink/Stitch updates it.
    assert_eq!(setting(&file, "max_stitch_length_mm"), [("svg:square:fill", Some("3")), ("svg:c/square:fill", Some("3"))]);
    assert!(warnings(&file)[0].contains(": 1 shape with a fill and no longest stitch of its own sews 3 mm stitches"), "{:?}", warnings(&file));
}

#[test]
fn req_svg_012_a_clone_is_drawn_as_its_style_and_transform_say() {
    let file = svg(drawing(&format!(
        r##"{SYMBOLS}<defs><g id="nothing"><rect width="5" height="5" fill="none"/></g></defs>
            <rect id="square" width="10" height="10" fill="red"/>
            <use id="flat" xlink:href="#square" transform="scale(0 1)"/>
            <use id="blurred" xlink:href="#square" x="20" style="filter:url(#blur)"/>
            <use id="empty" xlink:href="#nothing"/>{}"##,
        command("t1", "trim", (50.0, 50.0), "empty"),
    )));
    assert_eq!(ids(&file), ["svg:square:fill", "svg:blurred/square:fill"]);
    assert_eq!(
        warnings(&file),
        [
            "warning SC-W0804: The transform of `flat` flattens it to a line or a point, which a viewer doesn't show, so it is left out with \
             all it shows.",
            "warning SC-W0802: `blurred` has filter effects (blurs, shadows and the like), which are not stitched.",
            "warning SC-W0802: `empty` has an Ink/Stitch `trim` command, which is not applied: `empty` is not a shape that is stitched.",
        ]
    );
}

#[test]
fn diag_sc_w0804_a_place_that_doesnt_read_round_a_clone_inside_a_clone_or_a_symbol_is_named() {
    let file = svg(drawing(
        r##"<defs><rect id="square" width="10" height="10" fill="red" inkstitch:angle="30"/></defs>
            <g transform="bogus"><g id="holder"><use id="inner" xlink:href="#square" transform="rotate(90)"/></g>
              <symbol id="sym"><rect width="10" height="10" fill="red" inkstitch:angle="30"/></symbol></g>
            <use id="outer" xlink:href="#holder" x="50"/>
            <use id="shown" xlink:href="#sym" transform="rotate(90)"/>"##,
    ));
    // `outer` shows what lies in a group whose transform doesn't read. Inside it, `inner` is drawn where a
    // viewer draws it, but has no place of its own to turn its fill's angle from; `shown` turns its symbol's
    // angle from where it is, but can't move the commands the symbol's content has.
    assert_eq!(setting(&file, "angle"), [("svg:outer/inner/square:fill", Some("30")), ("svg:shown/rect@1:fill", Some("-60"))]);
    let codes: Vec<Code> = file.warnings.iter().map(|w| w.code).collect();
    assert_eq!(codes, [Code::SvgGeometryUnusable; 4]);
    let said = warnings(&file);
    let shown: Vec<&str> = said[1..].iter().map(|w| w.split(", whose place").next().unwrap_or_default()).collect();
    assert_eq!(
        shown,
        ["warning SC-W0804: `outer` shows `holder`", "warning SC-W0804: `outer/inner` shows `square`", "warning SC-W0804: `shown` shows `sym`"]
    );
}

#[test]
fn req_svg_012_ink_stitch_s_update_moves_a_command_a_clone_copies_once() {
    let file = svg(r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
          xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:inkstitch="http://inkstitch.org/namespace"
          width="100mm" height="100mm" viewBox="0 0 100 100">
          <metadata><inkstitch:inkstitch_svg_version>2</inkstitch:inkstitch_svg_version></metadata>
          <defs><symbol id="inkstitch_fill_start"><circle r="1"/></symbol></defs>
          <rect id="square" width="10" height="10" fill="red" inkstitch:max_stitch_length_mm="4"/>
          <path d="M 20,5 10,5" inkscape:connection-start="#s" inkscape:connection-end="#square"/>
          <use id="s" xlink:href="#inkstitch_fill_start" x="20" y="5"/>
          <use id="c" xlink:href="#square" x="30"/></svg>"##);
    // Each is at its connector's end on the square, where the update moves it, and the clone's moved with it.
    let starts: Vec<_> = file.design.elements().iter().map(|e| placed(&e.hints)[0]).collect();
    assert_eq!(starts, [Some((10.0, 5.0)), Some((40.0, 5.0))]);
    assert!(
        warnings(&file)[0].ends_with(": 1 start or end command works at its connector's end on its object, as before version 3."),
        "{:?}",
        warnings(&file)
    );
}
