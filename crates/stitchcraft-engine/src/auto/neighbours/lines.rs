//! Thin lines merged into their neighbours (`REQ-AUTO-090`), each said with `SC-I0915`. Design:
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
//! part of that area's fill. The rule goes by the line's width alone, never by what the picture shows.
//!
//! The edge 2 shapes share is measured as a speck's is (`specks::shared`): how much of the area's rings lies
//! within [`TOUCHING`] of the line. Every area gives up its thin parts first, so a line goes into what its
//! neighbours keep, never into another line. On a tie, it goes into the area that sews first. A line that
//! shares no edge with an automatic area stays where it is and sews as a line. A speck is left to the specks
//! (`REQ-AUTO-008`), and an area that turns `auto_fill_uneven` off, or names its stitch with `auto_stitch`,
//! keeps its lines.
//!
//! **Work.** Each area has the budget's work to itself to find its thin parts, and each line to find where
//! it goes. It costs the work of the rings near it and a unit for each area it looks past. An area whose
//! thin parts need more keeps them, and a line whose neighbours need more stays where it is, which is said.

use std::borrow::Cow;

use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Meter, Mm};

use parts::Fate;
pub use parts::is_part;

use super::layers::Flattened;
use super::specks::shared;
use super::{Failed, TOUCHING, envelope, joined, near, parts_of, speck};
use crate::auto::narrow::{Edges, piece, spread, within};
use crate::auto::{AutoParams, outline};
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
}

/// A line merged into a neighbour: the line, and the place in the design of the area it merged into.
pub(in crate::auto) struct Merge {
    /// The line.
    pub(in crate::auto) line: Polygon,
    /// The place in the design of the area it merged into, which may be its own.
    pub(in crate::auto) into: usize,
}

/// A thin part of an area: a line.
struct Line {
    /// The place among the elements of the area it is a part of.
    from: usize,
    /// The part.
    part: Polygon,
}

/// `flattened`, each automatic area's thin parts merged into the automatic area beside it that it shares the
/// longest edge with ([the module](self)), and what is said of each. `rules` holds each element's settings
/// of the rules by its place in the design, and `narrowest` is the design's narrowest satin stroke. Each area
/// has `budget`'s work to itself to find its lines, and each line to find where it goes. Each line merged is
/// given back too, with where it went.
pub(in crate::auto) fn merged<'d>(
    flattened: Flattened<'d>,
    rules: &[Option<AutoParams>],
    narrowest: Mm,
    budget: &Budget,
) -> (Flattened<'d>, Vec<Merge>) {
    let Flattened { elements, kept, mut said } = flattened;
    let mut areas: Vec<Option<Area>> =
        elements.iter().map(|(index, element)| area(element, rules.get(*index).and_then(Option::as_ref), budget)).collect();
    // Every area gives up its lines first, so that a line goes into what its neighbours keep.
    let mut lines = Vec::new();
    let mut done = Vec::new();
    for (from, (index, element)) in elements.iter().enumerate() {
        let Some(rules) = rules.get(*index).and_then(Option::as_ref).filter(|rules| merges(rules)) else { continue };
        let Some(area) = areas.get_mut(from).and_then(Option::as_mut) else { continue };
        match thin(&area.parts, narrowest, rules, &mut budget.meter()) {
            Ok(thin) => {
                let parts = std::mem::take(&mut area.parts);
                for (part, thin) in parts.into_iter().zip(thin) {
                    if thin {
                        lines.push(Line { from, part });
                    } else {
                        area.parts.push(part);
                    }
                }
            }
            Err(failed) => {
                said.push(failed.said(Code::AutoLineMerged, "the thin lines of this area", "it keeps them", budget.max_work, element.id.clone()));
            }
        }
    }
    // Merged, a line that is a part of its own takes its colour away: each colour keeps its share of parts.
    let colour = |place: usize| elements.get(place).map(|(_, element)| element.thread.color);
    let rule = |place: usize| elements.get(place).and_then(|(index, _)| rules.get(*index)).and_then(Option::as_ref);
    let fates = parts::fates(&lines, &areas, colour, rule, budget);
    let mut sewn_as_lines = Vec::new();
    for (line, fate) in lines.into_iter().zip(fates) {
        let Some((_, element)) = elements.get(line.from) else { continue };
        let this = format!("A line of this area, of {} mm²,", mm((line.part.area() * 100.0).round() / 100.0));
        match fate {
            Fate::Merges => {}
            Fate::Kept { least } => {
                let message = format!(
                    "{this} is narrower than the narrowest satin stroke, but sews as a line: it is a part of its colour by itself, and merged it would take the picture's likeness under {} (`auto_min_likeness`).",
                    mm(least)
                );
                let hint = Fix::Hint("To merge it, lower `auto_min_likeness`.".to_string());
                said.push(Diagnostic::new(Code::AutoLineMerged, message).with_element(element.id.clone()).with_fix(hint));
                sewn_as_lines.push(line);
                continue;
            }
            Fate::Unmeasured(failed) => {
                said.push(failed.said(
                    Code::AutoLineMerged,
                    "whether this line is a part of its colour",
                    "it sews as a line",
                    budget.max_work,
                    element.id.clone(),
                ));
                sewn_as_lines.push(line);
                continue;
            }
        }
        match into(&mut areas, &line, &mut budget.meter()) {
            Ok(Some(to)) => {
                let message = if to == line.from {
                    format!(
                        "{this} is narrower than the narrowest satin stroke, so it is joined to the rest of this area, which it shares the longest edge with."
                    )
                } else {
                    let into = elements.get(to).map(|(_, element)| element.id.as_str().to_string()).unwrap_or_default();
                    format!(
                        "{this} is narrower than the narrowest satin stroke, so it is merged into `{into}`, which it shares the longest edge with."
                    )
                };
                let hint = Fix::Hint("To sew it as a running stitch, turn `auto_fill_uneven` off, or give the shape settings.".to_string());
                said.push(Diagnostic::new(Code::AutoLineMerged, message).with_element(element.id.clone()).with_fix(hint));
                if let Some((into, _)) = elements.get(to) {
                    done.push(Merge { line: line.part, into: *into });
                }
            }
            Ok(None) => back(&mut areas, line),
            Err(failed) => {
                said.push(failed.said(Code::AutoLineMerged, "where to merge this line", "it sews as a line", budget.max_work, element.id.clone()));
                back(&mut areas, line);
            }
        }
    }
    // The lines kept go back once every other has merged, so that none merges into a line.
    for line in sewn_as_lines {
        back(&mut areas, line);
    }
    let mut sewing = Vec::with_capacity(elements.len());
    for ((index, element), area) in elements.into_iter().zip(areas) {
        match area.filter(|area| area.changed) {
            None => sewing.push((index, element)),
            // An area whose every part merged into its neighbours sews nothing, and has no place for a trim or a
            // stop after it.
            Some(area) if area.parts.is_empty() => said.extend(trim_or_stop_left_out(&element).map(|left| left.with_element(element.id.clone()))),
            Some(area) => {
                let shape = Shape::Fill { path: outline(&area.parts), rule: FillRule::NonZero, gradient: None };
                sewing.push((index, Cow::Owned(Element { shape, ..element.into_owned() })));
            }
        }
    }
    (Flattened { elements: sewing, kept, said }, done)
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
    Some(Area { parts, bounds, changed: false })
}

/// For each of `parts`, whether it is a line that merges: narrower than `narrowest` everywhere, no speck, no
/// smaller than `rules`' smallest area, and ragged, its width varying by `rules`' least unevenness of a line
/// or more, with `meter`'s work.
fn thin(parts: &[Polygon], narrowest: Mm, rules: &AutoParams, meter: &mut Meter) -> Result<Vec<bool>, Failed> {
    let mut thin = Vec::with_capacity(parts.len());
    for part in parts {
        meter.charge(1)?;
        let narrow = !speck(part.area(), rules.auto_min_area_mm2) && within(part, narrowest, meter)? == Some(true);
        thin.push(narrow && ragged(part, rules.auto_min_line_unevenness, meter)?);
    }
    Ok(thin)
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
    if let Some(from) = areas.get_mut(line.from).and_then(Option::as_mut).filter(|_| line.from != place) {
        from.changed = true;
    }
    Ok(Some(place))
}

/// `line` put back into the area it is a part of: it shares no edge with an automatic area, or where it
/// goes couldn't be found, and it sews as a line.
fn back(areas: &mut [Option<Area>], line: Line) {
    if let Some(area) = areas.get_mut(line.from).and_then(Option::as_mut) {
        area.parts.push(line.part);
    }
}

mod parts;
#[cfg(test)]
mod tests;
