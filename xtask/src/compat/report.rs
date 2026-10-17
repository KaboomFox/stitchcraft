//! What a scheduled run of the gate does with its results (`docs/src/design/compatibility-gate.md` ›
//! Automation): an issue for each track that fails, closed when it passes again, and a pull request that
//! adopts a new release once it passes both levels.
//!
//! **Why a record.** The supported-versions page is generated, never written by hand, from
//! `compat/supported.toml`: the releases that passed both levels, and when. Only the adopting pull request
//! adds to it, with the pins that make the release the one pull requests are checked against. So the record
//! changes by review, as the pins do, and a branch's daily result, which changes every day, stays in its
//! issue.
//!
//! **Why decisions.** `cargo xtask compat report` decides, and the workflow acts on what it prints with `gh`:
//! the rules are here, where they are tested, and the workflow holds only the calls to GitHub.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::release::SHA_KEY;
use super::{CONTRACT, PINS, Version, load_pins, pinned_tag};
use crate::util;

/// The record of the releases that passed both levels.
pub const SUPPORTED: &str = "compat/supported.toml";
/// Where each track's job writes its result.
pub const RESULTS: &str = "target/compat/results";
/// The generated page.
pub const PAGE: &str = "compatibility.md";
/// The tracks, in the order the pins file and the page name them.
pub const TRACKS: [&str; 4] = ["stable", "prerelease", "release-branch", "main"];

/// How one level of the gate went on a track.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    /// Whether every check passed.
    pub passed: bool,
    /// What failed, for the issue.
    #[serde(default)]
    pub failures: Vec<String>,
}

/// A track's result, which its job writes for the report.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackResult {
    /// `stable`, `prerelease`, `release-branch` or `main`.
    pub track: String,
    /// The VectorCraft tag or commit tested.
    #[serde(rename = "ref")]
    pub reference: String,
    /// Level A, the contract in VectorCraft's plug-in host.
    pub contract: Level,
    /// Level B, the scenarios with `vectorcraft-cli`.
    pub e2e: Level,
    /// The SHA-256 of the release's Linux archive, when a release was tested: what an adoption pins.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub archive_sha256: Option<String>,
}

impl TrackResult {
    /// Whether both levels passed.
    pub fn passed(&self) -> bool {
        self.contract.passed && self.e2e.passed
    }
}

/// Writes `result` into `dir`, where the report finds it, and returns the file.
pub fn write_result(dir: &Path, result: &TrackResult) -> Result<PathBuf, String> {
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", util::rel(dir)))?;
    let path = dir.join(format!("{}.json", result.track));
    let text = serde_json::to_string_pretty(result).map_err(|e| e.to_string())?;
    std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", util::rel(&path)))?;
    Ok(path)
}

/// A release that passed both levels.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Row {
    /// Its tag.
    pub tag: String,
    /// The track it was found on: `stable` or `prerelease`.
    pub track: String,
    /// The day, in UTC, the run that found it passing ran.
    pub checked: String,
}

/// The record.
#[derive(Debug, Default, Deserialize, Serialize)]
pub struct Supported {
    /// The releases, newest first.
    #[serde(default)]
    pub version: Vec<Row>,
}

/// What heads the record, which the report writes whole.
const RECORD_HEADER: &str = "\
# The VectorCraft releases that passed both levels of the compatibility gate, newest first
# (docs/src/design/compatibility-gate.md). Written by `cargo xtask compat report` in the pull request that
# adopts a release, never by hand. docs/src/user/reference/compatibility.md is generated from it.
";

/// The record in `root`.
pub fn load_supported(root: &Path) -> Result<Supported, String> {
    toml::from_str(&util::read(&root.join(SUPPORTED))?).map_err(|e| format!("{SUPPORTED}: {e}"))
}

/// The record's text: its header and its rows, newest first.
fn record_text(supported: &Supported) -> Result<String, String> {
    let mut rows = supported.version.clone();
    rows.sort_by(|a, b| Version::parse(&b.tag).cmp(&Version::parse(&a.tag)));
    let body = toml::to_string(&Supported { version: rows }).map_err(|e| e.to_string())?;
    Ok(format!("{RECORD_HEADER}\n{body}"))
}

/// The supported-versions page, from the record and the pins.
pub fn page() -> Result<String, String> {
    Ok(render(&load_supported(&util::root())?, &load_pins()?))
}

/// The supported-versions page of `supported`, which names the stable release `pins` pins.
fn render(supported: &Supported, pins: &toml::Table) -> String {
    let mut out = format!(
        "<!-- Generated by `cargo xtask docs` from {SUPPORTED} and {PINS}. Edit those, not this page. -->\n\n\
         # VectorCraft compatibility\n\n\
         The VectorCraft releases the StitchCraft plug-ins work with. Each passed both levels of the \
         [compatibility gate](../../design/compatibility-gate.md): the plug-in contract, in VectorCraft's own \
         plug-in host, and the end-to-end scenarios, in which VectorCraft's command line applies the effects, \
         saves, and StitchCraft sews what it saved. The gate checks VectorCraft's latest release, its latest \
         pre-release, its `release` branch and its `main` every day. A release that passes is added here by the \
         pull request that adopts it.\n\n"
    );
    if supported.version.is_empty() {
        out.push_str("No release has passed both levels yet.\n");
    } else {
        out.push_str("| VectorCraft | Track | Passed the gate |\n|---|---|---|\n");
        for row in &supported.version {
            let _ = writeln!(out, "| {} | {} | {} |", row.tag.trim_start_matches('v'), row.track, row.checked);
        }
    }
    let stable = pinned_tag(pins, "stable").trim_start_matches('v');
    let _ = write!(
        out,
        "\nThe plug-ins speak VectorCraft's plug-in ABI v1. Pull requests are checked against VectorCraft {stable}, the \
         release the gate pins in `{PINS}`.\n"
    );
    out
}

/// An issue the run opens or comments on while its track fails, and closes once it passes.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Issue {
    /// The track, or `move` for the rehearsal of moving into VectorCraft.
    pub key: String,
    /// Whether it fails now.
    pub failing: bool,
    /// The issue's title, by which the workflow finds it.
    pub title: String,
    /// Its labels.
    pub labels: Vec<String>,
    /// What to say in it.
    pub body: String,
}

/// The pull request that adopts new releases.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Adoption {
    /// Its branch.
    pub branch: String,
    /// Its title.
    pub title: String,
    /// Its description.
    pub body: String,
}

/// What a scheduled run does.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Decisions {
    /// An issue for each track, and for moving into VectorCraft.
    pub issues: Vec<Issue>,
    /// The adoption, when a release passed that the gate doesn't pin yet.
    pub adopt: Option<Adoption>,
}

/// What the run knows.
pub struct Inputs<'a> {
    /// What `cargo xtask compat discover --json` said: each track's tag or commit.
    pub discovered: &'a Value,
    /// Each track's result, by track.
    pub results: &'a BTreeMap<String, TrackResult>,
    /// Whether moving into VectorCraft failed.
    pub move_failed: bool,
    /// The pins.
    pub pins: &'a toml::Table,
    /// The record.
    pub supported: &'a Supported,
    /// The run, for people to look at.
    pub run_url: &'a str,
}

/// The label of the issue for `key`.
fn label(key: &str) -> &'static str {
    match key {
        "stable" => "upstream-stable",
        "prerelease" => "upstream-prerelease",
        _ => "upstream-canary",
    }
}

/// The tag or commit `discovered` names for `track`, or nothing.
fn discovered_ref<'a>(discovered: &'a Value, track: &str) -> Option<&'a str> {
    let key = if matches!(track, "stable" | "prerelease") { "latest" } else { "sha" };
    discovered.get(track).and_then(|t| t.get(key)).and_then(Value::as_str).filter(|r| !r.is_empty())
}

/// What the run decides.
pub fn decide(inputs: &Inputs) -> Decisions {
    let repository = inputs.pins.get("repository").and_then(toml::Value::as_str).unwrap_or_default();
    let last_good = inputs.supported.version.first().map_or_else(|| pinned_tag(inputs.pins, "stable").to_string(), |row| row.tag.clone());
    let mut issues = Vec::new();
    for track in TRACKS {
        let title = format!("VectorCraft compatibility: {track} fails");
        let labels = vec![label(track).to_string()];
        let reference = discovered_ref(inputs.discovered, track);
        let (failing, body) = match (inputs.results.get(track), reference) {
            (Some(result), _) if result.passed() => (false, format!("VectorCraft {} passes the gate again: {}", result.reference, inputs.run_url)),
            (Some(result), _) => (true, failing_body(track, result, repository, &last_good, inputs.run_url)),
            (None, Some(reference)) => {
                let result = TrackResult {
                    track: track.to_string(),
                    reference: reference.to_string(),
                    contract: Level {
                        passed: false,
                        failures: vec!["the track's job recorded no result: it stopped before its checks ran".to_string()],
                    },
                    e2e: Level::default(),
                    archive_sha256: None,
                };
                (true, failing_body(track, &result, repository, &last_good, inputs.run_url))
            }
            (None, None) => (false, format!("VectorCraft has nothing on the {track} track now: {}", inputs.run_url)),
        };
        issues.push(Issue { key: track.to_string(), failing, title, labels, body });
    }
    let moving = if inputs.move_failed {
        format!("Putting StitchCraft into VectorCraft's workspace failed, as ADR-0011 rehearses it: {}", inputs.run_url)
    } else {
        format!("Moving into VectorCraft works again: {}", inputs.run_url)
    };
    let title = "VectorCraft compatibility: moving into VectorCraft fails".to_string();
    issues.push(Issue { key: "move".to_string(), failing: inputs.move_failed, title, labels: vec![label("move").to_string()], body: moving });
    Decisions { issues, adopt: adoption(inputs) }
}

/// What the issue of a failing `track` says: what failed at each level, and VectorCraft's changes since
/// the last release that passed.
fn failing_body(track: &str, result: &TrackResult, repository: &str, last_good: &str, run_url: &str) -> String {
    let mut body = format!("StitchCraft's plug-ins fail the compatibility gate against VectorCraft {} ({track}).\n\n", result.reference);
    for (name, level) in [("Contract (Level A)", &result.contract), ("End to end (Level B)", &result.e2e)] {
        let _ = writeln!(body, "**{name}:** {}", if level.passed { "passed" } else { "failed" });
        for failure in &level.failures {
            let _ = writeln!(body, "- {failure}");
        }
        body.push('\n');
    }
    let _ = write!(body, "VectorCraft's changes since {last_good}: {repository}/compare/{last_good}...{}\n\nRun: {run_url}\n", result.reference);
    body
}

/// The new releases that passed, which the run adopts: on the stable and pre-release tracks, a tag the pins
/// don't name and the record doesn't hold yet.
fn adopted<'a>(inputs: &'a Inputs) -> Vec<&'a TrackResult> {
    ["stable", "prerelease"]
        .into_iter()
        .filter_map(|track| inputs.results.get(track))
        .filter(|result| {
            result.passed()
                && Version::parse(&result.reference).is_some()
                && result.reference != pinned_tag(inputs.pins, &result.track)
                && !inputs.supported.version.iter().any(|row| row.tag == result.reference)
        })
        .collect()
}

/// The pull request adopting [`adopted`] releases, if there are any.
fn adoption(inputs: &Inputs) -> Option<Adoption> {
    let new = adopted(inputs);
    let tags: Vec<&str> = new.iter().map(|r| r.reference.as_str()).collect();
    if tags.is_empty() {
        return None;
    }
    let mut body = String::from("VectorCraft released, and StitchCraft's plug-ins pass both levels of the compatibility gate against it:\n\n");
    for result in &new {
        let _ = writeln!(body, "- {} ({})", result.reference, result.track);
    }
    let _ = write!(
        body,
        "\nThis pull request pins it in `{PINS}`, so that pull requests are checked against it, and adds it to \
         `{SUPPORTED}`, from which the supported-versions page is generated. A stable release's Linux archive is \
         pinned by the SHA-256 its release lists, which the run checked.\n\nRun: {}\n",
        inputs.run_url
    );
    Some(Adoption { branch: format!("compat/vectorcraft-{}", tags.join("-")), title: format!("compat: VectorCraft {}", tags.join(" and ")), body })
}

/// Writes what adopting the new releases changes: their pins, the contract crate's tag for a stable one,
/// and their rows in the record, checked on `date`.
pub fn adopt(root: &Path, inputs: &Inputs, date: &str) -> Result<(), String> {
    let new = adopted(inputs);
    if new.is_empty() {
        return Ok(());
    }
    let pins_path = root.join(PINS);
    let mut pins: toml_edit::DocumentMut = util::read(&pins_path)?.parse().map_err(|e| format!("{PINS}: {e}"))?;
    let mut rows = load_supported(root)?.version;
    for result in &new {
        let track = &mut pins["tracks"][result.track.as_str()];
        track["tag"] = toml_edit::value(result.reference.as_str());
        if result.track == "stable" {
            let sum = result.archive_sha256.as_deref().ok_or(format!("the stable result for {} records no archive SHA-256", result.reference))?;
            track[SHA_KEY] = toml_edit::value(sum);
            let manifest = root.join(CONTRACT).join("Cargo.toml");
            let old = format!("tag = \"{}\"", pinned_tag(inputs.pins, "stable"));
            let text = util::read(&manifest)?;
            if !text.contains(&old) {
                return Err(format!("{CONTRACT}/Cargo.toml doesn't pin `{old}`"));
            }
            std::fs::write(&manifest, text.replace(&old, &format!("tag = \"{}\"", result.reference)))
                .map_err(|e| format!("{}: {e}", util::rel(&manifest)))?;
        }
        rows.push(Row { tag: result.reference.clone(), track: result.track.clone(), checked: date.to_string() });
    }
    std::fs::write(&pins_path, pins.to_string()).map_err(|e| format!("{PINS}: {e}"))?;
    let supported = root.join(SUPPORTED);
    std::fs::write(&supported, record_text(&Supported { version: rows })?).map_err(|e| format!("{SUPPORTED}: {e}"))
}

/// The results the tracks' jobs wrote into `dir`, by track.
pub fn load_results(dir: &Path) -> Result<BTreeMap<String, TrackResult>, String> {
    let mut results = BTreeMap::new();
    for path in util::files(dir, &["json"]) {
        let result: TrackResult = serde_json::from_str(&util::read(&path)?).map_err(|e| format!("{}: {e}", util::rel(&path)))?;
        results.insert(result.track.clone(), result);
    }
    Ok(results)
}

/// `cargo xtask compat report`: prints what the run decides, as JSON, and with `write` writes the adoption.
pub fn run(results: &Path, discovered: &str, move_failed: bool, date: &str, run_url: &str, write: bool) -> Result<(), String> {
    let root = util::root();
    let discovered: Value = serde_json::from_str(discovered).map_err(|e| format!("--discovered: {e}"))?;
    let results = load_results(results)?;
    let pins = load_pins()?;
    let supported = load_supported(&root)?;
    let inputs = Inputs { discovered: &discovered, results: &results, move_failed, pins: &pins, supported: &supported, run_url };
    if write {
        adopt(&root, &inputs, date)?;
    }
    println!("{}", serde_json::to_string(&decide(&inputs)).map_err(|e| e.to_string())?);
    Ok(())
}

#[cfg(test)]
mod tests;
