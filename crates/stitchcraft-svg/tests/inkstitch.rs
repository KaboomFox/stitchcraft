//! `REQ-SVG-003` and `REQ-ASM-004`: Ink/Stitch's own objects in a drawing are not part of the design.
//! Its commands are read for what they ask (a trim, a stop, leaving objects out), and its connectors and
//! helper paths are never stitched.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

mod common;

use common::{fixture, ids, svg, warnings};
use stitchcraft_core::{Code, Severity};

/// The design's elements with the value each gives `key`, or `None`.
fn setting<'s>(svg: &'s stitchcraft_svg::Svg, key: &str) -> Vec<(&'s str, Option<&'s str>)> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.params.get(key))).collect()
}

/// A drawing in millimetres with the namespaces Ink/Stitch's files declare and its current file version,
/// as Ink/Stitch saves it (`REQ-SVG-007`), around `body`.
fn drawing(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:inkstitch="http://inkstitch.org/namespace"
             width="100mm" height="100mm" viewBox="0 0 100 100">
             <metadata><inkstitch:inkstitch_svg_version>4</inkstitch:inkstitch_svg_version></metadata>{body}</svg>"#
    )
}

#[test]
fn req_svg_003_commands_connectors_and_helper_paths_are_not_stitched() {
    let svg = svg(fixture("inkstitch-objects.svg"));
    // Neither the command symbols, nor their connectors, nor the guide line, the pattern sample or the
    // connector between two shapes: only the design itself.
    assert_eq!(ids(&svg), ["svg:outline:stroke", "svg:badge:fill", "svg:badge:stroke"]);
    assert_eq!(
        warnings(&svg),
        [
            "info SC-I0805: `template` is left out: an Ink/Stitch `ignore_object` command is attached to it.",
            "info SC-I0805: `notes` is left out, with everything in it: its Ink/Stitch setting `ignore_object` is on.",
            "warning SC-W0802: `guide` is an Ink/Stitch guide line, a helper for other shapes, so it is not stitched; StitchCraft does not apply guide lines yet.",
            "warning SC-W0802: `sample` is an Ink/Stitch stitch pattern, a helper for the other shapes in its group, so it is not stitched; StitchCraft does not apply patterns yet.",
            "warning SC-W0802: `flow` is a connector (drawn with Inkscape's connector tool), which Ink/Stitch does not stitch; it is left out.",
            "info SC-I0805: `layer2` is left out, with everything in it: an Ink/Stitch `ignore_layer` command is in it.",
            "warning SC-W0802: `use5` is Ink/Stitch's `origin` command, which StitchCraft does not apply yet.",
        ]
    );
}

#[test]
fn req_svg_003_trim_and_stop_commands_set_the_shapes_settings() {
    let svg = svg(fixture("inkstitch-objects.svg"));
    // A command applies to every part of its shape (its fill and its stroke), as in Ink/Stitch, whichever
    // end of the connector the symbol is at, and whatever copy suffix the symbol's id has.
    assert_eq!(setting(&svg, "trim_after"), [("svg:outline:stroke", Some("true")), ("svg:badge:fill", None), ("svg:badge:stroke", None)]);
    assert_eq!(setting(&svg, "stop_after"), [("svg:outline:stroke", None), ("svg:badge:fill", Some("true")), ("svg:badge:stroke", Some("true"))]);
}

#[test]
fn req_svg_003_a_command_on_something_not_stitched_says_so() {
    let svg = svg(drawing(
        r##"<defs><symbol id="inkstitch_trim"><circle r="1"/></symbol><symbol id="inkstitch_stop"><circle r="1"/></symbol></defs>
            <g id="group1"><path id="inner" d="M 0 0 L 10 0" stroke="red" fill="none"/></g>
            <text id="words">hello</text>
            <path id="hidden" d="M 0 5 L 10 5" stroke="red" display="none"/>
            <path id="invisible" d="M 0 6 L 10 6" stroke="red" visibility="hidden"/>
            <path id="c4" d="M 0 0 1 1" stroke="black" inkscape:connection-start="#t3" inkscape:connection-end="#invisible"/>
            <use id="t3" xlink:href="#inkstitch_trim"/>
            <path id="c1" d="M 0 0 1 1" stroke="black" inkscape:connection-start="#t1" inkscape:connection-end="#group1"/>
            <use id="t1" xlink:href="#inkstitch_trim"/>
            <path id="c2" d="M 0 0 1 1" stroke="black" inkscape:connection-start="#s1" inkscape:connection-end="#words"/>
            <use id="s1" xlink:href="#inkstitch_stop"/>
            <path id="c3" d="M 0 0 1 1" stroke="black" inkscape:connection-start="#t2" inkscape:connection-end="#hidden"/>
            <use id="t2" xlink:href="#inkstitch_trim"/>"##,
    ));
    assert_eq!(ids(&svg), ["svg:inner:stroke"]);
    assert_eq!(setting(&svg, "trim_after"), [("svg:inner:stroke", None)]);
    // A hidden or invisible object is left out quietly, and so is its command.
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: `words` is text, which is not stitched; it is left out.",
            "warning SC-W0802: `group1` has an Ink/Stitch `trim` command, which is not applied: `group1` is not a shape that is stitched.",
            "warning SC-W0802: `words` has an Ink/Stitch `stop` command, which is not applied: `words` is not a shape that is stitched.",
        ]
    );
}

#[test]
fn req_svg_003_start_and_end_commands_are_noted() {
    // Ink/Stitch starts a fill or a satin column at its `starting_point` command and ends it at its
    // `ending_point`. StitchCraft does not apply them yet, and says so for each object a viewer shows: on a
    // shape it sews, and, as for a trim, on anything else. A `target_point` aims stitch types StitchCraft
    // does not sew, so it changes nothing sewn here, and a command on a hidden object is quiet.
    let svg = svg(drawing(
        r##"<defs><symbol id="inkstitch_starting_point"><circle r="1"/></symbol><symbol id="inkstitch_ending_point-2"><circle r="1"/></symbol>
              <symbol id="inkstitch_target_point"><circle r="1"/></symbol></defs>
            <rect id="patch" x="10" y="10" width="40" height="30" fill="#0000ff"/>
            <path id="line" d="M 10 60 L 90 60" stroke="red" fill="none"/>
            <g id="group"><path d="M 10 70 L 90 70" stroke="red" fill="none"/></g>
            <path id="hidden" d="M 10 80 L 90 80" stroke="red" fill="none" display="none"/>
            <path id="c1" d="M 60,60 50,40" style="display:none" inkscape:connection-start="#u1" inkscape:connection-end="#patch"/>
            <use id="u1" xlink:href="#inkstitch_starting_point" x="50" y="40"/>
            <path id="c2" d="M 5,5 10,10" style="display:none" inkscape:connection-start="#patch" inkscape:connection-end="#u2"/>
            <use id="u2" xlink:href="#inkstitch_ending_point-2" x="10" y="10"/>
            <path id="c3" d="M 0,0 1,1" style="display:none" inkscape:connection-start="#u3" inkscape:connection-end="#patch"/>
            <use id="u3" xlink:href="#inkstitch_target_point" x="30" y="25"/>
            <path id="c4" d="M 0,0 1,1" style="display:none" inkscape:connection-start="#u4" inkscape:connection-end="#line"/>
            <use id="u4" xlink:href="#inkstitch_ending_point-2" x="90" y="60"/>
            <path id="c5" d="M 0,0 1,1" style="display:none" inkscape:connection-start="#u5" inkscape:connection-end="#group"/>
            <use id="u5" xlink:href="#inkstitch_starting_point" x="10" y="70"/>
            <path id="c6" d="M 0,0 1,1" style="display:none" inkscape:connection-start="#u6" inkscape:connection-end="#hidden"/>
            <use id="u6" xlink:href="#inkstitch_starting_point" x="10" y="80"/>"##,
    ));
    assert_eq!(ids(&svg), ["svg:patch:fill", "svg:line:stroke", "svg:path@2:stroke"]);
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: `patch` has an Ink/Stitch `starting_point` command, which StitchCraft does not apply yet. Ink/Stitch applies it to fills and satin columns.",
            "warning SC-W0802: `patch` has an Ink/Stitch `ending_point` command, which StitchCraft does not apply yet. Ink/Stitch applies it to fills and satin columns.",
            "warning SC-W0802: `line` has an Ink/Stitch `ending_point` command, which StitchCraft does not apply yet. Ink/Stitch applies it to fills and satin columns.",
            "warning SC-W0802: `group` has an Ink/Stitch `starting_point` command, which is not applied: `group` is not a shape that is stitched.",
        ]
    );
    let named: Vec<_> = svg.warnings.iter().map(|w| w.element.as_ref().unwrap().to_string()).collect();
    assert_eq!(named, ["svg:patch", "svg:patch", "svg:line", "svg:group"]);
}

#[test]
fn req_svg_003_only_ink_stitch_symbols_are_commands() {
    let svg = svg(drawing(
        r##"<defs><symbol id="inkstitch_sparkle"><circle r="1"/></symbol><symbol id="logo"><circle r="1"/></symbol>
              <symbol id="inkstitch_satin_cut_point"><circle r="1"/></symbol><symbol id="inkstitch_trim"><circle r="1"/></symbol></defs>
            <g id="inkstitch_stop"><circle r="1"/></g>
            <path id="p" d="M 0 0 L 10 0" stroke="red" fill="none"/>
            <use id="unknown" xlink:href="#inkstitch_sparkle"/>
            <use id="clone" href="#logo"/>
            <use id="not-a-symbol" xlink:href="#inkstitch_stop"/>
            <use id="spaced" xlink:href=" #inkstitch_trim"/>
            <image id="picture" xlink:href="#inkstitch_trim" width="1" height="1"/>
            <path id="c" d="M 0 0 1 1" stroke="black" inkscape:connection-start="#cut" inkscape:connection-end="#p"/>
            <use id="cut" xlink:href="#inkstitch_satin_cut_point"/>"##,
    ));
    // A command is a `<use>` of a `<symbol>` named for one of Ink/Stitch's commands, linked exactly as
    // `#` and its id; anything else is what it would be without Ink/Stitch. A command that only
    // Ink/Stitch's tools use (cutting a satin, routing) changes nothing that is sewn.
    assert_eq!(ids(&svg), ["svg:circle@5:fill", "svg:p:stroke"], "the group named like a command's symbol is drawn");
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: `unknown` is a clone (`<use>`), which is not stitched yet; it is left out.",
            "warning SC-W0802: `clone` is a clone (`<use>`), which is not stitched yet; it is left out.",
            "warning SC-W0802: `not-a-symbol` is a clone (`<use>`), which is not stitched yet; it is left out.",
            "warning SC-W0802: `spaced` is a clone (`<use>`), which is not stitched yet; it is left out.",
            "warning SC-W0802: `picture` is a raster image, which is not stitched; it is left out.",
        ]
    );
}

#[test]
fn req_svg_003_ink_stitch_2_command_symbols_are_commands() {
    // Before its file version 3, Ink/Stitch named 7 commands otherwise; its updater renames them in the
    // files it updates, and they work as the commands they became. Read as clones, they would be named as
    // such, with a hint that gets the symbol's drawing sewn; Ink/Stitch at d59c9ab, run as a black box,
    // sewed the symbol where it did not update the file (`DEV-SVG-014`).
    let starting = "warning SC-W0802: `patch` has an Ink/Stitch `starting_point` command, which StitchCraft does not apply yet. Ink/Stitch \
                    applies it to fills and satin columns.";
    let ending = starting.replace("starting_point", "ending_point");
    for (old, said) in [
        ("fill_start", Some(starting)),
        ("fill_end-2", Some(ending.as_str())),
        ("satin_start", None),
        ("satin_end", None),
        ("run_start", None),
        ("run_end", None),
        ("ripple_target", None),
    ] {
        let file = svg(drawing(&format!(
            r##"<defs><symbol id="inkstitch_{old}"><path d="M0,0 L2,0 L1,2 Z" fill="#000"/></symbol></defs>
                <rect id="patch" x="10" y="10" width="50" height="30" fill="red"/>
                <path id="conn" d="M 70,60 L 40,30" style="fill:none;stroke:#000;stroke-opacity:0.5" inkscape:connector-type="polyline"
                      inkscape:connection-start="#cmd" inkscape:connection-end="#patch"/>
                <use id="cmd" xlink:href="#inkstitch_{old}" x="70" y="60"/>"##
        )));
        assert_eq!(ids(&file), ["svg:patch:fill"], "{old}");
        assert_eq!(warnings(&file), said.into_iter().collect::<Vec<_>>(), "{old}");
    }
}

#[test]
fn req_asm_004_ignored_objects_and_layers_are_left_out_and_listed() {
    let svg = svg(drawing(
        r##"<defs><symbol id="inkstitch_ignore_layer"><circle r="1"/></symbol></defs>
            <g id="outer" inkscape:groupmode="layer" fill="none" stroke="red">
              <path id="kept" d="M 0 0 L 10 0"/>
              <path id="yes" d="M 0 1 L 10 1" inkstitch:ignore_object=" yes "/>
              <path id="t" d="M 0 2 L 10 2" inkstitch:ignore_object="T"/>
              <path id="no" d="M 0 3 L 10 3" inkstitch:ignore_object="on"/>
              <path id="empty" d="M 0 4 L 10 4" inkstitch:ignore_object=""/>
            </g>
            <g id="plain-group" fill="none" stroke="red"><use xlink:href="#inkstitch_ignore_layer"/><path id="in-plain-group" d="M 0 5 L 10 5"/></g>
            <g id="layer-b" inkscape:groupmode="layer" fill="none" stroke="red">
              <g id="sublayer" inkscape:groupmode="layer"><use xlink:href="#inkstitch_ignore_layer"/></g>
              <path id="beside-sublayer" d="M 0 6 L 10 6"/>
            </g>"##,
    ));
    // Ink/Stitch's own reading of a yes: yes, y, true, t or 1, in any case; "on" is not one. An ignore-layer
    // symbol leaves out every layer it is in, sublayers' parents included, and only layers. The settings
    // note is not given for a setting StitchCraft reads.
    assert_eq!(ids(&svg), ["svg:kept:stroke", "svg:no:stroke", "svg:empty:stroke", "svg:in-plain-group:stroke"]);
    assert_eq!(
        warnings(&svg),
        [
            "info SC-I0805: `yes` is left out: its Ink/Stitch setting `ignore_object` is on.",
            "info SC-I0805: `t` is left out: its Ink/Stitch setting `ignore_object` is on.",
            "info SC-I0805: `layer-b` is left out, with everything in it: an Ink/Stitch `ignore_layer` command is in it.",
            "warning SC-W0802: `use@1` is Ink/Stitch's `ignore_layer` command, which is not applied: it is not in a layer.",
        ]
    );
    let listed: Vec<_> = svg.warnings.iter().filter(|w| w.code == Code::SvgObjectIgnored).map(|w| w.element.as_ref().unwrap().to_string()).collect();
    assert_eq!(listed, ["svg:yes", "svg:t", "svg:layer-b"]);
}

#[test]
fn diag_sc_i0805_an_object_left_out_as_the_file_asks_is_named() {
    let svg = svg(drawing(r#"<path id="note" d="M 0 0 L 10 0" stroke="red" inkstitch:ignore_object="True"/>"#));
    assert!(ids(&svg).is_empty());
    let [note] = &svg.warnings[..] else { panic!("{:?}", svg.warnings) };
    assert_eq!(note.to_string(), "info SC-I0805: `note` is left out: its Ink/Stitch setting `ignore_object` is on.");
    assert_eq!((note.code.severity(), note.element.as_ref().map(ToString::to_string).as_deref()), (Severity::Info, Some("svg:note")));
}

#[test]
fn req_svg_003_helper_markers_are_ink_stitch_s_own_and_on_the_start() {
    let svg = svg(drawing(
        r#"<path id="anchor" d="M 0 0 L 10 0" style="stroke:red;marker-start:url(#inkstitch-anchor-line-marker-12)"/>
            <path id="arrow" d="M 0 1 L 10 1" style="stroke:red;fill:none;marker-start:url(#arrow-head)"/>
            <path id="end-only" d="M 0 2 L 10 2" style="stroke:red;fill:none;marker-end:url(#inkstitch-guide-line-marker)"/>
            <path id="attribute" d="M 0 3 L 10 3" stroke="red" fill="none" marker-start="url(#inkstitch-guide-line-marker)"/>"#,
    ));
    // Ink/Stitch knows its helpers by the start marker in the `style` attribute; any other marker is an
    // ordinary one, drawn by a viewer and not stitched.
    assert_eq!(ids(&svg), ["svg:arrow:stroke", "svg:end-only:stroke", "svg:attribute:stroke"]);
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: `anchor` is an Ink/Stitch anchor line, a helper for other shapes, so it is not stitched; StitchCraft does not apply anchor lines yet.",
            "warning SC-W0802: `arrow` has markers (arrowheads and the like), which are not stitched.",
            "warning SC-W0802: `end-only` has markers (arrowheads and the like), which are not stitched.",
            "warning SC-W0802: `attribute` has markers (arrowheads and the like), which are not stitched.",
        ]
    );
}

#[test]
fn req_svg_001_any_marker_property_that_names_a_marker_is_noted() {
    // Each marker property counts on its own: one set to `none` does not cancel another, and a marker
    // inherited from a group is still drawn.
    let svg = svg(drawing(
        r#"<path id="a" d="M 0 0 L 10 0" style="stroke:red;fill:none;marker-start:url(#m);marker-end:none"/>
            <g marker-mid="url(#m)"><path id="b" d="M 0 1 L 10 1" stroke="red" fill="none" marker-end="none"/></g>
            <path id="c" d="M 0 2 L 10 2" stroke="red" fill="none" style="marker:url(#m);marker-start:none;marker-mid:none;marker-end:none"/>"#,
    ));
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: `a` has markers (arrowheads and the like), which are not stitched.",
            "warning SC-W0802: `b` has markers (arrowheads and the like), which are not stitched.",
        ]
    );
}
