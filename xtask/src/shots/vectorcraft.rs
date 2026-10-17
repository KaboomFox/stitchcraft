//! The VectorCraft shots, `vectorcraft-render` and `vectorcraft-ui`: pictures that VectorCraft makes itself,
//! of its canvas with StitchCraft's previews, and of its menus and dialogs with StitchCraft's plug-ins in
//! them (`docs/src/design/docs-pipeline.md` › Images).
//!
//! **Why VectorCraft makes them.** A picture of VectorCraft that a person took goes stale with its next
//! release, and nobody notices. These are made by the stable release that `compat/vectorcraft.toml` pins,
//! from its Linux archive (`compat/release.rs`), on each run of the `shots-ui` job:
//!
//! - A shot's document is drawn by `vectorcraft-cli`, with the plug-in's modules installed and the shot's
//!   steps, as Level B's scenarios draw theirs (`compat/scenario.rs`). So VectorCraft only ever opens what
//!   it wrote. The run ends with a drawing, which runs the live effects, and each module is asked for its
//!   last error after it. VectorCraft keeps a module's last error only until its next success, so a shot
//!   applies each family once.
//! - A `vectorcraft-render` shot is that drawing, a PNG export: the canvas, live effects and all, without
//!   the window around it.
//! - A `vectorcraft-ui` shot opens the document in VectorCraft's window, in a desktop of its own
//!   (`window.rs`), where the window probe's `tests/shots.rs` presses the shot's menu path through the
//!   accessibility tree and crops the picture to the menus, or to the dialog of the family's effect. A
//!   dialog is taken in a taller window (`window::DIALOG_WINDOW`), since it doesn't scroll, and must show a
//!   row for each parameter of the module's manifest, which the plug-in crate makes from the parameter
//!   registry and gives here: a label, and the control VectorCraft gives the parameter's type.
//!
//! **Why a tolerance.** Mesa draws VectorCraft's window in software, and a newer Mesa from Ubuntu's updates
//! may move an anti-aliased edge. A picture stays while no more than [`PER_THOUSAND`] in a thousand of its
//! pixels differ from the committed one by more than [`TOLERANCE`] in a channel, 0.2 %, so that the pictures
//! change when VectorCraft does.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};
use stitchcraft_params::Family;
use stitchcraft_vc_plugin::manifest::Schema;

use super::{GENERATED, Same, Shot, keep};
use crate::compat::scenario::{self, Run, Step};
use crate::compat::{self, release};
use crate::util::{self, Findings};
use crate::window;

/// The kind of a picture of VectorCraft's canvas, by `vectorcraft-cli`.
pub(super) const RENDER: &str = "vectorcraft-render";
/// The kind of a picture of VectorCraft's menus or a dialog, from its window.
pub(super) const UI: &str = "vectorcraft-ui";
/// How many pixels in a thousand may differ from the committed picture's by more than [`TOLERANCE`].
const PER_THOUSAND: u64 = 2;
/// How far a channel of a pixel may differ, of 255, before the pixel counts as different.
const TOLERANCE: u8 = 8;
/// Where the shots' documents and pictures, and the window's records, are made.
const OUT: &str = "target/shots-vectorcraft";
/// The drawing that ends a `vectorcraft-ui` shot's run, in its folder: a render shot's is its picture.
const CANVAS: &str = "canvas.png";

/// Whether the pictures `a` and `b`, two PNG files, are the same within the tolerance: of one size, with no
/// more than [`PER_THOUSAND`] in a thousand of their pixels further apart than [`TOLERANCE`] in a channel.
pub(super) fn close(a: &[u8], b: &[u8]) -> bool {
    let (Ok(a), Ok(b)) = (tiny_skia::Pixmap::decode_png(a), tiny_skia::Pixmap::decode_png(b)) else { return false };
    if (a.width(), a.height()) != (b.width(), b.height()) {
        return false;
    }
    let apart = a
        .data()
        .chunks_exact(4)
        .zip(b.data().chunks_exact(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > TOLERANCE))
        .count();
    let pixels = u64::from(a.width()) * u64::from(a.height());
    (apart as u64).saturating_mul(1000) <= PER_THOUSAND.saturating_mul(pixels)
}

/// Without `--vectorcraft`: `shot` is declared as a VectorCraft shot can be, and its picture is committed.
pub(super) fn committed(root: &Path, shot: &Shot, findings: &mut Findings) {
    for problem in problems(shot) {
        findings.error(format!("shot `{}`: {problem}", shot.id));
    }
    let path = root.join(GENERATED).join(format!("{}.png", shot.id));
    if !path.is_file() {
        findings.error(format!(
            "{} is missing: `cargo xtask shots --vectorcraft` makes it on Linux, or add the `docs:refresh-shots` label to the pull request",
            util::rel(&path)
        ));
    }
}

/// What is wrong with the VectorCraft shot `shot` before VectorCraft runs: modules and families that don't
/// exist, steps that don't read or export, a family applied twice or without its module, and fields of
/// another kind.
fn problems(shot: &Shot) -> Vec<String> {
    let mut found = Vec::new();
    let modules = compat::modules();
    for module in shot.plugins.iter().filter(|module| !modules.contains(&module.as_str())) {
        found.push(format!("there is no module `{module}`"));
    }
    let mut applied = BTreeSet::new();
    for step in &shot.steps {
        if step.export.is_some() {
            found.push("its steps export nothing: the shot exports what it shows".to_string());
        } else if let Err(e) = step.check() {
            found.push(e);
        } else if let Some(family) = &step.apply {
            if !shot.plugins.contains(family) {
                found.push(format!("it applies `{family}` without installing that module"));
            }
            if !applied.insert(family) {
                found.push(format!(
                    "it applies `{family}` twice: VectorCraft keeps a module's last error only until its next success, so the second could hide the first's"
                ));
            }
        }
    }
    let stitch_fields = shot.sheet.is_some() || shot.fixture.is_some() || !shot.panels.is_empty() || !shot.allow.is_empty() || shot.style.is_some();
    if stitch_fields {
        found.push("`sheet`, `fixture`, `panels`, `allow` and `style` are a stitch shot's".to_string());
    }
    if shot.kind == RENDER && (!shot.menu.is_empty() || shot.dialog.is_some()) {
        found.push("`menu` and `dialog` are a vectorcraft-ui shot's".to_string());
    }
    if shot.kind == UI {
        if shot.menu.is_empty() {
            found.push("a vectorcraft-ui shot presses a `menu` path".to_string());
        }
        if shot.scale.is_some() {
            found.push("`scale` is a vectorcraft-render shot's: the window's pictures are its pixels".to_string());
        }
        match shot.dialog.as_deref() {
            Some(name) if Family::ALL.iter().all(|f| f.id() != name) => found.push(format!("`dialog = \"{name}\"` names no stitch family")),
            Some(name) if !shot.plugins.iter().any(|m| m == name) => found.push(format!("its dialog is `{name}`'s, which it doesn't install")),
            _ => {}
        }
    }
    found
}

/// With `--vectorcraft`: makes the VectorCraft shots, and writes their pictures or, with `check_only`,
/// compares them.
pub(super) fn run(root: &Path, shots: &[Shot], check_only: bool, findings: &mut Findings) {
    let shots: Vec<&Shot> = shots.iter().filter(|shot| matches!(shot.kind.as_str(), RENDER | UI)).collect();
    let mut sound = true;
    for shot in &shots {
        for problem in problems(shot) {
            findings.error(format!("shot `{}`: {problem}", shot.id));
            sound = false;
        }
    }
    if !sound || shots.is_empty() {
        return;
    }
    if let Err(e) = make(root, &shots, check_only, findings) {
        findings.error(format!("VectorCraft's shots: {e}"));
    }
}

/// Makes `shots` with the pinned stable release: the documents and the canvases with `vectorcraft-cli`,
/// then the menus and dialogs in its window.
fn make(root: &Path, shots: &[&Shot], check_only: bool, findings: &mut Findings) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("they are made on Linux, by the pinned release's Linux archive".to_string());
    }
    let pins = compat::load_pins()?;
    let fetched = release::fetch(compat::pinned_tag(&pins, "stable"))?;
    let modules = compat::build_modules()?;
    let out = root.join(OUT);
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("{OUT}: {e}"))?;
    }
    let mut ui = Vec::new();
    for shot in shots {
        let dir = out.join(&shot.id);
        let (document, picture) = (format!("{}.vectorcraft", shot.id), format!("{}.png", shot.id));
        let export = |file: &str| Step { cmd: None, apply: None, params: None, export: Some(file.to_string()) };
        let mut steps = shot.steps.clone();
        steps.push(export(&document));
        steps.push(export(if shot.kind == RENDER { &picture } else { CANVAS }));
        let run = Run { open: None, plugins: shot.plugins.clone(), steps, scale: shot.scale.map(f64::from) };
        if let Err(e) = scenario::draw(&run, &fetched.root.join("bin/vectorcraft-cli"), &modules, &dir) {
            findings.error(format!("shot `{}`: {e}", shot.id));
        } else if shot.kind == RENDER {
            keep(root, &shot.id, &util::read_bytes(&dir.join(&picture))?, check_only, findings, Same::Close);
        } else {
            ui.push((shot, job(shot, &dir.join(&document), &modules, &dir.join(&picture))?));
        }
    }
    if ui.is_empty() {
        return Ok(());
    }
    let job_file = out.join("ui.json");
    let entries: Vec<&Value> = ui.iter().map(|(_, entry)| entry).collect();
    std::fs::write(&job_file, json!({ "shots": entries }).to_string()).map_err(|e| format!("{}: {e}", util::rel(&job_file)))?;
    for error in window::shots(&fetched.root.join("bin/vectorcraft"), &job_file, &out.join("window"))? {
        findings.error(format!("VectorCraft's window: {error}"));
    }
    for (shot, entry) in ui {
        let taken = entry.get("picture").and_then(Value::as_str).map(Path::new).filter(|picture| picture.is_file());
        if let Some(picture) = taken {
            keep(root, &shot.id, &util::read_bytes(picture)?, check_only, findings, Same::Close);
        }
    }
    Ok(())
}

/// The window probe's job for the `vectorcraft-ui` shot `shot` (`compat/vc-window/tests/shots.rs`): the
/// document, the modules' files, the menu path, and for a dialog the window's size, the family's row, and
/// its parameters with the control each shows, which its manifest gives; and where the picture goes.
fn job(shot: &Shot, document: &Path, modules: &Path, picture: &Path) -> Result<Value, String> {
    let plugins: Vec<String> = shot.plugins.iter().map(|module| modules.join(format!("{module}.wasm")).to_string_lossy().into_owned()).collect();
    let (window, dialog) = match shot.dialog.as_deref() {
        None => (Value::Null, Value::Null),
        Some(name) => {
            let family = Family::ALL.iter().copied().find(|f| f.id() == name).ok_or(format!("no stitch family `{name}`"))?;
            let manifest = stitchcraft_vc_plugin::manifest::family(family);
            let params: Vec<Value> = manifest.params.iter().map(|(key, schema)| json!([key, control(schema)])).collect();
            (json!(window::DIALOG_WINDOW), json!({ "row": manifest.name, "params": params }))
        }
    };
    Ok(json!({
        "id": shot.id,
        "document": document.to_string_lossy(),
        "plugins": plugins,
        "menu": shot.menu,
        "window": window,
        "dialog": dialog,
        "picture": picture.to_string_lossy(),
    }))
}

/// The role in the accessibility tree of the control VectorCraft's dialog gives a parameter of the type
/// `schema` (its `form::schema_fields`): a text field for a number, a checkbox for a toggle, and a drop-down
/// list for a choice.
fn control(schema: &Schema) -> &'static str {
    match schema {
        Schema::Number { .. } | Schema::Int { .. } => "text_field",
        Schema::Bool { .. } => "check_box",
        Schema::Choice { .. } => "combo_box",
    }
}

#[cfg(test)]
mod tests;
