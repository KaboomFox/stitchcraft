//! `cargo xtask compat`: the VectorCraft compatibility gate (`docs/src/design/compatibility-gate.md`).
//!
//! - `discover [--json]`: the latest stable release, the latest pre-release (alpha/beta/rc) and the heads
//!   of VectorCraft's `release` and `main` branches, from `git ls-remote` (no API token needed), compared
//!   with the pins in `compat/vectorcraft.toml`.
//! - `contract --ref <tag|sha>`: Level A — builds the plug-in's modules for wasm32, *hello*, one for each
//!   stitch family and one for each machine format, prints their sizes, loads them into VectorCraft's real
//!   plug-in host at that ref and runs `compat/vc-contract`'s tests.
//! - `e2e (--ref <tag> | --cli <file>)`: Level B — runs the scenarios of `compat/scenarios/` with a
//!   release's `vectorcraft-cli`, fetched and checked by its SHA-256 (`release.rs`), or with one built from
//!   a branch (`scenario.rs`).
//! - `track <track> --ref <tag|sha> [--cli <file>]`: both levels on one track, and its result in
//!   `target/compat/results/`, for the report.
//! - `report`: what a scheduled run does about the results: an issue for each failing track, and the
//!   adoption of a new release that passes, which writes its pins and its row in `compat/supported.toml`
//!   (`report.rs`). The supported-versions page is generated from that record.
//! - `issue <json>`: files one of the issues `report` decides with GitHub's `gh`, on the open issue with its
//!   title when there is one.
//! - `join [--nested] <vectorcraft checkout>`: makes this repository, copied into a VectorCraft checkout
//!   as a folder, part of VectorCraft's build (`join.rs`; ADR-0011).
//! - `window [--app <file>]`: runs `compat/vc-window`'s tests against VectorCraft's window, the pinned
//!   stable release's by default, in a private desktop without a screen (`window.rs`; spike M0.8).

pub mod release;
pub mod report;
pub mod scenario;

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use serde_json::json;
use stitchcraft_vc_plugin::Module;
use stitchcraft_vc_plugin::manifest::BudgetClass;

use crate::{util, wasm};
use report::{Level, TrackResult};

pub(crate) const PINS: &str = "compat/vectorcraft.toml";
pub(crate) const CONTRACT: &str = "compat/vc-contract";

/// A SemVer version (`1.2.3` or `1.2.3-rc.1`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    core: [u64; 3],
    pre: Vec<String>,
}

impl Version {
    /// Parses `v1.2.3[-pre]` or `1.2.3[-pre]`; build metadata is ignored.
    pub fn parse(text: &str) -> Option<Version> {
        let text = text.strip_prefix('v').unwrap_or(text);
        let text = text.split('+').next().unwrap_or(text);
        let (core, pre) = match text.split_once('-') {
            Some((c, p)) => (c, p.split('.').map(str::to_string).collect()),
            None => (text, Vec::new()),
        };
        let mut numbers = core.split('.').map(|n| n.parse::<u64>().ok());
        let version = Version { core: [numbers.next()??, numbers.next()??, numbers.next()??], pre };
        numbers.next().is_none().then_some(version)
    }

    /// Whether this is a pre-release (alpha, beta, rc, …).
    pub fn is_pre(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                for (a, b) in self.pre.iter().zip(&other.pre) {
                    let order = match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(x), Ok(y)) => x.cmp(&y),
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => a.cmp(b),
                    };
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        })
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// What `git ls-remote` tells us about the repository.
#[derive(Debug, Default, PartialEq)]
pub struct Remote {
    /// (tag name, version) for every SemVer tag.
    pub tags: Vec<(String, Version)>,
    /// Commit of `refs/heads/release`.
    pub release: Option<String>,
    /// Commit of `refs/heads/main`.
    pub main: Option<String>,
}

/// Parses `git ls-remote --tags --heads` output.
pub fn parse_ls_remote(text: &str) -> Remote {
    let mut remote = Remote::default();
    for line in text.lines() {
        let Some((sha, reference)) = line.split_once('\t') else { continue };
        if let Some(tag) = reference.strip_prefix("refs/tags/") {
            let tag = tag.trim_end_matches("^{}");
            if let Some(v) = Version::parse(tag)
                && !remote.tags.iter().any(|(t, _)| t == tag)
            {
                remote.tags.push((tag.to_string(), v));
            }
        } else if reference == "refs/heads/release" {
            remote.release = Some(sha.to_string());
        } else if reference == "refs/heads/main" {
            remote.main = Some(sha.to_string());
        }
    }
    remote
}

impl Remote {
    fn latest(&self, pre: bool) -> Option<&(String, Version)> {
        self.tags.iter().filter(|(_, v)| v.is_pre() == pre).max_by(|a, b| a.1.cmp(&b.1))
    }
}

pub fn discover(as_json: bool) -> Result<(), String> {
    let pins = load_pins()?;
    let repository = repository(&pins)?;
    let output =
        std::process::Command::new("git").args(["ls-remote", "--tags", "--heads", repository]).output().map_err(|e| format!("git ls-remote: {e}"))?;
    if !output.status.success() {
        return Err(format!("git ls-remote {repository} failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let remote = parse_ls_remote(&String::from_utf8_lossy(&output.stdout));
    let pinned = |track: &str| pinned_tag(&pins, track);
    let stable = remote.latest(false).map(|(t, _)| t.clone());
    let pre = remote.latest(true).map(|(t, _)| t.clone());
    // A pre-release older than the latest stable is not news.
    let pre = pre.filter(|p| match (Version::parse(p), stable.as_deref().and_then(Version::parse)) {
        (Some(p), Some(s)) => p > s,
        _ => true,
    });
    let report = json!({
        "stable": { "latest": stable, "pinned": pinned("stable"), "new": stable.as_deref().is_some_and(|s| s != pinned("stable")) },
        "prerelease": { "latest": pre, "pinned": pinned("prerelease"), "new": pre.as_deref().is_some_and(|p| p != pinned("prerelease")) },
        "release-branch": { "sha": remote.release },
        "main": { "sha": remote.main },
    });
    if as_json {
        println!("{report}");
    } else {
        println!("VectorCraft ({repository})");
        println!("  stable          {}", stable.as_deref().unwrap_or("none"));
        println!("  pre-release     {}", pre.as_deref().unwrap_or("none newer than stable"));
        println!("  release branch  {}", remote.release.as_deref().unwrap_or("none"));
        println!("  main            {}", remote.main.as_deref().unwrap_or("none"));
        println!("  pinned stable   {}", pinned("stable"));
    }
    Ok(())
}

/// The pins file.
pub(crate) fn load_pins() -> Result<toml::Table, String> {
    toml::from_str(&util::read(&util::root().join(PINS))?).map_err(|e| format!("{PINS}: {e}"))
}

/// VectorCraft's repository, which `pins` names: where discovery looks, releases come from, and changes are
/// compared.
pub(crate) fn repository(pins: &toml::Table) -> Result<&str, String> {
    pins.get("repository").and_then(toml::Value::as_str).ok_or(format!("{PINS}: no `repository`"))
}

/// The tag pinned for `track` (empty when the track pins none).
pub(crate) fn pinned_tag<'a>(pins: &'a toml::Table, track: &str) -> &'a str {
    pins.get("tracks").and_then(|t| t.get(track)).and_then(|t| t.get("tag")).and_then(toml::Value::as_str).unwrap_or("")
}

/// A copy of the contract crate whose VectorCraft dependencies point at `reference`.
fn prepare_contract(reference: &str) -> Result<PathBuf, String> {
    let safe: String = reference.chars().map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' { c } else { '_' }).collect();
    let root = util::root();
    let dest = root.join("target/compat").join(format!("vc-contract-{safe}"));
    for dir in ["src", "tests"] {
        std::fs::create_dir_all(dest.join(dir)).map_err(|e| format!("{}: {e}", util::rel(&dest)))?;
    }
    for file in ["src/lib.rs", "tests/contract.rs"] {
        std::fs::copy(root.join(CONTRACT).join(file), dest.join(file)).map_err(|e| format!("copy {file}: {e}"))?;
    }
    let manifest = util::read(&root.join(CONTRACT).join("Cargo.toml"))?;
    let selector = if Version::parse(reference).is_some() { format!("tag = \"{reference}\"") } else { format!("rev = \"{reference}\"") };
    // The contract crate pins the same stable tag as compat/vectorcraft.toml (one pin, checked here).
    let pinned = format!("tag = \"{}\"", pinned_tag(&load_pins()?, "stable"));
    if !manifest.contains(&pinned) {
        return Err(format!("{CONTRACT}/Cargo.toml must pin the stable tag of {PINS} (`{pinned}`)"));
    }
    std::fs::write(dest.join("Cargo.toml"), manifest.replace(&pinned, &selector)).map_err(|e| format!("write manifest: {e}"))?;
    Ok(dest)
}

pub fn contract(reference: &str) -> Result<(), String> {
    run_contract(&build_modules()?, reference)
}

/// Level A with the modules in `modules`: the contract crate's tests against VectorCraft at `reference`.
fn run_contract(modules: &Path, reference: &str) -> Result<(), String> {
    let dir = prepare_contract(reference)?;
    let mut cmd = util::cargo();
    // Optimized, as VectorCraft builds its host for release, so that the tests time it as users run it.
    // What they measure, such as the plug-in's budget, is printed even when they pass.
    cmd.args(["test", "--release", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .args(["--", "--show-output"])
        .env("STITCHCRAFT_PLUGINS", modules)
        // One shared build directory for every ref, inside the cached `target/`.
        .env("CARGO_TARGET_DIR", util::root().join("target/compat-build"));
    util::run(cmd, &format!("contract tests against VectorCraft {reference}"))?;
    println!("compat: plug-in contract holds against VectorCraft {reference}");
    Ok(())
}

/// `cargo xtask compat e2e`: Level B with the `vectorcraft-cli` at `cli`, or with the one in the archive of
/// the release `reference`.
pub fn e2e(reference: Option<&str>, cli: Option<&Path>) -> Result<(), String> {
    let modules = build_modules()?;
    let (cli, _) = vectorcraft_cli(reference, cli)?;
    let level = run_e2e(&cli, &modules);
    if level.passed {
        println!("compat: every scenario passes with {}", cli.display());
        Ok(())
    } else {
        Err(format!("{} scenario checks failed", level.failures.len()))
    }
}

/// The `vectorcraft-cli` to run: `cli` when it is given, else the one in the archive of the release
/// `reference`, with the archive's SHA-256. A branch's has to be built and given.
fn vectorcraft_cli(reference: Option<&str>, cli: Option<&Path>) -> Result<(PathBuf, Option<String>), String> {
    match (cli, reference) {
        (Some(cli), _) => Ok((cli.to_path_buf(), None)),
        (None, Some(tag)) if Version::parse(tag).is_some() => {
            let fetched = release::fetch(tag)?;
            Ok((fetched.root.join("bin/vectorcraft-cli"), Some(fetched.sha256)))
        }
        (None, Some(other)) => Err(format!("VectorCraft {other} is no release: build its vectorcraft-cli and name it with --cli")),
        (None, None) => Err("name a VectorCraft release with --ref, or a vectorcraft-cli with --cli".to_string()),
    }
}

/// Level B: each scenario with the `vectorcraft-cli` at `cli` and the modules in `modules`, said as it goes,
/// and in GitHub Actions as annotations.
fn run_e2e(cli: &Path, modules: &Path) -> Level {
    let outcomes = match scenario::run_all(cli, modules, &scenario::out_dir()) {
        Ok(outcomes) => outcomes,
        Err(e) => return Level { passed: false, failures: vec![e] },
    };
    let annotate = std::env::var_os("GITHUB_ACTIONS").is_some();
    let mut failures = Vec::new();
    for (name, failed) in outcomes {
        if failed.is_empty() {
            println!("compat: scenario {name} passes");
        }
        for failure in failed {
            let line = format!("{name}: {failure}");
            if annotate {
                println!("{}", util::annotation("error", "Level B", &line));
            } else {
                eprintln!("compat: scenario {line}");
            }
            failures.push(line);
        }
    }
    Level { passed: failures.is_empty(), failures }
}

/// `cargo xtask compat track`: both levels against VectorCraft `reference` on `track`. Its result is written
/// to `target/compat/results/` whatever happens, for the report.
pub fn track(track: &str, reference: &str, cli: Option<&Path>) -> Result<(), String> {
    if !report::TRACKS.contains(&track) {
        return Err(format!("there is no track `{track}`: the tracks are {}", report::TRACKS.join(", ")));
    }
    let failed = |e: String| Level { passed: false, failures: vec![e] };
    let modules = build_modules();
    let contract = match &modules {
        Ok(dir) => run_contract(dir, reference).map_or_else(failed, |()| Level { passed: true, failures: Vec::new() }),
        Err(e) => failed(e.clone()),
    };
    let mut archive_sha256 = None;
    let e2e = match (&modules, vectorcraft_cli(Some(reference), cli)) {
        (Ok(dir), Ok((cli, sha))) => {
            archive_sha256 = sha;
            run_e2e(&cli, dir)
        }
        (Err(e), _) => failed(e.clone()),
        (_, Err(e)) => failed(e),
    };
    let result = TrackResult { track: track.to_string(), reference: reference.to_string(), contract, e2e, archive_sha256 };
    let written = report::write_result(&util::root().join(report::RESULTS), &result)?;
    let said = |level: &Level| if level.passed { "passes" } else { "fails" };
    println!(
        "compat: against VectorCraft {reference} ({track}), the contract {} and the scenarios {}; result in {}",
        said(&result.contract),
        said(&result.e2e),
        util::rel(&written)
    );
    if result.passed() { Ok(()) } else { Err(format!("the gate fails against VectorCraft {reference}")) }
}

/// The plug-in's modules, by name: *hello*, built without features, the live effect of each stitch family
/// and the exporter of each machine format, each built with the feature its name names: the family's id,
/// or the format's extension ([`Module::name`]).
pub(crate) fn modules() -> Vec<&'static str> {
    Module::all().into_iter().map(Module::name).collect()
}

/// The kind of plug-in VectorCraft installs the module `name` as, which its manifest says: `exporter` for a
/// machine format's, which VectorCraft has from 0.14, `filter` for a filter's, and `effect` for the others.
pub(crate) fn kind(name: &str) -> &'static str {
    Module::all().into_iter().find(|module| module.name() == name).map_or("effect", |module| module.manifest().kind)
}

/// Whether the module `name` asks for a heavy budget, which VectorCraft 0.17 installs only once the user
/// allows it (`allowHeavy`): Stroke to Satin's.
pub(crate) fn heavy(name: &str) -> bool {
    Module::all().into_iter().find(|module| module.name() == name).is_some_and(|module| module.manifest().budget == BudgetClass::Heavy)
}

/// Builds each of the plug-in's modules for wasm32 into `target/compat/plugins/<name>.wasm`, and prints
/// their sizes, which a CI run shows as a notice: the folder.
pub(crate) fn build_modules() -> Result<PathBuf, String> {
    if !wasm::target_installed() {
        return Err(format!("the {} target is not installed: rustup target add {}", wasm::TARGET, wasm::TARGET));
    }
    let root = util::root();
    let out = root.join("target/compat/plugins");
    std::fs::create_dir_all(&out).map_err(|e| format!("{}: {e}", util::rel(&out)))?;
    let built = root.join("target").join(wasm::TARGET).join("release/stitchcraft_vc_plugin.wasm");
    let mut sizes = Vec::new();
    for name in modules() {
        let mut cmd = util::cargo();
        cmd.args(["build", "-p", "stitchcraft-vc-plugin", "--release", "--locked", "--target", wasm::TARGET]);
        if name != "hello" {
            cmd.args(["--features", name]);
        }
        util::run(cmd, &format!("cargo build the {name} plug-in for {}", wasm::TARGET))?;
        let module = out.join(format!("{name}.wasm"));
        std::fs::copy(&built, &module).map_err(|e| format!("copy the {name} plug-in to {}: {e}", util::rel(&module)))?;
        sizes.push((name, std::fs::metadata(&module).map_err(|e| format!("{}: {e}", util::rel(&module)))?.len()));
    }
    println!("::notice title=plug-in sizes::{}", sizes_line(&sizes));
    Ok(out)
}

/// The modules' sizes in KiB, and what each stitch family's adds to *hello*'s, which holds the ABI shim and
/// the JSON reader alone.
fn sizes_line(sizes: &[(&str, u64)]) -> String {
    let kib = |bytes: u64| bytes.div_ceil(1024);
    let hello = sizes.iter().find(|(name, _)| *name == "hello").map_or(0, |(_, bytes)| *bytes);
    let each: Vec<String> = sizes
        .iter()
        .map(|(name, bytes)| match name {
            &"hello" => format!("hello {} KiB", kib(*bytes)),
            _ => format!("{name} {} KiB ({} KiB more)", kib(*bytes), kib(bytes.saturating_sub(hello))),
        })
        .collect();
    each.join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_modules_are_hello_each_family_s_and_each_format_s() {
        assert_eq!(modules(), ["hello", "stroke", "satin", "fill", "pes", "dst", "settings", "stroke-to-satin", "testsheet"]);
        let kinds: Vec<&str> = modules().into_iter().map(kind).collect();
        assert_eq!(kinds, ["effect", "effect", "effect", "effect", "exporter", "exporter", "filter", "filter", "filter"]);
        assert_eq!(modules().into_iter().filter(|name| heavy(name)).collect::<Vec<_>>(), ["stroke-to-satin"]);
        let sizes = [("hello", 1024), ("fill", 5000)];
        assert_eq!(sizes_line(&sizes), "hello 1 KiB, fill 5 KiB (4 KiB more)");
    }

    #[test]
    fn the_contract_crate_pins_the_stable_tag_of_the_pins_file() {
        let pins = load_pins().unwrap();
        let tag = pinned_tag(&pins, "stable");
        assert!(Version::parse(tag).is_some(), "stable pin `{tag}`");
        let manifest = util::read(&util::root().join(CONTRACT).join("Cargo.toml")).unwrap();
        assert!(manifest.contains(&format!("tag = \"{tag}\"")), "{CONTRACT}/Cargo.toml must pin {tag}");
    }

    #[test]
    fn semver_orders_pre_releases_before_releases() {
        let v = |s: &str| Version::parse(s).unwrap();
        assert!(v("v0.7.0-alpha.1") < v("v0.7.0-alpha.2"));
        assert!(v("v0.7.0-alpha.2") < v("v0.7.0-beta"));
        assert!(v("v0.7.0-beta") < v("v0.7.0-rc.1"));
        assert!(v("v0.7.0-rc.1") < v("v0.7.0"));
        assert!(v("v0.6.0") < v("v0.7.0-rc.1"));
        assert!(v("v0.10.0") > v("v0.9.9"));
        assert!(Version::parse("nightly").is_none() && Version::parse("v1.2").is_none());
    }

    #[test]
    fn ls_remote_tracks_are_found() {
        let text = "aaa\trefs/heads/main\nbbb\trefs/heads/release\nccc\trefs/tags/v0.6.0\nddd\trefs/tags/v0.6.0^{}\neee\trefs/tags/v0.7.0-rc.1\nfff\trefs/tags/not-a-version\n";
        let remote = parse_ls_remote(text);
        assert_eq!(remote.main.as_deref(), Some("aaa"));
        assert_eq!(remote.release.as_deref(), Some("bbb"));
        assert_eq!(remote.tags.len(), 2);
        assert_eq!(remote.latest(false).map(|(t, _)| t.as_str()), Some("v0.6.0"));
        assert_eq!(remote.latest(true).map(|(t, _)| t.as_str()), Some("v0.7.0-rc.1"));
    }
}
