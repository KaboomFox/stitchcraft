//! Stable element ids.
//!
//! An element is one object of the user's design: an SVG path, a VectorCraft object, a part of a test
//! sheet. Its id comes from the host (`svg:path123`, `vc:42`, `ts:cross`) and stays the same while the
//! user edits, so diagnostics can point at the object, previews can be updated per object, and seeded
//! randomness stays attached to the object (`rng::SplitMix64::for_element`).

use core::fmt;

/// A stable, host-assigned element id: non-empty, at most [`ElementId::MAX_LEN`] bytes, no control
/// characters (it is shown to users and written into reports).
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ElementId(String);

/// Why a string cannot be an element id.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ElementIdError {
    /// The id was empty.
    #[error("an element id cannot be empty")]
    Empty,
    /// The id was longer than [`ElementId::MAX_LEN`] bytes.
    #[error("an element id is at most {max} bytes; this one is {len}", max = ElementId::MAX_LEN)]
    TooLong {
        /// Length of the rejected id in bytes.
        len: usize,
    },
    /// The id contained a control character such as a newline.
    #[error("an element id cannot contain control characters")]
    ControlCharacter,
}

impl ElementId {
    /// The longest id, in bytes.
    pub const MAX_LEN: usize = 256;

    /// An id from the host's string, checked.
    pub fn new(id: impl Into<String>) -> Result<Self, ElementIdError> {
        let id = id.into();
        if id.is_empty() {
            Err(ElementIdError::Empty)
        } else if id.len() > Self::MAX_LEN {
            Err(ElementIdError::TooLong { len: id.len() })
        } else if id.chars().any(char::is_control) {
            Err(ElementIdError::ControlCharacter)
        } else {
            Ok(ElementId(id))
        }
    }

    /// The id as text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ElementId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_checked() {
        assert_eq!(ElementId::new("svg:path123").unwrap().as_str(), "svg:path123");
        assert_eq!(ElementId::new(""), Err(ElementIdError::Empty));
        assert_eq!(ElementId::new("x".repeat(257)), Err(ElementIdError::TooLong { len: 257 }));
        assert!(ElementId::new("x".repeat(256)).is_ok());
        assert_eq!(ElementId::new("a\nb"), Err(ElementIdError::ControlCharacter));
    }

    #[test]
    fn errors_read_well() {
        assert_eq!(ElementIdError::TooLong { len: 300 }.to_string(), "an element id is at most 256 bytes; this one is 300");
    }
}
