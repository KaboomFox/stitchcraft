//! Diagnostics: problems with the user's design, explained in their terms, with stable codes.
//!
//! A diagnostic is a value, not a failure: the engine collects diagnostics and keeps planning whatever
//! it can, and every host shows them the same way. Each carries a [`Code`] from the registry in this
//! module, the single source of truth for ids, severities, titles and explanations. `stitch explain`,
//! the generated diagnostics index and the VectorCraft plug-in's messages are all built from it, and
//! tests check every entry, so an error message cannot ship without ever having been rendered
//! (`docs/src/design/diagnostics.md`).
//!
//! Codes are never reused. A retired code stays registered, with "(retired)" in its title.

use core::fmt;

use crate::element::ElementId;
use crate::units::Point;

/// How serious a diagnostic is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    /// The element, or the whole design, cannot be stitched.
    Error,
    /// Stitched, but probably not what the user wants.
    Warning,
    /// Worth knowing; nothing is wrong.
    Info,
}

impl Severity {
    /// The word hosts print before a message: `error`, `warning` or `info`.
    pub const fn label(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
            Severity::Info => "info",
        }
    }

    /// The letter that follows `SC-` in a code of this severity.
    pub const fn letter(self) -> char {
        match self {
            Severity::Error => 'E',
            Severity::Warning => 'W',
            Severity::Info => 'I',
        }
    }
}

/// Declares the registry. Each entry's doc comment is its explanation, so the text users read in
/// `stitch explain` is also the rustdoc of the variant — one copy.
macro_rules! registry {
    ($(
        $(#[doc = $doc:literal])+
        $name:ident = $id:literal, $severity:ident, $title:literal;
    )+) => {
        /// A registered diagnostic code. Each variant's documentation is the explanation users read.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[non_exhaustive]
        pub enum Code {
            $( $(#[doc = $doc])+ $name, )+
        }

        impl Code {
            /// Every registered code, in registry order.
            pub const ALL: &'static [Code] = &[$(Code::$name),+];

            /// The stable id, for example `SC-W0702`.
            pub const fn id(self) -> &'static str {
                match self { $(Code::$name => $id,)+ }
            }

            /// How serious diagnostics with this code are.
            pub const fn severity(self) -> Severity {
                match self { $(Code::$name => Severity::$severity,)+ }
            }

            /// A short title, for example "Design does not fit the hoop".
            pub const fn title(self) -> &'static str {
                match self { $(Code::$name => $title,)+ }
            }

            /// The explanation as written in the registry (each line keeps its leading space).
            const fn raw_explanation(self) -> &'static str {
                match self { $(Code::$name => concat!($($doc, "\n"),+),)+ }
            }
        }
    };
}

registry! {
    /// A budget limits how much work StitchCraft may spend on one element, and how many stitches one
    /// design may have, so that no input can make it run for hours or run out of memory — on the
    /// command line or inside VectorCraft's live preview.
    ///
    /// When planning, the element named in the message was skipped and the rest of the design was
    /// planned: simplify the element (fewer nodes, wider spacing, a smaller area) or split it into
    /// several elements. When reading a file or drawing a preview, nothing was produced: simplify the
    /// drawing, or split it into several files.
    BudgetExhausted = "SC-E0004", Error, "Budget exhausted";

    /// Preview images have a size limit, so that drawing one never runs out of memory. At the scale
    /// asked for, this design's preview would be larger, so no image was written.
    ///
    /// Use a smaller scale: the message says the largest that fits. A design that runs more than 10
    /// metres from the hoop centre cannot be previewed at any scale.
    PreviewTooLarge = "SC-E0005", Error, "Preview too large";

    /// StitchCraft checks every stitch plan against its own rules before writing a machine file. One of
    /// those checks failed, which means StitchCraft has a bug: the file was not written, so nothing
    /// wrong reaches your machine. `stitch plan` writes a bug-report bundle instead, next to the file it
    /// was asked for.
    ///
    /// Please attach the bundle to an issue; the message names the rule that failed. The bundle holds
    /// your design, so that the bug can be reproduced.
    InternalCheckFailed = "SC-E0009", Error, "Internal check failed";

    /// The design has no stitches: it is empty, or every element was skipped (see the other messages).
    /// StitchCraft never writes an empty machine file, because some machines refuse or mishandle them.
    ///
    /// Add an element with an embroidery stitch type, or fix the errors reported for the elements.
    NothingToStitch = "SC-E0010", Error, "Nothing to stitch";

    /// The element's stitch type is one this version of StitchCraft does not sew yet, so the element was
    /// skipped; the rest of the design is planned. Running and manual stitches are sewn from milestone M3,
    /// satin columns from M4 and tatami fills from M5; the other stitch types follow
    /// (`docs/src/plan/roadmap.md`).
    ///
    /// The message names the stitch type. Choose one StitchCraft sews, or sew the element with another
    /// tool for now.
    StitchTypeNotYet = "SC-W0011", Warning, "Stitch type not sewn yet; element skipped";

    /// `stitch bug-report --replay` was given a file that is not a bug-report bundle it can replay: not
    /// JSON, not a bundle, a bundle format newer than this StitchCraft, or one that names a profile or a
    /// format this StitchCraft does not have. Nothing was replayed.
    ///
    /// The message says what is wrong. A bundle written by a newer StitchCraft needs that version or a
    /// later one; a bundle changed by hand, or by an email program, needs the original file.
    BundleUnreadable = "SC-E0012", Error, "Bug-report bundle could not be replayed";

    /// A parameter's value could not be understood: a number where a word was expected, a choice that
    /// is not one of the parameter's options, or a list with the wrong number of values. The element was
    /// not stitched: guessing could sew something you did not ask for.
    ///
    /// The message names the parameter, the value and what the parameter accepts. The parameter
    /// reference lists every parameter with its accepted values.
    ParamInvalid = "SC-E0101", Error, "Parameter has the wrong type or an unknown choice";

    /// A parameter's value is outside the range StitchCraft accepts, so the nearest allowed value was
    /// used, and the element was stitched with it. Values outside the range are impossible (a negative
    /// length) or beyond what machines sew reliably.
    ///
    /// The message names the parameter, your value and the range. Change the value to remove the
    /// warning.
    ParamClamped = "SC-W0102", Warning, "Parameter clamped to its allowed range";

    /// The design carries a parameter StitchCraft does not know: one from a newer version of StitchCraft
    /// or Ink/Stitch, or one for a stitch type StitchCraft does not support yet. It was kept, so saving
    /// the design does not lose it, but it does not change the stitches.
    ///
    /// The Ink/Stitch compatibility page says when each Ink/Stitch parameter is supported.
    ParamUnknown = "SC-W0105", Warning, "Unknown parameter preserved but ignored";

    /// A satin column's path has no subpath longer than a point, so it has no rails to sew between. The
    /// element was not stitched.
    ///
    /// Draw the column as two rails, its edges, with rungs across both.
    SatinWithoutRails = "SC-E0201", Error, "Satin column has no rails";

    /// Where the subpaths of a satin column meet does not say which two are its rails, so the two
    /// longest were taken as the rails. The rule is Ink/Stitch's: with one rung, the rails are the two
    /// subpaths that meet only one other, and with more rungs, the two that meet more than two others.
    /// With exactly two rungs across two rails, each of the four meets two others, as the strokes of a
    /// `#` do, and the rule cannot tell rails from rungs.
    ///
    /// The message names the two subpaths taken, numbered from 1 in the order the path draws them. If
    /// they are not the rails, use three rungs or more, each crossing both rails once.
    SatinRailsByLength = "SC-W0202", Warning, "Satin rails taken as the two longest subpaths";

    /// A rung of a satin column misses one of the rails, or both. It still says which points go together:
    /// where it misses a rail, the point of that rail nearest the rung is used, as in Ink/Stitch.
    ///
    /// The message names the rung, numbered from 1 in the order the path draws it. Extend it across both
    /// rails, so that where it joins them is what you drew.
    SatinRungDangling = "SC-W0203", Warning, "Satin rung does not cross both rails";

    /// A subpath of a satin column is one point, a stray node, so it is neither a rail nor a rung. It was
    /// left out.
    ///
    /// The message names the subpath, numbered from 1 in the order the path draws it. Delete the node, or
    /// if it was meant as a rung, draw the rung across both rails.
    SatinSubpathPoint = "SC-W0205", Warning, "Satin subpath is one point; left out";

    /// A satin column's contour underlay stops short of the column's start and end by its inset, but a side
    /// of it is so short that this would leave less than half a CSS pixel (0.13 mm), so that side runs to
    /// the column's ends, as in Ink/Stitch.
    ///
    /// The message gives the insets at each end. Lower `contour_underlay_inset_mm`, or turn the contour
    /// underlay off for so short a column.
    SatinContourTooShort = "SC-W0206", Warning, "Satin contour underlay too short for its insets; it keeps its length";

    /// A rung of a satin column crosses a rail more than once, so it does not say which point of one rail
    /// goes with which point of the other, and it was left out. The column is sewn without it.
    ///
    /// The message names the subpath, numbered from 1 in the order the path draws it. Redraw the rung as a
    /// straight line across both rails.
    SatinRungAmbiguous = "SC-W0207", Warning, "Satin rung crosses a rail more than once; left out";

    /// A satin column's path draws no rungs, so its rails' nodes pair up instead, as in Ink/Stitch: the
    /// 2nd node of one rail with the 2nd of the other, and on in order. Its rails have different numbers
    /// of nodes, so the rail with more has nodes that pair with none, and between them the stitches
    /// follow the rails' lengths alone.
    ///
    /// The message gives both counts. Add rungs across the column, or give both rails the same number of
    /// nodes.
    SatinNodesUnequal = "SC-W0210", Warning, "Satin rails without rungs have different numbers of nodes";

    /// A satin column's push compensation, taken off a rail at the column's start and end, would leave less
    /// than half a CSS pixel (0.13 mm) of it, so that rail keeps its length, as in Ink/Stitch. A negative
    /// value at the other end still lengthens it.
    ///
    /// The message gives the push compensation at each end. Lower `push_compensation_mm`, or lengthen the
    /// column.
    SatinPushTooLong = "SC-W0211", Warning, "Satin push compensation too long for a rail; that rail keeps its length";

    /// A satin column is drawn as one path, its centre line, and its stroke is no wider than the design's
    /// `min_satin_stroke_width_mm` (1 mm unless the design sets it). A column that narrow is sewn as a
    /// stroke, by the element's stroke settings, as in Ink/Stitch: a running stitch unless they say
    /// otherwise.
    ///
    /// The message gives the stroke's width and the limit. Widen the stroke to sew a satin column, or draw
    /// the column with 2 rails.
    SatinTooNarrow = "SC-W0212", Warning, "Satin column drawn as one path too narrow; sewn as a stroke";

    /// A satin column drawn as one path, its centre line, is made into rails by offsetting the line by half
    /// the stroke's width to each side. Where the line crosses itself or comes back near itself, the
    /// offsets split into several curves, and the line is cut in half, and the halves again, at most 20
    /// times. Parts whose offsets still split are left out, and so are parts that turn so tightly that an
    /// offset vanishes. Ink/Stitch leaves out the same parts without a word. The rest of the column is sewn.
    ///
    /// The message says how many parts were left out. Draw the column with 2 rails where its line crosses
    /// itself or turns more tightly than the column is wide, or make the stroke narrower.
    SatinCentreLinePartsLeftOut = "SC-W0213", Warning, "Parts of a satin column drawn as one path left out";

    /// A satin column drawn as one path, its centre line, could not be made into rails. Its offsets split
    /// however it is cut, an offset vanishes because the stroke is wider than the line's turns allow, or
    /// the line is shorter than a CSS pixel. The column is not sewn.
    ///
    /// Draw the column with 2 rails, or make the stroke narrower.
    SatinCentreLineFailed = "SC-E0214", Error, "Satin column drawn as one path could not be made into rails";

    /// A satin column is so small that every needle point of it lies within the shortest stitch of its
    /// first: a speck of a column, a few tenths of a millimetre across. Any stitch it could sew would be
    /// shorter than the machine sews well, would hammer one spot of the fabric and could break the thread,
    /// so it is left out, as a stroke that small is (`SC-W0401`). Ink/Stitch sews such a column.
    ///
    /// The message gives the shortest stitch. Draw the column larger, or delete it if it is a stray.
    SatinTooSmall = "SC-W0215", Warning, "Satin column too small for the shortest stitch; left out";
    /// A satin column's rungs, or its push compensation, leave each of its sections with no length on one of
    /// its rails, so no stitch goes across it and the column is not sewn. A rung from the first rail's
    /// start to the second rail's end does this, and so does push compensation that takes off the parts of
    /// a short column's rails where its rungs meet them. Ink/Stitch says such a column cannot be stitched.
    ///
    /// Move the rungs so that each meets both rails away from their ends, or lower `push_compensation_mm`.
    SatinNoSection = "SC-E0216", Error, "Satin column has no section to sew";

    /// A fill is sewn in rows of stitches across each part of its area. A part of 0.21 mm² or less (3
    /// square CSS pixels), a speck about half a millimetre across, is too small for a row, so it is left
    /// out, as Ink/Stitch leaves it out. Where every part is that small, the fill sews nothing. So does a
    /// fill that bounds no area at all: subpaths that are lines or points, or that its fill rule leaves
    /// empty, such as 2 copies of one square under the even-odd rule.
    ///
    /// The message counts the parts left out and is placed at the first. Draw them larger, or sew them as a
    /// running stitch.
    FillPartsTooSmall = "SC-W0303", Warning, "Parts of a fill too small to sew left out";

    /// This fill covers less than 1.4 mm² (20 square CSS pixels). Rows of stitches packed into so small an
    /// area pile up, and a running stitch round it or a satin column across it usually sews it better.
    /// Ink/Stitch gives the same advice. The fill is kept as drawn.
    FillSmall = "SC-W0304", Warning, "Fill smaller than 1.4 mm²";

    /// A fill is sewn in rows of stitches the row spacing apart, and a part of it so thin that no row
    /// crosses it gets none. Such a part is sewn as a running stitch round its outline instead, as
    /// Ink/Stitch sews it.
    ///
    /// The message counts the parts. Make them wider, set a smaller `row_spacing_mm`, or sew them as a
    /// stroke or a satin column.
    FillTooThin = "SC-W0305", Warning, "Fill part too thin for rows; sewn round its outline";

    /// A fill's fill rule says which parts of it are filled where its subpaths lie inside each other or
    /// overlap. Under the rule `nonzero`, SVG's default, a part that the subpaths wind round twice the same
    /// way is filled; the even-odd rule leaves it empty. StitchCraft fills it, as the drawing shows it.
    /// Ink/Stitch takes every subpath but the largest as a hole, whatever the fill rule. Where such a part
    /// lies inside the fill, away from its edge, the same file leaves it empty in Ink/Stitch.
    ///
    /// To leave the part empty in both, set the fill rule to even-odd, or draw the inner subpath the other
    /// way round.
    FillRuleNotEvenOdd = "SC-I0306", Info, "Nonzero fill rule fills a part inside that the even-odd rule leaves empty";

    /// A fill whose area falls into separate parts, such as one drawn as several shapes side by side, or
    /// an outline that crosses itself, is sewn one part after another: each part's rows on their own, with
    /// a jump to the next, which the plan may trim. Ink/Stitch warns about such fills too.
    ///
    /// To choose the order the parts are sewn in, break the fill apart into one element for each part.
    FillPartsApart = "SC-W0307", Warning, "Fill in parts that are apart; each sewn on its own";

    /// A fill's underlay is sewn in rows `fill_underlay_row_spacing_mm` apart, 3 times the row spacing
    /// unless set. On a part so thin across those rows that none crosses it, that layer of underlay is
    /// sewn as a running stitch round the part's outline instead, as Ink/Stitch sews it, and the top rows
    /// are sewn over it as usual.
    ///
    /// The message counts the layers, and each part has one for each underlay angle. To sew the underlay
    /// in rows, set a smaller `fill_underlay_row_spacing_mm` or another `fill_underlay_angle`. To sew no
    /// underlay, turn `fill_underlay` off.
    FillUnderlayTooThin = "SC-I0308", Info, "Fill underlay too thin for rows; sewn round the outline";

    /// A fill's top rows are sewn on each part grown by `expand_mm`, or shrunk where it is negative. A part
    /// too thin for that much shrinking, which wears it away entirely, is sewn at its own size instead, as
    /// Ink/Stitch sews it, and Ink/Stitch warns of it too.
    ///
    /// The message counts the parts. Make `expand_mm` nearer 0.
    FillExpandShrinksAway = "SC-W0309", Warning, "Fill's expand shrinks a part away; sewn at its own size";

    /// A fill's underlay is sewn on each part shrunk by `fill_underlay_inset_mm`. A part too thin for that
    /// much shrinking, which wears it away entirely, has its underlay sewn over all of it instead, as
    /// Ink/Stitch sews it, and Ink/Stitch warns of it too.
    ///
    /// The message counts the parts. Set a smaller `fill_underlay_inset_mm`, or turn `fill_underlay` off.
    FillUnderlayInsetShrinksAway = "SC-W0310", Warning, "Fill's underlay inset shrinks a part away; underlay over all of it";

    /// A fill's rows reach past its edge by its pull compensation, which makes up for the thread pulling
    /// the fabric in. Ink/Stitch reads a fill's `pull_compensation_mm` or `pull_compensation_percent` below
    /// 0 as 0, so that the rows reach the edge and no less, and so does StitchCraft. A satin column reads
    /// the same settings below 0 as narrower.
    ///
    /// The message names the settings. Set them to 0 or more; to keep the rows inside the edge, use a
    /// negative `expand_mm`.
    FillPullCompensationNegative = "SC-W0312", Warning, "Fill's pull compensation below 0; read as 0";

    /// Two subpaths of a fill run along each other at its edge, around a part that its fill rule leaves
    /// empty. A notch drawn as a hole whose sides lie on the outline is one, and a shape drawn twice is
    /// another. StitchCraft leaves the part empty, as the drawing shows it. Ink/Stitch repairs such a shape
    /// by merging each stretch drawn twice into one, which fills the part, and the same file then sews it
    /// filled in Ink/Stitch.
    ///
    /// To sew it alike in both, draw the outline round the empty part instead of a subpath along it.
    FillStretchDrawnTwice = "SC-I0313", Info, "Fill drawn twice along its edge; Ink/Stitch fills the part left empty";

    /// A part of a stroke is too small for the shortest stitch the machine sews well, so it was left out:
    /// one stitch that short would hammer one spot of the fabric and could break the thread. The part is
    /// a single point (a stray node), shorter than the shortest stitch, or longer but curled up so that
    /// all of it lies within the shortest stitch of its ends: a tiny closed loop, say.
    ///
    /// The message gives the part's length and the shortest stitch. Enlarge the part, join it to its
    /// neighbour, or delete it if it is a stray.
    StrokeTooSmall = "SC-W0401", Warning, "Path too small for the shortest stitch; skipped";

    /// A running stitch's length, a stroke's or that of one of a satin column's underlays, is shorter than
    /// twice the shortest stitch, so it was lengthened to twice the shortest stitch. Stitches are spread
    /// evenly between corners; at half the length or more, every stitch then stays at or above the
    /// shortest stitch, whatever the distance between corners.
    ///
    /// The message names the parameter and gives both lengths. Use a longer stitch length, or a shorter
    /// minimum if your machine sews shorter stitches well.
    StitchLengthRaised = "SC-W0402", Warning, "Stitch length below twice the shortest stitch; raised";

    /// Hand-placed stitches (manual stitch) shorter than the shortest stitch the machine sews well: the
    /// needle point that made each one too short was left out, so the stitch before it runs on to the
    /// next point. A part's last point is always kept; the one before it goes instead.
    ///
    /// The message gives how many and the shortest of them. Move the nodes apart, or delete the extra
    /// ones.
    HandStitchTooShort = "SC-W0403", Warning, "Hand-placed stitch shorter than the shortest stitch; point left out";

    /// Hand-placed stitches (manual stitch) sewn longer than `max_stitch_length_mm`. A stitch longer than
    /// that is split into equal parts no longer than it, but never into parts shorter than the shortest
    /// stitch the machine sews well, which here took fewer parts: a shorter stitch would hammer one spot and
    /// could break the thread. Ink/Stitch splits such a stitch into the fewest parts no longer than the
    /// setting, however short they are.
    ///
    /// The message gives how many stitches are longer and the longest of them. Set `max_stitch_length_mm`
    /// to at least twice the shortest stitch: every split then leaves parts long enough.
    HandStitchKeptLong = "SC-W0404", Warning, "Hand-placed stitch longer than max_stitch_length_mm; kept for the shortest stitch";

    /// A lock stitch would have been shorter than 0.2 mm, the shortest stitch a lock may have. The needle
    /// would go back into the hole it just left, which can cut the thread and does not lock it. Each such
    /// step of a lock made of steps (back and forth, or a custom lock written as numbers) was lengthened
    /// to 0.2 mm, and a drawn lock was enlarged until its shortest stitch is 0.2 mm long. A lock of steps
    /// follows the stitching. Where a sharp turn would fold it onto itself, it was sewn straight along the
    /// first (or last) stitch.
    ///
    /// The message says which lock, and by how much. Set a larger lock size (`lock_start_scale_mm`,
    /// `lock_end_scale_mm`) or scale (`lock_start_scale_percent`, `lock_end_scale_percent`), or write
    /// longer custom steps.
    LockStitchLengthened = "SC-W0502", Warning, "Lock stitch shorter than 0.2 mm; lengthened";

    /// The lock is set to custom, but its shape cannot be sewn as written. A custom lock is numbers
    /// separated by spaces, the steps the needle takes along the stitching in sizes of
    /// `lock_*_scale_mm`, or an SVG path that draws it. StitchCraft sews the numbers. It does not sew a
    /// drawn custom lock yet, and sews the half stitch in its place. Parts that are not numbers, and steps
    /// longer than 10 m, are left out. If no step is left, the half stitch is sewn instead.
    ///
    /// The message says what was wrong. Write the lock as numbers, such as `1 -1 1 -1`, or choose
    /// another lock shape.
    CustomLockUnusable = "SC-W0503", Warning, "Custom lock cannot be sewn as written";

    /// Needle points less than the shortest stitch from a needle point next to them were left out, so the
    /// stitch before each runs on to the next and none is shorter than the machine sews well. This happens
    /// where one element's stitching runs straight on into the next and they nearly touch: the stitch
    /// between them is whatever is left of the gap. A point that stays, such as a lock point or the last
    /// point before a jump, trim or stop, leaves out the points before it that it is too near, and a stitch
    /// into or out of a lock point may be as short as 0.2 mm. Where small elements are sewn over each other,
    /// a point that stays can go too, when it comes within the shortest stitch of another that stays.
    /// Nothing visible changes.
    ///
    /// The message gives how many points were left out and the shortest stitches that applied to them.
    /// Nothing needs doing. If the count is large, check for elements drawn on top of each other.
    StitchesMerged = "SC-I0504", Info, "Stitches shorter than the shortest stitch merged";
    /// The element is set to trim or stop after it (`trim_after`, `stop_after`, or Ink/Stitch's trim and
    /// stop commands), but it sews no stitch, so there is no place for the trim or the stop: it is left
    /// out, as Ink/Stitch leaves it out. Another message says why the element sews nothing.
    ///
    /// Make the element sew, or set the trim or stop on the element before it.
    TrimOrStopLeftOut = "SC-W0505", Warning, "Trim or stop after an element that sews nothing; left out";

    /// The file format can record only a limited number of colour changes and stops (PES: 255). This
    /// design has more, so the file was not written.
    ///
    /// Merge elements that use the same thread so they sew in one block, or remove stops.
    TooManyColorChanges = "SC-E0601", Error, "Too many colour changes for the file format";

    /// The design's coordinates or stitch data do not fit the fields of this file format, so the file
    /// was not written. Within a real hoop this does not happen; it points at a design that is far
    /// larger than any hoop, or at stray objects far from the rest of the design.
    ///
    /// Check the design's size and remove stray objects.
    TooLargeForFormat = "SC-E0602", Error, "Design too large for the file format";

    /// The machine file could not be read: it is not in the format its name or first bytes suggest, it
    /// ends early, or a record in it makes no sense. StitchCraft reads every file as if it could be
    /// damaged or hostile, so a bad file is reported, never half-read in silence.
    ///
    /// The message says where reading stopped. If the machine sews the file, it may be a format variant
    /// StitchCraft does not know yet: please report it with the file.
    UnreadableFile = "SC-E0603", Error, "Machine file could not be read";

    /// The file stores no thread colours (DST files never do: they only say where the machine pauses for
    /// the next thread), so every thread has a black placeholder colour in previews. A PES file converted
    /// from this one names each thread by the Brother colour nearest black that is still free (Black, Dark
    /// Gray, Dark Brown and so on), which says nothing of the threads the design needs.
    ///
    /// The stitches are not affected. Load the threads the design needs, in the order the design's
    /// author gives; StitchCraft cannot know them.
    ThreadColorsUnknown = "SC-W0604", Warning, "Thread colours unknown";

    /// DST has no trim command: machines cut the thread when they meet three or more jump records in a
    /// row (a machine setting; three is the common one), and a jump longer than 24.2 mm takes three or
    /// more records. So the thread will be cut before such a jump, although the plan does not trim there.
    /// With a tie-off before the jump the cut is usually welcome: there is no jump thread to clip by hand.
    ///
    /// Nothing to do. Where an element's ties are off, turn them on so its stitching holds when it is
    /// cut, or sew a PES file, where only trims cut. A machine set to another number of jump records cuts
    /// before other jumps: see its DST setting.
    JumpsCutInDst = "SC-I0605", Info, "Long jumps cut in DST";

    /// A PES file names each thread by a colour of the Brother palette, which has 61 thread colours.
    /// StitchCraft gives each different thread a colour of its own, the nearest one still free, so the
    /// machine asks for each thread change. A design with more different threads than that has some
    /// share a colour, and the machine shows that colour for each of them: at a change between 2 of them it
    /// shows the thread already in the machine, as for a stop.
    ///
    /// Load the threads in the order the report lists them, rather than by the colours the machine shows.
    /// Or use fewer different threads: merge those of nearly the same colour.
    ThreadsShareColor = "SC-W0606", Warning, "Threads share a Brother colour";

    /// The design is wider or taller than the machine's hoop, so the machine cannot sew it in one
    /// hooping (most machines refuse the file). A design that fits the hoop can still reach past its edge
    /// where it is placed: its origin, which goes to the hoop's centre, can lie far from its middle, and
    /// its stop position, where the frame goes before each stop, can lie outside the hoop.
    ///
    /// If the message says the design fits when turned 90 degrees, rotate it. Otherwise scale it down,
    /// or split it into parts that are sewn in separate hoopings. Where it reaches past the edge from
    /// where it is placed, move its origin nearer its middle, or its stop position nearer the design.
    OutsideHoop = "SC-E0701", Error, "Design does not fit the hoop";

    /// The design fits the hoop but is larger than the profile's comfort zone: the area where this
    /// machine and hoop sew most accurately. Near the hoop's edge fabric is held less firmly, so large
    /// designs pucker and shift more.
    ///
    /// The file was written. Rotate the design if the message says that brings it inside the comfort
    /// zone, use a firmer stabilizer, or scale the design down.
    OutsideComfortZone = "SC-W0702", Warning, "Design is larger than the comfort zone";

    /// Stitches longer than the machine's longest stitch were split into equal parts, each no longer than
    /// it: a long stitch placed by hand, say, or a custom lock's long step. A long loose stitch snags and
    /// sags; the machine may refuse it.
    ///
    /// To choose where the needle goes down instead, add nodes (manual stitch) or set
    /// `max_stitch_length_mm`.
    StitchesSplit = "SC-I0703", Info, "Stitches longer than the machine's longest stitch; split";

    /// The SVG file could not be read: it is not well-formed XML, its root is not an `<svg>` element,
    /// its size or viewBox does not read or makes no sense, or it is larger than StitchCraft reads, its
    /// style sheets included. Nothing was stitched.
    ///
    /// The message says what is wrong and, for XML errors, where. Open the file in a vector editor and
    /// save it again as plain SVG.
    SvgUnreadable = "SC-E0801", Error, "SVG could not be read";

    /// Part of the SVG uses a feature that StitchCraft does not stitch, so that part is left out or
    /// simplified: text that is not converted to paths, raster images, clones (`<use>`), nested `<svg>`
    /// elements, style sheet rules with selectors StitchCraft does not read, or a gradient or pattern used
    /// as a colour. Values that do not read are named too. A colour that does not read is ignored, as a
    /// viewer ignores it, and one of Ink/Stitch's design settings keeps its default.
    ///
    /// The message names the element and what was done. Convert text and clones to paths in the editor
    /// (in Inkscape: Path › Object to Path, Edit › Clone › Unlink Clone), and give shapes plain colours.
    SvgFeatureIgnored = "SC-W0802", Warning, "SVG feature ignored";

    /// An object or a layer is left out because the file asks for it: an Ink/Stitch "ignore object" or
    /// "ignore layer" command, or the object's Ink/Stitch setting `ignore_object`. Designs keep
    /// templates, placement lines and notes this way, in the drawing but out of the sew-out, and
    /// Ink/Stitch leaves them out too.
    ///
    /// The message names what was left out and why. To stitch it, delete the command's symbol, or turn the
    /// setting off in Ink/Stitch's parameters.
    SvgObjectIgnored = "SC-I0805", Info, "Object left out, as the file asks";

    /// An element's geometry cannot be used: its path data or its list of points has an error (the path is
    /// stitched up to the error, as SVG viewers draw it), its transform is not valid (it is skipped with
    /// everything inside it), it draws nothing, or it lies more than 10 metres from the document's origin,
    /// beyond what machine files can record (it is skipped). A stroke width, join or miter limit that does
    /// not read is named as well, and ignored as a viewer ignores it, and so is an empty width, height or
    /// viewBox of the drawing, or an aspect ratio that does not read. A stroke too wide to measure is
    /// skipped.
    ///
    /// The message names the element. Check its path data, points and transform, or remove stray objects
    /// far from the design.
    SvgGeometryUnusable = "SC-W0804", Warning, "Element geometry invalid or out of range";

    /// The `.vectorcraft` file comes from a newer VectorCraft: its format version is newer than the newest
    /// StitchCraft reads. StitchCraft reads it as that version, usually all of a design's paths and paints,
    /// where older versions of VectorCraft refuse the file.
    ///
    /// The message gives the file's format version and the newest StitchCraft reads. Compare the stitches
    /// with the drawing, or save a copy in that format from the newer VectorCraft: File › Save a Copy, then
    /// the Version in VectorCraft Options.
    VcNewerFormat = "SC-W0803", Warning, "VectorCraft file from a newer format version; read best-effort";

    /// The `.vectorcraft` file could not be read: it is not a VectorCraft document (its JSON does not
    /// read, it names another format, or it holds no document), the data VectorCraft keeps after its JSON
    /// is damaged, or it unpacks to more JSON than StitchCraft reads. Nothing was stitched.
    ///
    /// The message says what is wrong and, for JSON errors, where. Open the file in VectorCraft and save it
    /// again.
    VcUnreadable = "SC-E0806", Error, "VectorCraft file could not be read";

    /// Part of the VectorCraft document is not stitched: text, images, symbols, objects VectorCraft
    /// computes as it draws them (blends, envelopes, meshes, repeats, placed documents and Intertwine
    /// objects), a kind of object StitchCraft does not read, clipping and opacity masks, VectorCraft's own
    /// effects, a brush's art, a StitchCraft effect it cannot use, or a group's or a layer's own fills and
    /// strokes. A gradient is sewn in its first colour, and a pattern, or a colour that does not read, in
    /// black.
    ///
    /// The message names the object and what was done. In VectorCraft, Type › Create Outlines makes paths
    /// of text, and Object › Expand of blends, envelopes, repeats and placed documents. Break Link to Symbol
    /// makes paths of a symbol, and Object › Expand Appearance of what effects and brushes draw.
    VcFeatureIgnored = "SC-W0807", Warning, "VectorCraft object or feature not stitched";

    /// An object's geometry cannot be used, and the object, or the stroke, is left out: a coordinate or a
    /// stroke's width is not a number, the object draws nothing, or it lies more than 10 metres from the
    /// document's origin, beyond what machine files can record. A StitchCraft stroke or satin effect takes
    /// a width that is not a number as 0.
    ///
    /// The message names the object. Check its anchors, or remove stray objects far from the design.
    VcGeometryUnusable = "SC-W0808", Warning, "VectorCraft geometry unusable";

    /// A layer or a guide is left out because VectorCraft does not print it: a template layer, a layer
    /// whose Print option is off, or a guide. VectorCraft shows them, dimmed or as guides, and keeps them
    /// out of what it prints and exports; StitchCraft keeps them out of the sew-out.
    ///
    /// The message names the layer or the guide. To stitch a layer, turn its Template option off, or its
    /// Print option on, in VectorCraft's Layer Options.
    VcNotPrinted = "SC-I0809", Info, "VectorCraft layer or guide left out, as it does not print";
}

impl Code {
    /// The explanation as Markdown paragraphs, for `stitch explain` and the diagnostics index.
    pub fn explanation(self) -> String {
        crate::text::doc_comment(self.raw_explanation())
    }
}

impl fmt::Display for Code {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Changes a host can apply for the user with one click, and undo.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Edit {
    /// Turn the whole design by 90 degrees.
    RotateDesign90,
}

/// How to fix a diagnostic.
#[derive(Clone, Debug, PartialEq)]
pub enum Fix {
    /// Advice, in embroidery terms.
    Hint(String),
    /// A change the host can apply.
    Apply(Edit),
}

impl Fix {
    /// The fix as one sentence for people.
    pub fn describe(&self) -> String {
        match self {
            Fix::Hint(text) => text.clone(),
            Fix::Apply(Edit::RotateDesign90) => "Rotate the design 90°.".to_string(),
        }
    }
}

/// A problem with the user's design, explained in their terms.
#[derive(Clone, Debug, PartialEq)]
pub struct Diagnostic {
    /// What kind of problem this is.
    pub code: Code,
    /// One specific sentence: what is wrong, and by how much.
    pub message: String,
    /// The element it concerns, when it concerns one.
    pub element: Option<ElementId>,
    /// Where on the canvas, in millimetres, when it has a place.
    pub at: Option<Point>,
    /// A hint, or a change the host can apply.
    pub fix: Option<Fix>,
}

impl Diagnostic {
    /// A diagnostic with `code` and `message`, not tied to an element or a place.
    pub fn new(code: Code, message: impl Into<String>) -> Self {
        Diagnostic { code, message: message.into(), element: None, at: None, fix: None }
    }

    /// This diagnostic, concerning `element`.
    #[must_use]
    pub fn with_element(mut self, element: ElementId) -> Self {
        self.element = Some(element);
        self
    }

    /// This diagnostic, located at `at`.
    #[must_use]
    pub fn located(mut self, at: Point) -> Self {
        self.at = Some(at);
        self
    }

    /// This diagnostic, with a fix.
    #[must_use]
    pub fn with_fix(mut self, fix: Fix) -> Self {
        self.fix = Some(fix);
        self
    }

    /// The severity, which always comes from the code.
    pub fn severity(&self) -> Severity {
        self.code.severity()
    }
}

/// `warning SC-W0702: Design is 190.0 × 150.0 mm, …` (one line; hosts render element, place and fix).
impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}: {}", self.severity().label(), self.code, self.message)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn ids_are_unique_well_formed_and_match_their_severity() {
        let mut seen = BTreeSet::new();
        for code in Code::ALL {
            let id = code.id();
            assert!(seen.insert(id), "{id} is registered twice");
            let rest = id.strip_prefix("SC-").unwrap_or_else(|| panic!("{id} does not start with SC-"));
            let mut chars = rest.chars();
            assert_eq!(chars.next(), Some(code.severity().letter()), "{id}: letter does not match severity");
            let digits: String = chars.collect();
            assert!(digits.len() == 4 && digits.chars().all(|c| c.is_ascii_digit()), "{id}: expected four digits");
        }
    }

    #[test]
    fn every_code_is_explained() {
        for code in Code::ALL {
            assert!(!code.title().is_empty(), "{code} has no title");
            let text = code.explanation();
            assert!(text.len() > 80, "{code}: explanation too short to help anyone");
            assert!(!text.starts_with(' ') && !text.contains("\n "), "{code}: indentation leaked");
            assert!(text.ends_with('.'), "{code}: explanation should end with a full sentence");
        }
    }

    #[test]
    fn explanations_keep_their_paragraphs() {
        let text = Code::OutsideHoop.explanation();
        assert!(text.starts_with("The design is wider or taller than the machine's hoop"));
        assert!(text.contains(".\n\nIf the message says"));
    }

    #[test]
    fn diagnostics_render_as_one_line() {
        let d = Diagnostic::new(Code::OutsideComfortZone, "Design is 190.0 × 150.0 mm.").with_fix(Fix::Apply(Edit::RotateDesign90));
        assert_eq!(d.to_string(), "warning SC-W0702: Design is 190.0 × 150.0 mm.");
        assert_eq!(d.severity(), Severity::Warning);
        assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Rotate the design 90°."));
    }
}
