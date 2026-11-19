//! The `stitch` binary as people run it: what `main` adds to the commands — printing what they say,
//! ending with their exit status, and the panic guard around them — and what each command writes and
//! prints, run on real files.

// Test code may unwrap (clippy.toml allows it inside #[test] functions; this helper is test code too). It
// runs `stitch` on files in folders of its own.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

use std::path::Path;
use std::process::{Command, Output};

use stitchcraft_testkit::temp::TempDir;

fn stitch(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).output().unwrap()
}

/// `stitch` run in `dir` with `args`, and the environment's `NO_COLOR` and `TERM` taken away.
fn stitch_in(dir: impl AsRef<Path>, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(dir).env_remove("NO_COLOR").env_remove("TERM").output().unwrap()
}

/// A new folder for one test, with the fixture `strokes.svg` in it, removed when the test is done.
fn folder(name: &str) -> TempDir {
    let dir = TempDir::new(&format!("binary-{name}"));
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
    // A bug's bundle goes next to the file asked for (`REQ-CLI-002`), so no other file may be there.
    refused(
        &["plan", "strokes.svg", "-o", "x.pes", "--report", "x.bug-report.json"],
        "x.bug-report.json is where a bug-report bundle for x.pes would be written: name it another file",
    );
    refused(
        &["plan", "x.bug-report.json", "-o", "x.pes"],
        "a bug-report bundle for x.pes would replace x.bug-report.json, which it is made from: name the output another file",
    );
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
    // Built from a git checkout, one whose git tracks these sources, the bundle names the commit, and
    // `--version` shows it. A copy of the sources without git, as cargo-mutants builds, has no commit to
    // name, and nor has a copy inside a repository that does not track it, as the rehearsal of the move into
    // VectorCraft makes: that repository's commit holds none of the sources.
    let commit = env!("STITCHCRAFT_COMMIT");
    let checkout = Command::new("git")
        .args(["ls-files", "--error-unmatch", "build.rs"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .is_ok_and(|out| out.status.success());
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

/// REQ-FONT-021: `stitch` sews a VectorCraft document's lettered text in a font of the folder
/// `STITCHCRAFT_FONTS` names, and leaves the text out, saying so, when the variable names none.
#[test]
fn req_font_021_stitch_finds_a_document_s_font_in_the_folder_the_variable_names() {
    let dir = folder("fonts");
    let glyphs = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100"><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 1,60 L 5,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"#;
    std::fs::create_dir_all(dir.join("fonts/plain")).unwrap();
    std::fs::write(dir.join("fonts/plain/font.json"), r#"{"name": "Plain Font", "min_scale": 0.5, "max_scale": 2}"#).unwrap();
    std::fs::write(dir.join("fonts/plain/ltr.svg"), glyphs).unwrap();
    let document = r#"{"format": "vectorcraft", "version": 4, "document": {"layers": [{"id": 1, "kind": {"type": "layer", "children": [
        {"id": 2, "appearance": {"effects": [{"id": "plugin.dev.stitchcraft.lettering", "params": {"font": "Plain Font"}, "visible": true}]},
         "kind": {"type": "text", "kind": {"kind": "point"}, "xf": [1, 0, 0, 1, 72, 72], "runs": [{"text": "aaa"}]}}]}}]}}"#;
    std::fs::write(dir.join("name.vectorcraft"), document).unwrap();
    let run = |fonts: &std::ffi::OsStr| {
        Command::new(env!("CARGO_BIN_EXE_stitch"))
            .args(["plan", "name.vectorcraft", "-o", "name.pes"])
            .current_dir(&dir)
            .env("STITCHCRAFT_FONTS", fonts)
            .output()
            .unwrap()
    };
    let sewn = run(dir.join("fonts").as_os_str());
    assert!(sewn.status.success(), "{}", text(&sewn.stderr));
    assert!(!text(&sewn.stderr).contains("SC-W1111"), "{}", text(&sewn.stderr));
    assert!(std::fs::read(dir.join("name.pes")).unwrap().starts_with(b"#PES"));
    // Named by nothing, the variable names no folder: the text is left out, which leaves nothing to stitch.
    let none = run(std::ffi::OsStr::new(""));
    assert!(!none.status.success());
    assert!(text(&none.stderr).contains("warning SC-W1111") && text(&none.stderr).contains("No embroidery font is here."), "{}", text(&none.stderr));
}

/// REQ-CLI-020, REQ-SVG-016: `stitch plan --text` and a shape labelled `stitch:text` in an SVG file sew the
/// letters inside them as lettering in a font of the folder `STITCHCRAFT_FONTS` names, and a bundle replays
/// the marks.
#[test]
fn req_cli_020_stitch_plan_sews_marked_letters_as_lettering() {
    let dir = folder("text");
    let glyphs = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100"><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 0,60 L 4,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"##;
    std::fs::create_dir_all(dir.join("fonts/small_font")).unwrap();
    std::fs::write(dir.join("fonts/small_font/font.json"), r#"{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 4}"#).unwrap();
    std::fs::write(dir.join("fonts/small_font/ltr.svg"), glyphs).unwrap();
    let picture = |marker: &str| {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="60mm" height="40mm" viewBox="0 0 60 40"><rect id="field" width="60" height="40" fill="#ffffff"/><rect id="l1" x="10" y="10" width="8" height="20" fill="#ff0000"/>{marker}</svg>"##
        )
    };
    std::fs::write(dir.join("plain.svg"), picture("")).unwrap();
    std::fs::write(
        dir.join("marked.svg"),
        picture(r##"<rect id="m" inkscape:label="stitch:text a" x="5" y="5" width="20" height="30" fill="none" stroke="#0000ff"/>"##),
    )
    .unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(&dir).env("STITCHCRAFT_FONTS", dir.join("fonts")).output().unwrap()
    };
    let said = "replaced by the text \"a\" in the font `small_font`, 20.0 mm tall.";
    let marked = run(&["plan", "marked.svg", "--auto", "-o", "marked.pes"]);
    assert!(
        marked.status.success()
            && text(&marked.stderr).contains(&format!("info SC-I1112: The text marker `svg:m:stroke`: 1 traced outlines, 20.0 mm tall, are {said}")),
        "{}",
        text(&marked.stderr)
    );
    let option = run(&["plan", "plain.svg", "--auto", "--text", "0,0,50,100:a", "-o", "option.pes"]);
    assert!(
        option.status.success()
            && text(&option.stderr).contains(&format!("info SC-I1112: The text marker `text-1`: 1 traced outlines, 20.0 mm tall, are {said}")),
        "{}",
        text(&option.stderr)
    );
    // Marked either way, the picture sews the same stitches.
    let stitches =
        |file: &str| text(&run(&["inspect", file]).stdout).lines().filter(|line| line.contains("stitches")).map(str::to_string).collect::<Vec<_>>();
    assert!(
        !stitches("marked.pes").is_empty() && stitches("marked.pes") == stitches("option.pes"),
        "{:?} {:?}",
        stitches("marked.pes"),
        stitches("option.pes")
    );
    // Without the mark the traced letter sews, as a shape: another count of stitches.
    assert!(run(&["plan", "plain.svg", "--auto", "-o", "plain.pes"]).status.success());
    assert_ne!(stitches("plain.pes"), stitches("option.pes"));
    // A mark that doesn't read is refused before anything is read.
    let refused = run(&["plan", "plain.svg", "--auto", "--text", "0,0,150,100:a", "-o", "refused.pes"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(text(&refused.stderr).contains("isn't LEFT,TOP,WIDTH,HEIGHT:TEXT") && !dir.join("refused.pes").exists(), "{}", text(&refused.stderr));
    // A bundle keeps the mark with the rules' settings, and its replay sews the lettering again.
    let bundled = run(&["bug-report", "plain.svg", "--auto", "--text", "0,0,50,100:a", "-o", "text.bug-report.json"]);
    assert!(bundled.status.success(), "{}", text(&bundled.stderr));
    assert!(std::fs::read_to_string(dir.join("text.bug-report.json")).unwrap().contains("0,0,50,100:a"));
    let replayed = run(&["bug-report", "--replay", "text.bug-report.json"]);
    assert!(replayed.status.success(), "{} {}", text(&replayed.stdout), text(&replayed.stderr));
}

/// `REQ-CLI-021`: `--keep LEFT,TOP,WIDTH,HEIGHT` marks a rectangle of the design, in percent of its box, as a
/// shape labelled `stitch:keep` marks one (`REQ-AUTO-104`), and the bundle of a bug report keeps it.
#[test]
fn req_cli_021_stitch_plan_keeps_the_parts_a_keep_option_marks() {
    let dir = folder("keep");
    // A white field 60 mm square, a red square on it, and a blue speck of 0.8 mm² at (20, 20), which merges
    // into the square unless it is marked.
    let picture = |marker: &str| {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="60mm" height="60mm" viewBox="0 0 60 60"><rect id="field" width="60" height="60" fill="#ffffff"/><rect id="square" x="10" y="10" width="40" height="40" fill="#ff0000"/><rect id="speck" x="20" y="20" width="1.6" height="0.5" fill="#0000ff"/>{marker}</svg>"##
        )
    };
    std::fs::write(dir.join("plain.svg"), picture("")).unwrap();
    std::fs::write(
        dir.join("marked.svg"),
        picture(
            r##"<rect id="m" inkscape:label="stitch:keep" x="18" y="18" width="6" height="6" fill="none" stroke="#00ff00" stroke-width="0.2"/>"##,
        ),
    )
    .unwrap();
    let run = |args: &[&str]| Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(&dir).output().unwrap();
    let stitches =
        |file: &str| text(&run(&["inspect", file]).stdout).lines().filter(|line| line.contains("stitches")).map(str::to_string).collect::<Vec<_>>();
    // The speck merges without a mark, and is said to.
    let plain = run(&["plan", "plain.svg", "--auto", "-o", "plain.pes"]);
    assert!(plain.status.success() && text(&plain.stderr).contains("SC-I0905"), "{}", text(&plain.stderr));
    // A shape labelled stitch:keep, or the option over the same area, 30 % from the left and the top of the
    // field and 10 % of it across, keeps it: the same stitches either way, and not those of the plain plan.
    let marked = run(&["plan", "marked.svg", "--auto", "-o", "marked.pes"]);
    assert!(marked.status.success() && !text(&marked.stderr).contains("SC-I0905"), "{}", text(&marked.stderr));
    let option = run(&["plan", "plain.svg", "--auto", "--keep", "30,30,10,10", "-o", "option.pes"]);
    assert!(option.status.success() && text(&option.stderr).contains("SC-I0916"), "{}", text(&option.stderr));
    assert!(!stitches("marked.pes").is_empty() && stitches("marked.pes") == stitches("option.pes"));
    assert_ne!(stitches("plain.pes"), stitches("option.pes"));
    // A mark that doesn't read is refused before anything is read.
    let refused = run(&["plan", "plain.svg", "--auto", "--keep", "90,0,20,10", "-o", "refused.pes"]);
    assert_eq!(refused.status.code(), Some(2));
    assert!(text(&refused.stderr).contains("isn't LEFT,TOP,WIDTH,HEIGHT") && !dir.join("refused.pes").exists(), "{}", text(&refused.stderr));
    // A bundle keeps the mark with the rules' settings, and its replay keeps the speck again.
    let bundled = run(&["bug-report", "plain.svg", "--auto", "--keep", "30,30,10,10", "-o", "keep.bug-report.json"]);
    assert!(bundled.status.success(), "{}", text(&bundled.stderr));
    assert!(std::fs::read_to_string(dir.join("keep.bug-report.json")).unwrap().contains("30,30,10,10"));
    let replayed = run(&["bug-report", "--replay", "keep.bug-report.json"]);
    assert!(replayed.status.success(), "{} {}", text(&replayed.stdout), text(&replayed.stderr));
}

/// `REQ-AUTO-069`, `REQ-SVG-017`: `stitch plan --auto` sews the text of a drawn SVG design as lettering in the
/// default font of the folder `STITCHCRAFT_FONTS` names, where the text stands, and says which font stands in.
/// Without `--auto`, and without a font, the text is left out and that is said.
#[test]
fn req_auto_069_stitch_plan_sews_a_drawn_designs_text_as_lettering() {
    let dir = folder("text-objects");
    let glyphs = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100"><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 0,60 L 4,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"##;
    std::fs::create_dir_all(dir.join("fonts/small_font")).unwrap();
    std::fs::write(dir.join("fonts/small_font/font.json"), r#"{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 4}"#).unwrap();
    std::fs::write(dir.join("fonts/small_font/ltr.svg"), glyphs).unwrap();
    // A badge with a word on it, and a star beside it: the word 20 mm in font size, so 14 mm tall as lettering.
    let badge = |word: &str| {
        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="60mm" height="40mm" viewBox="0 0 60 40"><rect id="field" width="50" height="40" fill="#ffffff"/><rect id="star" x="54" y="2" width="4" height="4" fill="#0000c0"/>{word}</svg>"##
        )
    };
    std::fs::write(dir.join("word.svg"), badge(r##"<text id="word" x="10" y="30" font-size="20" font-family="Georgia" fill="#c00000">aaa</text>"##))
        .unwrap();
    std::fs::write(dir.join("blank.svg"), badge("")).unwrap();
    let run_in = |fonts: &str, args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(&dir).env("STITCHCRAFT_FONTS", dir.join(fonts)).output().unwrap()
    };
    let run = |args: &[&str]| run_in("fonts", args);
    let lettered = run(&["plan", "word.svg", "--auto", "-o", "word.pes"]);
    let note = "info SC-I1116 (svg:word): The text `svg:word` is sewn in the embroidery font `small_font`, 14.0 mm tall: 0.7 of its font size";
    assert!(lettered.status.success() && text(&lettered.stderr).contains(note), "{}", text(&lettered.stderr));
    assert!(text(&lettered.stderr).contains("Its own typeface, Georgia, can't be read yet"), "{}", text(&lettered.stderr));
    // The word sews: more stitches than the badge without it, in one more thread.
    let line = |file: &str, what: &str| {
        text(&run(&["inspect", file]).stdout).lines().find(|line| line.contains(what)).map(str::to_string).unwrap_or_default()
    };
    assert!(run(&["plan", "blank.svg", "--auto", "-o", "blank.pes"]).status.success());
    assert_ne!(line("word.pes", "stitches"), line("blank.pes", "stitches"));
    // Without `--auto` the text is left out with the warning it had, and the badge sews as without it.
    let plain = run(&["plan", "word.svg", "-o", "plain.pes"]);
    assert!(
        plain.status.success() && text(&plain.stderr).contains("SC-W0802 (svg:word): `word` is text, which is not stitched; it is left out."),
        "{}",
        text(&plain.stderr)
    );
    assert!(!text(&plain.stderr).contains("SC-I1116"));
    // Without a font the text is left out, which is said, and the rest sews.
    std::fs::create_dir_all(dir.join("none")).unwrap();
    let unlettered = run_in("none", &["plan", "word.svg", "--auto", "-o", "unlettered.pes"]);
    assert!(unlettered.status.success(), "{}", text(&unlettered.stderr));
    let why = "warning SC-W1111 (svg:word): The text `svg:word` is not stitched: no embroidery font is here to sew it in.";
    assert!(text(&unlettered.stderr).contains(why), "{}", text(&unlettered.stderr));
    assert_eq!(line("unlettered.pes", "stitches"), line("blank.pes", "stitches"));
}
