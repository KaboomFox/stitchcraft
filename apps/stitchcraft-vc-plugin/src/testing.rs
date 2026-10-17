//! What the native tests give a live effect: VectorCraft's input and parameters, as its host passes them.

use serde_json::{Map, Value, json};
use stitchcraft_params::Family;

/// VectorCraft's input for one object whose path is `path`.
pub(crate) fn input(path: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"objects": [{"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 100, 100]}]})).unwrap()
}

/// The parameters VectorCraft resolves from `family`'s manifest for a record that sets `set`: every other
/// parameter at its default, and the effect's context.
pub(crate) fn params(family: Family, set: &[(&str, Value)]) -> Vec<u8> {
    let manifest = serde_json::to_value(crate::manifest::family(family)).unwrap();
    let mut params: Map<String, Value> =
        manifest["params"].as_object().unwrap().iter().map(|(key, spec)| (key.clone(), spec["default"].clone())).collect();
    params.insert("_context".into(), json!({"mode": "effect", "bounds": [0, 0, 100, 100]}));
    for (key, value) in set {
        params.insert((*key).into(), value.clone());
    }
    serde_json::to_vec(&params).unwrap()
}

/// A square `side` points wide, closed.
pub(crate) fn square(side: f64) -> Value {
    json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [side, 0]}, {"p": [side, side]}, {"p": [0, side]}]}]})
}

/// A satin column's two rails, `length` points long and 20 points (7 mm) apart.
pub(crate) fn rails(length: f64) -> Value {
    json!({"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [length, 0]}]}, {"anchors": [{"p": [0, 20]}, {"p": [length, 20]}]}]})
}
