//! The `.vectorcraft` JSON as the adapter reads it: the few fields it uses, and nothing else.
//!
//! **Why typed.** A document can hold megabytes the adapter never needs: images and previews as base64,
//! text runs, swatches, styles, symbols. Read into these types, they are skipped as the JSON is read and
//! never kept, where a generic JSON tree would hold all of them.
//!
//! **Why lenient.** VectorCraft adds kinds of objects, paints and colours without changing its format
//! version (Intertwine objects arrived in 0.12.0, still format 4). So kinds, paints, models, joins and
//! fill rules are kept as text, and a name the adapter doesn't read is reported, not a reason to refuse the
//! file. Fields have VectorCraft's defaults when a file leaves them out, as its own writer does. A number
//! VectorCraft wrote for a value that wasn't one is `null` in the file, and reads as `None`. Everything
//! else is as VectorCraft writes it, and a file it would refuse is refused.
//!
//! **Why coordinates are read from their text.** VectorCraft writes a number as the shortest decimal that
//! reads back to it, and reads it back exactly (it turns on serde_json's `float_roundtrip`). serde_json's
//! default reader can be a unit in the last place off on such decimals. Its exact reader would make the
//! plug-in's live effects, which share the workspace's serde_json, read their JSON a third slower than
//! M0.6's contract test allows. So a coordinate is taken as its JSON text and read by Rust's own exact
//! parser ([`Coordinate`]), and a point is where VectorCraft draws it.
//!
//! Field names are VectorCraft's (`crates/doc` and `crates/geom` of its source), which its format keeps.

use serde::de::{self, IgnoredAny, SeqAccess, Unexpected, Visitor};
use serde::{Deserialize, Deserializer};
use serde_json::Value;
use serde_json::value::RawValue;

/// A whole file: its format's name and version, and the document.
#[derive(Deserialize)]
pub(crate) struct File {
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default)]
    pub version: Option<Value>,
    #[serde(default)]
    pub document: Option<Document>,
}

/// The document: its layers, bottom first.
#[derive(Deserialize)]
pub(crate) struct Document {
    #[serde(default)]
    pub layers: Vec<Node>,
}

/// One object: a layer, a group, a path, a compound path, or a kind the adapter leaves out.
#[derive(Deserialize)]
pub(crate) struct Node {
    #[serde(default)]
    pub id: Option<u64>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default)]
    pub appearance: Appearance,
    /// An opacity mask, when there is one.
    #[serde(default)]
    pub mask: Option<IgnoredAny>,
    /// Set on a Shaper group, which shows the shapes it makes of its first child.
    #[serde(default)]
    pub shaper: Option<IgnoredAny>,
    #[serde(default)]
    pub kind: Option<Kind>,
}

/// What an object is, by its `type`, with the fields of the kinds the adapter reads.
#[derive(Deserialize, Default)]
pub(crate) struct Kind {
    #[serde(rename = "type", default)]
    pub name: String,
    #[serde(default)]
    pub children: Vec<Node>,
    #[serde(default)]
    pub path: Option<PathData>,
    #[serde(default)]
    pub rule: Option<String>,
    #[serde(default)]
    pub clip: bool,
    #[serde(default)]
    pub template: bool,
    #[serde(default = "yes")]
    pub printable: bool,
    #[serde(default)]
    pub guide: bool,
    /// A path that is its clip group's outline, which VectorCraft draws no art of.
    #[serde(default)]
    pub clipping: bool,
}

/// How an object paints: its fills and strokes, in paint order, and its effects.
#[derive(Deserialize, Default)]
pub(crate) struct Appearance {
    #[serde(default)]
    pub items: Vec<Item>,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// A fill or a stroke of an appearance.
#[derive(Deserialize)]
pub(crate) struct Item {
    /// `fill` or `stroke`.
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub paint: Option<Paint>,
    #[serde(default)]
    pub opacity: Option<f64>,
    #[serde(default = "yes")]
    pub visible: bool,
    /// A stroke's weight in points.
    #[serde(default)]
    pub width: Option<f64>,
    #[serde(default)]
    pub join: Option<String>,
    #[serde(default)]
    pub miter_limit: Option<f64>,
    /// The brush a stroke is drawn with, by its name.
    #[serde(default)]
    pub brush: Option<String>,
    #[serde(default)]
    pub effects: Vec<Effect>,
}

/// What fills or strokes: by its `type`, `none`, `solid`, `gradient` or `pattern`.
#[derive(Deserialize)]
pub(crate) struct Paint {
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(default)]
    pub color: Option<Color>,
    /// The name of the swatch a solid colour comes from.
    #[serde(default)]
    pub swatch: Option<String>,
    #[serde(default)]
    pub gradient: Option<Gradient>,
}

/// A colour in its model: `rgb`, `cmyk` or `gray`, components from 0 to 1, or `lab`.
#[derive(Clone, Copy, Deserialize, PartialEq)]
pub(crate) struct Color {
    #[serde(default, deserialize_with = "model")]
    pub model: Model,
    #[serde(default)]
    pub r: Option<f64>,
    #[serde(default)]
    pub g: Option<f64>,
    /// Blue in RGB, b* in Lab.
    #[serde(default)]
    pub b: Option<f64>,
    #[serde(default)]
    pub c: Option<f64>,
    #[serde(default)]
    pub m: Option<f64>,
    #[serde(default)]
    pub y: Option<f64>,
    /// Black in CMYK, the amount of ink in grey.
    #[serde(default)]
    pub k: Option<f64>,
    #[serde(default)]
    pub l: Option<f64>,
    #[serde(default)]
    pub a: Option<f64>,
}

/// A colour model, by its name in the file.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) enum Model {
    Rgb,
    Cmyk,
    Gray,
    Lab,
    /// A model the adapter doesn't read, or none.
    #[default]
    Other,
}

/// A gradient: its stops, in order.
#[derive(Deserialize)]
pub(crate) struct Gradient {
    #[serde(default)]
    pub stops: Vec<Stop>,
}

/// A gradient's stop: its colour.
#[derive(Deserialize)]
pub(crate) struct Stop {
    #[serde(default)]
    pub color: Option<Color>,
}

/// A live effect in an appearance: its id, parameters and whether it is on.
#[derive(Deserialize)]
pub(crate) struct Effect {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub params: Value,
    #[serde(default = "yes")]
    pub visible: bool,
}

/// What a live effect is given (plug-in ABI v1): the object's geometry, in a list of one.
#[derive(Deserialize)]
pub(crate) struct EffectInput {
    #[serde(default)]
    pub objects: Vec<EffectObject>,
}

/// An object a live effect is given: its path, in document coordinates, and its fill rule (`nonzero` or
/// `evenodd`). Its `type` and `bounds` are not needed.
#[derive(Deserialize)]
pub(crate) struct EffectObject {
    #[serde(default)]
    pub path: Option<PathData>,
    #[serde(rename = "fillRule", default)]
    pub fill_rule: Option<String>,
}

/// A path: its subpaths.
#[derive(Deserialize)]
pub(crate) struct PathData {
    #[serde(default)]
    pub subpaths: Vec<SubPath>,
}

/// A run of anchors, closed back to the first or not.
#[derive(Deserialize)]
pub(crate) struct SubPath {
    #[serde(default)]
    pub anchors: Vec<Anchor>,
    #[serde(default)]
    pub closed: bool,
}

/// An anchor and its handles, at absolute positions; a handle left out is on the anchor.
#[derive(Deserialize)]
pub(crate) struct Anchor {
    pub p: WirePoint,
    #[serde(rename = "in", default)]
    pub h_in: Option<WirePoint>,
    #[serde(rename = "out", default)]
    pub h_out: Option<WirePoint>,
}

/// A point, written `[x, y]` (format 2 on) or `{"x": x, "y": y}` (format 1, and read in any): its
/// coordinates in points, or `None` when one of them is not a number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct WirePoint(pub Option<(f64, f64)>);

impl<'de> Deserialize<'de> for WirePoint {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(PointVisitor)
    }
}

struct PointVisitor;

impl<'de> Visitor<'de> for PointVisitor {
    type Value = WirePoint;

    fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str("a point as [x, y] or {\"x\": x, \"y\": y}")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<WirePoint, A::Error> {
        let Coordinate(x) = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(0, &self))?;
        let Coordinate(y) = seq.next_element()?.ok_or_else(|| de::Error::invalid_length(1, &self))?;
        if seq.next_element::<IgnoredAny>()?.is_some() {
            return Err(de::Error::invalid_length(3, &self));
        }
        Ok(WirePoint(x.zip(y)))
    }

    fn visit_map<A: de::MapAccess<'de>>(self, mut map: A) -> Result<WirePoint, A::Error> {
        let (mut x, mut y): (Option<Coordinate>, Option<Coordinate>) = (None, None);
        while let Some(key) = map.next_key::<std::borrow::Cow<'de, str>>()? {
            match key.as_ref() {
                "x" => x = Some(map.next_value()?),
                "y" => y = Some(map.next_value()?),
                _ => {
                    map.next_value::<IgnoredAny>()?;
                }
            }
        }
        let Coordinate(x) = x.ok_or_else(|| de::Error::missing_field("x"))?;
        let Coordinate(y) = y.ok_or_else(|| de::Error::missing_field("y"))?;
        Ok(WirePoint(x.zip(y)))
    }
}

/// A coordinate, read exactly from its JSON text by Rust's parser: `None` for `null`, which VectorCraft
/// writes for a number that is not one. A number too large for a float is refused, as serde_json refuses
/// it in VectorCraft.
struct Coordinate(Option<f64>);

impl<'de> Deserialize<'de> for Coordinate {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let raw = <&RawValue>::deserialize(deserializer)?;
        match raw.get() {
            "null" => Ok(Coordinate(None)),
            text => match text.parse::<f64>() {
                Ok(v) if v.is_finite() => Ok(Coordinate(Some(v))),
                _ => Err(de::Error::invalid_value(Unexpected::Other(text), &"a number or null")),
            },
        }
    }
}

/// A colour's `model`, by its name; a name the adapter doesn't read is [`Model::Other`].
fn model<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Model, D::Error> {
    let name = Option::<String>::deserialize(deserializer)?;
    Ok(match name.as_deref() {
        Some("rgb") => Model::Rgb,
        Some("cmyk") => Model::Cmyk,
        Some("gray") => Model::Gray,
        Some("lab") => Model::Lab,
        _ => Model::Other,
    })
}

fn yes() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn points_read_in_either_form_and_a_null_coordinate_reads_as_none() {
        let read = |json: &str| serde_json::from_str::<WirePoint>(json);
        assert_eq!(read("[1.5, -2]").unwrap(), WirePoint(Some((1.5, -2.0))));
        assert_eq!(read(r#"{"y": 4, "x": 3, "z": 9}"#).unwrap(), WirePoint(Some((3.0, 4.0))));
        assert_eq!(read("[null, 2]").unwrap(), WirePoint(None));
        assert_eq!(read(r#"{"x": 1, "y": null}"#).unwrap(), WirePoint(None));
        for wrong in ["[1]", "[1, 2, 3]", r#"{"x": 1}"#, r#"{"y": 1}"#, "7", r#""1 2""#, r#"["1", 2]"#, r#"{"x": 1, "y": true}"#, "[1e400, 0]"] {
            assert!(read(wrong).is_err(), "{wrong}");
        }
        // Exactly, where serde_json's default reader is a unit in the last place off.
        assert_eq!(read("[180.17933438838418, -1e-9]").unwrap(), WirePoint(Some((180.179_334_388_384_18, -1e-9))));
        assert!(read("7").unwrap_err().to_string().contains("a point as [x, y]"));
    }

    #[test]
    fn a_colour_model_is_known_by_its_name() {
        let model = |json: &str| serde_json::from_str::<Color>(json).unwrap().model;
        assert_eq!(model(r#"{"model": "rgb"}"#), Model::Rgb);
        assert_eq!(model(r#"{"model": "cmyk"}"#), Model::Cmyk);
        assert_eq!(model(r#"{"model": "gray"}"#), Model::Gray);
        assert_eq!(model(r#"{"model": "lab"}"#), Model::Lab);
        assert_eq!(model(r#"{"model": "hsv"}"#), Model::Other);
        assert_eq!(model(r#"{"model": null}"#), Model::Other);
        assert_eq!(model("{}"), Model::Other);
    }
}
