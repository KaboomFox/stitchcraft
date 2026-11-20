//! An expanded design written into the `.vectorcraft` document it came from (`REQ-CLI-025`, roadmap step
//! M13.26; `docs/src/design/image-to-embroidery.md` › Expand): what `stitch expand picture.vectorcraft -o
//! expanded.vectorcraft` writes, which VectorCraft opens and a person edits part by part.
//!
//! **What changes.** Each object of a layer that holds a shape of the automatic design is hidden and
//! locked, the traced group among them, so the person can compare and go back, as Image Trace keeps its
//! source image. A new group of the expanded parts goes on top of the layer that holds the first of them:
//! each part a path with its thread's paint and its StitchCraft effect, in the order the parts sew. A part
//! with a start, an end or a target is a group of its path and the markers that place them (`REQ-VC-009`),
//! and a guided fill a group of its path and its `stitch:guide` (`REQ-VC-032`). The design's origin and
//! stop position are markers in the group when the hidden objects held them. The document's `next_id`
//! counts past the new objects. Every other byte stays as VectorCraft saved it, as [`crate::named`] keeps a
//! traced document, and a compressed file is compressed again.
//!
//! **The parts' settings.** Each is written as a value of the kind VectorCraft's dialog holds: a toggle as
//! `true` or `false`, a number as a number when it reads back as the same text, and anything else as text,
//! which the reader reads as it is ([`crate::values::text`]). `satin_column` is the effect a stroke has
//! rather than a setting. What the cap holds the parts to is kept with the Design Settings filter's data
//! ([`crate::settings`]), with the design's other settings and its machine, in place of what it kept.
//!
//! **What it can't write.** A clipping object for a part's clips: the part is written whole, and a warning
//! names it (`SC-W0922`). Points go through VectorCraft's points and back, so they read to within a unit in
//! their last place: most products of a float and the points' size in millimetres can't be written so that
//! they read back exactly. A plan can move with a point a unit off, by a few needle points of 10,000 on 2 of
//! the traced dogs (`docs/src/design/image-to-embroidery.md` › Expand).

use serde_json::{Map, Number, Value, json};
use stitchcraft_core::units::MM_PER_POINT;
use stitchcraft_core::{Budget, Code, Diagnostic, Point};
use stitchcraft_engine::design::{Cap, Design, Element, FillRule, Join, Path, Shape};
use stitchcraft_engine::generate::stitch_type_of;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_params::Family;
use stitchcraft_plan::MachineProfile;

use crate::filter::{gradient, path_data, solid};
use crate::geometry::in_points;
use crate::settings::{SETTINGS_FILTER, kept_data};
use crate::{container, effect_id, plugin_id};

/// What [`written`] gives: the document, and what it couldn't write.
#[derive(Debug)]
pub struct Written {
    /// The document's bytes, compressed when the one given was.
    pub document: Vec<u8>,
    /// `SC-W0922` naming each part written without its clips.
    pub warnings: Vec<Diagnostic>,
}

/// The name of the group of the expanded parts.
pub const GROUP: &str = "Expanded embroidery";

/// `document`, the `.vectorcraft` file `original` was read from, with `expanded`, the design Expand made of
/// it for `machine`, which the document then keeps, written in (see the module's docs), with `budget`'s work: a unit for each byte read and each part
/// written. A document that is not JSON as VectorCraft writes it is `SC-E0009`, and work that runs out
/// `SC-E0004`.
pub fn written(document: &[u8], original: &Design, expanded: &Design, machine: &MachineProfile, budget: &Budget) -> Result<Written, Diagnostic> {
    let mut meter = budget.meter();
    let (whole, compressed) = container::whole(document, &mut meter)?;
    let end = whole.iter().position(|&b| b == 0).map_or(whole.len(), |nul| nul.saturating_sub(1));
    let (json, after) = whole.split_at_checked(end).unwrap_or((&whole, &[]));
    let work = json.len().saturating_add(expanded.elements().len());
    meter.charge(u64::try_from(work).unwrap_or(u64::MAX)).map_err(|exhausted| exhausted.diagnostic(budget, None))?;
    let filter = plugin_id(SETTINGS_FILTER);
    let scanned = scan(json, &filter).ok_or_else(broken)?;
    // The objects that hold the automatic design's shapes, by the ids its elements are read with.
    let sewn: Vec<u64> = original.elements().iter().filter_map(|element| object_of(element.id.as_str())).collect();
    let hidden: Vec<&Top> = scanned.tops.iter().filter(|top| top.ids.iter().any(|id| sewn.contains(id))).collect();
    let layer = hidden.first().and_then(|top| scanned.layers.iter().find(|layer| layer.close > top.open)).or(scanned.layers.first());
    let Some(layer) = layer else { return Err(broken()) };
    let mut next = scanned.next_id.map_or(1, |(_, _, id)| id).max(scanned.largest.saturating_add(1));
    let mut warnings = Vec::new();
    let mut parts = Vec::with_capacity(expanded.elements().len());
    for element in expanded.elements() {
        if !element.clips.is_empty() {
            warnings.push(unclipped(element));
        }
        parts.push(part(element, expanded, &mut next));
    }
    // The origin and the stop position, when the hidden objects held their markers.
    let markers = [("stitch:origin", expanded.settings.origin), ("stitch:stop-position", expanded.settings.stop_position)];
    for (name, at) in markers {
        let held = at.is_some() && !hidden.is_empty() && json_holds(json, name);
        if let Some(at) = at.filter(|_| held) {
            parts.push(marker(name, at, &mut next));
        }
    }
    let group = json!({"id": take(&mut next), "name": GROUP, "kind": {"type": "group", "children": parts}});
    let mut edits: Vec<(usize, usize, String)> = Vec::new();
    for top in &hidden {
        for (key, span) in [("visible", top.visible), ("locked", top.locked)] {
            let value = if key == "visible" { "false" } else { "true" };
            match span {
                Some((at, len)) => edits.push((at, len, value.to_string())),
                None => edits.push((top.open + 1, 0, format!("\"{key}\": {value}, "))),
            }
        }
    }
    let comma = if layer.empty { "" } else { ", " };
    edits.push((layer.close, 0, format!("{comma}{group}")));
    let span = |at: usize| scanned.spans.iter().find(|(kept, _)| *kept == at).map(|(_, span)| *span);
    let document_at = span(0).map(|(open, _)| open).ok_or_else(broken)?;
    match scanned.next_id {
        Some((at, len, _)) => edits.push((at, len, next.to_string())),
        None => edits.push((document_at + 1, 0, format!("\"next_id\": {next}, "))),
    }
    // What the cap holds the parts to, with the design's other settings and the machine the document keeps.
    let data = kept_data(&expanded.settings, machine).to_string();
    let quoted = serde_json::to_string(&filter).map_err(|_| broken())?;
    let object = |at: Option<(usize, usize)>| at.filter(|(open, _)| json.get(*open) == Some(&b'{'));
    let opened = |(open, close): (usize, usize)| {
        let empty = json.get(open + 1..close.saturating_sub(1)).is_some_and(|inside| inside.iter().all(u8::is_ascii_whitespace));
        (open + 1, if empty { "" } else { ", " })
    };
    edits.push(match (span(3), object(span(2)), object(span(1))) {
        (Some((open, close)), _, _) => (open, close - open, data),
        (None, Some(plugins), _) => {
            let (at, comma) = opened(plugins);
            (at, 0, format!("{quoted}: {data}{comma}"))
        }
        (None, None, Some(unknown)) => match span(2) {
            Some((open, close)) => (open, close - open, format!("{{{quoted}: {data}}}")),
            None => {
                let (at, comma) = opened(unknown);
                (at, 0, format!("\"pluginData\": {{{quoted}: {data}}}{comma}"))
            }
        },
        // Foreign data that isn't an object is a document VectorCraft doesn't open.
        (None, None, None) if span(1).is_some() => return Err(broken()),
        (None, None, None) => (document_at + 1, 0, format!("\"unknown\": {{\"pluginData\": {{{quoted}: {data}}}}}, ")),
    });
    let mut out = spliced(json, edits).ok_or_else(broken)?;
    out.extend_from_slice(after);
    Ok(Written { document: if compressed { container::packed(&out) } else { out }, warnings })
}

/// `SC-E0009`: the document isn't JSON as VectorCraft writes it.
fn broken() -> Diagnostic {
    Diagnostic::new(Code::InternalCheckFailed, "The expanded document can't be written: the document given isn't JSON as VectorCraft writes it.")
}

/// `SC-W0922` for a part written without its clips.
fn unclipped(element: &Element) -> Diagnostic {
    let name = element.name.as_deref().map_or_else(|| format!("`{}`", element.id), |name| format!("\"{name}\""));
    let message =
        format!("Expand couldn't keep everything. {name} is cut to a clipping shape, and the document has it whole: it sews all of its shape there.");
    Diagnostic::new(Code::AutoExpandLost, message).with_element(element.id.clone())
}

/// The number of the object the element `id` is read from: the label after `vc:`, when it is a number.
fn object_of(id: &str) -> Option<u64> {
    id.strip_prefix("vc:")?.split([':', '#', '~']).next()?.parse().ok()
}

/// Whether `json` names an object `name`, in any place.
fn json_holds(json: &[u8], name: &str) -> bool {
    let quoted = format!("\"{name}\"");
    json.windows(quoted.len()).any(|window| window == quoted.as_bytes())
}

/// The next id, which is then taken.
fn take(next: &mut u64) -> u64 {
    let id = *next;
    *next = next.saturating_add(1);
    id
}

/// `element` as a VectorCraft object with its StitchCraft effect, its ids taken from `next`: a path, or a
/// group of its path and the markers of its hints.
fn part(element: &Element, design: &Design, next: &mut u64) -> Value {
    let family = stitch_type_of(element, &design.settings).map_or(
        match element.shape {
            Shape::Fill { .. } => Family::Fill,
            Shape::Stroke { .. } => Family::Stroke,
        },
        |stitch| stitch.family(),
    );
    let params: Map<String, Value> =
        element.params.iter().filter(|(key, _)| *key != SATIN_COLUMN).map(|(key, value)| (key.to_string(), valued(value))).collect();
    let effect = json!({"id": effect_id(family.id()), "params": params, "visible": true});
    let (items, rule) = match &element.shape {
        Shape::Fill { rule, gradient: painted, .. } => {
            let paint = painted.as_ref().map_or_else(|| solid(element.thread.color), gradient).unwrap_or_default();
            let rule = match rule {
                FillRule::NonZero => "NonZero",
                FillRule::EvenOdd => "EvenOdd",
            };
            (vec![json!({"kind": "fill", "paint": paint})], Some(rule))
        }
        Shape::Stroke { width, join, cap, .. } => {
            let paint = solid(element.thread.color).unwrap_or_default();
            let (join, limit) = match join {
                Join::Miter { limit } => ("Miter", Some(*limit)),
                Join::Round => ("Round", None),
                Join::Bevel => ("Bevel", None),
            };
            let cap = match cap {
                Cap::Butt => "Butt",
                Cap::Round => "Round",
                Cap::Square => "Square",
            };
            let mut stroke = json!({"kind": "stroke", "paint": paint, "width": width.get() / MM_PER_POINT, "join": join, "cap": cap});
            if let (Some(limit), Some(stroke)) = (limit, stroke.as_object_mut()) {
                stroke.insert("miter_limit".to_string(), json!(limit));
            }
            // A stroke no wider than 0 isn't drawn: a fill of the thread's paint gives it its thread, and its
            // corners and ends are read as a stroke's without a style.
            let filled = (width.get() <= 0.0).then(|| json!({"kind": "fill", "paint": paint}));
            (filled.into_iter().chain([stroke]).collect(), None)
        }
    };
    let mut kind = json!({"type": "path", "path": path_data(element.shape.path()).unwrap_or_default()});
    if let (Some(rule), Some(kind)) = (rule, kind.as_object_mut()) {
        kind.insert("rule".to_string(), json!(rule));
    }
    let mut object = json!({"id": take(next), "appearance": {"items": items, "effects": [effect]}, "kind": kind});
    if let (Some(name), Some(object)) = (&element.name, object.as_object_mut()) {
        object.insert("name".to_string(), json!(name));
    }
    let hints = &element.hints;
    let mut markers: Vec<Value> = [("stitch:start", hints.start), ("stitch:end", hints.end), ("stitch:target", hints.target)]
        .into_iter()
        .filter_map(|(name, at)| Some(marker(name, at?, next)))
        .collect();
    markers.extend(hints.guide.as_ref().map(|guide| line("stitch:guide", &guide.path, next)));
    markers.extend(hints.anchor.as_ref().map(|anchor| line("stitch:anchor", anchor, next)));
    if markers.is_empty() {
        return object;
    }
    let children: Vec<Value> = [object].into_iter().chain(markers).collect();
    json!({"id": take(next), "kind": {"type": "group", "children": children}})
}

/// A marker named `name` at `at`: a path of one anchor there, which places it.
fn marker(name: &str, at: Point, next: &mut u64) -> Value {
    let one = in_points(at);
    let data = json!({"subpaths": [{"anchors": [{"p": one}], "closed": false}]});
    json!({"id": take(next), "name": name, "kind": {"type": "path", "path": data}})
}

/// A marker line named `name` along `path`, a guide or an anchor.
fn line(name: &str, path: &Path, next: &mut u64) -> Value {
    json!({"id": take(next), "name": name, "kind": {"type": "path", "path": path_data(path).unwrap_or_default()}})
}

/// A setting's text as the value VectorCraft's dialog holds for it: a toggle, a number that reads back as
/// the same text, or the text.
fn valued(text: &str) -> Value {
    match text {
        "true" => Value::Bool(true),
        "false" => Value::Bool(false),
        _ => match text.parse::<f64>().ok().and_then(Number::from_f64) {
            Some(number) if crate::values::text(&Value::Number(number.clone())).as_deref() == Some(text) => Value::Number(number),
            _ => Value::String(text.to_string()),
        },
    }
}

/// `json` with `edits` made, each at its place in place of its length; `None` when one lies outside it.
fn spliced(json: &[u8], mut edits: Vec<(usize, usize, String)>) -> Option<Vec<u8>> {
    edits.sort_by_key(|edit| edit.0);
    let mut out = Vec::with_capacity(json.len() + edits.iter().map(|edit| edit.2.len()).sum::<usize>());
    let mut from = 0;
    for (at, len, text) in edits {
        out.extend_from_slice(json.get(from..at)?);
        out.extend_from_slice(text.as_bytes());
        from = at + len;
    }
    out.extend_from_slice(json.get(from..)?);
    Some(out)
}

/// An object of a layer, as [`scan`] finds it.
#[derive(Debug, Default)]
struct Top {
    /// Where its opening brace is.
    open: usize,
    /// The ids of it and of everything inside it.
    ids: Vec<u64>,
    /// Where its `visible` value is, and how long, when it has one.
    visible: Option<(usize, usize)>,
    /// Where its `locked` value is, and how long, when it has one.
    locked: Option<(usize, usize)>,
}

/// A layer's list of objects, as [`scan`] finds it.
#[derive(Debug)]
struct Layer {
    /// Where its closing bracket is.
    close: usize,
    /// Whether it holds nothing.
    empty: bool,
}

/// What [`scan`] finds in a document's JSON.
#[derive(Debug, Default)]
struct Scanned {
    tops: Vec<Top>,
    layers: Vec<Layer>,
    /// The document's `next_id`: where its value is, how long, and what it is.
    next_id: Option<(usize, usize, u64)>,
    /// Where the values of the [`KEPT`] paths are, from their first byte to past their last, by path.
    spans: Vec<(usize, (usize, usize))>,
    /// The largest object id.
    largest: u64,
}

/// The paths of keys whose values [`scan`] finds: the document, its foreign data, what filters keep there,
/// and the Design Settings filter's. The filter's id is put in its place when the scan starts.
const KEPT: [&[&str]; 4] =
    [&["document"], &["document", "unknown"], &["document", "unknown", "pluginData"], &["document", "unknown", "pluginData", ""]];

/// A container being scanned: an object, with the key being read, or a list; each with its depth in lists of
/// objects, the path of keys to it, and where it opens.
enum Frame {
    Object { key: Option<String>, depth: usize, top: Option<usize>, is_top: bool, path: Vec<String>, open: usize },
    List { children: bool, depth: usize, items: usize, top: Option<usize>, path: Vec<String>, open: usize },
}

impl Frame {
    /// The path of keys to the value that starts inside it now: its own and, in an object, the key read.
    fn inner(&self) -> Option<Vec<String>> {
        match self {
            Frame::Object { key: Some(key), path, .. } => Some(path.iter().cloned().chain([key.clone()]).collect()),
            Frame::Object { .. } | Frame::List { .. } => None,
        }
    }
}

/// The objects of the layers of the JSON text `json`, the layers' lists, the document's `next_id` and the
/// [`KEPT`] values, as VectorCraft writes them, `filter` the Design Settings filter's id: an object in a
/// `children` list of a layer, which is in no other such list, is a layer's. `None` when it isn't JSON.
fn scan(json: &[u8], filter: &str) -> Option<Scanned> {
    let text = std::str::from_utf8(json).ok()?;
    let kept = |path: &[String]| {
        KEPT.iter().position(|want| want.len() == path.len() && want.iter().zip(path).all(|(w, p)| if w.is_empty() { p == filter } else { w == p }))
    };
    let mut found = Scanned::default();
    let mut stack: Vec<Frame> = Vec::new();
    let mut i = 0;
    while let Some(&byte) = json.get(i) {
        let path = stack.last().and_then(Frame::inner).unwrap_or_default();
        match byte {
            b'{' | b'[' => {
                let (children, depth, top) = match stack.last_mut() {
                    Some(Frame::List { children, depth, items, top, .. }) => {
                        *items += 1;
                        (false, *depth + usize::from(*children), *top)
                    }
                    Some(Frame::Object { key, depth, top, .. }) => (key.as_deref() == Some("children"), *depth, *top),
                    None => (false, 0, None),
                };
                let in_children = matches!(stack.last(), Some(Frame::List { children: true, depth: 0, .. }));
                if byte == b'[' {
                    stack.push(Frame::List { children, depth, items: 0, top, path, open: i });
                } else {
                    let is_top = in_children && byte == b'{';
                    let top = if is_top {
                        found.tops.push(Top { open: i, ..Top::default() });
                        Some(found.tops.len() - 1)
                    } else {
                        top
                    };
                    stack.push(Frame::Object { key: None, depth, top, is_top, path, open: i });
                }
            }
            b'}' | b']' => {
                let (path, open) = match stack.pop()? {
                    Frame::Object { path, open, .. } if byte == b'}' => (path, open),
                    Frame::List { children, depth, items, path, open, .. } if byte == b']' => {
                        if children && depth == 0 {
                            found.layers.push(Layer { close: i, empty: items == 0 });
                        }
                        (path, open)
                    }
                    _ => return None,
                };
                found.spans.extend(kept(&path).map(|at| (at, (open, i + 1))));
            }
            b'"' => {
                let end = string_end(json, i)?;
                let content = text.get(i + 1..end)?;
                match stack.last_mut() {
                    Some(Frame::Object { key: key @ None, .. }) => *key = Some(content.to_string()),
                    Some(Frame::List { items, .. }) => *items += 1,
                    _ => found.spans.extend(kept(&path).map(|at| (at, (i, end + 1)))),
                }
                i = end;
            }
            b',' => {
                if let Some(Frame::Object { key, .. }) = stack.last_mut() {
                    *key = None;
                }
            }
            b':' | b' ' | b'\n' | b'\r' | b'\t' => {}
            _ => {
                let len =
                    json.get(i..)?.iter().position(|b| matches!(b, b',' | b'}' | b']' | b' ' | b'\n' | b'\r' | b'\t')).unwrap_or(json.len() - i);
                let token = text.get(i..i + len)?;
                found.spans.extend(kept(&path).map(|at| (at, (i, i + len))));
                match stack.last_mut() {
                    Some(Frame::Object { key: Some(key), depth, top, is_top, .. }) => {
                        let place = top.and_then(|top| found.tops.get_mut(top));
                        match key.as_str() {
                            "id" => {
                                let id = token.parse::<u64>().ok();
                                found.largest = found.largest.max(id.unwrap_or(0));
                                if let (Some(id), Some(place)) = (id, place.filter(|_| *depth > 0)) {
                                    place.ids.push(id);
                                }
                            }
                            "visible" if *is_top => place.into_iter().for_each(|place| place.visible = place.visible.or(Some((i, len)))),
                            "locked" if *is_top => place.into_iter().for_each(|place| place.locked = place.locked.or(Some((i, len)))),
                            "next_id" if path == ["document", "next_id"] => found.next_id = token.parse().ok().map(|id| (i, len, id)),
                            _ => {}
                        }
                    }
                    Some(Frame::List { items, .. }) => *items += 1,
                    _ => {}
                }
                i += len;
                continue;
            }
        }
        i += 1;
    }
    stack.is_empty().then_some(found)
}

/// Where the JSON string that opens at `open` in `bytes` ends: its closing quote.
fn string_end(bytes: &[u8], open: usize) -> Option<usize> {
    let mut i = open + 1;
    loop {
        match bytes.get(i)? {
            b'\\' => i += 2,
            b'"' => return Some(i),
            _ => i += 1,
        }
    }
}

#[cfg(test)]
mod tests;
