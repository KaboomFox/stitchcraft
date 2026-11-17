//! Thin lines merged into their neighbours (`REQ-AUTO-090`, `REQ-AUTO-091`), each said with `SC-I0915`. Design:
//! `docs/src/design/image-to-embroidery.md` › Neighbours.
//!
//! A trace of a photo draws soft shading and the strands of fur as thin lines of one colour between patches
//! of others. Sewn as drawn, each is a running stitch along its middle, and the fills on either side of it
//! end their rows on it: a traced dog had 15 to 34 shapes of such lines, and they left the slivers between
//! fills, the rows that show on another colour, the moves across the line and the thickest spots where 5 to
//! 9 elements meet. A digitizer sews such a strand as part of the patch it lies in. So, once the layers are
//! flattened and before anything is digitized, each part of an automatic area narrower than the design's
//! narrowest satin stroke everywhere ([`within`]), a part the rules would sew as a line, is merged into the
//! automatic area it shares the longest edge with, which may be the rest of its own area. It then sews as
//! part of that area's fill. A trace also draws a strand as a thin tail of a patch, which sorting cuts off
//! the patch and sews as a running stitch: such a piece, ragged as a line is, merges too (`REQ-AUTO-091`).
//! A piece that goes back to the rest of its own area is said of nowhere. The rule goes by the line's width
//! and how it varies, never by what the picture shows.
//!
//! The edge 2 shapes share is measured as a speck's is (`specks::shared`): how much of the area's rings lies
//! within [`TOUCHING`] of the line. Every area gives up its thin parts first, so a line goes into what its
//! neighbours keep, never into another line. On a tie, it goes into the area that sews first. A line that
//! shares no edge with an automatic area stays where it is and sews as a line. A speck is left to the specks
//! (`REQ-AUTO-008`), and an area that turns `auto_fill_uneven` off, or names its stitch with `auto_stitch`,
//! keeps its lines.
//!
//! **Sorting kept** (`REQ-AUTO-092`). Each area's parts are sorted as digitizing sorts them (`narrow::sorted`)
//! to find its lines, with the holes digitizing keeps open. An area that takes nothing in sews as it was
//! sorted then, less the lines it gave up and their rules, which `SC-I0912` no longer names: sorted again,
//! what is left of a part that gave up a piece would split into new tails, which sew as lines where the
//! piece left one. An area that takes a line in is sorted again.
//!
//! **Work.** Each area has the budget's work to itself to find its thin parts, and each line to find where
//! it goes. It costs the work of the rings near it and a unit for each area it looks past. An area whose
//! thin parts need more keeps them, and a line whose neighbours need more stays where it is, which is said.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Meter, Mm, Rect};

use super::layers::Flattened;
use super::specks::shared;
use super::{Failed, TOUCHING, envelope, joined, near, parts_of};
use crate::auto::columns::Sewn;
use crate::auto::narrow::{self, Along, Edges, Parts, piece, spread};
use crate::auto::rules::Bounds;
use crate::auto::{AutoParams, Open, clear, outline};
use crate::design::{Element, FillRule, Shape};
use crate::generate::trim_or_stop_left_out;
use crate::generators::mm;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops;
use crate::normalize::strips::centre_lines;

/// An automatic area as its lines are merged.
struct Area {
    /// Its parts.
    parts: Vec<Polygon>,
    /// The box round them.
    bounds: Envelope,
    /// Whether its parts changed: it gave up a line, or took one in.
    changed: bool,
    /// Its parts as digitizing sorts them, and the places among those sewn along their middles of the lines
    /// that left it, while it takes nothing in: none once it does, or when it wasn't sorted.
    sorted: Option<(Parts, BTreeSet<usize>)>,
}

/// What merging the lines makes of the flattened layers.
pub(in crate::auto) struct Merged<'d> {
    /// The layers, each area's lines merged.
    pub(in crate::auto) layers: Flattened<'d>,
    /// Each line merged, with where it went.
    pub(in crate::auto) merges: Vec<Merge>,
    /// The parts of each area that took nothing in, by its place in the design, as digitizing sorts them,
    /// less the lines that left it: it sews them so.
    pub(in crate::auto) sorted: BTreeMap<usize, Parts>,
}

/// A line merged into a neighbour: the line, and the place in the design of the area it merged into.
pub(in crate::auto) struct Merge {
    /// The line.
    pub(in crate::auto) line: Polygon,
    /// The place in the design of the area it merged into, which may be its own.
    pub(in crate::auto) into: usize,
}

/// A thin part of an area, or a thin piece of a broader part of it: a line.
struct Line {
    /// The place among the elements of the area it is a part of.
    from: usize,
    /// The part or piece.
    part: Polygon,
    /// Whether it is a whole part of the area, not a piece of one.
    whole: bool,
    /// Its place among the area's parts sewn along their middles.
    at: usize,
}

/// What an area gives up: its lines, and the rest of it.
struct Given {
    /// The rest of the area, joined.
    rest: Vec<Polygon>,
    /// Its lines, each with whether it is a whole part of the area and its place among the parts sewn along
    /// their middles.
    lines: Vec<(Polygon, bool, usize)>,
}

/// `flattened`, each automatic area's thin parts merged into the automatic area beside it that it shares the
/// longest edge with ([the module](self)), and what is said of each. `design` holds the design's elements,
/// whose shapes drawn after an area say which of its holes stay open, and `rules` each one's settings of the
/// rules, both by place in the design, and `narrowest` is the design's narrowest satin stroke. Each area has
/// `budget`'s work to itself to find its lines, and each line to find where it goes. Each line merged is given
/// back too, with where it went, and the parts of each area that took nothing in, as they sew.
pub(in crate::auto) fn merged<'d>(
    flattened: Flattened<'d>,
    design: &[Element],
    rules: &[Option<AutoParams>],
    narrowest: Mm,
    budget: &Budget,
) -> Merged<'d> {
    let Flattened { elements, kept, mut said } = flattened;
    let mut areas: Vec<Option<Area>> =
        elements.iter().map(|(index, element)| area(element, rules.get(*index).and_then(Option::as_ref), budget)).collect();
    // Every area gives up its lines first, so that a line goes into what its neighbours keep.
    let mut lines = Vec::new();
    let mut done = Vec::new();
    for (from, (index, element)) in elements.iter().enumerate() {
        let Some(rules) = rules.get(*index).and_then(Option::as_ref).filter(|rules| merges(rules)) else { continue };
        let Some(area) = areas.get_mut(from).and_then(Option::as_mut) else { continue };
        // A hole that a shape drawn later meets is not kept open, as when the area is digitized.
        let later = design.get(index + 1..).unwrap_or_default();
        let open = |hole: Rect, meter: &mut Meter| clear(later, hole, meter);
        match given(&element.id, &area.parts, narrowest, rules, &open, &mut budget.meter()) {
            Ok((sorted, None)) => area.sorted = Some((sorted, BTreeSet::new())),
            Ok((sorted, Some(given))) => {
                area.parts = given.rest;
                area.sorted = Some((sorted, BTreeSet::new()));
                lines.extend(given.lines.into_iter().map(|(part, whole, at)| Line { from, part, whole, at }));
            }
            Err(failed) => {
                said.push(failed.said(Code::AutoLineMerged, "the thin lines of this area", "it keeps them", budget.max_work, element.id.clone()));
            }
        }
    }
    for line in lines {
        let Some((_, element)) = elements.get(line.from) else { continue };
        let area = mm((line.part.area() * 100.0).round() / 100.0);
        let this = if line.whole {
            format!("A line of this area, of {area} mm², is narrower than the narrowest satin stroke,")
        } else {
            format!("A thin piece of this area, of {area} mm², would sew as a running stitch,")
        };
        match into(&mut areas, &line, &mut budget.meter()) {
            // A piece that goes back to the rest of its own area sews as it would have.
            Ok(Some(to)) if to == line.from && !line.whole => {}
            Ok(Some(to)) => {
                let message = if to == line.from {
                    format!("{this} so it is joined to the rest of this area, which it shares the longest edge with.")
                } else {
                    let into = elements.get(to).map(|(_, element)| element.id.as_str().to_string()).unwrap_or_default();
                    format!("{this} so it is merged into `{into}`, which it shares the longest edge with.")
                };
                let hint = Fix::Hint("To sew it as a running stitch, turn `auto_fill_uneven` off, or give the shape settings.".to_string());
                said.push(Diagnostic::new(Code::AutoLineMerged, message).with_element(element.id.clone()).with_fix(hint));
                // The line left its area's parts sewn along their middles.
                if let Some((_, left)) = areas.get_mut(line.from).and_then(Option::as_mut).and_then(|area| area.sorted.as_mut()) {
                    left.insert(line.at);
                }
                if let Some((into, _)) = elements.get(to) {
                    done.push(Merge { line: line.part, into: *into });
                }
            }
            Ok(None) => back(&mut areas, line, &mut budget.meter()),
            Err(failed) => {
                said.push(failed.said(Code::AutoLineMerged, "where to merge this line", "it sews as a line", budget.max_work, element.id.clone()));
                back(&mut areas, line, &mut budget.meter());
            }
        }
    }
    let mut sewing = Vec::with_capacity(elements.len());
    let mut sorted = BTreeMap::new();
    for ((index, element), area) in elements.into_iter().zip(areas) {
        let area = area.map(|mut area| {
            if let Some((parts, left)) = area.sorted.take() {
                sorted.insert(index, without(parts, &left));
            }
            area
        });
        match area.filter(|area| area.changed) {
            None => sewing.push((index, element)),
            // An area whose every part merged into its neighbours sews nothing, and has no place for a trim or a
            // stop after it.
            Some(area) if area.parts.is_empty() => {
                sorted.remove(&index);
                said.extend(trim_or_stop_left_out(&element).map(|left| left.with_element(element.id.clone())));
            }
            Some(area) => {
                let shape = Shape::Fill { path: outline(&area.parts), rule: FillRule::NonZero, gradient: None };
                sewing.push((index, Cow::Owned(Element { shape, ..element.into_owned() })));
            }
        }
    }
    Merged { layers: Flattened { elements: sewing, kept, said }, merges: done, sorted }
}

/// `parts`, an area's sorted, less those sewn along their middles at the places `left`, which left it, and
/// less the rule each of those sewed by: it sews by its new area's rules now, and `SC-I0912` names only the
/// rules the area's own parts sew by.
fn without(parts: Parts, left: &BTreeSet<usize>) -> Parts {
    if left.is_empty() {
        return parts;
    }
    let Parts { filled, along, said, fired, judged } = parts;
    let gone: BTreeSet<usize> = left.iter().filter_map(|at| judged.get(*at).copied()).collect();
    let fired = fired.into_iter().enumerate().filter(|(place, _)| !gone.contains(place)).map(|(_, fired)| fired).collect();
    let (along, judged) = along
        .into_iter()
        .zip(judged)
        .enumerate()
        .filter(|(at, _)| !left.contains(at))
        // Each rule kept moves up by the rules before it that left.
        .map(|(_, (along, place))| (along, place - gone.range(..place).count()))
        .unzip();
    Parts { filled, along, said, fired, judged }
}

/// Whether an area with `rules` merges its thin lines: unless it turns that off, or names its stitch.
fn merges(rules: &AutoParams) -> bool {
    rules.auto_fill_uneven && rules.auto_stitch == "auto"
}

/// `element` as an area whose lines may merge or take lines in: an automatic area with settings of the
/// rules, `rules`, whose region is built with `budget`'s work to itself. Any other element, and an area that
/// needs more or whose region the region code fails to join, is none.
fn area(element: &Element, rules: Option<&AutoParams>, budget: &Budget) -> Option<Area> {
    let (true, Some(_), Shape::Fill { path, rule, .. }) = (element.automatic, rules, &element.shape) else { return None };
    let parts = parts_of(path, *rule, &mut budget.meter()).ok()?.ok().filter(|parts| !parts.is_empty())?;
    let bounds = envelope(&parts)?;
    Some(Area { parts, bounds, changed: false, sorted: None })
}

/// `parts`, an automatic area `id`'s whose settings of the rules are `rules`, sorted as digitizing sorts them
/// (`narrow::sorted`) with the holes `open` says, and the lines it gives up, with the rest of it, with
/// `meter`'s work: each part or piece that the rules sew as a running stitch along its middle, a line or a
/// column too narrow for satin, and that is ragged, its width varying by `rules`' least unevenness of a line
/// or more. `narrowest` is the design's narrowest satin stroke. No lines when it gives up none.
fn given(
    id: &ElementId,
    parts: &[Polygon],
    narrowest: Mm,
    rules: &AutoParams,
    open: &Open<'_>,
    meter: &mut Meter,
) -> Result<(Parts, Option<Given>), Failed> {
    let bounds = Bounds::of(rules, narrowest);
    let sorted = narrow::sorted(id, parts.to_vec(), &bounds, rules, open, meter)?;
    let mut rest = sorted.filled.clone();
    let mut lines = Vec::new();
    for (at, along) in sorted.along.iter().enumerate() {
        meter.charge(1)?;
        let (part, runs) = match along {
            Along::Narrow(narrow) => (&narrow.part, true),
            Along::Columns(columns) => {
                let runs = !columns.sewn.is_empty() && columns.sewn.iter().all(|sewn| matches!(sewn, Sewn::Running { .. }));
                (&columns.part, runs)
            }
        };
        if runs && ragged(part, rules.auto_min_line_unevenness, meter)? {
            lines.push((part.clone(), parts.contains(part), at));
        } else {
            rest.push(part.clone());
        }
    }
    if lines.is_empty() {
        return Ok((sorted, None));
    }
    let rest = joined(ops::union([rest.as_slice()], meter))?;
    Ok((sorted, Some(Given { rest, lines })))
}

/// Whether the width of `part`, a thin line, varies along its centre lines by `least` or more, measured as a
/// column's unevenness is (`narrow::spread`), with `meter`'s work. A line drawn with an even pen varies by
/// less. A part whose centre lines can't be found, or have no length, is no ragged line.
fn ragged(part: &Polygon, least: f64, meter: &mut Meter) -> Result<bool, Failed> {
    let cell = piece(part);
    let Ok(lines) = centre_lines(part, cell, meter)? else { return Ok(false) };
    let edges = Edges::of(part, cell, meter)?;
    let Some(spread) = spread(&edges, &lines, meter)? else { return Ok(false) };
    Ok(spread.deviation / spread.mean.get() >= least)
}

/// Merges `line` into the area of `areas` it shares the longest edge with, its own included, and gives
/// that area's place; `None` when it shares no edge with one.
fn into(areas: &mut [Option<Area>], line: &Line, meter: &mut Meter) -> Result<Option<usize>, Failed> {
    let zone = joined(ops::buffered(std::slice::from_ref(&line.part), TOUCHING, meter))?;
    let Some(bounds) = envelope(&zone) else { return Ok(None) };
    let mut best: Option<(usize, f64)> = None;
    for (place, area) in areas.iter().enumerate() {
        meter.charge(1)?;
        let Some(area) = area.as_ref().filter(|area| near(&area.bounds, 0.0, &bounds)) else { continue };
        let length = shared(&area.parts, &zone, &bounds, meter)?;
        if length > 0.0 && best.is_none_or(|(_, longest)| length > longest) {
            best = Some((place, length));
        }
    }
    let Some((place, _)) = best else { return Ok(None) };
    let Some(area) = areas.get_mut(place).and_then(Option::as_mut) else { return Ok(None) };
    let mut merged = joined(ops::union([area.parts.as_slice(), std::slice::from_ref(&line.part)], meter))?;
    // Joined, the line closes up with the area; apart, rounding left a gap, which the zone bridges.
    if merged.len() > area.parts.len() {
        merged = joined(ops::union([area.parts.as_slice(), zone.as_slice()], meter))?;
    }
    area.bounds = envelope(&merged).unwrap_or(area.bounds);
    area.parts = merged;
    area.changed = true;
    // A piece back in its own area still sews as it was sorted; anything else changes what the area sorts.
    if place != line.from || line.whole {
        area.sorted = None;
    }
    if let Some(from) = areas.get_mut(line.from).and_then(Option::as_mut).filter(|_| line.from != place) {
        from.changed = true;
    }
    Ok(Some(place))
}

/// `line` put back into the area it is a part of, with `meter`'s work: it shares no edge with an automatic
/// area, or where it goes couldn't be found, and it sews as a line. A piece is joined to the rest again; when
/// that needs more work than `meter` has, it is put back as a part of its own.
fn back(areas: &mut [Option<Area>], line: Line, meter: &mut Meter) {
    if let Some(area) = areas.get_mut(line.from).and_then(Option::as_mut) {
        if line.whole {
            area.parts.push(line.part);
            return;
        }
        match joined(ops::union([area.parts.as_slice(), std::slice::from_ref(&line.part)], meter)) {
            Ok(parts) => area.parts = parts,
            Err(_) => area.parts.push(line.part),
        }
    }
}

#[cfg(test)]
mod tests;
