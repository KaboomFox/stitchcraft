//! Ink/Stitch's design settings: the settings it keeps for the whole design rather than for one element
//! (`REQ-SVG-005`).
//!
//! Ink/Stitch writes 3 of them into the file's first `<metadata>` element, wherever that is: each is an
//! element of Ink/Stitch's namespace named after the setting, its text the value as JSON, such as
//! `<inkstitch:collapse_len_mm>3</inkstitch:collapse_len_mm>`. It reads the first element of each name
//! among that element's children, and nothing else. The values are millimetres, and they become the
//! design's [`DesignSettings`]. Read at Ink/Stitch `d59c9ab`: `lib/metadata.py`, `lib/utils/settings.py`.
//!
//! A value that is not a number of 0 or more is ignored, and the design keeps its default; the document
//! reader names it (`SC-W0802`). Ink/Stitch takes text that is not JSON as unset, and so uses its default
//! too, but it sews a negative number as it is. A file without a setting has StitchCraft's default, which
//! is Ink/Stitch's own unless the person sewing it has changed theirs in Ink/Stitch's preferences.

use std::collections::BTreeSet;

use roxmltree::{Node, NodeId};
use stitchcraft_core::Mm;
use stitchcraft_engine::design::DesignSettings;

use crate::inkstitch::INKSTITCH_NS;

/// One of Ink/Stitch's design settings: its name, and the field of [`DesignSettings`] it sets.
pub(crate) struct Setting {
    /// The name, as Ink/Stitch writes it.
    pub(crate) name: &'static str,
    /// Sets the field to a value.
    set: fn(&mut DesignSettings, Mm),
}

/// The design settings Ink/Stitch keeps in a file, each with the setting it becomes.
pub(crate) const SETTINGS: [Setting; 3] = [
    Setting { name: "collapse_len_mm", set: |settings, value| settings.collapse_len = value },
    Setting { name: "min_stitch_len_mm", set: |settings, value| settings.min_stitch_len = Some(value) },
    Setting { name: "min_satin_stroke_width_mm", set: |settings, value| settings.min_satin_stroke_width = value },
];

/// Ink/Stitch's design settings, as the document reader comes to them.
#[derive(Debug, Default)]
pub(crate) struct Found {
    /// The file's first `<metadata>` element, once the reader has come to it.
    metadata: Option<NodeId>,
    /// The names read so far: the first element of each name counts.
    read: BTreeSet<&'static str>,
    /// The settings so far.
    pub(crate) settings: DesignSettings,
}

impl Found {
    /// Takes in `node`, which the reader comes to in document order, `is_svg` when it is an SVG element.
    /// When it is a design setting whose value does not read, the message that names it.
    pub(crate) fn visit(&mut self, node: Node<'_, '_>, is_svg: bool) -> Option<String> {
        if self.metadata.is_none() {
            if is_svg && node.tag_name().name() == "metadata" {
                self.metadata = Some(node.id());
            }
            return None;
        }
        if node.parent().map(|parent| parent.id()) != self.metadata || node.tag_name().namespace() != Some(INKSTITCH_NS) {
            return None;
        }
        let setting = Setting::named(node.tag_name().name()).filter(|setting| self.read.insert(setting.name))?;
        let text = node.text().unwrap_or_default().trim();
        (!setting.apply(&mut self.settings, text)).then(|| {
            format!(
                "Ink/Stitch's design setting `{}`, \"{text}\", is not a number of 0 or more; it is ignored, and the design keeps its default.",
                setting.name
            )
        })
    }
}

impl Setting {
    /// The setting called `name`, if there is one.
    pub(crate) fn named(name: &str) -> Option<&'static Setting> {
        SETTINGS.iter().find(|setting| setting.name == name)
    }

    /// Sets this setting of `settings` to the value `text` gives; `false`, changing nothing, when `text` is
    /// not a number of 0 or more.
    pub(crate) fn apply(&self, settings: &mut DesignSettings, text: &str) -> bool {
        let value = text.trim().parse::<f64>().ok().filter(|v| *v >= 0.0).and_then(|v| Mm::new(v).ok());
        if let Some(value) = value {
            (self.set)(settings, value);
        }
        value.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_setting_sets_its_own_field() {
        let mut settings = DesignSettings::default();
        for (name, text) in [("collapse_len_mm", "5"), ("min_stitch_len_mm", "0.3"), ("min_satin_stroke_width_mm", "2")] {
            assert!(Setting::named(name).is_some_and(|setting| setting.apply(&mut settings, text)), "{name}");
        }
        assert_eq!((settings.collapse_len.get(), settings.min_stitch_len.map(Mm::get), settings.min_satin_stroke_width.get()), (5.0, Some(0.3), 2.0));
        assert!(Setting::named("min_stitch_length_mm").is_none(), "an element's setting is not the design's");
        let unchanged = settings;
        assert!(!SETTINGS[0].apply(&mut settings, "-0.1"));
        assert_eq!(settings, unchanged);
    }
}
