//! The spirals: a contour fill's rings followed from the outline in, as one spiral or as 2, one in and one
//! back out (`REQ-FILL-CON-004`, `docs/src/design/algorithms/fills.md` › Contour fill › Single and double
//! spiral).
//!
//! **The chain.** A spiral follows one ring after another: each ring goes on to its one child, or, where it
//! has several, to the child whose middle lies nearest its own, the middle of a ring being that of its line,
//! weighted by length. The other children are not sewn, nor any ring under them, which `SC-W0311` says.
//!
//! **A turn** goes from a ring to the next. Both are cut into N points, 20 to each longest stitch of the
//! outer ring's length, evenly along each from its start and including its end, and each list then starts
//! again at its point nearest the needle where the fill began. Point i of the turn lies i/(N − 1) of the
//! way from the outer ring's point i to the inner ring's, and the turn is simplified within 0.1 CSS pixel.
//! The turns are joined end to end, each but the last without its last point, as Ink/Stitch joins them,
//! and each is a stretch of its own, stitched as a span.
//!
//! **Double.** The double spiral turns in on the chain's rings 0, 2, 4 and on, and back out on rings 1, 3,
//! 5 and on, reversed.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::line::{length, point_at};
use super::tree::{ROOT, Tree};
use crate::generators::made;
use crate::normalize::simplify::simplify;

/// A turn's points to each longest stitch of its outer ring's length.
const POINTS_PER_STITCH: f64 = 20.0;
/// Each turn is simplified within this, in millimetres: 0.1 CSS pixel.
const SIMPLIFIED: f64 = 0.1 * MM_PER_SVG_PX;

/// The rings a spiral follows, from the outline in, and how many rings of the tree it leaves out. One unit
/// of `meter` for each ring looked at.
pub(super) fn chain(tree: &Tree, meter: &mut Meter) -> Result<(Vec<usize>, usize), Exhausted> {
    let (mut rings, mut left_out, mut node) = (Vec::new(), 0, ROOT);
    loop {
        meter.charge(1)?;
        rings.push(node);
        let children = tree.children(node);
        let next = match children {
            [] => break,
            [only] => *only,
            several => {
                let centre = middle(tree.ring(node));
                let distance = |child: usize| middle(tree.ring(child)).distance(centre);
                let nearest = several.iter().copied().reduce(|best, child| if distance(child) < distance(best) { child } else { best });
                let Some(nearest) = nearest else { break };
                left_out += several.iter().filter(|&&child| child != nearest).map(|&child| tree.count(child)).sum::<usize>();
                nearest
            }
        };
        node = next;
    }
    Ok((rings, left_out))
}

/// The single spiral along `rings`, from the outermost in, its turns cut by `longest`, from the points
/// nearest `start`, as stretches. Empty when no turn has 2 points.
pub(super) fn single(rings: &[&[Point]], longest: f64, start: Point, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    spiral(rings, longest, start, meter)
}

/// The double spiral along `rings`: in on the even ones, back out on the odd ones.
pub(super) fn double(rings: &[&[Point]], longest: f64, start: Point, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let even: Vec<&[Point]> = rings.iter().step_by(2).copied().collect();
    let odd: Vec<&[Point]> = rings.iter().skip(1).step_by(2).copied().collect();
    let mut path = spiral(&even, longest, start, meter)?;
    path.extend(spiral(&odd, longest, start, meter)?.into_iter().rev().map(|mut turn| {
        turn.reverse();
        turn
    }));
    Ok(path)
}

/// The turns from each of `rings` to the next, a stretch each: each but the last without its last point,
/// and the last point only when the last turn has one.
fn spiral(rings: &[&[Point]], longest: f64, start: Point, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let (mut stretches, mut end): (Vec<Vec<Point>>, Option<Point>) = (Vec::new(), None);
    for pair in rings.windows(2) {
        let mut turn = turn(pair[0], pair[1], longest, start, meter)?;
        end = turn.pop();
        if !turn.is_empty() {
            stretches.push(turn);
        }
    }
    match (end, stretches.last_mut()) {
        (Some(end), Some(last)) => last.push(end),
        (Some(end), None) => stretches.push(vec![end]),
        (None, _) => {}
    }
    Ok(stretches)
}

/// The turn from the ring `outer` to the ring `inner`: empty when the outer ring takes fewer than 2 points.
/// A unit of `meter`'s work and a stitch for each point.
fn turn(outer: &[Point], inner: &[Point], longest: f64, start: Point, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let count = POINTS_PER_STITCH * length(outer) / longest;
    if !count.is_finite() || count < 2.0 {
        return Ok(Vec::new());
    }
    // A whole number of at least 2, truncated as Ink/Stitch truncates it, and `as` saturates: the budget
    // refuses as many points as that before they are made.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let n = count as usize;
    made(meter, n)?;
    let (from, to) = (from_nearest(resampled(outer, n), start), from_nearest(resampled(inner, n), start));
    let last = (n - 1) as f64;
    let points: Vec<Point> = from.iter().zip(&to).enumerate().map(|(i, (&a, &b))| blend(a, b, i as f64 / last)).collect();
    simplify(&points, SIMPLIFIED, meter)
}

/// The point `w` of the way from `a` to `b`, weighted as numpy's interpolation weighs them: `a` times
/// 1 - `w`, plus `b` times `w`.
fn blend(a: Point, b: Point, w: f64) -> Point {
    Point::new(a.x() * (1.0 - w) + b.x() * w, a.y() * (1.0 - w) + b.y() * w).unwrap_or(a)
}

/// `n` points evenly along `ring` from its start to its end, both included.
fn resampled(ring: &[Point], n: usize) -> Vec<Point> {
    let long = length(ring);
    let step = if n > 1 { long / (n - 1) as f64 } else { 0.0 };
    (0..n).map(|i| if i + 1 == n { long } else { i as f64 * step }).filter_map(|at| point_at(ring, at)).collect()
}

/// `points` started again at the one nearest `start`, the first of equally near ones.
fn from_nearest(mut points: Vec<Point>, start: Point) -> Vec<Point> {
    let nearest = points.iter().enumerate().fold((0, f64::INFINITY), |best, (i, p)| {
        let d = p.distance(start);
        if d < best.1 { (i, d) } else { best }
    });
    points.rotate_left(nearest.0);
    points
}

/// The middle of the line `ring`: the middles of its sides, each weighted by its length, or its first
/// point when it has no length.
fn middle(ring: &[Point]) -> Point {
    let (mut x, mut y, mut total) = (0.0, 0.0, 0.0);
    for side in ring.windows(2) {
        let (a, b) = (side[0], side[1]);
        let length = a.distance(b);
        x += length * (a.x() + b.x()) / 2.0;
        y += length * (a.y() + b.y()) / 2.0;
        total += length;
    }
    if total > 0.0 { Point::new(x / total, y / total).unwrap_or(Point::ORIGIN) } else { ring.first().copied().unwrap_or(Point::ORIGIN) }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::contour::tree::Node;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn square(x0: f64, x1: f64) -> Vec<Point> {
        vec![p(x0, x0), p(x1, x0), p(x1, x1), p(x0, x1), p(x0, x0)]
    }

    #[test]
    fn a_turn_goes_from_a_ring_to_the_next_point_by_point() {
        let meter = &mut Budget::DEFAULT.meter();
        // A square 16 round, 4 to a longest stitch of 20: 80 points, 0.2 apart. The turn starts on the outer
        // ring at its point nearest the start and ends on the inner ring at its point nearest it.
        let (outer, inner) = (square(0.0, 4.0), square(1.0, 3.0));
        let turn = turn(&outer, &inner, 4.0, p(0.0, 0.0), meter).unwrap();
        assert_eq!(turn.first(), Some(&p(0.0, 0.0)));
        assert!(turn.last().unwrap().distance(p(1.0, 1.0)) < 1e-12, "{turn:?}");
        // A ring too short for 2 points makes no turn: 16 round with a longest stitch of 200, 1.6 points.
        assert!(super::turn(&outer, &inner, 200.0, p(0.0, 0.0), meter).unwrap().is_empty());
        assert_eq!(super::turn(&outer, &inner, 160.0, p(0.0, 0.0), meter).unwrap().len(), 2);
        assert_eq!(resampled(&outer, 5), [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]);
        assert_eq!(from_nearest(vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)], p(5.0, 5.0)), [p(4.0, 4.0), p(0.0, 0.0), p(4.0, 0.0)]);
    }

    #[test]
    fn the_spirals_join_their_turns_end_to_end() {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = [square(0.0, 8.0), square(1.0, 7.0), square(2.0, 6.0), square(3.0, 5.0)];
        let rings: Vec<&[Point]> = rings.iter().map(Vec::as_slice).collect();
        // A stretch for each of the 3 turns.
        let single = single(&rings, 4.0, p(0.0, 0.0), meter).unwrap();
        assert_eq!(single.len(), 3);
        let single = single.concat();
        assert_eq!(single.first(), Some(&p(0.0, 0.0)));
        assert!(single.last().unwrap().distance(p(3.0, 3.0)) < 1e-12);
        // The double spiral goes in on rings 0 and 2, and back out on 3 and 1, ending on ring 1.
        let double = double(&rings, 4.0, p(0.0, 0.0), meter).unwrap().concat();
        assert_eq!(double.first(), Some(&p(0.0, 0.0)));
        assert!(double.last().unwrap().distance(p(1.0, 1.0)) < 1e-12, "{:?}", double.last());
        // One ring makes no spiral, and 2 no double one.
        assert!(super::single(&rings[..1], 4.0, p(0.0, 0.0), meter).unwrap().is_empty());
        assert!(super::double(&rings[..2], 4.0, p(0.0, 0.0), meter).unwrap().is_empty());
    }

    #[test]
    fn a_spiral_goes_on_to_the_child_whose_middle_lies_nearest() {
        // A ring with 2 children, the second nearer its middle: the chain takes it, and leaves the first out
        // with its child.
        let tree = Tree {
            nodes: vec![
                Node { ring: square(0.0, 10.0), children: vec![1, 3] },
                Node { ring: vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0), p(1.0, 1.0)], children: vec![2] },
                Node { ring: square(1.5, 2.5), children: Vec::new() },
                Node { ring: square(4.0, 6.0), children: Vec::new() },
            ],
        };
        assert_eq!(chain(&tree, &mut Budget::DEFAULT.meter()).unwrap(), (vec![0, 3], 2));
        assert_eq!(middle(&square(0.0, 2.0)), p(1.0, 1.0));
        assert_eq!(middle(&[p(3.0, 3.0), p(3.0, 3.0)]), p(3.0, 3.0));
    }
}
