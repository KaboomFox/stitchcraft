//! Clones (`REQ-SVG-012`, `REQ-SVG-013`): a `<use>` that names another element, its source, shows it where
//! the use is, and sews what it shows there, in the use's place in the document's order, as Ink/Stitch sews a
//! clone. Inkscape draws its clones so (Edit › Clone › Create Clone), and its symbols too.
//!
//! **What it shows.** The reader walks the source and everything in it as it walks the document
//! ([`Reader::walk`]), under the map of the use: its transform, then its `x` and `y`, then the source's own
//! transform, as a viewer draws it. Style comes as a viewer gives it: the source inherits from the use, and
//! the source's own declarations, its presentation attributes and the style sheets' rules that select it,
//! win. In what the use shows, `context-fill` and `context-stroke` paint with the use's fill and stroke
//! ([`Style::context`]). A clip path on the use cuts what it shows. The elements a clone sews are named after it:
//! `clone1/square` for the shape `square` it shows, so each has an id of its own. A clone inside the source
//! is drawn the same way, inside the one that shows it.
//!
//! **A symbol** is drawn in the viewport the use gives it, as a viewer draws it: its `viewBox` fitted, by its
//! `preserveAspectRatio`, to the use's `width` and `height`, the symbol's own when the use sets none (SVG 2),
//! and the whole page when neither does. What lies outside that viewport is sewn: Inkscape draws it, where a
//! browser clips it (Inkscape's issue 11611 shows it in Inkscape 1.2.2).
//!
//! **What Ink/Stitch adds** (`lib/elements/clone.py` at `d59c9ab`, read and run as a black box):
//!
//! - The clone's own settings ([`CLONE_SETTINGS`]): `clone` off sews nothing, and is listed (`SC-I0805`);
//!   `angle` gives every fill the clone shows that angle, and `flip_angle` reverses it.
//! - A fill's angle turns with the clone. Ink/Stitch takes the fill's rows through the map from where the
//!   source is drawn to where the clone draws it ([`Affine::turn`]), each clone in its own place, the
//!   innermost first, and rounds each angle to 6 decimal places. It takes the source's place from the
//!   transforms round the source, and a symbol's from those round the use. Only shapes whose fill is sewn
//!   get the angle.
//! - Commands: those tied to the source go with the clone, at their places moved by the same map, and so
//!   do those inside the source whose connector is inside it too, which it copies with the rest. A command
//!   tied to a shape inside the source from outside it stays behind.
//! - The clone's own `trim_after` and `stop_after`, and a trim or a stop command tied to it, trim or stop
//!   after the last of what it sews. Its `min_stitch_length_mm` and `min_jump_stitch_length_mm` are given to
//!   everything it sews. Its other Ink/Stitch settings change nothing, and `SC-W0802` names them.
//!
//! Where Ink/Stitch sews a clone otherwise than this, the ledger says how (`DEV-SVG-021` to `DEV-SVG-024`).
//! Ink/Stitch was run on files written for this, with its stitch-plan cache off, which otherwise gave a clone
//! the stitches of an earlier file: clones turned, mirrored and scaled, in turned groups, with their own
//! `angle`, `flip_angle` and `clone`, of groups, of clones and of symbols; commands tied to a source, inside
//! it, at the root and on the clone; the use's style, presentation attributes, clip path and `context-fill`;
//! and the loops and units it fails on. Each ledger entry says what it did.
//!
//! **What is left out.** A use that names nothing, or an element that isn't SVG, shows nothing, as in a
//! viewer. A clone that shows itself, directly or through the clones in its source, is left out, as a viewer
//! leaves it out (`SC-W0804`), where Ink/Stitch fails. So is a clone inside more than 16 others (`SC-W0802`),
//! which bounds the reader's depth. Before it draws anything, a clone follows the clones in its source to see
//! whether it comes back to itself: a unit of work for each node, and the walk costs what the document's
//! does.

use std::ops::Range;

use roxmltree::Node;
use std::str::FromStr;
use stitchcraft_core::{Code, Diagnostic};
use stitchcraft_engine::common::{MIN_JUMP_STITCH_LENGTH, MIN_STITCH_LENGTH};
use stitchcraft_params::{Kind as ParamKind, ParamSet, Value};
use svgtypes::{AspectRatio, ViewBox};

use super::{Axis, Context, MAX_HOPS, Reader};
use crate::inkstitch::{
    self, ANGLE, CLONE, CLONE_SETTINGS, Does, FLIP_ANGLE, IGNORE_OBJECT, INKSTITCH_NS, Place, STOP_AFTER, TRIM_AFTER, Tied, place,
};
use crate::style::Style;
use crate::transform::{self, Affine};

/// A command that applies to a node, and whether its place moves with the clone that shows the node.
#[derive(Clone, Copy, Debug)]
pub(super) struct Applied {
    pub tied: Tied,
    pub moves: bool,
}

/// A clone whose content the reader is drawing.
#[derive(Clone, Debug)]
pub(super) struct Frame<'a, 'input> {
    /// What the clone is called, which what it shows is called after.
    pub label: String,
    /// The element it shows.
    pub source: Node<'a, 'input>,
    /// The places of the source and of everything in it.
    pub within: Range<Place>,
    /// The map from where the source is drawn to where the clone draws it, in millimetres, which moves the
    /// commands the clone copies; `None` when the source's place doesn't read.
    pub moved: Option<Affine>,
    /// The same map with the clone in its own place in the document, which turns the angles of its fills, as
    /// Ink/Stitch turns them one clone at a time.
    pub turn: Option<Affine>,
    /// The angle its `angle` setting gives every fill it shows.
    pub angle: Option<f64>,
    /// Whether its `flip_angle` setting reverses them.
    pub flip: bool,
}

/// The settings of a clone that the reader applies, but its own: Ink/Stitch's `ignore_object`, and what it
/// gives the stitches of a clone.
const APPLIED: [&str; 5] = [IGNORE_OBJECT, TRIM_AFTER, STOP_AFTER, MIN_STITCH_LENGTH, MIN_JUMP_STITCH_LENGTH];

impl<'a, 'input> Reader<'_, 'a, 'input> {
    /// Draws what the clone `node`, labelled `label`, shows, as the module's docs say: `map` takes its user
    /// space to millimetres, `style` and `clipped` are what it is drawn with, and `commands` the commands that
    /// apply to it.
    pub(super) fn cloned(
        &mut self,
        node: Node<'a, 'input>,
        label: &str,
        map: Affine,
        (style, clipped): (&Style<'a>, Option<usize>),
        commands: &[Applied],
    ) -> Result<(), Diagnostic> {
        self.unread_settings(node, label)?;
        // A link to nothing shows nothing, as in a viewer.
        let Some(source) = self.source(node) else { return Ok(()) };
        if node.attribute((INKSTITCH_NS, CLONE)).is_some_and(|value| !value.trim().is_empty() && !inkstitch::yes(value)) {
            let message = format!("`{label}` is left out, with all it shows: its Ink/Stitch setting `{CLONE}` is off.");
            self.warn(Code::SvgObjectIgnored, Some(label), message, None);
            return Ok(());
        }
        if self.frames.len() >= MAX_HOPS as usize {
            self.note(Some(label), format!("`{label}` is a clone inside more than {MAX_HOPS} clones, and what it shows is left out."), None);
            return Ok(());
        }
        if self.shows_itself(node, source)? {
            let message = format!("`{label}` shows itself, directly or through other clones, which a viewer doesn't draw, so it is left out.");
            self.unusable(label, message);
            return Ok(());
        }
        let (x, y) = (self.at(node, "x", Axis::X, label), self.at(node, "y", Axis::Y, label));
        let symbol = source.tag_name().name() == "symbol";
        let view = if symbol { self.view(node, source, label)? } else { Some(Affine::IDENTITY) };
        let Some(view) = view else { return Ok(()) };
        let content = map.after(Affine::translate(x, y)).after(view);
        // The source is drawn in its parent's user space, which the clone moves here. Ink/Stitch turns the
        // angles of a symbol's fills from the clone's parent's instead, and each clone's in its own place.
        let space = self.placed(source.parent())?;
        let moved = space.and_then(Affine::inverse).map(|back| content.after(back));
        let here = self.placed(Some(node))?.map(|here| here.after(Affine::translate(x, y)).after(view));
        let origin = if symbol { self.placed(node.parent())? } else { space };
        let turn = here.zip(origin.and_then(Affine::inverse)).map(|(here, back)| here.after(back));
        if moved.is_none() || turn.is_none() {
            let message = format!(
                "`{label}` shows `{}`, whose place in the drawing doesn't read: a transform round it, or round `{label}`, cannot be used or \
                 flattens it. The angles of its fills don't turn with the clone, and the commands tied to it stay where they are.",
                source.attribute("id").unwrap_or_default()
            );
            self.unusable(label, message);
        }
        let frame = Frame {
            label: label.to_string(),
            source,
            within: self.within(source)?,
            moved,
            turn,
            angle: self.own_angle(node, label),
            flip: node.attribute((INKSTITCH_NS, FLIP_ANGLE)).is_some_and(inkstitch::yes),
        };
        let Some(parent) = source.parent() else { return Ok(()) };
        let start = self.elements.len();
        self.frames.push(frame);
        let walked = self.walk(source.descendants(), parent.id(), Context::Shown { map: content, style: style.context(), clipped });
        self.frames.pop();
        walked?;
        self.sews(node, start, commands)
    }

    /// What Ink/Stitch gives the stitches of the clone `node`, which sews the elements from `start` on: its
    /// shortest stitch and jump, when it sets them, to each, and a trim or a stop after the last, which its
    /// settings or the `commands` tied to it ask for.
    fn sews(&mut self, node: Node<'a, 'input>, start: usize, commands: &[Applied]) -> Result<(), Diagnostic> {
        let asked = |key: &str, does: Does| {
            node.attribute((INKSTITCH_NS, key)).is_some_and(inkstitch::yes) || commands.iter().any(|applied| applied.tied.command.does == does)
        };
        let (trim, stop) = (asked(TRIM_AFTER, Does::Trim), asked(STOP_AFTER, Does::Stop));
        let lengths: Vec<(&str, &str)> =
            [MIN_STITCH_LENGTH, MIN_JUMP_STITCH_LENGTH].into_iter().filter_map(|key| Some((key, node.attribute((INKSTITCH_NS, key))?))).collect();
        let sewn = self.elements.get_mut(start..).unwrap_or_default();
        let count = sewn.len();
        for element in sewn.iter_mut() {
            for (key, value) in &lengths {
                element.params.set(*key, *value);
            }
        }
        if let Some(last) = sewn.last_mut() {
            for (key, on) in [(TRIM_AFTER, trim), (STOP_AFTER, stop)] {
                if on {
                    last.params.set(key, "true");
                }
            }
        }
        if count > 0 && self.frames.is_empty() {
            self.sewn_clones.insert(place(node));
        }
        self.charge(count as u64)
    }

    /// The commands that apply to `node`, where the reader meets it: in the document, every command tied to
    /// it, which stays where it is. In what a clone shows, those Ink/Stitch copies with it: every command tied
    /// to the clone's source, and of those tied to an element in it, the ones whose connector is in it too,
    /// each moving with the clone when its symbol is copied too. A unit of work for each command, in a clone.
    pub(super) fn commands(&mut self, node: Node<'a, 'input>) -> Result<Vec<Applied>, Diagnostic> {
        let tied = self.objects.on(node);
        let Some(frame) = self.frames.last() else { return Ok(tied.iter().map(|&tied| Applied { tied, moves: false }).collect()) };
        let source = frame.source == node;
        let applied: Vec<Applied> = tied
            .iter()
            .filter(|tied| source || frame.within.contains(&tied.connector))
            .map(|&tied| Applied { tied, moves: source || frame.within.contains(&tied.symbol_use) })
            .collect();
        self.charge(tied.len() as u64)?;
        Ok(applied)
    }

    /// `own`, the label of an element, as the reader calls it where it meets it: after the clone that shows
    /// it, if one does.
    pub(super) fn cloned_label(&self, own: String) -> String {
        match self.frames.last() {
            Some(frame) => format!("{}/{own}", frame.label),
            None => own,
        }
    }

    /// Turns the angle in `params`, those of a fill labelled `label`, as the clones that show it turn it, the
    /// innermost first: each takes it through its map, or gives its own `angle`, and reverses it with
    /// `flip_angle`, rounded to 6 decimal places, as Ink/Stitch writes it. An angle that doesn't read is left
    /// as it is, which refuses the fill (`SC-E0101`) where a clone gives none of its own. An `automatic` shape
    /// is digitized by the rules, and the clones' angle settings are named as not applied.
    pub(super) fn turned(&mut self, label: &str, params: &mut ParamSet, automatic: bool) {
        if automatic {
            if self.frames.iter().any(|frame| frame.angle.is_some() || frame.flip) {
                let message = format!(
                    "`{label}` is digitized automatically, which chooses its stitches: the Ink/Stitch settings `{ANGLE}` and `{FLIP_ANGLE}` \
                     of the clones that show it are not applied."
                );
                self.note(Some(label), message, None);
            }
            return;
        }
        if self.frames.is_empty() {
            return;
        }
        // A fill's angle is 0 unless it sets one, as Ink/Stitch reads it.
        let mut angle = match params.get(ANGLE).map(str::trim) {
            None | Some("") => Some(0.0),
            Some(text) => degrees(text),
        };
        for frame in self.frames.iter().rev() {
            let turned = frame.angle.or_else(|| angle.map(|degrees| frame.turn.map_or(degrees, |turn| turn.turn(degrees))));
            angle = turned.map(|degrees| rounded(if frame.flip { -degrees } else { degrees }));
        }
        if let Some(angle) = angle {
            params.set(ANGLE, angle.to_string());
        }
    }

    /// The clone `node`'s own `angle` setting: `None` when it sets none, and when it doesn't read, which
    /// `SC-W0802` names, as Ink/Stitch then turns the clone's fills.
    fn own_angle(&mut self, node: Node<'a, 'input>, label: &str) -> Option<f64> {
        let text = node.attribute((INKSTITCH_NS, ANGLE)).filter(|text| !text.trim().is_empty())?;
        let angle = degrees(text);
        if angle.is_none() {
            let message = format!(
                "The Ink/Stitch setting `{ANGLE}` of `{label}`, \"{text}\", is not an angle; the clone turns the angles of its fills with it, \
                 as Ink/Stitch reads it."
            );
            self.note(Some(label), message, None);
        }
        angle
    }

    /// `SC-W0802` naming the Ink/Stitch settings of the clone `node`, `label`, that change nothing: all but
    /// a clone's own ([`CLONE_SETTINGS`]) and those it gives what it sews. One unit of work for each.
    fn unread_settings(&mut self, node: Node<'a, 'input>, label: &str) -> Result<(), Diagnostic> {
        let read = |name: &str| CLONE_SETTINGS.iter().any(|setting| setting.name == name) || APPLIED.contains(&name);
        let names: Vec<String> =
            node.attributes().filter(|a| a.namespace() == Some(INKSTITCH_NS) && !read(a.name())).map(|a| format!("`{}`", a.name())).collect();
        self.charge(names.len() as u64)?;
        if !names.is_empty() {
            let message = format!(
                "The clone `{label}` has Ink/Stitch settings, {}, which Ink/Stitch doesn't read from a clone: they change nothing.",
                names.join(", ")
            );
            self.note(Some(label), message, Some("Set them on what the clone shows."));
        }
        Ok(())
    }

    /// The element the clone `node` names, `#` and its id, when it is an SVG element.
    fn source(&self, node: Node<'a, 'input>) -> Option<Node<'a, 'input>> {
        let id = inkstitch::href(node)?.trim().strip_prefix('#')?;
        self.by_id(id).filter(|source| self.is_svg(*source))
    }

    /// Whether drawing the clone `node`, which shows `source`, would draw `node` again: `source` is it or holds
    /// it, or shows it through the clones it holds. A unit of work for each node looked at.
    fn shows_itself(&mut self, node: Node<'a, 'input>, source: Node<'a, 'input>) -> Result<bool, Diagnostic> {
        let mut followed = std::collections::BTreeSet::new();
        let mut todo = vec![source];
        while let Some(next) = todo.pop() {
            for inner in next.descendants() {
                self.charge(1)?;
                if inner == node {
                    return Ok(true);
                }
                if self.is_svg(inner) && inner.tag_name().name() == "use" && self.objects.shown_by(inner).is_none() && followed.insert(place(inner)) {
                    todo.extend(self.source(inner));
                }
            }
        }
        Ok(false)
    }

    /// The places of `source` and of everything in it: up to the node after it in document order. A unit of
    /// work for each element looked at on the way.
    fn within(&mut self, source: Node<'a, 'input>) -> Result<Range<Place>, Diagnostic> {
        for node in source.ancestors() {
            self.charge(1)?;
            if let Some(next) = node.next_sibling() {
                return Ok(place(source)..place(next));
            }
        }
        Ok(place(source)..Place::MAX)
    }

    /// The map from the user space of the children of `node` to millimetres, as the document draws them:
    /// the transforms of `node` and of the elements round it, and the root's map. `None` when one of them
    /// doesn't read, or there is no `node`.
    fn placed(&mut self, node: Option<Node<'a, 'input>>) -> Result<Option<Affine>, Diagnostic> {
        let Some(node) = node else { return Ok(None) };
        let map = inkstitch::placed(node, &mut self.meter).map_err(|_| self.exhausted())?;
        Ok(map.map(|map| self.root.after(map)))
    }

    /// The length `name` of the clone `node`, `label`, in user units: 0 when it is missing, and when it
    /// doesn't read, which `SC-W0804` names, as a viewer reads it.
    fn at(&mut self, node: Node<'a, 'input>, name: &str, axis: Axis, label: &str) -> f64 {
        match self.length(node, name, axis) {
            Ok(value) => value.unwrap_or(0.0),
            Err(text) => {
                self.unusable(label, format!("The {name} of `{label}`, \"{text}\", is not a length; 0 is used, as a viewer uses it."));
                0.0
            }
        }
    }

    /// The map a symbol's viewport gives what the clone `node`, `label`, shows of `symbol`: its viewBox fitted
    /// to the clone's width and height, the symbol's own when the clone sets none (SVG 2), and the page's
    /// when neither does, as its `preserveAspectRatio` says. Without a viewBox, nothing. `None` when the
    /// viewport has no area, and a viewer draws nothing of it, which `SC-W0804` names. What doesn't read is
    /// ignored, as a viewer ignores it, and named.
    fn view(&mut self, node: Node<'a, 'input>, symbol: Node<'a, 'input>, label: &str) -> Result<Option<Affine>, Diagnostic> {
        let name = symbol.attribute("id").unwrap_or_default();
        let Some(text) = symbol.attribute("viewBox") else { return Ok(Some(Affine::IDENTITY)) };
        let Ok(vb) = ViewBox::from_str(text) else {
            let message = format!("The viewBox of `{name}`, which `{label}` shows, \"{text}\", is not 4 numbers of an area; it is ignored.");
            self.unusable(label, message);
            return Ok(Some(Affine::IDENTITY));
        };
        let aspect = match symbol.attribute("preserveAspectRatio").map(|text| (text, AspectRatio::from_str(text.trim()))) {
            None => AspectRatio::default(),
            Some((_, Ok(aspect))) => aspect,
            Some((text, Err(_))) => {
                let message = format!(
                    "The preserveAspectRatio of `{name}`, which `{label}` shows, \"{text}\", is not an aspect ratio SVG names; it is \
                     ignored, as a viewer ignores it."
                );
                self.unusable(label, message);
                AspectRatio::default()
            }
        };
        let (width, height) = (self.size(node, symbol, "width", Axis::X, label), self.size(node, symbol, "height", Axis::Y, label));
        if !(width > 0.0 && height > 0.0) {
            let message =
                format!("`{label}` gives the symbol `{name}` a viewport {width} by {height}, which a viewer draws nothing in; it is left out.");
            self.unusable(label, message);
            return Ok(None);
        }
        Ok(Some(transform::fit(width, height, vb, aspect)))
    }

    /// The side `name` of a symbol's viewport, in user units: the clone `node`'s, `label`, or the `symbol`'s
    /// own, or else 100 % of the page's. One that doesn't read is ignored, as a viewer ignores it, and named
    /// with the element whose it is.
    fn size(&mut self, node: Node<'a, 'input>, symbol: Node<'a, 'input>, name: &str, axis: Axis, label: &str) -> f64 {
        let whose = [format!("`{label}`,"), format!("`{}`, which `{label}` shows,", symbol.attribute("id").unwrap_or_default())];
        for (element, whose) in [node, symbol].into_iter().zip(whose) {
            match self.length(element, name, axis) {
                Ok(Some(value)) => return value,
                Ok(None) => {}
                Err(text) => {
                    let message = format!("The {name} of {whose} \"{text}\", is not a length; it is ignored, as a viewer ignores it.");
                    self.unusable(label, message);
                }
            }
        }
        self.reference(axis)
    }
}

/// An angle as an angle parameter reads it, in degrees; `None` when it doesn't read.
fn degrees(text: &str) -> Option<f64> {
    let Some((Value::Angle(degrees), _)) = ParamKind::Angle.parse(text) else { return None };
    Some(degrees)
}

/// `degrees` to 6 decimal places, as Ink/Stitch writes a clone's angles, and 0 rather than -0.
fn rounded(degrees: f64) -> f64 {
    (degrees * 1e6).round() / 1e6 + 0.0
}
