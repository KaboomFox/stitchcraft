//! The Stroke to Satin filter: Object › Plug-ins › StitchCraft Stroke to Satin makes each selected line into
//! a satin column drawn with its 2 rails and rungs, which sews as the line does with the satin effect
//! (`REQ-VC-014`), as Ink/Stitch's Stroke to Satin, once Convert Line to Satin, makes one whose rails and
//! rungs can then be edited.
//!
//! **What it makes.** Each subpath of each selected path that has a stroke is a line: the centre line of a
//! satin column as wide as the stroke the satin effect would follow ([`Object::stroke`]). The engine makes
//! it into the rails and rungs it makes of such a column when it sews one ([`from_line`]). Each column is a
//! compound path, a member for each rail and rung, without fills, which would fill its members, and with
//! the line's stroke one CSS pixel wide ([`Mm::SVG_PX`], the width the engine gives a stroke drawn without
//! one): its paint, which keeps its swatch, is the thread, the rails show, and the satin effect draws its
//! stitches thin. The first column takes the object's place, and the others go just above it, each with
//! the object's opacity. Each has the object's effects, with the satin effect in the place of the first of
//! its StitchCraft effects: the satin effect the line had, with its settings, or a new one at the defaults,
//! which VectorCraft admits only while StitchCraft Satin is installed. The others are left out. An object
//! the satin effect already reads as a column drawn with rails is left as it is.
//!
//! **What it refuses** (`SC-E0814`): nothing selected, a path without a stroke, a path that can't be used,
//! and a line that makes no column that sews as it does ([`Unmade`]). It then changes nothing. Before
//! VectorCraft 0.15, which keeps no effect record a filter gives, it declines with `SC-E0813`.
//!
//! **Its budget.** Making the rails of a curved line took up to 162 million instructions in VectorCraft's
//! host (M6.4, `docs/src/design/vectorcraft-integration.md`), more than the 50 million a light filter's run
//! gets. So it is a heavy filter, which VectorCraft 0.17 runs with [`HEAVY_FUEL`] instructions and 5
//! minutes, once the user allows it. The engine's work is priced as a preview's is ([`crate::preview`]), at
//! [`PER_UNIT`] instructions a unit, and the filter stops with `SC-E0004` before VectorCraft would stop it.
//! A VectorCraft before 0.17 runs every filter with a light run's budget, and stops one that needs more
//! with its own error.

use serde_json::Map;
use stitchcraft_core::{Code, Diagnostic, Fix, Meter, Mm};
use stitchcraft_engine::generators::satin::drawn::{Unmade, from_line};
use stitchcraft_engine::normalize::satin::{Shape as Column, recognize};
use stitchcraft_params::Family;
use stitchcraft_vectorcraft::effect_error;
use stitchcraft_vectorcraft::filter::{self, Object, Out, Record};

use crate::effect::{RunError, too_old};
use crate::preview::{HEAVY_FUEL, spendable, work};

/// The filter's name: its module's, and its plug-in id's last part.
pub const STROKE_TO_SATIN: &str = "stroke-to-satin";

/// The instructions VectorCraft's host runs for each unit of work the engine charges making the rails and
/// rungs of a line, reading and writing included: at most 360 on the lines M6.4 measured, the most a circle
/// 50 mm across, and a quarter more (`req_vc_014_*` in `compat/vc-contract`, which fails when a line it
/// converts runs out).
pub const PER_UNIT: u64 = 450;

/// The filter's run on `input`, the selected objects, with `params`, which hold only VectorCraft's
/// `_context`: each line made into satin columns, or why none was.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let mut meter = work(spendable(HEAVY_FUEL, input.len(), params.len()), 9, PER_UNIT).meter();
    let made = filter::read(input, params).and_then(|given| columns(&given.objects, &mut meter));
    match made {
        Ok(objects) => filter::output(&objects).map_err(|_| RunError::Encode),
        Err(why) => Ok(effect_error(&why)),
    }
}

/// What the filter gives back for `objects`, the selection, charging `meter`: each line made into satin
/// columns, and each column already drawn with rails as it is; or why it changes nothing.
fn columns(objects: &[Object], meter: &mut Meter) -> Result<Vec<Out>, Diagnostic> {
    if objects.is_empty() {
        let hint = "Select the lines, then choose Object › Plug-ins › StitchCraft Stroke to Satin again.";
        return Err(Diagnostic::new(Code::VcNotConverted, "Nothing is selected to make into a satin column.").with_fix(Fix::Hint(hint.into())));
    }
    let mut out = Vec::new();
    for (n, object) in objects.iter().enumerate() {
        let called = |what: &str| called(object, what, n + 1, objects.len());
        let Some(effects) = object.effects() else {
            let message = "This VectorCraft keeps no effect record a filter gives the objects it makes, as VectorCraft does from 0.15, so no line \
                           was made into a satin column.";
            return Err(too_old(message, "0.17"));
        };
        let path = match object.path(meter).map_err(|_| exhausted())? {
            Ok(path) => path,
            Err(why) => return Err(not_made(format!("{} can't be made into a satin column. {why}.", sentence(&called("path"))))),
        };
        let satin = effects.iter().find(|record| record.visible && record.family() == Some(Family::Satin)).cloned();
        if satin.is_some() && path.subpaths.len() > 1 && matches!(recognize(&path, meter).map_err(|_| exhausted())?.shape, Ok(Column::Rails(_))) {
            out.push(Out::given(object));
            continue;
        }
        let Some(stroke) = object.stroke() else {
            let message = format!("{} has no stroke that paints, which a satin column takes its width from.", sentence(&called("path")));
            return Err(not_made(message));
        };
        let effects = with_satin(effects, satin)?;
        let drawn = stroke.as_wide_as(Mm::SVG_PX).map_err(|e| unwritten(&e))?;
        let parts = path.subpaths.len();
        for (k, line) in path.subpaths.iter().enumerate() {
            let column = from_line(line, stroke.width, stroke.join, meter).map_err(|_| exhausted())?.map_err(|why: Unmade| {
                let line = if parts > 1 { format!("part {} of {}", k + 1, called("line")) } else { called("line") };
                not_made(format!("{} {why}.", sentence(&line)))
            })?;
            let made = if k == 0 { Out::given(object) } else { Out::default().with_opacity_of(object) };
            let made = made.with_path(&column.path(), true).map_err(|e| unwritten(&e))?;
            out.push(made.with_fills(Vec::new()).with_strokes(vec![drawn.clone()]).with_effects(effects.clone()));
        }
    }
    Ok(out)
}

/// `effects` with `satin`, or a new satin effect at the defaults, in the place of the first StitchCraft
/// effect among them, or after them, and without their other StitchCraft effects.
fn with_satin(effects: Vec<Record>, satin: Option<Record>) -> Result<Vec<Record>, Diagnostic> {
    let mut satin = Some(match satin {
        Some(satin) => satin,
        None => Record::ours(Family::Satin.id(), &Map::new()).map_err(|e| unwritten(&e))?,
    });
    let mut records = Vec::with_capacity(effects.len() + 1);
    for record in effects {
        if record.is_stitchcraft() {
            records.extend(satin.take());
        } else {
            records.push(record);
        }
    }
    records.extend(satin);
    Ok(records)
}

/// How a message names `object`, the `n`th of `count` selected, as a `what`: by its name, or else by where
/// it is in the selection.
fn called(object: &Object, what: &str, n: usize, count: usize) -> String {
    match (object.quoted_name(), count) {
        (Some(name), _) => format!("the {what} {name}"),
        (None, 1) => format!("the selected {what}"),
        (None, _) => format!("{what} {n} of the {count} selected"),
    }
}

/// `text` with a capital, to start a sentence.
fn sentence(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_uppercase().chain(chars).collect()).unwrap_or_default()
}

/// `SC-E0814` for a line `message` says can't be made into a satin column.
fn not_made(message: String) -> Diagnostic {
    let hint = "Give the path a stroke as wide as the column, draw that column with its 2 rails, or leave the object out of the selection.";
    Diagnostic::new(Code::VcNotConverted, message).with_fix(Fix::Hint(hint.into()))
}

/// `SC-E0004`: the lines need more work than the filter may do.
fn exhausted() -> Diagnostic {
    Diagnostic::new(Code::BudgetExhausted, "Making the selected lines into satin columns needs more work than VectorCraft lets a filter do.")
        .with_fix(Fix::Hint("Select fewer lines at a time.".into()))
}

/// `SC-E0009` for output that couldn't be written as JSON, which no column makes.
fn unwritten(e: &serde_json::Error) -> Diagnostic {
    Diagnostic::new(Code::InternalCheckFailed, format!("The satin columns could not be written: {e}."))
}

#[cfg(test)]
mod tests;
