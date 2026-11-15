//! Rows that close inside a piece, joined to the rows round them and to a row that meets the piece's outline,
//! as Ink/Stitch joins a parallel offset's and a buffer's rows (read at `d59c9ab`,
//! `lib/stitches/utils/connect_geometries.py`, `REQ-FILL-GDD-011`).
//!
//! **Why join.** A row that meets the outline has its ends there, and the route reaches it along the outline
//! like a tatami fill's row. A ring round a guide inside the shape has none, and sewn as it is, it would be
//! a stretch of its own that the needle jumps to. So each such ring is spliced into the ring round it, and
//! the outermost into a row that meets the outline, and the fill sews as one route.
//!
//! **Sorted out.** A row that meets the outline ring, the outline of the piece without its holes, is an
//! outline row. When it closes and starts inside the piece, it is moved to start at the middle of its first
//! stretch outside it. Every other row lies inside and is an inner ring, closed if it isn't. Without inner
//! rings the rows stay as they are. Without outline rows, the piece's own rings take their place.
//!
//! **Ring into ring.** The inner rings go smallest first, each into the first larger one that covers it.
//! Both are made to run counter-clockwise with y up, as shapely's `is_ccw` reads it, and a walk goes along
//! the inner ring from its start, a CSS pixel at a time. At each step it takes the point there and the point
//! a row spacing on, and their nearest points on the outer ring. Where both pairs lie less than 2½ row
//! spacings apart, the rings are joined there: the outer ring from beside the second point round to a row
//! spacing short of it, then the inner ring from the first point back round to the second. With `skip_last`
//! the inner ring is cut into stitches no longer than the longest stitch first, and its ends are left out.
//! A ring the walk finds no place for is not sewn, nor the rings joined into it.
//!
//! **Ring into row.** Each ring left is joined into the first outline row within a row spacing of it. The
//! walk goes along the ring as before, measuring to the row's parts inside the piece, until both points lie
//! within 1½ row spacings. The row is cut there, a row spacing taken out of it where the earlier of the 2
//! nearest points lies, and the ring, opened a row spacing long at the first point, goes in the gap, the way
//! round that keeps the row from crossing itself. The ring is done when that row meets the outline ring. A
//! row inside a hole's ring isn't enough: the ring, joined into it, is joined on into the next row within a
//! row spacing, or with a row that doesn't start inside, the 2 are crossed into 2 rows. A ring with no row
//! near is tried again after the others, at most 5 more times, and then left out.
//!
//! **Cost.** Each step of a walk charges a unit of work, and each nearest point or distance a unit for each
//! side it measures.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::closed::{closed, closed_up, covered, inside, made_counter_clockwise, outside, rolled, segmentized};
use super::copies::{Bounds, meets};
use super::line::simple;
use crate::generators::tatami::rings::Rings;
use crate::normalize::along::Along;
use crate::normalize::near::{nearest_pair, nearest_to_point};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{Hit, hit, signed_area};

/// How far each step of a walk along a ring goes, in millimetres: a CSS pixel, Ink/Stitch's unit.
const STEP: f64 = MM_PER_SVG_PX;
/// A ring within this of a row spacing of a row is near it, in millimetres: a thousandth of a CSS pixel.
const NEAR: f64 = 1e-3 * MM_PER_SVG_PX;
/// How many times more a ring with no row near is tried.
const RETRIES: usize = 5;

/// The rows of a piece once its inner rings are joined, as the module says. `rows` are the piece's rows in
/// the order found, `spacing` the row spacing, never 0, and `longest` the longest stitch, which cuts an inner
/// ring with `skip_last`. Costs as the module says.
pub(super) fn joined(
    piece: &Polygon,
    rings: &Rings<'_>,
    rows: Vec<Vec<Point>>,
    (spacing, longest, skip_last): (f64, f64, bool),
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let (mut outline_rows, mut inner) = (Vec::new(), Vec::new());
    for row in &rows {
        if !meets(row, rings, meter)? {
            continue;
        }
        if !meets_outline(row, rings, meter)? {
            inner.push(closed_up(&with_4_points(row, meter)?));
            continue;
        }
        let started_inside = match row.first() {
            Some(&p) if closed(row) => rings.inside(p, meter)?,
            _ => false,
        };
        match outside(row, rings, meter)?.filter(|_| started_inside) {
            Some(at) => outline_rows.push(rolled(row, at, meter)?),
            None => outline_rows.push(row.clone()),
        }
    }
    if inner.is_empty() {
        return Ok(rows);
    }
    if outline_rows.is_empty() {
        outline_rows = piece.rings().map(<[Point]>::to_vec).collect();
    }
    inner.sort_by(|a, b| signed_area(a).abs().total_cmp(&signed_area(b).abs()));
    let outer = coverers(&inner, meter)?;
    let mut left: Vec<Option<Vec<Point>>> = inner.into_iter().map(Some).collect();
    for (i, outer) in outer.into_iter().enumerate() {
        let Some(j) = outer else { continue };
        let (Some(Some(ring)), Some(Some(round))) = (left.get(i), left.get(j)) else { continue };
        let found = ring_into_ring(ring, round, (spacing, longest, skip_last), meter)?;
        if let (Some(joined), Some(slot)) = (found, left.get_mut(j)) {
            *slot = Some(joined);
        }
        if let Some(slot) = left.get_mut(i) {
            *slot = None;
        }
    }
    let mut waiting: Vec<Vec<Point>> = left.into_iter().flatten().collect();
    for _ in 0..=RETRIES {
        let mut pass = Pass { removed: Vec::new(), added: Vec::new(), unjoined: Vec::new() };
        for ring in waiting {
            pass.ring_into_rows(rings, ring, &mut outline_rows, spacing, meter)?;
        }
        pass.removed.sort_unstable_by(|a, b| b.cmp(a));
        pass.removed.dedup();
        for i in pass.removed {
            if i < outline_rows.len() {
                outline_rows.remove(i);
            }
        }
        outline_rows.extend(pass.added);
        if pass.unjoined.is_empty() {
            break;
        }
        waiting = pass.unjoined;
    }
    Ok(outline_rows)
}

/// `row`, of 4 points or more: a row of fewer cut into thirds of its length, as Ink/Stitch makes a polygon
/// of it.
fn with_4_points(row: &[Point], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    if row.len() >= 4 {
        return Ok(row.to_vec());
    }
    segmentized(row, Along::new(row, meter)?.length() / 3.0, meter)
}

/// Whether `row` meets the piece's outline ring, the first of `rings`: crosses it, touches it or runs along
/// it. Costs as [`Rings::meeting`] does.
fn meets_outline(row: &[Point], rings: &Rings<'_>, meter: &mut Meter) -> Result<bool, Exhausted> {
    for side in row.windows(2) {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        let mut met = false;
        rings.meeting(p, q, meter, |edge| met |= edge.ring == 0 && hit(p, q, edge.a, edge.b) != Hit::Apart)?;
        if met {
            return Ok(true);
        }
    }
    Ok(false)
}

/// For each of the closed `rings`, smallest first, the first larger one that covers it, if any. A ring whose
/// box doesn't lie in another's isn't covered by it.
fn coverers(rings: &[Vec<Point>], meter: &mut Meter) -> Result<Vec<Option<usize>>, Exhausted> {
    let polygons: Vec<Polygon> = rings.iter().map(|ring| Polygon { outline: ring.clone(), holes: Vec::new() }).collect();
    let boxes: Vec<Bounds> = rings.iter().map(|ring| Bounds::of(ring)).collect();
    let mut found = Vec::with_capacity(rings.len());
    for (i, ring) in rings.iter().enumerate() {
        let mut outer = None;
        for (j, polygon) in polygons.iter().enumerate().skip(i + 1) {
            meter.charge(1)?;
            let (Some(inner_box), Some(outer_box)) = (boxes.get(i), boxes.get(j)) else { continue };
            if outer_box.holds_box(inner_box) && covered(&Rings::new(polygon, meter)?, ring, meter)? {
                outer = Some(j);
                break;
            }
        }
        found.push(outer);
    }
    Ok(found)
}

/// The closed `ring` joined into the closed `round`, which covers it, as the module says; `None` where the
/// walk finds no place. `spacing`, `longest` and `skip_last` are [`joined`]'s.
fn ring_into_ring(
    ring: &[Point],
    round: &[Point],
    (spacing, longest, skip_last): (f64, f64, bool),
    meter: &mut Meter,
) -> Result<Option<Vec<Point>>, Exhausted> {
    let (ring, round) = (made_counter_clockwise(ring.to_vec()), made_counter_clockwise(round.to_vec()));
    let (inner, outer) = (Along::new(&ring, meter)?, Along::new(&round, meter)?);
    let within = spacing + 1.5 * spacing;
    let mut offset = 0.0;
    while offset + spacing < inner.length() {
        meter.charge(1)?;
        let (p1, p2) = (inner.point(offset), inner.point(offset + spacing));
        let (Some((_, d1)), Some((q2, d2))) = (nearest_to_point(&[&round], p1, meter)?, nearest_to_point(&[&round], p2, meter)?) else { break };
        if d1 < within && d2 < within {
            let ring_rolled = rolled(&ring, inner.project(p2, meter)?, meter)?;
            let round_rolled = rolled(&round, outer.project(q2, meter)?, meter)?;
            let (ring_along, round_along) = (Along::new(&ring_rolled, meter)?, Along::new(&round_rolled, meter)?);
            let back = sub(&ring_along, 0.0, ring_along.length() - spacing);
            let round_part = sub(&round_along, 0.0, round_along.length() - spacing);
            let mut back = if skip_last {
                let cut = segmentized(&back, longest, meter)?;
                cut.get(1..cut.len().saturating_sub(1)).unwrap_or_default().to_vec()
            } else {
                back
            };
            back.reverse();
            return Ok(Some(closed_up(&[round_part, back].concat())));
        }
        offset += STEP;
    }
    Ok(None)
}

/// What a pass that joins rings into the outline rows found: the rows taken out, those added, and the rings
/// it found no row near.
struct Pass {
    removed: Vec<usize>,
    added: Vec<Vec<Point>>,
    unjoined: Vec<Vec<Point>>,
}

impl Pass {
    /// `ring` joined into the first of `rows` within a row spacing of it, and on into a second where the
    /// first doesn't meet the outline ring, as the module says; kept for the next pass where none is near.
    fn ring_into_rows(&mut self, rings: &Rings<'_>, ring: Vec<Point>, rows: &mut [Vec<Point>], spacing: f64, meter: &mut Meter) -> Result<(), Exhausted> {
        let original = ring.clone();
        let (mut ring, mut joined_at) = (ring, None);
        for i in 0..rows.len() {
            if self.removed.contains(&i) {
                continue;
            }
            let Some(row) = rows.get(i).cloned() else { break };
            let near = nearest_pair(&[&original], &[&row], meter)?.is_some_and(|(a, b)| a.distance(b) <= spacing + NEAR);
            if !near {
                continue;
            }
            match joined_at {
                None => {
                    let Some(joined) = ring_into_row(rings, &ring, &row, spacing, meter)? else { continue };
                    if let Some(slot) = rows.get_mut(i) {
                        *slot = joined.clone();
                    }
                    ring = joined;
                    joined_at = Some(i);
                    if meets_outline(&row, rings, meter)? {
                        return Ok(());
                    }
                }
                Some(at) => {
                    let starts_inside = match row.first() {
                        Some(&p) => rings.inside(p, meter)?,
                        None => false,
                    };
                    let first = if starts_inside {
                        let Some(joined) = ring_into_row(rings, &ring, &row, spacing, meter)? else { continue };
                        joined
                    } else {
                        let (first, second) = crossed(&ring, &row, spacing, meter)?;
                        self.added.push(second);
                        first
                    };
                    if let Some(slot) = rows.get_mut(at) {
                        *slot = first;
                    }
                    self.removed.push(i);
                    return Ok(());
                }
            }
        }
        self.unjoined.push(ring);
        Ok(())
    }
}

/// `ring` spliced into `row` as the module says; `None` where `row` has no part in the piece or the walk
/// finds no place.
fn ring_into_row(rings: &Rings<'_>, ring: &[Point], row: &[Point], spacing: f64, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let parts = inside(row, rings, meter)?;
    if parts.is_empty() {
        return Ok(None);
    }
    let parts: Vec<&[Point]> = parts.iter().map(Vec::as_slice).collect();
    let ring = closed_up(ring);
    let around = Along::new(&ring, meter)?;
    let along = Along::new(row, meter)?;
    let within = 1.5 * spacing;
    let mut offset = 0.0;
    while around.length() >= spacing + offset {
        meter.charge(1)?;
        let (p1, p2) = (around.point(offset), around.point(offset + spacing));
        let (Some((p3, d1)), Some((p4, d2))) = (nearest_to_point(&parts, p1, meter)?, nearest_to_point(&parts, p2, meter)?) else { break };
        if d1 < within && d2 < within {
            let at = along.project(p3, meter)?.min(along.project(p4, meter)?);
            let before = sub(&along, 0.0, at);
            let after = sub(&along, at + spacing, along.length());
            let opened = rolled(&ring, around.project(p1, meter)?, meter)?;
            let opened_along = Along::new(&opened, meter)?;
            let mut middle = sub(&opened_along, spacing, opened_along.length());
            let joined = [before.as_slice(), &middle, &after].concat();
            if simple(&joined, meter)? {
                return Ok(Some(joined));
            }
            middle.reverse();
            return Ok(Some([before, middle, after].concat()));
        }
        offset += STEP;
    }
    Ok(None)
}

/// The lines `first` and `second` crossed into 2, where they come nearest: `first` up to its nearest point,
/// then `second` back from its nearest point to its start; and `first` back from its end to a row spacing
/// past its nearest point, then `second` on from a row spacing past its own to its end. Where the 2 lines
/// across would cross, `second` is taken the other way round and the second line across goes a row spacing
/// back. Costs a unit of work for each pair of sides measured, and as measuring along them does.
fn crossed(first: &[Point], second: &[Point], spacing: f64, meter: &mut Meter) -> Result<(Vec<Point>, Vec<Point>), Exhausted> {
    let Some((p1, p3)) = nearest_pair(&[first], &[second], meter)? else { return Ok((first.to_vec(), second.to_vec())) };
    let one = Along::new(first, meter)?;
    let two = Along::new(second, meter)?;
    let p2 = one.point(one.project(p1, meter)? + spacing);
    let at3 = two.project(p3, meter)?;
    let mut p4 = two.point(at3 + spacing);
    let mut second = second.to_vec();
    if hit(p1, p3, p2, p4) != Hit::Apart {
        let back = at3 - spacing;
        p4 = two.point(if back < 0.0 { two.length() + back } else { back });
        second.reverse();
    }
    let two = Along::new(&second, meter)?;
    let mut to_start = sub(&two, 0.0, two.project(p3, meter)?);
    to_start.reverse();
    let mut from_end = sub(&one, one.project(p2, meter)?, one.length());
    from_end.reverse();
    let onward = sub(&two, two.project(p4, meter)?, two.length());
    Ok(([sub(&one, 0.0, one.project(p1, meter)?), to_start].concat(), [from_end, onward].concat()))
}

/// The part of the line `along` measures from `from` to `to`, each held to its ends: one point where they
/// meet or cross, as shapely's `substring` gives a point.
fn sub(along: &Along<'_>, from: f64, to: f64) -> Vec<Point> {
    let (from, to) = (from.clamp(0.0, along.length()), to.clamp(0.0, along.length()));
    if to <= from { vec![along.point(from)] } else { along.part(from, to) }
}

#[cfg(test)]
mod tests;
