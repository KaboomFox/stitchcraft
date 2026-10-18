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
    let inputs = Inputs { discovered: Some(&discovered), results: &found, moved: Some(false), pins: &pins, supported: &supported, run_url: "RUN" };
    let decisions = decide(&inputs).unwrap();
    let issues: Vec<(&str, bool, &str, &str)> =
        decisions.issues.iter().map(|i| (i.key.as_str(), i.failing, i.title.as_str(), i.label.as_str())).collect();
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
    // Without leave to open pull requests, the run files an issue for it, as a release blocker.
    assert_eq!((adoption.waiting.title.as_str(), adoption.waiting.label.as_str()), (WAITING, "upstream-stable"));
    assert!(adoption.waiting.failing && adoption.waiting.body.contains("the branch `compat/vectorcraft-v0.12.0` waits"));
    assert!(adoption.body.contains("- v0.12.0 (stable)\n") && adoption.body.ends_with("Run: RUN\n"), "{}", adoption.body);
}

#[test]
fn only_a_new_release_that_passes_is_adopted() {
    let (pins, discovered) = (pins(), discovered());
    let adopt = |list: Vec<TrackResult>, supported: &Supported| {
        let found = results(list);
        decide(&Inputs { discovered: Some(&discovered), results: &found, moved: Some(true), pins: &pins, supported, run_url: "RUN" })
            .unwrap()
            .adopt
            .map(|a| (a.title, a.waiting.label))
    };
    let none = Supported::default();
    assert_eq!(adopt(vec![result("stable", "v0.12.0", Some("x"))], &none), None);
    let pre = adopt(vec![result("prerelease", "v0.13.0-rc.1", None)], &none);
    assert_eq!(pre, Some(("compat: VectorCraft v0.13.0-rc.1".to_string(), "upstream-prerelease".to_string())));
    assert_eq!(adopt(vec![result("stable", "v0.7.0", None)], &none), None);
    let recorded = Supported { version: vec![Row { tag: "v0.12.0".into(), track: "stable".into(), checked: "2026-10-18".into() }] };
    assert_eq!(adopt(vec![result("stable", "v0.12.0", None)], &recorded), None);
    // A branch's commit is never adopted, and a release and a pre-release are adopted together.
    assert_eq!(adopt(vec![result("main", "f7de3d6a01ffa6f5783bb2d989e14daebcc40aec", None)], &none), None);
    let both = vec![result("stable", "v0.12.0", None), result("prerelease", "v0.13.0-rc.1", None)];
    let both = adopt(both, &none);
    assert_eq!(both, Some(("compat: VectorCraft v0.12.0 and v0.13.0-rc.1".to_string(), "upstream-stable".to_string())));
}

/// A copy of the files an adoption writes, in a folder of its own: the pins and the contract crate's
/// manifest as the repository has them, but with VectorCraft 0.7.0 pinned for stable whatever the
/// repository pins, and a record of no release. Adopting 0.12.0 is then new there, also in the pull
/// request that adopts it here.
fn copied(name: &str) -> PathBuf {
    let root = util::root();
    let dir = root.join("target/xtask-tests/report").join(name);
    let pinned = format!("tag = \"{}\"", pinned_tag(&load_pins().unwrap(), "stable"));
    for file in [PINS, &format!("{CONTRACT}/Cargo.toml")] {
        let to = dir.join(file);
        std::fs::create_dir_all(to.parent().unwrap()).unwrap();
        std::fs::write(&to, util::read(&root.join(file)).unwrap().replace(&pinned, "tag = \"v0.7.0\"")).unwrap();
    }
    std::fs::write(dir.join(SUPPORTED), RECORD_HEADER).unwrap();
    dir
}

#[test]
fn adopting_writes_the_pins_the_contract_s_tag_and_the_record() {
    let dir = copied("adopt");
    let pins: toml::Table = toml::from_str(&util::read(&dir.join(PINS)).unwrap()).unwrap();
    let old = pinned_tag(&pins, "stable").to_string();
    let found = results(vec![result("stable", "v0.12.0", None), result("prerelease", "v0.13.0-rc.1", None)]);
    let (supported, discovered) = (load_supported(&dir).unwrap(), discovered());
    let inputs = Inputs { discovered: Some(&discovered), results: &found, moved: Some(true), pins: &pins, supported: &supported, run_url: "RUN" };
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

#[test]
fn a_track_that_always_names_a_version_fails_when_discovery_names_none() {
    // VectorCraft has releases and a main branch: naming neither means discovery failed, or a branch was
    // renamed, and the track's jobs failed without a result. A pre-release may be missing.
    let mut discovered = discovered();
    discovered["stable"]["latest"] = Value::Null;
    discovered["main"]["sha"] = Value::Null;
    let (pins, supported, found) = (pins(), Supported::default(), results(Vec::new()));
    let inputs = Inputs { discovered: Some(&discovered), results: &found, moved: Some(true), pins: &pins, supported: &supported, run_url: "RUN" };
    let decisions = decide(&inputs).unwrap();
    let failing: Vec<(&str, bool)> = decisions.issues.iter().map(|i| (i.key.as_str(), i.failing)).collect();
    // With nothing to adopt, an issue left waiting for an adoption's pull request is closed.
    assert_eq!(failing, [("stable", true), ("prerelease", false), ("release-branch", true), ("main", true), ("move", false), ("adopt", false)]);
    let main = decisions.issues.iter().find(|i| i.key == "main").unwrap();
    assert!(main.body.starts_with("Discovery named no VectorCraft version on the main track"), "{}", main.body);
}

#[test]
fn a_run_without_discovery_leaves_alone_what_it_can_t_know() {
    // The discover job failed: the tracks that always name a version fail, the others' issues stay as they
    // are, and so does the move's when the rehearsal didn't run.
    let (pins, supported, found) = (pins(), Supported::default(), results(Vec::new()));
    let inputs = Inputs { discovered: None, results: &found, moved: None, pins: &pins, supported: &supported, run_url: "RUN" };
    let decisions = decide(&inputs).unwrap();
    let failing: Vec<(&str, bool)> = decisions.issues.iter().map(|i| (i.key.as_str(), i.failing)).collect();
    assert_eq!(failing, [("stable", true), ("main", true), ("adopt", false)]);
    assert_eq!(decisions.adopt, None);
}

#[test]
fn a_pins_file_without_a_track_s_table_is_refused() {
    let dir = copied("no-table");
    let text = "repository = \"https://github.com/storytold/vectorcraft\"\n[tracks]\nprerelease = \"\"\n[tracks.stable]\ntag = \"v0.7.0\"\n";
    std::fs::write(dir.join(PINS), text).unwrap();
    let pins: toml::Table = toml::from_str(text).unwrap();
    let found = results(vec![result("prerelease", "v0.13.0-rc.1", None)]);
    let (supported, discovered) = (load_supported(&dir).unwrap(), discovered());
    let inputs = Inputs { discovered: Some(&discovered), results: &found, moved: Some(true), pins: &pins, supported: &supported, run_url: "RUN" };
    let refused = adopt(&dir, &inputs, "2026-10-18").unwrap_err();
    assert!(refused.contains("[tracks.prerelease]"), "{refused}");
}

#[test]
fn the_workflow_files_each_issue_with_xtask_and_reports_no_cancelled_run() {
    let workflow = util::read(&util::root().join(".github/workflows/compat.yml")).unwrap();
    for call in ["gh issue create", "gh issue comment", "gh issue close"] {
        assert!(!workflow.contains(call), "`{call}` in the workflow: `cargo xtask compat issue` finds an open issue first");
    }
    // `always()` holds for a cancelled run too, whose tracks wrote no results and would be filed as failing.
    // Lines, not `\n`: a checkout on Windows ends them with `\r\n`.
    let lines: Vec<&str> = workflow.lines().collect();
    let report = lines.iter().position(|line| *line == "  report:").unwrap();
    let condition = lines[report..].iter().find(|line| line.trim_start().starts_with("if:")).unwrap();
    assert!(condition.contains("!cancelled()") && !condition.contains("always()"), "{condition}");
}

/// A stand-in for `gh`: it adds each call's arguments, one call a line, to the file `calls` beside it, and
/// answers `issue list` with the file `issues` beside it and `label list` with the file `labels`.
#[cfg(unix)]
const GH: &str = r#"#!/bin/sh
here=$(dirname "$0")
echo "$*" >> "$here/calls"
case "$1 $2" in
  "issue list") cat "$here/issues" ;;
  "label list") cat "$here/labels" ;;
esac
"#;

#[cfg(unix)]
#[test]
fn an_issue_is_opened_once_commented_on_while_it_fails_and_closed_when_it_passes() {
    use std::os::unix::fs::PermissionsExt;
    let main = |failing| issue("main", failing, "body".to_string());
    let filed = |case: &str, issue: Issue, issues: &str, labels: &str| -> Vec<String> {
        let dir = util::root().join("target/xtask-tests/issue").join(case);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let gh = dir.join("gh");
        std::fs::write(&gh, GH).unwrap();
        std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
        std::fs::write(dir.join("issues"), issues).unwrap();
        std::fs::write(dir.join("labels"), labels).unwrap();
        file_issue(&gh, "o/r", &issue).unwrap();
        util::read(&dir.join("calls")).unwrap().lines().map(str::to_string).collect()
    };
    let list = "issue list --state open --search \"VectorCraft compatibility: main fails\" in:title --limit 100 --json number,title --repo o/r";
    let labels = "label list --search upstream-canary --limit 100 --json name --repo o/r";
    // The search finds words, so another title that holds them is no match.
    let open =
        r#"[{"number": 8, "title": "VectorCraft compatibility: main fails often"}, {"number": 7, "title": "VectorCraft compatibility: main fails"}]"#;
    let other = r#"[{"number": 8, "title": "VectorCraft compatibility: main fails often"}]"#;
    let create = "issue create --title VectorCraft compatibility: main fails --label upstream-canary --body body --repo o/r";
    let about = label("main").1;
    assert_eq!(
        filed("first", main(true), other, r#"[{"name": "upstream-stable"}]"#),
        [list, labels, &format!("label create upstream-canary --color B60205 --description {about} --repo o/r"), create]
    );
    assert_eq!(filed("labelled", main(true), "[]", r#"[{"name": "upstream-canary"}]"#), [list, labels, create]);
    // When no label matches its search, `gh label list` writes an empty output, JSON asked for or not, and
    // exits with 0: the label is created, as the adopting run's waiting issue needed for `upstream-stable`.
    assert_eq!(
        filed("unlabelled", main(true), other, ""),
        [list, labels, &format!("label create upstream-canary --color B60205 --description {about} --repo o/r"), create]
    );
    assert_eq!(filed("again", main(true), open, "[]"), [list, "issue comment 7 --body body --repo o/r"]);
    assert_eq!(filed("passes", main(false), open, "[]"), [list, "issue close 7 --comment body --repo o/r"]);
    assert_eq!(filed("passed", main(false), other, "[]"), [list]);
}

#[test]
fn the_job_results_and_a_failed_discovery_are_read() {
    let read: Vec<Option<bool>> = JOB_RESULTS.iter().map(|result| read_move(result).unwrap()).collect();
    assert_eq!(read, [Some(true), Some(false), Some(false), None]);
    assert!(read_move("timed_out").unwrap_err().contains("success, failure, cancelled, skipped"));
    assert_eq!(read_discovery(" ").unwrap(), None);
    assert_eq!(read_discovery("{\"main\": {}}").unwrap(), Some(json!({ "main": {} })));
    assert!(read_discovery("{").is_err());
}

#[test]
fn a_label_s_description_fits_github() {
    // GitHub refuses a label description longer than 100 characters.
    for key in TRACKS.iter().chain(&["move"]) {
        let (_, about) = label(key);
        assert!(about.chars().count() <= 100, "{about}");
    }
}
