//! Ink/Stitch's design settings: the settings it keeps for the whole design rather than for one element
//! (`REQ-SVG-005`).
//!
//! Ink/Stitch writes them into the file's first `<metadata>` element, wherever that is: each is an element
//! of Ink/Stitch's namespace named after the setting, its text the value as JSON, such as
//! `<inkstitch:collapse_len_mm>3</inkstitch:collapse_len_mm>`. It reads the first element of each name
//! among that element's children, and nothing else. Read at Ink/Stitch `d59c9ab`: `lib/metadata.py`,
//! `lib/utils/settings.py`, `lib/gui/preferences.py`, `lib/extensions/output.py`.
//!
//! - **The 3 lengths**, `collapse_len_mm`, `min_stitch_len_mm` and `min_satin_stroke_width_mm`, are
//!   millimetres, and they become the design's [`DesignSettings`]. A value that is not a number of 0 or
//!   more is ignored, and the design keeps its default; the document reader names it (`SC-W0802`).
//!   Ink/Stitch takes text that is not JSON as unset, and so uses its default too, but it sews a negative
//!   number as it is. A file without a setting has StitchCraft's default, which is Ink/Stitch's own unless
//!   the person sewing it has changed theirs in Ink/Stitch's preferences.
//! - **`rotate_on_export`** turns the design as Ink/Stitch writes it into a machine file, by 90° either
//!   way, for machines that don't turn a design to fit their hoop. StitchCraft does not apply it yet: a
//!   value other than 0 is named (`SC-W0802`), and the design is written as drawn.
//! - **`thread-palette`** names the threads in Ink/Stitch's own thread lists and print-outs, which
//!   StitchCraft does not write; it changes nothing StitchCraft sews or writes, and is passed over.
//! - **`inkstitch_svg_version`** is the file's version, which [`crate::version`] reads with the file.
//!
//! Any other element of Ink/Stitch's there is named, once per name, as a setting StitchCraft does not know.

use std::collections::BTreeSet;

use roxmltree::{Node, NodeId};
use stitchcraft_core::Mm;
use stitchcraft_engine::design::{DesignSettings, Setting};

use crate::inkstitch::INKSTITCH_NS;
use crate::version;

/// The design setting that turns the design as it is written.
const ROTATE_ON_EXPORT: &str = "rotate_on_export";

/// The design setting that names the threads in Ink/Stitch's own lists.
const THREAD_PALETTE: &str = "thread-palette";

/// Ink/Stitch's design settings, as the document reader comes to them.
#[derive(Debug, Default)]
pub(crate) struct Found {
    /// The file's first `<metadata>` element, once the reader has come to it.
    metadata: Option<NodeId>,
    /// The names read so far: the first element of each name counts.
    read: BTreeSet<String>,
    /// The settings so far.
    pub(crate) settings: DesignSettings,
}

impl Found {
    /// Takes in `node`, which the reader comes to in document order, `is_svg` when it is an SVG element.
    /// When it is a design setting that is not applied as it is written, what to say, and the hint.
    pub(crate) fn visit(&mut self, node: Node<'_, '_>, is_svg: bool) -> Option<(String, Option<&'static str>)> {
        if self.metadata.is_none() {
            if is_svg && node.tag_name().name() == "metadata" {
                self.metadata = Some(node.id());
            }
            return None;
        }
        if node.parent().map(|parent| parent.id()) != self.metadata || node.tag_name().namespace() != Some(INKSTITCH_NS) {
            return None;
        }
        let name = node.tag_name().name();
        if !self.read.insert(name.to_string()) {
            return None;
        }
        let text = node.text().unwrap_or_default().trim();
        if let Some(setting) = Setting::named(name) {
            let message = format!(
                "Ink/Stitch's design setting `{name}`, \"{text}\", is not a number of 0 or more; it is ignored, and the design keeps its default."
            );
            return (!apply(setting, &mut self.settings, text)).then_some((message, None));
        }
        match name {
            ROTATE_ON_EXPORT => turns(text).then(|| {
                let message = format!(
                    "Ink/Stitch's design setting `{name}`, \"{text}\", turns the design as it is written into a machine file; StitchCraft does \
                     not apply it yet, and writes the design as drawn."
                );
                (message, Some("Rotate the drawing itself instead."))
            }),
            THREAD_PALETTE | version::ELEMENT => None,
            _ => Some((
                format!("The file's metadata has Ink/Stitch's `{name}`, which is not a design setting StitchCraft knows; it is ignored."),
                None,
            )),
        }
    }
}

/// Sets `setting` of `settings` to the value `text` gives; `false`, changing nothing, when `text` is not a
/// number of 0 or more.
fn apply(setting: &Setting, settings: &mut DesignSettings, text: &str) -> bool {
    let value = text.trim().parse::<f64>().ok().filter(|v| *v >= 0.0).and_then(|v| Mm::new(v).ok());
    if let Some(value) = value {
        setting.set(settings, value);
    }
    value.is_some()
}

/// Whether `rotate_on_export`'s text `text` turns the design: a number other than 0. Ink/Stitch turns
/// nothing for one that is not JSON, which it reads as none.
fn turns(text: &str) -> bool {
    text.parse::<f64>().is_ok_and(|degrees| degrees.is_finite() && degrees != 0.0)
}

#[cfg(test)]
mod tests {
    use stitchcraft_engine::design::SETTINGS;

    use super::*;

    #[test]
    fn each_setting_sets_its_own_field() {
        let mut settings = DesignSettings::default();
        for (name, text) in [("collapse_len_mm", "5"), ("min_stitch_len_mm", "0.3"), ("min_satin_stroke_width_mm", "2")] {
            assert!(Setting::named(name).is_some_and(|setting| apply(setting, &mut settings, text)), "{name}");
        }
        assert_eq!((settings.collapse_len.get(), settings.min_stitch_len.map(Mm::get), settings.min_satin_stroke_width.get()), (5.0, Some(0.3), 2.0));
        assert!(Setting::named("min_stitch_length_mm").is_none(), "an element's setting is not the design's");
        let unchanged = settings;
        assert!(!apply(&SETTINGS[0], &mut settings, "-0.1"));
        assert_eq!(settings, unchanged);
    }
}
