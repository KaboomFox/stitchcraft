//! Which thin lines merge, so that the picture keeps its likeness (`REQ-AUTO-095`). Design:
//! `docs/src/design/image-to-embroidery.md` › Neighbours and › Likeness.
//!
//! A thin line merged into a neighbour of another colour takes its colour away from where it lies. When the
//! line touches a larger piece of its colour, that piece still shows, a little smaller. When it touches none,
//! it is a whole part of the picture in its colour, and merging it loses the part: the twig of a branch, the
//! shadow along it, the pale rim of a petal. Geometry can't tell a twig from a strand of fur, and no rule
//! may know what a picture shows, so the rule goes by counts, as the likeness measure counts (`REQ-AUTO-093`):
//! the picture's likeness is the mean over its colours of the share of their parts kept, and merging keeps
//! `auto_min_likeness` of it. Merged lines take running stitches and jump threads away, so a picture whose
//! colours have many parts still merges most of its lines, and one whose few parts are lines keeps them.
//!
//! **Parts.** A part is a piece of one colour as large as the smallest area and in which a disc a thread
//! across fits ([`is_part`]): the measure counts the same on its grid. The lines of one colour that touch
//! each other are one group, and a group that touches a piece of its colour that is no speck belongs to that
//! piece's part and is free to merge. A colour's parts are its areas' pieces that are no specks and its other
//! groups that are parts.
//!
//! **Cost.** A group that is a part and merges costs the picture's likeness 1 of its colour's parts divided
//! by the number of colours. The groups that share an edge with an area merge the cheapest first, those of
//! the colour with the most parts, and of 2 as cheap the smaller, while their cost stays within the number of
//! colours times 1 less `auto_min_likeness`. The rest sew as lines.
//!
//! **Work.** Each line has the budget's work to itself to find what it touches. A line that needs more
//! sews as a line, which is said.

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Meter, Mm};
use stitchcraft_plan::Rgb;
use stitchcraft_plan::thickness::THREAD;

use super::super::specks::shared;
use super::super::{Failed, TOUCHING, envelope, joined, near, speck};
use super::{Area, Line};
use crate::auto::AutoParams;
use crate::auto::narrow::within;
use crate::normalize::region::Polygon;
use crate::normalize::region::ops;

/// Whether a piece of one colour, `area` square millimetres large, is a part of the picture as the likeness
/// measure counts it (`REQ-AUTO-093`) and line merging keeps it (`REQ-AUTO-095`): no speck by `rules`'
/// smallest area, and a disc a thread across fits in it, as `holds_thread` says.
pub fn is_part(area: f64, holds_thread: bool, rules: &AutoParams) -> bool {
    holds_thread && !speck(area, rules.auto_min_area_mm2)
}

/// What becomes of a thin line.
#[derive(Debug)]
pub(super) enum Fate {
    /// It merges into its neighbour.
    Merges,
    /// It sews as a line: merged, it would take the picture's likeness under `least`.
    Kept {
        /// The least likeness the picture keeps.
        least: f64,
    },
    /// What it touches needs more work than the budget allows, and it sews as a line.
    Unmeasured(Failed),
}

/// What a line touches.
struct Touch {
    /// Whether it is a part by itself.
    part: bool,
    /// Whether it touches a piece of its colour that is no speck.
    anchored: bool,
    /// Whether it shares an edge with an area, of any colour, that it could merge into.
    beside: bool,
    /// The lines after it, of its colour, that it touches.
    lines: Vec<usize>,
}

/// A group of lines of one colour that touch each other.
#[derive(Default)]
struct Group {
    /// Its lines.
    lines: Vec<usize>,
    /// Their area.
    area: f64,
    /// Whether one of them is a part by itself.
    part: bool,
    /// Whether one touches a piece of its colour that is no speck.
    anchored: bool,
    /// Whether one shares an edge with an area it could merge into.
    beside: bool,
}

/// What becomes of each of `lines`, taken from `areas`, which hold what is left of each element's area,
/// `None` for an element that is no such area ([the module](self)). `colour` gives the colour of the element
/// at a place, and `rules` its settings of the rules. Each line has `budget`'s work to itself.
pub(super) fn fates<'r>(
    lines: &[Line],
    areas: &[Option<Area>],
    colour: impl Fn(usize) -> Option<Rgb>,
    rules: impl Fn(usize) -> Option<&'r AutoParams>,
    budget: &Budget,
) -> Vec<Fate> {
    let mut fates: Vec<Option<Fate>> = Vec::with_capacity(lines.len());
    let mut touches: Vec<Option<Touch>> = Vec::with_capacity(lines.len());
    for at in 0..lines.len() {
        match touch(at, lines, areas, &colour, &rules, &mut budget.meter()) {
            Ok(found) => {
                touches.push(Some(found));
                fates.push(None);
            }
            Err(failed) => {
                touches.push(None);
                fates.push(Some(Fate::Unmeasured(failed)));
            }
        }
    }
    // The groups of lines that touch, each by its first line.
    let mut group: Vec<usize> = (0..lines.len()).collect();
    for (at, found) in touches.iter().enumerate() {
        for &next in found.iter().flat_map(|found| &found.lines) {
            let (a, b) = (root(&group, at), root(&group, next));
            if let Some(slot) = group.get_mut(a.max(b)) {
                *slot = a.min(b);
            }
        }
    }
    let mut groups: BTreeMap<usize, Group> = BTreeMap::new();
    for (at, (found, line)) in touches.iter().zip(lines).enumerate() {
        let Some(found) = found else { continue };
        let entry = groups.entry(root(&group, at)).or_default();
        entry.lines.push(at);
        entry.area += line.part.area();
        entry.part |= found.part;
        entry.anchored |= found.anchored;
        entry.beside |= found.beside;
    }
    // The picture's colours, each with its parts, and the strictest share kept that a shape with lines sets.
    let mut parts: BTreeMap<Rgb, usize> = BTreeMap::new();
    for (place, area) in areas.iter().enumerate() {
        let Some(colour) = colour(place) else { continue };
        let pieces = area
            .as_ref()
            .zip(rules(place))
            .map_or(0, |(area, rules)| area.parts.iter().filter(|part| !speck(part.area(), rules.auto_min_area_mm2)).count());
        *parts.entry(colour).or_insert(0) += pieces;
    }
    let mut least: f64 = 0.0;
    let mut costly = Vec::new();
    for found in groups.values() {
        let Some(line) = found.lines.first().and_then(|&at| lines.get(at)) else { continue };
        let (Some(colour), Some(rules)) = (colour(line.from), rules(line.from)) else { continue };
        least = least.max(rules.auto_min_likeness);
        // A group that is a part of its own is one of its colour's parts, and merged it is a part lost.
        if found.part && !found.anchored {
            *parts.entry(colour).or_insert(0) += 1;
            if found.beside {
                costly.push((colour, found));
            }
        }
    }
    // A part lost costs its colour's likeness 1 of its parts, and the picture's a share of that, as many
    // colours as it has. The cheapest go first, of 2 as cheap the smaller.
    let share = |colour: &Rgb| parts.get(colour).copied().unwrap_or(1).max(1);
    costly.sort_by(|(a, one), (b, other)| {
        share(b).cmp(&share(a)).then(one.area.total_cmp(&other.area)).then(one.lines.first().cmp(&other.lines.first()))
    });
    let allowed = parts.len() as f64 * (1.0 - least) + 1e-9;
    let mut lost = 0.0;
    let mut kept = vec![false; lines.len()];
    for (colour, found) in costly {
        let cost = 1.0 / share(&colour) as f64;
        if lost + cost <= allowed {
            lost += cost;
            continue;
        }
        for &at in &found.lines {
            if let Some(slot) = kept.get_mut(at) {
                *slot = true;
            }
        }
    }
    // A line of a group kept that shares no edge with an area stays where it is anyway, and nothing is said.
    let beside = touches.iter().map(|found| found.as_ref().is_some_and(|found| found.beside));
    let decided = fates.into_iter().zip(kept.into_iter().zip(beside));
    decided.map(|(fate, kept)| fate.unwrap_or(if kept == (true, true) { Fate::Kept { least } } else { Fate::Merges })).collect()
}

/// The first line of the group of `at` among `group`, which holds for each line one before it in its
/// group, or itself.
fn root(group: &[usize], mut at: usize) -> usize {
    while let Some(&up) = group.get(at).filter(|&&up| up != at) {
        at = up;
    }
    at
}

/// What the line `at` of `lines` touches: the pieces of its colour in `areas` that are no specks, and the
/// lines after it of its colour. `colour` and `rules` are as [`fates`] has them, and `meter` meters the work.
fn touch<'r>(
    at: usize,
    lines: &[Line],
    areas: &[Option<Area>],
    colour: &impl Fn(usize) -> Option<Rgb>,
    rules: &impl Fn(usize) -> Option<&'r AutoParams>,
    meter: &mut Meter,
) -> Result<Touch, Failed> {
    let (Some(line), Some(rules)) = (lines.get(at), lines.get(at).and_then(|line| rules(line.from))) else {
        return Ok(Touch { part: false, anchored: true, beside: false, lines: Vec::new() });
    };
    let own = colour(line.from);
    let part = counts(&line.part, rules, meter)?;
    let zone = joined(ops::buffered(std::slice::from_ref(&line.part), TOUCHING, meter))?;
    let Some(bounds) = envelope(&zone) else { return Ok(Touch { part, anchored: false, beside: false, lines: Vec::new() }) };
    let (mut anchored, mut beside) = (false, false);
    for (place, area) in areas.iter().enumerate() {
        meter.charge(1)?;
        let Some(area) = area.as_ref().filter(|area| near(&area.bounds, 0.0, &bounds)) else { continue };
        if colour(place) == own {
            let pieces: Vec<Polygon> = area.parts.iter().filter(|part| !speck(part.area(), rules.auto_min_area_mm2)).cloned().collect();
            if shared(&pieces, &zone, &bounds, meter)? > 0.0 {
                anchored = true;
                break;
            }
        }
        beside = beside || shared(&area.parts, &zone, &bounds, meter)? > 0.0;
    }
    let mut touching = Vec::new();
    for (next, other) in lines.iter().enumerate().skip(at + 1) {
        meter.charge(1)?;
        let beside = colour(other.from) == own && envelope(std::slice::from_ref(&other.part)).is_some_and(|around| near(&around, 0.0, &bounds));
        if beside && shared(std::slice::from_ref(&other.part), &zone, &bounds, meter)? > 0.0 {
            touching.push(next);
        }
    }
    Ok(Touch { part, anchored, beside, lines: touching })
}

/// Whether `piece` is a part ([`is_part`]) by `rules`, with `meter`'s work.
fn counts(piece: &Polygon, rules: &AutoParams, meter: &mut Meter) -> Result<bool, Failed> {
    let Ok(thread) = Mm::new(THREAD) else { return Ok(false) };
    let holds = !speck(piece.area(), rules.auto_min_area_mm2) && within(piece, thread, meter)? != Some(true);
    Ok(is_part(piece.area(), holds, rules))
}
