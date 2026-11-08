//! Conformance level L3, routing (`docs/src/design/conformance.md` › L3, `REQ-ROUTE-008`, `REQ-ROUTE-013`):
//! elements of a test sheet routed by a pinned Ink/Stitch's Auto-Route Satin Columns or Auto-Route Running
//! Stitch, run as a black box (ADR-0001, decision 6), and by `stitchcraft_engine::route`, and the 2 routes
//! sewn and compared by the differential's measures ([`differential::judged`]).
//!
//! **Why.** A route is a design of its own: where the columns are cut, which stretches run under the satin,
//! where the jumps go and which are trimmed. Sewing it in both programs, as the other cases sew a design,
//! would compare 2 sewings of StitchCraft's route. So each program routes the elements itself, with the same
//! options, and sews its own route.
//!
//! **A case** is an `inkstitch` case of a sheet with a `[route]` table: `elements`, the start of the ids of
//! the sheet's elements to route, the `tool`, `satins` for Auto-Route Satin Columns, the tool when left out,
//! or `runs` for Auto-Route Running Stitch, and the tool's options by Ink/Stitch's names, each at its default
//! when left out (`stitchcraft_engine::route::Tool::options`):
//!
//! ```toml
//! [route]
//! elements = "ts17c:lines-drawn:"
//! tool = "runs"
//! trim = true
//! ```
//!
//! **The files**, in [`WRITTEN`]: the elements as the SVG file Ink/Stitch routes, `route-<case>.svg`, each
//! painted by a `style` attribute, as Inkscape paints a path, since Auto-Route Running Stitch gives the
//! elements it makes the style of their strokes and nothing else of their paint; the file Ink/Stitch's tool
//! writes, `route-<case>.routed.svg`, and its PES file; and StitchCraft's route as an SVG file Ink/Stitch
//! would read, `route-<case>.stitchcraft.svg`, for a reviewer to set beside it.

use std::collections::BTreeMap;
use std::path::Path;

use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Design, Element};
use stitchcraft_engine::route::{Options, Route, Tool, route};
use stitchcraft_engine::testsheets;
use stitchcraft_plan::profiles;
use stitchcraft_testkit::inkstitch;

use super::differential::{self, Inkstitch, WRITTEN};
use super::runner::Outcome;
use crate::util;

/// What a case routes, and how.
#[derive(Clone, Debug, PartialEq)]
pub struct Routing {
    /// The start of the ids of the elements routed.
    pub elements: String,
    /// The tool that routes them.
    pub tool: Tool,
    /// The tool's options.
    pub options: Options,
}

/// The tools a case can route with, by the names its `tool` gives them, and the extension of Ink/Stitch's
/// that routes as each does.
const TOOLS: [(&str, Tool, &str); 2] = [("satins", Tool::Satins, "auto_satin"), ("runs", Tool::Runs, "auto_run")];

/// The `[route]` table of a case, read; or why it can't be: no `elements`, a tool that is none, a key that
/// is no option of the tool, or an option that is no `true` or `false`.
pub fn read(table: &toml::Table) -> Result<Routing, String> {
    let elements = table.get("elements").and_then(toml::Value::as_str).ok_or("`[route]` needs `elements`, the start of the ids to route")?;
    let tool = match table.get("tool") {
        None => Tool::Satins,
        Some(value) => {
            let names: Vec<&str> = TOOLS.iter().map(|(name, ..)| *name).collect();
            let named = value.as_str().and_then(|given| TOOLS.iter().find(|(name, ..)| *name == given));
            named.map(|(_, tool, _)| *tool).ok_or_else(|| format!("`tool` is one of {}", names.join(", ")))?
        }
    };
    let mut options = tool.defaults();
    for (key, value) in table.iter().filter(|(key, _)| !matches!(key.as_str(), "elements" | "tool")) {
        let names: Vec<&str> = tool.options().iter().map(|option| option.name).collect();
        let option = tool.option(key).ok_or_else(|| format!("`{key}` is no option of {} ({})", tool.name(), names.join(", ")))?;
        option.set(&mut options, value.as_bool().ok_or_else(|| format!("`{key}` is `true` or `false`"))?);
    }
    Ok(Routing { elements: elements.to_string(), tool, options })
}

/// Runs the routing case `case` of the sheet `sheet`, as [`differential::run`] runs a case, with the measures
/// `excused` excuses by a deviation's id.
pub fn run(root: &Path, case: &str, sheet: &str, routing: &Routing, excused: &BTreeMap<String, String>, outcome: &mut Outcome) {
    let Some(inkstitch) = Inkstitch::ready(root, outcome) else { return };
    match routed(root, &inkstitch, case, sheet, routing) {
        Ok((theirs, ours)) => differential::judged(theirs, ours, excused, outcome),
        Err(e) => outcome.failures.push(e),
    }
}

/// Ink/Stitch's PES file of its route of the elements `routing` names of `sheet`, and StitchCraft's of its
/// own, with the files of the module's docs written; or why there is nothing to route.
type Sewn = (Result<Vec<u8>, String>, Result<Vec<u8>, String>);

/// See [`Sewn`].
fn routed(root: &Path, inkstitch: &Inkstitch, case: &str, sheet: &str, routing: &Routing) -> Result<Sewn, String> {
    let found = testsheets::find(sheet).ok_or_else(|| format!("there is no test sheet `{sheet}`"))?;
    let design = found.design().ok_or_else(|| format!("{sheet} is drawn stitch by stitch"))?.map_err(|e| format!("{sheet}: {e}"))?;
    let elements: Vec<Element> = design.elements().iter().filter(|element| element.id.as_str().starts_with(&routing.elements)).cloned().collect();
    if elements.is_empty() {
        return Err(format!("{sheet} has no element whose id starts with `{}`", routing.elements));
    }
    let name = |what: &str| root.join(WRITTEN).join(format!("route-{case}{what}"));
    let given = name(".svg");
    write(&given, &inkstitch::svg_styled(&elements, &design.settings)?)?;
    let extension = TOOLS.iter().find(|(_, tool, _)| *tool == routing.tool).map_or("auto_satin", |(.., extension)| *extension);
    let mut arguments = vec![format!("--extension={extension}")];
    arguments.extend(routing.tool.options().iter().map(|option| format!("--{}={}", option.name, option.get(&routing.options))));
    arguments.extend((0..elements.len()).map(|index| format!("--id=e{index}")));
    let arguments: Vec<&str> = arguments.iter().map(String::as_str).collect();
    let what = format!("SVG file from {}", routing.tool.name());
    let theirs = inkstitch.output(root, &arguments, &given, &what).and_then(|svg| {
        let routed = name(".routed.svg");
        write_bytes(&routed, &svg)?;
        inkstitch.pes(root, &routed).and_then(|bytes| differential::kept(&routed, bytes))
    });
    let how = Route { tool: routing.tool, options: routing.options, start: None, end: None };
    let ours = route(&elements, &how, &design.settings, &Budget::DEFAULT);
    if let Some(error) = ours.diagnostics.iter().find(|said| said.code.severity() == stitchcraft_core::Severity::Error) {
        return Ok((theirs, Err(format!("StitchCraft routed nothing: {error}"))));
    }
    let pieces: Vec<Element> = ours.pieces.into_iter().map(|piece| piece.element).collect();
    write(&name(".stitchcraft.svg"), &inkstitch::svg(&pieces, &design.settings)?)?;
    let planned = Design::new(pieces, design.settings).map_err(|e| e.to_string()).and_then(|routed| {
        let planned = stitchcraft_engine::plan(&routed, profiles::REFERENCE, &Budget::DEFAULT);
        planned.plan.ok_or_else(|| format!("StitchCraft planned nothing: {:?}", planned.diagnostics))
    });
    Ok((theirs, planned.and_then(|plan| differential::encoded(&plan, case))))
}

/// `text` written to `path`, its folder made first.
fn write(path: &Path, text: &str) -> Result<(), String> {
    write_bytes(path, text.as_bytes())
}

/// `bytes` written to `path`, its folder made first.
fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let written = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(path, bytes));
    written.map_err(|e| format!("{}: {e}", util::rel(path)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_route_table_names_the_elements_and_sets_the_options_it_gives() {
        let table: toml::Table = toml::from_str("elements = \"ts17c:drawn:\"\npreserve_order = true\ntrim = false").unwrap();
        let routing = read(&table).unwrap();
        assert_eq!((routing.elements.as_str(), routing.tool), ("ts17c:drawn:", Tool::Satins));
        assert_eq!(routing.options, Options { trim: false, preserve_order: true, keep_originals: false, break_up: false });
        // Left out, an option is at its default, the tool's.
        let defaults = read(&toml::from_str("elements = \"a\"").unwrap()).unwrap();
        assert_eq!(defaults.options, Tool::Satins.defaults());
        let runs = read(&toml::from_str("elements = \"a\"\ntool = \"runs\"\ntrim = true").unwrap()).unwrap();
        assert_eq!((runs.tool, runs.options), (Tool::Runs, Options { trim: true, ..Tool::Runs.defaults() }));
        for (bad, why) in [
            ("trim = true", "needs `elements`"),
            ("elements = \"a\"\nsort = true", "`sort` is no option of Auto-Route Satin Columns"),
            ("elements = \"a\"\ntool = \"runs\"\nkeep_originals = true", "`keep_originals` is no option of Auto-Route Running Stitch"),
            ("elements = \"a\"\ntool = \"fills\"", "`tool` is one of satins, runs"),
            ("elements = \"a\"\ntrim = 1", "is `true` or `false`"),
        ] {
            let error = read(&toml::from_str(bad).unwrap()).unwrap_err();
            assert!(error.contains(why), "{bad}: {error}");
        }
    }
}
