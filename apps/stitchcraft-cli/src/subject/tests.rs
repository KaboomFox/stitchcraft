//! `stitch plan --subject` and `stitch cutout` with stand-ins for the models: tiny ONNX models with the real
//! ones' inputs and outputs, run by the same tract runner (`conformance/fixtures/cutout/make_stand_ins.py`).
//! The stand-in U²-Net-p maps how bright the picture is, so on [`picture`] it finds its white left half, and
//! the stand-in decoder's map is the sum of the prompt's labels less 5.5: a box alone, 2 and 3, keeps
//! nothing, and a point of the subject, 1 more, keeps the whole picture.

use std::path::Path;

use serde_json::Value;
use stitchcraft_core::Code;
use stitchcraft_cutout::pins::{Ready, sha256};
use stitchcraft_cutout::run::Tract;
use stitchcraft_testkit::temp::TempDir;

use super::*;
use crate::cli::{CutoutArgs, MarkArgs, PlanArgs, ReadingArgs, SubjectArgs, TracingArgs};
use crate::commands::{Status, cutout, plan};

/// A stand-in's bytes, by name.
fn stand_in(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../conformance/fixtures/cutout/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// The stand-ins for the 3 models, and their sums under the real ones' names.
fn stand_ins(work: &mut Work) -> Result<Provided, Diagnostic> {
    let ready =
        Ready { u2netp: stand_in("stand-in-u2netp.onnx"), encoder: stand_in("stand-in-encoder.onnx"), decoder: stand_in("stand-in-decoder.onnx") };
    let sums = [("u2netp.onnx", &ready.u2netp), ("mobilesam-encoder.onnx", &ready.encoder), ("mobilesam-decoder.onnx", &ready.decoder)]
        .map(|(file, bytes)| (file.to_string(), sha256(bytes)))
        .into();
    Ok((Box::new(Tract::load(&ready, work)?), sums))
}

/// A PNG file of 320 × 320 px, white on its left half and black on its right.
fn picture() -> Vec<u8> {
    let image = image::RgbImage::from_fn(320, 320, |x, _| if x < 160 { image::Rgb([255; 3]) } else { image::Rgb([0; 3]) });
    let mut bytes = Vec::new();
    image.write_to(&mut std::io::Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
    bytes
}

/// A folder of one test's own, holding [`picture`] as `picture.png`.
fn folder(test: &str) -> TempDir {
    let dir = TempDir::new(&format!("subject-{test}"));
    std::fs::write(dir.join("picture.png"), picture()).unwrap();
    dir
}

/// A point of the subject, in the white half.
fn subject_point() -> MarkArgs {
    MarkArgs { subject_point: vec![stitchcraft_cutout::Point { x: 80.0, y: 160.0 }], ..MarkArgs::default() }
}

fn cutout_args(dir: &TempDir, marks: MarkArgs) -> CutoutArgs {
    CutoutArgs { picture: dir.join("picture.png"), output: dir.join("cut.png"), marks }
}

/// The names in `dir`, sorted.
fn left(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir).unwrap().map(|entry| entry.unwrap().file_name().into_string().unwrap()).collect();
    names.sort();
    names
}

#[test]
fn req_cli_014_stitch_cutout_writes_the_cut_out_alone_and_says_what_it_kept() {
    let dir = folder("cutout");
    let out = cutout::run_with(&cutout_args(&dir, subject_point()), &stand_ins);
    assert_eq!(out.status, Status::Done, "{}", out.stderr);
    let cut = image::open(dir.join("cut.png")).unwrap().into_rgba8();
    assert_eq!(cut.dimensions(), (320, 320));
    assert_eq!(cut.get_pixel(0, 0).0, [255, 255, 255, 159], "255 × logistic(0.5), the stand-in decoder's map");
    let picture = dir.join("picture.png");
    let cutout = dir.join("cut.png");
    assert_eq!(
        out.stdout,
        format!(
            "{}\n  cutout    {} (320 × 320 pixels)\n  crop      0,0,320,320 of the picture's 320 × 320 pixels\n",
            picture.display(),
            cutout.display()
        )
    );
    assert_eq!(
        out.stderr,
        "info SC-I1206: The subject was cut out of the box 0,0,160,320, which U²-Net-p found: 100.0 % of the picture is kept, cropped to 320 × 320 px.\n  \
         hint: To correct it, give another box with --subject-box, or mark points with --subject-point and --not-subject.\n"
    );
}

#[test]
fn req_cli_017_when_no_pixel_is_kept_nothing_is_written() {
    let dir = folder("nothing-kept");
    let out = cutout::run_with(&cutout_args(&dir, MarkArgs::default()), &stand_ins);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(
        out.stderr.starts_with("error SC-E1202: MobileSAM kept no pixel of the picture in the box 0,0,160,320, which U²-Net-p found.\n"),
        "{}",
        out.stderr
    );
    assert!(out.stderr.ends_with("  more: stitch explain SC-E1202\nnothing was written\n"), "{}", out.stderr);
    assert_eq!(left(dir.path()), ["picture.png"]);
}

#[test]
fn req_cli_015_marks_that_dont_fit_the_picture_and_a_picture_that_doesnt_read_are_refused() {
    let dir = folder("marks");
    let outside = MarkArgs { not_subject: vec![stitchcraft_cutout::Point { x: 400.0, y: 10.0 }], ..subject_point() };
    let out = cutout::run_with(&cutout_args(&dir, outside), &stand_ins);
    assert_eq!(
        (out.status, out.stderr.as_str()),
        (Status::Usage, "stitch: --not-subject: the point 400,10 lies outside the picture, which is 320 × 320 px\n")
    );
    let past = MarkArgs { subject_box: Some(stitchcraft_cutout::PixelBox { x0: 0.0, y0: 0.0, x1: 321.0, y1: 10.0 }), ..MarkArgs::default() };
    let out = cutout::run_with(&cutout_args(&dir, past), &stand_ins);
    assert_eq!(
        (out.status, out.stderr.as_str()),
        (Status::Usage, "stitch: --subject-box: the box 0,0,321,10 reaches past the picture, which is 320 × 320 px\n")
    );
    // Not a picture: SC-E1203, before the models.
    std::fs::write(dir.join("picture.png"), b"<svg/>").unwrap();
    let never = |_: &mut Work| -> Result<Provided, Diagnostic> { panic!("the models are never needed") };
    let out = cutout::run_with(&cutout_args(&dir, subject_point()), &never);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.starts_with("error SC-E1203: The picture could not be read for the subject cut-out: "), "{}", out.stderr);
    // The cut-out and the picture are 2 files.
    let same = CutoutArgs { output: dir.join("picture.png"), ..cutout_args(&dir, MarkArgs::default()) };
    assert_eq!(cutout::run_with(&same, &stand_ins).status, Status::Usage);
    let missing = CutoutArgs { picture: dir.join("missing.png"), ..cutout_args(&dir, MarkArgs::default()) };
    assert_eq!(cutout::run_with(&missing, &stand_ins).status, Status::Io);
    assert_eq!(left(dir.path()), ["picture.png"]);
}

fn plan_args(dir: &TempDir, subject: SubjectArgs) -> PlanArgs {
    PlanArgs {
        design: dir.join("picture.png"),
        output: dir.join("picture.pes"),
        profile: None,
        format: None,
        preview: None,
        report: Some(dir.join("picture.json")),
        reading: ReadingArgs::default(),
        // No VectorCraft here: tracing the cut-out fails, after it is made.
        tracing: TracingArgs { vectorcraft_cli: Some(dir.join("no-vectorcraft-cli")), ..TracingArgs::default() },
        subject,
    }
}

#[test]
fn req_cli_014_stitch_plan_subject_cuts_the_picture_out_before_vectorcraft_traces_it() {
    let dir = folder("plan");
    let subject = SubjectArgs { subject: true, marks: subject_point(), cutout: Some(dir.join("cut.png")) };
    let out = plan::run_with(&plan_args(&dir, subject), &stand_ins);
    // The cut-out is said first, and VectorCraft's command line, which isn't there, after it.
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.starts_with("info SC-I1206: The subject was cut out of the box 0,0,160,320, which U²-Net-p found: "), "{}", out.stderr);
    assert!(out.stderr.contains("\nerror SC-E0815: VectorCraft's command line, ") && out.stderr.ends_with("nothing was written\n"), "{}", out.stderr);
    // Only the report is written, and it records the cut-out: the models' sums, the box, the points.
    assert_eq!(left(dir.path()), ["picture.json", "picture.png"]);
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("picture.json")).unwrap()).unwrap();
    let codes: Vec<&str> = report["diagnostics"].as_array().unwrap().iter().map(|d| d["code"].as_str().unwrap()).collect();
    assert_eq!(codes, ["SC-I1206", "SC-E0815"]);
    let record: Record = serde_json::from_value(report["subject"].clone()).unwrap();
    assert_eq!(
        record.models.iter().map(|m| m.file.as_str()).collect::<Vec<_>>(),
        ["u2netp.onnx", "mobilesam-encoder.onnx", "mobilesam-decoder.onnx"]
    );
    assert_eq!(record.models[2].sha256, sha256(&stand_in("stand-in-decoder.onnx")));
    assert_eq!((record.subject_box, record.box_from.as_str(), record.crop), ([0.0, 0.0, 160.0, 320.0], "u2netp", [0, 0, 320, 320]));
    assert_eq!((record.subject_points, record.not_subject), (vec![[80.0, 160.0]], vec![]));
}

#[test]
fn req_cli_017_no_subject_found_writes_only_the_report() {
    let dir = folder("plan-nothing");
    let out = plan::run_with(&plan_args(&dir, SubjectArgs { subject: true, ..SubjectArgs::default() }), &stand_ins);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.starts_with("error SC-E1202: MobileSAM kept no pixel"), "{}", out.stderr);
    assert_eq!(left(dir.path()), ["picture.json", "picture.png"]);
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("picture.json")).unwrap()).unwrap();
    assert_eq!((report["diagnostics"][0]["code"].as_str(), report.get("subject")), (Some("SC-E1202"), None));
}

#[test]
fn req_cli_015_the_cut_outs_options_are_for_a_picture_and_take_subject() {
    let dir = folder("plan-options");
    let svg = dir.join("design.svg");
    std::fs::write(&svg, br#"<svg xmlns="http://www.w3.org/2000/svg" width="10mm" height="10mm"/>"#).unwrap();
    let not_picture =
        PlanArgs { design: svg, tracing: TracingArgs::default(), ..plan_args(&dir, SubjectArgs { subject: true, ..SubjectArgs::default() }) };
    let out = plan::run_with(&not_picture, &stand_ins);
    assert_eq!((out.status, out.stderr.as_str()), (Status::Usage, "stitch: --subject and its options are for a picture, a PNG or JPEG file\n"));
    let marks_alone = [
        SubjectArgs { marks: subject_point(), ..SubjectArgs::default() },
        SubjectArgs {
            marks: MarkArgs { not_subject: vec![stitchcraft_cutout::Point { x: 1.0, y: 1.0 }], ..MarkArgs::default() },
            ..SubjectArgs::default()
        },
        SubjectArgs {
            marks: MarkArgs { subject_box: Some(stitchcraft_cutout::PixelBox { x0: 0.0, y0: 0.0, x1: 1.0, y1: 1.0 }), ..MarkArgs::default() },
            ..SubjectArgs::default()
        },
        SubjectArgs { cutout: Some(dir.join("cut.png")), ..SubjectArgs::default() },
    ];
    for subject in marks_alone {
        let out = plan::run_with(&plan_args(&dir, subject), &stand_ins);
        assert_eq!(
            (out.status, out.stderr.as_str()),
            (Status::Usage, "stitch: --subject-box, --subject-point, --not-subject and --cutout take --subject\n")
        );
    }
    // A mark outside the picture is a usage error once the picture is read, and so is the cut-out named as
    // another file written.
    let outside = SubjectArgs {
        subject: true,
        marks: MarkArgs { subject_point: vec![stitchcraft_cutout::Point { x: 10.0, y: 320.0 }], ..MarkArgs::default() },
        ..SubjectArgs::default()
    };
    assert_eq!(plan::run_with(&plan_args(&dir, outside), &stand_ins).status, Status::Usage);
    let same = SubjectArgs { subject: true, cutout: Some(dir.join("picture.json")), ..SubjectArgs::default() };
    assert_eq!(plan::run_with(&plan_args(&dir, same), &stand_ins).status, Status::Usage);
    assert_eq!(left(dir.path()), ["design.svg", "picture.png"]);
}

#[test]
fn diag_sc_e1203_a_picture_that_doesnt_read_is_said_and_only_the_report_written() {
    let dir = folder("plan-unreadable");
    std::fs::write(dir.join("picture.png"), b"\x89PNG\r\n\x1a\n and no more").unwrap();
    let out = plan::run_with(&plan_args(&dir, SubjectArgs { subject: true, ..SubjectArgs::default() }), &stand_ins);
    assert_eq!(out.status, Status::DesignErrors);
    assert!(out.stderr.starts_with("error SC-E1203: The picture could not be read for the subject cut-out: "), "{}", out.stderr);
    let report: Value = serde_json::from_slice(&std::fs::read(dir.join("picture.json")).unwrap()).unwrap();
    assert_eq!(report["diagnostics"][0]["code"].as_str(), Some(Code::CutoutPictureUnreadable.id()));
}
