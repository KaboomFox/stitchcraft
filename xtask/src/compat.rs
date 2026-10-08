//! `cargo xtask compat`: the VectorCraft compatibility gate (`docs/src/design/compatibility-gate.md`).
//!
//! - `discover [--json]`: the latest stable release, the latest pre-release (alpha/beta/rc) and the heads
//!   of VectorCraft's `release` and `main` branches, from `git ls-remote` (no API token needed), compared
//!   with the pins in `compat/vectorcraft.toml`.
//! - `contract --ref <tag|sha> --wasm <file>`: Level A — loads our built plug-in into VectorCraft's real
//!   plug-in host at that ref and runs `compat/vc-contract`'s tests.
//! - `report`: the generated supported-versions page (roadmap step M6.6).

use std::cmp::Ordering;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::util;

const REPOSITORY: &str = "https://github.com/storytold/vectorcraft";
const PINS: &str = "compat/vectorcraft.toml";
const CONTRACT: &str = "compat/vc-contract";

/// `cargo xtask compat …`.
pub fn run(args: &[String]) -> Result<(), String> {
    let value = |flag: &str| args.windows(2).find(|w| w.first().is_some_and(|f| f == flag)).and_then(|w| w.get(1)).cloned();
    match args.first().map(String::as_str) {
        Some("discover") => discover(args.iter().any(|a| a == "--json")),
        Some("contract") => {
            let reference = value("--ref").ok_or("contract needs --ref <tag|sha>")?;
            let wasm = value("--wasm").ok_or("contract needs --wasm <file>")?;
            contract(&reference, Path::new(&wasm))
        }
        Some("report") => Err("`compat report` arrives with roadmap step M6.6".into()),
        _ => Err("usage: cargo xtask compat discover [--json] | contract --ref <tag|sha> --wasm <file>".into()),
    }
}

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

fn discover(as_json: bool) -> Result<(), String> {
    let output =
        std::process::Command::new("git").args(["ls-remote", "--tags", "--heads", REPOSITORY]).output().map_err(|e| format!("git ls-remote: {e}"))?;
    if !output.status.success() {
        return Err(format!("git ls-remote {REPOSITORY} failed: {}", String::from_utf8_lossy(&output.stderr)));
    }
    let remote = parse_ls_remote(&String::from_utf8_lossy(&output.stdout));
    let pins: toml::Table = toml::from_str(&util::read(&util::root().join(PINS))?).map_err(|e| format!("{PINS}: {e}"))?;
    let pinned = |track: &str| pins.get("tracks").and_then(|t| t.get(track)).and_then(|t| t.get("tag")).and_then(toml::Value::as_str).unwrap_or("");
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
        println!("VectorCraft ({REPOSITORY})");
        println!("  stable          {}", stable.as_deref().unwrap_or("none"));
        println!("  pre-release     {}", pre.as_deref().unwrap_or("none newer than stable"));
        println!("  release branch  {}", remote.release.as_deref().unwrap_or("none"));
        println!("  main            {}", remote.main.as_deref().unwrap_or("none"));
        println!("  pinned stable   {}", pinned("stable"));
    }
    Ok(())
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
    let pinned = "tag = \"v0.6.0\"";
    if !manifest.contains(pinned) {
        return Err(format!("{CONTRACT}/Cargo.toml no longer pins `{pinned}`; update xtask/src/compat.rs"));
    }
    std::fs::write(dest.join("Cargo.toml"), manifest.replace(pinned, &selector)).map_err(|e| format!("write manifest: {e}"))?;
    Ok(dest)
}

fn contract(reference: &str, wasm: &Path) -> Result<(), String> {
    let wasm = std::fs::canonicalize(wasm).map_err(|e| format!("{}: {e}", wasm.display()))?;
    let dir = prepare_contract(reference)?;
    let mut cmd = util::cargo();
    cmd.args(["test", "--manifest-path"])
        .arg(dir.join("Cargo.toml"))
        .env("STITCHCRAFT_PLUGIN_WASM", &wasm)
        // One shared build directory for every ref, inside the cached `target/`.
        .env("CARGO_TARGET_DIR", util::root().join("target/compat-build"));
    util::run(cmd, &format!("contract tests against VectorCraft {reference}"))?;
    println!("compat: plug-in contract holds against VectorCraft {reference}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
