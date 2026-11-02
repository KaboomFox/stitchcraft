//! The Test Sheet filter: Object › Plug-ins › StitchCraft Test Sheet draws one of `stitch testsheet`'s sheets
//! into the document (`REQ-VC-015`). A machine checkpoint can then start from VectorCraft: the sheet's
//! objects show their stitches, the exporters write it, and the dialogs show its settings.
//!
//! **Which sheets.** Those drawn as designs, TS-02B to TS-09 and the TS-13 sheets, whose elements are
//! shapes ([`TestSheet::design`]). The others are drawn stitch by stitch, as plans, which no object holds,
//! and only `stitch testsheet` writes them.
//!
//! **What it draws.** The sheet's design moved by whole millimetres, so that the middle of its bounds is as
//! near the middle of the artboard VectorCraft names as that allows: the artboard under the selection, or
//! else the first ([`placed`]). Each element is an object, named by its id, in sewing order, bottom first.
//! It has the element's path, a compound path for more than one subpath, its thread as the paint its family
//! sews with, a fill's fill or a stroke's and a satin column's stroke, and its settings as the record of
//! its family's StitchCraft effect. A shape the sheet digitizes automatically, as the TS-13 sheets' top
//! halves are ([`TestSheet::digitized`]), has the Auto effect's record instead (`REQ-VC-016`). Each setting
//! is the value its dialog row takes, and reads back as the setting it was (`record_params`); one a record
//! can't hold is a bug in the sheet, which the tests catch. The objects selected are given back as they
//! are, and the sheet goes just above them.
//!
//! **How it sews.** As the sheet's design, moved. A fill's rows lie where the design's origin puts them, so
//! across a shape moved they may fall a little elsewhere than `stitch testsheet` sews them, and look the
//! same. VectorCraft unlinks the swatch of a paint a filter gives, so the threads lose the names of the
//! Brother palette the sheets sew with; a machine file keeps the colour alone.
//!
//! **What it needs.** VectorCraft admits an effect record a filter gives only for an installed plug-in, so
//! StitchCraft Stroke, Satin and Fill must be installed, and StitchCraft Auto for the TS-13 sheets. It keeps
//! such records from 0.15. A 0.15 with nothing selected looks to a filter as a 0.14 does, which would drop
//! them, and only the `documentData` VectorCraft gives every filter from 0.16 tells them apart. So before 0.16
//! the filter declines with `SC-E0813`.

use serde_json::{Map, Value};
use stitchcraft_core::{Code, Diagnostic, Fix, Point};
use stitchcraft_engine::design::{Design, Element, Shape, within_working_limit};
use stitchcraft_engine::generate::stitch_type_of;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::registry::PARAMETERS;
use stitchcraft_engine::testsheets::{SHEETS, TestSheet};
use stitchcraft_params::{Family, ParamSet, ParamSpec, StitchType, find_for};
use stitchcraft_vectorcraft::filter::{self, Given, Out, Record, solid, stroke};
use stitchcraft_vectorcraft::{effect_error, param_text};

use crate::Module;
use crate::dialog::own;
use crate::effect::{RunError, too_old};
use crate::manifest::{Param, Schema};

/// The filter's name: its module's, and its plug-in id's last part.
pub const TESTSHEET: &str = "testsheet";
/// The parameter that names the sheet.
pub const SHEET: &str = "sheet";

/// The page that says how to draw a test sheet in VectorCraft, which the sheet's help links to.
const PAGE: &str = "user/how-to/embroider-a-vectorcraft-design.html#draw-a-test-sheet";

/// The sheets drawn as designs, which the filter draws, in protocol order.
pub fn sheets() -> impl Iterator<Item = &'static TestSheet> {
    SHEETS.iter().filter(|sheet| sheet.designed())
}

/// The filter's dialog: a choice of the sheets, by their ids, the first by default, with what each tests.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    let ids: Vec<&'static str> = sheets().map(|sheet| sheet.id).collect();
    let default = ids.first().copied().unwrap_or_default();
    let tests: Vec<String> = sheets().map(|sheet| format!("- {}: {}", sheet.id, sheet.title)).collect();
    let help = format!("The test sheet to draw, as the machine tests name it:\n\n{}", tests.join("\n"));
    vec![(SHEET, own(Schema::Choice { options: ids, default }, "Sheet", &help, PAGE))]
}

/// The filter's run on `input`, the selected objects, with `params`, its dialog's: the sheet drawn, or why it
/// isn't. A sheet its dialog doesn't offer is [`RunError::BadInput`], since VectorCraft passes only one it
/// offers.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let named: Value = serde_json::from_slice(params).map_err(|_| RunError::BadInput)?;
    let id = named.get(SHEET).and_then(Value::as_str);
    let sheet = sheets().find(|sheet| Some(sheet.id) == id).ok_or(RunError::BadInput)?;
    let drawn = filter::read(input, params).and_then(|given| {
        if !given.document_data {
            let message = "This VectorCraft doesn't tell a filter that it keeps the effect records a filter gives the objects it draws, as \
                           VectorCraft does from 0.16, so the test sheet wasn't drawn.";
            return Err(too_old(message, "0.16"));
        }
        objects(sheet, &given)
    });
    match drawn {
        Ok(objects) => filter::output(&objects).map_err(|_| RunError::Encode),
        Err(why) => Ok(effect_error(&why)),
    }
}

/// What the filter gives back for `sheet`, given `given`: the objects selected as they are, then the sheet's
/// elements, each an object, placed as [`placed`] places the sheet.
fn objects(sheet: &TestSheet, given: &Given) -> Result<Vec<Out>, Diagnostic> {
    let design = placed(sheet, given.artboard_centre)?;
    let declared = declared();
    let mut out: Vec<Out> = given.objects.iter().map(Out::given).collect();
    for element in design.elements() {
        out.push(object(sheet, element, &design, &declared)?);
    }
    Ok(out)
}

/// `sheet`'s design moved by whole millimetres, so that the middle of its bounds is as near to `centre` as
/// that allows; where `stitch testsheet` draws it without a centre. `SC-E0817` when parts of it would lie
/// more than 10 m from the origin there, farther than a design's points lie ([`within_working_limit`]), and
/// `SC-E0009` for a sheet the filter doesn't draw, or that can't be drawn, which is a bug.
pub fn placed(sheet: &TestSheet, centre: Option<Point>) -> Result<Design, Diagnostic> {
    let bug = |why: String| Diagnostic::new(Code::InternalCheckFailed, format!("The test sheet {} can't be drawn: {why}.", sheet.id));
    let design = sheet.design().ok_or_else(|| bug("it is drawn stitch by stitch".into()))?.map_err(|e| bug(e.to_string()))?;
    let (Some(centre), Some(bounds)) = (centre, design.bounds()) else { return Ok(design) };
    let middle = bounds.center();
    let [dx, dy] = [(centre.x() - middle.x()).round(), (centre.y() - middle.y()).round()];
    let reached = [bounds.min(), bounds.max()].into_iter().all(|corner| Point::new(corner.x() + dx, corner.y() + dy).is_ok_and(within_working_limit));
    if !reached {
        let message = format!(
            "The test sheet {} wasn't drawn: in the middle of the artboard, parts of it would lie more than 10 m from the document's \
             origin, farther than StitchCraft stitches.",
            sheet.id
        );
        let fix = Fix::Hint("Select an object on an artboard nearer the origin, or move the artboard nearer it, and draw the sheet again.".into());
        return Err(Diagnostic::new(Code::VcSheetNotDrawn, message).with_fix(fix));
    }
    design.moved([dx, dy])
}

/// The settings each StitchCraft effect declares, a stitch family's or the Auto effect's, by their keys, as
/// its manifest declares them.
fn declared() -> Vec<(Module, Vec<(&'static str, Schema)>)> {
    let effects = Family::ALL.iter().map(|family| Module::Family(*family)).chain([Module::Auto]);
    effects.map(|effect| (effect, effect.manifest().params.into_iter().map(|(key, param)| (key, param.schema)).collect())).collect()
}

/// `element` of `sheet`'s `design` as an object: its path, its thread as the paint its family sews with, and
/// its settings as its family's effect record, or the Auto effect's for an automatic element, by the
/// settings `declared`.
fn object(sheet: &TestSheet, element: &Element, design: &Design, declared: &[(Module, Vec<(&'static str, Schema)>)]) -> Result<Out, Diagnostic> {
    let bug =
        |why: String| Diagnostic::new(Code::InternalCheckFailed, format!("The test sheet {}'s {} can't be drawn: {why}.", sheet.id, element.id));
    let written = |e: serde_json::Error| bug(format!("it can't be written: {e}"));
    let effect = if element.automatic {
        Module::Auto
    } else {
        Module::Family(stitch_type_of(element, &design.settings).map(StitchType::family).ok_or_else(|| bug("it names no stitch".into()))?)
    };
    let schemas = declared.iter().find(|(each, _)| *each == effect).map_or(&[][..], |(_, schemas)| schemas.as_slice());
    let record = Record::ours(effect.name(), &record_params(effect, &element.params, schemas).map_err(bug)?).map_err(written)?;
    let path = element.shape.path();
    let out = Out::default().with_name(element.id.to_string()).with_path(path, path.subpaths.len() > 1).map_err(written)?;
    let paint = solid(element.thread.color).map_err(written)?;
    Ok(match &element.shape {
        Shape::Fill { rule, .. } => out.with_fill_rule(*rule).with_fills(vec![paint]),
        Shape::Stroke { width, join, cap, .. } => out.with_strokes(vec![stroke(&paint, *width, *join, *cap).map_err(written)?]),
    }
    .with_effects(vec![record]))
}

/// The parameters of the record of `effect`, a stitch family's effect or the Auto effect, that sew as
/// `params`, an element's: each as the value of its row in the dialog, by the settings the effect's manifest
/// declares, `schemas`; or which one a record can't hold. VectorCraft drops a setting the manifest doesn't
/// declare, refuses a value of another kind and clamps a number into its row's range, so each must be one
/// the row takes, and read back as the setting it was. A family's `satin_column` is left out: the effect
/// sets it ([`SATIN_COLUMN`]).
pub(crate) fn record_params(effect: Module, params: &ParamSet, schemas: &[(&'static str, Schema)]) -> Result<Map<String, Value>, String> {
    let mut record = Map::new();
    for (key, text) in params.iter().filter(|(key, _)| *key != SATIN_COLUMN) {
        let held = |why: &str| format!("its setting `{key} = {text}` {why} its {} effect's record", effect.name());
        let schema = schemas.iter().find(|(declared, _)| *declared == key).map(|(_, schema)| schema).ok_or_else(|| held("is no setting of"))?;
        let value = value(schema, text).ok_or_else(|| held("is no value of its row in"))?;
        let read = |text: &str| spec_of(effect, key).and_then(|spec| spec.kind.parse(text)).map(|(value, _)| value);
        if param_text(&value).is_none_or(|back| read(&back).is_none() || read(&back) != read(text)) {
            return Err(held("doesn't read back from"));
        }
        record.insert(key.to_string(), value);
    }
    Ok(record)
}

/// The registry's declaration of the setting `key` of `effect`'s record: its stitch family's, or one of the
/// settings of automatic digitizing for the Auto effect's.
fn spec_of(effect: Module, key: &str) -> Option<ParamSpec> {
    match effect {
        Module::Family(family) => find_for(PARAMETERS, key, family).map(|spec| spec.in_family(family)),
        _ => PARAMETERS.iter().filter(|group| group.automatic()).flat_map(|group| group.specs).find(|spec| spec.key == key).copied(),
    }
}

/// `text` as the value of a dialog row of `schema`, within its range: `None` for one the row doesn't take.
fn value(schema: &Schema, text: &str) -> Option<Value> {
    match schema {
        Schema::Number { min, max, .. } => text.trim().parse::<f64>().ok().filter(|v| (*min..=*max).contains(v)).map(Value::from),
        Schema::Int { min, max, .. } => text.trim().parse::<i64>().ok().filter(|v| (*min..=*max).contains(v)).map(Value::from),
        Schema::Bool { .. } => text.trim().parse::<bool>().ok().map(Value::Bool),
        Schema::Choice { options, .. } => options.contains(&text).then(|| Value::from(text)),
    }
}

#[cfg(test)]
mod tests;
