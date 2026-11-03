//! What a name that begins with `stitch:` asks of StitchCraft (`REQ-VC-009`): VectorCraft's plug-in ABI v1
//! has no command objects, so an object's or a layer's name carries Ink/Stitch's ignore commands and its
//! start and end positions.

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
    assert_eq!(hint.as_deref(), Some("StitchCraft reads stitch:ignore, stitch:start and stitch:end."));
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
