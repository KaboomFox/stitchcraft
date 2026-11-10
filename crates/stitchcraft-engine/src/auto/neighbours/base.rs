//! The base layer: one underlay under the whole design (`REQ-AUTO-028`), said with `SC-I0909`.
//!
//! The fabric's colour isn't the design's to choose, and on dark fabric a slit between 2 shapes shows as a
//! dark line. So a design digitized automatically first sews one open fill under everything, in one thread,
//! which then lies under each slit (`docs/src/design/image-to-embroidery.md` › Covering the fabric). It is
//! the fill that Ink/Stitch's Knockdown Fill tool makes, with that tool's parameters, so no stitch code is
//! new here: this module finds its shape, and the tatami fill sews it.
//!
//! - **The shape** is what the automatic elements paint together once the specks are merged: each area its
//!   region, each line the area its stroke covers, and each part's line or column its part. It is grown by
//!   [`CLOSING`], half a thread, and shrunk by as much, which closes the slits narrower than a thread, and
//!   then offset by `auto_base_offset_mm`. Growing by a length and then moving the edge in by the rest is
//!   the same shape, found in 2 steps.
//! - **Pieces.** Each part of that shape is one fill, without its holes when `auto_base_keep_holes` is off,
//!   and a part smaller than the smallest area is left out, as a speck is.
//! - **Thread.** The thread that paints the largest area, the first drawn of 2 as large. A part's columns
//!   each paint their part, which counts once.
//! - **Underlay.** Every automatic area then sews no underlay of its own: the base layer is its underlay.
//!
//! The design goes by the settings its lowest automatic shape has for the base layer. A shape that needs
//! more work than the budget allows leaves the design without a base layer, said with `SC-W0910`, and each
//! fill keeps the underlay of its settings.

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Labels, Meter};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::Thread;

use super::{Failed, Shapes, joined, outline, speck};
use crate::auto::{AutoParams, numbered};
use crate::common::MAX_STITCH_LENGTH;
use crate::design::{Element, FillRule, Hints, Shape};
use crate::generators::mm;
use crate::normalize::region::Polygon;
use crate::normalize::region::ops::{area_of, buffered, union};

/// The id of the base layer's first piece. The others are numbered from it, `#2` and so on.
pub(crate) const BASE: &str = "stitchcraft:base";

/// How far the design's outer shape is grown and then shrunk, in millimetres: half a thread 0.4 mm wide, so
/// that a slit narrower than a thread between 2 shapes is part of the shape.
const CLOSING: f64 = 0.2;

/// The row spacing of a fill whose needle points make triangles, as a share of its stitch length: √3/2, the
/// height of a triangle with equal sides.
const ROWS: f64 = 0.866_025_403_784_438_6;

/// The key that turns a fill's underlay on or off.
const UNDERLAY: &str = "fill_underlay";

/// The base layer of a design.
#[derive(Default)]
pub(super) struct Laid {
    /// Its pieces, each a fill; none when the design has no base layer.
    pub(super) pieces: Vec<Element>,
    /// What is said of it.
    pub(super) said: Vec<Diagnostic>,
}

/// The base layer under the automatic elements of `shapes`, by the settings `rules`, its pieces named with
/// ids that `taken` doesn't hold yet, with `budget`'s work to itself. The automatic areas of `shapes` then
/// sew no underlay of their own. It says `SC-I0909` of the base layer, or of a design that has room for
/// none, and `SC-W0910` when its shape needs more work than the budget allows.
pub(super) fn laid(shapes: &mut Shapes, rules: &AutoParams, taken: &mut Labels, budget: &Budget) -> Laid {
    let off = Fix::Hint("To sew the design without a base layer, turn `auto_base_layer` off.".to_string());
    match pieces(shapes, rules, taken, &mut budget.meter()) {
        Ok(None) => Laid::default(),
        Ok(Some((pieces, _))) if pieces.is_empty() => {
            let message = "No part of the design is wide enough for a base layer, so it sews none, and each fill sews the underlay of its settings.";
            Laid { pieces, said: vec![Diagnostic::new(Code::AutoBase, message)] }
        }
        Ok(Some((pieces, area))) => {
            let (count, thread) = (pieces.len(), pieces.first().map(|piece| piece.thread.color.to_string()).unwrap_or_default());
            let parts = if count == 1 { "1 piece".to_string() } else { format!("{count} pieces") };
            let message = format!("A base layer of {} mm² in {parts} sews first under the design, in the thread {thread}.", mm(area));
            Laid { pieces, said: vec![Diagnostic::new(Code::AutoBase, message).with_fix(off)] }
        }
        Err(failed) => {
            let consequence = "the design sews no base layer, and each fill sews the underlay of its settings";
            Laid { pieces: Vec::new(), said: vec![failed.said_of_all(Code::AutoBaseLeftOut, "the shape of the base layer", consequence, budget.max_work)] }
        }
    }
}

/// The pieces of the base layer under the automatic elements of `shapes` and their area, with `meter`'s
/// work; `None` when they paint nothing. With a piece, the automatic areas' underlays are turned off.
fn pieces(shapes: &mut Shapes, rules: &AutoParams, taken: &mut Labels, meter: &mut Meter) -> Result<Option<(Vec<Element>, f64)>, Failed> {
    let mut painted: Vec<Polygon> = Vec::new();
    // How much each thread paints, in the order the threads are first drawn.
    let mut threads: Vec<(Thread, f64)> = Vec::new();
    for index in 0..shapes.len() {
        meter.charge(1)?;
        if !shapes.automatic(index) {
            continue;
        }
        shapes.find(index);
        let (Some((_, parts)), Some(element)) = (shapes.paint(index), shapes.elements.get(index)) else { continue };
        // Each column of a part paints the part.
        if painted.ends_with(parts) {
            continue;
        }
        painted.extend_from_slice(parts);
        match threads.iter_mut().find(|(thread, _)| thread.color == element.thread.color) {
            Some((_, sum)) => *sum += area_of(parts),
            None => threads.push((element.thread.clone(), area_of(parts))),
        }
    }
    let mut most: Option<(Thread, f64)> = None;
    for (thread, area) in threads {
        if most.as_ref().is_none_or(|(_, largest)| area > *largest) {
            most = Some((thread, area));
        }
    }
    let Some((thread, _)) = most else { return Ok(None) };
    let grown = joined(buffered(&joined(union([painted.as_slice()], meter))?, CLOSING, meter))?;
    let shape = joined(buffered(&grown, rules.auto_base_offset_mm.get() - CLOSING, meter))?;
    let mut kept = Vec::new();
    for piece in shape {
        meter.charge(1)?;
        let piece = if rules.auto_base_keep_holes { piece } else { Polygon { holes: Vec::new(), ..piece } };
        if !speck(piece.area(), rules.auto_min_area_mm2) {
            kept.push(piece);
        }
    }
    let params = knockdown(rules.auto_base_stitch_length_mm.get());
    let mut pieces = Vec::with_capacity(kept.len());
    for (place, piece) in kept.iter().enumerate() {
        // The first piece has the base layer's id, and the others are numbered from it. An id that the
        // design has already steps aside, and fits, as a part's does (`auto::named`).
        let id = ElementId::new(BASE).ok().and_then(|id| if place == 0 { Some(id) } else { numbered(&id, place + 1) });
        let label = taken.claim(id.as_ref().map(ElementId::as_str).unwrap_or(BASE).to_string(), meter)?;
        let id = ElementId::new(label).ok().ok_or(Failed::Work)?;
        let shape = Shape::Fill { path: outline(std::slice::from_ref(piece)), rule: FillRule::NonZero, gradient: None };
        pieces.push(Element { id, name: None, shape, thread: thread.clone(), params: params.clone(), automatic: false, hints: Hints::default(), clips: Vec::new() });
    }
    // The base layer is the underlay of the fills on it.
    if !pieces.is_empty() {
        meter.charge(shapes.len() as u64)?;
        for element in shapes.elements.iter_mut().zip(&shapes.area).filter_map(|(element, area)| area.as_ref().map(|_| element)) {
            element.params.set(UNDERLAY, "false");
        }
    }
    Ok(Some((pieces, area_of(&kept))))
}

/// The parameters that Ink/Stitch's Knockdown Fill writes for a stitch `length` millimetres long: rows that
/// far times [`ROWS`] apart, written to 4 decimals, staggered over 2 rows, under an underlay at 60° and −60°
/// with the same length and spacing, and no travel along the outline under either.
fn knockdown(length: f64) -> ParamSet {
    let (length, spacing) = (length.to_string(), format!("{:.4}", length * ROWS));
    let mut params = ParamSet::new();
    for (key, value) in [
        (MAX_STITCH_LENGTH, length.as_str()),
        ("row_spacing_mm", spacing.as_str()),
        ("staggers", "2"),
        ("underpath", "false"),
        (UNDERLAY, "true"),
        ("fill_underlay_angle", "60 -60"),
        ("fill_underlay_max_stitch_length_mm", length.as_str()),
        ("fill_underlay_row_spacing_mm", spacing.as_str()),
        ("underlay_underpath", "false"),
    ] {
        params.set(key, value);
    }
    params
}
