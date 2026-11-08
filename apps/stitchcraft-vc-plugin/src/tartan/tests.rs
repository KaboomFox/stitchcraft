//! StitchCraft Tartan (`REQ-VC-021`): its dialog, what it gives back for a selection, and the sett, kept as
//! VectorCraft keeps it, sewing as the code says.

use serde_json::{Value, json};
use stitchcraft_core::Code;

use super::*;
use crate::manifest::{BudgetClass, tartan, validate};
use crate::testing::{document, kept, sewn, square};

/// A record of StitchCraft's effect `family` with `params`, as VectorCraft stores one.
fn record(family: &str, params: &Value, visible: bool) -> Value {
    json!({"id": format!("plugin.dev.stitchcraft.{family}"), "params": params, "visible": visible})
}

/// The square `id`, 72 points (25.4 mm) a side, filled red, with `effects`.
fn object(id: u64, effects: &Value) -> Value {
    let red = json!({"type": "solid", "color": {"model": "rgb", "r": 0.9, "g": 0.1, "b": 0.1}});
    json!({"id": id, "type": "path", "path": square(72.0), "fillRule": "nonzero", "fills": [red], "strokes": [], "effects": effects, "opacity": 1.0})
}

/// What the filter gives back for `objects`, with the rows `set`, run as VectorCraft 0.15 runs it.
fn ran(objects: &[Value], set: &Value) -> Value {
    let input = serde_json::to_vec(&json!({"objects": objects})).unwrap();
    let mut params = set.clone();
    params["_context"] = json!({"mode": "filter", "artboard": [0, 0, 612, 792]});
    serde_json::from_slice(&run(&input, &serde_json::to_vec(&params).unwrap()).unwrap()).unwrap()
}

/// The error `out` declines with.
fn refusal(out: &Value) -> &str {
    out["error"].as_str().unwrap()
}

/// The settings the object `n` of `out` is given in its record of the fill effect: its `fill_method` and its
/// `tartan`.
fn given_sett(out: &Value, n: usize) -> (String, String) {
    let effects = out["objects"][n]["effects"].as_array().unwrap();
    let fill = effects.iter().find(|e| e["id"] == "plugin.dev.stitchcraft.fill").unwrap();
    (fill["params"]["fill_method"].as_str().unwrap().to_string(), fill["params"]["tartan"].as_str().unwrap().to_string())
}

#[test]
fn req_vc_021_the_dialog_takes_a_code_a_thread_s_width_and_how_the_sett_is_laid() {
    let manifest = tartan();
    validate(&serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert_eq!((manifest.id.as_str(), manifest.kind, manifest.budget), ("dev.stitchcraft.tartan", "filter", BudgetClass::Light));
    let keys: Vec<&str> = manifest.params.iter().map(|(key, _)| *key).collect();
    assert_eq!(keys, ["code", "thread_width_mm", "rotate", "scale", "offset_x", "offset_y", "min_stripe_width"]);
    let schema = |key: &str| manifest.params.iter().find(|(k, _)| *k == key).map(|(_, param)| param.schema.clone()).unwrap();
    assert_eq!(schema("code"), Schema::Text { default: DEFAULT_PALETTE, max_length: 4_096 });
    // Ink/Stitch's editor starts from a thread 0.2 mm wide, unturned, unscaled, unmoved, with runs up to 1 mm.
    assert_eq!(schema("thread_width_mm"), Schema::Number { min: 0.01, max: 10.0, default: 0.2 });
    assert_eq!(schema("rotate"), Schema::Number { min: -180.0, max: 180.0, default: 0.0 });
    assert_eq!(schema("scale"), Schema::Number { min: 1.0, max: 1_000.0, default: 100.0 });
    assert_eq!(schema("offset_y"), Schema::Number { min: -1_000.0, max: 1_000.0, default: 0.0 });
    assert_eq!(schema("min_stripe_width"), Schema::Number { min: 0.0, max: 100.0, default: 1.0 });
    let units: Vec<Option<&str>> = manifest.params.iter().map(|(_, param)| param.ui.unit).collect();
    assert_eq!(units, [None, Some("mm"), Some("°"), Some("%"), Some("mm"), Some("mm"), Some("mm")]);
    assert!(manifest.params.iter().all(|(_, param)| param.ui.help_url.as_deref().is_some_and(|url| url.ends_with(PAGE))));
}

#[test]
fn req_vc_021_a_fill_s_record_gets_the_sett_and_keeps_its_other_settings() {
    let effects = json!([{"id": "distort.roughen", "params": {}}, record("fill", &json!({"angle": 30, "fill_method": "auto_fill", "preview_level": "outline"}), true)]);
    let set = json!({"code": "B/24 W4 B24 R2 K24 G/24", "thread_width_mm": 0.25, "rotate": 30, "scale": 150, "offset_x": 1.5, "offset_y": -2, "min_stripe_width": 0.5});
    let out = ran(&[object(2, &effects)], &set);
    let (method, sett) = given_sett(&out, 0);
    assert_eq!(method, "tartan_fill");
    assert_eq!(
        sett,
        r#"{"symmetry": true, "equal_warp_weft": true, "rotate": 30.0, "scale": 150.0, "offset_x": 1.5, "offset_y": -2.0, "palette": "(#0000ff)/6 (#ffffff)1 (#0000ff)6 (#ff0000)0.5 (#000000)6 (#289c18)/6", "min_stripe_width": 0.5}"#
    );
    // Its other settings, and the other effect before it, are as VectorCraft gave them.
    let effects = out["objects"][0]["effects"].as_array().unwrap();
    assert_eq!(effects[0], json!({"id": "distort.roughen", "params": {}}));
    assert_eq!((effects[1]["params"]["angle"].clone(), effects[1]["params"]["preview_level"].clone()), (json!(30), json!("outline")));
    assert_eq!(effects[1]["visible"], json!(true));
}

#[test]
fn req_vc_021_a_path_without_a_stitchcraft_effect_gets_the_fill_effect_and_others_stay() {
    let roughen = json!({"id": "distort.roughen", "params": {}});
    let objects = [
        object(2, &json!([roughen])),
        object(3, &json!([record("satin", &json!({}), true)])),
        // The fill effect turned off, before the stroke effect: the first StitchCraft effect on is the stroke's.
        object(4, &json!([record("fill", &json!({}), false), record("stroke", &json!({}), true)])),
        object(5, &json!([record("fill", &json!({}), false)])),
        object(6, &json!([record("auto", &json!({}), true)])),
    ];
    let out = ran(&objects, &json!({"code": "(#ff0000)/4 (#0000ff)/2"}));
    let ids: Vec<u64> = out["objects"].as_array().unwrap().iter().map(|o| o["id"].as_u64().unwrap()).collect();
    assert_eq!(ids, [2, 3, 4, 5, 6]);
    // The first gets the fill effect after its own, with the 2 settings, which VectorCraft fills in.
    let effects = out["objects"][0]["effects"].as_array().unwrap();
    assert_eq!(effects.len(), 2);
    assert_eq!(effects[0], roughen);
    assert_eq!(effects[1]["id"], "plugin.dev.stitchcraft.fill");
    let (method, sett) = given_sett(&out, 0);
    assert_eq!(method, "tartan_fill");
    assert!(sett.contains(r#""palette": "(#ff0000)/4 (#0000ff)/2""#), "{sett}");
    // The others stay as they are: given back without their effects.
    for n in 1..5 {
        assert_eq!(out["objects"][n].as_object().unwrap().keys().collect::<Vec<_>>(), ["id"], "{n}");
    }
}

#[test]
fn req_vc_021_the_sett_kept_sews_as_its_code_says() {
    let square = object(2, &json!([record("fill", &json!({"fill_underlay": false}), true)]));
    let out = ran(std::slice::from_ref(&square), &json!({"code": "R/8 K4 G/12", "thread_width_mm": 0.5}));
    // VectorCraft keeps what the filter leaves out as it was: here all but the effects.
    let mut whole = square;
    whole["effects"] = out["objects"][0]["effects"].clone();
    let (file, said) = sewn(&document(&[kept(&whole, 2)]));
    assert_eq!(said, Vec::<Code>::new());
    // The same square with the setting written by hand sews alike.
    let by_hand = record(
        "fill",
        &json!({"fill_underlay": false, "fill_method": "tartan_fill", "tartan": r#"{"palette": "(#ff0000)/4 (#000000)2 (#289c18)/6"}"#}),
        true,
    );
    let (expected, _) = sewn(&document(&[kept(&object(2, &json!([by_hand])), 2)]));
    assert_eq!(file, expected);
}

#[test]
fn diag_sc_e0821_a_code_that_can_t_be_read_or_names_unknown_letters_changes_nothing() {
    let objects = [object(2, &json!([]))];
    let refused = |code: &str| refusal(&ran(&objects, &json!({"code": code}))).to_string();
    assert_eq!(
        refused("B1.2.3"),
        "error SC-E0821: The sett's code can't be read: the count `1.2.3` of a stripe isn't a number. Write each stripe as a colour in brackets \
         and its width, `(#0000ff)/4 (#ffffff)1`, or as the register's colour letters and a count of threads, `B/24 W4`."
    );
    assert!(refused("B24 X4 QZ2").starts_with(
        "error SC-E0821: The sett's code names `X` and `QZ`, which the Scottish Register of Tartans uses for no colour, and the fill would leave unsewn."
    ));
    assert!(refused("").starts_with("error SC-E0821: The sett's code gives no stripe."));
    // A `|` with no stripe on either side gives none either.
    assert!(refused("|").starts_with("error SC-E0821: The sett's code gives no stripe."));
    // A weft alone is a stripe.
    assert!(ran(&objects, &json!({"code": "| (#ff0000)4"})).get("error").is_none());
}

#[test]
fn diag_sc_e0821_a_sett_longer_than_the_fill_s_sett_holds_changes_nothing() {
    // 400 stripes of a thread's width: the setting is longer than 4,096 characters.
    let code: Vec<&str> = std::iter::repeat_n("R1 K1", 200).collect();
    let refused = refusal(&ran(&[object(2, &json!([]))], &json!({"code": code.join(" ")}))).to_string();
    assert!(refused.starts_with("error SC-E0821: The sett's setting is 5"), "{refused}");
    assert!(refused.contains(" characters long, and the fill's Sett holds at most 4096."), "{refused}");
}

#[test]
fn a_setting_fits_the_sett_s_row_up_to_its_4096_characters() {
    assert!(fits(&"é".repeat(4_096)));
    assert!(!fits(&"é".repeat(4_097)));
}

#[test]
fn diag_sc_e0821_a_selection_without_a_path_that_takes_the_sett_changes_nothing() {
    assert_eq!(
        refusal(&ran(&[], &json!({}))),
        "error SC-E0821: Nothing is selected to take a sett. Select the paths to sew as a tartan, and put the fill effect on the paths \
         themselves rather than on their group."
    );
    let satin = object(3, &json!([record("satin", &json!({}), true)]));
    assert!(refusal(&ran(&[satin.clone(), satin], &json!({}))).starts_with("error SC-E0821: None of the 2 objects selected takes a sett: "));
}

#[test]
fn diag_sc_e0813_before_vectorcraft_0_15_the_filter_declines() {
    let mut old = object(2, &json!([]));
    old.as_object_mut().unwrap().remove("effects");
    assert!(refusal(&ran(&[old], &json!({}))).starts_with("error SC-E0813: This VectorCraft keeps no effect record a filter gives"));
}

#[test]
fn parameters_that_aren_t_json_are_bad_input() {
    assert!(matches!(run(br#"{"objects": []}"#, b"not json"), Err(RunError::BadInput)));
    // The dialog's defaults set Ink/Stitch's default sett.
    let (_, sett) = given_sett(&ran(&[object(2, &json!([]))], &json!({})), 0);
    assert!(sett.contains(r#""palette": "(#101010)/5 (#ffffff)/?5""#), "{sett}");
}

#[test]
fn a_record_the_filter_can_t_write_is_a_bug_it_says_as_one() {
    assert_eq!(bug("a reason").to_string(), "error SC-E0009: The sett could not be written: a reason.");
}
