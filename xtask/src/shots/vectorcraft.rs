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
//!   row for each parameter of the module's manifest that shows at the defaults, which the plug-in crate
//!   makes from the parameter registry and gives here (`dialog::shown_at_defaults`): the parameter's
//!   label, and the control VectorCraft gives its type.
//!
//! **Why a tolerance.** Mesa draws VectorCraft's window in software, and a newer Mesa from Ubuntu's updates
//! may move an anti-aliased edge. A picture stays while no more than [`PER_THOUSAND`] in a thousand of its
//! pixels differ from the committed one by more than [`TOLERANCE`] in a channel, 0.2 %, and no more than
//! [`PER_TILE`] in any square of [`TILE`] pixels a side. A moved edge is a few pixels in many places, and a
//! change of content, such as a default value in a dialog's field, a few dozen in one: the pictures change
//! when VectorCraft or a dialog's contents do.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::{Value, json};
use stitchcraft_vc_plugin::manifest::Schema;
use stitchcraft_vc_plugin::{Module, dialog};

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
/// The side, in pixels, of the squares a picture is cut into to find a change in one place.
const TILE: usize = 16;
/// How many of a [`TILE`]'s pixels may differ by more than [`TOLERANCE`].
const PER_TILE: u32 = 8;
/// Where the shots' documents and pictures, and the window's records, are made.
const OUT: &str = "target/shots-vectorcraft";
/// The drawing that ends a `vectorcraft-ui` shot's run, in its folder: a render shot's is its picture.
const CANVAS: &str = "canvas.png";

/// Whether the pictures `a` and `b`, two PNG files, are the same within the tolerance: of one size, with no
/// more than [`PER_THOUSAND`] in a thousand of their pixels further apart than [`TOLERANCE`] in a channel,
/// and no more than [`PER_TILE`] of them in any [`TILE`] square.
pub(super) fn close(a: &[u8], b: &[u8]) -> bool {
    let (Ok(a), Ok(b)) = (tiny_skia::Pixmap::decode_png(a), tiny_skia::Pixmap::decode_png(b)) else { return false };
    if (a.width(), a.height()) != (b.width(), b.height()) {
        return false;
    }
    let width = a.width() as usize;
    let across = width.div_ceil(TILE);
    let mut in_tile = vec![0_u32; across * (a.height() as usize).div_ceil(TILE)];
    let mut apart: u64 = 0;
    let pixels = a.data().as_chunks::<4>().0.iter().zip(b.data().as_chunks::<4>().0.iter());
    for (i, _) in pixels.enumerate().filter(|(_, (p, q))| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > TOLERANCE)) {
        apart += 1;
        if let Some(count) = in_tile.get_mut(i / width / TILE * across + i % width / TILE) {
            *count += 1;
        }
    }
    let pixels = u64::from(a.width()) * u64::from(a.height());
    apart.saturating_mul(1000) <= PER_THOUSAND.saturating_mul(pixels) && in_tile.iter().all(|count| *count <= PER_TILE)
}

/// Without `--vectorcraft`: `shot` is declared as a VectorCraft shot can be, and its picture, and its
/// document when it keeps one, are committed.
pub(super) fn committed(root: &Path, shot: &Shot, findings: &mut Findings) {
    for problem in problems(shot) {
        findings.error(format!("shot `{}`: {problem}", shot.id));
    }
    let picture = root.join(GENERATED).join(format!("{}.png", shot.id));
    for path in std::iter::once(picture).chain(shot.document.iter().map(|document| root.join(document))) {
        if !path.is_file() {
            findings.error(format!(
                "{} is missing: `cargo xtask shots --vectorcraft` makes it on Linux, or add the `docs:refresh-shots` label to the pull request",
                util::rel(&path)
            ));
        }
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
    match shot.document.as_deref() {
        Some(_) if shot.kind == UI => found.push("`document` is a vectorcraft-render shot's".to_string()),
        Some(document) if !document.ends_with(".vectorcraft") => found.push(format!("its document `{document}` is not a `.vectorcraft` file")),
        _ => {}
    }
    if shot.kind == UI {
        if shot.menu.is_empty() {
            found.push("a vectorcraft-ui shot presses a `menu` path".to_string());
        }
        if shot.scale.is_some() {
            found.push("`scale` is a vectorcraft-render shot's: the window's pictures are its pixels".to_string());
        }
        match shot.dialog.as_deref() {
            Some(name) if live_effect(name).is_none() => found.push(format!("`dialog = \"{name}\"` names no live effect")),
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
    if shots.iter().any(|shot| shot.kind == UI) {
        window::ready()?;
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
        let mut steps = shot.steps.clone();
        steps.push(Step::exporting(&document));
        steps.push(Step::exporting(if shot.kind == RENDER { &picture } else { CANVAS }));
        let run = Run { open: None, plugins: shot.plugins.clone(), steps, scale: shot.scale.map(f64::from) };
        if let Err(e) = scenario::draw(&run, &fetched.root.join("bin/vectorcraft-cli"), &modules, &dir) {
            findings.error(format!("shot `{}`: {e}", shot.id));
        } else if shot.kind == RENDER {
            keep(root, &shot.id, &util::read_bytes(&dir.join(&picture))?, check_only, findings, Same::Close);
            if let Some(kept) = &shot.document {
                keep_document(root, kept, &util::read_bytes(&dir.join(&document))?, check_only, findings);
            }
        } else {
            ui.push((*shot, job(shot, &dir.join(&document), &modules, &dir.join(&picture))?));
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
    kept(root, &ui, check_only, findings)
}

/// Writes `drawn`, the document VectorCraft drew, as the file `kept`, or with `check_only` says it is stale,
/// unless the committed file sews the same: VectorCraft may write the same drawing otherwise, as another
/// release does.
fn keep_document(root: &Path, kept: &str, drawn: &[u8], check_only: bool, findings: &mut Findings) {
    let sewn = match scenario::sewn(drawn) {
        Ok(sewn) => sewn,
        Err(e) => return findings.error(format!("{kept}: the document VectorCraft drew: {e}")),
    };
    let path = root.join(kept);
    let committed = std::fs::read(&path).ok();
    if committed.as_deref().is_some_and(|committed| scenario::sewn(committed).is_ok_and(|pes| pes == sewn)) {
        return;
    }
    if check_only {
        let what = if committed.is_some() { "sews otherwise than the document VectorCraft draws" } else { "is missing" };
        findings.error(format!("{kept} {what}: run `cargo xtask shots --vectorcraft`, or add the `docs:refresh-shots` label to the pull request"));
    } else {
        match std::fs::write(&path, drawn) {
            Ok(()) => println!("updated {kept}"),
            Err(e) => findings.error(format!("{kept}: {e}")),
        }
    }
}

/// Keeps the pictures VectorCraft's window took for the UI shots `ui`, each with its job entry.
fn kept(root: &Path, ui: &[(&Shot, Value)], check_only: bool, findings: &mut Findings) -> Result<(), String> {
    for (shot, entry) in ui {
        let taken = entry.get("picture").and_then(Value::as_str).map(Path::new).filter(|picture| picture.is_file());
        match taken {
            Some(picture) => keep(root, &shot.id, &util::read_bytes(picture)?, check_only, findings, Same::Close),
            None => findings.error(format!("shot `{}`: VectorCraft's window took no picture", shot.id)),
        }
    }
    Ok(())
}

/// The window probe's job for the `vectorcraft-ui` shot `shot` (`compat/vc-window/tests/shots.rs`): the
/// document, the modules' files, the menu path, and for a dialog the window's size, the family's row, and
/// the label and control of each row it shows at the defaults, which its manifest gives; and where the
/// picture goes.
fn job(shot: &Shot, document: &Path, modules: &Path, picture: &Path) -> Result<Value, String> {
    let plugins: Vec<String> = shot.plugins.iter().map(|module| modules.join(format!("{module}.wasm")).to_string_lossy().into_owned()).collect();
    let (window, dialog) = match shot.dialog.as_deref() {
        None => (Value::Null, Value::Null),
        Some(name) => {
            let manifest = live_effect(name).ok_or(format!("no live effect `{name}`"))?.manifest();
            let shown = dialog::shown_at_defaults(&manifest.params);
            let params: Vec<Value> = manifest
                .params
                .iter()
                .filter(|(key, _)| shown.contains(key))
                .map(|(key, param)| json!([param.ui.label.unwrap_or(key), control(&param.schema)]))
                .collect();
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

/// The module of the live effect `name` names, whose dialog a shot can show: a stitch family's, or the Auto
/// effect's.
fn live_effect(name: &str) -> Option<Module> {
    Module::all().into_iter().find(|module| module.name() == name && matches!(module, Module::Family(_) | Module::Auto))
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
