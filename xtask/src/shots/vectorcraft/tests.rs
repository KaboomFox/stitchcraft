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

/// A PNG of `width` × `height` grey pixels, with the `block` (x, y, width, height) 9 lighter in red.
fn picture_with(width: u32, height: u32, block: (u32, u32, u32, u32)) -> Vec<u8> {
    let mut pixmap = tiny_skia::Pixmap::new(width, height).unwrap();
    pixmap.fill(tiny_skia::Color::from_rgba8(100, 100, 100, 255));
    let (bx, by, bw, bh) = block;
    for (i, pixel) in pixmap.data_mut().chunks_exact_mut(4).enumerate() {
        let i = u32::try_from(i).unwrap();
        let (x, y) = (i % width, i / width);
        if (bx..bx + bw).contains(&x) && (by..by + bh).contains(&y) {
            pixel[0] = 109;
        }
    }
    pixmap.encode_png().unwrap()
}

#[test]
fn a_change_in_one_place_is_never_close() {
    // A setting's value changed in a dialog: 600 pixels of a 400 × 1,400 picture, 0.11 % of it.
    let committed = picture_with(400, 1400, (0, 0, 0, 0));
    assert!(!close(&committed, &picture_with(400, 1400, (200, 700, 30, 20))));
    // A few pixels apart in each of several places stay close, as an anti-aliased edge moved.
    let mut scattered = tiny_skia::Pixmap::decode_png(&committed).unwrap();
    for (i, pixel) in scattered.data_mut().chunks_exact_mut(4).enumerate().filter(|(i, _)| i % 997 == 0) {
        pixel[0] = 109 + u8::try_from(i % 2).unwrap();
    }
    assert!(close(&committed, &scattered.encode_png().unwrap()));
}

#[test]
fn a_ui_shot_the_window_took_no_picture_of_is_an_error() {
    let ui = shot(UI, "plugins = [\"fill\"]\nmenu = [\"Effect\"]");
    let entry = json!({ "id": "probe", "picture": "/no/such/picture.png" });
    let mut findings = Findings::default();
    kept(&util::root(), &[(&ui, entry)], true, &mut findings).unwrap();
    assert!(findings.errors.iter().any(|e| e.contains("shot `probe`: VectorCraft's window took no picture")), "{:?}", findings.errors);
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
    // VectorCraft keeps a module's last error only until its next success: one apply a family.
    let twice = shot(RENDER, "plugins = [\"fill\"]\nsteps = [{ apply = \"fill\" }, { apply = \"satin\" }, { apply = \"fill\" }]");
    assert_eq!(
        problems(&twice),
        [
            "it applies `satin` without installing that module",
            "it applies `fill` twice: VectorCraft keeps a module's last error only until its next success, so the second could hide the first's",
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
    assert_eq!(problems(&shot(UI, "menu = [\"Effect\"]\ndocument = \"a.vectorcraft\"")), ["`document` is a vectorcraft-render shot's"]);
    assert_eq!(problems(&shot(RENDER, "document = \"a.svg\"")), ["its document `a.svg` is not a `.vectorcraft` file"]);
}

#[test]
fn the_window_s_job_names_the_dialog_s_row_and_its_parameters_from_the_manifest() {
    let ui = shot(UI, "plugins = [\"fill\"]\nmenu = [\"Effect\", \"Plug-ins\"]\ndialog = \"fill\"");
    let entry = job(&ui, "v0.7.0", Path::new("/out/probe.vectorcraft"), Path::new("/modules"), Path::new("/out/probe.png")).unwrap();
    let manifest = stitchcraft_vc_plugin::manifest::family(Family::Fill);
    assert_eq!(entry["dialog"]["row"], json!(manifest.name));
    let params = entry["dialog"]["params"].as_array().unwrap();
    assert_eq!(params.len(), manifest.params.len());
    // Each with the control VectorCraft gives its type.
    for row in [json!(["angle", "text_field"]), json!(["fill_method", "combo_box"]), json!(["fill", "check_box"])] {
        assert!(params.contains(&row), "{row} in {params:?}");
    }
    assert_eq!(control(&Schema::Int { min: 0, max: 1, default: 0 }, "v0.7.0"), "text_field");
    // VectorCraft 0.12.0's toggles show no role.
    let entry = job(&ui, UNNAMED_CHECKBOXES, Path::new("/out/probe.vectorcraft"), Path::new("/modules"), Path::new("/out/probe.png")).unwrap();
    let params = entry["dialog"]["params"].as_array().unwrap();
    assert!(params.contains(&json!(["fill", "unknown"])) && params.contains(&json!(["angle", "text_field"])), "{params:?}");
    assert_eq!(entry["window"], json!([1440, 2000]), "a dialog's picture is taken in a taller window");
    assert_eq!(entry["menu"], json!(["Effect", "Plug-ins"]));
    assert_eq!(entry["plugins"], json!([Path::new("/modules").join("fill.wasm").to_string_lossy()]));
    // A menu's picture has no dialog, and the window keeps its size.
    let menu = shot(UI, "plugins = [\"fill\"]\nmenu = [\"Effect\", \"Plug-ins\"]");
    let entry = job(&menu, "v0.7.0", Path::new("/d"), Path::new("/m"), Path::new("/p")).unwrap();
    assert_eq!((&entry["dialog"], &entry["window"]), (&Value::Null, &Value::Null));
}

#[test]
fn a_kept_document_is_written_only_when_it_sews_otherwise() {
    let root = util::root();
    let dir = "target/xtask-tests/vectorcraft-document";
    let _ = std::fs::remove_dir_all(root.join(dir));
    std::fs::create_dir_all(root.join(dir)).unwrap();
    let kept = format!("{dir}/patch.vectorcraft");
    let drawn = std::fs::read(root.join("docs/fixtures/round-patch.vectorcraft")).unwrap();
    let errors = |drawn: &[u8], check_only: bool| {
        let mut findings = Findings::default();
        keep_document(&root, &kept, drawn, check_only, &mut findings);
        findings.errors
    };
    assert!(errors(&drawn, true).iter().any(|e| e.contains("patch.vectorcraft is missing")));
    assert!(errors(&drawn, false).is_empty());
    assert_eq!(std::fs::read(root.join(&kept)).unwrap(), drawn, "written");
    // The same drawing written otherwise, here without its indentation, sews the same and is kept.
    let compact = serde_json::to_vec(&serde_json::from_slice::<Value>(&drawn).unwrap()).unwrap();
    assert!(errors(&compact, true).is_empty());
    // With the fill's rows at 90 degrees it sews otherwise.
    let turned = String::from_utf8(drawn).unwrap().replace("\"angle\": 45", "\"angle\": 90");
    let said = errors(turned.as_bytes(), true);
    assert!(said.iter().any(|e| e.contains("patch.vectorcraft sews otherwise than the document VectorCraft draws")), "{said:?}");
    assert!(errors(b"{}", true).iter().any(|e| e.contains("the document VectorCraft drew:")));
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
