//! The design settings and the machine that StitchCraft's Design Settings filter keeps with a VectorCraft
//! document (`REQ-VC-013`, roadmap step M6.11), read and written here, in one place.
//!
//! **Where they are.** From 0.16 VectorCraft keeps up to 1 MiB of JSON with a document for each filter,
//! which the filter returns as `documentData`. The file keeps it in `document.unknown.pluginData`, under
//! the plug-in's id, and an exporter is given it with the rest of the document (VectorCraft's
//! `docs/plugins.md`, Output). A live effect is not given it, so a preview keeps the defaults and the
//! reference machine.
//!
//! **What they are.** The filter's data is an object: Ink/Stitch's design settings by their names, each a
//! length in millimetres ([`SETTINGS`]), and `profile`, the machine's id ([`kept_data`]):
//!
//! ```json
//! {"profile": "brother-pe800-5x7", "collapse_len_mm": 3, "min_stitch_len_mm": 0, "min_satin_stroke_width_mm": 1}
//! ```
//!
//! **How they read.** A length is a number of 0 or more, as the SVG adapter reads Ink/Stitch's, and a
//! machine is one of the built-in profiles. What doesn't read keeps its default, and so does a setting
//! named that this StitchCraft doesn't read, as a newer one may write, each with `SC-W0812`. Data that is
//! not an object keeps every default, with `SC-W0812` too. A document the filter never ran on has the
//! defaults and no machine of its own.

use serde_json::{Map, Value, json};
use stitchcraft_core::{Code, Diagnostic, Fix, Mm};
use stitchcraft_engine::design::{DesignSettings, SETTINGS, Setting};
use stitchcraft_plan::MachineProfile;
use stitchcraft_plan::profiles::{self, REFERENCE};

/// The name of the Design Settings filter's module: its plug-in id is [`crate::plugin_id`] of it.
pub const SETTINGS_FILTER: &str = "settings";

/// The key of the machine's id in the data the filter keeps, which is also its dialog's parameter.
pub const PROFILE: &str = "profile";

/// What the Design Settings filter keeps with a document, as the reader reads it.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct Kept {
    /// The design's settings: the defaults, but for those the filter keeps that read.
    pub(crate) settings: DesignSettings,
    /// The machine the document is planned for, when the filter keeps one StitchCraft has.
    pub(crate) machine: Option<&'static MachineProfile>,
}

/// The data the filter keeps for a design with `settings`, planned for `machine`.
pub fn kept_data(settings: &DesignSettings, machine: &MachineProfile) -> Value {
    let mut data = Map::new();
    data.insert(PROFILE.to_string(), json!(machine.id));
    for setting in &SETTINGS {
        data.insert(setting.name.to_string(), json!(setting.get(settings).get()));
    }
    Value::Object(data)
}

/// What the filter keeps with a document whose filters keep `plugin_data`, by plug-in id, with a warning
/// about each part that doesn't read. Like VectorCraft, it finds nothing kept in a value that isn't an
/// object.
pub(crate) fn kept(plugin_data: Option<&Value>, warnings: &mut Vec<Diagnostic>) -> Kept {
    let mut kept = Kept::default();
    let Some(data) = plugin_data.and_then(|all| all.get(crate::plugin_id(SETTINGS_FILTER).as_str())) else { return kept };
    let Some(entries) = data.as_object() else {
        warnings.push(ignored(&format!("The design settings kept with the document, {}, are not a list of settings", quoted(data))));
        return kept;
    };
    for (name, value) in entries {
        if name == PROFILE {
            kept.machine = value.as_str().and_then(profiles::find);
            if kept.machine.is_none() {
                warnings.push(ignored(&format!(
                    "The machine kept with the document, {}, is not one this StitchCraft has, so the design is planned for the machine \
                     `--profile` or the export names, or else for {}",
                    quoted(value),
                    REFERENCE.id
                )));
            }
        } else if let Some(setting) = Setting::named(name) {
            match value.as_f64().filter(|v| *v >= 0.0).and_then(|v| Mm::new(v).ok()) {
                Some(length) => setting.set(&mut kept.settings, length),
                None => warnings.push(ignored(&format!(
                    "The design setting `{name}` kept with the document, {}, is not a length of 0 or more, so it keeps its default",
                    quoted(value)
                ))),
            }
        } else {
            warnings.push(ignored(&format!("The document keeps a design setting this StitchCraft doesn't read, `{}`", shortened(name))));
        }
    }
    kept
}

/// `SC-W0812` saying `what`, with how to keep the settings anew.
fn ignored(what: &str) -> Diagnostic {
    Diagnostic::new(Code::VcSettingIgnored, format!("{what}."))
        .with_fix(Fix::Hint("Keep the settings anew with Object › Plug-ins › StitchCraft Design Settings.".to_string()))
}

/// The most characters of what the document keeps that a message quotes.
const QUOTED: usize = 40;

/// `value` as a message quotes it: its JSON, at most [`QUOTED`] characters of it.
fn quoted(value: &Value) -> String {
    shortened(&value.to_string())
}

/// `text` at most [`QUOTED`] characters long, an ellipsis after it when it is longer.
fn shortened(text: &str) -> String {
    match text.char_indices().nth(QUOTED) {
        Some((end, _)) => format!("{}…", text.get(..end).unwrap_or(text)),
        None => text.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::profiles::BROTHER_PE800_SMALL;

    use super::*;

    /// What a document keeps whose filters keep `data` for the Design Settings filter, and nothing else.
    fn read(data: &Value) -> (Kept, Vec<String>) {
        let mut warnings = Vec::new();
        let mut plugin_data = Map::new();
        plugin_data.insert(crate::plugin_id(SETTINGS_FILTER), data.clone());
        let kept = kept(Some(&Value::Object(plugin_data)), &mut warnings);
        (kept, warnings.iter().map(Diagnostic::one_line).collect())
    }

    #[test]
    fn what_the_filter_writes_reads_back() {
        let mut settings = DesignSettings::default();
        for (setting, value) in SETTINGS.iter().zip([5.0, 0.3, 2.0]) {
            setting.set(&mut settings, Mm::new(value).unwrap());
        }
        let data = kept_data(&settings, &BROTHER_PE800_SMALL);
        assert_eq!(
            data,
            json!({"profile": BROTHER_PE800_SMALL.id, "collapse_len_mm": 5.0, "min_stitch_len_mm": 0.3, "min_satin_stroke_width_mm": 2.0})
        );
        assert_eq!(read(&data), (Kept { settings, machine: Some(&BROTHER_PE800_SMALL) }, Vec::new()));
        // A document the filter never ran on has the defaults and no machine of its own.
        assert_eq!(kept(None, &mut Vec::new()), Kept::default());
        assert_eq!(read(&json!({})), (Kept::default(), Vec::new()));
    }

    #[test]
    fn diag_sc_w0812_a_setting_that_does_not_read_keeps_its_default() {
        let (kept, said) = read(&json!({"profile": "singer", "collapse_len_mm": -1, "min_stitch_len_mm": 0.5, "origin": [0, 0]}));
        assert_eq!(kept.machine, None);
        assert_eq!((kept.settings.collapse_len, kept.settings.min_stitch_len.map(Mm::get)), (DesignSettings::COLLAPSE_LEN, Some(0.5)));
        let hint = "Keep the settings anew with Object › Plug-ins › StitchCraft Design Settings.";
        assert_eq!(
            said,
            [
                format!(
                    "warning SC-W0812: The design setting `collapse_len_mm` kept with the document, -1, is not a length of 0 or more, so it keeps its default. {hint}"
                ),
                format!("warning SC-W0812: The document keeps a design setting this StitchCraft doesn't read, `origin`. {hint}"),
                format!(
                    "warning SC-W0812: The machine kept with the document, \"singer\", is not one this StitchCraft has, so the design is planned for the machine `--profile` or the export names, or else for brother-pe800-5x7. {hint}"
                ),
            ]
        );
        let (kept, said) = read(&json!(["a", "list"]));
        assert_eq!(kept, Kept::default());
        assert_eq!(
            said,
            [format!("warning SC-W0812: The design settings kept with the document, [\"a\",\"list\"], are not a list of settings. {hint}")]
        );
        // What a message quotes is short.
        let (_, said) = read(&json!({"x".repeat(60): 1, "profile": 5}));
        assert!(said.iter().all(|line| line.chars().count() < 400), "{said:?}");
        assert!(said.iter().any(|line| line.contains(&format!("`{}…`", "x".repeat(QUOTED)))), "{said:?}");
    }
}
