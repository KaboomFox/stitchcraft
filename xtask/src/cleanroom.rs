//! `cargo xtask cleanroom`: keeps the repository free of GPL material
//! (`docs/src/design/adr/0012-read-dont-copy.md`).
//!
//! Ink/Stitch's source may be read but never copied. This scans every text file in the repository —
//! code, tests, fixtures, workflows and documents — for GPL-family licence text (a GPL file came in) and
//! for the imports of Ink/Stitch's Python (its code was pasted). Documents are scanned too, so a design
//! note cannot carry the code to implementers second-hand; a link to the source a note read is fine. A
//! heuristic, not a proof: the review checklist still asks that nothing was copied.

use std::path::Path;

use crate::util::{self, Findings};

/// File types scanned.
const EXTENSIONS: &[&str] = &["rs", "toml", "md", "svg", "json", "csv", "txt", "py", "yml", "yaml"];

/// This file defines the patterns, so it is the one file not scanned.
const SELF: &str = "xtask/src/cleanroom.rs";

/// (pattern, why it is forbidden). Patterns are assembled from pieces so this file never matches itself.
fn patterns() -> Vec<(String, &'static str)> {
    let gnu = "GNU ";
    let spdx = "SPDX-License-Identifier: ";
    vec![
        (format!("{gnu}General Public License"), "GPL licence text"),
        (format!("{gnu}Affero General Public"), "AGPL licence text"),
        (format!("{gnu}Lesser General Public"), "LGPL licence text"),
        (format!("{spdx}GPL"), "GPL licence identifier"),
        (format!("{spdx}AGPL"), "AGPL licence identifier"),
        (format!("{spdx}LGPL"), "LGPL licence identifier"),
        (format!("import {}", "inkex"), "Ink/Stitch's Inkscape dependency (Python source)"),
        (format!("from {} import", "shapely"), "Python source"),
        (format!("import {}", "networkx"), "Python source"),
    ]
}

/// The violations in `text` (one per matching line).
fn scan(text: &str, patterns: &[(String, &str)]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        if let Some((pattern, why)) = patterns.iter().find(|(p, _)| line.contains(p.as_str())) {
            out.push((number + 1, format!("{why}: `{pattern}`")));
        }
    }
    out
}

/// `cargo xtask cleanroom`.
pub fn run() -> Result<(), String> {
    let root = util::root();
    let patterns = patterns();
    let mut findings = Findings::default();
    let mut scanned = 0usize;
    for path in util::files(&root, EXTENSIONS) {
        if util::rel(&path) == SELF {
            continue;
        }
        scanned += 1;
        scan_file(&path, &patterns, &mut findings)?;
    }
    findings.finish("cleanroom", &format!("{scanned} files free of GPL text and pasted Python source"))
}

fn scan_file(path: &Path, patterns: &[(String, &str)], findings: &mut Findings) -> Result<(), String> {
    let text = util::read(path)?;
    for (line, why) in scan(&text, patterns) {
        findings.error(format!("{}:{line}: {why}", util::rel(path)));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documents_are_scanned_too() {
        let files: Vec<String> = util::files(&util::root(), EXTENSIONS).iter().map(|p| util::rel(p)).collect();
        assert!(files.iter().any(|f| f == "docs/src/design/tdd.md"), "the design documents are scanned");
        assert!(files.iter().any(|f| f == "AGENTS.md"));
    }

    #[test]
    fn flags_gpl_text_and_pasted_python() {
        let patterns = patterns();
        let bad = format!("// This program is free software under the GNU {}\n{} inkex\n", "General Public License", "import");
        assert_eq!(scan(&bad, &patterns).len(), 2);
        assert!(scan("MIT OR Apache-2.0; tatami rows are staggered", &patterns).is_empty());
    }

    #[test]
    fn links_to_the_source_a_document_read_are_allowed() {
        // ADR-0012: reading Ink/Stitch's source is allowed and a design doc may link what it read.
        let link = format!("[running stitch](https://github.com/inkstitch/inkstitch/blob/0123abc/lib/{}/running_stitch.py)", "stitches");
        assert!(scan(&link, &patterns()).is_empty());
    }
}
