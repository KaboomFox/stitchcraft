//! Helpers shared by the xtask subcommands: the repository root, file walking, running commands and
//! collecting findings.

use std::path::{Path, PathBuf};
use std::process::Command;

/// The repository root (the parent of `xtask/`).
pub fn root() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.parent().map(Path::to_path_buf).unwrap_or(dir)
}

/// Directory names never scanned.
const SKIP_DIRS: &[&str] = &["target", ".git", "book", "node_modules", "corpus"];

/// Files under `dir` (recursively, skipping build output) with one of the extensions `exts`, sorted so
/// every report is in the same order.
pub fn files(dir: &Path, exts: &[&str]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    walk(dir, exts, &mut out);
    out.sort();
    out
}

fn walk(dir: &Path, exts: &[&str], out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        if path.is_dir() {
            if !SKIP_DIRS.iter().any(|skip| name == *skip) {
                walk(&path, exts, out);
            }
        } else if path.extension().and_then(|e| e.to_str()).is_some_and(|e| exts.contains(&e)) {
            out.push(path);
        }
    }
}

/// The file's contents, or an error naming the file.
pub fn read(path: &Path) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", rel(path)))
}

/// `path` relative to the repository root, with `/` separators.
pub fn rel(path: &Path) -> String {
    let root = root();
    path.strip_prefix(&root).unwrap_or(path).to_string_lossy().replace('\\', "/")
}

/// `cargo`, as the one running xtask (respects `rustup` overrides).
pub fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// Runs `cmd` in the repository root; an error if it cannot start or exits unsuccessfully.
pub fn run(mut cmd: Command, what: &str) -> Result<(), String> {
    let status = cmd.current_dir(root()).status().map_err(|e| format!("{what}: {e}"))?;
    if status.success() { Ok(()) } else { Err(format!("`{what}` failed ({status})")) }
}

/// Whether we run in CI, where optional tools are required.
pub fn in_ci() -> bool {
    std::env::var("CI").is_ok_and(|v| !v.is_empty() && v != "false")
}

/// Whether `program args…` runs successfully (used to detect optional tools).
pub fn tool_available(program: &str, args: &[&str]) -> bool {
    Command::new(program).args(args).output().is_ok_and(|o| o.status.success())
}

/// Problems found by a check: errors fail it, warnings are printed.
#[derive(Debug, Default)]
pub struct Findings {
    /// Rule violations that fail the check.
    pub errors: Vec<String>,
    /// Problems worth printing that do not fail the check.
    pub warnings: Vec<String>,
}

impl Findings {
    /// Records a violation.
    pub fn error(&mut self, message: impl Into<String>) {
        self.errors.push(message.into());
    }

    /// Records a warning.
    pub fn warn(&mut self, message: impl Into<String>) {
        self.warnings.push(message.into());
    }

    /// Prints the findings under `title` and turns errors into the check's result.
    pub fn finish(self, title: &str, summary_ok: &str) -> Result<(), String> {
        for w in &self.warnings {
            eprintln!("warning: {w}");
        }
        for e in &self.errors {
            eprintln!("error: {e}");
        }
        if self.errors.is_empty() {
            println!("{title}: {summary_ok}");
            Ok(())
        } else {
            Err(format!("{title}: {} problem(s)", self.errors.len()))
        }
    }
}
