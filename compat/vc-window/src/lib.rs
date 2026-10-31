//! The window probe: VectorCraft's released app in a desktop without a screen (spike M0.8).
//!
//! **Why.** The docs show VectorCraft's menus and dialogs with StitchCraft's plug-ins in them, taken from
//! the app itself on every run (the `vectorcraft-ui` pictures, `docs/src/design/docs-pipeline.md`).
//! That takes three things CI might not give: a window with no screen to show it on, the same pixels each
//! time, and a way to reach the controls a person uses. The tests in `tests/window.rs` check the three
//! against the app as VectorCraft releases it:
//!
//! - its window opens under Xvfb, drawn by Mesa's software renderer;
//! - `ui.screenshot`, over the app's control channel, gives the same pixels twice, and again once the app
//!   has started anew;
//! - xa11y finds the window in the accessibility tree (AT-SPI on Linux) and presses a menu item there,
//!   and the control channel sees what the press did.
//!
//! `cargo xtask compat window` runs them in a private desktop (`xtask/src/window.rs`) and names the app
//! in [`APP`]. Each test records what it measured, or why it failed, with [`record`], and the command
//! shows the records, as annotations in CI.
//!
//! The docs' `vectorcraft-ui` pictures are taken the same way (roadmap step M6.7): `tests/shots.rs` drives
//! the window to each menu and dialog and crops the screenshot to it ([`Pixels::crop`]), when
//! `cargo xtask shots --vectorcraft` names a job in [`SHOTS`].
#![forbid(unsafe_code)]
// A harness, not a crate of the engine: it starts a process, waits on it and on a socket, and writes and
// reads files, all of which `clippy.toml` keeps out of the engine's crates for determinism.
#![allow(clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The variable that names the app: `bin/vectorcraft` of a VectorCraft release.
pub const APP: &str = "STITCHCRAFT_VECTORCRAFT_APP";
/// The variable that names the folder the tests record into: screenshots, the tree, the app's log and
/// [`FINDINGS`].
pub const OUT: &str = "STITCHCRAFT_WINDOW_OUT";
/// The file in [`OUT`] that [`record`] adds to: one JSON object a line, `{test, level, title, message}`.
pub const FINDINGS: &str = "findings.jsonl";
/// The variable that names the job of `tests/shots.rs`: the `vectorcraft-ui` shots to take, as
/// `cargo xtask shots --vectorcraft` writes them.
pub const SHOTS: &str = "STITCHCRAFT_SHOTS";

/// How long the app may take to show its window and open the file. Mesa's software renderer builds its
/// shaders first.
const START: Duration = Duration::from_secs(90);
/// How long one call over the control channel may take. The app answers a screenshot once it has drawn 3
/// frames and waited out its animations, within 8 s, and gives up on any request after 60 s.
const CALL: Duration = Duration::from_secs(70);
/// How long the app may take to quit before it is ended.
const QUIT: Duration = Duration::from_secs(10);
/// How often a wait looks again.
const POLL: Duration = Duration::from_millis(100);

/// A VectorCraft window, and the app's control channel: JSON lines over a loopback socket, one request
/// and one reply a line (VectorCraft's `docs/control-protocol.md`).
pub struct Window {
    app: Child,
    /// Where the app's standard output and error go.
    log: PathBuf,
    channel: Option<BufReader<TcpStream>>,
    next: u64,
}

impl Window {
    /// Starts the app with `file` open, its log in `<name>.log`, and waits until its window shows the file.
    pub fn open(file: &Path, name: &str) -> Result<Window, String> {
        let app = std::env::var_os(APP).map(PathBuf::from).ok_or(format!("{APP} names no app: `cargo xtask compat window` sets it"))?;
        let log = out()?.join(format!("{name}.log"));
        let written = File::create(&log).map_err(|e| format!("{}: {e}", log.display()))?;
        let also = written.try_clone().map_err(|e| format!("{}: {e}", log.display()))?;
        let port = free_port()?;
        let child = Command::new(&app)
            .arg("--control")
            .arg(port.to_string())
            .arg(file)
            // Without preferences, every run starts as a new user's, and writes nothing outside the run.
            .env("VECTORCRAFT_NO_PREFS", "1")
            // OpenGL, which Mesa draws in software (llvmpipe) wherever there is no GPU. The app would take
            // Mesa's Vulkan renderer first where it is installed, and the pixels would depend on that.
            .env("WGPU_BACKEND", "gl")
            // The app asks GitHub for its releases as it starts, and once one newer than the pinned
            // release is out, it offers the update in a dialog over the window, in some starts and not
            // others. The check takes the proxy the environment names: one that nothing listens on fails
            // it at once, and a check at start that fails says nothing.
            .env("ALL_PROXY", "http://127.0.0.1:9")
            .stdin(Stdio::null())
            .stdout(also)
            .stderr(written)
            .spawn()
            .map_err(|e| format!("{}: {e}", app.display()))?;
        // From here on, dropping the window ends the app, on every way out.
        let mut window = Window { app: child, log, channel: None, next: 1 };
        window.connect(port)?;
        window.until("the file to open", |inspect| inspect.get("documents").and_then(Value::as_array).is_some_and(|d| !d.is_empty()))?;
        Ok(window)
    }

    /// The app's process id, by which xa11y finds it.
    pub fn pid(&self) -> u32 {
        self.app.id()
    }

    /// Calls `method` with `params` over the control channel: its result, or its error.
    pub fn call(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next;
        self.next += 1;
        let request = json!({"id": id, "method": method, "params": params});
        let line = {
            let channel = self.channel.as_mut().ok_or("the control channel is not open")?;
            writeln!(channel.get_mut(), "{request}").and_then(|()| channel.get_mut().flush()).map_err(|e| format!("{method}: {e}"))?;
            let mut line = String::new();
            channel.read_line(&mut line).map_err(|e| format!("{method}: no answer ({e})"))?;
            line
        };
        if line.is_empty() {
            return Err(self.failed(&format!("{method}: the app closed its control channel")));
        }
        let reply: Value = serde_json::from_str(&line).map_err(|e| format!("{method}: {e}: {}", line.trim()))?;
        if reply.get("id") != Some(&json!(id)) {
            return Err(format!("{method}: the reply is to another request: {}", line.trim()));
        }
        match reply.get("ok").and_then(Value::as_bool) {
            Some(true) => Ok(reply.get("result").cloned().unwrap_or(Value::Null)),
            _ => Err(format!("{method}: {}", reply.get("error").map_or_else(|| line.trim().to_string(), Value::to_string))),
        }
    }

    /// `ui.inspect`: the app's UI state.
    pub fn inspect(&mut self) -> Result<Value, String> {
        self.call("ui.inspect", json!({}))
    }

    /// Waits until `ui.inspect` shows what `done` looks for: the state it showed then.
    pub fn until(&mut self, what: &str, done: impl Fn(&Value) -> bool) -> Result<Value, String> {
        let started = Instant::now();
        loop {
            let inspect = self.inspect()?;
            if done(&inspect) {
                return Ok(inspect);
            }
            if started.elapsed() > START {
                return Err(self.failed(&format!("waited {START:?} for {what}; ui.inspect shows {inspect}")));
            }
            std::thread::sleep(POLL);
        }
    }

    /// Whether `ui.inspect` shows what `done` looks for within `wait`.
    pub fn shows(&mut self, wait: Duration, done: impl Fn(&Value) -> bool) -> Result<bool, String> {
        let started = Instant::now();
        loop {
            if done(&self.inspect()?) {
                return Ok(true);
            }
            if started.elapsed() > wait {
                return Ok(false);
            }
            std::thread::sleep(POLL);
        }
    }

    /// The first row of `shot`, a screenshot of this window, below the canvas: the status bar's. Its frame
    /// timings change with every frame the app draws, so two screenshots differ there.
    pub fn below_canvas(&mut self, shot: &Pixels) -> Result<u32, String> {
        let inspect = self.inspect()?;
        let number = |at: &str| inspect.pointer(at).and_then(Value::as_f64).ok_or(format!("ui.inspect shows no {at}: {inspect}"));
        // `window` and `canvasRect` are in points; the screenshot is in pixels.
        let scale = f64::from(shot.width) / number("/window/0")?;
        let row = ((number("/canvasRect/1")? + number("/canvasRect/3")?) * scale).round();
        if row > 0.0 && row <= f64::from(shot.height) {
            Ok(row as u32)
        } else {
            Err(format!("the canvas ends at row {row}, outside the {}×{} screenshot", shot.width, shot.height))
        }
    }

    /// A screenshot of the window, as the app writes it to `<name>.png`, and its pixels.
    pub fn screenshot(&mut self, name: &str) -> Result<Pixels, String> {
        self.screenshot_with(name, true)
    }

    /// A screenshot of the window as it is, without first giving it the focus: a menu stays open.
    pub fn screenshot_still(&mut self, name: &str) -> Result<Pixels, String> {
        self.screenshot_with(name, false)
    }

    fn screenshot_with(&mut self, name: &str, focus: bool) -> Result<Pixels, String> {
        let path = out()?.join(format!("{name}.png"));
        let text = path.to_str().ok_or(format!("{}: not UTF-8", path.display()))?;
        self.call("ui.screenshot", json!({"path": text, "focus": focus}))?;
        Pixels::read(&path)
    }

    /// Connects to the control channel on `port`, which the app opens once its window exists.
    fn connect(&mut self, port: u16) -> Result<(), String> {
        let started = Instant::now();
        loop {
            if let Ok(stream) = TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
                stream.set_read_timeout(Some(CALL)).map_err(|e| e.to_string())?;
                self.channel = Some(BufReader::new(stream));
                return Ok(());
            }
            if let Some(status) = self.app.try_wait().map_err(|e| e.to_string())? {
                return Err(self.failed(&format!("the app ended ({status}) before its control channel opened")));
            }
            if started.elapsed() > START {
                return Err(self.failed(&format!("the app's control channel did not open within {START:?}")));
            }
            std::thread::sleep(POLL);
        }
    }

    /// `what` went wrong, with the last lines the app logged.
    fn failed(&self, what: &str) -> String {
        let log = std::fs::read_to_string(&self.log).unwrap_or_default();
        let lines: Vec<&str> = log.lines().collect();
        let last = lines.get(lines.len().saturating_sub(30)..).unwrap_or_default();
        format!("{what}\nthe app's last lines ({}):\n{}", self.log.display(), last.join("\n"))
    }
}

impl Drop for Window {
    /// Asks the app to quit, as a person would, and ends it if it does not.
    fn drop(&mut self) {
        if self.channel.is_some() {
            // Its answer does not matter: the app is ended below if it stays.
            let _ = self.call("app.quit", json!({}));
        }
        let started = Instant::now();
        while started.elapsed() < QUIT {
            if let Ok(Some(_)) = self.app.try_wait() {
                return;
            }
            std::thread::sleep(POLL);
        }
        let _ = self.app.kill();
        let _ = self.app.wait();
    }
}

/// A screenshot's pixels: 8-bit RGBA, row by row from the top.
#[derive(Debug, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

impl Pixels {
    /// The pixels of the PNG at `path`, which must be 8-bit RGBA, as the app writes its screenshots.
    pub fn read(path: &Path) -> Result<Pixels, String> {
        let what = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        let file = File::open(path).map_err(|e| what(&e))?;
        let mut reader = png::Decoder::new(BufReader::new(file)).read_info().map_err(|e| what(&e))?;
        let size = reader.output_buffer_size().ok_or_else(|| what(&"too large"))?;
        let mut rgba = vec![0; size];
        let frame = reader.next_frame(&mut rgba).map_err(|e| what(&e))?;
        if (frame.color_type, frame.bit_depth) != (png::ColorType::Rgba, png::BitDepth::Eight) {
            return Err(what(&format!("{:?} at {:?} bits, not 8-bit RGBA", frame.color_type, frame.bit_depth)));
        }
        rgba.truncate(frame.buffer_size());
        Ok(Pixels { width: frame.width, height: frame.height, rgba })
    }

    /// How many pixels differ from `other`'s by more than `tolerance` in a channel; `None` when the two
    /// differ in size.
    pub fn differing(&self, other: &Pixels, tolerance: u8) -> Option<usize> {
        let same_size = (self.width, self.height) == (other.width, other.height) && self.rgba.len() == other.rgba.len();
        let differs = |(a, b): &(&[u8; 4], &[u8; 4])| a.iter().zip(b.iter()).any(|(x, y)| x.abs_diff(*y) > tolerance);
        same_size.then(|| self.rgba.as_chunks::<4>().0.iter().zip(other.rgba.as_chunks::<4>().0.iter()).filter(differs).count())
    }

    /// Rows `from` to `to`, the last excluded, cut to the picture.
    pub fn rows(&self, from: u32, to: u32) -> Pixels {
        let from = from.min(self.height);
        let to = to.clamp(from, self.height);
        let stride = self.width as usize * 4;
        let rgba = self.rgba.get(from as usize * stride..to as usize * stride).unwrap_or_default().to_vec();
        Pixels { width: self.width, height: to - from, rgba }
    }

    /// The rectangle `x`, `y`, `width` × `height` of the picture, cut to it.
    pub fn crop(&self, x: i64, y: i64, width: i64, height: i64) -> Pixels {
        let clamp = |v: i64, max: u32| u32::try_from(v.clamp(0, i64::from(max))).unwrap_or(0);
        let (left, top) = (clamp(x, self.width), clamp(y, self.height));
        let (right, bottom) = (clamp(x.saturating_add(width), self.width).max(left), clamp(y.saturating_add(height), self.height).max(top));
        let stride = self.width as usize * 4;
        let mut rgba = Vec::with_capacity((right - left) as usize * (bottom - top) as usize * 4);
        for row in top..bottom {
            let start = row as usize * stride + left as usize * 4;
            rgba.extend_from_slice(self.rgba.get(start..start + (right - left) as usize * 4).unwrap_or_default());
        }
        Pixels { width: right - left, height: bottom - top, rgba }
    }

    /// Writes the picture to `path` as an 8-bit RGBA PNG, making its folder.
    pub fn write(&self, path: &Path) -> Result<(), String> {
        let what = |e: &dyn std::fmt::Display| format!("{}: {e}", path.display());
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| what(&e))?;
        }
        let mut encoder = png::Encoder::new(File::create(path).map_err(|e| what(&e))?, self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().and_then(|mut w| w.write_image_data(&self.rgba)).map_err(|e| what(&e))
    }

    /// The number of pixels.
    pub fn count(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// A fingerprint of the size and the pixels (64-bit FNV-1a), by which runs compare their screenshots.
    pub fn fingerprint(&self) -> u64 {
        let size = [self.width.to_le_bytes(), self.height.to_le_bytes()].concat();
        size.iter().chain(&self.rgba).fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
    }
}

/// The folder the tests record into, [`OUT`] (or `target/window` in this crate), made if need be.
pub fn out() -> Result<PathBuf, String> {
    let dir = std::env::var_os(OUT).map_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")).join("target/window"), PathBuf::from);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    Ok(dir)
}

/// A drawing from `docs/fixtures/`, by its absolute path.
pub fn fixture(name: &str) -> Result<PathBuf, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/fixtures").join(name);
    std::fs::canonicalize(&path).map_err(|e| format!("{}: {e}", path.display()))
}

/// Adds what `test` found to [`FINDINGS`]: `level` is `notice` for a measurement, `error` for a failure.
pub fn record(test: &str, level: &str, title: &str, message: &str) -> Result<(), String> {
    let path = out()?.join(FINDINGS);
    let line = json!({"test": test, "level": level, "title": title, "message": message});
    let mut file = OpenOptions::new().create(true).append(true).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    writeln!(file, "{line}").map_err(|e| format!("{}: {e}", path.display()))
}

/// The role of a label in the accessibility tree, in snake case.
const LABEL: &str = "static_text";

/// The controls a plug-in's dialog shows, by label in lower case, from the nodes of its accessibility tree in
/// order, each its role (in snake case) and its name: a row is a label with a colon after it, and then its
/// control (`tests/shots.rs`). Only a [`LABEL`] is a label. VectorCraft 0.13.1 names a checkbox
/// after its row's label, and the node after the checkbox is the next row's label, not its control.
pub fn dialog_controls(nodes: &[(&'static str, Option<String>)]) -> BTreeMap<String, &'static str> {
    nodes
        .windows(2)
        .filter_map(|pair| {
            let [(LABEL, Some(label)), (control, _)] = pair else { return None };
            Some((label.strip_suffix(':')?.to_lowercase(), *control))
        })
        .collect()
}

/// A loopback port no one listens on, for the app's control channel.
fn free_port() -> Result<u16, String> {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).map_err(|e| format!("a free port: {e}"))?;
    listener.local_addr().map(|a| a.port()).map_err(|e| format!("a free port: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixels(rgba: &[u8]) -> Pixels {
        Pixels { width: 2, height: 1, rgba: rgba.to_vec() }
    }

    #[test]
    fn pixels_differ_by_more_than_the_tolerance_in_a_channel() {
        let a = pixels(&[10, 10, 10, 255, 0, 0, 0, 255]);
        let b = pixels(&[10, 19, 10, 255, 0, 0, 8, 255]);
        assert_eq!(a.differing(&b, 0), Some(2));
        assert_eq!(a.differing(&b, 8), Some(1));
        assert_eq!(a.differing(&b, 9), Some(0));
        assert_eq!(a.differing(&Pixels { width: 1, height: 2, rgba: a.rgba.clone() }, 0), None);
    }

    #[test]
    fn rows_are_cut_to_the_picture() {
        let tall = Pixels { width: 1, height: 3, rgba: (0..12).collect() };
        assert_eq!(tall.rows(1, 2), Pixels { width: 1, height: 1, rgba: vec![4, 5, 6, 7] });
        assert_eq!(tall.rows(2, 9), Pixels { width: 1, height: 1, rgba: vec![8, 9, 10, 11] });
        assert_eq!(tall.rows(2, 1).height, 0);
        assert_eq!(tall.rows(5, 9), Pixels { width: 1, height: 0, rgba: Vec::new() });
    }

    #[test]
    fn a_crop_is_cut_to_the_picture() {
        let picture = Pixels { width: 3, height: 2, rgba: (0..24).collect() };
        assert_eq!(picture.crop(1, 0, 1, 2), Pixels { width: 1, height: 2, rgba: vec![4, 5, 6, 7, 16, 17, 18, 19] });
        // Beyond an edge, the crop stops at it.
        assert_eq!(picture.crop(-5, 1, 7, 9), Pixels { width: 2, height: 1, rgba: (12..20).collect() });
        assert_eq!(picture.crop(5, 5, 2, 2), Pixels { width: 0, height: 0, rgba: Vec::new() });
    }

    #[test]
    fn a_written_picture_reads_back() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/unit/written.png");
        let picture = Pixels { width: 2, height: 1, rgba: vec![1, 2, 3, 255, 4, 5, 6, 255] };
        picture.write(&path).unwrap();
        assert_eq!(Pixels::read(&path).unwrap(), picture);
    }

    #[test]
    fn the_fingerprint_counts_the_size_and_every_byte() {
        let a = pixels(&[1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(a.fingerprint(), pixels(&[1, 2, 3, 4, 5, 6, 7, 8]).fingerprint());
        assert_ne!(a.fingerprint(), pixels(&[1, 2, 3, 4, 5, 6, 7, 9]).fingerprint());
        assert_ne!(a.fingerprint(), Pixels { width: 1, height: 2, rgba: a.rgba.clone() }.fingerprint());
        // 64-bit FNV-1a of 8 zero bytes, the size of an empty picture.
        assert_eq!(Pixels { width: 0, height: 0, rgba: Vec::new() }.fingerprint(), 0xa8c7_f832_281a_39c5);
    }

    #[test]
    fn a_dialog_s_row_is_its_label_and_the_control_after_it() {
        let node = |role: &'static str, name: Option<&str>| (role, name.map(str::to_string));
        // From VectorCraft 0.13.1's tree of the stroke dialog: a checkbox is named after its row's label.
        let nodes = [
            node("window", Some("StitchCraft Stroke")),
            node("static_text", Some("Enable_random_stitch_length:")),
            node("check_box", Some("Enable_random_stitch_length:")),
            node("static_text", Some("Random_stitch_length_jitter_percent:")),
            node("text_field", None),
            node("static_text", Some("Ties:")),
            node("combo_box", None),
        ];
        let controls = dialog_controls(&nodes);
        let expected = [("enable_random_stitch_length", "check_box"), ("random_stitch_length_jitter_percent", "text_field"), ("ties", "combo_box")];
        assert_eq!(controls, expected.map(|(key, control)| (key.to_string(), control)).into());
    }

    #[test]
    fn a_png_reads_back_as_the_pixels_written() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/unit");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("two.png");
        let mut encoder = png::Encoder::new(File::create(&path).unwrap(), 2, 1);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.write_header().unwrap().write_image_data(&[1, 2, 3, 4, 5, 6, 7, 8]).unwrap();
        assert_eq!(Pixels::read(&path).unwrap(), pixels(&[1, 2, 3, 4, 5, 6, 7, 8]));
        let mut grey = png::Encoder::new(File::create(&path).unwrap(), 1, 1);
        grey.set_color(png::ColorType::Grayscale);
        grey.write_header().unwrap().write_image_data(&[7]).unwrap();
        assert!(Pixels::read(&path).unwrap_err().contains("not 8-bit RGBA"));
    }
}
