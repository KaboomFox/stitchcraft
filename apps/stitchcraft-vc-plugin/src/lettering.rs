//! StitchCraft Lettering, a live effect for text objects (roadmap step M14.4, `REQ-VC-027`,
//! `docs/src/design/vectorcraft-integration.md` › Lettering).
//!
//! **What it is.** Its record on a text object keeps the embroidery font the text sews in and lettering's
//! settings, and the exporters read it with the document (`stitchcraft_vectorcraft::lettering`). The effect
//! itself changes nothing: it returns the geometry it is given.
//!
//! **Why it draws nothing.** A live effect is given its object's outlines and paints and nothing else
//! (the RFC's P6). On a text object those are the outlines of the screen font's glyphs. The text itself,
//! which lettering sets, isn't among them, so the effect can't draw the embroidery font's letters. The text
//! stays as VectorCraft draws it, and the export sews it in the embroidery font, with a warning when it
//! can't (`SC-W1111`).
//!
//! **Why a text field names the font.** A document keeps the font's name, and sews the same wherever it is
//! exported: in a plug-in built with other fonts, or by `stitch export` with a folder of fonts. A list of
//! the fonts built in would make the manifest differ from build to build, and VectorCraft checks a record's
//! value against the manifest of the plug-in installed.

use stitchcraft_svg::lettering::{BACK_AND_FORTH, COLOR_SORT, LETTER_SPACING, LINE_HEIGHT, SCALE, SETTINGS, Setting, TEXT_ALIGN, TRIM, WORD_SPACING};
use stitchcraft_vectorcraft::lettering::{DEFAULT_FONT, FONT};

use crate::dialog::own;
use crate::effect::RunError;
use crate::manifest::{Param, Schema};

/// The effect's page on the docs site.
const PAGE: &str = "user/how-to/letter-in-vectorcraft.html";

/// The most characters a font's name holds in the dialog.
const MAX_FONT_CHARS: usize = 128;

/// The settings of lettering the dialog shows, in its order, each with its row's label, and for a number
/// its unit and the range the dialog takes. The 2 that place a text along a path are not among them: a text
/// object's own place is where it sews.
const ROWS: [(Setting, &str, &str, f64, f64); 8] = [
    (SCALE, "Size", "%", 1.0, 1_000.0),
    (TEXT_ALIGN, "Align", "", 0.0, 0.0),
    (LETTER_SPACING, "Letter spacing", "mm", -100.0, 100.0),
    (WORD_SPACING, "Word spacing", "mm", -100.0, 100.0),
    (LINE_HEIGHT, "Line spacing", "mm", -100.0, 100.0),
    (BACK_AND_FORTH, "Sew lines back and forth", "", 0.0, 0.0),
    (COLOR_SORT, "Sort colours", "", 0.0, 0.0),
    (TRIM, "Trim", "", 0.0, 0.0),
];

/// The effect's dialog: the font, then lettering's settings by Ink/Stitch's names, each with lettering's
/// own default and help ([`SETTINGS`]). A setting takes a number, a toggle or one of its values, as its
/// `takes` says.
pub(crate) fn options() -> Vec<(&'static str, Param)> {
    let font = format!(
        "The embroidery font the text sews in, by its name or its folder's. Left empty, it is `{DEFAULT_FONT}`, Ink/Stitch Small Font. The \
         text keeps its own look in VectorCraft, and the export sews it in this font. The exporters hold the fonts they were built with, and \
         an export names them when it doesn't find this one."
    );
    let mut rows = vec![(FONT, own(Schema::Text { default: "", max_length: MAX_FONT_CHARS }, "Font", &font, PAGE))];
    for (setting, label, unit, min, max) in ROWS {
        debug_assert!(SETTINGS.contains(&setting));
        let schema = match setting.takes {
            "true|false" => Schema::Bool { default: setting.default == "true" },
            takes if takes.contains('|') => Schema::Choice { options: takes.split('|').collect(), default: setting.default },
            _ => Schema::Number { min, max, default: setting.default.parse().unwrap_or(0.0) },
        };
        let mut param = own(schema, label, &format!("{}.", setting.does), PAGE);
        param.ui.unit = Some(unit).filter(|unit| !unit.is_empty());
        rows.push((setting.name, param));
    }
    rows
}

/// The effect's run: the geometry of `input`, unchanged, whatever `params` say. The text sews when the
/// document is exported.
pub fn run(input: &[u8], params: &[u8]) -> Result<Vec<u8>, RunError> {
    crate::effect::hello(input, params)
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use stitchcraft_svg::lettering::Lettering;

    use super::*;

    #[test]
    fn req_vc_027_the_dialog_s_settings_are_lettering_s_own_at_its_defaults() {
        let rows = options();
        let names: Vec<&str> = rows.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, ["font", "scale", "text_align", "letter_spacing", "word_spacing", "line_height", "back_and_forth", "color_sort", "trim"]);
        // Each default, given to lettering by its name, leaves lettering's defaults as they are.
        let mut how = Lettering::default();
        for (name, param) in rows.iter().skip(1) {
            let value = stitchcraft_vectorcraft::param_text(&param.schema.default_value()).unwrap();
            assert!(how.take(name, &value), "{name}: {value}");
            assert!(param.ui.help.as_ref().is_some_and(|help| !help.is_empty()), "{name}");
        }
        assert_eq!(how, Lettering::default());
        // Every value a choice offers is one lettering reads, and a number's range holds its default.
        for (name, param) in &rows {
            match &param.schema {
                Schema::Choice { options, default } => {
                    assert!(options.contains(default), "{name}");
                    assert!(options.iter().all(|option| Lettering::default().take(name, option)), "{name}");
                }
                Schema::Number { min, max, default } => assert!(min < default && default < max || *default == 0.0 && *min < 0.0, "{name}"),
                Schema::Bool { default } => assert_eq!((*name, *default), ("back_and_forth", true)),
                Schema::Text { default, max_length } => assert_eq!((*name, *default, *max_length), ("font", "", 128)),
                Schema::Int { .. } => panic!("{name}"),
            }
        }
        // Back and forth is a toggle, the dialog's one, where a choice of `true` and `false` would read the same.
        let toggles: Vec<&str> = rows.iter().filter(|(_, param)| matches!(param.schema, Schema::Bool { .. })).map(|(name, _)| *name).collect();
        assert_eq!(toggles, ["back_and_forth"]);
        let units: Vec<Option<&str>> = rows.iter().map(|(_, param)| param.ui.unit).collect();
        assert_eq!(units, [None, Some("%"), None, Some("mm"), Some("mm"), Some("mm"), None, None, None]);
        // Trims stay off unless asked for, for a machine that cuts no thread.
        let trim = rows.iter().find(|(name, _)| *name == "trim").unwrap();
        assert_eq!(trim.1.schema.default_value(), json!("off"));
    }

    #[test]
    fn req_vc_027_the_effect_returns_its_text_s_outlines_unchanged() {
        let outline = json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [20, 0]}, {"p": [20, 20]}]}]});
        let input =
            serde_json::to_vec(&json!({"objects": [{"type": "path", "path": outline, "fillRule": "nonzero", "bounds": [0, 0, 20, 20]}]})).unwrap();
        let out: Value = serde_json::from_slice(&run(&input, br#"{"font": "small_font", "scale": 150}"#).unwrap()).unwrap();
        assert_eq!(out, json!({"objects": [{"path": outline}]}));
        assert_eq!(run(b"no json", b"{}"), Err(RunError::BadInput));
    }
}
