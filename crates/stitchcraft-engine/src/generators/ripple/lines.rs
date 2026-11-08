//! A ripple stitch's lines (`docs/src/design/algorithms/strokes.md` › Ripple stitch): which ripple a stroke
//! sews, the point it aims at, how many lines it has, where each lies, how they bend into each other and
//! which are left out.
//!
//! Every line runs through a point of each of the ripple's measures, pairs of points: line `k` through the
//! point its place `t` of the way from each measure's first point to its second. Along a column the measures
//! are the satin column's pairs across it, and towards a target point the needle points of the line's
//! running stitch, each paired with the target point. Points are mixed as `a · (1 − t) + b · t`
//! ([`Point::lerp`]), as Ink/Stitch mixes them when it bends lines, so a line at a place of 0 or 1 lies
//! exactly on a measure's end, as Ink/Stitch's does.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point, math};

use crate::generators::ripple::RippleParams;
use crate::normalize::region::Polygon;
use crate::route::trim::{Outline, hull};

/// How far apart, in millimetres, the ends of a stroke's one line lie at most for it to be closed: 0.05 CSS
/// pixels, as Ink/Stitch tells a closed line.
pub(super) const CLOSED: f64 = 0.05 * MM_PER_SVG_PX;

/// The most lines `min_line_dist_mm` gives, before a closed line or the point join adds one.
pub(super) const MOST: usize = 1000;

/// Which ripple a stroke sews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// Along a satin column, from its first rail to its second: a stroke of 2 lines or more.
    Column,
    /// Towards the target point: a stroke of 1 open line.
    Target,
    /// In a spiral to the target point: a stroke of 1 closed line.
    Spiral,
}

impl Kind {
    /// Whether the lines bend into each other, as `Some` of whether every other line flips, with the point
    /// join (`point`) or without `flip_copies`: a spiral's always, without flipping, and towards a target
    /// point with the point join, every other line flipping. Along a column, with the point join or without
    /// `flip_copies`, every other line flips when both are so.
    pub(super) const fn bend(self, point: bool, flip_copies: bool) -> Option<bool> {
        match self {
            Kind::Spiral => Some(false),
            Kind::Target if point => Some(true),
            Kind::Column if point || !flip_copies => Some(point && flip_copies),
            Kind::Target | Kind::Column => None,
        }
    }
}

/// The ripple a stroke of one line, `points`, sews: a spiral when its ends lie within [`CLOSED`] of each
/// other, and otherwise one towards its target point.
pub(super) fn kind_of(points: &[Point]) -> Kind {
    match (points.first(), points.last()) {
        (Some(first), Some(last)) if first.distance(*last) < CLOSED => Kind::Spiral,
        _ => Kind::Target,
    }
}

/// The point a ripple aims at: `given`, the target point its design gives, or the centroid of the convex
/// hull of `points`, its line's, as GEOS finds a polygon's. Points that lie in a line give the middle of the
/// 2 farthest apart, and one point itself. A unit of `meter`'s work for each point.
pub(super) fn target(given: Option<Point>, points: &[Point], meter: &mut Meter) -> Result<Point, Exhausted> {
    if let Some(given) = given {
        return Ok(given);
    }
    Ok(match hull(points, meter)? {
        Outline::Area(mut corners) => {
            corners.extend(corners.first().copied());
            Polygon { outline: corners, holes: Vec::new() }.centroid(meter)?
        }
        Outline::Lines(sides) => sides.first().map_or(points.first().copied().unwrap_or(Point::ORIGIN), |&[a, b]| a.lerp(b, 0.5)),
    })
}

/// The shortest of `lengths` as Ink/Stitch finds it: a length of 0 is taken, and the next replaces it, so
/// the shortest is that of the lengths after the last 0, or 0 when the last is 0. With none, 0.
pub(super) fn shortest(lengths: impl IntoIterator<Item = f64>) -> f64 {
    lengths.into_iter().fold(0.0, |shortest, length| if shortest == 0.0 || length < shortest { length } else { shortest })
}

/// How many lines a ripple has.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Count {
    /// The count.
    pub(super) lines: usize,
    /// The count `min_line_dist_mm` gave, when it was more than [`MOST`] and cut.
    pub(super) cut: Option<f64>,
}

/// How many lines `ripple` has, with `shortest` the shortest of its measures: its `line_count`, or with
/// `min_line_dist_mm` the shortest divided by it and rounded up, 1 less where that differs from
/// `line_count` in parity, at least 1 and at most [`MOST`] or 1 less, keeping the parity. Then 1 more when
/// `adds`, for a closed line or the point join, and at least 2 for a `column`.
pub(super) fn count(ripple: &RippleParams, shortest: f64, (adds, column): (bool, bool)) -> Count {
    let given = usize::try_from(ripple.line_count).unwrap_or(MOST).max(1);
    let parity = given % 2;
    let (mut lines, mut cut) = (given, None);
    if let Some(distance) = ripple.min_line_dist_mm {
        let raw = (shortest / distance.get()).ceil();
        let kept = if raw % 2.0 == f64::from(u8::from(parity == 1)) { raw } else { raw - 1.0 };
        match up_to_most(kept) {
            Some(whole) => lines = whole.max(1),
            None => (lines, cut) = (MOST - usize::from(MOST % 2 != parity), Some(kept)),
        }
    }
    lines += usize::from(adds);
    if column {
        lines = lines.max(2);
    }
    Count { lines, cut }
}

/// `count`, a whole number from −1 up, as a count of at least 0, or `None` above [`MOST`].
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)] // Checked against MOST first.
fn up_to_most(count: f64) -> Option<usize> {
    (count <= MOST as f64).then(|| count.max(0.0) as usize)
}

/// The places of `n` lines, from 0 to 1: line `k`'s `(k / (n − 1))^exponent`, and with `flip`, 1 less line
/// `n − 1 − k`'s. One line is at 0, or 1 flipped.
pub(super) fn places(n: usize, exponent: f64, flip: bool) -> Vec<f64> {
    let last = n.saturating_sub(1);
    let raised: Vec<f64> = (0..n).map(|k| math::pow(share(k, last), exponent)).collect();
    if flip { raised.iter().rev().map(|place| 1.0 - place).collect() } else { raised }
}

/// `k` of `last` as a share from 0 to 1, as `numpy.linspace` lays them out: exactly 1 for `last` itself, a number
/// divided by itself, and 0 when `last` is 0.
#[allow(clippy::cast_precision_loss)] // Line and point counts are far below 2⁵².
fn share(k: usize, last: usize) -> f64 {
    k as f64 / last.max(1) as f64
}

/// The lines through `measures` at `places`: line `k` through the point its place of the way from each
/// measure's first point to its second. They must fit in `meter`'s stitches left, and each point costs a
/// unit of its work.
pub(super) fn lines(measures: &[[Point; 2]], places: &[f64], meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    meter.room(measures.len().saturating_mul(places.len()))?;
    let mut lines = Vec::with_capacity(places.len());
    for &place in places {
        meter.charge(u64::try_from(measures.len()).unwrap_or(u64::MAX))?;
        lines.push(measures.iter().map(|&[from, to]| from.lerp(to, place)).collect());
    }
    Ok(lines)
}

/// `lines` bent into each other, 1 fewer: at measure `i` of `m`, line `k` lies `i / (m − 1)` of the way from
/// its own point to line `k + 1`'s, so it starts on its own start and ends on the next line's end. With
/// `flip`, every other line, from the second, lies `(m − 1 − i) / (m − 1)` of the way instead, so it starts
/// on the next line's start and ends on its own end. Each point costs a unit of `meter`'s work.
pub(super) fn bent(lines: &[Vec<Point>], flip: bool, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let last = lines.first().map_or(0, Vec::len).saturating_sub(1);
    let mut bent = Vec::with_capacity(lines.len().saturating_sub(1));
    for (k, pair) in lines.windows(2).enumerate() {
        let [own, next] = pair else { continue };
        meter.charge(u64::try_from(own.len()).unwrap_or(u64::MAX))?;
        let flipped = flip && k % 2 == 1;
        bent.push(own.iter().zip(next).enumerate().map(|(i, (&a, &b))| a.lerp(b, share(if flipped { last - i } else { i }, last))).collect());
    }
    Ok(bent)
}

/// `lines` without the first `skip[0]` and the last `skip[1]`, and the first's count. When they would leave
/// none, every line is kept, as Ink/Stitch keeps them, and the count is `None`.
pub(super) fn skipped(mut lines: Vec<Vec<Point>>, [start, end]: [usize; 2]) -> (Vec<Vec<Point>>, Option<usize>) {
    if start.saturating_add(end) >= lines.len() {
        return (lines, None);
    }
    lines.truncate(lines.len() - end);
    lines.drain(..start);
    (lines, Some(start))
}
