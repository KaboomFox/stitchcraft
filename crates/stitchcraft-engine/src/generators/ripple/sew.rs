//! Sewing a ripple stitch's lines (`docs/src/design/algorithms/strokes.md` › Ripple stitch › Sewing the
//! lines), as Ink/Stitch sews them: one at a time, back and forth, joined flat or at a point and staggered,
//! or all as one run.
//!
//! **One at a time.** Each line is a running stitch of its own ([`running::along_pattern`]), its random
//! lengths drawn from a stream that its place among the lines sewn keys. Every other line is sewn from its
//! end. With the flat join a straight running stitch joins each line's last needle point to the next line's
//! first, and with the point join the lines meet already. A line that starts where the one before ended
//! lists that needle point again, as Ink/Stitch lists it, so it takes its step of the bean list.
//!
//! **Staggers** move where a line's needle points fall. With any value but 0, line `i` is lengthened back
//! along its first segment by `((i / staggers) mod 1)` of the stitch lengths' sum, its phase, as Ink/Stitch
//! lengthens it, and sewn from there. The needle points on the stretch added are left out, the line's start
//! is its first needle point, and needle points nearer the start than the shortest stitch are left out too.
//! A line of no length, as a ripple's last line at its target point is, sews its one point, but none at a
//! phase of 0. Ink/Stitch at `d59c9ab`, run as a black box, sews it so: lengthening it by a phase above 0
//! moves its first point a ten-millionth of a millimetre, and it sews that stitch, which its plan drops.
//!
//! **One run.** A spiral's lines, and the others without `flip_copies`, are one running stitch through all
//! their points in order.

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::{MM_PER_SVG_PX, at_least};
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::running;

/// A line's first segment shorter than this, in millimetres, is lengthened back in proportion less than the
/// stagger: 0.1 CSS pixels, as Ink/Stitch lengthens it.
const SHORT_SEGMENT: f64 = 0.1 * MM_PER_SVG_PX;

/// A needle point this near a staggered line's added stretch, in millimetres, lies on it.
const ON_STRETCH: f64 = 1e-9;

/// How a ripple's lines are sewn.
pub(super) struct Sewing<'a> {
    /// The stitch lengths, taken in turn along each line, each at least twice the shortest stitch.
    pub(super) pattern: &'a [f64],
    /// How far a stitch may stray from its line, in millimetres.
    pub(super) tolerance: f64,
    /// The shortest stitch, in millimetres.
    pub(super) min: f64,
    /// How much each stitch length may vary at random, as a share of it, when it does.
    pub(super) jitter: Option<f64>,
    /// The element's generator, which keys a stream for each line's random lengths.
    pub(super) rng: &'a SplitMix64,
    /// How many lines the needle points take to come back to where they started; 0 for none.
    pub(super) staggers: f64,
    /// Whether the lines meet at a point (`join_style` 1), where the flat join runs a stitch between them.
    pub(super) point: bool,
    /// How many lines `skip_start` left out, which the point join counts every other line from.
    pub(super) skip_start: usize,
}

impl Sewing<'_> {
    /// The needle points of the running stitch along `points`, its random lengths, when they vary, drawn from
    /// the stream `line` keys.
    fn along(&self, points: &[Point], line: usize, jitter: Option<f64>, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        running::along_pattern(points, (self.pattern, self.tolerance, self.min), jitter, (self.rng, line), meter)
    }
}

/// `lines` sewn one at a time, as the module says: their needle points, from the first line's first.
pub(super) fn one_at_a_time(lines: &[Vec<Point>], sewing: &Sewing, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut run: Vec<Point> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        // A line of 1 point, from a column of 1 pair, is left out.
        let [first, .., last] = line.as_slice() else { continue };
        let back = if sewing.point { (i + sewing.skip_start) % 2 == 1 } else { i % 2 == 1 };
        let stitched = if sewing.staggers == 0.0 {
            let ordered: Vec<Point> = if back { line.iter().rev().copied().collect() } else { line.clone() };
            if line.iter().all(|point| point == first) { vec![*first] } else { sewing.along(&ordered, i, sewing.jitter, meter)? }
        } else {
            let mut placed = staggered(line, i, sewing, meter)?;
            if back {
                placed.reverse();
            }
            placed
        };
        if stitched.is_empty() {
            continue;
        }
        if let (false, Some(&from)) = (sewing.point, run.last()) {
            let join = sewing.along(&[from, if back { *last } else { *first }], 0, None, meter)?;
            run.extend(join.iter().skip(1).take(join.len().saturating_sub(2)));
        }
        run.extend(stitched);
    }
    Ok(run)
}

/// `lines` sewn as one run: a running stitch through all their points in order.
pub(super) fn one_run(lines: &[Vec<Point>], sewing: &Sewing, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    sewing.along(&lines.concat(), 0, sewing.jitter, meter)
}

/// The needle points of `line`, the `i`-th sewn, from its start, staggered as the module says. A line of no
/// length sews its one point, but none at a phase of 0.
fn staggered(line: &[Point], i: usize, sewing: &Sewing, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let (Some(&first), Some(&second)) = (line.first(), line.get(1)) else { return Ok(Vec::new()) };
    #[allow(clippy::cast_precision_loss)] // Line counts are far below 2⁵².
    let phase = (i as f64 / sewing.staggers).rem_euclid(1.0) * sewing.pattern.iter().sum::<f64>();
    if line.iter().all(|point| *point == first) {
        return Ok(if phase > 0.0 { vec![first] } else { Vec::new() });
    }
    let segment = first.distance(second);
    let reach = if segment > 0.0 { phase * (segment / SHORT_SEGMENT).min(1.0) } else { 0.0 };
    let back = if reach > 0.0 {
        let scale = reach / segment;
        Point::new(first.x() + (first.x() - second.x()) * scale, first.y() + (first.y() - second.y()) * scale).unwrap_or(first)
    } else {
        first
    };
    let lengthened: Vec<Point> = std::iter::once(back).chain(line.iter().copied()).collect();
    let placed = sewing.along(&lengthened, i, sewing.jitter, meter)?;
    // The needle points on the stretch added lie between `back` and `first`.
    let on_stretch = |q: &Point| *q != first && q.distance(back) + q.distance(first) <= reach + ON_STRETCH;
    let ahead: Vec<Point> = placed.into_iter().skip_while(on_stretch).skip_while(|q| *q == first).collect();
    let mut sewn = vec![first];
    let Some((&end, inner)) = ahead.split_last() else { return Ok(sewn) };
    sewn.extend(inner.iter().copied().skip_while(|q| !at_least(q.distance(first), sewing.min)));
    sewn.push(end);
    Ok(sewn)
}
