//! Level B of the compatibility gate: VectorCraft's own command line drives the plug-ins, and StitchCraft
//! sews what VectorCraft saves (`docs/src/design/compatibility-gate.md` › Level B).
//!
//! A scenario, a file in `compat/scenarios/`, is one or more runs of `vectorcraft-cli run`. A run installs
//! the plug-ins it names, runs its steps, and asks each plug-in for its last error at the end, which must be
//! none. Then StitchCraft checks the files the runs exported.
//!
//! **Why VectorCraft draws everything.** A run starts from a new document, or from a file an earlier run
//! saved, and draws its shapes with VectorCraft's commands. So every file VectorCraft opens is one it wrote,
//! and every file StitchCraft reads is one the version under test wrote. The steps name no object: a command
//! in VectorCraft 0.7.0 can't be given an earlier one's result, so each works on the selection, which drawing
//! a shape sets. An `apply` step applies a stitch family's live effect, as its dialog does, to the whole of
//! each selected object.
//!
//! **Why StitchCraft checks.** VectorCraft's pictures change from version to version, and what sews must not,
//! so the checks are StitchCraft's own:
//!
//! - `listed`: `plugin.list` lists each module's live effect.
//! - `applied`: a file sews as it does with the StitchCraft records of the objects that `apply` steps
//!   reached put back as the steps sent them. So VectorCraft stored what the dialog set, on the objects it
//!   was applied to, in a form StitchCraft reads (`REQ-PRM-003`).
//! - `same`: 2 files sew alike, such as a file and the one saved from it without the plug-ins.
//! - `elements`: a file holds so many elements of one kind in one thread.
//!
//! A file is sewn as `stitch plan` sews it, for the reference machine, into a PES file
//! ([`stitchcraft_cli::commands::plan::sew`]). Comparing two is comparing their bytes.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use stitchcraft_cli::commands::plan::{Source, sew};
use stitchcraft_core::{Budget, Severity};
use stitchcraft_params::Family;
use stitchcraft_plan::FormatId;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::PLUGIN_NAMESPACE;

use crate::util;

/// Where the scenarios are.
pub const SCENARIOS: &str = "compat/scenarios";
/// How often a run of `vectorcraft-cli` is looked at, and how many looks it gets before it counts as hung:
/// 10 minutes. Counted in looks, as budgets are counted in work, not read from a clock.
const LOOK: Duration = Duration::from_millis(50);
const LOOKS: u32 = 12_000;
/// The design name the machine files compared are written with, so that only the stitches differ.
const NAME: &str = "compat";

/// One scenario.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scenario {
    /// What it shows, for people.
    pub description: String,
    /// The requirements it exercises.
    #[serde(default)]
    pub requirements: Vec<String>,
    /// The runs of `vectorcraft-cli`, in order.
    pub run: Vec<Run>,
    /// What StitchCraft checks afterwards.
    #[serde(default)]
    pub expect: Expect,
}

/// One run of `vectorcraft-cli run`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    /// A file an earlier run exported, to open; without one, the run starts a new document.
    #[serde(default)]
    pub open: Option<String>,
    /// The modules to install first: `hello`, or a stitch family's.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// The steps.
    pub steps: Vec<Step>,
}

/// A step: a VectorCraft command, a stitch family's live effect applied, or an export.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    /// A command's id, such as `shape.rectangle`.
    #[serde(default)]
    pub cmd: Option<String>,
    /// A stitch family whose live effect `effect.apply` applies to the whole of each selected object.
    #[serde(default)]
    pub apply: Option<String>,
    /// The command's parameters, or the effect's.
    #[serde(default)]
    pub params: Option<toml::Table>,
    /// A file to export, by name, in the format its extension names.
    #[serde(default)]
    pub export: Option<String>,
}

/// What StitchCraft checks once the runs are done.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Expect {
    /// Modules whose live effects `plugin.list` lists.
    #[serde(default)]
    pub listed: Vec<String>,
    /// Files that sew as they do with the records of the effects applied put back as they were sent.
    #[serde(default)]
    pub applied: Vec<String>,
    /// Pairs of files that sew alike.
    #[serde(default)]
    pub same: Vec<[String; 2]>,
    /// Elements a file holds.
    #[serde(default)]
    pub elements: Vec<Elements>,
}

/// So many elements of one kind, in one thread if it is named, in a file.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Elements {
    /// The file.
    pub file: String,
    /// What the elements' ids end with: `fill`, `stroke`, or a stitch family for an effect's element.
    pub element: String,
    /// How many.
    pub count: usize,
    /// Their thread, `#rrggbb`.
    #[serde(default)]
    pub thread: Option<String>,
}

/// What a step is, once read.
#[derive(Debug, PartialEq)]
enum Action<'a> {
    Command(&'a str, Value),
    Apply(Family, Value),
    Export(&'a str),
}

impl Step {
    /// What the step does, or why it can't be read: exactly one of `cmd`, `apply` and `export`, and
    /// parameters only for the first two.
    fn action(&self) -> Result<Action<'_>, String> {
        let params = || self.params.clone().map_or_else(|| json!({}), |table| toml_to_json(toml::Value::Table(table)));
        match (&self.cmd, &self.apply, &self.export) {
            (Some(cmd), None, None) => Ok(Action::Command(cmd, params())),
            (None, Some(family), None) => {
                let family = family_of(family).ok_or(format!("`apply = \"{family}\"` names no stitch family"))?;
                Ok(Action::Apply(family, params()))
            }
            (None, None, Some(file)) if self.params.is_none() => Ok(Action::Export(file)),
            _ => Err("a step has one of `cmd`, `apply` and `export`, and an export no `params`".to_string()),
        }
    }
}

/// The stitch family whose id is `id`.
fn family_of(id: &str) -> Option<Family> {
    Family::ALL.iter().copied().find(|family| family.id() == id)
}

/// The plug-in id of the module `name`: `dev.stitchcraft.fill`.
pub fn plugin_id(name: &str) -> String {
    format!("{PLUGIN_NAMESPACE}.{name}")
}

/// The effect id VectorCraft records for `family`'s live effect: `plugin.dev.stitchcraft.fill`.
fn effect_id(family: Family) -> String {
    format!("plugin.{}", plugin_id(family.id()))
}

/// A TOML value as JSON. TOML's dates, which no step needs, become their text.
fn toml_to_json(value: toml::Value) -> Value {
    match value {
        toml::Value::String(s) => Value::String(s),
        toml::Value::Integer(i) => json!(i),
        toml::Value::Float(f) => json!(f),
        toml::Value::Boolean(b) => Value::Bool(b),
        toml::Value::Datetime(d) => Value::String(d.to_string()),
        toml::Value::Array(items) => Value::Array(items.into_iter().map(toml_to_json).collect()),
        toml::Value::Table(table) => Value::Object(table.into_iter().map(|(k, v)| (k, toml_to_json(v))).collect()),
    }
}

/// Every scenario, by name (its file name without `.toml`), in name order.
pub fn load(root: &Path) -> Result<Vec<(String, Scenario)>, String> {
    let mut scenarios = Vec::new();
    for path in util::files(&root.join(SCENARIOS), &["toml"]) {
        let name = path.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let scenario: Scenario = toml::from_str(&util::read(&path)?).map_err(|e| format!("{}: {e}", util::rel(&path)))?;
        scenarios.push((name, scenario));
    }
    Ok(scenarios)
}

/// What is wrong with `scenario` before it runs: steps that don't read, modules or families that don't
/// exist, requirements that don't, and checks of files no step exports or opens before it is exported.
pub fn problems(scenario: &Scenario, modules: &[&str], requirements: &std::collections::BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();
    let mut exported: Vec<&str> = Vec::new();
    for (i, run) in scenario.run.iter().enumerate() {
        if let Some(open) = &run.open
            && !exported.contains(&open.as_str())
        {
            found.push(format!("run {}: it opens `{open}`, which no earlier run exports", i + 1));
        }
        for module in &run.plugins {
            if !modules.contains(&module.as_str()) {
                found.push(format!("run {}: there is no module `{module}`", i + 1));
            }
        }
        for step in &run.steps {
            match step.action() {
                Ok(Action::Export(file)) => exported.push(file),
                Ok(Action::Apply(family, _)) if !run.plugins.iter().any(|m| m == family.id()) => {
                    found.push(format!("run {}: it applies `{}` without installing that module", i + 1, family.id()));
                }
                Ok(_) => {}
                Err(e) => found.push(format!("run {}: {e}", i + 1)),
            }
        }
    }
    let checked = scenario.expect.applied.iter().chain(scenario.expect.same.iter().flatten()).chain(scenario.expect.elements.iter().map(|e| &e.file));
    for file in checked {
        if !exported.contains(&file.as_str()) {
            found.push(format!("it checks `{file}`, which no run exports"));
        }
    }
    for module in &scenario.expect.listed {
        if !modules.contains(&module.as_str()) {
            found.push(format!("it expects a module `{module}` listed, and there is none"));
        }
    }
    for id in &scenario.requirements {
        if !requirements.contains(id) {
            found.push(format!("there is no requirement {id}"));
        }
    }
    found
}

/// The arguments of `vectorcraft-cli` for `run`: open the file it names in `out`, install the modules it
/// names from `plugins`, run its steps with exports into `out`, and ask each module for its last error.
fn arguments(run: &Run, plugins: &Path, out: &Path) -> Result<Vec<String>, String> {
    let path = |dir: &Path, file: &str| dir.join(file).to_string_lossy().into_owned();
    let command = |id: &str, params: Value| ["--cmd".to_string(), id.to_string(), "--params".to_string(), params.to_string()];
    let mut args = vec!["run".to_string()];
    if let Some(open) = &run.open {
        args.extend(["--in".to_string(), path(out, open)]);
    }
    for module in &run.plugins {
        args.extend(command("plugin.install", json!({ "path": path(plugins, &format!("{module}.wasm")) })));
    }
    for step in &run.steps {
        match step.action()? {
            Action::Command(id, params) => args.extend(command(id, params)),
            Action::Apply(family, params) => {
                args.extend(command("effect.apply", json!({ "effect": effect_id(family), "item": null, "params": params })))
            }
            Action::Export(file) => args.extend(["--export".to_string(), path(out, file)]),
        }
    }
    for module in &run.plugins {
        args.extend(command("plugin.info", json!({ "id": plugin_id(module) })));
    }
    Ok(args)
}

/// An effect an `apply` step applied: the objects VectorCraft says it applied it to, and the record the
/// step sent.
#[derive(Clone, Debug, PartialEq)]
struct Applied {
    ids: Vec<u64>,
    record: Value,
}

/// What the runs left: each exported file's effects applied so far, and what `plugin.list` listed.
#[derive(Default)]
struct Ran {
    applied: BTreeMap<String, Vec<Applied>>,
    listed: Vec<Value>,
}

/// Runs `scenario` with the `vectorcraft-cli` at `cli` and the modules in `plugins`, into `out`, and returns
/// what failed, or nothing.
pub fn run(scenario: &Scenario, cli: &Path, plugins: &Path, out: &Path) -> Vec<String> {
    if let Err(e) = std::fs::create_dir_all(out) {
        return vec![format!("{}: {e}", util::rel(out))];
    }
    let mut ran = Ran::default();
    for (i, run) in scenario.run.iter().enumerate() {
        if let Err(failure) = run_once(run, i + 1, cli, plugins, out, &mut ran) {
            return vec![failure];
        }
    }
    check(&scenario.expect, out, &ran)
}

/// One run: `vectorcraft-cli` and what its results say. Its exports' effects go to `ran`.
fn run_once(run: &Run, number: usize, cli: &Path, plugins: &Path, out: &Path, ran: &mut Ran) -> Result<(), String> {
    let args = arguments(run, plugins, out)?;
    let results = launch(cli, &args, &out.join(format!("run-{number}")))?;
    // One result for the opened file, one for each command and export, in order.
    let mut results = results.into_iter();
    let mut applied: Vec<Applied> = match &run.open {
        Some(open) => {
            results.next();
            ran.applied.get(open).cloned().unwrap_or_default()
        }
        None => Vec::new(),
    };
    let mut next = |what: &str| results.next().ok_or(format!("run {number}: vectorcraft-cli gave no result for {what}"));
    for module in &run.plugins {
        let installed = next(&format!("installing {module}"))?;
        let kind = installed.pointer("/result/kind").and_then(Value::as_str);
        if kind != Some("effect") {
            return Err(format!("run {number}: {module} installed as {kind:?}, not as a live effect: {installed}"));
        }
    }
    for step in &run.steps {
        match step.action()? {
            Action::Command(id, _) => {
                let result = next(id)?;
                if id == "plugin.list" {
                    ran.listed.extend(result.pointer("/result/plugins").and_then(Value::as_array).cloned().unwrap_or_default());
                }
            }
            Action::Apply(family, params) => {
                let result = next(&format!("applying {}", family.id()))?;
                let ids: Vec<u64> = result
                    .pointer("/result/ids")
                    .and_then(Value::as_array)
                    .map(|ids| ids.iter().filter_map(Value::as_u64).collect())
                    .unwrap_or_default();
                let item = result.pointer("/result/item").unwrap_or(&Value::Null);
                if ids.is_empty() || !item.is_null() {
                    return Err(format!(
                        "run {number}: {} was applied to the objects {ids:?} and the item {item}, not to a whole object: {result}",
                        family.id()
                    ));
                }
                applied.push(Applied { ids, record: json!({ "id": effect_id(family), "params": params }) });
            }
            Action::Export(file) => {
                next(&format!("exporting {file}"))?;
                ran.applied.insert(file.to_string(), applied.clone());
            }
        }
    }
    for module in &run.plugins {
        let info = next(&format!("{module}'s last error"))?;
        match info.pointer("/result/lastError") {
            Some(Value::Null) | None => {}
            Some(error) => return Err(format!("run {number}: {module}'s last error is {error}")),
        }
    }
    Ok(())
}

/// Runs `cli` with `args`, its output in files named `stem` with `.out` and `.err`, and returns the JSON
/// results it printed, one a line, or why it failed: its exit status, or its time running out, with the end
/// of what it wrote to stderr.
fn launch(cli: &Path, args: &[String], stem: &Path) -> Result<Vec<Value>, String> {
    let (out_path, err_path) = (stem.with_extension("out"), stem.with_extension("err"));
    let file = |path: &Path| std::fs::File::create(path).map_err(|e| format!("{}: {e}", util::rel(path)));
    let mut child = Command::new(cli)
        .args(args)
        .stdin(Stdio::null())
        .stdout(file(&out_path)?)
        .stderr(file(&err_path)?)
        .spawn()
        .map_err(|e| format!("{}: {e}", cli.display()))?;
    let mut looks = 0;
    let status = loop {
        match child.try_wait().map_err(|e| format!("{}: {e}", cli.display()))? {
            Some(status) => break status,
            None if looks >= LOOKS => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("vectorcraft-cli didn't finish within {} s", (LOOK * LOOKS).as_secs()));
            }
            None => {
                looks += 1;
                std::thread::sleep(LOOK);
            }
        }
    };
    let printed = std::fs::read_to_string(&out_path).unwrap_or_default();
    let results: Vec<Value> = printed.lines().filter(|line| !line.trim().is_empty()).filter_map(|line| serde_json::from_str(line).ok()).collect();
    if !status.success() {
        let said = std::fs::read_to_string(&err_path).unwrap_or_default();
        let tail: Vec<&str> = said.lines().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect();
        return Err(format!("vectorcraft-cli {} after {} results: {}", status, results.len(), tail.join(" | ")));
    }
    Ok(results)
}

/// What fails of `expect`, with the files in `out` and what the runs left.
fn check(expect: &Expect, out: &Path, ran: &Ran) -> Vec<String> {
    let mut failures = Vec::new();
    for module in &expect.listed {
        let id = plugin_id(module);
        let listed =
            ran.listed.iter().any(|p| p.get("id").and_then(Value::as_str) == Some(&id) && p.get("kind").and_then(Value::as_str) == Some("effect"));
        if !listed {
            failures.push(format!("plugin.list doesn't list {id} as a live effect: {}", Value::Array(ran.listed.clone())));
        }
    }
    let read = |file: &str| util::read_bytes(&out.join(file));
    for file in &expect.applied {
        let applied = ran.applied.get(file).map_or(&[][..], Vec::as_slice);
        if let Err(e) = read(file).and_then(|bytes| sews_as_applied(&bytes, applied)) {
            failures.push(format!("{file}: {e}"));
        }
    }
    for [a, b] in &expect.same {
        let sewn = |file: &str| read(file).and_then(|bytes| sewn(&bytes)).map_err(|e| format!("{file}: {e}"));
        match (sewn(a), sewn(b)) {
            (Ok(x), Ok(y)) if x == y => {}
            (Ok(x), Ok(y)) => failures.push(format!("{a} and {b} sew differently: {} and {} bytes of PES", x.len(), y.len())),
            (Err(e), _) | (_, Err(e)) => failures.push(e),
        }
    }
    for expected in &expect.elements {
        if let Err(e) = read(&expected.file).and_then(|bytes| holds(&bytes, expected)) {
            failures.push(format!("{}: {e}", expected.file));
        }
    }
    failures
}

/// The PES file `stitch plan` writes from the VectorCraft document `bytes` for the reference machine, or
/// the errors that stop it.
fn sewn(bytes: &[u8]) -> Result<Vec<u8>, String> {
    let sewn = sew(Source::VectorCraft, bytes, REFERENCE, FormatId::PesV1, NAME);
    let errors: Vec<String> = sewn.diagnostics.iter().filter(|d| d.severity() == Severity::Error).map(ToString::to_string).collect();
    match sewn.file {
        Some(file) if errors.is_empty() => Ok(file),
        _ => Err(format!("StitchCraft doesn't sew it: {}", errors.join("; "))),
    }
}

/// Whether the document `bytes` sews as it does with the StitchCraft records of the objects `applied`
/// reached put back as the steps sent them, or how it doesn't.
fn sews_as_applied(bytes: &[u8], applied: &[Applied]) -> Result<(), String> {
    let mut doc: Value =
        serde_json::from_slice(stitchcraft_vectorcraft::read_part(bytes)).map_err(|e| format!("not JSON StitchCraft can change: {e}"))?;
    let stored = restore(&mut doc, applied);
    let expected = serde_json::to_vec(&doc).map_err(|e| e.to_string())?;
    let (got, wanted) = (sewn(bytes)?, sewn(&expected)?);
    if got == wanted {
        return Ok(());
    }
    let sent: Vec<&Value> = applied.iter().map(|a| &a.record).collect();
    Err(format!("it sews otherwise than its effects as they were sent: VectorCraft stored {}, and the steps sent {}", json!(stored), json!(sent)))
}

/// Puts back in `doc` the StitchCraft records of the objects `applied` names as the steps sent them, and
/// returns the records VectorCraft had stored there. A record is put on the object as a whole, as an `apply`
/// step applies it, and any StitchCraft record on one of the object's fills or strokes is taken away.
fn restore(doc: &mut Value, applied: &[Applied]) -> Vec<Value> {
    let mut sent: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
    for a in applied {
        for id in &a.ids {
            sent.entry(*id).or_default().push(a.record.clone());
        }
    }
    let prefix = format!("plugin.{PLUGIN_NAMESPACE}.");
    let ours = |effect: &Value| effect.get("id").and_then(Value::as_str).is_some_and(|id| id.starts_with(&prefix));
    let mut stored = Vec::new();
    visit(doc, &mut |node| {
        let Some(records) = node.get("id").and_then(Value::as_u64).and_then(|id| sent.get(&id)) else { return };
        let Some(node) = node.as_object_mut() else { return };
        let Some(appearance) = node.entry("appearance").or_insert_with(|| json!({})).as_object_mut() else { return };
        let items = appearance.get_mut("items").and_then(Value::as_array_mut).into_iter().flatten();
        for effects in items.filter_map(|item| item.get_mut("effects").and_then(Value::as_array_mut)) {
            stored.extend(effects.iter().filter(|e| ours(e)).cloned());
            effects.retain(|e| !ours(e));
        }
        if let Some(effects) = appearance.entry("effects").or_insert_with(|| json!([])).as_array_mut() {
            stored.extend(effects.iter().filter(|e| ours(e)).cloned());
            effects.retain(|e| !ours(e));
            effects.extend(records.iter().cloned());
        }
    });
    stored
}

/// Calls `f` on every object of the document `value` that is a VectorCraft node: one with a `kind` that
/// says its `type`.
fn visit(value: &mut Value, f: &mut dyn FnMut(&mut Value)) {
    if value.get("kind").and_then(|kind| kind.get("type")).is_some() {
        f(value);
    }
    match value {
        Value::Object(map) => map.values_mut().for_each(|v| visit(v, f)),
        Value::Array(items) => items.iter_mut().for_each(|v| visit(v, f)),
        _ => {}
    }
}

/// Whether the document `bytes` holds the elements `expected` says, or what it holds instead.
fn holds(bytes: &[u8], expected: &Elements) -> Result<(), String> {
    let read = stitchcraft_vectorcraft::read(bytes, &Budget::DEFAULT).map_err(|e| e.to_string())?;
    let suffix = format!(":{}", expected.element);
    let all: Vec<(String, String)> = read.design.elements().iter().map(|e| (e.id.to_string(), e.thread.color.to_string())).collect();
    let count =
        all.iter().filter(|(id, thread)| id.ends_with(&suffix) && expected.thread.as_ref().is_none_or(|t| t.eq_ignore_ascii_case(thread))).count();
    if count == expected.count {
        return Ok(());
    }
    let thread = expected.thread.as_deref().map_or_else(String::new, |t| format!(" in {t}"));
    let found: Vec<String> = all.iter().map(|(id, thread)| format!("{id} {thread}")).collect();
    Err(format!("{} `{}` elements{thread} are expected, and it holds {count}: {}", expected.count, expected.element, found.join(", ")))
}

/// The scenarios' names, and the `vectorcraft-cli` at `cli`, ran with the modules in `plugins`: what failed,
/// by scenario. The runs' files go to `out/<scenario>/`.
pub fn run_all(cli: &Path, plugins: &Path, out: &Path) -> Result<Vec<(String, Vec<String>)>, String> {
    let root = util::root();
    let scenarios = load(&root)?;
    let requirements = crate::conformance::requirement_ids()?;
    let broken: Vec<String> = scenarios
        .iter()
        .flat_map(|(name, scenario)| {
            problems(scenario, &super::modules(), &requirements).into_iter().map(move |p| format!("{SCENARIOS}/{name}.toml: {p}"))
        })
        .collect();
    if !broken.is_empty() {
        return Err(broken.join("\n"));
    }
    if !cli.is_file() {
        return Err(format!("there is no vectorcraft-cli at {}", cli.display()));
    }
    let mut outcomes = Vec::new();
    for (name, scenario) in &scenarios {
        println!("compat: scenario {name}: {}", scenario.description);
        let failures = run(scenario, cli, plugins, &out.join(name));
        outcomes.push((name.clone(), failures));
    }
    Ok(outcomes)
}

/// Where Level B puts each scenario's files.
pub fn out_dir() -> PathBuf {
    util::root().join("target/compat/e2e")
}

#[cfg(test)]
mod tests;
