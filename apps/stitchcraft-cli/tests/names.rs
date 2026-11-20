//! The names of a traced design's shapes (roadmap step M13.25, `REQ-AUTO-125` to `REQ-AUTO-127`,
//! `REQ-CLI-023`; `docs/src/design/image-to-embroidery.md` › Names of shapes): each automatic shape without a
//! name is named by its thread, how it sews and its number among the shapes of both in the Layers panel's
//! order, a shape's own name is kept, and the notices and the report name each shape so.
//!
//! The design is the badge VectorCraft 0.18.0 traced (`traced-badge.vectorcraft`), as `stitch plan
//! picture.png` reads it: with `--auto`. Its shapes are a white field (node 3), and red (6), blue (9) and gold
//! (12) areas over it, each in one of the reference machine's threads exactly.

// Test code may unwrap. It runs `stitch` on files in folders of its own.
#![allow(clippy::unwrap_used, clippy::disallowed_methods)]

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{Value, json};
use stitchcraft_testkit::temp::TempDir;

/// The traced badge.
const TRACED: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/fixtures/vectorcraft/traced-badge.vectorcraft");

/// `stitch plan badge.vectorcraft --auto` in a folder of its own named after `test`, of the badge with the
/// nodes of `names` given those names in the Layers panel: what it said, and its report.
fn planned(test: &str, names: &[(u64, &str)]) -> (Output, Value) {
    let dir = TempDir::new(&format!("names-{test}"));
    let mut document: Value = serde_json::from_slice(&std::fs::read(TRACED).unwrap()).unwrap();
    for (id, name) in names {
        let group = document.pointer_mut("/document/layers/0/kind/children/0/kind/children").unwrap().as_array_mut().unwrap();
        let node = group.iter_mut().find(|node| node["id"] == json!(id)).unwrap();
        node["name"] = json!(name);
    }
    std::fs::write(dir.join("badge.vectorcraft"), serde_json::to_vec(&document).unwrap()).unwrap();
    let out = stitch(dir.path(), &["plan", "badge.vectorcraft", "--auto", "-o", "badge.pes", "--report", "badge.json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out.stderr));
    let report = serde_json::from_slice(&std::fs::read(dir.join("badge.json")).unwrap()).unwrap();
    (out, report)
}

fn stitch(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_stitch")).args(args).current_dir(dir).output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn req_auto_125_an_automatic_shape_is_named_by_its_thread_how_it_sews_and_its_number() {
    let (out, report) = planned("named", &[]);
    let said = text(&out.stderr);
    for line in [
        r#"info SC-I0912 ("Red fill (1)", vc:6:fill): This shape sews by the rule area."#,
        r#"info SC-I0912 ("Blue fill (1)", vc:9:fill): This shape sews by the rule area."#,
        r#"info SC-I0912 ("Deep Gold fill (1)", vc:12:fill): This shape sews by the rule area."#,
        r#"warning SC-W0906 ("White left out (1)", vc:3:fill): "#,
    ] {
        assert!(said.contains(line), "{line} in {said}");
    }
    // Planned again, the names are the same.
    let (again, _) = planned("named-again", &[]);
    assert_eq!(text(&again.stderr), said);
    // The report lists the shapes in their sewing order, the one left out last.
    let shape = |name: &str, element: &str, thread: &str, kind: &str, place: Value| json!({"name": name, "element": element, "thread": thread, "kind": kind, "place": place});
    assert_eq!(
        report["shapes"],
        json!([
            shape("Red fill (1)", "vc:6", "#ed171f", "fill", json!(1)),
            shape("Blue fill (1)", "vc:9", "#0a55a3", "fill", json!(2)),
            shape("Deep Gold fill (1)", "vc:12", "#e8a900", "fill", json!(3)),
            shape("White left out (1)", "vc:3", "#f0f0f0", "left out", Value::Null),
        ])
    );
}

#[test]
fn req_auto_126_a_shape_s_own_name_is_kept_and_a_name_in_use_is_not_given_again() {
    // The person named the blue area, and gave the gold one the name the red one would get: the red one
    // takes the next number.
    let (out, _) = planned("kept", &[(9, "Lakeside blue"), (12, "Red fill (1)")]);
    let said = text(&out.stderr);
    for line in [
        r#"info SC-I0912 ("Red fill (2)", vc:6:fill): "#,
        r#"info SC-I0912 ("Lakeside blue", vc:9:fill): "#,
        r#"info SC-I0912 ("Red fill (1)", vc:12:fill): "#,
    ] {
        assert!(said.contains(line), "{line} in {said}");
    }
}

#[test]
fn req_cli_023_the_report_names_the_shape_of_each_diagnostic() {
    let (_, report) = planned("report", &[]);
    let diagnostics = report["diagnostics"].as_array().unwrap();
    let rule = diagnostics.iter().find(|d| d["code"] == "SC-I0912" && d["element"] == "vc:6:fill").unwrap();
    assert_eq!(rule["name"], "Red fill (1)", "{rule}");
    // A diagnostic of the whole design names no shape.
    let summary = diagnostics.iter().find(|d| d["code"] == "SC-I0901").unwrap();
    assert_eq!(summary.get("name"), None, "{summary}");
}
