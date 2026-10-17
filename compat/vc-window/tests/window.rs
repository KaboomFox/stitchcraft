//! The window probe's checks (spike M0.8): the same pixels from `ui.screenshot`, and xa11y driving the
//! window. `src/lib.rs` says why; `cargo xtask compat window` runs them, one at a time.
// They time a screenshot and keep the tree's dump: the harness's own exemption (`src/lib.rs`).
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::time::Duration;

use serde_json::Value;
use stitchcraft_vc_window::{Pixels, Window, fixture, out, record};
use xa11y::{App, AppExt};

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

/// `a` and `b` alike to the pixel, or how they differ.
fn alike(what: &str, a: &Pixels, b: &Pixels) -> Result<(), String> {
    match a.differing(b, 0) {
        Some(0) => Ok(()),
        Some(n) => {
            // The docs' tolerance for `vectorcraft-ui` pictures counts channels more than 8/255 apart.
            let beyond = a.differing(b, 8).unwrap_or(n);
            Err(format!("{what}: {n} of {} pixels differ, {beyond} of them by more than 8/255", a.count()))
        }
        None => Err(format!("{what}: {}×{} pixels, then {}×{}", a.width, a.height, b.width, b.height)),
    }
}

#[test]
fn the_window_screenshots_the_same_pixels_twice() {
    let test = "the same pixels twice";
    checked(test, || {
        let mut window = Window::open(&fixture(BADGE)?, "twice")?;
        let adapter = window.inspect()?.get("graphicsAdapter").cloned().unwrap_or(Value::Null);
        let started = std::time::Instant::now();
        let first = window.screenshot("twice-1")?;
        let took = started.elapsed();
        let second = window.screenshot("twice-2")?;
        let measured = format!(
            "{}×{} pixels drawn by {adapter}; a screenshot took {took:.1?}; fingerprint {:016x}",
            first.width,
            first.height,
            first.fingerprint()
        );
        record(test, "notice", "window", &measured)?;
        alike("two screenshots of one window", &first, &second)
    });
}

#[test]
fn a_new_start_screenshots_the_same_pixels() {
    let test = "the same pixels after a new start";
    checked(test, || {
        // Each window quits when it is dropped, at the end of its statement.
        let first = Window::open(&fixture(BADGE)?, "start-1")?.screenshot("start-1")?;
        let second = Window::open(&fixture(BADGE)?, "start-2")?.screenshot("start-2")?;
        alike("screenshots of two starts", &first, &second)
    });
}

#[test]
fn xa11y_presses_a_menu_item_in_the_window() {
    let test = "xa11y drives the window";
    checked(test, || {
        let mut window = Window::open(&fixture(BADGE)?, "xa11y")?;
        let outline = |inspect: &Value| inspect.pointer("/ui/view/outline").and_then(Value::as_bool);
        let before = window.inspect()?;
        let was = outline(&before).ok_or(format!("ui.inspect shows no /ui/view/outline: {before}"))?;
        let app = App::by_pid(window.pid(), FIND).map_err(|e| format!("xa11y did not find the app's process in the accessibility tree: {e}"))?;
        // The tree as xa11y sees it, kept with the screenshots: the selectors below are written against it.
        let tree = app.dump(None).map_err(|e| format!("the tree: {e}"))?;
        let dump = out()?.join("tree.txt");
        std::fs::write(&dump, &tree).map_err(|e| format!("{}: {e}", dump.display()))?;
        let head: Vec<&str> = tree.lines().take(60).collect();
        record(test, "notice", "accessibility tree", &format!("{} lines; the first:\n{}", tree.lines().count(), head.join("\n")))?;
        // A person opens View and chooses Outline (its label has a check mark's room before it).
        app.locator(r#"button[name="View"], menu_item[name="View"]"#).press().map_err(|e| format!("pressing View: {e}"))?;
        let item = app.locator(r#"button[name$="Outline"], menu_item[name$="Outline"], check_box[name$="Outline"]"#);
        item.wait_attached(FIND).map_err(|e| format!("View's Outline did not show: {e}"))?;
        item.press().map_err(|e| format!("pressing Outline: {e}"))?;
        // The control channel sees the view switch to outlines.
        window.until("the view to switch to outlines", |inspect| outline(inspect) == Some(!was))?;
        window.screenshot("xa11y-outline")?;
        Ok(())
    });
}
