//! Text that StitchCraft writes from doc comments: diagnostic explanations, parameter help.
//!
//! Registries take their user-facing text from `///` comments, so the text users read is also the
//! rustdoc of the item — one copy. A doc comment arrives as one literal per line, each with the space
//! that followed `///`; [`doc_comment`] turns that into plain Markdown.

/// The Markdown of a doc comment given as its lines joined by newlines (each line keeping the space that
/// follows `///`): that space removed, trailing blank lines dropped.
pub fn doc_comment(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().map(|line| line.strip_prefix(' ').unwrap_or(line)).collect();
    lines.join("\n").trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn doc_comments_lose_their_leading_space_and_keep_their_paragraphs() {
        assert_eq!(doc_comment(" First line.\n Second line.\n\n Next paragraph.\n"), "First line.\nSecond line.\n\nNext paragraph.");
        assert_eq!(doc_comment("  Indented code.\n"), " Indented code.");
        assert_eq!(doc_comment(""), "");
    }
}
