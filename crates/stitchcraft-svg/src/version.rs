//! Ink/Stitch's file versions, and the updates its updater makes to a file an older Ink/Stitch saved
//! (`REQ-SVG-007`).
//!
//! **Why.** Ink/Stitch changes its defaults now and then: a fill's stitches were 3 mm long before its file
//! version 4, and are 4 mm long now. So that a design sews as it did, Ink/Stitch numbers the files it
//! works on (`inkstitch_svg_version`, in the metadata) and, when it opens a file of an older version,
//! writes the old default into each shape that relied on it, renames what it has renamed, and stamps the
//! file with the current version. Read as today's files are, the fills of every file saved before its
//! version 4 would sew a third longer stitches than in Ink/Stitch. Read at Ink/Stitch `d59c9ab`
//! (`lib/update.py`), and run there as a black box on such files (ADR-0001, decision 6).
//!
//! **Which files.** The version is the text of the first element named `inkstitch_svg_version`, in any
//! namespace, read as a whole number; text that is not one counts as none. Ink/Stitch updates only a file
//! with its own content (`crate::inkstitch::is_setting`), and the version's element is some. A file of a
//! version from 1 to 3 is updated. One with Ink/Stitch's content and no version is either an old design,
//! saved before Ink/Stitch numbered its files, or objects pasted into a new file; Ink/Stitch asks its user
//! which, and updates the first. StitchCraft can't ask: it reads such a file as it is, as Ink/Stitch does
//! the second, and says so (`SC-W0802`), as it says that a file from a newer Ink/Stitch is read as the
//! current version.
//!
//! **What is applied.** The updates that change what StitchCraft sews today, as parameters of the shapes,
//! as the reader applies Ink/Stitch's trim and stop commands:
//!
//! - files of version 1 to 3: a shape with a fill that sets no longest stitch of its own, its
//!   `inkstitch:max_stitch_length_mm` missing, empty, not a number or 0, gets `max_stitch_length_mm` 3, on
//!   the whole shape, as Ink/Stitch sets it: a manual stitch or a satin column of its stroke reads it too;
//! - files of version 1: a `<polyline>` whose `style` attribute sets a stroke other than `none` gets
//!   `stroke_method` `manual_stitch`;
//! - the commands' names from before version 3, read as the commands they became in any file
//!   ([`crate::inkstitch::RENAMED`]).
//!
//! A shape digitized automatically (`--auto`) sews by StitchCraft's rules instead, and is left as it is.
//! `SC-I0818` says how many shapes were updated, and how. The other updates change settings StitchCraft
//! does not read yet; the compatibility contract lists them all, with the milestone that applies them.

use stitchcraft_engine::common::MAX_STITCH_LENGTH;
use stitchcraft_params::{Family, ParamSet, StitchType};

/// Ink/Stitch's current file version: the one it gives the files it works on, at `d59c9ab`.
pub const CURRENT: i64 = 4;

/// The name of the element whose text is the file version, in any namespace.
pub(crate) const ELEMENT: &str = "inkstitch_svg_version";

/// The updates StitchCraft applies, by their ids in the compatibility contract, each with how.
pub const APPLIED: [(&str, &str); 3] = [
    ("longest-fill-stitch", "applied to the shapes whose fill is sewn, unless digitized automatically"),
    ("polyline-manual-stitch", "applied, unless digitized automatically"),
    ("command-names", "applied, in any file (`DEV-SVG-014`)"),
];

/// A fill's longest stitch before version 4, which the update gives the shapes that set none.
const OLD_LONGEST: &str = "3";

/// The version a file is read as.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Version {
    /// The current version, or a file without Ink/Stitch's content: read as it is.
    Current,
    /// A version from 1 to 3, in a file with Ink/Stitch's content: updated.
    Older(i64),
    /// Ink/Stitch's content and no version, or one below 1: read as it is, and said so.
    Unversioned,
    /// A version above the current one: read as the current one, and said so.
    Newer(i64),
}

/// What a shape is, for the updates.
pub(crate) struct Shape<'a> {
    /// Whether it sews a fill.
    pub fill: bool,
    /// Its own `inkstitch:max_stitch_length_mm`, if it has one.
    pub longest: Option<&'a str>,
    /// Whether it is a `<polyline>` whose `style` attribute sets a stroke other than `none`.
    pub styled_polyline: bool,
}

/// How many shapes the updates changed, and how.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Updated {
    /// Shapes given the old longest fill stitch.
    pub fills: u32,
    /// Polylines made manual stitches.
    pub polylines: u32,
}

impl Version {
    /// The version of a file whose version's text is `text`, `content` when it has Ink/Stitch's own content.
    pub(crate) fn of(text: Option<&str>, content: bool) -> Version {
        let number = text.and_then(|text| text.trim().parse::<i64>().ok()).unwrap_or(0);
        match number {
            newer if newer > CURRENT => Version::Newer(newer),
            CURRENT => Version::Current,
            _ if !content => Version::Current,
            older if older >= 1 => Version::Older(older),
            _ => Version::Unversioned,
        }
    }

    /// Updates the parameters `params` of a shape that is not digitized automatically, as Ink/Stitch
    /// updates a file of this version; what it changed is added to `updated`.
    pub(crate) fn update(self, shape: &Shape<'_>, params: &mut ParamSet, updated: &mut Updated) {
        let Version::Older(version) = self else { return };
        let own = shape.longest.and_then(|text| text.trim().parse::<f64>().ok()).is_some_and(|length| length != 0.0);
        if shape.fill && !own {
            params.set(MAX_STITCH_LENGTH, OLD_LONGEST);
            updated.fills += 1;
        }
        if version < 2 && shape.styled_polyline {
            params.set(Family::Stroke.method_param(), StitchType::ManualStitch.id());
            updated.polylines += 1;
        }
    }

    /// The note on a file read as it is although Ink/Stitch would update it or warn about it, with its
    /// hint (`SC-W0802`).
    pub(crate) fn warning(self) -> Option<(String, &'static str)> {
        match self {
            Version::Unversioned => Some((
                "The file has Ink/Stitch's settings but no Ink/Stitch file version, so it is read as it is. Ink/Stitch asks first: it updates \
                 an old design, saved before Ink/Stitch numbered its files, to sew as it did, and leaves objects pasted into a new file as \
                 they are."
                    .to_string(),
                "If it is an old design, open it in Ink/Stitch, choose Update, and save it.",
            )),
            Version::Newer(version) => Some((
                format!(
                    "The file was saved by a newer Ink/Stitch (file version {version}) than StitchCraft reads (up to version {CURRENT}): what \
                     that version changed is not known, and the file is read as version {CURRENT}."
                ),
                "Compare the stitches with the drawing.",
            )),
            Version::Current | Version::Older(_) => None,
        }
    }

    /// The note on a file updated as Ink/Stitch updates it, when the updates changed a shape (`SC-I0818`).
    pub(crate) fn note(self, updated: Updated) -> Option<String> {
        let Version::Older(version) = self else { return None };
        let mut changes = Vec::new();
        match updated.fills {
            0 => {}
            1 => changes.push("1 shape with a fill and no longest stitch of its own sews 3 mm stitches".to_string()),
            n => changes.push(format!("{n} shapes with a fill and no longest stitch of their own sew 3 mm stitches")),
        }
        if let Some(fills) = changes.first_mut() {
            fills.push_str(" (`max_stitch_length_mm`), the default before version 4");
        }
        match updated.polylines {
            0 => {}
            1 => changes.push("1 polyline whose `style` sets a stroke sews as a manual stitch (`stroke_method`), as before version 2".to_string()),
            n => changes.push(format!("{n} polylines whose `style` sets a stroke sew as manual stitches (`stroke_method`), as before version 2")),
        }
        (!changes.is_empty()).then(|| {
            format!(
                "The file was saved by an older Ink/Stitch (file version {version}), and is read as Ink/Stitch updates it: {}.",
                changes.join(", and ")
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_is_a_whole_number_and_counts_only_in_a_file_of_ink_stitch_s() {
        let of = Version::of;
        assert_eq!([of(Some("4"), true), of(Some(" 4 "), false), of(None, false), of(Some("3"), false)], [Version::Current; 4]);
        assert_eq!([of(Some("1"), true), of(Some("+3"), true)], [Version::Older(1), Version::Older(3)]);
        assert_eq!([of(None, true), of(Some("3.0"), true), of(Some("0"), true), of(Some("-2"), true)], [Version::Unversioned; 4]);
        assert_eq!([of(Some("5"), false), of(Some("12"), true)], [Version::Newer(5), Version::Newer(12)]);
    }

    #[test]
    fn only_an_older_version_updates_and_only_what_it_changed() {
        let shape = Shape { fill: true, longest: None, styled_polyline: true };
        for (version, expected) in [
            (Version::Older(1), Updated { fills: 1, polylines: 1 }),
            (Version::Older(2), Updated { fills: 1, polylines: 0 }),
            (Version::Older(3), Updated { fills: 1, polylines: 0 }),
            (Version::Current, Updated::default()),
            (Version::Unversioned, Updated::default()),
            (Version::Newer(5), Updated::default()),
        ] {
            let (mut params, mut updated) = (ParamSet::new(), Updated::default());
            version.update(&shape, &mut params, &mut updated);
            assert_eq!(updated, expected, "{version:?}");
            assert_eq!(params.get("max_stitch_length_mm"), (expected.fills == 1).then_some("3"), "{version:?}");
            assert_eq!(params.get("stroke_method"), (expected.polylines == 1).then_some("manual_stitch"), "{version:?}");
        }
        // A shape's own longest stitch counts unless it is 0; one that does not read counts as none.
        for (own, kept) in [("2.5", true), (" 1e1 ", true), ("-1", true), ("0", false), ("0.0", false), ("", false), ("x", false)] {
            let (mut params, mut updated) = (ParamSet::new(), Updated::default());
            Version::Older(3).update(&Shape { fill: true, longest: Some(own), styled_polyline: false }, &mut params, &mut updated);
            assert_eq!(params.get("max_stitch_length_mm").is_none(), kept, "{own}");
        }
    }
}
