//! `stitch inspect FILE`: what is in a machine file — size, stitches, threads, stitch lengths — and,
//! with `--profile`, whether it suits a machine.
//!
//! Works on any PES, PEC or DST file, not only StitchCraft's: the readers treat every file as possibly
//! damaged or hostile, so an unreadable file is a coded diagnostic (`SC-E0603`), never a crash.

use std::fmt::Write as _;

use sha2::{Digest, Sha256};
use stitchcraft_core::Severity;
use stitchcraft_plan::{PaletteId, profiles};

use super::{Outcome, Status, describe_plan, describe_threads, hex, read_machine_file, reader_warnings, render_diagnostics};
use crate::cli::InspectArgs;

/// Runs `stitch inspect`.
pub fn run(args: &InspectArgs) -> Outcome {
    let profile = match args.profile.as_deref().map(|id| profiles::find(id).ok_or(id)) {
        Some(Err(id)) => return Outcome::usage(format!("there is no profile `{id}`; `stitch profiles` shows them")),
        Some(Ok(profile)) => Some(profile),
        None => None,
    };
    let (bytes, decoded) = match read_machine_file(&args.file) {
        Ok(read) => read,
        Err(outcome) => return outcome,
    };
    let plan = &decoded.plan;

    let mut out = String::new();
    let name = if decoded.name.is_empty() { String::new() } else { format!(" · \"{}\"", decoded.name) };
    let _ = writeln!(out, "{} · {}{name}", args.file.display(), decoded.format);
    let _ = writeln!(out, "  sha256    {}", hex(&Sha256::digest(&bytes)));
    describe_plan(&mut out, plan);
    if let Some(b) = plan.bounds() {
        let (min, max) = (b.min(), b.max());
        let _ =
            writeln!(out, "  extent    x {:.1} to {:.1} mm, y {:.1} to {:.1} mm from where the machine starts", min.x(), max.x(), min.y(), max.y());
    }
    let lengths: Vec<f64> = plan.sewn_stitches().iter().map(|s| s.length()).collect();
    if let (Some(shortest), Some(longest)) = (lengths.iter().copied().reduce(f64::min), lengths.iter().copied().reduce(f64::max)) {
        let _ = writeln!(out, "  lengths   stitches from {shortest:.1} to {longest:.1} mm");
    }
    describe_threads(&mut out, plan, decoded.palette.map(PaletteId::palette));
    if decoded.palette.is_none() {
        let _ = writeln!(out, "            ({} stores no colours: each block is a pause for the next thread)", decoded.format);
    }

    let mut diagnostics = Vec::new();
    if let Some(profile) = profile {
        let _ = writeln!(out, "  profile   {} ({})", profile.id, profile.name);
        diagnostics.extend(plan.bounds().and_then(|b| profile.check_fit(b)));
        let (min, max) = (profile.min_stitch.get(), profile.max_stitch.get());
        let long = lengths.iter().filter(|l| **l > max + 1e-9).count();
        let short = lengths.iter().filter(|l| **l < min - 1e-9 && **l > 0.0).count();
        let _ = writeln!(out, "            {long} stitches longer than {max} mm, {short} shorter than {min} mm");
    }
    let mut stderr = reader_warnings(&decoded);
    stderr.push_str(&render_diagnostics(&diagnostics));
    let status = if diagnostics.iter().any(|d| d.severity() == Severity::Error) { Status::DesignErrors } else { Status::Done };
    Outcome { stdout: out, stderr, status }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn golden(name: &str) -> PathBuf {
        PathBuf::from(format!("{}/../../conformance/golden/{name}", env!("CARGO_MANIFEST_DIR")))
    }

    fn inspect(file: PathBuf, profile: Option<&str>) -> Outcome {
        run(&InspectArgs { file, profile: profile.map(str::to_string) })
    }

    #[test]
    fn describes_a_pes_file() {
        let out = inspect(golden("testsheets/TS-02.pes"), Some("brother-200x200"));
        assert_eq!(out.status, Status::Done, "{}", out.stderr);
        assert!(out.stdout.contains("TS-02.pes · PES (#PES0001) · \"TS-02\"\n"), "{}", out.stdout);
        assert!(out.stdout.contains("  size      140.0 × 70.0 mm\n"));
        assert!(out.stdout.contains("2 colour changes, 1 stop"));
        assert!(out.stdout.contains("  lengths   stitches from 2.5 to 2.5 mm\n"));
        assert!(out.stdout.contains("  threads   1. Red (#ed171f), shown as Brother PEC 5 \"Red\"\n"));
        assert!(out.stdout.contains("0 stitches longer than 12 mm, 0 shorter than 0.3 mm"));
        assert!(out.stderr.is_empty());
    }

    #[test]
    fn describes_a_dst_file() {
        let out = inspect(golden("testsheets/TS-01.dst"), None);
        assert_eq!(out.status, Status::Done);
        assert!(out.stdout.contains("· DST · \"TS-01\""));
        assert!(out.stdout.contains("7 trims"));
        assert!(out.stdout.contains("DST stores no colours"));
    }

    #[test]
    fn large_designs_get_the_profile_diagnostics() {
        let out = inspect(golden("testsheets/TS-10C.pes"), Some("brother-200x200"));
        assert_eq!(out.status, Status::Done);
        assert!(out.stderr.starts_with("warning SC-W0702"));
    }

    #[test]
    fn bad_files_are_reported_not_crashed_on() {
        let dir = std::env::temp_dir().join(format!("stitchcraft-inspect-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("broken.pes");
        std::fs::write(&path, b"#PES0001\x16\0\0\0").unwrap();
        let out = inspect(path, None);
        assert_eq!(out.status, Status::DesignErrors);
        assert!(out.stderr.starts_with("error SC-E0603: "), "{}", out.stderr);
        assert_eq!(inspect(dir.join("missing.pes"), None).status, Status::Io);
        assert_eq!(inspect(golden("testsheets/TS-01.pes"), Some("nope")).status, Status::Usage);
    }
}
