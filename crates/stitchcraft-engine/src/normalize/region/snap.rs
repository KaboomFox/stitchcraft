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
//! hot. Edges are bent and checked for crossings until none cross, which one or two rounds settle. This is
//! snap rounding (Hobby; Guibas and Marimont), with edges bent again as hot points are added (Halperin
//! and Packer's iterated form). Every edge then runs from grid point to grid point and meets the others
//! only at its ends, so the faces' winding numbers add up.
//!
//! What it does to a drawing: points that rounding left a hair apart almost always round to the same grid
//! point, so their shapes share the point, and a gap or an overlap that narrow closes. 2 points a hair apart
//! on either side of a cell's edge stay a cell apart, as they would on Ink/Stitch's grid. Nothing moves by
//! more than a few cells.

use std::collections::btree_map::Entry;
use std::collections::{BTreeMap, BTreeSet};

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

/// How many rounds of bending edges, and of making hot the cells where edges still cross, before the
/// rings are given up as a bug. Any drawing tried settles in 1 or 2.
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
/// runs through them, its own corners among them. `Unjoined` should the edges not settle, a bug. One unit
/// of `meter` for each edge bent and each hot point tested against it, and each pair of edges whose spans
/// along x overlap.
pub(crate) fn round(rings: &[Vec<Node>], meter: &mut Meter) -> Result<Result<Vec<Vec<Node>>, Unjoined>, Exhausted> {
    let mut hot: BTreeSet<Node> = rings.iter().flatten().copied().collect();
    let mut paths = rings.to_vec();
    // Each round bends the edges, or, once none can be bent further, makes hot the cells where they cross.
    for _ in 0..ROUNDS {
        if bend(&mut paths, &hot, meter)? {
            continue;
        }
        let crossing = crossings(&paths, meter)?;
        if crossing.is_empty() {
            return Ok(Ok(paths));
        }
        for [a, b, c, d] in crossing {
            // The cell is new: an edge that passes through a hot cell has been bent through its point.
            let at = geom::crossing(a.point(), b.point(), c.point(), d.point());
            let Some(cell) = meeting(a, b, c, d, at) else { return Ok(Err(Unjoined)) };
            hot.insert(cell);
        }
    }
    Ok(Err(Unjoined))
}

/// Bends each edge of `paths` that passes through the cell of a hot point other than its ends through
/// that point, the points in order along the edge. Whether any edge was bent. One unit of `meter` for each
/// edge and each hot point tested against it.
fn bend(paths: &mut [Vec<Node>], hot: &BTreeSet<Node>, meter: &mut Meter) -> Result<bool, Exhausted> {
    let mut bent = false;
    // An edge drawn several times, either way, is worked out once.
    let mut known: BTreeMap<(Node, Node), Vec<Node>> = BTreeMap::new();
    for path in paths.iter_mut() {
        let mut out = Vec::with_capacity(path.len());
        for pair in path.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            out.push(a);
            let key = (a.min(b), a.max(b));
            if let Entry::Vacant(slot) = known.entry(key) {
                meter.charge(1)?;
                slot.insert(through(key.0, key.1, hot, meter)?);
            }
            let points = known.get(&key).map_or(&[][..], Vec::as_slice);
            bent |= !points.is_empty();
            if a == key.0 {
                out.extend(points.iter().copied());
            } else {
                out.extend(points.iter().rev().copied());
            }
        }
        out.extend(path.last().copied());
        *path = out;
    }
    Ok(bent)
}

/// The hot points other than `a` and `b` whose cells the segment from `a` to `b` passes through, in order
/// from `a`. Every such point lies strictly between the ends along the segment, each cell further along
/// than the one before, so the order of their distances along it is the order the segment runs through
/// them. A cell can meet the segment only when its point lies within the segment's bounds, its sides being
/// a half cell from it and the segment's ends grid points. One unit of `meter` for each hot point tested.
fn through(a: Node, b: Node, hot: &BTreeSet<Node>, meter: &mut Meter) -> Result<Vec<Node>, Exhausted> {
    let (bottom, top) = (a.y.min(b.y), a.y.max(b.y));
    let lowest = Node { x: a.x.min(b.x), y: i64::MIN };
    let highest = Node { x: a.x.max(b.x), y: i64::MAX };
    let mut points = Vec::new();
    for &c in hot.range(lowest..=highest) {
        meter.charge(1)?;
        if c != a && c != b && (bottom..=top).contains(&c.y) && touches(a, b, c) {
            points.push(c);
        }
    }
    points.sort_by_key(|&c| along(a, b, c));
    Ok(points)
}

/// The pairs of edges of `paths` that cross, each once, as the ends of both. One unit of `meter` for each
/// pair of edges whose spans along x overlap.
fn crossings(paths: &[Vec<Node>], meter: &mut Meter) -> Result<Vec<[Node; 4]>, Exhausted> {
    // Each edge once, from its least end: in order of their least x.
    let edges: BTreeSet<(Node, Node)> = paths.iter().flat_map(|path| path.windows(2).map(|w| (w[0].min(w[1]), w[0].max(w[1])))).collect();
    let edges: Vec<(Node, Node)> = edges.into_iter().filter(|(a, b)| a != b).collect();
    let mut found = Vec::new();
    for (k, &(a, b)) in edges.iter().enumerate() {
        for &(c, d) in edges.iter().skip(k + 1) {
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
        // Past it, or ending short of it.
        assert!(!touches(n(-4, 2), n(4, 2), c));
        assert!(!touches(n(-8, 0), n(-2, 0), c));
        assert!(!touches(n(-8, 6), n(-2, 0), c));
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
    fn edges_are_bent_through_the_hot_points_they_pass_in_order() {
        // The segment from (0, 0) to (20, 10) passes through the cells of (6, 2), (10, 4) and (10, 6), the
        // last 2 one above the other, and misses those of (6, 6) and (14, 4).
        let mut meter = Budget::DEFAULT.meter();
        let hot: BTreeSet<Node> = [n(0, 0), n(20, 10), n(10, 6), n(6, 2), n(6, 6), n(14, 4), n(10, 4)].into_iter().collect();
        assert_eq!(through(n(0, 0), n(20, 10), &hot, &mut meter).unwrap(), [n(6, 2), n(10, 4), n(10, 6)]);
        // Out and back: bent the same both ways.
        let mut paths = vec![vec![n(0, 0), n(20, 10), n(0, 0)]];
        assert!(bend(&mut paths, &hot, &mut meter).unwrap());
        assert_eq!(paths[0], [n(0, 0), n(6, 2), n(10, 4), n(10, 6), n(20, 10), n(10, 6), n(10, 4), n(6, 2), n(0, 0)]);
        assert!(!bend(&mut paths, &hot, &mut meter).unwrap(), "bent through every one already");
    }

    #[test]
    fn rings_that_cross_are_bent_through_the_cells_of_their_crossings() {
        // A bow tie: its 2 sides cross at (2, 2) mm, a grid point.
        let corners = [p(0.0, 0.0), p(4.0, 4.0), p(4.0, 0.0), p(0.0, 4.0), p(0.0, 0.0)];
        let ring: Vec<Node> = corners.iter().map(|&q| Node::of(q).unwrap()).collect();
        let middle = Node::of(p(2.0, 2.0)).unwrap();
        let paths = round(&[ring], &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert_eq!(paths[0].iter().filter(|&&q| q == middle).count(), 2);
        assert_eq!(paths[0].len(), 7);
        // 3 lines through one point, crossing there a hair apart in floating point: one point of the grid.
        let ring: Vec<Node> = [(-0.5, 2.0), (1.3, -0.6), (-1.2, -0.1), (2.0, 1.5), (-1.2, 0.3), (2.0, 1.1), (-0.5, 2.0)]
            .iter()
            .map(|&(x, y)| Node::of(p(x, y)).unwrap())
            .collect();
        let paths = round(std::slice::from_ref(&ring), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        let middles: BTreeSet<Node> = paths[0].iter().filter(|q| q.point().distance(p(0.4, 0.7)) < 1e-6).copied().collect();
        assert_eq!(middles.len(), 1, "{middles:?}");
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
