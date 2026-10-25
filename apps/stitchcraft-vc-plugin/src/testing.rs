//! What the native tests give a plug-in, VectorCraft's input and parameters as its host passes them, and the
//! documents VectorCraft keeps of what a filter gives back.

use serde_json::{Map, Value, json};
use stitchcraft_params::Family;

/// The conformance fixture `patch.vectorcraft`, a document: a fill, a satin column, a running stitch and a
/// group's fill effect, with a placement line left out by its name and a start marker.
pub(crate) const PATCH: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fixtures/vectorcraft/patch.vectorcraft"));

/// VectorCraft's input for one object whose path is `path`, as up to 0.13 gives it: without its paints.
pub(crate) fn input(path: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"objects": [{"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 100, 100]}]})).unwrap()
}

/// VectorCraft's input for one object whose path is `path`, as 0.15 and later give it: with a black stroke
/// `width` points wide, mitred and with butt caps, and no fill.
pub(crate) fn stroked(path: &Value, width: f64) -> Vec<u8> {
    let stroke = json!({"paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 0}}, "width": width, "join": "miter", "cap": "butt", "miterLimit": 10});
    let object = json!({"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 100, 100], "fills": [], "strokes": [stroke]});
    serde_json::to_vec(&json!({"objects": [object]})).unwrap()
}

/// An open line through `points`, in points.
pub(crate) fn line(points: &[(f64, f64)]) -> Value {
    let anchors: Vec<Value> = points.iter().map(|(x, y)| json!({"p": [x, y]})).collect();
    json!({"subpaths": [{"anchors": anchors}]})
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

/// A document as VectorCraft saves it, whose one layer holds `nodes`, bottom first.
pub(crate) fn document(nodes: &[Value]) -> Vec<u8> {
    let layer = json!({"id": 1, "kind": {"type": "layer", "children": nodes}});
    let document = json!({"version": 1, "artboards": [], "next_id": 100_000, "layers": [layer]});
    serde_json::to_vec(&json!({"format": "vectorcraft", "version": 4, "document": document})).unwrap()
}

/// The node VectorCraft keeps of `out`, an object a filter gave back whole, as a document saves it with the
/// id `id`: a path, or a compound path of a path for each subpath, with its fill rule; its fills and its
/// strokes, as appearance items; its effect records, its opacity and its name. So VectorCraft's
/// `objects::apply_output` writes a new object, and an object given back with all of these, which the
/// contract tests check in its host.
pub(crate) fn kept(out: &Value, id: u64) -> Value {
    let rule = if out["fillRule"] == "evenodd" { "EvenOdd" } else { "NonZero" };
    let kind = if out["type"] == "compound" {
        let members = out["path"]["subpaths"].as_array().unwrap().iter().enumerate();
        let children: Vec<Value> =
            members.map(|(k, subpath)| json!({"id": id * 1_000 + k as u64 + 1, "kind": {"type": "path", "path": {"subpaths": [subpath]}}})).collect();
        json!({"type": "compound", "children": children, "rule": rule})
    } else {
        json!({"type": "path", "path": out["path"], "rule": rule})
    };
    let fills = out["fills"].as_array().into_iter().flatten().map(|paint| json!({"kind": "fill", "paint": paint}));
    // A document names a join and a cap as VectorCraft's code does (`Miter`), and a filter as SVG does (`miter`).
    let named = |name: &Value| {
        let name = name.as_str().unwrap_or_default();
        json!(name.get(..1).unwrap_or_default().to_uppercase() + name.get(1..).unwrap_or_default())
    };
    let strokes = out["strokes"].as_array().into_iter().flatten().map(|stroke| {
        let (join, cap) = (named(&stroke["join"]), named(&stroke["cap"]));
        json!({"kind": "stroke", "paint": stroke["paint"], "width": stroke["width"], "join": join, "cap": cap, "miter_limit": stroke["miterLimit"]})
    });
    let items: Vec<Value> = fills.chain(strokes).collect();
    let effects = out.get("effects").cloned().unwrap_or_else(|| json!([]));
    let mut node = json!({"id": id, "appearance": {"items": items, "effects": effects}, "kind": kind});
    for key in ["opacity", "name"] {
        if let Some(value) = out.get(key) {
            node[key] = value.clone();
        }
    }
    node
}

/// The PES file the document `bytes` sews as, as `stitch export` writes it for the reference machine, and the
/// codes of what reading and planning it said.
pub(crate) fn sewn(bytes: &[u8]) -> (Vec<u8>, Vec<stitchcraft_core::Code>) {
    let read = stitchcraft_vectorcraft::read(bytes, &stitchcraft_core::Budget::DEFAULT).unwrap();
    let (file, said) = planned(&read.design);
    (file, read.warnings.iter().map(|d| d.code).chain(said).collect())
}

/// The PES file `design` sews as for the reference machine, and the codes of what planning it said.
pub(crate) fn planned(design: &stitchcraft_engine::design::Design) -> (Vec<u8>, Vec<stitchcraft_core::Code>) {
    let outcome = stitchcraft_engine::plan(design, stitchcraft_plan::profiles::REFERENCE, &stitchcraft_core::Budget::DEFAULT);
    let said = outcome.diagnostics.iter().map(|d| d.code).collect();
    let plan = outcome.plan.unwrap_or_else(|| panic!("no plan: {:?}", outcome.diagnostics));
    (stitchcraft_formats::encode(&plan, stitchcraft_plan::FormatId::PesV1, "test").unwrap().bytes, said)
}
