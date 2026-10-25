//! Unit tests of bug-report bundles: writing one, replaying it, and what a panic leaves; `stitch` itself
//! is run on files in `tests/binary.rs`.

use serde_json::Value;

use stitchcraft_testkit::temp::TempDir;

use super::*;
use crate::cli::{Format, PlanArgs};

fn conformance(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../conformance/{name}", env!("CARGO_MANIFEST_DIR")))
}

/// A change to a bundle's JSON.
type Change = fn(&mut Value);

fn args(design: Option<PathBuf>) -> BugReportArgs {
    BugReportArgs { design, replay: None, output: None, profile: None, format: None, says: None, auto: false }
}

fn json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
}

fn replayed(bundle: &Path) -> Outcome {
    run(&BugReportArgs { replay: Some(bundle.to_path_buf()), ..args(None) })
}

/// `bundle` with `change` made to its JSON, written beside it as `name`.
fn changed(bundle: &Path, name: &str, change: impl FnOnce(&mut Value)) -> PathBuf {
    let mut value = json(bundle);
    change(&mut value);
    let path = bundle.with_file_name(name);
    std::fs::write(&path, serde_json::to_vec(&value).unwrap()).unwrap();
    path
}

#[test]
fn req_cli_001_a_bundle_reproduces_its_plan_byte_for_byte() {
    let dir = TempDir::new("bug-report");
    let (design, bundle) = (conformance("fixtures/svg/strokes.svg"), dir.join("strokes.bug-report.json"));
    let says = Some("the zigzag looks wrong".to_string());
    let out = run(&BugReportArgs { output: Some(bundle.clone()), says, ..args(Some(design.clone())) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert!(out.stdout.contains("  plan          556 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops\n"), "{}", out.stdout);
    assert!(out.stdout.ends_with(&format!("{HOLDS_THE_DESIGN}\n")));
    // The design itself, the profile, the format, and the digest of what `stitch plan` writes.
    let written = json(&bundle);
    assert_eq!(written["design"]["svg"].as_str().map(str::as_bytes), Some(std::fs::read(&design).unwrap().as_slice()));
    assert_eq!(
        (written["profile"].as_str(), written["format"].as_str(), written["says"].as_str()),
        (Some(profiles::REFERENCE.id), Some("pes"), Some("the zigzag looks wrong"))
    );
    let golden = std::fs::read(conformance("golden/plans/strokes.pes")).unwrap();
    assert_eq!(written["result"]["file"]["sha256"].as_str(), Some(hex(&Sha256::digest(&golden)).as_str()));
    // Replayed: all the same. The first line names the StitchCraft that wrote it, and its commit.
    let out = replayed(&bundle);
    assert_eq!(out.status, Status::Done, "{}", out.stdout);
    let built = match COMMIT {
        "" => env!("CARGO_PKG_VERSION").to_string(),
        commit => format!("{} (commit {commit})", env!("CARGO_PKG_VERSION")),
    };
    assert!(out.stdout.starts_with(&format!("{}: written by StitchCraft {built} on ", bundle.display())), "{}", out.stdout);
    assert!(out.stdout.contains("  plan          same: 556 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops (sha256 "), "{}", out.stdout);
    assert!(out.stdout.ends_with("  diagnostics   same: 2\nreproduced\n"), "{}", out.stdout);
    // A design changed in the bundle is not the one its results came from.
    let moved = changed(&bundle, "moved.json", |value| {
        let svg = value["design"]["svg"].as_str().unwrap().replace("M 10,35 H 25", "M 10,35 H 26");
        value["design"]["svg"] = Value::from(svg);
    });
    let out = replayed(&moved);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stdout.contains("  design        differs\n") && out.stdout.contains("  plan          differs\n"), "{}", out.stdout);
    assert!(out.stdout.ends_with("not reproduced\n"));
    // A changed result: only it differs.
    let other = changed(&bundle, "other.json", |value| value["result"]["diagnostics"][1] = Value::from("info SC-I0504: Something else."));
    let out = replayed(&other);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stdout.contains("  plan          same") && out.stdout.contains("    then only:  info SC-I0504: Something else.\n"), "{}", out.stdout);
}

#[test]
fn req_vc_008_a_bundle_holds_a_vectorcraft_document_and_reproduces_its_plan() {
    let dir = TempDir::new("bug-report");
    let (design, bundle) = (conformance("fixtures/vectorcraft/patch.vectorcraft"), dir.join("patch.bug-report.json"));
    let out = run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(design.clone())) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    let written = json(&bundle);
    assert_eq!(written["design"]["vectorcraft"].as_str().map(str::as_bytes), Some(std::fs::read(&design).unwrap().as_slice()));
    assert!(written["design"].get("svg").is_none() && written["result"]["plan"].is_object(), "{written}");
    let out = replayed(&bundle);
    assert!(out.status == Status::Done && out.stdout.ends_with("reproduced\n"), "{}", out.stdout);
    // VectorCraft keeps images and other data after the JSON, behind a marker, and StitchCraft reads none
    // of it: the bundle keeps the JSON alone, as text, and its digest.
    let text = std::fs::read(&design).unwrap();
    let (blobs, bundle) = (dir.join("blobs.vectorcraft"), dir.join("blobs.bug-report.json"));
    std::fs::write(&blobs, [&text[..], b"\n\0VCBLOBS\0\xff\xfe"].concat()).unwrap();
    assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(blobs)) }).status, Status::Done);
    let kept = &json(&bundle)["design"];
    assert_eq!(kept["vectorcraft"].as_str().map(str::as_bytes), Some(text.as_slice()), "{kept}");
    assert_eq!((kept.get("vectorcraft_hex"), kept["sha256"].as_str()), (None, Some(hex(&Sha256::digest(&text)).as_str())));
    assert!(replayed(&bundle).stdout.ends_with("\nreproduced\n"));
    // A compressed document, and one with a NUL byte that is not the marker's, are kept whole, in
    // hexadecimal.
    for (name, bytes) in [("packed.vectorcraft", b"\x1f\x8b\x08 not a stream".to_vec()), ("damaged.vectorcraft", [&text[..], b"\0\xff"].concat())] {
        let (design, bundle) = (dir.join(name), dir.join(format!("{name}.bug-report.json")));
        std::fs::write(&design, &bytes).unwrap();
        assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(design)) }).status, Status::Done);
        assert_eq!(json(&bundle)["design"]["vectorcraft_hex"].as_str(), Some(hex(&bytes).as_str()), "{name}");
        assert!(replayed(&bundle).stdout.ends_with("\nreproduced\n"), "{name}");
    }
    // The design is held once, under the field of its kind.
    let both = changed(&bundle, "both.json", |v| v["design"]["svg"] = Value::from("<svg/>"));
    assert!(replayed(&both).stderr.contains("must hold its design once"), "{}", replayed(&both).stderr);
}

#[test]
fn req_cli_005_a_replay_reads_a_bundle_larger_than_a_design() {
    let dir = TempDir::new("bug-report");
    // A bundle can hold a design of the most `stitch` reads, twice as large in hexadecimal, and what was
    // said of it: a replay reads it whole.
    let (design, bundle) = (conformance("fixtures/svg/strokes.svg"), dir.join("large.bug-report.json"));
    assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(design)) }).status, Status::Done);
    let mut file = std::fs::OpenOptions::new().append(true).open(&bundle).unwrap();
    std::io::copy(&mut std::io::Read::take(std::io::repeat(b' '), files::MAX_READ), &mut file).unwrap();
    let out = replayed(&bundle);
    assert!(out.status == Status::Done && out.stdout.ends_with("\nreproduced\n"), "{}{}", out.stdout, out.stderr);
    // Past that, a file is refused unread.
    let huge = dir.join("huge.bug-report.json");
    std::fs::File::create(&huge).unwrap().set_len(BUNDLE_MAX + 1).unwrap();
    let out = replayed(&huge);
    assert!(out.status == Status::Io && out.stderr.contains(&format!("stitch reads at most {BUNDLE_MAX}")), "{}", out.stderr);
}

#[test]
fn req_cli_001_each_difference_is_named_and_any_one_fails_the_replay() {
    let dir = TempDir::new("bug-report");
    let bundle = dir.join("each.bug-report.json");
    assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(conformance("fixtures/svg/strokes.svg"))) }).status, Status::Done);
    // The underline moved up 1 mm sews the same counts: only the plan's digest tells.
    let shifted = changed(&bundle, "shifted.json", |value| {
        let svg = value["design"]["svg"].as_str().unwrap().replace("M 10,50 H 40", "M 10,49 H 40");
        value["design"]["svg"] = Value::from(svg);
    });
    let out = replayed(&shifted);
    assert!(
        out.stdout.contains(
            "  plan          differs
    then:       556 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops (sha256 "
        ),
        "{}",
        out.stdout
    );
    assert!(out.stdout.contains("    now:        556 stitches, 6 jumps, 0 trims, 3 colour changes, 0 stops (sha256 "), "{}", out.stdout);
    // One recorded result changed at a time.
    let cases: [(&str, Change, &str); 3] = [
        ("plan.json", |v| v["result"]["plan"]["sha256"] = Value::from("0"), "  plan          differs\n"),
        ("file.json", |v| v["result"]["file"]["sha256"] = Value::from("0"), "  machine file  differs\n"),
        ("panic.json", |v| v["result"]["panic"] = Value::from("boom"), "  panic         differs\n"),
    ];
    for (name, change, line) in cases {
        let out = replayed(&changed(&bundle, name, change));
        assert_eq!(out.status, Status::DesignErrors, "{name}");
        assert!(out.stdout.contains(line) && out.stdout.ends_with("not reproduced\n"), "{name}: {}", out.stdout);
    }
    // A diagnostic left out, and the same ones in another order.
    let fewer = changed(&bundle, "fewer.json", |v| v["result"]["diagnostics"].as_array_mut().unwrap().truncate(1));
    let out = replayed(&fewer);
    let merged = "info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from a needle point next to it was left out.";
    assert!(out.stdout.contains(&format!("  diagnostics   differs\n    now only:   {merged}\n")), "{}", out.stdout);
    assert!(!out.stdout.contains("another order"), "{}", out.stdout);
    let swapped = changed(&bundle, "swapped.json", |v| v["result"]["diagnostics"].as_array_mut().unwrap().reverse());
    assert!(replayed(&swapped).stdout.contains("  diagnostics   differs\n    the same ones, in another order or number\n"));
}

#[test]
fn req_cli_001_a_design_stitchcraft_refuses_replays_too() {
    let dir = TempDir::new("bug-report");
    // Not UTF-8: kept in hexadecimal, and refused the same way when replayed. By default the bundle
    // goes next to the design.
    let design = dir.join("latin1.svg");
    std::fs::write(&design, b"<svg>\xe9</svg>").unwrap();
    let out = run(&BugReportArgs { format: Some(Format::Dst), ..args(Some(design)) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    let bundle = dir.join("latin1.bug-report.json");
    let written = json(&bundle);
    assert_eq!((written["design"]["svg_hex"].as_str(), written["format"].as_str()), (Some("3c7376673ee93c2f7376673e"), Some("dst")));
    assert!(written["result"]["diagnostics"][0].as_str().unwrap().starts_with("error SC-E0801: "), "{written}");
    assert_eq!((written["result"]["plan"].clone(), written["result"]["file"].clone()), (Value::Null, Value::Null));
    assert_eq!(replayed(&bundle).status, Status::Done);
}

#[test]
fn req_cli_002_a_bug_found_while_planning_writes_a_bundle() {
    let dir = TempDir::new("bug-report");
    // A failed plan check: a bundle next to the machine file asked for, and exit status 4.
    let design = strokes();
    let run = pes(Path::new("strokes.svg"), "strokes", &design);
    let check = Diagnostic::new(Code::InternalCheckFailed, "A stitch of 0.1 mm.");
    let sewn = Sewn { diagnostics: vec![check.clone()], plan: None, file: None, back: None };
    assert!(sewn.found_a_bug());
    assert!(!Sewn { diagnostics: vec![Diagnostic::new(Code::StitchesMerged, "Merged.")], plan: None, file: None, back: None }.found_a_bug());
    let out = beside(&dir.join("checked.pes"), &run, Ok(&sewn), Outcome::refuse(String::new(), &[check]));
    assert_eq!(out.status, Status::Bug);
    let bundle = dir.join("checked.bug-report.json");
    assert!(out.stderr.contains(&format!("A bug-report bundle was written to {}:\n", bundle.display())), "{}", out.stderr);
    assert_eq!(json(&bundle)["result"]["diagnostics"][0], "error SC-E0009: A stitch of 0.1 mm.");
    // A panic in `stitch plan`: caught, said, and bundled with the design.
    let plan = PlanArgs {
        design: conformance("fixtures/svg/strokes.svg"),
        output: dir.join("panicked.pes"),
        profile: None,
        format: None,
        preview: None,
        report: None,
        auto: false,
    };
    let out = guarded(&Command::Plan(plan), || panic!("a test of the panic guard"));
    assert_eq!(out.status, Status::Bug);
    assert!(out.stderr.starts_with("stitch: StitchCraft stopped on a bug in itself: a test of the panic guard\n"), "{}", out.stderr);
    let bundle = dir.join("panicked.bug-report.json");
    // A panic while planning leaves nothing planned to record.
    let result = json(&bundle)["result"].take();
    assert_eq!(result["panic"], "a test of the panic guard");
    assert_eq!((&result["phase"], &result["plan"], &result["file"]), (&Value::from("plan"), &Value::Null, &Value::Null));
    // This design plans without a panic, so the replay says what differs.
    let differs = "  panic         differs\n    then:       a test of the panic guard (while planning)\n    now:        none\n";
    assert!(replayed(&bundle).stdout.contains(differs));
    // Any other command says how to report it; a command that ends normally is left alone.
    let out = guarded(&Command::Profiles, || panic!("{}", String::from("another test")));
    assert_eq!(
        out.stderr,
        format!(
            "stitch: StitchCraft stopped on a bug in itself: another test\nPlease report it at {ISSUES}, with the command you ran and the files it read.\n"
        )
    );
    assert_eq!(guarded(&Command::Profiles, || Outcome::done("fine".to_string())).stdout, "fine");
}

#[test]
fn req_cli_002_a_panic_after_planning_is_bundled_with_the_plan_and_where_it_came() {
    let dir = TempDir::new("bug-report");
    // Planned again, the plan is the run's. A panic while drawing the picture is drawn again by the
    // replay, and this design's picture does not panic, so it differs. One while writing is not rerun.
    let design = strokes();
    let run = pes(Path::new("strokes.svg"), "strokes", &design);
    for (phase, name, status) in [(Phase::Picture, "picture", Status::DesignErrors), (Phase::Writing, "writing", Status::Done)] {
        let stopped = Outcome { stdout: String::new(), stderr: String::new(), status: Status::Bug };
        assert_eq!(beside(&dir.join(format!("late-{name}.pes")), &run, Err(("a late panic", phase)), stopped).status, Status::Bug);
        let bundle = dir.join(format!("late-{name}.bug-report.json"));
        let written = json(&bundle);
        assert_eq!((written["result"]["panic"].as_str(), written["result"]["phase"].as_str()), (Some("a late panic"), Some(name)));
        assert!(written["result"]["plan"]["sha256"].is_string() && written["result"]["file"]["sha256"].is_string(), "{written}");
        let out = replayed(&bundle);
        assert_eq!(out.status, status, "{}", out.stdout);
        assert!(out.stdout.contains("  plan          same: ") && out.stdout.contains("  machine file  same: "), "{}", out.stdout);
    }
    let picture = replayed(&dir.join("late-picture.bug-report.json")).stdout;
    let differs = "  panic         differs\n    then:       a late panic (while drawing the picture)\n    now:        none\n";
    assert!(picture.contains(differs), "{picture}");
    let writing = replayed(&dir.join("late-writing.bug-report.json")).stdout;
    assert!(writing.contains("while writing the files, which a replay does not do") && !writing.contains("not reproduced"), "{writing}");
}

#[test]
fn req_auto_001_a_bundle_records_auto_and_its_replay_digitizes_the_same() {
    let dir = TempDir::new("bug-report-auto");
    let design = dir.join("line.svg");
    std::fs::write(&design, r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><path id="line" d="M 10 50 L 60 50" stroke="red" stroke-width="3" fill="none"/></svg>"#).unwrap();
    let (with, without) = (dir.join("auto.json"), dir.join("plain.json"));
    let out = run(&BugReportArgs { output: Some(with.clone()), auto: true, ..args(Some(design.clone())) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert_eq!(json(&with)["auto"], Value::Bool(true));
    // A bundle of a run without the switch is written as before, without the field.
    let out = run(&BugReportArgs { output: Some(without.clone()), ..args(Some(design)) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert_eq!(json(&without).get("auto"), None);
    assert_ne!(json(&with)["result"]["plan"], json(&without)["result"]["plan"]);
    let out = replayed(&with);
    assert_eq!((out.status, out.stdout.lines().last()), (Status::Done, Some("reproduced")), "{}", out.stdout);
    // Without its record of the switch, the run the bundle holds is not what a replay plans.
    let dropped = changed(&with, "dropped.json", |value| {
        value.as_object_mut().unwrap().remove("auto");
    });
    let out = replayed(&dropped);
    assert_eq!((out.status, out.stdout.lines().last()), (Status::DesignErrors, Some("not reproduced")), "{}", out.stdout);
}

/// The fixture `strokes.svg`.
fn strokes() -> Vec<u8> {
    std::fs::read(conformance("fixtures/svg/strokes.svg")).unwrap()
}

/// A run of `design` from `path`, named `name`, planned for the reference profile as PES.
fn pes<'a>(path: &'a Path, name: &'a str, design: &'a [u8]) -> Run<'a> {
    Run {
        design_path: path,
        source: Source::Svg,
        reading: Reading::default(),
        name,
        design,
        profile: profiles::REFERENCE,
        format: FormatId::PesV1,
        says: None,
    }
}

/// A bundle of the fixture `strokes.svg` as `name` from `path`, planned for the reference profile as PES,
/// written to `written`.
fn bundled(path: &Path, name: &str, written: PathBuf) -> PathBuf {
    let design = strokes();
    let run = pes(path, name, &design);
    write(&written, &super::bundle(&run, borrowed(&guarded_sew(&run, Phase::Plan)))).unwrap();
    written
}

#[test]
fn req_cli_001_a_bundle_replays_with_the_design_name_it_recorded() {
    let dir = TempDir::new("bug-report");
    // Written on Linux, `a\b.svg` names the design `a\b`, and the machine file's label holds it. Windows
    // would read `b` from that file name, so the replay takes the name the bundle recorded.
    let bundle = bundled(Path::new("a\\b.svg"), "a\\b", dir.join("backslash.bug-report.json"));
    assert_eq!(json(&bundle)["design"]["name"], "a\\b");
    assert_eq!(replayed(&bundle).status, Status::Done);
    // A bundle written before names were recorded names the design after its file, as then.
    let then = bundled(Path::new("strokes.svg"), "strokes", dir.join("named.bug-report.json"));
    let unnamed = changed(&then, "unnamed.json", |v| {
        v["design"].as_object_mut().unwrap().remove("name");
    });
    assert_eq!(replayed(&unnamed).status, Status::Done);
    // Another name is another label, and another machine file.
    let renamed = changed(&then, "renamed.json", |v| v["design"]["name"] = Value::from("other"));
    assert!(replayed(&renamed).stdout.contains("  machine file  differs\n"));
}

/// A file name that is not UTF-8 names the design `design`, which the bundle records, though the file
/// name it keeps for people reads `\u{FFFD}t\u{FFFD}.svg`.
#[cfg(unix)]
#[test]
fn req_cli_001_a_design_whose_file_name_is_not_utf_8_replays() {
    let dir = TempDir::new("bug-report");
    use std::os::unix::ffi::OsStrExt as _;
    let path = Path::new(std::ffi::OsStr::from_bytes(b"\xe9t\xe9.svg"));
    let bundle = bundled(path, design_name(path), dir.join("latin1-name.bug-report.json"));
    let written = json(&bundle);
    assert_eq!((written["design"]["file"].as_str(), written["design"]["name"].as_str()), (Some("\u{FFFD}t\u{FFFD}.svg"), Some("design")));
    assert_eq!(replayed(&bundle).status, Status::Done);
}

#[test]
fn req_cli_004_a_replay_prints_what_the_bundle_says_as_text_only() {
    let dir = TempDir::new("bug-report");
    // A bundle anyone may attach to an issue: a platform that would set the terminal's title and clear
    // its screen, and a diagnostic that would print a line of its own and hide the lines after it.
    let bundle = dir.join("terminal.bug-report.json");
    assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(conformance("fixtures/svg/strokes.svg"))) }).status, Status::Done);
    let hostile = changed(&bundle, "hostile.json", |v| {
        v["platform"] = Value::from("linux x86_64\u{1b}]0;title\u{7}\u{1b}[2J");
        v["result"]["diagnostics"][0] = Value::from("warning\nreproduced\u{1b}[8m");
    });
    let out = replayed(&hostile);
    let printed = out.stdout.clone() + &out.stderr;
    assert!(!printed.chars().any(|c| c.is_control() && c != '\n'), "{printed:?}");
    assert!(out.stdout.contains(" on linux x86_64\\u{1b}]0;title\\u{7}\\u{1b}[2J, replayed by "), "{}", out.stdout);
    assert!(out.stdout.contains("\n    then only:  warning\\nreproduced\\u{1b}[8m\n"), "{}", out.stdout);
    assert!(out.stdout.ends_with("\nnot reproduced\n") && !out.stdout.lines().any(|line| line == "reproduced"), "{}", out.stdout);
    // So are the names of a bundle that cannot be replayed (SC-E0012).
    let profile = changed(&bundle, "profile.json", |v| v["profile"] = Value::from("pe800\u{1b}[2J"));
    assert!(replayed(&profile).stderr.contains(" names the profile `pe800\\u{1b}[2J`, which "));
}

#[test]
fn diag_sc_e0012_a_file_that_is_not_a_bundle_is_not_replayed() {
    let dir = TempDir::new("bug-report");
    let notes = dir.join("notes.txt");
    std::fs::write(&notes, b"the zigzag looks wrong").unwrap();
    let out = replayed(&notes);
    assert_eq!(out.status, Status::DesignErrors);
    assert_eq!(
        out.stderr,
        format!(
            "error SC-E0012: {} is not a bug-report bundle: expected ident at line 1 column 2.\n  more: stitch explain SC-E0012\n",
            notes.display()
        )
    );
    // Bundles this version cannot replay say why.
    let bundle = dir.join("whole.bug-report.json");
    assert_eq!(run(&BugReportArgs { output: Some(bundle.clone()), ..args(Some(conformance("fixtures/svg/strokes.svg"))) }).status, Status::Done);
    let cases: [(&str, Change, &str); 5] = [
        ("newer.json", |v| v["stitchcraft_bug_report"] = Value::from(2), "is a bundle of format 2, and this StitchCraft replays format 1."),
        ("embroider.json", |v| v["command"] = Value::from("embroider"), "records `stitch embroider`, which this StitchCraft does not replay."),
        ("singer.json", |v| v["profile"] = Value::from("singer"), "names the profile `singer`, which this StitchCraft does not have."),
        ("jef.json", |v| v["format"] = Value::from("jef"), "names the format `jef`, which this StitchCraft does not write."),
        (
            "twice.json",
            |v| v["design"]["svg_hex"] = Value::from("00"),
            "must hold its design once: as `svg` or `svg_hex` for an SVG file, as `vectorcraft` or `vectorcraft_hex` for a VectorCraft document.",
        ),
    ];
    for (name, change, says) in cases {
        let path = changed(&bundle, name, change);
        let out = replayed(&path);
        assert_eq!(out.stderr, format!("error SC-E0012: {} {says}\n  more: stitch explain SC-E0012\n", path.display()), "{name}");
    }
    assert_eq!((unhex("3c7E"), unhex("3c7"), unhex("zz")), (Some(vec![0x3c, 0x7e]), None, None));
}
