//! `stitch testsheet`: writes a machine-checkpoint test sheet and says what to check after sewing it.
//!
//! The sheet is checked like any design before anything is written: the profile's hoop and comfort zone
//! (`SC-E0701`, `SC-W0702`), then the plan invariants (a violation is a StitchCraft bug, `SC-E0009`). The
//! profile is the sheet's own, the hoop it is sewn in, unless `--profile` names another. A sheet larger
//! than that hoop is refused before the invariants run, whose positions outside the hoop would call it a
//! bug. The report prints the file's SHA-256, so a sew-out report names exactly the bytes that were sewn,
//! and its size and counts are the file's, read back as a machine reads it (`REQ-CLI-008`).

use std::fmt::Write as _;
use std::path::Path;

use stitchcraft_core::{Diagnostic, Severity};
use stitchcraft_engine::testsheets::{self, SHEETS, TestSheet};
use stitchcraft_formats::Encoded;
use stitchcraft_plan::invariants;
use stitchcraft_plan::profiles;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan};

use super::{Outcome, Status, describe_file, describe_plan, describe_threads, output_format, read_back, render_diagnostics, write_files};
use crate::cli::TestsheetArgs;

/// Runs `stitch testsheet`.
pub fn run(args: &TestsheetArgs) -> Outcome {
    if args.list {
        return Outcome::done(SHEETS.iter().map(|s| format!("{:<7} {}\n", s.id, s.title)).collect());
    }
    let (Some(id), Some(output)) = (&args.sheet, &args.output) else {
        return Outcome::usage("name a sheet and a file (-o), or use --list");
    };
    let Some(sheet) = testsheets::find(id) else {
        return Outcome::usage(format!("there is no test sheet `{id}`; `stitch testsheet --list` shows them"));
    };
    let profile = match args.profile.as_deref().map(|id| profiles::find(id).ok_or(id)) {
        None => sheet.profile,
        Some(Ok(profile)) => profile,
        Some(Err(id)) => return Outcome::usage(format!("there is no profile `{id}`; `stitch profiles` shows them")),
    };
    let format = match output_format(args.format, output) {
        Ok(format) => format,
        Err(outcome) => return outcome,
    };

    let plan = match sheet.plan() {
        Ok(plan) => plan,
        Err(e) => {
            let bug = Diagnostic::new(stitchcraft_core::Code::InternalCheckFailed, format!("Test sheet {} could not be drawn: {e}.", sheet.id));
            return Outcome::refuse(String::new(), &[bug]);
        }
    };
    let fit = plan.bounds().and_then(|bounds| profile.check_fit(bounds));
    if let Some(larger) = fit.as_ref().filter(|d| d.severity() == Severity::Error) {
        return Outcome::refuse(String::new(), std::slice::from_ref(larger));
    }
    let mut diagnostics: Vec<Diagnostic> = invariants::check(&plan, profile).iter().map(invariants::Violation::diagnostic).collect();
    diagnostics.extend(fit);
    if diagnostics.iter().any(|d| d.severity() == Severity::Error) {
        return Outcome::refuse(String::new(), &diagnostics);
    }
    let Encoded { bytes, notes } = match stitchcraft_formats::encode(&plan, format, sheet.id) {
        Ok(encoded) => encoded,
        Err(e) => {
            diagnostics.push(e.diagnostic());
            return Outcome::refuse(String::new(), &diagnostics);
        }
    };
    diagnostics.extend(notes);
    let back = match read_back(&bytes, &plan) {
        Ok(back) => back,
        Err(bug) => return Outcome::refuse(String::new(), &[bug]),
    };
    let said = render_diagnostics(&diagnostics);
    let stdout = report(sheet, profile, format, output, (&plan, &back), &bytes);
    if let Err(unwritten) = write_files(&[(output, &bytes)], &said) {
        return unwritten;
    }
    Outcome { stdout, stderr: said, status: Status::Done }
}

/// What is written, read `back`, what the machine will ask for of the `plan`, and what to check.
fn report(
    sheet: &TestSheet,
    profile: &MachineProfile,
    format: FormatId,
    output: &Path,
    (plan, back): (&StitchPlan, &StitchPlan),
    bytes: &[u8],
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} · {}", sheet.id, sheet.title);
    describe_file(&mut out, output, format, bytes);
    let _ = writeln!(out, "  profile   {} ({})", profile.id, profile.name);
    describe_plan(&mut out, back);
    describe_threads(&mut out, plan, format.palette().map(|id| id.palette()));
    let _ = writeln!(out, "after sewing, check:");
    for check in sheet.checks {
        let _ = writeln!(out, "  - {check}");
    }
    out
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use sha2::{Digest, Sha256};

    use stitchcraft_testkit::temp::TempDir;

    use super::*;
    use crate::cli::Format;
    use crate::commands::hex;

    fn args(sheet: &str, output: PathBuf, format: Option<Format>) -> TestsheetArgs {
        TestsheetArgs { sheet: Some(sheet.into()), list: false, profile: None, output: Some(output), format }
    }

    #[test]
    fn writes_a_sheet_and_reports_what_to_check() {
        let dir = TempDir::new("testsheet");
        let path = dir.join("TS-01.pes");
        let out = run(&args("ts-01", path.clone(), None));
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stderr.is_empty());
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"#PES0001"));
        assert!(out.stdout.starts_with("TS-01 · Orientation and scale\n"));
        assert!(out.stdout.contains(&format!("sha256    {}", hex(&Sha256::digest(&bytes)))));
        assert!(out.stdout.contains("size      120.0 × 120.0 mm"));
        // The file's counts: the sheet's plan ends with a trim, which a PES file has no record for.
        assert!(out.stdout.contains("stitches  274 stitches, 7 jumps, 6 trims, 0 colour changes, 0 stops"));
        assert!(out.stdout.contains("  threads   1. Black (#000000), shown as Brother PEC 20 \"Black\"\n"));
        assert!(out.stdout.contains("after sewing, check:\n  - The F reads normally"));
    }

    #[test]
    fn a_sheet_is_checked_against_its_own_hoop_unless_told_otherwise() {
        let dir = TempDir::new("testsheet");
        let out = run(&args("TS-10C", dir.join("TS-10C.pes"), None));
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stdout.contains("  profile   brother-pe800-small (Brother PE800 with its small hoop)\n"), "{}", out.stdout);
        // Too large for the profile named: refused, and nothing written.
        let mut small = args("TS-10B", dir.join("TS-10B-small.pes"), None);
        small.profile = Some("brother-pe800-small".into());
        let out = run(&small);
        assert_eq!(out.status, Status::DesignErrors);
        assert!(
            out.stderr
                .starts_with("error SC-E0701: The design is 130.0 × 180.0 mm, but the Brother PE800 with its small hoop sews at most 20 × 60 mm."),
            "{}",
            out.stderr
        );
        assert!(!dir.join("TS-10B-small.pes").exists());
    }

    #[test]
    fn the_format_comes_from_the_flag_or_the_extension() {
        let dir = TempDir::new("testsheet");
        let out = run(&args("TS-01", dir.join("ts01-as-dst.bin"), Some(Format::Dst)));
        assert_eq!(out.status, Status::Done);
        assert!(std::fs::read(dir.join("ts01-as-dst.bin")).unwrap().starts_with(b"LA:TS-01"));
        let out = run(&args("TS-01", dir.join("ts01.unknown"), None));
        assert_eq!(out.status, Status::Usage);
        assert!(out.stderr.contains("cannot tell the format"));
    }

    #[test]
    fn unknown_names_are_usage_errors() {
        let dir = TempDir::new("testsheet");
        assert_eq!(run(&args("TS-99", dir.join("x.pes"), None)).status, Status::Usage);
        let mut bad_profile = args("TS-01", dir.join("x.pes"), None);
        bad_profile.profile = Some("singer".into());
        assert_eq!(run(&bad_profile).status, Status::Usage);
    }

    #[test]
    fn stops_are_listed_as_threads_the_machine_asks_for() {
        let dir = TempDir::new("testsheet");
        let out = run(&args("TS-02", dir.join("TS-02.pes"), None));
        assert!(out.stdout.contains("4. Emerald Green (#00673e), shown as Brother PEC 54 \"Emerald Green\" — a stop: keep the same thread"));
        assert!(out.stdout.contains("2 colour changes, 1 stop\n"));
    }

    #[test]
    fn lists_the_sheets() {
        let out = run(&TestsheetArgs { sheet: None, list: true, profile: None, output: None, format: None });
        assert!(out.stdout.starts_with("TS-01   Orientation and scale\nTS-02   "));
    }
}
