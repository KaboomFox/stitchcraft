//! The window probe's checks (spike M0.8): the same pixels from `ui.screenshot`, and xa11y driving the
//! window. `src/lib.rs` says why; `cargo xtask compat window` runs them, one at a time.
// They time a screenshot and keep the tree's dump: the harness's own exemption (`src/lib.rs`).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::time::{Duration, Instant};

use serde_json::Value;
use stitchcraft_vc_window::{Pixels, Window, fixture, out, record};
use xa11y::{App, AppExt, Element};

/// The drawing the window opens: the SVG tutorial's badge, a red star inside a blue ring.
const BADGE: &str = "badge.svg";

/// How long xa11y waits for the app to appear in the accessibility tree, and for a control to show.
const FIND: Duration = Duration::from_secs(30);

/// Runs `check` as `test`, and records why it failed before failing.
fn checked(test: &str, check: impl FnOnce() -> Result<(), String>) {
    if let Err(why) = check() {
        // The record is how CI shows the failure; the panic below fails the test either way.
        let _ = record(test, "error", test, &why);
        panic!("{why}");
    }
}

/// How `a` and `b` compare above `bar`, the status bar's first row, and below it: the pixels that differ
/// above (an error unless there are none) and below.
fn compare(what: &str, a: &Pixels, b: &Pixels, bar: u32) -> Result<(usize, usize), String> {
    let size = || format!("{what}: {}×{} pixels, then {}×{}", a.width, a.height, b.width, b.height);
    let above = a.rows(0, bar).differing(&b.rows(0, bar), 0).ok_or_else(size)?;
    let below = a.rows(bar, a.height).differing(&b.rows(bar, b.height), 0).ok_or_else(size)?;
    Ok((above, below))
}

/// What `compare` found, as a sentence.
fn described(above: usize, below: usize, bar: u32) -> String {
    format!("{above} pixels differ above the status bar (rows 0 to {bar}), and {below} in it")
}

#[test]
fn the_window_screenshots_the_same_pixels_twice() {
    let test = "the same pixels twice";
    checked(test, || {
        let mut window = Window::open(&fixture(BADGE)?, "twice")?;
        let adapter = window.inspect()?.get("graphicsAdapter").cloned().unwrap_or(Value::Null);
        let started = Instant::now();
        let first = window.screenshot("twice-1")?;
        let took = started.elapsed();
        let second = window.screenshot("twice-2")?;
        let bar = window.below_canvas(&first)?;
        let (above, below) = compare("two screenshots of one window", &first, &second, bar)?;
        let measured = format!(
            "{}×{} pixels drawn by {adapter}; a screenshot took {took:.1?}; {}; above the status bar, fingerprint {:016x}",
            first.width,
            first.height,
            described(above, below, bar),
            first.rows(0, bar).fingerprint()
        );
        record(test, "notice", "window", &measured)?;
        if above == 0 { Ok(()) } else { Err(format!("two screenshots of one window: {}", described(above, below, bar))) }
    });
}

#[test]
fn a_new_start_screenshots_the_same_pixels() {
    let test = "the same pixels after a new start";
    checked(test, || {
        let (first, bar) = {
            // The window quits at the end of this block.
            let mut window = Window::open(&fixture(BADGE)?, "start-1")?;
            let shot = window.screenshot("start-1")?;
            let bar = window.below_canvas(&shot)?;
            (shot, bar)
        };
        let second = Window::open(&fixture(BADGE)?, "start-2")?.screenshot("start-2")?;
        let (above, below) = compare("screenshots of two starts", &first, &second, bar)?;
        record(test, "notice", "a new start", &described(above, below, bar))?;
        if above == 0 { Ok(()) } else { Err(format!("screenshots of two starts: {}", described(above, below, bar))) }
    });
}

#[test]
fn xa11y_chooses_a_menu_item_in_the_window() {
    let test = "xa11y drives the window";
    checked(test, || {
        let mut window = Window::open(&fixture(BADGE)?, "xa11y")?;
        let outline = |inspect: &Value| inspect.pointer("/ui/view/outline").and_then(Value::as_bool);
        let before = window.inspect()?;
        let was = outline(&before).ok_or(format!("ui.inspect shows no /ui/view/outline: {before}"))?;
        let app = App::by_pid(window.pid(), FIND).map_err(|e| format!("xa11y did not find the app's process in the accessibility tree: {e}"))?;
        // A screen reader's way: AT-SPI's action on View, then on its row Outline.
        let view = title(&app, "View")?;
        let lines = keep(&app, "tree")?;
        view.press().map_err(|e| format!("pressing View: {e}"))?;
        let outlines = row(&app, "Outline")?;
        let rows = keep(&app, "tree-view-menu")?;
        outlines.press().map_err(|e| format!("pressing {:?}: {e}", outlines.name))?;
        window.until("the view to show outlines", |inspect| outline(inspect) == Some(!was))?;
        window.screenshot("xa11y-outline")?;
        // A person's way: clicks where the tree places View, then its row Preview, which shows the drawing
        // again.
        let input = xa11y::input_sim().map_err(|e| format!("xa11y's input: {e}"))?;
        let view = title(&app, "View")?;
        input.mouse().click(&view).map_err(|e| format!("clicking View at {:?}: {e}", view.bounds))?;
        let preview = row(&app, "Preview")?;
        input.mouse().click(&preview).map_err(|e| format!("clicking {:?} at {:?}: {e}", preview.name, preview.bounds))?;
        window.until("the view to show the drawing again", |inspect| outline(inspect) == Some(was))?;
        let place = app.windows().ok().and_then(|windows| windows.first().and_then(|w| w.bounds));
        let found = format!(
            "{lines} lines, {rows} with View open. AT-SPI's press chose View, then {:?}; clicks chose View at {:?}, then {:?}, in a window at {place:?}",
            outlines.name, view.bounds, preview.name
        );
        record(test, "notice", "accessibility tree", &found)
    });
}

/// The menu bar's title `name`, once the tree shows it.
fn title(app: &App, name: &str) -> Result<Element, String> {
    app.locator(&format!(r#"button[name="{name}"]"#)).wait_attached(FIND).map_err(|e| format!("the menu {name}: {e}"))
}

/// The open menu's row whose label, past the room for a check mark, starts with `label`, once the tree
/// shows it. Its name ends with its shortcut (`     Outline Ctrl+Y`).
fn row(app: &App, label: &str) -> Result<Element, String> {
    let started = Instant::now();
    loop {
        let buttons = app.locator("button").elements().map_err(|e| format!("the menu's rows: {e}"))?;
        let starts = |name: &str| name.trim_start_matches(['✓', ' ']).starts_with(label);
        if let Some(found) = buttons.into_iter().find(|button| button.name.as_deref().is_some_and(starts)) {
            return Ok(found);
        }
        if started.elapsed() > FIND {
            return Err(format!("no row {label} showed in the open menu within {FIND:?}"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Keeps the app's tree, as xa11y sees it, in `<name>.txt`: its number of lines.
fn keep(app: &App, name: &str) -> Result<usize, String> {
    let tree = app.dump(None).map_err(|e| format!("the tree: {e}"))?;
    let path = out()?.join(format!("{name}.txt"));
    std::fs::write(&path, &tree).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(tree.lines().count())
}
