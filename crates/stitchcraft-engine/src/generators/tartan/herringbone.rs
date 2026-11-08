//! A tartan's herringbone (milestone M10.7): its rows in bands `herringbone_width_mm` wide, each band's rows
//! slanting the other way from the band before, and the weft's rows joined across the bands, as Ink/Stitch
//! lays them (`docs/src/design/algorithms/tartan.md` › Herringbone, read at `d59c9ab`).
//!
//! **Why bands.** A herringbone twill turns its diagonal ribs at the edge of each band, so that the cloth
//! shows columns of chevrons. The rows do the same. In the sett's frame, before it is turned, the bands
//! stand upright side by side from the box's left side, a hair apart, and each band's rows cross it at 45°:
//! down to the right in the first band, as the canvas shows them, up to the right in the next, and so on.
//! The rows' angle is the herringbone's, and `tartan_angle` doesn't turn them.
//!
//! **The rows of a band** join its 2 sides, each as long as the box is high plus the band's width and cut
//! into equal steps no longer than the row spacing: a row from each step on the left side to the same step
//! on the right. Every band has the same rows, moved across and slanted its own way, so they are held once
//! ([`Herringbone`]). Their needle points are staggered from the band's first row. In the first band, the
//! first `rows_per_thread` rows of every 2 × `rows_per_thread` go to the weft and the others to the warp,
//! and in the next band the other way round, and so on.
//!
//! **The warp** ([`Herringbone::warp`]) is laid band by band, the second, fourth and so on first, then the
//! first, third and so on: in each band, each warp stripe's rows are joined along the stripe's edges as a
//! straight tartan's are, and cut by the piece, but a line that ends inside the piece doesn't run on to its
//! rings. A colour comes in the order its first line does.
//!
//! **The weft** ([`Herringbone::weft`]) is joined across the bands. Each band's weft rows are first joined
//! end to start into one zigzag, which each weft stripe cuts into pieces. A band's pieces in a stripe are
//! joined along the stripe's edges, and then, while an edge of the stripe crosses the piece, the bands'
//! lines are joined into one, from the band at one end of the stripe to the band at the other, each joined
//! to the next along the edge its end lies on. That one line is cut by the piece, nudged by a hair as Ink/Stitch
//! nudges it. Every colour whose weft stripes reach the piece is listed, lines or none.
//!
//! **What it costs.** The rows of every band, each needle point, each row tested against a stripe and each
//! point of a zigzag are charged to the meter before memory is taken for them.

use std::f64::consts::{FRAC_1_SQRT_2, SQRT_2};

use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_plan::Rgb;

use super::Tartan;
use super::sett::PX;
use super::stripes::{
    Bounds, Direction, Frame, Laying, Row, SHORTEST_STRETCH, Taken, Turn, Walked, add, in_turns, needle_points, segmentized, steps,
};
use crate::generators::made;
use crate::normalize::region::cut::inside;

/// The gap Ink/Stitch leaves between a herringbone's bands, in millimetres: 0.005 pixel, so that no 2 bands'
/// rows meet.
pub(super) const GAP: f64 = 0.005 * PX;

/// How far a row's ends may lie from where its step puts them, in millimetres, for the rounding of the
/// arithmetic that finds them: a row this near a stripe is taken as reaching it.
const ROUNDING: f64 = 1e-9;

/// How far Ink/Stitch moves the weft's lines right and down, as the canvas shows them, before it cuts them by
/// the piece, in millimetres: 0.00123 pixel. A line along the piece's outline then falls inside or outside it as
/// it does there.
pub(super) const NUDGE: f64 = 0.00123 * PX;

/// A herringbone's rows across the box, in the sett's frame. Every band has the same rows, moved across and
/// slanted its own way, so they are held once.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Herringbone {
    /// The first band's left side, the box's, and how far each band's lies from the one before's: the bands'
    /// width and the gap.
    left: f64,
    pitch: f64,
    /// How many bands there are: as many as start before the box's right side.
    bands: usize,
    /// The bands' width.
    width: f64,
    /// The box's top and its height.
    top: f64,
    height: f64,
    /// How many equal steps cut each band's sides.
    count: usize,
    /// Along each row, how far its needle points lie from its start, its ends included, by its place in its
    /// band.
    points: Vec<Vec<f64>>,
    /// A band's rows in the order taken, each with whether it is sewn back: the first `rows_per_thread` of
    /// every 2 × `rows_per_thread`, then the others.
    groups: [Vec<(usize, bool)>; 2],
}

impl Herringbone {
    /// The herringbone of `tartan`, `tartan.herringbone` mm wide, across `frame`, as the module's docs say.
    /// The meter is charged for every band's rows, as if each were made, and for the needle points.
    pub(super) fn new(frame: &Frame, tartan: &Tartan, meter: &mut Meter) -> Result<Herringbone, Exhausted> {
        let width = tartan.herringbone;
        let ([left, top], [right, bottom]) = (frame.min, frame.max);
        let height = bottom - top;
        let pitch = width + GAP;
        let count = steps(((height + width) / tartan.spacing).ceil(), 1);
        let bands = steps(((right - left) / pitch).ceil(), 0);
        let rows = u64::try_from(count).unwrap_or(u64::MAX).saturating_add(1);
        meter.charge(rows.saturating_mul(u64::try_from(bands).unwrap_or(u64::MAX)))?;
        let mut points = Vec::with_capacity(count + 1);
        for k in 0..=count {
            points.push(needle_points(k, width * SQRT_2, tartan, meter)?);
        }
        // A band's rows start on its left side, upright in the sett's frame, which the sett's turn turns.
        let side = Turn::new(frame.middle, tartan.sett.rotate).direction((0.0, 1.0));
        let groups = in_turns(count, tartan.rows_per_thread, side);
        Ok(Herringbone { left, pitch, bands, width, top, height, count, points, groups })
    }

    /// Band `j`'s left side.
    fn left(&self, j: usize) -> f64 {
        self.left + j as f64 * self.pitch
    }

    /// How far down a band's sides step `k` lies from their tops.
    fn side(&self, k: usize) -> f64 {
        (k as f64 / self.count as f64) * (self.height + self.width)
    }

    /// Row `k` of band `j`: in the first band, the third and so on, from step `k` of its left side, which
    /// starts a width above the box, down to the right; in the others, from step `k` of its left side, which
    /// starts at the box's top, up to the right.
    fn row(&self, j: usize, k: usize) -> Option<Row<'_>> {
        let down = j.is_multiple_of(2);
        let y = if down { self.top - self.width } else { self.top } + self.side(k);
        let along = (FRAC_1_SQRT_2, if down { FRAC_1_SQRT_2 } else { -FRAC_1_SQRT_2 });
        Some(Row { start: Point::new(self.left(j), y).ok()?, along, length: self.width * SQRT_2, points: self.points.get(k)? })
    }

    /// The places in its band of the rows `direction` takes in band `j`, in order: the first of each turn's
    /// in the weft of the first band, the third and so on, and in the warp of the others.
    fn group(&self, j: usize, direction: Direction) -> &[(usize, bool)] {
        let first = j.is_multiple_of(2) == (direction == Direction::Weft);
        self.groups.get(usize::from(!first)).map(Vec::as_slice).unwrap_or_default()
    }

    /// The rows `direction` takes in band `j`, in order.
    fn taken(&self, j: usize, direction: Direction) -> Vec<Taken<'_>> {
        self.group(j, direction).iter().filter_map(|&(k, back)| Some((self.row(j, k)?, back))).collect()
    }

    /// Adds each warp colour's lines over the piece `laying` lays on to `fills`, band by band, the second,
    /// fourth and so on first, then the first, third and so on, as the module's docs say. A unit of `meter`
    /// for each stripe each band is tested against, and the rest as [`Laying::band`] charges.
    pub(super) fn warp(
        &self,
        laying: &Laying<'_>,
        walked: &Walked,
        fills: &mut Vec<(Rgb, Vec<Vec<Point>>)>,
        meter: &mut Meter,
    ) -> Result<(), Exhausted> {
        let ([reach_left, _], [reach_right, _]) = laying.reach;
        for j in (1..self.bands).step_by(2).chain((0..self.bands).step_by(2)) {
            let (left, right) = (self.left(j), self.left(j) + self.width);
            let mut taken = None;
            for (colour, stripes) in &walked.bands {
                meter.charge(u64::try_from(stripes.len()).unwrap_or(u64::MAX))?;
                let mut lines = Vec::new();
                // Only the stripes that meet both the piece and the band have rows inside the piece.
                for &stripe in stripes.iter().filter(|s| s[0] <= reach_right && s[1] >= reach_left && s[0] <= right && s[1] >= left) {
                    let taken = taken.get_or_insert_with(|| self.taken(j, Direction::Warp));
                    lines.extend(laying.band(stripe, taken, false, meter)?);
                }
                add(fills, *colour, lines, false);
            }
        }
        Ok(())
    }

    /// Adds each weft colour's lines over the piece `laying` lays on to `fills`, after the warp's, stripe by
    /// stripe, as the module's docs say: a colour whose stripes reach the piece is listed, lines or none, and
    /// the others are left out. Charged as [`Herringbone::stripe`] charges.
    pub(super) fn weft(
        &self,
        laying: &Laying<'_>,
        walked: &Walked,
        fills: &mut Vec<(Rgb, Vec<Vec<Point>>)>,
        meter: &mut Meter,
    ) -> Result<(), Exhausted> {
        let ([_, reach_top], [_, reach_bottom]) = laying.reach;
        for (colour, stripes) in &walked.bands {
            let mut lines = Vec::new();
            let mut reached = false;
            for &stripe in stripes.iter().filter(|s| s[0] < s[1] && s[0] <= reach_bottom && s[1] >= reach_top) {
                reached = true;
                lines.extend(self.stripe(laying, stripe, meter)?);
            }
            if reached {
                add(fills, *colour, lines, true);
            }
        }
        Ok(())
    }

    /// The weft's lines in `stripe`, from its top to its bottom, over the piece `laying` lays on, turned back
    /// to the piece's frame: each band's pieces of its zigzag inside the stripe, joined along its edges, then
    /// the bands' lines joined into one while an edge crosses the piece, and cut by the piece.
    fn stripe(&self, laying: &Laying<'_>, stripe: [f64; 2], meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
        let mut banded = Vec::with_capacity(self.bands);
        for j in 0..self.bands {
            let zigzag = self.zigzag(j, stripe, laying.tartan.run_length, meter)?;
            banded.push(joined(clipped(&zigzag, stripe, laying.frame, meter)?, laying, meter)?);
        }
        let chained = if crosses(stripe, laying.reach) { vec![chained(banded, stripe, laying, meter)?] } else { banded.concat() };
        let (dx, dy) = laying.turn.back().direction((NUDGE, NUDGE));
        let mut lines = Vec::new();
        for line in chained.iter().filter(|line| line.len() >= 2) {
            let nudged: Vec<Point> = line.iter().filter_map(|q| Point::new(q.x() + dx, q.y() + dy).ok()).collect();
            for line in inside(&nudged, std::slice::from_ref(laying.piece), meter)? {
                made(meter, line.len())?;
                lines.push(line.into_iter().map(|p| laying.turn.apply(p)).collect());
            }
        }
        Ok(lines)
    }

    /// Band `j`'s weft rows near `stripe`, each with its needle points, joined end to start by straight
    /// stitches no longer than `run`, as Ink/Stitch joins all of a band's weft rows: from the first row that,
    /// with the join to the row after it, reaches the stripe's height, to the row after the last such. The
    /// rest of the band's zigzag lies above or below the stripe. A unit of `meter` for each row looked at and
    /// each point made.
    fn zigzag(&self, j: usize, stripe: [f64; 2], run: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let group = self.group(j, Direction::Weft);
        meter.charge(u64::try_from(group.len()).unwrap_or(u64::MAX))?;
        // Row `k` lies between a width above its step and its step, in either band, and a join between 2 rows
        // between the highest and lowest of their ends.
        let span = |k: usize| (self.top - self.width + self.side(k), self.top + self.side(k));
        let reaches = |i: usize| {
            let ((low, high), next) = (group.get(i).map(|&(k, _)| span(k)).unwrap_or_default(), group.get(i + 1).map(|&(k, _)| span(k)));
            let (low, high) = next.map_or((low, high), |(l, h)| (low.min(l), high.max(h)));
            low <= stripe[1] + ROUNDING && high >= stripe[0] - ROUNDING
        };
        let (Some(first), Some(last)) = ((0..group.len()).find(|&i| reaches(i)), (0..group.len()).rfind(|&i| reaches(i))) else {
            return Ok(Vec::new());
        };
        let near = group.get(first..=(last + 1).min(group.len().saturating_sub(1))).unwrap_or_default();
        let mut zigzag: Vec<Point> = Vec::new();
        for &(k, back) in near {
            let Some(row) = self.row(j, k) else { continue };
            meter.charge(u64::try_from(row.points.len()).unwrap_or(u64::MAX))?;
            let mut points: Vec<Point> = row.points.iter().filter_map(|&t| row.at(t)).collect();
            if back {
                points.reverse();
            }
            if let (Some(&end), Some(&start)) = (zigzag.last(), points.first()) {
                let join = segmentized(end, start, run, meter)?;
                zigzag.extend(join.get(1..join.len().saturating_sub(1)).unwrap_or_default());
            }
            zigzag.extend(points);
        }
        Ok(zigzag)
    }
}

/// Whether an edge of `stripe` crosses the piece whose bounds in the sett's frame are `reach`, which joins the
/// bands' lines across them.
fn crosses(stripe: [f64; 2], reach: Bounds) -> bool {
    let ([_, top], [_, bottom]) = reach;
    stripe.iter().any(|&edge| edge >= top && edge <= bottom)
}

/// The pieces of `line` inside `stripe`, between its edges and across `frame` from its left side to its
/// right, in order along it: those longer than half a pixel, as Ink/Stitch keeps them. A unit of `meter` for
/// each point.
fn clipped(line: &[Point], stripe: [f64; 2], frame: &Frame, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    meter.charge(u64::try_from(line.len()).unwrap_or(u64::MAX))?;
    let (least, most) = ([frame.min[0], stripe[0]], [frame.max[0], stripe[1]]);
    let mut pieces: Vec<Vec<Point>> = Vec::new();
    // Whether the last piece runs on to the last point looked at.
    let mut open = false;
    for pair in line.windows(2) {
        // A point twice in a row is one: no segment lies between.
        let &[p, q] = pair else { continue };
        if p == q {
            continue;
        }
        let inside = within(p, q, least, most).filter(|(t0, t1)| t1 > t0);
        let Some((t0, t1)) = inside else {
            open = false;
            continue;
        };
        let end = if t1 == 1.0 { q } else { p.lerp(q, t1) };
        match pieces.last_mut() {
            Some(piece) if open && t0 == 0.0 => piece.push(end),
            _ => pieces.push(vec![if t0 == 0.0 { p } else { p.lerp(q, t0) }, end]),
        }
        open = t1 == 1.0;
    }
    pieces.retain(|piece| {
        piece.windows(2).filter_map(|pair| if let &[a, b] = pair { Some(a.distance(b)) } else { None }).sum::<f64>() > SHORTEST_STRETCH
    });
    Ok(pieces)
}

/// The part of the segment from `p` to `q` between `least` and `most` on both axes, as the fractions of it
/// where it enters and leaves; `None` where it misses them.
fn within(p: Point, q: Point, least: [f64; 2], most: [f64; 2]) -> Option<(f64, f64)> {
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for ((from, to), (low, high)) in [(p.x(), q.x()), (p.y(), q.y())].into_iter().zip(least.into_iter().zip(most)) {
        let way = to - from;
        if way == 0.0 {
            if from < low || from > high {
                return None;
            }
            continue;
        }
        let (a, b) = ((low - from) / way, (high - from) / way);
        (t0, t1) = (t0.max(a.min(b)), t1.min(a.max(b)));
    }
    (t0 <= t1).then_some((t0, t1))
}

/// `pieces`, in order along a band's zigzag inside a stripe, each joined to the next by straight stitches no
/// longer than the running stitch, unless either end lies on the piece's rings, as a straight tartan's
/// stretches are joined ([`Laying::band`]).
fn joined(pieces: Vec<Vec<Point>>, laying: &Laying<'_>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let mut lines: Vec<Vec<Point>> = Vec::new();
    for piece in pieces {
        let ends = lines.last().and_then(|line| line.last().copied()).zip(piece.first().copied());
        let joins = match ends {
            Some((end, start)) => !laying.on_rings(end, meter)? && !laying.on_rings(start, meter)?,
            None => false,
        };
        match (ends.filter(|_| joins), lines.last_mut()) {
            (Some((end, start)), Some(line)) => {
                let join = segmentized(end, start, laying.tartan.run_length, meter)?;
                line.extend(join.get(1..join.len().saturating_sub(1)).unwrap_or_default());
                line.extend(piece);
            }
            _ => lines.push(piece),
        }
    }
    Ok(lines)
}

/// `banded`, each band's lines in `stripe` in band order, joined into one line as Ink/Stitch joins a
/// herringbone's weft across its bands. Ink/Stitch takes the stripe's edges from its outline as its geometry
/// library orders it, from the corner of least x: with the sett turned less than a quarter turn either way,
/// or a quarter turn exactly, the lower edge, as the sett's frame shows it with y down, runs to the right and
/// the bands are taken from the right, and otherwise the upper edge runs to the left and they are taken from
/// the left. The first band's last line decides which way the edges run: the end of it farther along that
/// edge, from where the edge starts nearer it, goes on to the next band. Each line after it starts at its end
/// on the edge nearer where the line before ended, the first edge on a tie, and is joined to it there by
/// straight stitches no longer than the running stitch.
fn chained(banded: Vec<Vec<Vec<Point>>>, stripe: [f64; 2], laying: &Laying<'_>, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let ([left, _], [right, _]) = (laying.frame.min, laying.frame.max);
    let (cos, sin) = laying.turn.direction((1.0, 0.0));
    let rightwards = cos > 0.0 || (cos == 0.0 && sin > 0.0);
    let [upper, lower] = stripe;
    let (first_edge, second_edge) = if rightwards { (lower, upper) } else { (upper, lower) };
    let mut lines: Vec<Vec<Point>> = if rightwards { banded.concat() } else { banded.into_iter().rev().flatten().collect() };
    lines.reverse();
    let mut lines = lines.into_iter().filter(|line| line.len() >= 2);
    let Some(first) = lines.next() else { return Ok(Vec::new()) };
    // How far along the first edge, from its start, a point lies: from the left side when it runs right.
    let length = right - left;
    let along = |p: Point, forwards: bool| if forwards { p.x() - left } else { right - p.x() }.clamp(0.0, length);
    let (Some(&a), Some(&b)) = (first.first(), first.last()) else { return Ok(Vec::new()) };
    let forwards = rightwards != (along(a, rightwards) > length / 2.0);
    let mut chain = first;
    if along(a, forwards) > along(b, forwards) {
        chain.reverse();
    }
    let on_first = |p: Point| (p.y() - first_edge).abs() <= (p.y() - second_edge).abs();
    for line in lines {
        let (Some(&previous), Some(&a), Some(&b)) = (chain.last(), line.first(), line.last()) else { break };
        let turned = on_first(previous) != on_first(a);
        let start = if turned { b } else { a };
        let join = segmentized(previous, start, laying.tartan.run_length, meter)?;
        chain.extend(join.get(1..join.len().saturating_sub(1)).unwrap_or_default());
        // A line that starts where the one before ended is joined without its first point twice.
        let skip = usize::from(previous == start);
        if turned {
            chain.extend(line.into_iter().rev().skip(skip));
        } else {
            chain.extend(line.into_iter().skip(skip));
        }
    }
    Ok(chain)
}

#[cfg(test)]
mod tests;
