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
//! parameter entities are refused. Entities are declared only in the internal subset of the document type
//! declaration, `<!DOCTYPE svg [ … ]>`, so the text elsewhere, comments and character data among it,
//! declares none. A name declared twice counts at its longer value: XML keeps the first declaration, and
//! a long value declared first, with a short one after it, must not slip past the limit.

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
    let lengths = match internal_subset(text) {
        Some(subset) => declared(subset)?,
        None => return Ok(()),
    };
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

/// The text after the `[` that opens the internal subset of the file's document type declaration, if it
/// has one. Before the declaration come only the XML declaration, comments, processing instructions and
/// white space: anything else is the root element, and the file declares no entities.
fn internal_subset(text: &str) -> Option<&str> {
    let mut rest = text;
    loop {
        rest = rest.trim_start();
        if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->")?.1;
        } else if let Some(after) = rest.strip_prefix("<?") {
            rest = after.split_once("?>")?.1;
        } else {
            // The name and any external identifier, its literals quoted, come before the subset.
            let after = rest.strip_prefix("<!DOCTYPE")?;
            let (at, opens) = outside_quotes(after).find(|&(_, c)| c == '[' || c == '>')?;
            return if opens == '[' { after.get(at + 1..) } else { None };
        }
    }
}

/// The length of the value each name the internal subset `subset` declares, up to the `]` that ends it: of
/// its longest value, for a name declared more than once. Refuses a value that is not plain text, an
/// external or parameter entity, and anything in the subset but declarations, comments and processing
/// instructions. Where the subset, or a comment or declaration in it, is left open, the count stops, and
/// the parser refuses the file.
fn declared(subset: &str) -> Result<BTreeMap<&str, usize>, Diagnostic> {
    let mut lengths: BTreeMap<&str, usize> = BTreeMap::new();
    let mut rest = subset;
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with(']') {
            return Ok(lengths);
        } else if let Some(after) = rest.strip_prefix("<!--") {
            rest = after.split_once("-->").map_or("", |(_, tail)| tail);
        } else if let Some(after) = rest.strip_prefix("<?") {
            rest = after.split_once("?>").map_or("", |(_, tail)| tail);
        } else if let Some(after) = rest.strip_prefix("<!ENTITY") {
            let (name, length, tail) = entity_declaration(after)?;
            let longest = lengths.entry(name).or_insert(length);
            *longest = (*longest).max(length);
            rest = tail;
        } else if let Some(after) = rest.strip_prefix("<!") {
            // Another declaration (an element's, its attributes', a notation's), which declares no entity. The
            // parser takes it to end at the first `>`, quoted or not, and so does this count.
            rest = after.split_once('>').map_or("", |(_, tail)| tail);
        } else {
            return Err(unreadable(
                "The file's document type declaration refers to a parameter entity; StitchCraft reads entities of plain text only.",
            ));
        }
    }
}

/// The name an entity declaration declares and its value's length, and the text after it, from the text
/// after its `<!ENTITY`; refused unless its value is plain text in quotes.
fn entity_declaration(after: &str) -> Result<(&str, usize, &str), Diagnostic> {
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
    let Some(tail) = tail.trim_start().strip_prefix('>') else { return Err(entity(name, "which is not plain text in the file")) };
    Ok((name, value.len(), tail))
}

/// The characters of `text` outside its quoted literals, with their offsets.
fn outside_quotes(text: &str) -> impl Iterator<Item = (usize, char)> + '_ {
    let mut quote: Option<char> = None;
    text.char_indices().filter(move |&(_, c)| match quote {
        Some(q) => {
            if c == q {
                quote = None;
            }
            false
        }
        None if matches!(c, '"' | '\'') => {
            quote = Some(c);
            false
        }
        None => true,
    })
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
        assert_eq!(
            decode(b"<?xml version=\"1.0\" encoding=\"latin1\"?>Z\xfcrich").unwrap(),
            "<?xml version=\"1.0\" encoding=\"latin1\"?>Z\u{fc}rich"
        );
        assert!(matches!(decode(b"<?xml version=\"1.0\" encoding=\"Shift_JIS\"?><svg/>").unwrap(), Cow::Borrowed(_)));
        assert_eq!(
            decode("<?xml version=\"1.0\" encoding=\"UTF-8\"?>Z\u{fc}rich".as_bytes()).unwrap(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>Z\u{fc}rich"
        );
    }

    #[test]
    fn a_file_may_expand_to_the_limit_and_no_further() {
        // 62 references to a 1 MiB value, and plain text after them to bring the expanded file to exactly
        // `MAX_BYTES`, then one byte past it.
        let (value, references) = ("x".repeat(1 << 20), 62);
        let file = |padding: usize| format!("<!DOCTYPE svg [<!ENTITY e '{value}'>]><svg>{}{}</svg>", "&e;".repeat(references), "y".repeat(padding));
        let expanded_without_padding = file(0).len() + references * (value.len() - "&e;".len());
        let fits = MAX_BYTES - expanded_without_padding;
        assert!(check_entities(&file(fits)).is_ok());
        assert!(check_entities(&file(fits + 1)).is_err());
    }

    #[test]
    fn references_add_their_values_length() {
        let file = |count: usize| format!("<!DOCTYPE svg [<!ENTITY e '{}'>]><svg>{}</svg>", "x".repeat(1 << 20), "&e;".repeat(count));
        assert!(check_entities(&file(62)).is_ok());
        assert!(check_entities(&file(64)).is_err());
        assert!(check_entities("<svg>&amp; &unknown; &dangling</svg>").is_ok(), "no declarations, nothing to count");
        assert!(check_entities("<!DOCTYPE svg [<!ENTITY e 'never closed>]><svg/>").is_err());
    }

    /// 80 references to `a`, and the file's text before them.
    fn eighty(prolog: &str) -> String {
        format!("{prolog}<svg>{}</svg>", "&a;".repeat(80))
    }

    #[test]
    fn diag_sc_e0801_a_name_declared_twice_counts_at_its_longest_value() {
        // XML keeps the first of 2 declarations of a name, and the parser expands that one. The last used
        // to set the name's length: a value of 1 MiB declared first and one of 1 byte after it let 80
        // references expand the file to 80 MiB. Either way round, the file is refused.
        let long = "x".repeat(1 << 20);
        for subset in [format!("<!ENTITY a '{long}'><!ENTITY a 'y'>"), format!("<!ENTITY a 'y'><!ENTITY a '{long}'>")] {
            assert!(check_entities(&eighty(&format!("<!DOCTYPE svg [{subset}]>"))).is_err(), "{}", &subset[..20]);
        }
    }

    #[test]
    fn diag_sc_e0801_only_the_document_type_s_own_subset_declares_entities() {
        // A declaration in a comment, or in character data, declares nothing: it neither shortens a value
        // nor refuses a file.
        let long = "x".repeat(1 << 20);
        let after = eighty(&format!("<!DOCTYPE svg [<!ENTITY a '{long}'>]><!-- <!ENTITY a 'y'> -->"));
        assert!(check_entities(&after).is_err(), "a declaration in a comment after the subset");
        let within = eighty(&format!("<!DOCTYPE svg [<!ENTITY a '{long}'><!-- <!ENTITY a 'y'> -->]>"));
        assert!(check_entities(&within).is_err(), "a declaration in a comment inside the subset");
        // The parser ends an element's or attributes' declaration at its first `>`, even one in quotes, so
        // a declaration after it declares an entity, whatever quote the one before seems to open.
        let hidden = eighty(&format!("<!DOCTYPE svg [<!ATTLIST svg x CDATA '> <!ENTITY a '{long}'> <!ATTLIST svg y CDATA '>]>"));
        assert!(check_entities(&hidden).is_err(), "a declaration after a quote in another declaration");
        for valid in [
            r#"<!-- <!ENTITY logo SYSTEM "logo.svg"> --><svg/>"#,
            r#"<svg><desc><![CDATA[ e.g. <!ENTITY % x "y"> ]]></desc></svg>"#,
            r#"<?xml version="1.0"?><!DOCTYPE svg PUBLIC "-//W3C//DTD SVG 1.1//EN" "http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd"><svg/>"#,
            r#"<!DOCTYPE svg [<!-- ] --><!ATTLIST svg x CDATA "]"><!ENTITY ns_ai "http://ns.adobe.com/AdobeIllustrator/10.0/">]><svg>&ns_ai;</svg>"#,
        ] {
            assert_eq!(check_entities(valid), Ok(()), "{valid}");
        }
        // In the subset, the rules hold as before.
        for refused in [
            r#"<!DOCTYPE svg [<!ENTITY logo SYSTEM "logo.svg">]><svg/>"#,
            r#"<!DOCTYPE svg [<!ENTITY % p "<!ENTITY a 'x'>">]><svg/>"#,
            r#"<!DOCTYPE svg [<!ENTITY a "<g/>">]><svg/>"#,
            r#"<!DOCTYPE svg [<!ENTITY a "x"> %p;]><svg/>"#,
        ] {
            assert!(check_entities(refused).is_err(), "{refused}");
        }
    }
}
