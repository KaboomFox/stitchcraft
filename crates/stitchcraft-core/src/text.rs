//! Text that StitchCraft writes from doc comments: diagnostic explanations, parameter help.
//!
//! Registries take their user-facing text from `///` comments, so the text users read is also the
//! rustdoc of the item — one copy. A doc comment arrives as one literal per line, each with the space
//! that followed `///`; [`doc_comment`] turns that into plain Markdown, and [`plain`] that into text for a
//! host that shows text as it is, such as a tooltip in VectorCraft's dialogs.

/// The Markdown of a doc comment given as its lines joined by newlines (each line keeping the space that
/// follows `///`): that space removed, trailing blank lines dropped.
pub fn doc_comment(raw: &str) -> String {
    let lines: Vec<&str> = raw.lines().map(|line| line.strip_prefix(' ').unwrap_or(line)).collect();
    lines.join("\n").trim_end().to_string()
}

/// `markdown` as plain text, for a host that shows text as it is: each paragraph's lines joined into one
/// line, paragraphs a line apart, a list's items a line each, and the marks of code and strong emphasis
/// left out, a link's text kept without its address.
pub fn plain(markdown: &str) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut paragraph: Vec<&str> = Vec::new();
    let flush = |paragraph: &mut Vec<&str>, lines: &mut Vec<String>| {
        if !paragraph.is_empty() {
            lines.push(unmarked(&paragraph.join(" ")));
            paragraph.clear();
        }
    };
    for line in markdown.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("- ") {
            flush(&mut paragraph, &mut lines);
        }
        if !line.is_empty() {
            paragraph.push(line);
        }
    }
    flush(&mut paragraph, &mut lines);
    lines.join("\n")
}

/// `text` without the marks of code and strong emphasis, and with each link's text without its address.
fn unmarked(text: &str) -> String {
    let text = text.replace('`', "").replace("**", "");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(open) = rest.find('[') {
        out.push_str(rest.get(..open).unwrap_or_default());
        let from = rest.get(open..).unwrap_or_default();
        // A link is `[words](address)`, its words in the first brackets.
        let link = from.find(']').and_then(|close| {
            let address = from.get(close + 1..)?.strip_prefix('(')?;
            let end = address.find(')')?;
            Some((from.get(1..close)?, address.get(end + 1..)?))
        });
        match link {
            Some((words, after)) => {
                out.push_str(words);
                rest = after;
            }
            None => {
                out.push('[');
                rest = from.get(1..).unwrap_or_default();
            }
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_text_keeps_the_words_and_leaves_out_the_marks() {
        let markdown = "Whether the fill is sewn (`SC-W0314`).\nOff, it sews **nothing**.\n\nSee [the guide](../guide.md) and [this](x).\n\n- One.\n- Two,\n  long.";
        assert_eq!(plain(markdown), "Whether the fill is sewn (SC-W0314). Off, it sews nothing.\nSee the guide and this.\n- One.\n- Two, long.");
        assert_eq!(plain(""), "");
        // Brackets that are no link stay.
        assert_eq!(plain("A [note] and [another](x"), "A [note] and [another](x");
    }

    #[test]
    fn doc_comments_lose_their_leading_space_and_keep_their_paragraphs() {
        assert_eq!(doc_comment(" First line.\n Second line.\n\n Next paragraph.\n"), "First line.\nSecond line.\n\nNext paragraph.");
        assert_eq!(doc_comment("  Indented code.\n"), " Indented code.");
        assert_eq!(doc_comment(""), "");
    }
}
