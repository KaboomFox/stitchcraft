//! Snap rounding: a fill's rings put on a fine grid and bent where they meet, so that the edges between
//! the cuts meet only at their ends, whatever rounding did to the drawing (`REQ-FILL-001`).
//!
//! **Why.** A drawing's shapes often meet at a corner or share an edge, and its numbers reach the shared
//! points by different sums: relative path data and transforms leave them a few units in the last place
//! apart. Worked out in floating point, the crossings of edges that nearly coincide land anywhere near
//! where they should. Cutting the rings at such crossings, or at points found near them, can leave 2 edges
//! crossing with no point between them, or an edge running past its segment's end. The faces' winding
//! numbers then stop adding up, and whole faces come out filled or empty against the fill rule.
//!
//! **How.** Every point is put on a grid whose cell is [`GRID`] wide, about 2.9e-11 mm, much as
//! Ink/Stitch puts every fill's shape on a grid of 1e-10 CSS pixels, about 2.6e-11 mm. On the grid the
//! arithmetic is exact: a [`Node`] counts half cells in whole numbers, even for a grid point and odd for
//! its cell's corners, and every product fits in 128 bits. A segment of a ring that passes through the
//! cell of a hot point, a point the rings make, is bent through that point. The hot points are the rings'
//! corners at first, and then the cells where 2 edges still cross: a crossing worked out in floating
//! point lies in one of the cells round it, and the nearest of those that both edges pass through becomes
//! hot. Edges are bent and checked for crossings until none cross: in every drawing tried, the first check
//! that finds crossings finds them all, and bending through them settles the edges. This is snap rounding
//! (Hobby; Guibas and Marimont), with edges bent again as hot points are added (Halperin and Packer's
//! iterated form). Every edge then runs from grid point to grid point and meets the others only at its
//! ends, so the faces' winding numbers add up.
//!
//! What it does to a drawing: points that rounding left a hair apart almost always round to the same grid
//! point, so their shapes share the point, and a gap or an overlap that narrow closes. 2 points a hair apart
//! on either side of a cell's edge stay a cell apart, as they would on Ink/Stitch's grid. Nothing moves by
//! more than a few cells.

use stitchcraft_core::{Exhausted, Meter, Point};

use super::assemble::Unjoined;
use super::geom;

/// The grid's cell, in millimetres: 2^-35 mm, about 2.9e-11 mm. A power of 2, so that a point is put on
/// the grid, and taken off it, exactly.
pub(crate) const GRID: f64 = 1.0 / 34_359_738_368.0;

/// Half a cell, in millimetres: the unit a [`Node`] counts in.
const HALF: f64 = GRID / 2.0;

/// How far from the origin a point can be put on the grid, in millimetres: 2^16 mm, about 65 m. Within it
/// a node's coordinates stay below 2^53 half cells, and every product taken of them fits in 128 bits. A
/// design lies within 10 m of the origin (`design::WORKING_LIMIT_MM`), and a part grows by 10 mm at most.
const REACH: f64 = 65_536.0;

/// How many rounds, each bending edges or else making hot the cells where they cross, before the rings are
/// given up as a bug. Every drawing tried settles within 6: up to 4 rounds of bending, 1 of crossings, and
/// 1 that finds nothing left to do.
const ROUNDS: usize = 64;

/// A point of the grid, counted in half cells from the origin: its coordinates are even. Its cell holds
/// every point that rounds to it, from 1 below it to just short of 1 above it on each axis, so that the
/// cell's left and bottom sides and the corner where they meet are its own. Ordered by x, then y.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct Node {
    x: i64,
    y: i64,
}

impl Node {
    /// The grid point `p` rounds to, halves up, or `None` beyond [`REACH`].
    pub(crate) fn of(p: Point) -> Option<Node> {
        Some(Node { x: half_cells(p.x())?, y: half_cells(p.y())? })
    }

    /// Where it lies, in millimetres: exactly a multiple of [`GRID`].
    pub(crate) fn point(self) -> Point {
        // Below 2^53 in size, a node's coordinates are exact as floats, and so are their multiples of HALF.
        Point::new(self.x as f64 * HALF, self.y as f64 * HALF).unwrap_or(Point::ORIGIN)
    }
}

/// The half cells to the grid point `v` rounds to, halves up, or `None` beyond [`REACH`]. Within it,
/// `v / GRID` is exact, adding a half is too, and the whole number is below 2^52.
#[allow(clippy::cast_possible_truncation)]
fn half_cells(v: f64) -> Option<i64> {
    (v.abs() <= REACH).then(|| 2 * (v / GRID + 0.5).floor() as i64)
}

/// Twice the area that `c` and the segment from `a` to `b` span: positive when `c` lies to the left of the
/// segment's line in y-up axes, negative to its right, 0 on it. Exact.
fn turn(a: Node, b: Node, c: Node) -> i128 {
    let (ax, ay) = (i128::from(a.x), i128::from(a.y));
    (i128::from(b.x) - ax) * (i128::from(c.y) - ay) - (i128::from(b.y) - ay) * (i128::from(c.x) - ax)
}

/// How far along the segment from `a` to `b` the point `c` lies: its distance from `a` along the segment,
/// times the segment's length. Exact.
fn along(a: Node, b: Node, c: Node) -> i128 {
    let (ax, ay) = (i128::from(a.x), i128::from(a.y));
    (i128::from(b.x) - ax) * (i128::from(c.x) - ax) + (i128::from(b.y) - ay) * (i128::from(c.y) - ay)
}

/// Whether the segment from `a` to `b` passes through the cell of `c`: through its inside, or through its
/// own corner, the bottom left. Its ends are grid points and the cell's sides lie between grid points, so
/// the segment can neither end on a side nor run along one. It misses the cell when its box ends a whole
/// cell short of `c` on some side, and otherwise the line through it meets the inside unless all 4 corners
/// lie on one side of it. Exact.
fn touches(a: Node, b: Node, c: Node) -> bool {
    if a == b {
        return a == c;
    }
    if a.x.max(b.x) <= c.x - 2 || a.x.min(b.x) >= c.x + 2 || a.y.max(b.y) <= c.y - 2 || a.y.min(b.y) >= c.y + 2 {
        return false;
    }
    let side = |x: i64, y: i64| turn(a, b, Node { x, y }).signum();
    let sides = [side(c.x - 1, c.y - 1), side(c.x + 1, c.y - 1), side(c.x + 1, c.y + 1), side(c.x - 1, c.y + 1)];
    (sides.contains(&1) && sides.contains(&-1)) || sides[0] == 0
}

/// Whether the segments from `a` to `b` and from `c` to `d` cross at a point inside both. Exact.
fn crosses(a: Node, b: Node, c: Node, d: Node) -> bool {
    let apart = |s: i128, t: i128| s.signum() * t.signum() < 0;
    apart(turn(a, b, c), turn(a, b, d)) && apart(turn(c, d, a), turn(c, d, b))
}

/// The rings, each a closed list of grid points (its first repeated at its end), bent through every hot
/// point whose cell they pass through until no 2 of their edges cross: each ring's points in the order it
/// runs through them, its own corners among them. `Unjoined` should the edges not settle in [`ROUNDS`]
/// rounds, a bug. One unit of `meter` for each edge bent and each hot point tested against it, and each
/// pair of edges whose spans along x overlap.
pub(crate) fn round(rings: &[Vec<Node>], meter: &mut Meter) -> Result<Result<Rounded, Unjoined>, Exhausted> {
    settle(rings, ROUNDS, meter)
}

/// Rings bent through the hot points their edges pass ([`round`]).
#[derive(Debug)]
pub(crate) struct Rounded {
    /// The rings, bent.
    pub(crate) paths: Vec<Vec<Node>>,
    /// The hot points, in order of x and then y, each once: every point of the paths is one.
    pub(crate) hot: Vec<Node>,
}

/// The hot points, in order of x and then y, each once: a sorted list, which a segment's search walks from
/// its least x to its greatest, and which gains points only where edges cross, a round's at once.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Hot(Vec<Node>);

impl Hot {
    /// The points `points`, each once.
    fn of(points: impl IntoIterator<Item = Node>) -> Hot {
        let mut points: Vec<Node> = points.into_iter().collect();
        points.sort_unstable();
        points.dedup();
        Hot(points)
    }

    /// Adds `points`, those not hot already. Put after the list, which is sorted, they are sorted on their
    /// own, and the stable sort that merges them in is fast on 2 sorted runs one after the other, as its
    /// docs say. Inserting them one by one would move the list's tail for each, and a drawing whose edges
    /// cross many times would take time the work budget never charges.
    fn extend(&mut self, points: impl IntoIterator<Item = Node>) {
        let start = self.0.len();
        self.0.extend(points);
        if let Some(new) = self.0.get_mut(start..) {
            new.sort_unstable();
        }
        self.0.sort();
        self.0.dedup();
    }

    /// The points whose x lies from `least` to `most`, in order.
    fn across(&self, least: i64, most: i64) -> &[Node] {
        let start = self.0.partition_point(|n| n.x < least);
        let end = start + self.0.get(start..).map_or(0, |rest| rest.partition_point(|n| n.x <= most));
        self.0.get(start..end).unwrap_or_default()
    }
}

/// [`round`], with the edges given up as `Unjoined` should they not settle in `rounds` rounds.
fn settle(rings: &[Vec<Node>], rounds: usize, meter: &mut Meter) -> Result<Result<Rounded, Unjoined>, Exhausted> {
    let mut hot = Hot::of(rings.iter().flatten().copied());
    let mut paths = rings.to_vec();
    // Each round bends the edges, or, once none can be bent further, makes hot the cells where they cross.
    for _ in 0..rounds {
        let Bent::None(edges) = bend(&mut paths, &hot, meter)? else { continue };
        let crossing = crossings(&edges, meter)?;
        if crossing.is_empty() {
            return Ok(Ok(Rounded { paths, hot: hot.0 }));
        }
        // Each cell is new: an edge that passes through a hot cell has been bent through its point.
        let mut cells = Vec::with_capacity(crossing.len());
        for [a, b, c, d] in crossing {
            let at = geom::crossing(a.point(), b.point(), c.point(), d.point());
            let Some(cell) = meeting(a, b, c, d, at) else { return Ok(Err(Unjoined)) };
            cells.push(cell);
        }
        hot.extend(cells);
    }
    Ok(Err(Unjoined))
}

/// What [`bend`] did.
#[derive(Debug, PartialEq, Eq)]
enum Bent {
    /// Bent an edge or more.
    Some,
    /// Bent none: each edge of the paths once, from its least end, in order of their least ends, as
    /// [`crossings`] takes them.
    None(Vec<(Node, Node)>),
}

/// Bends each edge of `paths` that passes through the cell of a hot point other than its ends through
/// that point, the points in order along the edge. One unit of `meter` for each edge and each hot point
/// tested against it.
fn bend(paths: &mut [Vec<Node>], hot: &Hot, meter: &mut Meter) -> Result<Bent, Exhausted> {
    // Each edge, from its least end, with where it is drawn: an edge drawn several times, either way, is
    // worked out once. Sorted, the edges drawn alike lie side by side.
    let mut edges: Vec<((Node, Node), usize, usize)> = Vec::with_capacity(paths.iter().map(Vec::len).sum());
    for (p, path) in paths.iter().enumerate() {
        for (i, pair) in path.windows(2).enumerate() {
            if let &[a, b] = pair {
                edges.push(((a.min(b), a.max(b)), p, i));
            }
        }
    }
    edges.sort_unstable();
    // The points each edge is bent through, from its least end: a stretch of `points` for each edge drawn.
    let mut points: Vec<Node> = Vec::new();
    let mut bends: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut edge = 0;
    while let Some(&(key, ..)) = edges.get(edge) {
        meter.charge(1)?;
        let start = points.len();
        through(key.0, key.1, hot, meter, &mut points)?;
        let alike = edges.get(edge..).map_or(0, |rest| rest.iter().take_while(|(k, ..)| *k == key).count());
        if points.len() > start {
            bends.extend(edges.get(edge..edge + alike).into_iter().flatten().map(|&(_, p, i)| (p, i, start, points.len())));
        }
        edge += alike.max(1);
    }
    if bends.is_empty() {
        let mut once: Vec<(Node, Node)> = edges.into_iter().map(|(key, ..)| key).collect();
        once.dedup();
        return Ok(Bent::None(once));
    }
    // Each path that has an edge bent, rebuilt with the points of its edges in the order it runs.
    bends.sort_unstable();
    let mut next = 0;
    for (p, path) in paths.iter_mut().enumerate() {
        let mine = bends.get(next..).map_or(0, |rest| rest.iter().take_while(|&&(q, ..)| q == p).count());
        let Some(path_bends) = bends.get(next..next + mine).filter(|bent| !bent.is_empty()) else { continue };
        next += mine;
        let mut out = Vec::with_capacity(path.len() + path_bends.iter().map(|&(_, _, from, to)| to - from).sum::<usize>());
        let mut pending = path_bends.iter().peekable();
        for (i, pair) in path.windows(2).enumerate() {
            let &[a, b] = pair else { continue };
            out.push(a);
            if let Some(&&(_, at, from, to)) = pending.peek()
                && at == i
            {
                pending.next();
                let between = points.get(from..to).unwrap_or_default();
                if a <= b {
                    out.extend(between.iter().copied());
                } else {
                    out.extend(between.iter().rev().copied());
                }
            }
        }
        out.extend(path.last().copied());
        *path = out;
    }
    Ok(Bent::Some)
}

/// The hot points other than `a` and `b` whose cells the segment from `a` to `b` passes through, in order
/// from `a`, added to `points`. Every such point lies strictly between the ends along the segment, each
/// cell further along than the one before, so the order of their distances along it is the order the
/// segment runs through them. A cell can meet the segment only when its point lies within the segment's
/// bounds, its sides being a half cell from it and the segment's ends grid points. One unit of `meter` for
/// each hot point tested.
fn through(a: Node, b: Node, hot: &Hot, meter: &mut Meter, points: &mut Vec<Node>) -> Result<(), Exhausted> {
    let (bottom, top) = (a.y.min(b.y), a.y.max(b.y));
    let start = points.len();
    let across = hot.across(a.x.min(b.x), a.x.max(b.x));
    meter.charge(across.len() as u64)?;
    points.extend(across.iter().copied().filter(|&c| c != a && c != b && (bottom..=top).contains(&c.y) && touches(a, b, c)));
    points.get_mut(start..).into_iter().for_each(|found| found.sort_by_key(|&c| along(a, b, c)));
    Ok(())
}

/// The pairs of `edges` that cross, each once, as the ends of both: the edges of the paths, each once,
/// from its least end, in order of their least ends ([`Bent::None`]). One unit of `meter` for each pair of
/// edges whose spans along x overlap.
fn crossings(edges: &[(Node, Node)], meter: &mut Meter) -> Result<Vec<[Node; 4]>, Exhausted> {
    let mut found = Vec::new();
    for (k, &(a, b)) in edges.iter().enumerate().filter(|(_, (a, b))| a != b) {
        for &(c, d) in edges.iter().skip(k + 1).filter(|(c, d)| c != d) {
            if c.x > b.x {
                break;
            }
            meter.charge(1)?;
            if crosses(a, b, c, d) {
                found.push([a, b, c, d]);
            }
        }
    }
    Ok(found)
}

/// The hot point for the crossing of the edges from `a` to `b` and from `c` to `d`, which floating point
/// puts at `at`: of the cell `at` rounds to and the 8 round it, the one nearest `at` that both edges pass
/// through (the first in order where 2 are as near). The crossing lies in one of them unless rounding
/// moved it by more than a cell, which only edges crossing at a very small angle can do. Those run
/// through the same cells for a long way, and once both are bent through the point of any one of them,
/// they no longer cross.
fn meeting(a: Node, b: Node, c: Node, d: Node, at: Point) -> Option<Node> {
    let near = Node::of(at)?;
    let steps = [-2, 0, 2];
    steps
        .iter()
        .flat_map(|&dx| steps.iter().map(move |&dy| Node { x: near.x + dx, y: near.y + dy }))
        .filter(|&cell| touches(a, b, cell) && touches(c, d, cell))
        .min_by(|p, q| p.point().distance(at).total_cmp(&q.point().distance(at)).then(p.cmp(q)))
}

/// Twice the area a closed ring of grid points bounds, exactly, in square half cells: positive when it
/// turns counter-clockwise in y-up axes. A sum of many products can pass 2^127, so it is kept as the times
/// it wrapped round 2^128 and what is left; ordered as the numbers they stand for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TwiceArea {
    wraps: i64,
    rest: i128,
}

impl TwiceArea {
    /// None.
    const ZERO: TwiceArea = TwiceArea { wraps: 0, rest: 0 };

    /// Of the closed ring through `ring`'s points (the first need not be repeated at the end).
    pub(crate) fn of(ring: &[Node]) -> TwiceArea {
        let next = ring.iter().skip(1).chain(ring.first());
        ring.iter().zip(next).fold(TwiceArea::ZERO, |area, (a, b)| area.plus(i128::from(a.x) * i128::from(b.y) - i128::from(b.x) * i128::from(a.y)))
    }

    /// This and `term`, which is less than 2^127 in size.
    fn plus(self, term: i128) -> TwiceArea {
        let (rest, wrapped) = self.rest.overflowing_add(term);
        let wraps = match (wrapped, term > 0) {
            (false, _) => self.wraps,
            (true, true) => self.wraps + 1,
            (true, false) => self.wraps - 1,
        };
        TwiceArea { wraps, rest }
    }

    /// Whether the ring turns clockwise in y-up axes.
    pub(crate) fn clockwise(self) -> bool {
        self < TwiceArea::ZERO
    }

    /// Whether the ring turns counter-clockwise in y-up axes.
    pub(crate) fn counter_clockwise(self) -> bool {
        self > TwiceArea::ZERO
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The node `(x, y)` half cells from the origin.
    fn n(x: i64, y: i64) -> Node {
        Node { x, y }
    }

    #[test]
    fn points_round_to_the_nearest_grid_point_halves_up_and_back_exactly() {
        assert_eq!(Node::of(p(0.0, 0.0)), Some(n(0, 0)));
        assert_eq!(Node::of(p(GRID, -GRID)), Some(n(2, -2)));
        assert_eq!(Node::of(p(0.49 * GRID, -0.49 * GRID)), Some(n(0, 0)));
        // A half rounds up, on either side of 0.
        assert_eq!(Node::of(p(0.5 * GRID, -0.5 * GRID)), Some(n(2, 0)));
        assert_eq!(Node::of(p(1.5 * GRID, -1.5 * GRID)), Some(n(4, -2)));
        // Whole and half millimetres are grid points, and come back as they were.
        for v in [1.0, -2.5, 10.0, 130.0, 9_999.75] {
            assert_eq!(Node::of(p(v, -v)).unwrap().point(), p(v, -v));
        }
        // 2 sums that should be the same number, a few units in the last place apart, round to one point.
        let (a, b) = (0.1 + 0.2, 0.3);
        assert_ne!(a, b);
        assert_eq!(Node::of(p(a, a)), Node::of(p(b, b)));
        // Beyond the reach, there is no grid point.
        assert!(Node::of(p(REACH, -REACH)).is_some());
        assert_eq!(Node::of(p(REACH * 1.0001, 0.0)), None);
        assert_eq!(Node::of(p(0.0, -REACH * 1.0001)), None);
    }

    #[test]
    fn a_segment_passes_through_a_cell_through_its_inside_or_its_bottom_left_corner() {
        let c = n(0, 0);
        // Through the inside, and its own ends.
        assert!(touches(n(-4, 0), n(4, 0), c));
        assert!(touches(n(0, 0), n(4, 4), c));
        assert!(touches(n(-4, 2), n(4, -2), c));
        // Through the bottom left corner (-1, -1) only: the cell's own.
        assert!(touches(n(-4, 2), n(2, -4), c));
        // Through any other corner only: the next cell's.
        assert!(!touches(n(-4, -2), n(2, 4), c), "top left");
        assert!(!touches(n(-2, -4), n(4, 2), c), "bottom right");
        assert!(!touches(n(4, -2), n(-2, 4), c), "top right");
        // Past it.
        assert!(!touches(n(-4, 2), n(4, 2), c));
        // Ending short of it on a line through it, on each side, or short of its own corner: only the
        // segment's bounds tell.
        assert!(!touches(n(-8, 0), n(-2, 0), c), "left");
        assert!(!touches(n(2, 0), n(8, 0), c), "right");
        assert!(!touches(n(0, -8), n(0, -2), c), "below");
        assert!(!touches(n(0, 2), n(0, 8), c), "above");
        assert!(!touches(n(-8, 6), n(-2, 0), c), "short of the corner");
        // A segment of no length passes through its own cell only.
        assert!(!touches(n(2, 2), n(2, 2), c));
    }

    #[test]
    fn segments_cross_only_inside_both() {
        assert!(crosses(n(0, 0), n(4, 4), n(0, 4), n(4, 0)));
        assert!(!crosses(n(0, 0), n(4, 4), n(2, 2), n(4, 0)), "an end on the other");
        assert!(!crosses(n(0, 0), n(4, 4), n(4, 4), n(8, 0)), "an end shared");
        assert!(!crosses(n(0, 0), n(4, 0), n(2, 0), n(6, 0)), "along each other");
        assert!(!crosses(n(0, 0), n(4, 4), n(6, 0), n(8, 4)), "apart");
    }

    #[test]
    fn hot_points_are_kept_once_in_order_and_found_by_their_x() {
        let mut hot = Hot::of([n(4, 2), n(0, 0), n(2, 6), n(4, -2), n(2, 6), n(8, 0)]);
        assert_eq!(hot.0, [n(0, 0), n(2, 6), n(4, -2), n(4, 2), n(8, 0)], "in order of x, then y, each once");
        hot.extend([n(6, 4), n(2, 6), n(-2, 0), n(6, 4)]);
        assert_eq!(hot.0, [n(-2, 0), n(0, 0), n(2, 6), n(4, -2), n(4, 2), n(6, 4), n(8, 0)], "the crossings' points join in their places, once");
        // From the least x to the greatest, both included.
        assert_eq!(hot.across(2, 4), [n(2, 6), n(4, -2), n(4, 2)]);
        assert_eq!(hot.across(4, 4), [n(4, -2), n(4, 2)], "an upright segment's");
        assert_eq!(hot.across(-8, 0), [n(-2, 0), n(0, 0)]);
        assert_eq!(hot.across(8, 20), [n(8, 0)]);
        assert_eq!(hot.across(-8, 20), hot.0);
        assert!(hot.across(1, 1).is_empty() && hot.across(10, 20).is_empty() && hot.across(-8, -4).is_empty());
    }

    #[test]
    fn edges_are_bent_through_the_hot_points_they_pass_in_order() {
        // The segment from (0, 0) to (20, 10) passes through the cells of (6, 2), (10, 4) and (10, 6), the
        // last 2 one above the other, and misses those of (6, 6) and (14, 4).
        let mut meter = Budget::DEFAULT.meter();
        let hot = Hot::of([n(0, 0), n(20, 10), n(10, 6), n(6, 2), n(6, 6), n(14, 4), n(10, 4)]);
        let through = |a, b, hot: &Hot, meter: &mut Meter| {
            let mut points = Vec::new();
            super::through(a, b, hot, meter, &mut points).unwrap();
            points
        };
        assert_eq!(through(n(0, 0), n(20, 10), &hot, &mut meter), [n(6, 2), n(10, 4), n(10, 6)]);
        // Out and back: bent the same both ways.
        let mut paths = vec![vec![n(0, 0), n(20, 10), n(0, 0)]];
        assert_eq!(bend(&mut paths, &hot, &mut meter).unwrap(), Bent::Some);
        assert_eq!(paths[0], [n(0, 0), n(6, 2), n(10, 4), n(10, 6), n(20, 10), n(10, 6), n(10, 4), n(6, 2), n(0, 0)]);
        assert!(matches!(bend(&mut paths, &hot, &mut meter).unwrap(), Bent::None(_)), "bent through every one already");
        // The same turned upside down and moved: running downhill, the edge reaches (-10, 26) before
        // (-10, 24), the other way round from the points' own order.
        let hot = Hot::of([n(-20, 30), n(0, 20), n(-10, 24), n(-14, 28), n(-14, 24), n(-6, 26), n(-10, 26)]);
        assert_eq!(through(n(-20, 30), n(0, 20), &hot, &mut meter), [n(-14, 28), n(-10, 26), n(-10, 24)]);
    }

    #[test]
    fn every_path_is_bent_at_once_and_an_edge_drawn_twice_alike() {
        // 2 squares side by side share their edge from (8, 0) to (8, 8), drawn up by one and down by the
        // other, and (8, 4) is hot; so is (4, 0), on the first square's bottom.
        let mut paths = vec![vec![n(0, 0), n(8, 0), n(8, 8), n(0, 8), n(0, 0)], vec![n(8, 0), n(16, 0), n(16, 8), n(8, 8), n(8, 0)]];
        let hot = Hot::of(paths.iter().flatten().copied().chain([n(8, 4), n(4, 0)]));
        let mut meter = Budget::DEFAULT.meter();
        assert_eq!(bend(&mut paths, &hot, &mut meter).unwrap(), Bent::Some);
        assert_eq!(paths[0], [n(0, 0), n(4, 0), n(8, 0), n(8, 4), n(8, 8), n(0, 8), n(0, 0)]);
        assert_eq!(paths[1], [n(8, 0), n(16, 0), n(16, 8), n(8, 8), n(8, 4), n(8, 0)]);
    }

    #[test]
    fn crossings_are_sought_among_the_edges_side_by_side_along_x() {
        // The bow tie's sides, from their least ends, and an edge to the right of both: one pair of edges
        // overlaps along x, and crosses. An edge of no length crosses nothing.
        let edges = [(n(0, 0), n(4, 4)), (n(0, 4), n(4, 0)), (n(2, 2), n(2, 2)), (n(6, 0), n(8, 2))];
        let mut meter = Budget::DEFAULT.meter();
        assert_eq!(crossings(&edges, &mut meter).unwrap(), [[n(0, 0), n(4, 4), n(0, 4), n(4, 0)]]);
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 1);
    }

    #[test]
    fn how_far_along_a_segment_a_point_lies_is_its_distance_along_times_the_length() {
        // From (2, 4) toward (8, 12), 10 long: (5, 8) lies 5 along, (8, 12) 10, (10, 4) 4.8 and (-4, -4) 10 back.
        let along_from = |c| along(n(2, 4), n(8, 12), c);
        assert_eq!([n(5, 8), n(8, 12), n(10, 4), n(-4, -4)].map(along_from), [50, 100, 48, -100]);
    }

    /// A bow tie: its 2 sides cross at (2, 2) mm, a grid point.
    fn bow_tie() -> Vec<Node> {
        [p(0.0, 0.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 4.0), p(0.0, 0.0)].iter().map(|&q| Node::of(q).unwrap()).collect()
    }

    #[test]
    fn rings_that_cross_are_bent_through_the_cells_of_their_crossings() {
        let middle = Node::of(p(2.0, 2.0)).unwrap();
        let paths = round(&[bow_tie()], &mut Budget::DEFAULT.meter()).unwrap().unwrap().paths;
        assert_eq!(paths[0].iter().filter(|&&q| q == middle).count(), 2);
        assert_eq!(paths[0].len(), 7);
        // 3 lines through one point, crossing there a hair apart in floating point: one point of the grid.
        let ring: Vec<Node> = [(-0.5, 2.0), (1.3, -0.6), (-1.2, -0.1), (2.0, 1.5), (-1.2, 0.3), (2.0, 1.1), (-0.5, 2.0)]
            .iter()
            .map(|&(x, y)| Node::of(p(x, y)).unwrap())
            .collect();
        let rounded = round(std::slice::from_ref(&ring), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let paths = &rounded.paths;
        assert!(paths.iter().flatten().all(|q| rounded.hot.binary_search(q).is_ok()), "every point of the paths is hot");
        let middles: std::collections::BTreeSet<Node> = paths[0].iter().filter(|q| q.point().distance(p(0.4, 0.7)) < 1e-6).copied().collect();
        assert_eq!(middles.len(), 1, "{middles:?}");
    }

    /// An even number of half cells from -2 `span` to 2 `span`: a grid point's coordinate.
    fn even(rng: &mut stitchcraft_core::SplitMix64, span: u64) -> i64 {
        2 * (i64::try_from(rng.next_u64() % (2 * span + 1)).unwrap() - i64::try_from(span).unwrap())
    }

    #[test]
    fn nearly_parallel_edges_however_long_settle() {
        // Pairs of edges up to several metres long, level or at a slant, each end a few half cells aside
        // from a line through a common middle: they cross at angles down to a few half cells in their
        // length. Floating point puts many of their crossings cells along them from where they are, and
        // both edges still pass through a cell round that point.
        let mut rng = stitchcraft_core::SplitMix64::new(0x5eed_0002);
        let (mut crossed, mut far) = (0, 0);
        for _ in 0..20_000 {
            let middle = (even(&mut rng, 1 << 39), even(&mut rng, 1 << 39));
            let step = if rng.next_u64().is_multiple_of(2) { (2, 0) } else { (2 + even(&mut rng, 500).abs(), even(&mut rng, 500)) };
            let mut edge = || {
                let k = (1_i64 << (20 + rng.next_u64() % 29)) / step.0.max(step.1.abs());
                let mut end =
                    |sign: i64| Node { x: middle.0 + sign * k * step.0 + even(&mut rng, 5), y: middle.1 + sign * k * step.1 + even(&mut rng, 5) };
                (end(-1), end(1))
            };
            let ((a, b), (c, d)) = (edge(), edge());
            if !crosses(a, b, c, d) {
                continue;
            }
            crossed += 1;
            // Whether floating point puts the crossing more than a cell, 2 half cells, from where it is.
            let at = geom::crossing(a.point(), b.point(), c.point(), d.point());
            let t = turn(c, d, a) as f64 / (turn(c, d, a) - turn(c, d, b)) as f64;
            let (x, y) = (at.x() / HALF - a.x as f64 - t * (b.x - a.x) as f64, at.y() / HALF - a.y as f64 - t * (b.y - a.y) as f64);
            far += usize::from(x * x + y * y > 4.0);
            let settled = round(&[vec![a, b], vec![c, d]], &mut Budget::DEFAULT.meter()).unwrap();
            assert!(settled.is_ok(), "{a:?}–{b:?} and {c:?}–{d:?}");
        }
        assert!(crossed > 5_000 && far > crossed / 10, "{crossed} crossed, {far} put more than a cell off");
    }

    #[test]
    fn edges_that_do_not_settle_in_the_rounds_allowed_are_unjoined() {
        // The bow tie takes 3 rounds: one finds where its sides cross, one bends them through that point,
        // and one finds nothing left to bend and no crossing.
        let mut meter = Budget::DEFAULT.meter();
        assert!(settle(&[bow_tie()], 2, &mut meter).unwrap().is_err());
        assert!(settle(&[bow_tie()], 3, &mut meter).unwrap().is_ok());
    }

    #[test]
    fn a_crossing_rounded_into_the_wrong_cell_takes_the_nearest_cell_both_edges_pass_through() {
        // The edges cross at (0, 0). Worked out there, the crossing's own cell. Put a cell to the right, as
        // rounding can put the crossing of edges at a small angle, a cell only one edge passes through: the
        // nearest that both do. Put 2 cells off, none of the 9 cells round it.
        let (a, b, c, d) = (n(-20, 0), n(20, 0), n(0, -20), n(0, 20));
        assert_eq!(meeting(a, b, c, d, n(0, 0).point()), Some(n(0, 0)));
        assert_eq!(meeting(a, b, c, d, n(2, 0).point()), Some(n(0, 0)));
        assert_eq!(meeting(a, b, c, d, n(4, 2).point()), None);
        // Edges at a small angle pass through the same cells, here those of (0, 0), (2, 0) and (4, 0). Put on
        // the side between the first 2, as near both: the first in order.
        assert_eq!(meeting(a, b, n(-20, -2), n(20, 2), n(1, 0).point()), Some(n(0, 0)));
    }

    #[test]
    fn areas_are_exact_and_ordered() {
        let square = [n(0, 0), n(2, 0), n(2, 2), n(0, 2)];
        assert_eq!(TwiceArea::of(&square), TwiceArea { wraps: 0, rest: 8 });
        assert!(TwiceArea::of(&square).counter_clockwise());
        let turned: Vec<Node> = square.iter().rev().copied().collect();
        assert!(TwiceArea::of(&turned).clockwise());
        // A sliver a half cell wide and 2^52 half cells long: no float would say which way it turns.
        let far = 1_i64 << 52;
        let sliver = [n(-far, 0), n(far, 0), n(far, 2), n(-far + 2, 2)];
        assert!(TwiceArea::of(&sliver).counter_clockwise());
        // Sums past 2^127 wrap, and still compare as the numbers they stand for.
        let top = TwiceArea { wraps: 0, rest: i128::MAX };
        assert_eq!(top.plus(1), TwiceArea { wraps: 1, rest: i128::MIN });
        assert!(top.plus(1) > top && top.plus(1).plus(-1) == top);
        assert_eq!(TwiceArea::ZERO.plus(i128::MIN).plus(-1), TwiceArea { wraps: -1, rest: i128::MAX });
        assert!(TwiceArea::ZERO.plus(i128::MIN).plus(-1).clockwise());
        assert!(top.plus(1).counter_clockwise());
    }
}
