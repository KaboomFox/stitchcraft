//! Design names as machine files store them.
//!
//! PEC and DST headers hold a 16-character label that machines show on their screens. Machines differ in
//! which characters they can display, so labels keep only characters every machine shows — ASCII letters,
//! digits, space, `-`, `_` and `.` — replace the rest with `_`, and stop at 16 characters. The rule is
//! deterministic, so the same name always produces the same bytes.

/// The length of a label field.
pub const LABEL_LEN: usize = 16;

/// `name` as a label: safe characters only, at most [`LABEL_LEN`], padded with spaces to exactly that.
pub fn label(name: &str) -> [u8; LABEL_LEN] {
    let mut field = [b' '; LABEL_LEN];
    for (slot, c) in field.iter_mut().zip(name.chars()) {
        *slot = match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | ' ' | '-' | '_' | '.' => u8::try_from(c).unwrap_or(b'_'),
            _ => b'_',
        };
    }
    field
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_are_safe_padded_and_truncated() {
        assert_eq!(&label("TS-01"), b"TS-01           ");
        assert_eq!(&label("Rosé façade/2026!"), b"Ros_ fa_ade_2026");
        assert_eq!(&label(""), b"                ");
    }
}
