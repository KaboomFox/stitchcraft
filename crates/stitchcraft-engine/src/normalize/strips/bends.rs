//! A part's columns cut where 2 of its strokes meet at a bend of a centre line (`REQ-AUTO-115`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Joins of traced letters. Most letters of a typeface
//! don't read as strokes ([`super::strokes`]), and sew along their centre lines ([`super::columns`]). A
//! centre line runs on round an apex where no junction cuts it, as at the foot of a V, the top of an A and
//! the corners of an N, and its column turns through the apex, its stitches fanning out like the rays of a
//! sunburst. So a column is cut in 2 where 2 strokes meet:
//!
//! 1. **A bend.** Where the column's centre line turns by [`BEND`] or more, between its ways from a point
//!    [`LEGS`] of the column's width before it and to a point as far after it. The column's width is its
//!    area over its line's length. The sharpest bend is cut first, and each half is looked at again.
//! 2. **A corner inside it.** 2 strokes meet there only where the part's outline turns into the part by
//!    [`SHARP`] or more within [`TURNING`] of the width, on the inside of the bend and within [`LEGS`]
//!    widths of it: a notch, as between a V's legs. A stroke that bends by itself, as an S or a U does,
//!    turns smoothly on both sides, and isn't cut.
//! 3. **The cut** runs from the notch to the column's other side: along the line that halves the angle
//!    between the 2 halves where they meet between 15° and 60°, as 2 strokes that end at a join meet
//!    (`REQ-AUTO-088`), and else along the longer half, which runs through, the other stopping against it
//!    and ending under it. Each half's end along the cut is flat where its 2 points lie within the
//!    strokes' flat end of each other along it, and else ends in a point, the cut on its inner rail.
//! 4. **Square stitches.** Each half keeps the rungs of the column it was cut from short of the bend's
//!    measure, and from there to its end has a rung square to its way into the bend at every corner of its
//!    rails, so that its stitches lie square to the stroke up to the cut. A half whose rails run back
//!    along that way, or that doesn't draw as a satin column, isn't cut there.
//!
//! A ring isn't cut: it goes round its hole. Every step costs a unit of work.

use std::collections::BTreeSet;

use stitchcraft_core::{Exhausted, Meter, Point};

use super::strokes::{APEX, CROSSING, FLAT};
use super::{Column, Line};
use crate::generators::satin::drawn;
use crate::normalize::region::Polygon;
use crate::normalize::stroke::distance_to_segment;

/// The cosine of 40°: a centre line that turns by this or more between its ways [`LEGS`] widths before a
/// point and after it bends there.
const BEND: f64 = 0.766_044_443_118_978;

/// How far before and after a bend its ways are measured, and how far from it a corner of the outline may
/// lie, in the column's widths.
const LEGS: f64 = 1.5;

/// The cosine of 50°: the outline turns into the part by this or more at a notch where 2 strokes meet.
const SHARP: f64 = 0.642_787_609_686_539_3;

/// How far along the outline before and after a corner its turn is measured, in the column's widths: a
/// traced outline rounds a notch over several points.
const TURNING: f64 = 0.3;

/// How far into the part from a corner, in the column's widths, a notch holds the part: on the side away
/// from the middle of its 2 neighbours.
const PROBE: f64 = 0.15;

/// How close, in millimetres, 2 rungs' points may lie on a rail, and a rung's to a rail's end.
const RUNGS_APART: f64 = 0.05;

/// How far a rail may run back along a half's way, in millimetres, and still be its rail.
const BACK: f64 = 0.05;

/// How close, in millimetres, a rail's point lies to the notch to be it.
const SAME: f64 = 1e-9;

/// The most cuts a part's columns take: each cut makes one more column.
const MOST_CUTS: usize = 64;

/// A way, with length 1.
type Way = (f64, f64);

/// `found`, the columns of `part` along its centre lines, each cut in 2 at each bend where 2 strokes meet
/// (`REQ-AUTO-115`), with `meter`'s work. 2 columns of a line cut at a junction's middle where it turns
/// sharply ([`super::columns`]) are first made one again and cut so, the halves in their places. Of a
/// column cut at a bend, a half takes its place and the other comes last. A column that ended under one cut
/// ends under the half nearer its end.
pub(crate) fn bends(part: &Polygon, found: &[Column], meter: &mut Meter) -> Result<Vec<Column>, Exhausted> {
    let mut columns = found.to_vec();
    // The pairs made one and cut again, which a half's end at the cut would find again.
    let mut done = BTreeSet::new();
    for x in 0..columns.len() {
        for end in 0..2 {
            meter.charge(1)?;
            let Some(one) = columns.get(x) else { continue };
            let Some(y) = one.under.get(end).copied().flatten().filter(|&y| y != x && !done.contains(&(x.min(y), x.max(y)))) else { continue };
            let Some(other) = columns.get(y) else { continue };
            let Some((joined, at)) = joined(one, end, other) else { continue };
            if let Some(halves) = cut(part, &joined, Some(at), meter)? {
                placed(&mut columns, halves, (x, y), meter)?;
                done.insert((x.min(y), x.max(y)));
            }
        }
    }
    let mut k = 0;
    let mut cuts = 0;
    while cuts < MOST_CUTS {
        meter.charge(1)?;
        let Some(column) = columns.get(k) else { break };
        let Some(halves) = cut(part, column, None, meter)? else {
            k += 1;
            continue;
        };
        let added = columns.len();
        let copy = column.clone();
        columns.push(copy);
        placed(&mut columns, halves, (k, added), meter)?;
        cuts += 1;
    }
    Ok(columns)
}

/// `halves` put in the places `(first, second)` of `columns`, the one that stops under the other, and a
/// column that ended under what was in either place under the half nearer its end, with `meter`'s work.
fn placed(columns: &mut [Column], halves: Halves, (k, l): (usize, usize), meter: &mut Meter) -> Result<(), Exhausted> {
    let Halves { mut first, mut second, stops } = halves;
    match stops {
        Some(Stops::First) => first.under[1] = Some(l),
        Some(Stops::Second) => second.under[0] = Some(k),
        None => {}
    }
    for (i, other) in columns.iter_mut().enumerate() {
        if i == k || i == l {
            continue;
        }
        for end in 0..2 {
            let Some(under) = other.under.get(end).copied().flatten().filter(|&u| u == k || u == l) else { continue };
            meter.charge(1)?;
            let Some(at) = end_of(other, end) else { continue };
            let chosen = if nearest(&second, at) < nearest(&first, at) { l } else { k };
            if let Some(slot) = other.under.get_mut(end)
                && chosen != under
            {
                *slot = Some(chosen);
            }
        }
    }
    if let Some(slot) = columns.get_mut(k) {
        *slot = first;
    }
    if let Some(slot) = columns.get_mut(l) {
        *slot = second;
    }
    Ok(())
}

/// The column `one`, which ends under `other` at its `end`, at a junction's middle on `other`'s line, and
/// `other` made one: from `one`'s other end to the end of `other` farther from the junction, and how far
/// along its line the junction's middle lies. `other` must end within [`LEGS`] of its width beyond the
/// junction, as at an apex whose corner a column takes in, and not run on, as the bar of a T does. `None`
/// when they don't meet so, or their outlines don't share the chord across `one`'s end.
fn joined(one: &Column, end: usize, other: &Column) -> Option<(Column, f64)> {
    if one.closed || other.closed {
        return None;
    }
    let one = if end == 0 { one.reversed() } else { one.clone() };
    let middle = *one.walked.points.last()?;
    let at = other.walked.points.iter().position(|p| *p == middle)?;
    let before = Line { points: other.walked.points.get(..=at)?.to_vec(), closed: false }.length();
    let after = other.walked.length() - before;
    let width = other.outline().area() / other.line.length();
    if before.min(after) > LEGS * width {
        return None;
    }
    let other = if before <= after { other.clone() } else { other.reversed() };
    // Round `other`'s outline from each corner of the chord, away from the other corner, to its far end.
    let ring = other.outline().outline;
    let ring = ring.get(..ring.len().saturating_sub(1))?;
    let count = ring.len();
    let place = |p: &Point| ring.iter().position(|x| x == p);
    let (c0, c1) = (one.rails[0].last()?, one.rails[1].last()?);
    let (i0, i1) = (place(c0)?, place(c1)?);
    let step = if (i0 + 1) % count == i1 {
        count - 1
    } else if (i1 + 1) % count == i0 {
        1
    } else {
        return None;
    };
    let far = [other.rails[0].last()?, other.rails[1].last()?];
    let walk = |from: usize, step: usize| -> Option<(Vec<Point>, usize)> {
        let mut points = Vec::new();
        let mut k = from;
        for _ in 0..count {
            k = (k + step) % count;
            let p = ring.get(k)?;
            points.push(*p);
            if let Some(side) = far.iter().position(|f| *f == p) {
                return Some((points, side));
            }
        }
        None
    };
    let (on0, side0) = walk(i0, step)?;
    let (on1, side1) = walk(i1, count - step)?;
    if side0 == side1 {
        return None;
    }
    let mut rails = one.rails.clone();
    rails[0].extend(on0);
    rails[1].extend(on1);
    let mut rungs = one.rungs.clone();
    rungs.extend(other.rungs.iter().map(|&[x, y]| if side0 == 0 { [x, y] } else { [y, x] }));
    let at = one.line.length() + one.line.points.last().map_or(0.0, |last| last.distance(middle));
    let mut line = one.line.points.clone();
    let from = other.line.points.iter().position(|p| *p == middle)?;
    line.extend(other.line.points.get(from..)?.iter().copied());
    let mut walked = one.walked.points.clone();
    let from = other.walked.points.iter().position(|p| *p == middle)?;
    walked.extend(other.walked.points.get(from + 1..)?.iter().copied());
    let under = [one.under[0], other.under[1]];
    Some((Column::cut(rails, rungs, Line { points: line, closed: false }, Line { points: walked, closed: false }, under), at))
}

/// Which half of a column cut at a butt join stops against the other.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stops {
    /// The half from the column's start.
    First,
    /// The half to its end.
    Second,
}

/// A column cut in 2: the half from its start to the cut, the half from the cut to its end, and which
/// stops against the other, none for a cut on the halving line.
struct Halves {
    first: Column,
    second: Column,
    stops: Option<Stops>,
}

/// The middle of `column`'s rails' ends at its start (`end` 0) or its end (1).
fn end_of(column: &Column, end: usize) -> Option<Point> {
    let [one, other] = &column.rails;
    let (a, b) = if end == 0 { (one.first()?, other.first()?) } else { (one.last()?, other.last()?) };
    Point::new((a.x() + b.x()) / 2.0, (a.y() + b.y()) / 2.0).ok()
}

/// How far `p` lies from `column`'s rails.
fn nearest(column: &Column, p: Point) -> f64 {
    column.rails.iter().flat_map(|rail| rail.windows(2)).map(|w| distance_to_segment(p, w[0], w[1])).fold(f64::INFINITY, f64::min)
}

/// A bend of a column's line.
struct Bend {
    /// Its place among the line's points.
    at: usize,
    /// How far along the line it lies, in millimetres.
    along: f64,
    /// The line's points [`LEGS`] widths before it and after it.
    legs: [Point; 2],
    /// The ways into it from either side along its strokes: from the points [`LEGS`] widths before and
    /// after it to the points a width nearer, short of where the line rounds the corner.
    ways: [Way; 2],
    /// The cosine of its turn.
    cosine: f64,
}

/// `column` of `part` cut in 2 at its sharpest bend where 2 strokes meet, and only within [`LEGS`] of its
/// width of `near` along its line when given, with `meter`'s work; `None` when it has none, or is a ring.
fn cut(part: &Polygon, column: &Column, near: Option<f64>, meter: &mut Meter) -> Result<Option<Halves>, Exhausted> {
    if column.closed {
        return Ok(None);
    }
    let points = &column.line.points;
    let length = column.line.length();
    let width = column.outline().area() / length;
    if !(width > 0.0 && width.is_finite()) {
        return Ok(None);
    }
    let reach = LEGS * width;
    let mut along = Vec::with_capacity(points.len());
    let mut sum = 0.0;
    for (i, &p) in points.iter().enumerate() {
        meter.charge(1)?;
        sum += i.checked_sub(1).and_then(|before| points.get(before)).map_or(0.0, |before| before.distance(p));
        along.push(sum);
    }
    let mut found = Vec::new();
    for (i, (&b, &s)) in points.iter().zip(&along).enumerate() {
        meter.charge(1)?;
        if s < reach || s + reach > length || near.is_some_and(|near| (s - near).abs() > reach) {
            continue;
        }
        let (Some(p), Some(q)) = (point_at(points, &along, s - reach), point_at(points, &along, s + reach)) else { continue };
        let (Some(u), Some(v)) = (unit(p, b), unit(b, q)) else { continue };
        let cosine = dot(u, v);
        let (Some(p_in), Some(q_in)) = (point_at(points, &along, s - reach + width), point_at(points, &along, s + reach - width)) else { continue };
        let (Some(into_first), Some(into_second)) = (unit(p, p_in), unit(q, q_in)) else { continue };
        if cosine <= BEND {
            found.push(Bend { at: i, along: s, legs: [p, q], ways: [into_first, into_second], cosine });
        }
    }
    found.sort_by(|a, b| a.cosine.total_cmp(&b.cosine).then(a.at.cmp(&b.at)));
    for bend in &found {
        meter.charge(1)?;
        let Some(notch) = notch(part, points, bend, width, meter)? else { continue };
        if let Some(halves) = halves(part, column, bend, notch, width, meter)? {
            return Ok(Some(halves));
        }
    }
    Ok(None)
}

/// The corner of `part`'s outline nearest `bend` of the line `points` where the outline turns into the
/// part by [`SHARP`] or more within [`TURNING`] of `width`, on the inside of the bend and within [`LEGS`]
/// widths of it, with `meter`'s work.
fn notch(part: &Polygon, points: &[Point], bend: &Bend, width: f64, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let Some(&b) = points.get(bend.at) else { return Ok(None) };
    let [p, q] = bend.legs;
    let turning = cross((b.x() - p.x(), b.y() - p.y()), (q.x() - b.x(), q.y() - b.y()));
    let mut best: Option<(f64, Point)> = None;
    for ring in part.rings() {
        // A ring ends where it starts.
        let corners = ring.get(..ring.len().saturating_sub(1)).unwrap_or_default();
        for (j, &m) in corners.iter().enumerate() {
            meter.charge(1)?;
            let distance = m.distance(b);
            let inside = cross((b.x() - p.x(), b.y() - p.y()), (m.x() - b.x(), m.y() - b.y())) * turning > 0.0;
            if distance > LEGS * width || !inside || best.is_some_and(|(kept, _)| kept <= distance) {
                continue;
            }
            let (Some(a), Some(c)) = (round(corners, j, -TURNING * width, meter)?, round(corners, j, TURNING * width, meter)?) else { continue };
            let (Some(u), Some(v)) = (unit(a, m), unit(m, c)) else { continue };
            if dot(u, v) > SHARP {
                continue;
            }
            let Some(out) = unit(m, Point::new((a.x() + c.x()) / 2.0, (a.y() + c.y()) / 2.0).unwrap_or(m)) else { continue };
            let Ok(probe) = Point::new(m.x() - out.0 * PROBE * width, m.y() - out.1 * PROBE * width) else { continue };
            if part.covers(probe) {
                best = Some((distance, m));
            }
        }
    }
    Ok(best.map(|(_, m)| m))
}

/// The point of the closed ring `corners` (its last corner joined to its first) `by` millimetres round it
/// from its corner `j`, on for a positive `by` and back for a negative one, with `meter`'s work.
fn round(corners: &[Point], j: usize, by: f64, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let count = corners.len();
    let Some(&start) = corners.get(j) else { return Ok(None) };
    let mut at = start;
    let mut left = by.abs();
    let mut k = j;
    for _ in 0..count {
        meter.charge(1)?;
        k = if by >= 0.0 { (k + 1) % count } else { (k + count - 1) % count };
        let Some(&next) = corners.get(k) else { return Ok(None) };
        let step = at.distance(next);
        if step >= left && step > 0.0 {
            let share = left / step;
            return Ok(Point::new(at.x() + (next.x() - at.x()) * share, at.y() + (next.y() - at.y()) * share).ok());
        }
        left -= step;
        at = next;
    }
    Ok(None)
}

/// The halves of `column` of `part`, `width` wide, cut at `bend` from `notch` across to its other rail,
/// with `meter`'s work; `None` when they can't be made.
fn halves(part: &Polygon, column: &Column, bend: &Bend, notch: Point, width: f64, meter: &mut Meter) -> Result<Option<Halves>, Exhausted> {
    let Some(&b) = column.line.points.get(bend.at) else { return Ok(None) };
    let [p, q] = bend.legs;
    let [into_first, into_second] = bend.ways;
    let first_longer = bend.along >= column.line.length() - bend.along;
    let mitre = (CROSSING..=APEX).contains(&cross(into_first, into_second).abs());
    let (way, stops) = if mitre {
        (unit_of((into_first.0 + into_second.0, into_first.1 + into_second.1)), None)
    } else if first_longer {
        (Some(into_first), Some(Stops::Second))
    } else {
        (Some(into_second), Some(Stops::First))
    };
    let Some(way) = way else { return Ok(None) };
    // The rail the notch lies on is the inner one.
    let Some((inner, at)) =
        column.rails.iter().enumerate().find_map(|(r, rail)| rail.iter().position(|&x| x.distance(notch) <= SAME).map(|i| (r, i)))
    else {
        return Ok(None);
    };
    let outer = 1 - inner;
    let (Some(inside), Some(outside)) = (column.rails.get(inner), column.rails.get(outer)) else { return Ok(None) };
    if at == 0 || at + 1 >= inside.len() {
        return Ok(None);
    }
    meter.charge(u64::try_from(outside.len()).unwrap_or(u64::MAX))?;
    let Some((piece, across)) = hit(outside, notch, way) else { return Ok(None) };
    let middle = Point::new((notch.x() + across.x()) / 2.0, (notch.y() + across.y()) / 2.0).ok();
    if across.distance(notch) > 2.0 * LEGS * width || !middle.is_some_and(|middle| part.covers(middle)) {
        return Ok(None);
    }
    let start = |rail: &[Point], to: usize| rail.get(..=to).unwrap_or_default().to_vec();
    let finish = |rail: &[Point], from: usize| rail.get(from..).unwrap_or_default().to_vec();
    // Each half's rails run into the cut, the second's turned round for the while.
    let mut first = [start(inside, at), start(outside, piece)];
    first[1].push(across);
    let mut second = [finish(inside, at), finish(outside, piece + 1)];
    second[1].insert(0, across);
    second.iter_mut().for_each(|rail| rail.reverse());
    let flat = FLAT * width;
    for (rails, into) in [(&mut first, into_first), (&mut second, into_second)] {
        let apart = dot((across.x() - notch.x(), across.y() - notch.y()), into);
        if apart > flat {
            rails[0].push(across);
        } else if apart < -flat {
            rails[1].push(notch);
        }
        for rail in rails.iter_mut() {
            rail.dedup_by(|x, y| x.distance(*y) < RUNGS_APART / 10.0);
        }
    }
    let line = &column.line.points;
    let walked = &column.walked.points;
    let Some(in_walked) = walked.iter().position(|&x| x == b) else { return Ok(None) };
    let positions = positions(column, meter)?;
    let kept = |keep: &dyn Fn(f64) -> bool| -> Vec<[Point; 2]> {
        column.rungs.iter().zip(&positions).filter(|(_, at)| keep(**at)).map(|(&rung, _)| rung).collect()
    };
    let before = kept(&|at| at < bend.along - LEGS * width);
    let mut after = kept(&|at| at > bend.along + LEGS * width);
    after.reverse();
    // The rails in the column's order, the inner first while they are made.
    let order = |[one, other]: [Vec<Point>; 2]| if inner == 0 { [one, other] } else { [other, one] };
    let swap = |rungs: Vec<[Point; 2]>| -> Vec<[Point; 2]> { if inner == 0 { rungs } else { rungs.into_iter().map(|[x, y]| [y, x]).collect() } };
    let (Some((one_rails, one_rungs)), Some((two_rails, two_rungs))) =
        (square(first, swap(before), (p, into_first), meter)?, square(second, swap(after), (q, into_second), meter)?)
    else {
        return Ok(None);
    };
    let first_rails = order(one_rails);
    let first_rungs = swap(one_rungs);
    let mut second_rails = order(two_rails);
    second_rails.iter_mut().for_each(|rail| rail.reverse());
    let mut second_rungs = swap(two_rungs);
    second_rungs.reverse();
    for (rails, rungs) in [(&first_rails, &first_rungs), (&second_rails, &second_rungs)] {
        if drawn::from_rails(rails.clone(), rungs.clone(), [0.0, 0.0], meter)?.is_err() {
            return Ok(None);
        }
    }
    let lines = (
        Line { points: line.get(..=bend.at).unwrap_or_default().to_vec(), closed: false },
        Line { points: line.get(bend.at..).unwrap_or_default().to_vec(), closed: false },
    );
    let walks = (
        Line { points: walked.get(..=in_walked).unwrap_or_default().to_vec(), closed: false },
        Line { points: walked.get(in_walked..).unwrap_or_default().to_vec(), closed: false },
    );
    let first = Column::cut(first_rails, first_rungs, lines.0, walks.0, [column.under[0], None]);
    let second = Column::cut(second_rails, second_rungs, lines.1, walks.1, [None, column.under[1]]);
    Ok(Some(Halves { first, second, stops }))
}

/// How far along `column`'s line each of its rungs lies, in millimetres: where the line comes nearest the
/// rung's middle, with `meter`'s work.
fn positions(column: &Column, meter: &mut Meter) -> Result<Vec<f64>, Exhausted> {
    let line = &column.line.points;
    let mut found = Vec::with_capacity(column.rungs.len());
    for [x, y] in &column.rungs {
        meter.charge(u64::try_from(line.len()).unwrap_or(u64::MAX))?;
        let middle = ((x.x() + y.x()) / 2.0, (x.y() + y.y()) / 2.0);
        let (mut best, mut at, mut sum) = (f64::INFINITY, 0.0, 0.0);
        for pair in line.windows(2) {
            let [a, c] = pair else { continue };
            let (dx, dy) = (c.x() - a.x(), c.y() - a.y());
            let piece = (dx * dx + dy * dy).sqrt();
            let share = if piece > 0.0 { (((middle.0 - a.x()) * dx + (middle.1 - a.y()) * dy) / (piece * piece)).clamp(0.0, 1.0) } else { 0.0 };
            let (px, py) = (a.x() + dx * share, a.y() + dy * share);
            let distance = ((px - middle.0).powi(2) + (py - middle.1).powi(2)).sqrt();
            if distance < best {
                (best, at) = (distance, sum + piece * share);
            }
            sum += piece;
        }
        found.push(at);
    }
    Ok(found)
}

/// A half's rails `[inner, outer]`, both run into the cut, and its rungs `kept` from the column short of the
/// bend's measure, with a rung square to `into` at every corner of its rails past `from` but their ends,
/// with `meter`'s work; `None` when a rail runs back along `into` past `from`.
#[allow(clippy::type_complexity)]
fn square(
    rails: [Vec<Point>; 2],
    kept: Vec<[Point; 2]>,
    (from, into): (Point, Way),
    meter: &mut Meter,
) -> Result<Option<([Vec<Point>; 2], Vec<[Point; 2]>)>, Exhausted> {
    let along = |x: Point| dot((x.x() - from.x(), x.y() - from.y()), into);
    // Each rail from its last point short of `from`: the stretch the new rungs lie across.
    let tail = |rail: &[Point]| -> Vec<Point> {
        let first = rail.iter().rposition(|&x| along(x) < 0.0).unwrap_or(0);
        rail.get(first..).unwrap_or_default().to_vec()
    };
    let tails = [tail(&rails[0]), tail(&rails[1])];
    for rail in &tails {
        meter.charge(u64::try_from(rail.len()).unwrap_or(u64::MAX))?;
        let mut farthest = f64::NEG_INFINITY;
        for &x in rail {
            let at = along(x);
            if at < farthest - BACK {
                return Ok(None);
            }
            farthest = farthest.max(at);
        }
    }
    let meet = |rail: &[Point], at: f64| -> Option<Point> {
        rail.windows(2).find_map(|pair| {
            let [a, b] = pair else { return None };
            let (ta, tb) = (along(*a), along(*b));
            if (ta - at) * (tb - at) > 0.0 || ta == tb {
                return None;
            }
            let share = (at - ta) / (tb - ta);
            Point::new(a.x() + (b.x() - a.x()) * share, a.y() + (b.y() - a.y()) * share).ok()
        })
    };
    let mut found: Vec<(f64, [Point; 2])> = Vec::new();
    if let (Some(x), Some(y)) = (meet(&tails[0], 0.0), meet(&tails[1], 0.0)) {
        found.push((0.0, [x, y]));
    }
    for (r, rail) in tails.iter().enumerate() {
        let Some(other) = tails.get(1 - r) else { continue };
        for &x in rail.get(1..rail.len().saturating_sub(1)).unwrap_or_default() {
            meter.charge(1)?;
            let at = along(x);
            if at <= 0.0 {
                continue;
            }
            if let Some(y) = meet(other, at) {
                found.push((at, if r == 0 { [x, y] } else { [y, x] }));
            }
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    let ends = [rails[0].first(), rails[0].last(), rails[1].first(), rails[1].last()];
    let near_end = |[x, y]: [Point; 2]| ends.iter().flatten().any(|&&e| e.distance(x) < RUNGS_APART || e.distance(y) < RUNGS_APART);
    let mut rungs: Vec<[Point; 2]> = kept.into_iter().filter(|&[x, y]| along(x) < -RUNGS_APART && along(y) < -RUNGS_APART).collect();
    for (_, rung) in found {
        let crowded = rungs.last().is_some_and(|last| last[0].distance(rung[0]) < RUNGS_APART || last[1].distance(rung[1]) < RUNGS_APART);
        if !crowded && !near_end(rung) {
            rungs.push(rung);
        }
    }
    Ok(Some((rails, rungs)))
}

/// Where a ray from `from` along `way` first crosses `rail`: the piece of the rail it crosses, by its first
/// point's place, and the point.
fn hit(rail: &[Point], from: Point, way: Way) -> Option<(usize, Point)> {
    let mut nearest: Option<(f64, usize)> = None;
    for (k, pair) in rail.windows(2).enumerate() {
        let [p, q] = pair else { continue };
        let (ex, ey) = (q.x() - p.x(), q.y() - p.y());
        let denominator = way.0 * ey - way.1 * ex;
        if denominator == 0.0 {
            continue;
        }
        let (dx, dy) = (p.x() - from.x(), p.y() - from.y());
        let t = (dx * ey - dy * ex) / denominator;
        let s = (dx * way.1 - dy * way.0) / denominator;
        if t > SAME && (0.0..=1.0).contains(&s) && nearest.is_none_or(|(kept, _)| t < kept) {
            nearest = Some((t, k));
        }
    }
    let (t, k) = nearest?;
    Some((k, Point::new(from.x() + way.0 * t, from.y() + way.1 * t).ok()?))
}

/// The point `along` millimetres along the line `points`, whose points lie `at` along it.
fn point_at(points: &[Point], at: &[f64], along: f64) -> Option<Point> {
    let k = at.iter().position(|&s| s >= along)?;
    let (&b, &sb) = (points.get(k)?, at.get(k)?);
    let Some(before) = k.checked_sub(1) else { return Some(b) };
    let (&a, &sa) = (points.get(before)?, at.get(before)?);
    let share = if sb > sa { (along - sa) / (sb - sa) } else { 0.0 };
    Point::new(a.x() + (b.x() - a.x()) * share, a.y() + (b.y() - a.y()) * share).ok()
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

/// The dot product of `a` and `b`.
fn dot(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.0 + a.1 * b.1
}

/// The cross product of `a` and `b`: positive when `b` turns left of `a` in y-up axes.
fn cross(a: (f64, f64), b: (f64, f64)) -> f64 {
    a.0 * b.1 - a.1 * b.0
}

#[cfg(test)]
mod tests;
