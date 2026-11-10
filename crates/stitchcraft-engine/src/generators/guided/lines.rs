//! A guided fill's rows over one piece made into the lines it sews: each row smoothed, stitched from its
//! stagger, started outside the piece where it closes, and cut by the piece, as Ink/Stitch staggers and cuts
//! its rows (read at `d59c9ab`, `REQ-FILL-GDD-005`, `REQ-FILL-GDD-012`).
//!
//! **Smoothed** by `smoothness_mm`, each row first.
//!
//! **A closed row** that doesn't cross itself, and starts inside the piece or has an anchor line beside it,
//! is started elsewhere before it is stitched. Where it crosses the anchor line, it is cut at each crossing,
//! and each piece is stitched on its own from the crossing it starts at, the anchor line setting where its
//! needle points start ([`anchored`]). Otherwise it starts at the middle of its first stretch outside the
//! piece's outline, holes left out, and is stitched from there. A closed row with no such stretch is left
//! as it is, its own points its needle points, as Ink/Stitch leaves it. Any other row is stitched from its
//! own start.
//!
//! **The stagger** counts from the row's number in the order the rows are taken, and for a buffer's ring
//! from the whole number of row spacings, rounded half to even as Python rounds, that it lies from the guide,
//! so that rings the same distance away start alike ([`super::stitched`]).
//!
//! **Started outside.** A row the anchor line started, and a stitched row that starts inside the piece, is
//! started again at the middle of its first stretch outside the outline, so that the cut by the piece starts
//! there. A row that doesn't close is closed first, as shapely does to a line it rolls.
//!
//! **Cut** by the piece ([`super::cut`]).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::closed::{closed, closed_up, outside, rolled};
use super::line::simple;
use super::stitched::{smoothed, stitched};
use super::{Guided, Strategy, cut};
use crate::generators::tatami::rings::Rings;
use crate::normalize::along::Along;
use crate::normalize::near::nearest_pair;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{Hit, hit};

/// What the rows of a piece are stitched with: the lines the strategy offset or copied, which a buffer's
/// rings are measured from, and the anchor line's subpaths, flattened, none without one.
pub(super) struct Lines<'a> {
    pub(super) guide: &'a [Vec<Point>],
    pub(super) anchor: &'a [Vec<Point>],
}

/// The lines `rows`, taken in their order, sew over `piece`, whose rings are `rings`, as the module says.
/// Each row's random lengths draw from a stream of `rng` that its number keys. Costs as stitching and
/// cutting the rows do.
pub(super) fn lines(
    piece: &Polygon,
    rings: &Rings<'_>,
    rows: &[Vec<Point>],
    guided: &Guided,
    with: &Lines<'_>,
    rng: &SplitMix64,
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let outline = Polygon { outline: piece.outline.clone(), holes: Vec::new() };
    let outline_rings = Rings::new(&outline, meter)?;
    let mut lines = Vec::new();
    for (i, row) in rows.iter().enumerate() {
        let row = smoothed(row, guided, meter)?;
        let starts_inside = |line: &[Point], meter: &mut Meter| match line.first() {
            Some(&p) => rings.inside(p, meter),
            None => Ok(false),
        };
        let ring = closed(&row) && simple(&row, meter)?;
        let mut moved = false;
        let sewn = if ring && (!with.anchor.is_empty() || starts_inside(&row, meter)?) {
            match anchored(&row, i, guided, with, rng, meter)? {
                Some(sewn) => {
                    moved = true;
                    sewn
                }
                None => match outside(&row, &outline_rings, meter)? {
                    Some(at) => {
                        let started = rolled(&row, at, meter)?;
                        stitched(&started, place(&started, i, guided, with, meter)?, i, guided, rng, meter)?
                    }
                    None => row,
                },
            }
        } else {
            stitched(&row, place(&row, i, guided, with, meter)?, i, guided, rng, meter)?
        };
        let sewn = match moved || starts_inside(&sewn, meter)? {
            true => match outside(&sewn, &outline_rings, meter)? {
                Some(at) => rolled(&closed_up(&sewn), at, meter)?,
                None => sewn,
            },
            false => sewn,
        };
        lines.extend(cut::cut(&sewn, rings, meter)?);
    }
    Ok(lines)
}

/// Where the stagger of `line`, the `i`-th row, counts from: `i`, or for a buffer's ring the row spacings
/// it lies from the guide, rounded half to even. Costs a unit of work for each pair of sides measured.
fn place(line: &[Point], i: usize, guided: &Guided, with: &Lines<'_>, meter: &mut Meter) -> Result<f64, Exhausted> {
    if guided.strategy != Strategy::Buffer {
        #[allow(clippy::cast_precision_loss)] // Row counts are far below 2⁵².
        return Ok(i as f64);
    }
    let guide: Vec<&[Point]> = with.guide.iter().map(Vec::as_slice).collect();
    let away = nearest_pair(&[line], &guide, meter)?.map_or(0.0, |(a, b)| a.distance(b));
    Ok((away / guided.spacing).round_ties_even())
}

/// The closed `row`, the `i`-th, stitched from where it crosses or touches the anchor line: started at the
/// first crossing along it, cut at the others, and each piece stitched from its start, as the module says;
/// `None` where it doesn't meet the anchor line. Costs as finding the crossings, stitching and measuring do.
fn anchored(row: &[Point], i: usize, guided: &Guided, with: &Lines<'_>, rng: &SplitMix64, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let along = Along::new(row, meter)?;
    let mut crossings = Vec::new();
    for (side, &start) in row.windows(2).zip(&along.at) {
        for anchor in with.anchor {
            for other in anchor.windows(2) {
                meter.charge(1)?;
                if let Hit::Cross(x) | Hit::Touch(x) = hit(side[0], side[1], other[0], other[1]) {
                    crossings.push(start + side[0].distance(x));
                }
            }
        }
    }
    crossings.sort_by(f64::total_cmp);
    crossings.dedup();
    let Some(&first) = crossings.first() else { return Ok(None) };
    let started = rolled(row, first, meter)?;
    let around = Along::new(&started, meter)?;
    let cuts: Vec<f64> = std::iter::once(0.0).chain(crossings.iter().skip(1).map(|at| at - first)).chain(std::iter::once(around.length())).collect();
    let mut sewn = Vec::new();
    for pair in cuts.windows(2) {
        let (from, to) = (pair[0], pair[1]);
        if to > from {
            let piece = around.part(from, to);
            sewn.extend(stitched(&piece, place(&piece, i, guided, with, meter)?, i, guided, rng, meter)?);
        }
    }
    Ok(Some(sewn))
}
