//! Where a part's strokes meet, and what becomes of each there (`REQ-AUTO-088`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Letters sewn as strokes. Kept apart from the reading
//! of strokes and the making of their columns ([`super`]) so that each file stays short enough to read.

use stitchcraft_core::{Exhausted, Meter, Point};

use super::{APEX, Band, CROSSING, Cut, Joined, SLIVER, TIE, Way, cosine, polygon, sine, unit_of};
use crate::normalize::region::{Polygon, ops};

/// The cuts of a join: each the stroke, its cut, and the end it ends under the other with and that other.
pub(super) type Cuts = Vec<(usize, Cut, Option<(f64, usize)>)>;

/// What becomes of each of `bands` where they meet in `part` (`REQ-AUTO-088`), the stroke that stops
/// reaching `reach` past the other along its line, with `meter`'s work; `None` when 2 strokes lie along each
/// other or a region can't be measured.
pub(super) fn joins(part: &Polygon, bands: &[Band], reach: f64, meter: &mut Meter) -> Result<Option<Vec<Joined>>, Exhausted> {
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
pub(super) fn meeting(part: &Polygon, one: &Band, other: &Band, meter: &mut Meter) -> Result<Option<Option<Vec<Point>>>, Exhausted> {
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
pub(super) fn join(part: &Polygon, (i, one): (usize, &Band), (j, other): (usize, &Band), meeting: &[Point], reach: f64) -> Option<Cuts> {
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
            } else if (CROSSING..=APEX).contains(&sine(one.u, other.u)) {
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
pub(super) fn stops(stopping: &Band, running: &Band, reach: f64) -> Option<Cut> {
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
pub(super) fn square(part: &Polygon, band: &Band, points: &[Point]) -> Option<f64> {
    let middle = (band.from + band.to) / 2.0;
    let sign = if end(band, points) >= middle { 1.0 } else { -1.0 };
    let inset = 0.1 * band.width();
    let a = leaves(part, band.at(middle, band.lo + inset)?, (sign * band.u.0, sign * band.u.1))?;
    let b = leaves(part, band.at(middle, band.hi - inset)?, (sign * band.u.0, sign * band.u.1))?;
    Some((a - b).abs() / band.width())
}

/// How far from `from` along `way` a ray first crosses an edge of `part`.
pub(super) fn leaves(part: &Polygon, from: Point, way: Way) -> Option<f64> {
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
pub(super) fn halved((i, one): (usize, &Band), (j, other): (usize, &Band), reach: f64) -> Option<Cuts> {
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
pub(super) fn crossed(one: &Band, other: &Band) -> Option<Point> {
    let p = one.at(0.0, (one.lo + one.hi) / 2.0)?;
    let q = other.at(0.0, (other.lo + other.hi) / 2.0)?;
    let denominator = one.u.0 * other.u.1 - one.u.1 * other.u.0;
    if denominator == 0.0 {
        return None;
    }
    let t = ((q.x() - p.x()) * other.u.1 - (q.y() - p.y()) * other.u.0) / denominator;
    Point::new(p.x() + one.u.0 * t, p.y() + one.u.1 * t).ok()
}
