//! `cargo xtask filesize`: small modules with one job (`docs/src/design/guardrails.md`).
//!
//! Rust files warn above [`WARN`] lines and fail above [`FAIL`]. Generated files (first line containing
//! `GENERATED`) are exempt. VectorCraft has files of 3,000+ lines; we split before that happens.

use crate::util::{self, Findings};

/// Lines above which a file gets a warning.
pub const WARN: usize = 800;
/// Lines above which a file fails the check.
pub const FAIL: usize = 1500;

/// `cargo xtask filesize`.
pub fn run() -> Result<(), String> {
    let root = util::root();
    let mut findings = Findings::default();
    let mut count = 0usize;
    for dir in ["crates", "apps", "xtask", "compat"] {
        for path in util::files(&root.join(dir), &["rs"]) {
            let text = util::read(&path)?;
            if text.lines().next().is_some_and(|l| l.contains("GENERATED")) {
                continue;
            }
            count += 1;
            let lines = text.lines().count();
            let rel = util::rel(&path);
            if lines > FAIL {
                findings.error(format!("{rel}: {lines} lines (limit {FAIL}); split it by concern"));
            } else if lines > WARN {
                findings.warn(format!("{rel}: {lines} lines (warning above {WARN})"));
            }
        }
    }
    findings.finish("filesize", &format!("{count} Rust files within {FAIL} lines"))
}
