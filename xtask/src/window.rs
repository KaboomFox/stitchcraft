//! `cargo xtask compat window [--app <file>]`: the window probe (`compat/vc-window`; spike M0.8).
//!
//! **Why.** The docs' `vectorcraft-ui` pictures are taken from VectorCraft's own window
//! (`docs/src/design/docs-pipeline.md`), so CI must run that window with no screen, get the same pixels
//! each time and reach its controls through the accessibility tree. The probe's tests check the three;
//! this command gives them an app and a desktop:
//!
//! - The app is `bin/vectorcraft` from the Linux x86_64 archive of the stable release that
//!   `compat/vectorcraft.toml` pins, downloaded into `target/compat/` and checked against the SHA-256 the
//!   pins file records. `--app` names another build instead, such as one made from VectorCraft's `main`.
//! - The desktop is private: a D-Bus session of its own (`dbus-run-session`), whose accessibility bus is
//!   told that a screen reader is on, since AccessKit, VectorCraft's accessibility layer, shows the tree
//!   only then; and an X server of its own (`xvfb-run`), where Mesa's software renderer draws the window.
//!   Nothing reaches the desktop the command is started from: the screen-reader flag stays in memory
//!   (GSettings would keep it in the person's settings, and GNOME would start a screen reader at their
//!   next login), the buses and Mesa's caches live under `target/compat/window/`, and the window opens in
//!   the private X server even under Wayland.
//!
//! The tests record what they measured, and why they failed, in `target/compat/window/findings.jsonl`
//! (`compat/vc-window/src/lib.rs`); the command prints the records, as annotations in GitHub Actions.
//! Linux only: the desktop is X11's and AT-SPI's.
//!
//! The docs' `vectorcraft-ui` pictures are taken in the same desktop ([`shots`]): the probe's
//! `tests/shots.rs` drives the window to each menu and dialog that `cargo xtask shots --vectorcraft` asks
//! for (`xtask/src/shots/vectorcraft.rs`).

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::compat::{self, release};
use crate::util;

/// The probe's crate.
const PROBE: &str = "compat/vc-window";
/// The probe's names for what this tells it, as its `src/lib.rs` declares them: the variables that name
/// the app (its `APP`), the folder it records into (`OUT`) and the shots' job (`SHOTS`), and its file of
/// records in that folder (`FINDINGS`). Its workspace is its own, so a test checks the two agree.
const APP_VAR: &str = "STITCHCRAFT_VECTORCRAFT_APP";
const OUT_VAR: &str = "STITCHCRAFT_WINDOW_OUT";
const SHOTS_VAR: &str = "STITCHCRAFT_SHOTS";
const FINDINGS: &str = "findings.jsonl";
/// Where the tests record: screenshots, the tree, the app's logs and the findings.
const OUT: &str = "target/compat/window";
/// VectorCraft's window at the size it opens, which it keeps on a screen that also has room for a taskbar.
const DEFAULT_WINDOW: [u32; 2] = [1440, 900];
/// The X screen's width and height in pixels: room for the window at [`DEFAULT_WINDOW`] and at
/// [`DIALOG_WINDOW`], with the window at the screen's corner. X starts the pointer at the screen's centre,
/// where it would light up whatever it is over: this screen is wide enough to keep it right of the window.
const SCREEN: [u32; 2] = [3000, 2500];
/// The size of VectorCraft's window for the picture of a plug-in's dialog: its default width, and tall
/// enough for satin's, the tallest, about 2,050 pixels with its 43 rows in 17 sections at the defaults. At
/// the default height, fill's and satin's would scroll their rows, as VectorCraft does from 0.14 when a
/// dialog is taller than its window (its `modal::scrolled_rows`), and a picture would show only some of
/// them. The probe fails on a dialog that scrolls.
pub(crate) const DIALOG_WINDOW: [u32; 2] = [1440, 2400];
const _: () = assert!(DIALOG_WINDOW[0] <= SCREEN[0] && DIALOG_WINDOW[1] <= SCREEN[1], "the dialogs' window fits on the screen");
const _: () = assert!(SCREEN[0] / 2 >= DIALOG_WINDOW[0] && SCREEN[0] / 2 >= DEFAULT_WINDOW[0], "the pointer starts right of the window");
/// Turns the accessibility bus's screen-reader flag on, then runs the rest of the command line. `sh -c`
/// takes the command line as its arguments (`"$@"`), so nothing is quoted into the script.
const SCREEN_READER_ON: &str = "dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
    org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:ScreenReaderEnabled variant:boolean:true >/dev/null && exec \"$@\"";
/// The programs the desktop takes.
const PROGRAMS: [&str; 3] = ["xvfb-run", "dbus-run-session", "dbus-send"];
/// What the desktop needs: its programs, and where the Ubuntu packages that have them are listed, with the
/// libraries the probe builds with.
const NEEDS: &str = "xvfb-run, dbus-run-session and dbus-send (on Ubuntu, the packages `.github/actions/desktop` installs)";

/// Whether the private desktop can be made here: on Linux, with its [`PROGRAMS`] in the folders of
/// `path`, `PATH`'s value. Checked first, before an app is fetched or a previous run's records cleared.
fn ready_with(path: Option<&OsStr>) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("VectorCraft's window runs on Linux here: its desktop is Xvfb's and AT-SPI's".to_string());
    }
    if !PROGRAMS.iter().all(|program| on_path(path, program)) {
        return Err(format!("VectorCraft's window needs {NEEDS}"));
    }
    Ok(())
}

/// [`ready_with`] the `PATH` this runs with.
pub(crate) fn ready() -> Result<(), String> {
    ready_with(std::env::var_os("PATH").as_deref())
}

/// Runs the probe's tests with `app`, or the pinned release's, in a private desktop.
pub fn run(app: Option<&Path>) -> Result<(), String> {
    run_with(app, std::env::var_os("PATH").as_deref())
}

/// [`run`], with the desktop's programs looked for in `path`.
fn run_with(app: Option<&Path>, path: Option<&OsStr>) -> Result<(), String> {
    ready_with(path)?;
    let app = match app {
        Some(app) => std::fs::canonicalize(app).map_err(|e| format!("{}: {e}", app.display()))?,
        None => released()?,
    };
    let out = util::root().join(OUT);
    let ran = in_desktop(&app, &out, &[], None);
    let errors = show(&out, std::env::var_os("GITHUB_ACTIONS").is_some())?;
    match ran {
        Ok(()) => {
            println!("compat: VectorCraft's window ({}) runs headless, photographs alike and answers xa11y", util::rel(&app));
            Ok(())
        }
        Err(e) if errors.is_empty() => Err(e),
        Err(_) => Err(errors.join("\n\n")),
    }
}

/// Takes the `vectorcraft-ui` shots that the job file `job` lists, in the window of `app`, recording into
/// `out`: what the shots recorded as errors. Their notices are shown, as annotations in GitHub Actions.
pub(crate) fn shots(app: &Path, job: &Path, out: &Path) -> Result<Vec<String>, String> {
    ready()?;
    let ran = in_desktop(app, out, &["--test", "shots"], Some(job));
    let errors = show(out, std::env::var_os("GITHUB_ACTIONS").is_some())?;
    match ran {
        Err(e) if errors.is_empty() => Err(e),
        _ => Ok(errors),
    }
}

/// Runs the probe's tests, or those `tests` picks, with the app `app` in a private desktop, recording into
/// `out`, which starts empty, and with `job` as the shots to take. The caller has checked that the desktop
/// can be made ([`ready`]).
fn in_desktop(app: &Path, out: &Path, tests: &[&str], job: Option<&Path>) -> Result<(), String> {
    let root = util::root();
    if out.exists() {
        std::fs::remove_dir_all(out).map_err(|e| format!("{}: {e}", util::rel(out)))?;
    }
    let runtime = out.join("runtime");
    private_folder(&runtime)?;
    let mut line: Vec<OsString> = vec![
        util::cargo().get_program().to_os_string(),
        "test".into(),
        "--locked".into(),
        "--manifest-path".into(),
        root.join(PROBE).join("Cargo.toml").into(),
    ];
    line.extend(tests.iter().map(OsString::from));
    // One window at a time: each test finds its own by its process.
    line.extend(["--".into(), "--test-threads=1".into()]);
    let mut cmd = Command::new("dbus-run-session");
    cmd.args(desktop(line))
        // The desktop's own: settings in memory, sockets and caches in `out`, and X11 alone.
        .env("GSETTINGS_BACKEND", "memory")
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("XDG_CACHE_HOME", out.join("cache"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("AT_SPI_BUS_ADDRESS")
        .env(APP_VAR, app)
        .env(OUT_VAR, out)
        // One build directory for the compatibility crates, inside the cached `target/`.
        .env("CARGO_TARGET_DIR", root.join("target/compat-build"));
    if let Some(job) = job {
        cmd.env(SHOTS_VAR, job);
    }
    util::run(cmd, "VectorCraft's window")
}

/// The arguments of `dbus-run-session` that run `tests` in the private desktop.
fn desktop(tests: Vec<OsString>) -> Vec<OsString> {
    let screen = format!("-screen 0 {}x{}x24", SCREEN[0], SCREEN[1]);
    let head = ["--", "sh", "-c", SCREEN_READER_ON, "window", "xvfb-run", "--auto-servernum", "--server-args", screen.as_str()];
    head.into_iter().map(OsString::from).chain(tests).collect()
}

/// Makes `dir`, which only its owner may open, as a runtime folder must be.
fn private_folder(dir: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", util::rel(dir)))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700)).map_err(|e| format!("{}: {e}", util::rel(dir)))?;
    }
    Ok(())
}

/// Prints the notices the tests recorded in `out`, as annotations with `annotate`, and returns their
/// errors.
fn show(out: &Path, annotate: bool) -> Result<Vec<String>, String> {
    let path = out.join(FINDINGS);
    let text = if path.exists() { util::read(&path)? } else { String::new() };
    let mut errors = Vec::new();
    for line in text.lines() {
        let finding: Value = serde_json::from_str(line).map_err(|e| format!("{}: {e}", util::rel(&path)))?;
        let field = |key: &str| finding.get(key).and_then(Value::as_str).unwrap_or_default().to_string();
        let (title, message) = (field("title"), field("message"));
        match field("level").as_str() {
            "notice" if annotate => println!("{}", util::annotation("notice", &title, &message)),
            "notice" => println!("{title}: {message}"),
            _ => errors.push(format!("{}: {message}", field("test"))),
        }
    }
    Ok(errors)
}

/// `bin/vectorcraft` of the pinned stable release's Linux x86_64 archive, fetched if need be and checked
/// against the pins file's SHA-256 either way.
fn released() -> Result<PathBuf, String> {
    let pins = compat::load_pins()?;
    Ok(release::fetch(compat::pinned_tag(&pins, "stable"))?.root.join("bin/vectorcraft"))
}

/// Whether `program` is a file in a folder of `path`, `PATH`'s value.
fn on_path(path: Option<&OsStr>, program: &str) -> bool {
    path.is_some_and(|path| std::env::split_paths(path).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_tests_run_in_a_private_desktop_with_the_screen_reader_flag_on() {
        let line = desktop(vec!["cargo".into(), "test".into()]);
        let expected = [
            "--",
            "sh",
            "-c",
            SCREEN_READER_ON,
            "window",
            "xvfb-run",
            "--auto-servernum",
            "--server-args",
            "-screen 0 3000x2500x24",
            "cargo",
            "test",
        ];
        assert_eq!(line, expected.map(OsString::from));
        assert!(SCREEN_READER_ON.ends_with("&& exec \"$@\"") && SCREEN_READER_ON.contains("ScreenReaderEnabled variant:boolean:true"));
    }

    #[test]
    fn the_names_are_the_probe_s() {
        let probe = util::read(&util::root().join(PROBE).join("src/lib.rs")).unwrap();
        for (name, value) in [("APP", APP_VAR), ("OUT", OUT_VAR), ("SHOTS", SHOTS_VAR), ("FINDINGS", FINDINGS)] {
            assert!(probe.contains(&format!("pub const {name}: &str = \"{value}\";")), "the probe's {name} is not {value}");
        }
    }

    #[test]
    fn the_desktop_is_checked_before_the_app() {
        // With none of the desktop's programs on `PATH`, that is what is said: neither the app nor a
        // previous run's findings.
        let said = run_with(Some(Path::new("/no/such/vectorcraft")), Some(OsStr::new(""))).unwrap_err();
        let desktop = if cfg!(target_os = "linux") { "needs xvfb-run" } else { "runs on Linux" };
        assert!(said.contains(desktop), "{said}");
    }

    #[test]
    fn notices_are_shown_and_errors_returned() {
        let out = util::root().join("target/xtask-tests/window");
        std::fs::create_dir_all(&out).unwrap();
        let lines = [
            r#"{"test":"a","level":"notice","title":"window","message":"1440×900"}"#,
            r#"{"test":"b","level":"error","title":"b","message":"no tree"}"#,
        ];
        std::fs::write(out.join(FINDINGS), lines.join("\n")).unwrap();
        assert_eq!(show(&out, false).unwrap(), ["b: no tree"]);
        std::fs::remove_file(out.join(FINDINGS)).unwrap();
        assert!(show(&out, false).unwrap().is_empty());
        // The command runs on Linux, where `sh` always is.
        let path = std::env::var_os("PATH");
        assert!(!on_path(path.as_deref(), "no-such-program-anywhere") && (on_path(path.as_deref(), "sh") || !cfg!(target_os = "linux")));
    }
}
