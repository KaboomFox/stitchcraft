//! `cargo xtask cleanroom`: keeps the repository free of GPL material
//! (`docs/src/design/adr/0012-read-dont-copy.md`).
//!
//! Ink/Stitch's source may be read but never copied. This scans every text file in the repository, code,
//! tests, fixtures, workflows, documents and licence files, whatever their names, for GPL-family licence
//! text (a GPL file came in) and for the lines that start Ink/Stitch's Python (its code was pasted).
//! A file with a NUL byte, or that is not UTF-8, is a picture or a machine file and is passed over.
//! Documents are scanned too, so a design note cannot carry the code to implementers second-hand; a link
//! to the source a note read is fine.
//!
//! - **Licence text**, in any case: a licence's title, as its file spells it in capitals, an SPDX
//!   identifier, and the short notice Python modules carry, which names the licence and its version.
//!   The repository's own mention of Ink/Stitch's licence names it without its text.
//! - **Python imports**: of Ink/Stitch's Inkscape and graph libraries in any form, from shapely, as
//!   Ink/Stitch's modules import it, and the relative imports of a package's modules. The repository's
//!   shapely oracles import the library whole, which is allowed.
//!
//! A heuristic, not a proof: the review checklist still asks that nothing was copied.

use std::path::{Path, PathBuf};

use crate::util::{self, Findings};

/// This file defines the patterns, so it is the one file not scanned.
const SELF: &str = "xtask/src/cleanroom.rs";

/// (pattern, why it is forbidden), matched in any case. Patterns are assembled from pieces, so that a
/// search of the repository for them finds only what they find.
fn patterns() -> Vec<(String, &'static str)> {
    let gnu = "gnu ";
    let spdx = "spdx-license-identifier: ";
    vec![
        (format!("{gnu}general public license"), "GPL licence text"),
        (format!("{gnu}affero general public"), "AGPL licence text"),
        (format!("{gnu}lesser general public"), "LGPL licence text"),
        (format!("{gnu}{} version", "gpl"), "a GPL notice"),
        (format!("{spdx}gpl"), "GPL licence identifier"),
        (format!("{spdx}agpl"), "AGPL licence identifier"),
        (format!("{spdx}lgpl"), "LGPL licence identifier"),
    ]
}

/// Ink/Stitch's libraries a line of Python may not import in any form: its Inkscape and graph libraries.
const LIBRARIES: [&str; 2] = ["inkex", "networkx"];

/// Why a line that imports, read as Python, is pasted source: it imports one of [`LIBRARIES`], or imports
/// from shapely, or from a package's own modules by a relative path. `None` for any other line.
fn pasted_import(line: &str) -> Option<&'static str> {
    let mut words = line.split_whitespace();
    let (keyword, module) = (words.next()?, words.next()?);
    let from = match keyword {
        "from" if words.next() == Some("import") => true,
        "import" => false,
        _ => return None,
    };
    let top = module.split(['.', ',']).next().unwrap_or_default();
    if from && module.starts_with('.') {
        Some("Python source: a relative import, as a package's modules write")
    } else if LIBRARIES.contains(&top) {
        Some("Python source: an import of Ink/Stitch's Inkscape or graph library")
    } else if from && top == "shapely" {
        Some("Python source: an import from shapely, as Ink/Stitch's modules write")
    } else {
        None
    }
}

/// The violations in `text` (one per matching line).
fn scan(text: &str, patterns: &[(String, &str)]) -> Vec<(usize, String)> {
    let mut out = Vec::new();
    for (number, line) in text.lines().enumerate() {
        let lower = line.to_lowercase();
        if let Some((pattern, why)) = patterns.iter().find(|(p, _)| lower.contains(p.as_str())) {
            out.push((number + 1, format!("{why}: `{pattern}`")));
        } else if let Some(why) = pasted_import(line) {
            out.push((number + 1, format!("{why}: `{}`", line.trim())));
        }
    }
    out
}

/// `bytes` as text, or `None` for a file with a NUL byte or that is not UTF-8.
fn text(bytes: &[u8]) -> Option<&str> {
    if bytes.contains(&0) { None } else { std::str::from_utf8(bytes).ok() }
}

/// The files under `root` this gate reads, with their text: every text file but this one.
fn texts(root: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let mut out = Vec::new();
    for path in util::every_file(root) {
        if util::rel(&path) == SELF {
            continue;
        }
        if let Some(text) = text(&util::read_bytes(&path)?) {
            out.push((path, text.to_string()));
        }
    }
    Ok(out)
}

/// `cargo xtask cleanroom`.
pub fn run() -> Result<(), String> {
    let patterns = patterns();
    let mut findings = Findings::default();
    let texts = texts(&util::root())?;
    for (path, text) in &texts {
        for (line, why) in scan(text, &patterns) {
            findings.error(format!("{}:{line}: {why}", util::rel(path)));
        }
    }
    findings.finish("cleanroom", &format!("{} files free of GPL text and pasted Python source", texts.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_text_file_is_scanned_whatever_its_name_and_no_binary_one() {
        let texts = texts(&util::root()).unwrap();
        let scanned: Vec<String> = texts.iter().map(|(path, _)| util::rel(path)).collect();
        for file in ["docs/src/design/tdd.md", "AGENTS.md", "NOTICE", "LICENSE-MIT", ".github/scripts/annotated.sh", "docs/site-root/index.html"] {
            assert!(scanned.iter().any(|f| f == file), "{file} is scanned");
        }
        for file in ["conformance/golden/testsheets/TS-01.pes", "docs/src/images/generated/badge.png", SELF] {
            assert!(!scanned.iter().any(|f| f == file), "{file} is not");
        }
        assert_eq!(text(b"a\0b"), None);
        assert_eq!(text(&[0xff, 0xfe]), None);
        assert_eq!(text(b"plain"), Some("plain"));
    }

    #[test]
    fn flags_gpl_text_and_pasted_python() {
        let patterns = patterns();
        let bad = format!("// This program is free software under the GNU {}\n{} inkex\n", "General Public License", "import");
        assert_eq!(scan(&bad, &patterns).len(), 2);
        assert!(scan("MIT OR Apache-2.0; tatami rows are staggered", &patterns).is_empty());
    }

    #[test]
    fn flags_a_short_gpl_notice_licence_text_in_capitals_and_every_form_of_ink_stitch_s_imports() {
        let patterns = patterns();
        let flagged = |line: &str| scan(line, &patterns).into_iter().next().map(|(_, why)| why);
        // A one-line notice naming the licence by its short name and version, as Python modules carry it,
        // and a licence's title as its own file spells it.
        assert_eq!(
            flagged(&format!("# Shared on the terms of the {} version 2 or later.", "GNU GPL")).as_deref(),
            Some("a GPL notice: `gnu gpl version`")
        );
        assert_eq!(flagged(&format!("{} GENERAL PUBLIC LICENSE", "GNU")).as_deref(), Some("GPL licence text: `gnu general public license`"));
        assert!(flagged(&format!("SPDX-License-Identifier: {}-2.1-or-later", "LGPL")).is_some());
        // Imports of Ink/Stitch's libraries in any form, from shapely, and from a package's own modules.
        for (line, why) in [
            ("from inkex import Path", "an import of Ink/Stitch's Inkscape or graph library"),
            ("import networkx as nx", "an import of Ink/Stitch's Inkscape or graph library"),
            ("import inkex, sys", "an import of Ink/Stitch's Inkscape or graph library"),
            ("from shapely.geometry import Point", "an import from shapely, as Ink/Stitch's modules write"),
            ("    from ..utils import Point", "a relative import, as a package's modules write"),
            ("from . import geometry", "a relative import, as a package's modules write"),
        ] {
            assert_eq!(flagged(line), Some(format!("Python source: {why}: `{}`", line.trim())), "{line}");
        }
        // The repository's own: the shapely oracles import the library whole, and the notice names
        // Ink/Stitch's licence without its text.
        for line in ["import shapely", "import shapely.ops", "Ink/Stitch is licensed under the GNU GPL v3.", "Ink/Stitch's source (GPL-3.0)"] {
            assert_eq!(flagged(line), None, "{line}");
        }
        for prose in ["from there, import the design again", "import", "from shapely's docs, import nothing", "imported = 1"] {
            assert_eq!(flagged(prose), None, "{prose}");
        }
    }

    #[test]
    fn links_to_the_source_a_document_read_are_allowed() {
        // ADR-0012: reading Ink/Stitch's source is allowed and a design doc may link what it read.
        let link = format!("[running stitch](https://github.com/inkstitch/inkstitch/blob/0123abc/lib/{}/running_stitch.py)", "stitches");
        assert!(scan(&link, &patterns()).is_empty());
    }
}
