//! Just enough Markdown for the docs checks: code fences, inline code, links and mdBook's heading ids.

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

/// Every link and image in the prose of `text` (code blocks and code spans excluded).
pub fn links(text: &str) -> Vec<Link> {
    let mut out = Vec::new();
    for (number, raw) in prose_lines(text) {
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
    for (_, line) in prose_lines(text) {
        let trimmed = line.trim_start();
        let hashes = trimmed.bytes().take_while(|b| *b == b'#').count();
        let Some(rest) = trimmed.get(hashes..) else { continue };
        if !(1..=6).contains(&hashes) || !(rest.is_empty() || rest.starts_with(' ')) {
            continue;
        }
        let title = rest.trim().trim_end_matches('#').trim();
        let id = normalize_id(&plain_heading(title));
        let count = seen.entry(id.clone()).or_insert(0);
        ids.insert(if *count == 0 { id } else { format!("{id}-{count}") });
        *count += 1;
    }
    ids
}

/// Heading text as rendered: link syntax reduced to its text, code and emphasis markers removed.
fn plain_heading(title: &str) -> String {
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
    out.chars().filter(|c| *c != '`' && *c != '*').collect()
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
    fn inline_code_is_blanked() {
        assert_eq!(without_inline_code("a `b` c"), format!("a{}c", " ".repeat(5)));
        assert_eq!(without_inline_code("x ``a ` b`` y"), format!("x{}y", " ".repeat(11)));
    }
}
