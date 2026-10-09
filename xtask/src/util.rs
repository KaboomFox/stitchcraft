//! Helpers shared by the xtask subcommands: the repository root, this repository's packages, file
//! walking, running commands and collecting findings.
//!
//! The tooling works the same whether this repository is its own Cargo workspace or a folder inside
//! VectorCraft's (`docs/src/design/adr/0011-movable-into-vectorcraft.md`): files are found from the
//! folder [`root`] returns, never from the Git root or the current directory, and Cargo commands name
//! this repository's [`packages`] instead of saying `--workspace`.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

/// The repository root: the folder that holds `xtask/`, wherever that folder is.
pub fn root() -> PathBuf {
    let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.parent().map(Path::to_path_buf).unwrap_or(dir)
}

/// This repository's packages, as Cargo sees them from [`root`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Packages {
    /// Their names, in Cargo's order.
    pub names: Vec<String>,
    /// Whether the repository is a folder in a larger workspace (VectorCraft's, after
    /// `cargo xtask compat join`): the other members are not ours to build, lint or judge.
    pub guest: bool,
}

impl Packages {
    /// `-p <name>` for each package: what a Cargo command gets instead of `--workspace`.
    pub fn args(&self) -> Vec<String> {
        self.names.iter().flat_map(|name| ["-p".to_string(), name.clone()]).collect()
    }
}

/// `cargo metadata --no-deps` for the workspace [`root`] belongs to.
pub fn metadata() -> Result<Value, String> {
    let output =
        cargo().args(["metadata", "--format-version", "1", "--no-deps"]).current_dir(root()).output().map_err(|e| format!("cargo metadata: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    serde_json::from_slice(&output.stdout).map_err(|e| format!("cargo metadata output: {e}"))
}

/// This repository's packages.
pub fn packages() -> Result<Packages, String> {
    Ok(packages_in(&metadata()?, &root()))
}

/// The packages in `metadata` whose manifests are inside `folder`, and whether the workspace is larger
/// than `folder`.
pub fn packages_in(metadata: &Value, folder: &Path) -> Packages {
    let folder = canonical(folder);
    let names = metadata
        .get("packages")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|p| p.get("manifest_path").and_then(Value::as_str).is_some_and(|m| canonical(Path::new(m)).starts_with(&folder)))
        .filter_map(|p| p.get("name").and_then(Value::as_str).map(str::to_string))
        .collect();
    let guest = metadata.get("workspace_root").and_then(Value::as_str).is_some_and(|w| canonical(Path::new(w)) != folder);
    Packages { names, guest }
}

/// `path` with links resolved when it exists, as given otherwise.
fn canonical(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
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

/// A GitHub Actions workflow command showing `message` as a `level` annotation titled `title`, placed on
/// a file and line when the message starts with `path:line: `.
pub fn annotation(level: &str, title: &str, message: &str) -> String {
    // Encodings from GitHub's "Workflow commands" documentation: data escapes %, CR and LF; property
    // values also escape `:` and `,`.
    let data = |s: &str| s.replace('%', "%25").replace('\r', "%0D").replace('\n', "%0A");
    let property = |s: &str| data(s).replace(':', "%3A").replace(',', "%2C");
    let located = message.split_once(": ").and_then(|(place, rest)| {
        let (file, line) = place.rsplit_once(':')?;
        (line.parse::<u32>().is_ok() && !file.contains(' ')).then(|| (format!(",file={},line={line}", property(file)), rest))
    });
    let (location, text) = located.unwrap_or((String::new(), message));
    format!("::{level} title={}{location}::{}", property(title), data(text))
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

    /// Prints the findings under `title` and turns errors into the check's result. In GitHub Actions each
    /// finding is also an annotation, so it shows on the run's page, through the checks API and, when it
    /// starts with `path:line:`, on that line of the pull request's diff.
    pub fn finish(self, title: &str, summary_ok: &str) -> Result<(), String> {
        let annotate = std::env::var_os("GITHUB_ACTIONS").is_some();
        for w in &self.warnings {
            eprintln!("warning: {w}");
            if annotate {
                println!("{}", annotation("warning", title, w));
            }
        }
        for e in &self.errors {
            eprintln!("error: {e}");
            if annotate {
                println!("{}", annotation("error", title, e));
            }
        }
        if self.errors.is_empty() {
            println!("{title}: {summary_ok}");
            Ok(())
        } else {
            Err(format!("{title}: {} problem(s)", self.errors.len()))
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn packages_are_the_ones_in_this_folder() {
        let alone = json!({
            "workspace_root": "/r",
            "packages": [{"name": "stitchcraft-core", "manifest_path": "/r/crates/stitchcraft-core/Cargo.toml"}],
        });
        let ours = packages_in(&alone, Path::new("/r"));
        assert_eq!(ours, Packages { names: vec!["stitchcraft-core".into()], guest: false });
        assert_eq!(ours.args(), ["-p", "stitchcraft-core"]);
        let joined = json!({
            "workspace_root": "/vc",
            "packages": [
                {"name": "vectorcraft-geom", "manifest_path": "/vc/crates/geom/Cargo.toml"},
                {"name": "stitchcraft-core", "manifest_path": "/vc/stitchcraft/crates/stitchcraft-core/Cargo.toml"},
                {"name": "stitchcraft-xtask", "manifest_path": "/vc/stitchcraft/xtask/Cargo.toml"},
            ],
        });
        let ours = packages_in(&joined, Path::new("/vc/stitchcraft"));
        assert_eq!(ours, Packages { names: vec!["stitchcraft-core".into(), "stitchcraft-xtask".into()], guest: true });
    }

    #[test]
    fn findings_become_annotations_on_their_line() {
        assert_eq!(
            annotation("error", "docs", "docs/src/a.md:12: SC-W9999 is not registered"),
            "::error title=docs,file=docs/src/a.md,line=12::SC-W9999 is not registered"
        );
        assert_eq!(annotation("warning", "shots", "50% done\nnext: x"), "::warning title=shots::50%25 done%0Anext: x");
        assert_eq!(annotation("error", "a:b", "REQ-FMT-005 is active: no case"), "::error title=a%3Ab::REQ-FMT-005 is active: no case");
    }
}
