//! A guided fill's rows over one piece with the buffer strategy: the rings round its guide line, a row
//! spacing apart, as Ink/Stitch buffers its guide (read at `d59c9ab`, `REQ-FILL-GDD-010`).
//!
//! **Why rings.** A guide inside a shape, or at its edge, is a middle the rows grow out from: each row runs
//! round the guide at its own distance, like the rings of a tree round a branch. Every subpath of the guide
//! counts, so rings round 2 short lines grow apart and then join into one.
//!
//! **The guide.** Each subpath, flattened, is simplified within a ten-thousandth of a CSS pixel, which leaves
//! out only points in line with their neighbours, and set in shapely's normal order: an open line runs from
//! its lesser end, x then y, and the lines go greatest first. That order says where each ring starts.
//!
//! **The rings.** Row `k` is the edge of the area within `(k + ½)` row spacings of the guide, the first a
//! ten-thousandth of a CSS pixel farther, as Ink/Stitch takes the boundary of shapely's buffer: round each
//! open subpath with round caps and corners of 16 segments to a quarter circle, and round each closed one on
//! both its sides ([`crate::normalize::offset::line_curves`]). Its outlines run clockwise with y up and its
//! holes the other way, and every ring is a row that meets the piece. The rows start at the whole number of
//! row spacings the guide lies from the piece, 0 where it meets the piece, and go outward while their rings
//! meet the piece's bounds. GEOS starts a line's buffer where the raw curve round it starts, at its first
//! side's end moved to its left with y up, and each ring here starts at its point nearest that.
//!
//! **Cost.** Each ring costs as its raw curves and the area they wind round do, and each test of a row
//! against the piece looks only at the sides of the rings near it.

use std::cmp::Ordering;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::Guided;
use super::copies::{Bounds, meets};
use super::offset::ROW_0;
use crate::generators::tatami::rings::Rings;
use crate::normalize::near::nearest_pair;
use crate::normalize::offset::line_curves;
use crate::normalize::region::geom::{Hit, hit};
use crate::normalize::region::{Polygon, area};
use crate::normalize::simplify::simplify;

/// How far a guide's subpath is simplified, in millimetres: a ten-thousandth of a CSS pixel, as Ink/Stitch
/// simplifies a buffered guide.
const SIMPLIFY: f64 = 1e-4 * MM_PER_SVG_PX;

/// The guide line's flattened `subpaths` made ready to buffer, as the module says. A unit of `meter`'s work
/// for each point looked at.
pub(super) fn prepared(subpaths: Vec<Vec<Point>>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let mut lines = Vec::with_capacity(subpaths.len());
    for subpath in subpaths {
        let mut line = simplify(&subpath, SIMPLIFY, meter)?;
        if lesser_from_end(&line) {
            line.reverse();
        }
        lines.push(line);
    }
    // Greatest first, as GEOS sorts a collection it normalizes.
    lines.sort_by(|a, b| compared(b, a));
    Ok(lines)
}

/// Whether `line` runs from its greater end, as GEOS tells it: its points compared from both ends inward, x
/// then y, the first pair that differs deciding.
fn lesser_from_end(line: &[Point]) -> bool {
    line.iter().zip(line.iter().rev()).take(line.len() / 2).find(|(a, b)| a != b).is_some_and(|(a, b)| coordinate(*a, *b) == Ordering::Greater)
}

/// 2 points compared x first, then y.
fn coordinate(a: Point, b: Point) -> Ordering {
    a.x().total_cmp(&b.x()).then(a.y().total_cmp(&b.y()))
}

/// 2 lines compared point by point, a line that runs out first being the lesser.
fn compared(a: &[Point], b: &[Point]) -> Ordering {
    a.iter().zip(b).map(|(p, q)| coordinate(*p, *q)).find(|order| order.is_ne()).unwrap_or_else(|| a.len().cmp(&b.len()))
}

/// The rows the prepared guide `lines` gives over `piece`, whose rings are `rings`, as `guided` spaces them:
/// the rings round it that meet the piece, nearest the guide first, as the module says. `unjoined` is set
/// where a buffer's edges don't join into rings, a bug. Costs as the module says.
pub(super) fn rows(
    piece: &Polygon,
    rings: &Rings<'_>,
    lines: &[Vec<Point>],
    guided: &Guided,
    unjoined: &mut bool,
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let bounds = Bounds::of(&piece.outline);
    let away = distance(piece, rings, lines, meter)?;
    #[allow(clippy::cast_possible_truncation)] // A row count far below i64's range, the bounds held to the working limit.
    let mut k = (away / guided.spacing).trunc() as i64;
    let mut rows = Vec::new();
    loop {
        meter.charge(1)?;
        #[allow(clippy::cast_precision_loss)] // Row numbers are far below 2⁵².
        let by = k as f64 * guided.spacing;
        let by = if by == 0.0 { ROW_0 } else { by } + guided.spacing / 2.0;
        let curves = line_curves(lines, by, meter)?;
        let Ok(parts) = area(&curves, |w| w <= -1, meter)?.parts else { *unjoined = true; break };
        let start = curves.first().and_then(|curve| curve.first()).copied();
        let mut met = false;
        let mut found = Vec::new();
        for ring in parts.into_iter().flat_map(|part| std::iter::once(part.outline).chain(part.holes)) {
            met |= bounds.met_by(&ring, meter)?;
            if meets(&ring, rings, meter)? {
                found.push(start.map_or_else(|| ring.clone(), |start| started_nearest(&ring, start)));
            }
        }
        if !met {
            break;
        }
        rows.extend(found);
        k += 1;
    }
    Ok(rows)
}

/// How far the guide `lines` lies from `piece`, whose rings are `rings`: 0 where a point of it lies in the
/// piece or on its edge, or a side of it meets the edge, and otherwise the distance between their nearest
/// points. Costs as [`Rings::covers`] and [`Rings::meeting`] do, and a unit of `meter`'s work for each pair
/// of sides measured.
fn distance(piece: &Polygon, rings: &Rings<'_>, lines: &[Vec<Point>], meter: &mut Meter) -> Result<f64, Exhausted> {
    for line in lines {
        if let Some(&p) = line.first()
            && rings.covers(p, meter)?
        {
            return Ok(0.0);
        }
        for side in line.windows(2) {
            meter.charge(1)?;
            let (p, q) = (side[0], side[1]);
            let mut met = false;
            rings.meeting(p, q, meter, |edge| met |= hit(p, q, edge.a, edge.b) != Hit::Apart)?;
            if met || rings.covers(q, meter)? {
                return Ok(0.0);
            }
        }
    }
    let guide: Vec<&[Point]> = lines.iter().map(Vec::as_slice).collect();
    let edge: Vec<&[Point]> = piece.rings().collect();
    Ok(nearest_pair(&guide, &edge, meter)?.map_or(0.0, |(a, b)| a.distance(b)))
}

/// The closed `ring` started at its point nearest `mark`, the first of equally near ones.
fn started_nearest(ring: &[Point], mark: Point) -> Vec<Point> {
    let open = ring.len().saturating_sub(1);
    let nearest = ring.iter().take(open).enumerate().min_by(|a, b| a.1.distance(mark).total_cmp(&b.1.distance(mark))).map_or(0, |(i, _)| i);
    let (Some(head), Some(tail)) = (ring.get(..nearest), ring.get(nearest..open)) else { return ring.to_vec() };
    tail.iter().chain(head).chain(tail.first()).copied().collect()
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::p;
    use crate::normalize::region::geom::signed_area;

    fn xy(points: &[Point]) -> Vec<(f64, f64)> {
        points.iter().map(|q| ((q.x() * 1e6).round() / 1e6, (q.y() * 1e6).round() / 1e6)).collect()
    }

    /// The rings of the buffer of `lines` at `by`.
    fn buffered(lines: &[Vec<Point>], by: f64) -> Vec<Vec<Point>> {
        let meter = &mut Budget::DEFAULT.meter();
        let curves = line_curves(lines, by, meter).unwrap();
        let parts = area(&curves, |w| w <= -1, meter).unwrap().parts.unwrap();
        let start = curves[0][0];
        parts.into_iter().flat_map(|part| std::iter::once(part.outline).chain(part.holes)).map(|ring| started_nearest(&ring, start)).collect()
    }

    #[test]
    fn req_fill_gdd_010_a_line_s_buffer_starts_where_its_raw_curve_does() {
        // As shapely 2.2 on GEOS 3.14 buffers them: a line along x by 1 is 2 sides and 2 half circles of 32
        // segments, from its end moved to its left with y up, clockwise; a bent line from its first corner.
        let along = buffered(&[vec![p(0.0, 0.0), p(10.0, 0.0)]], 1.0);
        assert_eq!((along.len(), along[0].len()), (1, 67));
        assert_eq!(xy(&along[0][..2]), [(10.0, 1.0), (10.098017, 0.995185)]);
        assert!(signed_area(&along[0]) < 0.0);
        let bent = buffered(&[vec![p(0.0, 0.0), p(3.0, 2.0), p(6.0, -2.0), p(9.0, 0.0)]], 2.0);
        assert_eq!(xy(&bent[0][..1]), [(1.890599, 3.664101)]);
        // A closed line: an outline and a hole, the hole running the other way.
        let square = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)];
        let ring = buffered(&[square], 1.0);
        assert_eq!(ring.len(), 2);
        assert!(signed_area(&ring[0]) < 0.0 && signed_area(&ring[1]) > 0.0);
        assert!((signed_area(&ring[1]) - 64.0).abs() < 1e-9, "the hole is the square shrunk by 1");
        // 2 lines apart, and close enough to join.
        let two = |gap: f64| buffered(&[vec![p(0.0, 0.0), p(10.0, 0.0)], vec![p(0.0, gap), p(10.0, gap)]], 1.0).len();
        assert_eq!((two(5.0), two(1.5)), (2, 1));
    }

    #[test]
    fn req_fill_gdd_010_the_guide_is_simplified_and_set_in_shapely_s_order() {
        let meter = &mut Budget::DEFAULT.meter();
        // A line from its greater end turned round, a point in line left out, and the lines greatest first, as
        // shapely normalizes MULTILINESTRING ((10 0, 0 0), (0 5, 5 5.00001, 10 5)).
        let lines = prepared(vec![vec![p(10.0, 0.0), p(0.0, 0.0)], vec![p(0.0, 5.0), p(5.0, 5.000_000_01), p(10.0, 5.0)]], meter).unwrap();
        assert_eq!(lines.iter().map(|line| xy(line)).collect::<Vec<_>>(), [vec![(0.0, 5.0), (10.0, 5.0)], vec![(0.0, 0.0), (10.0, 0.0)]]);
        // A closed line keeps its start, its sides compared inward from both ends.
        let closed = [p(0.0, 0.0), p(0.0, 10.0), p(10.0, 10.0), p(10.0, 0.0), p(0.0, 0.0)];
        assert!(lesser_from_end(&closed));
        assert!(!lesser_from_end(&[p(0.0, 0.0), p(1.0, 1.0), p(0.0, 0.0)]));
        assert_eq!(compared(&closed, &closed[..2]), Ordering::Greater);
    }

    #[test]
    fn req_fill_gdd_010_rows_start_at_the_whole_spacings_the_guide_lies_away() {
        let meter = &mut Budget::DEFAULT.meter();
        let part = crate::generators::tatami::fixture::rectangle(4.0);
        let rings = Rings::new(&part, meter).unwrap();
        let inside = [vec![p(4.0, 2.0), p(6.0, 2.0)]];
        assert_eq!(distance(&part, &rings, &inside, meter).unwrap(), 0.0);
        let across = [vec![p(-2.0, 2.0), p(-1.0, 2.0), p(12.0, 2.0)]];
        assert_eq!(distance(&part, &rings, &across, meter).unwrap(), 0.0);
        let below = [vec![p(0.0, -3.0), p(10.0, -3.0)]];
        assert_eq!(distance(&part, &rings, &below, meter).unwrap(), 3.0);
    }
}
