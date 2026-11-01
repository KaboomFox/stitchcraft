//! Zigzag stroke: stitches across the path from side to side, as wide as the stroke. Ink/Stitch's zig-zag
//! stitch (read at `d59c9ab`) is sewn alike.
//!
//! Design: `docs/src/design/algorithms/strokes.md` › Zigzag stroke. Each part of the path, one per
//! subpath, is sewn on its own.
//!
//! 1. **Spread.** The part is flattened as a satin's or a fill's subpaths are, and n + 1 points are spread
//!    along it at equal distances, its ends included. n is its length divided by half the spacing, rounded
//!    half to even, at least 2, and made even, so the zigzag starts and ends on the same side.
//! 2. **Directions** come from the spread points. The ends' are the lines to and from their neighbours,
//!    and every other point's lies halfway between its two lines, each taken with length 1.
//! 3. **Across.** Each point moves square to its direction, turned by the angle. The even ones move right
//!    of the way the path goes by half the width plus the second pull compensation. The odd ones move left
//!    by half the width plus the first.
//! 4. A closed stroke, one whose only subpath that moves ends where it starts, ends on its first needle
//!    point. A subpath that draws nothing does not count (`DEV-RUN-008`).
//! 5. **Repeats and bean stitch** ([`passes`]). A turn takes no step of the bean list, as in Ink/Stitch's
//!    zigzag.
//!
//! Ink/Stitch spreads the points along a running stitch at half the spacing, which follows the path. Here
//! they are spread along the path itself (`DEV-RUN-007`). The needle points come from arithmetic, square
//! roots and the angle's sine and cosine from [`math`] alone, and every platform places them alike.

mod params;
#[cfg(test)]
mod tests;

pub use params::ZigzagStrokeParams;
use stitchcraft_core::units::{MM_PER_SVG_PX, at_least};
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point, math};

use crate::design::Path;
use crate::generators::passes::{self, RepeatParams, Turn};
use crate::generators::{Stitched, TooSmall, mm, settled, too_small};
use crate::normalize::along::Along;
use crate::normalize::stroke::{self, SHAPE_TOLERANCE};

/// Ends nearer each other than this, in millimetres, close a stroke's only subpath that moves: 0.05 CSS
/// pixels, as in Ink/Stitch.
const CLOSED: f64 = 0.05 * MM_PER_SVG_PX;

/// The stitches of `path`, a stroke `width` millimetres wide, in a zigzag as `params` says, each part sewn
/// its repeats and bean stitch as `passes` says. A part that is one point is not stitched, and `SC-W0401`
/// names it with the shortest stitch `min_stitch`. `SC-W0405` names stitches shorter than that, which
/// finishing thins out. Every needle point costs a stitch from `meter`.
pub fn zigzag_stroke(
    path: &Path,
    width: Mm,
    params: &ZigzagStrokeParams,
    passes: &RepeatParams,
    min_stitch: Mm,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(path, width, params, passes, min_stitch, meter))
}

/// [`zigzag_stroke`], before its stitches are settled.
fn sewn(
    path: &Path,
    width: Mm,
    params: &ZigzagStrokeParams,
    passes: &RepeatParams,
    min_stitch: Mm,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    // The registry reads 1 spacing or more; the first is the zigzag's.
    let Some(spacing) = params.zigzag_spacing_mm.first() else {
        let missing = Diagnostic::new(Code::InternalCheckFailed, "A zigzag stroke's spacing is missing.");
        return Ok(Stitched { runs: Vec::new(), warnings: vec![missing] });
    };
    let stroke = stroke::flatten(path, SHAPE_TOLERANCE, meter)?;
    let closed = is_closed(stroke.pieces.iter().map(|piece| piece.points.as_slice()));
    let sides = Sides::new(width, params);
    let mut runs = Vec::with_capacity(stroke.pieces.len());
    let mut warnings = Vec::new();
    let mut short = Short::default();
    for piece in &stroke.pieces {
        meter.charge(1)?;
        if piece.points.len() < 2 {
            warnings.push(too_small(TooSmall::Point, min_stitch.get()));
            continue;
        }
        let mut points = zigzag(&Along::new(&piece.points, meter)?, spacing.get() / 2.0, sides, meter)?;
        if closed
            && let Some(&first) = points.first()
            && let Some(last) = points.last_mut()
        {
            *last = first;
        }
        // Counted as sewn: a closed zigzag ends with the stitch onto its first needle point.
        short.count(&points, min_stitch.get(), meter)?;
        runs.push(passes::sew(&points, passes.repeats, &passes.bean_stitch_repeats, Turn::Joined, meter)?);
    }
    warnings.extend(short.diagnostic(spacing.get(), min_stitch.get()));
    Ok(Stitched { runs, warnings })
}

/// A zigzag's stitches before repeats, and those shorter than the shortest stitch, which finishing leaves
/// a needle point of out (`SC-W0405`).
#[derive(Debug, Default)]
struct Short {
    stitches: usize,
    short: usize,
    shortest: Option<f64>,
}

impl Short {
    /// Counts the stitches between `points`, and those shorter than `min` among them, measured as finishing
    /// measures them ([`at_least`]). A unit of `meter` each.
    fn count(&mut self, points: &[Point], min: f64, meter: &mut Meter) -> Result<(), Exhausted> {
        for pair in points.windows(2) {
            meter.charge(1)?;
            let [a, b] = pair else { continue };
            let length = a.distance(*b);
            self.stitches += 1;
            if !at_least(length, min) {
                self.short += 1;
                self.shortest = Some(self.shortest.map_or(length, |shortest| shortest.min(length)));
            }
        }
        Ok(())
    }

    /// `SC-W0405` for the short stitches, if any, of a zigzag at `spacing` for a shortest stitch `min`, with
    /// the width a straight zigzag needs: a stitch across it is `√(w² + (spacing/2)²)` long.
    fn diagnostic(&self, spacing: f64, min: f64) -> Option<Diagnostic> {
        let shortest = self.shortest?;
        let (min_text, shortest_text) = (mm(min), mm(shortest));
        // A part of a zigzag has 2 stitches at least, so all of them are never 1.
        let message = match (self.short, self.stitches) {
            (short, all) if short == all => format!(
                "All {all} of the zigzag's stitches are shorter than the shortest stitch ({min_text} mm), at {shortest_text} mm, so finishing leaves out every other needle point, and the zigzag sews as a line."
            ),
            (1, all) => format!(
                "1 of the zigzag's {all} stitches is shorter than the shortest stitch ({min_text} mm), at {shortest_text} mm, so finishing leaves out a needle point of it, and the zigzag sews narrower there."
            ),
            (short, all) => format!(
                "{short} of the zigzag's {all} stitches are shorter than the shortest stitch ({min_text} mm), the shortest {shortest_text} mm long, so finishing leaves out a needle point of each, and the zigzag sews narrower there."
            ),
        };
        let half = spacing / 2.0;
        let hint = if min > half {
            // Rounded up to the hundredth a message shows, so that the width it names is enough.
            let width = ((min * min - half * half).sqrt() * 100.0).ceil() / 100.0;
            format!("Make the zigzag at least {} mm wide, or space it out.", mm(width))
        } else {
            "Make the zigzag wider, or space it out.".to_string()
        };
        Some(Diagnostic::new(Code::ZigzagTooNarrow, message).with_fix(Fix::Hint(hint)))
    }
}

/// Whether a stroke whose parts are `pieces` is closed, as Ink/Stitch closes a zigzag: it has one part
/// that moves, and that part's ends lie nearer each other than [`CLOSED`].
fn is_closed<'a>(pieces: impl Iterator<Item = &'a [Point]>) -> bool {
    let moving: Vec<&[Point]> = pieces.filter(|points| points.len() > 1).collect();
    matches!(moving.as_slice(), [only] if only.first().zip(only.last()).is_some_and(|(a, b)| a.distance(*b) < CLOSED))
}

/// How far a zigzag's needle points go from the path, and which way: the odd ones to the left of the way
/// the path goes and the even ones to its right, square to it turned by the angle's cosine and sine.
#[derive(Clone, Copy, Debug)]
struct Sides {
    left: f64,
    right: f64,
    cos: f64,
    sin: f64,
}

impl Sides {
    /// The sides of a stroke `width` millimetres wide, as `params` widens and turns them.
    fn new(width: Mm, params: &ZigzagStrokeParams) -> Sides {
        let [first, second] = params.stroke_pull_compensation_mm.map(Mm::get);
        let (sin, cos) = math::sin_cos(math::to_radians(params.zigzag_angle));
        Sides { left: width.get() / 2.0 + first, right: width.get() / 2.0 + second, cos, sin }
    }
}

/// The needle points of one part, measured `along`, before repeats: spread at equal distances in a whole
/// number of zigs and zags, `half` a spacing apart or a little more or less, then moved across. Every point
/// costs `meter` a unit of work twice, and a stitch, spent before any is placed.
fn zigzag(along: &Along, half: f64, sides: Sides, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let length = along.length();
    let n = halves(length, half);
    meter.charge_stitches(n.saturating_add(1))?;
    let mut spread = Vec::new();
    for i in 0..=n {
        meter.charge(1)?;
        spread.push(along.point(f64::from(i) / f64::from(n) * length));
    }
    let mut points = Vec::with_capacity(spread.len());
    for (i, &at) in spread.iter().enumerate() {
        meter.charge(1)?;
        let before = i.checked_sub(1).and_then(|j| spread.get(j)).copied();
        let after = spread.get(i + 1).copied();
        points.push(across(at, direction(before, at, after), i % 2 == 1, sides));
    }
    Ok(points)
}

/// How many half spacings of `half` millimetres a part `length` millimetres long takes, as Ink/Stitch
/// counts them: the length over the half spacing, rounded half to even, at least 2, and one more when
/// odd.
fn halves(length: f64, half: f64) -> u32 {
    // A whole number of at least 2, and `as` saturates: the budget refuses as many points as that long
    // before it, so the saturated count's one more never matters.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = (length / half).round_ties_even().max(2.0) as u32;
    n.saturating_add(n % 2)
}

/// The direction at `at`, a spread point, from the point `before` it to the point `after` it: the line to
/// the next for the first, the line from the one before for the last, and halfway between the two for the
/// others, each taken with length 1. A line with no length gives way to the other.
fn direction(before: Option<Point>, at: Point, after: Option<Point>) -> (f64, f64) {
    let line = |from: Point, to: Point| (to.x() - from.x(), to.y() - from.y());
    match (before.map(|b| line(b, at)), after.map(|a| line(at, a))) {
        (Some(incoming), Some(outgoing)) => match (unit(incoming), unit(outgoing)) {
            (Some((in_x, in_y)), Some((out_x, out_y))) => ((in_x + out_x) / 2.0, (in_y + out_y) / 2.0),
            (Some(_), None) => incoming,
            _ => outgoing,
        },
        (Some(one), None) | (None, Some(one)) => one,
        (None, None) => (0.0, 0.0),
    }
}

/// `v` with length 1, or `None` when it has no length.
fn unit((x, y): (f64, f64)) -> Option<(f64, f64)> {
    let length = (x * x + y * y).sqrt();
    (length > 0.0).then(|| (x / length, y / length))
}

/// `at` moved across `direction`: to the left of it when `odd`, else to its right, by that side's
/// distance, square to it turned by the angle. A point with no direction stays where it is.
fn across(at: Point, direction: (f64, f64), odd: bool, sides: Sides) -> Point {
    let Some((x, y)) = unit(direction) else { return at };
    // Square to the direction, to its right as the drawing shows it (y grows down), then turned.
    let (nx, ny) = (-y, x);
    let (nx, ny) = (nx * sides.cos - ny * sides.sin, nx * sides.sin + ny * sides.cos);
    let by = if odd { -sides.left } else { sides.right };
    Point::new(at.x() + nx * by, at.y() + ny * by).unwrap_or(at)
}
