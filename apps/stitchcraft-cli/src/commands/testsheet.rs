//! `stitch testsheet`: writes a machine-checkpoint test sheet and says what to check after sewing it.
//!
//! The sheet is checked like any design before anything is written: plan invariants (a violation is a
//! StitchCraft bug, `SC-E0009`), then the profile's hoop and comfort zone (`SC-E0701`, `SC-W0702`). The
//! report prints the file's SHA-256, so a sew-out report names exactly the bytes that were sewn.

use std::fmt::Write as _;
use std::path::Path;

use stitchcraft_core::{Diagnostic, Severity};
use stitchcraft_engine::testsheets::{self, SHEETS, TestSheet};
use stitchcraft_formats::Encoded;
use stitchcraft_plan::invariants;
use stitchcraft_plan::profiles;
use stitchcraft_plan::{FormatId, MachineProfile, StitchPlan};

use super::{Outcome, Status, describe_file, describe_plan, describe_threads, output_format, render_diagnostics, write_file};
use crate::cli::TestsheetArgs;

/// Runs `stitch testsheet`.
pub fn run(args: &TestsheetArgs) -> Outcome {
    if args.list {
        return Outcome::done(SHEETS.iter().map(|s| format!("{:<7} {}\n", s.id, s.title)).collect());
    }
    let (Some(id), Some(profile_id), Some(output)) = (&args.sheet, &args.profile, &args.output) else {
        return Outcome::usage("name a sheet, a profile (--profile) and a file (-o), or use --list");
    };
    let Some(sheet) = testsheets::find(id) else {
        return Outcome::usage(format!("there is no test sheet `{id}`; `stitch testsheet --list` shows them"));
    };
    let Some(profile) = profiles::find(profile_id) else {
        return Outcome::usage(format!("there is no profile `{profile_id}`; `stitch profiles` shows them"));
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
    let mut diagnostics: Vec<Diagnostic> = invariants::check(&plan, profile).iter().map(invariants::Violation::diagnostic).collect();
    diagnostics.extend(plan.bounds().and_then(|bounds| profile.check_fit(bounds)));
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
    if let Err(outcome) = write_file(output, &bytes) {
        return outcome;
    }
    Outcome { stdout: report(sheet, profile, format, output, &plan, &bytes), stderr: render_diagnostics(&diagnostics), status: Status::Done }
}

/// What was written, what the machine will ask for, and what to check.
fn report(sheet: &TestSheet, profile: &MachineProfile, format: FormatId, output: &Path, plan: &StitchPlan, bytes: &[u8]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{} · {}", sheet.id, sheet.title);
    describe_file(&mut out, output, format, bytes);
    let _ = writeln!(out, "  profile   {} ({})", profile.id, profile.name);
    describe_plan(&mut out, plan);
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

    use super::*;
    use crate::cli::Format;
    use crate::commands::hex;

    fn args(sheet: &str, output: PathBuf, format: Option<Format>) -> TestsheetArgs {
        TestsheetArgs { sheet: Some(sheet.into()), list: false, profile: Some("brother-200x200".into()), output: Some(output), format }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("stitchcraft-cli-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    #[test]
    fn writes_a_sheet_and_reports_what_to_check() {
        let path = temp("TS-01.pes");
        let out = run(&args("ts-01", path.clone(), None));
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stderr.is_empty());
        let bytes = std::fs::read(&path).unwrap();
        assert!(bytes.starts_with(b"#PES0001"));
        assert!(out.stdout.starts_with("TS-01 · Orientation and scale\n"));
        assert!(out.stdout.contains(&format!("sha256    {}", hex(&Sha256::digest(&bytes)))));
        assert!(out.stdout.contains("size      120.0 × 120.0 mm"));
        assert!(out.stdout.contains("stitches  274 stitches, 7 jumps, 7 trims, 0 colour changes, 0 stops"));
        assert!(out.stdout.contains("  threads   1. Black (#000000), shown as Brother PEC 20 \"Black\"\n"));
        assert!(out.stdout.contains("after sewing, check:\n  - The F reads normally"));
    }

    #[test]
    fn large_sheets_warn_but_are_written() {
        let out = run(&args("TS-10B", temp("TS-10B.pes"), None));
        assert_eq!(out.status, Status::Done);
        assert!(out.stderr.starts_with("warning SC-W0702: The design is 190.0 × 150.0 mm"), "{}", out.stderr);
    }

    #[test]
    fn the_format_comes_from_the_flag_or_the_extension() {
        let out = run(&args("TS-01", temp("ts01-as-dst.bin"), Some(Format::Dst)));
        assert_eq!(out.status, Status::Done);
        assert!(std::fs::read(temp("ts01-as-dst.bin")).unwrap().starts_with(b"LA:TS-01"));
        let out = run(&args("TS-01", temp("ts01.unknown"), None));
        assert_eq!(out.status, Status::Usage);
        assert!(out.stderr.contains("cannot tell the format"));
    }

    #[test]
    fn unknown_names_are_usage_errors() {
        assert_eq!(run(&args("TS-99", temp("x.pes"), None)).status, Status::Usage);
        let mut bad_profile = args("TS-01", temp("x.pes"), None);
        bad_profile.profile = Some("singer".into());
        assert_eq!(run(&bad_profile).status, Status::Usage);
    }

    #[test]
    fn stops_are_listed_as_threads_the_machine_asks_for() {
        let out = run(&args("TS-02", temp("TS-02.pes"), None));
        assert!(out.stdout.contains("4. Emerald Green (#00673e), shown as Brother PEC 54 \"Emerald Green\" — a stop: keep the same thread"));
        assert!(out.stdout.contains("2 colour changes, 1 stop\n"));
    }

    #[test]
    fn lists_the_sheets() {
        let out = run(&TestsheetArgs { sheet: None, list: true, profile: None, output: None, format: None });
        assert!(out.stdout.starts_with("TS-01   Orientation and scale\nTS-02   "));
    }
}
