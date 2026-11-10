//! Ink/Stitch's guide lines and anchor lines (`REQ-SVG-003`, `REQ-RIP-012` to `REQ-RIP-014`): the helper
//! paths that steer the shapes grouped with them, read for those shapes' elements.
//!
//! Read at Ink/Stitch `d59c9ab` (`lib/marker.py`, `lib/elements/stroke.py`, `lib/elements/element.py`):
//! Ink/Stitch looks for a shape's helpers among the children of the group the shape is in, and nowhere
//! else, so a helper at the document's root steers nothing. A helper is a shape whose `style` attribute
//! starts it with Ink/Stitch's guide-line or anchor-line marker ([`Helper::of`]) and has a stroke.
//!
//! - **The guide line** of a stroke is the first such guide that is a satin column, its `satin_column`
//!   setting on, or else the first that is not. Its path in millimetres, under its own transform and the
//!   group's, its stroke's width and join, and its settings go to the shape's stroke. The engine reads it as
//!   a line, or as a satin column where it is one or has several subpaths.
//! - **The guide line** of a fill is the first that is not a satin column, as Ink/Stitch's fill looks only
//!   at those (`REQ-SVG-015`). A guided fill copies its first subpath.
//! - **The anchor line** is the first anchor that is not a satin column: the start and the end of its first
//!   subpath, which set where a satin guide places each copy's ends.
//!
//! A helper steers its shapes even when `display: none` hides it, as Ink/Stitch at `d59c9ab` follows a
//! hidden guide line, run as a black box. A group's children are looked at once, to find whether any is a
//! helper, and only a group that has one is read again for each shape in it. A helper whose path or
//! transform can't be used is left out, as a viewer leaves it out.

use roxmltree::Node;
use stitchcraft_core::{Diagnostic, Mm};
use stitchcraft_engine::design::{Guide, Hints};
use stitchcraft_params::ParamSet;

use super::{Kind, Reader};
use crate::inkstitch::{self, Helper, INKSTITCH_NS, place};
use crate::path;
use crate::style::{Paint, Plain, Style};
use crate::transform::{self, Affine};

/// The setting that makes a helper a satin column.
const SATIN_COLUMN: &str = "satin_column";

impl<'a, 'input> Reader<'_, 'a, 'input> {
    /// `hints` with the guide line and the anchor line that steer the stroke of the shape `node`, found among
    /// the children of its group, which is drawn with the map `map` and the style `style`, and the guide line
    /// that steers its fill.
    pub(super) fn guided(
        &mut self,
        node: Node<'a, 'input>,
        (map, style): (Affine, &Style<'a>),
        hints: &mut Hints,
    ) -> Result<Option<Guide>, Diagnostic> {
        let Some(group) = node.parent_element().filter(|parent| self.is_svg(*parent) && parent.tag_name().name() == "g") else { return Ok(None) };
        if !self.has_helpers(group)? {
            return Ok(None);
        }
        let (mut satin, mut line, mut anchor) = (None, None, None);
        for child in group.children().filter(Node::is_element) {
            self.charge(1)?;
            let Some(Kind::Shape(geometry)) = Kind::of(child.tag_name().name()).filter(|_| self.is_svg(child)) else { continue };
            let declared = self.declared(child)?;
            let Some(helper @ (Helper::GuideLine | Helper::AnchorLine)) = Helper::of(&declared) else { continue };
            let own = style.inherited(&declared);
            // A helper without a stroke steers nothing.
            if matches!(own.stroke, Paint::Plain(Plain::None)) {
                continue;
            }
            let is_satin = child.attribute((INKSTITCH_NS, SATIN_COLUMN)).is_some_and(inkstitch::yes);
            let wanted = match helper {
                Helper::AnchorLine => anchor.is_none() && !is_satin,
                _ if is_satin => satin.is_none(),
                _ => line.is_none(),
            };
            if !wanted {
                continue;
            }
            let label = child.attribute("id").unwrap_or(child.tag_name().name()).to_string();
            let Some(drawn) = self.helper_path(child, geometry, &label, map)? else { continue };
            if helper == Helper::AnchorLine {
                let Some(first) = drawn.0.subpaths.first() else { continue };
                let end = if first.closed { first.start } else { first.segments.last().map_or(first.start, |segment| segment.end()) };
                anchor = Some([first.start, end]);
                continue;
            }
            let params: ParamSet = child.attributes().filter(|a| a.namespace() == Some(INKSTITCH_NS)).map(|a| (a.name(), a.value())).collect();
            let guide = Guide { path: drawn.0, width: self.stroke_width(&own, drawn.1).unwrap_or(Mm::ZERO), join: own.join(), params };
            if is_satin {
                satin = Some(guide);
            } else {
                line = Some(guide);
            }
        }
        hints.guide = satin.or_else(|| line.clone());
        hints.anchor = anchor;
        Ok(line)
    }

    /// Whether any child of `group` is a guide line or an anchor line, looked at once for each group: one unit
    /// of work for each child.
    fn has_helpers(&mut self, group: Node<'a, 'input>) -> Result<bool, Diagnostic> {
        let id = place(group);
        if let Some(&has) = self.helper_groups.get(&id) {
            return Ok(has);
        }
        let mut has = false;
        for child in group.children().filter(Node::is_element) {
            self.charge(1)?;
            let declared = self.declared(child)?;
            has |= matches!(Helper::of(&declared), Some(Helper::GuideLine | Helper::AnchorLine));
        }
        self.helper_groups.insert(id, has);
        Ok(has)
    }

    /// The path of the helper `node`, `label`, a shape of `geometry`, in millimetres under its own transform
    /// and its group's `map`, with the map; `None` when its transform or its path can't be used.
    fn helper_path(
        &mut self,
        node: Node<'a, 'input>,
        geometry: super::Geometry,
        label: &str,
        map: Affine,
    ) -> Result<Option<(stitchcraft_engine::design::Path, Affine)>, Diagnostic> {
        let map = match node.attribute("transform") {
            None => map,
            Some(text) => {
                self.charge_text(text)?;
                match transform::parse(text) {
                    Ok(own) if !map.after(own).flattens() => map.after(own),
                    _ => return Ok(None),
                }
            }
        };
        let Some(subs) = self.outline(node, geometry, label)? else { return Ok(None) };
        let points = path::to_mm(&subs, map).ok().filter(|drawn| drawn.points().next().is_some());
        Ok(points.map(|drawn| (drawn, map)))
    }
}
