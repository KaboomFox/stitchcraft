//! Plug-in identities and manifests.
//!
//! VectorCraft reads a JSON manifest from every module (`docs/plugins.md` in VectorCraft) and builds the
//! plug-in's dialog from its parameters. A family's manifest is made from the parameter registry
//! (`stitchcraft_engine::registry::PARAMETERS`), and every dialog follows it. A key declared for several
//! families is in each family's manifest as that family declares it. The manifests are checked with
//! VectorCraft's rules, so one VectorCraft would refuse fails these tests instead of a user's install
//! (`REQ-VC-002`).
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
//! since the dialog cannot hold the text, and the methods StitchCraft sews as another: Ink/Stitch's legacy
//! fill, sewn as a tatami fill (`REQ-FILL-005`). `satin_column` is set by the effect itself, by which
//! family it is ([`SATIN_COLUMN`]).
//!
//! **Order.** The dialog lists parameters in the manifest's order, which is the registry's within each
//! group. The group with the family's method comes first, then the groups only the family uses, then
//! those it shares with other families, and last the settings every element shares (`CommonParams`),
//! where Ink/Stitch's dialogs list them too. A fill's dialog opens with `fill` and `fill_method`, as
//! Ink/Stitch's does.
//!
//! **The preview.** Last comes `preview_level` ([`PREVIEW_LEVEL`]), which is the plug-in's and no
//! stitch's: how much of the stitches the live effect shows ([`crate::preview::Level`]).
//!
//! **How the dialog shows them.** Each parameter comes with its label, unit, section, help and help page,
//! and the values of the parameters before it that show it, which VectorCraft reads from 0.14 and earlier
//! releases ignore ([`crate::dialog`], `REQ-VC-012`).
//!
//! **Exporters.** An exporter's manifest ([`exporter`]) lists its format's extension, and its dialog holds
//! the 2 options of `stitch export` it takes ([`crate::export`]): `auto`, which digitizes the shapes without
//! settings, and `profile`, a choice of the built-in machine profiles, the reference machine's first.
//! VectorCraft refuses an exporter before 0.14, which is when its kind came.

use serde::de::{Deserializer, MapAccess, Visitor};
use serde::ser::{SerializeMap, Serializer};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use stitchcraft_core::Mm;
use stitchcraft_engine::common::CommonParams;
use stitchcraft_engine::generate::sewn_as;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_params::{Empty, Family, Kind, ParamGroup, ParamSpec, StitchType, Value as Parsed, find_for};
use stitchcraft_plan::FormatId;
use stitchcraft_vectorcraft::settings::SETTINGS_FILTER;
pub use stitchcraft_vectorcraft::{PREVIEW_LEVEL, plugin_id};

use crate::dialog::{self, Ui, registry_ui};
use crate::{export, preview};

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
    /// Most file extensions an exporter lists (from VectorCraft 0.14).
    pub const MAX_EXTENSIONS: usize = 8;
    /// Longest file extension, in characters.
    pub const MAX_EXTENSION_CHARS: usize = 16;
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
    /// `effect`, `filter` or, from VectorCraft 0.14, `exporter`.
    pub kind: &'static str,
    /// Who the plug-in is by.
    pub author: &'static str,
    /// What it does, for VectorCraft's plug-in list.
    pub description: &'static str,
    /// The parameters, in the dialog's order.
    #[serde(serialize_with = "in_order")]
    pub params: Vec<(&'static str, Param)>,
    /// An exporter's file extensions, lower case and without the dot, the one its files get first. Only an
    /// exporter lists them.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub extensions: Vec<&'static str>,
}

/// One parameter as VectorCraft declares it: its kind of value, which every release reads, and how
/// dialogs show it, which releases before 0.14 ignore (`Param` in VectorCraft's `crates/plugins`).
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Param {
    /// Its kind of value, range and default.
    #[serde(flatten)]
    pub schema: Schema,
    /// How dialogs show it.
    #[serde(flatten)]
    pub ui: Ui,
}

/// A parameter's kind of value, range and default (`ParamSpec` in VectorCraft's `crates/plugins`).
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

impl Schema {
    /// Its value when the user leaves it, as VectorCraft passes it: a number within its range, as
    /// VectorCraft clamps a default into it.
    pub fn default_value(&self) -> Value {
        match self {
            Schema::Number { min, max, default } => Value::from(default.clamp(*min, *max)),
            Schema::Int { min, max, default } => Value::from(*default.clamp(min, max)),
            Schema::Bool { default } => Value::Bool(*default),
            Schema::Choice { default, .. } => Value::from(*default),
        }
    }
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
        extensions: Vec::new(),
    }
}

/// The manifest of the live effect that sews an object as `family`, with the parameters of the registry
/// that apply to it.
pub fn family(family: Family) -> Manifest {
    let (name, description) = match family {
        Family::Stroke => ("StitchCraft Stroke", "Sews the object along its path: running, bean, zigzag or manual stitch."),
        Family::Satin => (
            "StitchCraft Satin",
            "Sews the object as a satin column, an E-stitch, an S-stitch or a zigzag satin, as wide as its stroke or between its two rails.",
        ),
        Family::Fill => ("StitchCraft Fill", "Fills the object with rows of stitches, tatami fill."),
    };
    Manifest {
        id: plugin_id(family.id()),
        name,
        version: env!("CARGO_PKG_VERSION"),
        kind: "effect",
        author: AUTHOR,
        description,
        params: params(family).into_iter().chain([(PREVIEW_LEVEL, preview::param(family))]).collect(),
        extensions: Vec::new(),
    }
}

/// The manifest of the Design Settings filter, which keeps a document's design settings and machine with it
/// (VectorCraft 0.16, [`crate::settings`]).
pub fn settings() -> Manifest {
    Manifest {
        id: plugin_id(SETTINGS_FILTER),
        name: "StitchCraft Design Settings",
        version: env!("CARGO_PKG_VERSION"),
        kind: "filter",
        author: AUTHOR,
        description: "Keeps the design's settings and machine with the document, which File › Export and `stitch export` plan it with: \
                      the shortest jump and stitch, the narrowest satin stroke, and the machine and hoop. It changes no object.",
        params: crate::settings::options(),
        extensions: Vec::new(),
    }
}

/// The manifest of the exporter that writes the document as a file of `format`, as `stitch export` writes
/// it: its extension, and the options it takes, `auto` and the machine's `profile`.
pub fn exporter(format: FormatId) -> Manifest {
    let (name, description) = match format {
        FormatId::PesV1 => (
            "PES embroidery (StitchCraft)",
            "Writes the document as a Brother PES file, sewn as its StitchCraft effects say, as `stitch export` writes it.",
        ),
        FormatId::Dst => (
            "DST embroidery (StitchCraft)",
            "Writes the document as a Tajima DST file, sewn as its StitchCraft effects say, as `stitch export` writes it.",
        ),
    };
    Manifest {
        id: plugin_id(format.extension()),
        name,
        version: env!("CARGO_PKG_VERSION"),
        kind: "exporter",
        author: AUTHOR,
        description,
        params: export::options(),
        extensions: vec![format.extension()],
    }
}

/// The parameters `family`'s manifest declares, in the dialog's order.
fn params(family: Family) -> Vec<(&'static str, Param)> {
    params_of(PARAMETERS, family)
}

/// The parameters of `registry` that `family`'s manifest declares, in the dialog's order, each as the
/// dialog shows it. A key that several families declare is there as `family` declares it (REQ-PRM-005).
/// One no method the dialog offers shows is left out.
fn params_of(registry: &[&ParamGroup], family: Family) -> Vec<(&'static str, Param)> {
    let default = find_for(registry, family.method_param(), family).and_then(|method| StitchType::from_id(family, method.default));
    let mut groups = registry.to_vec();
    groups.sort_by_key(|group| place(group, family, default));
    let mut params: Vec<(&'static str, Param)> = Vec::new();
    for group in groups {
        for spec in group.specs.iter().filter(|spec| spec.key != SATIN_COLUMN && spec.applies_to.iter().any(|t| t.family() == family)) {
            let spec = spec.in_family(family);
            let Some(schema) = schema(&spec, group.specs) else { continue };
            // The method comes first, so the rows of some methods only can say which.
            let before = params.as_slice();
            if let Some(ui) = registry_ui(&spec, &schema, group, family, before) {
                params.push((spec.key, Param { schema, ui }));
            }
        }
    }
    params
}

/// Where `group` comes in `family`'s dialog, from 0. First is the group with the family's method, then
/// those the family alone uses for its `default` method, those it shares with other families, those of its
/// other methods, and the settings every element shares last. A default method's settings so come before
/// the other methods', as in Ink/Stitch's dialogs.
fn place(group: &ParamGroup, family: Family, default: Option<StitchType>) -> u8 {
    if group.name == CommonParams::GROUP.name {
        4
    } else if group.specs.iter().any(|spec| spec.key == family.method_param()) {
        0
    } else if !group.specs.iter().all(|spec| spec.applies_to.iter().all(|t| t.family() == family)) {
        2
    } else if default.is_some_and(|method| group.applies_to.contains(&method)) {
        1
    } else {
        3
    }
}

/// How a manifest declares `spec`, one of the parameters `siblings` declared together, or `None` when no
/// one value of VectorCraft's kinds means what it means. A choice leaves out each option that a text
/// among `siblings` is shown for alone: the text spells out that option's stitches. A method parameter
/// leaves out the methods StitchCraft sews as another ([`sewn_as`]).
pub fn schema(spec: &ParamSpec, siblings: &[ParamSpec]) -> Option<Schema> {
    let number = |min: f64, max: f64, default: f64| Some(Schema::Number { min, max, default });
    let default = spec.kind.parse(spec.default).map(|(value, _)| value)?;
    match (spec.kind, default) {
        (Kind::Length { min, max, empty: Empty::Never | Empty::Blank }, Parsed::Length(Some(mm))) => number(min, max, mm.get()),
        (Kind::Length { max, empty: Empty::AtMostZero, .. }, Parsed::Length(mm)) => number(0.0, max, mm.map_or(0.0, Mm::get)),
        (Kind::Angle, Parsed::Angle(angle)) => number(-180.0, 180.0, angle),
        (Kind::BoundedAngle { min, max }, Parsed::Angle(angle)) => number(min, max, angle),
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
            let family = Family::ALL.iter().copied().find(|family| family.method_param() == spec.key);
            let other = |option: &str| family.and_then(|family| StitchType::from_id(family, option)).is_some_and(|t| sewn_as(t) != t);
            let options: Vec<&'static str> = options.iter().map(|o| o.id).filter(|o| !spelled(o) && !other(o)).collect();
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
fn in_order<S: Serializer>(params: &[(&'static str, Param)], serializer: S) -> Result<S::Ok, S::Error> {
    let mut map = serializer.serialize_map(Some(params.len()))?;
    for (key, schema) in params {
        map.serialize_entry(key, schema)?;
    }
    map.end()
}

/// Checks the manifest `json`, as `vc_manifest` serves it, against VectorCraft's rules; the error names
/// the first broken rule. VectorCraft reads the parameters in the order the text gives them, which a
/// parameter's `visibleWhen` depends on, so the text is checked, not a value of it.
pub fn validate(json: &[u8]) -> Result<(), String> {
    if json.len() > limits::MAX_MANIFEST_BYTES {
        return Err(format!("manifest is {} bytes (limit {})", json.len(), limits::MAX_MANIFEST_BYTES));
    }
    let manifest: Value = serde_json::from_slice(json).map_err(|e| format!("the manifest doesn't read: {e}"))?;
    let text = |key: &str| manifest.get(key).and_then(Value::as_str).ok_or_else(|| format!("`{key}` must be a string"));
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
    let kind = text("kind")?;
    if !matches!(kind, "filter" | "effect" | "exporter") {
        return Err("`kind` must be \"filter\", \"effect\" or \"exporter\"".into());
    }
    valid_extensions(kind, manifest.get("extensions"))?;
    let Ordered(params) = serde_json::from_slice::<Declared>(json).map_err(|e| format!("`params` must be an object: {e}"))?.params;
    if params.len() > limits::MAX_PARAMS {
        return Err(format!("{} parameters (limit {})", params.len(), limits::MAX_PARAMS));
    }
    let declared: Vec<(&String, &Value)> = params.iter().map(|(key, spec)| (key, spec)).collect();
    for (i, (key, spec)) in declared.iter().enumerate() {
        let ok = !key.is_empty()
            && !key.starts_with('_')
            && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            && !matches!(key.as_str(), "id" | "ids" | "params" | "preview");
        if !ok {
            return Err(format!("parameter name `{key}` is not allowed"));
        }
        valid_spec(spec).map_err(|why| format!("parameter `{key}`: {why}"))?;
        dialog::validate(spec, declared.get(..i).unwrap_or_default()).map_err(|why| format!("parameter `{key}`: {why}"))?;
    }
    Ok(())
}

/// A manifest's parameters, in the order its text gives them: none when it has no `params`, as VectorCraft
/// reads it.
#[derive(Deserialize)]
struct Declared {
    #[serde(default)]
    params: Ordered,
}

/// A JSON object's entries in the order of its text, which `serde_json`'s own map doesn't keep.
#[derive(Default)]
struct Ordered(Vec<(String, Value)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        /// Reads an object's entries one by one.
        struct Entries;
        impl<'de> Visitor<'de> for Entries {
            type Value = Ordered;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a JSON object")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Ordered, A::Error> {
                let mut entries = Vec::new();
                while let Some(entry) = map.next_entry()? {
                    entries.push(entry);
                }
                Ok(Ordered(entries))
            }
        }
        deserializer.deserialize_map(Entries)
    }
}

/// Checks the file extensions a manifest of `kind` lists, as VectorCraft does from 0.14: an exporter lists 1
/// to 8 distinct ones, each 1 to 16 lower-case letters and digits, and a filter or an effect lists none.
fn valid_extensions(kind: &str, extensions: Option<&Value>) -> Result<(), String> {
    let listed = match extensions {
        None => Vec::new(),
        Some(Value::Array(listed)) => listed.iter().map(|e| e.as_str().ok_or("an extension must be a string")).collect::<Result<_, _>>()?,
        Some(_) => return Err("`extensions` must be a list".into()),
    };
    if kind != "exporter" {
        return if listed.is_empty() { Ok(()) } else { Err("only an exporter lists `extensions`".into()) };
    }
    if listed.is_empty() || listed.len() > limits::MAX_EXTENSIONS {
        return Err(format!("an exporter lists 1 to {} `extensions`", limits::MAX_EXTENSIONS));
    }
    for (i, extension) in listed.iter().enumerate() {
        let well_formed =
            (1..=limits::MAX_EXTENSION_CHARS).contains(&extension.len()) && extension.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit());
        if !well_formed {
            return Err(format!("extension `{extension}` must be 1–16 lower-case letters and digits, without the dot"));
        }
        if listed.get(..i).is_some_and(|before| before.contains(extension)) {
            return Err(format!("extension `{extension}` is listed twice"));
        }
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
        for bad in [
            json!([]),
            json!(["DST"]),
            json!([".dst"]),
            json!([""]),
            json!(["abcdefghijklmnopq"]),
            json!(["dst", "dst"]),
            nine,
            json!("dst"),
            json!([1]),
        ] {
            assert!(validate(&exporting(bad.clone())).is_err(), "{bad}");
        }
        assert_eq!(validate(&exporting(json!(["dst", "dst2", "abcdefghijklmnop"]))), Ok(()));
        assert_eq!(validate(&exporting(json!(["a", "b", "c", "d", "e", "f", "g", "h"]))), Ok(()), "8, the most");
        assert!(validate(&with("kind", json!("exporter"))).is_err(), "an exporter without extensions");
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
}
