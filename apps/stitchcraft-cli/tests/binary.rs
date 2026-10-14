//! The `stitch` binary as people run it: what `main` adds to the commands — printing what they say,
//! ending with their exit status, and the panic guard around them — and what each command writes and
//! prints, run on real files.

// Test code may unwrap (clippy.toml allows it inside #[test] functions; this helper is test code too). It
// runs `stitch` on files in folders of its own.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn stitch(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).output().unwrap()
}

/// `stitch` run in `dir` with `args`, and the environment's `NO_COLOR` and `TERM` taken away.
fn stitch_in(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(dir).env_remove("NO_COLOR").env_remove("TERM").output().unwrap()
}

/// A new empty folder for one test, with the fixture `strokes.svg` in it.
fn folder(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("stitchcraft-binary-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../conformance/fixtures/svg/strokes.svg");
    std::fs::copy(fixture, dir.join("strokes.svg")).unwrap();
    dir
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn prints_what_a_command_says_and_ends_with_its_status() {
    let explained = stitch(&["explain", "SC-E0012"]);
    assert_eq!(explained.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&explained.stdout).starts_with("SC-E0012: Bug-report bundle could not be replayed"));
    assert_eq!(stitch(&["sew-it"]).status.code(), Some(2));
    let missing = stitch(&["inspect", "no-such-file.pes"]);
    assert_eq!(missing.status.code(), Some(3));
    assert!(String::from_utf8_lossy(&missing.stderr).starts_with("stitch: cannot read no-such-file.pes"));
}

#[test]
fn req_cli_007_an_output_that_is_another_output_or_an_input_is_refused() {
    let dir = folder("same");
    let design = std::fs::read(dir.join("strokes.svg")).unwrap();
    let refused = |args: &[&str], says: &str| {
        let out = stitch_in(&dir, args);
        assert_eq!(out.status.code(), Some(2), "{args:?}: {}", text(&out.stderr));
        assert!(text(&out.stderr).contains(says), "{args:?}: {}", text(&out.stderr));
    };
    refused(&["plan", "strokes.svg", "-o", "x.pes", "--report", "x.pes"], "x.pes and x.pes are the same file");
    refused(&["plan", "strokes.svg", "-o", "x.pes", "--preview", "./x.pes"], "are the same file");
    refused(&["plan", "strokes.svg", "-o", "strokes.svg", "--format", "pes"], "would replace strokes.svg, which it is made from");
    refused(&["bug-report", "strokes.svg", "-o", "strokes.svg"], "would replace strokes.svg");
    assert!(!dir.join("x.pes").exists());
    assert_eq!(std::fs::read(dir.join("strokes.svg")).unwrap(), design, "the design is as it was");
    assert_eq!(stitch_in(&dir, &["plan", "strokes.svg", "-o", "x.pes"]).status.code(), Some(0));
    refused(&["preview", "x.pes", "-o", "x.pes"], "would replace x.pes");
    refused(&["convert", "x.pes", "-o", "x.pes"], "would replace x.pes");
    assert!(std::fs::read(dir.join("x.pes")).unwrap().starts_with(b"#PES0001"));
}

#[test]
fn req_cli_008_a_summary_counts_the_file_it_names() {
    // DST machines cut the thread before a long jump, which the plan does not trim: the file's counts say so,
    // and `stitch inspect` counts the same.
    let dir = folder("counts");
    let planned = text(&stitch_in(&dir, &["plan", "strokes.svg", "-o", "strokes.dst"]).stdout);
    let inspected = text(&stitch_in(&dir, &["inspect", "strokes.dst"]).stdout);
    let counts = |out: &str| out.lines().find(|line| line.starts_with("  stitches  ")).map(str::to_string);
    assert_eq!(counts(&planned), counts(&inspected), "{planned}\n{inspected}");
    assert!(counts(&planned).is_some_and(|line| line.contains(", 1 trim,")), "{planned}");
    let converted = text(&stitch_in(&dir, &["convert", "strokes.dst", "-o", "again.pes"]).stdout);
    assert_eq!(counts(&converted), counts(&text(&stitch_in(&dir, &["inspect", "again.pes"]).stdout)), "{converted}");
}

#[test]
fn req_cli_009_an_output_that_cannot_be_written_is_said_and_ends_with_3() {
    if !Path::new("/dev/full").exists() {
        return;
    }
    let full = std::fs::OpenOptions::new().write(true).open("/dev/full").unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_stitch")).arg("profiles").stdout(full).output().unwrap();
    assert_eq!(out.status.code(), Some(3));
    assert!(text(&out.stderr).starts_with("stitch: cannot write the output: "), "{}", text(&out.stderr));
}

#[test]
fn req_cli_010_help_and_errors_are_not_coloured_off_a_terminal() {
    let dir = folder("colour");
    for args in [&["--help"][..], &["plan"], &["bug-report", "--replay", "b.json", "--profile", "brother-pe800-4x4"]] {
        let out = stitch_in(&dir, args);
        let said = text(&out.stdout) + &text(&out.stderr);
        assert!(!said.contains('\u{1b}'), "{args:?}: {said:?}");
    }
}

#[test]
fn a_replay_takes_no_profile_of_its_own() {
    // The bundle names the profile it was planned for, and a replay plans for that one.
    let out = stitch(&["bug-report", "--replay", "b.json", "--profile", "brother-pe800-4x4"]);
    assert_eq!(out.status.code(), Some(2));
    assert!(text(&out.stderr).contains("'--replay <REPLAY>' cannot be used with '--profile <PROFILE>'"), "{}", text(&out.stderr));
}

#[test]
fn req_cli_001_a_bundle_names_the_commit_its_stitchcraft_was_built_from() {
    let dir = folder("commit");
    let version = text(&stitch(&["--version"]).stdout);
    assert!(version.starts_with(&format!("stitch {}", env!("CARGO_PKG_VERSION"))), "{version}");
    let out = stitch_in(&dir, &["bug-report", "strokes.svg"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let bundle: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("strokes.bug-report.json")).unwrap()).unwrap();
    // Built from a git checkout, the bundle names the commit, and `--version` shows it. A copy of the sources
    // without git, as cargo-mutants builds, has no commit to name.
    let commit = env!("STITCHCRAFT_COMMIT");
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../.git").exists();
    assert_eq!(!commit.is_empty(), checkout, "commit {commit:?}");
    if checkout {
        assert!(commit.len() == 12 && commit.bytes().all(|b| b.is_ascii_hexdigit()), "{commit}");
        assert_eq!(bundle["commit"], commit);
        assert!(version.contains(&format!("(commit {commit})")), "{version}");
    } else {
        assert!(bundle.get("commit").is_none() && !version.contains("commit"), "{bundle}\n{version}");
    }
    // A bundle of another format says so, whatever else it holds.
    std::fs::write(dir.join("v2.json"), br#"{"stitchcraft_bug_report": 2, "version": null}"#).unwrap();
    let out = stitch_in(&dir, &["bug-report", "--replay", "v2.json"]);
    assert_eq!(out.status.code(), Some(1));
    assert!(text(&out.stderr).contains("v2.json is a bundle of format 2, and this StitchCraft replays format 1."), "{}", text(&out.stderr));
}

#[test]
fn inspect_counts_stitches_in_place_and_those_a_lock_may_explain() {
    use stitchcraft_core::Point;
    use stitchcraft_plan::{FormatId, PlanBuilder, Provenance, Rgb, Role, Thread};
    let dir = folder("inspect");
    // What `stitch inspect` says of a PES file of stitches at `xs` along a line, as other software may write it.
    let inspected = |xs: &[f64]| {
        let mut plan = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        for x in xs {
            plan.stitch(Point::new(*x, 0.0).unwrap(), Provenance::plan(Role::Top));
        }
        std::fs::write(dir.join("probe.pes"), stitchcraft_formats::encode(&plan.finish(), FormatId::PesV1, "probe").unwrap().bytes).unwrap();
        text(&stitch_in(&dir, &["inspect", "probe.pes", "--profile", "brother-pe800-4x4"]).stdout)
    };
    // Stitches of 2 mm, none (in place), 0.2 mm and 2 mm.
    let out = inspected(&[0.0, 2.0, 2.0, 2.2, 4.2]);
    assert!(out.contains("  lengths   stitches from 0.20 to 2.00 mm, and 1 in place\n"), "{out}");
    assert!(out.contains("0 stitches longer than 12 mm, 1 shorter than 0.3 mm, 1 of them 0.2 mm or longer, as a lock stitch may be\n"), "{out}");
    // A stitch as short as the profile's shortest is not too short, nor one as long as its longest too long.
    let out = inspected(&[0.0, 0.3, 12.3, 24.8]);
    assert!(out.contains("  lengths   stitches from 0.30 to 12.50 mm\n"), "{out}");
    assert!(out.contains("1 stitches longer than 12 mm, 0 shorter than 0.3 mm\n"), "{out}");
    // Stitches in place alone, and one needle point, which sews no stitch.
    assert!(inspected(&[1.0, 1.0, 1.0]).contains("  lengths   2 stitches in place\n"));
    let out = inspected(&[1.0]);
    assert!(!out.contains("  lengths "), "{out}");
}
