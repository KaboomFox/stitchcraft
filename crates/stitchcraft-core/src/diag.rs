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
    /// wrong reaches your machine.
    ///
    /// Please report it with the design that triggered it; the message names the rule that failed.
    InternalCheckFailed = "SC-E0009", Error, "Internal check failed";

    /// The design has no stitches: it is empty, or every element was skipped (see the other messages).
    /// StitchCraft never writes an empty machine file, because some machines refuse or mishandle them.
    ///
    /// Add an element with an embroidery stitch type, or fix the errors reported for the elements.
    NothingToStitch = "SC-E0010", Error, "Nothing to stitch";

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

    /// A part of a stroke is too small for the shortest stitch the machine sews well, so it was left out:
    /// one stitch that short would hammer one spot of the fabric and could break the thread. The part is
    /// a single point (a stray node), shorter than the shortest stitch, or longer but curled up so that
    /// all of it lies within the shortest stitch of its ends: a tiny closed loop, say.
    ///
    /// The message gives the part's length and the shortest stitch. Enlarge the part, join it to its
    /// neighbour, or delete it if it is a stray.
    StrokeTooSmall = "SC-W0401", Warning, "Path too small for the shortest stitch; skipped";

    /// The running stitch length is shorter than twice the shortest stitch, so it was lengthened to
    /// twice the shortest stitch. Stitches are spread evenly between corners; at half the length or
    /// more, every stitch then stays at or above the shortest stitch, whatever the distance between
    /// corners.
    ///
    /// The message gives both lengths. Use a longer stitch length, or a shorter minimum if your machine
    /// sews shorter stitches well.
    StitchLengthRaised = "SC-W0402", Warning, "Stitch length below twice the shortest stitch; raised";

    /// Hand-placed stitches (manual stitch) shorter than the shortest stitch the machine sews well: the
    /// needle point that made each one too short was left out, so the stitch before it runs on to the
    /// next point. A part's last point is always kept; the one before it goes instead.
    ///
    /// The message gives how many and the shortest of them. Move the nodes apart, or delete the extra
    /// ones.
    HandStitchTooShort = "SC-W0403", Warning, "Hand-placed stitch shorter than the shortest stitch; point left out";

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
    /// the next thread), so every thread has a placeholder colour — in previews, and in files converted
    /// from this one, where the machine shows that colour at each thread change.
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

    /// The design is wider or taller than the machine's hoop, so the machine cannot sew it in one
    /// hooping (most machines refuse the file).
    ///
    /// If the message says the design fits when turned 90 degrees, rotate it. Otherwise scale it down,
    /// or split it into parts that are sewn in separate hoopings.
    OutsideHoop = "SC-E0701", Error, "Design does not fit the hoop";

    /// The design fits the hoop but is larger than the profile's comfort zone: the area where this
    /// machine and hoop sew most accurately. Near the hoop's edge fabric is held less firmly, so large
    /// designs pucker and shift more.
    ///
    /// The file was written. Rotate the design if the message says that brings it inside the comfort
    /// zone, use a firmer stabilizer, or scale the design down.
    OutsideComfortZone = "SC-W0702", Warning, "Design is larger than the comfort zone";

    /// The SVG file could not be read: it is not well-formed XML, its root is not an `<svg>` element,
    /// its size or viewBox makes no sense, or it is larger than StitchCraft reads. Nothing was stitched.
    ///
    /// The message says what is wrong and, for XML errors, where. Open the file in a vector editor and
    /// save it again as plain SVG.
    SvgUnreadable = "SC-E0801", Error, "SVG could not be read";

    /// Part of the SVG uses a feature that StitchCraft does not stitch, so that part is left out or
    /// simplified: text that is not converted to paths, raster images, clones (`<use>`), nested `<svg>`
    /// elements, style sheets, or a gradient or pattern used as a colour.
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

    /// An element's geometry cannot be used: its path data has an error (the path is stitched up to the
    /// error, as SVG viewers draw it), its transform is not valid (it is skipped with everything inside
    /// it), it draws nothing, or it lies more than 10 metres from the document's origin, beyond what
    /// machine files can record (it is skipped).
    ///
    /// The message names the element. Check its path data and transform, or remove stray objects far from
    /// the design.
    SvgGeometryUnusable = "SC-W0804", Warning, "Element geometry invalid or out of range";
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
