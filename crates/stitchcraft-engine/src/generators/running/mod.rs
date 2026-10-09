//! Running stitch: single stitches along a path, for outlines, details, travel and underlay.
//!
//! Design: `docs/src/design/algorithms/strokes.md` › Running stitch. A stroke is stitched in five steps:
//!
//! 1. **Flatten** the path ([`crate::normalize::stroke`]) to within a tenth of the curve tolerance, and
//!    mark its corners. The other nine tenths are left for the stitches.
//! 2. **Cut at the corners.** Every corner gets a needle penetration, so sharp shapes stay sharp. The
//!    exception is a corner closer than the shortest stitch to the previous penetration or to the end,
//!    measured along the path: the spans on either side of it are joined.
//! 3. **Spread the stitches evenly.** Each span gets as many stitches of the pattern as it takes to
//!    reach its length, all shortened by one factor so that the last one ends exactly on the corner: no
//!    stitch is longer than its pattern length, and none is a short leftover. Pattern lengths are raised
//!    to at least twice the shortest stitch (`SC-W0402`). A span longer than the longest pattern length
//!    then shortens its stitches by less than half, which keeps every one at or above the shortest
//!    stitch; in a shorter span, a stitch shortened below it joins its shorter neighbour, and the joined
//!    stitch is no longer than the span.
//! 4. **Measure straight.** Steps 2 and 3 measure along the path, but a stitch is the straight line
//!    between two needle points, and where the path bends back on itself within less than the shortest
//!    stitch (a cusp, a tight loop) two points far apart along it can be close together. So the needle
//!    points that would make a stitch shorter than the shortest stitch are dropped, and points are added
//!    along the path where a drop would make one longer than the longest pattern length.
//! 5. **Follow the curve.** A stitch whose straight line strays from the path by more than the remaining
//!    nine tenths of the tolerance is split at a point of the path, the farthest one from it, as often as
//!    needed, unless the split would leave a stitch outside those lengths.
//!
//! When the rules disagree, the shortest stitch wins (a shorter stitch hammers one spot and can break
//! the thread), then corners, then the tolerance. A piece of the path that is a single point, shorter
//! than the shortest stitch, or all within it of its ends, is not stitched (`SC-W0401`). Only arithmetic and square
//! roots are used: every platform places the same stitches.

mod params;

pub use params::RunningParams;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Mm, Point};

use crate::design::Path;
use crate::normalize::stroke::{self, Piece, distance_to_segment};

/// The share of the curve tolerance that flattening may use; the stitches have the rest.
const FLATTEN_SHARE: f64 = 0.1;

/// The stitches of one running-stitch stroke.
#[derive(Clone, Debug, PartialEq)]
pub struct Stitched {
    /// The needle points of each piece of the stroke that is stitched, in drawing order. Each run starts
    /// where its piece starts and ends where it ends.
    pub runs: Vec<Vec<Point>>,
    /// What was changed or left out (`SC-W0402`, `SC-W0401`). They name no element: the caller adds it.
    pub warnings: Vec<Diagnostic>,
}

/// The running stitch along `path`, with no stitch shorter than `min_stitch` (the element's shortest
/// stitch, or the machine's). Every point of the flattened path costs work from `meter`.
pub fn running_stitch(path: &Path, params: &RunningParams, min_stitch: Mm, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let min = min_stitch.get();
    let mut warnings = Vec::new();
    let pattern = pattern(&params.running_stitch_length_mm, min, &mut warnings);
    let tolerance = params.running_stitch_tolerance_mm.get();
    let stroke = stroke::flatten(path, tolerance * FLATTEN_SHARE, meter)?;
    let mut runs = Vec::with_capacity(stroke.pieces.len());
    for piece in &stroke.pieces {
        let along = Along::new(piece, meter)?;
        let length = along.length();
        let skipped = if piece.points.len() < 2 {
            "A part of the stroke is a single point, so it is not stitched.".to_string()
        } else if !at_least(length, min) {
            format!("A part of the stroke is {} mm long, shorter than the shortest stitch ({} mm), so it is not stitched.", mm(length), mm(min))
        } else if let Some(run) = stitch_piece(&along, &piece.corners, &pattern, min, tolerance * (1.0 - FLATTEN_SHARE), meter)? {
            runs.push(run);
            continue;
        } else {
            format!(
                "A part of the stroke is {} mm long, but all of it lies within the shortest stitch ({} mm) of its ends, so it is not stitched.",
                mm(length),
                mm(min)
            )
        };
        warnings.push(Diagnostic::new(Code::StrokeTooSmall, skipped));
    }
    Ok(Stitched { runs, warnings })
}

/// The pattern of stitch lengths, each raised to at least twice the shortest stitch, with `SC-W0402` for
/// the ones that were.
fn pattern(lengths: &[Mm], min: f64, warnings: &mut Vec<Diagnostic>) -> Vec<f64> {
    let floor = 2.0 * min;
    let short: Vec<String> = lengths.iter().map(|l| l.get()).filter(|l| !at_least(*l, floor)).map(mm).collect();
    let pattern: Vec<f64> = lengths.iter().map(|l| l.get().max(floor)).collect();
    let message = match short.as_slice() {
        [] if !pattern.is_empty() => None,
        // The registry never gives an empty list; a direct caller that does gets the shortest safe length.
        [] => Some(format!("No stitch length is given, so {} mm, twice the shortest stitch, is used.", mm(floor))),
        [one] => Some(format!("The stitch length {one} mm is shorter than twice the shortest stitch ({} mm), so {} mm is used.", mm(min), mm(floor))),
        many => Some(format!(
            "The stitch lengths {} mm are shorter than twice the shortest stitch ({} mm), so {} mm is used for each.",
            many.join(", "),
            mm(min),
            mm(floor)
        )),
    };
    if let Some(message) = message {
        warnings.push(Diagnostic::new(Code::StitchLengthRaised, message));
    }
    if pattern.is_empty() { vec![floor] } else { pattern }
}

/// A needle penetration: how far along the piece it is, where it is, and whether it is on a corner.
#[derive(Clone, Copy, Debug)]
struct Needle {
    at: f64,
    point: Point,
    corner: bool,
}

/// Distances along a piece: where each of its points lies, measured along it from its start.
struct Along<'a> {
    points: &'a [Point],
    at: Vec<f64>,
}

impl<'a> Along<'a> {
    fn new(piece: &'a Piece, meter: &mut Meter) -> Result<Along<'a>, Exhausted> {
        let mut at = Vec::with_capacity(piece.points.len());
        let mut total = 0.0;
        let mut previous = None;
        for point in &piece.points {
            meter.charge(1)?;
            total += previous.map_or(0.0, |p: Point| p.distance(*point));
            at.push(total);
            previous = Some(*point);
        }
        Ok(Along { points: &piece.points, at })
    }

    fn length(&self) -> f64 {
        self.at.last().copied().unwrap_or(0.0)
    }

    /// The needle on the piece's point `index`, exactly.
    fn vertex(&self, index: usize) -> Needle {
        let at = self.at.get(index).copied().unwrap_or(0.0);
        Needle { at, point: self.points.get(index).copied().unwrap_or(Point::ORIGIN), corner: false }
    }

    /// The needle `at` along the piece.
    fn needle(&self, at: f64) -> Needle {
        // The side the distance falls on: from the last point at or before it to the next one. A piece
        // repeats no point, so every side has a length; were one empty, the NaN fraction would put the
        // needle on the side's first point.
        let end = self.at.partition_point(|a| *a <= at).clamp(1, self.points.len().saturating_sub(1).max(1));
        let (from, to) = (self.vertex(end - 1), self.vertex(end));
        Needle { at, point: from.point.lerp(to.point, (at - from.at) / (to.at - from.at)), corner: false }
    }

    /// The indices of the piece's points between two needles: after `from` and before `to`, a point
    /// within the length slack of either counting as on it.
    fn between(&self, from: &Needle, to: &Needle) -> std::ops::Range<usize> {
        self.at.partition_point(|a| at_least(from.at, *a))..self.at.partition_point(|a| !at_least(*a, to.at))
    }

    /// The first point of the piece after `from`, and no later than `to`, that is `radius` from it in a
    /// straight line.
    fn crossing(&self, from: &Needle, radius: f64, to: &Needle, meter: &mut Meter) -> Result<Option<Needle>, Exhausted> {
        let mut p = (from.point, from.at);
        let points = self.between(from, to).filter_map(|i| Some((*self.points.get(i)?, *self.at.get(i)?)));
        for q in points.chain(std::iter::once((to.point, to.at))) {
            meter.charge(1)?;
            if let Some(t) = leaves(from.point, radius, p.0, q.0) {
                return Ok(Some(Needle { at: p.1 + t * (q.1 - p.1), point: p.0.lerp(q.0, t), corner: false }));
            }
            p = q;
        }
        Ok(None)
    }

    /// The point of the piece farthest from both `start` and `end` (the first along the piece, on a tie),
    /// with its distance to the nearer of them. Along a side, the distance to a point is greatest at one of
    /// the side's ends, so the candidates are the piece's points and, where a side crosses the line of
    /// points as far from `start` as from `end`, the crossing (where the nearer of the two changes).
    fn farthest(&self, start: Point, end: Point, meter: &mut Meter) -> Result<Option<(f64, Needle)>, Exhausted> {
        let (ux, uy) = (end.x() - start.x(), end.y() - start.y());
        let middle = start.lerp(end, 0.5);
        let mut best: Option<(f64, Needle)> = None;
        let mut previous: Option<Needle> = None;
        for q in (0..self.points.len()).map(|index| self.vertex(index)) {
            meter.charge(1)?;
            // Where the side from the previous point crosses the line, if it does. A side parallel to
            // the line (or no line: `start` is `end`) gives an infinite or NaN `t`.
            let crossing = previous.and_then(|p| {
                let across = (q.point.x() - p.point.x()) * ux + (q.point.y() - p.point.y()) * uy;
                let t = ((middle.x() - p.point.x()) * ux + (middle.y() - p.point.y()) * uy) / across;
                (0.0..=1.0).contains(&t).then(|| Needle { at: p.at + t * (q.at - p.at), point: p.point.lerp(q.point, t), corner: false })
            });
            for candidate in crossing.into_iter().chain(std::iter::once(q)) {
                let distance = candidate.point.distance(start).min(candidate.point.distance(end));
                if best.is_none_or(|(b, _)| distance > b) {
                    best = Some((distance, candidate));
                }
            }
            previous = Some(q);
        }
        Ok(best)
    }
}

/// How far along the segment from `p` to `q` it leaves the circle of `radius` round `centre`, as a
/// fraction of the way, for `p` inside the circle; `None` when `q` is inside too.
fn leaves(centre: Point, radius: f64, p: Point, q: Point) -> Option<f64> {
    if q.distance(centre) < radius {
        return None;
    }
    // |p − centre + t (q − p)| = radius: a t² + 2 b t + c = 0 with c ≤ 0, so the larger root is where it
    // leaves. Rounding moves that point by at most about the radius times the float precision.
    let (dx, dy) = (q.x() - p.x(), q.y() - p.y());
    let (ex, ey) = (p.x() - centre.x(), p.y() - centre.y());
    let (a, b, c) = (dx * dx + dy * dy, ex * dx + ey * dy, ex * ex + ey * ey - radius * radius);
    if a == 0.0 {
        // `q` is `p`, so `p` is on the circle.
        return Some(0.0);
    }
    Some((((b * b - a * c).max(0.0).sqrt() - b) / a).clamp(0.0, 1.0))
}

/// The needle points of one piece at least `min` long, or `None` if it lies all within `min` of its ends.
fn stitch_piece(
    along: &Along,
    corners: &[usize],
    pattern: &[f64],
    min: f64,
    budget: f64,
    meter: &mut Meter,
) -> Result<Option<Vec<Point>>, Exhausted> {
    let total = along.length();
    let (start, end) = (along.vertex(0), along.vertex(along.points.len().saturating_sub(1)));
    // The penetrations that bound the spans: the start, the corners far enough along from the previous
    // one and from the end, and the end.
    let mut cuts = vec![start];
    for corner in corners {
        meter.charge(1)?;
        let cut = Needle { corner: true, ..along.vertex(*corner) };
        if cuts.last().is_some_and(|previous| at_least(cut.at - previous.at, min)) && at_least(total - cut.at, min) {
            cuts.push(cut);
        }
    }
    cuts.push(end);
    // Each span's stitches, with the pattern carrying on from span to span.
    let mut needles = vec![start];
    let mut next = 0;
    for span in cuts.windows(2) {
        let [from, to] = span else { continue };
        let lengths = fit(to.at - from.at, pattern, &mut next, min, meter)?;
        let mut at = from.at;
        for length in lengths.iter().take(lengths.len().saturating_sub(1)) {
            at += length;
            needles.push(along.needle(at));
        }
        needles.push(*to);
    }
    let longest = pattern.iter().copied().fold(2.0 * min, f64::max);
    let Some(needles) = space(needles, along, min, longest, meter)? else { return Ok(None) };
    let needles = follow(needles, along, budget, min, longest, meter)?;
    Ok(Some(needles.into_iter().map(|needle| needle.point).collect()))
}

/// The stitch lengths for a span `length` long: the pattern's next lengths, as many as it takes to reach
/// `length`, all shortened by one factor to end exactly there; then each one shorter than `min` joined to
/// its shorter neighbour.
fn fit(length: f64, pattern: &[f64], next: &mut usize, min: f64, meter: &mut Meter) -> Result<Vec<f64>, Exhausted> {
    if pattern.is_empty() {
        return Ok(vec![length]);
    }
    let mut lengths = Vec::new();
    let mut sum = 0.0;
    while lengths.is_empty() || !at_least(sum, length) {
        meter.charge(1)?;
        let stitch = pattern.get(*next % pattern.len()).copied().unwrap_or(length);
        lengths.push(stitch);
        sum += stitch;
        *next += 1;
    }
    let scale = length / sum;
    for stitch in &mut lengths {
        *stitch *= scale;
    }
    while let Some((short, neighbour)) = joinable(&lengths, min) {
        meter.charge(1)?;
        let (keep, gone) = (short.min(neighbour), short.max(neighbour));
        let joined = lengths.get(keep).copied().unwrap_or(0.0) + lengths.get(gone).copied().unwrap_or(0.0);
        if let Some(slot) = lengths.get_mut(keep) {
            *slot = joined;
        }
        lengths.remove(gone);
    }
    Ok(lengths)
}

/// The shortest of `lengths`, if it is shorter than `min` and has a neighbour, with the neighbour it
/// joins: the shorter one, the earlier on a tie.
fn joinable(lengths: &[f64], min: f64) -> Option<(usize, usize)> {
    let (short, length) = lengths.iter().copied().enumerate().min_by(|a, b| a.1.total_cmp(&b.1))?;
    if at_least(length, min) {
        return None;
    }
    let before = short.checked_sub(1).and_then(|i| lengths.get(i).map(|l| (i, *l)));
    let after = lengths.get(short + 1).map(|l| (short + 1, *l));
    let (neighbour, _) = before.into_iter().chain(after).min_by(|a, b| a.1.total_cmp(&b.1))?;
    Some((short, neighbour))
}

/// `needles` with every stitch, measured straight, from `min` to `longest` long, or `None` when the
/// piece lies all within `min` of its ends.
///
/// A stitch is never longer straight than along the path, so spacing along it already keeps every stitch
/// within `longest`; only stitches that are too short need work. Going along the needles:
///
/// - a needle closer than `min` to the last one kept is dropped;
/// - a corner drops the needles kept since the previous corner instead, as many as it takes; if it is
///   still too close, it is dropped itself (the shortest stitch comes before corners);
/// - the end is always kept, and drops whatever it takes, corners too. When only the start is left and
///   the end is too close to it (a closed piece, or one that comes back to where it started), the stitch
///   goes by way of the point farthest from both ends, if that is far enough; if not, the piece lies all
///   within `min` of its ends.
///
/// A drop makes the stitch over the dropped needle longer; [`reach`] splits it if it is longer than
/// `longest`.
fn space(needles: Vec<Needle>, along: &Along, min: f64, longest: f64, meter: &mut Meter) -> Result<Option<Vec<Needle>>, Exhausted> {
    let too_close = |a: &Needle, b: &Needle| !at_least(a.point.distance(b.point), min);
    let mut needles = needles.into_iter();
    let (Some(start), Some(end)) = (needles.next(), needles.next_back()) else { return Ok(None) };
    let mut kept = vec![start];
    for needle in needles {
        meter.charge(1)?;
        while needle.corner && kept.len() > 1 && kept.last().is_some_and(|last| !last.corner && too_close(last, &needle)) {
            kept.pop();
        }
        if kept.last().is_some_and(|last| !too_close(last, &needle)) {
            reach(&mut kept, needle, along, longest, meter)?;
        }
    }
    while kept.len() > 1 && kept.last().is_some_and(|last| too_close(last, &end)) {
        meter.charge(1)?;
        kept.pop();
    }
    if kept.len() == 1 && too_close(&start, &end) {
        match along.farthest(start.point, end.point, meter)? {
            Some((distance, far)) if at_least(distance, min) => reach(&mut kept, far, along, longest, meter)?,
            _ => return Ok(None),
        }
    }
    reach(&mut kept, end, along, longest, meter)?;
    Ok(Some(kept))
}

/// Appends `to` to `kept`, first splitting the stitch to it while that is longer than `longest`: at the
/// first needle along the piece half the stitch's straight length from its start, or `longest` if that
/// is less. Both parts are then longer than half of `longest`, and `longest` is at least twice the
/// shortest stitch.
fn reach(kept: &mut Vec<Needle>, to: Needle, along: &Along, longest: f64, meter: &mut Meter) -> Result<(), Exhausted> {
    while let Some(from) = kept.last().copied() {
        meter.charge(1)?;
        let gap = from.point.distance(to.point);
        if at_least(longest, gap) {
            break;
        }
        // Always found: `to` itself is farther than that from `from`.
        let Some(step) = along.crossing(&from, (gap / 2.0).min(longest), &to, meter)? else { break };
        kept.push(step);
    }
    kept.push(to);
    Ok(())
}

/// `needles` with more added wherever a stitch strays from the piece by more than `budget`: each split
/// is at the piece's point farthest from the stitch (the first, on a tie) among those that leave both
/// halves from `min` to `longest` long, measured straight.
fn follow(needles: Vec<Needle>, along: &Along, budget: f64, min: f64, longest: f64, meter: &mut Meter) -> Result<Vec<Needle>, Exhausted> {
    let fits = |a: Point, b: Point| {
        let length = a.distance(b);
        at_least(length, min) && at_least(longest, length)
    };
    let mut done: Vec<Needle> = Vec::with_capacity(needles.len());
    let mut pending: Vec<Needle> = Vec::new();
    for needle in needles {
        let Some(&from) = done.last() else {
            done.push(needle);
            continue;
        };
        let mut from = from;
        pending.push(needle);
        while let Some(&to) = pending.last() {
            let mut worst: Option<(f64, usize)> = None;
            for index in along.between(&from, &to) {
                meter.charge(1)?;
                let Some(&point) = along.points.get(index) else { continue };
                let off = distance_to_segment(point, from.point, to.point);
                if worst.is_none_or(|(w, _)| off > w) && fits(from.point, point) && fits(point, to.point) {
                    worst = Some((off, index));
                }
            }
            match worst {
                Some((off, index)) if off > budget => pending.push(along.vertex(index)),
                _ => {
                    done.push(to);
                    pending.pop();
                    from = to;
                }
            }
        }
    }
    Ok(done)
}

/// `value` millimetres for a message: at most two decimals, without trailing zeros.
fn mm(value: f64) -> String {
    let text = format!("{value:.2}");
    text.trim_end_matches('0').trim_end_matches('.').to_string()
}

#[cfg(test)]
mod tests;
