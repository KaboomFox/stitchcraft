//! Automatic digitizing by default for VectorCraft documents (roadmap step M13.8, `REQ-AUTO-163` to
//! `REQ-AUTO-166`, `REQ-CLI-026`, `REQ-CLI-027`; `docs/src/design/image-to-embroidery.md` › Automatic
//! digitizing by default).
//!
//! The designs are the badge VectorCraft 0.18.0 traced (`traced-badge.vectorcraft`), whose shapes have no
//! StitchCraft effect, the patch (`patch.vectorcraft`), every shape of which has one, and the badge's SVG
//! file (`docs/fixtures/traced-badge.svg`). What sews plain is what the library reads with
//! `Reading::default()`, which hosts ask for explicitly.

// Test code may unwrap. It runs `stitch` on files in folders of its own.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use stitchcraft_cli::commands::plan::{Source, machine_for, sew};
use stitchcraft_engine::design::Reading;
use stitchcraft_plan::FormatId;
use stitchcraft_testkit::temp::TempDir;

/// The traced badge: no shape of it has a StitchCraft effect.
const TRACED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fixtures/vectorcraft/traced-badge.vectorcraft");
/// The patch: every shape of it has a StitchCraft effect.
const PATCH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fixtures/vectorcraft/patch.vectorcraft");
/// The badge as an SVG file.
const SVG: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/fixtures/traced-badge.svg");

fn stitch(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(dir).output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The PES file the library writes of the VectorCraft document `bytes`, read as `reading` asks, as `stitch
/// plan` writes it under the design name `name`.
fn sewn(bytes: &[u8], reading: Reading, name: &str) -> Vec<u8> {
    let profile = machine_for(None, Source::VectorCraft, bytes);
    sew(Source::VectorCraft, bytes, (reading, &[]), profile, FormatId::PesV1, name).file.unwrap()
}

/// `stderr` without the lines of the notice `SC-I0924`.
fn without_notice(stderr: &[u8]) -> String {
    text(stderr).lines().filter(|line| !line.contains("SC-I0924")).map(|line| format!("{line}\n")).collect()
}

#[test]
fn req_auto_163_a_vectorcraft_document_sews_as_with_auto_by_default_and_an_svg_file_does_not() {
    let dir = TempDir::new("auto-default-badge");
    std::fs::copy(TRACED, dir.join("badge.vectorcraft")).unwrap();
    std::fs::copy(SVG, dir.join("badge.svg")).unwrap();
    let default = stitch(dir.path(), &["plan", "badge.vectorcraft", "-o", "default.pes"]);
    let asked = stitch(dir.path(), &["plan", "badge.vectorcraft", "--auto", "-o", "asked.pes"]);
    assert_eq!(default.status.code(), Some(0), "{}", text(&default.stderr));
    assert_eq!(asked.status.code(), Some(0), "{}", text(&asked.stderr));
    // The same machine file, byte for byte, and the same diagnostics, the default's notice aside: the cap and
    // the warning of how thick it sews hold as they do with `--auto`.
    let file = |name: &str| std::fs::read(dir.join(name)).unwrap();
    assert_eq!(file("default.pes"), file("asked.pes"));
    assert_eq!(without_notice(&default.stderr), text(&asked.stderr));
    assert!(text(&default.stderr).contains("info SC-I0924"), "{}", text(&default.stderr));
    // An SVG file still sews as Ink/Stitch sews its shapes, and `--auto` still changes it.
    let plain = stitch(dir.path(), &["plan", "badge.svg", "-o", "plain-svg.pes"]);
    let auto = stitch(dir.path(), &["plan", "badge.svg", "--auto", "-o", "auto-svg.pes"]);
    assert_eq!(plain.status.code(), Some(0), "{}", text(&plain.stderr));
    assert_eq!(auto.status.code(), Some(0), "{}", text(&auto.stderr));
    assert_ne!(file("plain-svg.pes"), file("auto-svg.pes"));
    assert!(!text(&plain.stderr).contains("SC-I0924") && !text(&plain.stderr).contains("SC-I0901"), "{}", text(&plain.stderr));
}

#[test]
fn req_auto_164_shapes_set_by_hand_and_expanded_designs_sew_as_they_are_set() {
    let dir = TempDir::new("auto-default-set");
    // Every shape of the patch has a StitchCraft effect: the default changes nothing, and says nothing.
    std::fs::copy(PATCH, dir.join("patch.vectorcraft")).unwrap();
    let patch = stitch(dir.path(), &["plan", "patch.vectorcraft", "-o", "patch.pes"]);
    assert_eq!(patch.status.code(), Some(0), "{}", text(&patch.stderr));
    assert_eq!(std::fs::read(dir.join("patch.pes")).unwrap(), sewn(&std::fs::read(PATCH).unwrap(), Reading::default(), "patch"));
    assert!(!text(&patch.stderr).contains("SC-I0924") && !text(&patch.stderr).contains("SC-I0901"), "{}", text(&patch.stderr));
    // An expanded design is not digitized again: it sews by default as `--no-auto` sews it, which is the
    // automatic design's plan (`REQ-AUTO-142`).
    std::fs::copy(TRACED, dir.join("badge.vectorcraft")).unwrap();
    let expanded = stitch(dir.path(), &["expand", "badge.vectorcraft", "-o", "expanded.vectorcraft"]);
    assert_eq!(expanded.status.code(), Some(0), "{}", text(&expanded.stderr));
    for (args, output) in [(&[][..], "default.pes"), (&["--no-auto"][..], "plain.pes"), (&["--auto"][..], "asked.pes")] {
        let out = stitch(dir.path(), &[&["plan", "expanded.vectorcraft", "-o", output][..], args].concat());
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out.stderr));
        assert!(!text(&out.stderr).contains("SC-I0924"), "{args:?}: {}", text(&out.stderr));
    }
    let file = |name: &str| std::fs::read(dir.join(name)).unwrap();
    assert_eq!(file("default.pes"), file("plain.pes"));
    assert_eq!(file("default.pes"), file("asked.pes"));
}

/// The traced badge with a StitchCraft Fill effect on its first shape.
fn one_set_by_hand() -> Vec<u8> {
    let mut document: Value = serde_json::from_slice(&std::fs::read(TRACED).unwrap()).unwrap();
    let shapes = document.pointer_mut("/document/layers/0/kind/children/0/kind/children").unwrap().as_array_mut().unwrap();
    let first = shapes.iter_mut().find(|node| node["kind"]["type"] == json!("path")).unwrap();
    first["appearance"]["effects"] = json!([{"id": "plugin.dev.stitchcraft.fill", "params": {}, "visible": true}]);
    serde_json::to_vec(&document).unwrap()
}

#[test]
fn req_auto_165_one_notice_counts_the_shapes_digitized_by_default_and_those_set_by_hand() {
    let document = one_set_by_hand();
    // The library says how many are automatic once the shape with the effect is left to it.
    let read =
        stitchcraft_vectorcraft::read_with(&document, &stitchcraft_core::Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap();
    let automatic = read.design.elements().iter().filter(|element| element.automatic).count();
    let set = read.design.elements().len() - automatic;
    assert!(automatic > 1 && set == 1, "{automatic} automatic, {set} set by hand");
    let dir = TempDir::new("auto-default-mixed");
    std::fs::write(dir.join("mixed.vectorcraft"), &document).unwrap();
    let out = stitch(dir.path(), &["plan", "mixed.vectorcraft", "-o", "mixed.pes"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let notices: Vec<String> = text(&out.stderr).lines().filter(|line| line.contains("SC-I0924")).map(str::to_string).collect();
    assert_eq!(notices.len(), 1, "{notices:?}");
    let notice = &notices[0];
    assert!(notice.starts_with("info SC-I0924: "), "{notice}");
    assert!(notice.contains(&format!("{automatic} shapes without StitchCraft effects are digitized automatically")), "{notice}");
    assert!(notice.contains("1 shape is set by hand"), "{notice}");
    assert!(notice.contains("--no-auto"), "{notice}");
    // The shape set by hand sews as its effect says: the file is the one `--auto` writes, where it is left
    // to its effect too.
    let asked = stitch(dir.path(), &["plan", "mixed.vectorcraft", "--auto", "-o", "asked.pes"]);
    assert_eq!(std::fs::read(dir.join("mixed.pes")).unwrap(), std::fs::read(dir.join("asked.pes")).unwrap());
    assert!(!text(&asked.stderr).contains("SC-I0924"), "the person asked: {}", text(&asked.stderr));
}

#[test]
fn req_cli_026_no_auto_sews_a_vectorcraft_document_as_drawn_and_is_refused_where_it_means_nothing() {
    let dir = TempDir::new("auto-default-no-auto");
    std::fs::copy(TRACED, dir.join("badge.vectorcraft")).unwrap();
    std::fs::copy(SVG, dir.join("badge.svg")).unwrap();
    let plain = stitch(dir.path(), &["plan", "badge.vectorcraft", "--no-auto", "-o", "plain.pes"]);
    assert_eq!(plain.status.code(), Some(0), "{}", text(&plain.stderr));
    assert_eq!(std::fs::read(dir.join("plain.pes")).unwrap(), sewn(&std::fs::read(TRACED).unwrap(), Reading::default(), "badge"));
    assert!(!text(&plain.stderr).contains("SC-I0924") && !text(&plain.stderr).contains("SC-I0901"), "{}", text(&plain.stderr));
    // Of an SVG file it is what the file sews without a switch.
    let svg = stitch(dir.path(), &["plan", "badge.svg", "--no-auto", "-o", "svg.pes"]);
    let again = stitch(dir.path(), &["plan", "badge.svg", "-o", "again.pes"]);
    assert_eq!((svg.status.code(), again.status.code()), (Some(0), Some(0)));
    assert_eq!(std::fs::read(dir.join("svg.pes")).unwrap(), std::fs::read(dir.join("again.pes")).unwrap());
    // Where it means nothing, it is refused: the 2 switches together, a picture, which is traced to be
    // digitized automatically, and `stitch expand`, which expands what automatic digitizing makes.
    std::fs::write(dir.join("badge.png"), b"not read").unwrap();
    for (args, said) in [
        (&["plan", "badge.vectorcraft", "--auto", "--no-auto", "-o", "x.pes"][..], "--no-auto"),
        (&["plan", "badge.png", "--no-auto", "-o", "x.pes"][..], "--no-auto"),
        (&["expand", "badge.vectorcraft", "--no-auto", "-o", "x.vectorcraft"][..], "--no-auto"),
    ] {
        let out = stitch(dir.path(), args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out.stderr));
        assert!(text(&out.stderr).contains(said), "{args:?}: {}", text(&out.stderr));
        assert!(!dir.join("x.pes").exists() && !dir.join("x.vectorcraft").exists(), "{args:?}");
    }
}

#[test]
fn req_cli_027_a_bundle_records_the_reading_the_run_used_and_an_old_bundle_replays_as_it_was() {
    let dir = TempDir::new("auto-default-bundle");
    std::fs::copy(TRACED, dir.join("badge.vectorcraft")).unwrap();
    // A run by default records `auto`, so that its replay digitizes the same.
    let out = stitch(dir.path(), &["bug-report", "badge.vectorcraft", "-o", "default.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let mut bundle: Value = serde_json::from_slice(&std::fs::read(dir.join("default.json")).unwrap()).unwrap();
    assert_eq!(bundle["auto"], Value::Bool(true));
    // And that a marker sets its shape by hand, as the default reads it (`REQ-AUTO-166`).
    assert_eq!(bundle["marked_as_drawn"], Value::Bool(true));
    let replay = stitch(dir.path(), &["bug-report", "--replay", "default.json"]);
    assert_eq!(replay.status.code(), Some(0), "{}", text(&replay.stdout));
    // A run with `--no-auto` records nothing, as a bundle from before the default does, and both replay
    // plain: an old bundle reproduces what it recorded.
    let out = stitch(dir.path(), &["bug-report", "badge.vectorcraft", "--no-auto", "-o", "plain.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let plain: Value = serde_json::from_slice(&std::fs::read(dir.join("plain.json")).unwrap()).unwrap();
    assert_eq!((plain.get("auto"), plain.get("marked_as_drawn")), (None, None));
    let out = stitch(dir.path(), &["bug-report", "badge.vectorcraft", "--auto", "-o", "asked.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let asked: Value = serde_json::from_slice(&std::fs::read(dir.join("asked.json")).unwrap()).unwrap();
    assert_eq!((&asked["auto"], asked.get("marked_as_drawn")), (&Value::Bool(true), None));
    let replay = stitch(dir.path(), &["bug-report", "--replay", "plain.json"]);
    assert_eq!(replay.status.code(), Some(0), "{}", text(&replay.stdout));
    // The default's bundle without its `auto` replays plain, as a bundle from before the default does,
    // and so no longer reproduces the automatic run it recorded.
    bundle.as_object_mut().unwrap().remove("auto");
    std::fs::write(dir.join("stripped.json"), serde_json::to_vec(&bundle).unwrap()).unwrap();
    let replay = stitch(dir.path(), &["bug-report", "--replay", "stripped.json"]);
    assert_eq!(replay.status.code(), Some(1), "{}", text(&replay.stdout));
}

/// The patch with the StitchCraft effects taken off the objects named in `plain`: its Background is grouped
/// with a `stitch:start` marker.
fn patch_without_effects(plain: &[&str]) -> Vec<u8> {
    fn strip(node: &mut Value, plain: &[&str]) {
        if node["name"].as_str().is_some_and(|name| plain.contains(&name)) {
            node["appearance"]["effects"] = json!([]);
        }
        if let Some(children) = node.pointer_mut("/kind/children").and_then(Value::as_array_mut) {
            children.iter_mut().for_each(|child| strip(child, plain));
        }
    }
    let mut document: Value = serde_json::from_slice(&std::fs::read(PATCH).unwrap()).unwrap();
    document["document"]["layers"].as_array_mut().unwrap().iter_mut().for_each(|layer| strip(layer, plain));
    serde_json::to_vec(&document).unwrap()
}

#[test]
fn req_auto_166_a_shape_with_a_start_or_end_marker_counts_as_set_by_hand_under_the_default() {
    let dir = TempDir::new("auto-default-marker");
    // The Background without its Fill effect, its start marker beside it: by default it sews as drawn from the
    // marker, as `--no-auto` sews it, and nothing is digitized by default, so nothing is said of it.
    std::fs::write(dir.join("patch.vectorcraft"), patch_without_effects(&["Background"])).unwrap();
    let run = |args: &[&str], output: &str| {
        let out = stitch(dir.path(), &[&["plan", "patch.vectorcraft", "-o", output][..], args].concat());
        assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out.stderr));
        (std::fs::read(dir.join(output)).unwrap(), text(&out.stderr))
    };
    let (default, said) = run(&[], "default.pes");
    let (plain, plain_said) = run(&["--no-auto"], "plain.pes");
    let (asked, asked_said) = run(&["--auto"], "asked.pes");
    assert_eq!(default, plain);
    assert_eq!(said, plain_said);
    assert!(!said.contains("SC-I0924") && !said.contains("SC-I0901"), "{said}");
    // With `--auto` the person asked for automatic digitizing: the shape is digitized, and its marker unused.
    assert_ne!(default, asked);
    assert!(asked_said.contains("SC-I0901"), "{asked_said}");
    let unused = |said: &str| said.lines().filter(|line| line.contains("SC-W0807") && line.contains("markers grouped with it are not used")).count();
    assert_eq!((unused(&said), unused(&asked_said)), (0, 1), "{said}\n{asked_said}");
    // The Stem without its effect too: the notice counts the Background among the shapes set by hand.
    std::fs::write(dir.join("patch.vectorcraft"), patch_without_effects(&["Background", "Stem"])).unwrap();
    let (_, said) = run(&[], "mixed.pes");
    let notice: Vec<&str> = said.lines().filter(|line| line.contains("SC-I0924")).collect();
    assert_eq!(notice.len(), 1, "{said}");
    assert!(notice[0].contains("1 shape without StitchCraft effects is digitized automatically"), "{said}");
    assert!(notice[0].contains("4 shapes are set by hand"), "{said}");
}
