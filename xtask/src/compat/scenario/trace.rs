//! What the `trace` scenario gives VectorCraft to trace, and what it checks of what VectorCraft traced
//! (`compat/scenarios/trace.toml`, `docs/src/design/compatibility-gate.md` › Level B). From a picture to a
//! machine file, VectorCraft traces and StitchCraft embroiders (`docs/src/design/image-to-embroidery.md` ›
//! The flow): the scenario loads the reference machine's threads as a swatch library, places a picture drawn
//! in their colours, traces it with Image Trace's Document Library palette, and saves the paths.
//!
//! - **Data.** A step's parameter written `{ stitchcraft = "…" }` is given what StitchCraft makes of the
//!   name: `palette`, the reference machine's threads as `stitch palette` writes them, or `badge.png`, a
//!   test picture (`stitchcraft_engine::testsheets::pictures::badge`) drawn flat on white, as a PNG in
//!   base64. Its pixels are its shapes' colours or white, with no edge between them in another colour, so
//!   that the trace finds the shapes' colours, each a thread's. The badge is 200 pixels across in the
//!   middle of a picture 300 across, so that the white round it is the colour with the most pixels, which
//!   Image Trace's Abutting method traces lowest, as a picture's background. VectorCraft places a picture
//!   at 72 pixels to the inch, so the badge is placed 70.6 mm across, which the reference machine's hoop
//!   holds.
//! - **`traced`.** A document sews digitized automatically, as `stitch plan --auto` sews it: every shape is
//!   an automatic element, the white background is left out (`SC-W0906`), nothing else is said that stops
//!   it, and it sews the threads the check names, each the colour of a thread of the reference machine's
//!   palette.

use serde_json::Value;
use stitchcraft_cli::commands::palette;
use stitchcraft_cli::commands::plan::{Source, sew};
use stitchcraft_core::{Code, Severity};
use stitchcraft_engine::design::{FillRule, Reading, Segment, Shape};
use stitchcraft_engine::testsheets::pictures::{self, Picture, Trace};
use stitchcraft_plan::FormatId;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vc_plugin::export::base64;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Transform};

/// How many pixels across the picture of the badge is.
const PIXELS: u32 = 300;
/// How many pixels to the millimetre the badge, 50 mm across, is drawn in the middle of it.
const PER_MM: f32 = 4.0;

/// `value`, a step's parameters, with each object `{ "stitchcraft": "name" }` in it replaced by the data
/// StitchCraft makes of the name; or which name it makes nothing of.
pub(super) fn generated(value: Value) -> Result<Value, String> {
    Ok(match value {
        Value::Object(object) => match object.get("stitchcraft").and_then(Value::as_str).filter(|_| object.len() == 1) {
            Some(name) => data(name)?,
            None => Value::Object(object.into_iter().map(|(key, value)| Ok((key, generated(value)?))).collect::<Result<_, String>>()?),
        },
        Value::Array(items) => Value::Array(items.into_iter().map(generated).collect::<Result<_, _>>()?),
        other => other,
    })
}

/// The data StitchCraft makes of `name`, or why it makes none.
fn data(name: &str) -> Result<Value, String> {
    match name {
        "palette" => Ok(Value::String(palette::gpl(REFERENCE))),
        "badge.png" => {
            let badge = pictures::badge((25.0, 25.0), 1.0, Trace::Stacked).map_err(|e| e.to_string())?;
            Ok(Value::String(base64(&drawn(&badge, 50.0)?)))
        }
        other => Err(format!("StitchCraft makes no data called `{other}`: it makes `palette` and `badge.png`")),
    }
}

/// `picture`, `across` millimetres wide and high from its origin, drawn [`PER_MM`] pixels to the millimetre in
/// the middle of a PNG [`PIXELS`] across: each shape, lowest first, filled in its thread's colour on white,
/// its pixels whole.
fn drawn(picture: &Picture, across: f32) -> Result<Vec<u8>, String> {
    let mut pixmap = Pixmap::new(PIXELS, PIXELS).ok_or("a picture with no pixels")?;
    pixmap.fill(Color::WHITE);
    // A picture's size is far below 2^24 pixels, so its count of them is an exact float.
    #[allow(clippy::cast_precision_loss)]
    let margin = (PIXELS as f32 - across * PER_MM) / 2.0;
    let place = Transform::from_translate(margin, margin).pre_scale(PER_MM, PER_MM);
    for element in &picture.elements {
        let Shape::Fill { path, rule: FillRule::NonZero } = &element.shape else { return Err(format!("{} is no area", element.id)) };
        let mut outline = PathBuilder::new();
        #[allow(clippy::cast_possible_truncation)]
        let at = |p: stitchcraft_core::Point| (p.x() as f32, p.y() as f32);
        for subpath in &path.subpaths {
            let (x, y) = at(subpath.start);
            outline.move_to(x, y);
            for segment in &subpath.segments {
                match *segment {
                    Segment::Line(end) => outline.line_to(at(end).0, at(end).1),
                    Segment::Quad(control, end) => outline.quad_to(at(control).0, at(control).1, at(end).0, at(end).1),
                    Segment::Cubic(first, second, end) => {
                        outline.cubic_to(at(first).0, at(first).1, at(second).0, at(second).1, at(end).0, at(end).1);
                    }
                }
            }
            outline.close();
        }
        let outline = outline.finish().ok_or(format!("{} has no outline", element.id))?;
        let mut paint = Paint::default();
        let color = element.thread.color;
        paint.set_color_rgba8(color.r, color.g, color.b, 255);
        paint.anti_alias = false;
        pixmap.fill_path(&outline, &paint, tiny_skia::FillRule::Winding, place, None);
    }
    pixmap.encode_png().map_err(|e| format!("the PNG encoder failed: {e}"))
}

/// Whether `bytes`, a VectorCraft document, sews digitized automatically as [the module](self) says, in the
/// threads `threads` names (`#rrggbb`, in any order), or how it doesn't.
pub(super) fn traced(bytes: &[u8], threads: &[String]) -> Result<(), String> {
    let reading = Reading { auto: true, ..Reading::default() };
    let (design, _) = Source::VectorCraft.read(bytes, reading).map_err(|unread| unread.to_string())?;
    let set: Vec<&str> = design.elements().iter().filter(|e| !e.automatic).map(|e| e.id.as_str()).collect();
    if !set.is_empty() || design.elements().is_empty() {
        return Err(format!("{} shapes, and these are not automatic: {set:?}", design.elements().len()));
    }
    let sewn = sew(Source::VectorCraft, bytes, reading, REFERENCE, FormatId::PesV1, "trace");
    let stopped: Vec<String> = sewn.diagnostics.iter().filter(|d| d.severity() == Severity::Error).map(ToString::to_string).collect();
    let plan = sewn.plan.as_ref().filter(|_| stopped.is_empty()).ok_or(format!("it doesn't sew: {stopped:?}"))?;
    if !sewn.diagnostics.iter().any(|d| d.code == Code::AutoBackground) {
        return Err("its background isn't left out (SC-W0906)".to_string());
    }
    let mut sewing: Vec<String> = plan.blocks.iter().map(|block| hex(block.thread.color)).collect();
    sewing.sort();
    sewing.dedup();
    let mut named: Vec<String> = threads.iter().map(|thread| thread.to_ascii_lowercase()).collect();
    named.sort();
    if sewing != named {
        return Err(format!("it sews the threads {sewing:?}, not {named:?}"));
    }
    let palette = REFERENCE.palette.palette();
    match plan.blocks.iter().find(|block| !palette.entries.iter().any(|entry| entry.matchable && entry.color == block.thread.color)) {
        Some(block) => Err(format!("its thread {} is no thread of the {}", hex(block.thread.color), palette.name)),
        None => Ok(()),
    }
}

/// `color` as `#rrggbb`.
fn hex(color: stitchcraft_plan::Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", color.r, color.g, color.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_steps_data_is_what_stitchcraft_makes_of_its_name() {
        let params = serde_json::json!({"name": "threads.gpl", "data": {"stitchcraft": "palette"}, "list": [{"stitchcraft": "palette"}], "n": 1});
        let given = generated(params).unwrap();
        assert_eq!(given["data"], Value::String(palette::gpl(REFERENCE)));
        assert_eq!(given["list"][0], given["data"]);
        assert_eq!((&given["name"], &given["n"]), (&serde_json::json!("threads.gpl"), &serde_json::json!(1)));
        // An object with more than the name is left as it is, and a name StitchCraft makes nothing of is said.
        let more = serde_json::json!({"stitchcraft": "palette", "other": 1});
        assert_eq!(generated(more.clone()).unwrap(), more);
        assert!(generated(serde_json::json!({"stitchcraft": "nothing"})).unwrap_err().contains("`nothing`"));
    }

    #[test]
    fn a_document_with_settings_or_without_a_background_is_no_traced_picture() {
        // The patch fixture's objects have StitchCraft effects; without them, they are automatic, and none is
        // a background.
        let patch = crate::util::read_bytes(&crate::util::root().join("conformance/fixtures/vectorcraft/patch.vectorcraft")).unwrap();
        let threads = ["#ed171f".to_string()];
        let said = traced(&patch, &threads).unwrap_err();
        assert!(said.contains("and these are not automatic: ["), "{said}");
        let mut bare: Value = serde_json::from_slice(&patch).unwrap();
        strip(&mut bare);
        let said = traced(&serde_json::to_vec(&bare).unwrap(), &threads).unwrap_err();
        assert_eq!(said, "its background isn't left out (SC-W0906)");
    }

    /// `value` without the effects of any object in it.
    fn strip(value: &mut Value) {
        match value {
            Value::Object(object) => {
                object.remove("effects");
                object.values_mut().for_each(strip);
            }
            Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }

    #[test]
    fn the_badge_is_drawn_in_its_threads_colours_on_white_with_no_other() {
        let Value::String(_) = data("badge.png").unwrap() else { panic!("no base64") };
        let badge = pictures::badge((25.0, 25.0), 1.0, Trace::Stacked).unwrap();
        let pixmap = Pixmap::decode_png(&drawn(&badge, 50.0).unwrap()).unwrap();
        assert_eq!((pixmap.width(), pixmap.height()), (PIXELS, PIXELS));
        let mut colours: Vec<[u8; 3]> = pixmap.pixels().iter().map(|p| [p.red(), p.green(), p.blue()]).collect();
        colours.sort_unstable();
        colours.dedup();
        let threads = badge.elements.iter().map(|e| [e.thread.color.r, e.thread.color.g, e.thread.color.b]);
        let mut expected: Vec<[u8; 3]> = threads.chain([[255, 255, 255]]).collect();
        expected.sort_unstable();
        assert_eq!(colours, expected);
        // The middle is the gold disc's, and the red disc reaches 100 pixels from it. White has the most
        // pixels: Image Trace's Abutting method traces it lowest.
        let at = |x: u32, y: u32| pixmap.pixel(x, y).map(|p| [p.red(), p.green(), p.blue()]);
        assert_eq!(at(150, 150), Some([0xe8, 0xa9, 0x00]));
        assert_eq!((at(52, 150), at(48, 150)), (Some([0xed, 0x17, 0x1f]), Some([255, 255, 255])));
        let white = pixmap.pixels().iter().filter(|p| [p.red(), p.green(), p.blue()] == [255, 255, 255]).count();
        assert!(white * 2 > pixmap.pixels().len(), "{white}");
    }
}
