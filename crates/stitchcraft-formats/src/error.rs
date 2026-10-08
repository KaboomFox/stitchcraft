//! Why a plan could not be encoded.
//!
//! Writers refuse rather than produce a file a machine could misread. Each error maps to a registered
//! diagnostic code ([`EncodeError::code`]), so hosts report it like any other problem with the design.

use stitchcraft_core::{Code, Diagnostic};

/// Why a plan could not be written in a format.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum EncodeError {
    /// The plan has no `Normal` stitch; StitchCraft never writes an empty machine file.
    #[error("the design has no stitches")]
    Empty,
    /// More colour changes and stops than the format records.
    #[error("the design has {changes} colour changes and stops; {format} records at most {max}")]
    TooManyColorChanges {
        /// The format's name.
        format: &'static str,
        /// Colour changes plus stops in the plan.
        changes: usize,
        /// The format's limit.
        max: usize,
    },
    /// A coordinate, extent or count exceeds a field of the format.
    #[error("{what} does not fit the {format} format")]
    TooLarge {
        /// The format's name.
        format: &'static str,
        /// What did not fit, for people.
        what: String,
    },
    /// A trim or stop is not at the needle's position (a bug in whatever built the plan).
    #[error("the {kind} at block {block}, entry {index} is not where the needle is")]
    CommandAwayFromNeedle {
        /// `trim` or `stop`.
        kind: &'static str,
        /// The colour block.
        block: usize,
        /// The entry within the block.
        index: usize,
    },
    /// The format's palette has no entry automatic matching may use (a bug in the palette table).
    #[error("the {palette} palette has no thread colours to match against")]
    NoPaletteMatch {
        /// The palette's name.
        palette: &'static str,
    },
}

impl EncodeError {
    /// The registered code hosts report this as.
    pub fn code(&self) -> Code {
        match self {
            EncodeError::Empty => Code::NothingToStitch,
            EncodeError::TooManyColorChanges { .. } => Code::TooManyColorChanges,
            EncodeError::TooLarge { .. } => Code::TooLargeForFormat,
            EncodeError::CommandAwayFromNeedle { .. } | EncodeError::NoPaletteMatch { .. } => Code::InternalCheckFailed,
        }
    }

    /// The diagnostic hosts show.
    pub fn diagnostic(&self) -> Diagnostic {
        let mut sentence = self.to_string();
        if let Some(first) = sentence.get(..1) {
            sentence = format!("{}{}.", first.to_uppercase(), sentence.get(1..).unwrap_or_default());
        }
        Diagnostic::new(self.code(), sentence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_become_registered_diagnostics() {
        let d = EncodeError::Empty.diagnostic();
        assert_eq!((d.code.id(), d.message.as_str()), ("SC-E0010", "The design has no stitches."));
        let d = EncodeError::TooManyColorChanges { format: "PES v1", changes: 300, max: 255 }.diagnostic();
        assert_eq!(d.code.id(), "SC-E0601");
        assert_eq!(d.message, "The design has 300 colour changes and stops; PES v1 records at most 255.");
        assert_eq!(EncodeError::TooLarge { format: "DST", what: "x".into() }.code().id(), "SC-E0602");
    }
}
