//! Stroke to Satin (`REQ-VC-014`): what it gives back for a selection, and the columns, kept as VectorCraft
//! keeps them, sewing as the lines did.

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_vectorcraft::filter::path_data;

use super::*;
use crate::manifest::{BudgetClass, stroke_to_satin, validate};
use crate::testing::{document, kept};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// `path`, in millimetres, as VectorCraft gives a filter a path.
fn data(path: &Path) -> Value {
    serde_json::from_str(path_data(path).unwrap().get()).unwrap()
}

/// A path of one open polyline through `points`, in millimetres.
fn polyline(points: &[(f64, f64)]) -> Path {
    let (first, rest) = points.split_first().unwrap();
    Path {
        subpaths: vec![Subpath { start: p(first.0, first.1), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: false }],
    }
}

/// A path of one subpath of cubic curves from `start`, each `[control, control, end]`, in millimetres.
fn curves(start: (f64, f64), curves: &[[(f64, f64); 3]], closed: bool) -> Path {
    let segments = curves.iter().map(|[a, b, c]| Segment::Cubic(p(a.0, a.1), p(b.0, b.1), p(c.0, c.1))).collect();
    Path { subpaths: vec![Subpath { start: p(start.0, start.1), segments, closed }] }
}

/// A circle `radius` mm round (`x`, `y`).
fn circle((x, y): (f64, f64), r: f64) -> Path {
    let k = 4.0 / 3.0 * (std::f64::consts::SQRT_2 - 1.0) * r;
    let quarters = [
        [(x + r, y + k), (x + k, y + r), (x, y + r)],
        [(x - k, y + r), (x - r, y + k), (x - r, y)],
        [(x - r, y - k), (x - k, y - r), (x, y - r)],
        [(x + k, y - r), (x + r, y - k), (x + r, y)],
    ];
    curves((x + r, y), &quarters, true)
}

/// A navy stroke `mm` wide, as VectorCraft gives a filter one.
fn stroke(mm: f64) -> Value {
    json!({"paint": {"type": "solid", "color": {"model": "rgb", "r": 0.0, "g": 0.2, "b": 0.6}, "swatch": "Navy"}, "width": mm * 72.0 / 25.4, "join": "miter", "cap": "butt", "miterLimit": 10})
}

/// A record of StitchCraft's effect `family` with `params`, as VectorCraft stores one.
fn record(family: &str, params: &Value) -> Value {
    json!({"id": format!("plugin.dev.stitchcraft.{family}"), "params": params, "visible": true})
}

/// An object `id` as VectorCraft 0.15 and later give a filter one, of `path` (`PathData`), with `strokes` and
/// `effects`.
fn object(id: u64, path: &Value, strokes: &Value, effects: &Value) -> Value {
    json!({"id": id, "type": "path", "path": path, "fillRule": "nonzero", "fills": [], "strokes": strokes, "effects": effects, "opacity": 1.0, "bounds": [0, 0, 100, 100]})
}

/// What the filter gives back for `objects`, run as VectorCraft 0.17 runs it.
fn ran(objects: &[Value]) -> Value {
    let input = serde_json::to_vec(&json!({"objects": objects})).unwrap();
    let params = json!({"_context": {"mode": "filter", "bounds": [0, 0, 100, 100], "artboard": [0, 0, 612, 792], "documentData": null}});
    serde_json::from_slice(&run(&input, &serde_json::to_vec(&params).unwrap()).unwrap()).unwrap()
}

/// What the filter declines with for `objects`.
fn declined(objects: &[Value]) -> String {
    let out = ran(objects);
    out["error"].as_str().unwrap_or_else(|| panic!("not declined: {out}")).to_string()
}

/// The objects the filter gave back, `out`, kept as VectorCraft keeps them in a document: each given back
/// in its place, and each new one above the one before, with an id of its own.
fn kept_all(out: &Value) -> Vec<u8> {
    let objects = out["objects"].as_array().unwrap_or_else(|| panic!("no objects: {out}"));
    let nodes: Vec<Value> = objects.iter().enumerate().map(|(k, object)| kept(object, object["id"].as_u64().unwrap_or(500 + k as u64))).collect();
    document(&nodes)
}

/// The needle points of the document `bytes` and what each stitch is, as `stitch export` plans it, with what
/// reading and planning it said.
fn stitches(bytes: &[u8]) -> (Vec<(String, Point)>, Vec<stitchcraft_core::Code>) {
    let read = stitchcraft_vectorcraft::read(bytes, &Budget::DEFAULT).unwrap();
    let outcome = stitchcraft_engine::plan(&read.design, stitchcraft_plan::profiles::REFERENCE, &Budget::DEFAULT);
    let said = read.warnings.iter().chain(&outcome.diagnostics).map(|d| d.code).collect();
    let plan = outcome.plan.unwrap_or_else(|| panic!("no plan: {:?}", outcome.diagnostics));
    (plan.stitches().map(|s| (format!("{:?}", s.kind), s.at)).collect(), said)
}

#[test]
fn req_vc_014_a_line_with_the_satin_effect_sews_as_the_column_made_of_it() {
    let s = curves((0.0, 0.0), &[[(5.0, -8.0), (10.0, -8.0), (15.0, 0.0)], [(20.0, 8.0), (25.0, 8.0), (30.0, 0.0)]], false);
    let shapes = [
        ("a straight line", polyline(&[(0.0, 0.0), (30.0, 0.0)])),
        ("a V", polyline(&[(0.0, 0.0), (15.0, 20.0), (30.0, 0.0)])),
        ("an S curve", s),
        ("a circle 30 mm across", circle((15.0, 15.0), 15.0)),
    ];
    for (name, path) in shapes {
        let effects = json!([record("satin", &json!({"zigzag_spacing_mm": 0.5}))]);
        let line = object(2, &data(&path), &json!([stroke(3.0)]), &effects);
        let before = document(&[kept(&line, 2)]);
        let after = kept_all(&ran(&[line]));
        let ((a, said_a), (b, said_b)) = (stitches(&before), stitches(&after));
        assert_eq!(said_b, said_a, "{name}");
        let alike = a.len() == b.len() && a.iter().zip(&b).all(|((k, p), (l, q))| k == l && p.distance(*q) < 1e-6);
        assert!(alike, "{name}: {} stitches as a line, {} as a column", a.len(), b.len());
    }
}

#[test]
fn req_vc_014_the_column_has_the_line_s_paint_thin_and_its_satin_effect_in_the_place_of_its_stitchcraft_effects() {
    let line = data(&polyline(&[(0.0, 0.0), (20.0, 0.0)]));
    let roughen = json!({"id": "distort.roughen", "params": {"size": 5}, "visible": true});
    let satin = record("satin", &json!({"zigzag_spacing_mm": 0.5}));
    let effects = json!([roughen, record("stroke", &json!({})), satin, record("fill", &json!({}))]);
    let out = ran(&[object(4, &line, &json!([{"paint": null, "width": 1}, stroke(3.0)]), &effects)]);
    let [column] = out["objects"].as_array().unwrap().as_slice() else { panic!("{out}") };
    assert_eq!((column["id"].as_u64(), column["type"].as_str(), column["fills"].clone()), (Some(4), Some("compound"), json!([])));
    // The satin effect the line had, with its settings, where its first StitchCraft effect was.
    assert_eq!(column["effects"], json!([roughen, satin]));
    // The paint is the line's, its swatch too; the stroke is a CSS pixel wide.
    let [drawn] = column["strokes"].as_array().unwrap().as_slice() else { panic!("{out}") };
    assert_eq!((drawn["paint"].clone(), drawn["join"].as_str(), drawn["cap"].as_str()), (stroke(3.0)["paint"].clone(), Some("miter"), Some("butt")));
    assert!(drawn["width"].as_f64().is_some_and(|width| (width - 0.75).abs() < 1e-12), "{drawn}");
    // 2 rails and a rung across them in the middle.
    assert_eq!(column["path"]["subpaths"].as_array().map(Vec::len), Some(3));
    // Without a visible satin effect, a new one at the defaults takes the first StitchCraft effect's place,
    // or comes after the others.
    let hidden = json!({"id": "plugin.dev.stitchcraft.satin", "params": {}, "visible": false});
    let new = json!({"id": "plugin.dev.stitchcraft.satin", "params": {}});
    let cases = [
        (json!([roughen, record("stroke", &json!({})), hidden]), json!([roughen, new])),
        (json!([roughen]), json!([roughen, new])),
        (json!([]), json!([new])),
    ];
    for (effects, expected) in cases {
        let out = ran(&[object(4, &line, &json!([stroke(3.0)]), &effects)]);
        assert_eq!(out["objects"][0]["effects"], expected, "{effects}");
    }
}

#[test]
fn req_vc_014_each_line_of_a_path_is_a_column_and_the_others_go_above_the_first() {
    let two =
        Path { subpaths: [polyline(&[(0.0, 0.0), (20.0, 0.0)]), polyline(&[(0.0, 10.0), (20.0, 12.0)])].map(|p| p.subpaths[0].clone()).to_vec() };
    let mut line = object(4, &data(&two), &json!([stroke(2.0)]), &json!([record("stroke", &json!({}))]));
    line["opacity"] = json!(0.5);
    let out = ran(&[line]);
    let objects = out["objects"].as_array().unwrap();
    assert_eq!(objects.iter().map(|o| (o["id"].as_u64(), o["opacity"].as_f64())).collect::<Vec<_>>(), [(Some(4), None), (None, Some(0.5))]);
    assert!(objects.iter().all(|o| o["type"] == "compound" && o["effects"] == json!([{"id": "plugin.dev.stitchcraft.satin", "params": {}}])));
    // Each sews as a column of its own: 2 satin columns.
    let read = stitchcraft_vectorcraft::read(&kept_all(&out), &Budget::DEFAULT).unwrap();
    assert_eq!(read.design.elements().iter().map(|e| e.id.as_str().ends_with(":satin")).collect::<Vec<_>>(), [true, true]);
}

#[test]
fn req_vc_014_a_satin_column_drawn_with_rails_is_left_as_it_is() {
    let rails =
        Path { subpaths: [polyline(&[(0.0, 0.0), (20.0, 0.0)]), polyline(&[(0.0, 3.0), (20.0, 3.0)])].map(|p| p.subpaths[0].clone()).to_vec() };
    let column = object(6, &data(&rails), &json!([stroke(1.0)]), &json!([record("satin", &json!({}))]));
    let line = object(7, &data(&polyline(&[(0.0, 10.0), (20.0, 10.0)])), &json!([stroke(3.0)]), &json!([]));
    let out = ran(&[column, line]);
    let objects = out["objects"].as_array().unwrap();
    assert_eq!(objects[0], json!({"id": 6}));
    assert_eq!((objects.len(), objects[1]["id"].as_u64()), (2, Some(7)));
    // The same 2 lines with a stroke effect are lines, each made into a column.
    let lines = object(6, &data(&rails), &json!([stroke(1.0)]), &json!([record("stroke", &json!({}))]));
    assert_eq!(ran(&[lines])["objects"].as_array().map(Vec::len), Some(2));
}

#[test]
fn diag_sc_e0814_what_can_t_be_made_into_a_satin_column_changes_nothing() {
    let start = "error SC-E0814: ";
    let hint = "Give the path a stroke as wide as the column, draw that column with its 2 rails, or leave the object out of the selection.";
    assert_eq!(
        declined(&[]),
        format!(
            "{start}Nothing is selected to make into a satin column. Select the lines, then choose Object › Plug-ins › StitchCraft Stroke to Satin again."
        )
    );
    let line = data(&polyline(&[(0.0, 0.0), (20.0, 0.0)]));
    let fine = object(1, &line, &json!([stroke(3.0)]), &json!([]));
    let mut bare = object(2, &line, &json!([{"paint": null, "width": 2}]), &json!([]));
    assert_eq!(
        declined(&[fine, bare.clone()]),
        format!("{start}Path 2 of the 2 selected has no stroke that paints, which a satin column takes its width from. {hint}")
    );
    bare["name"] = json!("Stem");
    assert_eq!(declined(&[bare]), format!("{start}The path \"Stem\" has no stroke that paints, which a satin column takes its width from. {hint}"));
    let dot = object(3, &json!({"subpaths": [{"anchors": [{"p": [1, 1]}]}]}), &json!([stroke(3.0)]), &json!([]));
    assert_eq!(declined(&[dot]), format!("{start}The selected path can't be made into a satin column. It draws nothing. {hint}"));
    // Too short for its width, and a second line that is: each says which.
    let short = object(4, &data(&polyline(&[(0.0, 0.0), (0.2, 0.0)])), &json!([stroke(3.0)]), &json!([]));
    assert!(declined(&[short]).starts_with(&format!("{start}The selected line is too short or turns too tightly")));
    let two =
        Path { subpaths: [polyline(&[(0.0, 0.0), (20.0, 0.0)]), polyline(&[(0.0, 10.0), (0.2, 10.0)])].map(|p| p.subpaths[0].clone()).to_vec() };
    let mut parts = object(5, &data(&two), &json!([stroke(3.0)]), &json!([]));
    parts["name"] = json!("Stem");
    assert!(declined(&[parts]).starts_with(&format!("{start}Part 2 of the line \"Stem\" is too short or turns too tightly")));
}

#[test]
fn diag_sc_e0813_a_vectorcraft_that_keeps_no_effect_record_is_told_so() {
    // Before 0.15 VectorCraft gives a filter no effects, and keeps none it gives back.
    let mut older = object(1, &data(&polyline(&[(0.0, 0.0), (20.0, 0.0)])), &json!([stroke(3.0)]), &json!([]));
    older.as_object_mut().unwrap().remove("effects");
    let error = declined(&[older]);
    assert!(
        error.starts_with("error SC-E0813: This VectorCraft keeps no effect record") && error.ends_with("Update VectorCraft to 0.17 or later."),
        "{error}"
    );
}

#[test]
fn diag_sc_e0004_lines_that_need_more_work_than_the_budget_stop_the_filter() {
    let input = json!({"objects": [object(1, &data(&circle((15.0, 15.0), 15.0)), &json!([stroke(3.0)]), &json!([]))]});
    let given = filter::read(&serde_json::to_vec(&input).unwrap(), br#"{"_context": {}}"#).unwrap();
    let poor = Budget { max_stitches: u32::MAX, max_work: 1_000 };
    let error = columns(&given.objects, &mut poor.meter()).unwrap_err();
    assert_eq!(error.code, Code::BudgetExhausted);
    assert_eq!(effect_error(&error), br#"{"error":"error SC-E0004: Making the selected lines into satin columns needs more work than VectorCraft lets a filter do. Select fewer lines at a time."}"#);
}

#[test]
fn req_vc_014_the_filter_s_manifest_is_a_heavy_filter_vectorcraft_accepts() {
    let manifest = stroke_to_satin();
    assert_eq!(validate(&serde_json::to_vec(&manifest).unwrap()), Ok(()));
    assert_eq!((manifest.id.as_str(), manifest.kind, manifest.budget), ("dev.stitchcraft.stroke-to-satin", "filter", BudgetClass::Heavy));
    // Without parameters, VectorCraft runs it at once, as Ink/Stitch's Stroke to Satin.
    assert!(manifest.params.is_empty());
    assert_eq!(serde_json::to_value(&manifest).unwrap()["budget"], json!("heavy"));
}
