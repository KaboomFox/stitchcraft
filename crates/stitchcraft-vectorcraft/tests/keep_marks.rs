//! Marks that keep parts in a VectorCraft document (roadmap step M13.39, `REQ-VC-031`): a path named
//! `stitch:keep`, drawn with VectorCraft's own tools over a traced picture, keeps the parts that lie wholly
//! inside it from merging into their neighbours (`REQ-AUTO-104`), and is never sewn.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{RED, document, ids};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::Reading;
use stitchcraft_plan::{Rgb, Thread};
use stitchcraft_vectorcraft::{VectorCraft, read_with};

/// A green line 1 pt wide, as an appearance: how a person draws a mark over a picture.
const OUTLINE: &str = r#"{"items": [{"kind": "stroke", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 1, "b": 0}}, "width": 1}]}"#;
/// A white fill, as an appearance.
const WHITE: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]}"#;
/// A blue fill, as an appearance.
const BLUE: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}]}"#;

/// The closed path `id` round the box from (`x0`, `y0`) to (`x1`, `y1`), in millimetres, with `appearance`
/// and, if any, `name`.
fn boxed(id: u32, name: Option<&str>, [x0, y0, x1, y1]: [f64; 4], appearance: &str) -> String {
    let [x0, y0, x1, y1] = [x0, y0, x1, y1].map(|mm| mm / MM_PER_POINT);
    let name = name.map_or_else(String::new, |name| format!("\"name\": {}, ", serde_json::to_string(name).unwrap()));
    format!(
        r#"{{{name}"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true,
            "anchors": [{{"p": [{x0}, {y0}]}}, {{"p": [{x1}, {y0}]}}, {{"p": [{x1}, {y1}]}}, {{"p": [{x0}, {y1}]}}]}}]}}}}}}"#
    )
}

/// A white field 60 mm square, a red square on it from (10, 10) to (50, 50), a blue speck of 0.8 mm² on the
/// square at (20, 20), and `more`.
fn picture(more: &[String]) -> Vec<u8> {
    let mut objects = vec![
        boxed(2, None, [0.0, 0.0, 60.0, 60.0], WHITE),
        boxed(3, None, [10.0, 10.0, 50.0, 50.0], RED),
        boxed(4, None, [20.0, 20.0, 21.6, 20.5], BLUE),
    ];
    objects.extend(more.iter().cloned());
    document(4, &objects.join(","))
}

/// The document `bytes` read with every shape without an effect digitized automatically.
fn read(bytes: &[u8]) -> VectorCraft {
    read_with(bytes, &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap()
}

/// Whether the design `read` gives sews anything in blue once it is digitized.
fn sews_blue(read: &VectorCraft) -> bool {
    let (elements, _) = digitized(&read.design, &Budget::DEFAULT);
    elements.iter().any(|element| element.thread == Thread::new(Rgb::new(0, 0, 255)))
}

#[test]
fn req_vc_031_a_path_named_stitch_keep_keeps_the_parts_inside_it_and_is_never_sewn() {
    // Without a mark, the speck merges into the square.
    let plain = read(&picture(&[]));
    assert!(!sews_blue(&plain));
    // A rectangle drawn round it with a stroke and no fill, and named stitch:keep in the Layers panel, keeps
    // it. The name is read as every marker's: in any case, after white space, and a note after it is the
    // person's own, such as which part it keeps.
    for name in ["stitch:keep", "  Stitch:KEEP the eye"] {
        let marked = read(&picture(&[boxed(5, Some(name), [18.0, 18.0, 24.0, 24.0], OUTLINE)]));
        assert!(sews_blue(&marked), "{name}");
        assert!(ids(&marked).iter().all(|id| !id.starts_with("vc:5")), "{:?}", ids(&marked));
        assert!(marked.warnings.iter().all(|warning| warning.code != Code::VcFeatureIgnored), "{:?}", marked.warnings);
    }
}

#[test]
fn req_vc_031_a_group_named_stitch_keep_is_no_mark() {
    // A group or a layer is never a marker: what it holds is read as under any other name, and SC-W0807 says so.
    let group = format!(
        r#"{{"name": "stitch:keep", "id": 5, "kind": {{"type": "group", "children": [{}]}}}}"#,
        boxed(6, None, [18.0, 18.0, 24.0, 24.0], OUTLINE)
    );
    let read = read(&picture(&[group]));
    assert!(!sews_blue(&read));
    assert!(read.warnings.iter().any(|warning| warning.code == Code::VcFeatureIgnored && warning.message.contains("keep")), "{:?}", read.warnings);
}
