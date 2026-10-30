//! Specks: automatic areas, or parts of them, too small to sew (`REQ-AUTO-008`), each said with
//! `SC-I0905`.
//!
//! A trace leaves specks where the picture's pixels were noisy. A fill smaller than [`SMALLEST_AREA`] sews
//! a knot, and leaving it out shows fabric where it was. So each speck is merged into the automatic area it
//! shares the longest edge with: the area around it when it fills a hole, which then closes, or the rest of
//! its own area, when rounding left the two a hair apart. On a tie, it goes into the area drawn first. A
//! speck that shares no edge with an automatic area has nothing to go into, and is left out.
//!
//! The edge a speck shares with an area is how much of the area's rings lies within [`TOUCHING`] of the
//! speck: the speck grown by that much, and each segment of the rings cut where it crosses the grown
//! speck's rings. Only the segments whose bounds come that near are cut, so a speck costs the work of the
//! rings beside it, and a unit for each other area it looks past. Every area is split into the parts it
//! keeps and its specks first, so a speck goes into what its neighbours keep, never into another speck. A
//! speck merged in is joined to the area; where rounding left a gap between them, the speck grown by
//! [`TOUCHING`] bridges it.
//!
//! Each speck has the budget's work to itself. One that needs more is left out, and that is said: put back
//! as drawn, it could overlap a speck merged into its area beside it, and the parts of a region never
//! overlap.

use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Meter, Point};

use super::{Failed, SMALLEST_AREA, Shapes, TOUCHING, envelope, joined, near};
use crate::generators::mm;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops;

/// The specks of `shapes`' automatic areas merged into their neighbours or left out, each with what is
/// said of it. Each speck has the budget's work to itself; one that needs more is left out.
pub(super) fn merged(shapes: &mut Shapes, budget: &Budget) -> Vec<Diagnostic> {
    // Every area split into the parts it keeps and its specks.
    let mut specks = Vec::new();
    for index in 0..shapes.len() {
        let Some(parts) = shapes.region(index) else { continue };
        let (small, kept): (Vec<Polygon>, Vec<Polygon>) = parts.iter().cloned().partition(|part| part.area() < SMALLEST_AREA);
        if small.is_empty() {
            continue;
        }
        let whole = kept.is_empty() && small.len() == 1;
        shapes.set_region(index, kept);
        specks.extend(small.into_iter().map(|speck| Speck { index, speck, whole }));
    }
    let mut said = Vec::with_capacity(specks.len());
    for speck in specks {
        let Some(id) = shapes.id(speck.index) else { continue };
        let this = speck.this();
        let message = match into(shapes, &speck, &mut budget.meter()) {
            Ok(Some(index)) if index == speck.index => {
                format!("{this} is too small to sew, so it is joined to the rest of this area, which it shares the longest edge with.")
            }
            Ok(Some(index)) => {
                let into = shapes.id(index).map(|id| id.as_str().to_string()).unwrap_or_default();
                format!("{this} is too small to sew, so it is merged into `{into}`, which it shares the longest edge with.")
            }
            Ok(None) => format!("{this} is too small to sew and shares no edge with an automatic area, so it is left out."),
            Err(failed) => {
                said.push(failed.said(Code::AutoSpeck, "where to merge this speck", "it is left out", budget.max_work, id));
                continue;
            }
        };
        said.push(
            Diagnostic::new(Code::AutoSpeck, message).with_element(id).with_fix(Fix::Hint("To sew it as it is drawn, give it settings.".to_string())),
        );
    }
    said
}

/// A speck: a part of the automatic area `index` too small to sew, its only part when `whole`.
struct Speck {
    index: usize,
    speck: Polygon,
    whole: bool,
}

impl Speck {
    /// The speck as its message names it, by its area to a hundredth of a square millimetre, which rounding
    /// leaves under [`SMALLEST_AREA`].
    fn this(&self) -> String {
        let hundredths = (self.speck.area() * 100.0).round().min((SMALLEST_AREA * 100.0).round() - 1.0);
        let area = mm(hundredths / 100.0);
        if self.whole { format!("This area of {area} mm²") } else { format!("A part of this area, of {area} mm²,") }
    }
}

/// Merges `speck` into the automatic area of `shapes` it shares the longest edge with, its own included,
/// and gives that area's index; `None` when it shares no edge with one, and is left out.
fn into(shapes: &mut Shapes, speck: &Speck, meter: &mut Meter) -> Result<Option<usize>, Failed> {
    let zone = joined(ops::buffered(std::slice::from_ref(&speck.speck), TOUCHING, meter))?;
    let Some(bounds) = envelope(&zone) else { return Ok(None) };
    let mut best: Option<(usize, f64)> = None;
    for index in 0..shapes.len() {
        meter.charge(1)?;
        if !shapes.area.get(index).copied().unwrap_or(false) || !shapes.bounds(index).is_some_and(|area| near(&area, 0.0, &bounds)) {
            continue;
        }
        let Some(parts) = shapes.region(index) else { continue };
        let shared = shared(parts, &zone, &bounds, meter)?;
        if shared > 0.0 && best.is_none_or(|(_, longest)| shared > longest) {
            best = Some((index, shared));
        }
    }
    let Some((index, _)) = best else { return Ok(None) };
    let parts = shapes.region(index).map(<[Polygon]>::to_vec).unwrap_or_default();
    let mut merged = joined(ops::union([parts.as_slice(), std::slice::from_ref(&speck.speck)], meter))?;
    // Joined, the speck closes up with the area; apart, rounding left a gap, which the zone bridges.
    if merged.len() > parts.len() {
        merged = joined(ops::union([parts.as_slice(), zone.as_slice()], meter))?;
    }
    shapes.set_region(index, merged);
    Ok(Some(index))
}

/// How long a stretch of the rings of `parts` lies within `zone`, whose bounds are `bounds`.
pub(super) fn shared(parts: &[Polygon], zone: &[Polygon], bounds: &Envelope, meter: &mut Meter) -> Result<f64, Failed> {
    let mut length = 0.0;
    for ring in parts.iter().flat_map(Polygon::rings) {
        for pair in ring.windows(2) {
            meter.charge(1)?;
            let [a, b] = [pair[0], pair[1]];
            if near(&Envelope::of(&[a, b]), 0.0, bounds) {
                length += within(a, b, zone, meter)?;
            }
        }
    }
    Ok(length)
}

/// How long a stretch of the segment from `a` to `b` lies within `zone`: the segment cut where it crosses
/// the zone's rings, and each piece kept when its middle lies in the zone.
pub(super) fn within(a: Point, b: Point, zone: &[Polygon], meter: &mut Meter) -> Result<f64, Failed> {
    let mut cuts = vec![0.0, 1.0];
    for ring in zone.iter().flat_map(Polygon::rings) {
        for pair in ring.windows(2) {
            meter.charge(1)?;
            cuts.extend(crossing(a, b, pair[0], pair[1]));
        }
    }
    cuts.sort_by(f64::total_cmp);
    let mut inside = 0.0;
    for pair in cuts.windows(2) {
        let [from, to] = [pair[0], pair[1]];
        if zone.iter().any(|part| part.covers(a.lerp(b, (from + to) / 2.0))) {
            inside += to - from;
        }
    }
    Ok(inside * a.distance(b))
}

/// Where along the segment from `a` to `b`, as a fraction of it, it crosses the segment from `c` to `d`;
/// `None` when they don't cross, or run side by side.
pub(super) fn crossing(a: Point, b: Point, c: Point, d: Point) -> Option<f64> {
    let (ab, cd, ac) = ((b.x() - a.x(), b.y() - a.y()), (d.x() - c.x(), d.y() - c.y()), (c.x() - a.x(), c.y() - a.y()));
    let cross = |u: (f64, f64), v: (f64, f64)| u.0 * v.1 - u.1 * v.0;
    let denominator = cross(ab, cd);
    if denominator == 0.0 {
        return None;
    }
    let (t, s) = (cross(ac, cd) / denominator, cross(ac, ab) / denominator);
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&s)).then_some(t)
}
