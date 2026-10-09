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
//! the settings would; `origin` and `stop_position` are not applied yet (`SC-W0802`); the rest position
//! fills, satins and ripples, or feed Ink/Stitch's own tools, so they change nothing StitchCraft sews yet.

use std::collections::{BTreeMap, BTreeSet};

use roxmltree::Node;
use stitchcraft_core::{Exhausted, Meter};

use crate::style::Declared;

pub(crate) const INKSCAPE_NS: &str = "http://www.inkscape.org/namespaces/inkscape";
pub(crate) const XLINK_NS: &str = "http://www.w3.org/1999/xlink";
pub(crate) const INKSTITCH_NS: &str = "http://inkstitch.org/namespace";

/// The setting the `trim` command turns on, as Ink/Stitch names it.
pub(crate) const TRIM_AFTER: &str = "trim_after";
/// The setting the `stop` command turns on, as Ink/Stitch names it.
pub(crate) const STOP_AFTER: &str = "stop_after";
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
    /// Nothing: it positions a fill, satin or ripple, which StitchCraft does not sew yet.
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
    Command { name: "starting_point", does: Does::Positions },
    Command { name: "ending_point", does: Does::Positions },
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

/// The command a symbol with this id stands for: `inkstitch_` and the name, perhaps with a copy's `-n`.
fn command(symbol_id: &str) -> Option<Command> {
    let name = symbol_id.strip_prefix("inkstitch_")?.split('-').next()?;
    COMMANDS.into_iter().find(|c| c.name == name)
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
        declared.in_style("marker-start").find_map(|value| MARKERS.into_iter().find(|(prefix, _)| value.starts_with(prefix)).map(|(_, h)| h))
    }
}

/// Ink/Stitch's reading of a yes (`lib/elements/element.py`): yes, y, true, t or 1, in any case, around
/// spaces. Anything else, "on" included, is no.
pub(crate) fn yes(value: &str) -> bool {
    ["yes", "y", "true", "t", "1"].iter().any(|word| value.trim().eq_ignore_ascii_case(word))
}

/// Whether `node` is a layer: a group Inkscape shows in its layers list.
pub(crate) fn is_layer(node: Node<'_, '_>) -> bool {
    node.tag_name().name() == "g" && node.attribute((INKSCAPE_NS, "groupmode")) == Some("layer")
}

/// Whether `node` is a connector: a path tied to two objects, which Ink/Stitch never stitches.
pub(crate) fn is_connector(node: Node<'_, '_>) -> bool {
    ["connection-start", "connection-end", "connector-type"].iter().any(|name| node.has_attribute((INKSCAPE_NS, *name)))
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

/// The ids in a document, and its Ink/Stitch commands.
pub(crate) struct Found<'a, 'input> {
    /// The first element with each id, as `getElementById` finds it.
    pub ids: BTreeMap<&'a str, Node<'a, 'input>>,
    /// The commands.
    pub objects: Objects,
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

    /// The trim and stop commands, with the places of the objects they are tied to, in document order.
    pub(crate) fn trims_and_stops(&self) -> Vec<(Place, Command)> {
        let all = self.commands.iter().flat_map(|(node, commands)| commands.iter().map(|c| (*node, *c)));
        all.filter(|(_, c)| matches!(c.does, Does::Trim | Does::Stop)).collect()
    }

    /// The commands that are not applied, by their `<use>` elements' places, in document order.
    pub(crate) fn unapplied(&self) -> &[(Place, Command, Unapplied)] {
        &self.unapplied
    }
}

/// The ids and Ink/Stitch commands of the document under `root`, whose SVG elements are in namespace
/// `ns`. One unit of work per node, and one per step from a layer command up to its layers.
pub(crate) fn find<'a, 'input>(root: Node<'a, 'input>, ns: Option<&str>, meter: &mut Meter) -> Result<Found<'a, 'input>, Exhausted> {
    let mut ids = BTreeMap::new();
    let (mut connectors, mut uses) = (Vec::new(), Vec::new());
    for node in root.descendants() {
        meter.charge(1)?;
        if !node.is_element() {
            continue;
        }
        if let Some(id) = node.attribute("id") {
            ids.entry(id).or_insert(node);
        }
        if node.has_attribute((INKSCAPE_NS, "connection-start")) || node.has_attribute((INKSCAPE_NS, "connection-end")) {
            connectors.push(node);
        }
        if node.tag_name().namespace() == ns && node.tag_name().name() == "use" && href(node).is_some_and(|h| h.starts_with("#inkstitch_")) {
            uses.push(node);
        }
    }
    let by_url = |url: Option<&str>| url.and_then(|u| u.trim().strip_prefix('#')).and_then(|id| ids.get(id).copied());
    let mut objects = Objects::default();
    for node in uses {
        let symbol = by_url(href(node)).filter(|s| s.tag_name().namespace() == ns && s.tag_name().name() == "symbol");
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
    for connector in connectors {
        let start = by_url(connector.attribute((INKSCAPE_NS, "connection-start")));
        let end = by_url(connector.attribute((INKSCAPE_NS, "connection-end")));
        let (Some(start), Some(end)) = (start, end) else { continue };
        let tied = [(start, end), (end, start)].into_iter().find_map(|(symbol, target)| Some((objects.shown_by(symbol)?, target)));
        if let Some((command, target)) = tied {
            objects.command_connectors.insert(place(connector));
            objects.commands.entry(place(target)).or_default().push(command);
        }
    }
    Ok(Found { ids, objects })
}

#[cfg(test)]
mod tests {
    use stitchcraft_engine::registry::PARAMETERS;
    use stitchcraft_params::{Kind, find};

    use super::*;

    #[test]
    fn the_commands_settings_are_registered_toggles() {
        for key in [TRIM_AFTER, STOP_AFTER] {
            assert_eq!(find(PARAMETERS, key).map(|spec| spec.kind), Some(Kind::Toggle), "{key}");
        }
        // Ink/Stitch's own setting for leaving an object out is not a parameter of a stitch.
        assert_eq!(find(PARAMETERS, IGNORE_OBJECT), None);
    }

    #[test]
    fn symbols_name_their_commands() {
        assert_eq!(command("inkstitch_trim"), Some(Command { name: "trim", does: Does::Trim }));
        assert_eq!(command("inkstitch_stop_position-12").map(|c| c.does), Some(Does::NotYet));
        assert_eq!(command("inkstitch_stop-3").map(|c| c.does), Some(Does::Stop));
        for other in ["inkstitch_", "inkstitch_sparkle", "trim", "Inkstitch_trim", "inkstitch_TRIM"] {
            assert_eq!(command(other), None, "{other}");
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
