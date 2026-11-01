//! Ink/Stitch's own objects in a drawing: its commands, the connectors that tie them to objects, and its
//! helper paths. None of them is part of the design.
//!
//! Ink/Stitch keeps some of its controls in the drawing itself, as objects a viewer draws. Read as plain
//! SVG they would be stitched: a line from each command's symbol to its object, the guide lines that steer
//! other shapes' stitches, and even the templates a file asks to leave out. Ink/Stitch never stitches
//! them, and neither does StitchCraft. Read at Ink/Stitch `d59c9ab`: `lib/commands.py`,
//! `lib/elements/utils/nodes.py`, `lib/marker.py`.
//!
//! - **Commands** are `<use>` elements that show a `<symbol>` whose id is `inkstitch_` and the command's
//!   name; a symbol copied between files may add `-` and a number. A command for one object is tied to
//!   it by a connector, a path whose `inkscape:connection-start` and `inkscape:connection-end` name the
//!   `<use>` and the object, either way round. Layer and document commands stand alone. A `<use>` of any
//!   other symbol is an ordinary clone.
//! - **Connectors**, paths with those attributes or with `inkscape:connector-type`, are never stitched.
//!   One that ties no command (drawn with Inkscape's connector tool) is reported.
//! - **Helper paths** carry one of Ink/Stitch's own start markers in their `style` attribute: a guide
//!   line, an anchor line or a stitch pattern. They steer the stitches of other shapes and are not
//!   stitched themselves; StitchCraft does not apply them yet, and says so.
//! - **Leaving out.** The `ignore_object` command, or the `inkstitch:ignore_object` setting, leaves out an
//!   object and everything in it; an `ignore_layer` command leaves out every layer it is in. Each is
//!   listed (`SC-I0805`).
//!
//! The other commands: `trim` and `stop` turn on the object's `trim_after` and `stop_after`, exactly as
//! the settings would. `starting_point` and `ending_point` place where Ink/Stitch starts and ends a fill
//! or a satin column; they, `origin` and `stop_position` are not applied yet (`SC-W0802`). `target_point`
//! aims circular fills and ripples, which StitchCraft does not sew yet, and the rest feed Ink/Stitch's own
//! tools, so they change nothing StitchCraft sews. Before its file version 3, Ink/Stitch named 7 commands
//! otherwise ([`RENAMED`]); they are read as the commands they became.
//!
//! **Ink/Stitch's own content** is what tells its updater that a file is one of its own
//! ([`crate::version`]): an element or an attribute in its namespace, or an `embroider_` attribute, the
//! settings as Ink/Stitch's early versions wrote them (`is_setting`). This pass finds it, and the file's version, as it finds the
//! commands.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::{Attribute, ExpandedName, Node};
use stitchcraft_core::{Exhausted, Meter};

use crate::style::Declared;
use crate::version;

pub(crate) const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
pub(crate) const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
/// Ink/Stitch's XML namespace, of its settings and its metadata.
pub const INKSTITCH_NS: &str = "http://inkstitch.org/namespace";

// The settings the `trim` and `stop` commands turn on.
pub(crate) use stitchcraft_engine::common::{STOP_AFTER, TRIM_AFTER};
/// Ink/Stitch's own setting for leaving an object out; not a parameter, so it is read here.
pub(crate) const IGNORE_OBJECT: &str = "ignore_object";
/// The `inkstitch:*` attributes, other than parameters, that this reader reads; any other attribute
/// means the file has settings it does not read yet. The compatibility contract lists them all.
pub const READ_ATTRIBUTES: &[&str] = &[IGNORE_OBJECT];

/// What StitchCraft does with an Ink/Stitch command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Does {
    /// Turns on the object's `trim_after`.
    Trim,
    /// Turns on the object's `stop_after`.
    Stop,
    /// Leaves the object out, and lists it.
    IgnoreObject,
    /// Leaves out every layer the symbol is in, and lists each.
    IgnoreLayer,
    /// Nothing yet, and says so: it changes where the design sews.
    NotYet,
    /// Nothing yet, and says so on the object it is tied to: it places where Ink/Stitch starts or ends a
    /// fill or a satin column.
    StartOrEnd,
    /// Nothing: it aims a circular fill or a ripple, stitch types StitchCraft does not sew yet.
    Positions,
    /// Nothing: it is input to Ink/Stitch's own tools, not to sewing.
    ToolInput,
}

impl Does {
    /// What StitchCraft does, for the compatibility contract.
    pub const fn describe(self) -> &'static str {
        match self {
            Does::Trim => "applied: turns on `trim_after`",
            Does::Stop => "applied: turns on `stop_after`",
            Does::IgnoreObject => "applied: the object is left out and listed (`SC-I0805`)",
            Does::IgnoreLayer => "applied: every layer it is in is left out and listed (`SC-I0805`)",
            Does::NotYet => "planned: noted as not applied (`SC-W0802`)",
            Does::StartOrEnd => "planned: noted on the object as not applied (`SC-W0802`)",
            Does::Positions => "read with the stitch types it positions",
            Does::ToolInput => "nothing to sew: input to Ink/Stitch's tools",
        }
    }
}

/// An Ink/Stitch command: its name, and what StitchCraft does with it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Command {
    /// The name its symbol's id carries after `inkstitch_`.
    pub name: &'static str,
    /// What StitchCraft does with it.
    pub does: Does,
}

/// Ink/Stitch's commands, by the names its symbols carry (`lib/commands.py`); the compatibility
/// contract lists the same names.
pub const COMMANDS: [Command; 12] = [
    Command { name: "starting_point", does: Does::StartOrEnd },
    Command { name: "ending_point", does: Does::StartOrEnd },
    Command { name: "target_point", does: Does::Positions },
    Command { name: "autoroute_start", does: Does::ToolInput },
    Command { name: "autoroute_end", does: Does::ToolInput },
    Command { name: "stop", does: Does::Stop },
    Command { name: "trim", does: Does::Trim },
    Command { name: "ignore_object", does: Does::IgnoreObject },
    Command { name: "satin_cut_point", does: Does::ToolInput },
    Command { name: "ignore_layer", does: Does::IgnoreLayer },
    Command { name: "origin", does: Does::NotYet },
    Command { name: "stop_position", does: Does::NotYet },
];

/// Ink/Stitch's command names from before its file version 3, each with the command it became. Its
/// updater renames them in the files it updates, where they then work as those commands (`lib/update.py`,
/// read at `d59c9ab`); StitchCraft reads them so in any file (`DEV-SVG-014`). The compatibility contract
/// lists the same.
pub const RENAMED: [(&str, &str); 7] = [
    ("fill_start", "starting_point"),
    ("fill_end", "ending_point"),
    ("satin_start", "autoroute_start"),
    ("satin_end", "autoroute_end"),
    ("run_start", "autoroute_start"),
    ("run_end", "autoroute_end"),
    ("ripple_target", "target_point"),
];

/// The command a symbol with this id stands for: `inkstitch_` and the name, perhaps with a copy's `-n`. An
/// old name stands for the command it became.
fn command(symbol_id: &str) -> Option<Command> {
    let name = symbol_id.strip_prefix("inkstitch_")?.split('-').next()?;
    let name = RENAMED.iter().find(|(old, _)| *old == name).map_or(name, |(_, new)| new);
    COMMANDS.into_iter().find(|c| c.name == name)
}

/// Whether `attribute` holds one of Ink/Stitch's settings: it is in Ink/Stitch's namespace, or it is one
/// of the `embroider_` attributes Ink/Stitch's early versions wrote its settings as, which its updater turns
/// into its own (`lib/update.py`).
pub(crate) fn is_setting(attribute: &Attribute<'_, '_>) -> bool {
    match attribute.namespace() {
        Some(ns) => ns == INKSTITCH_NS,
        None => attribute.name().starts_with("embroider_"),
    }
}

/// Ink/Stitch's own helpers, which a start marker in the `style` attribute marks (`lib/marker.py`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Helper {
    AnchorLine,
    Pattern,
    GuideLine,
}

impl Helper {
    /// What it is, and what it would do: "an Ink/Stitch guide line, a helper for other shapes".
    pub(crate) const fn what(self) -> (&'static str, &'static str) {
        match self {
            Helper::AnchorLine => ("an Ink/Stitch anchor line, a helper for other shapes", "anchor lines"),
            Helper::Pattern => ("an Ink/Stitch stitch pattern, a helper for the other shapes in its group", "patterns"),
            Helper::GuideLine => ("an Ink/Stitch guide line, a helper for other shapes", "guide lines"),
        }
    }

    /// The helper a shape is, if its own `style` attribute starts it with one of Ink/Stitch's markers.
    /// Ink/Stitch looks only there: a marker set another way, or at the other end, is an ordinary one.
    pub(crate) fn of(declared: &Declared<'_, '_>) -> Option<Helper> {
        const MARKERS: [(&str, Helper); 3] = [
            ("url(#inkstitch-anchor-line-marker", Helper::AnchorLine),
            ("url(#inkstitch-pattern-marker", Helper::Pattern),
            ("url(#inkstitch-guide-line-marker", Helper::GuideLine),
        ];
        declared
            .in_style("marker-start")
            .into_iter()
            .find_map(|value| MARKERS.into_iter().find(|(prefix, _)| value.starts_with(prefix)).map(|(_, h)| h))
    }
}

/// Ink/Stitch's reading of a yes (`lib/elements/element.py`): one of its words for on, which a toggle
/// parameter reads too ([`stitchcraft_params::toggle`]). Anything else, "on" included, is no.
pub(crate) fn yes(value: &str) -> bool {
    stitchcraft_params::toggle(value) == Some(true)
}

/// Whether `node` is a layer: a group Inkscape shows in its layers list.
pub(crate) fn is_layer(node: Node<'_, '_>) -> bool {
    node.tag_name().name() == "g" && node.attribute((INKSCAPE_NS, "groupmode")) == Some("layer")
}

/// Whether `node` is a connector: a path tied to two objects, which Ink/Stitch never stitches.
pub(crate) fn is_connector(node: Node<'_, '_>) -> bool {
    ["connection-start", "connection-end", "connector-type"].iter().any(|name| node.has_attribute((INKSCAPE_NS, *name)))
}

/// The name of the SVG element `name` in a document whose SVG elements are in namespace `ns`.
fn svg_tag<'a>(ns: Option<&'a str>, name: &'static str) -> ExpandedName<'a, 'static> {
    match ns {
        Some(ns) => ExpandedName::from((ns, name)),
        None => ExpandedName::from(name),
    }
}

/// Where `node` points with its link: `xlink:href`, or SVG 2's plain `href`.
pub(crate) fn href<'a>(node: Node<'a, '_>) -> Option<&'a str> {
    node.attribute((XLINK_NS, "href")).or_else(|| node.attribute("href"))
}

/// A node's place in its document: roxmltree numbers nodes in document order, so places sort that way.
pub(crate) type Place = u32;

/// `node`'s place in its document.
pub(crate) fn place(node: Node<'_, '_>) -> Place {
    node.id().get()
}

/// The ids in a document, its style sheets, its Ink/Stitch commands and what tells Ink/Stitch's version.
pub(crate) struct Found<'a, 'input> {
    /// The first element with each id, as `getElementById` finds it.
    pub ids: BTreeMap<&'a str, Node<'a, 'input>>,
    /// The `<style>` elements, in order: wherever one is, its rules apply to the whole document.
    pub sheets: Vec<Node<'a, 'input>>,
    /// The commands.
    pub objects: Objects,
    /// Whether the document has Ink/Stitch's own content: an element or an attribute in its namespace,
    /// or an `embroider_` attribute.
    pub content: bool,
    /// The first text in an element named for Ink/Stitch's file version, as its updater finds it: the
    /// first in document order inside any of them.
    pub version: Option<&'a str>,
}

/// Every Ink/Stitch command in a document, by where it applies.
#[derive(Default)]
pub(crate) struct Objects {
    /// The commands tied to each object.
    commands: BTreeMap<Place, Vec<Command>>,
    /// The connectors that tie a command to its object.
    command_connectors: BTreeSet<Place>,
    /// Every `<use>` of a command's symbol, and its command.
    uses: BTreeMap<Place, Command>,
    /// The layers an `ignore_layer` command is in.
    ignored_layers: BTreeSet<Place>,
    /// The `<use>` elements of commands that change where the design sews but are not applied, or that
    /// are not where they apply (an `ignore_layer` outside every layer), with why.
    unapplied: Vec<(Place, Command, Unapplied)>,
}

/// Why a command a file has is not applied.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Unapplied {
    /// StitchCraft does not apply it yet.
    NotYet,
    /// An `ignore_layer` command that is in no layer.
    NoLayer,
}

impl Objects {
    /// The commands tied to `node`.
    pub(crate) fn on(&self, node: Node<'_, '_>) -> &[Command] {
        self.commands.get(&place(node)).map_or(&[], Vec::as_slice)
    }

    /// Whether `node` is the connector of a command, which the command speaks for.
    pub(crate) fn ties_a_command(&self, node: Node<'_, '_>) -> bool {
        self.command_connectors.contains(&place(node))
    }

    /// The command `node` shows, if it is a `<use>` of a command's symbol.
    pub(crate) fn shown_by(&self, node: Node<'_, '_>) -> Option<Command> {
        self.uses.get(&place(node)).copied()
    }

    /// Whether an `ignore_layer` command is in the layer `node`.
    pub(crate) fn layer_ignored(&self, node: Node<'_, '_>) -> bool {
        self.ignored_layers.contains(&place(node))
    }

    /// The commands tied to objects, with the places of the objects, in document order.
    pub(crate) fn tied(&self) -> impl Iterator<Item = (Place, Command)> + '_ {
        self.commands.iter().flat_map(|(node, commands)| commands.iter().map(|c| (*node, *c)))
    }

    /// The commands that are not applied, by their `<use>` elements' places, in document order.
    pub(crate) fn unapplied(&self) -> &[(Place, Command, Unapplied)] {
        &self.unapplied
    }
}

/// The ids, style sheets, Ink/Stitch commands and Ink/Stitch content and version of the document under
/// `root`, whose SVG elements are in namespace `ns`. One unit of work per node, one per step from a layer
/// command up to its layers, and one per node searched for the version's text or per step up from an
/// element of the version's name.
pub(crate) fn find<'a, 'input>(root: Node<'a, 'input>, ns: Option<&str>, meter: &mut Meter) -> Result<Found<'a, 'input>, Exhausted> {
    let mut ids = BTreeMap::new();
    let (mut connectors, mut uses, mut sheets) = (Vec::new(), Vec::new(), Vec::new());
    let (mut content, mut version, mut searched) = (false, None, Vec::new());
    for node in root.descendants() {
        meter.charge(1)?;
        if !node.is_element() {
            continue;
        }
        content = content || node.tag_name().namespace() == Some(INKSTITCH_NS) || node.attributes().any(|a| is_setting(&a));
        if version.is_none() && node.tag_name().name() == version::ELEMENT {
            version = version_text(node, &mut searched, meter)?;
        }
        if let Some(id) = node.attribute("id") {
            ids.entry(id).or_insert(node);
        }
        // A connector ties two objects; one that names only one end ties nothing.
        if let (Some(start), Some(end)) = (node.attribute((INKSCAPE_NS, "connection-start")), node.attribute((INKSCAPE_NS, "connection-end"))) {
            connectors.push((node, start, end));
        }
        if node.tag_name() == svg_tag(ns, "use") {
            uses.push(node);
        }
        if node.tag_name() == svg_tag(ns, "style") {
            sheets.push(node);
        }
    }
    // A link is `#` and an id, exactly: Ink/Stitch does not trim it either.
    let by_url = |url: Option<&str>| url.and_then(|u| u.strip_prefix('#')).and_then(|id| ids.get(id).copied());
    let mut objects = Objects::default();
    for node in uses {
        let symbol = by_url(href(node)).filter(|s| s.tag_name() == svg_tag(ns, "symbol"));
        let Some(command) = symbol.and_then(|s| s.attribute("id")).and_then(command) else { continue };
        objects.uses.insert(place(node), command);
        match command.does {
            Does::IgnoreLayer => {
                let mut in_a_layer = false;
                for ancestor in node.ancestors().skip(1) {
                    meter.charge(1)?;
                    if is_layer(ancestor) {
                        objects.ignored_layers.insert(place(ancestor));
                        in_a_layer = true;
                    }
                }
                if !in_a_layer {
                    objects.unapplied.push((place(node), command, Unapplied::NoLayer));
                }
            }
            Does::NotYet => objects.unapplied.push((place(node), command, Unapplied::NotYet)),
            _ => {}
        }
    }
    for (connector, start, end) in connectors {
        let (Some(start), Some(end)) = (by_url(Some(start)), by_url(Some(end))) else { continue };
        let tied = [(start, end), (end, start)].into_iter().find_map(|(symbol, target)| Some((objects.shown_by(symbol)?, target)));
        if let Some((command, target)) = tied {
            objects.command_connectors.insert(place(connector));
            objects.commands.entry(place(target)).or_default().push(command);
        }
    }
    Ok(Found { ids, sheets, objects, content, version })
}

/// The first text inside `element`, an element of the version's name, unless it is inside one of those
/// `searched` already, which had none. Each element searched is added to them.
fn version_text<'a, 'input>(
    element: Node<'a, 'input>,
    searched: &mut Vec<Node<'a, 'input>>,
    meter: &mut Meter,
) -> Result<Option<&'a str>, Exhausted> {
    for ancestor in element.ancestors().skip(1) {
        meter.charge(1)?;
        if searched.contains(&ancestor) {
            return Ok(None);
        }
    }
    searched.push(element);
    for node in element.descendants() {
        meter.charge(1)?;
        if node.is_text() {
            return Ok(node.text());
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_engine::registry::PARAMETERS;
    use stitchcraft_params::{Kind, declarations, find as find_param};

    use super::*;

    #[test]
    fn the_commands_settings_are_registered_toggles() {
        // Every declaration: a command turns its setting on whatever the object sews as.
        for key in [TRIM_AFTER, STOP_AFTER] {
            let kinds: Vec<Kind> = declarations(PARAMETERS, key).map(|spec| spec.kind).collect();
            assert!(!kinds.is_empty() && kinds.iter().all(|kind| *kind == Kind::Toggle), "{key}: {kinds:?}");
        }
        // Ink/Stitch's own setting for leaving an object out is not a parameter of a stitch.
        assert_eq!(find_param(PARAMETERS, IGNORE_OBJECT), None);
    }

    #[test]
    fn what_each_command_does_is_described_once() {
        assert_eq!(Does::Trim.describe(), "applied: turns on `trim_after`");
        let all = [Does::Trim, Does::Stop, Does::IgnoreObject, Does::IgnoreLayer, Does::NotYet, Does::StartOrEnd, Does::Positions, Does::ToolInput];
        let texts: BTreeSet<&str> = all.iter().map(|d| d.describe()).collect();
        assert_eq!(texts.len(), all.len(), "{texts:?}");
        // Every command is described, and every description is used.
        let used: BTreeSet<&str> = COMMANDS.iter().map(|c| c.does.describe()).collect();
        assert_eq!(used, texts);
    }

    #[test]
    fn symbols_name_their_commands() {
        assert_eq!(command("inkstitch_trim"), Some(Command { name: "trim", does: Does::Trim }));
        assert_eq!(command("inkstitch_stop_position-12").map(|c| c.does), Some(Does::NotYet));
        assert_eq!(command("inkstitch_stop-3").map(|c| c.does), Some(Does::Stop));
        for other in ["inkstitch_", "inkstitch_sparkle", "trim", "Inkstitch_trim", "inkstitch_TRIM", "fill_start", "inkstitch_FILL_START"] {
            assert_eq!(command(other), None, "{other}");
        }
        // An old name is the command it became, and each becomes one of today's.
        assert_eq!(command("inkstitch_fill_start-2").map(|c| c.name), Some("starting_point"));
        for (old, new) in RENAMED {
            assert_eq!(command(&format!("inkstitch_{old}")).map(|c| c.name), Some(new), "{old}");
            assert!(command(&format!("inkstitch_{new}")).is_some() && command(&format!("inkstitch_{old}_{new}")).is_none());
        }
    }

    #[test]
    fn ink_stitch_s_settings_are_its_namespace_s_attributes_and_its_first_version_s() {
        let doc = roxmltree::Document::parse(
            r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:i="http://inkstitch.org/namespace" xmlns:o="http://example.org/o"
                 i:fill_method="tatami_fill" embroider_satin_column="yes" o:embroider_x="1" xembroider_y="1" fill="red"/>"#,
        )
        .unwrap();
        let settings: Vec<&str> = doc.root_element().attributes().filter(is_setting).map(|a| a.name()).collect();
        assert_eq!(settings, ["fill_method", "embroider_satin_column"]);
    }

    #[test]
    fn the_version_is_the_first_text_inside_an_element_of_its_name() {
        let found = |xml: &str| {
            let doc = roxmltree::Document::parse(xml).unwrap();
            let found = find(doc.root_element(), doc.root_element().tag_name().namespace(), &mut Budget::DEFAULT.meter()).unwrap();
            (found.version.map(str::to_string), found.content)
        };
        assert_eq!(found("<svg><inkstitch_svg_version>3</inkstitch_svg_version></svg>"), (Some("3".to_string()), false));
        // Nested ones without text are searched once, and an empty one leaves the search to the next.
        let nested = "<svg><inkstitch_svg_version><inkstitch_svg_version><b/></inkstitch_svg_version></inkstitch_svg_version>\
                      <i:inkstitch_svg_version xmlns:i='http://inkstitch.org/namespace'>2</i:inkstitch_svg_version></svg>";
        assert_eq!(found(nested), (Some("2".to_string()), true));
        // 6 nodes; for the outer element 2 steps up, to the document, and its 3 elements searched; for the
        // inner one 1 step up, to the outer one, which was searched; for the last 2 steps up and its 2 nodes
        // searched.
        let doc = roxmltree::Document::parse(nested).unwrap();
        let work = 6 + (2 + 3) + 1 + (2 + 2);
        for (max_work, done) in [(work, true), (work - 1, false)] {
            let mut meter = Budget { max_stitches: 1, max_work }.meter();
            assert_eq!(find(doc.root_element(), None, &mut meter).is_ok(), done, "{max_work}");
        }
    }

    #[test]
    fn a_yes_as_ink_stitch_reads_it() {
        for word in ["yes", "Y", " true ", "T", "1"] {
            assert!(yes(word), "{word}");
        }
        for word in ["", "no", "on", "2", "yess", "false"] {
            assert!(!yes(word), "{word}");
        }
    }
}
