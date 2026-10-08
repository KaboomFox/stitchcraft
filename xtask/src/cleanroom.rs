//! `cargo xtask cleanroom`: keeps code, tests and fixtures free of GPL material
//! (`docs/src/design/adr/0001-license-and-clean-room.md`).
//!
//! Scans everything that ships or tests — `crates/`, `apps/`, `xtask/`, `compat/` and the conformance
//! cases, fixtures and goldens — for GPL-family licence text and for traces of Ink/Stitch's Python source
//! (its module paths and its imports). The design documents are not scanned: they cite Ink/Stitch files as
//! evidence for reviewers. A heuristic, not a proof: the review checklist still asks about provenance.

use std::path::Path;

use crate::util::{self, Findings};

/// Directories scanned, relative to the repository root.
const SCANNED: &[&str] = &["crates", "apps", "xtask", "compat", "conformance/cases", "conformance/fixtures", "conformance/golden"];

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
        (format!("lib/{}/", "stitches"), "an Ink/Stitch source path"),
        (format!("lib/{}/", "elements"), "an Ink/Stitch source path"),
        (format!("lib/{}/", "stitch_plan"), "an Ink/Stitch source path"),
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
    for dir in SCANNED {
        for path in util::files(&root.join(dir), EXTENSIONS) {
            if util::rel(&path) == SELF {
                continue;
            }
            scanned += 1;
            scan_file(&path, &patterns, &mut findings)?;
        }
    }
    findings.finish("cleanroom", &format!("{scanned} files free of GPL text and Ink/Stitch source traces"))
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
    fn flags_gpl_text_and_source_paths() {
        let patterns = patterns();
        let bad = format!("// This program is free software under the GNU {}\nsee lib/{}/fill.py", "General Public License", "stitches");
        assert_eq!(scan(&bad, &patterns).len(), 2);
        assert!(scan("MIT OR Apache-2.0; tatami rows are staggered", &patterns).is_empty());
    }
}
