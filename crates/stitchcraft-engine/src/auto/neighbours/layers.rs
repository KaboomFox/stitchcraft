//! The layers of a traced picture flattened before it is digitized (`REQ-AUTO-023`), said with `SC-I0908`.
//! Design: `docs/src/design/image-to-embroidery.md` › Layers.
//!
//! VectorCraft's Image Trace draws a picture's colours one of 2 ways. Its Overlapping method stacks each
//! colour whole under the colours drawn over it, and its Abutting method cuts a hole in each area for each
//! shape on it. Digitized as drawn, a stacked colour's stitches would be chosen for its whole shape, most of
//! it hidden, and a broad area with details on it splits into narrow columns between them, traced either
//! way. A digitizer fills such an area whole and sews the details on top. So before anything is digitized,
//! each automatic area is given what it sews, in 3 steps:
//!
//! 1. **What shows** ([`shown`]). The automatic areas drawn after an area are taken from it where they
//!    overlap it by more than [`TOUCHING`], as the order measures an overlap (`order::overlap`). A part of
//!    what is left no wider than [`TOUCHING`] anywhere is a hairline that rounding leaves where edges drawn
//!    as one meet, and shows nothing ([`hairlines`]). One that they cover whole sews nothing (`SC-I0904`).
//!    The picture's background is left out but for what shows of it inside the picture: its parts that come
//!    no nearer than the margin to the box's sides (`background`), as an abutting trace draws such a part as
//!    a shape of its own.
//! 2. **Details** ([`details`]). An area wider than its widest satin column somewhere, as it shows, has
//!    details: the automatic areas that fill one of its holes, what they leave of it no wider than
//!    [`TOUCHING`] or smaller than the area's smallest area. The hole is closed, so that the area's
//!    stitches are chosen for it whole. An area no wider than a satin column everywhere, such as a letter or
//!    an outline, keeps its holes: closed, it would be a broad fill under the shape inside it. Only the parts
//!    whose boxes meet the hole's are looked at, so a hole costs the work of the parts near it, not of the
//!    whole picture.
//! 3. **Moved back** ([`moved`]). An area sews before its details, so that they lie on it and the steps
//!    after this one cut it away under them (`under`). It moves back in paint order past its details, past
//!    the automatic areas whose part that shows doesn't overlap it, and past the other shapes whose boxes
//!    don't come within [`TOUCHING`] of its box, but never past a stop, nor when it has a stop after it. The
//!    areas move the largest first, so that an area moves before the areas inside it, which may then follow
//!    it. Once all have moved, a hole whose details don't all sew after its area stays open: sewn after
//!    them, the area would cover them. The area couldn't move back past them, or another area that holds one
//!    of them moved back past it.
//!
//! An area whose region changed is given its new outline. One whose region is what it was keeps its path as
//! drawn, so that a trace whose colours abut sews as before but for the holes its details fill.
//!
//! **Work.** Each area has the budget's work to itself for each step: what shows of it, its holes, its move
//! and what it is given to sew. It costs a unit for each shape and each part it looks past, and the work of
//! those near it. One that needs more for a step does without it, and that is said: one whose part that
//! shows can't be found is sewn as it is drawn, in its place, or left out whole when it is the background;
//! one whose holes can't be found keeps them open; one that can't be moved keeps its place; and one whose
//! region can't be given is sewn as it is drawn.

use std::borrow::Cow;
use std::collections::BTreeSet;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Meter, Mm};

use super::background::{BACKGROUND_MARGIN, inside};
use super::order::{drawn, overlap};
use super::under::covered;
use super::{Failed, TOUCHING, envelope, joined, near, no_area, parts_of, speck};
use crate::auto::narrow::within;
use crate::auto::{AutoParams, outline};
use crate::design::{Design, Element, FillRule, Shape};
use crate::generate::{stops_after, trim_or_stop_left_out};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops::{area_of, buffered, difference, intersection, union};

/// The design's elements as the layers are flattened ([`flattened`]).
pub(in crate::auto) struct Flattened<'d> {
    /// The elements that sew, in the order they are digitized, each with its place in the design: an area
    /// moved back under its details, and given what it sews when that isn't its region as drawn.
    pub(in crate::auto) elements: Vec<(usize, Cow<'d, Element>)>,
    /// How many parts of the picture's background are sewn, which show inside the picture: none when it is
    /// left out whole.
    pub(in crate::auto) kept: usize,
    /// What is said: `SC-I0904` for each area covered whole, `SC-W0505` for a trim or a stop after one,
    /// `SC-I0908` for each area whose layers need more work than the budget allows, and `SC-I0908` counting
    /// the areas that sew only what shows of them and those digitized whole under their details.
    pub(in crate::auto) said: Vec<Diagnostic>,
}

/// An automatic area as the layers are flattened.
struct Layer {
    /// Its settings of the rules.
    rules: AutoParams,
    /// Its region as drawn.
    drawn: Vec<Polygon>,
    /// The box round its region as drawn.
    bounds: Envelope,
    /// What shows of it: its region less the areas drawn after it.
    shown: Vec<Polygon>,
    /// Its holes closed under its details.
    closed: Vec<Hole>,
    /// Whether its layers need more work to find than the budget allows: it is sewn as drawn, in its place.
    unfound: bool,
}

/// A hole of an area that its details fill.
struct Hole {
    /// The place of the part it is a hole of among the parts of what shows of the area.
    part: usize,
    /// Its place among that part's holes.
    hole: usize,
    /// The places in the design of the areas that fill it.
    details: BTreeSet<usize>,
}

/// The elements of `design`, each automatic area's settings of the rules in `rules`, as the layers are
/// flattened ([the module](self)), its lowest shape left out as the picture's `background` but for what of
/// it shows inside the picture. Each area has `budget`'s work to itself.
pub(in crate::auto) fn flattened<'d>(design: &'d Design, rules: &[Option<AutoParams>], background: bool, budget: &Budget) -> Flattened<'d> {
    let elements = design.elements();
    let mut layers: Vec<Option<Layer>> =
        elements.iter().enumerate().map(|(index, element)| layer(element, rules.get(index).and_then(Option::as_ref), budget)).collect();
    let mut said = Vec::new();
    // What shows of each area, found from the areas as drawn.
    let found: Vec<Result<Option<Vec<Polygon>>, Failed>> = (0..layers.len()).map(|index| shown(&layers, index, &mut budget.meter())).collect();
    for ((index, layer), found) in layers.iter_mut().enumerate().zip(found) {
        let Some(layer) = layer else { continue };
        match found {
            Ok(Some(parts)) => layer.shown = parts,
            Ok(None) => {}
            Err(failed) => {
                let left_out = background && index == 0;
                let consequence =
                    if left_out { "it is left out whole as the picture's background" } else { "it is sewn as it is drawn, in its place" };
                said.extend(id(elements, index).map(|id| failed.said(Code::AutoLayers, "what of this area shows", consequence, budget.max_work, id)));
                layer.unfound = true;
            }
        }
    }
    let kept = if background { inner(design, &mut layers) } else { 0 };
    // Each area's holes that its details fill, found from what shows of the areas.
    let boxes = boxes(&layers);
    let holes: Vec<Result<Vec<Hole>, Failed>> = (0..layers.len()).map(|index| details(&layers, &boxes, index, &mut budget.meter())).collect();
    for ((index, layer), holes) in layers.iter_mut().enumerate().zip(holes) {
        let Some(layer) = layer.as_mut().filter(|layer| !layer.unfound) else { continue };
        match holes {
            Ok(holes) => layer.closed = holes,
            Err(failed) => {
                said.extend(
                    id(elements, index).map(|id| failed.said(Code::AutoLayers, "the holes of this area", "it keeps them", budget.max_work, id)),
                );
            }
        }
    }
    // The elements that sew, in paint order: an area covered whole sews nothing, and so does the background
    // when nothing of it shows inside the picture.
    let mut order = Vec::with_capacity(elements.len());
    for (index, element) in elements.iter().enumerate() {
        let lowest = background && index == 0;
        match layers.get(index).and_then(Option::as_ref) {
            Some(layer) if !layer.shown.is_empty() => order.push(index),
            None if !lowest => order.push(index),
            Some(_) if !lowest => {
                said.push(covered(element.id.clone()));
                said.extend(trim_or_stop_left_out(element).map(|left| left.with_element(element.id.clone())));
            }
            // The background, left out whole.
            _ => {}
        }
    }
    moved(elements, &mut layers, &mut order, budget, &mut said);
    let (mut flattened, mut whole) = (0_usize, 0_usize);
    let mut sewing = Vec::with_capacity(order.len());
    for index in order {
        let Some(element) = elements.get(index) else { continue };
        let layer = layers.get(index).and_then(Option::as_ref).filter(|layer| !layer.unfound);
        let Some(layer) = layer else {
            sewing.push((index, Cow::Borrowed(element)));
            continue;
        };
        let given = given(layer, &mut budget.meter());
        // An area whose holes are closed is digitized whole under its details, drawn whole or not.
        whole += usize::from(given.is_ok() && !layer.closed.is_empty());
        match given {
            Ok(Given::Drawn) => sewing.push((index, Cow::Borrowed(element))),
            Ok(Given::Region { parts, hidden }) => {
                flattened += usize::from(hidden && !(background && index == 0));
                let shape = Shape::Fill { path: outline(&parts), rule: FillRule::NonZero };
                sewing.push((index, Cow::Owned(Element { shape, ..element.clone() })));
            }
            Err(failed) => {
                said.push(failed.said(Code::AutoLayers, "what this area sews", "it is sewn as it is drawn", budget.max_work, element.id.clone()));
                sewing.push((index, Cow::Borrowed(element)));
            }
        }
    }
    said.extend(summary(flattened, whole));
    Flattened { elements: sewing, kept, said }
}

/// The id of element `index` of `elements`.
fn id(elements: &[Element], index: usize) -> Option<ElementId> {
    elements.get(index).map(|element| element.id.clone())
}

/// `element` as a layer when it is an automatic area, whose settings of the rules are `rules`, with its
/// region built with `budget`'s work to itself. An area that needs more, whose region the region code fails
/// to join, or that bounds no area, is no layer: it is sewn as it is drawn, and its fill says what it says
/// of it then.
fn layer(element: &Element, rules: Option<&AutoParams>, budget: &Budget) -> Option<Layer> {
    let (true, Some(rules), Shape::Fill { path, rule }) = (element.automatic, rules, &element.shape) else { return None };
    let drawn = parts_of(path, *rule, &mut budget.meter()).ok()?.ok().filter(|parts| !parts.is_empty())?;
    let bounds = envelope(&drawn)?;
    Some(Layer { rules: rules.clone(), shown: drawn.clone(), drawn, bounds, closed: Vec::new(), unfound: false })
}

/// What shows of the area `index` of `layers` under the areas drawn after it, its [`hairlines`] dropped,
/// with `meter`'s work: a unit for each shape drawn after it, and the work of those near it. `None` when
/// they overlap it nowhere by more than [`TOUCHING`], and it shows whole; no parts when they cover it whole.
fn shown(layers: &[Option<Layer>], index: usize, meter: &mut Meter) -> Result<Option<Vec<Polygon>>, Failed> {
    let Some(Some(layer)) = layers.get(index) else { return Ok(None) };
    let mut over = Vec::new();
    for other in layers.iter().skip(index + 1) {
        meter.charge(1)?;
        if let Some(other) = other.as_ref().filter(|other| near(&layer.bounds, TOUCHING, &other.bounds)) {
            over.extend_from_slice(&other.drawn);
        }
    }
    if over.is_empty() {
        return Ok(None);
    }
    let over = joined(union([over.as_slice()], meter))?;
    if !overlap(&layer.drawn, &over, meter)? {
        return Ok(None);
    }
    let parts = joined(difference(&layer.drawn, &over, meter))?;
    Ok(Some(hairlines(parts, meter)?))
}

/// `parts` but for those no wider than [`TOUCHING`] anywhere, measured as a narrow part is (`narrow::within`),
/// with `meter`'s work: hairlines that rounding leaves where edges drawn as one meet, as along the rim of a
/// disc drawn a hair larger than the disc drawn on it. A part whose width can't be found (a bug, which its
/// stitches say) is kept.
fn hairlines(parts: Vec<Polygon>, meter: &mut Meter) -> Result<Vec<Polygon>, Failed> {
    // TOUCHING is a finite length, so this is never left: no part is dropped then.
    let Ok(touching) = Mm::new(TOUCHING) else { return Ok(parts) };
    let mut kept = Vec::with_capacity(parts.len());
    for part in parts {
        if within(&part, touching, meter)? != Some(true) {
            kept.push(part);
        }
    }
    Ok(kept)
}

/// Keeps of the background, the lowest of `layers`, what shows of it inside the picture of `design`: its
/// parts that come no nearer than [`BACKGROUND_MARGIN`] to the sides of the box round the design. Gives how
/// many parts are kept.
fn inner(design: &Design, layers: &mut [Option<Layer>]) -> usize {
    let Some(Some(lowest)) = layers.first_mut() else { return 0 };
    let sides = design.bounds().and_then(|bounds| inside(bounds, BACKGROUND_MARGIN)).map(|inner| Envelope::of(&inner.outline));
    let Some(sides) = sides.filter(|_| !lowest.unfound) else {
        lowest.shown.clear();
        return 0;
    };
    lowest.shown.retain(|part| sides.covers(&Envelope::of(&part.outline)));
    lowest.shown.len()
}

/// The box round each part that shows of each of `layers`, by place, so that a hole is filled only from the
/// parts near it ([`details`]).
fn boxes(layers: &[Option<Layer>]) -> Vec<Vec<Envelope>> {
    layers
        .iter()
        .map(|layer| layer.as_ref().map_or_else(Vec::new, |layer| layer.shown.iter().map(|part| Envelope::of(&part.outline)).collect()))
        .collect()
}

/// The holes of what shows of the area `index` of `layers` that the areas fill, each with those areas, its
/// details, with `meter`'s work: a unit for each area and each part looked at for each hole, and the work of
/// the parts whose boxes, in `boxes` ([`boxes`]), meet the hole's. None when the area is no wider than its
/// widest satin column everywhere, nor for a part of it that narrow. A speck in a hole is no detail: a hole
/// that only specks fill stays open, and the specks merge into the area round them (`specks`), which closes
/// it as a speck does. An area whose layers weren't found, sewn as drawn in its place, has no holes closed
/// and fills none.
fn details(layers: &[Option<Layer>], boxes: &[Vec<Envelope>], index: usize, meter: &mut Meter) -> Result<Vec<Hole>, Failed> {
    let Some(layer) = layers.get(index).and_then(Option::as_ref).filter(|layer| !layer.unfound) else { return Ok(Vec::new()) };
    let mut holes = Vec::new();
    for (p, part) in layer.shown.iter().enumerate() {
        // A part as narrow as a satin column everywhere, or whose width can't be found (a bug, which its
        // stitches say), keeps its holes.
        if part.holes.is_empty() || within(part, layer.rules.auto_max_satin_width_mm, meter)? != Some(false) {
            continue;
        }
        for (h, ring) in part.holes.iter().enumerate() {
            // The hole as a part of its own, its ring the other way round.
            let hole = [Polygon { outline: ring.iter().rev().copied().collect(), holes: Vec::new() }];
            let around = Envelope::of(ring);
            // The parts whose boxes meet the hole's, but for the part it is a hole of, and for each other area
            // where its parts lie among them.
            let (mut filling, mut near_it) = (Vec::new(), Vec::new());
            for (other, layer) in layers.iter().enumerate() {
                meter.charge(1)?;
                let Some(layer) = layer.as_ref().filter(|layer| !layer.unfound && near(&around, 0.0, &layer.bounds)) else { continue };
                let first = filling.len();
                for (q, (piece, bounds)) in layer.shown.iter().zip(boxes.get(other).map_or(&[][..], Vec::as_slice)).enumerate() {
                    meter.charge(1)?;
                    if (other, q) != (index, p) && near(&around, 0.0, bounds) {
                        filling.push(piece.clone());
                    }
                }
                if other != index && filling.len() > first {
                    near_it.push((other, first..filling.len(), layer.rules.auto_min_area_mm2));
                }
            }
            // What the areas leave of it: none wider than TOUCHING, but for specks.
            let left = joined(difference(&hole, &joined(union([filling.as_slice()], meter))?, meter))?;
            let wide = joined(buffered(&left, -TOUCHING / 2.0, meter))?;
            if wide.iter().any(|piece| !speck(piece.area(), layer.rules.auto_min_area_mm2)) {
                continue;
            }
            let mut details = BTreeSet::new();
            for (other, parts, smallest) in near_it {
                if detail(&hole, filling.get(parts).unwrap_or_default(), smallest, meter)? {
                    details.insert(other);
                }
            }
            if !details.is_empty() {
                holes.push(Hole { part: p, hole: h, details });
            }
        }
    }
    Ok(holes)
}

/// Whether `parts`, of an area whose smallest area is `smallest`, fill the hole `hole` in part, as a detail
/// does, with `meter`'s work: some of them lies in the hole, wider than [`TOUCHING`] and no speck.
fn detail(hole: &[Polygon], parts: &[Polygon], smallest: f64, meter: &mut Meter) -> Result<bool, Failed> {
    let inside = joined(intersection(hole, parts, meter))?;
    if inside.iter().all(|piece| speck(piece.area(), smallest)) {
        return Ok(false);
    }
    Ok(!no_area(area_of(&joined(buffered(&inside, -TOUCHING / 2.0, meter))?)))
}

/// Moves each area of `layers` with holes closed back in `order`, the places of the `elements` that sew,
/// before its details, the largest first ([the module](self)), then opens again each area's holes whose
/// details don't all sew after it. Each area has `budget`'s work to itself; one that needs more keeps its
/// place, which is said in `said`.
fn moved(elements: &[Element], layers: &mut [Option<Layer>], order: &mut Vec<usize>, budget: &Budget, said: &mut Vec<Diagnostic>) {
    // The areas to move, the largest first, as their holes closed make them.
    let mut moving: Vec<(f64, usize)> = Vec::new();
    for (index, layer) in layers.iter().enumerate() {
        let Some(layer) = layer.as_ref().filter(|layer| !layer.closed.is_empty() && !layer.unfound) else { continue };
        if let Ok(region) = closed(layer, &mut budget.meter()) {
            moving.push((area_of(&region), index));
        }
    }
    moving.sort_by(|(a, i), (b, j)| b.total_cmp(a).then(i.cmp(j)));
    for (_, index) in moving {
        if let Err(failed) = move_back(elements, layers, order, index, &mut budget.meter()) {
            said.extend(
                id(elements, index).map(|id| failed.said(Code::AutoLayers, "where this area sews", "it keeps its place", budget.max_work, id)),
            );
        }
    }
    // Once all have moved: an area that moved back past another may have taken a detail of that one along.
    // Reopening a hole moves nothing, so one pass will do. Its work is that of the order and of the details,
    // which finding them was charged for.
    let mut at = vec![usize::MAX; layers.len()];
    for (place, &index) in order.iter().enumerate() {
        if let Some(slot) = at.get_mut(index) {
            *slot = place;
        }
    }
    let sews = |index: usize| at.get(index).copied().unwrap_or(usize::MAX);
    for (index, layer) in layers.iter_mut().enumerate() {
        if let Some(layer) = layer {
            layer.closed.retain(|hole| hole.details.iter().all(|&detail| sews(detail) > sews(index)));
        }
    }
}

/// Moves the area `index` of `layers` back in `order` before its details, as far as the shapes between
/// them let it ([the module](self)), with `meter`'s work: a unit for each element looked at to find where it
/// and its details sew, one for each shape it moves past, and the work of finding whether an area's part
/// that shows overlaps it.
fn move_back(elements: &[Element], layers: &[Option<Layer>], order: &mut Vec<usize>, index: usize, meter: &mut Meter) -> Result<(), Failed> {
    let Some(Some(layer)) = layers.get(index) else { return Ok(()) };
    let details: BTreeSet<usize> = layer.closed.iter().flat_map(|hole| hole.details.iter().copied()).collect();
    meter.charge(2 * order.len() as u64)?;
    let (Some(from), Some(earliest)) = (order.iter().position(|&at| at == index), order.iter().position(|at| details.contains(at))) else {
        return Ok(());
    };
    // Nothing moves across a stop, and an element with a stop after it keeps its place.
    if earliest >= from || elements.get(index).is_none_or(stops_after) {
        return Ok(());
    }
    let region = closed(layer, meter)?;
    let Some(bounds) = envelope(&region) else { return Ok(()) };
    let mut to = from;
    while to > earliest {
        let Some(&before) = order.get(to - 1) else { break };
        meter.charge(1)?;
        let Some(element) = elements.get(before) else { break };
        let passed = if stops_after(element) {
            false
        } else if details.contains(&before) {
            true
        } else if let Some(Some(other)) = layers.get(before) {
            !overlap(&other.shown, &region, meter)?
        } else {
            drawn(element).is_none_or(|other| !near(&bounds, TOUCHING, &other))
        };
        if !passed {
            break;
        }
        to -= 1;
    }
    if to < from {
        let moved = order.remove(from);
        order.insert(to, moved);
    }
    Ok(())
}

/// What shows of `layer` with the holes its details fill closed, with `meter`'s work: a part of it inside
/// such a hole lies inside the outline round the hole too, and joined they are one part.
fn closed(layer: &Layer, meter: &mut Meter) -> Result<Vec<Polygon>, Failed> {
    if layer.closed.is_empty() {
        return Ok(layer.shown.clone());
    }
    let parts: Vec<Polygon> = layer
        .shown
        .iter()
        .enumerate()
        .map(|(p, part)| {
            let open = part.holes.iter().enumerate().filter(|(h, _)| !layer.closed.iter().any(|hole| hole.part == p && hole.hole == *h));
            Polygon { outline: part.outline.clone(), holes: open.map(|(_, ring)| ring.clone()).collect() }
        })
        .collect();
    joined(union([parts.as_slice()], meter))
}

/// What an area is given to sew.
enum Given {
    /// Its region as drawn, its path kept.
    Drawn,
    /// Another region; `hidden` when the areas drawn over it hide some of its region as drawn.
    Region { parts: Vec<Polygon>, hidden: bool },
}

/// What `layer` is given to sew, with `meter`'s work: what shows of it with the holes its details fill
/// closed, or its path as drawn when that is its region as drawn, to within no area.
fn given(layer: &Layer, meter: &mut Meter) -> Result<Given, Failed> {
    let parts = closed(layer, meter)?;
    let hidden = !no_area(area_of(&joined(difference(&layer.drawn, &parts, meter))?));
    let added = !no_area(area_of(&joined(difference(&parts, &layer.drawn, meter))?));
    Ok(if hidden || added { Given::Region { parts, hidden } } else { Given::Drawn })
}

/// `SC-I0908` counting the areas that sew only what shows of them, `flattened`, and those digitized whole
/// under their details, `whole`; `None` when there are neither.
fn summary(flattened: usize, whole: usize) -> Option<Diagnostic> {
    let shown = match flattened {
        0 => None,
        1 => Some("1 area sews only what shows of it under the areas drawn over it.".to_string()),
        n => Some(format!("{n} areas sew only what shows of them under the areas drawn over them.")),
    };
    let whole = match whole {
        0 => None,
        1 => Some("1 area is digitized whole under the areas that fill its holes, which sew after it.".to_string()),
        n => Some(format!("{n} areas are digitized whole under the areas that fill their holes, which sew after them.")),
    };
    let message = [shown, whole].into_iter().flatten().collect::<Vec<_>>().join(" ");
    (!message.is_empty()).then(|| Diagnostic::new(Code::AutoLayers, message))
}

#[cfg(test)]
mod tests;
