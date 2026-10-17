//! VectorCraft's shots before VectorCraft runs: the tolerance, the declarations, and the window's job.

use super::*;
use crate::shots::load;

/// A PNG of `width` × 10 grey pixels, `changed` of which in the first row are 9 lighter.
fn picture(width: u32, changed: usize) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(width, 10).unwrap();
    pixmap.fill(tiny_skia::Color::from_rgba8(100, 100, 100, 255));
    for pixel in pixmap.data_mut().chunks_exact_mut(4).take(changed) {
        pixel[0] = 109;
    }
    pixmap.encode_png().unwrap()
}

#[test]
fn pictures_within_two_pixels_in_a_thousand_are_the_same() {
    let committed = picture(100, 0);
    assert!(close(&committed, &picture(100, 2)), "2 of 1,000 pixels apart");
    assert!(!close(&committed, &picture(100, 3)), "3 of 1,000 pixels apart");
    assert!(!close(&committed, &picture(99, 0)), "another size");
    assert!(!close(&committed, b"not a picture"));
    // 8 apart in a channel is within the tolerance.
    let mut pixmap = tiny_skia::Pixmap::decode_png(&committed).unwrap();
    pixmap.data_mut().iter_mut().step_by(4).for_each(|red| *red += TOLERANCE);
    assert!(close(&committed, &pixmap.encode_png().unwrap()));
}

/// A `kind` shot `probe`, as `docs/shots.toml` would declare it with `toml`.
fn shot(kind: &str, toml: &str) -> Shot {
    toml::from_str(&format!("id = \"probe\"\nkind = \"{kind}\"\nalt = \"A probe\"\n{toml}")).unwrap()
}

#[test]
fn a_vectorcraft_shot_is_declared_with_its_kind_s_fields() {
    let declared = load(&util::root()).unwrap();
    for shot in declared.iter().filter(|shot| matches!(shot.kind.as_str(), RENDER | UI)) {
        assert_eq!(problems(shot), Vec::<String>::new(), "{}", shot.id);
    }
    let render =
        shot(RENDER, "plugins = [\"zigzag\"]\nsteps = [{ export = \"x.png\" }, { apply = \"lace\" }]\nmenu = [\"Effect\"]\nstyle = \"simple\"");
    assert_eq!(
        problems(&render),
        [
            "there is no module `zigzag`",
            "its steps export nothing: the shot exports what it shows",
            "`apply = \"lace\"` names no stitch family",
            "`sheet`, `fixture`, `panels`, `allow` and `style` are a stitch shot's",
            "`menu` and `dialog` are a vectorcraft-ui shot's",
        ]
    );
    assert_eq!(
        problems(&shot(UI, "plugins = [\"fill\"]\nscale = 2.0\ndialog = \"satin\"")),
        [
            "a vectorcraft-ui shot presses a `menu` path",
            "`scale` is a vectorcraft-render shot's: the window's pictures are its pixels",
            "its dialog is `satin`'s, which it doesn't install",
        ]
    );
    assert_eq!(problems(&shot(UI, "menu = [\"Effect\"]\ndialog = \"lace\"")), ["`dialog = \"lace\"` names no stitch family"]);
}

#[test]
fn the_window_s_job_names_the_dialog_s_row_and_its_parameters_from_the_manifest() {
    let ui = shot(UI, "plugins = [\"fill\"]\nmenu = [\"Effect\", \"Plug-ins\"]\ndialog = \"fill\"");
    let entry = job(&ui, Path::new("/out/probe.vectorcraft"), Path::new("/modules"), Path::new("/out/probe.png")).unwrap();
    let manifest = stitchcraft_vc_plugin::manifest::family(Family::Fill);
    assert_eq!(entry["dialog"]["row"], json!(manifest.name));
    let params = entry["dialog"]["params"].as_array().unwrap();
    assert_eq!(params.len(), manifest.params.len());
    assert!(params.contains(&json!(["angle", "number"])) && params.contains(&json!(["fill_method", "choice"])), "{params:?}");
    assert_eq!(entry["menu"], json!(["Effect", "Plug-ins"]));
    assert_eq!(entry["plugins"], json!([Path::new("/modules").join("fill.wasm").to_string_lossy()]));
    // A menu shot has no dialog.
    let menu = shot(UI, "plugins = [\"fill\"]\nmenu = [\"Effect\", \"Plug-ins\"]");
    assert_eq!(job(&menu, Path::new("/d"), Path::new("/m"), Path::new("/p")).unwrap()["dialog"], Value::Null);
}

#[test]
fn a_missing_picture_says_how_to_make_it() {
    let dir = util::root().join("target/xtask-tests/vectorcraft-shots");
    let _ = std::fs::remove_dir_all(&dir);
    let mut findings = Findings::default();
    committed(&dir, &shot(UI, "plugins = [\"fill\"]\nmenu = [\"Effect\"]"), &mut findings);
    assert!(
        findings.errors.iter().any(|e| e.contains("probe.png is missing: `cargo xtask shots --vectorcraft` makes it on Linux")),
        "{:?}",
        findings.errors
    );
}
