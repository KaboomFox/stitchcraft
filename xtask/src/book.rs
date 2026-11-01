//! `cargo xtask book [--no-build]`: builds the book with mdBook, and checks every link of the pages it made.
//!
//! `cargo xtask docs --check` checks the Markdown, but readers follow the published site, and mdBook
//! changes links on the way there. A section's `README.md` becomes its folder's `index.html`, `print.html`
//! gathers every page into one, and every page gets a "suggest an edit" link made from `edit-url-template`.
//! So a link can be right in the Markdown and broken on the site: the links that named a `README.md` were,
//! and so was the edit link of every page. This reads the HTML mdBook wrote, which is what gets published,
//! and fails when
//!
//! - a link or an image (`href`, `src`) leads to no file of the book, or to a folder without an
//!   `index.html`;
//! - an `#anchor` names no `id` of the page it leads to;
//! - a link to a file of this repository on GitHub, on `main`, names no file or folder of the checkout.
//!   The edit links are such links, and the repository is `git-repository-url` in `docs/book.toml`.
//!
//! A page's relative links start where its `<base href>` says when it has one. mdBook gives the 404 page
//! one, made from `site-url`, so that it works at any depth. Links to other sites are not followed: a check
//! that needs the network would fail for reasons of its own.
//!
//! A problem is reported once, with the first page that has it, since `print.html` repeats every page's.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::util::{self, Findings};

/// The book's settings: where mdBook builds it, where it is published and its repository.
const BOOK_TOML: &str = "docs/book.toml";

/// The routes of a GitHub repository's address that name one of its files or folders at a ref, as in
/// `blob/main/README.md`.
const FILE_ROUTES: &[&str] = &["blob", "tree", "edit", "raw", "blame"];

/// The branch the book is published from. Links to the repository on another ref are not checked.
const BRANCH: &str = "main";

/// `cargo xtask book`: builds the book, unless `no_build`, and checks it.
pub fn run(no_build: bool) -> Result<(), String> {
    if !no_build {
        let mut mdbook = Command::new("mdbook");
        mdbook.args(["build", "docs"]);
        util::run(mdbook, "mdbook build docs")?;
    }
    let root = util::root();
    let settings = Settings::read(&root)?;
    let pages = pages(&settings.dir)?;
    let links: usize = pages.values().map(|scan| scan.links.len()).sum();
    check(&settings, &root, &pages).finish("book", &format!("{} pages and their {links} links checked", pages.len()))
}

/// What `docs/book.toml` says about the built book.
#[derive(Debug)]
pub struct Settings {
    /// The folder mdBook builds the book in.
    dir: PathBuf,
    /// The folder of the Markdown pages, whose files the report names.
    src: PathBuf,
    /// The page the site shows for an address it has no page for (`input-404`). mdBook makes it into
    /// `404.html` from its own file, and it is no chapter of the book.
    pub not_found: PathBuf,
    /// The address of the book on its site: an absolute path that ends in `/`.
    site_url: String,
    /// The repository's address on GitHub, without a `/` at its end.
    repository: Option<String>,
}

impl Settings {
    /// The settings in the repository at `root`, each mdBook's default when the file leaves it out.
    pub fn read(root: &Path) -> Result<Settings, String> {
        let path = root.join(BOOK_TOML);
        let table: toml::Table = util::read(&path)?.parse().map_err(|e| format!("{BOOK_TOML}: {e}"))?;
        let text = |section: &str, key: &str| {
            let mut value = Some(&table);
            for name in section.split('.') {
                value = value.and_then(|t| t.get(name)).and_then(toml::Value::as_table);
            }
            value.and_then(|t| t.get(key)).and_then(toml::Value::as_str).map(str::to_string)
        };
        let folder = path.parent().unwrap_or(root);
        let mut site_url = text("output.html", "site-url").unwrap_or_else(|| "/".to_string());
        if !site_url.ends_with('/') {
            site_url.push('/');
        }
        let src = folder.join(text("book", "src").unwrap_or_else(|| "src".to_string()));
        Ok(Settings {
            dir: folder.join(text("build", "build-dir").unwrap_or_else(|| "book".to_string())),
            not_found: src.join(text("output.html", "input-404").unwrap_or_else(|| "404.md".to_string())),
            src,
            site_url,
            repository: text("output.html", "git-repository-url").map(|url| url.trim_end_matches('/').to_string()),
        })
    }
}

/// The pages of the book built in `dir`, by their path in it (`user/index.html`), each scanned.
fn pages(dir: &Path) -> Result<BTreeMap<String, Scan>, String> {
    let mut pages = BTreeMap::new();
    for path in util::files(dir, &["html"]) {
        let name = path.strip_prefix(dir).map_err(|e| format!("{}: {e}", util::rel(&path)))?.to_string_lossy().replace('\\', "/");
        pages.insert(name, scan(&util::read(&path)?));
    }
    if pages.is_empty() {
        return Err(format!("{} has no pages: build the book first (`mdbook build docs`)", util::rel(dir)));
    }
    Ok(pages)
}

/// The problems of the links of `pages`, the book `settings` describe, whose repository is checked out at
/// `root`.
fn check(settings: &Settings, root: &Path, pages: &BTreeMap<String, Scan>) -> Findings {
    // Each problem, with the pages that have it and the link as each writes it.
    let mut problems: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
    for (page, scan) in pages {
        let base = match base(settings, page, scan.base.as_deref()) {
            Ok(base) => base,
            Err(problem) => {
                problems.entry(problem).or_default().push((page, scan.base.as_deref().unwrap_or_default()));
                continue;
            }
        };
        for link in &scan.links {
            if let Err(problem) = follow(settings, root, pages, &base, link) {
                problems.entry(problem).or_default().push((page, link));
            }
        }
    }
    let mut findings = Findings::default();
    for (problem, places) in &problems {
        let Some((page, link)) = places.first() else { continue };
        let more = match places.len() - 1 {
            0 => String::new(),
            1 => " (and on 1 more page)".to_string(),
            n => format!(" (and on {n} more pages)"),
        };
        findings.error(format!("{}: `{link}` {problem}{more}", shown(settings, page)));
    }
    findings
}

/// `page` as the report names it: the Markdown file it was made from, when it has one, so that the person
/// reading knows what to change.
fn shown(settings: &Settings, page: &str) -> String {
    let stem = page.strip_suffix(".html").unwrap_or(page);
    // A folder's index.html is made from its README.md, or from an index.md.
    let folder = (stem == "index").then_some("").or_else(|| stem.strip_suffix("index").filter(|folder| folder.ends_with('/')));
    let sources = match folder {
        Some(folder) => vec![format!("{folder}README.md"), format!("{folder}index.md")],
        None => vec![format!("{stem}.md")],
    };
    match sources.iter().map(|source| settings.src.join(source)).find(|path| path.is_file()) {
        Some(source) => format!("{} (the book's {page})", util::rel(&source)),
        None => format!("the book's {page}"),
    }
}

/// The path in the book that the relative links of `page` start from: the page's own, or its `<base href>`'s.
/// A link that is only an `#anchor` leads into it, and the others start from its folder.
fn base(settings: &Settings, page: &str, base: Option<&str>) -> Result<String, String> {
    let Some(base) = base else { return Ok(page.to_string()) };
    let target = if has_scheme(base) || base.starts_with("//") {
        None
    } else if base.starts_with('/') {
        inside_site(settings, base)
    } else {
        resolve(folder(page), base)
    };
    target.ok_or_else(|| format!("is the page's `<base href>`, which is outside the book at `{}`", settings.site_url))
}

/// The folder of the book that `path`, a path in the book, is in, with its `/`: empty for the book's own.
fn folder(path: &str) -> &str {
    path.rfind('/').and_then(|end| path.get(..=end)).unwrap_or_default()
}

/// Follows `link` from a page whose relative links start at `base` ([`base`]): `Err` says what is wrong.
fn follow(settings: &Settings, root: &Path, pages: &BTreeMap<String, Scan>, base: &str, link: &str) -> Result<(), String> {
    if let Some(path) = settings.repository.as_deref().and_then(|repository| file_in_repository(repository, link)) {
        return if path.is_empty() || root.join(&path).exists() {
            Ok(())
        } else {
            Err(format!("names `{path}` on GitHub, which is not a file or folder of the repository"))
        };
    }
    if has_scheme(link) || link.starts_with("//") {
        // Another site, or a `mailto:` or `javascript:` link.
        return Ok(());
    }
    let (reference, anchor) = link.split_once('#').map_or((link, None), |(reference, anchor)| (reference, Some(anchor)));
    let reference = reference.split_once('?').map_or(reference, |(path, _)| path);
    let reference = percent_decoded(reference);
    let target = if reference.is_empty() {
        Some(base.to_string())
    } else if reference.starts_with('/') {
        inside_site(settings, &reference)
    } else {
        resolve(folder(base), &reference)
    };
    let target = target.ok_or_else(|| format!("leads out of the book, which is at `{}`", settings.site_url))?;
    let file = if target.is_empty() || target.ends_with('/') || settings.dir.join(&target).is_dir() {
        format!("{}/index.html", target.trim_end_matches('/')).trim_start_matches('/').to_string()
    } else {
        target
    };
    if !settings.dir.join(&file).is_file() {
        return Err(format!("leads to `{file}`, which the book does not have"));
    }
    match (anchor.map(percent_decoded), pages.get(&file)) {
        (Some(anchor), Some(page)) if !anchor.is_empty() && !page.ids.contains(&anchor) => {
            Err(format!("leads to `{file}#{anchor}`, but that page has no element with the id `{anchor}`"))
        }
        _ => Ok(()),
    }
}

/// The path in the book of `path`, an absolute path on the book's site, or `None` when it is outside the
/// book's folder there.
fn inside_site(settings: &Settings, path: &str) -> Option<String> {
    if path == settings.site_url.trim_end_matches('/') {
        return Some(String::new());
    }
    resolve("", path.strip_prefix(&settings.site_url)?)
}

/// The path of the repository's file or folder that `link` names on `main`, when it is an address of the
/// repository at `repository` that names one: `blob/main/README.md` names `README.md`, and the repository's
/// own address names nothing.
fn file_in_repository(repository: &str, link: &str) -> Option<String> {
    let rest = link.strip_prefix(repository)?.strip_prefix('/')?;
    let rest = rest.split(['#', '?']).next().unwrap_or_default();
    let (route, rest) = rest.split_once('/')?;
    let (branch, path) = rest.split_once('/').unwrap_or((rest, ""));
    (FILE_ROUTES.contains(&route) && branch == BRANCH).then(|| percent_decoded(path.trim_end_matches('/')))
}

/// Whether `link` starts with a scheme, such as `https:` or `mailto:` (RFC 3986, section 3.1).
fn has_scheme(link: &str) -> bool {
    link.split_once(':').is_some_and(|(scheme, _)| {
        scheme.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && scheme.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    })
}

/// The path in the book that `reference`, a relative path, leads to from the folder `folder`, or `None`
/// when it climbs out of the book. A path that names a folder keeps its `/` at the end.
fn resolve(folder: &str, reference: &str) -> Option<String> {
    let mut names: Vec<&str> = folder.split('/').filter(|name| !name.is_empty()).collect();
    for name in reference.split('/') {
        match name {
            "" | "." => {}
            ".." => {
                names.pop()?;
            }
            name => names.push(name),
        }
    }
    let mut path = names.join("/");
    let names_a_folder = reference.is_empty() || reference.ends_with('/') || reference.ends_with("/.") || reference.ends_with("/..");
    if names_a_folder && !path.is_empty() {
        path.push('/');
    }
    Some(path)
}

/// `text` with each `%` and two hexadecimal digits made the byte they stand for.
fn percent_decoded(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while let Some(&byte) = bytes.get(at) {
        let escaped = bytes
            .get(at + 1..at + 3)
            .filter(|digits| byte == b'%' && digits.iter().all(u8::is_ascii_hexdigit))
            .and_then(|digits| u8::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok());
        match escaped {
            Some(decoded) => {
                out.push(decoded);
                at += 3;
            }
            None => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// What a page of the built book links to, the anchors it has and its `<base href>`.
#[derive(Debug, Default, PartialEq, Eq)]
struct Scan {
    /// The `href` and `src` of its tags, in order.
    links: Vec<String>,
    /// The ids of its elements, and the names of its `<a name>` anchors.
    ids: BTreeSet<String>,
    /// The `href` of its `<base>` tag.
    base: Option<String>,
}

/// Tags whose contents are code, not markup: an `href="` in a script is a string.
const RAW_TEXT: &[&str] = &["script", "style"];

/// The links, anchors and base of the page `html`. mdBook writes well-formed HTML, so the tags are read as
/// written, without a whole HTML parser: comments and the contents of scripts and styles are skipped, and
/// each tag's attributes read, quoted or not.
fn scan(html: &str) -> Scan {
    let mut scan = Scan::default();
    let mut rest = html;
    while let Some((_, after)) = rest.split_once('<') {
        if let Some(comment) = after.strip_prefix("!--") {
            rest = comment.split_once("-->").map_or("", |(_, after)| after);
            continue;
        }
        let length = after.find(|c: char| !c.is_ascii_alphanumeric()).unwrap_or(after.len());
        let Some((name, tail)) = after.split_at_checked(length) else { break };
        if name.is_empty() {
            // An end tag, a doctype, or a `<` in text.
            rest = tail;
            continue;
        }
        let name = name.to_ascii_lowercase();
        let (attributes, after_tag) = attributes(tail);
        for (key, value) in attributes {
            match key.as_str() {
                "href" if name == "base" => scan.base = Some(value),
                "href" | "src" => scan.links.push(value),
                "id" => {
                    scan.ids.insert(value);
                }
                "name" if name == "a" => {
                    scan.ids.insert(value);
                }
                _ => {}
            }
        }
        rest = after_tag;
        if RAW_TEXT.contains(&name.as_str()) {
            rest = closed(rest, &name);
        }
    }
    scan
}

/// What follows the end tag `</name>` in `text`, matched whatever its case, or nothing when it has none.
fn closed<'a>(text: &'a str, name: &str) -> &'a str {
    let mut rest = text;
    while let Some((_, after)) = rest.split_once("</") {
        if after.get(..name.len()).is_some_and(|tag| tag.eq_ignore_ascii_case(name)) {
            return after.split_once('>').map_or("", |(_, after)| after);
        }
        rest = after;
    }
    ""
}

/// The attributes of a tag whose name has been read from the start of `text`, keys in lower case and values
/// with their character references replaced, and what follows the tag.
fn attributes(mut text: &str) -> (Vec<(String, String)>, &str) {
    let mut out = Vec::new();
    loop {
        text = text.trim_start();
        if let Some(after) = text.strip_prefix('>').or_else(|| text.strip_prefix("/>")) {
            return (out, after);
        }
        let length = text.find(|c: char| c.is_whitespace() || matches!(c, '=' | '>' | '/')).unwrap_or(text.len());
        let Some((key, after_key)) = text.split_at_checked(length) else { return (out, "") };
        if key.is_empty() {
            // A `/` that does not close the tag, or the end of the text.
            let mut chars = text.chars();
            if chars.next().is_none() {
                return (out, "");
            }
            text = chars.as_str();
            continue;
        }
        let after_key = after_key.trim_start();
        let Some(value) = after_key.strip_prefix('=').map(str::trim_start) else {
            out.push((key.to_ascii_lowercase(), String::new()));
            text = after_key;
            continue;
        };
        let (value, after_value) = match value.chars().next() {
            Some(quote @ ('"' | '\'')) => value.get(1..).unwrap_or_default().split_once(quote).unwrap_or((value, "")),
            _ => {
                let length = value.find(|c: char| c.is_whitespace() || c == '>').unwrap_or(value.len());
                value.split_at_checked(length).unwrap_or((value, ""))
            }
        };
        out.push((key.to_ascii_lowercase(), unescaped(value)));
        text = after_value;
    }
}

/// `value` with its character references (`&amp;`, `&#39;`, `&#x27;`) replaced by the characters they stand for.
fn unescaped(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some((before, after)) = rest.split_once('&') {
        out.push_str(before);
        let reference = after.split_once(';').and_then(|(name, tail)| Some((character(name)?, tail)));
        match reference {
            Some((c, tail)) => {
                out.push(c);
                rest = tail;
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

/// The character that the reference `&name;` stands for, of the ones in links and ids.
fn character(name: &str) -> Option<char> {
    match name {
        "amp" => Some('&'),
        "lt" => Some('<'),
        "gt" => Some('>'),
        "quot" => Some('"'),
        "apos" => Some('\''),
        _ => {
            let number = name.strip_prefix('#')?;
            let code = match number.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok()?,
                None => number.parse().ok()?,
            };
            char::from_u32(code)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The repository's address, as `docs/book.toml` gives it.
    const REPOSITORY: &str = "https://github.com/KaboomFox/stitchcraft";

    /// A book in a folder of `target/` that holds `files`, with the settings that mdBook's defaults and
    /// `site_url` make.
    fn book(name: &str, site_url: &str, files: &[(&str, &str)]) -> (Settings, BTreeMap<String, Scan>) {
        let dir = util::root().join("target/xtask-tests/book").join(name);
        let _ = std::fs::remove_dir_all(&dir);
        for (path, text) in files {
            let path = dir.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        let src = dir.join("no-sources");
        let repository = Some(REPOSITORY.to_string());
        let settings = Settings { dir: dir.clone(), not_found: src.join("404.md"), src, site_url: site_url.to_string(), repository };
        let pages = pages(&dir).unwrap();
        (settings, pages)
    }

    fn errors(settings: &Settings, pages: &BTreeMap<String, Scan>) -> Vec<String> {
        check(settings, &util::root(), pages).errors
    }

    #[test]
    fn a_page_is_read_for_its_links_anchors_and_base() {
        let html = r#"<!DOCTYPE html><html><head><base href="/">
<link rel="stylesheet" href="css/a.css"><script src="b.js"></script>
<script>const s = '<a href="in-a-script.html">';</script><style>a[href="in-a-style"] {}</style>
</head><body><!-- <a href="in-a-comment.html"> --><h2 id="first-heading">One</h2>
<a class=nav href=next.html>next</a> <a href="q.html?x=1&amp;y=2#it&#39;s">q</a> <a name="old-anchor"></a>
<img src='images/a.png' alt="a > b"><input disabled><meta name="viewport"></body></html>"#;
        let scan = scan(html);
        assert_eq!(scan.links, ["css/a.css", "b.js", "next.html", "q.html?x=1&y=2#it's", "images/a.png"]);
        assert_eq!(scan.ids, BTreeSet::from(["first-heading".to_string(), "old-anchor".to_string()]));
        assert_eq!(scan.base.as_deref(), Some("/"));
    }

    #[test]
    fn paths_resolve_within_the_book_and_folders_keep_their_slash() {
        assert_eq!(resolve("user/stitches/", "../README.html").as_deref(), Some("user/README.html"));
        assert_eq!(resolve("user/", "stitches/").as_deref(), Some("user/stitches/"));
        assert_eq!(resolve("user/", "..").as_deref(), Some(""));
        assert_eq!(resolve("user/", "./a.html").as_deref(), Some("user/a.html"));
        assert_eq!(resolve("", "../outside.html"), None);
        assert_eq!((folder("user/stitches/running.html"), folder("index.html"), folder("user/")), ("user/stitches/", "", "user/"));
        assert_eq!(percent_decoded("a%20b%2Fc%zz%4"), "a b/c%zz%4");
        assert!(has_scheme("https://example.com") && has_scheme("mailto:a@b.c") && !has_scheme("page.html#a:b") && !has_scheme("../a.html"));
    }

    #[test]
    fn a_link_that_leads_nowhere_in_the_book_fails_with_the_first_page_that_has_it() {
        let (settings, pages) = book(
            "links",
            "/",
            &[
                (
                    "index.html",
                    r##"<h1 id="top">Book</h1><a href="user/">user</a> <a href="user/README.html">gone</a> <a href="user/page.html#there">ok</a>
<a href="user/page.html#gone">no anchor</a> <img src="images/a.png"> <a href="../outside.html">out</a> <a href="#top">top</a>
<a href="#">top too</a> <a href="https://example.com/nothing.html">another site</a> <a href="mailto:a@b.c">mail</a>"##,
                ),
                ("user/index.html", r#"<a href="../index.html#top">home</a> <a href="page.html">page</a> <a href="../user/README.html">gone</a>"#),
                (
                    "user/page.html",
                    r##"<h2 id="there">There</h2><a href="#there">here</a> <a href="#nowhere">nowhere</a> <a href="pictures/">no index</a>"##,
                ),
                ("user/pictures/a.png", ""),
                ("images/a.png", ""),
            ],
        );
        assert_eq!(
            errors(&settings, &pages),
            [
                "the book's index.html: `../outside.html` leads out of the book, which is at `/`",
                "the book's index.html: `user/README.html` leads to `user/README.html`, which the book does not have (and on 1 more page)",
                "the book's index.html: `user/page.html#gone` leads to `user/page.html#gone`, but that page has no element with the id `gone`",
                "the book's user/page.html: `#nowhere` leads to `user/page.html#nowhere`, but that page has no element with the id `nowhere`",
                "the book's user/page.html: `pictures/` leads to `user/pictures/index.html`, which the book does not have",
            ]
        );
    }

    #[test]
    fn a_page_with_a_base_links_from_where_the_site_url_puts_the_book() {
        let files = [
            ("index.html", "<p>home</p>"),
            ("user/page.html", "<p>page</p>"),
            (
                "404.html",
                r#"<base href="/stitchcraft/dev/"><a href="index.html">home</a> <a href="user/page.html">page</a> <a href="gone.html">gone</a>"#,
            ),
        ];
        let (settings, pages) = book("base", "/stitchcraft/dev/", &files);
        assert_eq!(errors(&settings, &pages), ["the book's 404.html: `gone.html` leads to `gone.html`, which the book does not have"]);
        // A book built for the site's root, whose base is outside where it is published, fails.
        let (settings, pages) = book("base", "/elsewhere/", &files);
        assert_eq!(
            errors(&settings, &pages),
            ["the book's 404.html: `/stitchcraft/dev/` is the page's `<base href>`, which is outside the book at `/elsewhere/`"]
        );
    }

    #[test]
    fn links_to_the_repository_on_main_name_its_files() {
        let link = |path: &str| format!(r#"<a href="{REPOSITORY}/{path}">x</a>"#);
        let page = [
            link("edit/main/docs/src/README.md"),
            link("blob/main/xtask/src/book.rs#L1"),
            link("tree/main/docs/src/user/"),
            link("edit/main/docs/src/src/README.md"),
            link("blob/v0.1.0/no/such/file.rs"),
            link("issues/1"),
            format!(r#"<a href="{REPOSITORY}">the repository</a>"#),
        ]
        .concat();
        let (settings, pages) = book("repository", "/", &[("index.html", &page)]);
        assert_eq!(
            errors(&settings, &pages),
            [format!(
                "the book's index.html: `{REPOSITORY}/edit/main/docs/src/src/README.md` names `docs/src/src/README.md` on GitHub, which is not a file \
                 or folder of the repository"
            )]
        );
    }

    #[test]
    fn the_edit_link_of_a_page_names_its_markdown_file() {
        // mdBook puts the page's path from the book's folder, `src/` included, for `{path}` in
        // `edit-url-template`. At ce2fcf2f the template ended in `docs/src/{path}`, and the published
        // page made from docs/src/README.md linked to `…/edit/main/docs/src/src/README.md`, which GitHub
        // has no file for (checked in the gh-pages branch's dev/index.html, 2026-11-01).
        let root = util::root();
        let settings = Settings::read(&root).unwrap();
        let book: toml::Table = std::fs::read_to_string(root.join(BOOK_TOML)).unwrap().parse().unwrap();
        let template = book["output"]["html"]["edit-url-template"].as_str().unwrap();
        let docs = root.join(BOOK_TOML).parent().unwrap().to_path_buf();
        for page in ["README.md", "user/stitches/running.md"] {
            let source = settings.src.join(page);
            let from_book = source.strip_prefix(&docs).unwrap().to_string_lossy().replace('\\', "/");
            let link = template.replace("{path}", &from_book);
            let named = file_in_repository(settings.repository.as_deref().unwrap(), &link);
            assert_eq!(named.as_deref(), Some(util::rel(&source).as_str()), "{link}");
        }
    }

    #[test]
    fn a_page_is_shown_as_the_markdown_file_it_was_made_from() {
        let settings = Settings::read(&util::root()).unwrap();
        assert_eq!(shown(&settings, "user/index.html"), "docs/src/user/README.md (the book's user/index.html)");
        assert_eq!(shown(&settings, "user/stitches/running.html"), "docs/src/user/stitches/running.md (the book's user/stitches/running.html)");
        assert_eq!(shown(&settings, "print.html"), "the book's print.html");
    }
}
