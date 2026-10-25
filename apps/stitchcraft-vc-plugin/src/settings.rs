//! The Design Settings filter: Object › Plug-ins › StitchCraft Design Settings keeps a document's design
//! settings and machine with it (`REQ-VC-013`, from VectorCraft 0.16).
//!
//! A filter returns the objects it changes, and may return `documentData`, which VectorCraft keeps with the
//! document for it (VectorCraft's `docs/plugins.md`, Output). This one changes no object: it returns each
//! object it is given as it is, by its id, since VectorCraft deletes an input object a filter leaves out.
//! Its data is its dialog's settings, as the VectorCraft adapter writes and reads them
//! ([`stitchcraft_vectorcraft::settings`]), and `stitch export` and the exporters plan the document with
//! them.
//!
//! VectorCraft opens a filter's dialog at its defaults, not at what the filter last kept, so the filter
//! keeps all of its settings each time it runs. A VectorCraft before 0.16 runs a filter too, and drops
//! the `documentData` it returns, so there the filter declines with `SC-E0813` rather than seem to keep
//! them.

use serde::Deserialize;
use serde_json::{Value, json};
use stitchcraft_core::{Code, Diagnostic, Fix, Mm};
use stitchcraft_engine::design::{DesignSettings, SETTINGS};
use stitchcraft_params::{Empty, Kind};
use stitchcraft_plan::profiles::{self, BUILTIN, REFERENCE};
use stitchcraft_vectorcraft::effect_error;
use stitchcraft_vectorcraft::settings::{PROFILE, kept_data};

use crate::dialog::{self, own};
use crate::effect::RunError;
use crate::export::{MACHINE_HELP, PROFILE_PAGE};
use crate::manifest::{Param, Schema};

/// The page that says how to keep a document's settings and machine with it, which each setting's help
/// links to.
const PAGE: &str = "user/how-to/embroider-a-vectorcraft-design.html#keep-the-designs-settings-and-machine-with-it";

/// The filter's dialog: the machine, a choice of the built-in profiles, the reference machine by default,
/// then each design setting ([`SETTINGS`]), a length from 0, at its default.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    let machines = BUILTIN.iter().map(|profile| profile.id).collect();
    let help = format!("{MACHINE_HELP} `stitch export` and File › Export plan the document for it unless they are told another.");
    let machine = own(Schema::Choice { options: machines, default: REFERENCE.id }, "Machine", &help, PROFILE_PAGE);
    let defaults = DesignSettings::default();
    let lengths = SETTINGS.iter().map(|setting| {
        let schema = Schema::Number { min: 0.0, max: setting.max, default: setting.get(&defaults).get() };
        let mut param = own(schema, setting.label, setting.help, PAGE);
        param.ui.unit = dialog::unit(Kind::Length { min: 0.0, max: setting.max, empty: Empty::Never });
        (setting.name, param)
    });
    std::iter::once((PROFILE, machine)).chain(lengths).collect()
}

/// What VectorCraft gives a filter: the selected paths and compound paths, each with its id.
#[derive(Deserialize)]
struct Input {
    #[serde(default)]
    objects: Vec<Object>,
}

/// An object a filter is given, by its id: the rest is left as it is.
#[derive(Deserialize)]
struct Object {
    id: u64,
}

/// The filter's run on `input`, with its dialog's `params`: each object as it is, and the settings to keep
/// with the document. Parameters that aren't what VectorCraft passes, every setting a number of 0 or more
/// and a machine its manifest offers, are [`RunError::BadInput`]. A VectorCraft before 0.16 gives the
/// filter no `documentData` in its `_context` and would drop what it returned, so there the filter
/// declines with `SC-E0813`.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    let input: Input = serde_json::from_slice(input).map_err(|_| RunError::BadInput)?;
    let params: Value = serde_json::from_slice(params).map_err(|_| RunError::BadInput)?;
    let mut settings = DesignSettings::default();
    for setting in &SETTINGS {
        let value = params.get(setting.name).and_then(Value::as_f64).filter(|v| *v >= 0.0).ok_or(RunError::BadInput)?;
        setting.set(&mut settings, Mm::new(value).map_err(|_| RunError::BadInput)?);
    }
    let machine = params.get(PROFILE).and_then(Value::as_str).and_then(profiles::find).ok_or(RunError::BadInput)?;
    if params.pointer("/_context/documentData").is_none() {
        return Ok(effect_error(&cannot_keep()));
    }
    let objects: Vec<Value> = input.objects.iter().map(|object| json!({"id": object.id})).collect();
    serde_json::to_vec(&json!({"objects": objects, "documentData": kept_data(&settings, machine)})).map_err(|_| RunError::Encode)
}

/// `SC-E0813`: the VectorCraft that ran the filter keeps no data with a document for it.
fn cannot_keep() -> Diagnostic {
    Diagnostic::new(
        Code::VcCannotKeep,
        "This VectorCraft keeps no data with a document for a filter, as VectorCraft does from 0.16, so the design settings were not kept.",
    )
    .with_fix(Fix::Hint("Update VectorCraft to 0.16 or later.".to_string()))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::profiles::{BROTHER_PE800_SMALL, REFERENCE};

    use super::*;
    use crate::manifest::{settings, validate};

    /// The parameters VectorCraft passes the filter at the dialog's defaults, with `set` over them.
    fn params(set: &Value) -> Vec<u8> {
        let mut params: Value = settings().params.iter().map(|(key, param)| ((*key).to_string(), param.schema.default_value())).collect();
        for (key, value) in set.as_object().unwrap() {
            params[key] = value.clone();
        }
        params["_context"] = json!({"mode": "filter", "documentData": null});
        serde_json::to_vec(&params).unwrap()
    }

    fn ran(input: &Value, set: &Value) -> Value {
        serde_json::from_slice(&run(&serde_json::to_vec(input).unwrap(), &params(set)).unwrap()).unwrap()
    }

    #[test]
    fn req_vc_013_the_filter_keeps_its_dialog_s_settings_and_changes_no_object() {
        let input = json!({"objects": [{"id": 12, "type": "path", "path": {"subpaths": []}}, {"id": 14, "type": "compound"}]});
        let out = ran(&input, &json!({"profile": BROTHER_PE800_SMALL.id, "collapse_len_mm": 10.0}));
        let settings = DesignSettings { collapse_len: Mm::new(10.0).unwrap(), ..DesignSettings::default() };
        assert_eq!(out, json!({"objects": [{"id": 12}, {"id": 14}], "documentData": kept_data(&settings, &BROTHER_PE800_SMALL)}));
        // At the defaults, with nothing selected: the defaults, for the reference machine.
        assert_eq!(
            ran(&json!({"objects": []}), &json!({})),
            json!({"objects": [], "documentData": kept_data(&DesignSettings::default(), REFERENCE)})
        );
    }

    #[test]
    fn req_vc_013_the_settings_filter_s_manifest_is_one_vectorcraft_accepts() {
        let manifest = settings();
        assert_eq!(validate(&serde_json::to_vec(&manifest).unwrap()), Ok(()));
        assert_eq!((manifest.id.as_str(), manifest.kind), ("dev.stitchcraft.settings", "filter"));
        // The machine, the reference machine by default, then each design setting, a length at its default.
        let defaults = DesignSettings::default();
        let machines = BUILTIN.iter().map(|profile| profile.id).collect();
        let mut expected = vec![(PROFILE, Schema::Choice { options: machines, default: REFERENCE.id })];
        expected.extend(
            SETTINGS.iter().map(|setting| (setting.name, Schema::Number { min: 0.0, max: setting.max, default: setting.get(&defaults).get() })),
        );
        let schemas: Vec<(&str, Schema)> = manifest.params.iter().map(|(key, param)| (*key, param.schema.clone())).collect();
        assert_eq!(schemas, expected);
        for (key, param) in &manifest.params {
            assert!(param.ui.label.is_some() && param.ui.help.is_some() && param.ui.help_url.is_some(), "{key}");
        }
        assert!(manifest.params.iter().skip(1).all(|(_, param)| param.ui.unit == Some("mm")));
    }

    #[test]
    fn diag_sc_e0813_a_vectorcraft_that_cannot_keep_the_settings_is_told_so() {
        // A VectorCraft before 0.16 gives a filter no `documentData` in its context, and would drop what the
        // filter returned as its own.
        let mut older: Value = serde_json::from_slice(&params(&json!({}))).unwrap();
        older["_context"] = json!({"mode": "filter", "bounds": null, "artboard": null});
        let out = run(&serde_json::to_vec(&json!({"objects": [{"id": 3}]})).unwrap(), &serde_json::to_vec(&older).unwrap()).unwrap();
        let out: Value = serde_json::from_slice(&out).unwrap();
        assert_eq!(out.as_object().map(|out| out.len()), Some(1), "only the error: {out}");
        let error = out["error"].as_str().unwrap();
        assert!(error.starts_with("error SC-E0813: ") && error.ends_with("Update VectorCraft to 0.16 or later."), "{error}");
    }

    #[test]
    fn parameters_that_are_not_what_vectorcraft_passes_are_bad_input() {
        let input = serde_json::to_vec(&json!({"objects": []})).unwrap();
        for bad in [json!({"profile": "singer"}), json!({"collapse_len_mm": -1.0}), json!({"min_stitch_len_mm": "short"})] {
            assert_eq!(run(&input, &params(&bad)), Err(RunError::BadInput), "{bad}");
        }
        assert_eq!(run(b"{", &params(&json!({}))), Err(RunError::BadInput));
        assert_eq!(run(&input, b"{}"), Err(RunError::BadInput));
    }
}
