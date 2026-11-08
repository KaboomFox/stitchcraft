//! The Tartan filter: Object › Plug-ins › StitchCraft Tartan sets the sett of the selected paths' tartan fills
//! from a code of its stripes (`REQ-VC-021`), as Ink/Stitch's tartan editor sets one
//! (`docs/src/design/algorithms/tartan.md` › The editor).
//!
//! **Why a filter.** A sett is one setting, `tartan`, an object of JSON, which the fill dialog shows as text
//! from VectorCraft 0.14 ([`crate::manifest::TEXTS`]). Written by hand it is easy to get wrong, and a
//! threadcount, the form the Scottish Register of Tartans gives a sett in, isn't one. The filter's dialog
//! takes the code a person has, and how the sett is laid, and writes the setting the fill reads.
//!
//! **What it reads.** Its dialog's code, as Ink/Stitch's editor reads one ([`Draft::from_code`]): a palette
//! code, a threadcount of colour letters, whose letters stand for the register's shades, or the threadcount
//! the register's search gives with a palette of its own. Each thread of a threadcount is as wide as
//! `thread_width_mm`, 0.2 mm as in Ink/Stitch's editor. The code says whether the sett is mirrored. Then come
//! the sett's turn, its scale, how far it is moved, and the width up to which a stripe sews as a run.
//!
//! **What it writes.** Each selected path's own first StitchCraft effect, when it is the fill effect, gets
//! `fill_method` set to `tartan_fill` and `tartan` to the setting ([`Draft::setting`]), its other settings
//! kept as they were ([`Record::with`]). A path with no StitchCraft effect gets the fill effect with those 2
//! settings, after its other effects. The others stay as they are: a path whose first StitchCraft effect is
//! the stroke, the satin or the Auto effect, and one sewn by its group's.
//!
//! **What it refuses** (`SC-E0821`): a code that can't be read, one that names letters the register doesn't
//! use, which the fill would leave unsewn, one that gives no stripe, a setting longer than the fill's Sett
//! holds, and a selection without a path that takes the sett. A filter can't warn, so it says so and changes
//! nothing. Before VectorCraft 0.15, which keeps the records a filter gives, it declines with `SC-E0813`.
//!
//! **What it needs.** VectorCraft admits a record of the fill effect only while StitchCraft Fill is installed.
//! Reading the code and writing the setting are a few passes over the code's text, well within a light
//! filter's budget.

use serde_json::{Map, Value, json};
use stitchcraft_core::{Code, Diagnostic, Fix};
use stitchcraft_engine::generators::tartan::editor::Draft;
use stitchcraft_engine::generators::tartan::{DEFAULT_PALETTE, SETT};
use stitchcraft_params::{Family, StitchType};
use stitchcraft_vectorcraft::effect_error;
use stitchcraft_vectorcraft::filter::{self, Object, Out, Record};

use crate::dialog::own;
use crate::effect::{RunError, too_old};
use crate::manifest::{Param, Schema, limits};

/// The filter's name: its module's, and its plug-in id's last part.
pub const TARTAN: &str = "tartan";

/// The page that says how to set a tartan's sett in VectorCraft, which the options' help links to.
const PAGE: &str = "user/stitches/tartan-fill.html#in-vectorcraft";

/// The width of a thread of a threadcount the dialog starts from, in millimetres, as Ink/Stitch's editor's.
const THREAD: f64 = 0.2;

/// The dialog's rows: the code, the width of a thread, and how the sett is laid, each by its key, its kind
/// of value and its default.
const CODE: &str = "code";
const THREAD_WIDTH: &str = "thread_width_mm";
const LAID: [(&str, &str, &str, f64, f64, f64, &str); 5] = [
    ("rotate", "Rotate", "°", -180.0, 180.0, 0.0, "How far the sett is turned, in degrees clockwise as the canvas shows it."),
    ("scale", "Scale", "%", 1.0, 1_000.0, 100.0, "How much every stripe's width is scaled, in percent."),
    ("offset_x", "Move right", "mm", -1_000.0, 1_000.0, 0.0, "How far the sett is moved right, in millimetres."),
    ("offset_y", "Move down", "mm", -1_000.0, 1_000.0, 0.0, "How far the sett is moved down, in millimetres."),
    (
        "min_stripe_width",
        "Runs up to",
        "mm",
        0.0,
        100.0,
        1.0,
        "A stripe this wide or narrower, once scaled, is sewn as a run along its middle, and the stripes beside it share its width.",
    ),
];

/// The filter's dialog: the code, the width of a thread of a threadcount, and how the sett is laid, with the
/// defaults Ink/Stitch's editor starts from.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    let code = "The sett's stripes, in order: a palette code, each stripe a colour in brackets and its width in millimetres, `(#0000ff)/4 \
                (#ffffff)1`, or a threadcount, each stripe the Scottish Register of Tartans' colour letters and a count of threads, `B/24 W4`. \
                A slash marks the stripes the sett is mirrored at, and a sett between `...` repeats. A `?` before a width leaves its stripe \
                unsewn, and a `|` parts the warp's stripes from the weft's. The threadcount the register's search gives, with its palette, \
                reads too.";
    let thread = "How wide each thread of a threadcount is: its stripes are as wide as their counts of threads.";
    let mut rows = vec![
        (CODE, own(Schema::Text { default: DEFAULT_PALETTE, max_length: limits::MAX_TEXT }, "Code", code, PAGE)),
        (THREAD_WIDTH, with_unit(own(Schema::Number { min: 0.01, max: 10.0, default: THREAD }, "Thread width", thread, PAGE), "mm")),
    ];
    for (key, label, unit, min, max, default, help) in LAID {
        rows.push((key, with_unit(own(Schema::Number { min, max, default }, label, help, PAGE), unit)));
    }
    rows
}

/// `param` with its value in `unit`.
fn with_unit(mut param: Param, unit: &'static str) -> Param {
    param.ui.unit = Some(unit);
    param
}

/// The filter's run on `input`, the selected objects, with `params`, its dialog's: the selection with the sett
/// set, or why nothing changes. Parameters that aren't JSON are [`RunError::BadInput`]; a row left out is at
/// its default.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let named: Value = serde_json::from_slice(params).map_err(|_| RunError::BadInput)?;
    let set = filter::read(input, params).and_then(|given| setting(&named).and_then(|setting| set(&given.objects, &setting)));
    match set {
        Ok(objects) => filter::output(&objects).map_err(|_| RunError::Encode),
        Err(why) => Ok(effect_error(&why)),
    }
}

/// The `tartan` setting of the sett the dialog's rows `named` give, as the module's docs say; or why there is
/// none.
fn setting(named: &Value) -> Result<String, Diagnostic> {
    let number = |key: &str, default: f64| named.get(key).and_then(Value::as_f64).unwrap_or(default);
    let code = named.get(CODE).and_then(Value::as_str).unwrap_or(DEFAULT_PALETTE);
    let drafted = Draft::from_code(code, number(THREAD_WIDTH, THREAD)).map_err(|why| refused(&format!("The sett's code can't be read: {why}.")))?;
    if !drafted.unread.is_empty() {
        let names: Vec<String> = drafted.unread.iter().map(|letters| format!("`{letters}`")).collect();
        let message = format!(
            "The sett's code names {}, which the Scottish Register of Tartans uses for no colour, and the fill would leave unsewn.",
            names.join(" and ")
        );
        return Err(refused(&message));
    }
    if drafted.draft.warp.is_empty() && drafted.draft.weft.as_ref().is_none_or(Vec::is_empty) {
        return Err(refused("The sett's code gives no stripe."));
    }
    let [rotate, scale, offset_x, offset_y, narrowest] = LAID.map(|(key, .., default, _)| number(key, default));
    let draft = Draft { rotate, scale, offset: [offset_x, offset_y], narrowest, ..drafted.draft };
    let setting = draft.setting();
    if setting.chars().count() > limits::MAX_TEXT {
        let message =
            format!("The sett's setting is {} characters long, and the fill's Sett holds at most {}.", setting.chars().count(), limits::MAX_TEXT);
        return Err(refused(&message));
    }
    Ok(setting)
}

/// What the filter gives back for `objects`, the selection, with the sett `setting`: each path that takes it
/// with it, as the module's docs say, and the others as they are; or why nothing changes.
fn set(objects: &[Object], setting: &str) -> Result<Vec<Out>, Diagnostic> {
    let changes = [("fill_method", json!(StitchType::TartanFill.id())), (SETT, json!(setting))];
    let mut out = Vec::with_capacity(objects.len());
    let mut taken = 0;
    for object in objects {
        let Some(effects) = object.effects() else {
            let message = "This VectorCraft keeps no effect record a filter gives, as VectorCraft does from 0.15, so no sett was set.";
            return Err(too_old(message, "0.15"));
        };
        let first = effects.iter().position(|record| record.visible && record.is_stitchcraft());
        let records = match first.and_then(|at| effects.get(at).map(|record| (at, record))) {
            Some((at, record)) if record.family() == Some(Family::Fill) => {
                let changed = record.with(&changes).map_err(bug)?;
                effects.iter().enumerate().map(|(n, record)| if n == at { changed.clone() } else { record.clone() }).collect()
            }
            None if !effects.iter().any(Record::is_stitchcraft) => {
                let params: Map<String, Value> = changes.iter().map(|(key, value)| ((*key).to_string(), value.clone())).collect();
                effects.into_iter().chain([Record::ours(Family::Fill.id(), &params).map_err(bug)?]).collect()
            }
            _ => {
                out.push(Out::given(object));
                continue;
            }
        };
        taken += 1;
        out.push(Out::given(object).with_effects(records));
    }
    if taken == 0 {
        return Err(nothing(objects.len()));
    }
    Ok(out)
}

/// `SC-E0821` with `message`, and how to write a code.
fn refused(message: &str) -> Diagnostic {
    Diagnostic::new(Code::VcSettNotSet, message).with_fix(Fix::Hint(
        "Write each stripe as a colour in brackets and its width, `(#0000ff)/4 (#ffffff)1`, or as the register's colour letters and a count \
         of threads, `B/24 W4`."
            .into(),
    ))
}

/// `SC-E0821`: none of the `count` objects selected takes a sett.
fn nothing(count: usize) -> Diagnostic {
    let message = match count {
        0 => "Nothing is selected to take a sett.".to_string(),
        _ => format!(
            "None of the {count} objects selected takes a sett: a path takes one when its own first StitchCraft effect is the fill effect, or \
             when it has no StitchCraft effect, and the filter then gives it the fill effect."
        ),
    };
    let hint = "Select the paths to sew as a tartan, and put the fill effect on the paths themselves rather than on their group.";
    Diagnostic::new(Code::VcSettNotSet, message).with_fix(Fix::Hint(hint.into()))
}

/// `SC-E0009` for a record the filter can't write, because of `why`, which a setting never makes.
fn bug(why: impl std::fmt::Display) -> Diagnostic {
    Diagnostic::new(Code::InternalCheckFailed, format!("The sett could not be written: {why}."))
}

#[cfg(test)]
mod tests;
