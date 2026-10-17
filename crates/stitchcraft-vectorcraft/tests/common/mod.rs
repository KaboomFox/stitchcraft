//! What the VectorCraft adapter's tests share: documents written for a test, reading them, and looking at
//! what they sew.

// Each test file uses some of these helpers, so each sees the others as unused.
#![allow(dead_code)]

use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::Element;
use stitchcraft_vectorcraft::{VectorCraft, read};

/// A red fill, as an appearance.
pub const RED: &str = r#"{"items": [{"kind": "fill", "paint": {"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}}}]}"#;

/// `bytes` read with the default budget; reading must succeed.
pub fn read_ok(bytes: &[u8]) -> VectorCraft {
    read(bytes, &Budget::DEFAULT).unwrap()
}

/// The ids of the design's elements, in stitching order.
pub fn ids(read: &VectorCraft) -> Vec<&str> {
    read.design.elements().iter().map(|e| e.id.as_str()).collect()
}

/// The element `id`, which must be there.
pub fn element<'a>(read: &'a VectorCraft, id: &str) -> &'a Element {
    read.design.elements().iter().find(|e| e.id.as_str() == id).unwrap_or_else(|| panic!("no {id} in {:?}", ids(read)))
}

/// The parameters of `element`, in order.
pub fn params(element: &Element) -> Vec<(&str, &str)> {
    element.params.iter().collect()
}

/// Each warning as its code and the object it is about.
pub fn said(read: &VectorCraft) -> Vec<(Code, String)> {
    read.warnings.iter().map(|w| (w.code, w.element.as_ref().map_or(String::new(), |e| e.to_string()))).collect()
}

/// A document of one layer holding `objects`, in format `version`.
pub fn document(version: u32, objects: &str) -> Vec<u8> {
    format!(
        r#"{{"format": "vectorcraft", "version": {version}, "document": {{"version": 1, "artboards": [], "next_id": 99,
            "layers": [{{"id": 1, "kind": {{"type": "layer", "color": {{"Preset": 0}}, "children": [{objects}]}}}}]}}}}"#
    )
    .into_bytes()
}

/// A square path of side 36 pt at (`x`, `y`) pt, with `id` and `appearance`.
pub fn square(id: u32, x: f64, y: f64, appearance: &str) -> String {
    let (r, b) = (x + 36.0, y + 36.0);
    format!(
        r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", "path": {{"subpaths": [{{"closed": true,
            "anchors": [{{"p": [{x}, {y}]}}, {{"p": [{r}, {y}]}}, {{"p": [{r}, {b}]}}, {{"p": [{x}, {b}]}}]}}]}}}}}}"#
    )
}

/// A line 50 points long with `id`, `extra` fields in its kind, and `appearance`.
pub fn line(id: u32, extra: &str, appearance: &str) -> String {
    format!(
        r#"{{"id": {id}, "appearance": {appearance}, "kind": {{"type": "path", {extra} "path": {{"subpaths": [{{"anchors": [{{"p": [0, 0]}}, {{"p": [50, 0]}}]}}]}}}}}}"#
    )
}
