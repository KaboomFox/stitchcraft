//! Manual stitch: a needle point on every node of the path, for hand-placed stitches and imported stitch
//! files.
//!
//! Design: `docs/src/design/algorithms/strokes.md` › Manual stitch. Each subpath is a run:
//!
//! 1. **Nodes.** The start and every segment's end, in order. A curve gives only its end node: its
//!    control points are not stitched and it is not flattened. A closed subpath comes back to its start.
//!    A node where the needle already is sews no stitch.
//! 2. **The shortest stitch.** A node closer than the shortest stitch to the needle point before it is
//!    left out, so the stitch before it runs on to the next node; the last node is always kept, leaving
//!    out the one before it instead. `SC-W0403` says how many were left out. A part that is a single
//!    point, or lies all within the shortest stitch of its ends, is not stitched (`SC-W0401`).
//! 3. **The longest stitch.** A stitch longer than `max_stitch_length_mm`, if it is set, is split into the
//!    fewest equal parts no longer than it, but never into parts shorter than the shortest stitch: the
//!    shortest stitch wins, and `SC-W0404` says how many stitches stay longer.
//! 4. **Bean stitch** applies ([`crate::generators::passes`]); repeats do not.
//!
//! Ink/Stitch's bean stitch takes a step of its list for every point it lists, and it lists some where
//! the needle already is: a node drawn twice, a closed subpath's start where the path came back to it
//! already, and the end of each stitch it splits, twice. Its stitch plan leaves out the stitches shorter
//! than its shortest stitch only after the bean stitch, so a node left out for that takes its step too.
//! Each of these takes its step here as well, sewing nothing, so that the list falls on the same
//! stitches in both tools: the run holds the needle point once more for each, which bean stitch steps
//! over ([`passes::sew`]).

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point};

use crate::design::{Path, Subpath};
use crate::generators::passes::{self, RepeatParams};
use crate::generators::{Stitched, TooSmall, mm, too_small};

/// The manual stitch along `path`, bean-stitched as `passes` say (its repeats do not apply), with no
/// stitch shorter than `min_stitch`, and none longer than `max_stitch` where it is set (the element's
/// `max_stitch_length_mm`). Every node and every split costs work from `meter`.
pub fn manual_stitch(path: &Path, max_stitch: Option<Mm>, passes: &RepeatParams, min_stitch: Mm, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let min = min_stitch.get();
    let longest = max_stitch.map(Mm::get);
    let mut runs = Vec::with_capacity(path.subpaths.len());
    let mut warnings = Vec::new();
    let mut long = Long::default();
    for subpath in &path.subpaths {
        let nodes = nodes(subpath, meter)?;
        let Some(Spaced { kept, short }) = spaced(&nodes, min, meter)? else {
            let length: f64 = nodes.windows(2).map(|s| if let [a, b] = s { a.distance(*b) } else { 0.0 }).sum();
            let one_point = nodes.windows(2).all(|s| s.first() == s.get(1));
            warnings.push(too_small(
                match () {
                    () if one_point => TooSmall::Point,
                    () if !at_least(length, min) => TooSmall::Short(length),
                    () => TooSmall::Curled(length),
                },
                min,
            ));
            continue;
        };
        if let Some(warning) = left_out(&short, min) {
            warnings.push(warning);
        }
        let run = split(&kept, longest, min, &mut long, meter)?;
        runs.push(passes::sew(&run, 1, &passes.bean_stitch_repeats, meter)?);
    }
    warnings.extend(longest.and_then(|longest| kept_long(&long, longest, min)));
    Ok(Stitched { runs, warnings })
}

/// The subpath's nodes in order, as Ink/Stitch lists them: its start and every segment's end, a node drawn
/// twice listed twice, and the start again at the end of a closed subpath.
fn nodes(subpath: &Subpath, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut nodes = vec![subpath.start];
    let closing = subpath.closed.then_some(subpath.start);
    for end in subpath.segments.iter().map(|s| s.end()).chain(closing) {
        meter.charge(1)?;
        nodes.push(end);
    }
    Ok(nodes)
}

/// Where the needle is after each node, and the lengths of the stitches that were too short.
#[derive(Debug, PartialEq)]
struct Spaced {
    kept: Vec<Point>,
    short: Vec<f64>,
}

/// Where the needle is after each of `nodes`: at the node, or where it was, when the node is closer than
/// `min` to the needle point kept before it, or is that point. The last node is always kept, in place of
/// the ones before it closer than `min` to it. `None` when not even one stitch of `min` fits between the
/// first and the last node.
fn spaced(nodes: &[Point], min: f64, meter: &mut Meter) -> Result<Option<Spaced>, Exhausted> {
    let (Some((&first, rest)), Some(&last)) = (nodes.split_first(), nodes.last()) else { return Ok(None) };
    // The needle points sewn, and for each node between the first and the last, the place in them of the
    // point it leaves the needle at.
    let mut sewn = vec![first];
    let mut at = Vec::with_capacity(rest.len());
    let mut short = Vec::new();
    for &node in rest.iter().take(rest.len().saturating_sub(1)) {
        meter.charge(1)?;
        let length = sewn.last().map_or(0.0, |k| k.distance(node));
        if at_least(length, min) {
            sewn.push(node);
        } else if length > 0.0 {
            short.push(length);
        }
        at.push(sewn.len() - 1);
    }
    while let Some(&previous) = sewn.last().filter(|k| sewn.len() > 1 && **k != last && !at_least(k.distance(last), min)) {
        short.push(previous.distance(last));
        sewn.pop();
    }
    if !at_least(first.distance(last), min) && sewn.len() == 1 {
        return Ok(None);
    }
    // A node whose needle point was left out for the last node's sake leaves the needle at the one before.
    let needle = |i: usize| sewn.get(i).or(sewn.last()).copied().unwrap_or(first);
    let kept = std::iter::once(first).chain(at.into_iter().map(needle)).chain(std::iter::once(last)).collect();
    Ok(Some(Spaced { kept, short }))
}

/// The stitches sewn longer than the longest stitch because more parts would be shorter than the
/// shortest: how many, and the longest of them.
#[derive(Debug, Default, PartialEq)]
struct Long {
    count: usize,
    longest: f64,
}

/// Each stitch of `run` longer than `longest`, if set, split into the fewest equal parts no longer than
/// it, as long as each part is at least `min`; `long` counts the parts that stay longer for that. A stitch
/// that needs splitting takes as many steps of a bean list as Ink/Stitch lists points for it: one for each
/// part it needs, and one more for its end, which Ink/Stitch lists twice. The steps of parts too short to
/// make, and of the second end, are the needle point listed again.
fn split(run: &[Point], longest: Option<f64>, min: f64, long: &mut Long, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some(longest) = longest else { return Ok(run.to_vec()) };
    let mut out = Vec::with_capacity(run.len());
    out.extend(run.first());
    for pair in run.windows(2) {
        let [from, to] = pair else { continue };
        let length = from.distance(*to);
        // The parts it needs, then as many of them as leave none too short. Each part is paid for here,
        // so the points below need no charge; a count past `u32` is past any budget.
        let mut needed = 1_u32;
        while !at_least(longest, length / f64::from(needed)) {
            meter.charge(1)?;
            needed = needed.checked_add(1).ok_or(Exhausted::Work)?;
        }
        let mut parts = needed;
        while parts > 1 && !at_least(length / f64::from(parts), min) {
            meter.charge(1)?;
            parts -= 1;
        }
        if parts < needed {
            long.count = long.count.saturating_add(usize::try_from(parts).unwrap_or(usize::MAX));
            long.longest = long.longest.max(length / f64::from(parts));
        }
        for part in 1..parts {
            out.push(from.lerp(*to, f64::from(part) / f64::from(parts)));
        }
        out.push(*to);
        if needed > 1 {
            out.extend(std::iter::repeat_n(*to, usize::try_from(needed - parts + 1).unwrap_or(usize::MAX)));
        }
    }
    Ok(out)
}

/// `SC-W0404` for the stitches `long` counts, sewn longer than `longest` for the shortest stitch `min`, if
/// any.
fn kept_long(long: &Long, longest: f64, min: f64) -> Option<Diagnostic> {
    let message = match long.count {
        0 => return None,
        1 => format!(
            "A hand-placed stitch is sewn {} mm long, longer than `max_stitch_length_mm` ({} mm): splitting it would sew stitches shorter than the shortest stitch ({} mm).",
            mm(long.longest),
            mm(longest),
            mm(min)
        ),
        n => format!(
            "{n} hand-placed stitches are sewn longer than `max_stitch_length_mm` ({} mm), up to {} mm: splitting them would sew stitches shorter than the shortest stitch ({} mm).",
            mm(longest),
            mm(long.longest),
            mm(min)
        ),
    };
    let hint = format!("Set `max_stitch_length_mm` to at least twice the shortest stitch, {} mm.", mm(2.0 * min));
    Some(Diagnostic::new(Code::HandStitchKeptLong, message).with_fix(Fix::Hint(hint)))
}

/// `SC-W0403` for the hand-placed stitches that were too short, if any.
fn left_out(short: &[f64], min: f64) -> Option<Diagnostic> {
    let shortest = short.iter().copied().reduce(f64::min)?;
    let message = match short.len() {
        1 => format!(
            "A hand-placed stitch is {} mm long, shorter than the shortest stitch ({} mm), so a needle point is left out.",
            mm(shortest),
            mm(min)
        ),
        n => format!(
            "{n} hand-placed stitches are shorter than the shortest stitch ({} mm), the shortest {} mm long, so a needle point of each is left out.",
            mm(min),
            mm(shortest)
        ),
    };
    Some(Diagnostic::new(Code::HandStitchTooShort, message))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::design::Segment;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn xs(points: &[Point]) -> Vec<f64> {
        points.iter().map(|q| q.x()).collect()
    }

    #[test]
    fn nodes_are_segment_ends_as_ink_stitch_lists_them() {
        let mut meter = Budget::DEFAULT.meter();
        let curve = Subpath {
            start: p(0.0, 0.0),
            segments: vec![Segment::Line(p(1.0, 0.0)), Segment::Line(p(1.0, 0.0)), Segment::Cubic(p(5.0, 5.0), p(6.0, -5.0), p(2.0, 0.0))],
            closed: true,
        };
        assert_eq!(nodes(&curve, &mut meter).unwrap(), [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(0.0, 0.0)]);
    }

    #[test]
    fn points_too_close_to_the_one_before_are_left_out_where_the_needle_is() {
        let mut meter = Budget::DEFAULT.meter();
        let line = |xs_: &[f64]| xs_.iter().map(|x| p(*x, 0.0)).collect::<Vec<_>>();
        let Spaced { kept, short } = spaced(&line(&[0.0, 0.1, 5.0]), 0.3, &mut meter).unwrap().unwrap();
        assert_eq!((xs(&kept), short), (vec![0.0, 0.0, 5.0], vec![0.1]));
        // The last point stays; the one before it goes, and leaves the needle where it was.
        let Spaced { kept, short } = spaced(&line(&[0.0, 5.0, 5.2]), 0.3, &mut meter).unwrap().unwrap();
        assert_eq!(xs(&kept), [0.0, 0.0, 5.2]);
        assert!((short[0] - 0.2).abs() < 1e-12);
        // A node drawn twice is no stitch too short: the needle is there already. So is a last node that is
        // the one before it.
        let Spaced { kept, short } = spaced(&line(&[0.0, 5.0, 5.0, 9.0, 9.0]), 0.3, &mut meter).unwrap().unwrap();
        assert_eq!((xs(&kept), short), (vec![0.0, 5.0, 5.0, 9.0, 9.0], vec![]));
        // Nothing fits.
        assert_eq!(spaced(&line(&[0.0, 0.2]), 0.3, &mut meter).unwrap(), None);
        assert_eq!(spaced(&line(&[0.0, 0.0]), 0.3, &mut meter).unwrap(), None);
        assert_eq!(spaced(&line(&[0.0]), 0.3, &mut meter).unwrap(), None);
    }

    #[test]
    fn long_stitches_split_evenly_but_never_below_the_shortest_stitch() {
        let mut meter = Budget::DEFAULT.meter();
        let run = [p(0.0, 0.0), p(10.0, 0.0), p(11.0, 0.0)];
        // Split, the end comes twice, as Ink/Stitch lists it.
        assert_eq!(xs(&split(&run, Some(3.0), 0.3, &mut Long::default(), &mut meter).unwrap()), [0.0, 2.5, 5.0, 7.5, 10.0, 10.0, 11.0]);
        assert_eq!(xs(&split(&run, Some(10.0), 0.3, &mut Long::default(), &mut meter).unwrap()), [0.0, 10.0, 11.0], "exactly the longest");
        assert_eq!(xs(&split(&run, None, 0.3, &mut Long::default(), &mut meter).unwrap()), [0.0, 10.0, 11.0]);
        // A longest stitch below the shortest: as many parts as keep each at least the shortest, 3 of the 5
        // it needs, and the needle point listed again for the 2 not made and for its end.
        let short = [p(0.0, 0.0), p(1.0, 0.0)];
        let parts = xs(&split(&short, Some(0.2), 0.3, &mut Long::default(), &mut meter).unwrap());
        assert_eq!(parts.len(), 4 + 3, "{parts:?}");
        assert!(parts[3..].iter().all(|x| (x - 1.0).abs() < 1e-12), "{parts:?}");
        // A stitch longer than the longest and shorter than the shortest is not split: its end comes once for
        // each of the 2 parts it needs and once more, 3 steps, as Ink/Stitch lists 3 points.
        assert_eq!(xs(&split(&[p(0.0, 0.0), p(0.25, 0.0)], Some(0.2), 0.3, &mut Long::default(), &mut meter).unwrap()), [0.0, 0.25, 0.25, 0.25]);
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let mut meter = Budget { max_stitches: 1, max_work: 3 }.meter();
        assert_eq!(split(&[p(0.0, 0.0), p(100.0, 0.0)], Some(1.0), 0.3, &mut Long::default(), &mut meter), Err(Exhausted::Work));
    }
}
