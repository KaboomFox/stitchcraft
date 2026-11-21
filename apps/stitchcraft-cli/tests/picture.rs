//! `stitch plan picture.png` end to end, with this test program standing in for VectorCraft's command line
//! (`REQ-CLI-012`, `REQ-CLI-013`, `SC-E0816`), with `--subject` (`REQ-CLI-014`, `REQ-CLI-016`), and with kept colours (`REQ-AUTO-160`, `SC-W0822`).
//!
//! **Why a stand-in.** VectorCraft isn't installed where these tests run. The compatibility gate's `picture`
//! case runs the real one on every track, and here `stitch` is given this program as `--vectorcraft-cli`.
//! Run with [`STAND_IN`] in its environment, which `stitch` passes on, it does what `vectorcraft-cli run
//! --commands - --export FILE` does as far as `stitch` can tell. It reads the steps on standard input, keeps
//! the text it read with its arguments in the file the variable names, a line for each time it runs, since a
//! picture with kept colours is traced twice (`REQ-AUTO-160`), and saves to `FILE` a document
//! VectorCraft traced: the fixture `traced-badge.vectorcraft`, which VectorCraft 0.18.0 traced when the
//! `picture` case ran. With [`FAILS`] it fails as VectorCraft does, saying why, and with [`SAVES_NOTHING`] it
//! ends without saving.
//!
//! **Why a harness of its own.** A program that stands in for another must start as that one does, and the
//! standard test harness reads its arguments first. So `main` runs the tests itself, and prints for each the
//! line the standard harness prints (`test NAME ... ok`), which `cargo xtask conformance` reads. Like it, it
//! runs the tests whose names hold one of its arguments, and lists them with `--list`.
//!
//! **Models only.** The cases of [`MODELS`] cut a photo's subject out with the cut-out's models, which only
//! the models' job fetches (`docs/src/design/conformance.md` › Cases with the models). As the standard
//! harness does for an ignored test, `main` says each is ignored, with the reason "models only: …", unless
//! `--ignored` asks for them alone or `--include-ignored` for every test.
// Test code may unwrap and print. It runs `stitch` on files in folders of its own, and reads its environment.
#![allow(clippy::unwrap_used, clippy::panic, clippy::print_stdout, clippy::print_stderr, clippy::disallowed_methods)]

use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Output};

use serde_json::{Value, json};
use stitchcraft_cli::trace::{COLOURS, Fitting, steps};
use stitchcraft_core::Mm;
use stitchcraft_plan::profiles::{BROTHER_PE800_4X4, REFERENCE};
use stitchcraft_testkit::temp::TempDir;

/// Set, the program is VectorCraft's stand-in, and keeps what it is given in the file the variable names.
const STAND_IN: &str = "STITCHCRAFT_TEST_VECTORCRAFT_KEEPS";
/// Set, the stand-in fails as VectorCraft does on a file it doesn't read as a picture.
const FAILS: &str = "STITCHCRAFT_TEST_VECTORCRAFT_FAILS";
/// Set, the stand-in ends well without saving a document.
const SAVES_NOTHING: &str = "STITCHCRAFT_TEST_VECTORCRAFT_SAVES_NOTHING";
/// The document the stand-in saves: a badge VectorCraft traced.
const TRACED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fixtures/vectorcraft/traced-badge.vectorcraft");
/// What the stand-in says when it fails: VectorCraft's own words, and a colour code a terminal would act on.
const SAID: &str = "vectorcraft-cli: step 1 (file.place): not an image VectorCraft reads (see document.formats)\x1b[0m";
/// A picture, as far as the stand-in reads one: it reads none.
const PICTURE: &[u8] = b"\x89PNG\r\n\x1a\n a badge";

/// The tests, by name.
const TESTS: &[(&str, fn())] = &[
    ("req_cli_025_stitch_expand_traces_a_picture_first", req_cli_025_stitch_expand_traces_a_picture_first),
    (
        "req_cli_012_stitch_plan_traces_a_picture_with_vectorcraft_s_command_line_and_sews_it_digitized_automatically",
        req_cli_012_stitch_plan_traces_a_picture_with_vectorcraft_s_command_line_and_sews_it_digitized_automatically,
    ),
    (
        "req_cli_012_the_picture_is_traced_for_the_machine_at_the_size_and_in_the_colours_asked",
        req_cli_012_the_picture_is_traced_for_the_machine_at_the_size_and_in_the_colours_asked,
    ),
    (
        "req_cli_024_fewer_points_asks_image_trace_for_its_fewest_points_in_every_style_and_only_then",
        req_cli_024_fewer_points_asks_image_trace_for_its_fewest_points_in_every_style_and_only_then,
    ),
    (
        "diag_sc_e0816_what_vectorcraft_s_command_line_said_when_it_failed_ends_the_message",
        diag_sc_e0816_what_vectorcraft_s_command_line_said_when_it_failed_ends_the_message,
    ),
    ("req_cli_013_a_document_vectorcraft_doesn_t_save_is_said", req_cli_013_a_document_vectorcraft_doesn_t_save_is_said),
    (
        "req_vc_033_the_traced_document_names_each_shape_and_changes_nothing_else",
        req_vc_033_the_traced_document_names_each_shape_and_changes_nothing_else,
    ),
    ("req_auto_160_a_kept_colour_traces_the_picture_twice", req_auto_160_a_kept_colour_traces_the_picture_twice),
    ("diag_sc_w0822_a_kept_thread_the_trace_leaves_out_is_named", diag_sc_w0822_a_kept_thread_the_trace_leaves_out_is_named),
    (
        "req_cli_016_models_that_aren_t_ready_are_said_with_each_file_and_the_folder_and_only_the_report_is_written",
        req_cli_016_models_that_aren_t_ready_are_said_with_each_file_and_the_folder_and_only_the_report_is_written,
    ),
];

/// The tests that need the cut-out's models, by name: models only.
const MODELS: &[(&str, fn())] = &[(
    "req_cli_014_stitch_plan_subject_gives_vectorcraft_the_photos_cut_out_and_writes_it_with_the_rest",
    req_cli_014_stitch_plan_subject_gives_vectorcraft_the_photos_cut_out_and_writes_it_with_the_rest,
)];
/// Why the standard harness's way leaves [`MODELS`] out.
const MODELS_ONLY: &str = "models only: the models job runs tests/picture.rs";

fn main() -> ExitCode {
    if let Some(keeps) = std::env::var_os(STAND_IN) {
        return stand_in(Path::new(&keeps));
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--list") {
        for (name, _) in TESTS.iter().chain(MODELS) {
            println!("{name}: test");
        }
        return ExitCode::SUCCESS;
    }
    let given = |flag: &str| args.iter().any(|arg| arg == flag);
    let (only_ignored, with_ignored) = (given("--ignored"), given("--include-ignored"));
    let mut run: Vec<&(&str, fn())> = if only_ignored { Vec::new() } else { TESTS.iter().filter(|(name, _)| chosen(name, &args)).collect() };
    let models: Vec<&(&str, fn())> = MODELS.iter().filter(|(name, _)| chosen(name, &args)).collect();
    let ignored = if only_ignored || with_ignored {
        run.extend(&models);
        0
    } else {
        models.len()
    };
    println!("\nrunning {} tests", run.len() + ignored);
    if !(only_ignored || with_ignored) {
        for (name, _) in &models {
            println!("test {name} ... ignored, {MODELS_ONLY}");
        }
    }
    let failed: Vec<&str> = run
        .iter()
        .filter_map(|(name, test)| {
            let passed = std::panic::catch_unwind(test).is_ok();
            println!("test {name} ... {}", if passed { "ok" } else { "FAILED" });
            (!passed).then_some(*name)
        })
        .collect();
    let verdict = if failed.is_empty() { "ok" } else { "FAILED" };
    println!("\ntest result: {verdict}. {} passed; {} failed; {ignored} ignored\n", run.len() - failed.len(), failed.len());
    if failed.is_empty() { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Whether the standard harness would choose the test `name` given `args`: those whose names hold one of its
/// filters, or exactly match one with `--exact`, and not one `--skip` names. The values of the options that
/// take one are not filters.
fn chosen(name: &str, args: &[String]) -> bool {
    let mut filters = Vec::new();
    let mut skips = Vec::new();
    let mut words = args.iter();
    while let Some(word) = words.next() {
        match word.as_str() {
            "--skip" => skips.extend(words.next()),
            "--test-threads" | "--format" | "--logfile" | "--color" | "--shuffle-seed" | "-Z" => {
                words.next();
            }
            option if option.starts_with('-') => {}
            filter => filters.push(filter),
        }
    }
    let exact = args.iter().any(|arg| arg == "--exact");
    let matches = |filter: &str| if exact { name == filter } else { name.contains(filter) };
    (filters.is_empty() || filters.iter().any(|filter| matches(filter))) && !skips.iter().any(|skip| matches(skip))
}

/// What VectorCraft's command line does as far as `stitch` can tell, keeping what it was given in `keeps`.
fn stand_in(keeps: &Path) -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut steps = String::new();
    std::io::stdin().read_to_string(&mut steps).unwrap();
    let no_prefs = std::env::var("VECTORCRAFT_NO_PREFS").ok();
    // The text it read, as VectorCraft reads it: a number parsed and written again may differ in its last digit.
    let kept = json!({"args": args, "no_prefs": no_prefs, "steps": steps});
    let mut file = std::fs::OpenOptions::new().create(true).append(true).open(keeps).unwrap();
    std::io::Write::write_all(&mut file, format!("{kept}\n").as_bytes()).unwrap();
    if std::env::var_os(FAILS).is_some() {
        eprintln!("{SAID}");
        return ExitCode::from(1);
    }
    if std::env::var_os(SAVES_NOTHING).is_none() {
        let saved = args.iter().skip_while(|arg| *arg != "--export").nth(1).unwrap();
        std::fs::copy(TRACED, saved).unwrap();
    }
    ExitCode::SUCCESS
}

/// `stitch` run in `dir` with `args`, this program standing in for VectorCraft's command line with
/// `behaviour` set, and what it was given.
fn stitch(dir: &Path, args: &[&str], behaviour: Option<&str>) -> (Output, Option<Value>) {
    stitch_with(dir, args, behaviour, &[])
}

/// [`stitch`], with the variables `vars` set as well.
fn stitch_with(dir: &Path, args: &[&str], behaviour: Option<&str>, vars: &[(&str, &Path)]) -> (Output, Option<Value>) {
    let (out, mut calls) = stitch_calls(dir, args, behaviour, vars);
    (out, calls.pop())
}

/// [`stitch_with`], with what the stand-in was given each time it ran, in order.
fn stitch_calls(dir: &Path, args: &[&str], behaviour: Option<&str>, vars: &[(&str, &Path)]) -> (Output, Vec<Value>) {
    let keeps = dir.join("kept.json");
    let _ = std::fs::remove_file(&keeps);
    let mut command = Command::new(env!("CARGO_BIN_EXE_stitch"));
    command.args(args).arg("--vectorcraft-cli").arg(std::env::current_exe().unwrap()).current_dir(dir);
    command.env(STAND_IN, &keeps).env_remove(FAILS).env_remove(SAVES_NOTHING).envs(vars.iter().copied());
    if let Some(behaviour) = behaviour {
        command.env(behaviour, "1");
    }
    let out = command.output().unwrap();
    let kept = std::fs::read_to_string(&keeps).unwrap_or_default();
    (out, kept.lines().map(|line| serde_json::from_str(line).unwrap()).collect())
}

/// A new folder for one test, with [`PICTURE`] in it as `name`.
fn folder(test: &str, name: &str) -> TempDir {
    let dir = TempDir::new(&format!("picture-{test}"));
    std::fs::write(dir.join(name), PICTURE).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

/// The names of the files and folders in `dir`, but `kept.json`, in order.
fn left(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.retain(|name| name != "kept.json");
    names.sort();
    names
}

fn req_cli_012_stitch_plan_traces_a_picture_with_vectorcraft_s_command_line_and_sews_it_digitized_automatically() {
    let dir = folder("traced", "badge.png");
    let args = ["plan", "badge.png", "-o", "badge.pes", "--traced", "traced.vectorcraft", "--report", "badge.json"];
    let (out, kept) = stitch(dir.path(), &args, None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    // VectorCraft runs the steps from standard input, without the user's preferences, and saves the document
    // into a folder of the command's own beside the machine file, which is gone.
    let kept = kept.unwrap();
    assert_eq!(kept["steps"], steps("badge.png", PICTURE, REFERENCE, None, COLOURS, Fitting::Defaults).to_string());
    assert_eq!(kept["no_prefs"], "1");
    let given: Vec<&str> = kept["args"].as_array().unwrap().iter().map(|arg| arg.as_str().unwrap()).collect();
    assert_eq!(given[..4], ["run", "--commands", "-", "--export"]);
    let saved = PathBuf::from(given[4]);
    let scratch = saved.parent().unwrap().file_name().unwrap().to_str().unwrap().to_string();
    assert!(scratch.starts_with("badge.pes.") && scratch.ends_with(".partial") && saved.ends_with("traced.vectorcraft"), "{}", saved.display());
    assert_eq!(left(dir.path()), ["badge.json", "badge.pes", "badge.png", "traced.vectorcraft"]);
    // The document VectorCraft saved is written, with its shapes' names (`REQ-VC-033`, which checks that
    // nothing else changes), and sews digitized automatically, as `stitch plan --auto` sews it under the
    // picture's name.
    let traced = std::fs::read(TRACED).unwrap();
    let written = std::fs::read(dir.join("traced.vectorcraft")).unwrap();
    assert!(written.len() > traced.len() && String::from_utf8_lossy(&written).contains(r#""name": "Fill 1, Red","#));
    let again = TempDir::new("picture-traced-again");
    std::fs::write(again.join("badge.vectorcraft"), &traced).unwrap();
    let planned = Command::new(env!("CARGO_BIN_EXE_stitch"))
        .args(["plan", "badge.vectorcraft", "--auto", "-o", "badge.pes"])
        .current_dir(again.path())
        .output()
        .unwrap();
    assert_eq!(planned.status.code(), Some(0), "{}", text(&planned.stderr));
    assert_eq!(std::fs::read(dir.join("badge.pes")).unwrap(), std::fs::read(again.join("badge.pes")).unwrap());
    assert_eq!(text(&out.stderr), text(&planned.stderr));
    assert!(text(&out.stderr).contains("warning SC-W0906 ("), "the white round the badge is left out: {}", text(&out.stderr));
    let stdout = text(&out.stdout);
    assert!(stdout.starts_with("badge.png\n") && stdout.ends_with("  report    badge.json\n  traced    traced.vectorcraft\n"), "{stdout}");
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("badge.json")).unwrap()).unwrap();
    assert_eq!((&report["design"], &report["profile"]), (&json!("badge.png"), &json!(REFERENCE.id)));
}

/// What a run of the stand-in traced in: the names of the swatch library's threads, and Image Trace's
/// colour count.
fn traced_in(call: &Value) -> (Vec<String>, u64) {
    let steps: Value = serde_json::from_str(call["steps"].as_str().unwrap()).unwrap();
    let library = steps[0]["params"]["data"].as_str().unwrap();
    let names = library.lines().skip(4).map(|line| line.split_once('\t').unwrap().1.to_string()).collect();
    (names, steps[2]["params"]["params"]["colors"].as_u64().unwrap())
}

fn names(names: &[&str]) -> Vec<String> {
    names.iter().map(|name| name.to_string()).collect()
}

fn req_auto_160_a_kept_colour_traces_the_picture_twice() {
    let threads = stitchcraft_cli::colours::threads(REFERENCE.palette.palette()).count();
    let dir = folder("keep", "badge.png");
    // First in the threads but the kept one, in one colour fewer; then in the threads the badge was traced
    // in, red, blue, gold and white, and the kept one, in the colours asked.
    let (out, calls) = stitch_calls(dir.path(), &["plan", "badge.png", "-o", "badge.pes", "--keep-colour", "red"], None, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(calls.len(), 2);
    let (first, colours) = traced_in(&calls[0]);
    assert_eq!((first.len(), colours), (threads - 1, u64::from(COLOURS) - 1));
    assert!(!first.contains(&"Red".to_string()));
    assert_eq!(traced_in(&calls[1]), (names(&["Blue", "Red", "Deep Gold", "White"]), u64::from(COLOURS)));
    assert!(!text(&out.stderr).contains("SC-W0822"), "{}", text(&out.stderr));
    // The first trace is in 2 colours at the least, and with as many kept threads as colours there is none.
    let three = ["plan", "badge.png", "-o", "b.pes", "--colours", "3", "--keep-colour", "Red", "--keep-colour", "#0a55a3"];
    let (_, calls) = stitch_calls(dir.path(), &three, None, &[]);
    assert_eq!((traced_in(&calls[0]).1, traced_in(&calls[1]).1), (2, 3));
    let two = ["plan", "badge.png", "-o", "c.pes", "--colours", "2", "--keep-colour", "Red", "--keep-colour", "Blue"];
    let (out, calls) = stitch_calls(dir.path(), &two, None, &[]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(calls.iter().map(traced_in).collect::<Vec<_>>(), [(names(&["Blue", "Red"]), 2)]);
    // A mistake is a usage error before VectorCraft runs: for `stitch expand` too.
    let (out, calls) = stitch_calls(dir.path(), &["expand", "badge.png", "-o", "badge.svg", "--keep-colour", "Redd"], None, &[]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out.stderr));
    let said = text(&out.stderr);
    assert!(said.starts_with("stitch: --keep-colour Redd: Brother PEC has no thread of that name. The nearest names are Red"), "{said}");
    assert!(calls.is_empty());
}

fn diag_sc_w0822_a_kept_thread_the_trace_leaves_out_is_named() {
    // The stand-in's badge has no fuchsia, as a picture without a pixel nearer it than the other threads.
    let warning = "warning SC-W0822: The thread Dark Fuchsia (#c70156), kept with --keep-colour, isn't in the picture's trace: no part of the \
                   picture is nearer its colour than the other threads'.\n  hint: Keep a colour the picture has, such as one of its pixels as \
                   X,Y, or trace it in more --colours.\n";
    let dir = folder("keep-untraced", "badge.png");
    for (command, output) in [("plan", "badge.pes"), ("expand", "badge.svg")] {
        let (out, calls) = stitch_calls(dir.path(), &[command, "badge.png", "-o", output, "--keep-colour", "Dark Fuchsia"], None, &[]);
        assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
        assert_eq!(calls.len(), 2);
        assert!(text(&out.stderr).starts_with(warning), "{command}: {}", text(&out.stderr));
    }
}

fn req_cli_025_stitch_expand_traces_a_picture_first() {
    let dir = folder("expand", "badge.png");
    let args = ["expand", "badge.png", "-o", "badge.vectorcraft", "--traced", "traced.vectorcraft"];
    let (out, kept) = stitch(dir.path(), &args, None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert!(kept.is_some());
    // The traced document is written as VectorCraft saved it, and the expanded one is it with its parts.
    assert_eq!(std::fs::read(dir.join("traced.vectorcraft")).unwrap(), std::fs::read(TRACED).unwrap());
    let expanded = text(&std::fs::read(dir.join("badge.vectorcraft")).unwrap());
    assert!(expanded.contains(r#""name":"Expanded embroidery""#), "{expanded}");
    assert!(text(&out.stdout).starts_with("badge.png · expanded into 7 parts → badge.vectorcraft"), "{}", text(&out.stdout));
    // When VectorCraft fails, nothing is written, and what it said is.
    let failed = folder("expand-fails", "badge.png");
    let (out, _) = stitch(failed.path(), &["expand", "badge.png", "-o", "badge.svg"], Some(FAILS));
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("error SC-E0816"), "{}", text(&out.stderr));
    assert_eq!(left(failed.path()), ["badge.png"]);
}

fn req_vc_033_the_traced_document_names_each_shape_and_changes_nothing_else() {
    // `--traced` writes each shape's name into its node (docs/src/design/image-to-embroidery.md › Names of
    // shapes), which the Layers panel shows and its Search All finds.
    let dir = folder("named", "badge.png");
    let args = ["plan", "badge.png", "-o", "badge.pes", "--traced", "traced.vectorcraft"];
    let (out, _) = stitch(dir.path(), &args, None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let mut written: Value = serde_json::from_slice(&std::fs::read(dir.join("traced.vectorcraft")).unwrap()).unwrap();
    let shapes = written.pointer_mut("/document/layers/0/kind/children/0/kind/children").unwrap().as_array_mut().unwrap();
    let names: Vec<(u64, String)> = shapes
        .iter_mut()
        .map(|node| {
            (node["id"].as_u64().unwrap(), node.as_object_mut().unwrap().remove("name").map(|n| n.as_str().unwrap().to_string()).unwrap_or_default())
        })
        .collect();
    assert_eq!(names, [(3, "Left out 1, White".into()), (6, "Fill 1, Red".into()), (9, "Fill 1, Blue".into()), (12, "Fill 1, Deep Gold".into())]);
    // Without the names, it is the document VectorCraft saved.
    let traced: Value = serde_json::from_slice(&std::fs::read(TRACED).unwrap()).unwrap();
    assert_eq!(written, traced);
    // The named document sews the same stitches, and its notices name the shapes alike.
    // Under the picture's name, which the machine file keeps.
    let again = TempDir::new("picture-named-again");
    std::fs::copy(dir.join("traced.vectorcraft"), again.join("badge.vectorcraft")).unwrap();
    let planned = Command::new(env!("CARGO_BIN_EXE_stitch"))
        .args(["plan", "badge.vectorcraft", "--auto", "-o", "badge.pes"])
        .current_dir(again.path())
        .output()
        .unwrap();
    assert_eq!(planned.status.code(), Some(0), "{}", text(&planned.stderr));
    assert_eq!(std::fs::read(dir.join("badge.pes")).unwrap(), std::fs::read(again.join("badge.pes")).unwrap());
    assert_eq!(text(&out.stderr), text(&planned.stderr));
    assert!(text(&out.stderr).contains(r#"info SC-I0912 ("Fill 1, Red", vc:6:fill): "#), "{}", text(&out.stderr));
}

fn req_cli_012_the_picture_is_traced_for_the_machine_at_the_size_and_in_the_colours_asked() {
    let dir = folder("asked", "photo.JPG");
    let args = ["plan", "photo.JPG", "-o", "photo.pes", "--profile", BROTHER_PE800_4X4.id, "--size", "80", "--colours", "4"];
    let (out, kept) = stitch(dir.path(), &args, None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    assert_eq!(
        kept.unwrap()["steps"],
        steps("photo.JPG", PICTURE, &BROTHER_PE800_4X4, Some(Mm::new(80.0).unwrap()), 4, Fitting::Defaults).to_string()
    );
    assert!(text(&out.stdout).contains(&format!("  profile   {} (", BROTHER_PE800_4X4.id)), "{}", text(&out.stdout));
}

fn req_cli_024_fewer_points_asks_image_trace_for_its_fewest_points_in_every_style_and_only_then() {
    // Without the option, in any style, Image Trace gets no fitting option: the request `stitch plan` sent
    // before the option was added.
    let main = json!({"mode": "color", "palette": "documentLibrary", "library": "loaded/threads.gpl", "colors": 6, "method": "abutting"});
    let fewest = json!({"mode": "color", "palette": "documentLibrary", "library": "loaded/threads.gpl", "colors": 6, "method": "abutting", "paths": 0, "corners": 0});
    for (style, at) in [(&[][..], "none"), (&["--style", "logo"][..], "logo"), (&["--style", "photo"][..], "photo")] {
        for (option, fitting, trace) in [(&[][..], Fitting::Defaults, &main), (&["--fewer-points"][..], Fitting::Fewest, &fewest)] {
            let dir = folder(&format!("fitting-{at}-{}", option.len()), "badge.png");
            let args = [&["plan", "badge.png", "-o", "badge.pes"][..], style, option].concat();
            let (out, kept) = stitch(dir.path(), &args, None);
            assert_eq!(out.status.code(), Some(0), "{args:?}: {}", text(&out.stderr));
            let sent = kept.unwrap()["steps"].as_str().unwrap().to_string();
            assert_eq!(sent, steps("badge.png", PICTURE, REFERENCE, None, COLOURS, fitting).to_string(), "{args:?}");
            let sent: Value = serde_json::from_str(&sent).unwrap();
            assert_eq!(&sent[2]["params"]["params"], trace, "{args:?}");
        }
    }
}

fn diag_sc_e0816_what_vectorcraft_s_command_line_said_when_it_failed_ends_the_message() {
    let dir = folder("fails", "badge.png");
    let (out, kept) = stitch(dir.path(), &["plan", "badge.png", "-o", "badge.pes", "--report", "badge.json"], Some(FAILS));
    assert!(kept.is_some(), "the stand-in ran");
    assert_eq!(out.status.code(), Some(1));
    assert_eq!(
        text(&out.stderr),
        "error SC-E0816: VectorCraft's command line didn't trace badge.png: it ended with status 1, and said: vectorcraft-cli: step 1 \
         (file.place): not an image VectorCraft reads (see document.formats)\\u{1b}[0m\n  hint: Check that VectorCraft places the picture, \
         or trace it with Image Trace in VectorCraft and plan the document you save.\n  more: stitch explain SC-E0816\nnothing was written\n"
    );
    assert_eq!(left(dir.path()), ["badge.json", "badge.png"]);
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("badge.json")).unwrap()).unwrap();
    assert_eq!(report["diagnostics"][0]["code"], "SC-E0816");
}

fn req_cli_013_a_document_vectorcraft_doesn_t_save_is_said() {
    let dir = folder("saves-nothing", "badge.png");
    let (out, _) = stitch(dir.path(), &["plan", "badge.png", "-o", "badge.pes"], Some(SAVES_NOTHING));
    assert_eq!(out.status.code(), Some(1));
    let said = text(&out.stderr);
    assert!(said.starts_with("error SC-E0816: VectorCraft's command line didn't trace badge.png: it saved no document ("), "{said}");
    assert_eq!(left(dir.path()), ["badge.png"]);
}

/// A fixture of the subject cut-out's, by name.
fn cutout_fixture(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../conformance/fixtures/cutout/{name}", env!("CARGO_MANIFEST_DIR")))
}

fn req_cli_016_models_that_aren_t_ready_are_said_with_each_file_and_the_folder_and_only_the_report_is_written() {
    let dir = TempDir::new("picture-models-not-ready");
    std::fs::copy(cutout_fixture("dog-running.jpg"), dir.join("dog.jpg")).unwrap();
    // The folder STITCHCRAFT_MODELS names is a file, so it can't be made, and nothing is fetched.
    let models = dir.join("models");
    std::fs::write(&models, b"not a folder").unwrap();
    let args = ["plan", "dog.jpg", "-o", "dog.pes", "--subject", "--report", "dog.json"];
    let (out, kept) = stitch_with(dir.path(), &args, None, &[(stitchcraft_cutout::pins::VARIABLE, &models)]);
    assert!(kept.is_none(), "VectorCraft's command line wasn't run");
    assert_eq!(out.status.code(), Some(1));
    let said = text(&out.stderr);
    assert_eq!(left(dir.path()), ["dog.jpg", "dog.json", "models"]);
    // A `stitch` built without the cut-out says so first, and fetches nothing (SC-E1207).
    if cfg!(not(feature = "cutout")) {
        assert!(said.starts_with("error SC-E1207: This stitch was built without the subject cut-out"), "{said}");
        return;
    }
    let start = format!("error SC-E1201: The subject cut-out's models are not ready in {}: the folder could not be made (", models.display());
    assert!(said.starts_with(&start), "{said}");
    // One reason for the 3 files, said once.
    assert_eq!(said.matches("the folder could not be made").count(), 1, "{said}");
    for pin in stitchcraft_cutout::pins::Pins::pinned().unwrap().all() {
        assert!(
            said.contains(&format!("{}, ", pin.file)) && said.contains(&format!(" bytes, SHA-256 {}, from {}", pin.sha256, pin.address)),
            "{said}"
        );
    }
    assert!(said.ends_with("  more: stitch explain SC-E1201\nnothing was written\n"), "{said}");
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("dog.json")).unwrap()).unwrap();
    assert_eq!((report["diagnostics"][0]["code"].as_str(), report.get("subject")), (Some("SC-E1201"), None));
}

fn req_cli_014_stitch_plan_subject_gives_vectorcraft_the_photos_cut_out_and_writes_it_with_the_rest() {
    // The models come from the folder STITCHCRAFT_MODELS names, which `stitch` inherits: the models job's.
    let dir = TempDir::new("picture-subject");
    std::fs::copy(cutout_fixture("dog-running.jpg"), dir.join("dog.jpg")).unwrap();
    let args = ["plan", "dog.jpg", "-o", "dog.pes", "--subject", "--not-subject", "640,720", "--cutout", "cut.png", "--report", "dog.json"];
    let (out, kept) = stitch(dir.path(), &args, None);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    // VectorCraft placed the cut-out, which `--cutout` wrote with the other files.
    let cut = std::fs::read(dir.join("cut.png")).unwrap();
    let steps: Value = serde_json::from_str(kept.unwrap()["steps"].as_str().unwrap()).unwrap();
    assert_eq!(steps[1]["params"]["dataBase64"], stitchcraft_vectorcraft::base64(&cut));
    assert_eq!(left(dir.path()), ["cut.png", "dog.jpg", "dog.json", "dog.pes"]);
    // The running dog, cropped round what was kept: 500 × 1,064 px of the photo's 1,200 × 1,786.
    let decoded = image::load_from_memory(&cut).unwrap();
    assert_eq!((decoded.width(), decoded.height()), (500, 1064));
    let said = text(&out.stderr);
    assert!(said.starts_with("info SC-I1206: The subject was cut out of the box 161.25,223.25,742.5,1289.26875, which U²-Net-p found: "), "{said}");
    assert!(text(&out.stdout).contains("  cutout    cut.png (500 × 1064 pixels)\n"), "{}", text(&out.stdout));
    // The report records it: the models by the sums they are pinned with, the box and the point.
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("dog.json")).unwrap()).unwrap();
    let pins = stitchcraft_cutout::pins::Pins::pinned().unwrap();
    let sums: Vec<&str> = report["subject"]["models"].as_array().unwrap().iter().map(|model| model["sha256"].as_str().unwrap()).collect();
    assert_eq!(sums, pins.all().map(|pin| pin.sha256.as_str()));
    assert_eq!((&report["subject"]["box_from"], &report["subject"]["not_subject"]), (&json!("u2netp"), &json!([[640.0, 720.0]])));
}
