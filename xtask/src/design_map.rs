//! Docs that follow the code: each page names the source files it describes, and a branch that changes
//! those files reviews the page.
//!
//! A page declares the files with one comment on its own line, usually right below its title:
//!
//! ```markdown
//! <!-- implements: crates/stitchcraft-formats/src/**, apps/stitchcraft-cli/src/commands/convert.rs -->
//! ```
//!
//! Patterns are paths from the repository root; `*` matches within one path segment and `**` any number of
//! segments. From those comments:
//!
//! - `cargo xtask docs for PATH…` lists the pages that describe each file, so a person or an agent reads
//!   them before changing it. `cargo xtask docs for --hook` does the same for a Claude Code `PostToolUse`
//!   hook (`.claude/settings.json`): it reads the edited file from the hook's input and answers with
//!   context for the agent.
//! - `cargo xtask docs --check` fails when a pattern matches no file, and when a source file of a crate or
//!   an app is in no page's comment.
//! - It also fails when the branch changes a file that a page describes but leaves the page as it is,
//!   unless a commit message on the branch has a `Design-reviewed: <page>` line. Either the page changes
//!   with the code, or someone checked that it still holds. The branch is what changed since the merge
//!   base with `origin/main`, uncommitted changes included; without `origin/main` this part is skipped,
//!   with a warning.

use std::collections::BTreeSet;
use std::io::Read;
use std::path::{Path, PathBuf};

use crate::util::{self, Findings};

/// The comment that declares a page's files.
const MARKER: &str = "<!-- implements:";

/// The commit-message line that records a review of a page the branch did not change.
const REVIEWED: &str = "Design-reviewed:";

/// The source files every page together must describe: the crates and apps.
const SOURCES: &[&str] = &["crates/*/src/**", "apps/*/src/**"];

/// A page and the files it describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Page {
    /// Path from the repository root.
    pub path: String,
    /// Its first heading.
    pub title: String,
    /// The patterns of the files it describes.
    pub patterns: Vec<String>,
}

/// The pages among `pages` (absolute paths with their text) that declare the files they describe.
pub fn pages(pages: &[(PathBuf, String)]) -> Vec<Page> {
    let mut out = Vec::new();
    for (path, text) in pages {
        let patterns: Vec<String> = text
            .lines()
            .filter_map(|line| line.trim().strip_prefix(MARKER)?.strip_suffix("-->"))
            .flat_map(|list| list.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect::<Vec<_>>())
            .collect();
        if patterns.is_empty() {
            continue;
        }
        let title = text.lines().find_map(|l| l.strip_prefix("# ")).unwrap_or_default().trim().to_string();
        out.push(Page { path: util::rel(path), title, patterns });
    }
    out
}

/// Whether `path` matches `pattern`: `*` within a segment, `**` across any number of segments.
pub fn matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.split('/').collect();
    let path: Vec<&str> = path.split('/').collect();
    segments(&pattern, &path)
}

fn segments(pattern: &[&str], path: &[&str]) -> bool {
    match (pattern.split_first(), path.split_first()) {
        (None, None) => true,
        (Some((&"**", rest)), _) => segments(rest, path) || path.split_first().is_some_and(|(_, tail)| segments(pattern, tail)),
        (Some((p, rest)), Some((s, tail))) => segment(p, s) && segments(rest, tail),
        _ => false,
    }
}

/// One path segment against one pattern segment with `*` wildcards.
fn segment(pattern: &str, name: &str) -> bool {
    match pattern.split_once('*') {
        None => pattern == name,
        Some((head, rest)) => {
            let Some(after) = name.strip_prefix(head) else { return false };
            (0..=after.len()).filter(|&i| after.is_char_boundary(i)).any(|i| after.get(i..).is_some_and(|tail| segment(rest, tail)))
        }
    }
}

/// The pages that describe `file`.
pub fn governing<'p>(pages: &'p [Page], file: &str) -> Vec<&'p Page> {
    pages.iter().filter(|page| page.patterns.iter().any(|p| matches(p, file))).collect()
}

/// The checks of `cargo xtask docs --check` for the declarations.
pub fn check(root: &Path, md: &[(PathBuf, String)], findings: &mut Findings) {
    let pages = pages(md);
    let files = tracked(root);
    for page in &pages {
        for pattern in &page.patterns {
            if !files.iter().any(|f| matches(pattern, f)) {
                findings.error(format!("{}: `{pattern}` in its implements comment matches no file", page.path));
            }
        }
    }
    let uncovered: Vec<&String> =
        files.iter().filter(|f| SOURCES.iter().any(|s| matches(s, f)) && f.ends_with(".rs") && governing(&pages, f).is_empty()).collect();
    for file in uncovered {
        findings.error(format!("{file} is in no docs page's implements comment: add it to the page that describes it"));
    }
    match branch() {
        Some((changed, messages)) => {
            for problem in drift(&pages, &changed, &messages) {
                findings.error(problem);
            }
        }
        None => findings.warn("design drift not checked: no merge base with origin/main"),
    }
}

/// The files tracked by Git, plus untracked ones that are not ignored.
fn tracked(root: &Path) -> Vec<String> {
    let listed = util::git(root, &["ls-files", "--cached", "--others", "--exclude-standard"]).unwrap_or_default();
    listed.lines().map(str::to_string).collect()
}

/// What the branch changed since its merge base with `origin/main`, and its commit messages; `None`
/// without a merge base.
fn branch() -> Option<(BTreeSet<String>, String)> {
    let root = util::root();
    let base = util::git(&root, &["merge-base", "origin/main", "HEAD"]).ok()?;
    let base = base.trim();
    let mut changed: BTreeSet<String> = util::git(&root, &["diff", "--name-only", base]).ok()?.lines().map(str::to_string).collect();
    changed.extend(util::git(&root, &["ls-files", "--others", "--exclude-standard"]).ok()?.lines().map(str::to_string));
    let messages = util::git(&root, &["log", "--format=%B", &format!("{base}..HEAD")]).ok()?;
    Some((changed, messages))
}

/// The pages whose files `changed` touches while the page itself is unchanged and no commit message
/// records a review of it.
fn drift(pages: &[Page], changed: &BTreeSet<String>, messages: &str) -> Vec<String> {
    let reviewed: BTreeSet<&str> = messages.lines().filter_map(|l| l.trim().strip_prefix(REVIEWED)).map(str::trim).collect();
    let mut problems = Vec::new();
    for page in pages {
        if changed.contains(&page.path) || reviewed.contains(page.path.as_str()) {
            continue;
        }
        let touched: Vec<&str> = changed.iter().filter(|f| page.patterns.iter().any(|p| matches(p, f))).map(String::as_str).collect();
        if let Some(first) = touched.first() {
            let more = if touched.len() > 1 { format!(" and {} more", touched.len() - 1) } else { String::new() };
            problems.push(format!(
                "{}: the branch changes {first}{more}, which this page describes, but not the page. Update the page, or add `{REVIEWED} {}` to a commit message once you have checked that it still holds",
                page.path, page.path
            ));
        }
    }
    problems
}

/// `cargo xtask docs for PATH…`, or `--hook` for Claude Code.
pub fn run_for(args: &[String]) -> Result<(), String> {
    let root = util::root();
    let md: Vec<(PathBuf, String)> = util::files(&root, &["md"]).into_iter().map(|p| util::read(&p).map(|t| (p, t))).collect::<Result<_, _>>()?;
    let pages = pages(&md);
    if args.first().is_some_and(|a| a == "--hook") {
        let mut input = String::new();
        std::io::stdin().read_to_string(&mut input).map_err(|e| format!("stdin: {e}"))?;
        if let Some(context) = hook_context(&root, &pages, &input) {
            println!("{}", serde_json::json!({ "hookSpecificOutput": { "hookEventName": "PostToolUse", "additionalContext": context } }));
        }
        return Ok(());
    }
    if args.is_empty() {
        return Err("usage: cargo xtask docs for PATH… (or --hook)".to_string());
    }
    for arg in args {
        let file = relative(&root, arg).unwrap_or_else(|| arg.clone());
        let found = governing(&pages, &file);
        if found.is_empty() {
            println!("{file}: no page describes it");
        }
        for page in found {
            println!("{file}: {} ({})", page.path, page.title);
        }
    }
    Ok(())
}

/// `path` relative to the repository root, with `/` separators; `None` for a path outside it.
fn relative(root: &Path, path: &str) -> Option<String> {
    let path = Path::new(path);
    let absolute = if path.is_absolute() { path.to_path_buf() } else { std::env::current_dir().ok()?.join(path) };
    Some(absolute.strip_prefix(root).ok()?.to_string_lossy().replace('\\', "/"))
}

/// What to tell the agent after it edits a file: the pages that describe it. `None` for input that names
/// no file in the repository, or a file that no page describes and that needs none.
fn hook_context(root: &Path, pages: &[Page], input: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(input).ok()?;
    let edited = value.get("tool_input")?.get("file_path")?.as_str()?;
    let file = relative(root, edited)?;
    let found = governing(pages, &file);
    if found.is_empty() {
        let source = SOURCES.iter().any(|s| matches(s, &file)) && file.ends_with(".rs");
        return source.then(|| format!("{file} is in no docs page's implements comment. Add it to the page that describes it."));
    }
    let list: Vec<String> = found.iter().map(|p| format!("{} ({})", p.path, p.title)).collect();
    Some(format!(
        "{file} is described by {}. Keep the page true to the change in the same branch, or add `{REVIEWED} <page>` to a commit message once you have checked it.",
        list.join(", ")
    ))
}

/// For tests: the pages a file is described by, by path.
#[cfg(test)]
fn by_file<'p>(pages: &'p [Page], files: &[&str]) -> std::collections::BTreeMap<String, Vec<&'p str>> {
    files.iter().map(|f| ((*f).to_string(), governing(pages, f).iter().map(|p| p.path.as_str()).collect())).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page(path: &str, patterns: &[&str]) -> Page {
        Page { path: path.to_string(), title: "T".to_string(), patterns: patterns.iter().map(|p| (*p).to_string()).collect() }
    }

    #[test]
    fn patterns_match_segments_and_any_depth() {
        assert!(matches("crates/*/src/**", "crates/stitchcraft-svg/src/lib.rs"));
        assert!(matches("crates/*/src/**", "crates/a/src/x/y/z.rs"));
        assert!(!matches("crates/*/src/**", "crates/a/tests/x.rs"));
        assert!(matches("crates/a/src/**/mod.rs", "crates/a/src/mod.rs"));
        assert!(matches("apps/a/src/commands/c*.rs", "apps/a/src/commands/convert.rs"));
        assert!(!matches("apps/a/src/commands/c*.rs", "apps/a/src/commands/inspect.rs"));
        assert!(matches("docs/src/a.md", "docs/src/a.md") && !matches("docs/src/a.md", "docs/src/a.mdx"));
        assert!(matches("*", "x") && !matches("*", "x/y"));
    }

    #[test]
    fn pages_declare_their_files_in_one_comment() {
        let root = util::root();
        let md = vec![
            (root.join("docs/src/f.md"), "# Formats\n\n<!-- implements: crates/f/src/**, apps/c/src/convert.rs -->\n\nText.".to_string()),
            (root.join("docs/src/other.md"), "# Other\n\nNo comment.".to_string()),
        ];
        let found = pages(&md);
        assert_eq!(found, [page("docs/src/f.md", &["crates/f/src/**", "apps/c/src/convert.rs"]).with_title("Formats")]);
        let map = by_file(&found, &["crates/f/src/pes.rs", "apps/c/src/inspect.rs"]);
        assert_eq!(map["crates/f/src/pes.rs"], ["docs/src/f.md"]);
        assert!(map["apps/c/src/inspect.rs"].is_empty());
    }

    #[test]
    fn a_page_drifts_when_its_code_changes_without_it_or_a_review() {
        let pages = [page("docs/src/f.md", &["crates/f/src/**"]), page("docs/src/r.md", &["crates/r/src/**"])];
        let changed: BTreeSet<String> = ["crates/f/src/a.rs", "crates/f/src/b.rs", "crates/r/src/c.rs"].iter().map(|s| (*s).to_string()).collect();
        let problems = drift(&pages, &changed, "Fix\n\nDesign-reviewed: docs/src/r.md\n");
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].starts_with("docs/src/f.md: the branch changes crates/f/src/a.rs and 1 more, which this page describes"), "{problems:?}");
        let mut with_page = changed;
        with_page.insert("docs/src/f.md".to_string());
        assert!(drift(&pages, &with_page, "Design-reviewed: docs/src/r.md").is_empty());
    }

    #[test]
    fn the_hook_names_the_pages_of_the_edited_file() {
        let root = util::root();
        let pages = [page("docs/src/f.md", &["crates/f/src/**"])];
        // Built as JSON, as Claude Code sends it: a Windows path's backslashes are escaped.
        let input = |path: &str| serde_json::json!({ "tool_name": "Edit", "tool_input": { "file_path": path } }).to_string();
        let said = hook_context(&root, &pages, &input(&root.join("crates/f/src/a.rs").to_string_lossy())).unwrap();
        assert!(said.starts_with("crates/f/src/a.rs is described by docs/src/f.md (T)."), "{said}");
        let lonely = hook_context(&root, &pages, &input(&root.join("crates/g/src/b.rs").to_string_lossy())).unwrap();
        assert!(lonely.contains("is in no docs page's implements comment"), "{lonely}");
        // A backslash, which is the separator on Windows and a character of the name elsewhere.
        let windows = hook_context(&root, &pages, &input(&root.join("crates/f/src/a\\q.rs").to_string_lossy())).unwrap();
        assert!(windows.starts_with("crates/f/src/a/q.rs is described by docs/src/f.md (T)."), "{windows}");
        assert_eq!(hook_context(&root, &pages, &input(&root.join("docs/src/x.md").to_string_lossy())), None);
        assert_eq!(hook_context(&root, &pages, &input("/elsewhere/a.rs")), None);
        assert_eq!(hook_context(&root, &pages, "not json"), None);
    }

    impl Page {
        fn with_title(mut self, title: &str) -> Page {
            self.title = title.to_string();
            self
        }
    }
}
