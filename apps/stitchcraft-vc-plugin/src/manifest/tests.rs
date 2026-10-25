//! The manifests' tests: each is one VectorCraft accepts, made from the parameter registry in the dialog's
//! order, and the validator catches what VectorCraft refuses.

use serde_json::json;
use stitchcraft_engine::generate::FILL_METHODS;
use stitchcraft_params::{ChoiceOption, Condition, Origin, ParamSet, Stability};
use stitchcraft_plan::profiles::{BUILTIN, REFERENCE};

use super::*;
use crate::export::{AUTO, DOCUMENT, PROFILE};

fn value(manifest: &Manifest) -> Value {
    serde_json::to_value(manifest).unwrap()
}

/// `manifest` as `vc_manifest` serves it.
fn text(manifest: &Manifest) -> Vec<u8> {
    serde_json::to_vec(manifest).unwrap()
}

/// The kinds of value of `params`, by key.
fn schemas(params: &[(&'static str, Param)]) -> Vec<(&'static str, Schema)> {
    params.iter().map(|(key, param)| (*key, param.schema.clone())).collect()
}

#[test]
fn hello_manifest_is_valid() {
    assert_eq!(validate(&text(&hello())), Ok(()));
    assert_eq!(hello().id, "dev.stitchcraft.hello");
}

#[test]
fn req_vc_002_each_family_s_manifest_is_one_vectorcraft_accepts() {
    for each in Family::ALL {
        let manifest = family(*each);
        assert_eq!(validate(&text(&manifest)), Ok(()), "{each:?}");
        assert_eq!(manifest.id, format!("dev.stitchcraft.{}", each.id()));
        assert!(manifest.params.iter().all(|(key, _)| *key != SATIN_COLUMN), "{each:?}");
    }
}

#[test]
fn req_vc_011_each_exporter_s_manifest_is_one_vectorcraft_accepts() {
    let profiles: Vec<&str> = BUILTIN.iter().map(|profile| profile.id).collect();
    for format in FormatId::ALL {
        let manifest = exporter(*format);
        assert_eq!(validate(&text(&manifest)), Ok(()), "{format:?}");
        assert_eq!(manifest.id, format!("dev.stitchcraft.{}", format.extension()));
        assert_eq!((manifest.kind, manifest.extensions.as_slice()), ("exporter", &[format.extension()][..]));
        // `stitch export`'s options, as they are when it is not given them: no automatic digitizing, and the
        // machine the document keeps, else the reference machine, listed first after it.
        let machines: Vec<&str> = std::iter::once(DOCUMENT).chain(profiles.iter().copied()).collect();
        let options = vec![(AUTO, Schema::Bool { default: false }), (PROFILE, Schema::Choice { options: machines, default: DOCUMENT })];
        assert_eq!(schemas(&manifest.params), options, "{format:?}");
        assert_eq!(profiles.first(), Some(&REFERENCE.id));
    }
    // The live effects list no extensions, which a VectorCraft before 0.14 would refuse.
    assert!(Family::ALL.iter().all(|each| value(&family(*each)).get("extensions").is_none()));
    assert!(value(&hello()).get("extensions").is_none());
}

#[test]
fn each_family_s_dialog_ends_with_its_preview_levels_auto_first() {
    let levels = |each: Family| schemas(&family(each).params).last().cloned();
    let choice = |options: &[&'static str]| Some((PREVIEW_LEVEL, Schema::Choice { options: options.to_vec(), default: "auto" }));
    assert_eq!(levels(Family::Fill), choice(&["auto", "exact", "rows", "outline"]));
    assert_eq!(levels(Family::Stroke), choice(&["auto", "exact", "outline"]));
    assert_eq!(levels(Family::Satin), choice(&["auto", "exact", "outline"]));
    // It is the plug-in's, and no parameter of the registry has its name.
    assert!(PARAMETERS.iter().flat_map(|group| group.specs).all(|spec| spec.key != PREVIEW_LEVEL));
}

#[test]
fn req_vc_002_the_dialog_opens_with_the_method_and_ends_with_the_settings_every_element_shares() {
    let keys = |each: Family| -> Vec<&'static str> { family(each).params.iter().map(|(key, _)| *key).collect() };
    // VectorCraft's dialog follows the JSON text, which keeps the manifest's order.
    let text = serde_json::to_string(&family(Family::Fill)).unwrap();
    let found: Vec<Option<usize>> = keys(Family::Fill).iter().map(|key| text.find(&format!("\"{key}\":"))).collect();
    assert!(found.iter().all(Option::is_some) && found.is_sorted(), "{found:?}");
    let place = |keys: &[&str], key: &str| keys.iter().position(|k| *k == key).unwrap();
    let fill = keys(Family::Fill);
    assert_eq!(fill[..3], ["fill", "fill_method", "angle"]);
    assert!(place(&fill, "underlay_underpath") < place(&fill, "running_stitch_length_mm"));
    assert!(place(&fill, "pull_compensation_mm") < place(&fill, "ties"));
    assert_eq!(fill[fill.len() - 2..], ["max_stitch_length_mm", PREVIEW_LEVEL]);
    let stroke = keys(Family::Stroke);
    assert_eq!(stroke[..3], ["stroke_method", "repeats", "bean_stitch_repeats"]);
    assert_eq!(stroke[3], "running_stitch_length_mm");
    // Another method's settings come after those the family shares, and before those every element
    // shares, as in Ink/Stitch.
    assert!(place(&stroke, "random_stitch_length_jitter_percent") < place(&stroke, "zigzag_spacing_mm"));
    assert!(place(&stroke, "zigzag_angle") < place(&stroke, "ties"));
    let satin = keys(Family::Satin);
    assert_eq!(satin[..2], ["satin_method", "zigzag_spacing_mm"]);
    // Within a group, the registry's order.
    assert!(place(&satin, "ties") < place(&satin, "trim_after") && place(&satin, "trim_after") < place(&satin, "min_stitch_length_mm"));
}

#[test]
fn req_fill_005_the_fill_dialog_offers_every_method_but_legacy_fill() {
    // StitchCraft sews legacy fill as a tatami fill. The others come in Ink/Stitch's order.
    let method = family(Family::Fill).params.into_iter().find(|(key, _)| *key == "fill_method").map(|(_, param)| param.schema);
    let options: Vec<&str> = FILL_METHODS.iter().map(|o| o.id).filter(|id| *id != "legacy_fill").collect();
    assert_eq!(options.len(), FILL_METHODS.len() - 1);
    assert_eq!(method, Some(Schema::Choice { options, default: "tatami_fill" }));
}

#[test]
fn req_prm_005_each_family_s_dialog_has_its_own_declaration_of_a_key() {
    // A list for zigzag strokes and one length for satin columns, each with its own range and default.
    const SATIN: &[ParamSpec] = &[ParamSpec {
        key: "zigzag_spacing_mm",
        label: "Zigzag spacing",
        help: " The spacing.\n",
        kind: Kind::Length { min: 0.01, max: 10.0, empty: Empty::Never },
        default: "0.4",
        family_defaults: &[],
        group: "Spacing",
        applies_to: &[StitchType::SatinColumn],
        visible_when: None,
        stability: Stability::Stable,
        origin: Origin::InkStitch,
    }];
    const STROKE: &[ParamSpec] =
        &[ParamSpec { kind: Kind::LengthList { min: 0.1, max: 5.0 }, default: "1.5", applies_to: &[StitchType::ZigzagStitch], ..SATIN[0] }];
    let satin = ParamGroup { name: "SatinSpacingParams", help: " Satin.\n", applies_to: SATIN[0].applies_to, specs: SATIN };
    let stroke = ParamGroup { name: "StrokeSpacingParams", help: " Stroke.\n", applies_to: STROKE[0].applies_to, specs: STROKE };
    let registry = [&satin, &stroke];
    let spacing = |min, max, default| vec![("zigzag_spacing_mm", Schema::Number { min, max, default })];
    assert_eq!(schemas(&params_of(&registry, Family::Satin)), spacing(0.01, 10.0, 0.4));
    assert_eq!(schemas(&params_of(&registry, Family::Stroke)), spacing(0.1, 5.0, 1.5));
    assert_eq!(params_of(&registry, Family::Fill), []);
}

#[test]
fn req_vc_002_a_record_of_the_defaults_reads_as_no_settings_at_all() {
    // VectorCraft stores every declared parameter, at its default when the user leaves it: each must
    // read as the registry's default, as an element without settings reads it.
    for each in Family::ALL {
        for (key, schema) in schemas(&family(*each).params).into_iter().filter(|(key, _)| *key != PREVIEW_LEVEL) {
            let spec = stitchcraft_params::find_for(PARAMETERS, key, *each).unwrap().in_family(*each);
            // VectorCraft stores a number's default clamped into its range.
            let stored = match schema {
                Schema::Number { min, max, default } => default.clamp(min, max).to_string(),
                Schema::Int { min, max, default } => default.clamp(min, max).to_string(),
                Schema::Bool { default } => default.to_string(),
                Schema::Choice { default, .. } => default.to_string(),
            };
            let mut set = ParamSet::new();
            set.set(key, stored.as_str());
            let read = |set: &ParamSet| spec.kind.parse(set.get(key).unwrap_or(spec.default)).map(|(value, _)| value);
            assert_eq!(read(&set), read(&ParamSet::new()), "{each:?} {key} = {stored}");
        }
    }
}

#[test]
fn each_kind_is_declared_as_one_value_or_left_out() {
    let spec = |kind: Kind, default: &'static str| ParamSpec {
        key: "p",
        label: "P",
        help: "",
        kind,
        default,
        family_defaults: &[],
        group: "",
        applies_to: &[],
        visible_when: None,
        stability: Stability::Stable,
        origin: Origin::StitchCraft,
    };
    let length = Kind::Length { min: 0.1, max: 10.0, empty: Empty::Never };
    assert_eq!(schema(&spec(length, "2"), &[]), Some(Schema::Number { min: 0.1, max: 10.0, default: 2.0 }));
    let optional = Kind::Length { min: 0.1, max: 10.0, empty: Empty::AtMostZero };
    assert_eq!(schema(&spec(optional, ""), &[]), Some(Schema::Number { min: 0.0, max: 10.0, default: 0.0 }));
    assert_eq!(schema(&spec(Kind::Length { min: 0.0, max: 10.0, empty: Empty::Blank }, ""), &[]), None);
    assert_eq!(schema(&spec(Kind::Angle, "90"), &[]), Some(Schema::Number { min: -180.0, max: 180.0, default: 90.0 }));
    let tilt = Kind::BoundedAngle { min: -89.0, max: 89.0 };
    assert_eq!(schema(&spec(tilt, "30"), &[]), Some(Schema::Number { min: -89.0, max: 89.0, default: 30.0 }));
    assert_eq!(schema(&spec(Kind::Percent { min: 0.0, max: 50.0 }, "15"), &[]), Some(Schema::Number { min: 0.0, max: 50.0, default: 15.0 }));
    assert_eq!(schema(&spec(Kind::Number { min: 0.01, max: 9.0 }, "4"), &[]), Some(Schema::Number { min: 0.01, max: 9.0, default: 4.0 }));
    assert_eq!(schema(&spec(Kind::Count { min: 1, max: 9 }, "2"), &[]), Some(Schema::Int { min: 1, max: 9, default: 2 }));
    assert_eq!(schema(&spec(Kind::Toggle, "true"), &[]), Some(Schema::Bool { default: true }));
    let pair = Kind::LengthPair { min: 0.0, max: 5.0, optional: false };
    assert_eq!(schema(&spec(pair, "0.4"), &[]), Some(Schema::Number { min: 0.0, max: 5.0, default: 0.4 }));
    assert_eq!(schema(&spec(pair, "0.4 0.2"), &[]), None);
    assert_eq!(schema(&spec(Kind::LengthPair { min: 0.0, max: 5.0, optional: true }, ""), &[]), None);
    let percents = Kind::PercentPair { min: -100.0, max: 100.0, optional: false };
    assert_eq!(schema(&spec(percents, "10"), &[]), Some(Schema::Number { min: -100.0, max: 100.0, default: 10.0 }));
    assert_eq!(schema(&spec(percents, "10 20"), &[]), None);
    let list = Kind::PercentList { min: 0.0, max: 50.0 };
    assert_eq!(schema(&spec(list, "15"), &[]), Some(Schema::Number { min: 0.0, max: 50.0, default: 15.0 }));
    assert_eq!(schema(&spec(list, "15 20"), &[]), None);
    assert_eq!(schema(&spec(Kind::LengthList { min: 0.1, max: 9.0 }, "2.5"), &[]), Some(Schema::Number { min: 0.1, max: 9.0, default: 2.5 }));
    assert_eq!(schema(&spec(Kind::LengthList { min: 0.1, max: 9.0 }, "1 2"), &[]), None);
    assert_eq!(schema(&spec(Kind::CountList { min: 0, max: 9 }, "0"), &[]), Some(Schema::Int { min: 0, max: 9, default: 0 }));
    assert_eq!(schema(&spec(Kind::CountList { min: 0, max: 9 }, "1 2"), &[]), None);
    assert_eq!(schema(&spec(Kind::AngleList { optional: true }, ""), &[]), None);
    assert_eq!(schema(&spec(Kind::Seed, ""), &[]), None);
    assert_eq!(schema(&spec(Kind::Text { max_bytes: 9 }, ""), &[]), None);
    // A choice leaves out an option a text is shown for alone, and is left out when that is its default.
    const OPTIONS: &[ChoiceOption] = &[ChoiceOption { id: "half", label: "Half" }, ChoiceOption { id: "custom", label: "Custom" }];
    let choice = spec(Kind::Choice { options: OPTIONS }, "half");
    let shape = ParamSpec { key: "shape", visible_when: Some(Condition { key: "p", any_of: &["custom"] }), ..spec(Kind::Text { max_bytes: 9 }, "") };
    assert_eq!(schema(&choice, &[choice, shape]), Some(Schema::Choice { options: vec!["half"], default: "half" }));
    let both = ParamSpec { visible_when: Some(Condition { key: "p", any_of: &["half", "custom"] }), ..shape };
    assert_eq!(schema(&choice, &[choice, both]), Some(Schema::Choice { options: vec!["half", "custom"], default: "half" }));
    let size = ParamSpec { kind: Kind::Count { min: 1, max: 9 }, ..shape };
    assert_eq!(schema(&choice, &[choice, size]), Some(Schema::Choice { options: vec!["half", "custom"], default: "half" }));
    assert_eq!(schema(&spec(Kind::Choice { options: OPTIONS }, "custom"), &[shape]), None);
}

#[test]
fn only_stroke_to_satin_names_its_budget() {
    // A light budget is the default, left out, so the other manifests read as they did before VectorCraft
    // 0.17 named budgets.
    let light = [hello(), settings(), testsheet()]
        .into_iter()
        .chain(Family::ALL.iter().map(|each| family(*each)))
        .chain(FormatId::ALL.iter().map(|format| exporter(*format)));
    for manifest in light {
        assert_eq!((manifest.budget, value(&manifest).get("budget")), (BudgetClass::Light, None), "{}", manifest.id);
    }
    assert_eq!(value(&stroke_to_satin())["budget"], json!("heavy"));
}

/// A DST exporter's manifest that asks for a heavy budget.
fn exporting_with_budget() -> Vec<u8> {
    let mut m = value(&exporter(FormatId::Dst));
    m["budget"] = json!("heavy");
    serde_json::to_vec(&m).unwrap()
}

#[test]
fn validation_catches_what_vectorcraft_rejects() {
    let base = value(&hello());
    let with = |key: &str, v: Value| {
        let mut m = base.clone();
        m[key] = v;
        serde_json::to_vec(&m).unwrap()
    };
    assert!(validate(&with("id", json!("has space"))).is_err());
    assert!(validate(&with("name", json!(""))).is_err());
    assert!(validate(&with("author", json!("x".repeat(limits::MAX_AUTHOR_BYTES + 1)))).is_err());
    let large = validate(&with("description", json!("x".repeat(limits::MAX_MANIFEST_BYTES))));
    assert!(large.is_err_and(|why| why.starts_with("manifest is ")));
    // White space after the JSON counts too: 64 KiB is the most.
    let padded = |len: usize| {
        let mut text = serde_json::to_vec(&base).unwrap();
        text.resize(len, b' ');
        validate(&text)
    };
    assert_eq!(padded(limits::MAX_MANIFEST_BYTES), Ok(()));
    assert!(padded(limits::MAX_MANIFEST_BYTES + 1).is_err_and(|why| why.starts_with("manifest is 65537 bytes")));
    assert!(validate(&with("kind", json!("tool"))).is_err());
    // An exporter lists 1 to 8 extensions, each 1 to 16 lower-case letters and digits, and only an exporter
    // lists any.
    let exporting = |extensions: Value| {
        let mut m = value(&exporter(FormatId::Dst));
        m["extensions"] = extensions;
        serde_json::to_vec(&m).unwrap()
    };
    let nine = json!(["a", "b", "c", "d", "e", "f", "g", "h", "i"]);
    for bad in
        [json!([]), json!(["DST"]), json!([".dst"]), json!([""]), json!(["abcdefghijklmnopq"]), json!(["dst", "dst"]), nine, json!("dst"), json!([1])]
    {
        assert!(validate(&exporting(bad.clone())).is_err(), "{bad}");
    }
    assert_eq!(validate(&exporting(json!(["dst", "dst2", "abcdefghijklmnop"]))), Ok(()));
    assert_eq!(validate(&exporting(json!(["a", "b", "c", "d", "e", "f", "g", "h"]))), Ok(()), "8, the most");
    assert!(validate(&with("kind", json!("exporter"))).is_err(), "an exporter without extensions");
    // From 0.17 a filter may ask for a heavy budget, and nothing else may.
    let filter = |budget: Value| {
        let mut m = value(&settings());
        m["budget"] = budget;
        serde_json::to_vec(&m).unwrap()
    };
    assert_eq!(validate(&filter(json!("heavy"))), Ok(()));
    assert_eq!(validate(&filter(json!("light"))), Ok(()));
    for bad in [json!("medium"), json!(null), json!(true)] {
        assert!(validate(&filter(bad.clone())).is_err(), "{bad}");
    }
    assert!(validate(&with("budget", json!("heavy"))).is_err_and(|why| why.contains("only a filter")), "a heavy live effect");
    assert!(validate(&exporting_with_budget()).is_err_and(|why| why.contains("only a filter")), "a heavy exporter");
    assert_eq!(validate(&with("budget", json!("light"))), Ok(()));
    assert!(validate(&with("extensions", json!(["pes"]))).is_err(), "a live effect with extensions");
    assert_eq!(validate(&with("extensions", json!([]))), Ok(()));
    // A manifest may leave its parameters out, as VectorCraft reads it, but they are an object.
    let mut bare = base.clone();
    bare.as_object_mut().unwrap().remove("params");
    assert_eq!(validate(&serde_json::to_vec(&bare).unwrap()), Ok(()));
    assert!(validate(&with("params", json!([]))).is_err_and(|why| why.contains("expected a JSON object")));
    assert!(validate(&with("params", json!({"preview": {"type": "bool", "default": false}}))).is_err());
    let too_many: serde_json::Map<String, Value> =
        (0..=limits::MAX_PARAMS).map(|i| (format!("p{i}"), json!({"type": "bool", "default": false}))).collect();
    assert!(validate(&with("params", Value::Object(too_many.clone()))).is_err());
    let most: serde_json::Map<String, Value> = too_many.into_iter().take(limits::MAX_PARAMS).collect();
    assert_eq!(validate(&with("params", Value::Object(most))), Ok(()));
    for bad in [
        json!({"type": "number", "min": 2, "max": 1, "default": 1}),
        json!({"type": "number", "min": 0, "default": 1}),
        json!({"type": "int", "min": 5, "max": 1}),
        json!({"type": "int", "default": 1.5}),
        json!({"type": "bool", "default": "yes"}),
        json!({"type": "choice", "options": []}),
        json!({"type": "choice", "options": ["a", ""]}),
        json!({"type": "choice", "options": ["a", "b"], "default": "c"}),
        json!({"type": "choice", "options": ["a", 1]}),
        json!({"type": "choice", "options": (0..=limits::MAX_OPTIONS).map(|i| format!("o{i}")).collect::<Vec<_>>()}),
        json!({"type": "text"}),
    ] {
        assert!(validate(&with("params", json!({"p": bad.clone()}))).is_err(), "{bad}");
    }
    for good in [
        json!({"type": "number", "min": 0, "max": 1, "default": 1}),
        json!({"type": "number", "min": 1, "max": 1, "default": 1}),
        json!({"type": "int"}),
        json!({"type": "int", "min": 1, "max": 1}),
        json!({"type": "choice", "options": (1..=limits::MAX_OPTIONS).map(|i| format!("o{i}")).collect::<Vec<_>>()}),
        json!({"type": "bool"}),
        json!({"type": "choice", "options": ["a", "b"], "default": "b"}),
    ] {
        assert_eq!(validate(&with("params", json!({"p": good.clone()}))), Ok(()), "{good}");
    }
}
