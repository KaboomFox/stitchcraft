//! A satin column drawn as one path, its centre line, made into rails and rungs (`REQ-SAT-016`,
//! `REQ-SAT-017`), as Ink/Stitch makes them when it sews one.
//!
//! The rails are the centre line offset by half the stroke's width to each side, with the stroke's join
//! ([`super::offset`]): the first rail to the right of the line in y-up axes, the second to its left.
//! Rungs cross the column where its line runs straight, away from sharp corners, and at its nodes:
//!
//! - **Scores.** Each corner of the line scores the square of its turn in degrees, in the hundredth of the
//!   line's length where it lies. A blur spreads each score over 4 hundredths to either side, weighted 1,
//!   2, 4, 8, 16, 8, 4, 2, 1. A rung goes where the blurred score stops falling or starts rising: just
//!   before and after each sharp corner, where the column can bend its stitches round it.
//! - **Nodes.** A rung goes at each node of the line too, but never at its very start or end, and a straight
//!   line of 2 nodes gets one in its middle.
//! - **Spacing.** Going along the line, a rung nearer than 1 mm to the last one kept is left out.
//! - **Each rung** is perpendicular to the line where it lies, 1.2 times the column's width long, and is
//!   kept only when it crosses each rail at exactly one point. It pairs those 2 points.
//!
//! A line whose offsets split, because it crosses itself or comes back near itself, or whose ends are
//! within a CSS pixel of each other, is cut in half by length, and each half made on its own, at most 20
//! times deep. The halves' rails are joined end to end, and a rung pairs the first points of the second
//! half's rails, where the halves meet. A half whose offsets still split after 20 cuts is left out, and
//! so is one whose offset vanishes to one side, where the line turns more tightly than the column is
//! wide. A stretch shorter than a CSS pixel is left out at once, since every half of it would be.
//!
//! The rails made stay the column's rails. Ink/Stitch tells rails from rungs again among the lines it
//! made, as among drawn subpaths, and sometimes takes rungs for rails (`DEV-SAT-007`).
//!
//! Before all this, a closed line is rolled to start in the middle of its first segment where a rung, a
//! CSS thousandth longer than the column is wide, crosses the stroke's edge exactly twice: away from where
//! the line meets itself or folds tight. A line that ends where it starts is then made to start and end
//! in the middle of its first segment, and its points are rounded to a ten-thousandth of a CSS pixel, the
//! points that then repeat left out. Ink/Stitch measures in CSS pixels, and these are its limits.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point, math};

use crate::design::Join;
use crate::normalize::near::{length, to_side};
use crate::normalize::offset::{self, Meet, Offset};

/// Ends nearer than this are a loop, which is cut in half: a CSS pixel, in millimetres.
const LOOP: f64 = MM_PER_SVG_PX;
/// The deepest a line is cut in half.
const MOST_CUTS: u32 = 20;
/// Rungs nearer than this to the last one kept are left out, in millimetres.
const RUNG_SPACING: f64 = 1.0;
/// How long a rung is, as a fraction of the column's width.
const RUNG_REACH: f64 = 1.2;
/// How far along the line, as a fraction of its length, the direction at a rung is taken.
const TANGENT_STEP: f64 = 0.001;
/// The blur spread over the scores, 4 hundredths each way.
const BLUR: [i64; 9] = [1, 2, 4, 8, 16, 8, 4, 2, 1];
/// Points are rounded to this, in millimetres: a ten-thousandth of a CSS pixel.
const GRID: f64 = 1e-4 * MM_PER_SVG_PX;
/// How much longer than the column is wide the rung that tests a closed line's start is: a thousandth of
/// a CSS pixel.
const TEST_RUNG_EXTRA: f64 = 0.001 * MM_PER_SVG_PX;

/// A column's rails and rungs, made from its centre line.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Made {
    /// The 2 rails.
    pub(crate) rails: [Vec<Point>; 2],
    /// The rungs, as the points of the rails each pairs, the first rail's first: where each rung crosses
    /// them, and where 2 parts meet, the first points of the second part's rails.
    pub(crate) rungs: Vec<[Point; 2]>,
    /// How many stretches of the line were left out: parts whose offsets still split after 20 cuts, or
    /// whose offset vanishes to one side because the line turns more tightly than the column is wide.
    pub(crate) left_out: usize,
}

/// A part of the column: its 2 rails and its rungs, as [`Made`] has them.
#[derive(Clone, Debug, PartialEq)]
struct Part {
    rails: [Vec<Point>; 2],
    rungs: Vec<[Point; 2]>,
}

/// The rails and rungs of the column whose centre line is `line` (`closed` when its path closes with Z),
/// `width` wide, with `join` at its corners; `None` when no part of it can be made.
pub(crate) fn rails_and_rungs(line: &[Point], closed: bool, width: f64, join: Join, meter: &mut Meter) -> Result<Option<Made>, Exhausted> {
    let line = if closed { first_node(line, width, meter)? } else { line.to_vec() };
    let line = rounded(&looped(&line));
    let mut pieces = Vec::new();
    made(&line, width, join, 0, &mut pieces, meter)?;
    let pieces: Vec<Option<Part>> = pieces.into_iter().map(|piece| piece.filter(|part| part.rails.iter().all(|rail| rail.len() >= 2))).collect();
    let left_out = pieces.split(Option::is_some).filter(|run| !run.is_empty()).count();
    let mut parts = pieces.into_iter().flatten();
    let Some(mut joined) = parts.next() else { return Ok(None) };
    for part in parts {
        join_part(&mut joined, part);
    }
    let Part { rails, rungs } = joined;
    Ok(Some(Made { rails, rungs, left_out }))
}

/// Adds the pieces of `line` to `pieces`, in order along it: each part made, or `None` for a stretch left
/// out. The line is cut in half where it cannot be made, `depth` cuts deep so far. A line shorter than
/// [`LOOP`] is left out at once: its ends, and every half's, are nearer than that.
fn made(line: &[Point], width: f64, join: Join, depth: u32, pieces: &mut Vec<Option<Part>>, meter: &mut Meter) -> Result<(), Exhausted> {
    meter.charge(1)?;
    if total(line) < LOOP {
        pieces.push(None);
        return Ok(());
    }
    if let Some(part) = part(line, width, join, meter)? {
        pieces.push(Some(part));
        return Ok(());
    }
    if depth >= MOST_CUTS {
        pieces.push(None);
        return Ok(());
    }
    let (first, second) = halves(line);
    made(&first, width, join, depth + 1, pieces, meter)?;
    made(&second, width, join, depth + 1, pieces, meter)
}

/// The part made from `line`, or `None` when it must be cut: its ends meet, or an offset splits.
fn part(line: &[Point], width: f64, join: Join, meter: &mut Meter) -> Result<Option<Part>, Exhausted> {
    if line.first().zip(line.last()).is_none_or(|(start, end)| length(*start, *end) < LOOP) {
        return Ok(None);
    }
    let half = width / 2.0;
    let rail = |offset: Offset| match offset {
        Offset::Line(points) => Some(points),
        Offset::Empty => Some(Vec::new()),
        Offset::Apart(_) => None,
    };
    let (Some(a), Some(b)) = (rail(offset::offset(line, -half, join, meter)?), rail(offset::offset(line, half, join, meter)?)) else {
        return Ok(None);
    };
    let rungs = rungs(line, width, [&a, &b], meter)?;
    Ok(Some(Part { rails: [a, b], rungs }))
}

/// Adds `part` to the end of `joined`: each rail continues with the part's, less its first point, and a
/// rung pairs those first points, where the parts meet.
fn join_part(joined: &mut Part, part: Part) {
    let Part { rails: [a, b], rungs } = part;
    if let (Some(&a0), Some(&b0)) = (a.first(), b.first()) {
        joined.rungs.push([a0, b0]);
    }
    joined.rails[0].extend(a.iter().skip(1));
    joined.rails[1].extend(b.iter().skip(1));
    joined.rungs.extend(rungs);
}

/// `line` made to start and end in the middle of its first segment when it ends where it starts.
fn looped(line: &[Point]) -> Vec<Point> {
    match line {
        [first, second, ..] if line.last() == Some(first) => {
            let middle = Point::new((first.x() + second.x()) / 2.0, (first.y() + second.y()) / 2.0).unwrap_or(*first);
            std::iter::once(middle).chain(line.iter().skip(1).copied()).chain([*first, middle]).collect()
        }
        _ => line.to_vec(),
    }
}

/// `line` with its coordinates rounded to [`GRID`], without the points that then repeat the one before.
fn rounded(line: &[Point]) -> Vec<Point> {
    let round = |v: f64| (v / GRID).round_ties_even() * GRID;
    let mut out: Vec<Point> = Vec::with_capacity(line.len());
    for p in line {
        let q = Point::new(round(p.x()), round(p.y())).unwrap_or(*p);
        if out.last() != Some(&q) {
            out.push(q);
        }
    }
    out
}

/// The line's length, summed segment by segment.
fn total(line: &[Point]) -> f64 {
    line.windows(2).map(|pair| length(pair[0], pair[1])).sum()
}

/// The point `at` along `line` from its start: its start before 0, its end beyond its length.
fn point_at(line: &[Point], at: f64) -> Point {
    let (Some(&first), Some(&last)) = (line.first(), line.last()) else { return Point::ORIGIN };
    if at <= 0.0 {
        return first;
    }
    let mut start = 0.0;
    for pair in line.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let span = length(a, b);
        if start + span > at {
            let t = (at - start) / span;
            return Point::new(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y())).unwrap_or(a);
        }
        start += span;
    }
    last
}

/// `line` cut in half by length: from its start to its middle, and from its middle to its end. Each half
/// keeps the line's points that lie strictly within it.
fn halves(line: &[Point]) -> (Vec<Point>, Vec<Point>) {
    let all = total(line);
    let middle_at = all / 2.0;
    let middle = point_at(line, middle_at);
    let (mut first, mut second) = (vec![point_at(line, 0.0)], vec![middle]);
    let mut at = 0.0;
    for pair in line.windows(2) {
        let p = pair[0];
        if 0.0 < at && at < middle_at {
            first.push(p);
        }
        if middle_at < at && at < all {
            second.push(p);
        }
        at += length(p, pair[1]);
    }
    first.push(middle);
    second.push(point_at(line, all));
    (first, second)
}

/// The rungs of the part along `line`, `width` wide, between its `rails` (see the module docs), each as
/// the points where it crosses them.
fn rungs(line: &[Point], width: f64, rails: [&[Point]; 2], meter: &mut Meter) -> Result<Vec<[Point; 2]>, Exhausted> {
    let all = total(line);
    let mut places: Vec<f64> = minima(&blurred(&scores(line, all, meter)?)).into_iter().map(index_f64).collect();
    for &p in line {
        places.push(projected(line, p, meter)? / all * 100.0);
    }
    places.sort_by(f64::total_cmp);
    places.dedup();
    places.retain(|&place| place != 0.0 && place != 100.0);
    if places.is_empty() {
        places.push(50.0);
    }
    let mut kept = Vec::new();
    let mut last: Option<Point> = None;
    for place in places {
        meter.charge(1)?;
        let fraction = place / 100.0;
        let centre = point_at(line, fraction * all);
        if last.is_some_and(|l| length(centre, l) < RUNG_SPACING) {
            continue;
        }
        last = Some(centre);
        let ahead = point_at(line, (fraction + TANGENT_STEP) * all);
        let (dx, dy) = (ahead.x() - centre.x(), ahead.y() - centre.y());
        let span = (dx * dx + dy * dy).sqrt();
        let (tx, ty) = if span == 0.0 { (0.0, 0.0) } else { (dx / span, dy / span) };
        let (ox, oy) = (-ty * (width / 2.0) * RUNG_REACH, tx * (width / 2.0) * RUNG_REACH);
        let rung = [Point::new(centre.x() + ox, centre.y() + oy).unwrap_or(centre), Point::new(centre.x() - ox, centre.y() - oy).unwrap_or(centre)];
        if let [Some(a), Some(b)] = rails.map(|rail| crossing(rung, rail)) {
            kept.push([a, b]);
        }
    }
    Ok(kept)
}

/// Each corner's score, its turn in degrees squared, added into the hundredth of the line's length `all`
/// where it lies, whole numbers kept as Ink/Stitch's array of 32-bit integers keeps them.
fn scores(line: &[Point], all: f64, meter: &mut Meter) -> Result<[i64; 101], Exhausted> {
    let mut scores = [0_i64; 101];
    let mut so_far = 0.0;
    let mut previous: Option<(f64, f64)> = None;
    for pair in line.windows(2) {
        meter.charge(1)?;
        let (a, b) = (pair[0], pair[1]);
        let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
        let span = (dx * dx + dy * dy).sqrt();
        let direction = if span == 0.0 { (0.0, 0.0) } else { (dx / span, dy / span) };
        if let Some((px, py)) = previous {
            let cos = (px * direction.0 + py * direction.1).clamp(-1.0, 1.0);
            let angle = math::to_degrees(math::acos(cos)).abs();
            let bucket = (so_far / all * 100.0).round_ties_even();
            // The bucket is a whole number from 0 to 100; a score stays far below 2^53.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
            if let Some(score) = scores.get_mut(bucket as usize) {
                *score = (*score as f64 + angle * angle).trunc() as i64;
            }
        }
        so_far += span;
        previous = Some(direction);
    }
    Ok(scores)
}

/// `scores` blurred by [`BLUR`], the same length, as if zero beyond its ends.
fn blurred(scores: &[i64; 101]) -> [i64; 101] {
    let mut out = [0_i64; 101];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = BLUR.iter().enumerate().map(|(k, w)| (i + k).checked_sub(4).and_then(|j| scores.get(j)).map_or(0, |s| s * w)).sum();
    }
    out
}

/// Where `values` stops falling or starts rising: each place whose step up from the one before is less
/// than its step up to the next, counting only the steps' signs.
fn minima(values: &[i64; 101]) -> Vec<usize> {
    let steps: Vec<i64> = values.windows(2).map(|pair| (pair[1] - pair[0]).signum()).collect();
    steps.windows(2).enumerate().filter(|(_, pair)| pair[1] > pair[0]).map(|(i, _)| i + 1).collect()
}

/// `i` as a float: places are below 101.
#[allow(clippy::cast_precision_loss)]
fn index_f64(i: usize) -> f64 {
    i as f64
}

/// The point where the segment `rung` crosses the polyline `rail`, when it is exactly one.
fn crossing(rung: [Point; 2], rail: &[Point]) -> Option<Point> {
    let mut found: Vec<Point> = Vec::new();
    for pair in rail.windows(2) {
        match offset::segments_meet(rung[0], rung[1], pair[0], pair[1]) {
            Meet::Apart => {}
            Meet::At(p) => {
                if !found.contains(&p) {
                    found.push(p);
                }
            }
            Meet::Along(_) => return None,
        }
    }
    match found.as_slice() {
        [one] => Some(*one),
        _ => None,
    }
}

/// `ring`, a closed line, rolled to start in the middle of its first segment where a rung across the
/// stroke's edge crosses it exactly twice; unchanged when no segment has one.
fn first_node(ring: &[Point], width: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    Ok(match start_segment(ring, width, TEST_RUNG_EXTRA, meter)? {
        Some(middle) => rolled(ring, projected(ring, middle, meter)?),
        None => ring.to_vec(),
    })
}

/// The middle of the first segment of `ring` whose rung, `extra` longer than `width`, crosses the edge of
/// the stroke `width` wide exactly twice.
fn start_segment(ring: &[Point], width: f64, extra: f64, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let radius = width / 2.0;
    let half = (width + extra) / 2.0;
    for pair in ring.windows(2) {
        let (a, b) = (pair[0], pair[1]);
        let span = length(a, b);
        if span == 0.0 {
            continue;
        }
        let middle = a.lerp(b, 0.5);
        let (nx, ny) = (-(b.y() - a.y()) / span * half, (b.x() - a.x()) / span * half);
        let rung = (Point::new(middle.x() + nx, middle.y() + ny).unwrap_or(middle), Point::new(middle.x() - nx, middle.y() - ny).unwrap_or(middle));
        if edge_crossings(rung, ring, radius, meter)? == 2 {
            return Ok(Some(middle));
        }
    }
    Ok(None)
}

/// How many times the segment `rung` crosses the edge of the stroke `radius` wide on each side of `ring`:
/// the boundary of the points within `radius` of it.
fn edge_crossings(rung: (Point, Point), ring: &[Point], radius: f64, meter: &mut Meter) -> Result<usize, Exhausted> {
    let mut within: Vec<(f64, f64)> = Vec::new();
    for pair in ring.windows(2) {
        meter.charge(1)?;
        if let Some(span) = near_segment(rung, pair[0], pair[1], radius) {
            within.push(span);
        }
    }
    within.sort_by(|x, y| x.0.total_cmp(&y.0));
    let mut crossings = 0;
    let mut current: Option<(f64, f64)> = None;
    for (lo, hi) in within {
        match current {
            Some((clo, chi)) if lo <= chi => current = Some((clo, chi.max(hi))),
            _ => {
                crossings += current.map_or(0, ends_on);
                current = Some((lo, hi));
            }
        }
    }
    crossings += current.map_or(0, ends_on);
    Ok(crossings)
}

/// How many ends of the stretch from `lo` to `hi` lie on the rung, from 0 to 1. An end of the rung on the
/// edge counts, as shapely counts a point where the rung touches the edge.
fn ends_on((lo, hi): (f64, f64)) -> usize {
    usize::from((0.0..=1.0).contains(&lo)) + usize::from((0.0..=1.0).contains(&hi))
}

/// The stretch of the line through `rung`, from its start (0) to its end (1), within `radius` of the
/// segment from `a` to `b`, if any: a stretch, since the points near a segment form a convex shape. A rung
/// of no length has no line, and is near nothing.
fn near_segment(rung: (Point, Point), a: Point, b: Point, radius: f64) -> Option<(f64, f64)> {
    let (r0, r1) = rung;
    let (dx, dy) = (r1.x() - r0.x(), r1.y() - r0.y());
    let qa = dx * dx + dy * dy;
    if qa == 0.0 {
        return None;
    }
    let mut spans = Vec::with_capacity(3);
    // Near either end: inside the circle round it.
    for c in [a, b] {
        let (fx, fy) = (r0.x() - c.x(), r0.y() - c.y());
        let qb = 2.0 * (dx * fx + dy * fy);
        let qc = fx * fx + fy * fy - radius * radius;
        let disc = qb * qb - 4.0 * qa * qc;
        if disc >= 0.0 {
            let root = disc.sqrt();
            spans.push(((-qb - root) / (2.0 * qa), (-qb + root) / (2.0 * qa)));
        }
    }
    // Beside the segment: within its length along it, and within the radius across it.
    let span = length(a, b);
    if span > 0.0 {
        let (tx, ty) = ((b.x() - a.x()) / span, (b.y() - a.y()) / span);
        let along = (((r0.x() - a.x()) * tx + (r0.y() - a.y()) * ty), dx * tx + dy * ty);
        let across = ((r0.x() - a.x()) * -ty + (r0.y() - a.y()) * tx, dx * -ty + dy * tx);
        if let Some(s) = clip((f64::NEG_INFINITY, f64::INFINITY), along, 0.0, span).and_then(|s| clip(s, across, -radius, radius)) {
            spans.push(s);
        }
    }
    let lo = spans.iter().map(|s| s.0).fold(f64::INFINITY, f64::min);
    let hi = spans.iter().map(|s| s.1).fold(f64::NEG_INFINITY, f64::max);
    (lo <= hi).then_some((lo, hi))
}

/// The part of the stretch `span` where `f0 + f1·u` lies from `min` to `max`, if any.
fn clip(span: (f64, f64), (f0, f1): (f64, f64), min: f64, max: f64) -> Option<(f64, f64)> {
    let (mut lo, mut hi) = span;
    if f1 == 0.0 {
        return (min <= f0 && f0 <= max).then_some(span);
    }
    let (u0, u1) = ((min - f0) / f1, (max - f0) / f1);
    lo = lo.max(u0.min(u1));
    hi = hi.min(u0.max(u1));
    (lo <= hi).then_some((lo, hi))
}

/// How far along `line` its point nearest `p` lies, as GEOS projects a point: the first segment nearest
/// `p`, the measure up to it, and the fraction of it to the foot of `p`.
fn projected(line: &[Point], p: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
    let (mut best, mut place, mut start) = (f64::INFINITY, 0.0, 0.0);
    for pair in line.windows(2) {
        meter.charge(1)?;
        let (a, b) = (pair[0], pair[1]);
        let span = length(a, b);
        let distance = to_side(p, a, b);
        if distance < best {
            let factor = if p == a {
                0.0
            } else if p == b {
                1.0
            } else {
                let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
                ((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / (dx * dx + dy * dy)
            };
            best = distance;
            place = start + factor.clamp(0.0, 1.0) * span;
        }
        start += span;
    }
    Ok(place)
}

/// The closed line `ring` made to start `at` along it: cut there, the piece after the cut first, and
/// unchanged when `at` is its start or end.
fn rolled(ring: &[Point], at: f64) -> Vec<Point> {
    let mut travelled = 0.0;
    let segment = ring.windows(2).position(|pair| {
        travelled += length(pair[0], pair[1]);
        travelled >= at
    });
    match segment {
        Some(i) if at > 0.0 && at < total(ring) => {
            if travelled == at {
                // At the node ending the segment: the ring from it round to it again.
                ring.iter().skip(i + 1).chain(ring.iter().skip(1).take(i + 1)).copied().collect()
            } else {
                let cut = point_at(ring, at);
                std::iter::once(cut).chain(ring.iter().skip(i + 1).copied()).chain(ring.iter().skip(1).take(i).copied()).chain([cut]).collect()
            }
        }
        _ => ring.to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::normalize::fixture::cases;

    /// Shapely's answers for random lines (`conformance/fixtures/geometry/shapely-centre-line.txt`, written
    /// by `conformance/oracle/centre_line.py`).
    const SHAPELY: &str = include_str!("../../../../conformance/fixtures/geometry/shapely-centre-line.txt");

    /// Answers this close are the same: shapely sums lengths and interpolates in its own order.
    const CLOSE: f64 = 1e-9;

    fn same(a: &[Point], b: &[Point]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(p, q)| p.distance(*q) <= CLOSE)
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn points(coordinates: &[(f64, f64)]) -> Vec<Point> {
        coordinates.iter().map(|&(x, y)| p(x, y)).collect()
    }

    #[test]
    fn a_closed_line_is_rolled_to_start_where_it_is_cut() {
        let square = points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]);
        // At a node: the ring from it round to it again.
        assert_eq!(rolled(&square, 10.0), points(&[(10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0), (10.0, 0.0)]));
        // Between nodes: cut there.
        assert_eq!(rolled(&square, 15.0), points(&[(10.0, 5.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0), (10.0, 0.0), (10.0, 5.0)]));
        // At its start or its end, as it is, even when its end is repeated.
        assert_eq!(rolled(&square, 0.0), square);
        assert_eq!(rolled(&square, 40.0), square);
        let repeated = points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0), (0.0, 0.0)]);
        assert_eq!(rolled(&repeated, 40.0), repeated);
    }

    #[test]
    fn a_point_as_near_2_segments_is_placed_on_the_first() {
        // As GEOS projects it: the middle of a U lies 1 mm from both its arms.
        let u = points(&[(0.0, 0.0), (10.0, 0.0), (10.0, 2.0), (0.0, 2.0)]);
        assert_eq!(projected(&u, p(5.0, 1.0), &mut Budget::DEFAULT.meter()).unwrap(), 5.0);
    }

    #[test]
    fn a_closed_line_keeps_its_start_when_no_rung_crosses_the_edge_twice() {
        let meter = &mut Budget::DEFAULT.meter();
        // A rectangle 1 mm high in a stroke 2 mm wide: every rung's inner end lies inside the stroke.
        let thin = points(&[(0.0, 0.0), (20.0, 0.0), (20.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
        assert_eq!(first_node(&thin, 2.0, meter).unwrap(), thin);
        // 2 mm high, it starts in the middle of its first segment.
        let wide = points(&[(0.0, 0.0), (20.0, 0.0), (20.0, 2.0), (0.0, 2.0), (0.0, 0.0)]);
        assert_eq!(first_node(&wide, 1.0, meter).unwrap(), points(&[(10.0, 0.0), (20.0, 0.0), (20.0, 2.0), (0.0, 2.0), (0.0, 0.0), (10.0, 0.0)]));
        // A segment of no length is passed over.
        let square = points(&[(0.0, 0.0), (0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]);
        assert_eq!(start_segment(&square, 2.0, TEST_RUNG_EXTRA, meter).unwrap(), Some(p(5.0, 0.0)));
    }

    #[test]
    fn a_rung_along_a_rail_does_not_cross_it_once() {
        let rail = points(&[(1.0, 0.0), (3.0, 0.0), (3.0, 2.0)]);
        assert_eq!(crossing([p(0.0, 0.0), p(2.0, 0.0)], &rail), None, "along");
        assert_eq!(crossing([p(2.0, -1.0), p(2.0, 1.0)], &rail), Some(p(2.0, 0.0)), "once");
        assert_eq!(crossing([p(2.0, 1.0), p(4.0, 1.0)], &rail), Some(p(3.0, 1.0)), "once, the other segment");
        assert_eq!(crossing([p(2.0, 1.0), p(4.0, -1.0)], &rail), Some(p(3.0, 0.0)), "once, at the corner both segments share");
        assert_eq!(crossing([p(1.5, -0.5), p(3.5, 1.5)], &rail), None, "twice");
    }

    #[test]
    fn the_rung_that_tests_a_start_is_a_thousandth_of_a_pixel_longer_than_the_column_is_wide() {
        // A rectangle 1.001 mm high in a stroke 1 mm wide leaves a gap 0.001 mm high inside the stroke. A
        // rung across the bottom 0.00026 mm longer than the stroke is wide stops short of the gap's far side,
        // so it crosses the edge twice and the bottom is the start. A rung 0.0038 mm longer would cross the
        // edge 3 times, and the start would move to the right side.
        let rectangle = points(&[(0.0, 0.0), (20.0, 0.0), (20.0, 1.001), (0.0, 1.001), (0.0, 0.0)]);
        assert_eq!(start_segment(&rectangle, 1.0, TEST_RUNG_EXTRA, &mut Budget::DEFAULT.meter()).unwrap(), Some(p(10.0, 0.0)));
    }

    #[test]
    fn a_rung_whose_end_touches_the_edge_meets_it_there() {
        // A rectangle 2.5 mm high in a stroke 2 mm wide leaves a gap inside the stroke from 1 mm to 1.5 mm
        // high. A rung from 1.5 mm below the bottom to 1.5 mm above it ends on the gap's far side, which
        // shapely counts as a third meeting, as it counts a rung starting there.
        let ring = points(&[(0.0, 0.0), (20.0, 0.0), (20.0, 2.5), (0.0, 2.5), (0.0, 0.0)]);
        let meter = &mut Budget::DEFAULT.meter();
        assert_eq!(edge_crossings((p(10.0, -1.5), p(10.0, 1.5)), &ring, 1.0, meter).unwrap(), 3);
        assert_eq!(edge_crossings((p(10.0, 1.5), p(10.0, -1.5)), &ring, 1.0, meter).unwrap(), 3);
        assert_eq!(edge_crossings((p(10.0, -1.5), p(10.0, 1.4)), &ring, 1.0, meter).unwrap(), 2, "short of the far side");
    }

    #[test]
    fn a_rung_is_near_a_segment_beside_it_and_round_its_ends() {
        let (a, b) = (p(0.0, 0.0), p(10.0, 0.0));
        let close = |found: Option<(f64, f64)>, lo: f64, hi: f64| found.is_some_and(|(l, h)| (l - lo).abs() < 1e-12 && (h - hi).abs() < 1e-12);
        // Across its middle: from 1 mm below it to 1 mm above, on a rung 4 mm long.
        assert!(close(near_segment((p(5.0, -2.0), p(5.0, 2.0)), a, b, 1.0), 0.25, 0.75));
        // Along it: from 1 mm before its start to 1 mm past its end, on a rung 20 mm long.
        assert!(close(near_segment((p(-5.0, 0.0), p(15.0, 0.0)), a, b, 1.0), 0.2, 0.8));
        // Past its end, where x = 10.5 + 0.2y crosses the circle of radius 1 about it, both crossings beyond
        // the segment: 1.04y² + 0.2y − 0.75 = 0.
        let root = 3.16_f64.sqrt();
        let (lo, hi) = (((-0.2 - root) / 2.08 + 2.0) / 4.0, ((-0.2 + root) / 2.08 + 2.0) / 4.0);
        let tilted = (p(10.1, -2.0), p(10.9, 2.0));
        assert!(close(near_segment(tilted, a, b, 1.0), lo, hi), "{:?}", near_segment(tilted, a, b, 1.0));
        // A segment of no length is the circle about its point.
        assert!(close(near_segment(tilted, b, b, 1.0), lo, hi));
        // Far from it, and a rung of no length: near nothing.
        assert_eq!(near_segment((p(20.0, -2.0), p(20.0, 2.0)), a, b, 1.0), None);
        assert_eq!(near_segment((p(5.0, 0.0), p(5.0, 0.0)), a, b, 1.0), None);
    }

    #[test]
    fn a_point_along_no_line_is_the_origin() {
        assert_eq!(point_at(&[], 1.0), Point::ORIGIN);
    }

    #[test]
    fn a_point_at_a_node_is_the_node() {
        // In floating point 0.7 + (0.1 − 0.7) is not 0.1: a node is taken as it is, not reached along a segment.
        let line = points(&[(0.7, 0.0), (0.1, 0.0), (0.1, 2.0)]);
        let at = length(line[0], line[1]);
        assert_eq!(point_at(&line, at), p(0.1, 0.0));
        assert_eq!(point_at(&line[..2], at), p(0.1, 0.0), "the end");
    }

    #[test]
    fn a_corner_scores_its_turn_in_degrees_squared_where_it_lies() {
        // Turns of 53.13° (cos 0.6) and 73.74° (cos 0.28), half and three quarters of the way along a line
        // 20 mm long that starts away from the origin, with segments of 3 lengths.
        let line = points(&[(1.0, 1.0), (11.0, 1.0), (14.0, 5.0), (11.0, 9.0)]);
        let found = scores(&line, total(&line), &mut Budget::DEFAULT.meter()).unwrap();
        let mut expected = [0; 101];
        (expected[50], expected[75]) = (2_822, 5_437);
        assert_eq!(found, expected);
    }

    #[test]
    fn scores_are_blurred_across_9_hundredths() {
        let mut spikes = [0; 101];
        (spikes[2], spikes[50]) = (1, 3);
        let mut expected = [0; 101];
        expected[..7].copy_from_slice(&[4, 8, 16, 8, 4, 2, 1]);
        expected[46..55].copy_from_slice(&BLUR.map(|w| 3 * w));
        assert_eq!(blurred(&spikes), expected);
    }

    #[test]
    fn rungs_1_mm_apart_are_kept() {
        let line = points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (3.0, 0.0), (4.0, 0.0)]);
        let rails = [points(&[(0.0, -1.0), (4.0, -1.0)]), points(&[(0.0, 1.0), (4.0, 1.0)])];
        let found = rungs(&line, 2.0, [&rails[0], &rails[1]], &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(found.len(), 3, "{found:?}");
        for (rung, x) in found.iter().zip([1.0, 2.0, 3.0]) {
            assert!(same(rung, &[p(x, -1.0), p(x, 1.0)]), "{found:?}");
        }
    }

    #[test]
    fn a_rung_at_a_node_is_square_to_the_segment_after_it() {
        // A turn of 22.6° to the left half way along, its rails a mitred stroke 2 mm wide: rungs 5 hundredths
        // before the corner, at it and after it. The one at the corner crosses the outer rail where the
        // second segment's offset starts and the inner rail before its corner.
        let line = points(&[(0.0, 0.0), (13.0, 0.0), (25.0, 5.0)]);
        let rails = [
            points(&[(0.0, -1.0), (13.2, -1.0), (25.0 + 5.0 / 13.0, 5.0 - 12.0 / 13.0)]),
            points(&[(0.0, 1.0), (12.8, 1.0), (25.0 - 5.0 / 13.0, 5.0 + 12.0 / 13.0)]),
        ];
        let found = rungs(&line, 2.0, [&rails[0], &rails[1]], &mut Budget::DEFAULT.meter()).unwrap();
        let expected = [
            [(11.7, -1.0), (11.7, 1.0)],
            [(13.0 + 5.0 / 13.0, -12.0 / 13.0), (13.0 - 5.0 / 12.0, 1.0)],
            [(14.2 + 5.0 / 13.0, 0.5 - 12.0 / 13.0), (14.2 - 5.0 / 13.0, 0.5 + 12.0 / 13.0)],
        ];
        assert_eq!(found.len(), expected.len(), "{found:?}");
        for (rung, wanted) in found.iter().zip(expected) {
            assert!(same(rung, &points(&wanted)), "{found:?}");
        }
    }

    #[test]
    fn halves_keep_the_points_strictly_within_them() {
        // As shapely's substring does: a node at the middle is the middle, and a repeated end is the end.
        let line = points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 0.0)]);
        assert_eq!(halves(&line), (points(&[(0.0, 0.0), (1.0, 0.0)]), points(&[(1.0, 0.0), (2.0, 0.0)])));
    }

    #[test]
    fn a_line_exactly_a_pixel_long_is_made() {
        // Ink/Stitch cuts a line whose ends are nearer than a CSS pixel, and offsets one whose ends are a
        // pixel apart.
        let line = points(&[(0.0, 0.0), (LOOP, 0.0)]);
        let made = rails_and_rungs(&line, false, 1.0, Join::UNSET, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert_eq!(made.rails, [points(&[(0.0, -0.5), (LOOP, -0.5)]), points(&[(0.0, 0.5), (LOOP, 0.5)])]);
        assert_eq!(made.left_out, 0);
    }

    #[test]
    fn a_line_still_closed_after_20_cuts_is_left_out() {
        // Squares through the origin, each half as wide as the one before and the last one twice, so that
        // every cut in half falls at the origin. Square k is the first half after k + 1 cuts, closed, and
        // is cut once more into 2 open halves, those of the smallest squares too tight for an offset
        // inside. Square 19 and the 2 last squares together are still closed 20 cuts deep: left out.
        let mut sides: Vec<f64> = (0..=20).map(|k| 100_000.0 / f64::from(1_u32 << k)).collect();
        sides.push(100_000.0 / f64::from(1_u32 << 20));
        let mut line = vec![Point::ORIGIN];
        for s in sides {
            line.extend(points(&[(s, 0.0), (s, s), (0.0, s), (0.0, 0.0)]));
        }
        let mut pieces = Vec::new();
        made(&line, 1.0, Join::UNSET, 0, &mut pieces, &mut Budget::DEFAULT.meter()).unwrap();
        let made_parts: Vec<bool> = pieces.iter().map(Option::is_some).collect();
        assert_eq!(made_parts, [[true; 38].as_slice(), &[false; 2]].concat(), "the 2 halves of each of squares 0 to 18, then 2 left out");
        // A line shorter than a CSS pixel is left out at once, however deep.
        let mut pieces = Vec::new();
        made(&points(&[(0.0, 0.0), (0.2, 0.0)]), 1.0, Join::UNSET, 0, &mut pieces, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(pieces, [None]);
    }

    #[test]
    fn measures_are_shapely_s() {
        let meter = &mut Budget::DEFAULT.meter();
        let (mut differ, mut seen) = (Vec::new(), 0);
        for (number, mut words) in cases(SHAPELY) {
            seen += 1;
            let kind = words.word().to_string();
            let line = words.polyline();
            let agrees = match kind.as_str() {
                "half" => {
                    let (first, second) = halves(&line);
                    same(&first, &words.polyline()) && same(&second, &words.polyline())
                }
                "at" => {
                    let f = words.number();
                    point_at(&line, f * total(&line)).distance(words.point()) <= CLOSE
                }
                "place" => {
                    let p = words.point();
                    (projected(&line, p, meter).unwrap() - words.number()).abs() <= CLOSE
                }
                "once" => {
                    let rung = [words.point(), words.point()];
                    let single = words.count() == 1;
                    crossing(rung, &line).is_some() == single
                }
                "start" => {
                    let width = words.number();
                    let expected = usize::try_from(words.word().parse::<i64>().unwrap()).ok();
                    let found = start_segment(&line, width, 0.001, meter).unwrap();
                    let index = found.and_then(|m| line.windows(2).position(|pair| pair[0].lerp(pair[1], 0.5) == m));
                    index == expected
                }
                _ => panic!("line {number}: unknown kind {kind}"),
            };
            if !agrees {
                differ.push(number);
            }
        }
        assert_eq!(seen, 1200, "every case of the fixture is read");
        assert_eq!(differ, Vec::<usize>::new(), "the fixture's lines whose answer differs");
    }
}
