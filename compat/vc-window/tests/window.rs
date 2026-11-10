//! The window probe's checks (spike M0.8): the same pixels from `ui.screenshot`, and xa11y driving the
//! window. `src/lib.rs` says why; `cargo xtask compat window` runs them, one at a time.
// They time a screenshot and keep the tree's dump: the harness's own exemption (`src/lib.rs`).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::time::{Duration, Instant};

use serde_json::Value;
use stitchcraft_vc_window::{Difference, Pixels, Window, changes, fixture, out, record};
use xa11y::{App, AppExt, Element};

/// The drawing the window opens: the SVG tutorial's badge, a red star inside a blue ring.
const BADGE: &str = "badge.svg";

/// How long xa11y waits for the app to appear in the accessibility tree, and for a control to show.
const FIND: Duration = Duration::from_secs(30);

/// Where `ui.inspect` says whether the view shows outlines, the newest VectorCraft's place first.
const OUTLINE: [&str; 2] = ["/view/outline", "/ui/view/outline"];

/// Runs `check` as `test`, and records why it failed before failing.
fn checked(test: &str, check: impl FnOnce() -> Result<(), String>) {
    if let Err(why) = check() {
        // The record is how CI shows the failure; the panic below fails the test either way.
        let _ = record(test, "error", test, &why);
        panic!("{why}");
    }
}

/// How `a` and `b` compare above `bar`, the status bar's first row, and below it: the pixels that differ
/// above (an error unless there are none) and how many differ below.
fn compare(what: &str, a: &Pixels, b: &Pixels, bar: u32) -> Result<(Difference, usize), String> {
    let size = || format!("{what}: {}×{} pixels, then {}×{}", a.width, a.height, b.width, b.height);
    let above = a.rows(0, bar).difference(&b.rows(0, bar), 0).ok_or_else(size)?;
    let below = a.rows(bar, a.height).differing(&b.rows(bar, b.height), 0).ok_or_else(size)?;
    Ok((above, below))
}

/// What `compare` found, as a sentence: above the status bar, the box round the pixels that differ, as
/// the column and row of its top left and bottom right pixels, says what changed.
fn described(above: Difference, below: usize, bar: u32) -> String {
    let place = above.bounds.map_or_else(String::new, |[left, top, right, bottom]| format!(", from ({left}, {top}) to ({right}, {bottom})"));
    format!("{} pixels differ above the status bar (rows 0 to {bar}){place}, and {below} in it", above.pixels)
}

/// A start of the app with the badge open, recorded as `name`: its screenshot, what `ui.inspect` shows
/// then, the status bar's first row, and how long the start took until the screenshot. From 0.23
/// VectorCraft's opening fit follows the canvas while the window settles, so each start draws the badge at
/// one zoom. An earlier release opens it at one of 2 zooms a hair apart, for which the probe fitted the
/// artboard again (`docs/src/design/docs-pipeline.md`). The app quits on the way out.
fn start(name: &str) -> Result<(Pixels, Value, u32, Duration), String> {
    let started = Instant::now();
    let mut window = Window::open(&fixture(BADGE)?, name)?;
    let shot = window.screenshot(name)?;
    let took = started.elapsed();
    let bar = window.below_canvas(&shot)?;
    Ok((shot, window.inspect()?, bar, took))
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
        if above.pixels == 0 { Ok(()) } else { Err(format!("two screenshots of one window: {}", described(above, below, bar))) }
    });
}

/// In 3 of the 97 runs from 1 to 3 November 2026, and in 1 on 8 November, the screenshots of the two
/// starts differed in 19,159 pixels above the status bar, the same number each time. The run on 8 November
/// named the reason: the 2 starts had opened the badge at zooms of 4.9812 and 4.9883, their canvas and the
/// view's middle alike, so VectorCraft had fitted the view to a canvas that was still settling.
/// [`start`] now fits the view again once the canvas has settled. The notice gives each start's time to
/// its screenshot and the fingerprint of its pixels above the status bar, which `the same pixels twice`
/// gives for a window of its own, and a failure names what `ui.inspect` shows differently, the frame
/// timings left out.
#[test]
fn a_new_start_screenshots_the_same_pixels() {
    let test = "the same pixels after a new start";
    checked(test, || {
        let (first, seen, bar, took) = start("start-1")?;
        let (second, then, _, took_again) = start("start-2")?;
        let (above, below) = compare("screenshots of two starts", &first, &second, bar)?;
        let fingerprint = |shot: &Pixels| format!("{:016x}", shot.rows(0, bar).fingerprint());
        let measured = format!(
            "{}. The first start took {took:.1?} to its screenshot, which has the fingerprint {} above the status bar, and the second took {took_again:.1?}, with {}",
            described(above, below, bar),
            fingerprint(&first),
            fingerprint(&second)
        );
        record(test, "notice", "a new start", &measured)?;
        if above.pixels == 0 {
            return Ok(());
        }
        let changed = changes(&seen, &then, &["/perf"]);
        let inspect =
            if changed.is_empty() { "ui.inspect shows the same".to_string() } else { format!("ui.inspect differs at {}", changed.join("; ")) };
        Err(format!("screenshots of two starts: {}. {inspect}", described(above, below, bar)))
    });
}

#[test]
fn xa11y_chooses_a_menu_item_in_the_window() {
    let test = "xa11y drives the window";
    checked(test, || {
        let mut window = Window::open(&fixture(BADGE)?, "xa11y")?;
        // VectorCraft 0.21 keeps whether the view shows outlines with the document's view, at `/view/outline`,
        // where 0.20 kept it with the window's settings, at `/ui/view/outline`.
        let outline = |inspect: &Value| OUTLINE.iter().find_map(|at| inspect.pointer(at).and_then(Value::as_bool));
        let before = window.inspect()?;
        let was = outline(&before).ok_or(format!("ui.inspect shows neither {}: {before}", OUTLINE.join(" nor ")))?;
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
