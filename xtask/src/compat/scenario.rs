//! Level B of the compatibility gate: VectorCraft's own command line drives the plug-ins, and StitchCraft
//! sews what VectorCraft saves (`docs/src/design/compatibility-gate.md` › Level B).
//!
//! A scenario, a file in `compat/scenarios/`, is one or more runs of `vectorcraft-cli run`. A run installs
//! the plug-ins it names and runs its steps. Then StitchCraft checks the files the runs exported.
//!
//! **Runs of `stitch`.** A run can instead be `stitch plan` with the arguments `stitch` names, which traces a
//! picture with the `vectorcraft-cli` under test (`REQ-CLI-012`): the command line's own steps, through the
//! code users run, on every track. It runs in this process, its files in the scenario's folder, and must end
//! well. The files it writes count as exported. A run's `files` are files StitchCraft makes, such as the
//! picture `badge.png`, written into the scenario's folder first ([`trace::made`]).
//!
//! **Why a drawing after each effect.** VectorCraft runs a live effect once for an object and its
//! parameters, keeps the result, and remembers only the module's last failure, which the next success
//! clears (its `crates/plugins/src/effect.rs`). So after each export that draws the document, a PNG,
//! each module installed is asked for its last error, which must be none. Each `apply` step must be shown
//! by a drawing before the same family is applied again, so that its object's failure can't be cleared
//! by another's success first. A scenario that gives an object an effect in another way, such as a graphic
//! style, draws it before the next.
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
//! - `listed`: `plugin.list` lists each module as the kind of plug-in it is: a live effect, or an exporter
//!   (`super::kind`).
//! - `applied`: a file sews as it does with the StitchCraft records of the objects that `apply` steps
//!   reached put back as the steps sent them. So VectorCraft stored what the dialog set, on the objects it
//!   was applied to, in a form StitchCraft reads (`REQ-PRM-003`).
//! - `same`: 2 files sew alike, such as a file and the one saved from it without the plug-ins.
//! - `elements`: a file holds so many elements of one kind in one thread, with the settings given. A
//!   paint's fill and a fill effect's are both `fill` elements, and the settings tell them apart.
//! - `exported`: a machine file that StitchCraft's exporter wrote in VectorCraft is the one `stitch export`
//!   writes of the document saved beside it, and the export's warnings are what `stitch export` says, as
//!   the exporter words them (`REQ-VC-011`).
//! - `kept`: a document keeps the design settings and machine the Design Settings filter was run with, as
//!   the VectorCraft adapter reads them (`REQ-VC-013`).
//! - `differ`: 2 machine files are not the same, such as one exported before the filter ran and one after.
//! - `sheets`: a document sews as the test sheet the Test Sheet filter drew into it: the sheet's design,
//!   moved by the whole millimetres that bring it nearest to where the document has it, as the filter
//!   moves it to its artboard (`REQ-VC-015`).
//!
//! **Why a heavy module is allowed.** VectorCraft 0.17 installs a filter that asks for a heavy budget only
//! once the user allows it, with `allowHeavy: true`, which a run passes for each heavy module it installs
//! (Stroke to Satin's, `super::heavy`).
//!
//! A file is sewn as `stitch plan` sews it without `--profile`, for the machine it keeps or else the
//! reference machine, into a PES file ([`stitchcraft_cli::commands::plan::sew`]). Comparing two is comparing
//! their bytes. Exporting a machine file draws nothing: VectorCraft gives the exporter the document's JSON,
//! and no live effect runs.
//!
//! **Why only a run's last step may fail.** An export that must fail names the code of the error it says,
//! with `fails`, such as `SC-E0701` for a design its machine's hoop can't hold. VectorCraft's `run` stops at
//! the first step that fails and says why, with its exit status a failure (its
//! `apps/vectorcraft-cli/src/main.rs`), so that export is its run's last step, and no check or later run
//! names the file it doesn't write.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use clap::Parser as _;
use std::time::Duration;

use serde::Deserialize;
use serde_json::{Value, json};
use stitchcraft_cli::cli::{Cli, Command as Stitch, PlanArgs};
use stitchcraft_cli::commands::Status;
use stitchcraft_cli::commands::plan::{self, Source, machine_for, sew};
use stitchcraft_core::{Budget, Code, Diagnostic, Rect, Severity};
use stitchcraft_engine::design::{Reading, Setting};
use stitchcraft_params::Family;
use stitchcraft_plan::FormatId;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vc_plugin::testsheet;
use stitchcraft_vectorcraft::{EXTENSIONS, effect_id, effect_name, param_text, plugin_id};

use super::{install, kind};
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

/// One run of `vectorcraft-cli run`, or of `stitch plan`.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Run {
    /// A file an earlier run exported, to open; without one, the run starts a new document.
    #[serde(default)]
    pub open: Option<String>,
    /// The modules to install first: `hello`, or a stitch family's.
    #[serde(default)]
    pub plugins: Vec<String>,
    /// The steps.
    #[serde(default)]
    pub steps: Vec<Step>,
    /// The pixels per point of the run's pictures (`--scale`); VectorCraft's own, 1, without one.
    #[serde(default)]
    pub scale: Option<f64>,
    /// `stitch`'s arguments, for a run of `stitch plan` instead, which traces a picture with the
    /// `vectorcraft-cli` under test. Its files are in the scenario's folder.
    #[serde(default)]
    pub stitch: Vec<String>,
    /// Files StitchCraft makes, written into the scenario's folder before the run ([`trace::made`]).
    #[serde(default)]
    pub files: Vec<String>,
}

/// A step: a VectorCraft command, a stitch family's live effect applied, or an export.
#[derive(Clone, Debug, Deserialize)]
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
    /// For an export that must fail, the code of the error it says, such as `SC-E0701`.
    #[serde(default)]
    pub fails: Option<String>,
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
    /// Pairs of a document and a machine file exported with it, which is what `stitch export` writes of it.
    #[serde(default)]
    pub exported: Vec<[String; 2]>,
    /// Documents, each with the design settings and the machine it keeps.
    #[serde(default)]
    pub kept: Vec<Kept>,
    /// Pairs of machine files that are not the same.
    #[serde(default)]
    pub differ: Vec<[String; 2]>,
    /// Documents, each with the test sheet the Test Sheet filter drew into it.
    #[serde(default)]
    pub sheets: Vec<Sheet>,
    /// Documents of traced pictures, each with the threads it sews digitized automatically.
    #[serde(default)]
    pub traced: Vec<Traced>,
}

/// A document of a traced picture and the threads it sews digitized automatically ([`trace::traced`]).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Traced {
    /// The document.
    pub file: String,
    /// The threads, `#rrggbb`, in any order.
    pub threads: Vec<String>,
    /// Whether StitchCraft's Auto effect makes its shapes automatic, so that it sews digitized
    /// automatically without `--auto` (`REQ-VC-016`).
    #[serde(default)]
    pub effect: bool,
}

/// A document and the test sheet the Test Sheet filter drew into it.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sheet {
    /// The document.
    pub file: String,
    /// The sheet's id, as the filter's dialog names it.
    pub sheet: String,
}

/// A document and what the Design Settings filter keeps with it: the machine as `profile`, and the design
/// settings it names, by their names, each in millimetres.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kept {
    /// The document.
    pub file: String,
    /// The settings, as the filter's dialog names them.
    pub settings: toml::Table,
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
    /// Settings they all have, as an effect's dialog sets them.
    #[serde(default)]
    pub params: Option<toml::Table>,
}

/// What a step is, once read.
#[derive(Debug, PartialEq)]
enum Action<'a> {
    Command(&'a str, Value),
    Apply(Family, Value),
    Export(&'a str),
}

impl Step {
    /// The step that exports `file`, in the format its extension names.
    pub(crate) fn exporting(file: &str) -> Self {
        Step { cmd: None, apply: None, params: None, export: Some(file.to_string()), fails: None }
    }

    /// Whether the step reads: exactly one of `cmd`, `apply` and `export`, a family that is one, parameters
    /// only for the first two, and a code of an error that `fails` only for an export.
    pub(crate) fn check(&self) -> Result<(), String> {
        self.action().map(drop)
    }

    /// What the step does, or why it can't be read: exactly one of `cmd`, `apply` and `export`, parameters
    /// only for the first two, and `fails` only for an export, with the code of an error.
    fn action(&self) -> Result<Action<'_>, String> {
        // A parameter written `{ stitchcraft = "…" }` is given the data StitchCraft makes of the name.
        let params = || trace::generated(self.params.clone().map_or_else(|| json!({}), |table| toml_to_json(toml::Value::Table(table))));
        if let Some(code) = &self.fails
            && !Code::ALL.iter().any(|known| known.id() == code && known.severity() == Severity::Error)
        {
            return Err(format!("`fails = \"{code}\"` names no error StitchCraft says"));
        }
        match (&self.cmd, &self.apply, &self.export) {
            (Some(cmd), None, None) if self.fails.is_none() => Ok(Action::Command(cmd, params()?)),
            (None, Some(family), None) if self.fails.is_none() => {
                let family = family_of(family).ok_or(format!("`apply = \"{family}\"` names no stitch family"))?;
                Ok(Action::Apply(family, params()?))
            }
            (None, None, Some(file)) if self.params.is_none() => Ok(Action::Export(file)),
            _ => Err("a step has one of `cmd`, `apply` and `export`, an export no `params`, and only an export `fails`".to_string()),
        }
    }
}

/// The stitch family whose id is `id`.
fn family_of(id: &str) -> Option<Family> {
    Family::ALL.iter().copied().find(|family| family.id() == id)
}

/// Whether exporting `file` draws the document, which runs its live effects: any format but VectorCraft's
/// own and the machine formats, which StitchCraft's exporters write from the document's JSON.
fn draws(file: &str) -> bool {
    extension(file).is_none_or(|e| !EXTENSIONS.contains(&e) && FormatId::from_extension(e).is_none())
}

/// The extension of `file`.
fn extension(file: &str) -> Option<&str> {
    Path::new(file).extension().and_then(|e| e.to_str())
}

/// The machine format StitchCraft writes into `file`, by its extension.
fn machine_format(file: &str) -> Option<FormatId> {
    extension(file).and_then(FormatId::from_extension)
}

/// Whether the module `name` is a live effect, which a drawing runs and which keeps its last error.
fn live(name: &str) -> bool {
    kind(name) == "effect"
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
/// exist, requirements that don't, checks of files no step exports, an export that fails before its run's
/// last step, an effect no drawing shows before its family is applied again or its run ends, and an
/// `applied` check of a file no effect was applied to.
pub fn problems(scenario: &Scenario, modules: &[&str], requirements: &std::collections::BTreeSet<String>) -> Vec<String> {
    let mut found = Vec::new();
    let mut exported: Vec<&str> = Vec::new();
    // The files exported after an `apply` step, in their run or before the file their run opened.
    let mut applied_to: Vec<&str> = Vec::new();
    for (i, run) in scenario.run.iter().enumerate() {
        let mut said = Vec::new();
        for file in &run.files {
            match trace::made(file) {
                Ok(_) => exported.push(file),
                Err(e) => said.push(e),
            }
        }
        if run.stitch.is_empty() {
            said.extend(vectorcraft_problems(run, modules, &mut exported, &mut applied_to));
        } else {
            said.extend(stitch_problems(run, &mut exported));
        }
        found.extend(said.into_iter().map(|problem| format!("run {}: {problem}", i + 1)));
    }
    let expect = &scenario.expect;
    let checked = expect.applied.iter().chain(expect.same.iter().flatten()).chain(expect.elements.iter().map(|e| &e.file));
    let checked = checked.chain(expect.kept.iter().map(|kept| &kept.file)).chain(expect.differ.iter().flatten());
    let checked = checked.chain(expect.sheets.iter().map(|sheet| &sheet.file)).chain(expect.traced.iter().map(|traced| &traced.file));
    for file in checked.chain(expect.exported.iter().flatten()) {
        if !exported.contains(&file.as_str()) {
            found.push(format!("it checks `{file}`, which no run exports"));
        }
    }
    for [document, machine] in &expect.exported {
        if machine_format(machine).is_none() {
            found.push(format!("it checks `{machine}` as a machine file, and StitchCraft writes no format with its extension"));
        }
        if extension(document).is_none_or(|e| !EXTENSIONS.contains(&e)) {
            found.push(format!("it checks `{machine}` against `{document}`, which isn't a VectorCraft document"));
        }
    }
    for file in &scenario.expect.applied {
        if exported.contains(&file.as_str()) && !applied_to.contains(&file.as_str()) {
            found.push(format!("it checks `{file}` as applied, and no `apply` step comes before it is exported"));
        }
    }
    for module in &scenario.expect.listed {
        if !modules.contains(&module.as_str()) {
            found.push(format!("it expects a module `{module}` listed, and there is none"));
        }
    }
    for sheet in &scenario.expect.sheets {
        if !testsheet::sheets().any(|drawn| drawn.id == sheet.sheet) {
            found.push(format!("it checks `{}` for the test sheet {}, which the Test Sheet filter doesn't draw", sheet.file, sheet.sheet));
        }
    }
    for id in &scenario.requirements {
        if !requirements.contains(id) {
            found.push(format!("there is no requirement {id}"));
        }
    }
    found
}

/// What is wrong with `run`, a run of VectorCraft's command line, before it runs: a file it opens that no
/// earlier run exports, a module that doesn't exist, steps that don't read, an export that fails before its
/// last step, and an effect no drawing shows before its family is applied again or the run ends. The files it
/// exports go to `exported`, and those exported after an `apply` step, or after opening a file one was
/// applied to, to `applied_to` too.
fn vectorcraft_problems<'a>(run: &'a Run, modules: &[&str], exported: &mut Vec<&'a str>, applied_to: &mut Vec<&'a str>) -> Vec<String> {
    let mut found = Vec::new();
    let mut applied = false;
    if let Some(open) = &run.open {
        if !exported.contains(&open.as_str()) {
            found.push(format!("it opens `{open}`, which no earlier run exports"));
        }
        applied = applied_to.contains(&open.as_str());
    }
    for module in &run.plugins {
        if !modules.contains(&module.as_str()) {
            found.push(format!("there is no module `{module}`"));
        }
    }
    // The families applied since the last drawing.
    let mut undrawn: Vec<Family> = Vec::new();
    let last = run.steps.len().saturating_sub(1);
    for (j, step) in run.steps.iter().enumerate() {
        if step.fails.is_some() && step.export.is_some() && j != last {
            found.push("an export that fails is not its last step, and VectorCraft runs no step after it".to_string());
        }
        match step.action() {
            Ok(Action::Export(file)) => {
                if step.fails.is_none() {
                    exported.push(file);
                    if applied {
                        applied_to.push(file);
                    }
                }
                if let Some(format) = machine_format(file).filter(|format| !run.plugins.iter().any(|m| m == format.extension())) {
                    found.push(format!("it exports `{file}` without installing the `{}` module", format.extension()));
                }
                if draws(file) {
                    undrawn.clear();
                }
            }
            Ok(Action::Apply(family, _)) => {
                if !run.plugins.iter().any(|m| m == family.id()) {
                    found.push(format!("it applies `{}` without installing that module", family.id()));
                }
                if undrawn.contains(&family) {
                    found.push(format!("it applies `{}` again before a drawing shows the first", family.id()));
                } else {
                    undrawn.push(family);
                }
                applied = true;
            }
            Ok(Action::Command(..)) => {}
            Err(e) => found.push(e),
        }
    }
    for family in undrawn {
        found.push(format!("it applies `{}`, and no drawing shows it after", family.id()));
    }
    found
}

/// What is wrong with `run`, a run of `stitch`, before it runs: arguments that aren't `stitch plan`'s, one
/// of VectorCraft's run's fields beside them, a `--vectorcraft-cli` of its own, or a design no file made
/// before it is. The files it writes go to `exported`.
fn stitch_problems<'a>(run: &'a Run, exported: &mut Vec<&'a str>) -> Vec<String> {
    let args = match plan_args(&run.stitch) {
        Ok(args) => args,
        Err(e) => return vec![e],
    };
    let mut found = Vec::new();
    if run.open.is_some() || !run.plugins.is_empty() || !run.steps.is_empty() || run.scale.is_some() {
        found.push("a run of `stitch` has no `open`, `plugins`, `steps` or `scale`".to_string());
    }
    if args.tracing.vectorcraft_cli.is_some() {
        found.push("a run of `stitch` traces with the `vectorcraft-cli` under test, and names none".to_string());
    }
    let design = run.stitch.iter().find(|word| Path::new(word) == args.design);
    match design {
        Some(design) if exported.contains(&design.as_str()) => {}
        _ => found.push(format!("`stitch` reads `{}`, which no file before it is", args.design.display())),
    }
    let writes: Vec<&Path> = [Some(&args.output), args.preview.as_ref(), args.report.as_ref(), args.tracing.traced.as_ref()]
        .into_iter()
        .flatten()
        .map(PathBuf::as_path)
        .collect();
    exported.extend(run.stitch.iter().filter(|word| writes.contains(&Path::new(word.as_str()))).map(String::as_str));
    found
}

/// `stitch plan`'s arguments in `words`, `stitch`'s arguments, or why they aren't.
fn plan_args(words: &[String]) -> Result<PlanArgs, String> {
    let shown = words.join(" ");
    let parsed = Cli::try_parse_from(std::iter::once("stitch").chain(words.iter().map(String::as_str)));
    match parsed.map_err(|e| format!("`stitch {shown}`: {}", e.render()))?.command {
        Stitch::Plan(args) => Ok(args),
        _ => Err(format!("`stitch {shown}` isn't `stitch plan`")),
    }
}

/// A run of `stitch plan` with `run`'s arguments, its files in `out`, which traces with the `vectorcraft-cli`
/// at `cli`; how it ended when it didn't end well.
fn stitch_once(run: &Run, number: usize, cli: &Path, out: &Path) -> Result<(), String> {
    let mut args = plan_args(&run.stitch).map_err(|e| format!("run {number}: {e}"))?;
    let inside = |path: &mut PathBuf| *path = out.join(&*path);
    inside(&mut args.design);
    inside(&mut args.output);
    args.preview.iter_mut().chain(args.report.iter_mut()).chain(args.tracing.traced.iter_mut()).for_each(inside);
    args.tracing.vectorcraft_cli = Some(cli.to_path_buf());
    let outcome = plan::run(&args);
    match outcome.status {
        Status::Done => Ok(()),
        status => Err(format!("run {number}: `stitch {}` ended with {status:?}: {}", run.stitch.join(" "), outcome.stderr.trim_end())),
    }
}

/// The arguments of `vectorcraft-cli` for `run`: open the file it names in `out`, install the modules it
/// names from `plugins`, and run its steps with exports into `out`, each module asked for its last error
/// after each export that draws.
fn arguments(run: &Run, plugins: &Path, out: &Path) -> Result<Vec<String>, String> {
    let path = |dir: &Path, file: &str| dir.join(file).to_string_lossy().into_owned();
    let command = |id: &str, params: Value| ["--cmd".to_string(), id.to_string(), "--params".to_string(), params.to_string()];
    let mut args = vec!["run".to_string()];
    if let Some(open) = &run.open {
        args.extend(["--in".to_string(), path(out, open)]);
    }
    if let Some(scale) = run.scale {
        args.extend(["--scale".to_string(), scale.to_string()]);
    }
    for module in &run.plugins {
        args.extend(command("plugin.install", install(module, &path(plugins, &format!("{module}.wasm")))));
    }
    for step in &run.steps {
        match step.action()? {
            Action::Command(id, params) => args.extend(command(id, params)),
            Action::Apply(family, params) => {
                args.extend(command("effect.apply", json!({ "effect": effect_id(family.id()), "item": null, "params": params })));
            }
            Action::Export(file) => {
                args.extend(["--export".to_string(), path(out, file)]);
                if draws(file) {
                    for module in run.plugins.iter().filter(|module| live(module)) {
                        args.extend(command("plugin.info", json!({ "id": plugin_id(module) })));
                    }
                }
            }
        }
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

/// What the runs left: each exported file's effects applied so far, what each export warned of, and what
/// `plugin.list` listed.
#[derive(Default)]
struct Ran {
    applied: BTreeMap<String, Vec<Applied>>,
    warnings: BTreeMap<String, Vec<String>>,
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
        let number = i + 1;
        let made = run.files.iter().try_for_each(|file| {
            let bytes = trace::made(file).map_err(|e| format!("run {number}: {e}"))?;
            std::fs::write(out.join(file), bytes).map_err(|e| format!("run {number}: {file}: {e}"))
        });
        let ran_once = made.and_then(|()| {
            if run.stitch.is_empty() { run_once(run, number, cli, plugins, out, &mut ran) } else { stitch_once(run, number, cli, out) }
        });
        if let Err(failure) = ran_once {
            return vec![failure];
        }
    }
    check(&scenario.expect, out, &ran)
}

/// Makes the files `run` exports, into `out`, with the `vectorcraft-cli` at `cli` and the modules in
/// `plugins`, as a scenario's run is made and checked: the docs' VectorCraft shots draw their documents so.
pub(crate) fn draw(run: &Run, cli: &Path, plugins: &Path, out: &Path) -> Result<(), String> {
    std::fs::create_dir_all(out).map_err(|e| format!("{}: {e}", util::rel(out)))?;
    run_once(run, 1, cli, plugins, out, &mut Ran::default())
}

/// One run: `vectorcraft-cli` and what its results say, and its last step's error when it must fail. Its
/// exports' effects go to `ran`.
fn run_once(run: &Run, number: usize, cli: &Path, plugins: &Path, out: &Path, ran: &mut Ran) -> Result<(), String> {
    let args = arguments(run, plugins, out)?;
    let (results, failed) = launch(cli, &args, &out.join(format!("run-{number}")))?;
    let failing = run.steps.last().and_then(|step| Some((step.export.as_deref()?, step.fails.as_deref()?)));
    match (failed, failing) {
        (Some(failure), None) => return Err(format!("run {number}: {failure}")),
        (None, Some((file, code))) => return Err(format!("run {number}: exporting {file} doesn't fail with {code}")),
        (Some(failure), Some((file, code))) if !failure.contains(&format!("{file}: ")) || !failure.contains(code) => {
            return Err(format!("run {number}: exporting {file} doesn't fail with {code}: {failure}"));
        }
        _ => {}
    }
    // The export that fails gives no result.
    let steps = run.steps.get(..run.steps.len().saturating_sub(usize::from(failing.is_some()))).unwrap_or_default();
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
        let installed_as = installed.pointer("/result/kind").and_then(Value::as_str);
        if installed_as != Some(kind(module)) {
            return Err(format!("run {number}: {module} installed as {installed_as:?}, not as {}: {installed}", kind_name(module)));
        }
    }
    for step in steps {
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
                applied.push(Applied { ids, record: json!({ "id": effect_id(family.id()), "params": params }) });
            }
            Action::Export(file) => {
                let result = next(&format!("exporting {file}"))?;
                let warnings = result.pointer("/result/warnings").and_then(Value::as_array).into_iter().flatten();
                ran.warnings.insert(file.to_string(), warnings.map(|w| w.as_str().map_or_else(|| w.to_string(), str::to_string)).collect());
                ran.applied.insert(file.to_string(), applied.clone());
                if draws(file) {
                    for module in run.plugins.iter().filter(|module| live(module)) {
                        let info = next(&format!("{module}'s last error after {file}"))?;
                        match info.pointer("/result/lastError") {
                            Some(Value::Null) | None => {}
                            Some(error) => return Err(format!("run {number}: after {file}, {module}'s last error is {error}")),
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

/// Runs `cli` with `args`, its output in files named `stem` with `.out` and `.err`, and returns the JSON
/// results it printed, one a line, and when its exit status is a failure, that status with the end of what
/// it wrote to stderr. It fails when `cli` doesn't start or its time runs out.
fn launch(cli: &Path, args: &[String], stem: &Path) -> Result<(Vec<Value>, Option<String>), String> {
    let (out_path, err_path) = (stem.with_extension("out"), stem.with_extension("err"));
    let file = |path: &Path| std::fs::File::create(path).map_err(|e| format!("{}: {e}", util::rel(path)));
    let mut child = util::spawn(Command::new(cli).args(args).stdin(Stdio::null()).stdout(file(&out_path)?).stderr(file(&err_path)?))
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
    let failed = (!status.success()).then(|| {
        let said = std::fs::read_to_string(&err_path).unwrap_or_default();
        let tail: Vec<&str> = said.lines().rev().take(20).collect::<Vec<_>>().into_iter().rev().collect();
        format!("vectorcraft-cli {} after {} results: {}", status, results.len(), tail.join(" | "))
    });
    Ok((results, failed))
}

/// What fails of `expect`, with the files in `out` and what the runs left.
fn check(expect: &Expect, out: &Path, ran: &Ran) -> Vec<String> {
    let mut failures = Vec::new();
    for module in &expect.listed {
        let id = plugin_id(module);
        let listed = ran
            .listed
            .iter()
            .any(|p| p.get("id").and_then(Value::as_str) == Some(&id) && p.get("kind").and_then(Value::as_str) == Some(kind(module)));
        if !listed {
            failures.push(format!("plugin.list doesn't list {id} as {}: {}", kind_name(module), Value::Array(ran.listed.clone())));
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
    for [document, machine] in &expect.exported {
        let warned = ran.warnings.get(machine).map_or(&[][..], Vec::as_slice);
        if let Err(e) =
            read(document).and_then(|document| Ok((document, read(machine)?))).and_then(|(d, m)| written_as_exported(&d, machine, &m, warned))
        {
            failures.push(format!("{machine}: {e}"));
        }
    }
    for kept in &expect.kept {
        if let Err(e) = read(&kept.file).and_then(|bytes| keeps(&bytes, &kept.settings)) {
            failures.push(format!("{}: {e}", kept.file));
        }
    }
    for [a, b] in &expect.differ {
        match (read(a), read(b)) {
            (Ok(x), Ok(y)) if x == y => failures.push(format!("{a} and {b} are the same {} bytes", x.len())),
            (Ok(_), Ok(_)) => {}
            (Err(e), _) | (_, Err(e)) => failures.push(e),
        }
    }
    for expected in &expect.sheets {
        if let Err(e) = read(&expected.file).and_then(|bytes| holds_sheet(&bytes, &expected.sheet)) {
            failures.push(format!("{}: {e}", expected.file));
        }
    }
    for expected in &expect.traced {
        if let Err(e) = read(&expected.file).and_then(|bytes| trace::traced(&bytes, &expected.threads, expected.effect)) {
            failures.push(format!("{}: {e}", expected.file));
        }
    }
    failures
}

/// Whether the document `bytes` sews as the test sheet `id` the Test Sheet filter drew into it, or how it
/// doesn't: as the sheet's design moved by the whole millimetres that bring the middle of its bounds
/// nearest to the middle of the document's design, as the filter moves it to its artboard
/// (`testsheet::placed`). Both are sewn into PES files, as `stitch plan` sews them.
fn holds_sheet(bytes: &[u8], id: &str) -> Result<(), String> {
    let sheet = testsheet::sheets().find(|sheet| sheet.id == id).ok_or(format!("the Test Sheet filter doesn't draw {id}"))?;
    let read = stitchcraft_vectorcraft::read(bytes, &Budget::DEFAULT).map_err(|unread| unread.to_string())?;
    let moved = testsheet::placed(sheet, read.design.bounds().map(Rect::center)).map_err(|bug| bug.to_string())?;
    let outcome = stitchcraft_engine::plan(&moved, REFERENCE, &Budget::DEFAULT);
    let plan = outcome.plan.ok_or_else(|| format!("{id}'s design doesn't sew: {:?}", outcome.diagnostics))?;
    let drawn = stitchcraft_formats::encode(&plan, FormatId::PesV1, NAME).map_err(|e| e.to_string())?.bytes;
    let sewn = sewn(bytes)?;
    if sewn == drawn {
        Ok(())
    } else {
        Err(format!("it sews otherwise than {id}'s design moved: {} bytes of PES, where the design's are {}", sewn.len(), drawn.len()))
    }
}

/// Whether the document `bytes` keeps `settings`, the machine as `profile` and the design settings by their
/// names, as the VectorCraft adapter reads them, or how it doesn't.
fn keeps(bytes: &[u8], settings: &toml::Table) -> Result<(), String> {
    let read = stitchcraft_vectorcraft::read(bytes, &Budget::DEFAULT).map_err(|unread| unread.to_string())?;
    let mut wrong = Vec::new();
    for (name, value) in settings {
        let (kept, wanted) = if name == stitchcraft_vectorcraft::settings::PROFILE {
            (read.machine.map(|machine| json!(machine.id)), toml_to_json(value.clone()))
        } else {
            let setting = Setting::named(name).ok_or(format!("there is no design setting `{name}`"))?;
            (Some(json!(setting.get(&read.design.settings).get())), toml_to_json(value.clone()))
        };
        let same = match (&kept, &wanted) {
            (Some(Value::Number(a)), Value::Number(b)) => a.as_f64() == b.as_f64(),
            (Some(a), b) => a == b,
            (None, _) => false,
        };
        if !same {
            wrong.push(format!("{name} {}, not {wanted}", kept.map_or_else(|| "none".to_string(), |kept| kept.to_string())));
        }
    }
    if wrong.is_empty() { Ok(()) } else { Err(format!("it keeps {}", wrong.join(", "))) }
}

/// How the plug-in `module` is named in a failure: as a live effect, an exporter or a filter.
fn kind_name(module: &str) -> &'static str {
    match kind(module) {
        "effect" => "a live effect",
        "exporter" => "an exporter",
        _ => "a filter",
    }
}

/// The PES file `stitch plan` writes from the VectorCraft document `bytes`, for the machine it keeps or else
/// the reference machine, or the errors that stop it.
pub(crate) fn sewn(bytes: &[u8]) -> Result<Vec<u8>, String> {
    stitch_export(bytes, FormatId::PesV1, NAME).map(|(file, _)| file)
}

/// The file of `format`, named `name`, that `stitch export` writes from the VectorCraft document `bytes`
/// without `--profile`, for the machine it keeps or else the reference machine, and what it says; or the
/// errors that stop it.
fn stitch_export(bytes: &[u8], format: FormatId, name: &str) -> Result<(Vec<u8>, Vec<Diagnostic>), String> {
    let profile = machine_for(None, Source::VectorCraft, bytes);
    let sewn = sew(Source::VectorCraft, bytes, Reading::default(), profile, format, name);
    let errors: Vec<String> = sewn.diagnostics.iter().filter(|d| d.severity() == Severity::Error).map(ToString::to_string).collect();
    match sewn.file {
        Some(file) if errors.is_empty() => Ok((file, sewn.diagnostics)),
        _ => Err(format!("StitchCraft doesn't sew it: {}", errors.join("; "))),
    }
}

/// Whether `bytes`, the machine file `machine` that StitchCraft's exporter wrote in VectorCraft, and the
/// export's `warnings`, are what `stitch export` writes of the `document` saved with it and what it says:
/// the file of the format its extension names, with its name as the design name, for the machine the
/// document keeps or else the reference machine, as the exporter's dialog has them by default, and what was
/// said, as the exporter words it.
fn written_as_exported(document: &[u8], machine: &str, bytes: &[u8], warnings: &[String]) -> Result<(), String> {
    let format = machine_format(machine).ok_or("StitchCraft writes no format with its extension")?;
    let name = Path::new(machine).file_stem().and_then(|stem| stem.to_str()).unwrap_or_default();
    let (file, said) = stitch_export(document, format, name)?;
    let said = stitchcraft_vc_plugin::export::warnings(&said);
    if file != bytes {
        return Err(format!(
            "it isn't the file `stitch export` writes of its document: {} bytes, where `stitch export` writes {}",
            bytes.len(),
            file.len()
        ));
    }
    if said != warnings {
        return Err(format!("the export warned {warnings:?}, and `stitch export` says {said:?}"));
    }
    Ok(())
}

/// Whether the document `bytes` sews as it does with the StitchCraft records of the objects `applied`
/// reached put back as the steps sent them, or how it doesn't.
fn sews_as_applied(bytes: &[u8], applied: &[Applied]) -> Result<(), String> {
    let mut doc: Value =
        serde_json::from_slice(stitchcraft_vectorcraft::read_part(bytes)).map_err(|e| format!("not JSON StitchCraft can change: {e}"))?;
    let stored = restore(&mut doc, applied)?;
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
/// step applies it, and any StitchCraft record on one of the object's fills or strokes is taken away. An
/// object the document doesn't hold is an error: nothing could be put back, and the comparison would prove
/// nothing.
fn restore(doc: &mut Value, applied: &[Applied]) -> Result<Vec<Value>, String> {
    let mut sent: BTreeMap<u64, Vec<Value>> = BTreeMap::new();
    for a in applied {
        for id in &a.ids {
            sent.entry(*id).or_default().push(a.record.clone());
        }
    }
    let ours = |effect: &Value| effect.get("id").and_then(Value::as_str).and_then(effect_name).is_some();
    let mut stored = Vec::new();
    let mut found = Vec::new();
    visit(doc, &mut |node| {
        let Some((id, records)) = node.get("id").and_then(Value::as_u64).and_then(|id| Some((id, sent.get(&id)?))) else { return };
        found.push(id);
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
    let missing: Vec<String> = sent.keys().filter(|id| !found.contains(id)).map(u64::to_string).collect();
    if !missing.is_empty() {
        return Err(format!("it holds no object {}, which `effect.apply` said it applied effects to", missing.join(", ")));
    }
    Ok(stored)
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
    // Each setting as the element would hold it: the text the reader makes of an effect's value.
    let settings: Vec<(&String, Option<String>)> =
        expected.params.iter().flatten().map(|(key, value)| (key, param_text(&toml_to_json(value.clone())))).collect();
    let elements = read.design.elements();
    let count = elements
        .iter()
        .filter(|e| e.id.to_string().ends_with(&suffix))
        .filter(|e| expected.thread.as_ref().is_none_or(|t| t.eq_ignore_ascii_case(&e.thread.color.to_string())))
        .filter(|e| settings.iter().all(|(key, text)| text.is_some() && e.params.get(key) == text.as_deref()))
        .count();
    if count == expected.count {
        return Ok(());
    }
    let thread = expected.thread.as_deref().map_or_else(String::new, |t| format!(" in {t}"));
    let with: Vec<String> = settings.iter().map(|(key, text)| format!("{key} = {}", text.as_deref().unwrap_or("?"))).collect();
    let with = if with.is_empty() { String::new() } else { format!(" with {}", with.join(", ")) };
    let found: Vec<String> = elements
        .iter()
        .map(|e| {
            let set: Vec<String> = settings.iter().filter_map(|(key, _)| Some(format!("{key} = {}", e.params.get(key)?))).collect();
            let set = if set.is_empty() { String::new() } else { format!(" ({})", set.join(", ")) };
            format!("{} {}{set}", e.id, e.thread.color)
        })
        .collect();
    Err(format!("{} `{}` elements{thread}{with} are expected, and it holds {count}: {}", expected.count, expected.element, found.join(", ")))
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

mod trace;

#[cfg(test)]
mod tests;
