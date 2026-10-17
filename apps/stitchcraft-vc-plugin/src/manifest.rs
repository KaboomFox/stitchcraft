//! Plug-in identities and manifests.
//!
//! VectorCraft reads a JSON manifest from every module (`docs/plugins.md` in VectorCraft) and builds the
//! plug-in's dialog from its parameters. A family's manifest is made from the parameter registry
//! (`stitchcraft_engine::registry::PARAMETERS`), so a parameter is declared once and every dialog follows
//! it. The manifests are checked with VectorCraft's rules, so one VectorCraft would refuse fails these
//! tests instead of a user's install (`REQ-VC-002`).
//!
//! **What a manifest can hold.** VectorCraft's parameters are numbers, whole numbers, toggles and choices.
//! Each always has a value: a record VectorCraft stores holds every declared parameter, at its default
//! when the user left it, and keys the manifest doesn't declare are dropped. So a parameter is declared
//! when one such value means what the parameter means:
//!
//! - a length, angle, percentage or number as a number in its range, and a count as a whole number;
//! - a list or a pair whose default is one value as that value, which stands for the whole list or for
//!   both sides, as one value does in Ink/Stitch;
//! - an optional length that 0 leaves unset as a number from 0, 0 being unset, as in Ink/Stitch.
//!
//! The rest keep their defaults in VectorCraft: texts, seeds, and settings left empty to take another's
//! value. A choice leaves out the options whose stitches a text spells out, such as the custom locks,
//! since the dialog cannot hold the text. `satin_column` is set by the effect itself, by which family it
//! is ([`SATIN_COLUMN`]).
//!
//! **Order.** The dialog lists parameters in the manifest's order, which is the registry's within each
//! group. The group with the family's method comes first, then the groups only the family uses, then
//! those it shares with other families, and last the settings every element shares (`CommonParams`),
//! where Ink/Stitch's dialogs list them too. A fill's dialog opens with `fill` and `fill_method`, as
//! Ink/Stitch's does.
//!
//! **The preview.** Last comes `preview_level` ([`PREVIEW_LEVEL`]), which is the plug-in's and no
//! stitch's: how much of the stitches the live effect shows ([`crate::preview::Level`]).

use serde::Serialize;
use serde::ser::{SerializeMap, Serializer};
use serde_json::Value;
use stitchcraft_core::Mm;
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_params::{Empty, Family, Kind, ParamGroup, ParamSpec, Value as Parsed};
pub use stitchcraft_vectorcraft::{PLUGIN_NAMESPACE, PREVIEW_LEVEL};

use crate::preview::Level;

/// VectorCraft ABI v1 limits on manifests (VectorCraft `crates/plugins/src/manifest.rs`).
pub mod limits {
    /// Largest manifest, in bytes.
    pub const MAX_MANIFEST_BYTES: usize = 64 * 1024;
    /// Most parameters one plug-in may declare.
    pub const MAX_PARAMS: usize = 64;
    /// Longest id and name, in characters.
    pub const MAX_ID_CHARS: usize = 64;
    /// Longest version string, in bytes.
    pub const MAX_VERSION_BYTES: usize = 32;
    /// Longest description, in bytes.
    pub const MAX_DESCRIPTION_BYTES: usize = 1024;
    /// Longest author string, in bytes.
    pub const MAX_AUTHOR_BYTES: usize = 128;
    /// Most options of a choice, and the longest option, in bytes.
    pub const MAX_OPTIONS: usize = 64;
}

/// Who the plug-ins are by.
const AUTHOR: &str = "StitchCraft contributors";

/// A plug-in's manifest, as `vc_manifest` returns it.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Manifest {
    /// `dev.stitchcraft.<name>`.
    pub id: String,
    /// The menu label.
    pub name: &'static str,
    /// The plug-in crate's version.
    pub version: &'static str,
    /// `effect` or `filter`.
    pub kind: &'static str,
    /// Who the plug-in is by.
    pub author: &'static str,
    /// What it does, for VectorCraft's plug-in list.
    pub description: &'static str,
    /// The parameters, in the dialog's order.
    #[serde(serialize_with = "in_order")]
    pub params: Vec<(&'static str, Schema)>,
}

/// One parameter as VectorCraft declares it (`ParamSpec` in VectorCraft's `crates/plugins`).
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Schema {
    /// A number within `min..=max`.
    Number {
        /// Smallest accepted value.
        min: f64,
        /// Largest accepted value.
        max: f64,
        /// Its value when the user leaves it.
        default: f64,
    },
    /// A whole number within `min..=max`.
    Int {
        /// Smallest accepted value.
        min: i64,
        /// Largest accepted value.
        max: i64,
        /// Its value when the user leaves it.
        default: i64,
    },
    /// A checkbox.
    Bool {
        /// Its value when the user leaves it.
        default: bool,
    },
    /// One of `options`.
    Choice {
        /// The options' ids, as designs store them.
        options: Vec<&'static str>,
        /// Its value when the user leaves it.
        default: &'static str,
    },
}

/// The id of plug-in `name` (`dev.stitchcraft.<name>`).
pub fn plugin_id(name: &str) -> String {
    format!("{PLUGIN_NAMESPACE}.{name}")
}

/// The manifest of the M0.6 *hello* live effect: returns the object's geometry unchanged.
pub fn hello() -> Manifest {
    Manifest {
        id: plugin_id("hello"),
        name: "StitchCraft Hello",
        version: env!("CARGO_PKG_VERSION"),
        kind: "effect",
        author: AUTHOR,
        description: "Returns the object's geometry unchanged. Proves the StitchCraft plug-in toolchain and VectorCraft's ABI v1 contract (roadmap step M0.6).",
        params: Vec::new(),
    }
}

/// The manifest of the live effect that sews an object as `family`, with the parameters of the registry
/// that apply to it.
pub fn family(family: Family) -> Manifest {
    let (name, description) = match family {
        Family::Stroke => ("StitchCraft Stroke", "Sews the object along its path: running, bean or manual stitch."),
        Family::Satin => ("StitchCraft Satin", "Sews the object as a satin column, as wide as its stroke or between its two rails."),
        Family::Fill => ("StitchCraft Fill", "Fills the object with rows of stitches, tatami fill."),
    };
    Manifest {
        id: plugin_id(family.id()),
        name,
        version: env!("CARGO_PKG_VERSION"),
        kind: "effect",
        author: AUTHOR,
        description,
        params: params(family).into_iter().chain([(PREVIEW_LEVEL, preview_level(family))]).collect(),
    }
}

/// How a family's manifest declares [`PREVIEW_LEVEL`]: a choice of the levels its previews offer, auto by
/// default.
fn preview_level(family: Family) -> Schema {
    Schema::Choice { options: Level::offered(family).iter().map(|level| level.id()).collect(), default: Level::Auto.id() }
}

/// The parameters `family`'s manifest declares, in the dialog's order.
fn params(family: Family) -> Vec<(&'static str, Schema)> {
    let mut groups = PARAMETERS.to_vec();
    groups.sort_by_key(|group| place(group, family));
    groups
        .iter()
        .flat_map(|group| group.specs.iter().map(|spec| (spec, group.specs)))
        .filter(|(spec, _)| spec.key != SATIN_COLUMN && spec.applies_to.iter().any(|t| t.family() == family))
        .filter_map(|(spec, siblings)| Some((spec.key, schema(&spec.in_family(family), siblings)?)))
        .collect()
}

/// Where `group` comes in `family`'s dialog, from 0: with the family's method, used by the family alone,
/// shared with other families, or the settings every element shares.
fn place(group: &ParamGroup, family: Family) -> u8 {
    if group.name == CommonParams::GROUP.name {
        3
    } else if group.specs.iter().any(|spec| spec.key == family.method_param()) {
        0
    } else if group.specs.iter().all(|spec| spec.applies_to.iter().all(|t| t.family() == family)) {
        1
    } else {
        2
    }
}

/// How a manifest declares `spec`, one of the parameters `siblings` declared together, or `None` when no
/// one value of VectorCraft's kinds means what it means. A choice leaves out each option that a text
/// among `siblings` is shown for alone: the text spells out that option's stitches.
pub fn schema(spec: &ParamSpec, siblings: &[ParamSpec]) -> Option<Schema> {
    let number = |min: f64, max: f64, default: f64| Some(Schema::Number { min, max, default });
    let default = spec.kind.parse(spec.default).map(|(value, _)| value)?;
    match (spec.kind, default) {
        (Kind::Length { min, max, empty: Empty::Never | Empty::Blank }, Parsed::Length(Some(mm))) => number(min, max, mm.get()),
        (Kind::Length { max, empty: Empty::AtMostZero, .. }, Parsed::Length(mm)) => number(0.0, max, mm.map_or(0.0, Mm::get)),
        (Kind::Angle, Parsed::Angle(angle)) => number(-180.0, 180.0, angle),
        (Kind::Percent { min, max }, Parsed::Percent(value)) | (Kind::Number { min, max }, Parsed::Number(value)) => number(min, max, value),
        (Kind::Count { min, max }, Parsed::Count(count)) => Some(Schema::Int { min: min.into(), max: max.into(), default: count.into() }),
        (Kind::CountList { min, max }, Parsed::Counts(counts)) => match counts.as_slice() {
            [count] => Some(Schema::Int { min: min.into(), max: max.into(), default: (*count).into() }),
            _ => None,
        },
        (Kind::Toggle, Parsed::Toggle(on)) => Some(Schema::Bool { default: on }),
        (Kind::Choice { options }, Parsed::Choice(id)) => {
            let spelled = |option: &str| {
                siblings.iter().any(|other| {
                    matches!(other.kind, Kind::Text { .. }) && other.visible_when.is_some_and(|when| when.key == spec.key && when.any_of == [option])
                })
            };
            let options: Vec<&'static str> = options.iter().map(|o| o.id).filter(|o| !spelled(o)).collect();
            options.contains(&id).then_some(Schema::Choice { options, default: id })
        }
        (Kind::LengthList { min, max }, Parsed::Lengths(lengths)) => match lengths.as_slice() {
            [length] => number(min, max, length.get()),
            _ => None,
        },
        (Kind::PercentList { min, max }, Parsed::Percents(percents)) => match percents.as_slice() {
            [percent] => number(min, max, *percent),
            _ => None,
        },
        (Kind::LengthPair { min, max, .. }, Parsed::LengthPair(Some([a, b]))) if a == b => number(min, max, a.get()),
        (Kind::PercentPair { min, max, .. }, Parsed::PercentPair(Some([a, b]))) if a == b => number(min, max, a),
        _ => None,
    }
}

/// Serializes `params` as a JSON object in their order, which VectorCraft's dialog keeps.
fn in_order<S: Serializer>(params: &[(&'static str, Schema)], serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(params.len()))?;
    for (key, schema) in params {
        map.serialize_entry(key, schema)?;
    }
    map.end()
}

/// Checks `manifest` against VectorCraft's ABI v1 rules; the error names the first broken rule.
pub fn validate(manifest: &Value) -> Result<(), String> {
    let text = |key: &str| manifest.get(key).and_then(Value::as_str).ok_or_else(|| format!("`{key}` must be a string"));
    let bytes = serde_json::to_vec(manifest).map_err(|e| e.to_string())?;
    if bytes.len() > limits::MAX_MANIFEST_BYTES {
        return Err(format!("manifest is {} bytes (limit {})", bytes.len(), limits::MAX_MANIFEST_BYTES));
    }
    let id = text("id")?;
    let id_ok =
        (1..=limits::MAX_ID_CHARS).contains(&id.chars().count()) && id.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'));
    if !id_ok {
        return Err(format!("id `{id}` must be 1–64 characters from A-Z a-z 0-9 . _ -"));
    }
    let name = text("name")?;
    if !(1..=limits::MAX_ID_CHARS).contains(&name.chars().count()) || name.chars().any(char::is_control) {
        return Err(format!("name `{name}` must be 1–64 printable characters"));
    }
    let checks = [("version", limits::MAX_VERSION_BYTES), ("description", limits::MAX_DESCRIPTION_BYTES), ("author", limits::MAX_AUTHOR_BYTES)];
    for (key, max) in checks {
        if text(key)?.len() > max {
            return Err(format!("`{key}` is longer than {max} bytes"));
        }
    }
    if !matches!(text("kind")?, "filter" | "effect") {
        return Err("`kind` must be \"filter\" or \"effect\"".into());
    }
    let params = manifest.get("params").and_then(Value::as_object).ok_or("`params` must be an object")?;
    if params.len() > limits::MAX_PARAMS {
        return Err(format!("{} parameters (limit {})", params.len(), limits::MAX_PARAMS));
    }
    for (key, spec) in params {
        let ok = !key.is_empty()
            && !key.starts_with('_')
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !matches!(key.as_str(), "id" | "ids" | "params" | "preview");
        if !ok {
            return Err(format!("parameter name `{key}` is not allowed"));
        }
        valid_spec(spec).map_err(|why| format!("parameter `{key}`: {why}"))?;
    }
    Ok(())
}

/// Checks one parameter's declaration as VectorCraft does.
fn valid_spec(spec: &Value) -> Result<(), String> {
    let number = |key: &str| spec.get(key).and_then(Value::as_f64).filter(|v| v.is_finite()).ok_or_else(|| format!("`{key}` must be a number"));
    match spec.get("type").and_then(Value::as_str) {
        Some("number") => {
            let (min, max, _) = (number("min")?, number("max")?, number("default")?);
            if min > max { Err("bad range".into()) } else { Ok(()) }
        }
        Some("int") => {
            let whole = |key: &str| spec.get(key).map_or(Ok(None), |v| v.as_i64().map(Some).ok_or_else(|| format!("`{key}` must be a whole number")));
            match (whole("min")?, whole("max")?, whole("default")?) {
                (Some(min), Some(max), _) if min > max => Err("bad range".into()),
                _ => Ok(()),
            }
        }
        Some("bool") => match spec.get("default") {
            None | Some(Value::Bool(_)) => Ok(()),
            Some(_) => Err("`default` must be true or false".into()),
        },
        Some("choice") => {
            let options: Vec<&str> = spec.get("options").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
            let count = spec.get("options").and_then(Value::as_array).map_or(0, Vec::len);
            let printable = |o: &&str| !o.is_empty() && o.len() <= limits::MAX_OPTIONS && !o.chars().any(char::is_control);
            if options.is_empty() || options.len() != count || count > limits::MAX_OPTIONS || !options.iter().all(printable) {
                return Err("choices must be 1–64 non-empty, printable strings".into());
            }
            match spec.get("default") {
                None => Ok(()),
                Some(Value::String(d)) if options.contains(&d.as_str()) => Ok(()),
                Some(_) => Err("default is not one of the options".into()),
            }
        }
        _ => Err("`type` must be number, int, bool or choice".into()),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use stitchcraft_params::{ChoiceOption, Condition, Origin, ParamSet, Stability};

    use super::*;

    fn value(manifest: &Manifest) -> Value {
        serde_json::to_value(manifest).unwrap()
    }

    #[test]
    fn hello_manifest_is_valid() {
        assert_eq!(validate(&value(&hello())), Ok(()));
        assert_eq!(hello().id, "dev.stitchcraft.hello");
    }

    #[test]
    fn req_vc_002_each_family_s_manifest_is_one_vectorcraft_accepts() {
        for each in Family::ALL {
            let manifest = family(*each);
            assert_eq!(validate(&value(&manifest)), Ok(()), "{each:?}");
            assert_eq!(manifest.id, format!("dev.stitchcraft.{}", each.id()));
            assert!(manifest.params.iter().all(|(key, _)| *key != SATIN_COLUMN), "{each:?}");
        }
    }

    #[test]
    fn each_family_s_dialog_ends_with_its_preview_levels_auto_first() {
        let levels = |each: Family| family(each).params.last().cloned();
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
        let satin = keys(Family::Satin);
        assert_eq!(satin[..2], ["satin_method", "zigzag_spacing_mm"]);
        // Within a group, the registry's order.
        assert!(place(&satin, "ties") < place(&satin, "trim_after") && place(&satin, "trim_after") < place(&satin, "min_stitch_length_mm"));
    }

    #[test]
    fn req_vc_002_a_record_of_the_defaults_reads_as_no_settings_at_all() {
        // VectorCraft stores every declared parameter, at its default when the user leaves it: each must
        // read as the registry's default, as an element without settings reads it.
        for each in Family::ALL {
            for (key, schema) in family(*each).params.into_iter().filter(|(key, _)| *key != PREVIEW_LEVEL) {
                let spec = PARAMETERS.iter().flat_map(|g| g.specs).find(|s| s.key == key).unwrap().in_family(*each);
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
        let shape =
            ParamSpec { key: "shape", visible_when: Some(Condition { key: "p", any_of: &["custom"] }), ..spec(Kind::Text { max_bytes: 9 }, "") };
        assert_eq!(schema(&choice, &[choice, shape]), Some(Schema::Choice { options: vec!["half"], default: "half" }));
        let both = ParamSpec { visible_when: Some(Condition { key: "p", any_of: &["half", "custom"] }), ..shape };
        assert_eq!(schema(&choice, &[choice, both]), Some(Schema::Choice { options: vec!["half", "custom"], default: "half" }));
        let size = ParamSpec { kind: Kind::Count { min: 1, max: 9 }, ..shape };
        assert_eq!(schema(&choice, &[choice, size]), Some(Schema::Choice { options: vec!["half", "custom"], default: "half" }));
        assert_eq!(schema(&spec(Kind::Choice { options: OPTIONS }, "custom"), &[shape]), None);
    }

    #[test]
    fn validation_catches_what_vectorcraft_rejects() {
        let base = value(&hello());
        let with = |key: &str, v: Value| {
            let mut m = base.clone();
            m[key] = v;
            m
        };
        assert!(validate(&with("id", json!("has space"))).is_err());
        assert!(validate(&with("name", json!(""))).is_err());
        assert!(validate(&with("author", json!("x".repeat(limits::MAX_AUTHOR_BYTES + 1)))).is_err());
        let large = validate(&with("description", json!("x".repeat(limits::MAX_MANIFEST_BYTES))));
        assert!(large.is_err_and(|why| why.starts_with("manifest is ")));
        assert!(validate(&with("kind", json!("tool"))).is_err());
        assert!(validate(&with("params", json!({"preview": {"type": "bool", "default": false}}))).is_err());
        let too_many: serde_json::Map<String, Value> =
            (0..=limits::MAX_PARAMS).map(|i| (format!("p{i}"), json!({"type": "bool", "default": false}))).collect();
        assert!(validate(&with("params", Value::Object(too_many))).is_err());
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
}
