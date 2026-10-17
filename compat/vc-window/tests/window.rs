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
/// How long a way of choosing a control has to show its effect before the next way is tried.
const TRY: Duration = Duration::from_secs(5);

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
        let lines = keep(&app, "tree")?;
        // A person opens View and chooses Outline, whose label may carry a check mark's room and a shortcut.
        let view = app.locator(r#"button[name="View"]"#).wait_attached(FIND).map_err(|e| format!("View: {e}"))?;
        let item = app.locator(r#"button[name*="Outline"]"#);
        let opened = choose(&view, || item.wait_attached(TRY).is_ok())?;
        let menu = keep(&app, "tree-view-menu")?;
        let chosen = item.element().map_err(|e| format!("View's Outline: {e}"))?;
        let label = format!("{:?}", chosen.name);
        let switched = choose(&chosen, || window.shows(TRY, |inspect| outline(inspect) == Some(!was)).unwrap_or(false))?;
        window.screenshot("xa11y-outline")?;
        record(
            test,
            "notice",
            "accessibility tree",
            &format!("{lines} lines, {menu} with View open; View opened by {opened}; its item {label} chosen by {switched}"),
        )
    });
}

/// Chooses `control` as a screen reader does, through AT-SPI's action, and when `done` doesn't see the
/// effect, as a person does, with a click where the tree places it: which of the two worked.
fn choose(control: &Element, mut done: impl FnMut() -> bool) -> Result<&'static str, String> {
    let name = format!("{:?}", control.name);
    control.press().map_err(|e| format!("pressing {name}: {e}"))?;
    if done() {
        return Ok("AT-SPI's press");
    }
    xa11y::input_sim().and_then(|input| input.mouse().click(control)).map_err(|e| format!("clicking {name}: {e}"))?;
    if done() {
        return Ok("a click where the tree places it");
    }
    Err(format!("neither AT-SPI's press nor a click where the tree places it chose {name} ({:?})", control.bounds))
}

/// Keeps the app's tree, as xa11y sees it, in `<name>.txt`: its number of lines.
fn keep(app: &App, name: &str) -> Result<usize, String> {
    let tree = app.dump(None).map_err(|e| format!("the tree: {e}"))?;
    let path = out()?.join(format!("{name}.txt"));
    std::fs::write(&path, &tree).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(tree.lines().count())
}
