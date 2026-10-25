//! Test Sheet (`REQ-VC-015`): each sheet drawn as a design, drawn into a document as VectorCraft keeps
//! what the filter gives back, sews as the sheet's design moved.

use serde_json::{Value, json};
use stitchcraft_core::Budget;
use stitchcraft_params::Family;

use super::*;
use crate::manifest::{BudgetClass, testsheet, validate};
use crate::testing::{document, kept, planned, sewn};

/// The parameters VectorCraft passes the filter for `sheet`, with `context`.
fn params(sheet: &str, context: &Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"sheet": sheet, "_context": context})).unwrap()
}

/// What VectorCraft 0.16 and later tell a filter of a new Letter-sized document.
fn letter() -> Value {
    json!({"mode": "filter", "bounds": null, "artboard": [0, 0, 612, 792], "documentData": null})
}

/// What the filter gives back for `sheet`, given the selected `objects` and `context`.
fn ran(sheet: &str, objects: &Value, context: &Value) -> Value {
    let input = serde_json::to_vec(&json!({ "objects": objects })).unwrap();
    serde_json::from_slice(&run(&input, &params(sheet, context)).unwrap()).unwrap()
}

/// The document VectorCraft keeps of the objects `out` the filter drew: each new, above the one before.
fn drawn(out: &Value) -> Vec<u8> {
    let objects = out["objects"].as_array().unwrap_or_else(|| panic!("nothing drawn: {out}"));
    document(&objects.iter().enumerate().map(|(k, object)| kept(object, 10 + k as u64)).collect::<Vec<_>>())
}

#[test]
fn req_vc_015_each_sheet_drawn_sews_as_its_design_moved_to_the_artboard() {
    let mut drawn_sheets = Vec::new();
    let mut kinds = Vec::new();
    for sheet in sheets() {
        let out = ran(sheet.id, &json!([]), &letter());
        // Each element a path, or a compound path when it has more than one subpath.
        for object in out["objects"].as_array().unwrap() {
            let subpaths = object["path"]["subpaths"].as_array().unwrap().len();
            kinds.push((subpaths > 1, object["type"].as_str().unwrap().to_string()));
        }
        let document = drawn(&out);
        // As `stitch export` sews the document, and as the engine sews the sheet's design, moved by whole
        // millimetres to the middle of the Letter page: 107.95 mm across and 139.7 mm down.
        let moved = placed(sheet, Point::new(107.95, 139.7).ok()).unwrap();
        assert_eq!(sewn(&document), planned(&moved), "{}", sheet.id);
        let read = stitchcraft_vectorcraft::read(&document, &Budget::DEFAULT).unwrap().design;
        let middle = read.bounds().unwrap().center();
        assert!((middle.x() - 107.95).abs() <= 0.5 && (middle.y() - 139.7).abs() <= 0.5, "{}: {middle:?}", sheet.id);
        // Each element an object of its family, named by its id.
        let ids: Vec<&str> = read.elements().iter().map(|e| e.id.as_str().rsplit(':').next().unwrap_or_default()).collect();
        let families: Vec<&str> = moved.elements().iter().map(|e| stitch_type_of(e, &moved.settings).unwrap().family().id()).collect();
        assert_eq!(ids, families, "{}", sheet.id);
        drawn_sheets.push(sheet.id);
    }
    assert_eq!(drawn_sheets, ["TS-02B", "TS-03", "TS-04", "TS-05", "TS-06", "TS-07", "TS-08", "TS-09"]);
    // The sheets draw both.
    assert!(kinds.iter().all(|(several, kind)| kind == if *several { "compound" } else { "path" }), "{kinds:?}");
    assert!(kinds.iter().any(|(several, _)| *several) && kinds.iter().any(|(several, _)| !several), "{kinds:?}");
}

#[test]
fn req_vc_015_without_an_artboard_a_sheet_is_drawn_where_stitch_testsheet_draws_it() {
    let sheet = sheets().find(|sheet| sheet.id == "TS-05").unwrap();
    let context = json!({"mode": "filter", "bounds": null, "artboard": null, "documentData": {"kept": true}});
    let document = drawn(&ran(sheet.id, &json!([]), &context));
    assert_eq!(sewn(&document), planned(&sheet.design().unwrap().unwrap()));
}

#[test]
fn req_vc_015_the_objects_selected_stay_as_they_are_and_the_sheet_goes_above_them() {
    let selected = json!([{"id": 3, "type": "path", "path": {"subpaths": []}}, {"id": 8, "type": "compound", "path": {"subpaths": []}}]);
    let out = ran("TS-07", &selected, &letter());
    let objects = out["objects"].as_array().unwrap();
    assert_eq!(objects[..2], [json!({"id": 3}), json!({"id": 8})]);
    // Then the sheet's elements, each a new object, named by its id: a fill and its outline, 3 times over.
    let names: Vec<&str> = objects[2..].iter().map(|o| o["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 6, "{names:?}");
    assert!(objects[2..].iter().all(|o| o.get("id").is_none() && o["name"].as_str().is_some_and(|name| name.starts_with("ts07:"))));
    let fill = &objects[2];
    assert_eq!((fill["fillRule"].as_str(), fill["strokes"].clone()), (Some("evenodd"), Value::Null));
    assert_eq!(fill["effects"][0]["id"], "plugin.dev.stitchcraft.fill");
}

#[test]
fn req_vc_015_a_record_holds_each_setting_as_its_dialog_row_takes_it() {
    let schemas = |family: Family| declared().into_iter().find(|(each, _)| *each == family).unwrap().1;
    let set = |pairs: &[(&str, &str)]| pairs.iter().copied().collect::<ParamSet>();
    let held = record_params(
        Family::Stroke,
        &set(&[("running_stitch_length_mm", "2.5"), ("bean_stitch_repeats", "2"), ("trim_after", "true"), ("lock_start", "back_forth")]),
        &schemas(Family::Stroke),
    )
    .unwrap();
    assert_eq!(
        Value::Object(held),
        json!({"bean_stitch_repeats": 2, "lock_start": "back_forth", "running_stitch_length_mm": 2.5, "trim_after": true})
    );
    // A satin column's `satin_column` is its effect's to set.
    assert_eq!(record_params(Family::Satin, &set(&[("satin_column", "true")]), &schemas(Family::Satin)), Ok(Map::new()));
    // A setting the effect doesn't declare, a value of another kind, and one out of its row's range: each
    // VectorCraft would drop, refuse or change.
    let refused = |pairs: &[(&str, &str)]| record_params(Family::Stroke, &set(pairs), &schemas(Family::Stroke)).unwrap_err();
    assert_eq!(refused(&[("row_spacing_mm", "0.3")]), "its setting `row_spacing_mm = 0.3` is no setting of its stroke effect's record");
    assert_eq!(
        refused(&[("running_stitch_length_mm", "long")]),
        "its setting `running_stitch_length_mm = long` is no value of its row in its stroke effect's record"
    );
    assert!(refused(&[("running_stitch_length_mm", "1000")]).contains("is no value of its row in"));
    // A value its row takes that doesn't read back as the setting, as from a row of another kind than the
    // setting's: the record would sew otherwise.
    let numbered = [("bean_stitch_repeats", Schema::Number { min: 0.0, max: 10.0, default: 0.0 })];
    assert_eq!(
        record_params(Family::Stroke, &set(&[("bean_stitch_repeats", "2.5")]), &numbered),
        Err("its setting `bean_stitch_repeats = 2.5` doesn't read back from its stroke effect's record".to_string())
    );
}

#[test]
fn diag_sc_e0813_before_0_16_the_test_sheet_is_not_drawn() {
    // A 0.15 with nothing selected gives what a 0.14 gives: no `documentData`.
    let out = ran("TS-03", &json!([]), &json!({"mode": "filter", "bounds": null, "artboard": [0, 0, 612, 792]}));
    let error = out["error"].as_str().unwrap();
    assert!(
        error.starts_with("error SC-E0813: This VectorCraft doesn't tell a filter") && error.ends_with("Update VectorCraft to 0.16 or later."),
        "{error}"
    );
    assert_eq!(out.as_object().map(|out| out.len()), Some(1), "only the error: {out}");
}

#[test]
fn a_sheet_the_dialog_does_not_offer_is_bad_input() {
    let input = serde_json::to_vec(&json!({"objects": []})).unwrap();
    for sheet in ["TS-01", "TS-10A", "TS-99", ""] {
        assert_eq!(run(&input, &params(sheet, &letter())), Err(RunError::BadInput), "{sheet}");
    }
    assert_eq!(run(&input, b"{"), Err(RunError::BadInput));
}

#[test]
fn req_vc_015_the_filter_s_manifest_offers_the_sheets_drawn_as_designs() {
    let manifest = testsheet();
    assert_eq!(validate(&serde_json::to_vec(&manifest).unwrap()), Ok(()));
    assert_eq!((manifest.id.as_str(), manifest.kind, manifest.budget), ("dev.stitchcraft.testsheet", "filter", BudgetClass::Light));
    let [(SHEET, sheet)] = manifest.params.as_slice() else { panic!("{:?}", manifest.params) };
    let ids = vec!["TS-02B", "TS-03", "TS-04", "TS-05", "TS-06", "TS-07", "TS-08", "TS-09"];
    assert_eq!(sheet.schema, Schema::Choice { options: ids, default: "TS-02B" });
    // Its help names what each sheet tests, from the sheets themselves.
    let help = sheet.ui.help.as_deref().unwrap();
    assert!(sheets().all(|sheet| help.contains(&format!("{}: {}", sheet.id, sheet.title))), "{help}");
    assert!(sheet.ui.help_url.as_deref().is_some_and(|url| url.ends_with(PAGE)), "{:?}", sheet.ui.help_url);
}
