//! `stitch expand` (roadmap step M13.26, `REQ-CLI-025`; `docs/src/design/image-to-embroidery.md` › Expand):
//! an automatic design written out as ordinary elements with ordinary effects, which sews as the automatic
//! design did.
//!
//! The design is the badge VectorCraft 0.18.0 traced (`traced-badge.vectorcraft`), as `stitch plan
//! picture.png` reads it: with `--auto`.

// Test code may unwrap. It runs `stitch` on files in folders of its own.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

use std::path::Path;
use std::process::{Command, Output};

use stitchcraft_testkit::temp::TempDir;

/// The traced badge.
const TRACED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fixtures/vectorcraft/traced-badge.vectorcraft");

fn stitch(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(dir).output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn req_cli_025_an_expanded_document_sews_as_the_automatic_design() {
    let dir = TempDir::new("expand-badge");
    std::fs::copy(TRACED, dir.join("badge.vectorcraft")).unwrap();
    let automatic = stitch(dir.path(), &["plan", "badge.vectorcraft", "--auto", "-o", "automatic.pes"]);
    assert_eq!(automatic.status.code(), Some(0), "{}", text(&automatic.stderr));
    // Each expanded design is written in a folder of its own, under the design's name, which a PES file
    // keeps as its label.
    for (folder, written) in [("vectorcraft", "badge.vectorcraft"), ("svg", "badge.svg")] {
        std::fs::create_dir(dir.join(folder)).unwrap();
        let written = format!("{folder}/{written}");
        let out = stitch(dir.path(), &["expand", "badge.vectorcraft", "-o", &written]);
        assert_eq!(out.status.code(), Some(0), "{written}: {}", text(&out.stderr));
        assert!(text(&out.stdout).contains("expanded into 7 parts"), "{}", text(&out.stdout));
        // Planned again without `--auto`, the expanded design sews as the automatic one. A PES file holds
        // tenths of a millimetre, so stitches within 0.001 mm of each other write the same bytes, unless one
        // lies on a rounding's edge, and none of the badge's does.
        let planned = format!("{folder}/badge.pes");
        let again = stitch(dir.path(), &["plan", &written, "-o", &planned]);
        assert_eq!(again.status.code(), Some(0), "{written}: {}", text(&again.stderr));
        assert!(std::fs::read(dir.join(&planned)).unwrap() == std::fs::read(dir.join("automatic.pes")).unwrap(), "{written} sews otherwise");
    }
}
