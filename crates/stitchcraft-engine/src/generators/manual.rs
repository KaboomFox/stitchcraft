//! Manual stitch: a needle point on every node of the path, for hand-placed stitches and imported stitch
//! files.
//!
//! Design: `docs/src/design/algorithms/strokes.md` › Manual stitch. Each subpath is a run:
//!
//! 1. **Nodes.** The start and every segment's end, in order. A curve gives only its end node: its
//!    control points are not stitched and it is not flattened. A node where the needle already is counts
//!    once, and a closed subpath comes back to its start.
//! 2. **The shortest stitch.** A node closer than the shortest stitch to the needle point before it is
//!    left out, so the stitch before it runs on to the next node; the last node is always kept, leaving
//!    out the one before it instead. `SC-W0403` says how many were left out. A part that is a single
//!    point, or lies all within the shortest stitch of its ends, is not stitched (`SC-W0401`).
//! 3. **The longest stitch.** A stitch longer than `max_stitch_length_mm`, if it is set, is split into the
//!    fewest equal parts no longer than it, but never into parts shorter than the shortest stitch: the
//!    shortest stitch wins.
//! 4. **Bean stitch** applies ([`crate::generators::passes`]); repeats do not.

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Mm, Point};

use crate::design::{Path, Subpath};
use crate::generators::passes::{self, RepeatParams};
use crate::generators::{Stitched, TooSmall, mm, too_small};

/// The manual stitch along `path`, bean-stitched as `passes` say (its repeats do not apply), with no
/// stitch shorter than `min_stitch`, and none longer than `max_stitch` where it is set (the element's
/// `max_stitch_length_mm`). Every node and every split costs work from `meter`.
pub fn manual_stitch(path: &Path, max_stitch: Option<Mm>, passes: &RepeatParams, min_stitch: Mm, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let min = min_stitch.get();
    let mut runs = Vec::with_capacity(path.subpaths.len());
    let mut warnings = Vec::new();
    for subpath in &path.subpaths {
        let nodes = nodes(subpath, meter)?;
        let Some(Spaced { kept, short }) = spaced(&nodes, min, meter)? else {
            let length: f64 = nodes.windows(2).map(|s| if let [a, b] = s { a.distance(*b) } else { 0.0 }).sum();
            warnings.push(too_small(
                match nodes.len() {
                    0 | 1 => TooSmall::Point,
                    _ if !at_least(length, min) => TooSmall::Short(length),
                    _ => TooSmall::Curled(length),
                },
                min,
            ));
            continue;
        };
        if let Some(warning) = left_out(&short, min) {
            warnings.push(warning);
        }
        let run = split(&kept, max_stitch.map(Mm::get), min, meter)?;
        runs.push(passes::sew(&run, 1, &passes.bean_stitch_repeats, meter)?);
    }
    Ok(Stitched { runs, warnings })
}

/// The subpath's nodes in order: its start and every segment's end, a node where the needle already is
/// once, and the start again at the end of a closed subpath.
fn nodes(subpath: &Subpath, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut nodes = vec![subpath.start];
    let closing = subpath.closed.then_some(subpath.start);
    for end in subpath.segments.iter().map(|s| s.end()).chain(closing) {
        meter.charge(1)?;
        if nodes.last() != Some(&end) {
            nodes.push(end);
        }
    }
    Ok(nodes)
}

/// The nodes kept, and the lengths of the stitches that were too short.
#[derive(Debug, PartialEq)]
struct Spaced {
    kept: Vec<Point>,
    short: Vec<f64>,
}

/// `nodes` without the ones closer than `min` to the needle point kept before them, the last node kept in
/// place of the one before it; `None` when not even one stitch of `min` fits between the first and the
/// last node.
fn spaced(nodes: &[Point], min: f64, meter: &mut Meter) -> Result<Option<Spaced>, Exhausted> {
    let (Some((&first, rest)), Some(&last)) = (nodes.split_first(), nodes.last()) else { return Ok(None) };
    let mut kept = vec![first];
    let mut short = Vec::new();
    for &node in rest.iter().take(rest.len().saturating_sub(1)) {
        meter.charge(1)?;
        let length = kept.last().map_or(0.0, |k| k.distance(node));
        if at_least(length, min) {
            kept.push(node);
        } else {
            short.push(length);
        }
    }
    while let Some(&previous) = kept.last().filter(|k| kept.len() > 1 && !at_least(k.distance(last), min)) {
        short.push(previous.distance(last));
        kept.pop();
    }
    if !at_least(first.distance(last), min) && kept.len() == 1 {
        return Ok(None);
    }
    kept.push(last);
    Ok(Some(Spaced { kept, short }))
}

/// Each stitch of `run` longer than `longest`, if set, split into the fewest equal parts no longer than
/// it, as long as each part is at least `min`.
fn split(run: &[Point], longest: Option<f64>, min: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some(longest) = longest else { return Ok(run.to_vec()) };
    let mut out = Vec::with_capacity(run.len());
    out.extend(run.first());
    for pair in run.windows(2) {
        let [from, to] = pair else { continue };
        let length = from.distance(*to);
        // One part more while the parts are too long and one more would leave none too short. Each part
        // is paid for here, so the points below need no charge; a count past `u32` is past any budget.
        let mut parts = 1_u32;
        while !at_least(longest, length / f64::from(parts)) && at_least(length / (f64::from(parts) + 1.0), min) {
            meter.charge(1)?;
            parts = parts.checked_add(1).ok_or(Exhausted::Work)?;
        }
        for part in 1..parts {
            out.push(from.lerp(*to, f64::from(part) / f64::from(parts)));
        }
        out.push(*to);
    }
    Ok(out)
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
    fn nodes_are_segment_ends_once_each() {
        let mut meter = Budget::DEFAULT.meter();
        let curve = Subpath {
            start: p(0.0, 0.0),
            segments: vec![Segment::Line(p(1.0, 0.0)), Segment::Line(p(1.0, 0.0)), Segment::Cubic(p(5.0, 5.0), p(6.0, -5.0), p(2.0, 0.0))],
            closed: true,
        };
        assert_eq!(nodes(&curve, &mut meter).unwrap(), [p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0), p(0.0, 0.0)]);
    }

    #[test]
    fn points_too_close_to_the_one_before_are_left_out() {
        let mut meter = Budget::DEFAULT.meter();
        let line = |xs_: &[f64]| xs_.iter().map(|x| p(*x, 0.0)).collect::<Vec<_>>();
        let Spaced { kept, short } = spaced(&line(&[0.0, 0.1, 5.0]), 0.3, &mut meter).unwrap().unwrap();
        assert_eq!((xs(&kept), short), (vec![0.0, 5.0], vec![0.1]));
        // The last point stays; the one before it goes.
        let Spaced { kept, short } = spaced(&line(&[0.0, 5.0, 5.2]), 0.3, &mut meter).unwrap().unwrap();
        assert_eq!(xs(&kept), [0.0, 5.2]);
        assert!((short[0] - 0.2).abs() < 1e-12);
        // Nothing fits.
        assert_eq!(spaced(&line(&[0.0, 0.2]), 0.3, &mut meter).unwrap(), None);
        assert_eq!(spaced(&line(&[0.0]), 0.3, &mut meter).unwrap(), None);
    }

    #[test]
    fn long_stitches_split_evenly_but_never_below_the_shortest_stitch() {
        let mut meter = Budget::DEFAULT.meter();
        let run = [p(0.0, 0.0), p(10.0, 0.0), p(11.0, 0.0)];
        assert_eq!(xs(&split(&run, Some(3.0), 0.3, &mut meter).unwrap()), [0.0, 2.5, 5.0, 7.5, 10.0, 11.0]);
        assert_eq!(xs(&split(&run, Some(10.0), 0.3, &mut meter).unwrap()), [0.0, 10.0, 11.0], "exactly the longest");
        assert_eq!(xs(&split(&run, None, 0.3, &mut meter).unwrap()), [0.0, 10.0, 11.0]);
        // A longest stitch below the shortest: as many parts as keep each at least the shortest.
        let short = [p(0.0, 0.0), p(1.0, 0.0)];
        let parts = xs(&split(&short, Some(0.2), 0.3, &mut meter).unwrap());
        assert_eq!(parts.len(), 4, "{parts:?}");
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let mut meter = Budget { max_stitches: 1, max_work: 3 }.meter();
        assert_eq!(split(&[p(0.0, 0.0), p(100.0, 0.0)], Some(1.0), 0.3, &mut meter), Err(Exhausted::Work));
    }
}
