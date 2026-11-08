//! What a name that begins with `stitch:` asks of StitchCraft (`REQ-VC-009`): VectorCraft's plug-in ABI v1
//! has no command objects, so an object's or a layer's name carries Ink/Stitch's ignore commands, its start
//! and end positions and its target point.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{RED, document, ids, read_ok, said, square};
use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::design::{Hints, Reading};
use stitchcraft_vectorcraft::read_with;

const IGNORED: Code = Code::VcObjectIgnored;
const NAMED: Code = Code::VcFeatureIgnored;

/// `object` with its name set to `name`.
fn named(object: &str, name: &str) -> String {
    object.replacen("{\"id\": ", &format!("{{\"name\": {}, \"id\": ", serde_json::to_string(name).unwrap()), 1)
}

/// `code` about object `id`, as [`said`] lists it.
fn about(code: Code, id: u32) -> (Code, String) {
    (code, format!("vc:{id}"))
}

#[test]
fn req_vc_009_stitch_ignore_leaves_an_object_out_with_everything_in_it() {
    let group = format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}]}}}}"#, square(4, 50.0, 0.0, RED));
    let objects = [
        named(&square(2, 0.0, 0.0, RED), "stitch:ignore"),
        // A note may follow, and the case of the letters doesn't matter.
        named(&group, "STITCH:Ignore the placement guide"),
        square(5, 100.0, 0.0, RED),
        // Not the marker: `stitch:ignored` is a word of its own.
        named(&square(6, 150.0, 0.0, RED), "stitch:ignored"),
    ];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:5:fill", "vc:6:fill"]);
    assert_eq!(said(&read), [about(IGNORED, 2), about(IGNORED, 3), about(NAMED, 6)]);
    // A layer so named is left out too.
    let layer = br#"{"format": "vectorcraft", "version": 4, "document": {"layers": [{"id": 1, "name": "stitch:ignore", "kind": {"type": "layer",
        "children": [{"id": 2, "appearance": {"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]},
        "kind": {"type": "path", "path": {"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [9, 0]}, {"p": [9, 9]}]}]}}}]}}]}}"#;
    let read = read_ok(layer);
    assert!(ids(&read).is_empty(), "{:?}", ids(&read));
    assert_eq!(said(&read), [about(IGNORED, 1)]);
}

#[test]
fn diag_sc_i0811_what_is_left_out_by_its_name_is_listed() {
    let read = read_ok(&document(4, &named(&square(2, 0.0, 0.0, RED), "stitch:ignore notes for me")));
    assert_eq!(said(&read), [about(IGNORED, 2)]);
    assert_eq!(read.warnings[0].message, "The object \"stitch:ignore notes for me\" is named stitch:ignore, and is left out with everything in it.");
    assert!(read.warnings[0].to_string().starts_with("info SC-I0811: "), "{}", read.warnings[0]);
}

#[test]
fn req_vc_009_start_and_end_markers_place_the_shapes_grouped_with_them() {
    // A small path grouped with the shapes whose start or end it marks, at the middle of its anchors. It is
    // never stitched. The shapes of its group start and end there, those of groups inside it too, and the
    // first marker of each kind counts: another is named, unused. A hidden one is left out without a word,
    // as everything hidden is.
    let inner = format!(r#"{{"id": 7, "kind": {{"type": "group", "children": [{}]}}}}"#, square(8, 100.0, 0.0, RED));
    let group = format!(
        r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {}, {}, {inner}, {}]}}}}"#,
        square(2, 0.0, 0.0, RED),
        named(&square(4, 2.0, 2.0, RED), "stitch:start"),
        named(&square(5, 30.0, 30.0, RED), "Stitch:End 2"),
        named(&square(9, 60.0, 60.0, RED), "stitch:start")
    );
    let hidden = named(&square(6, 50.0, 0.0, RED), "stitch:start").replacen("{\"name\"", "{\"visible\": false, \"name\"", 1);
    let read = read_ok(&document(4, &[group, hidden, square(10, 200.0, 0.0, RED)].join(",")));
    assert_eq!(ids(&read), ["vc:2:fill", "vc:8:fill", "vc:10:fill"]);
    // The middle of a 36 pt square, in millimetres.
    let at = |x: f64, y: f64| Some(Point::new((x + 18.0) * 25.4 / 72.0, (y + 18.0) * 25.4 / 72.0).unwrap());
    let placed = Hints { start: at(2.0, 2.0), end: at(30.0, 30.0), target: None };
    let hints: Vec<Hints> = read.design.elements().iter().map(|e| e.hints).collect();
    assert_eq!(hints, [placed, placed, Hints::default()]);
    assert_eq!(said(&read), [about(NAMED, 9)]);
    assert!(read.warnings[0].message.contains("another start marker in its group comes first"), "{}", read.warnings[0]);
}

#[test]
fn req_vc_009_a_shape_digitized_automatically_starts_and_ends_where_its_parts_do() {
    // Automatic digitizing places where an automatic shape's parts start and end, so a marker grouped with it
    // is not used, which is said.
    let group = format!(
        r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {}]}}}}"#,
        square(2, 0.0, 0.0, RED),
        named(&square(4, 2.0, 2.0, RED), "stitch:start")
    );
    let read = read_with(&document(4, &group), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap();
    assert_eq!(read.design.elements()[0].hints, Hints::default());
    assert_eq!(said(&read), [about(NAMED, 2)]);
    assert!(read.warnings[0].message.starts_with("It is digitized automatically"), "{}", read.warnings[0]);
}

#[test]
fn req_vc_009_a_stitch_name_stitchcraft_does_not_read_is_named_and_stitched() {
    let objects = [named(&square(2, 0.0, 0.0, RED), "stitch:begin"), named(&square(3, 50.0, 0.0, RED), "stitch:")];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:2:fill", "vc:3:fill"]);
    assert_eq!(said(&read), [about(NAMED, 2), about(NAMED, 3)]);
    let hint = read.warnings[0].fix.as_ref().map(stitchcraft_core::Fix::describe);
    assert_eq!(
        hint.as_deref(),
        Some(
            "StitchCraft reads stitch:ignore, stitch:start, stitch:end, stitch:target, stitch:origin, stitch:stop-position, stitch:route-start \
             and stitch:route-end."
        )
    );
}

#[test]
fn req_vc_009_a_target_marker_gives_the_shapes_grouped_with_it_the_point_they_aim_at() {
    // As Ink/Stitch's target_point command does (REQ-RIP-002, REQ-FILL-CIR-008): at the middle of its anchors,
    // for its group's shapes and those of the groups inside it. A group's own comes before those of the groups
    // round it, and the first of a group counts: another is named, unused.
    let inner = format!(
        r#"{{"id": 7, "kind": {{"type": "group", "children": [{}, {}]}}}}"#,
        square(8, 100.0, 0.0, RED),
        named(&square(11, 120.0, 0.0, RED), "stitch:target")
    );
    let group = format!(
        r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {}, {}, {inner}]}}}}"#,
        square(2, 0.0, 0.0, RED),
        named(&square(4, 10.0, 20.0, RED), "Stitch:Target the middle"),
        named(&square(9, 60.0, 60.0, RED), "stitch:target")
    );
    let read = read_ok(&document(4, &[group, square(10, 200.0, 0.0, RED)].join(",")));
    assert_eq!(ids(&read), ["vc:2:fill", "vc:8:fill", "vc:10:fill"]);
    let at = |x: f64, y: f64| Some(Point::new((x + 18.0) * 25.4 / 72.0, (y + 18.0) * 25.4 / 72.0).unwrap());
    let near = |got: Hints, want: Option<Point>| {
        (got.start, got.end) == (None, None) && got.target.zip(want).map_or(got.target == want, |(got, want)| got.distance(want) < 1e-9)
    };
    let hints: Vec<Hints> = read.design.elements().iter().map(|e| e.hints).collect();
    assert!(near(hints[0], at(10.0, 20.0)) && near(hints[1], at(120.0, 0.0)) && near(hints[2], None), "{hints:?}");
    assert_eq!(said(&read), [about(NAMED, 9)]);
    let message = "The object \"stitch:target\" would mark the point the shapes grouped with it aim at, and is not stitched. It is not used: it has no \
                   point that reads, or another target marker in its group comes first.";
    assert_eq!(read.warnings[0].message, message);
    // A group so named is never a marker.
    let group = named(&format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}]}}}}"#, square(4, 0.0, 0.0, RED)), "stitch:target");
    let read = read_ok(&document(4, &group));
    assert_eq!((ids(&read), said(&read)), (vec!["vc:4:fill"], vec![about(NAMED, 3)]));
    let message = "The group \"stitch:target\" is named as a target marker, which a group can't be, and the name is ignored.";
    assert_eq!(read.warnings[0].message, message);
}

#[test]
fn req_vc_019_route_markers_are_left_out_of_the_stitches_without_a_word() {
    // The routing filters read them (`route_marker`), and `stitch export` sews neither. A group so named is
    // never a marker, and the name is said to be ignored.
    let group = named(&format!(r#"{{"id": 4, "kind": {{"type": "group", "children": [{}]}}}}"#, square(5, 100.0, 0.0, RED)), "stitch:route-end");
    let objects = [named(&square(2, 0.0, 0.0, RED), "stitch:route-start"), named(&square(3, 50.0, 0.0, RED), "  Stitch:Route-End here"), group];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:5:fill"]);
    assert_eq!(said(&read), [about(NAMED, 4)]);
    assert_eq!(
        read.warnings[0].message,
        "The group \"stitch:route-end\" is named stitch:route-end, which a group can't be, and the name is ignored."
    );
    // What is said of a group named as either marker names that marker.
    for name in ["stitch:route-start", "stitch:route-end"] {
        let group = named(&format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}]}}}}"#, square(4, 0.0, 0.0, RED)), name);
        let read = read_ok(&document(4, &group));
        assert_eq!((ids(&read), said(&read)), (vec!["vc:4:fill"], vec![about(NAMED, 3)]), "{name}");
        assert_eq!(read.warnings[0].message, format!("The group \"{name}\" is named {name}, which a group can't be, and the name is ignored."));
        let hint = read.warnings[0].fix.as_ref().map(stitchcraft_core::Fix::describe);
        assert_eq!(hint, Some(format!("Name a small path {name}.")));
    }
    use stitchcraft_vectorcraft::{RouteMarker, route_marker};
    assert_eq!(route_marker(" stitch:ROUTE-start note"), Some(RouteMarker::Start));
    assert_eq!(route_marker("stitch:route-end"), Some(RouteMarker::End));
    for other in ["stitch:start", "stitch:route-started", "route-start", "stitch:"] {
        assert_eq!(route_marker(other), None, "{other}");
    }
}

#[test]
fn req_vc_009_a_name_is_not_read_on_what_vectorcraft_draws_nothing_of() {
    // A path that no paint paints, under a group's StitchCraft effect, is invisible in VectorCraft.
    let unpainted = named(&square(4, 0.0, 0.0, "{}"), "stitch:trim");
    let group = format!(
        r#"{{"id": 3, "appearance": {{"effects": [{{"id": "plugin.dev.stitchcraft.fill", "params": {{}}}}]}},
            "kind": {{"type": "group", "children": [{unpainted}]}}}}"#
    );
    let read = read_ok(&document(4, &group));
    assert!(ids(&read).is_empty(), "{:?}", ids(&read));
    assert_eq!(said(&read), []);
}

#[test]
fn req_vc_009_a_stitch_name_says_nothing_of_whether_its_object_is_stitched() {
    // Text isn't stitched, and a path that no paint paints draws nothing: neither is said to be stitched.
    let text = named(r#"{"id": 2, "kind": {"type": "text", "runs": [{"text": "Hi"}]}}"#, "stitch:trim");
    let unpainted = named(&square(3, 0.0, 0.0, "{}"), "stitch:trim");
    let read = read_ok(&document(4, &[text, unpainted].join(",")));
    assert!(ids(&read).is_empty(), "{:?}", ids(&read));
    for said in &read.warnings {
        assert!(!said.message.contains("is stitched"), "{said}");
    }
    assert_eq!(said(&read).first(), Some(&about(NAMED, 2)));
}

#[test]
fn req_vc_009_a_group_or_a_layer_is_never_a_marker() {
    // What a group or a layer named as a marker holds is stitched, and the name is said not to be read.
    let group = named(&format!(r#"{{"id": 4, "kind": {{"type": "group", "children": [{}]}}}}"#, square(5, 100.0, 0.0, RED)), "stitch:end");
    let layer = format!(
        r#"{{"format": "vectorcraft", "version": 4, "document": {{"layers": [{{"id": 1, "name": "stitch:start", "kind": {{"type": "layer",
            "children": [{}, {}, {group}]}}}}]}}}}"#,
        square(2, 0.0, 0.0, RED),
        square(3, 50.0, 0.0, RED)
    );
    let read = read_ok(layer.as_bytes());
    assert_eq!(ids(&read), ["vc:2:fill", "vc:3:fill", "vc:5:fill"]);
    assert_eq!(said(&read), [about(NAMED, 1), about(NAMED, 4)]);
    assert!(read.warnings.iter().all(|w| w.message.contains("can't be, and the name is ignored")), "{:?}", read.warnings);
}

#[test]
fn req_vc_009_a_marker_is_at_the_middle_of_the_box_round_its_anchors() {
    // A triangle's box, not its centroid, and its anchors alone, not the handles of its curves. A marker with
    // no anchor marks nothing, and is named.
    let triangle = r#"{"id": 4, "name": "stitch:end", "appearance": {"items": []}, "kind": {"type": "path", "path": {"subpaths": [{"closed": true,
        "anchors": [{"p": [0, 0], "out": [90, -90]}, {"p": [36, 0]}, {"p": [0, 72]}]}]}}}"#;
    let empty = r#"{"id": 5, "name": "stitch:start", "appearance": {"items": []}, "kind": {"type": "path", "path": {"subpaths": []}}}"#;
    let group = format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}, {triangle}, {empty}]}}}}"#, square(2, 100.0, 0.0, RED));
    let read = read_ok(&document(4, &group));
    let mm = |pt: f64| pt * 25.4 / 72.0;
    let end = Some(Point::new(mm(18.0), mm(36.0)).unwrap());
    assert_eq!(read.design.elements()[0].hints, Hints { end, ..Hints::default() });
    assert_eq!(said(&read), [about(NAMED, 5)]);
    assert!(read.warnings[0].message.contains("it has no point that reads"), "{}", read.warnings[0]);
}

#[test]
fn req_asm_005_origin_and_stop_position_markers_place_the_design_s_origin_and_stop_position() {
    // Anywhere in the document, at the middle of its anchors, and never stitched, as Ink/Stitch's commands
    // place them in an SVG file.
    let group = format!(r#"{{"id": 3, "kind": {{"type": "group", "children": [{}]}}}}"#, named(&square(4, 100.0, 0.0, RED), "stitch:origin"));
    let objects = [square(2, 0.0, 0.0, RED), group, named(&square(5, 200.0, 0.0, RED), "stitch:stop-position")];
    let read = read_ok(&document(4, &objects.join(",")));
    assert_eq!(ids(&read), ["vc:2:fill"]);
    assert!(said(&read).is_empty(), "{:?}", said(&read));
    let at = |x: f64, y: f64| Some(Point::new((x + 18.0) * 25.4 / 72.0, (y + 18.0) * 25.4 / 72.0).unwrap());
    let near = |got: Option<Point>, want: Option<Point>| got.zip(want).is_some_and(|(got, want)| got.distance(want) < 1e-9);
    assert!(near(read.design.settings.origin, at(100.0, 0.0)) && near(read.design.settings.stop_position, at(200.0, 0.0)));
    // A group so named is no marker, and the name is ignored.
    for name in ["stitch:origin", "stitch:stop-position"] {
        let group = format!(r#"{{"id": 3, "name": "{name}", "kind": {{"type": "group", "children": [{}]}}}}"#, square(4, 0.0, 0.0, RED));
        let read = read_ok(&document(4, &group));
        let settings = read.design.settings;
        assert_eq!((ids(&read), settings.origin, settings.stop_position), (vec!["vc:4:fill"], None, None));
        assert_eq!(said(&read), [about(NAMED, 3)]);
        let message = &read.warnings[0].message;
        assert!(message.ends_with(&format!(" is named {name}, which a group can't be, and the name is ignored.")), "{message}");
        let hint = read.warnings[0].fix.as_ref().map(stitchcraft_core::Fix::describe);
        assert_eq!(hint, Some(format!("Name a small path {name}.")));
    }
}

#[test]
fn req_asm_005_an_origin_or_stop_position_marker_with_no_point_that_reads_places_nothing_and_is_named() {
    for name in ["stitch:origin", "stitch:stop-position"] {
        let empty =
            format!(r#"{{"id": 5, "name": "{name}", "appearance": {{"items": []}}, "kind": {{"type": "path", "path": {{"subpaths": []}}}}}}"#);
        let read = read_ok(&document(4, &[square(2, 0.0, 0.0, RED), empty].join(",")));
        assert_eq!((read.design.settings.origin, read.design.settings.stop_position), (None, None));
        assert_eq!(said(&read), [about(NAMED, 5)]);
        assert_eq!(read.warnings[0].message, format!("The object named {name} has no point that reads, so it places nothing."));
    }
}

#[test]
fn diag_sc_e0819_two_origin_markers_or_two_stop_position_markers_and_a_stop_end_planning() {
    // 2 origins whatever the design; 2 stop positions where it stops, and nothing where it doesn't.
    let twice = |name: &str, stop: bool| {
        // A red square, whose fill effect stops after it.
        let stops = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}],
            "effects": [{"id": "plugin.dev.stitchcraft.fill", "params": {"stop_after": true}}]}"#;
        let shape = square(2, 0.0, 0.0, if stop { stops } else { RED });
        read_ok(&document(4, &[shape, named(&square(4, 50.0, 0.0, RED), name), named(&square(5, 60.0, 0.0, RED), name)].join(",")))
    };
    for (name, stop, has) in [("stitch:origin", false, ""), ("stitch:stop-position", true, ", and the design stops")] {
        let read = twice(name, stop);
        let error = read.warnings.iter().find(|d| d.code == Code::OriginOrStopPositionTwice).unwrap_or_else(|| panic!("{name}: {:?}", read.warnings));
        assert_eq!(error.message, format!("The document has 2 objects named {name}{has}: which one counts isn't known."));
        assert_eq!(read.design.settings.origin.or(read.design.settings.stop_position), None);
    }
    assert!(twice("stitch:stop-position", false).warnings.is_empty());
}
