//! `cargo xtask docs [--check]`: generated pages, and the checks that keep the documentation true.
//!
//! Without `--check` it regenerates the generated pages, and deletes the ones no generator writes any more.
//! With `--check` it writes nothing and fails when:
//!
//! - a generated page or a page's figure ([`crate::figures`]) is stale, or a generated directory holds a
//!   file no generator writes;
//! - a page is missing from `SUMMARY.md`, a relative link or an `#anchor` is broken (ids made as mdBook
//!   makes them, [`crate::markdown`]), or an included file does not exist;
//! - a document mentions a `cargo xtask` subcommand that does not exist;
//! - a `REQ-…` or `DEV-…` id is not in its file, or an `SC-…` code is not registered although its milestone
//!   is done (a page may name a code allocated to a milestone still to come);
//! - the ADR index disagrees with the ADR files;
//! - an image has no alt text, or is not an image a shot in `docs/shots.toml` declares
//!   (`docs/src/design/docs-pipeline.md`);
//! - a help page a VectorCraft dialog links to is not a page of the book, or has no heading its anchor names
//!   ([`help_pages`], `REQ-VC-012`);
//! - a long sentence is on two pages, or a path of the repository named in inline code does not exist
//!   ([`crate::docs_audit`]);
//! - a page's implements comment matches no file, or the branch changes a page's files but neither the page
//!   nor a `Design-reviewed:` line covers it ([`crate::design_map`]).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use stitchcraft_vc_plugin::{Module, dialog};

use crate::markdown::{self, Link};
use crate::util::{self, Findings};
use crate::{conformance, contract_page, design_map, deviations, docs_audit, examples, figures, param_pages, reference_pages, shots};

const DOCS_SRC: &str = "docs/src";
const DIAGNOSTICS_PAGE: &str = "docs/src/design/diagnostics.md";
const ROADMAP: &str = "docs/src/plan/roadmap.md";
const ADR_DIR: &str = "docs/src/design/adr";

/// The files the generators write, by path from the repository root: what `cargo xtask docs` writes and
/// `--check` compares. The docs audit and the prose lint leave these pages to their sources, and only
/// these: a page that says it is generated, and that no generator writes, is a page written by hand.
pub fn generated_paths(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut paths: BTreeSet<String> = reference_pages::paths(root)?.into_iter().chain(param_pages::paths()).collect();
    paths.insert(contract_page::PAGE.to_string());
    Ok(paths)
}

/// `cargo xtask docs`.
pub fn run(check_only: bool) -> Result<(), String> {
    let root = util::root();
    let mut findings = Findings::default();
    generated_pages(&root, check_only, &mut findings)?;
    if !check_only {
        return findings.finish("docs", "generated pages up to date");
    }
    let pages: Vec<(PathBuf, String)> =
        util::files(&root, &["md"]).into_iter().map(|p| util::read(&p).map(|text| (p, text))).collect::<Result<_, _>>()?;
    summary(&root, &mut findings)?;
    for (path, text) in &pages {
        check_links(&root, path, text, &mut findings);
    }
    xtask_mentions(&root, &pages, &mut findings)?;
    ids(&root, &pages, &mut findings)?;
    allocations(&root, &mut findings)?;
    adr_index(&root, &mut findings)?;
    images(&root, &pages, &mut findings)?;
    help_pages(&root, &mut findings);
    docs_audit::check(&root, &pages, &mut findings);
    design_map::check(&root, &pages, &mut findings);
    findings.finish("docs", &format!("{} Markdown files checked", pages.len()))
}

/// Regenerates (or, with `check_only`, compares) every generated page, and removes (or reports) files in
/// the generated directories that no generator writes any more: a parameter page whose declaration is
/// gone, an example's output whose example is. Where the parameter registry, the Ink/Stitch contract and
/// the deviations ledger disagree is an error either way.
fn generated_pages(root: &Path, check_only: bool, findings: &mut Findings) -> Result<(), String> {
    let registry = stitchcraft_engine::registry::PARAMETERS;
    let contract = contract_page::Contract::parse(&util::read(&root.join(contract_page::DATA))?)?;
    for problem in contract.check(registry) {
        findings.error(format!("{}: {problem}", contract_page::DATA));
    }
    let ledger = deviations::parse(&util::read(&root.join(deviations::LEDGER))?)?;
    for problem in deviations::check(&ledger, &conformance::requirement_ids()?, registry) {
        findings.error(format!("{}: {problem}", deviations::LEDGER));
    }
    let mut pages = vec![(contract_page::PAGE.to_string(), contract.render(registry))];
    pages.extend(reference_pages::pages()?);
    pages.extend(param_pages::pages(&ledger)?);
    for orphan in [(param_pages::DIR, "md"), (examples::OUTPUT, "txt")].iter().flat_map(|(dir, kind)| util::files(&root.join(dir), &[kind])) {
        let page = util::rel(&orphan);
        if pages.iter().any(|(path, _)| *path == page) {
            continue;
        }
        if check_only {
            findings.error(format!("{page} is written by no generator any more: run `cargo xtask docs` to delete it"));
        } else {
            std::fs::remove_file(&orphan).map_err(|e| format!("{page}: {e}"))?;
            println!("deleted {page}");
        }
    }
    // Hand-written pages with figures: the figures are written from their shots.
    let shots = shots::load(root)?;
    for path in util::files(&root.join(DOCS_SRC), &["md"]) {
        let page = util::rel(&path);
        let text = util::read(&path)?;
        if figures::has_figures(&text) && !pages.iter().any(|(generated, _)| *generated == page) {
            let fresh = figures::refresh(&page, &text, &shots)?;
            pages.push((page, fresh));
        }
    }
    for (page, fresh) in pages {
        let path = root.join(&page);
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == fresh {
            continue;
        }
        if check_only {
            findings.error(format!("{page} is stale: run `cargo xtask docs`"));
        } else {
            if let Some(dir) = path.parent() {
                std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
            }
            std::fs::write(&path, fresh).map_err(|e| format!("{page}: {e}"))?;
            println!("updated {page}");
        }
    }
    Ok(())
}

/// Every page under `docs/src` is linked from `SUMMARY.md`, and every link there exists.
fn summary(root: &Path, findings: &mut Findings) -> Result<(), String> {
    let src = root.join(DOCS_SRC);
    let summary = util::read(&src.join("SUMMARY.md"))?;
    let listed: BTreeSet<PathBuf> = markdown::links(&summary).into_iter().map(|l| src.join(l.target)).collect();
    for path in &listed {
        if !path.is_file() {
            findings.error(format!("SUMMARY.md links to a missing page: {}", util::rel(path)));
        }
    }
    for page in util::files(&src, &["md"]) {
        if page.file_name().is_some_and(|n| n == "SUMMARY.md") {
            continue;
        }
        if !listed.contains(&page) {
            findings.error(format!("{} is not in docs/src/SUMMARY.md", util::rel(&page)));
        }
    }
    Ok(())
}

/// Relative links and anchors resolve, pages inside `docs/src` link only inside it, and included files
/// exist (mdBook leaves a missing one's directive in the page and builds anyway).
fn check_links(root: &Path, path: &Path, text: &str, findings: &mut Findings) {
    let src = root.join(DOCS_SRC);
    let in_book = path.starts_with(&src);
    let dir = path.parent().unwrap_or(root);
    for Link { line, target, .. } in markdown::links(text) {
        let here = format!("{}:{line}", util::rel(path));
        if target.is_empty() || target.contains("://") || target.starts_with("mailto:") {
            continue;
        }
        let (file_part, anchor) = match target.split_once('#') {
            Some((f, a)) => (f, Some(a)),
            None => (target.as_str(), None),
        };
        let resolved = if file_part.is_empty() { path.to_path_buf() } else { dir.join(file_part) };
        if !resolved.exists() {
            findings.error(format!("{here}: broken link `{target}`"));
            continue;
        }
        if in_book && !normalized(&resolved).starts_with(normalized(&src)) {
            findings.error(format!("{here}: `{target}` leaves docs/src; the published book cannot follow it"));
        }
        if let Some(anchor) = anchor
            && resolved.extension().is_some_and(|e| e == "md")
        {
            let target_text = if resolved == path { text.to_string() } else { std::fs::read_to_string(&resolved).unwrap_or_default() };
            if !markdown::heading_ids(&target_text).contains(anchor) {
                findings.error(format!("{here}: no heading for anchor `#{anchor}` in {}", util::rel(&resolved)));
            }
        }
    }
    for (line, included) in markdown::includes(text) {
        if !dir.join(&included).is_file() {
            findings.error(format!("{}:{line}: `{{{{#include {included}}}}}` names no file", util::rel(path)));
        }
    }
}

/// `path` with `.` and `..` components resolved lexically (the file is known to exist).
fn normalized(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for part in path.components() {
        match part {
            std::path::Component::ParentDir => {
                out.pop();
            }
            std::path::Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// Every `cargo xtask <name>` in docs and workflows names a real subcommand.
fn xtask_mentions(root: &Path, pages: &[(PathBuf, String)], findings: &mut Findings) -> Result<(), String> {
    let mut texts: Vec<(PathBuf, String)> = pages.to_vec();
    for path in util::files(&root.join(".github"), &["yml", "yaml"]) {
        let text = util::read(&path)?;
        texts.push((path, text));
    }
    for (path, text) in &texts {
        for (number, line) in text.lines().enumerate() {
            let mut rest = line;
            while let Some(pos) = rest.find("cargo xtask ") {
                let after = rest.get(pos + "cargo xtask ".len()..).unwrap_or("");
                let name: String = after.chars().take_while(|c| c.is_ascii_lowercase() || *c == '-').collect();
                if !name.is_empty() && !crate::subcommands().contains(&name) {
                    findings.error(format!("{}:{}: `cargo xtask {name}` is not a subcommand", util::rel(path), number + 1));
                }
                rest = after;
            }
        }
    }
    Ok(())
}

/// Requirement and deviation ids mentioned in Markdown are in their files, and diagnostic codes are
/// registered once the milestone the design's table allocates them to is done.
fn ids(root: &Path, pages: &[(PathBuf, String)], findings: &mut Findings) -> Result<(), String> {
    let requirements = conformance::requirement_ids()?;
    let ledger: BTreeSet<String> = deviations::parse(&util::read(&root.join(deviations::LEDGER))?)?.into_iter().map(|d| d.id).collect();
    let registered: BTreeSet<&str> = stitchcraft_core::Code::ALL.iter().map(|code| code.id()).collect();
    let allocated = allocated(&util::read(&root.join(DIAGNOSTICS_PAGE))?);
    let done = done_milestones(&util::read(&root.join(ROADMAP))?);
    for (path, text) in pages {
        for (number, line) in text.lines().enumerate() {
            let here = format!("{}:{}", util::rel(path), number + 1);
            for id in find_ids(line, "REQ-").into_iter().filter(|id| !requirements.contains(id)) {
                findings.error(format!("{here}: {id} is not in conformance/requirements.toml"));
            }
            for id in find_ids(line, "DEV-").into_iter().filter(|id| !ledger.contains(id)) {
                findings.error(format!("{here}: {id} is not in {}", deviations::LEDGER));
            }
            for why in find_codes(line).iter().filter_map(|code| unregistered(code, &registered, &allocated, &done)) {
                findings.error(format!("{here}: {why}"));
            }
        }
    }
    Ok(())
}

/// Why a page may not name `code`, if it may not: an unregistered code must have its row in the design's
/// table ([`allocated`]), for a milestone whose steps are not all done ([`done_milestones`]).
fn unregistered(code: &str, registered: &BTreeSet<&str>, allocated: &BTreeMap<String, String>, done: &BTreeSet<String>) -> Option<String> {
    if registered.contains(code) {
        return None;
    }
    match allocated.get(code) {
        None => Some(format!("{code} is neither registered nor allocated in {DIAGNOSTICS_PAGE}")),
        Some(milestone) if done.contains(milestone) => Some(format!(
            "{code} is allocated to {milestone}, whose steps are done, but is not registered: register it, or move its row in \
             {DIAGNOSTICS_PAGE} to the milestone that will"
        )),
        Some(_) => None,
    }
}

/// The milestone the design's code table on `page` allocates each code to: its row's last cell.
fn allocated(page: &str) -> BTreeMap<String, String> {
    page.lines()
        .filter(|line| line.starts_with("| `SC-"))
        .filter_map(|line| {
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').map(str::trim).collect();
            Some((cells.first()?.trim_matches('`').to_string(), cells.last()?.to_string()))
        })
        .collect()
}

/// The milestones of `roadmap` that have steps and whose steps are all done: each step's row starts with its
/// id and the done mark, `| M3.1 ✅ |`.
fn done_milestones(roadmap: &str) -> BTreeSet<String> {
    let mut steps: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    for line in roadmap.lines() {
        let Some(first) = line.strip_prefix("| M").and_then(|rest| rest.split('|').next()) else { continue };
        let mut words = first.split_whitespace();
        let (Some(step), marks) = (words.next(), words.collect::<Vec<_>>()) else { continue };
        let Some((milestone, number)) = step.split_once('.') else { continue };
        if milestone.is_empty() || !milestone.bytes().all(|b| b.is_ascii_digit()) || !number.bytes().all(|b| b.is_ascii_digit()) {
            continue;
        }
        let tally = steps.entry(format!("M{milestone}")).or_default();
        tally.0 += 1;
        tally.1 += usize::from(marks == ["✅"]);
    }
    steps.into_iter().filter(|(_, (all, done))| all == done).map(|(milestone, _)| milestone).collect()
}

/// Every registered code has its row in the design's code table, with the same severity and a title that
/// starts with the registry's: a code allocated for one thing cannot quietly be registered for another.
fn allocations(root: &Path, findings: &mut Findings) -> Result<(), String> {
    let page = util::read(&root.join(DIAGNOSTICS_PAGE))?;
    for code in stitchcraft_core::Code::ALL {
        let id = code.id();
        let row = page.lines().find(|l| l.starts_with(&format!("| `{id}` |")));
        let cells: Vec<&str> = row.map(|r| r.split('|').map(str::trim).collect()).unwrap_or_default();
        let (Some(severity), Some(title)) = (cells.get(2), cells.get(3)) else {
            findings.error(format!("{id} is registered but has no row in {DIAGNOSTICS_PAGE}: allocate it there"));
            continue;
        };
        let registered = match code.severity() {
            stitchcraft_core::Severity::Error => "Error",
            stitchcraft_core::Severity::Warning => "Warning",
            stitchcraft_core::Severity::Info => "Info",
        };
        if *severity != registered || !title.starts_with(code.title()) {
            findings.error(format!(
                "{id} is registered as {registered} \"{}\" but {DIAGNOSTICS_PAGE} allocates it as {severity} \"{title}\"",
                code.title()
            ));
        }
    }
    Ok(())
}

/// Ids that start with `prefix` in `line`, such as `REQ-AREA[-SUB…]-NNN` or `DEV-AREA-NNN`.
fn find_ids(line: &str, prefix: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(pos) = rest.find(prefix) {
        let tail = rest.get(pos..).unwrap_or("");
        let token: String = tail.chars().take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '-').collect();
        let token = token.trim_end_matches('-');
        let digits = token.rsplit('-').next().unwrap_or("");
        if digits.len() == 3 && digits.chars().all(|c| c.is_ascii_digit()) && token.matches('-').count() >= 2 {
            out.push(token.to_string());
        }
        rest = tail.get(prefix.len()..).unwrap_or("");
    }
    out
}

/// `SC-[EWI]NNNN` codes in `line`.
fn find_codes(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = line;
    while let Some(pos) = rest.find("SC-") {
        let tail = rest.get(pos..).unwrap_or("");
        let code: String = tail.chars().take(8).collect();
        let mut chars = code.chars().skip(3);
        let severity_ok = chars.next().is_some_and(|c| matches!(c, 'E' | 'W' | 'I'));
        let digits_ok = code.chars().count() == 8 && chars.all(|c| c.is_ascii_digit());
        let boundary_ok = !tail.chars().nth(8).is_some_and(|c| c.is_ascii_alphanumeric());
        if severity_ok && digits_ok && boundary_ok {
            out.push(code);
        }
        rest = tail.get(3..).unwrap_or("");
    }
    out
}

/// The ADR index lists every ADR with the status its file declares.
fn adr_index(root: &Path, findings: &mut Findings) -> Result<(), String> {
    let dir = root.join(ADR_DIR);
    let index = util::read(&dir.join("README.md"))?;
    for path in util::files(&dir, &["md"]) {
        let Some(name) = path.file_name().and_then(|n| n.to_str()) else { continue };
        if !name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
            continue;
        }
        let Some(row) = index.lines().find(|l| l.contains(&format!("({name})"))) else {
            findings.error(format!("{ADR_DIR}/README.md does not list {name}"));
            continue;
        };
        let text = util::read(&path)?;
        let declared = text.lines().find_map(|l| l.strip_prefix("**Status:**")).and_then(|s| s.split_whitespace().next()).unwrap_or("");
        let listed = row.trim_end_matches('|').rsplit('|').next().and_then(|s| s.split_whitespace().next()).unwrap_or("");
        if declared.is_empty() || declared != listed {
            findings.error(format!("{name}: status `{declared}` in the file but `{listed}` in the ADR index"));
        }
    }
    Ok(())
}

/// Images have alt text; a local image in the book is one a shot in `docs/shots.toml` declares, at the path
/// the shot writes it to (`cargo xtask shots` checks the shots' own alt texts).
fn images(root: &Path, pages: &[(PathBuf, String)], findings: &mut Findings) -> Result<(), String> {
    let shots = shots::load(root)?;
    let src = root.join(DOCS_SRC);
    for (path, text) in pages {
        for link in markdown::links(text).into_iter().filter(|l| l.image) {
            let here = format!("{}:{}", util::rel(path), link.line);
            if link.text.trim().is_empty() {
                findings.error(format!("{here}: image `{}` has no alt text", link.target));
            }
            if path.starts_with(&src) && !link.target.contains("://") {
                let image = normalized(&path.parent().unwrap_or(root).join(&link.target));
                if !shots.iter().any(|shot| shot.declares(root, &image)) {
                    findings.error(format!("{here}: image `{}` is not declared in docs/shots.toml", link.target));
                }
            }
        }
    }
    Ok(())
}

/// Each help page the plug-in's dialogs link to is a page of the book, with the heading its anchor names:
/// the docs site has `docs/src/<path>.md` at [`dialog::SITE_URL`] and `<path>.html`.
fn help_pages(root: &Path, findings: &mut Findings) {
    for module in Module::all() {
        let manifest = module.manifest();
        for (key, param) in &manifest.params {
            let Some(url) = &param.ui.help_url else { continue };
            if let Err(e) = help_page(root, url) {
                findings.error(format!("{}'s `{key}`: its help page {url} {e}", manifest.id));
            }
        }
    }
}

/// Whether the docs site's address `url` is a page of the book under `root`, with the heading its anchor
/// names: why not, when it isn't.
fn help_page(root: &Path, url: &str) -> Result<(), String> {
    let path = url.strip_prefix(dialog::SITE_URL).ok_or(format!("is not on the docs site, {}", dialog::SITE_URL))?;
    let (page, anchor) = path.split_once('#').map_or((path, None), |(page, anchor)| (page, Some(anchor)));
    let stem = page.strip_suffix(".html").ok_or("is not a page of the book, whose addresses end in `.html`")?;
    let file = root.join(DOCS_SRC).join(format!("{stem}.md"));
    let text = util::read(&file).map_err(|_| format!("is no page: there is no {}", util::rel(&file)))?;
    match anchor {
        Some(anchor) if !markdown::heading_ids(&text).contains(anchor) => Err(format!("has no heading for `#{anchor}` in {}", util::rel(&file))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_vc_012_each_help_page_a_dialog_links_to_is_a_heading_of_the_book() {
        let root = util::root();
        let mut findings = Findings::default();
        help_pages(&root, &mut findings);
        assert!(findings.finish("help pages", "").is_ok());
        let page = |path: &str| help_page(&root, &dialog::page(path));
        assert_eq!(page("user/reference/params/fill.html#fill_method"), Ok(()));
        assert_eq!(page("user/reference/profiles.html"), Ok(()));
        assert!(page("user/reference/params/fill.html#no_such_key").is_err_and(|e| e.contains("has no heading for `#no_such_key`")));
        assert!(page("user/reference/nothing.html").is_err_and(|e| e.starts_with("is no page")));
        assert!(page("user/reference/profiles.md").is_err_and(|e| e.contains("`.html`")));
        assert!(help_page(&root, "https://example.org/a.html").is_err_and(|e| e.starts_with("is not on the docs site")));
    }

    #[test]
    fn finds_requirement_and_deviation_ids_but_not_placeholders() {
        assert_eq!(find_ids("see REQ-FILL-TAT-006 and REQ-RUN-001..003", "REQ-"), vec!["REQ-FILL-TAT-006", "REQ-RUN-001"]);
        assert!(find_ids("REQ-… and REQ-PLAN-* and `REQ-`", "REQ-").is_empty());
        assert_eq!(find_ids("`DEV-SVG-011`, DEV-FILL-TAT-001 and DEV-NNN.", "DEV-"), vec!["DEV-SVG-011", "DEV-FILL-TAT-001"]);
    }

    #[test]
    fn a_code_a_page_names_is_registered_once_its_milestone_is_done() {
        let table =
            "| `SC-W0101` | Warning | Unknown parameter | M3 |\n| `SC-W0901` | Warning | Planned | M7 |\n| `SC-W0902` | Warning | Forgotten | M4 |\n";
        let allocated = allocated(table);
        let registered = BTreeSet::from(["SC-W0101"]);
        let done =
            done_milestones("## M3\n| M3.1 ✅ | a |\n| M3.2 ✅ | b |\n## M4\n| M4.1 ✅ | c |\n## M6\n| M6.1 | d |\n| M0.5 👤 (partly ✅) | e |\n");
        assert_eq!(done, BTreeSet::from(["M3".to_string(), "M4".to_string()]));
        let why = |code: &str| unregistered(code, &registered, &allocated, &done);
        assert_eq!([why("SC-W0101"), why("SC-W0901")], [None, None], "registered, and planned for M7");
        assert!(why("SC-W0902").is_some_and(|why| why.starts_with("SC-W0902 is allocated to M4, whose steps are done, but is not registered")));
        assert!(why("SC-E0999").is_some_and(|why| why.contains("neither registered nor allocated")));
    }

    #[test]
    fn the_milestones_done_are_the_roadmap_s() {
        let done = done_milestones(&util::read(&util::root().join(ROADMAP)).unwrap());
        // M6.11 and M6.12, which use VectorCraft's newer plug-in features, are open.
        assert!(
            ["M1", "M2", "M3", "M4", "M5"].iter().all(|m| done.contains(*m)) && !["M0", "M6", "M7"].iter().any(|m| done.contains(*m)),
            "{done:?}"
        );
    }

    #[test]
    fn finds_diagnostic_codes_but_not_ranges() {
        assert_eq!(find_codes("`SC-E0201`, SC-W0702."), vec!["SC-E0201", "SC-W0702"]);
        assert!(find_codes("SC-E02xx SC-X0001 SC-W07021").is_empty());
    }
}
