//! The docs' `vectorcraft-ui` pictures (roadmap step M6.7): VectorCraft's menus and dialogs with
//! StitchCraft's plug-ins in them, reached through the accessibility tree as a person reaches them, and
//! cropped. A plug-in's dialog is checked against its module's manifest on the way.
//!
//! `cargo xtask shots --vectorcraft` writes the job, a JSON file that [`SHOTS`] names, and runs this test in
//! the window probe's private desktop (`xtask/src/window.rs`). Without the variable the test does nothing,
//! so `cargo xtask compat window`, which runs every test of the crate, passes it by. For each shot:
//!
//! 1. The window opens the document `vectorcraft-cli` drew for the shot. The control channel installs the
//!    modules and selects everything, which a person does with file dialogs and clicks on the drawing. For
//!    a dialog it makes the window the size the job gives: a plug-in's dialog doesn't scroll, and fill's
//!    and satin's are taller than the window at the size it opens.
//! 2. xa11y presses the menu bar's title and each row of the shot's menu path, as AT-SPI's press does for a
//!    screen reader, then with a dialog the row of the family's effect, which the module's name begins.
//! 3. The screenshot is cropped to the title and the rows that opened, with room for the menus' frames, or
//!    to the dialog's window, which must fit in it, and written where the job says.
//! 4. A dialog must show a row for each parameter of the module's manifest that shows at the defaults: its
//!    label with a colon after it, and then the control the job names for the parameter's type. From 0.14
//!    VectorCraft scrolls the rows of a dialog taller than its window, and then the picture would show only
//!    some: a dialog with a scroll bar is an error, the window too short for it. The job
//!    gives the label the manifest does (`REQ-VC-012`); a VectorCraft before 0.14 writes one from the key
//!    instead, first letter capital (its `form::humanize`).
//!
//! Each shot records what it found, a dialog's tree among it, or why it failed ([`record`]).
// The harness's own exemption (`src/lib.rs`): it waits on the app and reads and writes files.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::BTreeSet;
use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use stitchcraft_vc_window::{SHOTS, Window, dialog_controls, record};
use xa11y::{App, AppExt, Element};

/// The role in the tree, in snake case, of the scroll bar of rows that don't fit.
const SCROLL_BAR: &str = "scroll_bar";
/// How long xa11y waits for the app in the tree, and for a control to show.
const FIND: Duration = Duration::from_secs(30);
/// How often a wait looks again.
const POLL: Duration = Duration::from_millis(100);
/// Room around the menus' rows for their frames and shadows, in pixels.
const FRAME: i64 = 10;
/// How deep a dialog's tree is walked and kept.
const DEPTH: usize = 12;

#[test]
fn vectorcraft_ui_shots() {
    let Some(job) = std::env::var_os(SHOTS) else { return };
    let shots = std::fs::read_to_string(&job)
        .map_err(|e| e.to_string())
        .and_then(|text| serde_json::from_str::<Value>(&text).map_err(|e| e.to_string()))
        .and_then(|job| job.get("shots").and_then(Value::as_array).cloned().filter(|shots| !shots.is_empty()).ok_or("it lists no shots".to_string()));
    let shots = match shots {
        Ok(shots) => shots,
        Err(why) => {
            let _ = record("shots", "error", "the job", &format!("{}: {why}", Path::new(&job).display()));
            panic!("{why}");
        }
    };
    let mut failed = Vec::new();
    for shot in &shots {
        let id = shot.get("id").and_then(Value::as_str).unwrap_or("?");
        if let Err(why) = take(shot) {
            let _ = record(id, "error", id, &why);
            failed.push(id.to_string());
        }
    }
    assert!(failed.is_empty(), "shots that failed: {failed:?}");
}

/// Takes `shot`, one entry of the job.
fn take(shot: &Value) -> Result<(), String> {
    let text = |key: &str| shot.get(key).and_then(Value::as_str).ok_or(format!("the job's shot has no `{key}`"));
    let id = text("id")?;
    let mut window = Window::open(Path::new(text("document")?), id)?;
    for plugin in shot.get("plugins").and_then(Value::as_array).into_iter().flatten() {
        window.call("engine.execute", json!({"command": "plugin.install", "params": {"path": plugin}}))?;
    }
    window.call("engine.execute", json!({"command": "select.all", "params": {}}))?;
    let opened_at = size(&window.inspect()?)?;
    let at = match shot.get("window").filter(|wanted| !wanted.is_null()) {
        None => opened_at,
        Some(wanted) => {
            let wanted = wanted.as_array().and_then(|wh| Some((wh.first()?.as_f64()?, wh.get(1)?.as_f64()?)));
            let (width, height) = wanted.ok_or("the job's window is not [width, height]")?;
            window.call("ui.resize", json!({"width": width, "height": height}))?;
            let what = format!("the window at {width}×{height}");
            let resized = |inspect: &Value| size(inspect).is_ok_and(|(w, h)| (w - width).abs() < 0.5 && (h - height).abs() < 0.5);
            size(&window.until(&what, resized)?)?
        }
    };
    let app = App::by_pid(window.pid(), FIND).map_err(|e| format!("xa11y did not find the app's process in the accessibility tree: {e}"))?;
    let menu: Vec<&str> = shot.get("menu").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_str).collect();
    let (title, rows) = menu.split_first().ok_or("the shot's menu path is empty")?;
    let before = buttons(&app)?.iter().map(identity).collect::<BTreeSet<_>>();
    let title = app.locator(&format!(r#"button[name="{title}"]"#)).wait_attached(FIND).map_err(|e| format!("the menu {title}: {e}"))?;
    title.press().map_err(|e| format!("pressing {title:?}: {e}"))?;
    for label in rows {
        let row = row(&app, label)?;
        row.press().map_err(|e| format!("pressing {:?}: {e}", row.name))?;
    }
    let dialog = shot.get("dialog").filter(|dialog| !dialog.is_null());
    let (crop, mut found) = match dialog {
        Some(dialog) => opened(&app, dialog)?,
        None => menus(&app, &title, &before)?,
    };
    let picture = window.screenshot_still(id)?;
    let pixels = (i64::from(picture.width), i64::from(picture.height));
    if dialog.is_some() {
        if crop.0 < 0 || crop.1 < 0 || crop.0 + crop.2 > pixels.0 || crop.1 + crop.3 > pixels.1 {
            let (x, y, w, h) = crop;
            return Err(format!("the dialog, {w}×{h} pixels at ({x}, {y}), runs off the {}×{} window{found}", pixels.0, pixels.1));
        }
        // The window's size is in points, and the screenshot's in pixels.
        let scale = pixels.0 as f64 / at.0;
        let tall = (opened_at.1 * scale).round() as i64;
        if crop.3 > tall {
            found = format!(
                "; {} pixels tall, more than the window's {tall} at the size it opens, {}×{} points, where it runs off its top and bottom{found}",
                crop.3, opened_at.0, opened_at.1
            );
        }
    }
    let cropped = picture.crop(crop.0, crop.1, crop.2, crop.3);
    cropped.write(Path::new(text("picture")?))?;
    let said = format!("{}×{} pixels at ({}, {}) of the {}×{} window{found}", cropped.width, cropped.height, crop.0, crop.1, pixels.0, pixels.1);
    record(id, "notice", id, &said)
}

/// The window's width and height in points, as `ui.inspect` shows them.
fn size(inspect: &Value) -> Result<(f64, f64), String> {
    let number = |at: &str| inspect.pointer(at).and_then(Value::as_f64).ok_or(format!("ui.inspect shows no {at}: {inspect}"));
    Ok((number("/window/0")?, number("/window/1")?))
}

/// A rectangle in the screenshot's pixels: `x`, `y`, width and height.
type Crop = (i64, i64, i64, i64);

/// The rectangle of `element` in the tree, which is the screenshot's pixels: the window has no window
/// manager to move it from the screen's corner.
fn bounds(element: &Element) -> Result<Crop, String> {
    let b = element.bounds.ok_or(format!("{element} has no bounds in the tree"))?;
    Ok((i64::from(b.x), i64::from(b.y), i64::from(b.width), i64::from(b.height)))
}

/// The smallest rectangle around `a` and `b`.
fn union(a: Crop, b: Crop) -> Crop {
    let (left, top) = (a.0.min(b.0), a.1.min(b.1));
    let (right, bottom) = ((a.0 + a.2).max(b.0 + b.2), (a.1 + a.3).max(b.1 + b.3));
    (left, top, right - left, bottom - top)
}

/// What tells `element` from the others across two looks at the tree: its D-Bus object path, or else its
/// name and place.
fn identity(element: &Element) -> String {
    element.stable_id.clone().unwrap_or_else(|| format!("{:?} at {:?}", element.name, element.bounds))
}

/// Every button in the app's tree: the menu bar's titles and the rows of the menus open.
fn buttons(app: &App) -> Result<Vec<Element>, String> {
    app.locator("button").elements().map_err(|e| format!("the buttons: {e}"))
}

/// The open menus' rectangle: the menu bar's `title` and the rows that were not in the tree `before` it
/// was pressed, once they stop changing, with room for the menus' frames; and the rows' names, said.
fn menus(app: &App, title: &Element, before: &BTreeSet<String>) -> Result<(Crop, String), String> {
    let started = Instant::now();
    let mut last = Vec::new();
    loop {
        let rows: Vec<Element> = buttons(app)?.into_iter().filter(|b| !before.contains(&identity(b))).collect();
        let now: Vec<Crop> = rows.iter().filter_map(|row| bounds(row).ok()).collect();
        if !now.is_empty() && now == last {
            let around = now.into_iter().fold(bounds(title)?, union);
            let names: Vec<&str> = rows.iter().filter_map(|row| row.name.as_deref()).collect();
            let crop = (around.0 - FRAME, around.1 - FRAME, around.2 + 2 * FRAME, around.3 + 2 * FRAME);
            return Ok((crop, format!("; the rows that opened: {names:?}")));
        }
        if started.elapsed() > FIND {
            return Err(format!("the open menus' rows did not settle within {FIND:?}: {} rows", now.len()));
        }
        last = now;
        std::thread::sleep(POLL * 3);
    }
}

/// Presses the row of the effect `dialog` names, and waits for its dialog: the dialog's rectangle, and
/// what it shows, said after checking its rows against the module's parameters.
fn opened(app: &App, dialog: &Value) -> Result<(Crop, String), String> {
    let name = dialog.get("row").and_then(Value::as_str).ok_or("the shot's dialog names no row")?;
    let row = row(app, name)?;
    row.press().map_err(|e| format!("pressing {:?}: {e}", row.name))?;
    // The window's node and its heading both carry the dialog's name: the window is the larger.
    let started = Instant::now();
    let window = loop {
        let named = app.locator(&format!(r#"[name="{name}"]"#)).elements().map_err(|e| format!("{name}: {e}"))?;
        let largest = named
            .into_iter()
            .filter(|e| e.bounds.is_some_and(|b| b.height > 100))
            .max_by_key(|e| e.bounds.map_or(0, |b| u64::from(b.width) * u64::from(b.height)));
        if let Some(window) = largest {
            break window;
        }
        if started.elapsed() > FIND {
            return Err(format!("no dialog named {name:?} showed within {FIND:?}"));
        }
        std::thread::sleep(POLL);
    };
    let crop = bounds(&window)?;
    let tree = window.dump(Some(DEPTH)).map_err(|e| format!("the dialog's tree: {e}"))?;
    let nodes: Vec<(&'static str, Option<String>)> =
        in_order(&window, DEPTH)?.iter().map(|node| (node.role.to_snake_case(), node.name.clone())).collect();
    let rows = dialog_controls(&nodes);
    let params: Vec<(&str, &str)> = dialog
        .get("params")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p.get(0)?.as_str()?, p.get(1)?.as_str()?)))
        .collect();
    let wrong: Vec<String> = params
        .iter()
        .filter_map(|(label, control)| match rows.get(&label.to_lowercase()) {
            None => Some(format!("no row {label}")),
            Some(shown) if shown != control => Some(format!("a {shown} for {label}, not a {control}")),
            Some(_) => None,
        })
        .collect();
    let found = format!(
        "; {} of the {} rows it shows at the defaults found, each with its label and the control of its type. The dialog's tree, {} lines:\n{}",
        params.len() - wrong.len(),
        params.len(),
        tree.lines().count(),
        tree
    );
    if !wrong.is_empty() {
        return Err(format!("the dialog {name:?} shows {wrong:?}{found}"));
    }
    // From 0.14 VectorCraft scrolls the rows of a dialog taller than the window, and the picture would show
    // only those in view: the window must be tall enough for all of them.
    if nodes.iter().any(|(role, _)| *role == SCROLL_BAR) {
        return Err(format!("the dialog {name:?} scrolls its rows, as the window is too short for it{found}"));
    }
    Ok((crop, found))
}

/// `element` and those under it, `depth` levels down, in the tree's order: each before its children.
fn in_order(element: &Element, depth: usize) -> Result<Vec<Element>, String> {
    let mut found = vec![element.clone()];
    if depth > 0 {
        for child in element.children().map_err(|e| format!("the children of {element}: {e}"))? {
            found.extend(in_order(&child, depth - 1)?);
        }
    }
    Ok(found)
}

/// The open menu's row whose label, past the room for a check mark, starts with `label`, once the tree
/// shows it. Its name ends with its shortcut, as `tests/window.rs` found (`     Outline Ctrl+Y`).
fn row(app: &App, label: &str) -> Result<Element, String> {
    let started = Instant::now();
    loop {
        let starts = |name: &str| name.trim_start_matches(['✓', ' ']).starts_with(label);
        if let Some(found) = buttons(app)?.into_iter().find(|button| button.name.as_deref().is_some_and(starts)) {
            return Ok(found);
        }
        if started.elapsed() > FIND {
            return Err(format!("no row {label} showed in the open menu within {FIND:?}"));
        }
        std::thread::sleep(POLL);
    }
}
