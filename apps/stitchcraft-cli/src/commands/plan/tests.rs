use std::path::PathBuf;

use stitchcraft_core::ElementId;

use stitchcraft_testkit::temp::TempDir;

use stitchcraft_core::Mm;
use stitchcraft_engine::design::DesignSettings;
use stitchcraft_plan::profiles::{BROTHER_PE800_SMALL, REFERENCE};
use stitchcraft_vectorcraft::plugin_id;
use stitchcraft_vectorcraft::settings::{SETTINGS_FILTER, kept_data};

use super::*;
use crate::cli::{Format, ReadingArgs, SubjectArgs, TracingArgs};

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../conformance/fixtures/svg/{name}", env!("CARGO_MANIFEST_DIR")))
}

fn vectorcraft(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../conformance/fixtures/vectorcraft/{name}", env!("CARGO_MANIFEST_DIR")))
}

fn args(design: PathBuf, output: PathBuf) -> PlanArgs {
    PlanArgs {
        design,
        output,
        profile: None,
        format: None,
        preview: None,
        report: None,
        reading: ReadingArgs::default(),
        tracing: TracingArgs::default(),
        subject: SubjectArgs::default(),
    }
}

#[test]
fn req_thread_003_the_report_lists_each_thread_of_an_element_sewn_in_3() {
    // One element in red, green and blue, as a gradient fill of 3 stops sews, planned by hand: the report
    // is made of the plan a file of one would give.
    let plan = stitchcraft_testkit::designs::three_threads();
    let file = stitchcraft_formats::encode(&plan, FormatId::PesV1, "gradient").unwrap().bytes;
    let back = read_back(&file, &plan).unwrap();
    let args = args(PathBuf::from("gradient.svg"), PathBuf::from("gradient.pes"));
    let made = Made { file: Some((FormatId::PesV1, &file)), planned: Some((&plan, &back)), subject: None };
    let report: Value = serde_json::from_str(&report(&args, REFERENCE, &made, &[])).unwrap();
    let colours: Vec<&str> = report["threads"].as_array().unwrap().iter().map(|thread| thread["color"].as_str().unwrap()).collect();
    assert_eq!(colours, ["#c80000", "#00a000", "#0000c8"]);
    assert_eq!((report["color_changes"].as_u64(), report["stops"].as_u64()), (Some(2), Some(0)));
    // What `stitch plan` prints lists them too, each with the Brother colour it is written as.
    let printed = summary(&args, REFERENCE, FormatId::PesV1, (&plan, &back), &file);
    let listed: Vec<&str> = printed.lines().filter(|line| line.contains(", shown as Brother PEC ")).collect();
    assert_eq!(listed.len(), 3, "{printed}");
    for (line, colour) in listed.iter().zip(["1. unnamed (#c80000)", "2. unnamed (#00a000)", "3. unnamed (#0000c8)"]) {
        assert!(line.contains(colour), "{printed}");
    }
}

#[test]
fn writes_the_machine_file_a_picture_and_a_report() {
    let dir = TempDir::new("plan");
    let (output, preview, report) = (dir.join("strokes.pes"), dir.join("strokes.png"), dir.join("strokes.json"));
    let out = run(&PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(fixture("strokes.svg"), output.clone()) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert!(std::fs::read(&output).unwrap().starts_with(b"#PES0001"));
    assert!(std::fs::read(&preview).unwrap().starts_with(b"\x89PNG"));
    // The file's counts: a PES file flags 5 of the plan's 6 jumps as trims, which the machine cuts.
    assert!(out.stdout.contains("  stitches  556 stitches, 6 jumps, 5 trims, 3 colour changes, 0 stops\n"), "{}", out.stdout);
    assert!(out.stdout.contains("  threads   1. unnamed (#c00000), shown as Brother PEC 5 \"Red\"\n"));
    // What was said names its element, on the terminal and in the report.
    assert_eq!(
        out.stderr,
        "warning SC-W0307 (svg:patch:fill): This fill's area falls into 2 parts, sewn one after another, each on its own.\n  \
         hint: To choose the order the parts are sewn in, break the fill apart into one element for each part.\n\
         info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from a needle point next to it was left out.\n"
    );
    let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert_eq!((report["stitches"].as_u64(), report["trims"].as_u64(), report["color_changes"].as_u64()), (Some(556), Some(5), Some(3)));
    // The file's size, its needle points on the machine's 0.1 mm grid: the plan's is 59.9 mm wide.
    assert_eq!(report["size_mm"], json!([60.0, 45.0]));
    assert_eq!(report["diagnostics"][0]["element"], "svg:patch:fill");
    assert_eq!(report["file"]["sha256"], hex(&Sha256::digest(std::fs::read(&output).unwrap())));
    assert_eq!((report["design"].as_str(), report["file"]["path"].as_str()), (fixture("strokes.svg").to_str(), output.to_str()));
}

/// A line 3 mm wide and 50 mm long, without settings: a satin column when digitized automatically.
const LINE: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><path id="line" d="M 10 50 L 60 50" stroke="red" stroke-width="3" fill="none"/></svg>"#;

#[test]
fn req_auto_001_stitch_plan_auto_digitizes_the_shapes_without_settings() {
    let dir = TempDir::new("plan-auto");
    let design = dir.join("line.svg");
    std::fs::write(&design, LINE).unwrap();
    let (auto, plain) = (dir.join("auto.pes"), dir.join("plain.pes"));
    let out = run(&PlanArgs { reading: ReadingArgs { auto: true, ..ReadingArgs::default() }, ..args(design.clone(), auto.clone()) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert_eq!(out.stderr, "info SC-I0901: 1 shape is digitized automatically: it sews as a satin column.\n");
    // Without the switch, the line runs, as it did before.
    let out = run(&args(design, plain.clone()));
    assert_eq!((out.status, out.stderr.as_str()), (Status::Done, ""));
    assert_ne!(std::fs::read(auto).unwrap(), std::fs::read(plain).unwrap());
}

/// Red, blue and red squares 10 mm apart, without settings: tatami fills, as Ink/Stitch sews them.
const SQUARES: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100"><rect id="r1" x="0" y="0" width="10" height="10" fill="red"/><rect id="b" x="20" y="0" width="10" height="10" fill="blue"/><rect id="r2" x="40" y="0" width="10" height="10" fill="red"/></svg>"#;

#[test]
fn req_auto_011_stitch_plan_sort_colours_sews_the_shapes_with_settings_by_thread() {
    let dir = TempDir::new("plan-sorted");
    let design = dir.join("squares.svg");
    std::fs::write(&design, SQUARES).unwrap();
    let (sorted, drawn) = (dir.join("sorted.json"), dir.join("drawn.json"));
    let sorting = ReadingArgs { sort_colours: true, ..ReadingArgs::default() };
    let out = run(&PlanArgs { report: Some(sorted.clone()), reading: sorting, ..args(design.clone(), dir.join("sorted.pes")) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert!(
        out.stderr.contains("info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2.\n"),
        "{}",
        out.stderr
    );
    // Without the switch, the squares sew as drawn.
    let out = run(&PlanArgs { report: Some(drawn.clone()), ..args(design, dir.join("drawn.pes")) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    let changes = |report: &Path| serde_json::from_slice::<Value>(&std::fs::read(report).unwrap()).unwrap()["color_changes"].as_u64();
    assert_eq!((changes(&sorted), changes(&drawn)), (Some(1), Some(2)));
}

#[test]
fn req_vc_013_a_vectorcraft_document_is_planned_with_the_settings_and_for_the_machine_it_keeps() {
    let dir = TempDir::new("plan-settings");
    // The patch, with the small hoop and a shortest jump of 10 mm kept by the Design Settings filter.
    let patch = std::fs::read(vectorcraft("patch.vectorcraft")).unwrap();
    let mut file: Value = serde_json::from_slice(stitchcraft_vectorcraft::read_part(&patch)).unwrap();
    let settings = DesignSettings { collapse_len: Mm::new(10.0).unwrap(), ..DesignSettings::default() };
    file["document"]["unknown"] = json!({"pluginData": {plugin_id(SETTINGS_FILTER): kept_data(&settings, &BROTHER_PE800_SMALL)}});
    let kept = dir.join("kept.vectorcraft");
    std::fs::write(&kept, serde_json::to_vec(&file).unwrap()).unwrap();
    assert_eq!(machine_for(None, Source::VectorCraft, &std::fs::read(&kept).unwrap()), &BROTHER_PE800_SMALL);
    // Planned for the small hoop, which the patch doesn't fit, so nothing is written.
    let small = dir.join("small.pes");
    let out = run(&args(kept.clone(), small.clone()));
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.contains("error SC-E0701: ") && !small.exists(), "{}", out.stderr);
    // `--profile` names another, and the settings still hold: the shortest jump sews the patch otherwise.
    let on_reference = |design: PathBuf, output: PathBuf| {
        let out = run(&PlanArgs { profile: Some(REFERENCE.id.to_string()), ..args(design, output.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stdout.contains(&format!("  profile   {} (", REFERENCE.id)), "{}", out.stdout);
        std::fs::read(output).unwrap()
    };
    assert_ne!(on_reference(kept, dir.join("named.pes")), on_reference(vectorcraft("patch.vectorcraft"), dir.join("plain.pes")));
    // An SVG file keeps no machine.
    assert_eq!(machine_for(None, Source::Svg, &std::fs::read(fixture("strokes.svg")).unwrap()), REFERENCE);
}

#[test]
fn req_vc_008_a_design_is_told_by_its_extension() {
    let documents = ["patch.vectorcraft", "old/patch.drawcraft", "PATCH.VectorCraft", "start.vctemplate"];
    let svg = ["patch.svg", "vectorcraft", "patch.vectorcraft.svg", "patch.vectorcraf"];
    for (paths, source) in [(&documents[..], Source::VectorCraft), (&svg[..], Source::Svg)] {
        for path in paths {
            assert_eq!(Source::of(Path::new(path)), source, "{path}");
        }
    }
}

#[test]
fn req_vc_008_stitch_plan_reads_a_vectorcraft_document() {
    let dir = TempDir::new("plan");
    let (output, preview, report) = (dir.join("patch.pes"), dir.join("patch.png"), dir.join("patch.json"));
    let out =
        run(&PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(vectorcraft("patch.vectorcraft"), output.clone()) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert!(std::fs::read(&output).unwrap().starts_with(b"#PES0001"));
    assert!(std::fs::read(&preview).unwrap().starts_with(b"\x89PNG"));
    // What the reader said comes first: the placement line left out by its name, and the paint of the
    // group of dots. The start marker is used, and says nothing.
    assert!(out.stderr.starts_with("info SC-I0811 (vc:10): ") && out.stderr.contains("\nwarning SC-W0807 (vc:7): "), "{}", out.stderr);
    assert!(!out.stderr.contains("(vc:4)"), "{}", out.stderr);
    let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    let codes: Vec<&str> = report["diagnostics"].as_array().unwrap().iter().filter_map(|d| d["code"].as_str()).collect();
    assert!(codes.starts_with(&["SC-I0811", "SC-W0807"]), "{codes:?}");
    // The background's red, the border's blue, the stem's green and the dots' gold.
    assert!(out.stdout.contains(", 3 colour changes, "), "{}", out.stdout);
    // The same document under VectorCraft's former extension, which names the design the same.
    let old = dir.join("patch.drawcraft");
    std::fs::copy(vectorcraft("patch.vectorcraft"), &old).unwrap();
    let again = dir.join("patch-again.pes");
    assert_eq!(run(&args(old, again.clone())).status, Status::Done);
    assert_eq!(std::fs::read(&again).unwrap(), std::fs::read(&output).unwrap());
    // A file that isn't a VectorCraft document is refused as one.
    let not_vectorcraft = dir.join("not.vectorcraft");
    std::fs::write(&not_vectorcraft, b"<svg/>").unwrap();
    let refused = run(&args(not_vectorcraft, dir.join("not.pes")));
    assert_eq!(refused.status, Status::DesignErrors);
    assert!(refused.stderr.starts_with("error SC-E0806: "), "{}", refused.stderr);
}

#[test]
fn req_cli_013_the_tracing_options_are_for_a_picture_only() {
    let dir = TempDir::new("plan-tracing-options");
    let given = [
        TracingArgs { vectorcraft_cli: Some(PathBuf::from("vectorcraft-cli")), ..TracingArgs::default() },
        TracingArgs { size: Some(Mm::new(80.0).unwrap()), ..TracingArgs::default() },
        TracingArgs { colours: Some(4), ..TracingArgs::default() },
        TracingArgs { traced: Some(dir.join("traced.vectorcraft")), ..TracingArgs::default() },
    ];
    for tracing in given {
        let out = run(&PlanArgs { tracing, ..args(fixture("strokes.svg"), dir.join("strokes.pes")) });
        let usage = "stitch: --vectorcraft-cli, --size, --colours and --traced are for a picture, a PNG or JPEG file\n";
        assert_eq!((out.status, out.stderr.as_str()), (Status::Usage, usage));
    }
    assert!(!dir.join("strokes.pes").exists() && !dir.join("traced.vectorcraft").exists());
}

/// Red bars 3 mm wide down the sides of the rectangle `[x, y, width, height]`, in millimetres, as a
/// picture placed there traces: an SVG drawing of the reference machine's hoop.
fn sides([x, y, wide, tall]: [f64; 4]) -> String {
    let (width, height, right) = (REFERENCE.hoop.width.get(), REFERENCE.hoop.height.get(), x + wide - 3.0);
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{width}mm" height="{height}mm" viewBox="0 0 {width} {height}"><rect id="left" x="{x}" y="{y}" width="3" height="{tall}" fill="red"/><rect id="right" x="{right}" y="{y}" width="3" height="{tall}" fill="red"/></svg>"#
    )
}

#[test]
fn req_cli_012_shapes_along_the_sides_of_a_picture_fitted_to_the_hoop_sew_in_it() {
    let dir = TempDir::new("plan-sides");
    let planned = |rect: [f64; 4]| {
        let design = dir.join("sides.svg");
        std::fs::write(&design, sides(rect)).unwrap();
        let report = dir.join("sides.json");
        let out = run(&PlanArgs {
            report: Some(report.clone()),
            reading: ReadingArgs { auto: true, ..ReadingArgs::default() },
            ..args(design, dir.join("sides.pes"))
        });
        let report: Value = serde_json::from_slice(&std::fs::read(report).unwrap()).unwrap();
        (out, report["size_mm"].clone())
    };
    // Each bar sews as a satin column whose stitches reach past its rails by their pull compensation, the
    // overlap, 0.4 mm: bars at the hoop's own sides sew past them.
    let (width, height) = (REFERENCE.hoop.width.get(), REFERENCE.hoop.height.get());
    let (out, _) = planned([0.0, 0.0, width, height]);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.contains("\nerror SC-E0701: The design is 130.8 × 180.0 mm, "), "{}", out.stderr);
    // Where VectorCraft places a picture given no size, `file.place`'s `[x, y, width, height]`, they sew in
    // the hoop.
    let steps = trace::steps("sides.png", b"", REFERENCE, None, trace::COLOURS);
    let placed: [f64; 4] = serde_json::from_value(steps[1]["params"]["rect"].clone()).unwrap();
    let (out, size) = planned(placed.map(|points| Mm::from_points(points).unwrap().get()));
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert_eq!(size, json!([129.8, 179.0]));
}

#[test]
fn req_cli_013_without_vectorcraft_s_command_line_nothing_is_written_but_the_report() {
    let dir = TempDir::new("plan-no-vectorcraft");
    let picture = dir.join("badge.png");
    std::fs::write(&picture, b"\x89PNG\r\n\x1a\n").unwrap();
    let (output, report, traced) = (dir.join("badge.pes"), dir.join("badge.json"), dir.join("traced.vectorcraft"));
    let tracing = TracingArgs { vectorcraft_cli: Some(dir.join("no-vectorcraft-cli")), traced: Some(traced.clone()), ..TracingArgs::default() };
    let out = run(&PlanArgs { report: Some(report.clone()), tracing, ..args(picture.clone(), output.clone()) });
    assert_eq!(out.status, Status::DesignErrors);
    assert!(
        out.stderr.starts_with("error SC-E0815: VectorCraft's command line, ") && out.stderr.ends_with("nothing was written\n"),
        "{}",
        out.stderr
    );
    assert!(!output.exists() && !traced.exists());
    let said: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert_eq!((said["diagnostics"][0]["code"].as_str(), said.get("file")), (Some("SC-E0815"), None));
    // The folder VectorCraft would have saved into is gone too.
    let mut left: Vec<String> = std::fs::read_dir(dir.path()).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    left.sort();
    assert_eq!(left, ["badge.json", "badge.png"]);
    // A machine file in a folder that doesn't exist can't have VectorCraft's folder beside it.
    let nowhere = dir.join("no-such-folder").join("badge.pes");
    let out = run(&PlanArgs {
        tracing: TracingArgs { vectorcraft_cli: Some(dir.join("no-vectorcraft-cli")), ..TracingArgs::default() },
        ..args(picture, nowhere.clone())
    });
    assert_eq!(out.status, Status::Io);
    assert!(
        out.stderr.starts_with(&format!("stitch: cannot write {}: ", nowhere.display())) && out.stderr.ends_with("nothing was written\n"),
        "{}",
        out.stderr
    );
}

#[test]
fn req_cli_008_the_picture_is_of_the_file_as_written() {
    let dir = TempDir::new("plan");
    // The picture is of the file read back, as `stitch preview` of the file draws it, for the machine and
    // in the design's colours. A DST file cuts the thread before the 47 mm jump, where the plan leaves a
    // jump thread: a machine that obeys the trim command cuts there, and the reference PE800 cuts no
    // jump (MC-1).
    let (output, preview) = (dir.join("picture.dst"), dir.join("picture.png"));
    let out = run(&PlanArgs { preview: Some(preview.clone()), ..args(fixture("strokes.svg"), output) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    let sewn = sew(Source::Svg, &std::fs::read(fixture("strokes.svg")).unwrap(), Reading::default(), profiles::REFERENCE, FormatId::Dst, "picture");
    let drawn = std::fs::read(&preview).unwrap();
    let (back, plan) = (sewn.back.as_ref().unwrap(), sewn.plan.as_ref().unwrap());
    assert_eq!(drawn, picture(back, profiles::REFERENCE.trims).unwrap().png);
    assert_eq!(drawn, picture(plan, profiles::REFERENCE.trims).unwrap().png, "the jump thread the PE800 leaves");
    let cutting = |plan| picture(plan, TrimSupport::Command).unwrap().png;
    assert_ne!(cutting(back), cutting(plan), "the plan's jump thread, where the file cuts");
    let back = sewn.back.unwrap();
    assert_eq!(back.blocks.iter().map(|b| &b.thread).collect::<Vec<_>>(), sewn.plan.unwrap().blocks.iter().map(|b| &b.thread).collect::<Vec<_>>());
}

#[test]
fn a_design_with_errors_writes_only_its_report() {
    let dir = TempDir::new("plan");
    let (output, report) = (dir.join("unreadable.pes"), dir.join("unreadable.json"));
    let not_svg = dir.join("not.svg");
    std::fs::write(&not_svg, b"stitches, please").unwrap();
    let out = run(&PlanArgs { report: Some(report.clone()), ..args(not_svg, output.clone()) });
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.starts_with("error SC-E0801: "), "{}", out.stderr);
    assert!(out.stderr.ends_with("nothing was written\n"));
    assert!(!output.exists());
    let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    assert_eq!((report["diagnostics"][0]["code"].as_str(), report.get("file")), (Some("SC-E0801"), None));
    // A design with nothing to sew: a shape neither filled nor stroked.
    let unpainted = dir.join("unpainted.svg");
    std::fs::write(
        &unpainted,
        br#"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect width="10" height="10" fill="none"/></svg>"#,
    )
    .unwrap();
    let out = run(&args(unpainted, dir.join("unpainted.pes")));
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.contains("error SC-E0010: The design has nothing to stitch"), "{}", out.stderr);
}

#[test]
fn req_cli_003_an_element_left_out_with_an_error_writes_only_the_report() {
    let dir = TempDir::new("plan");
    // The engine leaves out an element whose planning runs past its work budget, with `SC-E0004`, and plans
    // the rest (`REQ-GEN-002`), as it does for a fill of 2,000 spikes after 12 s.
    let (output, preview, report) = (dir.join("left-out.pes"), dir.join("left-out.png"), dir.join("left-out.json"));
    let design = std::fs::read(fixture("strokes.svg")).unwrap();
    let mut sewn = sew(Source::Svg, &design, Reading::default(), profiles::REFERENCE, FormatId::PesV1, "strokes");
    let fill = ElementId::new("svg:patch:fill").unwrap();
    sewn.diagnostics.push(Budget::DEFAULT.meter().charge(u64::MAX).unwrap_err().diagnostic(&Budget::DEFAULT, Some(fill)));
    assert!(sewn.plan.is_some() && sewn.file.is_some());
    let asked = PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(fixture("strokes.svg"), output.clone()) };
    let (path, reading, profile, format) = (fixture("strokes.svg"), Reading::default(), profiles::REFERENCE, FormatId::PesV1);
    let run = Run { design_path: &path, source: Source::Svg, reading, name: "strokes", design: &design, profile, format, says: None, subject: None };
    let out = written(&asked, &run, sewn, &Cell::new(Phase::Plan), None);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.contains("\nerror SC-E0004 (svg:patch:fill): ") && out.stderr.ends_with("nothing was written\n"), "{}", out.stderr);
    assert!(!output.exists() && !preview.exists());
    let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
    let said: Vec<&str> = report["diagnostics"].as_array().unwrap().iter().filter_map(|d| d["code"].as_str()).collect();
    assert_eq!((said.last(), report.get("file")), (Some(&"SC-E0004"), None));
}

#[test]
fn formats_profiles_and_files_are_checked_first() {
    let dir = TempDir::new("plan");
    let dst = dir.join("strokes-as-dst.bin");
    let out = run(&PlanArgs { format: Some(Format::Dst), ..args(fixture("strokes.svg"), dst.clone()) });
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    assert!(std::fs::read(&dst).unwrap().starts_with(b"LA:strokes"));
    // What the format adds is said with the rest: DST machines cut before the 47 mm jump.
    assert!(out.stderr.ends_with("info SC-I0605: The thread will be cut at 1 place the plan does not trim: DST machines cut it before 3 or more jump records in a row, and a jump longer than 24.2 mm takes that many.\n"), "{}", out.stderr);
    assert_eq!(run(&args(fixture("strokes.svg"), dir.join("x.unknown"))).status, Status::Usage);
    assert_eq!(run(&PlanArgs { profile: Some("singer".to_string()), ..args(fixture("strokes.svg"), dir.join("x.pes")) }).status, Status::Usage);
    let missing = run(&args(fixture("missing.svg"), dir.join("x.pes")));
    assert_eq!(missing.status, Status::Io);
    assert!(missing.stderr.starts_with("stitch: cannot read "));
}

#[test]
fn req_plan_006_a_file_is_held_to_its_own_formats_colour_changes() {
    // 257 lines 0.5 mm apart, red and blue in turn: 256 thread changes, for the PE800, whose PES records
    // at most 255.
    let lines: String = (0..257)
        .map(|i| format!(r#"<path d="M 0 {} H 10" stroke="{}" fill="none"/>"#, f64::from(i) * 0.5, if i % 2 == 0 { "red" } else { "blue" }))
        .collect();
    let svg = format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="140mm" viewBox="0 0 20 140">{lines}</svg>"#);
    let sewn = |format| sew(Source::Svg, svg.as_bytes(), Reading::default(), REFERENCE, format, "many");
    let errors = |sewn: &Sewn| sewn.diagnostics.iter().filter(|d| d.severity() == Severity::Error).map(ToString::to_string).collect::<Vec<_>>();
    let pes = sewn(FormatId::PesV1);
    assert_eq!(errors(&pes), ["error SC-E0601: The design has 256 colour changes and stops, but PES v1 records at most 255."]);
    assert!(pes.file.is_none());
    // DST records 999: the file is written.
    let dst = sewn(FormatId::Dst);
    assert_eq!(errors(&dst), Vec::<String>::new());
    assert!(dst.file.is_some_and(|bytes| bytes.starts_with(b"LA:many")));
}

#[test]
fn req_cli_006_when_a_file_cannot_be_written_none_is_and_what_was_said_is_printed() {
    let dir = TempDir::new("plan");
    // A folder that does not exist: the machine file, the picture, the report, and the report of a
    // design that was refused. The other files are not written either, and what was said about the
    // design is printed before the file that could not be written.
    let nowhere = dir.join("no-such-folder").join("x");
    let output = dir.join("unwritten.pes");
    let _ = std::fs::remove_file(&output);
    let not_svg = dir.join("not-an-svg.svg");
    std::fs::write(&not_svg, b"stitches, please").unwrap();
    let cases = [
        (args(fixture("strokes.svg"), nowhere.join("strokes.pes")), nowhere.join("strokes.pes"), "SC-I0504"),
        (
            PlanArgs { preview: Some(nowhere.join("strokes.png")), ..args(fixture("strokes.svg"), output.clone()) },
            nowhere.join("strokes.png"),
            "SC-I0504",
        ),
        (
            PlanArgs { report: Some(nowhere.join("strokes.json")), ..args(fixture("strokes.svg"), output.clone()) },
            nowhere.join("strokes.json"),
            "SC-I0504",
        ),
        (PlanArgs { report: Some(nowhere.join("refused.json")), ..args(not_svg, output.clone()) }, nowhere.join("refused.json"), "SC-E0801"),
    ];
    for (asked, unwritten, said) in cases {
        let out = run(&asked);
        assert_eq!(out.status, Status::Io, "{}", out.stderr);
        assert!(out.stderr.contains(said) && out.stderr.contains(&format!("stitch: cannot write {}: ", unwritten.display())), "{}", out.stderr);
        assert!(out.stderr.ends_with("nothing was written\n") && out.stdout.is_empty(), "{}", out.stderr);
        assert!(!output.exists(), "{}", unwritten.display());
    }
}
