//! The geometry of smoothing one line of an outline (`REQ-AUTO-131`, `REQ-AUTO-134`): its corners, the line
//! resampled at even steps and averaged by a Gaussian, how far it moved, the fewest of its points that draw it,
//! and where lines cross.
//!
//! A line here is a polyline, open or closed. A closed line lists each point once: its last point joins its
//! first. Each function charges the meter for the points it walks, so that an outline of a million points
//! stops when the work runs out, as the rest of digitizing does.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Exhausted, Meter, Point, math};

/// The cosine of the least angle by which a line turns at a corner: 60°.
const CORNER_COS: f64 = 0.5;

/// How wide each cell is of the grid that finds the segments that may cross, in millimetres.
const CELL: f64 = 1.0;

/// The units `meter` is charged for `count` things.
fn units(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// How far apart `a` and `b` are.
fn between(a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    (dx * dx + dy * dy).sqrt()
}

/// The segments of a line: each point and the next, and for a closed line its last point and its first.
fn segments(points: &[Point], closed: bool) -> impl Iterator<Item = (Point, Point)> + '_ {
    let close = points.last().copied().zip(points.first().copied()).filter(|_| closed && points.len() > 2);
    points.windows(2).filter_map(|pair| Some((*pair.first()?, *pair.get(1)?))).chain(close)
}

/// How far along the line each of its points lies, and last how long it is: for a closed line, with the
/// segment that closes it.
fn arcs(points: &[Point], closed: bool) -> Vec<f64> {
    let mut arcs = Vec::with_capacity(points.len() + 1);
    let mut along = 0.0;
    arcs.push(along);
    for (a, b) in segments(points, closed) {
        along += between(a, b);
        arcs.push(along);
    }
    arcs
}

/// How long the line is: for a closed line, with the segment that closes it.
pub(super) fn length(points: &[Point], closed: bool) -> f64 {
    segments(points, closed).map(|(a, b)| between(a, b)).sum()
}

/// The point `along` millimetres along the line whose points lie at `arcs` ([`arcs`]): for a closed line
/// taken round it, for an open one held to its ends.
fn at(points: &[Point], arcs: &[f64], closed: bool, along: f64) -> Option<Point> {
    let total = arcs.last().copied().unwrap_or(0.0);
    let along = if closed && total > 0.0 { along.rem_euclid(total) } else { along.clamp(0.0, total) };
    // The segment that holds it: the last whose start lies no farther along.
    let segment = arcs.partition_point(|&arc| arc <= along).saturating_sub(1);
    let from = *points.get(segment)?;
    let to = points.get(segment + 1).or_else(|| points.first().filter(|_| closed)).copied().unwrap_or(from);
    let (start, end) = (*arcs.get(segment)?, arcs.get(segment + 1).copied().unwrap_or(total));
    let t = if end > start { (along - start) / (end - start) } else { 0.0 };
    Some(from.lerp(to, t.clamp(0.0, 1.0)))
}

/// The corners of a line: the points where its direction over `reach` millimetres before and after them
/// turns by 60° or more, the sharpest of each run of such points next to each other and no farther apart along
/// the line than `reach`. The ends of an open line are not among them. A unit of work for each point.
pub(super) fn corners(points: &[Point], closed: bool, reach: f64, meter: &mut Meter) -> Result<Vec<usize>, Exhausted> {
    meter.charge(units(points.len()))?;
    let arcs = arcs(points, closed);
    let turn = |index: usize| -> Option<f64> {
        let p = *points.get(index)?;
        let along = *arcs.get(index)?;
        let (before, after) = (at(points, &arcs, closed, along - reach)?, at(points, &arcs, closed, along + reach)?);
        let (ux, uy, vx, vy) = (p.x() - before.x(), p.y() - before.y(), after.x() - p.x(), after.y() - p.y());
        let lengths = (ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt();
        let cos = (ux * vx + uy * vy) / lengths;
        (lengths > 0.0 && cos <= CORNER_COS).then_some(cos)
    };
    let close = |from: usize, to: usize| arcs.get(to).zip(arcs.get(from)).is_some_and(|(to, from)| to - from <= reach);
    let inner = if closed { 0..points.len() } else { 1..points.len().saturating_sub(1) };
    // Each run of turning points, with the sharpest's index and cosine.
    let mut runs: Vec<(usize, usize, f64)> = Vec::new();
    for index in inner {
        let Some(cos) = turn(index) else { continue };
        match runs.last_mut() {
            Some((last, sharpest, least)) if *last + 1 == index && close(*last, index) => {
                *last = index;
                if cos < *least {
                    (*sharpest, *least) = (index, cos);
                }
            }
            _ => runs.push((index, index, cos)),
        }
    }
    // Round a closed line, a run that ends at its last point goes on into the one that starts at its first.
    let wraps = turn(0).is_some() && runs.last().is_some_and(|&(last, _, _)| last + 1 == points.len() && close(last, points.len()));
    if closed
        && runs.len() > 1
        && wraps
        && let Some((_, last_sharpest, last_least)) = runs.pop()
        && let Some(first) = runs.first_mut()
        && last_least < first.2
    {
        (first.1, first.2) = (last_sharpest, last_least);
    }
    let mut corners: Vec<usize> = runs.into_iter().map(|(_, sharpest, _)| sharpest).collect();
    corners.sort_unstable();
    Ok(corners)
}

/// The line through `points` resampled at `steps` even steps along it: `steps + 1` points from its start to
/// its end, or for a closed line `steps` points round it from its first. A unit of work for each point.
pub(super) fn resampled(points: &[Point], closed: bool, steps: usize, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let count = if closed { steps } else { steps + 1 };
    meter.charge(units(count))?;
    let arcs = arcs(points, closed);
    let total = arcs.last().copied().unwrap_or(0.0);
    let step = total / steps.max(1) as f64;
    Ok((0..count).map(|i| at(points, &arcs, closed, step * i as f64)).collect())
}

/// The sample `index` of `samples`, past the ends of an open line reflected through the end: the point as
/// far on the other side of the end as the sample as far inside it. Round a closed line, the samples repeat.
fn sample(samples: &[Point], closed: bool, index: i64) -> Option<(f64, f64)> {
    let count = i64::try_from(samples.len()).ok()?;
    if closed {
        let p = samples.get(usize::try_from(index.rem_euclid(count.max(1))).ok()?)?;
        return Some((p.x(), p.y()));
    }
    let (first, last) = (samples.first()?, samples.last()?);
    let end = count - 1;
    // Each reflection through an end turns the sign and adds twice the end: the sample is `sign` times the
    // sample at `index` plus `offset`.
    let (mut index, mut sign, mut offset) = (index, 1.0, (0.0, 0.0));
    while index < 0 || index > end {
        let through = if index < 0 { first } else { last };
        offset = (offset.0 + sign * 2.0 * through.x(), offset.1 + sign * 2.0 * through.y());
        sign = -sign;
        index = if index < 0 { -index } else { 2 * end - index };
        if end == 0 {
            index = 0;
        }
    }
    let p = samples.get(usize::try_from(index).ok()?)?;
    Some((offset.0 + sign * p.x(), offset.1 + sign * p.y()))
}

/// `samples` averaged by a Gaussian whose deviation is `deviation` samples, out to 3 deviations each side, the
/// ends of an open line held where they are ([`sample`]). A unit of work for each sample and each weight.
pub(super) fn blurred(samples: &[Point], closed: bool, deviation: f64, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    // A deviation this small moves no sample by a thousandth of the step.
    if deviation < 0.25 {
        return Ok(Some(samples.to_vec()));
    }
    let reach = whole((3.0 * deviation).ceil().min(samples.len() as f64 * 4.0 + 1.0));
    meter.charge(units(samples.len()).saturating_mul(units(usize::try_from(2 * reach + 1).unwrap_or(usize::MAX))))?;
    let weights: Vec<f64> = (-reach..=reach).map(|k| math::exp(-((k * k) as f64) / (2.0 * deviation * deviation))).collect();
    let sum: f64 = weights.iter().sum();
    let mut blurred = Vec::with_capacity(samples.len());
    for (index, p) in samples.iter().enumerate() {
        let index = i64::try_from(index).unwrap_or(i64::MAX);
        let (mut x, mut y) = (0.0, 0.0);
        for (k, weight) in (-reach..=reach).zip(&weights) {
            let Some((sx, sy)) = sample(samples, closed, index + k) else { return Ok(None) };
            (x, y) = (x + weight * sx, y + weight * sy);
        }
        // The ends of an open line stay exactly where they are.
        let held = !closed && (index == 0 || usize::try_from(index).is_ok_and(|i| i + 1 == samples.len()));
        blurred.push(if held { *p } else { Point::new(x / sum, y / sum).ok().unwrap_or(*p) });
    }
    Ok(Some(blurred))
}

/// How far `p` lies from the segment from `a` to `b`.
fn from_segment(p: Point, (a, b): (Point, Point)) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let squared = dx * dx + dy * dy;
    let t = if squared > 0.0 { (((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / squared).clamp(0.0, 1.0) } else { 0.0 };
    let (ex, ey) = (p.x() - a.x() - t * dx, p.y() - a.y() - t * dy);
    (ex * ex + ey * ey).sqrt()
}

/// The indices of the segments of a line `segments` long whose stretch along it, from `arcs[i]` to
/// `arcs[i + 1]`, meets `from..=to`: round a closed line the stretch is taken round it.
fn near(arcs: &[f64], closed: bool, from: f64, to: f64) -> Vec<usize> {
    let total = arcs.last().copied().unwrap_or(0.0);
    let count = arcs.len().saturating_sub(1);
    if closed && to - from >= total {
        return (0..count).collect();
    }
    let span = |from: f64, to: f64| {
        let first = arcs.partition_point(|&arc| arc < from).saturating_sub(1);
        let last = arcs.partition_point(|&arc| arc <= to).min(count);
        first..last
    };
    let mut near: Vec<usize> = span(from, to).collect();
    if closed && from < 0.0 {
        near.extend(span(from + total, total));
    }
    if closed && to > total {
        near.extend(span(0.0, to - total));
    }
    near
}

/// How far the smoothed line `smoothed` lies from the line `drawn` it was resampled from, the farther of the
/// 2 ways round: no point of either lies farther from the other. `smoothed` has a point at each even step
/// along `drawn`, so a point of one is measured to the segments of the other within `window` millimetres of
/// it along the line. A point nearer to a part of the other line farther along is measured as farther than it
/// is, so the distance never comes out shorter than it is. A unit of work for each pair measured.
pub(super) fn apart(drawn: &[Point], smoothed: &[Point], closed: bool, window: f64, meter: &mut Meter) -> Result<f64, Exhausted> {
    let drawn_arcs = arcs(drawn, closed);
    let total = drawn_arcs.last().copied().unwrap_or(0.0);
    let steps = if closed { smoothed.len() } else { smoothed.len().saturating_sub(1) };
    let step = total / steps.max(1) as f64;
    let smoothed_arcs: Vec<f64> = (0..=steps).map(|i| step * i as f64).collect();
    let drawn_segments: Vec<(Point, Point)> = segments(drawn, closed).collect();
    let smoothed_segments: Vec<(Point, Point)> = segments(smoothed, closed).collect();
    let mut farthest: f64 = 0.0;
    for (points, arcs, other, other_arcs) in
        [(smoothed, &smoothed_arcs, &drawn_segments, &drawn_arcs), (drawn, &drawn_arcs, &smoothed_segments, &smoothed_arcs)]
    {
        for (p, &along) in points.iter().zip(arcs.iter()) {
            let near = near(other_arcs, closed, along - window, along + window);
            meter.charge(units(near.len()).max(1))?;
            let nearest = near.into_iter().filter_map(|i| other.get(i)).map(|&segment| from_segment(*p, segment)).fold(f64::INFINITY, f64::min);
            farthest = farthest.max(nearest);
        }
    }
    Ok(farthest)
}

/// The fewest of the points of an open line, its ends among them, that draw it within `within` millimetres:
/// Douglas and Peucker's choice of the point farthest from each chord. A unit of work for each point measured.
pub(super) fn fewest(points: &[Point], within: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let last = points.len().saturating_sub(1);
    if last < 2 {
        return Ok(points.to_vec());
    }
    let mut kept = vec![false; points.len()];
    for at in [0, last] {
        if let Some(keep) = kept.get_mut(at) {
            *keep = true;
        }
    }
    let mut spans = vec![(0, last)];
    while let Some((from, to)) = spans.pop() {
        meter.charge(units(to - from))?;
        let (Some(&a), Some(&b)) = (points.get(from), points.get(to)) else { continue };
        let farthest = (from + 1..to)
            .filter_map(|i| Some((i, from_segment(*points.get(i)?, (a, b)))))
            .fold(None, |best: Option<(usize, f64)>, (i, d)| if best.is_some_and(|(_, b)| b >= d) { best } else { Some((i, d)) });
        if let Some((i, _)) = farthest.filter(|&(_, d)| d > within) {
            if let Some(keep) = kept.get_mut(i) {
                *keep = true;
            }
            spans.push((i, to));
            spans.push((from, i));
        }
    }
    Ok(points.iter().zip(kept).filter_map(|(p, keep)| keep.then_some(*p)).collect())
}

/// The fewest of the points of a closed line that draw it within `within` millimetres ([`fewest`]): its first
/// point and the one farthest from it are kept, and each half between them thinned as an open line is.
pub(super) fn fewest_round(points: &[Point], within: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some(&first) = points.first() else { return Ok(Vec::new()) };
    if points.len() < 4 {
        return Ok(points.to_vec());
    }
    meter.charge(units(points.len()))?;
    let far = points.iter().enumerate().fold((0, 0.0), |best, (i, &p)| if between(first, p) > best.1 { (i, between(first, p)) } else { best }).0;
    let (Some(one), Some(rest)) = (points.get(..=far), points.get(far..)) else { return Ok(points.to_vec()) };
    let other: Vec<Point> = rest.iter().copied().chain([first]).collect();
    let mut kept = fewest(one, within, meter)?;
    kept.extend(fewest(&other, within, meter)?.into_iter().skip(1));
    kept.pop();
    Ok(kept)
}

/// The area a ring of `points` encloses, by the shoelace formula: positive when it runs anticlockwise with
/// y up.
pub(super) fn area(points: &[Point]) -> f64 {
    segments(points, true).map(|(a, b)| a.x() * b.y() - b.x() * a.y()).sum::<f64>() / 2.0
}

/// Whether the segments `a` to `b` and `c` to `d` cross, other than at an end: an end that touches the other
/// segment, or segments along one line, don't cross.
fn cross((a, b): (Point, Point), (c, d): (Point, Point)) -> bool {
    let side = |p: Point, q: Point, r: Point| (q.x() - p.x()) * (r.y() - p.y()) - (q.y() - p.y()) * (r.x() - p.x());
    side(a, b, c) * side(a, b, d) < 0.0 && side(c, d, a) * side(c, d, b) < 0.0
}

/// `value`, a whole number, as an integer: the reach of a Gaussian or a cell of the grid, which a design's
/// working limit keeps far inside `i64`'s range. A value out of the range saturates.
#[allow(clippy::cast_possible_truncation, reason = "a whole number within the working limit; `as` saturates")]
fn whole(value: f64) -> i64 {
    value as i64
}

/// The cell of the grid that holds the point (`x`, `y`).
fn cell(x: f64, y: f64) -> (i64, i64) {
    (whole((x / CELL).floor()), whole((y / CELL).floor()))
}

/// The lines among `lines` that cross themselves or another of them, each line a polyline of segments with
/// whether it moved: 2 lines that didn't both stay as drawn. A unit of work for each cell of the grid a
/// segment meets, and for each pair of segments tested.
pub(super) fn crossing(lines: &[(Vec<(Point, Point)>, bool)], meter: &mut Meter) -> Result<BTreeSet<usize>, Exhausted> {
    let mut grid: BTreeMap<(i64, i64), Vec<(usize, usize)>> = BTreeMap::new();
    for (line, (segments, _)) in lines.iter().enumerate() {
        for (index, (a, b)) in segments.iter().enumerate() {
            let (low, high) = (cell(a.x().min(b.x()), a.y().min(b.y())), cell(a.x().max(b.x()), a.y().max(b.y())));
            let cells = (high.0 - low.0 + 1).saturating_mul(high.1 - low.1 + 1);
            meter.charge(u64::try_from(cells).unwrap_or(u64::MAX))?;
            for x in low.0..=high.0 {
                for y in low.1..=high.1 {
                    grid.entry((x, y)).or_default().push((line, index));
                }
            }
        }
    }
    let segment = |(line, index): (usize, usize)| lines.get(line).and_then(|(segments, moved)| Some((*segments.get(index)?, *moved)));
    let mut crossing = BTreeSet::new();
    for (&here, members) in &grid {
        meter.charge(units(members.len()).saturating_mul(units(members.len())) / 2)?;
        for (i, &one) in members.iter().enumerate() {
            for &other in members.iter().skip(i + 1) {
                let (Some((s, moved)), Some((t, other_moved))) = (segment(one), segment(other)) else { continue };
                // A pair that meets in several cells is tested in the one that holds the low corner of the
                // box they share.
                let low = cell(s.0.x().min(s.1.x()).max(t.0.x().min(t.1.x())), s.0.y().min(s.1.y()).max(t.0.y().min(t.1.y())));
                if (moved || other_moved) && low == here && cross(s, t) {
                    crossing.extend([one.0, other.0]);
                }
            }
        }
    }
    Ok(crossing)
}
