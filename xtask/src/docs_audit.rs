//! Two checks that keep the docs short and true, part of `cargo xtask docs --check`.
//!
//! The docs are context for people and for the AI agents that load them, so each stale or repeated word
//! costs every reader.
//!
//! - **One fact, one page.** A sentence of [`MIN_WORDS`] or more words that is on two pages fails. A fact
//!   written twice drifts apart when only one copy is updated. Pages generated from the code's
//!   registries, the changelog and the vendored lint rules are not compared: the registries are the
//!   single source of their text, and the changelog repeats facts on purpose.
//! - **Paths exist.** A repository path in inline code, such as `crates/stitchcraft-svg/src/lib.rs`,
//!   must name a file or directory that exists. Only this repository's own top-level paths are checked,
//!   because design pages also cite VectorCraft's files, which are in its repository. Decision records
//!   (ADRs) are not checked: they cite files as they were when the decision was made.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::markdown;
use crate::util::{self, Findings};

/// The shortest sentence compared between pages, in words.
pub const MIN_WORDS: usize = 12;

/// Pages never compared: history, templates and third-party text.
const NOT_COMPARED: &[&str] = &["CHANGELOG.md", ".github/", ".vale/"];

/// Pages whose paths are not checked: decision records.
const PATHS_NOT_CHECKED: &[&str] = &["docs/src/design/adr/"];

/// The path prefixes that only this repository has; a path in inline code that starts with one must
/// exist.
const OWN_PATHS: &[&str] = &["crates/stitchcraft-", "apps/stitchcraft-", "xtask/", "conformance/", "docs/src/", "fuzz/", "compat/"];

/// Both checks, over every Markdown page in `pages` (paths absolute, under `root`).
pub fn check(root: &Path, pages: &[(PathBuf, String)], findings: &mut Findings) {
    let mut seen: BTreeMap<String, (String, usize)> = BTreeMap::new();
    for (path, text) in pages {
        let rel = util::rel(path);
        if !PATHS_NOT_CHECKED.iter().any(|skip| rel.starts_with(skip)) {
            paths_exist(root, &rel, text, findings);
        }
        if NOT_COMPARED.iter().any(|skip| rel.starts_with(skip)) || generated(text) {
            continue;
        }
        // In line order; a sentence that a page repeats is compared once, where it first appears.
        let mut here = BTreeMap::new();
        for (line, sentence) in sentences(text) {
            here.entry(sentence).or_insert(line);
        }
        let mut lines: Vec<(usize, String)> = here.into_iter().map(|(sentence, line)| (line, sentence)).collect();
        lines.sort();
        for (line, sentence) in lines {
            if let Some((other, other_line)) = seen.get(&sentence) {
                findings.error(format!("{rel}:{line}: this sentence is also on {other}:{other_line}; keep it on one page and link to it"));
            } else {
                seen.insert(sentence, (rel.clone(), line));
            }
        }
    }
}

/// Whether a page is generated from the code (its first line says so).
pub fn generated(text: &str) -> bool {
    text.lines().next().is_some_and(|first| first.trim_start().to_ascii_lowercase().starts_with("<!-- generated"))
}

/// The page's sentences of [`MIN_WORDS`] or more words, as lowercase words joined by spaces, each with
/// the line it starts on. Prose only: code, tables, headings and comments are left out, and inline code
/// counts as one word.
fn sentences(text: &str) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    let mut words: Vec<String> = Vec::new();
    let mut start = 0;
    let flush = |words: &mut Vec<String>, start: usize, out: &mut Vec<(usize, String)>| {
        if words.len() >= MIN_WORDS {
            out.push((start, words.join(" ")));
        }
        words.clear();
    };
    for (number, line) in markdown::prose_lines(text) {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('|') || trimmed.starts_with('#') || trimmed.starts_with("<!--") {
            flush(&mut words, start, &mut out);
            continue;
        }
        for token in code_as_word(line).split_whitespace() {
            let word: String = token.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
            if words.is_empty() {
                start = number;
            }
            if !word.is_empty() {
                words.push(word);
            }
            if token.ends_with(['.', '!', '?']) {
                flush(&mut words, start, &mut out);
            }
        }
    }
    flush(&mut words, start, &mut out);
    out
}

/// `line` with each inline code span replaced by the word `code`.
fn code_as_word(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open..];
        let ticks = after.len() - after.trim_start_matches('`').len();
        let body = &after[ticks..];
        let Some(close) = body.find(&"`".repeat(ticks)) else { break };
        out.push_str(&rest[..open]);
        out.push_str(" code ");
        rest = &body[close + ticks..];
    }
    out.push_str(rest);
    out
}

/// Every path of this repository named in inline code on the page exists.
fn paths_exist(root: &Path, rel: &str, text: &str, findings: &mut Findings) {
    for (number, line) in markdown::prose_lines(text) {
        for span in inline_code(line) {
            let Some(path) = own_path(span) else { continue };
            if !root.join(path).exists() {
                findings.error(format!("{rel}:{number}: `{span}` names a path that does not exist"));
            }
        }
    }
}

/// The contents of the inline code spans on `line`.
fn inline_code(line: &str) -> Vec<&str> {
    let mut spans = Vec::new();
    let mut rest = line;
    while let Some(open) = rest.find('`') {
        let after = &rest[open..];
        let ticks = after.len() - after.trim_start_matches('`').len();
        let body = &after[ticks..];
        let Some(close) = body.find(&"`".repeat(ticks)) else { break };
        spans.push(body[..close].trim());
        rest = &body[close + ticks..];
    }
    spans
}

/// The repository path a code span names, if it is one of this repository's own paths: line numbers
/// (`:12`) and anchors (`#id`) dropped. Patterns and placeholders are not paths.
fn own_path(span: &str) -> Option<&str> {
    if !OWN_PATHS.iter().any(|prefix| span.starts_with(prefix)) || span.contains(['*', '<', '{', ' ', '…']) || span.contains("..") {
        return None;
    }
    let path = span.split(['#', ':']).next().unwrap_or(span);
    Some(path.trim_end_matches(['.', ',', ')']))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LONG: &str = "The reader leaves out every connector that ties a command to the object it applies to.";

    #[test]
    fn a_long_sentence_on_two_pages_is_reported_once() {
        let root = util::root();
        let (first, second) = LONG.split_at(LONG.find(" connector").unwrap());
        let pages = vec![
            (root.join("docs/src/a.md"), format!("# A\n\n{LONG}\n\nShort and repeated.\n")),
            (root.join("docs/src/b.md"), format!("Intro.\n\n- {first}\n {second}\n\nShort and repeated.\n")),
            (root.join("CHANGELOG.md"), format!("{LONG}\n")),
            (root.join("docs/src/c.md"), format!("<!-- Generated by `cargo xtask docs` -->\n{LONG}\n")),
        ];
        let mut findings = Findings::default();
        check(&root, &pages, &mut findings);
        assert_eq!(findings.errors, ["docs/src/b.md:3: this sentence is also on docs/src/a.md:3; keep it on one page and link to it"]);
    }

    #[test]
    fn sentences_skip_code_tables_and_headings_and_count_inline_code_as_a_word() {
        let text = "# A heading with many words in it that is not a sentence at all\n\n```\ncode with many words that would otherwise be a sentence of twelve words\n```\n| a table row with many words that would otherwise be a sentence here |\n\nThe `cargo xtask docs --check` command reads every page and reports what it finds there today.";
        let found = sentences(text);
        assert_eq!(found, [(8, "the code command reads every page and reports what it finds there today".to_string())]);
    }

    #[test]
    fn own_paths_must_exist_and_others_are_not_checked() {
        let root = util::root();
        let text = "See `crates/stitchcraft-core/src/lib.rs:12`, `docs/src/SUMMARY.md#intro`, `crates/stitchcraft-core/src/nope.rs`,\n\
                    `crates/plugins/src/runtime.rs`, `crates/stitchcraft-*/README.md`, `xtask/src/<module>.rs` and `docs/src/design/`.";
        let mut findings = Findings::default();
        paths_exist(&root, "x.md", text, &mut findings);
        assert_eq!(findings.errors, ["x.md:1: `crates/stitchcraft-core/src/nope.rs` names a path that does not exist"]);
        assert_eq!(inline_code("a ``b `c` d`` e `f`"), ["b `c` d", "f"]);
    }
}
