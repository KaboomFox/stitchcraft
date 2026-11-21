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
//! fitted, its aspect kept, inside a square `--size` millimetres across, or else inside the machine's hoop
//! [`HOOP_MARGIN`] in from each side, so that any picture sews without a size given. Image Trace keeps at
//! most `--colours` of the library's colours, [`COLOURS`] unless it says otherwise: Image Trace's own
//! default.
//!
//! **The fitting** ([`Fitting`]). Image Trace's fitting options, left at their defaults, draw a traced
//! outline with more points than thread can show: a staircase of pixels keeps its steps as corners, and
//! each curve follows the pixels closely. Measured on the repository's traced pictures, Paths [`PATHS`] and
//! Corners [`CORNERS`] draw them with about a third fewer anchors and fewer jags, for the same colour error
//! against the picture, and the tracer still fits each edge once for both shapes that share it. They are
//! sent only with `--fewer-points`, for every style, since the stitch rule a shape gets still reacts to how
//! its outline is drawn: on the traced dogs a third to a half of the shapes change their rule, and more of
//! the design sews thick (`REQ-CLI-024`; `docs/src/design/image-to-embroidery.md` › Outlines thread can't
//! show). Without it the steps leave every fitting option at Image Trace's default. Every VectorCraft
//! release the compatibility gate supports reads both options.
//!
//! **Why the margin.** A picture's shapes reach its edge, and digitized automatically, a satin column along
//! the edge sews past it: its stitches reach past its rails by their pull compensation
//! (`auto_pull_compensation_mm`), and a rail may lie as far outside the shape as shapes still meet
//! ([`TOUCHING`](stitchcraft_engine::auto::TOUCHING)). A picture fitted to the hoop itself would sew past
//! the hoop, which refuses it (`SC-E0701`). The margin is that reach, rounded up to a tenth of a
//! millimetre. A `--size` is the picture's own, as the user asks for it.
//!
//! **The program** is the one `--vectorcraft-cli` names, or else [`PROGRAM`] on the PATH, which
//! [`Command`] looks for. `vectorcraft-cli run --commands -` reads the steps from standard input, where a
//! picture's bytes fit whatever their size, and saves the document into a folder of the command's own beside
//! the machine file ([`Scratch`]), which is removed with it once it is read. It runs with
//! `VECTORCRAFT_NO_PREFS` set, so that it neither reads nor writes the user's VectorCraft preferences, and a
//! picture traces the same for everyone. Its standard input is written from a thread of its own while its
//! standard error is read, so that neither waits on the other.
//!
//! **A kept colour** (`REQ-AUTO-160`). Image Trace chooses which of the library's colours a picture gets,
//! and gives none a place among them: its `keepColors` traces only the listed colours and leaves the rest of
//! the picture out. So a picture with kept threads ([`Tracing::kept`]) is traced twice ([`picture`]). First
//! in at most `--colours` less the kept threads, 2 at the least, chosen from the machine's threads without
//! the kept ones: the threads the tracer would give the rest of the picture. Then again in a library of
//! exactly those threads and the kept ones, in at most `--colours` of them, and that trace is the design.
//! With as many kept threads as `--colours`, the first trace is left out. A kept thread the second trace
//! leaves out, since no pixel lies nearer it than the other threads, is said with `SC-W0822`. The threads
//! the first trace used are its shapes' fills, read as `stitch plan` reads the document, in loops over what
//! that reading's budget already bounded.
//!
//! **A dropped colour** (`REQ-AUTO-161`). `--drop-colour` leaves threads out of every trace's library
//! ([`Tracing::dropped`]), so that their pixels take the nearest of the others. A picture with dropped
//! threads and none kept is traced once, in the library without them. A merge of 2 traced colours into one
//! thread is not built: measured, leaving out the nearer of the 2 nearest threads is the tracer's own trace
//! in one colour fewer.
//!
//! **What fails.** A program that can't be started is `SC-E0815`, and one that fails or saves no document is
//! `SC-E0816`, which ends with the end of what it said on standard error, each character a terminal would
//! act on escaped ([`printable`]).

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::{Value, json};
use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Mm};
use stitchcraft_engine::design::Reading;
use stitchcraft_plan::MachineProfile;
use stitchcraft_plan::palette::PaletteEntry;

use crate::cli::TracingArgs;
use crate::colours::{self, Unchosen};
use crate::commands::{palette, printable};
use crate::files::{self, Scratch};

/// Image Trace's Paths for a picture traced with the fewest points ([`Fitting::Fewest`]): its curves follow
/// the pixels least closely, within about 0.8 pixels.
pub const PATHS: u32 = 0;
/// Image Trace's Corners for a picture traced with the fewest points ([`Fitting::Fewest`]): a turn of 90° or
/// more makes a corner, so a staircase of pixels is fitted as a line.
pub const CORNERS: u32 = 0;

/// How Image Trace fits the outlines of a picture `stitch plan` traces ([the module](self)).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fitting {
    /// With the fewest points that keep the picture: Paths [`PATHS`] and Corners [`CORNERS`], as
    /// `--fewer-points` asks.
    Fewest,
    /// With Image Trace's defaults: no fitting option is sent. Without `--fewer-points`.
    Defaults,
}

/// The extensions of the pictures `stitch plan` traces, in any case.
pub const EXTENSIONS: &[&str] = &["png", "jpg", "jpeg"];
/// How many colours Image Trace keeps when `--colours` doesn't say: its own default.
pub const COLOURS: u32 = 6;
/// How far in from each side of the machine's hoop a picture given no size is fitted: as far as an automatic
/// satin column along the picture's edge sews past it ([the module](self)).
pub const HOOP_MARGIN: Mm = Mm::from_tenths(5);
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
/// `profile`: fitted inside a square `size` across, or else inside the machine's hoop [`HOOP_MARGIN`] in from
/// each side, and traced in at most `colours` of them with the `fitting` ([the module](self)). `file.place`
/// fits the picture inside its `rect`, `[x, y, width, height]` in points.
pub fn steps(name: &str, bytes: &[u8], profile: &MachineProfile, size: Option<Mm>, colours: u32, fitting: Fitting) -> Value {
    steps_in(&palette::gpl(profile), name, bytes, profile, size, colours, fitting)
}

/// [`steps`], with the swatch library `library`, a GIMP palette of the machine's threads, in place of all
/// of them.
#[allow(clippy::too_many_arguments)]
fn steps_in(library: &str, name: &str, bytes: &[u8], profile: &MachineProfile, size: Option<Mm>, colours: u32, fitting: Fitting) -> Value {
    let rect = match size {
        Some(size) => [0.0, 0.0, size.to_points(), size.to_points()],
        None => {
            let (margin, hoop) = (HOOP_MARGIN.to_points(), profile.hoop);
            [margin, margin, hoop.width.to_points() - 2.0 * margin, hoop.height.to_points() - 2.0 * margin]
        }
    };
    let mut trace = json!({
        "mode": "color",
        "palette": "documentLibrary",
        "library": format!("loaded/{LIBRARY}"),
        "colors": colours,
        "method": "abutting",
    });
    if let (Fitting::Fewest, Some(options)) = (fitting, trace.as_object_mut()) {
        options.insert("paths".to_string(), json!(PATHS));
        options.insert("corners".to_string(), json!(CORNERS));
    }
    json!([
        {"command": "swatch.library.load", "params": {"name": LIBRARY, "data": library}},
        {"command": "file.place", "params": {"name": name, "dataBase64": stitchcraft_vectorcraft::base64(bytes), "rect": rect}},
        {"command": "imageTrace.make", "params": {"params": trace}},
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

/// How a picture is traced, as `--vectorcraft-cli`, `--size`, `--colours`, `--fewer-points` and
/// `--keep-colour` ask ([the module](self)).
#[derive(Clone, Debug, PartialEq)]
pub struct Tracing<'a> {
    /// The program that traces, or else [`PROGRAM`] on the PATH.
    pub program: Option<&'a Path>,
    /// The picture's longer side, or else the hoop's.
    pub size: Option<Mm>,
    /// How many colours at the most.
    pub colours: u32,
    /// How its outlines are fitted.
    pub fitting: Fitting,
    /// The machine's threads the picture keeps, each once, in the order named.
    pub kept: Vec<&'static PaletteEntry>,
    /// The machine's threads the picture is traced without, each once, in the order named.
    pub dropped: Vec<&'static PaletteEntry>,
}

impl<'a> Tracing<'a> {
    /// The tracing `args` ask for the picture `bytes`, traced for `profile`; or the usage error that says
    /// why it can't be, before VectorCraft runs: a kept colour that is no thread, or more kept threads than
    /// colours.
    pub fn asked(args: &'a TracingArgs, bytes: &[u8], profile: &MachineProfile) -> Result<Tracing<'a>, String> {
        const KEEP: &str = "--keep-colour";
        const DROP: &str = "--drop-colour";
        let palette = profile.palette.palette();
        let colours = args.colours.unwrap_or(COLOURS);
        let read = |given: &[colours::Colour], option: &str| {
            colours::chosen(given, option, palette, &mut colours::pixels(bytes, option)).map_err(|Unchosen::Usage(why)| why)
        };
        let kept = read(&args.keep_colour, KEEP)?;
        let dropped = read(&args.drop_colour, DROP)?;
        if let Some(both) = kept.iter().find(|entry| held(&dropped, entry)) {
            return Err(format!("{KEEP} and {DROP} both name the thread {}: keep it or leave it out", both.name));
        }
        if u32::try_from(kept.len()).map_or(true, |count| count > colours) {
            return Err(format!("{KEEP} keeps {} threads, more than the {colours} colours of --colours", kept.len()));
        }
        if colours::threads(palette).filter(|entry| !held(&dropped, entry)).nth(1).is_none() {
            return Err(format!("{DROP} leaves {} with fewer than 2 threads to trace in", palette.name));
        }
        let fitting = if args.fewer_points { Fitting::Fewest } else { Fitting::Defaults };
        Ok(Tracing { program: args.vectorcraft_cli.as_deref(), size: args.size, colours, fitting, kept, dropped })
    }
}

/// The document VectorCraft's command line traces the picture `bytes`, named `name`, into for `profile`, as
/// `tracing` asks, into the folder `scratch`, with what was said of the threads kept (`SC-W0822`); or why it
/// wasn't traced, as [`traced`] says. With kept threads it is traced twice ([the module](self)).
pub fn picture(
    name: &str,
    bytes: &[u8],
    profile: &MachineProfile,
    tracing: &Tracing<'_>,
    scratch: &Scratch,
) -> Result<(Vec<u8>, Vec<Diagnostic>), Diagnostic> {
    let Tracing { program, size, colours, fitting, ref kept, ref dropped } = *tracing;
    if kept.is_empty() && dropped.is_empty() {
        return traced(program, name, &steps(name, bytes, profile, size, colours, fitting), scratch).map(|document| (document, Vec::new()));
    }
    let threads: Vec<&'static PaletteEntry> = colours::threads(profile.palette.palette()).filter(|entry| !held(dropped, entry)).collect();
    if kept.is_empty() {
        let once = steps_in(&palette::gpl_of(profile, &threads), name, bytes, profile, size, colours, fitting);
        return traced(program, name, &once, scratch).map(|document| (document, Vec::new()));
    }
    let others: Vec<&'static PaletteEntry> = threads.iter().copied().filter(|entry| !held(kept, entry)).collect();
    let rest = colours.saturating_sub(u32::try_from(kept.len()).unwrap_or(u32::MAX));
    let mut library: Vec<&'static PaletteEntry> = kept.clone();
    if rest > 0 {
        let first = steps_in(&palette::gpl_of(profile, &others), name, bytes, profile, size, rest.max(2), fitting);
        let document = traced(program, name, &first, scratch)?;
        library.extend(used(&document, &others)?);
    }
    let second = steps_in(&palette::gpl_of(profile, &library), name, bytes, profile, size, colours, fitting);
    let document = traced(program, name, &second, scratch)?;
    let traced_in = used(&document, kept)?;
    let said = kept.iter().filter(|entry| !held(&traced_in, entry)).map(|entry| untraced(entry)).collect();
    Ok((document, said))
}

/// Whether `entry` is one of `entries`.
fn held(entries: &[&PaletteEntry], entry: &PaletteEntry) -> bool {
    entries.iter().any(|held| held.index == entry.index)
}

/// The threads of `library` that the traced `document`'s shapes are filled in, in the library's order.
fn used(document: &[u8], library: &[&'static PaletteEntry]) -> Result<Vec<&'static PaletteEntry>, Diagnostic> {
    let read = stitchcraft_vectorcraft::read_with(document, &Budget::DEFAULT, Reading { auto: true, ..Reading::default() })?;
    let elements = read.design.elements();
    Ok(library.iter().copied().filter(|entry| elements.iter().any(|element| element.thread.color == entry.color)).collect())
}

/// `SC-W0822`: the kept thread `entry` isn't in the picture's trace.
fn untraced(entry: &PaletteEntry) -> Diagnostic {
    Diagnostic::new(
        Code::VcKeptThreadUntraced,
        format!(
            "The thread {} ({}), kept with --keep-colour, isn't in the picture's trace: no part of the picture is nearer its colour than \
             the other threads'.",
            entry.name, entry.color
        ),
    )
    .with_fix(Fix::Hint("Keep a colour the picture has, such as one of its pixels as X,Y, or trace it in more --colours.".to_string()))
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
    fn req_auto_161_dropping_every_thread_but_one_or_keeping_a_dropped_one_is_a_usage_error() {
        let thread = |name: &str| crate::colours::Colour::Thread(name.to_string());
        // By colour: 2 threads are named Flesh Pink, and a name is the first of them.
        let all: Vec<crate::colours::Colour> =
            crate::colours::threads(REFERENCE.palette.palette()).map(|entry| crate::colours::Colour::Rgb(entry.color)).collect();
        let (one, two) = (all.get(1..).unwrap().to_vec(), all.get(2..).unwrap().to_vec());
        let args = TracingArgs { drop_colour: one, ..TracingArgs::default() };
        assert_eq!(Tracing::asked(&args, b"", REFERENCE).unwrap_err(), "--drop-colour leaves Brother PEC with fewer than 2 threads to trace in");
        let args = TracingArgs { drop_colour: two, ..TracingArgs::default() };
        assert_eq!(Tracing::asked(&args, b"", REFERENCE).unwrap().dropped.len(), all.len() - 2);
        let args = TracingArgs { keep_colour: vec![thread("white")], drop_colour: vec![thread("Red"), thread("White")], ..TracingArgs::default() };
        assert_eq!(
            Tracing::asked(&args, b"", REFERENCE).unwrap_err(),
            "--keep-colour and --drop-colour both name the thread White: keep it or leave it out"
        );
        let args = TracingArgs { colours: Some(2), keep_colour: vec![thread("Red"), thread("Blue")], ..TracingArgs::default() };
        assert_eq!(Tracing::asked(&args, b"", REFERENCE).unwrap().kept.len(), 2);
    }

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
        let given = steps("badge.png", picture, REFERENCE, None, COLOURS, Fitting::Defaults);
        let rect = &given[1]["params"]["rect"];
        let expected = json!([
            {"command": "swatch.library.load", "params": {"name": "threads.gpl", "data": palette::gpl(REFERENCE)}},
            {"command": "file.place", "params": {"name": "badge.png", "dataBase64": stitchcraft_vectorcraft::base64(picture), "rect": rect}},
            {"command": "imageTrace.make", "params": {"params": {"mode": "color", "palette": "documentLibrary", "library": "loaded/threads.gpl", "colors": 6, "method": "abutting"}}},
            {"command": "imageTrace.expand"},
        ]);
        assert_eq!(given, expected);
        // `[x, y, width, height]` in points: inside the reference machine's hoop, 130 × 180 mm, 0.5 mm in from
        // each side.
        let mm: Vec<f64> = rect.as_array().unwrap().iter().map(|points| Mm::from_points(points.as_f64().unwrap()).unwrap().get()).collect();
        assert!(mm.iter().zip([0.5, 0.5, 129.0, 179.0]).all(|(got, want)| (got - want).abs() < 1e-9), "{mm:?}");
        // A size fits it inside a square that size across, and another machine traces in its own threads.
        let sized = steps("photo.jpg", picture, &BROTHER_PE800_4X4, Some(Mm::new(80.0).unwrap()), 4, Fitting::Defaults);
        let points = Mm::new(80.0).unwrap().to_points();
        assert_eq!(sized[1]["params"]["rect"], json!([0.0, 0.0, points, points]));
        assert_eq!((&sized[0]["params"]["data"], &sized[2]["params"]["params"]["colors"]), (&json!(palette::gpl(&BROTHER_PE800_4X4)), &json!(4)));
        assert_eq!(sized[1]["params"]["name"], "photo.jpg");
    }

    #[test]
    fn req_cli_024_the_trace_fits_its_outlines_with_the_fewest_points_that_keep_the_picture() {
        // Paths 0 and Corners 0, and Noise and Snap Curves To Lines left at Image Trace's defaults.
        let picture = b"\xff\xd8\xff and the rest";
        let fewest = steps("photo.jpg", picture, REFERENCE, None, COLOURS, Fitting::Fewest);
        let trace = &fewest[2]["params"]["params"];
        let expected = json!({"mode": "color", "palette": "documentLibrary", "library": "loaded/threads.gpl", "colors": 6, "method": "abutting", "paths": 0, "corners": 0});
        assert_eq!(trace, &expected);
        // Only the trace's options differ from the steps with the defaults.
        let mut defaults = steps("photo.jpg", picture, REFERENCE, None, COLOURS, Fitting::Defaults);
        assert_eq!(defaults[2]["params"]["params"].as_object_mut().unwrap().len(), 5);
        defaults[2]["params"]["params"] = expected;
        assert_eq!(defaults, fewest);
    }

    #[test]
    fn req_cli_012_the_hoop_margin_is_as_far_as_an_automatic_satin_column_sews_past_a_pictures_edge() {
        // A satin column's pull compensation, from rails as far outside the shape as shapes still meet,
        // rounded up to a tenth of a millimetre. No shape is sewn after the column beyond the picture's edge,
        // so the column doesn't sew by the overlap past that rail (REQ-AUTO-029).
        let past = stitchcraft_testkit::designs::rules().auto_pull_compensation_mm.get() + stitchcraft_engine::auto::TOUCHING;
        assert_eq!(HOOP_MARGIN.get(), (past * 10.0).ceil() / 10.0, "{past}");
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
        // A long tail keeps its end, and one as long as is kept stays whole.
        let long = "x".repeat(SAID_CHARS) + "the end";
        let kept = said(long.as_bytes()).unwrap();
        assert!(kept.starts_with('…') && kept.ends_with("the end") && kept.chars().count() == SAID_CHARS + 1, "{kept}");
        let whole = "y".repeat(SAID_CHARS);
        assert_eq!(said(whole.as_bytes()), Some(whole));
    }
}
