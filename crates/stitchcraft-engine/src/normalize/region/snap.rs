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
//! **What is searched.** Bending an edge needs the hot points near it, and the check needs the edges that
//! may cross it. Both are sought in [`Buckets`]: the rings' bounds cut into about as many boxes as there
//! are points, each edge looking only in the boxes it passes. An area shrunk or grown is cut from a raw
//! outline that lies many times over itself: tens of thousands of short edges, of which hundreds share any
//! span along x. Searched by x alone, a traced photo's background took more than the work budget.
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
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
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

/// Whether the segments from `a` to `b` and from `c` to `d` share a height: the bounds of one along y reach
/// those of the other. Segments that share none don't cross, and this says so with 2 comparisons, before
/// [`crosses`] multiplies 8 times in 128 bits.
fn level(a: Node, b: Node, c: Node, d: Node) -> bool {
    c.y.max(d.y) >= a.y.min(b.y) && c.y.min(d.y) <= a.y.max(b.y)
}

/// The rings, each a closed list of grid points (its first repeated at its end), bent through every hot
/// point whose cell they pass through until no 2 of their edges cross: each ring's points in the order it
/// runs through them, its own corners among them. `Unjoined` should the edges not settle in [`ROUNDS`]
/// rounds, a bug. One unit of `meter` for each hot point and each edge put in a bucket, each bucket an edge
/// looks in, each hot point tested against an edge, and each pair of edges tested for a crossing.
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

    /// The points of `rings`, each once, and each ring as the places of its points among them. Sorted with
    /// where each comes from, the points are numbered as they are put in order.
    fn ranked(rings: &[Vec<Node>]) -> (Hot, Vec<Vec<usize>>) {
        let mut all: Vec<(Node, usize)> = rings.iter().flatten().copied().zip(0..).collect();
        all.sort_unstable();
        let mut points: Vec<Node> = Vec::with_capacity(all.len());
        let mut places = vec![0; all.len()];
        for (node, at) in all {
            if points.last() != Some(&node) {
                points.push(node);
            }
            if let Some(slot) = places.get_mut(at) {
                *slot = points.len() - 1;
            }
        }
        let mut places = places.into_iter();
        let paths = rings.iter().map(|ring| places.by_ref().take(ring.len()).collect()).collect();
        (Hot(points), paths)
    }

    /// Adds `points`, those not hot already, and gives the place each point hot before has now. Sorted on
    /// their own, they are merged into the list, which is sorted. Inserting them one by one would move the
    /// list's tail for each, and a drawing whose edges cross many times would take time the work budget
    /// never charges.
    fn extend(&mut self, points: impl IntoIterator<Item = Node>) -> Vec<usize> {
        let mut new: Vec<Node> = points.into_iter().collect();
        new.sort_unstable();
        new.dedup();
        let mut merged = Vec::with_capacity(self.0.len() + new.len());
        let mut moved = Vec::with_capacity(self.0.len());
        let mut new = new.into_iter().peekable();
        for &old in &self.0 {
            while let Some(added) = new.next_if(|&added| added < old) {
                merged.push(added);
            }
            new.next_if_eq(&old);
            moved.push(merged.len());
            merged.push(old);
        }
        merged.extend(new);
        self.0 = merged;
        moved
    }

    /// The point at `place`.
    fn at(&self, place: usize) -> Node {
        self.0.get(place).copied().unwrap_or_default()
    }

    /// The place of `node`, which is hot.
    fn place(&self, node: Node) -> usize {
        self.0.partition_point(|&h| h < node)
    }
}

/// How many buckets each way a grid for `count` points or edges has: the whole part of its square root, so
/// that there are no more buckets than things to put in them, and 1 at least.
fn side_for(count: usize) -> usize {
    count.isqrt().max(1)
}

/// A grid of buckets over the bounds of some grid points, `side` each way: the box of points a bucket
/// holds is as wide and as high as the bounds' `side`th part, rounded up. A point belongs to one bucket,
/// and one beyond the bounds to the nearest. A bucket's number is its column's times `side`, and its row's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Buckets {
    /// The least x and y of the points.
    least: Node,
    /// How wide and how high a bucket's box is, in half cells: 1 at least.
    width: i64,
    height: i64,
    /// How many columns, and how many rows: 1 at least.
    side: i64,
}

impl Buckets {
    /// Over the bounds of `nodes`, `side` buckets each way (1 for none).
    fn over(nodes: impl Iterator<Item = Node>, side: usize) -> Buckets {
        let side = i64::try_from(side).unwrap_or(i64::MAX).max(1);
        let bounds = nodes.fold(None, |bounds: Option<(Node, Node)>, n| {
            let (least, most) = bounds.unwrap_or((n, n));
            Some((Node { x: least.x.min(n.x), y: least.y.min(n.y) }, Node { x: most.x.max(n.x), y: most.y.max(n.y) }))
        });
        let (least, most) = bounds.unwrap_or_default();
        Buckets { least, width: (most.x - least.x) / side + 1, height: (most.y - least.y) / side + 1, side }
    }

    /// How many buckets there are.
    fn count(&self) -> usize {
        usize::try_from(self.side * self.side).unwrap_or(usize::MAX)
    }

    /// The column of the points at `x`.
    fn column(&self, x: i64) -> i64 {
        ((x - self.least.x) / self.width).clamp(0, self.side - 1)
    }

    /// The row of the points at `y`.
    fn row(&self, y: i64) -> i64 {
        ((y - self.least.y) / self.height).clamp(0, self.side - 1)
    }

    /// The number of the bucket `node` belongs to.
    fn of(&self, node: Node) -> usize {
        usize::try_from(self.column(node.x) * self.side + self.row(node.y)).unwrap_or(0)
    }

    /// Gives `visit` the number of every bucket that may hold a point whose cell the segment from `a` to
    /// `b` passes through, each once, in order: so of every bucket the segment itself passes through too.
    /// In each column the segment's stretch that lies within a half cell of the column's points is taken,
    /// and the rows from a half cell below its lowest point to a half cell above its highest.
    fn along(&self, a: Node, b: Node, mut visit: impl FnMut(usize) -> Result<(), Exhausted>) -> Result<(), Exhausted> {
        let (a, b) = (a.min(b), a.max(b));
        let (first, last) = (self.column(a.x), self.column(b.x));
        for column in first..=last {
            // A column after the segment's first starts after the segment does, and one before its last
            // ends before it does.
            let left = self.least.x + column * self.width;
            let from = if column == first { a.x } else { left - 1 };
            let to = if column == last { b.x } else { left + self.width };
            let ((low, _), (_, high)) = if a.y <= b.y { (height(a, b, from), height(a, b, to)) } else { (height(a, b, to), height(a, b, from)) };
            for row in self.row(low - 1)..=self.row(high + 1) {
                visit(usize::try_from(column * self.side + row).unwrap_or(0))?;
            }
        }
        Ok(())
    }
}

/// How high the segment from `a` to `b`, `a` the lesser, is at `x`, which lies between their x: in whole
/// half cells, rounded down and rounded up. An upright segment's lowest and highest points. Exact.
fn height(a: Node, b: Node, x: i64) -> (i64, i64) {
    let run = i128::from(b.x) - i128::from(a.x);
    if run == 0 {
        return (a.y, b.y);
    }
    // At an end, where most edges are asked for theirs: no division.
    if x == a.x || x == b.x {
        let y = if x == a.x { a.y } else { b.y };
        return (y, y);
    }
    let rise = (i128::from(b.y) - i128::from(a.y)) * (i128::from(x) - i128::from(a.x));
    // The rise at `x` is no more than the segment's own, so it is a coordinate's size.
    let down = a.y + i64::try_from(rise.div_euclid(run)).unwrap_or(0);
    (down, down + i64::from(rise.rem_euclid(run) != 0))
}

/// The hot points by the bucket each belongs to, for the edges to look near themselves.
#[derive(Debug)]
struct Near {
    buckets: Buckets,
    /// The points, in order of their buckets and then their own.
    nodes: Vec<Node>,
    /// Where each bucket's points start in `nodes`, and after the last where they end.
    starts: Vec<usize>,
}

impl Near {
    /// The points of `hot` in a grid of `side` buckets each way. One unit of `meter` for each point.
    fn of(hot: &Hot, side: usize, meter: &mut Meter) -> Result<Near, Exhausted> {
        meter.charge(hot.0.len() as u64)?;
        let buckets = Buckets::over(hot.0.iter().copied(), side);
        // The points are in order, so put in their buckets one by one they are in order in each.
        let (nodes, starts) = grouped(hot.0.iter().map(|&node| (buckets.of(node), node)), buckets.count());
        Ok(Near { buckets, nodes, starts })
    }

    /// The points of the bucket numbered `bucket`.
    fn held(&self, bucket: usize) -> &[Node] {
        let ends = self.starts.get(bucket).zip(self.starts.get(bucket + 1));
        ends.and_then(|(&start, &end)| self.nodes.get(start..end)).unwrap_or_default()
    }
}

/// `items`, each given with the number of its bucket, of `count` buckets: the items in order of their
/// buckets, those of one bucket in the order they came, and where each bucket's start among them, and after
/// the last where they end. Each bucket's items are counted, and each item is then put at the next place of
/// its bucket: sorting them by their buckets would compare each many times over.
fn grouped<T: Copy + Default>(items: impl Iterator<Item = (usize, T)> + Clone, count: usize) -> (Vec<T>, Vec<usize>) {
    let mut starts = vec![0; count + 1];
    for (bucket, _) in items.clone() {
        if let Some(start) = starts.get_mut(bucket + 1) {
            *start += 1;
        }
    }
    let mut before = 0;
    for start in &mut starts {
        before += *start;
        *start = before;
    }
    let mut next = starts.clone();
    let mut held = vec![T::default(); before];
    for (bucket, item) in items {
        let place = next.get_mut(bucket).map(|place| std::mem::replace(place, *place + 1));
        if let Some(slot) = place.and_then(|place| held.get_mut(place)) {
            *slot = item;
        }
    }
    (held, starts)
}

/// [`round`], with the edges given up as `Unjoined` should they not settle in `rounds` rounds.
fn settle(rings: &[Vec<Node>], rounds: usize, meter: &mut Meter) -> Result<Result<Rounded, Unjoined>, Exhausted> {
    // The paths are the places of their points among the hot points, which every point of theirs is.
    let (mut hot, mut paths) = Hot::ranked(rings);
    // Each round bends the edges, or, once none can be bent further, makes hot the cells where they cross.
    // An edge is looked at again only where it may now pass a hot point: one made by bending is looked at
    // among them all, and after a round of crossings every edge among the cells made hot.
    let mut near = Near::of(&hot, side_for(hot.0.len()), meter)?;
    let mut look = Look { new: paths.iter().map(|path| vec![true; path.len().saturating_sub(1)]).collect(), added: None };
    for _ in 0..rounds {
        let Bent::None(edges) = bend(&mut paths, &mut look, &hot, &near, meter)? else { continue };
        let crossing = crossings(&edges, meter)?;
        if crossing.is_empty() {
            let paths = paths.iter().map(|path| path.iter().map(|&place| hot.at(place)).collect()).collect();
            return Ok(Ok(Rounded { paths, hot: hot.0 }));
        }
        // Each cell is new: an edge that passes through a hot cell has been bent through its point.
        let mut cells = Vec::with_capacity(crossing.len());
        for [a, b, c, d] in crossing {
            let at = geom::crossing(a.point(), b.point(), c.point(), d.point());
            let Some(cell) = meeting(a, b, c, d, at) else { return Ok(Err(Unjoined)) };
            cells.push(cell);
        }
        let added = Hot::of(cells.iter().copied());
        let moved = hot.extend(cells);
        paths.iter_mut().flatten().for_each(|place| *place = moved.get(*place).copied().unwrap_or(*place));
        near = Near::of(&hot, side_for(hot.0.len()), meter)?;
        look.added = Some(Added::of(&added, meter)?);
    }
    Ok(Err(Unjoined))
}

/// Which edges [`bend`] looks at, those that may pass through the cell of a hot point other than their ends.
#[derive(Debug)]
struct Look {
    /// For each edge of each path, whether it is new since the hot points last changed, and is looked at
    /// among them all.
    new: Vec<Vec<bool>>,
    /// The hot points the last round of crossings made, which the other edges are looked at among. With
    /// none, an edge that isn't new passes none: it passed none when it was looked at, and neither it nor
    /// the hot points have changed since.
    added: Option<Added>,
}

/// The hot points a round of crossings made, and their bounds: an edge can pass only through the cell of a
/// point within its own bounds ([`through`]).
#[derive(Debug)]
struct Added {
    near: Near,
    least: Node,
    most: Node,
}

impl Added {
    /// The points of `added`, at least one, in a grid of their own. One unit of `meter` for each point.
    fn of(added: &Hot, meter: &mut Meter) -> Result<Added, Exhausted> {
        let near = Near::of(added, side_for(added.0.len()), meter)?;
        let (least, most) = added.0.iter().fold((Node { x: i64::MAX, y: i64::MAX }, Node { x: i64::MIN, y: i64::MIN }), |(least, most), n| {
            (Node { x: least.x.min(n.x), y: least.y.min(n.y) }, Node { x: most.x.max(n.x), y: most.y.max(n.y) })
        });
        Ok(Added { near, least, most })
    }

    /// Whether a point of these may lie within the bounds of the segment from `a` to `b`.
    fn within(&self, a: Node, b: Node) -> bool {
        a.x.min(b.x) <= self.most.x && self.least.x <= a.x.max(b.x) && a.y.min(b.y) <= self.most.y && self.least.y <= a.y.max(b.y)
    }
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

/// Bends each edge of `paths`, each a list of places among the points of `hot`, that passes through the
/// cell of a hot point other than its ends through that point, the points in order along the edge: `near`
/// holds the hot points, and `look` says which edges may pass one, which it then keeps. One unit of `meter`
/// for each edge, and for each bucket an edge looked at looks in and each hot point tested against it.
fn bend(paths: &mut [Vec<usize>], look: &mut Look, hot: &Hot, near: &Near, meter: &mut Meter) -> Result<Bent, Exhausted> {
    // Each edge, from its least end, with where it is drawn: an edge drawn several times, either way, is
    // worked out once. The hot points are in order, so the edges in order of their ends' places are in
    // order of their ends. Put by their least end's place, those of a place in the order the paths draw
    // them, and those of a place by their other end's, which keeps that order among the edges drawn alike,
    // they are sorted, and the edges drawn alike lie side by side. A place has few edges, and sorting all of
    // them compared each many times.
    let mut edges: Vec<(usize, usize, usize, usize)> = Vec::with_capacity(paths.iter().map(Vec::len).sum());
    for (p, path) in paths.iter().enumerate() {
        for (i, pair) in path.windows(2).enumerate() {
            if let &[a, b] = pair {
                edges.push((a.min(b), a.max(b), p, i));
            }
        }
    }
    let (mut sorted, starts) = grouped(edges.iter().enumerate().map(|(k, &(least, ..))| (least, k)), hot.0.len());
    for span in starts.windows(2) {
        let &[start, end] = span else { continue };
        if let Some(group) = sorted.get_mut(start..end).filter(|group| group.len() > 1) {
            group.sort_by_key(|&k| edges.get(k).map(|&(_, most, ..)| most));
        }
    }
    let key = |k: usize| edges.get(k).map(|&(least, most, ..)| (least, most));
    // The points each edge is bent through, from its least end: a stretch of `points` for each edge drawn.
    let (mut points, mut found): (Vec<usize>, Vec<Node>) = (Vec::new(), Vec::new());
    let mut bends: Vec<(usize, usize, usize, usize)> = Vec::new();
    let mut at = 0;
    while let Some(&k) = sorted.get(at) {
        meter.charge(1)?;
        let (least, most) = key(k).unwrap_or_default();
        let start = points.len();
        let alike = sorted.get(at..).map_or(0, |rest| rest.iter().take_while(|&&j| key(j) == Some((least, most))).count());
        let drawn = sorted.get(at..at + alike).unwrap_or_default();
        let (a, b) = (hot.at(least), hot.at(most));
        if drawn.iter().filter_map(|&j| edges.get(j)).any(|&(_, _, p, i)| look.new.get(p).and_then(|new| new.get(i)) == Some(&true)) {
            through(a, b, near, meter, &mut found)?;
        } else if let Some(added) = look.added.as_ref().filter(|added| added.within(a, b)) {
            through(a, b, &added.near, meter, &mut found)?;
        }
        points.extend(found.drain(..).map(|node| hot.place(node)));
        if points.len() > start {
            let drawn = sorted.get(at..at + alike).into_iter().flatten().filter_map(|&j| edges.get(j));
            bends.extend(drawn.map(|&(_, _, p, i)| (p, i, start, points.len())));
        }
        at += alike.max(1);
    }
    look.added = None;
    look.new.iter_mut().flatten().for_each(|new| *new = false);
    if bends.is_empty() {
        let mut once: Vec<(Node, Node)> = sorted.iter().filter_map(|&k| key(k)).map(|(least, most)| (hot.at(least), hot.at(most))).collect();
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
        // The edges an edge is bent into are new, and the others not.
        let mut new = Vec::with_capacity(out.capacity());
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
                new.extend(std::iter::repeat_n(true, between.len() + 1));
            } else {
                new.push(false);
            }
        }
        out.extend(path.last().copied());
        *path = out;
        if let Some(slot) = look.new.get_mut(p) {
            *slot = new;
        }
    }
    Ok(Bent::Some)
}

/// The hot points other than `a` and `b` whose cells the segment from `a` to `b` passes through, in order
/// from `a`, added to `points`: `near` holds the hot points. Every such point lies strictly between the
/// ends along the segment, each cell further along than the one before, so the order of their distances
/// along it is the order the segment runs through them. A cell can meet the segment only when its point
/// lies within the segment's bounds, its sides being a half cell from it and the segment's ends grid
/// points. A bucket's points are in order of x, so those within the segment's bounds along x lie together
/// there, and only they are tested: a bucket of a shape's outline holds many points, and a short edge has
/// few between its ends. One unit of `meter` for each bucket looked in and each hot point tested.
fn through(a: Node, b: Node, near: &Near, meter: &mut Meter, points: &mut Vec<Node>) -> Result<(), Exhausted> {
    let (bottom, top) = (a.y.min(b.y), a.y.max(b.y));
    let (left, right) = (a.x.min(b.x), a.x.max(b.x));
    let start = points.len();
    near.buckets.along(a, b, |bucket| {
        let held = near.held(bucket);
        let within = held.get(held.partition_point(|c| c.x < left)..held.partition_point(|c| c.x <= right)).unwrap_or_default();
        meter.charge(1 + within.len() as u64)?;
        points.extend(within.iter().copied().filter(|&c| c != a && c != b && (bottom..=top).contains(&c.y) && touches(a, b, c)));
        Ok(())
    })?;
    points.get_mut(start..).into_iter().for_each(|found| found.sort_by_key(|&c| (along(a, b, c), c)));
    Ok(())
}

/// The pairs of `edges` that cross, each once, as the ends of both, in the edges' order: the edges of the
/// paths, each once, from its least end, in order of their least ends ([`Bent::None`]). The edges are put
/// in the buckets they pass through, and those sharing a bucket are tested. One unit of `meter` for each
/// bucket an edge is put in, and each pair of edges tested.
fn crossings(edges: &[(Node, Node)], meter: &mut Meter) -> Result<Vec<[Node; 4]>, Exhausted> {
    // Long edges pass through many buckets. Should the edges take more room than 4 buckets each, the
    // buckets are made larger, down to 1 that holds every edge once: memory stays in step with the edges.
    let mut side = side_for(edges.len());
    while side > 1 {
        if let Some(found) = crossings_in(edges, side, meter)? {
            return Ok(found);
        }
        side /= 2;
    }
    Ok(crossings_in(edges, 1, meter)?.unwrap_or_default())
}

/// [`crossings`] with `side` buckets each way: `None` should the edges be put in more than 4 buckets each,
/// taken together. In a bucket the edges lie in their order, so an edge is tested against those after it
/// that start before it ends along x, as all the edges are in 1 bucket: first for a height they share
/// ([`level`]), and then for a crossing.
fn crossings_in(edges: &[(Node, Node)], side: usize, meter: &mut Meter) -> Result<Option<Vec<[Node; 4]>>, Exhausted> {
    let buckets = Buckets::over(edges.iter().flat_map(|&(a, b)| [a, b]), side);
    let most = 4 * edges.len();
    let mut put: Vec<(usize, usize)> = Vec::new();
    for (k, &(a, b)) in edges.iter().enumerate().filter(|(_, (a, b))| a != b) {
        buckets.along(a, b, |bucket| {
            put.push((bucket, k));
            meter.charge(1)
        })?;
        if put.len() > most {
            return Ok(None);
        }
    }
    // The edges were put in order, so taken by their buckets alone they are in order in each.
    let (held, starts) = grouped(put.iter().copied(), buckets.count());
    let mut found = Vec::new();
    for ends in starts.windows(2) {
        let &[start, end] = ends else { continue };
        let together = held.get(start..end).unwrap_or_default();
        for (at, &k) in together.iter().enumerate() {
            let Some(&(a, b)) = edges.get(k) else { continue };
            for &later in together.iter().skip(at + 1) {
                let Some(&(c, d)) = edges.get(later) else { continue };
                if c.x > b.x {
                    break;
                }
                meter.charge(1)?;
                if level(a, b, c, d) && crosses(a, b, c, d) {
                    found.push([a, b, c, d]);
                }
            }
        }
    }
    // Edges that share several buckets were tested in each.
    found.sort_unstable();
    found.dedup();
    Ok(Some(found))
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
    fn hot_points_are_kept_once_in_order() {
        let mut hot = Hot::of([n(4, 2), n(0, 0), n(2, 6), n(4, -2), n(2, 6), n(8, 0)]);
        assert_eq!(hot.0, [n(0, 0), n(2, 6), n(4, -2), n(4, 2), n(8, 0)], "in order of x, then y, each once");
        hot.extend([n(6, 4), n(2, 6), n(-2, 0), n(6, 4)]);
        assert_eq!(hot.0, [n(-2, 0), n(0, 0), n(2, 6), n(4, -2), n(4, 2), n(6, 4), n(8, 0)], "the crossings' points join in their places, once");
    }

    /// The hot points `hot` in a grid of `side` buckets each way.
    fn near(hot: &Hot, side: usize) -> Near {
        Near::of(hot, side, &mut Budget::DEFAULT.meter()).unwrap()
    }

    /// `paths` bent as [`bend`] bends them, as places among the points of `hot`, in `side` buckets each way.
    fn bent(paths: &mut Vec<Vec<Node>>, hot: &Hot, side: usize, meter: &mut Meter) -> Bent {
        let mut places: Vec<Vec<usize>> = paths.iter().map(|path| path.iter().map(|&node| hot.place(node)).collect()).collect();
        let mut look = Look { new: places.iter().map(|path| vec![true; path.len().saturating_sub(1)]).collect(), added: None };
        let bent = bend(&mut places, &mut look, hot, &near(hot, side), meter).unwrap();
        *paths = places.iter().map(|path| path.iter().map(|&place| hot.at(place)).collect()).collect();
        bent
    }

    /// The work `run` spends.
    fn spent<T>(run: impl FnOnce(&mut Meter) -> T) -> (T, u64) {
        let mut meter = Budget::DEFAULT.meter();
        let made = run(&mut meter);
        (made, Budget::DEFAULT.max_work - meter.work_left())
    }

    /// The grid points from (0, 0) to (10, 10) half cells, 36 of them.
    fn lattice() -> Vec<Node> {
        (0..6).flat_map(|i| (0..6).map(move |j| n(2 * i, 2 * j))).collect()
    }

    #[test]
    fn a_grid_has_a_bucket_each_way_for_the_square_root_of_what_it_holds() {
        assert_eq!([0, 1, 3, 4, 15, 16, 84_979].map(side_for), [1, 1, 1, 2, 3, 4, 291]);
    }

    #[test]
    fn buckets_cut_the_points_bounds_into_boxes_and_each_point_has_one() {
        // From (-4, 10) to (16, 14): 3 columns 7 half cells wide, and 3 rows 2 high.
        let buckets = Buckets::over([n(-4, 14), n(16, 10), n(2, 12)].into_iter(), 3);
        assert_eq!(buckets, Buckets { least: n(-4, 10), width: 7, height: 2, side: 3 });
        assert_eq!(buckets.count(), 9);
        assert_eq!([-4, 2, 4, 9, 10, 16].map(|x| buckets.column(x)), [0, 0, 1, 1, 2, 2]);
        assert_eq!([10, 11, 12, 13, 14].map(|y| buckets.row(y)), [0, 0, 1, 1, 2]);
        // Beyond the bounds: the nearest.
        assert_eq!([buckets.column(-20), buckets.column(40), buckets.row(0), buckets.row(40)], [0, 2, 0, 2]);
        assert_eq!([n(-4, 10), n(-4, 14), n(4, 12), n(16, 10), n(16, 14)].map(|q| buckets.of(q)), [0, 2, 4, 6, 8]);
        // No points, or no buckets asked for: 1 bucket, a half cell wide.
        let none = Buckets::over(std::iter::empty(), 0);
        assert_eq!(none, Buckets { least: n(0, 0), width: 1, height: 1, side: 1 });
        assert_eq!((none.count(), none.of(n(6, -6))), (1, 0));
    }

    #[test]
    fn a_segment_s_height_at_an_x_is_exact_rounded_down_and_up() {
        // From (0, 0) to (6, 4): a third of the way up at 2, and a sixth at 1. Down the same way.
        assert_eq!([0, 1, 2, 3, 6].map(|x| height(n(0, 0), n(6, 4), x)), [(0, 0), (0, 1), (1, 2), (2, 2), (4, 4)]);
        assert_eq!([0, 1, 3, 6].map(|x| height(n(0, 4), n(6, 0), x)), [(4, 4), (3, 4), (2, 2), (0, 0)]);
        assert_eq!(height(n(-6, -4), n(0, 0), -5), (-4, -3));
        // An upright segment: its ends.
        assert_eq!(height(n(2, -4), n(2, 8), 2), (-4, 8));
    }

    /// The buckets `buckets` gives a segment from `a` to `b`.
    fn passed(buckets: &Buckets, a: Node, b: Node) -> Vec<usize> {
        let mut seen = Vec::new();
        buckets
            .along(a, b, |bucket| {
                seen.push(bucket);
                Ok(())
            })
            .unwrap();
        seen
    }

    #[test]
    fn a_segment_looks_in_the_buckets_within_a_half_cell_of_it() {
        // 4 columns and 4 rows, each 5 half cells: bucket 4 c + r holds x from 5 c and y from 5 r.
        let buckets = Buckets::over([n(0, 0), n(18, 18)].into_iter(), 4);
        assert_eq!((buckets.width, buckets.height), (5, 5));
        // Level along the middle of a row: that row, either way it is drawn.
        assert_eq!(passed(&buckets, n(0, 2), n(18, 2)), [0, 4, 8, 12]);
        assert_eq!(passed(&buckets, n(18, 2), n(0, 2)), [0, 4, 8, 12]);
        // Level, a half cell under the next row: both rows. A cell above the row before: its own only.
        assert_eq!(passed(&buckets, n(6, 4), n(8, 4)), [4, 5]);
        assert_eq!(passed(&buckets, n(6, 6), n(8, 6)), [5]);
        // Upright in a column's middle: that column. Ending a half cell from the next row's points: that row too.
        assert_eq!(passed(&buckets, n(12, 6), n(12, 12)), [9, 10]);
        assert_eq!(passed(&buckets, n(12, 6), n(12, 14)), [9, 10, 11]);
        // Rising across the grid: each column's rows from where it enters, a half cell early, to where it
        // leaves. Falling: the same rows from the other side.
        assert_eq!(passed(&buckets, n(0, 0), n(18, 18)), [0, 1, 4, 5, 6, 9, 10, 11, 14, 15]);
        assert_eq!(passed(&buckets, n(0, 18), n(18, 0)), [2, 3, 5, 6, 7, 8, 9, 10, 12, 13]);
        // It stops as soon as it is told to.
        let mut seen = 0;
        let stopped = buckets.along(n(0, 0), n(18, 18), |_| {
            seen += 1;
            if seen < 3 { Ok(()) } else { Err(Exhausted::Work) }
        });
        assert_eq!((stopped, seen), (Err(Exhausted::Work), 3));
    }

    #[test]
    fn hot_points_are_held_by_bucket_in_order() {
        let hot = Hot::of([n(8, 8), n(0, 0), n(2, 0), n(0, 2), n(6, 2), n(8, 0)]);
        // 2 buckets each way, 5 half cells: (0, 0), (0, 2), (2, 0) in the first, none above it, then (6, 2)
        // and (8, 0), then (8, 8).
        let (near, work) = spent(|meter| Near::of(&hot, 2, meter).unwrap());
        assert_eq!(work, 6, "a unit for each point");
        assert_eq!(near.starts, [0, 3, 3, 5, 6]);
        assert_eq!(near.held(0), [n(0, 0), n(0, 2), n(2, 0)]);
        assert!(near.held(1).is_empty());
        assert_eq!(near.held(2), [n(6, 2), n(8, 0)]);
        assert_eq!(near.held(3), [n(8, 8)]);
        assert!(near.held(4).is_empty(), "past the last bucket");
        // Out of work before the first point.
        assert_eq!(Near::of(&hot, 2, &mut Budget { max_work: 5, ..Budget::DEFAULT }.meter()).unwrap_err(), Exhausted::Work);
    }

    #[test]
    fn items_are_grouped_by_their_buckets_in_the_order_they_came() {
        let items = [(2, 'a'), (0, 'b'), (2, 'c'), (1, 'd'), (2, 'e')];
        let (held, starts) = grouped(items.iter().copied(), 4);
        assert_eq!(held, ['b', 'd', 'a', 'c', 'e']);
        assert_eq!(starts, [0, 1, 2, 5, 5], "where each bucket's start, and after the last where they end");
        let (held, starts) = grouped(std::iter::empty::<(usize, char)>(), 2);
        assert!(held.is_empty());
        assert_eq!(starts, [0, 0, 0]);
    }

    #[test]
    fn an_edge_tests_only_the_hot_points_between_its_ends_along_x() {
        let found = |a, b, hot: &Hot| {
            spent(|meter| {
                let mut points = Vec::new();
                through(a, b, &near(hot, 1), meter, &mut points).unwrap();
                points
            })
        };
        // 7 points on a level line, 4 half cells apart, in 1 bucket. The edge from (8, 0) to (16, 0) tests the
        // 3 from its one end to its other, for the bucket's unit and 3 more, and is bent through the one
        // between its ends. Drawn the other way: the same.
        let hot = Hot::of((0..7).map(|k| n(4 * k, 0)));
        assert_eq!(found(n(8, 0), n(16, 0), &hot), (vec![n(12, 0)], 1 + 3));
        assert_eq!(found(n(16, 0), n(8, 0), &hot), (vec![n(12, 0)], 1 + 3));
        // An upright edge tests the points at its own x.
        let hot = Hot::of([n(0, 4), n(4, 0), n(4, 4), n(4, 8), n(8, 4)]);
        assert_eq!(found(n(4, 0), n(4, 8), &hot), (vec![n(4, 4)], 1 + 3));
    }

    #[test]
    fn segments_share_a_height_where_their_bounds_along_y_meet() {
        let (a, b) = (n(0, 4), n(8, 8));
        // One over the other, and one that ends at the height the other starts at, above it or below it,
        // drawn either way.
        assert!(level(a, b, n(2, 6), n(4, 20)) && level(a, b, n(2, 8), n(4, 12)) && level(a, b, n(2, 0), n(4, 4)));
        assert!(level(b, a, n(4, 12), n(2, 8)) && level(b, a, n(4, 4), n(2, 0)));
        // One wholly above the other, or below it.
        assert!(!level(a, b, n(2, 10), n(4, 12)) && !level(a, b, n(2, 0), n(4, 2)) && !level(b, a, n(4, 2), n(2, 0)));
    }

    #[test]
    fn however_many_buckets_an_edge_is_bent_through_the_same_hot_points() {
        // Every segment between 2 of 36 grid points, all of them hot, with 1 to 7 buckets each way, a
        // bucket down to a point's own cell: the points every hot point tested one by one gives.
        let nodes = lattice();
        let hot = Hot::of(nodes.iter().copied());
        for side in 1..=7 {
            let near = near(&hot, side);
            for (k, &a) in nodes.iter().enumerate() {
                for &b in nodes.iter().skip(k + 1) {
                    let mut expected: Vec<Node> = nodes.iter().copied().filter(|&c| c != a && c != b && touches(a, b, c)).collect();
                    expected.sort_by_key(|&c| along(a, b, c));
                    let mut found = Vec::new();
                    through(a, b, &near, &mut Budget::DEFAULT.meter(), &mut found).unwrap();
                    assert_eq!(found, expected, "{side} buckets each way, {a:?} to {b:?}");
                }
            }
        }
    }

    #[test]
    fn edges_are_bent_through_the_hot_points_they_pass_in_order() {
        // The segment from (0, 0) to (20, 10) passes through the cells of (6, 2), (10, 4) and (10, 6), the
        // last 2 one above the other, and misses those of (6, 6) and (14, 4).
        let mut meter = Budget::DEFAULT.meter();
        let hot = Hot::of([n(0, 0), n(20, 10), n(10, 6), n(6, 2), n(6, 6), n(14, 4), n(10, 4)]);
        let through = |a, b, hot: &Hot, meter: &mut Meter| {
            let mut points = Vec::new();
            super::through(a, b, &near(hot, 2), meter, &mut points).unwrap();
            points
        };
        assert_eq!(through(n(0, 0), n(20, 10), &hot, &mut meter), [n(6, 2), n(10, 4), n(10, 6)]);
        // 2 buckets each way: the edge looks in all 4, which hold the 7 points.
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 4 + 7);
        // Out and back: bent the same both ways.
        let mut paths = vec![vec![n(0, 0), n(20, 10), n(0, 0)]];
        assert_eq!(bent(&mut paths, &hot, 2, &mut meter), Bent::Some);
        assert_eq!(paths[0], [n(0, 0), n(6, 2), n(10, 4), n(10, 6), n(20, 10), n(10, 6), n(10, 4), n(6, 2), n(0, 0)]);
        assert!(matches!(bent(&mut paths, &hot, 2, &mut meter), Bent::None(_)), "bent through every one already");
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
        assert_eq!(bent(&mut paths, &hot, 3, &mut meter), Bent::Some);
        assert_eq!(paths[0], [n(0, 0), n(4, 0), n(8, 0), n(8, 4), n(8, 8), n(0, 8), n(0, 0)]);
        assert_eq!(paths[1], [n(8, 0), n(16, 0), n(16, 8), n(8, 8), n(8, 4), n(8, 0)]);
    }

    /// Every pair of `edges` that crosses, each tested against each.
    fn crossing_pairs(edges: &[(Node, Node)]) -> Vec<[Node; 4]> {
        let mut found = Vec::new();
        for (k, &(a, b)) in edges.iter().enumerate() {
            found.extend(edges.iter().skip(k + 1).filter(|&&(c, d)| crosses(a, b, c, d)).map(|&(c, d)| [a, b, c, d]));
        }
        found
    }

    #[test]
    fn crossings_are_sought_among_the_edges_that_share_a_bucket() {
        // The bow tie's sides, from their least ends, and an edge to the right of both: one pair of edges
        // overlaps along x, and crosses. An edge of no length crosses nothing, and is put in no bucket.
        let edges = [(n(0, 0), n(4, 4)), (n(0, 4), n(4, 0)), (n(2, 2), n(2, 2)), (n(6, 0), n(8, 2))];
        let (found, work) = spent(|meter| crossings(&edges, meter).unwrap());
        assert_eq!(found, [[n(0, 0), n(4, 4), n(0, 4), n(4, 0)]]);
        // 4 edges: 2 buckets each way, 5 half cells wide and 3 high. The sides are put in the first column's
        // 2 and the third edge in the second column's 2: 6, and the sides are tested in both of theirs.
        assert_eq!(work, 6 + 2);
        // In 1 bucket: each edge put once, and the sides tested once. The third edge starts after both end.
        let (found, work) = spent(|meter| crossings_in(&edges, 1, meter).unwrap());
        assert_eq!((found, work), (Some(vec![[n(0, 0), n(4, 4), n(0, 4), n(4, 0)]]), 3 + 1));
        // 2 level edges one after the other along x, in 1 bucket: the first is not tested against the second.
        let apart = [(n(0, 0), n(4, 0)), (n(6, 0), n(8, 0))];
        assert_eq!(spent(|meter| crossings_in(&apart, 1, meter).unwrap()), (Some(vec![]), 2));
        // Meeting end to end, they are.
        let meeting = [(n(0, 0), n(4, 0)), (n(4, 0), n(8, 0))];
        assert_eq!(spent(|meter| crossings_in(&meeting, 1, meter).unwrap()), (Some(vec![]), 3));
    }

    #[test]
    fn however_many_buckets_the_same_edges_cross() {
        // Every segment between 2 of 16 grid points, 120 of them, in order: 1 to 3 buckets each way hold
        // them in no more than 4 each, and give the pairs that testing each against each does, once each.
        let nodes: Vec<Node> = (0..4).flat_map(|i| (0..4).map(move |j| n(6 * i + 2 * j, 4 * j))).collect();
        let mut edges: Vec<(Node, Node)> = Vec::new();
        for (k, &a) in nodes.iter().enumerate() {
            edges.extend(nodes.iter().skip(k + 1).map(|&b| (a.min(b), a.max(b))));
        }
        edges.sort_unstable();
        let expected = crossing_pairs(&edges);
        assert!(expected.len() > 500, "{}", expected.len());
        let mut meter = Budget::DEFAULT.meter();
        for side in 1..=3 {
            assert_eq!(crossings_in(&edges, side, &mut meter).unwrap().as_ref(), Some(&expected), "{side} buckets each way");
        }
        assert_eq!(crossings(&edges, &mut meter).unwrap(), expected);
    }

    #[test]
    fn edges_too_long_for_their_buckets_are_put_in_larger_ones() {
        // 16 edges fanned out from one corner across the bounds and 4 short ones: 20 edges, so 4 buckets each
        // way at first.
        let mut edges: Vec<(Node, Node)> = (0..16).map(|k| (n(0, 0), n(64, 4 * k))).collect();
        edges.extend((0..4).map(|k| (n(60, 16 * k), n(64, 16 * k + 2))));
        edges.sort_unstable();
        let expected = crossing_pairs(&edges);
        let work_in = |side| spent(|meter| crossings_in(&edges, side, meter).unwrap());
        let ((four, four_work), (two, two_work), (one, one_work)) = (work_in(4), work_in(2), work_in(1));
        // In 4 each way they take more than 4 buckets each, and the search stops as soon as they do.
        assert_eq!((four, four_work), (None, 86));
        assert_eq!((two.as_ref(), one.as_ref()), (Some(&expected), Some(&expected)));
        assert_eq!((two_work, one_work), (363, 210));
        // So the search goes on with 2 each way, and need not go on to 1.
        assert_eq!(spent(|meter| crossings(&edges, meter).unwrap()), (expected, four_work + two_work));
        // Any less work than that runs out, in the first search or in the second.
        for work in 0..four_work + two_work {
            let short = Budget { max_work: work, ..Budget::DEFAULT };
            assert_eq!(crossings(&edges, &mut short.meter()).unwrap_err(), Exhausted::Work, "{work}");
        }
        // 2 edges have 1 bucket from the first: 2 units to put them in it, and 1 to test them.
        let two = [edges[0], edges[1]];
        assert_eq!(spent(|meter| crossings(&two, meter).unwrap()), (vec![], 3));
        for work in 0..3 {
            assert_eq!(crossings(&two, &mut Budget { max_work: work, ..Budget::DEFAULT }.meter()).unwrap_err(), Exhausted::Work, "{work}");
        }
    }

    #[test]
    fn rounding_runs_out_of_work_at_every_step_short_of_what_it_takes() {
        // 2 squares over each other's corners and the bow tie: edges to bend, and crossings to find.
        let square = |x: i64, y: i64| vec![n(x, y), n(x + 8, y), n(x + 8, y + 8), n(x, y + 8), n(x, y)];
        let rings = [square(0, 0), square(4, 4), bow_tie()];
        let (rounded, work) = spent(|meter| round(&rings, meter).unwrap());
        assert!(rounded.is_ok() && work > 100, "{work}");
        for short in 0..work {
            assert_eq!(round(&rings, &mut Budget { max_work: short, ..Budget::DEFAULT }.meter()).unwrap_err(), Exhausted::Work, "{short}");
        }
    }

    #[test]
    fn edges_fit_their_buckets_while_they_take_no_more_than_4_each() {
        // 2 level edges with 4 buckets each way, 4 half cells wide and 3 high: each edge is put in the 4
        // of its row, 8 in all, which is 4 each.
        let edges = [(n(0, 0), n(12, 0)), (n(0, 10), n(12, 10))];
        assert_eq!(spent(|meter| crossings_in(&edges, 4, meter).unwrap()), (Some(vec![]), 8));
        // 2 shorter ones, the buckets 3 half cells each way: 3 columns, and the upper edge lies a half cell
        // from the points of 2 rows. 9 in all, 1 too many.
        let edges = [(n(0, 0), n(8, 0)), (n(0, 8), n(8, 8))];
        assert_eq!(spent(|meter| crossings_in(&edges, 4, meter).unwrap()), (None, 9));
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
