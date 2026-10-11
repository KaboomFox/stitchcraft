//! Travel under the rows (`underpath`, on by default): between rows the needle runs inside the part, under
//! rows not sewn yet, which then hide it (`REQ-FILL-TAT-005`).
//!
//! **The lines travel follows.** As in Ink/Stitch, 3 gratings of lines cross the part: at 45° to the rows
//! either way, 2 mm apart, and square to the rows, √2 mm apart, each anchored at the design's origin as the
//! rows are ([`super::rows`]). A part under 10,000 square CSS pixels (700 mm²) has less room, and its lines
//! lie half as far apart. The square lines pass through every crossing of the diagonal ones, so the 3 make
//! one network: edges between those crossings, and from them to the lines' ends on the part's rings. Along
//! each ring an edge joins each node to the next, among the route's nodes and the lines' ends.
//!
//! **What an edge costs.** Ink/Stitch's costs, in its units, CSS pixels. Along a ring an edge costs 3 times
//! the straight line between its ends. Inside, an edge costs its length over its distance from the part's
//! rings, simplified within 0.5 mm, plus a tenth of a pixel. So travel keeps to the middle of the part and
//! off its rings.
//!
//! **Sewn rows close the lines across them.** Once a row is sewn, the edges that cross it close, and no later
//! travel runs over it: travel runs under rows sewn after it, which hide it.
//!
//! **The way.** The cheapest way between 2 nodes ([`super::travel::search`]), smoothed as Ink/Stitch smooths
//! it, then kept inside the part ([`super::clamp`]). Ink/Stitch travels between 2 rings only from the
//! fill's start to its end, before any row, and where no line joins them, as for a hole too small for any
//! line to reach, it goes straight. StitchCraft also travels between rings to pair nodes
//! (`DEV-FILL-006`), after rows are sewn, so where no line joins 2 nodes its needle runs along the rings
//! and rows instead, as with `underpath` off ([`super::sew`]), and never straight across rows already sewn.
//!
//! Equally cheap ways, the outline the costs measure from and the points a way is cut into before it is
//! smoothed follow StitchCraft's own rules where Ink/Stitch's come from its graph library, GEOS and its
//! running stitch, so the way can differ in its details (`DEV-FILL-007`).

use std::collections::BTreeMap;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::graph::NodeId;
use super::rings::{Place, Rings, stretches};
use super::route::Node;
use super::rows::{Grid, axes, rows};
use super::travel::{search, way_to};
use crate::generators::running::along_line;
use crate::normalize::near::{Snap, apart, key};
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::{Hit, hit};
use crate::normalize::stroke::distance_to_segment;

/// CSS pixels to a millimetre: Ink/Stitch weighs travel in pixels.
const PX_PER_MM: f64 = 1.0 / MM_PER_SVG_PX;
/// A part smaller than this, in square millimetres (10,000 square CSS pixels), gets its lines half as far
/// apart.
const SMALL_PART: f64 = 10_000.0 * MM_PER_SVG_PX * MM_PER_SVG_PX;
/// The 3 gratings: how far each turns from the rows, in degrees, and how far apart its lines are, in
/// millimetres, before a small part halves it. Ink/Stitch turns the square lines by -90°, which lays the
/// same lines.
const GRATINGS: [(f64, f64); 3] = [(45.0, 2.0), (-45.0, 2.0), (90.0, std::f64::consts::SQRT_2)];
/// An edge along a ring costs this many times the straight line between its ends.
const ALONG_RING: f64 = 3.0;
/// The rings that inside edges' distances are measured from are simplified within this, in millimetres.
const SIMPLIFIED: f64 = 0.5;
/// Added to an inside edge's distance from the rings, in pixels.
const OFF_RING: f64 = 0.1;
/// Crossings nearer each other than this, in millimetres, are one: 0.005 px, within which Ink/Stitch snaps
/// the square lines to the diagonal ones.
const SNAP: f64 = 0.005 * MM_PER_SVG_PX;
/// How long, in millimetres, the stitches are that a way is cut into before it is smoothed (10 px), and how
/// far they may stray from it (4 px): Ink/Stitch's, for a smoothness of 2.
const SMOOTH_LENGTH: f64 = 10.0 * MM_PER_SVG_PX;
const SMOOTH_TOLERANCE: f64 = 4.0 * MM_PER_SVG_PX;
/// How many times a way's corners are cut.
const SMOOTHING_ROUNDS: usize = 5;

/// An edge travel may follow: its ends, what it costs, and whether a sewn row has closed it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Edge {
    ends: [NodeId; 2],
    cost: f64,
    open: bool,
}

impl Edge {
    fn other(&self, node: NodeId) -> NodeId {
        if self.ends[0] == node { self.ends[1] } else { self.ends[0] }
    }
}

/// The lines travel may follow across one part, and the rows that close them.
#[derive(Clone, Debug)]
pub(crate) struct Underpath {
    /// Every node: the route's, with their numbers, then the lines' ends on the rings, then their crossings.
    points: Vec<Point>,
    /// Each node's edges, in the order they were added.
    around: Vec<Vec<usize>>,
    edges: Vec<Edge>,
    /// For each row segment, the inside edges that cross it, which sewing it closes.
    closes: Vec<Vec<usize>>,
}

impl Underpath {
    /// The lines travel may follow across `part`, whose rings are `rings`, between the route's `nodes`, for
    /// the fill's row `segments` at `angle` degrees. `None` when a diagonal grating misses the part, which
    /// then travels along its rings, as Ink/Stitch's does. One unit of `meter` for each line, each pair of
    /// lines looked at for a crossing, each edge, each side its distance is measured from, and each row
    /// looked at for crossing an edge.
    pub(crate) fn new(
        part: &Polygon,
        rings: &Rings<'_>,
        nodes: &[Node],
        segments: &[(Point, Point)],
        angle: f64,
        meter: &mut Meter,
    ) -> Result<Option<Underpath>, Exhausted> {
        let scale = if part.area() < SMALL_PART { 0.5 } else { 1.0 };
        let grating = |(turn, spacing): (f64, f64), meter: &mut Meter| -> Result<Vec<(Point, Point)>, Exhausted> {
            let grid = Grid { angle: angle + turn, spacing: scale * spacing, end_spacing: None };
            Ok(rows(part, &grid, meter)?.into_iter().flat_map(|row| row.segments).map(|s| (s.start, s.end)).collect())
        };
        let [first, second, third] = GRATINGS;
        let (diagonal, other, square) = (grating(first, meter)?, grating(second, meter)?, grating(third, meter)?);
        if diagonal.is_empty() || other.is_empty() {
            return Ok(None);
        }
        let mut built = Building::new(nodes);
        // The diagonal lines' ends are where a crossing on a ring snaps to, as Ink/Stitch snaps the square
        // lines to the diagonal ones.
        for (lines, snapped_to) in [(&diagonal, true), (&other, true), (&square, false)] {
            for &(a, b) in lines {
                for end in [a, b] {
                    let id = built.on_ring(end, rings, meter)?;
                    if snapped_to {
                        built.snap_to(id);
                    }
                }
            }
        }
        // Where the diagonal lines cross or touch each other, and where the square lines cross or touch the
        // first of them: a line along a ring is cut where the others end on it.
        let mut cuts: [Vec<Vec<(f64, NodeId)>>; 3] =
            [vec![Vec::new(); diagonal.len()], vec![Vec::new(); other.len()], vec![Vec::new(); square.len()]];
        for (i, &(a, b)) in diagonal.iter().enumerate() {
            for (j, &(c, d)) in other.iter().enumerate() {
                meter.charge(1)?;
                if let Hit::Cross(p) | Hit::Touch(p) = hit(a, b, c, d) {
                    let node = built.crossing(p);
                    push(&mut cuts[0], i, (a.distance(p), node));
                    push(&mut cuts[1], j, (c.distance(p), node));
                }
            }
        }
        for (k, &(e, f)) in square.iter().enumerate() {
            for &(a, b) in &diagonal {
                meter.charge(1)?;
                if let Hit::Cross(p) | Hit::Touch(p) = hit(e, f, a, b) {
                    let node = built.crossing(p);
                    push(&mut cuts[2], k, (e.distance(p), node));
                }
            }
        }
        built.join_rings(meter)?;
        let simplified = part.rings().map(|ring| simplify(ring, SIMPLIFIED, meter)).collect::<Result<Vec<_>, _>>()?;
        let mut inside = Vec::new();
        for (lines, cuts) in [&diagonal, &other, &square].into_iter().zip(&cuts) {
            for (&(a, b), along) in lines.iter().zip(cuts) {
                let mut along = along.clone();
                along.sort_by(|x, y| x.0.total_cmp(&y.0));
                let chain: Vec<NodeId> = std::iter::once(built.node(a))
                    .chain(along.iter().map(|&(_, n)| Some(n)))
                    .chain(std::iter::once(built.node(b)))
                    .flatten()
                    .collect();
                for pair in chain.windows(2) {
                    let &[from, to] = pair else { continue };
                    if from != to {
                        inside.push(built.inside(from, to, &simplified, meter)?);
                    }
                }
            }
        }
        let closes = closing(&built.points, &built.edges, &inside, segments, angle, meter)?;
        Ok(Some(Underpath { points: built.points, around: built.around, edges: built.edges, closes }))
    }

    /// The cheapest way from node `from` to node `to` over the open edges: the nodes' points, from the
    /// first to the last. `None` when no way joins them. One unit of `meter` for each node the search
    /// takes and each step of the way.
    pub(crate) fn way(&self, from: NodeId, to: NodeId, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
        let steps = |node: NodeId| {
            self.around.get(node).into_iter().flatten().filter_map(move |&id| {
                let edge = self.edges.get(id).filter(|edge| edge.open)?;
                Some((edge.other(node), edge.cost, ()))
            })
        };
        let searched = search(self.points.len(), from, Some(to), steps, meter)?;
        let Some(way) = way_to(&searched, from, to, meter)? else { return Ok(None) };
        Ok(Some(std::iter::once(from).chain(way.into_iter().map(|(node, ())| node)).filter_map(|node| self.points.get(node).copied()).collect()))
    }

    /// Row segment `segment` is sewn: the edges that cross it close.
    pub(crate) fn sew(&mut self, segment: usize) {
        for &id in self.closes.get(segment).into_iter().flatten() {
            if let Some(edge) = self.edges.get_mut(id) {
                edge.open = false;
            }
        }
    }
}

/// Adds `cut` to the cuts of line `index`.
fn push(cuts: &mut [Vec<(f64, NodeId)>], index: usize, cut: (f64, NodeId)) {
    if let Some(list) = cuts.get_mut(index) {
        list.push(cut);
    }
}

/// The network as it is built.
struct Building {
    points: Vec<Point>,
    /// Where each node on a ring lies along it; `None` for a crossing inside the part.
    places: Vec<Option<Place>>,
    around: Vec<Vec<usize>>,
    edges: Vec<Edge>,
    /// The nodes on the rings by their exact coordinates, so that a line ending on a node is joined to it.
    on_rings: BTreeMap<(u64, u64), NodeId>,
    /// The nodes a crossing snaps to within `SNAP`: the crossings found so far and the diagonal lines' ends.
    snaps: Snap<NodeId>,
}

impl Building {
    fn new(nodes: &[Node]) -> Building {
        let mut built = Building {
            points: Vec::new(),
            places: Vec::new(),
            around: Vec::new(),
            edges: Vec::new(),
            on_rings: BTreeMap::new(),
            snaps: Snap::new(SNAP),
        };
        for node in nodes {
            let id = built.add(node.point, Some(node.place));
            built.on_rings.entry(key(node.point)).or_insert(id);
        }
        built
    }

    fn add(&mut self, p: Point, place: Option<Place>) -> NodeId {
        self.points.push(p);
        self.places.push(place);
        self.around.push(Vec::new());
        self.points.len() - 1
    }

    /// The node on the rings at `p`, added where it lies on `rings` if new.
    fn on_ring(&mut self, p: Point, rings: &Rings<'_>, meter: &mut Meter) -> Result<NodeId, Exhausted> {
        if let Some(&id) = self.on_rings.get(&key(p)) {
            return Ok(id);
        }
        let place = rings.locate(p, |_| true, meter)?;
        let id = self.add(p, place);
        self.on_rings.insert(key(p), id);
        Ok(id)
    }

    /// The node at `p`, a line's end on the rings; `None` if there is none.
    fn node(&self, p: Point) -> Option<NodeId> {
        self.on_rings.get(&key(p)).copied()
    }

    /// The crossing at `p`: a node it snaps to within `SNAP`, else a new one.
    fn crossing(&mut self, p: Point) -> NodeId {
        let next = self.points.len();
        let id = self.snaps.snap(p, || next);
        if id == next {
            self.add(p, None);
        }
        id
    }

    /// Lets crossings snap to node `id`, unless one it would snap to is there already.
    fn snap_to(&mut self, id: NodeId) {
        if let Some(&p) = self.points.get(id) {
            self.snaps.snap(p, || id);
        }
    }

    /// Adds an edge between `a` and `b` costing `cost`; its number.
    fn link(&mut self, a: NodeId, b: NodeId, cost: f64) -> usize {
        let id = self.edges.len();
        self.edges.push(Edge { ends: [a, b], cost, open: true });
        for node in [a, b] {
            if let Some(list) = self.around.get_mut(node) {
                list.push(id);
            }
        }
        id
    }

    /// Joins each ring's nodes in order along it, each to the next and the last to the first, at 3 times
    /// the straight line between them, in pixels. One unit of `meter` for each node.
    fn join_rings(&mut self, meter: &mut Meter) -> Result<(), Exhausted> {
        let placed = self.places.iter().enumerate().filter_map(|(id, place)| Some((id, (*place)?))).collect();
        for [(a, _), (b, _)] in stretches(placed, meter)? {
            let (Some(&p), Some(&q)) = (self.points.get(a), self.points.get(b)) else { continue };
            self.link(a, b, ALONG_RING * p.distance(q) * PX_PER_MM);
        }
        Ok(())
    }

    /// Adds the inside edge from `a` to `b`, costing its length over its distance from the `simplified`
    /// rings plus a tenth of a pixel, in pixels; its number. One unit of `meter` for each side measured.
    fn inside(&mut self, a: NodeId, b: NodeId, simplified: &[Vec<Point>], meter: &mut Meter) -> Result<usize, Exhausted> {
        let (Some(&p), Some(&q)) = (self.points.get(a), self.points.get(b)) else { return Ok(self.link(a, b, f64::INFINITY)) };
        let mut nearest = f64::INFINITY;
        for ring in simplified {
            meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
            for side in ring.windows(2) {
                let &[c, d] = side else { continue };
                nearest = nearest.min(apart(p, q, c, d));
            }
        }
        Ok(self.link(a, b, p.distance(q) * PX_PER_MM / (nearest * PX_PER_MM + OFF_RING)))
    }
}

/// For each of the fill's row `segments` at `angle`, the `inside` edges that cross it. A row crosses an edge
/// only where its line passes strictly between the edge's ends, so the rows looked at for each edge are the
/// ones across the rows between its ends. One unit of `meter` for each row looked at.
fn closing(
    points: &[Point],
    edges: &[Edge],
    inside: &[usize],
    segments: &[(Point, Point)],
    angle: f64,
    meter: &mut Meter,
) -> Result<Vec<Vec<usize>>, Exhausted> {
    let (_, across) = axes(angle);
    let height = |p: Point| p.x() * across.0 + p.y() * across.1;
    let mut closes: Vec<Vec<usize>> = vec![Vec::new(); segments.len()];
    let mut by_height: Vec<(f64, usize)> = segments.iter().enumerate().map(|(i, &(start, _))| (height(start), i)).collect();
    by_height.sort_by(|a, b| a.0.total_cmp(&b.0));
    for &id in inside {
        let Some(edge) = edges.get(id) else { continue };
        let (Some(&p), Some(&q)) = (points.get(edge.ends[0]), points.get(edge.ends[1])) else { continue };
        let (low, high) = (height(p).min(height(q)), height(p).max(height(q)));
        let first = by_height.partition_point(|&(h, _)| h <= low);
        for &(h, segment) in by_height.get(first..).unwrap_or_default() {
            if h >= high {
                break;
            }
            meter.charge(1)?;
            let (Some(&(a, b)), Some(list)) = (segments.get(segment), closes.get_mut(segment)) else { continue };
            if let Hit::Cross(_) = hit(p, q, a, b) {
                list.push(id);
            }
        }
    }
    Ok(closes)
}

/// `ring` simplified by Douglas and Peucker's rule within `tolerance`: its first and last points stay, and
/// between 2 that stay, the point farthest from the line between them, the first of equally far ones, stays
/// if it lies farther than `tolerance` from it, and so on. One unit of `meter` for each point looked at.
fn simplify(ring: &[Point], tolerance: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let last = ring.len().saturating_sub(1);
    let mut keep = vec![false; ring.len()];
    for end in [0, last] {
        if let Some(slot) = keep.get_mut(end) {
            *slot = true;
        }
    }
    let mut spans = vec![(0, last)];
    while let Some((i, j)) = spans.pop() {
        let (Some(&a), Some(&b)) = (ring.get(i), ring.get(j)) else { continue };
        let mut farthest: Option<(usize, f64)> = None;
        for (k, &p) in ring.iter().enumerate().take(j).skip(i + 1) {
            meter.charge(1)?;
            let d = distance_to_segment(p, a, b);
            if farthest.is_none_or(|(_, far)| d > far) {
                farthest = Some((k, d));
            }
        }
        if let Some((k, _)) = farthest.filter(|&(_, d)| d > tolerance) {
            if let Some(slot) = keep.get_mut(k) {
                *slot = true;
            }
            spans.push((i, k));
            spans.push((k, j));
        }
    }
    Ok(ring.iter().zip(&keep).filter(|(_, kept)| **kept).map(|(p, _)| *p).collect())
}

/// `way` smoothed as Ink/Stitch smooths travel: cut into stitches of 10 pixels within 4 of it, whose
/// needle points after the first have their corners cut 5 times (a quarter of the way along each side from
/// each end, Chaikin's rule), and its own ends added back at either end. So the way leaves its start
/// straight, to the first point after it, and curves from there. One unit of `meter` for each point made.
pub(crate) fn smooth(way: &[Point], min_stitch: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let (Some(&first), Some(&last)) = (way.first(), way.last()) else { return Ok(Vec::new()) };
    let mut points: Vec<Point> = along_line(way, SMOOTH_LENGTH, SMOOTH_TOLERANCE, min_stitch, meter)?.into_iter().skip(1).collect();
    for _ in 0..SMOOTHING_ROUNDS {
        let mut cut = Vec::with_capacity(2 * points.len());
        cut.extend(points.first().copied());
        for pair in points.windows(2) {
            let &[p, q] = pair else { continue };
            meter.charge(2)?;
            cut.push(p.lerp(q, 0.25));
            cut.push(p.lerp(q, 0.75));
        }
        cut.extend(points.last().copied());
        points = cut;
    }
    Ok(std::iter::once(first).chain(points).chain(std::iter::once(last)).collect())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::math::sin_cos;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p, rectangle};
    use crate::normalize::stroke::distance_to_segment;

    fn underpath(part: &Polygon, segments: &[(Point, Point)], angle: f64) -> Option<Underpath> {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(part, meter).unwrap();
        Underpath::new(part, &rings, &[], segments, angle, meter).unwrap()
    }

    #[test]
    fn the_lines_cross_the_part_in_3_gratings_joined_where_they_cross() {
        // A 10 × 4 rectangle is a small part: its lines lie 1 mm and √2/2 mm apart. The diagonal lines at
        // 45° and 135° cross every 1 mm along each; the upright lines pass through those crossings.
        let part = rectangle(4.0);
        let u = underpath(&part, &[], 0.0).unwrap();
        let inside = |p: Point| p.x() > 1e-9 && p.x() < 10.0 - 1e-9 && p.y() > 1e-9 && p.y() < 4.0 - 1e-9;
        let crossings: Vec<Point> = u.points.iter().copied().filter(|&q| inside(q)).collect();
        assert!(!crossings.is_empty());
        // Every crossing has 6 edges, 2 along each grating, unless it lies by the rings.
        let away = |q: Point| q.x() > 1.0 && q.x() < 9.0 && q.y() > 1.0 && q.y() < 3.0;
        for (id, &q) in u.points.iter().enumerate() {
            if inside(q) && away(q) {
                assert_eq!(u.around[id].len(), 6, "{q:?}");
            }
        }
        // Each crossing lies on lines of all 3 gratings: the upright ones run through them, 1 mm/√2 apart.
        let spacing = std::f64::consts::SQRT_2 / 2.0;
        for q in &crossings {
            let k = q.x() / spacing;
            assert!((k - k.round()).abs() < 1e-6, "{q:?}");
        }
        // Each edge joins 2 neighbouring points of its line or ring: no other point lies on it. 2 lines that
        // end at one point of a ring end there worked out apart, as 2 nodes a rounding error apart.
        for edge in &u.edges {
            let (a, b) = (u.points[edge.ends[0]], u.points[edge.ends[1]]);
            for &q in &u.points {
                if q.distance(a) > 1e-9 && q.distance(b) > 1e-9 {
                    assert!(distance_to_segment(q, a, b) > 1e-9, "{q:?} on the edge from {a:?} to {b:?}");
                }
            }
        }
    }

    #[test]
    fn an_edge_costs_less_the_farther_it_lies_from_the_rings_and_3_times_its_length_along_them() {
        let part = rectangle(4.0);
        let u = underpath(&part, &[], 0.0).unwrap();
        let on_rings = |q: Point| q.x().abs() < 1e-9 || (q.x() - 10.0).abs() < 1e-9 || q.y().abs() < 1e-9 || (q.y() - 4.0).abs() < 1e-9;
        let (mut along_rings, mut inside) = (0, 0);
        for edge in &u.edges {
            let (a, b) = (u.points[edge.ends[0]], u.points[edge.ends[1]]);
            let length = a.distance(b) * PX_PER_MM;
            // Inside a rectangle an edge lies nearest its sides at one of its ends.
            let d = [a, b].iter().flat_map(|q| [q.x(), 10.0 - q.x(), q.y(), 4.0 - q.y()]).fold(f64::INFINITY, f64::min);
            if (edge.cost - 3.0 * length).abs() < 1e-9 * length {
                along_rings += 1;
                assert!(on_rings(a) && on_rings(b));
            } else {
                inside += 1;
                assert!((edge.cost - length / (d * PX_PER_MM + 0.1)).abs() < 1e-9 * edge.cost, "{a:?} {b:?}");
            }
        }
        // A ring joins each of its nodes to the next: as many edges along it as nodes on it.
        assert_eq!(along_rings, u.points.iter().filter(|&&q| on_rings(q)).count());
        assert!(inside > along_rings);
    }

    #[test]
    fn a_sewn_row_closes_the_edges_across_it() {
        // A row across the middle: once sewn, no way crosses it, and the way between its 2 sides goes
        // round its ends along the rings.
        let part = rectangle(4.0);
        let row = (p(0.0, 2.0), p(10.0, 2.0));
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let nodes: Vec<Node> = [row.0, row.1].iter().map(|&q| Node { point: q, place: rings.locate(q, |_| true, meter).unwrap().unwrap() }).collect();
        let mut u = Underpath::new(&part, &rings, &nodes, &[row], 0.0, meter).unwrap().unwrap();
        let below = u.points.iter().position(|q| q.y() > 2.5 && q.y() < 4.0 && q.x() > 4.0 && q.x() < 6.0).unwrap();
        let above = u.points.iter().position(|q| q.y() < 1.5 && q.y() > 0.0 && q.x() > 4.0 && q.x() < 6.0).unwrap();
        let crosses = |way: &[Point]| way.windows(2).any(|w| matches!(hit(w[0], w[1], row.0, row.1), Hit::Cross(_)));
        let open = u.way(above, below, meter).unwrap().unwrap();
        assert!(crosses(&open), "straight across while the row is not sewn: {open:?}");
        u.sew(0);
        let closed = u.way(above, below, meter).unwrap().unwrap();
        assert!(!crosses(&closed), "{closed:?}");
        // A segment the fill does not have closes nothing.
        u.sew(7);
        assert_eq!(u.way(above, below, meter).unwrap(), Some(closed));
    }

    #[test]
    fn a_part_under_700_mm2_gets_its_lines_half_as_far_apart() {
        // 10,000 square CSS pixels is 700.03 mm²: a square 26.45 mm across is under it, and one 26.47 mm
        // across over it. Crossings of the 45° lines lie 1 mm apart in the first and 2 mm in the second.
        let square = |side: f64| Polygon { outline: vec![p(0.0, 0.0), p(0.0, side), p(side, side), p(side, 0.0), p(0.0, 0.0)], holes: Vec::new() };
        let closest = |part: &Polygon| {
            let u = underpath(part, &[], 0.0).unwrap();
            let middle: Vec<Point> = u.points.iter().copied().filter(|q| (8.0..12.0).contains(&q.x()) && (8.0..12.0).contains(&q.y())).collect();
            let mut closest = f64::INFINITY;
            for (i, a) in middle.iter().enumerate() {
                for b in &middle[i + 1..] {
                    closest = closest.min(a.distance(*b));
                }
            }
            closest
        };
        assert!((closest(&square(26.45)) - 1.0).abs() < 1e-9);
        assert!((closest(&square(26.47)) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn a_part_the_diagonal_lines_miss_has_none() {
        // 0.2 mm tall: no diagonal line 1 mm apart need meet it in a segment, and these do not.
        let sliver = Polygon { outline: vec![p(0.3, 0.3), p(0.3, 0.5), p(0.6, 0.5), p(0.6, 0.3), p(0.3, 0.3)], holes: Vec::new() };
        assert!(underpath(&sliver, &[], 0.0).is_none());
        assert!(underpath(&frame(), &[], 0.0).is_some());
    }

    #[test]
    fn a_ring_is_simplified_by_douglas_and_peucker_s_rule() {
        let meter = &mut Budget::DEFAULT.meter();
        // A bump of 0.4 on a 10 long side stays out of a simplification within 0.5, as does one of exactly 0.5,
        // and one of 0.6 stays in.
        let ring = |bump: f64| vec![p(0.0, 0.0), p(5.0, -bump), p(10.0, 0.0), p(10.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)];
        for bump in [0.4, 0.5] {
            assert_eq!(simplify(&ring(bump), 0.5, meter).unwrap(), [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]);
        }
        assert_eq!(simplify(&ring(0.6), 0.5, meter).unwrap(), ring(0.6));
        assert_eq!(simplify(&[p(1.0, 1.0)], 0.5, meter).unwrap(), [p(1.0, 1.0)]);
        // Of 2 points as far from the line, 1 from the side from (0, 0) to (10, 0), the first stays, and the
        // second then lies 0.28 from the line through it.
        let tie = [p(0.0, 0.0), p(3.0, 1.0), p(5.0, 1.0), p(10.0, 0.0), p(10.0, -5.0), p(0.0, 0.0)];
        assert_eq!(simplify(&tie, 0.5, meter).unwrap(), [p(0.0, 0.0), p(3.0, 1.0), p(10.0, 0.0), p(10.0, -5.0), p(0.0, 0.0)]);
        // Each point between 2 that stay is looked at once for each span it lies in: 4, then 1, 2 and 1.
        let mut counted = Budget::DEFAULT.meter();
        simplify(&ring(0.6), 0.5, &mut counted).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - counted.work_left(), 8);
    }

    #[test]
    fn a_way_is_smoothed_by_cutting_its_corners_keeping_its_ends() {
        let meter = &mut Budget::DEFAULT.meter();
        let way = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0)];
        let smoothed = smooth(&way, 0.1, meter).unwrap();
        assert_eq!((smoothed.first(), smoothed.last()), (Some(&p(0.0, 0.0)), Some(&p(4.0, 4.0))));
        // Cut into stitches of 10 px (2.6 mm), the first side takes 2 of 2 mm: the way leaves its start
        // straight to the first needle point after it, and curves from there.
        assert_eq!(smoothed.get(1), Some(&p(2.0, 0.0)));
        // The corner is cut: no point comes within 0.2 mm of it, and the points stay within the way's box.
        assert!(smoothed.iter().all(|q| q.distance(p(4.0, 0.0)) > 0.2), "{smoothed:?}");
        assert!(smoothed.iter().all(|q| q.x() >= -1e-9 && q.x() <= 4.0 + 1e-9 && q.y() >= -1e-9 && q.y() <= 4.0 + 1e-9));
        assert!(smooth(&[], 0.1, meter).unwrap().is_empty());
    }

    #[test]
    fn a_way_is_cut_within_4_px_of_it_before_it_is_smoothed() {
        // A hairpin 0.34 mm wide. Stitches of 10 px across its turn would stray 1.15 mm from it, more than
        // 4 px (1.06 mm), so a point goes on the turn, and the smoothed way passes within 0.3 mm of it.
        let meter = &mut Budget::DEFAULT.meter();
        let r = 0.169;
        let mut way = vec![p(0.0, 3.4845)];
        for i in 0..=12 {
            let (sin, cos) = sin_cos(std::f64::consts::PI * f64::from(i) / 12.0);
            way.push(p(r - r * cos, -r * sin));
        }
        way.push(p(2.0 * r, 3.4845));
        let turn = p(r, -r);
        let nearest = smooth(&way, 0.3, meter).unwrap().iter().map(|q| q.distance(turn)).fold(f64::INFINITY, f64::min);
        assert!(nearest < 0.5, "{nearest}");
    }

    #[test]
    fn crossings_within_0_005_px_are_one_node() {
        // 0.005 px is 0.0013 mm: 0.001 mm away is the same crossing, and 0.002 mm away another.
        let mut built = Building::new(&[]);
        let first = built.crossing(p(1.0, 1.0));
        assert_eq!(built.crossing(p(1.001, 1.0)), first);
        assert_ne!(built.crossing(p(1.0, 1.002)), first);
        // Either side of where the grid the crossings are found by starts a new cell, found from the other
        // side, in x and in y.
        let edge = 1000.0 * SNAP;
        for (base, found, near) in [(3.0, 1e-7, -1e-7), (4.0, -1e-7, 1e-7)] {
            let across = built.crossing(p(edge + found, base));
            assert_eq!(built.crossing(p(edge + near, base)), across);
            let down = built.crossing(p(base + 2.0, edge + found));
            assert_eq!(built.crossing(p(base + 2.0, edge + near)), down);
        }
    }

    #[test]
    fn building_and_searching_are_charged_to_the_budget() {
        let part = rectangle(4.0);
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let mut used = Budget::DEFAULT.meter();
        Underpath::new(&part, &rings, &[], &[], 0.0, &mut used).unwrap().unwrap();
        let work = Budget::DEFAULT.max_work - used.work_left();
        for short in [1, work / 2, work - 1] {
            assert!(Underpath::new(&part, &rings, &[], &[], 0.0, &mut Budget { max_stitches: 1, max_work: short }.meter()).is_err());
        }
    }
}
