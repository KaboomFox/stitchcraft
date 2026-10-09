//! `stitch plan DESIGN.svg -o FILE.pes`: plans a design for a machine and writes the machine file, and a
//! picture of it and a report when asked.
//!
//! The engine does the planning (`stitchcraft_engine::plan`): it fits the plan to the machine and checks
//! it, so what comes back is written as it is. This command reads the SVG file, prints what was said about
//! the design — element by element — and writes the files. Nothing is written when the design has errors,
//! except the report, which says why.

use std::fmt::Write as _;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_plan::profiles;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan};
use stitchcraft_render::{Settings, Style};

use super::{Outcome, Status, describe_file, describe_plan, describe_threads, hex, output_format, render_diagnostics, write_file};
use crate::cli::PlanArgs;
use crate::files;

/// Runs `stitch plan`.
pub fn run(args: &PlanArgs) -> Outcome {
    let Some(profile) = profiles::find(&args.profile) else {
        return Outcome::usage(format!("there is no profile `{}`; `stitch profiles` shows them", args.profile));
    };
    let format = match output_format(args.format, &args.output) {
        Ok(format) => format,
        Err(outcome) => return outcome,
    };
    let bytes = match files::read_capped(&args.design) {
        Ok(bytes) => bytes,
        Err(e) => {
            return Outcome { stdout: String::new(), stderr: format!("stitch: cannot read {}: {e}\n", args.design.display()), status: Status::Io };
        }
    };
    let svg = match stitchcraft_svg::read(&bytes, &Budget::DEFAULT) {
        Ok(svg) => svg,
        Err(unreadable) => return refuse(args, profile, &[unreadable]),
    };
    let outcome = stitchcraft_engine::plan(&svg.design, profile, &Budget::DEFAULT);
    let mut diagnostics = svg.warnings;
    diagnostics.extend(outcome.diagnostics);
    let Some(plan) = outcome.plan else { return refuse(args, profile, &diagnostics) };
    let name = args.design.file_stem().and_then(|s| s.to_str()).unwrap_or("design");
    let machine_file = match stitchcraft_formats::encode(&plan, format, name) {
        Ok(machine_file) => machine_file,
        Err(e) => {
            diagnostics.push(e.diagnostic());
            return refuse(args, profile, &diagnostics);
        }
    };
    if let Err(outcome) = write_file(&args.output, &machine_file) {
        return outcome;
    }
    let mut stdout = summary(args, profile, format, &plan, &machine_file);
    // The default scale is in range, so there are always settings; a preview too large to draw is said.
    if let (Some(path), Some(settings)) = (&args.preview, Settings::new(Style::Realistic, Settings::DEFAULT_SCALE)) {
        match stitchcraft_render::preview(&plan, settings, &mut Budget::DEFAULT.meter()) {
            Ok(image) => {
                if let Err(outcome) = write_file(path, &image.png) {
                    return outcome;
                }
                let _ = writeln!(stdout, "  preview   {} ({} × {} pixels)", path.display(), image.width, image.height);
            }
            Err(e) => diagnostics.push(e.diagnostic()),
        }
    }
    if let Some(path) = &args.report {
        let report = report(args, profile, Some((format, &machine_file)), Some(&plan), &diagnostics);
        if let Err(outcome) = write_file(path, report.as_bytes()) {
            return outcome;
        }
        let _ = writeln!(stdout, "  report    {}", path.display());
    }
    Outcome { stdout, stderr: render_diagnostics(&diagnostics), status: Status::Done }
}

/// Nothing is written but the report, which says why.
fn refuse(args: &PlanArgs, profile: &MachineProfile, diagnostics: &[Diagnostic]) -> Outcome {
    if let Some(path) = &args.report
        && let Err(outcome) = write_file(path, report(args, profile, None, None, diagnostics).as_bytes())
    {
        return outcome;
    }
    Outcome::refuse(String::new(), diagnostics)
}

/// What was written and what the machine will ask for.
fn summary(args: &PlanArgs, profile: &MachineProfile, format: FormatId, plan: &StitchPlan, machine_file: &[u8]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{}", args.design.display());
    describe_file(&mut out, &args.output, format, machine_file);
    let _ = writeln!(out, "  profile   {} ({})", profile.id, profile.name);
    describe_plan(&mut out, plan);
    describe_threads(&mut out, plan, format.palette().map(|id| id.palette()));
    out
}

/// The report: the design, the machine file if one was written, the plan's counts and threads, and every
/// diagnostic with its element.
fn report(
    args: &PlanArgs,
    profile: &MachineProfile,
    file: Option<(FormatId, &[u8])>,
    plan: Option<&StitchPlan>,
    diagnostics: &[Diagnostic],
) -> String {
    let mut report = json!({
        "design": path(&args.design),
        "profile": profile.id,
        "diagnostics": diagnostics.iter().map(|d| json!({
            "code": d.code.to_string(),
            "severity": d.severity().label(),
            "element": d.element.as_ref().map(ToString::to_string),
            "message": d.message,
        })).collect::<Vec<Value>>(),
    });
    if let Some((format, bytes)) = file {
        report["file"] = json!({ "path": path(&args.output), "format": format.name(), "bytes": bytes.len(), "sha256": hex(&Sha256::digest(bytes)) });
    }
    if let Some(plan) = plan {
        let stats = plan.stats();
        report["stitches"] = json!(stats.stitches);
        report["jumps"] = json!(stats.jumps);
        report["trims"] = json!(stats.trims);
        report["stops"] = json!(stats.stops);
        report["color_changes"] = json!(stats.color_changes);
        if let Some(bounds) = plan.bounds() {
            report["size_mm"] = json!([round(bounds.width()), round(bounds.height())]);
        }
        report["threads"] =
            plan.color_entries().iter().map(|e| json!({ "color": e.thread.color.to_string(), "name": e.thread.name, "stop": e.stop })).collect();
    }
    let mut text = serde_json::to_string_pretty(&report).unwrap_or_default();
    text.push('\n');
    text
}

/// A path as the report shows it.
fn path(path: &Path) -> String {
    path.display().to_string()
}

/// `mm` to a tenth of a millimetre, as machine files keep it.
fn round(mm: f64) -> f64 {
    (mm * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::cli::Format;

    fn fixture(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/fixtures/svg/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stitchcraft-plan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn args(design: PathBuf, output: PathBuf) -> PlanArgs {
        PlanArgs { design, output, profile: "brother-200x200".to_string(), format: None, preview: None, report: None }
    }

    #[test]
    fn writes_the_machine_file_a_picture_and_a_report() {
        let (output, preview, report) = (temp("strokes.pes"), temp("strokes.png"), temp("strokes.json"));
        let out = run(&PlanArgs { preview: Some(preview.clone()), report: Some(report.clone()), ..args(fixture("strokes.svg"), output.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(std::fs::read(&output).unwrap().starts_with(b"#PES0001"));
        assert!(std::fs::read(&preview).unwrap().starts_with(b"\x89PNG"));
        assert!(out.stdout.contains("  stitches  118 stitches, 4 jumps, 0 trims, 2 colour changes, 0 stops\n"), "{}", out.stdout);
        assert!(out.stdout.contains("  threads   1. unnamed (#c00000), shown as Brother PEC 5 \"Red\"\n"));
        // What was said names its element, on the terminal and in the report.
        assert_eq!(
            out.stderr,
            "warning SC-W0011 (svg:patch:fill): This element is a fill, and this version of StitchCraft does not sew fills yet, so it is skipped.\n\
             info SC-I0504: A needle point less than the shortest stitch (0.3 mm) from the one before was left out.\n"
        );
        let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        assert_eq!((report["stitches"].as_u64(), report["color_changes"].as_u64()), (Some(118), Some(2)));
        assert_eq!(report["size_mm"], json!([60.0, 40.0]));
        assert_eq!(report["diagnostics"][0]["element"], "svg:patch:fill");
        assert_eq!(report["file"]["sha256"], hex(&Sha256::digest(std::fs::read(&output).unwrap())));
    }

    #[test]
    fn a_design_with_errors_writes_only_its_report() {
        let (output, report) = (temp("unreadable.pes"), temp("unreadable.json"));
        let not_svg = temp("not.svg");
        std::fs::write(&not_svg, b"stitches, please").unwrap();
        let out = run(&PlanArgs { report: Some(report.clone()), ..args(not_svg, output.clone()) });
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.starts_with("error SC-E0801: "), "{}", out.stderr);
        assert!(out.stderr.ends_with("nothing was written\n"));
        assert!(!output.exists());
        let report: Value = serde_json::from_slice(&std::fs::read(&report).unwrap()).unwrap();
        assert_eq!((report["diagnostics"][0]["code"].as_str(), report.get("file")), (Some("SC-E0801"), None));
        // A design with nothing to sew.
        let only_fill = temp("only-fill.svg");
        std::fs::write(
            &only_fill,
            br#"<svg xmlns="http://www.w3.org/2000/svg" width="20mm" height="20mm" viewBox="0 0 20 20"><rect width="10" height="10"/></svg>"#,
        )
        .unwrap();
        let out = run(&args(only_fill, temp("only-fill.pes")));
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.contains("error SC-E0010: The design has nothing to stitch"), "{}", out.stderr);
    }

    #[test]
    fn formats_profiles_and_files_are_checked_first() {
        let dst = temp("strokes-as-dst.bin");
        let out = run(&PlanArgs { format: Some(Format::Dst), ..args(fixture("strokes.svg"), dst.clone()) });
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(std::fs::read(&dst).unwrap().starts_with(b"LA:strokes"));
        assert_eq!(run(&args(fixture("strokes.svg"), temp("x.unknown"))).status, Status::Usage);
        assert_eq!(run(&PlanArgs { profile: "singer".to_string(), ..args(fixture("strokes.svg"), temp("x.pes")) }).status, Status::Usage);
        let missing = run(&args(fixture("missing.svg"), temp("x.pes")));
        assert_eq!(missing.status, Status::Io);
        assert!(missing.stderr.starts_with("stitch: cannot read "));
    }
}
