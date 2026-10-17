//! Level B's scenarios, how a run becomes a command line, and StitchCraft's checks, with the patch fixture
//! in place of what VectorCraft saves and, on Unix, a script in place of `vectorcraft-cli`.

use super::*;
use crate::compat::modules;
use crate::conformance::requirement_ids;

/// The patch fixture: a fill on object 2 at 45° with rows 0.4 mm apart, a satin column, a stroke, and 2
/// squares a group's gold fill effect sews.
fn patch() -> Vec<u8> {
    util::read_bytes(&util::root().join("conformance/fixtures/vectorcraft/patch.vectorcraft")).unwrap()
}

fn scenario(text: &str) -> Scenario {
    toml::from_str(text).unwrap()
}

/// The fill record of the patch's object 2, with `params`.
fn fill_on_2(params: &Value) -> Applied {
    Applied { ids: vec![2], record: json!({ "id": "plugin.dev.stitchcraft.fill", "params": params }) }
}

#[test]
fn every_scenario_checks_only_what_its_runs_export() {
    let scenarios = load(&util::root()).unwrap();
    let names: Vec<&str> = scenarios.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, ["apply", "compound-satin", "graphic-style", "install-and-list", "missing-plugin"]);
    let requirements = requirement_ids().unwrap();
    for (name, scenario) in &scenarios {
        assert_eq!(problems(scenario, &modules(), &requirements), Vec::<String>::new(), "{name}");
        assert!(!scenario.description.is_empty() && !scenario.requirements.is_empty(), "{name}");
    }
}

#[test]
fn a_scenario_that_doesn_t_hold_together_is_refused() {
    let bad = scenario(
        r#"description = "bad"
        requirements = ["REQ-VC-999"]
        [[run]]
        open = "earlier.vectorcraft"
        plugins = ["stroke", "zigzag"]
        steps = [
          { cmd = "shape.line", export = "both.vectorcraft" },
          { apply = "fill" },
          { apply = "lace" },
          { export = "a.png", params = { scale = 2 } },
          { export = "a.vectorcraft" },
        ]
        [expect]
        listed = ["satin", "zigzag"]
        applied = ["a.vectorcraft", "b.vectorcraft"]
        same = [["a.vectorcraft", "c.vectorcraft"]]
        elements = [{ file = "d.vectorcraft", element = "fill", count = 1 }]
        "#,
    );
    let found = problems(&bad, &modules(), &requirement_ids().unwrap());
    let one = "a step has one of `cmd`, `apply` and `export`, and an export no `params`";
    assert_eq!(
        found,
        [
            "run 1: it opens `earlier.vectorcraft`, which no earlier run exports".to_string(),
            "run 1: there is no module `zigzag`".to_string(),
            format!("run 1: {one}"),
            "run 1: it applies `fill` without installing that module".to_string(),
            "run 1: `apply = \"lace\"` names no stitch family".to_string(),
            format!("run 1: {one}"),
            "run 1: it applies `fill`, and no drawing shows it after".to_string(),
            "it checks `b.vectorcraft`, which no run exports".to_string(),
            "it checks `c.vectorcraft`, which no run exports".to_string(),
            "it checks `d.vectorcraft`, which no run exports".to_string(),
            "it expects a module `zigzag` listed, and there is none".to_string(),
            "there is no requirement REQ-VC-999".to_string(),
        ]
    );
}

#[test]
fn a_run_is_one_command_line_that_asks_each_module_for_its_last_error_after_each_drawing() {
    let s = scenario(
        r##"description = "d"
        [[run]]
        open = "in.vectorcraft"
        plugins = ["fill"]
        steps = [
          { cmd = "paint.setFill", params = { color = "#c00000" } },
          { apply = "fill", params = { angle = 45, row_spacing_mm = 0.4, underlay = true } },
          { export = "out.png" },
          { export = "out.vectorcraft" },
        ]
        "##,
    );
    let args = arguments(&s.run[0], Path::new("/plugins"), Path::new("/out")).unwrap();
    // A file's path is the platform's, with `\` before the file's name on Windows.
    let at = |dir: &str, file: &str| Path::new(dir).join(file).to_string_lossy().into_owned();
    let (opened, exported, saved) = (at("/out", "in.vectorcraft"), at("/out", "out.png"), at("/out", "out.vectorcraft"));
    let install = json!({ "path": at("/plugins", "fill.wasm") }).to_string();
    let expected = [
        "run",
        "--in",
        opened.as_str(),
        "--cmd",
        "plugin.install",
        "--params",
        install.as_str(),
        "--cmd",
        "paint.setFill",
        "--params",
        r##"{"color":"#c00000"}"##,
        "--cmd",
        "effect.apply",
        "--params",
        r#"{"effect":"plugin.dev.stitchcraft.fill","item":null,"params":{"angle":45,"row_spacing_mm":0.4,"underlay":true}}"#,
        "--export",
        exported.as_str(),
        "--cmd",
        "plugin.info",
        "--params",
        r#"{"id":"dev.stitchcraft.fill"}"#,
        // Saving draws nothing, so nothing new is asked.
        "--export",
        saved.as_str(),
    ];
    assert_eq!(args, expected);
}

#[test]
fn each_effect_is_drawn_before_its_module_is_asked_for_its_last_error() {
    let s = scenario(
        r#"description = "d"
        [[run]]
        plugins = ["fill", "stroke"]
        steps = [
          { apply = "fill" },
          { apply = "fill" },
          { export = "a.png" },
          { apply = "stroke" },
          { export = "b.vectorcraft" },
        ]
        [[run]]
        steps = [{ cmd = "shape.line" }, { export = "c.vectorcraft" }]
        [expect]
        applied = ["b.vectorcraft", "c.vectorcraft"]
        "#,
    );
    let found = problems(&s, &modules(), &std::collections::BTreeSet::new());
    assert_eq!(
        found,
        [
            "run 1: it applies `fill` again before a drawing shows the first",
            "run 1: it applies `stroke`, and no drawing shows it after",
            "it checks `c.vectorcraft` as applied, and no `apply` step comes before it is exported",
        ]
    );
}

#[test]
fn a_file_sews_as_its_effects_were_sent_only_when_vectorcraft_stored_them_so() {
    let bytes = patch();
    // The patch's fill on object 2 is what the step sent: the same stitches.
    assert_eq!(sews_as_applied(&bytes, &[fill_on_2(&json!({ "angle": 45, "row_spacing_mm": 0.4 }))]), Ok(()));
    // Had the step sent 90°, VectorCraft would have stored something else, and the message says what.
    let failure = sews_as_applied(&bytes, &[fill_on_2(&json!({ "angle": 90, "row_spacing_mm": 0.4 }))]).unwrap_err();
    assert!(failure.contains(r#"VectorCraft stored [{"id":"plugin.dev.stitchcraft.fill","params":{"angle":45,"#), "{failure}");
    assert!(failure.contains(r#"the steps sent [{"id":"plugin.dev.stitchcraft.fill","params":{"angle":90,"#), "{failure}");
    // A record VectorCraft put on a fill of the object, and not on the object, is taken away.
    let mut doc: Value = serde_json::from_slice(&bytes).unwrap();
    let node = |doc: &mut Value| doc.pointer_mut("/document/layers/0/kind/children/1/kind/children/0").unwrap().clone();
    let moved = doc.pointer_mut("/document/layers/0/kind/children/1/kind/children/0/appearance").unwrap();
    let record = moved["effects"].as_array_mut().unwrap().remove(0);
    moved["items"][0]["effects"] = json!([record]);
    let stored = restore(&mut doc, &[fill_on_2(&json!({ "angle": 45 }))]).unwrap();
    assert_eq!(stored.len(), 1);
    let restored = node(&mut doc);
    assert_eq!(restored["appearance"]["items"][0]["effects"], json!([]));
    assert_eq!(restored["appearance"]["effects"], json!([{ "id": "plugin.dev.stitchcraft.fill", "params": { "angle": 45 } }]));
}

#[test]
fn an_object_vectorcraft_didn_t_save_fails_the_check() {
    // `effect.apply` said it applied the fill to object 999, which the file doesn't hold: nothing would be
    // put back, and the file sewing as it does would prove nothing.
    let failure = sews_as_applied(&patch(), &[Applied { ids: vec![999], ..fill_on_2(&json!({ "angle": 90 })) }]).unwrap_err();
    assert!(failure.contains("999"), "{failure}");
}

#[test]
fn elements_are_counted_by_kind_thread_and_settings() {
    let bytes = patch();
    let elements = |element: &str, count, thread: Option<&str>, params: &str| Elements {
        file: "patch".into(),
        element: element.into(),
        count,
        thread: thread.map(str::to_string),
        params: (!params.is_empty()).then(|| toml::from_str(params).unwrap()),
    };
    assert_eq!(holds(&bytes, &elements("fill", 3, None, "")), Ok(()));
    assert_eq!(holds(&bytes, &elements("fill", 2, Some("#E6B31A"), "")), Ok(()));
    assert_eq!(holds(&bytes, &elements("satin", 1, None, "")), Ok(()));
    let failure = holds(&bytes, &elements("stroke", 2, Some("#008033"), "")).unwrap_err();
    assert!(failure.starts_with("2 `stroke` elements in #008033 are expected, and it holds 1: vc:2:fill #bf0000, vc:5:satin #0033b3"), "{failure}");
    // The group's fill effect sets 90°, which tells its 2 squares from a fill a paint alone would sew.
    assert_eq!(holds(&bytes, &elements("fill", 2, Some("#E6B31A"), "angle = 90")), Ok(()));
    assert_eq!(holds(&bytes, &elements("fill", 1, None, "angle = 45\nrow_spacing_mm = 0.4")), Ok(()));
    let failure = holds(&bytes, &elements("fill", 3, None, "angle = 90")).unwrap_err();
    assert!(failure.starts_with("3 `fill` elements with angle = 90 are expected, and it holds 2: vc:2:fill #bf0000 (angle = 45), "), "{failure}");
    // A setting no parameter could hold matches nothing.
    assert!(holds(&bytes, &elements("fill", 2, None, "angle = { a = 1 }")).is_err());
}

#[test]
fn toml_parameters_become_json() {
    let table: toml::Table = toml::from_str("a = 1\nb = 0.5\nc = \"x\"\nd = [true, { e = 2 }]\nf = 1979-05-27").unwrap();
    assert_eq!(toml_to_json(toml::Value::Table(table)), json!({ "a": 1, "b": 0.5, "c": "x", "d": [true, { "e": 2 }], "f": "1979-05-27" }));
}

/// A stand-in for `vectorcraft-cli run`: it prints a result for each step as VectorCraft does, applies every
/// effect to object 2, has each module's last error be what the file `error` beside it says, and exports
/// the file `doc` beside it names for a `.vectorcraft` file and an empty file for any other.
#[cfg(unix)]
const STUB: &str = r#"#!/bin/sh
here=$(dirname "$0")
shift
while [ $# -gt 0 ]; do
  case "$1" in
    --in) echo '{"step":"open","result":{}}'; shift 2 ;;
    --export)
      case "$2" in *.vectorcraft) cp "$(cat "$here/doc")" "$2" ;; *) : > "$2" ;; esac
      echo '{"step":"export","result":{}}'; shift 2 ;;
    --cmd)
      case "$2" in
        plugin.install) echo '{"step":"cmd","result":{"kind":"effect"}}' ;;
        plugin.list) echo '{"step":"cmd","result":{"plugins":[{"id":"dev.stitchcraft.fill","kind":"effect"}]}}' ;;
        plugin.info) echo "{\"step\":\"cmd\",\"result\":{\"lastError\":$(cat "$here/error")}}" ;;
        effect.apply) echo '{"step":"cmd","result":{"ids":[2],"index":0,"item":null}}' ;;
        fail) echo "vectorcraft-cli: no such command" >&2; exit 1 ;;
        *) echo '{"step":"cmd","result":{}}' ;;
      esac
      shift 4 ;;
    *) echo "unknown $1" >&2; exit 2 ;;
  esac
done
"#;

/// The stand-in, written into `dir` with the patch fixture as what it exports and `error` as each
/// module's last error.
#[cfg(unix)]
fn stub(dir: &Path, error: &str) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;
    std::fs::create_dir_all(dir).unwrap();
    let doc = util::root().join("conformance/fixtures/vectorcraft/patch.vectorcraft");
    std::fs::write(dir.join("doc"), doc.to_string_lossy().as_bytes()).unwrap();
    std::fs::write(dir.join("error"), error).unwrap();
    let path = dir.join("vectorcraft-cli");
    std::fs::write(&path, STUB).unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn a_scenario_runs_and_its_checks_say_what_fails() {
    let dir = util::root().join("target/xtask-tests/scenario");
    let run_with = |text: &str, error: &str, case: &str| {
        let cli = stub(&dir.join(case), error);
        run(&scenario(text), &cli, Path::new("/plugins"), &dir.join(case).join("out"))
    };
    let good = r##"description = "d"
        [[run]]
        plugins = ["fill"]
        steps = [
          { cmd = "plugin.list" },
          { apply = "fill", params = { angle = 45, row_spacing_mm = 0.4 } },
          { export = "a.vectorcraft" },
          { export = "a.png" },
        ]
        [[run]]
        open = "a.vectorcraft"
        steps = [{ export = "b.vectorcraft" }, { export = "b.png" }]
        [expect]
        listed = ["fill"]
        applied = ["a.vectorcraft", "b.vectorcraft"]
        same = [["a.vectorcraft", "b.vectorcraft"]]
        elements = [{ file = "b.vectorcraft", element = "fill", count = 2, thread = "#e6b31a" }]
        "##;
    assert_eq!(run_with(good, "null", "good"), Vec::<String>::new());
    // What was sent differs from what was stored, and a module isn't listed.
    let changed = good.replace("angle = 45", "angle = 90").replace("listed = [\"fill\"]", "listed = [\"fill\", \"satin\"]");
    let failures = run_with(&changed, "null", "changed");
    assert_eq!(failures.len(), 3, "{failures:?}");
    assert!(failures[0].starts_with("plugin.list doesn't list dev.stitchcraft.satin as a live effect"), "{failures:?}");
    assert!(failures[1].starts_with("a.vectorcraft: it sews otherwise than its effects as they were sent"), "{failures:?}");
    assert!(failures[2].starts_with("b.vectorcraft: it sews otherwise"), "{failures:?}");
    // A module's last error, and a command VectorCraft refuses, stop the scenario.
    let failures = run_with(good, r#""the budget ran out""#, "error");
    assert_eq!(failures, [r#"run 1: after a.png, fill's last error is "the budget ran out""#]);
    let refused = good.replace("{ cmd = \"plugin.list\" }", "{ cmd = \"fail\" }");
    let failures = run_with(&refused, "null", "refused");
    assert_eq!(failures, ["vectorcraft-cli exit status: 1 after 1 results: vectorcraft-cli: no such command"]);
}
