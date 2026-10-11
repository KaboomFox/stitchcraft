//! The order a part's rows are sewn in, each sewn once, and the needle's travel between them
//! (`REQ-FILL-TAT-001`), as Ink/Stitch routes them (`docs/src/design/algorithms/fills.md` › Routing).
//!
//! **The graph.** Each row segment is an edge between its 2 ends, which lie on the part's rings
//! ([`super::rings`]). Along each ring, an edge joins each node to the next one, and every other one of
//! those stretches is doubled. A node then meets its row and 3 stretches of ring, so every node has an
//! even number of edges, and one walk can take every edge once and come back to where it started: an
//! Euler circuit.
//!
//! **Start and end.** The fill starts at the point of its rings nearest the needle and ends at the point
//! nearest the next element. Each gets a node of its own there, which splits the stretch of that ring
//! whose chord passes nearest it. Without a needle the fill starts at its first row's start, and without a
//! next element it ends where it started. Only rings that rows reach are looked at (`DEV-FILL-006`).
//!
//! **Even degrees.** A row through a corner of the outline, or one along it, shares a node with the row
//! beside it, and a ring with 2 nodes doubles only one of its 2 stretches, so some nodes have an odd
//! number of edges. Ink/Stitch's graph library then adds paths between the odd nodes, and a row on such a
//! path is sewn a second time. StitchCraft pairs the odd nodes so that the travel between pairs is
//! shortest in all, along a ring or across the part along rows and rings, and joins each pair with one
//! edge. No row is sewn twice (`DEV-FILL-006`).
//!
//! **The walk.** The walk starts at the fill's end. At each node it takes the node's row if one is left,
//! and otherwise the node's first edge left, in the order [`super::graph`] keeps them. When it reaches a
//! node with no edge left it steps back, and the edges, taken in the order it steps back over them, are
//! the route. Travel then mostly comes just before the rows beside it, which cover it, and the rows go
//! back and forth like a mown lawn. The route comes back to the fill's end, so the fill first travels
//! from its start to its end and then sews it. Steps of travel in a row are one stretch of travel.
//!
//! The route follows Ink/Stitch's from the same graph. The rings start where the drawing starts them,
//! which can double other stretches than Ink/Stitch does and so change the route (`DEV-FILL-005`).
//! Rows that no walk reaches, which only rings that touch can cut off, are sewn after the rest, the needle
//! jumping to them.

use std::cmp::Ordering;
use std::collections::BTreeMap;

use stitchcraft_core::{Exhausted, Meter, Point};

use super::graph::{Edge, EdgeId, Graph, Kind, NodeId};
use super::rings::{Place, Rings, distance_to_side};
use super::travel::Network;

/// The most odd nodes whose pairing is searched in full; more are paired greedily, nearest first.
const PAIRED_IN_FULL: usize = 16;

/// A node of the route: a row's end, or the fill's start or end, and where on the rings it lies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Node {
    /// Where it is.
    pub point: Point,
    /// Its ring, and how far along it.
    pub place: Place,
}

/// One step of the route.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    /// Sew row segment `segment` from node `from` to node `to`.
    Row {
        /// The segment, by its place in the order the rows are taken.
        segment: usize,
        /// The end sewn from.
        from: NodeId,
        /// The end sewn to.
        to: NodeId,
    },
    /// Travel from node `from` to node `to`.
    Travel {
        /// Where it leaves.
        from: NodeId,
        /// Where it arrives.
        to: NodeId,
        /// Whether the 2 nodes share an edge, as the ends of neighbouring rows do.
        beside: bool,
    },
}

/// The route through a part's rows.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Route {
    /// The nodes the steps name.
    pub nodes: Vec<Node>,
    /// The steps, in sewing order.
    pub steps: Vec<Step>,
}

/// The route through `segments`, given in the order the rows are taken, each from its start to its end,
/// on `rings`: from the point nearest `start` to the point nearest `end`. `None` when there is no
/// segment. One unit of `meter` for each segment, node, edge looked at and step.
pub(crate) fn route(
    rings: &Rings<'_>,
    segments: &[(Point, Point)],
    start: Option<Point>,
    end: Option<Point>,
    meter: &mut Meter,
) -> Result<Option<Route>, Exhausted> {
    let mut built = Built::new(rings, segments, meter)?;
    let Some(first) = built.nodes.first().map(|n| n.point) else { return Ok(None) };
    built.join_rings();
    for p in [start, end].into_iter().flatten() {
        built.insert(rings, p, meter)?;
    }
    built.pair_odd_nodes(rings, segments, meter)?;
    // The nodes nearest the start and the end, which are almost always those just added there.
    let starting = nearest(&built.nodes, start.unwrap_or(first), meter)?.unwrap_or(0);
    let ending = match end {
        Some(p) => nearest(&built.nodes, p, meter)?.unwrap_or(starting),
        None => starting,
    };
    let mut walked = built.graph.clone();
    let mut raw = Vec::new();
    if starting != ending {
        raw.push(Step::Travel { from: starting, to: ending, beside: false });
    }
    let mut at = ending;
    loop {
        raw.extend(walk(&mut walked, at, meter)?);
        // Rows a walk did not reach: walk again from the first one left.
        let left = walked.all().into_iter().find_map(|(_, id)| walked.edge(id).filter(|e| matches!(e.kind, Kind::Row(_))));
        let Some(Edge { ends: [next, _], .. }) = left else { break };
        raw.push(Step::Travel { from: at, to: next, beside: false });
        at = next;
    }
    Ok(Some(Route { steps: collapse(&built.graph, raw, meter)?, nodes: built.nodes }))
}

/// The graph as it is built, with its nodes found by where they are.
struct Built {
    nodes: Vec<Node>,
    index: BTreeMap<(u64, u64), NodeId>,
    graph: Graph,
}

/// A point as a key: 2 points are one node when their coordinates are equal, -0 and 0 alike.
fn key(p: Point) -> (u64, u64) {
    ((p.x() + 0.0).to_bits(), (p.y() + 0.0).to_bits())
}

impl Built {
    /// A node at each end of `segments` on `rings`, and an edge for each segment. One unit of `meter` for
    /// each segment, and for each side of a ring a new node is looked for on.
    fn new(rings: &Rings<'_>, segments: &[(Point, Point)], meter: &mut Meter) -> Result<Built, Exhausted> {
        let mut built = Built { nodes: Vec::new(), index: BTreeMap::new(), graph: Graph::default() };
        for (i, &(a, b)) in segments.iter().enumerate() {
            meter.charge(1)?;
            let (a, b) = (built.node(rings, a, meter)?, built.node(rings, b, meter)?);
            built.graph.add(a, b, Kind::Row(i));
        }
        Ok(built)
    }

    /// The node at `p`, added where it lies on `rings` if new.
    fn node(&mut self, rings: &Rings<'_>, p: Point, meter: &mut Meter) -> Result<NodeId, Exhausted> {
        if let Some(&id) = self.index.get(&key(p)) {
            return Ok(id);
        }
        let place = rings.locate(p, |_| true, meter)?.unwrap_or(Place { ring: 0, at: 0.0 });
        Ok(self.add(p, place))
    }

    /// Adds a node at `p`, at `place`.
    fn add(&mut self, p: Point, place: Place) -> NodeId {
        let id = self.graph.add_node();
        self.index.insert(key(p), id);
        self.nodes.push(Node { point: p, place });
        id
    }

    fn place(&self, node: NodeId) -> Place {
        self.nodes.get(node).map_or(Place { ring: 0, at: 0.0 }, |n| n.place)
    }

    fn point(&self, node: NodeId) -> Point {
        self.nodes.get(node).map_or(Point::ORIGIN, |n| n.point)
    }

    /// Joins each ring's nodes in order along it, and doubles every other stretch, from the first.
    fn join_rings(&mut self) {
        let places: Vec<Place> = self.nodes.iter().map(|n| n.place).collect();
        let place = |n: &NodeId| places.get(*n).copied().unwrap_or(Place { ring: 0, at: 0.0 });
        let mut order: Vec<NodeId> = (0..places.len()).collect();
        order.sort_by(|a, b| place(a).ring.cmp(&place(b).ring).then(place(a).at.total_cmp(&place(b).at)));
        for ring in order.chunk_by(|a, b| place(a).ring == place(b).ring) {
            for (i, &a) in ring.iter().enumerate() {
                let Some(&b) = ring.get((i + 1) % ring.len()) else { continue };
                self.graph.add(a, b, Kind::Outline);
                if i % 2 == 0 {
                    self.graph.add(a, b, Kind::Extra);
                }
            }
        }
    }

    /// A node of its own at the point of the rings nearest `p`, splitting the stretch of that ring whose
    /// chord passes nearest; the node already there if there is one. Only rings with nodes are looked at.
    fn insert(&mut self, rings: &Rings<'_>, p: Point, meter: &mut Meter) -> Result<Option<NodeId>, Exhausted> {
        let reached: Vec<bool> = (0..rings.count()).map(|r| self.nodes.iter().any(|n| n.place.ring == r)).collect();
        let Some(place) = rings.locate(p, |r| reached.get(r).copied().unwrap_or(false), meter)? else { return Ok(None) };
        let Some(q) = rings.point(place) else { return Ok(None) };
        if let Some(&id) = self.index.get(&key(q)) {
            return Ok(Some(id));
        }
        let mut best: Option<(f64, EdgeId, NodeId, NodeId)> = None;
        for (from, id) in self.graph.all() {
            meter.charge(1)?;
            let Some(edge) = self.graph.edge(id).filter(|e| e.kind == Kind::Outline) else { continue };
            if self.place(from).ring != place.ring {
                continue;
            }
            let to = edge.other(from);
            let d = distance_to_side(q, self.point(from), self.point(to));
            if best.is_none_or(|(b, ..)| d < b) {
                best = Some((d, id, from, to));
            }
        }
        let Some((_, id, from, to)) = best else { return Ok(None) };
        let node = self.add(q, place);
        self.graph.remove(id);
        self.graph.add(from, node, Kind::Outline);
        self.graph.add(node, to, Kind::Outline);
        Ok(Some(node))
    }

    /// Joins the nodes with an odd number of edges in pairs, so that the travel between pairs is shortest in
    /// all: along their ring when 2 share one, and otherwise along rows and rings.
    fn pair_odd_nodes(&mut self, rings: &Rings<'_>, segments: &[(Point, Point)], meter: &mut Meter) -> Result<(), Exhausted> {
        let odd: Vec<NodeId> = (0..self.nodes.len()).filter(|&n| self.graph.degree(n) % 2 == 1).collect();
        if odd.len() < 2 {
            return Ok(());
        }
        let network = Network::new(&self.nodes, rings, &self.rows(segments.len()), meter)?;
        let mut cost = Vec::with_capacity(odd.len());
        for &a in &odd {
            let across = network.distances(a, meter)?;
            cost.push(
                odd.iter()
                    .map(|&b| {
                        let (pa, pb) = (self.place(a), self.place(b));
                        if pa.ring == pb.ring {
                            rings.way_length(pa.ring, pa.at, pb.at, rings.forwards(pa.ring, pa.at, pb.at))
                        } else {
                            across.get(b).copied().unwrap_or(f64::INFINITY)
                        }
                    })
                    .collect::<Vec<f64>>(),
            );
        }
        for (i, j) in matching(&cost, meter)? {
            let (Some(&a), Some(&b)) = (odd.get(i), odd.get(j)) else { continue };
            if cost.get(i).and_then(|row| row.get(j)).is_some_and(|c| c.is_finite()) {
                self.graph.add(a, b, Kind::Paired);
            }
        }
        Ok(())
    }

    /// Each row's 2 nodes, by the row's place.
    fn rows(&self, count: usize) -> Vec<[NodeId; 2]> {
        let mut rows = vec![[0, 0]; count];
        for (_, id) in self.graph.all() {
            if let Some(Edge { ends, kind: Kind::Row(i) }) = self.graph.edge(id)
                && let Some(slot) = rows.get_mut(i)
            {
                *slot = ends;
            }
        }
        rows
    }
}

/// The node nearest `p`, the first on a tie. One unit of `meter` for each node.
fn nearest(nodes: &[Node], p: Point, meter: &mut Meter) -> Result<Option<NodeId>, Exhausted> {
    meter.charge(u64::try_from(nodes.len()).unwrap_or(u64::MAX))?;
    Ok(nodes.iter().enumerate().min_by(|(_, a), (_, b)| a.point.distance(p).total_cmp(&b.point.distance(p))).map(|(id, _)| id))
}

/// The pairs of `cost`'s rows that make the least total, each row in one pair, the first found on a tie.
/// Up to [`PAIRED_IN_FULL`] rows every pairing is weighed; past that each row in turn pairs with the
/// nearest row left. One unit of `meter` for each row of each set of rows weighed, or past
/// [`PAIRED_IN_FULL`] rows, for each row a row is weighed against.
fn matching(cost: &[Vec<f64>], meter: &mut Meter) -> Result<Vec<(usize, usize)>, Exhausted> {
    let n = cost.len();
    let c = |i: usize, j: usize| cost.get(i).and_then(|row| row.get(j)).copied().unwrap_or(f64::INFINITY);
    let mut pairs = Vec::new();
    if n > PAIRED_IN_FULL {
        let mut left: Vec<usize> = (0..n).collect();
        while let Some((&i, rest)) = left.split_first() {
            meter.charge(u64::try_from(rest.len()).unwrap_or(u64::MAX))?;
            let Some(&j) = rest.iter().min_by(|&&a, &&b| c(i, a).total_cmp(&c(i, b))) else { break };
            pairs.push((i, j));
            left.retain(|&k| k != i && k != j);
        }
        return Ok(pairs);
    }
    // best[mask]: the least total pairing the rows in `mask`, and the row the lowest one pairs with.
    let full = (1usize << n) - 1;
    let mut best: Vec<Option<(f64, usize)>> = vec![None; full + 1];
    if let Some(slot) = best.get_mut(0) {
        *slot = Some((0.0, 0));
    }
    for mask in 1..=full {
        // A set of odd size has no pairing.
        if mask.count_ones() % 2 == 1 {
            continue;
        }
        meter.charge(u64::from(mask.count_ones()))?;
        // The set's lowest row pairs with another. A row outside the set leaves a set of odd size, which
        // has no pairing, so it is passed over.
        let i = mask.trailing_zeros() as usize;
        let mut found: Option<(f64, usize)> = None;
        for j in (i + 1)..n {
            let Some(Some((rest, _))) = best.get(mask & !(1 << i) & !(1 << j)) else { continue };
            let total = rest + c(i, j);
            if found.is_none_or(|(f, _)| total.total_cmp(&f) == Ordering::Less) {
                found = Some((total, j));
            }
        }
        if let Some(slot) = best.get_mut(mask) {
            *slot = found;
        }
    }
    let mut mask = full;
    while mask != 0 {
        let i = mask.trailing_zeros() as usize;
        let Some(Some((_, j))) = best.get(mask).copied() else { break };
        pairs.push((i, j));
        mask &= !(1 << i) & !(1 << j);
    }
    Ok(pairs)
}

/// The walk that takes every edge of `graph` it can reach from `from`, removing them, as steps: rows, and
/// travel between them. At each node the walk takes a row if one is left and otherwise the first edge
/// left. When it reaches a node with no edge left it steps back over the edge it came by, and that edge
/// is the next step, from the node it led to back to the node it left. On an Euler circuit each step
/// starts where the one before it ended.
fn walk(graph: &mut Graph, from: NodeId, meter: &mut Meter) -> Result<Vec<Step>, Exhausted> {
    // The walk so far: each node, with the edge that led to it from the node before.
    let mut stack: Vec<(NodeId, Option<Edge>)> = vec![(from, None)];
    let mut steps = Vec::new();
    while let Some(&(current, came)) = stack.last() {
        meter.charge(1)?;
        let next = graph
            .edges_at(current)
            .find(|&e| graph.edge(e).is_some_and(|e| matches!(e.kind, Kind::Row(_))))
            .or_else(|| graph.edges_at(current).next());
        match next.and_then(|id| Some((id, graph.edge(id)?))) {
            Some((id, edge)) => {
                stack.push((edge.other(current), Some(edge)));
                graph.remove(id);
            }
            None => {
                stack.pop();
                if let (Some(edge), Some(&(before, _))) = (came, stack.last()) {
                    steps.push(match edge.kind {
                        Kind::Row(segment) => Step::Row { segment, from: current, to: before },
                        Kind::Outline | Kind::Extra | Kind::Paired => Step::Travel { from: current, to: before, beside: false },
                    });
                }
            }
        }
    }
    Ok(steps)
}

/// `raw` with each stretch of travel steps made one, from where the stretch leaves to where the next row
/// starts, or to where the last step arrives. `beside` when `graph` has an edge between the 2.
fn collapse(graph: &Graph, raw: Vec<Step>, meter: &mut Meter) -> Result<Vec<Step>, Exhausted> {
    let mut steps = Vec::with_capacity(raw.len());
    let mut leaves: Option<NodeId> = None;
    let mut arrives: Option<NodeId> = None;
    for step in raw {
        meter.charge(1)?;
        match step {
            Step::Row { from, .. } => {
                if let Some(a) = leaves.take() {
                    steps.push(Step::Travel { from: a, to: from, beside: graph.joined(a, from) });
                }
                steps.push(step);
            }
            Step::Travel { from, to, .. } => {
                leaves.get_or_insert(from);
                arrives = Some(to);
            }
        }
    }
    if let (Some(a), Some(b)) = (leaves, arrives)
        && a != b
    {
        steps.push(Step::Travel { from: a, to: b, beside: false });
    }
    Ok(steps)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::rows::{Grid, rows};
    use crate::generators::tatami::travel::between;
    use crate::normalize::fixture::cases;
    use crate::normalize::region::Polygon;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A 10 × 3 rectangle as the region gives it: from its top left corner, down the left side first.
    fn rectangle() -> Polygon {
        Polygon { outline: vec![p(0.0, 0.0), p(0.0, 3.0), p(10.0, 3.0), p(10.0, 0.0), p(0.0, 0.0)], holes: Vec::new() }
    }

    /// Rows across the rectangle at y = 1 and 2, each from left to right. Their ends A (0, 1), B (10, 1),
    /// C (0, 2) and D (10, 2) are nodes 0 to 3, and lie 1, 15, 2 and 14 along the outline.
    fn two_rows() -> [(Point, Point); 2] {
        [(p(0.0, 1.0), p(10.0, 1.0)), (p(0.0, 2.0), p(10.0, 2.0))]
    }

    fn rings_of(part: &Polygon) -> Rings<'_> {
        Rings::new(part, &mut Budget::DEFAULT.meter()).unwrap()
    }

    /// The graph of `segments` on `rings`, its rings joined.
    fn joined(rings: &Rings<'_>, segments: &[(Point, Point)]) -> Built {
        let mut built = Built::new(rings, segments, &mut Budget::DEFAULT.meter()).unwrap();
        built.join_rings();
        built
    }

    /// The kinds of the edges between `a` and `b`, in the order `a` lists them.
    fn kinds(graph: &Graph, a: NodeId, b: NodeId) -> Vec<Kind> {
        graph.edges_at(a).filter_map(|e| graph.edge(e)).filter(|e| e.other(a) == b).map(|e| e.kind).collect()
    }

    fn routed(part: &Polygon, segments: &[(Point, Point)], start: Option<Point>, end: Option<Point>) -> Route {
        route(&rings_of(part), segments, start, end, &mut Budget::DEFAULT.meter()).unwrap().unwrap()
    }

    #[test]
    fn two_rows_go_the_way_ink_stitch_s_rules_lead() {
        // The outline joins A–C (doubled), C–D, D–B (doubled) and B–A. Walked by hand from A: row A–B, B–A,
        // A–C, row C–D, D–C, the second C–A, then back at C, D–B and the second B–D. Stepped back over, and
        // the travel between rows made one:
        let route = routed(&rectangle(), &two_rows(), None, None);
        let [a, b, c, d] = [0, 1, 2, 3];
        assert_eq!(
            route.nodes.iter().map(|n| (n.point, n.place.at)).collect::<Vec<_>>(),
            [(p(0.0, 1.0), 1.0), (p(10.0, 1.0), 15.0), (p(0.0, 2.0), 2.0), (p(10.0, 2.0), 14.0)]
        );
        assert_eq!(
            route.steps,
            [
                Step::Travel { from: a, to: d, beside: false },
                Step::Row { segment: 1, from: d, to: c },
                Step::Travel { from: c, to: b, beside: false },
                Step::Row { segment: 0, from: b, to: a },
            ]
        );
    }

    #[test]
    fn the_start_and_end_split_the_stretch_nearest_them() {
        let segments = two_rows();
        // From the left of the top row's start, at its height: that node itself.
        let route = routed(&rectangle(), &segments, Some(p(-5.0, 1.0)), None);
        assert_eq!(route.nodes.len(), 4);
        assert!(matches!(route.steps.first(), Some(Step::Travel { from: 0, .. } | Step::Row { from: 0, .. })));
        // From above the top side and to below the bottom one: new nodes at their feet, 23 and 8 along.
        let route = routed(&rectangle(), &segments, Some(p(3.0, -2.0)), Some(p(5.0, 9.0)));
        let node = |i: usize| route.nodes.get(i).map(|n| ((n.point.x() * 1e9).round() / 1e9, n.point.y(), n.place));
        assert_eq!(node(4), Some((3.0, 0.0, Place { ring: 0, at: 23.0 })));
        assert_eq!(node(5), Some((5.0, 3.0, Place { ring: 0, at: 8.0 })));
        assert!(matches!(route.steps.first(), Some(Step::Travel { from: 4, .. })));
        assert!(matches!(route.steps.last(), Some(Step::Travel { to: 5, .. })));
    }

    #[test]
    fn a_point_splits_the_stretch_whose_chord_passes_nearest_it_the_first_on_a_tie() {
        let part = rectangle();
        let rings = rings_of(&part);
        let [a, b, c, d] = [0, 1, 2, 3];
        let insert = |built: &mut Built, q: Point| built.insert(&rings, q, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        // Below the bottom side: its foot (5, 3) splits C–D, whose chord passes 1 from it. B–A's passes 2
        // from it, and A–C's and D–B's farther.
        let mut built = joined(&rings, &two_rows());
        let node = insert(&mut built, p(5.0, 9.0));
        assert_eq!(node, 4);
        assert_eq!(kinds(&built.graph, c, d), [Kind::Row(1)]);
        assert_eq!([kinds(&built.graph, c, node), kinds(&built.graph, node, d)], [[Kind::Outline], [Kind::Outline]]);
        // Left of the left side: its foot (0, 0.5) is 0.5 from the chords of A–C and of B–A. B–A comes
        // first in the order the graph lists its edges, and is split.
        let mut built = joined(&rings, &two_rows());
        let node = insert(&mut built, p(-5.0, 0.5));
        assert_eq!(kinds(&built.graph, a, b), [Kind::Row(0)]);
        assert_eq!(kinds(&built.graph, a, c), [Kind::Outline, Kind::Extra]);
        assert_eq!([kinds(&built.graph, a, node), kinds(&built.graph, node, b)], [[Kind::Outline], [Kind::Outline]]);
        // A point whose foot is a node: that node.
        assert_eq!(insert(&mut built, p(-5.0, 1.0)), a);
    }

    #[test]
    fn each_ring_joins_its_nodes_in_order_and_doubles_every_other_stretch_from_the_first() {
        let part = rectangle();
        let rings = rings_of(&part);
        let [a, b, c, d] = [0, 1, 2, 3];
        // In order along the outline: A, C, D, B.
        let built = joined(&rings, &two_rows());
        assert_eq!(kinds(&built.graph, a, c), [Kind::Outline, Kind::Extra]);
        assert_eq!(kinds(&built.graph, c, d), [Kind::Row(1), Kind::Outline]);
        assert_eq!(kinds(&built.graph, d, b), [Kind::Outline, Kind::Extra]);
        assert_eq!(kinds(&built.graph, b, a), [Kind::Row(0), Kind::Outline]);
        // A ring of 2 nodes: its second stretch joins the same 2 nodes as its first, so only the first is
        // there, doubled.
        let built = joined(&rings, &two_rows()[..1]);
        assert_eq!(kinds(&built.graph, a, b), [Kind::Row(0), Kind::Outline, Kind::Extra]);
    }

    #[test]
    fn a_point_given_at_minus_0_is_the_node_at_0() {
        let part = rectangle();
        let segments = [(p(0.0, 0.0), p(10.0, 3.0)), (p(-0.0, 3.0), p(10.0, -0.0)), (p(0.0, 3.0), p(10.0, 0.0))];
        let built = Built::new(&rings_of(&part), &segments, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(built.nodes.len(), 4);
    }

    #[test]
    fn odd_nodes_on_one_ring_pair_by_the_way_along_it_which_the_needle_travels() {
        // A dumbbell: 2 squares 10 wide, joined by a neck 10 long and 1 wide, with 2 rows across the neck.
        // Each row's ends lie 47 apart along the outline, and 1 apart along the row; each end lies 2 from
        // the end of the other row beside it. Before the rings are joined, every end is odd.
        let outline = [(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 5.5), (20.0, 5.5), (20.0, 10.0), (30.0, 10.0), (30.0, 0.0), (20.0, 0.0)];
        let neck = [(20.0, 4.5), (10.0, 4.5), (10.0, 0.0), (0.0, 0.0)];
        let part = Polygon { outline: outline.iter().chain(&neck).map(|&(x, y)| p(x, y)).collect(), holes: Vec::new() };
        let rings = rings_of(&part);
        let segments = [(p(14.0, 4.5), p(14.0, 5.5)), (p(16.0, 4.5), p(16.0, 5.5))];
        let meter = &mut Budget::DEFAULT.meter();
        let mut built = Built::new(&rings, &segments, meter).unwrap();
        built.pair_odd_nodes(&rings, &segments, meter).unwrap();
        assert_eq!(kinds(&built.graph, 0, 2), [Kind::Paired]);
        assert_eq!(kinds(&built.graph, 1, 3), [Kind::Paired]);
        assert_eq!(kinds(&built.graph, 0, 1), [Kind::Row(0)]);
    }

    /// The costs between rows at `xs` along a line.
    fn costs(xs: &[f64]) -> Vec<Vec<f64>> {
        xs.iter().map(|a| xs.iter().map(|b| (a - b).abs()).collect()).collect()
    }

    /// `count` blocks of 4 rows, 100 apart, at 2, 0, 3 and 5. Pairing the first 2 rows of a block and the
    /// last 2 costs 4. Pairing each row in turn with the nearest left pairs the first with the third, and
    /// costs 6.
    fn blocks(count: u32) -> Vec<f64> {
        (0..count).flat_map(|k| [2.0, 0.0, 3.0, 5.0].map(|x| x + 100.0 * f64::from(k))).collect()
    }

    #[test]
    fn up_to_16_rows_pair_for_the_least_total_and_more_each_with_the_nearest_left() {
        let pairing = |cost: &[Vec<f64>]| matching(cost, &mut Budget { max_stitches: 1, max_work: 1_000_000 }.meter()).unwrap();
        assert_eq!(pairing(&costs(&blocks(1))), [(0, 1), (2, 3)]);
        assert_eq!(pairing(&costs(&[0.0, 5.0, 1.0, 6.0])), [(0, 2), (1, 3)]);
        assert_eq!(pairing(&costs(&blocks(4))), (0..8).map(|k| (2 * k, 2 * k + 1)).collect::<Vec<_>>());
        // On a tie, the first pairing weighed: the lowest row with the lowest it can pair with.
        assert_eq!(pairing(&[vec![1.0; 4], vec![1.0; 4], vec![1.0; 4], vec![1.0; 4]]), [(0, 1), (2, 3)]);
        // 18 rows: each in turn with the nearest left.
        let mut xs = blocks(4);
        xs.extend([400.0, 401.0]);
        let nearest_first: Vec<(usize, usize)> = (0..4).flat_map(|k| [(4 * k, 4 * k + 2), (4 * k + 1, 4 * k + 3)]).chain([(16, 17)]).collect();
        assert_eq!(pairing(&costs(&xs)), nearest_first);
    }

    #[test]
    fn pairing_is_charged_to_the_budget() {
        // 6 pairs of 4 rows at 2 units each, and all 4 rows at 4.
        let mut meter = Budget::DEFAULT.meter();
        matching(&costs(&[0.0, 1.0, 2.0, 3.0]), &mut meter).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 16);
        // 18 rows: 17 weighed against the first, then 15, and so on down to 1.
        let mut meter = Budget::DEFAULT.meter();
        matching(&costs(&(0u8..18).map(f64::from).collect::<Vec<_>>()), &mut meter).unwrap();
        assert_eq!(Budget::DEFAULT.max_work - meter.work_left(), 81);
        assert!(matching(&costs(&[0.0, 1.0, 2.0, 3.0]), &mut Budget { max_stitches: 1, max_work: 15 }.meter()).is_err());
    }

    #[test]
    fn each_edge_the_walk_takes_is_one_step_back_from_where_it_led_to_where_it_left() {
        // Off an Euler circuit: from A the walk takes the row to B and steps back over it, then takes the
        // outline to C and steps back over that.
        let mut graph = Graph::default();
        let [a, b, c] = [graph.add_node(), graph.add_node(), graph.add_node()];
        graph.add(a, c, Kind::Outline);
        graph.add(a, b, Kind::Row(0));
        let steps = walk(&mut graph, a, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(steps, [Step::Row { segment: 0, from: b, to: a }, Step::Travel { from: c, to: a, beside: false }]);
        assert_eq!(graph.degree(a), 0);
    }

    /// Asserts that `route` sews each of `count` segments once, that each step starts where the one before
    /// ended, from node `ends.0` to node `ends.1`, and that travel finds a way for every step.
    fn assert_sound(part: &Polygon, route: &Route, count: usize, ends: (usize, usize), case: &str) {
        let mut sewn = vec![0; count];
        let mut at = ends.0;
        for step in &route.steps {
            let (from, to) = match *step {
                Step::Row { segment, from, to } => {
                    sewn[segment] += 1;
                    (from, to)
                }
                Step::Travel { from, to, .. } => (from, to),
            };
            assert_eq!(from, at, "{case}: a step from elsewhere: {:?}", route.steps);
            at = to;
        }
        assert_eq!(at, ends.1, "{case}: ends elsewhere");
        assert!(sewn.iter().all(|&n| n == 1), "{case}: rows sewn {sewn:?}");
        let meter = &mut Budget::DEFAULT.meter();
        let rings = rings_of(part);
        let rows: Vec<[usize; 2]> =
            route.steps.iter().filter_map(|s| if let Step::Row { from, to, .. } = *s { Some([from, to]) } else { None }).collect();
        let network = Network::new(&route.nodes, &rings, &rows, meter).unwrap();
        for step in &route.steps {
            if let Step::Travel { from, to, .. } = *step {
                assert!(between(&route.nodes, &rings, &network, from, to, meter).unwrap().is_some(), "{case}: no way from {from} to {to}");
            }
        }
    }

    /// Shapely's rows fixture: polygons of grid squares and half squares, many with holes, where rows run
    /// along edges and through corners and so share their ends.
    const SHAPELY: &str = include_str!("../../../../../conformance/fixtures/geometry/shapely-rows.txt");

    #[test]
    fn req_fill_tat_001_every_row_of_every_fixture_polygon_is_sewn_once_in_one_walk() {
        let (mut checked, mut paired) = (0, 0);
        for (line, mut words) in cases(SHAPELY) {
            let spacing = words.number();
            let rings = words.polylines();
            let part = Polygon { outline: rings[0].clone(), holes: rings[1..].to_vec() };
            for angle in [0.0, 30.0, 90.0] {
                let found = rows(&part, &Grid { angle, spacing, end_spacing: None }, &mut Budget::DEFAULT.meter()).unwrap();
                let segments: Vec<(Point, Point)> = found.iter().flat_map(|row| row.segments.iter().map(|s| (s.start, s.end))).collect();
                if segments.is_empty() {
                    continue;
                }
                let case = format!("line {line} at {angle}°");
                let built = joined(&rings_of(&part), &segments);
                paired += usize::from((0..built.nodes.len()).any(|n| built.graph.degree(n) % 2 == 1));
                let route = routed(&part, &segments, None, None);
                assert_sound(&part, &route, segments.len(), (0, 0), &case);
                // From a corner outside to a point inside.
                let (start, end) = (p(-3.0, -2.0), p(1.25, 1.75));
                let route = routed(&part, &segments, Some(start), Some(end));
                let near = |q: Point| nearest(&route.nodes, q, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
                assert_sound(&part, &route, segments.len(), (near(start), near(end)), &case);
                checked += 1;
            }
        }
        assert!(checked > 2000, "{checked}");
        assert!(paired > 500, "nodes of odd degree are common on the grid: {paired}");
    }
}
