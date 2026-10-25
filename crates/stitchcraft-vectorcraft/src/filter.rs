//! What a filter is given and gives back (VectorCraft's plug-in ABI v1): the selected paths and compound
//! paths, read as the engine reads paths, with the stroke a satin column drawn as one path would follow,
//! and the objects a filter gives back, changed or new, with their paints and effect records. StitchCraft's
//! tools are filters, Stroke to Satin and Test Sheet (roadmap step M6.12, `REQ-VC-014`, `REQ-VC-015`).
//!
//! **What a filter is given** (VectorCraft's `docs/plugins.md`, Input). Each selected path and compound
//! path, bottom first, as `{id, type, name?, path, fillRule, fills, strokes, effects, opacity, bounds}`, its
//! `effects` from VectorCraft 0.15. Beside the parameters, `_context` names the artboard under the
//! selection's centre, else the document's first, and from 0.16 holds `documentData`, which VectorCraft
//! gives every filter from then. A path reads as a document's does (`crate::geometry`), and the stroke a
//! stroke or a satin effect would follow is the one a live effect follows (`crate::effect`).
//!
//! **What is handed back unchanged stays as it was.** VectorCraft keeps a paint's swatch, and an effect
//! record as the object had it, only when a filter hands them back as it gave them, which it tells by
//! comparing their values. It reads numbers exactly, and the workspace's serde_json may read one a unit in
//! the last place off (`crate::wire`). So a paint, a record and an opacity handed back are kept as their
//! JSON text ([`RawValue`]), and written back as it.
//!
//! **What it writes** (Output). An object is given back by its id, changed or not, since VectorCraft deletes
//! one a filter leaves out, or is new, and goes just above the object listed before it ([`Out`]). A path is
//! written in points, each coordinate the shortest decimal that reads back to it, and the reader turns it
//! back into millimetres to within a unit in its last place. Each subpath is its anchors, with the handles
//! of its curves, a quadratic one raised to the cubic it is. A closed subpath that ends where it starts
//! leaves its last anchor out, since VectorCraft closes a subpath back to its first ([`path_data`]). A
//! colour is an RGB paint the reader reads as the same colour ([`solid`]), a stroke has its width, join and
//! cap ([`stroke`]), and a new StitchCraft effect record has its parameters ([`Record::ours`]).

use serde::de::IgnoredAny;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::value::{RawValue, to_raw_value};
use serde_json::{Map, Value, json};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Diagnostic, Exhausted, Meter, Mm, Point};
use stitchcraft_engine::design::{Cap, FillRule, Join, Path, Segment};
use stitchcraft_params::Family;
use stitchcraft_plan::Rgb;

use crate::document::quoted;
use crate::effect::{followed, refused_plugin};
use crate::geometry::{self, Unusable};
use crate::wire::{EffectStroke, PathData, yes};
use crate::{color, effect_id, effect_name};

/// What a filter is given, read.
pub struct Given {
    /// The selected paths and compound paths, bottom first.
    pub objects: Vec<Object>,
    /// The centre of the artboard `_context` names, in millimetres: the one under the selection's centre,
    /// else the document's first; `None` when the document has none.
    pub artboard_centre: Option<Point>,
    /// Whether `_context` holds `documentData`, which VectorCraft gives every filter from 0.16.
    pub document_data: bool,
}

/// What VectorCraft passes a filter's `vc_run`, read: its `input`, and of its `params` the `_context`.
/// `SC-E0806` when they don't read as the ABI says.
pub fn read(input: &[u8], params: &[u8]) -> Result<Given, Diagnostic> {
    let refused = |what: &str, e: serde_json::Error| refused_plugin("filter", &format!("{what} do not read: {e}."));
    let input: Input = serde_json::from_slice(input).map_err(|e| refused("Its objects", e))?;
    let params: Params = serde_json::from_slice(params).map_err(|e| refused("Its parameters", e))?;
    let context = params.context.unwrap_or_default();
    Ok(Given { objects: input.objects, artboard_centre: context.artboard.as_ref().and_then(centre), document_data: context.document_data })
}

/// A filter's input: the selected objects.
#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    objects: Vec<Object>,
}

/// A filter's parameters, of which only VectorCraft's `_context` is read here.
#[derive(Deserialize)]
struct Params {
    #[serde(rename = "_context", default)]
    context: Option<Context>,
}

/// What VectorCraft tells a filter beside its parameters.
#[derive(Default, Deserialize)]
struct Context {
    /// `[x0, y0, x1, y1]`, in points, or `null`.
    #[serde(default)]
    artboard: Option<Value>,
    #[serde(rename = "documentData", default, deserialize_with = "present")]
    document_data: bool,
}

/// `true` for whatever the key holds, `null` too: a key left out is never read, and is `false`.
fn present<'de, D: Deserializer<'de>>(deserializer: D) -> Result<bool, D::Error> {
    IgnoredAny::deserialize(deserializer).map(|_| true)
}

/// The centre of `rect`, `[x0, y0, x1, y1]` in points, in millimetres; `None` for anything else.
fn centre(rect: &Value) -> Option<Point> {
    let corners: Vec<f64> = rect.as_array()?.iter().map(Value::as_f64).collect::<Option<_>>()?;
    let [x0, y0, x1, y1] = <[f64; 4]>::try_from(corners).ok()?;
    Point::new((x0 + x1) / 2.0 * MM_PER_POINT, (y0 + y1) / 2.0 * MM_PER_POINT).ok()
}

/// A selected path or compound path a filter is given.
#[derive(Deserialize)]
pub struct Object {
    /// Its id, by which the filter's output names it.
    pub id: u64,
    /// Its name, if it has one.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    path: Option<PathData>,
    /// Its strokes, bottom first, each as VectorCraft gave it.
    #[serde(default)]
    strokes: Vec<Box<RawValue>>,
    /// Its effect records, in order, each as VectorCraft gave it; `None` before VectorCraft 0.15.
    #[serde(default)]
    effects: Option<Vec<Box<RawValue>>>,
    #[serde(default)]
    opacity: Option<Box<RawValue>>,
}

impl Object {
    /// Its name in quotes, as messages name an object, cut after 64 characters; `None` without one.
    pub fn quoted_name(&self) -> Option<String> {
        self.name.as_deref().map(quoted)
    }

    /// Its path in millimetres, each subpath charging `meter` as a document's path does; or why it can't be
    /// used, as the start of a sentence.
    pub fn path(&self, meter: &mut Meter) -> Result<Result<Path, &'static str>, Exhausted> {
        let Some(data) = &self.path else { return Ok(Err(Unusable::Nothing.why())) };
        Ok(geometry::path(&[data], meter)?.map_err(Unusable::why))
    }

    /// The stroke a stroke or a satin effect on the object would follow: the topmost of its strokes that
    /// paints and is wider than 0, as a live effect follows one; `None` without one. A stroke that doesn't
    /// read paints nothing.
    pub fn stroke(&self) -> Option<Stroke> {
        let strokes: Vec<EffectStroke> = self.strokes.iter().map(|raw| serde_json::from_str(raw.get()).unwrap_or_default()).collect();
        let (at, width, join, cap) = followed(&strokes)?;
        let painted: Painted<'_> = serde_json::from_str(self.strokes.get(at)?.get()).ok()?;
        Some(Stroke { width, join, cap, paint: painted.paint?.to_owned() })
    }

    /// Its effect records, in order; `None` when VectorCraft gave it none, as before 0.15, which keeps no
    /// record a filter gives.
    pub fn effects(&self) -> Option<Vec<Record>> {
        Some(self.effects.as_ref()?.iter().map(|raw| Record::given(raw)).collect())
    }
}

/// A stroke's paint, as VectorCraft gave it.
#[derive(Deserialize)]
struct Painted<'a> {
    #[serde(borrow, default)]
    paint: Option<&'a RawValue>,
}

/// The stroke an object's stroke or satin effect would follow: its width, join and cap as the reader takes
/// them, and its paint as VectorCraft gave it.
pub struct Stroke {
    /// How wide it is.
    pub width: Mm,
    /// How it turns its corners.
    pub join: Join,
    /// How it ends.
    pub cap: Cap,
    paint: Box<RawValue>,
}

impl Stroke {
    /// The stroke as a filter gives it back, `width` wide: its paint as VectorCraft gave it, which so keeps
    /// its swatch, with its join and cap.
    pub fn as_wide_as(&self, width: Mm) -> Result<Box<RawValue>, serde_json::Error> {
        stroke(&self.paint, width, self.join, self.cap)
    }
}

/// An effect record: one an object had, as VectorCraft gave it, or a new one of StitchCraft's.
#[derive(Clone, Debug)]
pub struct Record {
    /// The effect's id: `plugin.` and the plug-in's id for a plug-in's live effect.
    pub id: String,
    /// Whether it is on.
    pub visible: bool,
    raw: Box<RawValue>,
}

/// What a record says of itself: its effect's id, and whether it is on.
#[derive(Deserialize)]
struct Head {
    #[serde(default)]
    id: String,
    #[serde(default = "yes")]
    visible: bool,
}

impl Record {
    /// The record VectorCraft gave as `raw`. One that doesn't read is no StitchCraft effect's, and is handed
    /// back as it is.
    fn given(raw: &RawValue) -> Record {
        let head: Head = serde_json::from_str(raw.get()).unwrap_or(Head { id: String::new(), visible: true });
        Record { id: head.id, visible: head.visible, raw: raw.to_owned() }
    }

    /// A new record of StitchCraft's live effect `name`, a stitch family's id, with `params`, which
    /// VectorCraft checks against the effect's manifest and fills in with the defaults of those left out.
    pub fn ours(name: &str, params: &Map<String, Value>) -> Result<Record, serde_json::Error> {
        let id = effect_id(name);
        let raw = to_raw_value(&json!({"id": id, "params": params}))?;
        Ok(Record { id, visible: true, raw })
    }

    /// The stitch family whose StitchCraft effect it is; `None` for any other effect.
    pub fn family(&self) -> Option<Family> {
        let name = effect_name(&self.id)?;
        Family::ALL.iter().copied().find(|family| family.id() == name)
    }

    /// Whether it is one of StitchCraft's effects: a stitch family's, or one a newer StitchCraft has.
    pub fn is_stitchcraft(&self) -> bool {
        effect_name(&self.id).is_some()
    }
}

/// What an object is: a path, or a compound path of a member for each subpath.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum Type {
    Path,
    Compound,
}

/// An object a filter gives back: one it was given, by its id, or a new one. What it leaves out stays as it
/// was, or for a new object takes VectorCraft's defaults: a path without fills or strokes, at full opacity.
#[derive(Debug, Default, Serialize)]
pub struct Out {
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u64>,
    #[serde(rename = "type", skip_serializing_if = "Option::is_none")]
    kind: Option<Type>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<Box<RawValue>>,
    #[serde(rename = "fillRule", skip_serializing_if = "Option::is_none")]
    fill_rule: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fills: Option<Vec<Box<RawValue>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    strokes: Option<Vec<Box<RawValue>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    effects: Option<Vec<Box<RawValue>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    opacity: Option<Box<RawValue>>,
}

impl Out {
    /// `object`, given back as it is, or as the methods below change it.
    pub fn given(object: &Object) -> Out {
        Out { id: Some(object.id), ..Out::default() }
    }

    /// The geometry `path`, in millimetres: a path, or when `compound` a compound path of a member for each
    /// subpath ([`path_data`]).
    pub fn with_path(self, path: &Path, compound: bool) -> Result<Out, serde_json::Error> {
        let kind = if compound { Type::Compound } else { Type::Path };
        Ok(Out { kind: Some(kind), path: Some(path_data(path)?), ..self })
    }

    /// Its fill rule, for its fills.
    pub fn with_fill_rule(self, rule: FillRule) -> Out {
        let rule = match rule {
            FillRule::NonZero => "nonzero",
            FillRule::EvenOdd => "evenodd",
        };
        Out { fill_rule: Some(rule), ..self }
    }

    /// Its fills' paints ([`solid`]), bottom first, replacing its own.
    pub fn with_fills(self, fills: Vec<Box<RawValue>>) -> Out {
        Out { fills: Some(fills), ..self }
    }

    /// Its strokes ([`stroke`]), bottom first, replacing its own.
    pub fn with_strokes(self, strokes: Vec<Box<RawValue>>) -> Out {
        Out { strokes: Some(strokes), ..self }
    }

    /// Its effect records, in order, replacing its own. VectorCraft keeps a record the object had as it
    /// was, and admits a new one as `effect.apply` would apply it.
    pub fn with_effects(self, effects: Vec<Record>) -> Out {
        Out { effects: Some(effects.into_iter().map(|record| record.raw).collect()), ..self }
    }

    /// Its name.
    pub fn with_name(self, name: String) -> Out {
        Out { name: Some(name), ..self }
    }

    /// The opacity `object` has, as VectorCraft gave it.
    pub fn with_opacity_of(self, object: &Object) -> Out {
        Out { opacity: object.opacity.clone(), ..self }
    }
}

/// A filter's output: `objects`, in the order VectorCraft places new ones by.
pub fn output(objects: &[Out]) -> Result<Vec<u8>, serde_json::Error> {
    /// The output's one key.
    #[derive(Serialize)]
    struct Output<'a> {
        objects: &'a [Out],
    }
    serde_json::to_vec(&Output { objects })
}

/// A solid paint of `color`, as a filter gives one: RGB components from 0 to 1, which the reader reads as
/// `color` again.
pub fn solid(color: Rgb) -> Result<Box<RawValue>, serde_json::Error> {
    let c = color::component;
    to_raw_value(&json!({"type": "solid", "color": {"model": "rgb", "r": c(color.r), "g": c(color.g), "b": c(color.b)}}))
}

/// A stroke as a filter gives one: of `paint`, `width` wide, with `join`'s corners and `cap`'s ends, named
/// as VectorCraft names them, which the reader reads as the same.
pub fn stroke(paint: &RawValue, width: Mm, join: Join, cap: Cap) -> Result<Box<RawValue>, serde_json::Error> {
    /// A stroke as a filter writes it.
    #[derive(Serialize)]
    struct Written<'a> {
        paint: &'a RawValue,
        width: f64,
        join: &'static str,
        cap: &'static str,
        #[serde(rename = "miterLimit", skip_serializing_if = "Option::is_none")]
        miter_limit: Option<f64>,
    }
    let (join, miter_limit) = match join {
        Join::Miter { limit } => ("miter", Some(limit)),
        Join::Round => ("round", None),
        Join::Bevel => ("bevel", None),
    };
    let cap = match cap {
        Cap::Butt => "butt",
        Cap::Round => "round",
        Cap::Square => "square",
    };
    to_raw_value(&Written { paint, width: width.get() / MM_PER_POINT, join, cap, miter_limit })
}

/// A path as VectorCraft's `PathData`: its subpaths.
#[derive(Serialize)]
struct Written {
    subpaths: Vec<Run>,
}

/// A subpath as VectorCraft's `PathData` holds it.
#[derive(Serialize)]
struct Run {
    closed: bool,
    anchors: Vec<Anchor>,
}

/// An anchor and its handles, in points.
#[derive(Serialize)]
struct Anchor {
    p: [f64; 2],
    #[serde(rename = "in", skip_serializing_if = "Option::is_none")]
    h_in: Option<[f64; 2]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    out: Option<[f64; 2]>,
}

/// `path`, in millimetres, as VectorCraft's `PathData` in points, which the reader reads as `path`, each
/// coordinate to within a unit in its last place. A subpath without segments draws nothing, and is left out.
pub fn path_data(path: &Path) -> Result<Box<RawValue>, serde_json::Error> {
    let at = |p: Point| geometry::in_points(p);
    let subpaths = path
        .subpaths
        .iter()
        .filter(|subpath| !subpath.segments.is_empty())
        .map(|subpath| {
            let mut anchors = vec![Anchor { p: at(subpath.start), h_in: None, out: None }];
            let mut from = subpath.start;
            for segment in &subpath.segments {
                let (out, h_in, end) = match *segment {
                    Segment::Line(end) => (None, None, end),
                    Segment::Quad(control, end) => {
                        let [a, c, b] = [at(from), at(control), at(end)];
                        let toward = |p: [f64; 2]| [p[0] + 2.0 / 3.0 * (c[0] - p[0]), p[1] + 2.0 / 3.0 * (c[1] - p[1])];
                        (Some(toward(a)), Some(toward(b)), end)
                    }
                    Segment::Cubic(first, second, end) => (Some(at(first)), Some(at(second)), end),
                };
                if let Some(last) = anchors.last_mut() {
                    last.out = out;
                }
                anchors.push(Anchor { p: at(end), h_in, out: None });
                from = end;
            }
            // VectorCraft closes a subpath back to its first anchor, which so stands for its last.
            if subpath.closed && from == subpath.start && anchors.len() > 1 {
                let h_in = anchors.pop().and_then(|last| last.h_in);
                if let Some(first) = anchors.first_mut() {
                    first.h_in = h_in;
                }
            }
            Run { closed: subpath.closed, anchors }
        })
        .collect();
    to_raw_value(&Written { subpaths })
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_engine::design::Subpath;

    use super::*;
    use crate::document::stroke_style;
    use crate::wire::Paint;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// What a filter is given for `objects`, with `context`.
    fn given(objects: &Value, context: &Value) -> Given {
        let input = serde_json::to_vec(&json!({ "objects": objects })).unwrap();
        read(&input, &serde_json::to_vec(&json!({"sheet": "TS-03", "_context": context})).unwrap()).unwrap()
    }

    #[test]
    fn a_filter_s_objects_and_context_read_as_vectorcraft_gives_them() {
        let path = json!({"subpaths": [{"anchors": [{"p": [0, 0]}, {"p": [72, 0]}]}]});
        let objects = json!([{"id": 4, "type": "path", "name": "Stem", "path": path, "effects": []}, {"id": 7, "type": "compound", "path": path}]);
        let read = given(&objects, &json!({"mode": "filter", "bounds": null, "artboard": [0, 0, 612, 792], "documentData": null}));
        assert_eq!(read.objects.iter().map(|o| (o.id, o.quoted_name())).collect::<Vec<_>>(), [(4, Some("\"Stem\"".to_string())), (7, None)]);
        // Effects from 0.15: none at all is a VectorCraft that gives none.
        assert_eq!(read.objects.iter().map(|o| o.effects().map(|e| e.len())).collect::<Vec<_>>(), [Some(0), None]);
        let centre = read.artboard_centre.unwrap();
        assert!((centre.x() - 107.95).abs() < 1e-9 && (centre.y() - 139.7).abs() < 1e-9, "{centre:?}");
        // `documentData` is there from 0.16, `null` until the filter keeps something.
        assert!(read.document_data);
        let older = given(&objects, &json!({"mode": "filter", "bounds": null, "artboard": null}));
        assert!(!older.document_data && older.artboard_centre.is_none());
        let line = read.objects[0].path(&mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert_eq!(line.subpaths[0].segments, [Segment::Line(p(25.4, 0.0))]);
        let nothing: Object = serde_json::from_value(json!({"id": 1})).unwrap();
        assert_eq!(nothing.path(&mut Budget::DEFAULT.meter()).unwrap(), Err("It draws nothing"));
    }

    #[test]
    fn diag_sc_e0806_what_a_filter_cannot_read_is_refused() {
        let refused = |input: &str, params: &str| read(input.as_bytes(), params.as_bytes()).err().unwrap().to_string();
        let start = "error SC-E0806: What VectorCraft passed the filter does not read as its plug-in ABI v1 says.";
        assert!(refused(r#"{"objects": [{"type": "path"}]}"#, "{}").starts_with(&format!("{start} Its objects do not read: missing field `id`")));
        assert!(refused(r#"{"objects": []}"#, "7").starts_with(&format!("{start} Its parameters do not read: invalid type: integer `7`")));
    }

    /// A stroke as VectorCraft gives a filter one, of `paint`, `width` points wide.
    fn stroke_of(paint: &Value, width: f64) -> Value {
        json!({"paint": paint, "width": width, "join": "round", "cap": "square", "miterLimit": 10})
    }

    #[test]
    fn the_stroke_followed_is_the_topmost_that_paints_and_keeps_its_paint_as_given() {
        // A colour component VectorCraft wrote exactly, with as many digits as one has.
        let blue = r#"{"type":"solid","color":{"model":"rgb","r":0.10196078431372549,"g":0.3058823529411765,"b":0.611764705882353},"swatch":"Navy"}"#;
        let input = format!(
            r#"{{"objects": [{{"id": 3, "strokes": [{stroke}, {stroke}, {{"paint": null, "width": 5}}, {{"paint": {blue}, "width": 0}}]}}]}}"#,
            stroke = format!(r#"{{"paint": {blue}, "width": 8.5, "join": "round", "cap": "square", "miterLimit": 10}}"#),
        );
        let read = read(input.as_bytes(), br#"{"_context": {}}"#).unwrap();
        let followed = read.objects[0].stroke().unwrap();
        assert_eq!((followed.width, followed.join, followed.cap), (Mm::from_points(8.5).unwrap(), Join::Round, Cap::Square));
        // Given back thinner, its paint is the text VectorCraft gave, which keeps the swatch.
        let back: Value = serde_json::from_str(followed.as_wide_as(Mm::SVG_PX).unwrap().get()).unwrap();
        assert!(followed.as_wide_as(Mm::SVG_PX).unwrap().get().contains(blue));
        assert!(back["width"].as_f64().is_some_and(|width| (width - 0.75).abs() < 1e-12), "{back}");
        assert_eq!((back["join"].as_str(), back["cap"].as_str()), (Some("round"), Some("square")));
        // A stroke without paint, without width, or that doesn't read, is none.
        for strokes in [json!([{"paint": null, "width": 2}]), json!([stroke_of(&json!({"type": "none"}), 2.0)]), json!([3]), json!([])] {
            let object: Object = serde_json::from_value(json!({"id": 1, "strokes": strokes})).unwrap();
            assert!(object.stroke().is_none(), "{strokes}");
        }
    }

    #[test]
    fn a_stroke_written_reads_back_as_it_was() {
        let black = to_raw_value(&json!({"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 0}})).unwrap();
        for join in [Join::UNSET, Join::Miter { limit: 10.0 }, Join::Round, Join::Bevel] {
            for cap in [Cap::Butt, Cap::Round, Cap::Square] {
                let written: EffectStroke = serde_json::from_str(stroke(&black, Mm::new(3.0).unwrap(), join, cap).unwrap().get()).unwrap();
                let (width, read_join, read_cap) = stroke_style(written.width, written.join.as_deref(), written.miter_limit, written.cap.as_deref());
                assert!(width.is_some_and(|width| (width.get() - 3.0).abs() < 1e-12), "{width:?}");
                assert_eq!((read_join, read_cap), (join, cap));
            }
        }
    }

    #[test]
    fn a_colour_written_reads_back_as_itself() {
        for color in [Rgb::new(0, 0, 0), Rgb::new(255, 255, 255), Rgb::new(26, 78, 156), Rgb::new(1, 254, 128)] {
            let paint: Paint = serde_json::from_str(solid(color).unwrap().get()).unwrap();
            assert_eq!(paint.color.as_ref().and_then(color::rgb), Some(color));
        }
    }

    #[test]
    fn records_are_handed_back_as_given_and_new_ones_are_stitchcraft_s() {
        let given = [
            r#"{"id":"plugin.dev.stitchcraft.satin","params":{"zigzag_spacing_mm":0.30000000000000004},"visible":false}"#,
            r#"{"id":"distort.roughen","params":{}}"#,
            "7",
        ];
        let object: Object = serde_json::from_str(&format!(r#"{{"id": 2, "effects": [{}]}}"#, given.join(","))).unwrap();
        let records = object.effects().unwrap();
        let read: Vec<(Option<Family>, bool, bool)> = records.iter().map(|r| (r.family(), r.is_stitchcraft(), r.visible)).collect();
        assert_eq!(read, [(Some(Family::Satin), true, false), (None, false, true), (None, false, true)]);
        for (record, text) in records.iter().zip(given) {
            assert_eq!(record.raw.get(), text);
        }
        let new = Record::ours("fill", &Map::from_iter([("angle".to_string(), json!(30))])).unwrap();
        assert_eq!((new.family(), new.visible), (Some(Family::Fill), true));
        assert_eq!(serde_json::from_str::<Value>(new.raw.get()).unwrap(), json!({"id": "plugin.dev.stitchcraft.fill", "params": {"angle": 30}}));
        // Another StitchCraft effect, from a newer StitchCraft, is StitchCraft's, of no family.
        let other = Record::given(&to_raw_value(&json!({"id": "plugin.dev.stitchcraft.auto"})).unwrap());
        assert_eq!((other.family(), other.is_stitchcraft()), (None, true));
    }

    /// `path` written, as the reader reads it back.
    fn round_trip(path: &Path) -> Path {
        let written = path_data(path).unwrap();
        let data: PathData = serde_json::from_str(written.get()).unwrap();
        geometry::path(&[&data], &mut Budget::DEFAULT.meter()).unwrap().unwrap()
    }

    /// Whether `a` and `b` are the same path, each point to within a millionth of a micrometre.
    fn alike(a: &Path, b: &Path) -> bool {
        let points = |path: &Path| path.subpaths.iter().map(|s| (s.closed, s.segments.len())).collect::<Vec<_>>();
        points(a) == points(b) && a.points().zip(b.points()).all(|(a, b)| a.distance(b) < 1e-9) && a.points().count() == b.points().count()
    }

    #[test]
    fn a_path_written_reads_back_as_it_was() {
        let line = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(10.0, 0.0)), Segment::Line(p(10.0, 5.0))], closed: false };
        let (r, k) = (5.0, 4.0 / 3.0 * (std::f64::consts::SQRT_2 - 1.0) * 5.0);
        let quarters = [[(r, k), (k, r), (0.0, r)], [(-k, r), (-r, k), (-r, 0.0)], [(-r, -k), (-k, -r), (0.0, -r)], [(k, -r), (r, -k), (r, 0.0)]];
        let circle = Subpath {
            start: p(r, 0.0),
            segments: quarters
                .iter()
                .map(|[a, b, c]| Segment::Cubic(p(a.0 + 108.0, a.1 - 140.0), p(b.0 + 108.0, b.1 - 140.0), p(c.0 + 108.0, c.1 - 140.0)))
                .collect(),
            closed: true,
        };
        let circle = Subpath { start: p(r + 108.0, -140.0), ..circle };
        let path = Path { subpaths: vec![line, circle.clone()] };
        assert!(alike(&round_trip(&path), &path));
        // The circle is 4 anchors: the last one is the first, whose handle in it holds.
        let written: Value = serde_json::from_str(path_data(&Path { subpaths: vec![circle] }).unwrap().get()).unwrap();
        let anchors = written["subpaths"][0]["anchors"].as_array().unwrap();
        assert_eq!((anchors.len(), written["subpaths"][0]["closed"].as_bool()), (4, Some(true)));
        assert!(anchors[0].get("in").is_some() && anchors[0].get("out").is_some());
        // A ring closed by VectorCraft's closing line reads back with that line as its last segment, to its
        // start; a quadratic curve as the cubic it is; and a subpath without segments isn't written.
        let ring = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(4.0, 0.0)), Segment::Quad(p(6.0, 2.0), p(4.0, 4.0))], closed: true };
        let dot = Subpath { start: p(1.0, 1.0), segments: Vec::new(), closed: false };
        let read = round_trip(&Path { subpaths: vec![ring, dot] });
        let [read] = read.subpaths.as_slice() else { panic!("{read:?}") };
        let [Segment::Line(_), Segment::Cubic(c1, c2, end), Segment::Line(back)] = read.segments.as_slice() else { panic!("{read:?}") };
        assert!(c1.distance(p(4.0 + 4.0 / 3.0, 4.0 / 3.0)) < 1e-9 && c2.distance(p(4.0 + 4.0 / 3.0, 4.0 - 4.0 / 3.0)) < 1e-9, "{c1:?} {c2:?}");
        assert!(end.distance(p(4.0, 4.0)) < 1e-9 && back.distance(p(0.0, 0.0)) < 1e-9);
    }

    #[test]
    fn an_object_given_back_holds_only_what_changes() {
        let object: Object = serde_json::from_value(json!({"id": 12, "opacity": 0.5, "path": {"subpaths": []}})).unwrap();
        let line = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(25.4, 0.0))], closed: false }] };
        let red = solid(Rgb::new(255, 0, 0)).unwrap();
        let new = Out::default()
            .with_path(&line, true)
            .unwrap()
            .with_fill_rule(FillRule::EvenOdd)
            .with_fills(vec![red.clone()])
            .with_strokes(Vec::new())
            .with_effects(vec![Record::ours("fill", &Map::new()).unwrap()])
            .with_name("ts03:a".to_string())
            .with_opacity_of(&object);
        let out: Value = serde_json::from_slice(&output(&[Out::given(&object), new]).unwrap()).unwrap();
        let red: Value = serde_json::from_str(red.get()).unwrap();
        assert_eq!(
            out,
            json!({"objects": [
                {"id": 12},
                {"type": "compound", "name": "ts03:a", "path": {"subpaths": [{"closed": false, "anchors": [{"p": [0.0, 0.0]}, {"p": [72.0, 0.0]}]}]},
                 "fillRule": "evenodd", "fills": [red], "strokes": [], "effects": [{"id": "plugin.dev.stitchcraft.fill", "params": {}}], "opacity": 0.5}
            ]})
        );
    }
}
