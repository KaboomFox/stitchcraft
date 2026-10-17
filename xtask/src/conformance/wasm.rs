//! The golden files made as WebAssembly (`cargo xtask wasm --goldens`, `xtask/src/wasm.rs`).
//!
//! **Why.** The engine promises the same bytes on every platform (ADR-0004), and VectorCraft runs it as
//! WebAssembly, so the platform that matters most is the one the native test runs on Linux, macOS and
//! Windows never reach. Here `stitch`, built for WASI, makes every golden file of every test-sheet and plan
//! case under wasmtime, and each must be the golden file byte for byte. WASI gives `stitch` the files it
//! reads and writes, which the plug-in's own target lacks; both run WebAssembly's arithmetic, and the
//! engine calls no maths of the platform's (`clippy.toml`), so the two make the same numbers.
//!
//! The cases are the conformance suite's own (`conformance/cases/`), so a new golden file is checked here
//! the day it is added. Each is made with the command line the case stands for: `stitch testsheet` for a
//! sheet, `stitch plan` for a design, with the case's profile, into a file named as its golden file.

use std::ffi::OsString;
use std::path::Path;
use std::process::Command;

use super::cases::{self, Spec};
use crate::util::{self, Findings};

/// Where the WebAssembly build writes the machine files it makes, one folder per case.
const MADE: &str = "target/wasm-goldens";

/// The arguments with which `stitch` writes the machine file of `spec` to `out`, an SVG design read from
/// under `conformance`; none for a case that makes no machine file.
fn arguments(spec: &Spec, conformance: &Path, out: &Path) -> Option<Vec<OsString>> {
    let (command, profile): ([OsString; 2], &str) = match spec {
        Spec::Testsheet { sheet, profile, .. } => (["testsheet".into(), sheet.into()], profile),
        Spec::Plan { design, profile, .. } => (["plan".into(), conformance.join(design).into()], profile),
        Spec::Oracle { .. } => return None,
    };
    Some(command.into_iter().chain(["--profile".into(), profile.into(), "-o".into(), out.into()]).collect())
}

/// Runs `stitch`, a WASI module, under wasmtime for each golden file of each case that makes one, and
/// compares the bytes: the number of golden files it made alike, or what went wrong.
pub fn made_as_wasm(stitch: &Path) -> Result<usize, String> {
    let root = util::root();
    let conformance = root.join("conformance");
    let made = root.join(MADE);
    if made.exists() {
        std::fs::remove_dir_all(&made).map_err(|e| format!("{MADE}: {e}"))?;
    }
    let mut findings = Findings::default();
    let cases = cases::load_cases(&root, &mut findings);
    let mut alike = 0;
    for case in &cases {
        let (Spec::Testsheet { expect, .. } | Spec::Plan { expect, .. }) = &case.spec else { continue };
        let folder = made.join(&case.id);
        std::fs::create_dir_all(&folder).map_err(|e| format!("{}: {e}", util::rel(&folder)))?;
        for golden in &expect.golden {
            let name = Path::new(golden).file_name().ok_or_else(|| format!("{}: golden file `{golden}` has no name", case.file))?;
            let out = folder.join(name);
            let Some(arguments) = arguments(&case.spec, &conformance, &out) else { continue };
            // The repository's root, seen at the same path inside, so that every path means the same.
            let mut run = Command::new("wasmtime");
            run.arg("run").arg("--dir").arg(format!("{0}::{0}", root.display())).arg(stitch).args(&arguments).current_dir(&root);
            let ran = run.output().map_err(|e| format!("wasmtime: {e}"))?;
            if !ran.status.success() {
                // The last lines it wrote say why: a panic's message, or wasmtime's trap.
                let said = String::from_utf8_lossy(&ran.stderr);
                let last: Vec<&str> = said.lines().rev().take(4).collect();
                let why = last.into_iter().rev().collect::<Vec<_>>().join(" / ");
                findings.error(format!("{}: stitch, as WebAssembly, for {golden}: {} ({why})", case.file, ran.status));
                continue;
            }
            match (std::fs::read(&out), std::fs::read(conformance.join(golden))) {
                (Ok(ours), Ok(theirs)) if ours == theirs => alike += 1,
                (Ok(_), Ok(_)) => findings.error(format!("{}: {golden} differs when made as WebAssembly ({})", case.file, util::rel(&out))),
                (Err(e), _) | (_, Err(e)) => findings.error(format!("{}: {golden}: {e}", case.file)),
            }
        }
    }
    findings.finish("wasm goldens", &format!("{alike} golden files made as WebAssembly, byte for byte"))?;
    Ok(alike)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect() -> cases::SheetExpect {
        cases::SheetExpect { size_mm: [1.0, 1.0], diagnostics: Vec::new(), golden: vec!["golden/x.pes".into()] }
    }

    #[test]
    fn each_case_is_made_with_the_command_line_it_stands_for() {
        let (conformance, out) = (Path::new("repo/conformance"), Path::new("repo/target/wasm-goldens/case/x.pes"));
        // The paths as the platform joins them, and the words as given.
        let line = |command: &str, file: OsString, profile: &str| -> Vec<OsString> {
            vec![command.into(), file, "--profile".into(), profile.into(), "-o".into(), out.into()]
        };
        let sheet = Spec::Testsheet { sheet: "TS-03".into(), profile: "brother-pe800-5x7".into(), expect: expect() };
        assert_eq!(arguments(&sheet, conformance, out).unwrap(), line("testsheet", "TS-03".into(), "brother-pe800-5x7"));
        let plan = Spec::Plan { design: "fixtures/svg/strokes.svg".into(), profile: "p".into(), expect: expect() };
        assert_eq!(arguments(&plan, conformance, out).unwrap(), line("plan", conformance.join("fixtures/svg/strokes.svg").into(), "p"));
        assert!(arguments(&Spec::Oracle { files: vec!["golden/x.pes".into()] }, conformance, out).is_none());
    }
}
