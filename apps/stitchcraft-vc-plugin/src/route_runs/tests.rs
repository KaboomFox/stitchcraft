//! Auto-Route Running Stitch (`REQ-VC-020`, `REQ-VC-019`): what it gives back for a selection, and the route,
//! kept as VectorCraft keeps it, sewing as the engine routes the paths.

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::design::{Design, Path, Segment, Subpath};
use stitchcraft_engine::route::{Route, Tool, route};
use stitchcraft_vectorcraft::filter::path_data;

use super::*;
use crate::manifest::{BudgetClass, Schema, route_runs, validate};
use crate::testing::{document, kept};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The polyline through `points`, in millimetres, as VectorCraft gives a filter a path.
fn polyline(points: &[(f64, f64)]) -> Value {
    let (first, rest) = points.split_first().unwrap();
    let subpath = Subpath { start: p(first.0, first.1), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: false };
    serde_json::from_str(path_data(&Path { subpaths: vec![subpath] }).unwrap().get()).unwrap()
}

/// A red stroke, as VectorCraft gives a filter one.
fn red() -> Value {
    json!({"paint": {"type": "solid", "color": {"model": "rgb", "r": 0.9, "g": 0.1, "b": 0.1}}, "width": 1, "join": "miter", "cap": "butt", "miterLimit": 10})
}

/// A record of StitchCraft's effect `family` with `params`, as VectorCraft stores one.
fn record(family: &str, params: &Value) -> Value {
    json!({"id": format!("plugin.dev.stitchcraft.{family}"), "params": params, "visible": true})
}

/// The path `id`, named `name` if given, through `points`, with a red stroke and `effects`.
fn object(id: u64, name: Option<&str>, points: &[(f64, f64)], effects: &Value) -> Value {
    let mut object = json!({"id": id, "type": "path", "path": polyline(points), "fillRule": "nonzero", "fills": [], "strokes": [red()], "effects": effects, "opacity": 1.0});
    if let Some(name) = name {
        object["name"] = json!(name);
    }
    object
}

/// A satin column 3 mm wide along the line through `points`, its own satin effect making it one: object `id`.
fn column(id: u64, points: &[(f64, f64)]) -> Value {
    let mut column = object(id, Some("Column"), points, &json!([record("satin", &json!({}))]));
    column["strokes"][0]["width"] = json!(3.0 * 72.0 / 25.4);
    column
}

/// A rail, a post across it and a brace from the post to the rail, as the engine's tests route them, each
/// with the stroke effect and `settings`: objects 2, 3 and 4.
fn network(settings: &Value) -> [Value; 3] {
    let stroke = json!([record("stroke", settings)]);
    [
        object(2, None, &[(10.0, 32.0), (48.0, 32.0)], &stroke),
        object(3, Some("Post"), &[(16.0, 12.0), (16.0, 35.0)], &stroke),
        object(4, None, &[(16.0, 18.0), (30.0, 32.0)], &stroke),
    ]
}

/// What the filter gives back for `objects`, with the options `set`, run as VectorCraft 0.17 runs it.
fn ran(objects: &[Value], set: &Value) -> Value {
    let input = serde_json::to_vec(&json!({"objects": objects})).unwrap();
    let mut params = set.clone();
    params["_context"] = json!({"mode": "filter", "artboard": [0, 0, 612, 792], "documentData": null});
    serde_json::from_slice(&run(&input, &serde_json::to_vec(&params).unwrap()).unwrap()).unwrap()
}

/// The ids and names of the objects the filter gave back, new ones without an id.
fn listed(out: &Value) -> Vec<(Option<u64>, Option<String>)> {
    out["objects"].as_array().unwrap().iter().map(|o| (o["id"].as_u64(), o["name"].as_str().map(str::to_string))).collect()
}

/// The objects `out` gave back kept as VectorCraft keeps them: each given back in its place, and each new
/// one above the one before, with an id of its own.
fn kept_all(out: &Value) -> Vec<u8> {
    let objects = out["objects"].as_array().unwrap();
    document(&objects.iter().enumerate().map(|(k, object)| kept(object, object["id"].as_u64().unwrap_or(500 + k as u64))).collect::<Vec<_>>())
}

/// The needle points of `design` and what each stitch is, as `stitch export` plans it.
fn stitches(design: &Design) -> Vec<(String, Point)> {
    let plan = stitchcraft_engine::plan(design, stitchcraft_plan::profiles::REFERENCE, &Budget::DEFAULT).plan.unwrap();
    plan.stitches().map(|s| (format!("{:?}", s.kind), s.at)).collect()
}

/// `objects` as a document reads them, routed by the engine with `how`, as a design.
fn routed_by_the_engine(objects: &[Value], how: &Route) -> Design {
    let nodes: Vec<Value> = objects.iter().map(|object| kept(object, object["id"].as_u64().unwrap())).collect();
    let read = stitchcraft_vectorcraft::read(&document(&nodes), &Budget::DEFAULT).unwrap().design;
    let routed = route(read.elements(), how, &read.settings, &Budget::DEFAULT);
    assert!(routed.diagnostics.is_empty(), "{:?}", routed.diagnostics);
    Design::new(routed.pieces.into_iter().map(|piece| piece.element).collect(), read.settings).unwrap()
}

/// Whether `a` and `b` are the same stitches, each in its place to a millionth of a millimetre.
fn alike(a: &[(String, Point)], b: &[(String, Point)]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|((k, p), (l, q))| k == l && p.distance(*q) < 1e-6)
}

#[test]
fn req_vc_020_the_route_replaces_the_strokes_and_sews_as_the_engine_routes_them() {
    let fill = object(5, Some("Patch"), &[(0.0, 40.0), (5.0, 40.0)], &json!([record("fill", &json!({}))]));
    let satin = column(6, &[(0.0, 50.0), (5.0, 50.0)]);
    let [rail, post, brace] = network(&json!({}));
    let selection = [rail, fill, post, satin, brace];
    let out = ran(&selection, &json!({}));
    // The fill and the satin column stay, the brace's place holds the first element and the rest go above
    // it; the rail and the post are gone.
    let names: Vec<Option<String>> = (1..=8).map(|k| Some(if [2, 4, 6].contains(&k) { format!("AutoRun Underpath {k}") } else { format!("AutoRun {k}") })).collect();
    let mut want = vec![(Some(5), None), (Some(6), None), (Some(4), names[0].clone())];
    want.extend(names[1..].iter().map(|name| (None, name.clone())));
    assert_eq!(listed(&out), want);
    assert_eq!((out["objects"][0].clone(), out["objects"][1].clone()), (json!({"id": 5}), json!({"id": 6})), "given back as they are");
    // Kept, the elements sew as the engine routes the strokes.
    let read = stitchcraft_vectorcraft::read(&kept_all(&out), &Budget::DEFAULT).unwrap();
    let engine = routed_by_the_engine(&[selection[0].clone(), selection[2].clone(), selection[4].clone()], &Route {
        tool: Tool::Runs,
        options: Tool::Runs.defaults(),
        start: None,
        end: None,
    });
    let pieces: Vec<_> = read.design.elements().iter().filter(|e| e.name.as_deref().is_some_and(|name| name.starts_with("AutoRun"))).cloned().collect();
    let pieces = Design::new(pieces, read.design.settings).unwrap();
    assert!(alike(&stitches(&pieces), &stitches(&engine)), "{} and {} stitches", stitches(&pieces).len(), stitches(&engine).len());
}

#[test]
fn req_vc_020_each_element_has_its_path_s_stroke_thin_and_its_effects_with_the_element_s_record() {
    let roughen = json!({"id": "distort.roughen", "params": {"size": 5}, "visible": true});
    let settings = json!({"running_stitch_length_mm": 3, "repeats": 2, "bean_stitch_repeats": "1"});
    let [mut rail, post, brace] = network(&settings);
    rail["effects"] = json!([roughen, record("stroke", &settings), record("fill", &json!({}))]);
    let out = ran(&[rail, post, brace], &json!({"trim": true}));
    let objects = out["objects"].as_array().unwrap();
    let (top, under) = (&objects[0], &objects[1]);
    assert_eq!((top["type"].as_str(), under["type"].as_str()), (Some("path"), Some("path")));
    // The path's paint, 1 CSS pixel (0.75 pt) wide, and no fill.
    assert_eq!((top["strokes"][0]["paint"].clone(), top["fills"].clone()), (red()["paint"].clone(), json!([])));
    assert!(top["strokes"][0]["width"].as_f64().is_some_and(|width| (width - 0.75).abs() < 1e-12), "{top}");
    // Its other effects, and the element's record in the place of its first StitchCraft effect, the others
    // gone: on top the stroke's settings, its repeats lowered to 1; under the others its stitch length alone.
    let ids = |o: &Value| o["effects"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(ids(top), ["distort.roughen", "plugin.dev.stitchcraft.stroke"]);
    assert_eq!(ids(under), ["plugin.dev.stitchcraft.stroke"]);
    let params = &top["effects"][1]["params"];
    assert_eq!((params["repeats"].as_i64(), params["running_stitch_length_mm"].as_f64(), params.get("satin_column")), (Some(1), Some(3.0), None));
    let under = under["effects"][0]["params"].as_object().unwrap();
    assert_eq!(under.keys().collect::<Vec<_>>(), ["running_stitch_length_mm"]);
    // No jump is long enough to trim: each element joins the one before.
    let trims = objects.iter().filter(|o| o["effects"].as_array().unwrap().iter().any(|e| e["params"]["trim_after"] == json!(true))).count();
    assert_eq!(trims, 0);
}

#[test]
fn req_vc_020_a_long_jump_trims_and_with_preserve_order_each_path_s_elements_take_its_place() {
    // A line 5 mm below the rail, apart from the rest: the jump to it trims.
    let [rail, post, brace] = network(&json!({}));
    let apart = object(7, None, &[(40.0, 37.0), (46.0, 37.0)], &json!([record("stroke", &json!({}))]));
    let out = ran(&[rail.clone(), post.clone(), brace.clone(), apart.clone()], &json!({"trim": true}));
    let trimmed: Vec<bool> =
        out["objects"].as_array().unwrap().iter().map(|o| o["effects"].as_array().unwrap().iter().any(|e| e["params"]["trim_after"] == json!(true))).collect();
    assert_eq!(trimmed.iter().filter(|trims| **trims).count(), 2, "before the jump to the line apart, and before the jump back: {trimmed:?}");
    // With Preserve order, each path's elements, the runs along it among them, take its place.
    let ordered = ran(&[rail, post, brace, apart], &json!({"preserve_order": true}));
    let ids: Vec<Option<u64>> = listed(&ordered).iter().map(|(id, _)| *id).collect();
    assert_eq!(ids.iter().filter(|id| id.is_some()).collect::<Vec<_>>(), [&Some(2), &Some(3), &Some(4), &Some(7)]);
    assert_eq!(ids.first(), Some(&Some(2)));
}

#[test]
fn req_vc_019_markers_selected_with_the_strokes_start_and_end_the_route_and_are_deleted() {
    let [rail, post, brace] = network(&json!({}));
    let mut start = object(8, Some("stitch:route-start"), &[(15.5, 12.0), (16.5, 12.0)], &json!([]));
    let mut end = object(9, Some("stitch:route-end"), &[(15.5, 35.0), (16.5, 35.0)], &json!([]));
    for marker in [&mut start, &mut end] {
        marker["strokes"] = json!([]);
    }
    let out = ran(&[start, rail.clone(), post.clone(), brace.clone(), end], &json!({}));
    let ids: Vec<Option<u64>> = listed(&out).iter().map(|(id, _)| *id).collect();
    assert!(!ids.contains(&Some(8)) && !ids.contains(&Some(9)), "{ids:?}");
    // The route runs from the post's top to its foot.
    let how = Route { tool: Tool::Runs, options: Tool::Runs.defaults(), start: Some(p(16.0, 12.0)), end: Some(p(16.0, 35.0)) };
    let read = stitchcraft_vectorcraft::read(&kept_all(&out), &Budget::DEFAULT).unwrap();
    let (a, b) = (stitches(&read.design), stitches(&routed_by_the_engine(&[rail, post, brace], &how)));
    assert!(alike(&a, &b), "{} and {} stitches", a.len(), b.len());
}

#[test]
fn diag_sc_e0820_a_selection_with_no_stroke_to_route_or_2_markers_of_a_kind_is_refused() {
    let declined = |objects: &[Value]| ran(objects, &json!({}))["error"].as_str().unwrap().to_string();
    let hint = "Select the running stitches to route, and put the stroke effect on the paths themselves rather than on their group.";
    assert_eq!(declined(&[]), format!("error SC-E0820: Nothing is selected to route. {hint}"));
    let satin = column(5, &[(0.0, 0.0), (5.0, 0.0)]);
    let plain = object(6, None, &[(0.0, 0.0), (5.0, 0.0)], &json!([]));
    assert_eq!(
        declined(&[satin, plain]),
        format!(
            "error SC-E0820: None of the 2 objects selected is a running stitch to route: a path routes as one when its own first StitchCraft \
             effect is the stroke effect. {hint}"
        )
    );
    let [rail, post, _] = network(&json!({}));
    let twice = [object(8, Some("stitch:route-end"), &[(0.0, 0.0), (1.0, 0.0)], &json!([])), rail, post, object(9, Some("stitch:route-end"), &[(9.0, 9.0), (10.0, 9.0)], &json!([]))];
    assert_eq!(
        declined(&twice),
        "error SC-E0820: 2 objects selected are named stitch:route-end, and a route has one end, so nothing was routed. Select one \
         stitch:route-end marker with the paths."
    );
}

#[test]
fn the_dialog_offers_ink_stitch_s_options_and_reads_them() {
    let manifest = route_runs();
    validate(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(manifest.budget, BudgetClass::Heavy);
    let offered: Vec<(&str, Option<&str>, bool)> =
        manifest.params.iter().map(|(key, param)| (*key, param.ui.label, matches!(param.schema, Schema::Bool { default: true }))).collect();
    assert_eq!(
        offered,
        [
            ("break_up", Some("Add nodes at intersections"), true),
            ("preserve_order", Some("Preserve order of running stitches"), false),
            ("trim", Some("Trim jump stitches"), false)
        ]
    );
    // Without Add nodes at intersections, the lines meet only at their nodes: the post and the brace meet the
    // rail at none, and the route jumps between them.
    let unsplit = ran(&network(&json!({})), &json!({"break_up": false}));
    let split = ran(&network(&json!({})), &json!({}));
    assert!(listed(&unsplit).len() < listed(&split).len(), "{:?} {:?}", listed(&unsplit), listed(&split));
    assert!(matches!(run(b"{}", b"no"), Err(RunError::BadInput)));
}
