//! A guide on one shape of an automatic design in a VectorCraft document (roadmap step M13.24, `REQ-VC-032`):
//! a path named `stitch:guide`, drawn with VectorCraft's own tools over a traced picture, makes the filled
//! parts of the automatic areas it crosses sew as guided fills along it (`REQ-AUTO-109`), wherever it lies
//! in the layers and however many guides the layer holds, and is never sewn.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{RED, document};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Element, Reading, Shape};
use stitchcraft_vectorcraft::{VectorCraft, read_with};

/// A green line 1 pt wide, as an appearance: how a person draws a guide over a picture.
const LINE: &str = r#"{"items": [{"kind": "stroke", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 1, "b": 0}}, "width": 1}]}"#;
/// A white fill, as an appearance.
const WHITE: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "gray", "k": 1}}}]}"#;
/// A blue fill, as an appearance.
const BLUE: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 1}}}]}"#;

/// The closed path `id` round the box from (`x0`, `y0`) to (`x1`, `y1`), in millimetres, with `appearance`.
fn boxed(id: u32, [x0, y0, x1, y1]: [f64; 4], appearance: &str) -> String {
    let [x0, y0, x1, y1] = [x0, y0, x1, y1].map(|mm| mm / MM_PER_POINT);
    format!(
        r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true,
            "anchors": [{{"p": [{x0}, {y0}]}}, {{"p": [{x1}, {y0}]}}, {{"p": [{x1}, {y1}]}}, {{"p": [{x0}, {y1}]}}]}}]}}}}}}"#
    )
}

/// The open path `id` named `name` through `points`, in millimetres, drawn with a thin green line.
fn guide(id: u32, name: &str, points: &[(f64, f64)]) -> String {
    let anchors: Vec<String> = points.iter().map(|(x, y)| format!(r#"{{"p": [{}, {}]}}"#, x / MM_PER_POINT, y / MM_PER_POINT)).collect();
    format!(
        r#"{{"name": {}, "id": {id}, "appearance": {LINE}, "kind": {{"type": "path", "path": {{"subpaths": [{{"anchors": [{}]}}]}}}}}}"#,
        serde_json::to_string(name).unwrap(),
        anchors.join(", ")
    )
}

/// A white field 60 mm square, a red area from (5, 10) to (25, 50), a blue one from (35, 10) to (55, 50),
/// and `more`, in one layer.
fn picture(more: &[String]) -> Vec<u8> {
    let mut objects = vec![boxed(2, [0.0, 0.0, 60.0, 60.0], WHITE), boxed(3, [5.0, 10.0, 25.0, 50.0], RED), boxed(4, [35.0, 10.0, 55.0, 50.0], BLUE)];
    objects.extend(more.iter().cloned());
    document(4, &objects.join(","))
}

/// The document `bytes` read with every shape without an effect digitized automatically.
fn read(bytes: &[u8]) -> VectorCraft {
    read_with(bytes, &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap()
}

/// The ids of the elements the design `read` gives sews as guided fills, once digitized.
fn guided(read: &VectorCraft) -> Vec<String> {
    let (elements, _) = digitized(&read.design, &Budget::DEFAULT);
    let is_guided = |element: &Element| {
        matches!(element.shape, Shape::Fill { .. }) && element.params.get("fill_method") == Some("guided_fill") && element.hints.guide.is_some()
    };
    elements.iter().filter(|element| is_guided(element)).map(|element| element.id.to_string()).collect()
}

#[test]
fn req_vc_032_a_path_named_stitch_guide_steers_the_automatic_parts_it_crosses() {
    // Without a guide, nothing sews a guided fill.
    assert!(guided(&read(&picture(&[]))).is_empty());
    // A line down the red area, named stitch:guide in the Layers panel, in the layer that holds the whole
    // picture: the red area follows it, and the blue one, which it doesn't cross, stays a tatami fill.
    let read = read(&picture(&[guide(5, "stitch:guide", &[(15.0, 5.0), (12.0, 30.0), (15.0, 55.0)])]));
    let ids = guided(&read);
    assert!(!ids.is_empty() && ids.iter().all(|id| id.starts_with("vc:3")), "{ids:?}");
    let (elements, _) = digitized(&read.design, &Budget::DEFAULT);
    assert!(elements.iter().all(|element| !element.id.as_str().starts_with("vc:5")), "the guide is never sewn");
}

#[test]
fn req_vc_032_a_layer_may_hold_a_guide_for_each_shape() {
    // A traced picture lies in one layer, so a person who guides 2 shapes draws 2 guides there. Each steers
    // what it crosses, and neither is said to be unused.
    let first = guide(5, "stitch:guide", &[(15.0, 5.0), (12.0, 30.0), (15.0, 55.0)]);
    let second = guide(6, "Stitch:Guide the right", &[(30.0, 30.0), (45.0, 25.0), (60.0, 30.0)]);
    let read = read(&picture(&[first, second]));
    let ids = guided(&read);
    assert!(ids.iter().any(|id| id.starts_with("vc:3")) && ids.iter().any(|id| id.starts_with("vc:4")), "{ids:?}");
    assert!(read.warnings.iter().all(|warning| warning.code != Code::VcFeatureIgnored), "{:?}", read.warnings);
}
