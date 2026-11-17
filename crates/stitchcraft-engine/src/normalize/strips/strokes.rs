//! A part read as strokes, cut where its strokes meet, each stroke a satin column with its stitches square
//! to it (`REQ-AUTO-087`, `REQ-AUTO-088`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Letters sewn as strokes. A person draws a block letter
//! or a logo's bars in strokes: straight bands of nearly even width that meet at joins, and a font sews each
//! as a satin column of its own, its stitches across it. The columns along a part's centre lines
//! ([`super::columns`]) run on round the corners where strokes meet, and an M's middle strokes then took in
//! the corners of its stems, their rails ran on unevenly, and their stitches leaned up to 56°. So:
//!
//! 1. **Sides.** Each ring of the part is cut into straight runs ([`sides`]): a run whose points lie within
//!    [`RUN`] of the line between its ends.
//! 2. **Strokes.** 2 sides that run opposite ways within 5°, lie as far apart at both ends, overlap along
//!    each other by half that distance, are at least that long between them and half as long each, and
//!    hold the part between them, are a stroke's 2 sides ([`paired`]). The stroke is the band between their
//!    lines, as long as the 2 sides reach. Sides on the same 2 lines are one stroke, as an E's stem is one
//!    between its arms.
//! 3. **Joins.** Where 2 strokes' bands meet inside the part, the one the part goes on past on both sides
//!    runs through, and the other stops against it ([`join`]). Of 2 that end there, the one whose end lies
//!    square to it runs through: an M's stem, whose top is square, where the middle stroke's top is cut on
//!    the slant. Of 2 ends alike, as at the bottom of a V or the top of an A, the join is cut along the
//!    line that halves the angle between them. A stroke that stops reaches the pull compensation past the
//!    other's side along its own line, and at a halving line half of it.
//! 4. **Pieces.** Each stroke's piece is the part inside its band, cut so ([`pieces`]). The pieces must
//!    cover the part, but for [`UNCOVERED`] of it, or the part keeps its centre lines' columns.
//! 5. **Columns.** Each piece's rails are its 2 sides from its end nearest one way along the stroke to its
//!    end nearest the other, and its rungs lie square to the stroke at every corner of either rail
//!    ([`column`]), so that each stitch lies square to the stroke. An end within 20° of square is flat, and
//!    any other ends in a point.
//!
//! A part with a ring among its centre lines' columns keeps them, as does one with fewer than 2 strokes that
//! meet, one cut from a broad part, and one whose strokes don't cover it. Every step costs a unit of work.

use stitchcraft_core::{Exhausted, Meter, Point};

use super::{Column, Line};
use crate::normalize::region::geom::{self, Location};
use crate::normalize::region::{Polygon, ops};

/// How far a point of a side may lie from the line between the side's ends, in millimetres: a quarter of
/// the 0.2 mm that a needle point lies off a drawn edge before it shows.
const RUN: f64 = 0.05;

/// The cosine of 5°: 2 sides of a stroke run opposite ways within it.
const OPPOSITE: f64 = 0.996_194_698_091_745_5;

/// The cosine of 2°: 2 strokes whose lines run within it of each other and lie on each other are one.
const SAME_WAY: f64 = 0.999_390_827_019_095_7;

/// The share of the band between 2 sides that may lie outside the part, the sides still a stroke's.
const OUTSIDE: f64 = 0.02;

/// The share of a part its strokes' pieces may leave uncovered.
const UNCOVERED: f64 = 0.01;

/// How much more nearly square to its line one stroke's end must lie than the other's, along it in shares
/// of its width, for it to run through the join (`REQ-AUTO-088`).
const TIE: f64 = 0.15;

/// The tangent of 20°: an end whose points lie no farther apart along the stroke than this times its width
/// is flat, as a column ends along an edge within 20° of square ([`super::columns`]), as a font's letter's
/// foot does. Any other ends in a point, its stitches square to the stroke up to it.
const FLAT: f64 = 0.363_970_234_266_202_4;

/// The sine of 15°: 2 strokes whose lines cross more nearly parallel than this are not cut by a halving line.
const CROSSING: f64 = 0.258_819_045_102_520_8;

/// How far a band reaches past its sides across the stroke, in millimetres, so that the part's own edge lies
/// inside it: a ten-thousandth of a millimetre, which leaves a step that small where a piece's side leaves
/// the part's edge.
const EDGE: f64 = 1e-4;

/// How close 2 points of a piece may lie, in millimetres, and be 2: ten times a band's [`EDGE`].
const CLOSE: f64 = 1e-3;

/// How far a rail may run back along its stroke, in millimetres, and still be a rail of it: a quarter of
/// the 0.2 mm that a needle point lies off a drawn edge before it shows.
const BACK: f64 = 0.05;

/// How close, in millimetres, 2 rungs' points may lie on a rail.
const RUNGS_APART: f64 = 0.05;

/// The most pieces a stroke's middle line is cut into.
const LINE_POINTS: u32 = 400;

/// The smallest piece, in square millimetres: a smaller one is a sliver where 2 bands' corners meet.
const SLIVER: f64 = 0.01;

/// A way, with length 1.
type Way = (f64, f64);

/// A straight run of a ring, from `a` to `b` in the ring's order.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Side {
    a: Point,
    b: Point,
}

/// A stroke: the band between 2 lines along `u`, from `lo` to `hi` across it and from `from` to `to` along
/// it, both measured from `o`, `n` square to `u`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Band {
    o: Point,
    u: Way,
    n: Way,
    lo: f64,
    hi: f64,
    from: f64,
    to: f64,
}

impl Band {
    fn along(&self, p: Point) -> f64 {
        (p.x() - self.o.x()) * self.u.0 + (p.y() - self.o.y()) * self.u.1
    }

    fn across(&self, p: Point) -> f64 {
        (p.x() - self.o.x()) * self.n.0 + (p.y() - self.o.y()) * self.n.1
    }

    fn at(&self, along: f64, across: f64) -> Option<Point> {
        Point::new(self.o.x() + self.u.0 * along + self.n.0 * across, self.o.y() + self.u.1 * along + self.n.1 * across).ok()
    }

    fn width(&self) -> f64 {
        self.hi - self.lo
    }

    /// Its middle, half way along its sides and across.
    fn middle(&self) -> Option<Point> {
        self.at((self.from + self.to) / 2.0, (self.lo + self.hi) / 2.0)
    }

    /// The band as a polygon, a width longer at each end and [`EDGE`] wider on each side.
    fn reach(&self) -> Option<Vec<Point>> {
        let (from, to) = (self.from - self.width(), self.to + self.width());
        let (lo, hi) = (self.lo - EDGE, self.hi + EDGE);
        Some(vec![self.at(from, lo)?, self.at(to, lo)?, self.at(to, hi)?, self.at(from, hi)?])
    }
}

/// A cut of a stroke's band where it meets another stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Cut {
    /// It keeps the points `x` with `(x - p) · m >= 0`: a stroke that stops against another, or the halving
    /// line of a join.
    Keep { p: Point, m: Way },
    /// It loses the band of the stroke `of` that it crosses, narrowed by `by` on each side, and ends under it.
    Crossed { of: usize, by: f64 },
}

/// What becomes of each stroke at its joins: its cuts, and for its start, then its end, the stroke it ends
/// under.
#[derive(Clone, Debug, Default, PartialEq)]
struct Joined {
    cuts: Vec<Cut>,
    under: Vec<(f64, usize)>,
}

/// The columns of `part` sewn as strokes, given the columns along its centre lines `found` and the pull
/// compensation `reach` of its columns, with `meter`'s work: `None` when the part doesn't read as strokes,
/// and keeps `found`. Its strokes are no narrower than `narrowest` and no wider than `widest`.
pub(crate) fn strokes(part: &Polygon, found: &[Column], reach: f64, (narrowest, widest): (f64, f64), meter: &mut Meter) -> Result<Option<Vec<Column>>, Exhausted> {
    if found.len() < 2 || found.iter().any(|column| column.closed) {
        return Ok(None);
    }
    let mut runs = Vec::new();
    for ring in std::iter::once(&part.outline).chain(&part.holes) {
        runs.extend(sides(ring, meter)?);
    }
    let Some(bands) = bands(part, &runs, (narrowest, widest), meter)? else { return Ok(None) };
    if bands.len() < 2 {
        return Ok(None);
    }
    let Some(joined) = joins(part, &bands, reach, meter)? else { return Ok(None) };
    let Some(pieces) = pieces(part, &bands, &joined, meter)? else { return Ok(None) };
    columns_of(part, &bands, &joined, &pieces, meter)
}

/// The straight runs of the closed `ring`, each from a corner to the next, with `meter`'s work. A run's
/// points lie within [`RUN`] of the line between its ends. They start at the ring's sharpest corner.
fn sides(ring: &[Point], meter: &mut Meter) -> Result<Vec<Side>, Exhausted> {
    let points = ring.get(..ring.len().saturating_sub(1)).unwrap_or_default();
    let count = points.len();
    if count < 3 {
        return Ok(Vec::new());
    }
    let at = |k: usize| points.get(k % count).copied();
    // The sharpest corner: where the ring turns most.
    let mut start = 0;
    let mut sharpest = f64::INFINITY;
    for k in 0..count {
        meter.charge(1)?;
        let (Some(before), Some(here), Some(after)) = (at(k + count - 1), at(k), at(k + 1)) else { continue };
        let (Some(into), Some(out)) = (unit(before, here), unit(here, after)) else { continue };
        let turn = into.0 * out.0 + into.1 * out.1;
        if turn < sharpest {
            (start, sharpest) = (k, turn);
        }
    }
    let mut found = Vec::new();
    let mut first = start;
    while first < start + count {
        let mut last = first + 1;
        while last < start + count && straight(points, first, last + 1, meter)? {
            last += 1;
        }
        if let (Some(a), Some(b)) = (at(first), at(last)) {
            found.push(Side { a, b });
        }
        first = last;
    }
    Ok(found)
}

/// Whether the points of `points` from `first` to `last`, round the ring, lie within [`RUN`] of the line
/// between those 2, with `meter`'s work.
fn straight(points: &[Point], first: usize, last: usize, meter: &mut Meter) -> Result<bool, Exhausted> {
    let count = points.len();
    let at = |k: usize| points.get(k % count.max(1)).copied();
    let (Some(a), Some(b)) = (at(first), at(last)) else { return Ok(false) };
    for k in first + 1..last {
        meter.charge(1)?;
        let Some(p) = at(k) else { return Ok(false) };
        if crate::normalize::stroke::distance_to_segment(p, a, b) > RUN {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The strokes of `part` that pairs of its `runs` make, no narrower than `narrowest` and no wider than
/// `widest`, those on the same lines one, with `meter`'s work; `None` when a region can't be measured (a
/// bug).
fn bands(part: &Polygon, runs: &[Side], limits: (f64, f64), meter: &mut Meter) -> Result<Option<Vec<Band>>, Exhausted> {
    let mut found: Vec<Band> = Vec::new();
    for (i, one) in runs.iter().enumerate() {
        for other in runs.iter().skip(i + 1) {
            meter.charge(1)?;
            let Some((band, overlap)) = paired(*one, *other, limits) else { continue };
            match held(part, &band, overlap, meter)? {
                None => return Ok(None),
                Some(false) => continue,
                Some(true) => {}
            }
            match found.iter_mut().find(|kept| same(kept, &band)) {
                Some(kept) => merge(kept, &band),
                None => found.push(band),
            }
        }
    }
    Ok(Some(found))
}

/// The stroke whose 2 sides are `one` and `other`, when they are a stroke's ([`REQ-AUTO-087`]): opposite ways
/// within 5°, as far apart at both ends within a tenth, at least `narrowest` and at most `widest` apart,
/// overlapping along each other by half that, the longer at least that long and the shorter half as long;
/// with how far along it they overlap.
fn paired(one: Side, other: Side, (narrowest, widest): (f64, f64)) -> Option<(Band, (f64, f64))> {
    let u = unit(one.a, one.b)?;
    let v = unit(other.a, other.b)?;
    if u.0 * v.0 + u.1 * v.1 > -OPPOSITE {
        return None;
    }
    let mut band = Band { o: one.a, u, n: (-u.1, u.0), lo: 0.0, hi: 0.0, from: 0.0, to: 0.0 };
    let (c0, c1) = (band.across(other.a), band.across(other.b));
    let apart = (c0 + c1) / 2.0;
    let width = apart.abs();
    if (c0 - c1).abs() > 0.1 * width || width < narrowest || width > widest {
        return None;
    }
    let mine = ordered(0.0, band.along(one.b));
    let theirs = ordered(band.along(other.a), band.along(other.b));
    let overlap = (mine.0.max(theirs.0), mine.1.min(theirs.1));
    let (long, short) = (one.a.distance(one.b).max(other.a.distance(other.b)), one.a.distance(one.b).min(other.a.distance(other.b)));
    if overlap.1 - overlap.0 < width / 2.0 || long < width || short < width / 2.0 {
        return None;
    }
    (band.lo, band.hi) = ordered(0.0, apart);
    (band.from, band.to) = (mine.0.min(theirs.0), mine.1.max(theirs.1));
    Some((band, overlap))
}

/// Whether the part holds `band` between its sides where they overlap, from and to as `overlap` says, but for
/// [`OUTSIDE`] of it, with `meter`'s work; `None` when the region can't be measured (a bug).
fn held(part: &Polygon, band: &Band, (from, to): (f64, f64), meter: &mut Meter) -> Result<Option<bool>, Exhausted> {
    let Some(quad) = (|| Some(vec![band.at(from, band.lo)?, band.at(to, band.lo)?, band.at(to, band.hi)?, band.at(from, band.hi)?]))() else {
        return Ok(Some(false));
    };
    let quad = polygon(quad);
    let Ok(outside) = ops::difference(std::slice::from_ref(&quad), std::slice::from_ref(part), meter)? else { return Ok(None) };
    Ok(Some(ops::area_of(&outside) <= OUTSIDE * ops::area_of(std::slice::from_ref(&quad))))
}

/// Whether the strokes `kept` and `band` lie on the same 2 lines: within [`SAME_WAY`], and each side within a
/// tenth of the width of `kept`'s.
fn same(kept: &Band, band: &Band) -> bool {
    let way = kept.u.0 * band.u.0 + kept.u.1 * band.u.1;
    if way.abs() < SAME_WAY {
        return false;
    }
    let middle = (band.from + band.to) / 2.0;
    let (Some(a), Some(b)) = (band.at(middle, band.lo), band.at(middle, band.hi)) else { return false };
    let (lo, hi) = ordered(kept.across(a), kept.across(b));
    (lo - kept.lo).abs() <= 0.1 * kept.width() && (hi - kept.hi).abs() <= 0.1 * kept.width()
}

/// `kept` reaching as far along as `band` does too.
fn merge(kept: &mut Band, band: &Band) {
    let middle = (band.lo + band.hi) / 2.0;
    for along in [band.from, band.to] {
        if let Some(p) = band.at(along, middle) {
            let at = kept.along(p);
            (kept.from, kept.to) = (kept.from.min(at), kept.to.max(at));
        }
    }
}

/// What becomes of each of `bands` where they meet in `part` (`REQ-AUTO-088`), the stroke that stops
/// reaching `reach` past the other along its line, with `meter`'s work; `None` when 2 strokes lie along each
/// other or a region can't be measured.
fn joins(part: &Polygon, bands: &[Band], reach: f64, meter: &mut Meter) -> Result<Option<Vec<Joined>>, Exhausted> {
    let mut joined = vec![Joined::default(); bands.len()];
    let mut met = false;
    for (i, one) in bands.iter().enumerate() {
        for (j, other) in bands.iter().enumerate().skip(i + 1) {
            meter.charge(1)?;
            let Some(meeting) = meeting(part, one, other, meter)? else { return Ok(None) };
            let Some(meeting) = meeting else { continue };
            met = true;
            let Some(cuts) = join(part, (i, one), (j, other), &meeting, reach) else { return Ok(None) };
            for (stroke, cut, under) in cuts {
                let Some(entry) = joined.get_mut(stroke) else { return Ok(None) };
                entry.cuts.push(cut);
                entry.under.extend(under);
            }
        }
    }
    Ok(met.then_some(joined))
}

/// The points of the outline where the bands `one` and `other` meet inside `part`, with `meter`'s work:
/// `Some(None)` when they don't, or meet in less than a sliver, and `None` when the region can't be
/// measured (a bug).
fn meeting(part: &Polygon, one: &Band, other: &Band, meter: &mut Meter) -> Result<Option<Option<Vec<Point>>>, Exhausted> {
    let (Some(a), Some(b)) = (one.reach(), other.reach()) else { return Ok(Some(None)) };
    let Ok(inside) = ops::intersection(std::slice::from_ref(part), &[polygon(a)], meter)? else { return Ok(None) };
    let Ok(both) = ops::intersection(&inside, &[polygon(b)], meter)? else { return Ok(None) };
    let least = SLIVER.max(0.02 * one.width() * other.width());
    if ops::area_of(&both) < least {
        return Ok(Some(None));
    }
    Ok(Some(Some(both.iter().flat_map(|piece| piece.outline.iter().copied()).collect())))
}

/// The cuts where the strokes `one` (`i`) and `other` (`j`) of `part` meet at `meeting`'s points, each the
/// stroke, its cut and the end it ends under the other with; `None` when one lies along the other.
fn join(part: &Polygon, (i, one): (usize, &Band), (j, other): (usize, &Band), meeting: &[Point], reach: f64) -> Option<Vec<(usize, Cut, Option<(f64, usize)>)>> {
    let through = |band: &Band| {
        let (lo, hi) = span(band, meeting);
        lo - band.from >= band.width() / 2.0 && band.to - hi >= band.width() / 2.0
    };
    let longer = |a: &Band, b: &Band| a.to - a.from >= b.to - b.from;
    let stop = |(k, stopping): (usize, &Band), (l, running): (usize, &Band)| {
        let cut = stops(stopping, running, reach)?;
        Some(vec![(k, cut, Some((end(stopping, meeting), l)))])
    };
    let cross = |(k, crossed): (usize, &Band), (l, crossing): (usize, &Band)| {
        let by = reach * sine(crossed.u, crossing.u);
        let middle = span(crossed, meeting);
        Some(vec![(k, Cut::Crossed { of: l, by }, Some(((middle.0 + middle.1) / 2.0, l)))])
    };
    match (through(one), through(other)) {
        (true, false) => stop((j, other), (i, one)),
        (false, true) => stop((i, one), (j, other)),
        (true, true) if longer(one, other) => cross((j, other), (i, one)),
        (true, true) => cross((i, one), (j, other)),
        (false, false) => {
            let (a, b) = (square(part, one, meeting)?, square(part, other, meeting)?);
            if a + TIE < b {
                stop((j, other), (i, one))
            } else if b + TIE < a {
                stop((i, one), (j, other))
            } else if sine(one.u, other.u) >= CROSSING {
                halved((i, one), (j, other), reach)
            } else if longer(one, other) {
                stop((j, other), (i, one))
            } else {
                stop((i, one), (j, other))
            }
        }
    }
}

/// How far along `band` the `points` reach, least and most.
fn span(band: &Band, points: &[Point]) -> (f64, f64) {
    points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), &p| (lo.min(band.along(p)), hi.max(band.along(p))))
}

/// Where along `band` its end at the join of `points` lies, for the column to know which end is under the
/// other: the middle of the join.
fn end(band: &Band, points: &[Point]) -> f64 {
    let (lo, hi) = span(band, points);
    (lo + hi) / 2.0
}

/// The cut of `stopping` against `running`: it keeps what lies on its own side of `running`'s near side,
/// moved into `running` so that its end lies `reach` farther along its own line. `None` when its middle
/// lies inside `running`'s band.
fn stops(stopping: &Band, running: &Band, reach: f64) -> Option<Cut> {
    let middle = stopping.middle()?;
    let side = running.across(middle);
    // Its stitches run square to it, and their pull compensation reaches across the cut by its share along
    // `running`'s side already.
    let into = reach * (1.0 - cosine(stopping.u, running.u));
    let (edge, sign) = if side > running.hi {
        (running.hi - into, 1.0)
    } else if side < running.lo {
        (running.lo + into, -1.0)
    } else {
        return None;
    };
    let p = running.at(0.0, edge)?;
    Some(Cut::Keep { p, m: (sign * running.n.0, sign * running.n.1) })
}

/// How far apart along `band` its 2 sides reach where the part ends towards `points`, in shares of its
/// width: 0 for an end square to it. Measured from the band's middle along each side a tenth of its width
/// in, to where it first leaves `part`. `None` when a side never leaves it.
fn square(part: &Polygon, band: &Band, points: &[Point]) -> Option<f64> {
    let middle = (band.from + band.to) / 2.0;
    let sign = if end(band, points) >= middle { 1.0 } else { -1.0 };
    let inset = 0.1 * band.width();
    let a = leaves(part, band.at(middle, band.lo + inset)?, (sign * band.u.0, sign * band.u.1))?;
    let b = leaves(part, band.at(middle, band.hi - inset)?, (sign * band.u.0, sign * band.u.1))?;
    Some((a - b).abs() / band.width())
}

/// How far from `from` along `way` a ray first crosses an edge of `part`.
fn leaves(part: &Polygon, from: Point, way: Way) -> Option<f64> {
    let mut nearest: Option<f64> = None;
    for ring in std::iter::once(&part.outline).chain(&part.holes) {
        for pair in ring.windows(2) {
            let [p, q] = pair else { continue };
            let (ex, ey) = (q.x() - p.x(), q.y() - p.y());
            let denominator = way.0 * ey - way.1 * ex;
            if denominator == 0.0 {
                continue;
            }
            let (dx, dy) = (p.x() - from.x(), p.y() - from.y());
            let t = (dx * ey - dy * ex) / denominator;
            let s = (dx * way.1 - dy * way.0) / denominator;
            if t > 0.0 && (0.0..=1.0).contains(&s) {
                nearest = Some(nearest.map_or(t, |kept| kept.min(t)));
            }
        }
    }
    nearest
}

/// The cuts of a join of `one` and `other` along the line that halves the angle between them, through the
/// point where their lines cross, each reaching half of `reach` past it along its own line.
fn halved((i, one): (usize, &Band), (j, other): (usize, &Band), reach: f64) -> Option<Vec<(usize, Cut, Option<(f64, usize)>)>> {
    let crossing = crossed(one, other)?;
    let towards = |band: &Band| -> Option<Way> {
        let middle = band.middle()?;
        let sign = if (middle.x() - crossing.x()) * band.u.0 + (middle.y() - crossing.y()) * band.u.1 >= 0.0 { 1.0 } else { -1.0 };
        Some((sign * band.u.0, sign * band.u.1))
    };
    let (a, b) = (towards(one)?, towards(other)?);
    let halving = unit_of((a.0 + b.0, a.1 + b.1))?;
    let m = (-halving.1, halving.0);
    let cut = |band: &Band, way: Way| -> Option<Cut> {
        let sign = if way.0 * m.0 + way.1 * m.1 >= 0.0 { 1.0 } else { -1.0 };
        // Its stitches' pull compensation reaches across the halving line by its share square to the line,
        // and the piece reaches the line, so that the pieces cover the join.
        let past = (reach / 2.0 - reach * cosine(band.u, halving)).max(0.0);
        let p = Point::new(crossing.x() - sign * m.0 * past, crossing.y() - sign * m.1 * past).ok()?;
        Some(Cut::Keep { p, m: (sign * m.0, sign * m.1) })
    };
    Some(vec![(i, cut(one, a)?, None), (j, cut(other, b)?, None)])
}

/// Where the middle lines of `one` and `other` cross.
fn crossed(one: &Band, other: &Band) -> Option<Point> {
    let p = one.at(0.0, (one.lo + one.hi) / 2.0)?;
    let q = other.at(0.0, (other.lo + other.hi) / 2.0)?;
    let denominator = one.u.0 * other.u.1 - one.u.1 * other.u.0;
    if denominator == 0.0 {
        return None;
    }
    let t = ((q.x() - p.x()) * other.u.1 - (q.y() - p.y()) * other.u.0) / denominator;
    Point::new(p.x() + one.u.0 * t, p.y() + one.u.1 * t).ok()
}

/// Each stroke's pieces of `part`: the part inside its band, cut as `joined` says, with `meter`'s work.
/// `None` when the pieces leave more than [`UNCOVERED`] of the part, a stroke has no piece, a piece has a
/// hole, or a region can't be measured.
fn pieces(part: &Polygon, bands: &[Band], joined: &[Joined], meter: &mut Meter) -> Result<Option<Vec<Vec<Polygon>>>, Exhausted> {
    let mut all = Vec::with_capacity(bands.len());
    for (band, joined) in bands.iter().zip(joined) {
        meter.charge(1)?;
        let Some(mut kept) = band.reach() else { return Ok(None) };
        let mut crossed = Vec::new();
        for cut in &joined.cuts {
            match *cut {
                Cut::Keep { p, m } => kept = clip(&kept, p, m),
                Cut::Crossed { of, by } => crossed.extend(bands.get(of).and_then(|other| narrowed(other, by))),
            }
        }
        if kept.len() < 3 {
            return Ok(None);
        }
        let Ok(inside) = ops::intersection(std::slice::from_ref(part), &[polygon(kept)], meter)? else { return Ok(None) };
        let Ok(inside) = ops::difference(&inside, &crossed, meter)? else { return Ok(None) };
        let mine: Vec<Polygon> = inside.into_iter().filter(|piece| ops::area_of(std::slice::from_ref(piece)) >= SLIVER && cores(band, piece)).collect();
        if mine.is_empty() || mine.iter().any(|piece| !piece.holes.is_empty()) {
            return Ok(None);
        }
        all.push(mine);
    }
    let every: Vec<Polygon> = all.iter().flatten().cloned().collect();
    let Ok(covered) = ops::union([every.as_slice()], meter)? else { return Ok(None) };
    let Ok(left) = ops::difference(std::slice::from_ref(part), &covered, meter)? else { return Ok(None) };
    Ok((ops::area_of(&left) <= UNCOVERED * ops::area_of(std::slice::from_ref(part))).then_some(all))
}

/// Whether `piece` holds a point of `band`'s middle line between its sides' ends: 9 points evenly along it.
fn cores(band: &Band, piece: &Polygon) -> bool {
    (0..9).any(|k| {
        let along = band.from + (band.to - band.from) * (f64::from(k) + 0.5) / 9.0;
        band.at(along, (band.lo + band.hi) / 2.0).is_some_and(|p| geom::locate_in_ring(p, &piece.outline) == Location::Interior)
    })
}

/// `band`'s polygon narrowed by `by` on each side across it: what a stroke that crosses it loses.
fn narrowed(band: &Band, by: f64) -> Option<Polygon> {
    let (from, to) = (band.from - band.width(), band.to + band.width());
    let (lo, hi) = (band.lo + by, band.hi - by);
    (lo < hi).then_some(())?;
    Some(polygon(vec![band.at(from, lo)?, band.at(to, lo)?, band.at(to, hi)?, band.at(from, hi)?]))
}

/// The convex polygon `points` cut to the points `x` with `(x - p) · m >= 0`.
fn clip(points: &[Point], p: Point, m: Way) -> Vec<Point> {
    let side = |q: Point| (q.x() - p.x()) * m.0 + (q.y() - p.y()) * m.1;
    let mut kept = Vec::with_capacity(points.len() + 1);
    for (k, &a) in points.iter().enumerate() {
        let Some(&b) = points.get((k + 1) % points.len()) else { continue };
        let (sa, sb) = (side(a), side(b));
        if sa >= 0.0 {
            kept.push(a);
        }
        if (sa >= 0.0) != (sb >= 0.0) {
            kept.extend(Point::new(a.x() + (b.x() - a.x()) * sa / (sa - sb), a.y() + (b.y() - a.y()) * sa / (sa - sb)).ok());
        }
    }
    kept
}

/// The columns of the strokes' `pieces` of `part`, each stroke's in order, with `meter`'s work: `None` when a piece
/// makes no column.
fn columns_of(part: &Polygon, bands: &[Band], joined: &[Joined], pieces: &[Vec<Polygon>], meter: &mut Meter) -> Result<Option<Vec<Column>>, Exhausted> {
    let first: Vec<usize> = pieces.iter().scan(0, |next, mine| Some(std::mem::replace(next, *next + mine.len()))).collect();
    let mut columns = Vec::new();
    for ((band, joined), mine) in bands.iter().zip(joined).zip(pieces) {
        for piece in mine {
            let Some((rails, rungs, line)) = column(part, piece, band, meter)? else { return Ok(None) };
            let (start, finish) = (band.along(line.points.first().copied().unwrap_or(band.o)), band.along(line.points.last().copied().unwrap_or(band.o)));
            let mut under = [None, None];
            for &(at, other) in &joined.under {
                let Some(&index) = first.get(other) else { return Ok(None) };
                // The end nearer the join is the one under the other stroke.
                let slot = usize::from((at - start).abs() > (at - finish).abs());
                if let Some(entry) = under.get_mut(slot) {
                    *entry = Some(index);
                }
            }
            columns.push(Column::stroke(rails, rungs, line, under));
        }
    }
    Ok(Some(columns))
}

/// The rails, rungs and middle line of the column of `piece`, a piece of `band`'s stroke in `part`, with `meter`'s
/// work: its rails from its end least far along the stroke to its end farthest, each end flat when its
/// points lie within [`FLAT`] of its width along the stroke; a rung square to the stroke at every corner of
/// either rail. `None` when a rail runs back along the stroke.
#[allow(clippy::type_complexity)]
fn column(part: &Polygon, piece: &Polygon, band: &Band, meter: &mut Meter) -> Result<Option<([Vec<Point>; 2], Vec<[Point; 2]>, Line)>, Exhausted> {
    let ring = &cleaned(piece.outline.get(..piece.outline.len().saturating_sub(1)).unwrap_or_default(), part, meter)?;
    let count = ring.len();
    let along: Vec<f64> = ring.iter().map(|&p| band.along(p)).collect();
    let (Some(least), Some(most)) = (extreme(&along, false), extreme(&along, true)) else { return Ok(None) };
    let flat = FLAT * band.width();
    let (Some(&low), Some(&high)) = (along.get(least), along.get(most)) else { return Ok(None) };
    let start = run_round(&along, least, |t| t <= low + flat);
    let finish = run_round(&along, most, |t| t >= high - flat);
    // One side runs forward round the ring from the start's last point to the end's first, the other back
    // from the start's first to the end's last.
    let one = walk(ring, start.1, finish.0, 1);
    let other = walk(ring, start.0, finish.1, count - 1);
    meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
    if one.len() < 2 || other.len() < 2 || one.len() + other.len() > count + 2 || !onward(&one, band) || !onward(&other, band) {
        return Ok(None);
    }
    let rungs = rungs(&one, &other, band, meter)?;
    let middle = (band.lo + band.hi) / 2.0;
    let (first, last) = spanned(ring, band, middle).unwrap_or((low, high));
    // A point every half width along it, as a centre line has one on each chord across its part.
    let steps = ((last - first) / (band.width() / 2.0)).ceil().clamp(1.0, f64::from(LINE_POINTS));
    let mut points = Vec::new();
    for k in 0..=LINE_POINTS {
        let share = f64::from(k) / steps;
        if share > 1.0 {
            break;
        }
        meter.charge(1)?;
        points.extend(band.at(first + (last - first) * share, middle));
    }
    Ok(Some(([one, other], rungs, Line { points, closed: false })))
}

/// The open ring `ring` of a piece of `part` without the steps a band's [`EDGE`] leaves where it meets the
/// part's edge, with `meter`'s work: a point within [`CLOSE`] of a corner of the part is that corner, so
/// that a rail runs along a hole's edge where the hole does (`REQ-AUTO-084`), and a point within [`CLOSE`] of
/// the one kept before it is left out, as is the last when it lies that near the first.
fn cleaned(ring: &[Point], part: &Polygon, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut kept: Vec<Point> = Vec::with_capacity(ring.len());
    for &p in ring {
        let mut at = p;
        for ring in part.rings() {
            meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
            if let Some(&corner) = ring.iter().find(|corner| corner.distance(p) < CLOSE) {
                at = corner;
            }
        }
        if kept.last().is_none_or(|last| last.distance(at) >= CLOSE) {
            kept.push(at);
        }
    }
    if kept.len() > 1 && kept.first().zip(kept.last()).is_some_and(|(first, last)| first.distance(*last) < CLOSE) {
        kept.pop();
    }
    Ok(kept)
}

/// How far along `band` its line `middle` across it enters the closed `ring` and leaves it: the least and the
/// most of where it crosses the ring's edges. `None` when it crosses fewer than 2.
fn spanned(ring: &[Point], band: &Band, middle: f64) -> Option<(f64, f64)> {
    let mut crossings = Vec::new();
    for (k, &a) in ring.iter().enumerate() {
        let Some(&b) = ring.get((k + 1) % ring.len()) else { continue };
        let (ca, cb) = (band.across(a) - middle, band.across(b) - middle);
        if (ca >= 0.0) != (cb >= 0.0) {
            let share = ca / (ca - cb);
            crossings.push(band.along(a) + (band.along(b) - band.along(a)) * share);
        }
    }
    let least = crossings.iter().copied().fold(f64::INFINITY, f64::min);
    let most = crossings.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    (crossings.len() >= 2).then_some((least, most))
}

/// The index of the least, or the `most`, of `values`: the first of several alike.
fn extreme(values: &[f64], most: bool) -> Option<usize> {
    let mut found: Option<(usize, f64)> = None;
    for (k, &value) in values.iter().enumerate() {
        let better = found.is_none_or(|(_, kept)| if most { value > kept } else { value < kept });
        if better {
            found = Some((k, value));
        }
    }
    found.map(|(k, _)| k)
}

/// The run of the ring's points round `at` that `holds` holds for, as its first and last index round the
/// ring: back and on from `at` while it holds.
fn run_round(values: &[f64], at: usize, holds: impl Fn(f64) -> bool) -> (usize, usize) {
    let count = values.len().max(1);
    let holding = |k: usize| values.get(k % count).copied().is_some_and(&holds);
    let (mut first, mut last) = (at, at);
    let mut steps = 0;
    while steps + 1 < count && holding(first + count - 1) {
        first = (first + count - 1) % count;
        steps += 1;
    }
    while steps + 1 < count && holding(last + 1) {
        last = (last + 1) % count;
        steps += 1;
    }
    (first, last)
}

/// The points of `ring` from `from` to `to`, stepping `step` round it (1 on, the ring's length less 1 back).
fn walk(ring: &[Point], from: usize, to: usize, step: usize) -> Vec<Point> {
    let count = ring.len();
    let mut points = Vec::new();
    let mut k = from;
    for _ in 0..=count {
        points.extend(ring.get(k).copied());
        if k == to {
            return points;
        }
        k = (k + step) % count.max(1);
    }
    Vec::new()
}

/// Whether `rail` runs on along `band`, back by no more than [`BACK`] at any point.
fn onward(rail: &[Point], band: &Band) -> bool {
    let mut farthest = f64::NEG_INFINITY;
    rail.iter().all(|&p| {
        let at = band.along(p);
        farthest = farthest.max(at);
        at >= farthest - BACK
    })
}

/// A rung square to `band` at every corner of the rails `one` and `other` but their ends, in order along
/// the column, each from its point on `one` to its point on `other`, with `meter`'s work. Of rungs whose
/// points lie within [`RUNGS_APART`] on a rail, the first is kept.
fn rungs(one: &[Point], other: &[Point], band: &Band, meter: &mut Meter) -> Result<Vec<[Point; 2]>, Exhausted> {
    let inner = |rail: &[Point]| rail.get(1..rail.len().saturating_sub(1)).unwrap_or_default().to_vec();
    let mut found: Vec<(f64, [Point; 2])> = Vec::new();
    for p in inner(one) {
        meter.charge(1)?;
        found.extend(meet(other, band, band.along(p)).map(|q| (band.along(p), [p, q])));
    }
    for q in inner(other) {
        meter.charge(1)?;
        found.extend(meet(one, band, band.along(q)).map(|p| (band.along(q), [p, q])));
    }
    // A flat end on the slant pairs its 2 corners: a rung square to the stroke past the corner farther in
    // keeps the stitches after it square, where they would turn from the end's slant to square along the
    // whole of the rails' first pieces. It lies twice as far past that corner as the corners lie apart along
    // the stroke, so that the stitches between pair no more than 1.5 of one rail with 1 of the other, as an
    // even column does (`auto::columns::EVEN`), and crowd no needle points in.
    for ((a, b), inwards) in [(one.first().zip(other.first()), 1.0), (one.last().zip(other.last()), -1.0)].into_iter().filter_map(|(ends, way)| Some((ends?, way))) {
        meter.charge(1)?;
        let (ta, tb) = (band.along(*a), band.along(*b));
        let far = if inwards > 0.0 { ta.max(tb) } else { ta.min(tb) };
        if (ta - tb).abs() > 2.0 * RUNGS_APART {
            let at = far + inwards * 2.0 * (ta - tb).abs();
            if let (Some(p), Some(q)) = (meet(one, band, at), meet(other, band, at)) {
                found.push((at, [p, q]));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ends = [one.first(), one.last(), other.first(), other.last()];
    let near_end = |[p, q]: [Point; 2]| ends.iter().flatten().any(|&&e| e.distance(p) < RUNGS_APART || e.distance(q) < RUNGS_APART);
    let mut kept: Vec<[Point; 2]> = Vec::new();
    for (_, rung) in found {
        let crowded = kept.last().is_some_and(|last| last[0].distance(rung[0]) < RUNGS_APART || last[1].distance(rung[1]) < RUNGS_APART);
        if !crowded && !near_end(rung) {
            kept.push(rung);
        }
    }
    Ok(kept)
}

/// The point of `rail` at `along` along `band`: on its first piece that reaches there.
fn meet(rail: &[Point], band: &Band, along: f64) -> Option<Point> {
    rail.windows(2).find_map(|pair| {
        let [a, b] = pair else { return None };
        let (ta, tb) = (band.along(*a), band.along(*b));
        if (ta - along) * (tb - along) > 0.0 || ta == tb {
            return None;
        }
        let share = (along - ta) / (tb - ta);
        Point::new(a.x() + (b.x() - a.x()) * share, a.y() + (b.y() - a.y()) * share).ok()
    })
}

/// The closed ring of `points`, clockwise in y-up axes as a part's outline runs.
fn polygon(mut points: Vec<Point>) -> Polygon {
    if let Some(&first) = points.first() {
        points.push(first);
    }
    if geom::signed_area(&points) > 0.0 {
        points.reverse();
    }
    Polygon { outline: points, holes: Vec::new() }
}

/// The way from `a` to `b`, with length 1.
fn unit(a: Point, b: Point) -> Option<Way> {
    unit_of((b.x() - a.x(), b.y() - a.y()))
}

/// `(x, y)` with length 1, or `None` for no length.
fn unit_of((x, y): (f64, f64)) -> Option<Way> {
    let length = (x * x + y * y).sqrt();
    (length > 0.0 && length.is_finite()).then(|| (x / length, y / length))
}

/// The sine of the angle between the ways `a` and `b`, 0 to 1.
fn sine(a: Way, b: Way) -> f64 {
    (a.0 * b.1 - a.1 * b.0).abs()
}

/// The cosine of the angle between the ways `a` and `b`, taken as 0 to 1, whichever way each runs.
fn cosine(a: Way, b: Way) -> f64 {
    (a.0 * b.0 + a.1 * b.1).abs()
}

/// `a` and `b`, the smaller first.
fn ordered(a: f64, b: f64) -> (f64, f64) {
    if a <= b { (a, b) } else { (b, a) }
}

#[cfg(test)]
mod tests;
