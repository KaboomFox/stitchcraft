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
/// The badge as an SVG file.
const SVG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/fixtures/traced-badge.svg");

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

#[test]
fn req_cli_025_an_svg_file_is_expanded_as_automatic_digitizing_makes_it_without_auto() {
    let dir = TempDir::new("expand-svg");
    std::fs::copy(SVG, dir.join("badge.svg")).unwrap();
    // An SVG file isn't digitized by default, but `stitch expand` digitizes it all the same.
    for (args, written) in [(&[][..], "plain.svg"), (&["--auto"][..], "auto.svg")] {
        let out = stitch(dir.path(), &[&["expand", "badge.svg", "-o", written][..], args].concat());
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out.stderr));
    }
    assert_eq!(std::fs::read(dir.join("plain.svg")).unwrap(), std::fs::read(dir.join("auto.svg")).unwrap());
}

#[test]
fn req_cli_025_what_stitch_expand_can_t_write_is_a_usage_error() {
    let dir = TempDir::new("expand-usage");
    std::fs::copy(TRACED, dir.join("badge.vectorcraft")).unwrap();
    std::fs::write(dir.join("badge.svg"), "<svg xmlns=\"http://www.w3.org/2000/svg\"/>").unwrap();
    for (args, said) in [
        (&["expand", "badge.vectorcraft", "-o", "badge.pes"][..], "cannot tell what to write as `badge.pes`: name it .vectorcraft or .svg"),
        (
            &["expand", "badge.vectorcraft", "-o", "x.svg", "--size", "50"][..],
            "--vectorcraft-cli, --size, --colours, --keep-colour, --drop-colour and --traced are for a picture",
        ),
        (&["expand", "badge.svg", "-o", "x.vectorcraft"][..], "a VectorCraft document is written from a VectorCraft document or a picture"),
    ] {
        let out = stitch(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert!(text(&out.stderr).contains(said), "{}", text(&out.stderr));
    }
    // A design with nothing to expand is refused, and nothing is written.
    let out = stitch(dir.path(), &["expand", "badge.svg", "-o", "x.svg"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out.stderr));
    assert!(text(&out.stderr).contains("The design has nothing to expand") && !dir.join("x.svg").exists(), "{}", text(&out.stderr));
}
