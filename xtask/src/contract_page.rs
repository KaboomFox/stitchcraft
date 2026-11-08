//! Generates the Ink/Stitch compatibility contract page from `conformance/inkstitch-params.toml`, the
//! parameter registry and the SVG adapter, and checks that they agree.
//!
//! The data file holds facts only (names, types, units, defaults, applicability, phases); Ink/Stitch's
//! descriptions are GPL text and are never copied. The registry adds the StitchCraft column. A parameter
//! the registry declares with an Ink/Stitch origin must exist in the data file with a compatible kind and
//! the default Ink/Stitch sews, because defaults are part of the contract (`docs/src/design/params.md` ›
//! Naming rules); one with StitchCraft's own origin must not; the lock and method identifiers must match.
//! The SVG adapter's commands and clone settings must be Ink/Stitch's, and the attributes it reads must be
//! listed; the file versions' changes and renamed commands are checked by [`crate::contract_versions`]. A
//! clone's settings belong to no stitch type, so their StitchCraft column says what the adapter does with
//! each (`stitchcraft_svg::inkstitch::CLONE_SETTINGS`), as the commands' does.
//!
//! The default Ink/Stitch sews for a file that sets none is a row's `fallback` where it has one, and its
//! `default` otherwise. The default is what Ink/Stitch's settings window shows, and its code reads a few
//! missing attributes as another value (`conformance/inkstitch/check_params.py` finds them).
//!
//! Ink/Stitch gives some keys to several elements, with different defaults (`max_stitch_length_mm` has none
//! on satins and is 4 mm on fills), so a declaration is matched with the rows for the stitch types it
//! applies to, never by its key alone.
//!
//! The check also runs the other way ([`Contract::check_sewn`]). A row that the registry declares for any of
//! its stitch types must apply to every stitch type of the row that the engine sews. A declaration that
//! leaves one out ignores a setting that Ink/Stitch sews with, and the other checks don't see it. M10.6's
//! tartan fill was first written without `max_stitch_length_mm` so. Where Ink/Stitch's sewing of a type
//! doesn't read a setting that its settings window shows, or a deviation leaves the setting out, an
//! `[[unapplied]]` entry of the data file gives the reason, and the page lists it.

use std::collections::BTreeSet;

use serde::Deserialize;
use stitchcraft_engine::generate::sewn_as;
use stitchcraft_params::{Family, Kind, Origin, ParamGroup, ParamSpec, StitchType, declarations, find_for};
use stitchcraft_svg::inkstitch::{CLONE_SETTINGS, COMMANDS, READ_ATTRIBUTES};

use crate::contract_versions::{self, Renamed, Update};
use crate::param_pages;

/// Where the page is written, relative to the repository root.
pub const PAGE: &str = "docs/src/design/inkstitch-compat-contract.md";
/// The data file, relative to the repository root.
pub const DATA: &str = "conformance/inkstitch-params.toml";

/// The data file's contents.
#[derive(Deserialize)]
pub struct Contract {
    #[serde(default)]
    lock_ids: Vec<String>,
    #[serde(default)]
    meander_tiles: Vec<String>,
    param: Vec<Param>,
    #[serde(default)]
    method: Vec<Method>,
    #[serde(default)]
    command: Vec<CommandRow>,
    #[serde(default)]
    attribute: Vec<Attribute>,
    #[serde(default)]
    renamed_command: Vec<Renamed>,
    #[serde(default)]
    update: Vec<Update>,
    #[serde(default)]
    unapplied: Vec<Unapplied>,
}

/// Settings that Ink/Stitch gives a stitch type StitchCraft sews, and that the registry leaves out of it on
/// purpose, with the reason ([`Contract::check_sewn`]).
#[derive(Deserialize)]
struct Unapplied {
    /// The element of the settings' rows: `common`, `stroke`, `satin` or `fill`.
    element: String,
    /// The stitch type's method id.
    stitch_type: String,
    /// The settings' keys.
    names: Vec<String>,
    /// Why, in words a reviewer can check: where Ink/Stitch's sewing of the type leaves the settings unread,
    /// or the deviation (`DEV-…`) that leaves them out.
    reason: String,
}

/// One of Ink/Stitch's commands, and where it applies: `object`, `layer` or `document`.
#[derive(Deserialize)]
struct CommandRow {
    name: String,
    tied_to: String,
}

/// An `inkstitch:*` attribute that declares no parameter, its kind (`element`, `pattern`,
/// `stitch_plan`, `sew_stack` or `legacy`) and when StitchCraft plans to read it.
#[derive(Deserialize)]
struct Attribute {
    name: String,
    kind: String,
    phase: String,
    milestone: String,
}

#[derive(Deserialize)]
struct Param {
    element: String,
    name: String,
    #[serde(rename = "type")]
    kind: String,
    unit: String,
    /// The value Ink/Stitch's settings window shows.
    default: String,
    /// The value Ink/Stitch sews for a file that sets none, where it differs from `default`.
    #[serde(default)]
    fallback: Option<String>,
    applies_to: Vec<String>,
    /// Stitch types whose sewing reads the parameter though the settings window doesn't show it for them,
    /// as a contour fill's underlay reads the tatami fill's angle: they are given the key all the same.
    #[serde(default)]
    read_by: Vec<String>,
    phase: String,
    milestone: String,
}

impl Param {
    /// The value Ink/Stitch sews for a file that sets none.
    fn sewn(&self) -> &str {
        self.fallback.as_deref().unwrap_or(&self.default)
    }

    /// The family of stitch types of the row's element: none for the common settings and clones.
    fn family(&self) -> Option<Family> {
        match self.element.as_str() {
            "stroke" => Some(Family::Stroke),
            "satin" => Some(Family::Satin),
            "fill" => Some(Family::Fill),
            _ => None,
        }
    }

    /// The stitch types the row covers: the ones it lists, and those that read it without showing it, or
    /// every type of its element when it lists none (all of them for the common settings, none for clones).
    /// Some rows list another parameter's values instead, such as `custom` for the custom lock shapes;
    /// those do not narrow the stitch types. Only an id of the row's own family names a stitch type,
    /// because ids are unique only within a family: `zigzag` is a lock shape as well as a satin method.
    fn scope(&self) -> BTreeSet<StitchType> {
        let listed: BTreeSet<StitchType> = self.applies_to.iter().filter_map(|a| self.stitch_type(a)).collect();
        if !listed.is_empty() {
            return listed.into_iter().chain(self.read_by.iter().filter_map(|a| self.stitch_type(a))).collect();
        }
        StitchType::ALL.iter().copied().filter(|t| self.element == "common" || self.family() == Some(t.family())).collect()
    }

    /// The stitch type an `applies_to` entry names: an id of the row's own family, or none.
    fn stitch_type(&self, value: &str) -> Option<StitchType> {
        self.family().and_then(|family| StitchType::from_id(family, value))
    }

    /// Whether the row covers any of `types`.
    fn covers(&self, types: &[StitchType]) -> bool {
        let scope = self.scope();
        types.iter().any(|t| scope.contains(t))
    }
}

#[derive(Deserialize)]
struct Method {
    param: String,
    value: String,
    phase: String,
    milestone: String,
}

const SECTIONS: &[(&str, &str)] = &[
    ("common", "All stitch types (ties, locks, trims, stops)"),
    ("stroke", "Strokes: running, ripple, zigzag, manual"),
    ("satin", "Satin columns"),
    ("fill", "Fills"),
    ("clone", "Clones (`<use>`)"),
];

const INTRO: &str = "\
<!-- GENERATED by `cargo xtask docs` from conformance/inkstitch-params.toml. Do not edit by hand. -->

# Ink/Stitch compatibility contract

The parameters an Ink/Stitch SVG can carry, as `inkstitch:<name>` attributes, its other attributes and
its commands, and when StitchCraft supports each one. This is the interoperability contract: StitchCraft's
registry uses these names as its keys so files move between the tools unchanged
([ADR-0001](adr/0001-license-and-clean-room.md)).

- **Checked against:** Ink/Stitch `main` at `d59c9ab` (2026-09-17). The last line counts the declarations.
- **Facts only:** names, types, units, defaults and applicability. Ink/Stitch's descriptions are GPL text
  and are not copied; StitchCraft's own help text lives in the [parameter registry](params.md).
- **Machine-readable source:** `conformance/inkstitch-params.toml`. `cargo xtask docs` generates this page
  from it and from the parameter registry, and `cargo xtask docs --check` fails if they disagree. A
  parameter declared with an Ink/Stitch origin must be listed here with a compatible type, and its default
  must be the value Ink/Stitch sews. In the other direction, a registered row's parameter must apply to
  each stitch type of the row that StitchCraft sews, unless the section
  [Settings a stitch type does not take](#settings-a-stitch-type-does-not-take) gives the reason. A
  stitch type sewn as another takes that one's settings, as a legacy fill takes a tatami fill's.
- **Default** is the value Ink/Stitch's settings window shows. For a few parameters its code sews another
  value when a file sets none, and the cell gives that value after the default, as in `0.1 (0.2 when
  unset)`. `—` means no default: the value is derived from another parameter or the shape.
- **StitchCraft** says whether the [parameter registry](../user/reference/params.md) declares the
  parameter. *planned*: not yet; a design that sets it keeps it unchanged and gets `SC-W0105`. *not
  planned*: never, and a design that sets it gets the same.
  *registered*: StitchCraft reads and checks it under the same key, meaning and default; it changes the
  stitches once its stitch type or feature lands (see *Phase* and the parameter's reference entry).
  *deviates*: registered, with a meaning that differs as its reference entry describes. A clone's settings
  say what the SVG adapter does with them.
- **Applies to** lists the stitch types Ink/Stitch's settings window shows the parameter for. *Read by*
  adds those whose sewing reads it without showing it, as a contour fill's underlay reads the tatami
  fill's `angle`, and StitchCraft's registry gives it those too.
- **Phase** is when the parameter's stitch type lands ([roadmap](../plan/roadmap.md)); `—` when it is not
  planned.
";

/// The section of the settings Ink/Stitch gives a stitch type StitchCraft sews, which the registry leaves out
/// of it, before its rows.
const UNAPPLIED_INTRO: &str = "
## Settings a stitch type does not take

Ink/Stitch gives each stitch type here the settings beside it, and StitchCraft's registry doesn't, for the
reason given. They are the `[[unapplied]]` entries of `conformance/inkstitch-params.toml`. Every other
stitch type that StitchCraft sews takes the registered settings that its rows give it.

| Element | Stitch type | Settings | Reason |
|---|---|---|---|
";

impl Contract {
    /// Reads the data file's text.
    pub fn parse(data: &str) -> Result<Contract, String> {
        toml::from_str(data).map_err(|e| format!("{DATA}: {e}"))
    }

    /// Renders the page, with the StitchCraft column from `registry`.
    pub fn render(&self, registry: &[&ParamGroup]) -> String {
        let mut md = String::from(INTRO);
        for (key, title) in SECTIONS {
            md.push_str(&format!(
                "\n## {title}\n\n| Attribute | Type | Unit | Default | Applies to | Phase | StitchCraft |\n|---|---|---|---|---|---|---|\n"
            ));
            for p in self.param.iter().filter(|p| p.element == *key) {
                let listed = |values: &[String]| values.iter().map(|a| format!("`{a}`")).collect::<Vec<_>>().join(", ");
                let clone = p.element == "clone";
                let applies = match (clone, p.applies_to.is_empty(), p.read_by.is_empty()) {
                    (true, _, _) => "clones".to_string(),
                    (false, true, _) => "all".to_string(),
                    (false, false, true) => listed(&p.applies_to),
                    (false, false, false) => format!("{}; read by {}", listed(&p.applies_to), listed(&p.read_by)),
                };
                let ours = if clone {
                    CLONE_SETTINGS.iter().find(|ours| ours.name == p.name).map_or("planned", |ours| ours.does).to_string()
                } else {
                    status(registry, p)
                };
                let default = match &p.fallback {
                    Some(fallback) => format!("{} ({fallback} when unset)", p.default),
                    None => p.default.clone(),
                };
                md.push_str(&format!(
                    "| `{}` | {} | {} | {default} | {applies} | {} | {} |\n",
                    p.name,
                    p.kind,
                    p.unit.replace('|', "\\|"),
                    phase(&p.phase, &p.milestone),
                    ours
                ));
            }
        }
        md.push_str(UNAPPLIED_INTRO);
        for u in &self.unapplied {
            let names = u.names.iter().map(|name| format!("`{name}`")).collect::<Vec<_>>().join(", ");
            md.push_str(&format!("| {} | `{}` | {names} | {} |\n", u.element, u.stitch_type, u.reason.replace('|', "\\|")));
        }
        md.push_str(
            "\n## Method identifiers\n\nThe values the method parameters take (`stroke_method`, `satin_method`, `fill_method`), with phases. A\n\
             value of phase `—` is not planned as a method of its own, and the deviations ledger\n\
             (`conformance/deviations.toml`) says how an element set to it is sewn.\n\n",
        );
        md.push_str("| Parameter | Value | Phase |\n|---|---|---|\n");
        for m in &self.method {
            md.push_str(&format!("| `{}` | `{}` | {} |\n", m.param, m.value, phase(&m.phase, &m.milestone)));
        }
        let locks = self.lock_ids.iter().map(|l| format!("`{l}`")).collect::<Vec<_>>().join(", ");
        md.push_str(&format!(
            "\nLock stitch identifiers (`lock_start`, `lock_end`): {locks}. StitchCraft accepts every identifier; the shapes are its own\n\
             designs with the same intent, a deviation recorded in the deviations ledger (`conformance/deviations.toml`).\n"
        ));
        let tiles = self.meander_tiles.iter().map(|t| format!("`{t}`")).collect::<Vec<_>>().join(", ");
        md.push_str(&format!(
            "\nMeander tiles (`meander_pattern`), in the order Ink/Stitch's settings window lists them: {tiles}. StitchCraft accepts\n\
             every one and sews its own default tile for it, with a warning (`DEV-FILL-018`).\n"
        ));
        md.push_str(
            "\n## Commands\n\nInk/Stitch's command symbols (a `<use>` of the symbol `inkstitch_<name>`), where each applies, and what\n\
             StitchCraft's SVG adapter does with it (`stitchcraft_svg::inkstitch`). Their connectors are never stitched.\n\n\
             | Command | Applies to | StitchCraft |\n|---|---|---|\n",
        );
        for c in &self.command {
            let does = COMMANDS.iter().find(|ours| ours.name == c.name).map_or("planned", |ours| ours.does.describe());
            let applies = match c.tied_to.as_str() {
                "object" => "the object a connector ties it to",
                "layer" => "every layer it is in",
                _ => "the document",
            };
            md.push_str(&format!("| `{}` | {applies} | {does} |\n", c.name));
        }
        md.push_str(&contract_versions::render_renamed(&self.renamed_command));
        md.push_str(
            "\n## Other attributes\n\nThe `inkstitch:*` attributes that declare no parameter: settings of an element, of Ink/Stitch's\n\
             patterns, lettering, tools, stitch plan or sew stack, and legacy names Ink/Stitch's updater rewrites in older files.\n\
             The SVG adapter reads those marked read itself. It keeps any other of a shape as a parameter of the shape's\n\
             elements, which the registry names (`SC-W0105`).\n\n\
             | Attribute | Kind | Phase | StitchCraft |\n|---|---|---|---|\n",
        );
        for a in &self.attribute {
            let ours = match (READ_ATTRIBUTES.contains(&a.name.as_str()), a.milestone.as_str()) {
                (true, _) => "read",
                (false, NOT_PLANNED) => "not planned",
                (false, _) => "planned",
            };
            md.push_str(&format!("| `{}` | {} | {} | {ours} |\n", a.name, a.kind.replace('_', " "), phase(&a.phase, &a.milestone)));
        }
        md.push_str(&contract_versions::render_updates(&self.update));
        let registered = self.param.iter().filter(|p| declaration(registry, p).is_some()).count();
        md.push_str(&format!("\n_{} parameter declarations, {registered} registered in StitchCraft._\n", self.param.len()));
        md
    }

    /// Where `registry` and the data file disagree, one line each.
    pub fn check(&self, registry: &[&ParamGroup]) -> Vec<String> {
        let mut problems = Vec::new();
        for spec in registry.iter().flat_map(|g| g.specs) {
            let key = spec.key;
            // Whatever its stitch types: an SVG node's fill and stroke read the same settings, so one of
            // StitchCraft's own would read a value written for Ink/Stitch's meaning. A declaration is
            // compared with the rows for its stitch types (`REQ-PRM-005`).
            let named = self.param.iter().any(|p| p.name == key);
            match (spec.origin, named) {
                (Origin::StitchCraft, false) => {}
                (Origin::StitchCraft, true) => {
                    problems.push(format!("`{key}` is an Ink/Stitch parameter ({DATA}): declare it with an Ink/Stitch origin"));
                }
                (_, false) => problems.push(format!("`{key}` is declared with an Ink/Stitch origin, but {DATA} has no such parameter")),
                (origin, true) => {
                    let rows: Vec<&Param> = self.param.iter().filter(|p| p.name == key && p.covers(spec.applies_to)).collect();
                    let uncovered: Vec<String> =
                        spec.applies_to.iter().filter(|t| !rows.iter().any(|p| p.covers(&[**t]))).map(|t| format!("`{}`", t.id())).collect();
                    if !uncovered.is_empty() {
                        problems.push(format!("`{key}` applies to {} here, but Ink/Stitch does not give it that stitch type", uncovered.join(", ")));
                    }
                    for p in rows {
                        for problem in disagreements(spec, origin, p) {
                            if !problems.contains(&problem) {
                                problems.push(problem);
                            }
                        }
                    }
                }
            }
        }
        // The meander tiles are kept as options, so that a design naming one still reads.
        for spec in declarations(registry, "meander_pattern") {
            if let Kind::Choice { options } = spec.kind {
                let kept: Vec<&str> = options.iter().filter(|o| !o.offered).map(|o| o.id).collect();
                if kept != self.meander_tiles {
                    problems.push(format!("`meander_pattern` keeps {kept:?}, but the meander tiles in {DATA} are {:?}", self.meander_tiles));
                }
            }
        }
        for spec in ["lock_start", "lock_end"].into_iter().flat_map(|key| declarations(registry, key)) {
            let key = spec.key;
            if let Kind::Choice { options } = spec.kind {
                let ids: Vec<&str> = options.iter().map(|o| o.id).collect();
                if ids != self.lock_ids {
                    problems.push(format!("`{key}` offers {ids:?}, but the lock identifiers in {DATA} are {:?}", self.lock_ids));
                }
            }
        }
        for family in [Family::Stroke, Family::Satin, Family::Fill] {
            let ours: BTreeSet<&str> = StitchType::ALL.iter().filter(|t| t.family() == family).map(|t| t.id()).collect();
            let listed: Vec<&str> = self.method.iter().filter(|m| m.param == family.method_param()).map(|m| m.value.as_str()).collect();
            let theirs: BTreeSet<&str> = listed.iter().copied().collect();
            if ours != theirs {
                problems.push(format!("`{}`: StitchType has {ours:?}, but {DATA} lists {theirs:?}", family.method_param()));
            }
            // A registered method parameter offers them in Ink/Stitch's order, which its defaults count on.
            if let Some(Kind::Choice { options }) = find_for(registry, family.method_param(), family).map(|spec| spec.kind) {
                let offered: Vec<&str> = options.iter().map(|o| o.id).collect();
                if offered != listed {
                    problems.push(format!("`{}` offers {offered:?}, but Ink/Stitch's order is {listed:?}", family.method_param()));
                }
            }
        }
        let ours: BTreeSet<&str> = COMMANDS.iter().map(|c| c.name).collect();
        let theirs: BTreeSet<&str> = self.command.iter().map(|c| c.name.as_str()).collect();
        if ours != theirs {
            problems.push(format!("the SVG adapter knows the commands {ours:?}, but {DATA} lists {theirs:?}"));
        }
        let ours: BTreeSet<&str> = CLONE_SETTINGS.iter().map(|setting| setting.name).collect();
        let theirs: BTreeSet<&str> = self.param.iter().filter(|p| p.element == "clone").map(|p| p.name.as_str()).collect();
        if ours != theirs {
            problems.push(format!("the SVG adapter applies the clone settings {ours:?}, but {DATA} lists {theirs:?}"));
        }
        for name in READ_ATTRIBUTES {
            if !self.attribute.iter().any(|a| a.name == *name) {
                problems.push(format!("the SVG adapter reads `{name}`, which {DATA} does not list as an attribute"));
            }
        }
        problems.extend(contract_versions::check(&self.renamed_command, &self.update));
        problems
    }

    /// Where Ink/Stitch gives a stitch type that the engine sews (`sewn`) a setting, and the registry leaves the
    /// type out, one line each. Only a row that the registry declares for some of its stitch types is
    /// compared: the page calls it registered. A stitch type sewn as another is read as that one
    /// ([`sewn_as`]): a legacy fill's settings are a tatami fill's. An `[[unapplied]]` entry excuses a setting
    /// for a stitch type with the reason, and a setting it lists that needs no excuse, or an entry without a
    /// reason, is a problem too, so the list can't go stale.
    pub fn check_sewn(&self, registry: &[&ParamGroup], sewn: &[StitchType]) -> Vec<String> {
        let mut problems = Vec::new();
        let mut excused = BTreeSet::new();
        for row in self.param.iter().filter(|row| declaration(registry, row).is_some()) {
            for t in row.scope().into_iter().filter(|t| sewn.contains(t)) {
                let read_as = sewn_as(t);
                if declarations(registry, &row.name).any(|spec| spec.applies_to.contains(&read_as)) {
                    continue;
                }
                let excuse = self.unapplied.iter().position(|u| u.element == row.element && u.stitch_type == t.id() && u.names.contains(&row.name));
                let Some(entry) = excuse else {
                    let read = if read_as == t { String::new() } else { format!(" as `{}`", read_as.id()) };
                    problems.push(format!(
                        "`{}` does not apply to `{}` here, but Ink/Stitch gives it `{}` (its {} row), which StitchCraft sews{read}: add the \
                         stitch type to the key's declaration, or give the reason in an `[[unapplied]]` entry",
                        row.name,
                        read_as.id(),
                        t.id(),
                        row.element
                    ));
                    continue;
                };
                excused.insert((entry, row.name.as_str()));
            }
        }
        for (entry, u) in self.unapplied.iter().enumerate() {
            for name in u.names.iter().filter(|name| !excused.contains(&(entry, name.as_str()))) {
                problems.push(format!(
                    "the `[[unapplied]]` entry for `{}` ({}) excuses `{name}`, which needs no excuse: take it out of the entry (the \
                     registry gives it the stitch type, StitchCraft does not sew the type, or Ink/Stitch does not give it the setting)",
                    u.stitch_type, u.element
                ));
            }
            if u.reason.trim().is_empty() {
                problems.push(format!("the `[[unapplied]]` entry for `{}` ({}) gives no reason", u.stitch_type, u.element));
            }
        }
        problems
    }
}

/// Where a declaration and the Ink/Stitch row it matches disagree on the kind or the default, on
/// applying to every stitch type, or on when a settings window shows it.
fn disagreements(spec: &ParamSpec, origin: Origin, p: &Param) -> Vec<String> {
    let key = spec.key;
    let mut problems = Vec::new();
    if !compatible(spec.kind, &p.kind, &p.unit) {
        problems.push(format!("`{key}` is {} here, but Ink/Stitch's is a {} in {}", spec.kind.describe(), p.kind, p.unit));
    }
    let their_default = if p.sewn() == "—" { "" } else { p.sewn() };
    // Ink/Stitch gives some combo boxes their default as a place in the options, which the registry lists
    // in Ink/Stitch's order (`check` makes sure of it for the method parameters). It reads a toggle that
    // declares no default, such as `satin_column`, as off when the attribute is missing.
    let their_default = match (spec.kind, p.kind.as_str(), their_default.parse::<usize>()) {
        (Kind::Choice { options }, "combo", Ok(place)) => options.get(place).map_or(their_default, |o| o.id),
        (Kind::Toggle, _, _) if their_default.is_empty() => "false",
        _ => their_default,
    };
    // Ink/Stitch declares some parameters once for each kind of element, each with its own default; the
    // registry gives such a family its own default.
    let our_default = p.family().map_or(spec.default, |family| spec.default_for(family));
    let same = match (spec.read(our_default), spec.read(their_default)) {
        (Ok((ours, _)), Ok((theirs, None))) => ours == theirs,
        _ => false,
    };
    if origin == Origin::InkStitch && !same {
        problems.push(format!(
            "`{key}` defaults to \"{our_default}\" here, and Ink/Stitch sews \"{their_default}\" for a file that sets none: defaults are part of the contract (declare a deviation if the difference is deliberate)"
        ));
    }
    if p.element == "common" && spec.applies_to != StitchType::ALL {
        problems.push(format!("`{key}` applies to every stitch type in Ink/Stitch, but not here"));
    }
    // Values in `applies_to` that are not stitch types are another parameter's values Ink/Stitch shows
    // this one for (a lock's size, say, only for the shapes it sizes); the registry must say the same.
    let theirs: BTreeSet<&str> = p.applies_to.iter().map(String::as_str).filter(|a| p.stitch_type(a).is_none()).collect();
    let ours: BTreeSet<&str> = spec.visible_when.map(|c| c.any_of.iter().copied().collect()).unwrap_or_default();
    if !theirs.is_empty() && theirs != ours {
        let here = if ours.is_empty() { "always".to_string() } else { format!("only for {ours:?}") };
        problems.push(format!("`{key}` is shown only for {theirs:?} in Ink/Stitch, but {here} here"));
    }
    problems
}

/// The registry's declaration of `row`'s key for the stitch types the row covers, with its group.
fn declaration<'r>(registry: &[&'r ParamGroup], row: &Param) -> Option<(&'r ParamGroup, &'static ParamSpec)> {
    registry.iter().find_map(|group| group.specs.iter().find(|s| s.key == row.name && row.covers(s.applies_to)).map(|spec| (*group, spec)))
}

/// The milestone of a row StitchCraft does not plan to support.
const NOT_PLANNED: &str = "—";

/// A row's Phase cell: its phase and milestone, or `—` when it is not planned.
fn phase(phase: &str, milestone: &str) -> String {
    if milestone == NOT_PLANNED { NOT_PLANNED.to_string() } else { format!("{phase} ({milestone})") }
}

/// The StitchCraft column: planned, not planned, or a link to the declaration's reference entry.
fn status(registry: &[&ParamGroup], row: &Param) -> String {
    let Some((group, spec)) = declaration(registry, row) else {
        return if row.milestone == NOT_PLANNED { "not planned" } else { "planned" }.to_string();
    };
    let word = if matches!(spec.origin, Origin::InkStitchDeviates { .. }) { "deviates" } else { "registered" };
    format!("[{word}](../user/reference/{})", param_pages::anchor(group, &row.name))
}

/// Whether a StitchCraft kind can hold the values of an Ink/Stitch type and unit.
fn compatible(kind: Kind, their_type: &str, unit: &str) -> bool {
    match kind {
        Kind::Length { .. } => their_type == "float" && matches!(unit, "mm" | "mm/cycle"),
        Kind::LengthList { .. } => matches!(their_type, "float" | "string" | "str") && unit.starts_with("mm"),
        // A cross stitch's cells and its grid's offset are 1 length or 2, across and down, which Ink/Stitch reads as
        // it reads a value for each side.
        Kind::LengthPair { .. } => their_type == "float" && matches!(unit, "mm (each side)" | "mm (x y)"),
        // A meander's scale is 1 percentage or 2, across and down, which Ink/Stitch's window shows as 1.
        Kind::PercentPair { .. } => their_type == "float" && matches!(unit, "% (each side)" | "%"),
        Kind::PercentList { .. } => their_type == "float" && unit == "%",
        Kind::Angle | Kind::BoundedAngle { .. } | Kind::AngleList { .. } => their_type == "float" && matches!(unit, "deg" | "degrees" | "°"),
        // Ink/Stitch reads a cross stitch's fill coverage as a whole number, and its fonts write it as 50.0 too,
        // which it sews as its default, 50: a percentage reads both.
        Kind::Percent { .. } => matches!(their_type, "float" | "int") && matches!(unit, "%" | "± %"),
        // Ink/Stitch reads some numbers as floats that its settings window shows as whole numbers, such as a
        // fill's staggers, whose fractions it documents.
        Kind::Number { .. } => matches!(their_type, "float" | "int") && unit == "—",
        Kind::Count { .. } => their_type == "int",
        Kind::CountList { .. } => matches!(their_type, "int" | "string" | "str"),
        Kind::Toggle => matches!(their_type, "boolean" | "toggle"),
        Kind::Choice { .. } => matches!(their_type, "combo" | "dropdown"),
        Kind::Seed => their_type == "random_seed",
        Kind::Text { .. } => matches!(their_type, "string" | "str"),
        // Ink/Stitch has no area setting: StitchCraft's smallest area is its own.
        Kind::Area { .. } => false,
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_params::{ChoiceOption, Condition, Empty, ParamSpec, Stability};

    use super::*;

    /// A data file with one fill parameter, one lock id and every method id (so only what a test
    /// changes disagrees).
    fn data(default: &str) -> String {
        let mut data = format!(
            "lock_ids = [\"half_stitch\"]\n[[param]]\nelement = \"fill\"\nname = \"row_spacing_mm\"\ntype = \"float\"\nunit = \"mm\"\n\
             default = \"{default}\"\napplies_to = [\"tatami_fill\"]\nphase = \"P1\"\nmilestone = \"M5\"\n"
        );
        for t in StitchType::ALL {
            let param = t.family().method_param();
            data.push_str(&format!("[[method]]\nparam = \"{param}\"\nvalue = \"{}\"\nphase = \"P1\"\nmilestone = \"M5\"\n", t.id()));
        }
        for c in COMMANDS {
            data.push_str(&format!("[[command]]\nname = \"{}\"\ntied_to = \"object\"\n", c.name));
        }
        for c in CLONE_SETTINGS {
            data.push_str(&format!(
                "[[param]]\nelement = \"clone\"\nname = \"{}\"\ntype = \"toggle\"\nunit = \"—\"\ndefault = \"true\"\napplies_to = []\n\
                 phase = \"P2\"\nmilestone = \"M8\"\n",
                c.name
            ));
        }
        for a in READ_ATTRIBUTES {
            data.push_str(&format!("[[attribute]]\nname = \"{a}\"\nkind = \"element\"\nphase = \"P1\"\nmilestone = \"M3\"\n"));
        }
        for (old, new) in stitchcraft_svg::inkstitch::RENAMED {
            data.push_str(&format!("[[renamed_command]]\nold = \"{old}\"\nnew = \"{new}\"\n"));
        }
        for (id, _) in stitchcraft_svg::version::APPLIED {
            data.push_str(&format!("[[update]]\nversion = 4\nid = \"{id}\"\nchange = \"x\"\nmilestone = \"M3\"\n"));
        }
        data
    }

    const ROW_SPACING: ParamSpec = ParamSpec {
        key: "row_spacing_mm",
        label: "Row spacing",
        help: " Distance between rows.\n",
        kind: Kind::Length { min: 0.1, max: 10.0, empty: Empty::Never },
        default: "0.25",
        family_defaults: &[],
        group: "Fill",
        applies_to: &[StitchType::TatamiFill],
        visible_when: None,
        stability: Stability::Stable,
        origin: Origin::InkStitch,
    };

    fn registry(specs: &'static [ParamSpec]) -> ParamGroup {
        ParamGroup { name: "TatamiFillParams", help: " Tatami.\n", applies_to: &[StitchType::TatamiFill], specs }
    }

    /// The problem reported for a default unlike the one Ink/Stitch sews.
    fn unlike(key: &str, ours: &str, theirs: &str) -> String {
        format!(
            "`{key}` defaults to \"{ours}\" here, and Ink/Stitch sews \"{theirs}\" for a file that sets none: defaults are part of the contract (declare a deviation if the difference is deliberate)"
        )
    }

    #[test]
    fn renders_sections_methods_counts_and_status() {
        let contract = Contract::parse(&data("0.25")).unwrap();
        let planned = contract.render(&[]);
        assert!(planned.contains("| `row_spacing_mm` | float | mm | 0.25 | `tatami_fill` | P1 (M5) | planned |"));
        assert!(planned.contains("| `fill_method` | `tatami_fill` | P1 (M5) |"));
        assert!(planned.contains("_4 parameter declarations, 0 registered in StitchCraft._"));
        // A clone's settings say what the SVG adapter does with them.
        let off = "| `clone` | toggle | — | true | clones | P2 (M8) | applied: off, the clone sews nothing, and is listed (`SC-I0805`) |";
        assert!(planned.contains(off), "{planned}");
        assert!(planned.contains("| `trim` | the object a connector ties it to | applied: turns on `trim_after` |"));
        assert!(planned.contains("| `ignore_object` | element | P1 (M3) | read |"));
        let group = registry(&[ROW_SPACING]);
        let registered = contract.render(&[&group]);
        assert!(registered.contains("| P1 (M5) | [registered](../user/reference/params/tatami-fill.md#row_spacing_mm) |"));
        const DEVIATES: &[ParamSpec] = &[ParamSpec { origin: Origin::InkStitchDeviates { deviation: "DEV-FILL-001" }, ..ROW_SPACING }];
        let group = registry(DEVIATES);
        assert!(contract.render(&[&group]).contains("| [deviates](../user/reference/params/tatami-fill.md#row_spacing_mm) |"));
    }

    /// A key Ink/Stitch uses on two elements with different defaults (satin's row first), and our
    /// declaration of it for running and ripple stitches.
    fn shared(default: &'static str) -> (String, ParamGroup) {
        let row = |element: &str, default: &str, applies_to: &str, milestone: &str| {
            format!(
                "[[param]]\nelement = \"{element}\"\nname = \"running_stitch_tolerance_mm\"\ntype = \"float\"\nunit = \"mm\"\n\
                 default = \"{default}\"\napplies_to = [{applies_to}]\nphase = \"P1\"\nmilestone = \"{milestone}\"\n"
            )
        };
        let rows = row("satin", "0.1", "", "M4") + &row("stroke", "0.2", "\"running_stitch\", \"ripple_stitch\"", "M3");
        let data = data("0.25").replacen("[[param]]", &format!("{rows}[[param]]"), 1);
        let spec = ParamSpec {
            key: "running_stitch_tolerance_mm",
            default,
            applies_to: &[StitchType::RunningStitch, StitchType::RippleStitch],
            ..ROW_SPACING
        };
        let specs: &'static [ParamSpec] = Box::leak(Box::new([spec]));
        (data, ParamGroup { name: "RunningParams", help: " Running.\n", applies_to: &[StitchType::RunningStitch], specs })
    }

    #[test]
    fn keys_shared_by_elements_match_by_stitch_type() {
        let (data, group) = shared("0.2");
        let contract = Contract::parse(&data).unwrap();
        let page = contract.render(&[&group]);
        assert!(page.contains("| `running_stitch_tolerance_mm` | float | mm | 0.1 | all | P1 (M4) | planned |"), "satin's is not ours");
        assert!(page.contains("| P1 (M3) | [registered](../user/reference/params/running.md#running_stitch_tolerance_mm) |"));
        assert!(page.contains("_6 parameter declarations, 1 registered in StitchCraft._"));
        assert_eq!(contract.check(&[&group]), Vec::<String>::new(), "compared with the stroke's default, not the satin's");
        let (data, group) = shared("0.1");
        assert_eq!(Contract::parse(&data).unwrap().check(&[&group]), [unlike("running_stitch_tolerance_mm", "0.1", "0.2")]);
    }

    #[test]
    fn a_row_s_fallback_is_the_default_it_is_compared_with_and_the_page_shows_both() {
        // Ink/Stitch's settings window shows 0.1 and its code sews 0.2 when the attribute is missing.
        let data = data("0.25").replacen("default = \"0.25\"", "default = \"0.1\"\nfallback = \"0.2\"", 1);
        let contract = Contract::parse(&data).unwrap();
        assert!(contract.render(&[]).contains("| `row_spacing_mm` | float | mm | 0.1 (0.2 when unset) | `tatami_fill` |"));
        const SEWN: &[ParamSpec] = &[ParamSpec { default: "0.2", ..ROW_SPACING }];
        const SHOWN: &[ParamSpec] = &[ParamSpec { default: "0.1", ..ROW_SPACING }];
        assert_eq!(contract.check(&[&registry(SEWN)]), Vec::<String>::new());
        assert_eq!(contract.check(&[&registry(SHOWN)]), [unlike("row_spacing_mm", "0.1", "0.2")]);
    }

    #[test]
    fn a_family_with_its_own_default_is_compared_with_its_element_s_row() {
        // Declared for satin columns too, with their own default: each row is compared with its family's.
        let (data, _) = shared("0.2");
        let both = |family_defaults: &'static [(Family, &'static str)]| {
            let spec = ParamSpec {
                key: "running_stitch_tolerance_mm",
                default: "0.2",
                family_defaults,
                applies_to: &[StitchType::RunningStitch, StitchType::RippleStitch, StitchType::SatinColumn],
                ..ROW_SPACING
            };
            let specs: &'static [ParamSpec] = Box::leak(Box::new([spec]));
            ParamGroup { name: "RunningParams", help: " Running.\n", applies_to: &[StitchType::RunningStitch], specs }
        };
        let contract = Contract::parse(&data).unwrap();
        assert_eq!(contract.check(&[&both(&[(Family::Satin, "0.1")])]), Vec::<String>::new());
        assert_eq!(contract.check(&[&both(&[])]), [unlike("running_stitch_tolerance_mm", "0.2", "0.1")]);
    }

    #[test]
    fn req_prm_005_each_family_s_declaration_is_compared_with_its_element_s_row() {
        // Ink/Stitch's zigzag spacing is a list for a zigzag stroke and a length for satin columns, and a
        // meander fill reads it as a length that is 0 by default. Each family declares the key once, with
        // its own kind.
        let row = |element: &str, kind: &str, unit: &str, default: &str, applies_to: &str| {
            format!(
                "[[param]]\nelement = \"{element}\"\nname = \"zigzag_spacing_mm\"\ntype = \"{kind}\"\nunit = \"{unit}\"\n\
                 default = \"{default}\"\napplies_to = [{applies_to}]\nphase = \"P2\"\nmilestone = \"M7\"\n"
            )
        };
        let rows = row("stroke", "string", "mm", "0.4", "\"zigzag_stitch\"")
            + &row("satin", "float", "mm/cycle", "0.4", "")
            + &row("fill", "float", "mm", "0", "\"meander_fill\"");
        let contract = Contract::parse(&data("0.25").replacen("[[param]]", &format!("{rows}[[param]]"), 1)).unwrap();
        let declared = |name: &'static str, kind, default, applies_to: &'static [StitchType], origin| {
            let spec = ParamSpec { key: "zigzag_spacing_mm", kind, default, applies_to, origin, ..ROW_SPACING };
            let specs: &'static [ParamSpec] = Box::leak(Box::new([spec]));
            ParamGroup { name, help: " Spacing.\n", applies_to, specs }
        };
        let length = Kind::Length { min: 0.0, max: 10.0, empty: Empty::Never };
        let list = Kind::LengthList { min: 0.01, max: 10.0 };
        let stroke = declared("ZigzagStrokeParams", list, "0.4", &[StitchType::ZigzagStitch], Origin::InkStitch);
        let satin = declared("SatinParams", length, "0.4", &[StitchType::SatinColumn], Origin::InkStitch);
        let fill = |default| declared("MeanderFillParams", length, default, &[StitchType::MeanderFill], Origin::InkStitch);
        assert_eq!(contract.check(&[&stroke, &satin, &fill("0")]), Vec::<String>::new());
        assert_eq!(contract.check(&[&stroke, &satin, &fill("0.4")]), [unlike("zigzag_spacing_mm", "0.4", "0")]);
        // Each row links to its family's declaration.
        let page = contract.render(&[&stroke, &satin, &fill("0")]);
        for row in [
            "| `zigzag_spacing_mm` | string | mm | 0.4 | `zigzag_stitch` | P2 (M7) | [registered](../user/reference/params/zigzag-stroke.md#zigzag_spacing_mm) |",
            "| `zigzag_spacing_mm` | float | mm/cycle | 0.4 | all | P2 (M7) | [registered](../user/reference/params/satin.md#zigzag_spacing_mm) |",
            "| `zigzag_spacing_mm` | float | mm | 0 | `meander_fill` | P2 (M7) | [registered](../user/reference/params/meander-fill.md#zigzag_spacing_mm) |",
        ] {
            assert!(page.contains(row), "{row}");
        }
        // A key of Ink/Stitch's is never StitchCraft's own, even for a family Ink/Stitch does not give it:
        // a node's fill and stroke read the same settings, and the fill would read a value written for
        // Ink/Stitch's zigzag stroke.
        let ours = declared("TatamiFillParams", length, "0.4", &[StitchType::TatamiFill], Origin::StitchCraft);
        assert_eq!(
            contract.check(&[&stroke, &satin, &ours]),
            ["`zigzag_spacing_mm` is an Ink/Stitch parameter (conformance/inkstitch-params.toml): declare it with an Ink/Stitch origin"]
        );
        // Declared as Ink/Stitch's for that family, it is a stitch type Ink/Stitch does not give the key.
        let theirs = declared("TatamiFillParams", length, "0.4", &[StitchType::TatamiFill], Origin::InkStitch);
        assert_eq!(
            contract.check(&[&stroke, &satin, &theirs]),
            ["`zigzag_spacing_mm` applies to `tatami_fill` here, but Ink/Stitch does not give it that stitch type"]
        );
    }

    #[test]
    fn a_declaration_for_stitch_types_ink_stitch_does_not_give_the_key_is_reported() {
        const WIDER: &[ParamSpec] = &[ParamSpec { applies_to: &[StitchType::TatamiFill, StitchType::ContourFill], ..ROW_SPACING }];
        let group = registry(WIDER);
        assert_eq!(
            Contract::parse(&data("0.25")).unwrap().check(&[&group]),
            ["`row_spacing_mm` applies to `contour_fill` here, but Ink/Stitch does not give it that stitch type"]
        );
    }

    #[test]
    fn a_stitch_type_that_reads_the_key_without_showing_it_is_given_the_key() {
        // Ink/Stitch's window shows the key for tatami fills only, and a contour fill's sewing reads it too.
        const WIDER: &[ParamSpec] = &[ParamSpec { applies_to: &[StitchType::TatamiFill, StitchType::ContourFill], ..ROW_SPACING }];
        let group = registry(WIDER);
        let data = data("0.25").replacen("applies_to = [\"tatami_fill\"]", "applies_to = [\"tatami_fill\"]\nread_by = [\"contour_fill\"]", 1);
        let contract = Contract::parse(&data).unwrap();
        assert_eq!(contract.check(&[&group]), Vec::<String>::new());
        assert!(contract.render(&[&group]).contains("| `tatami_fill`; read by `contour_fill` |"), "{}", contract.render(&[&group]));
    }

    #[test]
    fn agreeing_registries_have_no_problems_and_numbers_compare_as_numbers() {
        let group = registry(&[ROW_SPACING]);
        assert_eq!(Contract::parse(&data("0.25")).unwrap().check(&[&group]), Vec::<String>::new());
        assert_eq!(Contract::parse(&data("0.250")).unwrap().check(&[&group]), Vec::<String>::new());
    }

    #[test]
    fn a_toggle_ink_stitch_gives_no_default_is_off() {
        let data = data("0.25").replacen(
            "[[param]]",
            "[[param]]\nelement = \"fill\"\nname = \"fill_underlay\"\ntype = \"toggle\"\nunit = \"—\"\ndefault = \"—\"\n\
             applies_to = [\"tatami_fill\"]\nphase = \"P1\"\nmilestone = \"M5\"\n[[param]]",
            1,
        );
        let contract = Contract::parse(&data).unwrap();
        const OFF: &[ParamSpec] = &[ROW_SPACING, ParamSpec { key: "fill_underlay", kind: Kind::Toggle, default: "false", ..ROW_SPACING }];
        const ON: &[ParamSpec] = &[ROW_SPACING, ParamSpec { key: "fill_underlay", kind: Kind::Toggle, default: "true", ..ROW_SPACING }];
        assert_eq!(contract.check(&[&registry(OFF)]), Vec::<String>::new());
        assert_eq!(contract.check(&[&registry(ON)]), [unlike("fill_underlay", "true", "false")]);
    }

    #[test]
    fn every_disagreement_is_reported() {
        const SPECS: &[ParamSpec] = &[
            ParamSpec { kind: Kind::Count { min: 1, max: 9 }, default: "1", ..ROW_SPACING },
            ParamSpec { key: "sparkle_mm", ..ROW_SPACING },
            ParamSpec {
                key: "lock_start",
                kind: Kind::Choice { options: &[ChoiceOption::new("bowtie", "Bowtie")] },
                default: "bowtie",
                origin: Origin::StitchCraft,
                ..ROW_SPACING
            },
        ];
        let group = registry(SPECS);
        let mut data = data("0.25");
        data = data.replace("value = \"legacy_fill\"", "value = \"old_fill\"");
        let problems = Contract::parse(&data).unwrap().check(&[&group]);
        assert_eq!(
            problems,
            [
                "`row_spacing_mm` is a whole number from 1 to 9 here, but Ink/Stitch's is a float in mm",
                unlike("row_spacing_mm", "1", "0.25").as_str(),
                "`sparkle_mm` is declared with an Ink/Stitch origin, but conformance/inkstitch-params.toml has no such parameter",
                "`lock_start` offers [\"bowtie\"], but the lock identifiers in conformance/inkstitch-params.toml are [\"half_stitch\"]",
                "`fill_method`: StitchType has {\"circular_fill\", \"contour_fill\", \"cross_stitch\", \"guided_fill\", \"legacy_fill\", \"linear_gradient_fill\", \"meander_fill\", \"tartan_fill\", \"tatami_fill\"}, but conformance/inkstitch-params.toml lists {\"circular_fill\", \"contour_fill\", \"cross_stitch\", \"guided_fill\", \"linear_gradient_fill\", \"meander_fill\", \"old_fill\", \"tartan_fill\", \"tatami_fill\"}",
            ]
        );
    }

    #[test]
    fn a_parameter_ink_stitch_shows_only_for_some_values_is_shown_for_the_same() {
        let data = data("0.25").replacen(
            "[[param]]",
            "[[param]]\nelement = \"common\"\nname = \"lock_start_scale_mm\"\ntype = \"float\"\nunit = \"mm\"\ndefault = \"0.7\"\n\
             applies_to = [\"back_forth\", \"custom\"]\nphase = \"P1\"\nmilestone = \"M3\"\n[[param]]",
            1,
        );
        let contract = Contract::parse(&data).unwrap();
        let size = |any_of: Option<&'static [&'static str]>| -> ParamGroup {
            let spec = ParamSpec {
                key: "lock_start_scale_mm",
                default: "0.7",
                applies_to: StitchType::ALL,
                visible_when: any_of.map(|any_of| Condition { key: "lock_start", any_of }),
                ..ROW_SPACING
            };
            ParamGroup { name: "CommonParams", help: " Common.\n", applies_to: StitchType::ALL, specs: Box::leak(Box::new([spec])) }
        };
        assert_eq!(contract.check(&[&size(Some(&["custom", "back_forth"]))]), Vec::<String>::new(), "in any order");
        assert_eq!(
            contract.check(&[&size(None)]),
            ["`lock_start_scale_mm` is shown only for {\"back_forth\", \"custom\"} in Ink/Stitch, but always here"]
        );
        assert_eq!(
            contract.check(&[&size(Some(&["custom"]))]),
            ["`lock_start_scale_mm` is shown only for {\"back_forth\", \"custom\"} in Ink/Stitch, but only for {\"custom\"} here"]
        );
        // `zigzag` is a lock shape here, though a satin method has the same id.
        let zigzag = self::data("0.25").replacen(
            "[[param]]",
            "[[param]]\nelement = \"common\"\nname = \"lock_start_scale_mm\"\ntype = \"float\"\nunit = \"mm\"\ndefault = \"0.7\"\n\
             applies_to = [\"zigzag\", \"custom\"]\nphase = \"P1\"\nmilestone = \"M3\"\n[[param]]",
            1,
        );
        assert_eq!(Contract::parse(&zigzag).unwrap().check(&[&size(Some(&["custom", "zigzag"]))]), Vec::<String>::new());
    }

    #[test]
    fn a_lock_shape_named_like_a_satin_method_is_not_a_stitch_type() {
        // The lock sizes are shown for some lock shapes, `zigzag` among them, which is also a satin
        // method's id: in a common row it is a lock shape, in a satin row the satin method.
        let row = |element: &str| Param {
            element: element.to_string(),
            name: "lock_start_scale_percent".to_string(),
            kind: "float".to_string(),
            unit: "%".to_string(),
            default: "100".to_string(),
            fallback: None,
            applies_to: vec!["arrow".to_string(), "zigzag".to_string()],
            read_by: Vec::new(),
            phase: "P1".to_string(),
            milestone: "M3".to_string(),
        };
        assert_eq!(row("common").scope().len(), StitchType::ALL.len());
        assert_eq!(row("satin").scope().into_iter().collect::<Vec<_>>(), [StitchType::SatinZigzag]);
        let strokes = row("stroke").scope();
        assert!(strokes.len() == 4 && strokes.iter().all(|t| t.family() == Family::Stroke), "no stroke method listed: every stroke type");
    }

    #[test]
    fn a_method_parameter_offers_ink_stitch_s_methods_in_its_order_from_its_place_default() {
        let data = data("0.25").replacen(
            "[[param]]",
            "[[param]]\nelement = \"fill\"\nname = \"fill_method\"\ntype = \"combo\"\nunit = \"—\"\ndefault = \"0\"\n\
             applies_to = []\nphase = \"P1\"\nmilestone = \"M5\"\n[[param]]",
            1,
        );
        let contract = Contract::parse(&data).unwrap();
        let fills = |order: &[StitchType], default: &'static str| -> ParamGroup {
            let options: Vec<ChoiceOption> = order.iter().map(|t| ChoiceOption::new(t.id(), t.name())).collect();
            let fills: Vec<StitchType> = StitchType::ALL.iter().copied().filter(|t| t.family() == Family::Fill).collect();
            let spec = ParamSpec {
                key: "fill_method",
                kind: Kind::Choice { options: Box::leak(options.into_boxed_slice()) },
                default,
                applies_to: Box::leak(fills.into_boxed_slice()),
                ..ROW_SPACING
            };
            ParamGroup { name: "FillParams", help: " Fill.\n", applies_to: &[StitchType::TatamiFill], specs: Box::leak(Box::new([spec])) }
        };
        let order: Vec<StitchType> =
            contract.method.iter().filter(|m| m.param == "fill_method").map(|m| StitchType::from_id(Family::Fill, &m.value).unwrap()).collect();
        assert_eq!(contract.check(&[&fills(&order, "tatami_fill")]), Vec::<String>::new(), "Ink/Stitch's 0 is the first method");
        assert_eq!(contract.check(&[&fills(&order, "contour_fill")]), [unlike("fill_method", "contour_fill", "tatami_fill")]);
        let mut reordered = order;
        reordered.swap(0, 1);
        let problems = contract.check(&[&fills(&reordered, reordered[0].id())]);
        assert_eq!(problems.len(), 1, "{problems:?}");
        let expected = format!("`fill_method` offers [\"{}\", \"{}\"", reordered[0].id(), reordered[1].id());
        assert!(problems[0].starts_with(&expected), "{problems:?}");
    }

    #[test]
    fn stitchcraft_parameters_may_not_take_ink_stitch_keys() {
        const OURS: &[ParamSpec] = &[ParamSpec { origin: Origin::StitchCraft, ..ROW_SPACING }];
        let group = registry(OURS);
        assert_eq!(
            Contract::parse(&data("0.25")).unwrap().check(&[&group]),
            ["`row_spacing_mm` is an Ink/Stitch parameter (conformance/inkstitch-params.toml): declare it with an Ink/Stitch origin"]
        );
    }

    #[test]
    fn the_engine_registry_agrees_with_the_committed_contract() {
        let data = crate::util::read(&crate::util::root().join(DATA)).unwrap();
        let registry = stitchcraft_engine::registry::PARAMETERS;
        let contract = Contract::parse(&data).unwrap();
        assert_eq!(contract.check(registry), Vec::<String>::new());
        assert_eq!(contract.check_sewn(registry, &crate::stitch_pages::sewn().unwrap()), Vec::<String>::new());
    }

    #[test]
    fn the_svg_adapter_s_commands_and_attributes_must_be_listed() {
        let data = data("0.25");
        let missing = data
            .replace("name = \"trim\"\ntied_to", "name = \"cut\"\ntied_to")
            .replace("name = \"ignore_object\"\nkind", "name = \"x\"\nkind")
            .replace("element = \"clone\"\nname = \"flip_angle\"", "element = \"clone\"\nname = \"flip\"");
        let problems = Contract::parse(&missing).unwrap().check(&[]);
        assert_eq!(problems.len(), 3, "{problems:?}");
        assert!(problems[0].starts_with("the SVG adapter knows the commands {"), "{problems:?}");
        assert_eq!(
            problems[1],
            "the SVG adapter applies the clone settings {\"angle\", \"clone\", \"flip_angle\"}, but conformance/inkstitch-params.toml lists \
             {\"angle\", \"clone\", \"flip\"}"
        );
        assert_eq!(problems[2], "the SVG adapter reads `ignore_object`, which conformance/inkstitch-params.toml does not list as an attribute");
        // A clone setting the adapter doesn't know is planned.
        let page = Contract::parse(&missing).unwrap().render(&[]);
        assert!(page.contains("| `flip` | toggle | — | true | clones | P2 (M8) | planned |"), "{page}");
        // Attributes the adapter reads, and those it doesn't: planned, or not planned at all.
        let more = format!(
            "{data}[[attribute]]\nname = \"stroke_first\"\nkind = \"element\"\nphase = \"P1\"\nmilestone = \"M8\"\n\
                            [[attribute]]\nname = \"tartan\"\nkind = \"element\"\nphase = \"P3\"\nmilestone = \"M10\"\n\
                            [[attribute]]\nname = \"sew_stack\"\nkind = \"sew_stack\"\nphase = \"—\"\nmilestone = \"—\"\n"
        );
        let page = Contract::parse(&more).unwrap().render(&[]);
        assert!(page.contains("| `stroke_first` | element | P1 (M8) | read |"), "{page}");
        assert!(page.contains("| `tartan` | element | P3 (M10) | planned |"), "{page}");
        assert!(page.contains("| `sew_stack` | sew stack | — | not planned |"), "{page}");
    }

    #[test]
    fn a_parameter_and_a_method_not_planned_say_so() {
        // The parameter's row comes first, then a row for each method.
        let data = data("0.25")
            .replacen("milestone = \"M5\"\n[[method]]", "milestone = \"—\"\n[[method]]", 1)
            .replace("value = \"legacy_fill\"\nphase = \"P1\"\nmilestone = \"M5\"", "value = \"legacy_fill\"\nphase = \"—\"\nmilestone = \"—\"");
        let page = Contract::parse(&data).unwrap().render(&[]);
        assert!(page.contains("| `row_spacing_mm` | float | mm | 0.25 | `tatami_fill` | — | not planned |"), "{page}");
        assert!(page.contains("| `fill_method` | `legacy_fill` | — |"), "{page}");
        assert!(page.contains("| `fill_method` | `tatami_fill` | P1 (M5) |"), "{page}");
    }

    #[test]
    fn bad_data_is_an_error() {
        assert!(Contract::parse("param = 3").is_err());
    }

    /// The data file with `applies_to` for the row of `row_spacing_mm`, and `more` after it.
    fn given(applies_to: &str, more: &str) -> Contract {
        let data = data("0.25").replacen("applies_to = [\"tatami_fill\"]", &format!("applies_to = [{applies_to}]"), 1);
        Contract::parse(&format!("{data}{more}")).unwrap()
    }

    /// An `[[unapplied]]` entry.
    fn entry(element: &str, stitch_type: &str, names: &str, reason: &str) -> String {
        format!("[[unapplied]]\nelement = \"{element}\"\nstitch_type = \"{stitch_type}\"\nnames = [{names}]\nreason = \"{reason}\"\n")
    }

    /// The problem reported for a stitch type StitchCraft sews that a registered row gives `key`, and that no
    /// declaration of the key applies to.
    fn missing(key: &str, stitch_type: &str, element: &str) -> String {
        format!(
            "`{key}` does not apply to `{stitch_type}` here, but Ink/Stitch gives it `{stitch_type}` (its {element} row), which StitchCraft sews: add \
             the stitch type to the key's declaration, or give the reason in an `[[unapplied]]` entry"
        )
    }

    /// The problem reported for a key an `[[unapplied]]` entry excuses that needs no excuse.
    fn stale(stitch_type: &str, element: &str, key: &str) -> String {
        format!(
            "the `[[unapplied]]` entry for `{stitch_type}` ({element}) excuses `{key}`, which needs no excuse: take it out of the entry (the \
             registry gives it the stitch type, StitchCraft does not sew the type, or Ink/Stitch does not give it the setting)"
        )
    }

    const TATAMI_AND_CONTOUR: &[StitchType] = &[StitchType::TatamiFill, StitchType::ContourFill];

    #[test]
    fn a_stitch_type_sewn_that_a_registered_row_gives_its_key_must_be_given_it_here() {
        // Ink/Stitch gives the row spacing to contour fills too, and the declaration leaves them out.
        let contract = given("\"tatami_fill\", \"contour_fill\"", "");
        let tatami = registry(&[ROW_SPACING]);
        assert_eq!(contract.check_sewn(&[&tatami], TATAMI_AND_CONTOUR), [missing("row_spacing_mm", "contour_fill", "fill")]);
        // Given to both, it agrees.
        const BOTH: &[ParamSpec] = &[ParamSpec { applies_to: TATAMI_AND_CONTOUR, ..ROW_SPACING }];
        assert_eq!(contract.check_sewn(&[&registry(BOTH)], TATAMI_AND_CONTOUR), Vec::<String>::new());
        // A stitch type the engine doesn't sew yet is left out until it sews.
        assert_eq!(contract.check_sewn(&[&tatami], &[StitchType::TatamiFill]), Vec::<String>::new());
        // A stitch type that reads the key without the window showing it must be given it too.
        let read = given("\"tatami_fill\"]\nread_by = [\"contour_fill\"", "");
        assert_eq!(read.check_sewn(&[&tatami], TATAMI_AND_CONTOUR), [missing("row_spacing_mm", "contour_fill", "fill")]);
        assert_eq!(read.check_sewn(&[&registry(BOTH)], TATAMI_AND_CONTOUR), Vec::<String>::new());
    }

    #[test]
    fn req_prm_005_a_row_is_given_its_key_by_its_family_s_declaration() {
        // A zigzag stroke and a meander fill each declare the zigzag spacing their own way: the fill's row is
        // given the key by the fill's declaration, whatever the stroke's applies to.
        let row = "[[param]]\nelement = \"fill\"\nname = \"zigzag_spacing_mm\"\ntype = \"float\"\nunit = \"mm\"\ndefault = \"0\"\n\
                   applies_to = [\"meander_fill\", \"circular_fill\"]\nphase = \"P2\"\nmilestone = \"M7\"\n";
        let contract = given("\"tatami_fill\"", row);
        let declared = |name: &'static str, kind, applies_to: &'static [StitchType]| -> ParamGroup {
            let spec = ParamSpec { key: "zigzag_spacing_mm", kind, default: "0.4", applies_to, ..ROW_SPACING };
            ParamGroup { name, help: " Spacing.\n", applies_to, specs: Box::leak(Box::new([spec])) }
        };
        let stroke = declared("ZigzagStrokeParams", Kind::LengthList { min: 0.01, max: 10.0 }, &[StitchType::ZigzagStitch]);
        let length = Kind::Length { min: 0.0, max: 10.0, empty: Empty::Never };
        let both = declared("MeanderFillParams", length, &[StitchType::MeanderFill, StitchType::CircularFill]);
        let meander = declared("MeanderFillParams", length, &[StitchType::MeanderFill]);
        let tatami = registry(&[ROW_SPACING]);
        let sewn = [StitchType::TatamiFill, StitchType::ZigzagStitch, StitchType::MeanderFill, StitchType::CircularFill];
        assert_eq!(contract.check_sewn(&[&tatami, &stroke, &both], &sewn), Vec::<String>::new());
        assert_eq!(contract.check_sewn(&[&tatami, &stroke, &meander], &sewn), [missing("zigzag_spacing_mm", "circular_fill", "fill")]);
    }

    #[test]
    fn a_row_the_registry_declares_for_none_of_its_stitch_types_is_planned_and_not_compared() {
        let contract = given("\"tatami_fill\", \"contour_fill\"", "");
        assert_eq!(contract.check_sewn(&[], TATAMI_AND_CONTOUR), Vec::<String>::new());
        // Declared for a stitch type the row doesn't cover, the key is still planned for the row's types.
        const MEANDER: &[ParamSpec] = &[ParamSpec { applies_to: &[StitchType::MeanderFill], ..ROW_SPACING }];
        assert_eq!(contract.check_sewn(&[&registry(MEANDER)], TATAMI_AND_CONTOUR), Vec::<String>::new());
    }

    #[test]
    fn a_stitch_type_sewn_as_another_takes_that_one_s_settings() {
        // StitchCraft sews a legacy fill as a tatami fill, and reads its settings as a tatami fill's.
        let sewn = [StitchType::TatamiFill, StitchType::LegacyFill];
        let contract = given("\"tatami_fill\", \"legacy_fill\"", "");
        assert_eq!(contract.check_sewn(&[&registry(&[ROW_SPACING])], &sewn), Vec::<String>::new());
        // A declaration for legacy fills alone is one a legacy fill never reads.
        const LEGACY: &[ParamSpec] = &[ParamSpec { applies_to: &[StitchType::LegacyFill], ..ROW_SPACING }];
        assert_eq!(
            given("\"legacy_fill\"", "").check_sewn(&[&registry(LEGACY)], &sewn),
            [
                "`row_spacing_mm` does not apply to `tatami_fill` here, but Ink/Stitch gives it `legacy_fill` (its fill row), which StitchCraft sews as \
                 `tatami_fill`: add the stitch type to the key's declaration, or give the reason in an `[[unapplied]]` entry"
            ]
        );
    }

    #[test]
    fn the_lock_sizes_rows_cover_every_stitch_type_whatever_lock_shapes_they_list() {
        // `zigzag` is a lock shape in a common row, though a satin method has the same id: the row covers every
        // stitch type, as its declaration must.
        let data = data("0.25").replacen(
            "[[param]]",
            "[[param]]\nelement = \"common\"\nname = \"lock_start_scale_percent\"\ntype = \"float\"\nunit = \"%\"\ndefault = \"100\"\n\
             applies_to = [\"arrow\", \"zigzag\", \"custom\"]\nphase = \"P1\"\nmilestone = \"M3\"\n[[param]]",
            1,
        );
        let contract = Contract::parse(&data).unwrap();
        let scale = |applies_to: Vec<StitchType>| -> ParamGroup {
            let applies_to: &'static [StitchType] = Box::leak(applies_to.into_boxed_slice());
            let spec = ParamSpec {
                key: "lock_start_scale_percent",
                kind: Kind::Percent { min: 10.0, max: 400.0 },
                default: "100",
                applies_to,
                ..ROW_SPACING
            };
            ParamGroup { name: "CommonParams", help: " Common.\n", applies_to: StitchType::ALL, specs: Box::leak(Box::new([spec])) }
        };
        let tatami = registry(&[ROW_SPACING]);
        assert_eq!(contract.check_sewn(&[&tatami, &scale(StitchType::ALL.to_vec())], StitchType::ALL), Vec::<String>::new());
        let but_running = StitchType::ALL.iter().copied().filter(|t| *t != StitchType::RunningStitch).collect();
        assert_eq!(
            contract.check_sewn(&[&tatami, &scale(but_running)], StitchType::ALL),
            [missing("lock_start_scale_percent", "running_stitch", "common")]
        );
        // Declared for the satin zigzag alone, the key leaves out every other stitch type sewn.
        let problems = contract.check_sewn(&[&tatami, &scale(vec![StitchType::SatinZigzag])], &[StitchType::SatinZigzag, StitchType::ZigzagStitch]);
        assert_eq!(problems, [missing("lock_start_scale_percent", "zigzag_stitch", "common")]);
    }

    #[test]
    fn an_unapplied_entry_excuses_a_stitch_type_with_its_reason_and_the_page_lists_it() {
        let tatami = registry(&[ROW_SPACING]);
        let excused = given("\"tatami_fill\", \"contour_fill\"", &entry("fill", "contour_fill", "\"row_spacing_mm\"", "Its rings | spaced so."));
        assert_eq!(excused.check_sewn(&[&tatami], TATAMI_AND_CONTOUR), Vec::<String>::new());
        let page = excused.render(&[&tatami]);
        assert!(page.contains("\n## Settings a stitch type does not take\n"), "{page}");
        assert!(page.contains("| fill | `contour_fill` | `row_spacing_mm` | Its rings \\| spaced so. |\n\n## Method identifiers"), "{page}");
        // An entry for another element, stitch type or key excuses nothing.
        for wrong in [
            entry("satin", "contour_fill", "\"row_spacing_mm\"", "Why."),
            entry("fill", "meander_fill", "\"row_spacing_mm\"", "Why."),
            entry("fill", "contour_fill", "\"angle\"", "Why."),
        ] {
            let problems = given("\"tatami_fill\", \"contour_fill\"", &wrong).check_sewn(&[&tatami], TATAMI_AND_CONTOUR);
            assert_eq!(problems.len(), 2, "{wrong}: {problems:?}");
            assert_eq!(problems[0], missing("row_spacing_mm", "contour_fill", "fill"), "{wrong}");
            assert!(problems[1].starts_with("the `[[unapplied]]` entry for `"), "{wrong}: {problems:?}");
        }
    }

    #[test]
    fn an_unapplied_entry_that_excuses_nothing_or_gives_no_reason_is_reported() {
        let tatami = registry(&[ROW_SPACING]);
        let excuse = entry("fill", "contour_fill", "\"row_spacing_mm\"", "Why.");
        // Once the registry gives contour fills the key, or while StitchCraft doesn't sew them, it excuses nothing.
        const BOTH: &[ParamSpec] = &[ParamSpec { applies_to: TATAMI_AND_CONTOUR, ..ROW_SPACING }];
        let contract = given("\"tatami_fill\", \"contour_fill\"", &excuse);
        assert_eq!(contract.check_sewn(&[&registry(BOTH)], TATAMI_AND_CONTOUR), [stale("contour_fill", "fill", "row_spacing_mm")]);
        assert_eq!(contract.check_sewn(&[&tatami], &[StitchType::TatamiFill]), [stale("contour_fill", "fill", "row_spacing_mm")]);
        // Each key and each entry on its own: the second entry's key is excused, the first entry's is not.
        let two =
            entry("fill", "tatami_fill", "\"row_spacing_mm\"", "Why.") + &entry("fill", "contour_fill", "\"row_spacing_mm\", \"angle\"", "Why.");
        assert_eq!(
            given("\"tatami_fill\", \"contour_fill\"", &two).check_sewn(&[&tatami], TATAMI_AND_CONTOUR),
            [stale("tatami_fill", "fill", "row_spacing_mm"), stale("contour_fill", "fill", "angle")]
        );
        let blank = given("\"tatami_fill\", \"contour_fill\"", &entry("fill", "contour_fill", "\"row_spacing_mm\"", " "));
        assert_eq!(blank.check_sewn(&[&tatami], TATAMI_AND_CONTOUR), ["the `[[unapplied]]` entry for `contour_fill` (fill) gives no reason"]);
    }
}
