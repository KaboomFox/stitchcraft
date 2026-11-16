//! `cargo xtask prepush [--base REF]`: the checks that most often cost a round of CI, in a few minutes.
//!
//! A round of CI takes 20 to 40 minutes, and most failed rounds fail on something a short local check
//! finds: formatting, a Clippy warning, a rustdoc link, a test that compares the engine's output with
//! numbers recorded in the repository, a generated page that is stale, an API snapshot, the prose rules.
//! This command runs those and no more. It is no gate and replaces none: `cargo xtask ci` is every gate,
//! and CI runs it on every pull request.
//!
//! What it leaves out is what takes long: the whole test suite, the conformance suite's run, the docs'
//! pictures, the WebAssembly builds, coverage and mutation testing. Clippy and rustdoc run on the packages
//! the branch changed, found from the files that differ from the merge base with `REF` (default
//! `origin/main`), uncommitted and untracked files included, as the prose lint finds its lines.
//!
//! The prose lint needs Vale. Without it, `STITCHCRAFT_PROSE_STANDIN` may name a program, with arguments
//! split at spaces, that checks the same lines instead: it is run from the repository's root with `--base
//! REF` and fails the check when it fails. A stand-in is not Vale, and the summary says which one ran.

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::Value;

use crate::ci::{Check, Outcome, optional, outcome, run_checks};
use crate::{api, cleanroom, conformance, docs, filesize, layers, prose, unsafe_audit, util};

/// The environment variable naming a program to run in Vale's place where Vale is not installed.
pub const STANDIN: &str = "STITCHCRAFT_PROSE_STANDIN";

/// The tests that compare the engine's and the plug-in's output with numbers and lists recorded in the
/// repository: the test sheets' sizes and stitch counts, the Auto effect's sheets and explanations, and the
/// plug-in's manifests. A change to what sews moves them, and they are what a round's tests most often
/// fail on. Each is a package and the arguments `cargo test` gets after it.
pub const RECORDED: &[(&str, &[&str])] = &[
    ("stitchcraft-engine", &["--lib", "testsheets::"]),
    ("stitchcraft-engine", &["--lib", "auto::tests::"]),
    ("stitchcraft-engine", &["--test", "auto_sheets"]),
    ("stitchcraft-vc-plugin", &["--lib", "manifest::"]),
];

/// The folders, besides the packages' own, whose files the recorded tests read.
const RECORDED_INPUTS: &[&str] = &["conformance/", "CHANGELOG.md"];

/// The package of this command, whose tests tie the workflows, the roadmap and the records to the code.
const XTASK: &str = "stitchcraft-xtask";

/// The files that differ from the merge base with `base`, uncommitted and untracked ones included,
/// relative to the repository's root.
fn changed_files(root: &Path, base: &str) -> Result<Vec<String>, String> {
    let since = util::git(root, &["merge-base", base, "HEAD"])?.trim().to_string();
    let mut files: BTreeSet<String> = util::git(root, &["diff", "--name-only", "--relative", &since])?.lines().map(str::to_string).collect();
    files.extend(util::git(root, &["ls-files", "--others", "--exclude-standard"])?.lines().map(str::to_string));
    Ok(files.into_iter().collect())
}

/// Each of this repository's packages in `metadata` with its folder relative to `root`, ending in `/`.
fn package_folders(metadata: &Value, root: &Path) -> Vec<(String, String)> {
    let root = std::fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    let packages = metadata.get("packages").and_then(Value::as_array).into_iter().flatten();
    packages
        .filter_map(|package| {
            let name = package.get("name").and_then(Value::as_str)?;
            let folder = Path::new(package.get("manifest_path").and_then(Value::as_str)?).parent()?;
            let folder = std::fs::canonicalize(folder).unwrap_or_else(|_| folder.to_path_buf());
            let relative = folder.strip_prefix(&root).ok()?.to_str()?.replace('\\', "/");
            Some((name.to_string(), format!("{relative}/")))
        })
        .collect()
}

/// The packages that hold one of `files`, by name. A file belongs to the package whose folder is the
/// longest one around it.
fn changed_packages(folders: &[(String, String)], files: &[String]) -> Vec<String> {
    let mut changed = BTreeSet::new();
    for file in files {
        let around = folders.iter().filter(|(_, folder)| file.starts_with(folder.as_str())).max_by_key(|(_, folder)| folder.len());
        if let Some((name, _)) = around {
            changed.insert(name.clone());
        }
    }
    changed.into_iter().collect()
}

/// Whether the recorded tests have something to say about `files`: a package other than xtask changed,
/// or one of the folders the tests read.
fn recorded_tests_apply(packages: &[String], files: &[String]) -> bool {
    packages.iter().any(|name| name != XTASK) || files.iter().any(|file| RECORDED_INPUTS.iter().any(|input| file.starts_with(input)))
}

/// `cargo <command> -p <each of packages> <rest>`, with `RUSTDOCFLAGS` denying warnings for `doc`.
fn cargo_on(command: &'static str, packages: &[String], rest: &'static [&'static str]) -> Box<dyn Fn() -> Outcome> {
    let packages = packages.to_vec();
    Box::new(move || {
        if packages.is_empty() {
            return Outcome::Skipped("the branch changes no package");
        }
        let mut cmd = util::cargo();
        cmd.arg(command).args(packages.iter().flat_map(|name| ["-p", name.as_str()])).args(rest);
        if command == "doc" {
            cmd.env("RUSTDOCFLAGS", "-D warnings");
        }
        outcome(util::run(cmd, &format!("cargo {command} {} {}", packages.join(" "), rest.join(" "))))
    })
}

/// The recorded tests, one `cargo test` after another. The first that fails fails the check.
fn recorded_tests(apply: bool) -> Box<dyn Fn() -> Outcome> {
    Box::new(move || {
        if !apply {
            return Outcome::Skipped("the branch changes nothing they read");
        }
        for (package, args) in RECORDED {
            let mut cmd = util::cargo();
            cmd.args(["test", "--locked", "-p", package]).args(*args);
            if let Err(e) = util::run(cmd, &format!("cargo test -p {package} {}", args.join(" "))) {
                return Outcome::Failed(e);
            }
        }
        Outcome::Passed
    })
}

/// The prose lint: Vale where it is installed, the stand-in `standin` names where it is not.
fn prose_check(base: &str, standin: Option<String>) -> Box<dyn Fn() -> Outcome> {
    let base = base.to_string();
    Box::new(move || {
        if prose::available() {
            return outcome(prose::run(&base));
        }
        let Some(words) = standin.as_deref().map(str::split_whitespace).map(Iterator::collect::<Vec<_>>) else {
            return Outcome::Skipped(prose::INSTALL);
        };
        let Some((program, args)) = words.split_first() else {
            return Outcome::Skipped(prose::INSTALL);
        };
        let mut cmd = std::process::Command::new(program);
        cmd.args(args).args(["--base", &base]);
        outcome(util::run(cmd, &format!("{STANDIN} ({program}), which is not Vale")))
    })
}

/// `cargo xtask prepush`.
pub fn run(base: &str) -> Result<(), String> {
    let root = util::root();
    let files = changed_files(&root, base)?;
    let all = util::packages()?;
    let changed = changed_packages(&package_folders(&util::metadata()?, &root), &files);
    println!(
        "prepush: {} files differ from {base}, in the packages: {}",
        files.len(),
        if changed.is_empty() { "none".to_string() } else { changed.join(", ") }
    );
    let all_args = all.args();
    let checks: Vec<Check> = vec![
        (
            "fmt",
            Box::new(move || {
                let mut cmd = util::cargo();
                cmd.arg("fmt").args(&all_args).args(["--", "--check"]);
                outcome(util::run(cmd, "cargo fmt -- --check"))
            }),
        ),
        ("clippy", cargo_on("clippy", &changed, &["--all-targets", "--locked", "--", "-D", "warnings"])),
        ("rustdoc", cargo_on("doc", &changed, &["--no-deps", "--locked"])),
        ("recorded-tests", recorded_tests(recorded_tests_apply(&changed, &files))),
        ("xtask-tests", cargo_on("test", &[XTASK.to_string()], &["--locked"])),
        ("layers", Box::new(|| outcome(layers::run()))),
        ("filesize", Box::new(|| outcome(filesize::run()))),
        ("cleanroom", Box::new(|| outcome(cleanroom::run()))),
        ("unsafe-audit", Box::new(|| outcome(unsafe_audit::run()))),
        ("docs", Box::new(|| outcome(docs::run(true)))),
        ("conformance-check", Box::new(|| outcome(conformance::run(true, None, None, false, &conformance::Glyphs::default())))),
        ("api", optional(api::available, api::INSTALL, true, Box::new(|| outcome(api::run(true))))),
        (
            "typos",
            optional(
                || util::tool_available("typos", &["--version"]),
                "cargo install typos-cli",
                true,
                Box::new(|| {
                    let mut cmd = std::process::Command::new("typos");
                    cmd.current_dir(util::root());
                    outcome(util::run(cmd, "typos"))
                }),
            ),
        ),
        ("prose", prose_check(base, std::env::var(STANDIN).ok().filter(|value| !value.trim().is_empty()))),
    ];
    run_checks("cargo xtask prepush", &checks, &[])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folders() -> Vec<(String, String)> {
        [("stitchcraft-engine", "crates/stitchcraft-engine/"), ("stitchcraft-xtask", "xtask/"), ("stitchcraft-vc-window", "compat/vc-window/")]
            .map(|(name, folder)| (name.to_string(), folder.to_string()))
            .to_vec()
    }

    fn files(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    #[test]
    fn a_changed_file_names_the_package_around_it() {
        let changed = |names: &[&str]| changed_packages(&folders(), &files(names));
        assert_eq!(changed(&[]), Vec::<String>::new());
        assert_eq!(changed(&["docs/src/design/guardrails.md", ".github/workflows/ci.yml", "xtaskfile"]), Vec::<String>::new());
        assert_eq!(
            changed(&["xtask/src/ci.rs", "crates/stitchcraft-engine/tests/auto.rs", "xtask/Cargo.toml"]),
            ["stitchcraft-engine", "stitchcraft-xtask"]
        );
        // A package inside another's folder is the one its files belong to.
        let nested = [("outer".to_string(), "a/".to_string()), ("inner".to_string(), "a/b/".to_string())];
        assert_eq!(changed_packages(&nested, &files(&["a/b/src/lib.rs"])), ["inner"]);
        assert_eq!(changed_packages(&nested, &files(&["a/src/lib.rs"])), ["outer"]);
    }

    #[test]
    fn the_packages_folders_are_read_from_cargo_s_metadata() {
        let root = util::root();
        let found = package_folders(&util::metadata().unwrap(), &root);
        assert!(found.contains(&(XTASK.to_string(), "xtask/".to_string())), "{found:?}");
        assert!(found.contains(&("stitchcraft-engine".to_string(), "crates/stitchcraft-engine/".to_string())), "{found:?}");
    }

    #[test]
    fn the_recorded_tests_run_when_a_package_or_what_they_read_changes() {
        let apply = |packages: &[&str], names: &[&str]| recorded_tests_apply(&files(packages), &files(names));
        assert!(!apply(&[], &["docs/src/design/guardrails.md"]));
        assert!(!apply(&[XTASK], &["xtask/src/ci.rs"]));
        assert!(apply(&[XTASK, "stitchcraft-svg"], &["crates/stitchcraft-svg/src/lib.rs"]));
        assert!(apply(&[], &["conformance/cases/ts-01/case.toml"]));
        assert!(apply(&[], &["CHANGELOG.md"]));
    }

    #[test]
    fn each_recorded_test_names_a_package_and_a_target_that_exist() {
        let root = util::root();
        let found = package_folders(&util::metadata().unwrap(), &root);
        for (package, args) in RECORDED {
            let folder = &found.iter().find(|(name, _)| name == package).unwrap_or_else(|| panic!("no package {package}")).1;
            let [kind, name] = args else { panic!("{args:?}: a kind of target and a name") };
            let there = match *kind {
                "--test" => root.join(folder).join(format!("tests/{name}.rs")).is_file(),
                // A filter on the library's tests starts with a module of the library.
                "--lib" => {
                    let module = name.split("::").next().unwrap();
                    root.join(folder).join(format!("src/{module}.rs")).is_file() || root.join(folder).join(format!("src/{module}")).is_dir()
                }
                _ => false,
            };
            assert!(there, "{package} {args:?}");
        }
    }
}
