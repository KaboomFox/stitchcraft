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
//! `SC-I0818` says how many shapes were updated, and how.
//!
//! **What is not applied yet.** Of the other updates, three change what StitchCraft sews now that it reads
//! the shapes' settings (M8.1): in a file of version 1 or 2, a zigzag stroke's pull compensation is halved
//! into `stroke_pull_compensation_mm`, and a satin column starts and ends where it is drawn; in a file of
//! version 1 to 3, a fill with `auto_fill` off is Ink/Stitch's legacy fill, which StitchCraft sews without
//! underlay. M8.3 applies them; until then `SC-W0802` counts the shapes they would change ([`Pending`]),
//! which sew as the file sets them. The others change nothing StitchCraft sews: `fill_method` `auto_fill`,
//! which the update renames `tatami_fill`, is not a fill method StitchCraft knows, and a fill set to it sews
//! the default, a tatami fill, and says so (`SC-W0102`). The compatibility contract lists every update, with
//! the milestone that applies it.

use stitchcraft_engine::common::MAX_STITCH_LENGTH;
use stitchcraft_engine::generators::pull::PULL_COMPENSATION;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
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
/// Ink/Stitch's setting, before version 4, that made a fill off a legacy fill.
const AUTO_FILL: &str = "auto_fill";

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

/// How many shapes the updates StitchCraft doesn't apply yet would change, of those that change what it sews
/// now that it reads the shapes' settings (M8.3 applies them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Pending {
    /// Zigzag strokes whose `pull_compensation_mm` the update halves into `stroke_pull_compensation_mm`.
    pub zigzag_pulls: u32,
    /// Satin columns the update makes start and end where they are drawn.
    pub satins: u32,
    /// Fills with `auto_fill` off, which the update makes legacy fills.
    pub legacy_fills: u32,
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

    /// Adds to `pending` what Ink/Stitch's updates change in a shape with the parameters `params`, once updated
    /// as [`Version::update`] updates them, `fill` when it sews a fill, and StitchCraft doesn't apply yet
    /// (`lib/update.py`): before version 3, a zigzag stroke's pull compensation, a number other than 0, and a
    /// satin column's nearest points; before version 4, a fill with `auto_fill` set to anything but one of
    /// Ink/Stitch's words for yes, or blank.
    pub(crate) fn pending(self, fill: bool, params: &ParamSet, pending: &mut Pending) {
        let Version::Older(version) = self else { return };
        if version < 3 {
            let zigzag = params.get(Family::Stroke.method_param()).map(str::trim) == Some(StitchType::ZigzagStitch.id());
            let pulled = params.get(PULL_COMPENSATION).and_then(|text| text.trim().parse::<f64>().ok()).is_some_and(|pull| pull != 0.0);
            pending.zigzag_pulls += u32::from(zigzag && pulled);
            pending.satins += u32::from(params.get(SATIN_COLUMN).is_some_and(crate::inkstitch::yes));
        }
        // Ink/Stitch reads a blank setting as none, and `auto_fill`'s default is on.
        let off = params.get(AUTO_FILL).is_some_and(|text| !text.trim().is_empty() && !crate::inkstitch::yes(text));
        pending.legacy_fills += u32::from(fill && off);
    }

    /// The note on a file whose shapes Ink/Stitch's update changes in ways StitchCraft doesn't apply yet, with
    /// its hint (`SC-W0802`).
    pub(crate) fn unapplied(self, pending: Pending) -> Option<(String, &'static str)> {
        let Version::Older(version) = self else { return None };
        let mut changes = Vec::new();
        match pending.zigzag_pulls {
            0 => {}
            1 => changes.push("the pull compensation of 1 zigzag stroke".to_string()),
            n => changes.push(format!("the pull compensation of {n} zigzag strokes")),
        }
        if let Some(pulls) = changes.first_mut() {
            pulls.push_str(", which it halves into `stroke_pull_compensation_mm`");
        }
        match pending.satins {
            0 => {}
            1 => changes.push("1 satin column, which it makes start and end where it is drawn".to_string()),
            n => changes.push(format!("{n} satin columns, which it makes start and end where they are drawn")),
        }
        match pending.legacy_fills {
            0 => {}
            1 => changes.push("1 fill with `auto_fill` off, which it makes a legacy fill, sewn without underlay".to_string()),
            n => changes.push(format!("{n} fills with `auto_fill` off, which it makes legacy fills, sewn without underlay")),
        }
        (!changes.is_empty()).then(|| {
            let message = format!(
                "The file was saved by an older Ink/Stitch (file version {version}). Ink/Stitch's update also changes {}. StitchCraft \
                 doesn't apply these changes yet, so those shapes sew as the file sets them.",
                changes.join(", and ")
            );
            (message, "To sew them as Ink/Stitch does, run any Ink/Stitch extension on the file and save it: that updates it.")
        })
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
    fn the_updates_not_applied_yet_are_counted_one_shape_or_several() {
        let one = Pending { zigzag_pulls: 1, satins: 1, legacy_fills: 1 };
        let (message, _) = Version::Older(1).unapplied(one).unwrap();
        assert!(message.contains("the pull compensation of 1 zigzag stroke, which it halves"), "{message}");
        assert!(message.contains("1 satin column, which it makes start and end where it is drawn"), "{message}");
        assert!(message.contains("1 fill with `auto_fill` off, which it makes a legacy fill,"), "{message}");
        let several = Pending { zigzag_pulls: 3, satins: 0, legacy_fills: 2 };
        let (message, _) = Version::Older(1).unapplied(several).unwrap();
        assert!(message.contains("changes the pull compensation of 3 zigzag strokes, which it halves into"), "{message}");
        assert!(message.contains("and 2 fills with `auto_fill` off, which it makes legacy fills,") && !message.contains("satin"), "{message}");
        assert_eq!(Version::Older(1).unapplied(Pending::default()), None);
        assert_eq!(Version::Current.unapplied(one), None);
        // A file read as it is is not updated, so nothing is pending in it.
        let (mut pending, params) = (Pending::default(), [("satin_column", "yes"), ("auto_fill", "no")].into_iter().collect::<ParamSet>());
        for version in [Version::Current, Version::Unversioned, Version::Newer(5)] {
            version.pending(true, &params, &mut pending);
        }
        assert_eq!(pending, Pending::default());
    }

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
