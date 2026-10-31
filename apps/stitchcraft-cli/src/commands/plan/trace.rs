//! A picture traced by VectorCraft's command line, which `stitch plan picture.png` then sews digitized
//! automatically (`REQ-CLI-012`, `REQ-CLI-013`; ADR-0015, `docs/src/design/image-to-embroidery.md` ›
//! Pictures from the command line).
//!
//! **Why VectorCraft traces.** StitchCraft traces nothing itself: VectorCraft's Image Trace does, measured
//! by its own harness, so a picture given to `stitch` sews as the same picture traced in VectorCraft does,
//! and a fix to the tracer helps both. Library crates start no processes, so running VectorCraft's command
//! line is the `stitch` app's.
//!
//! **The steps** ([`steps`]) are the compatibility gate's `trace` case's, which the user takes by hand in
//! the tutorial: the machine's threads, as `stitch palette` writes them, loaded as a swatch library; the
//! picture placed from its bytes; Image Trace in colour with that library's colours as they are, by the
//! Abutting method, so that the shapes share their edges; and Expand, which keeps the paths. The picture is
//! fitted, its aspect kept, inside a square `--size` millimetres across, or else inside the machine's hoop,
//! so that any picture sews without a size given. Image Trace keeps at most `--colours` of the library's
//! colours, [`COLOURS`] unless it says otherwise: Image Trace's own default.
//!
//! **The program** is the one `--vectorcraft-cli` names, or else [`PROGRAM`] on the PATH, which
//! [`Command`] looks for. `vectorcraft-cli run --commands -` reads the steps from standard input, where a
//! picture's bytes fit whatever their size, and saves the document into a folder of the command's own beside
//! the machine file ([`Scratch`]), which is removed with it once it is read. It runs with
//! `VECTORCRAFT_NO_PREFS` set, so that it neither reads nor writes the user's VectorCraft preferences, and a
//! picture traces the same for everyone. Its standard input is written from a thread of its own while its
//! standard error is read, so that neither waits on the other.
//!
//! **What fails.** A program that can't be started is `SC-E0815`, and one that fails or saves no document is
//! `SC-E0816`, which ends with the end of what it said on standard error, each character a terminal would
//! act on escaped ([`printable`]).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use stitchcraft_core::{Code, Diagnostic, Fix, Mm, Size};
use stitchcraft_plan::MachineProfile;

use crate::commands::{palette, printable};
use crate::files::{self, Scratch};

/// The extensions of the pictures `stitch plan` traces, in any case.
pub const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];
/// How many colours Image Trace keeps when `--colours` doesn't say: its own default.
pub const COLOURS: u32 = 6;
/// VectorCraft's command line, looked for on the PATH when `--vectorcraft-cli` names none.
pub const PROGRAM: &str = "vectorcraft-cli";
/// Where VectorCraft publishes its command line, with each release.
const RELEASES: &str = "https://github.com/storytold/vectorcraft/releases";
/// The name the threads are loaded as a swatch library under.
const LIBRARY: &str = "threads.gpl";
/// The document VectorCraft saves, in its folder.
const SAVED: &str = "traced.vectorcraft";
/// How many of the last lines VectorCraft said on standard error a failure keeps, and how many characters.
const SAID_LINES: usize = 10;
const SAID_CHARS: usize = 2_000;

/// Whether the design at `path` is a picture, by its extension.
pub fn is_picture(path: &Path) -> bool {
    let extension = path.extension().and_then(|extension| extension.to_str()).unwrap_or_default();
    EXTENSIONS.iter().any(|known| extension.eq_ignore_ascii_case(known))
}

/// The steps VectorCraft's command line runs to trace the picture `bytes`, named `name`, in the threads of
/// `profile`: fitted inside a square `size` across, or else inside the machine's hoop, and traced in at most
/// `colours` of them ([the module](self)).
pub fn steps(name: &str, bytes: &[u8], profile: &MachineProfile, size: Option<Mm>, colours: u32) -> Value {
    let fitted = size.map_or(profile.hoop, |size| Size::new(size, size));
    let rect = [0.0, 0.0, fitted.width.to_points(), fitted.height.to_points()];
    json!([
        {"command": "swatch.library.load", "params": {"name": LIBRARY, "data": palette::gpl(profile)}},
        {"command": "file.place", "params": {"name": name, "dataBase64": stitchcraft_vectorcraft::base64(bytes), "rect": rect}},
        {"command": "imageTrace.make", "params": {"params": {
            "mode": "color",
            "palette": "documentLibrary",
            "library": format!("loaded/{LIBRARY}"),
            "colors": colours,
            "method": "abutting",
        }}},
        {"command": "imageTrace.expand"},
    ])
}

/// The document VectorCraft's command line saves of the picture `name` when it runs `steps`, into the folder
/// `scratch`: the program `program` names, or else [`PROGRAM`] on the PATH. `SC-E0815` when it can't be
/// started, and `SC-E0816` when it fails or saves no document.
pub fn traced(program: Option<&Path>, name: &str, steps: &Value, scratch: &Scratch) -> Result<Vec<u8>, Diagnostic> {
    let saved = scratch.path().join(SAVED);
    let path = program.map_or_else(|| PathBuf::from(PROGRAM), Path::to_path_buf);
    let mut child = Command::new(&path)
        .args(["run", "--commands", "-", "--export"])
        .arg(&saved)
        .env("VECTORCRAFT_NO_PREFS", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| not_started(program, &e))?;
    // A thread writes the steps while this one reads what the program says, so that a program that says much
    // before it reads them all can't wait on a full pipe while this waits on it. One that stops reading ends
    // the writing, and its exit status says why.
    let input = steps.to_string().into_bytes();
    let writer = child.stdin.take().map(|mut stdin| std::thread::Builder::new().spawn(move || stdin.write_all(&input)));
    let output = child.wait_with_output().map_err(|e| failed(name, &format!("it could not be waited for ({e}).")))?;
    if let Some(Ok(writer)) = writer {
        // How the writing went is the program's to say: it reads all the steps, or stops with an error.
        let _ = writer.join();
    }
    if !output.status.success() {
        let status = output.status;
        let ended = status.code().map_or_else(|| format!("it was stopped ({status})"), |code| format!("it ended with status {code}"));
        let why = said(&output.stderr).map_or_else(|| format!("{ended}, and said nothing."), |said| format!("{ended}, and said: {said}"));
        return Err(failed(name, &why));
    }
    files::read_capped(&saved).map_err(|e| failed(name, &format!("it saved no document ({e}).")))
}

/// `SC-E0815`: the program `program` names, or [`PROGRAM`] on the PATH, didn't start, with `error`.
fn not_started(program: Option<&Path>, error: &std::io::Error) -> Diagnostic {
    let which = program.map_or_else(|| format!("`{PROGRAM}` on the PATH"), |path| path.display().to_string());
    let hint = format!(
        "Install VectorCraft's command line from {RELEASES}, and put it on the PATH or name it with --vectorcraft-cli. Or trace the \
         picture with Image Trace in VectorCraft, and plan the document you save."
    );
    Diagnostic::new(Code::VcCliNotStarted, format!("VectorCraft's command line, {which}, could not be started: {error}.")).with_fix(Fix::Hint(hint))
}

/// `SC-E0816`: VectorCraft's command line didn't trace the picture `name`, and `why`.
fn failed(name: &str, why: &str) -> Diagnostic {
    let hint = "Check that VectorCraft places the picture, or trace it with Image Trace in VectorCraft and plan the document you save.";
    Diagnostic::new(Code::VcTraceFailed, format!("VectorCraft's command line didn't trace {}: {why}", printable(name)))
        .with_fix(Fix::Hint(hint.to_string()))
}

/// What a program said on standard error, `stderr`: its last lines, as many as [`SAID_LINES`] and
/// [`SAID_CHARS`] keep, escaped; or nothing.
fn said(stderr: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.lines().map(str::trim).filter(|line| !line.is_empty()).collect();
    let last = lines.get(lines.len().saturating_sub(SAID_LINES)..).unwrap_or_default();
    let joined = printable(&last.join(" | "));
    let count = joined.chars().count();
    let kept = if count > SAID_CHARS { format!("…{}", joined.chars().skip(count - SAID_CHARS).collect::<String>()) } else { joined };
    (!kept.is_empty()).then_some(kept)
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::profiles::{BROTHER_PE800_4X4, REFERENCE};
    use stitchcraft_testkit::temp::TempDir;

    use super::*;

    #[test]
    fn req_cli_012_a_picture_is_told_by_its_extension() {
        for picture in ["badge.png", "BADGE.PNG", "photo.jpg", "photo.JPEG", "dir.svg/photo.jpeg"] {
            assert!(is_picture(Path::new(picture)), "{picture}");
        }
        for design in ["badge.svg", "badge.vectorcraft", "png", "badge.png.svg", "badge.pngx", "badge"] {
            assert!(!is_picture(Path::new(design)), "{design}");
        }
    }

    #[test]
    fn req_cli_012_the_steps_load_the_threads_place_the_picture_fitted_and_trace_it_in_them() {
        let picture = b"\x89PNG\r\n\x1a\n and the rest";
        let given = steps("badge.png", picture, REFERENCE, None, COLOURS);
        let expected = json!([
            {"command": "swatch.library.load", "params": {"name": "threads.gpl", "data": palette::gpl(REFERENCE)}},
            {"command": "file.place", "params": {"name": "badge.png", "dataBase64": stitchcraft_vectorcraft::base64(picture), "rect": [0.0, 0.0, REFERENCE.hoop.width.to_points(), REFERENCE.hoop.height.to_points()]}},
            {"command": "imageTrace.make", "params": {"params": {"mode": "color", "palette": "documentLibrary", "library": "loaded/threads.gpl", "colors": 6, "method": "abutting"}}},
            {"command": "imageTrace.expand"},
        ]);
        assert_eq!(given, expected);
        // The reference machine's hoop is 130 × 180 mm: 368.5 × 510.2 points.
        assert_eq!(given[1]["params"]["rect"][2].as_f64().map(|w| (w * 10.0).round()), Some(3685.0));
        // A size fits it inside a square that size across, and another machine traces in its own threads.
        let sized = steps("photo.jpg", picture, &BROTHER_PE800_4X4, Some(Mm::new(80.0).unwrap()), 4);
        let points = Mm::new(80.0).unwrap().to_points();
        assert_eq!(sized[1]["params"]["rect"], json!([0.0, 0.0, points, points]));
        assert_eq!((&sized[0]["params"]["data"], &sized[2]["params"]["params"]["colors"]), (&json!(palette::gpl(&BROTHER_PE800_4X4)), &json!(4)));
        assert_eq!(sized[1]["params"]["name"], "photo.jpg");
    }

    #[test]
    fn diag_sc_e0815_a_vectorcraft_command_line_that_is_not_there_is_named() {
        let dir = TempDir::new("trace-missing");
        let missing = dir.join("no-vectorcraft-cli");
        let scratch = Scratch::beside(&dir.join("badge.pes")).unwrap();
        let said = traced(Some(&missing), "badge.png", &json!([]), &scratch).unwrap_err();
        let error = Command::new(&missing).spawn().unwrap_err();
        assert_eq!(said.to_string(), format!("error SC-E0815: VectorCraft's command line, {}, could not be started: {error}.", missing.display()));
        assert_eq!(
            said.fix.map(|fix| fix.describe()).as_deref(),
            Some(
                "Install VectorCraft's command line from https://github.com/storytold/vectorcraft/releases, and put it on the PATH or name it \
                 with --vectorcraft-cli. Or trace the picture with Image Trace in VectorCraft, and plan the document you save."
            )
        );
        // Without one named, the one on the PATH is meant.
        assert!(not_started(None, &error).message.starts_with("VectorCraft's command line, `vectorcraft-cli` on the PATH, could not be started: "));
    }

    #[test]
    fn req_cli_013_what_vectorcraft_said_is_kept_from_its_end_and_escaped() {
        assert_eq!(said(b""), None);
        assert_eq!(said(b"\n  \n"), None);
        let one = b"vectorcraft-cli: step 1 (file.place): not an image VectorCraft reads (see document.formats)\n";
        assert_eq!(said(one).as_deref(), Some("vectorcraft-cli: step 1 (file.place): not an image VectorCraft reads (see document.formats)"));
        // Control characters are escaped, and only the last lines are kept, joined on one line.
        let many: String = (1..=12).map(|n| format!("line {n}\n")).collect::<String>() + "\x1b[31mred\x07\n";
        assert_eq!(
            said(many.as_bytes()).as_deref(),
            Some("line 4 | line 5 | line 6 | line 7 | line 8 | line 9 | line 10 | line 11 | line 12 | \\u{1b}[31mred\\u{7}")
        );
        // A long tail keeps its end.
        let long = "x".repeat(SAID_CHARS) + "the end";
        let kept = said(long.as_bytes()).unwrap();
        assert!(kept.starts_with('…') && kept.ends_with("the end") && kept.chars().count() == SAID_CHARS + 1, "{kept}");
    }
}
