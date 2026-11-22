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
fn req_vc_035_each_effect_declares_what_an_expanded_part_sets() {
    // VectorCraft admits a filter's new effect record with the parameters its manifest takes, so each
    // effect declares the settings an expanded part carries (docs/src/design/image-to-embroidery.md › Expand).
    for each in Family::ALL {
        let manifest = family(*each);
        assert!(manifest.params.iter().any(|(key, _)| *key == "digitized_as"), "{each:?}");
        assert_eq!(validate(&text(&manifest)), Ok(()), "{each:?}");
    }
    assert!(family(Family::Fill).params.iter().any(|(key, _)| *key == stitchcraft_engine::generators::tatami::TURN_STITCHES));
}

#[test]
fn req_vc_011_each_exporter_s_manifest_is_one_vectorcraft_accepts() {
    let profiles: Vec<&str> = BUILTIN.iter().map(|profile| profile.id).collect();
    for format in FormatId::ALL {
        let manifest = exporter(*format);
        assert_eq!(validate(&text(&manifest)), Ok(()), "{format:?}");
        assert_eq!(manifest.id, format!("dev.stitchcraft.{}", format.extension()));
        assert_eq!((manifest.kind, manifest.extensions.as_slice()), ("exporter", &[format.extension()][..]));
        // `stitch export`'s options, as they are when it is not given them: automatic digitizing, as a
        // document is digitized by default (`REQ-VC-036`), and the machine the document keeps, else the
        // reference machine, listed first after it.
        let machines: Vec<&str> = std::iter::once(DOCUMENT).chain(profiles.iter().copied()).collect();
        let options = vec![(AUTO, Schema::Bool { default: true }), (PROFILE, Schema::Choice { options: machines, default: DOCUMENT })];
        assert_eq!(schemas(&manifest.params), options, "{format:?}");
        assert_eq!(profiles.first(), Some(&REFERENCE.id));
    }
    // The live effects list no extensions, which a VectorCraft before 0.14 would refuse.
    assert!(Family::ALL.iter().all(|each| value(&family(*each)).get("extensions").is_none()));
    assert!(value(&hello()).get("extensions").is_none() && value(&auto()).get("extensions").is_none());
}

#[test]
fn req_vc_016_the_auto_effect_s_dialog_holds_the_rules_settings_and_its_preview_levels() {
    use stitchcraft_engine::auto::{
        ADVANCED, AutoParams, FILL_ANGLE, FILL_OUTLINE, FILL_UNEVEN, IGNORE_BACKGROUND, MIN_AREA, OVERLAP, PREFER, STITCH, TAPER,
    };

    let manifest = auto();
    assert_eq!(validate(&text(&manifest)), Ok(()));
    assert_eq!((manifest.id.as_str(), manifest.kind), ("dev.stitchcraft.auto", "effect"));
    let keys: Vec<&str> = manifest.params.iter().map(|(key, _)| *key).collect();
    let shape = [STITCH, PREFER, FILL_UNEVEN, FILL_ANGLE, FILL_OUTLINE];
    let neighbours = [OVERLAP, "auto_pull_compensation_mm", TAPER, IGNORE_BACKGROUND];
    let base = ["auto_base_layer", "auto_base_stitch_length_mm", "auto_base_keep_holes"];
    assert_eq!(keys, [&shape[..], &neighbours[..], &base[..], &[PREVIEW_LEVEL]].concat());
    // The advanced settings, the rules' bounds (the least unevenness of a thin line that merges among them, M13.18, and the
    // least likeness that merging keeps, M13.44), the base layer's offset, the 2 limits of the cover's check and the 4
    // of how thick the design sews (M13.29) with its cap (M13.35), the switch that sews the details last (M13.40), the smoothing of outlines (M13.52), the widest split satin column (M13.54), the overlap under a part's column (M13.55) and where a column's crowded points move in (M13.58), have no row: the dialog holds 12 rows, one fewer than the 13 that made it 3 px taller than its window. The
    // outline's switch has a row (M13.28), and the offset, the base layer's finest adjustment, gave it its place.
    let advanced: Vec<&str> = AutoParams::SPECS.iter().filter(|spec| spec.group == ADVANCED).map(|spec| spec.key).collect();
    assert_eq!(advanced.len(), 23);
    assert!(advanced.contains(&MIN_AREA) && advanced.iter().all(|key| !keys.contains(key)), "{advanced:?}");
    assert_eq!(keys.len() + advanced.len(), AutoParams::SPECS.len() + 1);
    // Each setting's row has the registry's label, unit, section, help and page, and always shows: the
    // dialog has no method.
    for (key, param) in manifest.params.iter().filter(|(key, _)| *key != PREVIEW_LEVEL) {
        let spec = stitchcraft_params::find(PARAMETERS, key).unwrap();
        assert_eq!((param.ui.label, param.ui.group, param.ui.unit), (Some(spec.label), Some(spec.group), dialog::unit(spec.kind)), "{key}");
        // A choice's help goes on to say what its options mean, which the drop-down list does not show.
        let (help, plain) = (param.ui.help.clone().unwrap(), dialog::help(&spec.help()).unwrap());
        let choice = matches!(spec.kind, Kind::Choice { .. });
        let rest = help.strip_prefix(&plain).unwrap();
        assert_eq!((rest.starts_with("\n\nOne of "), rest.is_empty()), (choice, !choice), "{key}: {help}");
        assert_eq!(param.ui.help_url, Some(dialog::page(&format!("user/reference/params/auto.html#{key}"))), "{key}");
        assert!(param.ui.visible_when.is_empty(), "{key}");
    }
    // The overlap is a number of millimetres in its range, the stitch a choice, the preview a choice of 3 levels.
    let schema = |key: &str| schemas(&manifest.params).into_iter().find(|(k, _)| *k == key);
    assert_eq!(schema(OVERLAP), Some((OVERLAP, Schema::Number { min: 0.0, max: 2.0, default: 1.0 })));
    assert_eq!(schema(STITCH), Some((STITCH, Schema::Choice { options: vec!["auto", "run", "satin", "fill"], default: "auto" })));
    assert_eq!(schema(PREFER), Some((PREFER, Schema::Choice { options: vec!["satin", "balanced", "fill"], default: "balanced" })));
    let preview = schemas(&manifest.params).pop();
    assert_eq!(preview, Some((PREVIEW_LEVEL, Schema::Choice { options: vec!["auto", "exact", "outline"], default: "auto" })));
    // A record of the defaults reads as no settings at all.
    let mut set = ParamSet::new();
    for (key, param) in manifest.params.iter().filter(|(key, _)| *key != PREVIEW_LEVEL) {
        set.set(*key, stitchcraft_vectorcraft::param_text(&param.schema.default_value()).unwrap().as_str());
    }
    assert_eq!(stitchcraft_engine::auto::rules(&set), stitchcraft_engine::auto::rules(&ParamSet::new()));
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
    assert_eq!(fill[fill.len() - 4..], ["max_stitch_length_mm", "underlays_first", "digitized_as", PREVIEW_LEVEL]);
    // Every family's dialog ends with the sewing order, StitchCraft's own toggle, off by default (M10.15), and
    // what an element digitizing made sews as (M13.26), no by default.
    for each in Family::ALL {
        let params = family(*each).params;
        let order = params.iter().rev().nth(2).map(|(key, param)| (*key, param.schema.clone(), param.ui.group, param.ui.label));
        assert_eq!(order, Some(("underlays_first", Schema::Bool { default: false }, Some("Sewing order"), Some("Underlays first"))), "{each:?}");
        let digitized = params.iter().rev().nth(1).map(|(key, param)| (*key, param.schema.default_value(), param.ui.group, param.ui.label));
        assert_eq!(digitized, Some(("digitized_as", serde_json::json!("no"), Some("Digitized"), Some("Digitized as"))), "{each:?}");
    }
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
fn req_vc_021_the_fill_dialog_holds_a_tartan_s_sett_as_text_after_its_method() {
    // From M10.7 the sett is a text field, which VectorCraft 0.12 and 0.13 refuse in a manifest, so the fill
    // effect installs from 0.14 (vectorcraft-integration.md). StitchCraft Tartan writes it.
    let fill = family(Family::Fill);
    let keys: Vec<&str> = fill.params.iter().map(|(key, _)| *key).collect();
    for key in ["tartan", "tartan_angle", "rows_per_thread", "herringbone_width_mm", "stop_at_ending_point"] {
        assert!(keys.contains(&key), "{key}");
    }
    let (at, sett) = fill.params.iter().enumerate().find(|(_, (key, _))| *key == "tartan").map(|(at, (_, param))| (at, param)).unwrap();
    assert_eq!(sett.schema, Schema::Text { default: "", max_length: 4_096 });
    assert_eq!(sett.schema.default_value(), Value::from(""));
    // It shows only for the tartan fill, after the method that shows it.
    assert_eq!(sett.ui.visible_when, [("fill_method", vec![Value::from("tartan_fill")])]);
    assert!(keys.iter().position(|key| *key == "fill_method").is_some_and(|method| method < at));
    assert_eq!(sett.ui.label, Some("Sett"));
    // No other text: the custom locks' and the other texts keep their defaults.
    let texts = |manifest: &Manifest| manifest.params.iter().filter(|(_, param)| matches!(param.schema, Schema::Text { .. })).count();
    assert_eq!(texts(&fill), 1);
    assert_eq!((texts(&family(Family::Stroke)), texts(&family(Family::Satin))), (0, 0));
    // From M10.10 the dialog holds 65, the guided fill's strategy among them: more than the 64 that VectorCraft
    // 0.12 and 0.13 take, which refuse the fill effect for its text anyway. From 0.14 a manifest holds 256, at
    // most 64 outside groups, and every one of these is in a group. From M13.26 it holds M13.16's
    // `turn_stitches`, which automatic digitizing sets on its own fills, and `digitized_as` (`REQ-VC-035`).
    assert_eq!(keys.len(), limits::MAX_PARAMS_BEFORE_0_14 + 3);
    assert!(fill.params.iter().all(|(_, param)| param.ui.group.is_some()));
    assert!(keys.contains(&"turn_stitches") && keys.contains(&"digitized_as"));
    validate(&serde_json::to_vec(&fill).unwrap()).unwrap();
    // The other families' effects install in 0.12 and 0.13 too, and stay within their 64.
    for each in [Family::Stroke, Family::Satin] {
        assert!(family(each).params.len() <= limits::MAX_PARAMS_BEFORE_0_14, "{each:?}");
    }
}

#[test]
fn a_text_is_declared_as_vectorcraft_reads_one() {
    let text = Schema::Text { default: "ab", max_length: 2 };
    assert_eq!(serde_json::to_value(&text).unwrap(), json!({"type": "string", "default": "ab", "maxLength": 2}));
    let checked = |spec: Value| valid_spec(&spec);
    assert_eq!(checked(json!({"type": "string"})), Ok(()));
    assert_eq!(checked(json!({"type": "string", "default": "ab", "maxLength": 2})), Ok(()));
    let wrong = Err("`maxLength` must be 1–4096, and the default a text no longer".to_string());
    for spec in [
        json!({"type": "string", "default": "abc", "maxLength": 2}),
        json!({"type": "string", "maxLength": 0}),
        json!({"type": "string", "maxLength": 4_097}),
        json!({"type": "string", "maxLength": -1}),
        json!({"type": "string", "default": 7}),
    ] {
        assert_eq!(checked(spec.clone()), wrong, "{spec}");
    }
    assert_eq!(checked(json!({"type": "string", "maxLength": 4_096})), Ok(()));
    // A default counts in characters, as VectorCraft counts them.
    assert_eq!(checked(json!({"type": "string", "default": "éé", "maxLength": 2})), Ok(()));
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
                Schema::Text { default, .. } => default.to_string(),
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
    // A choice leaves out an option a text is shown for alone, and is left out when that is its default, or
    // when 1 option is left: there is nothing to choose.
    const OPTIONS: &[ChoiceOption] = &[ChoiceOption::new("half", "Half"), ChoiceOption::new("custom", "Custom")];
    let choice = spec(Kind::Choice { options: OPTIONS }, "half");
    let shape = ParamSpec { key: "shape", visible_when: Some(Condition { key: "p", any_of: &["custom"] }), ..spec(Kind::Text { max_bytes: 9 }, "") };
    assert_eq!(schema(&choice, &[choice, shape]), None);
    const THREE: &[ChoiceOption] = &[ChoiceOption::new("half", "Half"), ChoiceOption::new("full", "Full"), ChoiceOption::new("custom", "Custom")];
    let three = spec(Kind::Choice { options: THREE }, "half");
    assert_eq!(schema(&three, &[three, shape]), Some(Schema::Choice { options: vec!["half", "full"], default: "half" }));
    const KEPT: &[ChoiceOption] = &[ChoiceOption::new("0", "Copy"), ChoiceOption::kept("1", "Parallel offset")];
    assert_eq!(schema(&spec(Kind::Choice { options: KEPT }, "0"), &[]), None);
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

/// `manifest` with `key` set to `v`, as the bytes VectorCraft reads.
fn with(manifest: &Manifest, key: &str, v: Value) -> Vec<u8> {
    let mut m = value(manifest);
    m[key] = v;
    serde_json::to_vec(&m).unwrap()
}

#[test]
fn validation_catches_what_vectorcraft_rejects() {
    assert!(validate(&with(&hello(), "id", json!("has space"))).is_err());
    assert!(validate(&with(&hello(), "name", json!(""))).is_err());
    assert!(validate(&with(&hello(), "author", json!("x".repeat(limits::MAX_AUTHOR_BYTES + 1)))).is_err());
    let large = validate(&with(&hello(), "description", json!("x".repeat(limits::MAX_MANIFEST_BYTES))));
    assert!(large.is_err_and(|why| why.starts_with("manifest is ")));
    // White space after the JSON counts too: 64 KiB is the most.
    let padded = |len: usize| {
        let mut text = serde_json::to_vec(&value(&hello())).unwrap();
        text.resize(len, b' ');
        validate(&text)
    };
    assert_eq!(padded(limits::MAX_MANIFEST_BYTES), Ok(()));
    assert!(padded(limits::MAX_MANIFEST_BYTES + 1).is_err_and(|why| why.starts_with("manifest is 65537 bytes")));
    assert!(validate(&with(&hello(), "kind", json!("tool"))).is_err());
}

#[test]
fn only_an_exporter_lists_extensions_from_1_to_8() {
    // An exporter lists 1 to 8 extensions, each 1 to 16 lower-case letters and digits, and only an exporter
    // lists any.
    let exporting = |extensions: Value| with(&exporter(FormatId::Dst), "extensions", extensions);
    let nine = json!(["a", "b", "c", "d", "e", "f", "g", "h", "i"]);
    for bad in
        [json!([]), json!(["DST"]), json!([".dst"]), json!([""]), json!(["abcdefghijklmnopq"]), json!(["dst", "dst"]), nine, json!("dst"), json!([1])]
    {
        assert!(validate(&exporting(bad.clone())).is_err(), "{bad}");
    }
    assert_eq!(validate(&exporting(json!(["dst", "dst2", "abcdefghijklmnop"]))), Ok(()));
    assert_eq!(validate(&exporting(json!(["a", "b", "c", "d", "e", "f", "g", "h"]))), Ok(()), "8, the most");
    assert!(validate(&with(&hello(), "kind", json!("exporter"))).is_err(), "an exporter without extensions");
    assert!(validate(&with(&hello(), "extensions", json!(["pes"]))).is_err(), "a live effect with extensions");
    assert_eq!(validate(&with(&hello(), "extensions", json!([]))), Ok(()));
}

#[test]
fn only_a_filter_may_ask_for_a_heavy_budget() {
    // From 0.17 a filter may ask for a heavy budget, and nothing else may.
    let filter = |budget: Value| with(&settings(), "budget", budget);
    assert_eq!(validate(&filter(json!("heavy"))), Ok(()));
    assert_eq!(validate(&filter(json!("light"))), Ok(()));
    for bad in [json!("medium"), json!(null), json!(true)] {
        assert!(validate(&filter(bad.clone())).is_err(), "{bad}");
    }
    let heavy_effect = with(&hello(), "budget", json!("heavy"));
    assert!(validate(&heavy_effect).is_err_and(|why| why.contains("only a filter")), "a heavy live effect");
    let heavy_exporter = with(&exporter(FormatId::Dst), "budget", json!("heavy"));
    assert!(validate(&heavy_exporter).is_err_and(|why| why.contains("only a filter")), "a heavy exporter");
    assert_eq!(validate(&with(&hello(), "budget", json!("light"))), Ok(()));
}

#[test]
fn req_vc_023_the_fill_dialog_offers_the_guided_fill_s_3_strategies_by_ink_stitch_s_numbers() {
    // M10.9 sewed one strategy, and a choice of one option isn't declared. From M10.10 all 3 are sewn, so the
    // strategy is in the dialog again, its options Ink/Stitch's ids, shown for the guided fill alone.
    let fill = family(Family::Fill);
    let (_, strategy) = fill.params.iter().find(|(key, _)| *key == "guided_fill_strategy").unwrap();
    assert_eq!(strategy.schema, Schema::Choice { options: vec!["0", "1", "2"], default: "0" });
    // Its label tells it from the contour fill's strategy, a row of the same dialog.
    assert_eq!((strategy.ui.label, strategy.ui.group), (Some("Guided fill strategy"), Some("Guided fill")));
    assert_eq!(strategy.ui.visible_when, [("fill_method", vec![Value::from("guided_fill")])]);
    let help = strategy.ui.help.as_deref().unwrap_or_default();
    assert!(help.ends_with("One of 0 (Copy), 1 (Parallel offset), 2 (Buffer)."), "{help}");
    // The stitch positions show for it too, and the copy angle, which may be empty, stays out.
    let keys: Vec<&str> = fill.params.iter().map(|(key, _)| *key).collect();
    assert!(keys.contains(&"stitch_position_method") && !keys.contains(&"guided_fill_angle"));
}

#[test]
fn parameters_are_an_object_of_valid_ones_up_to_the_most() {
    // A manifest may leave its parameters out, as VectorCraft reads it, but they are an object.
    let mut bare = value(&hello());
    bare.as_object_mut().unwrap().remove("params");
    assert_eq!(validate(&serde_json::to_vec(&bare).unwrap()), Ok(()));
    let params = |v: Value| with(&hello(), "params", v);
    assert!(validate(&params(json!([]))).is_err_and(|why| why.contains("expected a JSON object")));
    assert!(validate(&params(json!({"preview": {"type": "bool", "default": false}}))).is_err());
    // 64 parameters in a manifest that VectorCraft 0.12 and 0.13 take, and one more only with a text, which
    // they refuse anyway. Then up to 256, at most 64 of them outside groups.
    let toggles = |count: usize, grouped: usize, text: bool| {
        let toggle = |i: usize| {
            if i < grouped { json!({"type": "bool", "default": false, "group": "More"}) } else { json!({"type": "bool", "default": false}) }
        };
        let mut all: serde_json::Map<String, Value> = (0..count - usize::from(text)).map(|i| (format!("p{i}"), toggle(i))).collect();
        if text {
            all.insert("text".into(), json!({"type": "string", "group": "More"}));
        }
        validate(&params(Value::Object(all)))
    };
    let early = limits::MAX_PARAMS_BEFORE_0_14;
    assert_eq!(toggles(early, 0, false), Ok(()));
    assert!(toggles(early + 1, early + 1, false).is_err_and(|why| why.contains("65 parameters (limit 64 before VectorCraft 0.14")));
    assert_eq!(toggles(early + 1, 1, true), Ok(()));
    assert_eq!(toggles(early + 2, 1, true), Ok(()));
    assert!(toggles(early + 2, 0, true).is_err_and(|why| why.contains("65 parameters outside groups (limit 64)")));
    // An exporter is from 0.14 too.
    let grouped: serde_json::Map<String, Value> =
        (0..=early).map(|i| (format!("p{i}"), json!({"type": "bool", "default": false, "group": "More"}))).collect();
    assert_eq!(validate(&with(&exporter(FormatId::PesV1), "params", Value::Object(grouped))), Ok(()));
    assert_eq!(toggles(limits::MAX_PARAMS, limits::MAX_PARAMS - limits::MAX_UNGROUPED, true), Ok(()));
    assert!(toggles(limits::MAX_PARAMS + 1, limits::MAX_PARAMS, true).is_err_and(|why| why.contains("257 parameters (limit 256)")));
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
        assert!(validate(&params(json!({"p": bad.clone()}))).is_err(), "{bad}");
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
        assert_eq!(validate(&params(json!({"p": good.clone()}))), Ok(()), "{good}");
    }
}
