//! The file's bytes as the XML text the reader parses: which files are text StitchCraft reads, and which
//! entity declarations it lets the parser expand.
//!
//! **Encodings.** XML files are UTF-8 unless their declaration says otherwise. Older files and some
//! exporters write ISO-8859-1 (Latin-1), whose bytes are the first 256 characters, so a Latin-1 file is
//! turned into UTF-8 without loss. Any other encoding is refused when the file has a byte outside ASCII;
//! a file of ASCII alone reads the same in all of them.
//!
//! **Entities.** Illustrator's "Preserve Illustrator Editing Capabilities" declares its namespaces as
//! internal entities, and the namespaces are then written as `&ns_ai;`. The parser expands entities, and
//! an entity can make a small file very large, either by nesting (a value that refers to other entities)
//! or by many references to one long value. So an entity's value must be plain text, with no `&`, `<` or
//! `%`, and the file with every reference expanded may be no longer than [`MAX_BYTES`]. External and
//! parameter entities are refused.

use std::borrow::Cow;
use std::collections::BTreeMap;

use stitchcraft_core::{Code, Diagnostic, Fix};

/// The largest SVG file StitchCraft reads, in bytes (64 MiB), and the most its entities may expand it to.
/// The largest file in Ink/Stitch's font library is 14.5 MB; a file much larger than that usually carries
/// embedded images, which are not stitched anyway. Memory stays bounded by this and by the XML node limit.
pub const MAX_BYTES: usize = 64 << 20;

/// The names ISO-8859-1 goes by in XML declarations (IANA's, in any case).
const LATIN_1: [&str; 6] = ["iso-8859-1", "iso8859-1", "iso_8859-1", "latin1", "latin-1", "l1"];

/// The file's text, or why it is not text StitchCraft can read.
pub fn decode(bytes: &[u8]) -> Result<Cow<'_, str>, Diagnostic> {
    if bytes.len() > MAX_BYTES {
        return Err(unreadable(format!("The file is {} MB; StitchCraft reads SVG files of up to {} MB.", mb(bytes.len()), MAX_BYTES >> 20)));
    }
    if bytes.starts_with(&[0x1f, 0x8b]) {
        return Err(unreadable("The file is compressed SVG (.svgz); StitchCraft reads plain SVG."));
    }
    if bytes.starts_with(&[0xff, 0xfe]) || bytes.starts_with(&[0xfe, 0xff]) {
        return Err(unreadable("The file is UTF-16 text; StitchCraft reads UTF-8."));
    }
    let bytes = bytes.strip_prefix(b"\xef\xbb\xbf").unwrap_or(bytes);
    match declared_encoding(bytes) {
        _ if bytes.is_ascii() => {}
        Some(name) if LATIN_1.iter().any(|latin| name.eq_ignore_ascii_case(latin)) => {
            return Ok(Cow::Owned(bytes.iter().map(|&byte| char::from(byte)).collect()));
        }
        Some(name) if !name.eq_ignore_ascii_case("utf-8") && !name.eq_ignore_ascii_case("utf8") => {
            return Err(unreadable(format!("The file is {name} text; StitchCraft reads UTF-8 and ISO-8859-1.")));
        }
        _ => {}
    }
    std::str::from_utf8(bytes)
        .map(Cow::Borrowed)
        .map_err(|e| unreadable(format!("The file is not UTF-8 text: byte {} starts no character.", e.valid_up_to())))
}

/// The encoding the file's XML declaration names, when it starts with one.
fn declared_encoding(bytes: &[u8]) -> Option<&str> {
    let rest = bytes.strip_prefix(b"<?xml")?;
    let end = rest.windows(2).position(|pair| pair == b"?>")?;
    let declaration = std::str::from_utf8(rest.get(..end)?).ok()?;
    let value = declaration.split_once("encoding")?.1.trim_start().strip_prefix('=')?.trim_start();
    let quote = value.chars().next().filter(|c| matches!(c, '"' | '\''))?;
    value.get(1..)?.split(quote).next()
}

/// Checks the entities `text` declares: plain text values, no external or parameter entities, and the
/// text with every reference expanded within [`MAX_BYTES`].
pub fn check_entities(text: &str) -> Result<(), Diagnostic> {
    let mut lengths: BTreeMap<&str, usize> = BTreeMap::new();
    let mut rest = text;
    while let Some((_, after)) = rest.split_once("<!ENTITY") {
        let after = after.trim_start();
        let name = after.split(|c: char| c.is_whitespace()).next().unwrap_or_default();
        let declared = after.get(name.len()..).unwrap_or_default().trim_start();
        let quote = declared.chars().next().filter(|c| matches!(c, '"' | '\''));
        let Some((value, tail)) = quote.and_then(|q| declared.get(1..)?.split_once(q)) else {
            return Err(entity(name, "which is not plain text in the file"));
        };
        if name.starts_with('%') || value.contains(['&', '<', '%']) {
            return Err(entity(name, "whose value holds markup or other entities"));
        }
        lengths.insert(name, value.len());
        rest = tail;
    }
    if lengths.is_empty() {
        return Ok(());
    }
    // Each reference `&name;` becomes its value.
    let mut expanded = text.len();
    let mut rest = text;
    while let Some((_, after)) = rest.split_once('&') {
        rest = after;
        let Some((name, _)) = after.split_once(';') else { break };
        if let Some(&length) = lengths.get(name) {
            expanded = expanded.saturating_add(length).saturating_sub(name.len() + 2);
        }
    }
    if expanded > MAX_BYTES {
        return Err(unreadable(format!(
            "The file's XML entities would make it {} MB long; StitchCraft reads SVG files of up to {} MB.",
            mb(expanded),
            MAX_BYTES >> 20
        )));
    }
    Ok(())
}

/// `SC-E0801` for the entity `name`, which is `what`.
fn entity(name: &str, what: &str) -> Diagnostic {
    unreadable(format!("The file declares the XML entity `{name}`, {what}; StitchCraft reads entities of plain text only."))
}

/// A size in whole megabytes, rounded up.
fn mb(bytes: usize) -> usize {
    bytes.div_ceil(1 << 20)
}

/// `SC-E0801` with `message`.
pub fn unreadable(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new(Code::SvgUnreadable, message).with_fix(Fix::Hint("Open the file in a vector editor and save it again as plain SVG.".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_declaration_names_the_encoding() {
        assert_eq!(declared_encoding(br#"<?xml version="1.0" encoding="ISO-8859-1"?><svg/>"#), Some("ISO-8859-1"));
        assert_eq!(declared_encoding(b"<?xml version='1.0' encoding = 'latin1' standalone='no'?>"), Some("latin1"));
        assert_eq!(declared_encoding(br#"<?xml version="1.0"?><svg/>"#), None);
        assert_eq!(declared_encoding(b"<svg/>"), None);
        assert_eq!(declared_encoding(br#"<?xml version="1.0" encoding="utf-8""#), None, "never closed");
    }

    #[test]
    fn latin_1_becomes_utf_8_and_ascii_reads_in_any_encoding() {
        assert_eq!(decode(b"<?xml version=\"1.0\" encoding=\"latin1\"?>caf\xe9").unwrap(), "<?xml version=\"1.0\" encoding=\"latin1\"?>caf\u{e9}");
        assert!(matches!(decode(b"<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><svg/>").unwrap(), Cow::Borrowed(_)));
        assert_eq!(
            decode("<?xml version=\"1.0\" encoding=\"UTF-8\"?>caf\u{e9}".as_bytes()).unwrap(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>caf\u{e9}"
        );
    }

    #[test]
    fn references_add_their_values_length() {
        let file = |count: usize| format!("<!DOCTYPE svg [<!ENTITY e '{}'>]><svg>{}</svg>", "x".repeat(1 << 20), "&e;".repeat(count));
        assert!(check_entities(&file(62)).is_ok());
        assert!(check_entities(&file(64)).is_err());
        assert!(check_entities("<svg>&amp; &unknown; &dangling</svg>").is_ok(), "no declarations, nothing to count");
        assert!(check_entities("<!DOCTYPE svg [<!ENTITY e 'never closed>]><svg/>").is_err());
    }
}
