//! Clip paths (`REQ-SVG-011`): a shape that a `clip-path` cuts shows only inside it, and sews only what shows.
//!
//! A shape's `clip-path` property, and that of each group, `<a>` and `<switch>` round it, names a
//! `<clipPath>`, and the shape shows only inside every one. Each becomes a [`Clip`] of the shape's design
//! elements, which the engine cuts them to (`REQ-GEN-005`). A clip path's area is its children's together, as
//! a viewer takes them in:
//!
//! - Each path or basic shape, filled by its `clip-rule`, which it inherits from the clip path, under its own
//!   transform and the clip path's, in the user space of the element the clip path cuts: that element's own
//!   transform included, or its box with `clipPathUnits="objectBoundingBox"`. Its paint doesn't matter.
//! - A `<use>` of a path or basic shape, at the use's `x` and `y`.
//! - A group's children, as Inkscape draws them and Ink/Stitch reads them. SVG allows no group in a clip path,
//!   and a browser leaves one out, but Inkscape puts the group it clips with in one, as it is.
//! - Not a child that isn't displayed (`display: none`, `visibility: hidden`), nor a line, which has no area,
//!   nor text, which StitchCraft doesn't read and `SC-W0802` names.
//!
//! A clip path's own `clip-path` cuts it in turn, as another clip of the shape; one on a child isn't applied,
//! and `SC-W0802` names it. A clip path with no area leaves nothing of the shape, which sews nothing then
//! (`SC-W0013`), as a viewer shows nothing of it. A `clip-path` that names nothing, or names an element that
//! isn't a clip path, clips nothing, as in a viewer, and one that comes back to a clip path on its way, a
//! loop, clips no further, which `SC-W0802` names. So does `objectBoundingBox` on a group: the box of what a
//! group holds isn't known where the reader meets it. Ink/Stitch builds a clip otherwise in places, which the
//! ledger lists (`DEV-SVG-016` to `DEV-SVG-020`).
//!
//! The clips round an element are a chain the reader keeps ([`Link`]): each element with a clip path adds a
//! link to the chain its parent's children are drawn in, and a shape's elements take every clip along it.
//! Reading a clip path costs what reading its children as shapes costs, each time an element names it.

use roxmltree::Node;
use stitchcraft_core::Diagnostic;
use stitchcraft_engine::design::{Clip, ClipShape};

use super::{Axis, Geometry, Kind, MAX_HOPS, Reader};
use crate::inkstitch;
use crate::path;
use crate::style::{self, Declared, Style, names_something};
use crate::transform::{self, Affine};

/// A link of the chain of clips round an element: a clip, and the link of the clips round the element that
/// has it, if any.
#[derive(Clone, Debug)]
pub(super) struct Link {
    clip: Clip,
    up: Option<usize>,
}

/// The box of an element in its user units, as [`path::bounds`] gives it: the least x and y, the width and
/// the height.
pub(super) type Bbox = (f64, f64, f64, f64);

impl<'a, 'input> Reader<'_, 'a, 'input> {
    /// The chain `up` with the clips that the group `label` declares with its `clip-path` added, each a link,
    /// or `up` itself when it declares none: [`Reader::clips`], in the user space `map` takes to millimetres.
    pub(super) fn clipped(
        &mut self,
        label: &str,
        declared: &Declared<'a, 'input>,
        map: Affine,
        up: Option<usize>,
    ) -> Result<Option<usize>, Diagnostic> {
        let mut chain = up;
        for clip in self.clips(label, declared, map, None)? {
            self.links.push(Link { clip, up: chain });
            chain = Some(self.links.len() - 1);
        }
        Ok(chain)
    }

    /// Every clip along the chain from `link`, nearest first: what an element drawn in it is cut to. One unit
    /// of work for each.
    pub(super) fn chain(&mut self, mut link: Option<usize>) -> Result<Vec<Clip>, Diagnostic> {
        let mut clips = Vec::new();
        while let Some(Link { clip, up }) = link.and_then(|at| self.links.get(at)) {
            clips.push(clip.clone());
            link = *up;
            self.charge(1)?;
        }
        Ok(clips)
    }

    /// The clips that the `clip-path` property of `label`, as `declared`, cuts it to: the clip path it names,
    /// then each that clip path's own `clip-path` names, in the user space `map` takes to millimetres, that of
    /// its children, and with `bbox`, its box in those units, when it is a shape. None when it names no clip
    /// path, as in a viewer.
    pub(super) fn clips(&mut self, label: &str, declared: &Declared<'a, 'input>, map: Affine, bbox: Option<Bbox>) -> Result<Vec<Clip>, Diagnostic> {
        let Some(value) = declared.get("clip-path").filter(|value| names_something(value)) else { return Ok(Vec::new()) };
        let Some((mut id, "")) = style::reference(value) else {
            let message = format!("The clip path of `{label}`, \"{value}\", is not a reference to a clip path; it is not applied.");
            self.note(Some(label), message, Some("Clip it with a path (in Inkscape: Object › Clip › Set Clip)."));
            return Ok(Vec::new());
        };
        let mut clips = Vec::new();
        let mut seen: Vec<&str> = Vec::new();
        loop {
            if seen.contains(&id) || u32::try_from(seen.len()).map_or(true, |hops| hops >= MAX_HOPS) {
                let message = format!("The clip paths of `{label}` refer to each other in a loop, at `{id}`; it is cut no further.");
                self.note(Some(label), message, None);
                break;
            }
            seen.push(id);
            let Some(clip_path) = self.by_id(id).filter(|n| self.is_svg(*n) && n.tag_name().name() == "clipPath") else { break };
            if let Some(clip) = self.area(clip_path, id, label, map, bbox)? {
                clips.push(clip);
            }
            let declared = self.declared(clip_path)?;
            match declared.get("clip-path").filter(|value| names_something(value)).and_then(style::reference) {
                Some((next, "")) => id = next,
                _ => break,
            }
        }
        Ok(clips)
    }

    /// The area of the clip path `clip_path`, `id`, cutting `label`, whose user space `map` takes to
    /// millimetres and whose box is `bbox`, if it is a shape: its children's shapes together. `None`, with
    /// `SC-W0802`, when it can't be placed: in `objectBoundingBox` units round a group, whose box isn't known,
    /// or under a transform that doesn't read.
    fn area(&mut self, clip_path: Node<'a, 'input>, id: &str, label: &str, map: Affine, bbox: Option<Bbox>) -> Result<Option<Clip>, Diagnostic> {
        let mut base = map;
        if clip_path.attribute("clipPathUnits").is_some_and(|units| units.trim() == "objectBoundingBox") {
            let Some((x, y, width, height)) = bbox else {
                let message = format!(
                    "The clip path `{id}` of `{label}` is in units of the box round what `{label}` holds, which StitchCraft doesn't measure; it \
                     is not applied."
                );
                self.note(Some(label), message, Some("Set the clip path's units to user space (clipPathUnits=\"userSpaceOnUse\")."));
                return Ok(None);
            };
            base = base.after(Affine::translate(x, y)).after(Affine::scale(width, height));
        }
        let Some(base) = self.transformed(clip_path, id, label, base)? else { return Ok(None) };
        // The `display` of a clip path doesn't apply to it; the properties its children inherit do.
        let declared = self.declared(clip_path)?;
        let style = Style::default().child(&declared).unwrap_or_default();
        let mut shapes = Vec::new();
        self.shapes_in(clip_path, (id, label), base, &style, &mut shapes, 0)?;
        Ok(Some(Clip { shapes: shapes.into() }))
    }

    /// `map` after the transform of `node`, a clip path or one of its children, `id` in clip path, cutting
    /// `label`; `None`, with `SC-W0802`, when the transform doesn't read.
    fn transformed(&mut self, node: Node<'a, 'input>, id: &str, label: &str, map: Affine) -> Result<Option<Affine>, Diagnostic> {
        let Some(text) = node.attribute("transform") else { return Ok(Some(map)) };
        self.charge_text(text)?;
        match transform::parse(text) {
            Ok(own) => Ok(Some(map.after(own))),
            Err(reason) => {
                let message = format!("The transform of `{id}` in the clip path of `{label}` cannot be used ({reason}); it is left out of it.");
                self.note(Some(label), message, None);
                Ok(None)
            }
        }
    }

    /// The shapes of the children of `parent`, a clip path or a group in one, `(id, label)` the clip path's id
    /// and the label of what it cuts, in `shapes`: each child under `map` after its own transform, drawn with
    /// the style it inherits from `inherited`. A group's children are taken in as Inkscape draws them, down to
    /// [`MAX_HOPS`] groups deep.
    fn shapes_in(
        &mut self,
        parent: Node<'a, 'input>,
        (id, label): (&str, &str),
        map: Affine,
        inherited: &Style<'a>,
        shapes: &mut Vec<ClipShape>,
        depth: u32,
    ) -> Result<(), Diagnostic> {
        for child in parent.children() {
            self.charge(1)?;
            if !self.is_svg(child) {
                continue;
            }
            let declared = self.declared(child)?;
            let Some(style) = inherited.child(&declared) else { continue };
            let tag = child.tag_name().name();
            let Some(map) = self.transformed(child, id, label, map)? else { continue };
            match (tag, Kind::of(tag)) {
                ("g" | "a", _) if depth < MAX_HOPS => self.shapes_in(child, (id, label), map, &style, shapes, depth + 1)?,
                ("g" | "a", _) => {
                    let message = format!(
                        "The clip path `{id}` of `{label}` holds groups more than {MAX_HOPS} deep, and what lies deeper adds nothing to the clip."
                    );
                    self.note(Some(label), message, None);
                }
                (_, Some(Kind::Shape(geometry))) => self.clip_shape(child, geometry, (id, label), map, &style, &declared, shapes)?,
                ("use", _) => self.used(child, (id, label), map, &style, shapes)?,
                ("text" | "flowRoot", _) if style.visible => {
                    let message =
                        format!("The clip path `{id}` of `{label}` holds text, which StitchCraft doesn't read; it adds nothing to the clip.");
                    self.note(Some(label), message, Some("Convert the text to paths first (in Inkscape: Path › Object to Path)."));
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// The shape a `<use>` in a clip path, `node`, places, in `shapes`: the path or basic shape it refers to,
    /// at its `x` and `y` under `map`, which takes its own transform. A use of anything else adds nothing,
    /// which `SC-W0802` names.
    fn used(
        &mut self,
        node: Node<'a, 'input>,
        (id, label): (&str, &str),
        map: Affine,
        inherited: &Style<'a>,
        shapes: &mut Vec<ClipShape>,
    ) -> Result<(), Diagnostic> {
        let target = inkstitch::href(node).and_then(|href| href.trim().strip_prefix('#')).and_then(|target| self.by_id(target));
        let Some(target) = target.filter(|t| self.is_svg(*t)) else { return Ok(()) };
        let Some(Kind::Shape(geometry)) = Kind::of(target.tag_name().name()) else {
            let message =
                format!("The clip path `{id}` of `{label}` uses an element that isn't a path or a basic shape; it adds nothing to the clip.");
            self.note(Some(label), message, None);
            return Ok(());
        };
        let at = |reader: &Self, name, axis| reader.length(node, name, axis).ok().flatten().unwrap_or(0.0);
        let map = map.after(Affine::translate(at(self, "x", Axis::X), at(self, "y", Axis::Y)));
        let declared = self.declared(target)?;
        let Some(style) = inherited.child(&declared) else { return Ok(()) };
        let Some(map) = self.transformed(target, id, label, map)? else { return Ok(()) };
        self.clip_shape(target, geometry, (id, label), map, &style, &declared, shapes)
    }

    /// The shape of `node`, a path or basic shape in a clip path, in `shapes`: its outline under `map`, filled
    /// by its `clip-rule`, when it is visible and has an inside. A line has none.
    #[allow(clippy::too_many_arguments)]
    fn clip_shape(
        &mut self,
        node: Node<'a, 'input>,
        geometry: Geometry,
        (id, label): (&str, &str),
        map: Affine,
        style: &Style<'a>,
        declared: &Declared<'a, 'input>,
        shapes: &mut Vec<ClipShape>,
    ) -> Result<(), Diagnostic> {
        if !style.visible || geometry == Geometry::Line {
            return Ok(());
        }
        if declared.uses("clip-path") {
            let message = format!("A shape in the clip path `{id}` of `{label}` is clipped itself, which is not applied: all of it clips.");
            self.note(Some(label), message, None);
        }
        let own = node.attribute("id").map_or_else(|| format!("{id}/{}", node.tag_name().name()), str::to_string);
        let Some(subs) = self.outline(node, geometry, &own)? else { return Ok(()) };
        match path::to_mm(&subs, map) {
            Ok(path) => shapes.push(ClipShape { path, rule: style.clip_rule }),
            Err(reason) => self.unusable(&own, format!("`{own}` in the clip path of `{label}` cannot be used: {reason}. It is left out of it.")),
        }
        Ok(())
    }
}
