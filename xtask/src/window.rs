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

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::compat;
use crate::util;

/// The probe's crate.
const PROBE: &str = "compat/vc-window";
/// Where the tests record: screenshots, the tree, the app's logs and the findings.
const OUT: &str = "target/compat/window";
/// The pins file's key for the SHA-256 of the stable release's Linux x86_64 archive.
const SHA_KEY: &str = "linux-x86_64-sha256";
/// The X screen: room for VectorCraft's window at its default size, 1440 × 900, which it keeps on a
/// screen that also has room for a taskbar.
const SCREEN: &str = "-screen 0 1600x1200x24";
/// Turns the accessibility bus's screen-reader flag on, then runs the rest of the command line. `sh -c`
/// takes the command line as its arguments (`"$@"`), so nothing is quoted into the script.
const SCREEN_READER_ON: &str = "dbus-send --session --print-reply --dest=org.a11y.Bus /org/a11y/bus \
    org.freedesktop.DBus.Properties.Set string:org.a11y.Status string:ScreenReaderEnabled variant:boolean:true >/dev/null && exec \"$@\"";
/// The programs the desktop takes, and the Ubuntu packages that have them.
const NEEDS: &str = "xvfb-run, dbus-run-session and dbus-send (on Ubuntu: `apt install xvfb dbus at-spi2-core`)";

/// Runs the probe's tests with `app`, or the pinned release's, in a private desktop.
pub fn run(app: Option<&Path>) -> Result<(), String> {
    if !cfg!(target_os = "linux") {
        return Err("`compat window` runs on Linux: its desktop is Xvfb's and AT-SPI's".to_string());
    }
    if !["xvfb-run", "dbus-run-session", "dbus-send"].iter().all(|program| on_path(program)) {
        return Err(format!("`compat window` needs {NEEDS}"));
    }
    let app = match app {
        Some(app) => std::fs::canonicalize(app).map_err(|e| format!("{}: {e}", app.display()))?,
        None => released()?,
    };
    let root = util::root();
    let out = root.join(OUT);
    if out.exists() {
        std::fs::remove_dir_all(&out).map_err(|e| format!("{OUT}: {e}"))?;
    }
    let runtime = out.join("runtime");
    private_folder(&runtime)?;
    let tests: Vec<OsString> = vec![
        util::cargo().get_program().to_os_string(),
        "test".into(),
        "--locked".into(),
        "--manifest-path".into(),
        root.join(PROBE).join("Cargo.toml").into(),
        "--".into(),
        // One window at a time: each test finds its own by its process.
        "--test-threads=1".into(),
    ];
    let mut cmd = Command::new("dbus-run-session");
    cmd.args(desktop(tests))
        // The desktop's own: settings in memory, sockets and caches in `out`, and X11 alone.
        .env("GSETTINGS_BACKEND", "memory")
        .env("XDG_RUNTIME_DIR", &runtime)
        .env("XDG_CACHE_HOME", out.join("cache"))
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("AT_SPI_BUS_ADDRESS")
        .env("STITCHCRAFT_VECTORCRAFT_APP", &app)
        .env("STITCHCRAFT_WINDOW_OUT", &out)
        // One build directory for the compatibility crates, inside the cached `target/`.
        .env("CARGO_TARGET_DIR", root.join("target/compat-build"));
    let ran = util::run(cmd, "the window probe");
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

/// The arguments of `dbus-run-session` that run `tests` in the private desktop.
fn desktop(tests: Vec<OsString>) -> Vec<OsString> {
    let head = ["--", "sh", "-c", SCREEN_READER_ON, "window", "xvfb-run", "--auto-servernum", "--server-args", SCREEN];
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
    let path = out.join("findings.jsonl");
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

/// `bin/vectorcraft` of the pinned stable release's Linux x86_64 archive, downloaded if need be and
/// checked against the pins file's SHA-256 either way.
fn released() -> Result<PathBuf, String> {
    let pins = compat::load_pins()?;
    let (name, url) = archive(&pins)?;
    let sha = pins
        .get("tracks")
        .and_then(|t| t.get("stable"))
        .and_then(|t| t.get(SHA_KEY))
        .and_then(toml::Value::as_str)
        .ok_or(format!("{}: the stable track records no `{SHA_KEY}`", compat::PINS))?;
    let dir = util::root().join("target/compat").join(&name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", util::rel(&dir)))?;
    let file = dir.join(format!("{name}.tar.gz"));
    if !file.is_file() || sha256(&file)? != sha {
        let mut curl = Command::new("curl");
        curl.args(["--fail", "--silent", "--show-error", "--location", "--retry", "3", "--output"]).arg(&file).arg(&url);
        util::run(curl, &format!("curl {url}"))?;
    }
    let got = sha256(&file)?;
    if got != sha {
        return Err(format!("{url}: its SHA-256 is {got}, and {} records {sha}", compat::PINS));
    }
    let mut tar = Command::new("tar");
    tar.arg("-xzf").arg(&file).arg("-C").arg(&dir);
    util::run(tar, &format!("tar -xzf {}", util::rel(&file)))?;
    Ok(dir.join(&name).join("bin/vectorcraft"))
}

/// The pinned stable release's Linux x86_64 archive: its name without `.tar.gz`, which is also the folder
/// it holds, and its address.
fn archive(pins: &toml::Table) -> Result<(String, String), String> {
    let repository = pins.get("repository").and_then(toml::Value::as_str).ok_or(format!("{}: no `repository`", compat::PINS))?;
    let tag = compat::pinned_tag(pins, "stable");
    let version = tag.strip_prefix('v').filter(|v| !v.is_empty()).ok_or(format!("{}: the stable tag `{tag}` is no `v<version>`", compat::PINS))?;
    let name = format!("vectorcraft-{version}-linux-x86_64");
    let url = format!("{repository}/releases/download/{tag}/{name}.tar.gz");
    Ok((name, url))
}

/// The SHA-256 of `file`, in hexadecimal, by `sha256sum`.
fn sha256(file: &Path) -> Result<String, String> {
    let output = Command::new("sha256sum").arg(file).output().map_err(|e| format!("sha256sum: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let sum = text.split_whitespace().next().filter(|_| output.status.success());
    sum.map(str::to_string).ok_or(format!("sha256sum {}: {}", util::rel(file), String::from_utf8_lossy(&output.stderr).trim()))
}

/// Whether `program` is a file in a folder of `PATH`.
fn on_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|path| std::env::split_paths(&path).any(|dir| dir.join(program).is_file()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pinned_release_is_fetched_from_its_archive_and_checked_by_its_sha() {
        let pins = compat::load_pins().unwrap();
        let (name, url) = archive(&pins).unwrap();
        let tag = compat::pinned_tag(&pins, "stable");
        assert_eq!(url, format!("https://github.com/storytold/vectorcraft/releases/download/{tag}/{name}.tar.gz"));
        assert_eq!(name, format!("vectorcraft-{}-linux-x86_64", &tag[1..]));
        let sha = pins["tracks"]["stable"][SHA_KEY].as_str().unwrap();
        assert!(sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{sha}");
        let unversioned: toml::Table = toml::from_str("repository = \"r\"\n[tracks.stable]\ntag = \"latest\"").unwrap();
        assert!(archive(&unversioned).unwrap_err().contains("no `v<version>`"));
    }

    #[test]
    fn the_tests_run_in_a_private_desktop_with_the_screen_reader_flag_on() {
        let line = desktop(vec!["cargo".into(), "test".into()]);
        let expected = ["--", "sh", "-c", SCREEN_READER_ON, "window", "xvfb-run", "--auto-servernum", "--server-args", SCREEN, "cargo", "test"];
        assert_eq!(line, expected.map(OsString::from));
        assert!(SCREEN_READER_ON.ends_with("&& exec \"$@\"") && SCREEN_READER_ON.contains("ScreenReaderEnabled variant:boolean:true"));
    }

    #[test]
    fn notices_are_shown_and_errors_returned() {
        let out = util::root().join("target/xtask-tests/window");
        std::fs::create_dir_all(&out).unwrap();
        let lines = [
            r#"{"test":"a","level":"notice","title":"window","message":"1440×900"}"#,
            r#"{"test":"b","level":"error","title":"b","message":"no tree"}"#,
        ];
        std::fs::write(out.join("findings.jsonl"), lines.join("\n")).unwrap();
        assert_eq!(show(&out, false).unwrap(), ["b: no tree"]);
        std::fs::remove_file(out.join("findings.jsonl")).unwrap();
        assert!(show(&out, false).unwrap().is_empty());
        // The command runs on Linux, where `sh` always is.
        assert!(!on_path("no-such-program-anywhere") && (on_path("sh") || !cfg!(target_os = "linux")));
    }
}
