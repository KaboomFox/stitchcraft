//! Just enough Markdown for the docs checks: code fences, comments, inline code, links, includes and
//! mdBook's heading ids.
//!
//! A link's `#anchor` is checked against the ids the book will have, so the ids are made as mdBook 0.5.4,
//! the release CI builds the book with, makes them: from the text the heading shows, after smart
//! punctuation (`docs/book.toml` turns it on).
//! An indented line is code, and a heading inside an HTML comment is not shown, so neither has an id.

use std::collections::{BTreeMap, BTreeSet};

/// A link or image found in prose.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// 1-based line number.
    pub line: usize,
    /// The target as written (path, URL or `#anchor`).
    pub target: String,
    /// Whether it is an image (`![alt](…)`).
    pub image: bool,
    /// The link text, or the image's alt text.
    pub text: String,
}

/// Lines outside fenced code blocks, with their 1-based numbers.
pub fn prose_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut fence: Option<&str> = None;
    for (i, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        let marker = ["```", "~~~"].into_iter().find(|m| trimmed.starts_with(m));
        match (fence, marker) {
            (None, Some(m)) => fence = Some(m),
            (Some(open), Some(m)) if open == m => fence = None,
            (None, None) => out.push((i + 1, line)),
            _ => {}
        }
    }
    out
}

/// The lines the book shows as Markdown, with their 1-based numbers: [`prose_lines`] outside HTML comments.
/// A comment that starts a line hides it, and every line up to the one that closes it.
pub fn rendered_lines(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut in_comment = false;
    for (number, line) in prose_lines(text) {
        if !in_comment && !line.trim_start().starts_with("<!--") {
            out.push((number, line));
            continue;
        }
        let comment = if in_comment { line } else { line.trim_start().get(4..).unwrap_or("") };
        in_comment = !comment.contains("-->");
    }
    out
}

/// The files `text` includes with mdBook's `{{#include path}}` or `{{#rustdoc_include path}}`, each with
/// its line number: the path as written, without the line range or anchor after a `:`. mdBook includes
/// them before it reads the Markdown, so code blocks count too, and a `\` before the braces escapes one.
pub fn includes(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (i, line) in text.lines().enumerate() {
        let mut rest = line;
        while let Some(pos) = rest.find("{{#") {
            let escaped = rest.get(..pos).is_some_and(|before| before.ends_with('\\'));
            let after = rest.get(pos + 3..).unwrap_or("");
            let Some(end) = after.find("}}") else { break };
            let directive = after.get(..end).unwrap_or("");
            let path = directive.strip_prefix("include ").or_else(|| directive.strip_prefix("rustdoc_include "));
            if let (false, Some(path)) = (escaped, path) {
                out.push((i + 1, path.trim().split(':').next().unwrap_or("").to_string()));
            }
            rest = after.get(end + 2..).unwrap_or("");
        }
    }
    out
}

/// `line` with the contents of inline code spans blanked out.
pub fn without_inline_code(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut chars = line.chars().peekable();
    let mut delimiter = 0usize;
    while let Some(c) = chars.next() {
        if c == '`' {
            let mut run = 1;
            while chars.peek() == Some(&'`') {
                chars.next();
                run += 1;
            }
            if delimiter == 0 {
                delimiter = run;
            } else if run == delimiter {
                delimiter = 0;
            }
            out.extend(std::iter::repeat_n(' ', run));
        } else if delimiter == 0 {
            out.push(c);
        } else {
            out.push(' ');
        }
    }
    out
}

/// Every link and image the book shows of `text` (code blocks, code spans and comments excluded).
pub fn links(text: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for (number, raw) in rendered_lines(text) {
        let line = without_inline_code(raw);
        let mut rest = line.as_str();
        while let Some(pos) = rest.find("](") {
            let (before, after) = (rest.get(..pos).unwrap_or(""), rest.get(pos + 2..).unwrap_or(""));
            let Some(close) = closing(after, b'(', b')') else { break };
            let raw_target = after.get(..close).unwrap_or("");
            let target = raw_target.split_whitespace().next().unwrap_or("").trim_matches(|c| c == '<' || c == '>');
            let (image, link_text) = match opening(before) {
                Some(open) => {
                    let image = open > 0 && before.as_bytes().get(open - 1) == Some(&b'!');
                    (image, before.get(open + 1..).unwrap_or("").to_string())
                }
                None => (false, String::new()),
            };
            out.push(Link { line: number, target: target.to_string(), image, text: link_text });
            rest = after.get(close + 1..).unwrap_or("");
        }
    }
    out
}

/// Index of the `[` opening the link text that ends at the end of `before`.
fn opening(before: &str) -> Option<usize> {
    let mut depth = 0u32;
    for (i, b) in before.bytes().enumerate().rev() {
        match b {
            b']' => depth += 1,
            b'[' if depth == 0 => return Some(i),
            b'[' => depth -= 1,
            _ => {}
        }
    }
    None
}

/// Index of the delimiter closing an already opened group (nesting respected).
fn closing(after: &str, open: u8, close: u8) -> Option<usize> {
    let mut depth = 0u32;
    for (i, b) in after.bytes().enumerate() {
        if b == open {
            depth += 1;
        } else if b == close {
            if depth == 0 {
                return Some(i);
            }
            depth -= 1;
        }
    }
    None
}

/// The heading ids mdBook generates for `text` (duplicates get `-1`, `-2`, …).
pub fn heading_ids(text: &str) -> BTreeSet<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for (_, line) in rendered_lines(text) {
        let trimmed = line.trim_start_matches(' ');
        // Four spaces or a tab make the line code.
        if line.len() - trimmed.len() > 3 || trimmed.starts_with('\t') {
            continue;
        }
        let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
        let Some(rest) = trimmed.get(hashes..) else { continue };
        if !(1..=6).contains(&hashes) || !(rest.is_empty() || rest.starts_with(' ')) {
            continue;
        }
        let title = rest.trim().trim_end_matches('#').trim();
        let id = normalize_id(&shown(&link_text(title)));
        let count = seen.entry(id.clone()).or_insert(0);
        ids.insert(if *count == 0 { id } else { format!("{id}-{count}") });
        *count += 1;
    }
    ids
}

/// Heading text with each link reduced to its text.
fn link_text(title: &str) -> String {
    let mut out = String::new();
    let mut rest = title;
    while let Some(start) = rest.find('[') {
        out.push_str(rest.get(..start).unwrap_or(""));
        let after = rest.get(start + 1..).unwrap_or("");
        let link = after.find("](").and_then(|mid| {
            let tail = after.get(mid + 2..)?;
            let end = tail.find(')')?;
            Some((after.get(..mid)?, tail.get(end + 1..)?))
        });
        match link {
            Some((text, tail)) => {
                out.push_str(text);
                rest = tail;
            }
            None => {
                out.push('[');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The text the book shows for heading text `title`: code spans as written, and outside them no emphasis
/// markers and no hyphens that smart punctuation makes into a dash (2 or more in a row). An underscore
/// between letters or digits is text, as is one with spaces on both sides.
fn shown(title: &str) -> String {
    let chars: Vec<char> = title.chars().collect();
    let mut out = String::new();
    let mut code: Option<usize> = None;
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        let run = chars.get(i..).unwrap_or_default().iter().take_while(|&&d| d == c).count();
        let text = chars.get(i..i + run).unwrap_or_default();
        let (before, after) = (i.checked_sub(1).and_then(|b| chars.get(b)).copied(), chars.get(i + run).copied());
        match (c, code) {
            ('`', None) => code = Some(run),
            ('`', Some(open)) if open == run => code = None,
            (_, Some(_)) => out.extend(text),
            ('*', None) => {}
            ('_', None) => {
                let word = |c: Option<char>| c.is_some_and(char::is_alphanumeric);
                let space = |c: Option<char>| c.is_none_or(char::is_whitespace);
                if (word(before) && word(after)) || (space(before) && space(after)) {
                    out.extend(text);
                }
            }
            ('-', None) if run > 1 => {}
            _ => out.extend(text),
        }
        i += run;
    }
    out
}

/// mdBook's id normalization: keep alphanumerics, `_` and `-` (lowercased), turn whitespace into `-`.
pub fn normalize_id(content: &str) -> String {
    content
        .chars()
        .filter_map(|c| {
            if c.is_alphanumeric() || c == '_' || c == '-' {
                Some(c.to_ascii_lowercase())
            } else if c.is_whitespace() {
                Some('-')
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_skip_code_and_find_images() {
        let text = "See [the guide](guide.md#start) and ![a cat](images/cat.png).\n```\n[not](a-link.md)\n```\nAnd `[code](x.md)` too.";
        let links = links(text);
        assert_eq!(links.len(), 2, "{links:?}");
        assert_eq!(links[0].target, "guide.md#start");
        assert!(!links[0].image);
        assert!(links[1].image && links[1].text == "a cat");
    }

    #[test]
    fn link_text_with_brackets_and_titles() {
        let links = links(r#"[ADR [0001]](adr/0001.md "Licence")"#);
        assert_eq!(links[0].target, "adr/0001.md");
        assert_eq!(links[0].text, "ADR [0001]");
    }

    #[test]
    fn heading_ids_match_mdbook() {
        let text = "# Units and coordinates (`stitchcraft-core`)\n## F2 — Failures\n## Repeat\n## Repeat\n### [Linked](x.md) heading\n";
        let ids = heading_ids(text);
        for id in ["units-and-coordinates-stitchcraft-core", "f2--failures", "repeat", "repeat-1", "linked-heading"] {
            assert!(ids.contains(id), "missing {id} in {ids:?}");
        }
    }

    #[test]
    fn heading_ids_are_the_ids_mdbook_writes_with_smart_punctuation() {
        // Built with mdBook 0.5.4 and `smart-punctuation = true`, this page has exactly these ids: `--` becomes
        // an en dash, which an id drops, and emphasis markers are not text. An indented line is code, and a
        // heading in a comment is not shown. Underscores inside a word or a code span stay, and so does a code
        // span's `--`.
        let page = "# Page\n\n## Before -- after\n\n## The _emphasis_ heading\n\n## Rows... and more\n\n    # An indented code line\n\n\
                    <!--\n## A heading in a comment\n-->\n\n## `min_stitch_length_mm` and snake_case\n\n## The `--check` flag\n";
        let mdbook = ["page", "before--after", "the-emphasis-heading", "rows-and-more", "min_stitch_length_mm-and-snake_case", "the---check-flag"];
        assert_eq!(heading_ids(page), mdbook.iter().map(|id| (*id).to_string()).collect::<BTreeSet<_>>());
    }

    #[test]
    fn ci_builds_the_book_with_the_mdbook_these_ids_are_made_for() {
        assert_eq!(crate::util::pinned_version("mdbook").as_deref(), Some("0.5.4"), ".github/actions/tools/action.yml");
    }

    #[test]
    fn the_book_turns_double_hyphens_into_dashes() {
        // Heading ids above assume it.
        let book = std::fs::read_to_string(crate::util::root().join("docs/book.toml")).unwrap();
        assert!(book.lines().any(|line| line.replace(' ', "") == "smart-punctuation=true"), "docs/book.toml");
    }

    #[test]
    fn includes_are_found_in_code_blocks_too_but_not_escaped() {
        let text = "```text\n{{#include generated/a.txt}}\n```\nThe badge: {{#include ../badge.svg:2:10}} and \\{{#include escaped.txt}}.\n";
        assert_eq!(includes(text), [(2, "generated/a.txt".to_string()), (4, "../badge.svg".to_string())]);
    }

    #[test]
    fn inline_code_is_blanked() {
        assert_eq!(without_inline_code("a `b` c"), format!("a{}c", " ".repeat(5)));
        assert_eq!(without_inline_code("x ``a ` b`` y"), format!("x{}y", " ".repeat(11)));
    }
}
