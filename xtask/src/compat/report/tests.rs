//! The report's decisions, the adoption it writes, the record and the page, on results made up for each.

use serde_json::json;

use super::*;

const SUM: &str = "6750db109d3a253c51c82975af650f42384ba27a768f72529431edfb58508269";

/// A result on `track` against `reference`, passing both levels, or failing Level B with `e2e`.
fn result(track: &str, reference: &str, e2e: Option<&str>) -> TrackResult {
    TrackResult {
        track: track.to_string(),
        reference: reference.to_string(),
        contract: Level { passed: true, failures: Vec::new() },
        e2e: Level { passed: e2e.is_none(), failures: e2e.map(str::to_string).into_iter().collect() },
        archive_sha256: (track == "stable").then(|| SUM.to_string()),
    }
}

fn results(list: Vec<TrackResult>) -> BTreeMap<String, TrackResult> {
    list.into_iter().map(|r| (r.track.clone(), r)).collect()
}

/// VectorCraft as it was on 2026-10-17: 0.12.0 its latest release, no pre-release newer, and its branches.
fn discovered() -> Value {
    json!({
        "stable": { "latest": "v0.12.0", "pinned": "v0.7.0", "new": true },
        "prerelease": { "latest": null, "pinned": "", "new": false },
        "release-branch": { "sha": "8bc65329286dc1b84aa392997bb205613c3e96cb" },
        "main": { "sha": "f7de3d6a01ffa6f5783bb2d989e14daebcc40aec" },
    })
}

fn pins() -> toml::Table {
    toml::from_str(
        "repository = \"https://github.com/storytold/vectorcraft\"\n[tracks.stable]\ntag = \"v0.7.0\"\n\
         linux-x86_64-sha256 = \"d6b0\"\n[tracks.prerelease]\ntag = \"\"\n",
    )
    .unwrap()
}

#[test]
fn each_track_has_its_issue_and_a_passing_new_release_is_adopted() {
    let found = results(vec![
        result("stable", "v0.12.0", None),
        result("release-branch", "8bc65329286dc1b84aa392997bb205613c3e96cb", Some("apply: it sews otherwise")),
    ]);
    let (pins, supported, discovered) = (pins(), Supported::default(), discovered());
    let inputs = Inputs { discovered: &discovered, results: &found, move_failed: true, pins: &pins, supported: &supported, run_url: "RUN" };
    let decisions = decide(&inputs);
    let issues: Vec<(&str, bool, &str, &str)> =
        decisions.issues.iter().map(|i| (i.key.as_str(), i.failing, i.title.as_str(), i.labels[0].as_str())).collect();
    assert_eq!(
        issues,
        [
            ("stable", false, "VectorCraft compatibility: stable fails", "upstream-stable"),
            ("prerelease", false, "VectorCraft compatibility: prerelease fails", "upstream-prerelease"),
            ("release-branch", true, "VectorCraft compatibility: release-branch fails", "upstream-canary"),
            ("main", true, "VectorCraft compatibility: main fails", "upstream-canary"),
            ("move", true, "VectorCraft compatibility: moving into VectorCraft fails", "upstream-canary"),
        ]
    );
    let body = |key: &str| decisions.issues.iter().find(|i| i.key == key).map(|i| i.body.as_str()).unwrap();
    assert_eq!(body("stable"), "VectorCraft v0.12.0 passes the gate again: RUN");
    assert_eq!(body("prerelease"), "VectorCraft has nothing on the prerelease track now: RUN");
    let failing = body("release-branch");
    assert!(
        failing.contains("**Contract (Level A):** passed\n") && failing.contains("**End to end (Level B):** failed\n- apply: it sews otherwise\n"),
        "{failing}"
    );
    assert!(
        failing.contains(
            "VectorCraft's changes since v0.7.0: https://github.com/storytold/vectorcraft/compare/v0.7.0...8bc65329286dc1b84aa392997bb205613c3e96cb"
        ),
        "{failing}"
    );
    // A track whose job wrote no result fails: it stopped before its checks.
    assert!(body("main").contains("- the track's job recorded no result"), "{}", body("main"));
    let adoption = decisions.adopt.unwrap();
    assert_eq!((adoption.branch.as_str(), adoption.title.as_str()), ("compat/vectorcraft-v0.12.0", "compat: VectorCraft v0.12.0"));
    assert!(adoption.body.contains("- v0.12.0 (stable)\n") && adoption.body.ends_with("Run: RUN\n"), "{}", adoption.body);
}

#[test]
fn only_a_new_release_that_passes_is_adopted() {
    let (pins, discovered) = (pins(), discovered());
    let adopt = |list: Vec<TrackResult>, supported: &Supported| {
        let found = results(list);
        decide(&Inputs { discovered: &discovered, results: &found, move_failed: false, pins: &pins, supported, run_url: "RUN" })
            .adopt
            .map(|a| a.title)
    };
    let none = Supported::default();
    assert_eq!(adopt(vec![result("stable", "v0.12.0", Some("x"))], &none), None);
    assert_eq!(adopt(vec![result("stable", "v0.7.0", None)], &none), None);
    let recorded = Supported { version: vec![Row { tag: "v0.12.0".into(), track: "stable".into(), checked: "2026-10-18".into() }] };
    assert_eq!(adopt(vec![result("stable", "v0.12.0", None)], &recorded), None);
    // A branch's commit is never adopted, and a release and a pre-release are adopted together.
    assert_eq!(adopt(vec![result("main", "f7de3d6a01ffa6f5783bb2d989e14daebcc40aec", None)], &none), None);
    let both = vec![result("stable", "v0.12.0", None), result("prerelease", "v0.13.0-rc.1", None)];
    assert_eq!(adopt(both, &none).as_deref(), Some("compat: VectorCraft v0.12.0 and v0.13.0-rc.1"));
}

/// A copy of the files an adoption writes, in a folder of its own.
fn copied(name: &str) -> PathBuf {
    let root = util::root();
    let dir = root.join("target/xtask-tests/report").join(name);
    for file in [PINS, SUPPORTED, &format!("{CONTRACT}/Cargo.toml")] {
        let to = dir.join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::copy(root.join(file), &to).unwrap();
    }
    dir
}

#[test]
fn adopting_writes_the_pins_the_contract_s_tag_and_the_record() {
    let dir = copied("adopt");
    let pins: toml::Table = toml::from_str(&util::read(&dir.join(PINS)).unwrap()).unwrap();
    let old = pinned_tag(&pins, "stable").to_string();
    let found = results(vec![result("stable", "v0.12.0", None), result("prerelease", "v0.13.0-rc.1", None)]);
    let (supported, discovered) = (load_supported(&dir).unwrap(), discovered());
    let inputs = Inputs { discovered: &discovered, results: &found, move_failed: false, pins: &pins, supported: &supported, run_url: "RUN" };
    adopt(&dir, &inputs, "2026-10-18").unwrap();
    let written = util::read(&dir.join(PINS)).unwrap();
    let after: toml::Table = toml::from_str(&written).unwrap();
    assert_eq!((pinned_tag(&after, "stable"), pinned_tag(&after, "prerelease")), ("v0.12.0", "v0.13.0-rc.1"));
    assert_eq!(after["tracks"]["stable"][SHA_KEY].as_str(), Some(SUM));
    // The comments stay.
    assert!(written.contains("# The SHA-256 of the release's Linux x86_64 archive"), "{written}");
    let manifest = util::read(&dir.join(CONTRACT).join("Cargo.toml")).unwrap();
    assert_eq!(manifest.matches("tag = \"v0.12.0\"").count(), 2, "{manifest}");
    assert!(!manifest.contains(&format!("tag = \"{old}\"")), "{manifest}");
    let record = load_supported(&dir).unwrap();
    let rows: Vec<(&str, &str, &str)> = record.version.iter().map(|r| (r.tag.as_str(), r.track.as_str(), r.checked.as_str())).collect();
    assert_eq!(rows, [("v0.13.0-rc.1", "prerelease", "2026-10-18"), ("v0.12.0", "stable", "2026-10-18")]);
    assert!(util::read(&dir.join(SUPPORTED)).unwrap().starts_with(RECORD_HEADER));
    // A stable release whose result has no archive SHA-256 can't be pinned.
    let dir = copied("no-sum");
    let found = results(vec![TrackResult { archive_sha256: None, ..result("stable", "v0.12.0", None) }]);
    let inputs = Inputs { results: &found, ..inputs };
    assert!(adopt(&dir, &inputs, "2026-10-18").unwrap_err().contains("records no archive SHA-256"));
}

#[test]
fn the_page_lists_the_record_and_names_the_pinned_release() {
    let empty = render(&Supported::default(), &pins());
    assert!(empty.starts_with("<!-- Generated by `cargo xtask docs` from compat/supported.toml and compat/vectorcraft.toml."), "{empty}");
    assert!(
        empty.contains("No release has passed both levels yet.\n") && empty.contains("checked against VectorCraft 0.7.0, the release"),
        "{empty}"
    );
    let rows = vec![
        Row { tag: "v0.13.0-rc.1".into(), track: "prerelease".into(), checked: "2026-11-02".into() },
        Row { tag: "v0.12.0".into(), track: "stable".into(), checked: "2026-10-18".into() },
    ];
    let page = render(&Supported { version: rows }, &pins());
    assert!(
        page.contains(
            "| VectorCraft | Track | Passed the gate |\n|---|---|---|\n| 0.13.0-rc.1 | prerelease | 2026-11-02 |\n| 0.12.0 | stable | 2026-10-18 |\n"
        ),
        "{page}"
    );
}

#[test]
fn a_result_is_read_back_as_written() {
    let dir = util::root().join("target/xtask-tests/report/results");
    let _ = std::fs::remove_dir_all(&dir);
    let written = [result("stable", "v0.12.0", None), result("main", "f7de3d6", Some("compound-satin: run 1: vectorcraft-cli exit status: 1"))];
    for r in &written {
        write_result(&dir, r).unwrap();
    }
    assert_eq!(load_results(&dir).unwrap(), results(written.to_vec()));
    assert!(written[0].passed() && !written[1].passed());
}
