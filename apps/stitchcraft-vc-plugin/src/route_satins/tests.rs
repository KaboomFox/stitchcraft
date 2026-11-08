//! Auto-Route Satin Columns (`REQ-VC-018`, `REQ-VC-019`): what it gives back for a selection, and the route,
//! kept as VectorCraft keeps it, sewing as the engine routes the paths.

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::design::{Design, Path, Segment, Subpath};
use stitchcraft_engine::route::Route;
use stitchcraft_vectorcraft::filter::path_data;

use super::*;
use crate::manifest::{BudgetClass, route_satins, validate};
use crate::testing::{document, kept};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The straight lines `lines`, in millimetres, as VectorCraft gives a filter a path: a subpath each.
fn lines(lines: &[[(f64, f64); 2]]) -> Value {
    let subpaths = lines.iter().map(|[a, b]| Subpath { start: p(a.0, a.1), segments: vec![Segment::Line(p(b.0, b.1))], closed: false }).collect();
    serde_json::from_str(path_data(&Path { subpaths }).unwrap().get()).unwrap()
}

/// A navy stroke, as VectorCraft gives a filter one.
fn navy() -> Value {
    json!({"paint": {"type": "solid", "color": {"model": "rgb", "r": 0.0, "g": 0.2, "b": 0.6}}, "width": 1, "join": "miter", "cap": "butt", "miterLimit": 10})
}

/// A record of StitchCraft's effect `family` with `params`, as VectorCraft stores one.
fn record(family: &str, params: &Value) -> Value {
    json!({"id": format!("plugin.dev.stitchcraft.{family}"), "params": params, "visible": true})
}

/// The object `id`, named `name` if given, of `path`, with a navy stroke and `effects`.
fn object(id: u64, name: Option<&str>, path: &Value, effects: &Value) -> Value {
    let mut object = json!({"id": id, "type": "compound", "path": path, "fillRule": "nonzero", "fills": [], "strokes": [navy()], "effects": effects, "opacity": 1.0});
    if let Some(name) = name {
        object["name"] = json!(name);
    }
    object
}

/// An upright column from y = 5 to 25 between x = 5 and 8, and a bar from x = 15 to 25 between y = 5 and 8,
/// each with a rung, as the engine's tests route them (`two`), with the satin effect: objects 2 and 3.
fn two() -> [Value; 2] {
    let satin = json!([record("satin", &json!({}))]);
    [
        object(2, None, &lines(&[[(5.0, 5.0), (5.0, 25.0)], [(8.0, 5.0), (8.0, 25.0)], [(4.0, 15.0), (9.0, 15.0)]]), &satin),
        object(3, Some("Bar"), &lines(&[[(15.0, 5.0), (25.0, 5.0)], [(15.0, 8.0), (25.0, 8.0)], [(20.0, 4.0), (20.0, 9.0)]]), &satin),
    ]
}

/// A marker named `name` at (`x`, `y`) mm.
fn marker(id: u64, name: &str, (x, y): (f64, f64)) -> Value {
    let mut dot = object(id, Some(name), &lines(&[[(x - 0.5, y), (x + 0.5, y)]]), &json!([]));
    dot["type"] = json!("path");
    dot
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

/// `objects` as a filter is given them.
fn objects_of(objects: &[Value]) -> Vec<Object> {
    filter::read(&serde_json::to_vec(&json!({"objects": objects})).unwrap(), b"{}").unwrap().objects
}

/// What the filter declines with for `objects`.
fn declined(objects: &[Value]) -> String {
    ran(objects, &json!({}))["error"].as_str().unwrap().to_string()
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

#[test]
fn req_vc_018_the_route_replaces_the_columns_and_sews_as_the_engine_routes_them() {
    let fill = object(4, Some("Patch"), &lines(&[[(0.0, 40.0), (5.0, 40.0)]]), &json!([record("fill", &json!({}))]));
    let [upright, bar] = two();
    let selection = [upright, fill, bar];
    let out = ran(&selection, &json!({}));
    // The fill stays, the bar's place holds the first piece, and the rest go above it; the upright is gone.
    let names = ["AutoSatin 1", "AutoSatin Running Stitch 2", "AutoSatin 3", "AutoSatin 4"].map(|name| Some(name.to_string()));
    assert_eq!(
        listed(&out),
        [(Some(4), None), (Some(3), names[0].clone()), (None, names[1].clone()), (None, names[2].clone()), (None, names[3].clone())]
    );
    assert_eq!(out["objects"][0], json!({"id": 4}), "given back as it is");
    // Kept, the pieces sew as the engine routes the columns.
    let read = stitchcraft_vectorcraft::read(&kept_all(&out), &Budget::DEFAULT).unwrap();
    // The fill, given back as it is, is kept as a node of nothing, which sews nothing.
    assert_eq!(read.design.elements().len(), 4);
    let pieces = read.design;
    let engine = routed_by_the_engine(
        &[selection[0].clone(), selection[2].clone()],
        &Route { tool: Tool::Satins, options: Tool::Satins.defaults(), start: None, end: None },
    );
    let (a, b) = (stitches(&pieces), stitches(&engine));
    assert!(a.len() == b.len() && a.iter().zip(&b).all(|((k, p), (l, q))| k == l && p.distance(*q) < 1e-6), "{} and {} stitches", a.len(), b.len());
}

#[test]
fn req_vc_018_each_piece_has_its_path_s_stroke_thin_and_its_effects_with_the_piece_s_record() {
    let roughen = json!({"id": "distort.roughen", "params": {"size": 5}, "visible": true});
    let [mut upright, bar] = two();
    let satin = json!({"zigzag_spacing_mm": 0.5, "center_walk_underlay_repeats": 3, "center_walk_underlay_stitch_length_mm": 2});
    upright["effects"] = json!([roughen, record("satin", &satin), record("fill", &json!({}))]);
    let out = ran(&[upright, bar], &json!({}));
    let objects = out["objects"].as_array().unwrap();
    let (satin, run) = (&objects[0], &objects[1]);
    assert_eq!((satin["type"].as_str(), run["type"].as_str()), (Some("compound"), Some("path")));
    // The path's paint, 1 CSS pixel (0.75 pt) wide, and no fill.
    assert_eq!((satin["strokes"][0]["paint"].clone(), satin["fills"].clone()), (navy()["paint"].clone(), json!([])));
    assert!(satin["strokes"][0]["width"].as_f64().is_some_and(|width| (width - 0.75).abs() < 1e-12), "{satin}");
    // Its other effects, and the piece's record in the place of its first StitchCraft effect, the others gone.
    let ids = |o: &Value| o["effects"].as_array().unwrap().iter().map(|e| e["id"].as_str().unwrap().to_string()).collect::<Vec<_>>();
    assert_eq!(ids(satin), ["distort.roughen", "plugin.dev.stitchcraft.satin"]);
    assert_eq!(ids(run), ["distort.roughen", "plugin.dev.stitchcraft.stroke"]);
    let params = &satin["effects"][1]["params"];
    assert_eq!(
        (params["zigzag_spacing_mm"].as_f64(), params["center_walk_underlay_repeats"].as_i64(), params.get("satin_column")),
        (Some(0.5), Some(2), None)
    );
    // The run under the column takes its centre walk's stitch length.
    let run_params = run["effects"][1]["params"].as_object().unwrap();
    assert_eq!(run_params.keys().collect::<Vec<_>>(), ["running_stitch_length_mm"]);
    assert_eq!(run_params["running_stitch_length_mm"].as_f64(), Some(2.0));
    // The piece before the jump to the bar trims, and so does the last.
    let trims: Vec<bool> =
        objects.iter().map(|o| o["effects"].as_array().unwrap().iter().any(|e| e["params"]["trim_after"] == json!(true))).collect();
    assert_eq!(trims, [false, false, true, true]);
}

#[test]
fn req_vc_018_with_preserve_order_each_path_s_pieces_take_its_place_and_kept_originals_stay() {
    let ordered = ran(&two(), &json!({"preserve_order": true, "trim": false}));
    assert_eq!(listed(&ordered).iter().map(|(id, _)| *id).collect::<Vec<_>>(), [Some(2), None, None, Some(3)]);
    let none_trims = ordered["objects"]
        .as_array()
        .unwrap()
        .iter()
        .all(|o| o["effects"].as_array().unwrap().iter().all(|e| e["params"].get("trim_after") != Some(&json!(true))));
    assert!(none_trims, "{ordered}");
    // Kept, the paths stay as they were, and the route goes above the topmost.
    let kept = ran(&two(), &json!({"keep_originals": true}));
    assert_eq!(listed(&kept).iter().map(|(id, _)| *id).collect::<Vec<_>>(), [Some(2), Some(3), None, None, None, None]);
    assert_eq!(kept["objects"][1], json!({"id": 3}));
}

#[test]
fn req_vc_019_markers_selected_with_the_paths_start_and_end_the_route_and_are_deleted() {
    let [upright, bar] = two();
    let start = marker(5, "stitch:route-start", (21.0, 6.5));
    let end = marker(6, "Stitch:Route-End  foot", (6.5, 26.0));
    let out = ran(&[start.clone(), upright.clone(), bar.clone(), end.clone()], &json!({}));
    let ids: Vec<Option<u64>> = listed(&out).iter().map(|(id, _)| *id).collect();
    assert!(!ids.contains(&Some(5)) && !ids.contains(&Some(6)), "{ids:?}");
    // The route starts at the bar's middle, runs under its right half, and ends at the upright's foot.
    let how = Route { tool: Tool::Satins, options: Tool::Satins.defaults(), start: Some(p(21.0, 6.5)), end: Some(p(6.5, 26.0)) };
    let read = stitchcraft_vectorcraft::read(&kept_all(&out), &Budget::DEFAULT).unwrap();
    let (a, b) = (stitches(&read.design), stitches(&routed_by_the_engine(&[upright.clone(), bar.clone()], &how)));
    assert!(a.len() == b.len() && a.iter().zip(&b).all(|((k, p), (l, q))| k == l && p.distance(*q) < 1e-6), "{} and {} stitches", a.len(), b.len());
    // With the originals kept, the markers stay too; a marker without a point marks nothing and stays.
    let kept = ran(&[start, upright.clone(), bar.clone(), end], &json!({"keep_originals": true}));
    assert_eq!(listed(&kept).iter().filter(|(id, _)| matches!(id, Some(5 | 6))).count(), 2);
    let pointless = json!({"id": 7, "type": "path", "name": "stitch:route-start", "effects": []});
    assert_eq!(listed(&ran(&[pointless, upright, bar], &json!({})))[0].0, Some(7));
}

#[test]
fn diag_sc_e0820_a_selection_with_nothing_to_route_or_2_markers_of_a_kind_is_refused() {
    let hint = "Select the satin columns to route, with the strokes among them, and put the satin effect on the paths themselves rather than on \
                their group.";
    assert_eq!(declined(&[]), format!("error SC-E0820: Nothing is selected to route. {hint}"));
    let fill = object(4, None, &lines(&[[(0.0, 0.0), (5.0, 0.0)]]), &json!([record("fill", &json!({}))]));
    let plain = object(5, None, &lines(&[[(0.0, 0.0), (5.0, 0.0)]]), &json!([]));
    let automatic = object(6, None, &lines(&[[(0.0, 0.0), (5.0, 0.0)]]), &json!([record("auto", &json!({}))]));
    let stroke = object(7, None, &lines(&[[(0.0, 0.0), (5.0, 0.0)]]), &json!([record("stroke", &json!({}))]));
    let unusable = json!({"id": 8, "type": "path", "effects": [record("satin", &json!({}))]});
    assert_eq!(
        declined(&[fill, plain, automatic, stroke, unusable]),
        format!(
            "error SC-E0820: None of the 5 objects selected is a satin column to route: a path routes as one when its own first StitchCraft \
             effect is the satin effect. {hint}"
        )
    );
    let [upright, bar] = two();
    for (name, end) in [("stitch:route-start", "start"), ("stitch:route-end", "end")] {
        let selection = [marker(5, name, (0.0, 0.0)), upright.clone(), bar.clone(), marker(6, name, (9.0, 9.0))];
        assert_eq!(
            declined(&selection),
            format!(
                "error SC-E0820: 2 objects selected are named {name}, and a route has one {end}, so nothing was routed. Select one {name} marker with the paths."
            )
        );
    }
}

#[test]
fn a_vectorcraft_that_gives_no_effect_records_is_too_old_and_a_path_without_a_stroke_is_painted_black() {
    let [mut upright, mut bar] = two();
    upright.as_object_mut().unwrap().remove("effects");
    assert!(declined(&[upright]).starts_with("error SC-E0813: This VectorCraft keeps no effect record a filter gives"));
    bar["strokes"] = json!([]);
    let out = ran(&[bar], &json!({}));
    assert_eq!(out["objects"][0]["strokes"][0]["paint"]["color"], json!({"model": "rgb", "r": 0.0, "g": 0.0, "b": 0.0}));
}

#[test]
fn the_filter_stops_before_vectorcraft_would() {
    let objects = objects_of(&[marker(5, "stitch:route-start", (0.0, 0.0)), two()[0].clone()]);
    // Reading the marker, then the path, runs out.
    for skip in [0, 1] {
        let refused = selected(&objects[skip..], &mut Budget { max_work: 0, ..Budget::DEFAULT }.meter()).err().unwrap();
        assert_eq!(refused.to_string(), "error SC-E0004: Reading the selected paths needs more work than VectorCraft lets a filter do.");
    }
    // Routing runs out, with the engine's word.
    let routed = replaced(&objects[1..], Tool::Satins.defaults(), 100 * PER_UNIT).err().unwrap();
    assert!(routed.to_string().starts_with("error SC-E0004: Routing these elements needed more than the work budget"), "{routed}");
}

#[test]
fn the_dialog_offers_ink_stitch_s_options_and_reads_them() {
    let manifest = route_satins();
    validate(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!(manifest.budget, BudgetClass::Heavy);
    let offered: Vec<(&str, Option<&str>, bool)> =
        manifest.params.iter().map(|(key, param)| (*key, param.ui.label, matches!(param.schema, Schema::Bool { default: true }))).collect();
    assert_eq!(
        offered,
        [
            ("trim", Some("Trim jump stitches"), true),
            ("preserve_order", Some("Preserve order of satin columns"), false),
            ("keep_originals", Some("Keep original paths"), false)
        ]
    );
    assert!(matches!(run(b"{}", b"no"), Err(RunError::BadInput)));
}

#[test]
fn req_vc_018_lines_with_the_satin_effect_route_as_the_columns_the_engine_makes_of_them() {
    // `compat/scenarios/route-satins.toml`'s lines: an upright and a bar, each 3 mm wide, as `two`'s columns.
    let wide = json!([{"paint": navy()["paint"], "width": 3.0 * 72.0 / 25.4, "join": "miter", "cap": "butt", "miterLimit": 10}]);
    let satin = json!([record("satin", &json!({}))]);
    let mut upright = object(2, None, &lines(&[[(6.5, 5.0), (6.5, 25.0)]]), &satin);
    let mut bar = object(3, None, &lines(&[[(15.0, 6.5), (25.0, 6.5)]]), &satin);
    for line in [&mut upright, &mut bar] {
        line["type"] = json!("path");
        line["strokes"] = wide.clone();
    }
    let out = ran(&[upright.clone(), bar.clone()], &json!({}));
    let names: Vec<Option<String>> = listed(&out).into_iter().map(|(_, name)| name).collect();
    assert_eq!(names, ["AutoSatin 1", "AutoSatin Running Stitch 2", "AutoSatin 3", "AutoSatin 4"].map(|name| Some(name.to_string())));
    let read = stitchcraft_vectorcraft::read(&kept_all(&out), &Budget::DEFAULT).unwrap();
    let engine = routed_by_the_engine(&[upright, bar], &Route { tool: Tool::Satins, options: Tool::Satins.defaults(), start: None, end: None });
    let (a, b) = (stitches(&read.design), stitches(&engine));
    assert!(a.len() == b.len() && a.iter().zip(&b).all(|((k, p), (l, q))| k == l && p.distance(*q) < 1e-6), "{} and {} stitches", a.len(), b.len());
}

#[test]
fn a_piece_the_filter_can_t_write_is_a_bug_it_names() {
    assert_eq!(bug("a reason").to_string(), "error SC-E0009: The route could not be written: a reason.");
}
