//! Diagnostics: problems with the user's design, explained in their terms, with stable codes.
//!
//! A diagnostic is a value, not a failure: the engine collects diagnostics and keeps planning whatever
//! it can, and every host shows them the same way. Each carries a [`Code`] from the registry in
//! `diag/registry.rs`, the single source of truth for ids, severities, titles and explanations. `stitch explain`,
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

#[macro_use]
mod registry;

codes!(registry);

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

    /// The first line people read of it, naming its element when it has one:
    /// `warning SC-W0307 (svg:patch:fill): …`. The command line prints its fix on the next line.
    pub fn headline(&self) -> String {
        match &self.element {
            Some(element) => format!("{} {} ({element}): {}", self.severity().label(), self.code, self.message),
            None => self.to_string(),
        }
    }

    /// Its headline and then its fix, on one line, for a host that shows each diagnostic as one line of
    /// text, as VectorCraft shows an export's warnings.
    pub fn one_line(&self) -> String {
        match &self.fix {
            Some(fix) => format!("{} {}", self.headline(), fix.describe()),
            None => self.headline(),
        }
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
        assert_eq!(d.one_line(), "warning SC-W0702: Design is 190.0 × 150.0 mm. Rotate the design 90°.");
        assert_eq!(d.fix.map(|f| f.describe()).as_deref(), Some("Rotate the design 90°."));
    }

    #[test]
    fn a_headline_names_the_element_and_one_line_adds_the_fix() {
        let d = Diagnostic::new(Code::FillPartsApart, "The area falls into 2 parts.");
        assert_eq!((d.headline(), d.one_line()), (d.to_string(), d.to_string()));
        let d = d.with_element(ElementId::new("svg:patch:fill").unwrap()).with_fix(Fix::Hint("Break it apart.".into()));
        assert_eq!(d.headline(), "warning SC-W0307 (svg:patch:fill): The area falls into 2 parts.");
        assert_eq!(d.one_line(), "warning SC-W0307 (svg:patch:fill): The area falls into 2 parts. Break it apart.");
    }
}
