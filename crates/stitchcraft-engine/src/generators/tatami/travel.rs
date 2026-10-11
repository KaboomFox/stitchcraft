//! The needle's way between 2 nodes of a fill's route ([`super::route`]), which running stitches then
//! follow.
//!
//! **Along a ring.** Between 2 points of one ring the needle travels along the ring, the shorter way
//! round, as Ink/Stitch's travel along a fill's outline does ([`Rings::forwards`]).
//!
//! **Across the part.** Between points of 2 rings, or where the route pairs nodes on 2 rings, the needle
//! takes the shortest way along rings and rows: the rows' lines lie inside the part, and the travel along
//! them is covered by the rows when they are sewn after it. Ink/Stitch, travelling along the outline
//! only, sews the rows on such a way a second time, as rows (`DEV-FILL-006`). These are travel's ways with
//! `underpath` off, and in a part the lines travel under the rows follows miss ([`super::underpath`]).
//! Where no way joins the 2 points, the needle jumps.
//!
//! Distances are compared exactly as computed, and of 2 nodes as near the lower-numbered goes first, so
//! every platform finds the same way. The search is shared with travel under the rows.

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

use stitchcraft_core::{Exhausted, Meter, Point};

use super::graph::NodeId;
use super::rings::{Rings, stretches};
use super::route::Node;

/// How a step of a way runs: along a ring, in its direction or against it, or along a row's line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Along {
    /// Along ring `ring`, in its own direction when `forwards`.
    Ring {
        /// The ring.
        ring: usize,
        /// Whether in the ring's own direction.
        forwards: bool,
    },
    /// Straight along a row.
    Row,
}

/// The ways between a route's nodes: along each ring from each node to the next, and along each row.
#[derive(Clone, Debug)]
pub(crate) struct Network {
    around: Vec<Vec<(NodeId, f64, Along)>>,
}

/// What a search finds: each node's distance from where it started, and the step that reaches it.
pub(crate) type Searched<S> = (Vec<f64>, Vec<Option<(NodeId, S)>>);

/// A distance with a total order, for the queue.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Distance(f64);

impl Eq for Distance {}

impl PartialOrd for Distance {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Distance {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Network {
    /// The network of `nodes` on `rings`, with the rows joining the 2 nodes of each of `rows`. One unit of
    /// `meter` for each node and row.
    pub fn new(nodes: &[Node], rings: &Rings<'_>, rows: &[[NodeId; 2]], meter: &mut Meter) -> Result<Network, Exhausted> {
        let mut around = vec![Vec::new(); nodes.len()];
        for [(a, pa), (b, pb)] in stretches(nodes.iter().map(|n| n.place).enumerate().collect(), meter)? {
            let length = rings.way_length(pa.ring, pa.at, pb.at, true);
            link(&mut around, a, b, length, Along::Ring { ring: pa.ring, forwards: true });
            link(&mut around, b, a, length, Along::Ring { ring: pa.ring, forwards: false });
        }
        for &[a, b] in rows {
            meter.charge(1)?;
            let (Some(na), Some(nb)) = (nodes.get(a), nodes.get(b)) else { continue };
            let length = na.point.distance(nb.point);
            link(&mut around, a, b, length, Along::Row);
            link(&mut around, b, a, length, Along::Row);
        }
        Ok(Network { around })
    }

    /// How far each node is from `from` along the network: infinite where no way leads. One unit of
    /// `meter` for each step looked at.
    pub fn distances(&self, from: NodeId, meter: &mut Meter) -> Result<Vec<f64>, Exhausted> {
        Ok(self.search(from, None, meter)?.0)
    }

    /// The shortest way from `from` to `to`: each node after `from` with how the way reaches it. `None`
    /// when no way leads there. One unit of `meter` for each step looked at.
    pub fn way(&self, from: NodeId, to: NodeId, meter: &mut Meter) -> Result<Option<Vec<(NodeId, Along)>>, Exhausted> {
        way_to(&self.search(from, Some(to), meter)?, from, to, meter)
    }

    fn search(&self, from: NodeId, to: Option<NodeId>, meter: &mut Meter) -> Result<Searched<Along>, Exhausted> {
        search(self.around.len(), from, to, |node| self.around.get(node).into_iter().flatten().copied(), meter)
    }
}

/// Dijkstra's search over `count` nodes from `from`, stopping at `to` if given: each node's distance and
/// the step it is reached by. `steps` lists the steps out of a node: where each leads, how long it is, and
/// how it runs. Nodes leave the queue nearest first, the lower-numbered of 2 as near, and a node keeps the
/// first way to reach it unless a shorter one comes. One unit of `meter` for each node taken from the queue.
pub(crate) fn search<S: Copy, I: IntoIterator<Item = (NodeId, f64, S)>>(
    count: usize,
    from: NodeId,
    to: Option<NodeId>,
    steps: impl Fn(NodeId) -> I,
    meter: &mut Meter,
) -> Result<Searched<S>, Exhausted> {
    let mut distance = vec![f64::INFINITY; count];
    let mut came: Vec<Option<(NodeId, S)>> = vec![None; count];
    let mut queue = BinaryHeap::new();
    if let Some(d) = distance.get_mut(from) {
        *d = 0.0;
        queue.push(Reverse((Distance(0.0), from)));
    }
    while let Some(Reverse((Distance(d), node))) = queue.pop() {
        meter.charge(1)?;
        if distance.get(node).is_some_and(|&best| d > best) {
            continue;
        }
        if Some(node) == to {
            break;
        }
        for (next, length, how) in steps(node) {
            let through = d + length;
            if distance.get(next).is_some_and(|&best| through < best) {
                if let Some(slot) = distance.get_mut(next) {
                    *slot = through;
                }
                if let Some(slot) = came.get_mut(next) {
                    *slot = Some((node, how));
                }
                queue.push(Reverse((Distance(through), next)));
            }
        }
    }
    Ok((distance, came))
}

/// The shortest way from `from` to `to` that `searched` found: each node after `from` with how the way
/// reaches it. `None` when no way leads there. One unit of `meter` for each step.
pub(crate) fn way_to<S: Copy>(searched: &Searched<S>, from: NodeId, to: NodeId, meter: &mut Meter) -> Result<Option<Vec<(NodeId, S)>>, Exhausted> {
    let (distance, came) = searched;
    if !distance.get(to).is_some_and(|d| d.is_finite()) {
        return Ok(None);
    }
    let mut way = Vec::new();
    let mut at = to;
    while at != from {
        meter.charge(1)?;
        let Some(Some((before, how))) = came.get(at).copied() else { return Ok(None) };
        way.push((at, how));
        at = before;
    }
    way.reverse();
    Ok(Some(way))
}

/// Adds the step from `a` to `b` to the network.
fn link(around: &mut [Vec<(NodeId, f64, Along)>], a: NodeId, b: NodeId, length: f64, along: Along) {
    if let Some(list) = around.get_mut(a) {
        list.push((b, length, along));
    }
}

/// The points the needle travels through from node `from` to node `to`: along their ring when they share
/// one, and otherwise along the network's shortest way. `None` when no way leads there. One unit of
/// `meter` for each point and step looked at.
pub(crate) fn between(
    nodes: &[Node],
    rings: &Rings<'_>,
    network: &Network,
    from: NodeId,
    to: NodeId,
    meter: &mut Meter,
) -> Result<Option<Vec<Point>>, Exhausted> {
    let (Some(a), Some(b)) = (nodes.get(from), nodes.get(to)) else { return Ok(None) };
    if a.place.ring == b.place.ring {
        let forwards = rings.forwards(a.place.ring, a.place.at, b.place.at);
        return Ok(Some(ends_exact(rings.way(a.place.ring, a.place.at, b.place.at, forwards, meter)?, a.point, b.point)));
    }
    let Some(steps) = network.way(from, to, meter)? else { return Ok(None) };
    let mut points = vec![a.point];
    let mut at = *a;
    for (next, along) in steps {
        let Some(n) = nodes.get(next) else { return Ok(None) };
        match along {
            Along::Ring { ring, forwards } => {
                let way = ends_exact(rings.way(ring, at.place.at, n.place.at, forwards, meter)?, at.point, n.point);
                points.extend(way.into_iter().skip(1));
            }
            Along::Row => points.push(n.point),
        }
        at = *n;
    }
    Ok(Some(points))
}

/// `way` with its first and last points made exactly `start` and `end`: a ring gives back a node's point
/// within a rounding error, and the rows must meet the travel where they start.
fn ends_exact(mut way: Vec<Point>, start: Point, end: Point) -> Vec<Point> {
    if let Some(first) = way.first_mut() {
        *first = start;
    }
    if let Some(last) = way.last_mut() {
        *last = end;
    }
    way
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p};

    /// The ends of 2 rows across the frame at y = 1.5, one on each side of the hole, as nodes where they
    /// lie on `rings`: (0, 1.5) and (10, 1.5) on the outline, 1.5 and 16.5 along it, and (4, 1.5) and
    /// (6, 1.5) on the hole, 7.5 and 2.5 along it. Nodes 0 and 1 are the left row's ends, 2 and 3 the right
    /// row's.
    fn nodes(rings: &Rings<'_>) -> Vec<Node> {
        [p(0.0, 1.5), p(4.0, 1.5), p(6.0, 1.5), p(10.0, 1.5)]
            .map(|q| Node { point: q, place: rings.locate(q, |_| true, &mut Budget::DEFAULT.meter()).unwrap().unwrap() })
            .to_vec()
    }

    const ROWS: [[NodeId; 2]; 2] = [[0, 1], [2, 3]];

    #[test]
    fn the_shortest_way_runs_along_rows_and_rings() {
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let nodes = nodes(&rings);
        assert_eq!(nodes.iter().map(|n| (n.place.ring, n.place.at)).collect::<Vec<_>>(), [(0, 1.5), (1, 7.5), (1, 2.5), (0, 16.5)]);
        let network = Network::new(&nodes, &rings, &ROWS, meter).unwrap();
        // Across the left row, 3 along the hole the short way, past its corners at (4, 1) and (6, 1), and
        // across the right row: 11, where the outline's short way is 13.
        assert_eq!(network.distances(0, meter).unwrap(), [0.0, 4.0, 7.0, 11.0]);
        assert_eq!(network.way(0, 3, meter).unwrap(), Some(vec![(1, Along::Row), (2, Along::Ring { ring: 1, forwards: true }), (3, Along::Row)]));
        assert_eq!(network.way(2, 2, meter).unwrap(), Some(Vec::new()));
        // From one ring to the other: along the row, then the hole.
        assert_eq!(
            between(&nodes, &rings, &network, 0, 2, meter).unwrap(),
            Some(vec![p(0.0, 1.5), p(4.0, 1.5), p(4.0, 1.0), p(6.0, 1.0), p(6.0, 1.5)])
        );
    }

    #[test]
    fn between_2_points_of_one_ring_the_needle_runs_along_it_the_shorter_way() {
        // Round the outline's top corners, 13 long, though the way along the rows and the hole is 11.
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let nodes = nodes(&rings);
        let network = Network::new(&nodes, &rings, &ROWS, meter).unwrap();
        assert_eq!(between(&nodes, &rings, &network, 0, 3, meter).unwrap(), Some(vec![p(0.0, 1.5), p(0.0, 0.0), p(10.0, 0.0), p(10.0, 1.5)]));
        assert_eq!(between(&nodes, &rings, &network, 3, 7, meter).unwrap(), None, "no node 7");
    }

    #[test]
    fn where_no_way_leads_there_is_none() {
        // The rows' ends alone, with no rows: each ring has 2 nodes, and nothing joins the rings.
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let nodes = nodes(&rings);
        let network = Network::new(&nodes, &rings, &[], meter).unwrap();
        assert_eq!(network.distances(0, meter).unwrap(), [0.0, f64::INFINITY, f64::INFINITY, 13.0]);
        assert_eq!(network.way(0, 1, meter).unwrap(), None);
        assert_eq!(between(&nodes, &rings, &network, 0, 1, meter).unwrap(), None);
    }

    #[test]
    fn a_ring_with_one_node_has_no_way_along_it() {
        // The left row alone: one node on each ring, and only the row between them.
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let nodes = nodes(&rings)[..2].to_vec();
        let network = Network::new(&nodes, &rings, &[[0, 1]], meter).unwrap();
        assert_eq!(network.distances(0, meter).unwrap(), [0.0, 4.0]);
        assert_eq!(network.way(1, 0, meter).unwrap(), Some(vec![(0, Along::Row)]));
    }

    #[test]
    fn distances_order_as_their_values() {
        assert_eq!(Distance(1.0).partial_cmp(&Distance(2.0)), Some(Ordering::Less));
        assert!(Distance(f64::INFINITY) > Distance(1e300));
    }

    #[test]
    fn searching_is_charged_to_the_budget() {
        let part = frame();
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let nodes = nodes(&rings);
        assert!(Network::new(&nodes, &rings, &ROWS, &mut Budget { max_stitches: 1, max_work: 5 }.meter()).is_err());
        let network = Network::new(&nodes, &rings, &ROWS, meter).unwrap();
        assert!(network.way(0, 3, &mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
    }
}
