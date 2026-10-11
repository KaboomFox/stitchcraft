//! An undirected multigraph that keeps its edges in the order they were added, for routing a fill's rows
//! ([`super::route`]).
//!
//! **Why the order matters.** The route Ink/Stitch takes through a fill's rows depends on the order in
//! which it meets a node's edges: at every node it takes a row if one is left, and otherwise the first
//! edge left. Its graph library lists a node's edges neighbour by neighbour, each neighbour where its first
//! edge to the node was added and its edges in the order they were added. A neighbour whose last edge is
//! removed drops out of the list, and the others keep their places. This graph lists them the same way, so
//! that the route makes the same choices.
//!
//! An edge is its 2 ends and its [`Kind`]. Ink/Stitch's graph knows an edge between 2 nodes by its kind,
//! so adding an edge of a kind the 2 nodes already share adds nothing; the edges that pair nodes of odd
//! degree are the exception and may repeat. An edge from a node to itself is listed once among the node's
//! edges and counts twice towards its degree.
//!
//! Lookups never index blindly: a node or an edge that does not exist is simply not there.

/// A node, by the order it was added in.
pub(crate) type NodeId = usize;

/// An edge, by the order it was added in. Removed edges keep their numbers.
pub(crate) type EdgeId = usize;

/// What an edge stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    /// A row's segment, by its place in the order the rows are taken.
    Row(usize),
    /// The stretch of a ring between 2 neighbouring nodes on it.
    Outline,
    /// The same stretch a second time, on every other stretch of each ring.
    Extra,
    /// A way between 2 nodes of odd degree, added so that every degree is even.
    Paired,
}

/// An edge: its ends and what it stands for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Edge {
    /// Its ends, in the order it was added with.
    pub ends: [NodeId; 2],
    /// What it stands for.
    pub kind: Kind,
}

impl Edge {
    /// The end of the edge that is not `node`: `node` itself for an edge from a node to itself.
    pub fn other(&self, node: NodeId) -> NodeId {
        if self.ends[0] == node { self.ends[1] } else { self.ends[0] }
    }
}

/// A neighbour of a node, and the edges between them, in the order they were added.
#[derive(Clone, Debug)]
struct Neighbour {
    node: NodeId,
    edges: Vec<EdgeId>,
}

/// The graph.
#[derive(Clone, Debug, Default)]
pub(crate) struct Graph {
    /// Every edge ever added; `None` once removed.
    edges: Vec<Option<Edge>>,
    /// Each node's neighbours, in the order their first edge to it was added.
    around: Vec<Vec<Neighbour>>,
}

impl Graph {
    /// Adds a node; its number.
    pub fn add_node(&mut self) -> NodeId {
        self.around.push(Vec::new());
        self.around.len() - 1
    }

    /// The edge `id`, unless it was removed or never added.
    pub fn edge(&self, id: EdgeId) -> Option<Edge> {
        self.edges.get(id).copied().flatten()
    }

    /// Adds an edge of `kind` between `a` and `b`, unless they already share an edge of that kind and it is
    /// not [`Kind::Paired`]: the edge, new or old. `None` when a node does not exist.
    pub fn add(&mut self, a: NodeId, b: NodeId, kind: Kind) -> Option<EdgeId> {
        if a >= self.around.len() || b >= self.around.len() {
            return None;
        }
        if kind != Kind::Paired
            && let Some(old) = self.neighbour(a, b).and_then(|n| n.edges.iter().copied().find(|&e| self.edge(e).is_some_and(|e| e.kind == kind)))
        {
            return Some(old);
        }
        let id = self.edges.len();
        self.edges.push(Some(Edge { ends: [a, b], kind }));
        self.attach(a, b, id);
        if a != b {
            self.attach(b, a, id);
        }
        Some(id)
    }

    /// Lists `id` among `node`'s edges to `neighbour`, adding the neighbour at the end of its list if new.
    fn attach(&mut self, node: NodeId, neighbour: NodeId, id: EdgeId) {
        let Some(list) = self.around.get_mut(node) else { return };
        match list.iter_mut().find(|n| n.node == neighbour) {
            Some(n) => n.edges.push(id),
            None => list.push(Neighbour { node: neighbour, edges: vec![id] }),
        }
    }

    /// Removes the edge `id`; a neighbour left with no edge drops out of its node's list.
    pub fn remove(&mut self, id: EdgeId) {
        let Some(Edge { ends: [a, b], .. }) = self.edge(id) else { return };
        if let Some(slot) = self.edges.get_mut(id) {
            *slot = None;
        }
        self.detach(a, b, id);
        if a != b {
            self.detach(b, a, id);
        }
    }

    /// Takes `id` off `node`'s edges to `neighbour`.
    fn detach(&mut self, node: NodeId, neighbour: NodeId, id: EdgeId) {
        let Some(list) = self.around.get_mut(node) else { return };
        let Some(place) = list.iter().position(|n| n.node == neighbour) else { return };
        if let Some(n) = list.get_mut(place) {
            n.edges.retain(|&e| e != id);
            if n.edges.is_empty() {
                list.remove(place);
            }
        }
    }

    /// `node`'s entry for `neighbour`, if they share an edge.
    fn neighbour(&self, node: NodeId, neighbour: NodeId) -> Option<&Neighbour> {
        self.around.get(node)?.iter().find(|n| n.node == neighbour)
    }

    /// Whether `a` and `b` share an edge.
    pub fn joined(&self, a: NodeId, b: NodeId) -> bool {
        self.neighbour(a, b).is_some()
    }

    /// How many edges meet at `node`, an edge to itself counting twice.
    pub fn degree(&self, node: NodeId) -> usize {
        self.around.get(node).map_or(0, |list| list.iter().map(|n| n.edges.len() * if n.node == node { 2 } else { 1 }).sum())
    }

    /// `node`'s edges: neighbour by neighbour, each neighbour's in the order they were added.
    pub fn edges_at(&self, node: NodeId) -> impl Iterator<Item = EdgeId> + '_ {
        self.around.get(node).into_iter().flatten().flat_map(|n| n.edges.iter().copied())
    }

    /// Every edge once: node by node, each node's edges to the neighbours not yet passed, in the order
    /// [`Graph::edges_at`] lists them. Each comes with the node it was listed from first.
    pub fn all(&self) -> Vec<(NodeId, EdgeId)> {
        let mut passed = vec![false; self.around.len()];
        let mut all = Vec::new();
        for (node, list) in self.around.iter().enumerate() {
            for n in list {
                if !passed.get(n.node).copied().unwrap_or(true) {
                    all.extend(n.edges.iter().map(|&e| (node, e)));
                }
            }
            if let Some(slot) = passed.get_mut(node) {
                *slot = true;
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A graph of `count` nodes and no edges.
    fn nodes(count: usize) -> Graph {
        let mut g = Graph::default();
        for _ in 0..count {
            g.add_node();
        }
        g
    }

    fn kinds(graph: &Graph, node: NodeId) -> Vec<(NodeId, Kind)> {
        graph.edges_at(node).filter_map(|e| graph.edge(e)).map(|e| (e.other(node), e.kind)).collect()
    }

    #[test]
    fn a_node_lists_its_edges_neighbour_by_neighbour_in_the_order_they_came() {
        let mut g = nodes(3);
        g.add(0, 1, Kind::Row(0));
        g.add(0, 2, Kind::Outline);
        g.add(0, 2, Kind::Extra);
        // A later edge to the first neighbour is listed with it, ahead of the second neighbour.
        g.add(1, 0, Kind::Outline);
        assert_eq!(kinds(&g, 0), [(1, Kind::Row(0)), (1, Kind::Outline), (2, Kind::Outline), (2, Kind::Extra)]);
        assert_eq!(kinds(&g, 1), [(0, Kind::Row(0)), (0, Kind::Outline)]);
        assert_eq!((g.degree(0), g.degree(1), g.degree(2)), (4, 2, 2));
        assert!(g.joined(2, 0) && !g.joined(1, 2));
    }

    #[test]
    fn a_neighbour_left_without_edges_drops_out_and_comes_back_last() {
        let mut g = nodes(3);
        let row = g.add(0, 1, Kind::Row(0)).unwrap();
        g.add(0, 2, Kind::Outline);
        let back = g.add(1, 0, Kind::Outline).unwrap();
        g.remove(row);
        assert_eq!(kinds(&g, 0), [(1, Kind::Outline), (2, Kind::Outline)], "1 keeps its place while an edge is left");
        g.remove(back);
        assert_eq!(kinds(&g, 0), [(2, Kind::Outline)]);
        g.add(0, 1, Kind::Extra);
        assert_eq!(kinds(&g, 0), [(2, Kind::Outline), (1, Kind::Extra)]);
        // Removing twice, or an edge never added, changes nothing.
        g.remove(row);
        g.remove(99);
        assert_eq!(g.degree(0), 2);
        assert_eq!(g.edge(row), None);
    }

    #[test]
    fn a_kind_two_nodes_already_share_adds_nothing_unless_it_pairs() {
        let mut g = nodes(2);
        let first = g.add(0, 1, Kind::Outline);
        assert_eq!(g.add(1, 0, Kind::Outline), first, "the same edge, either way round");
        let paired = [g.add(0, 1, Kind::Paired), g.add(0, 1, Kind::Paired)];
        assert_ne!(paired[0], paired[1]);
        assert_eq!(g.degree(0), 3);
        assert_eq!(g.add(0, 5, Kind::Outline), None, "no node 5");
    }

    #[test]
    fn an_edge_to_itself_is_listed_once_and_counts_twice() {
        let mut g = nodes(2);
        g.add(0, 1, Kind::Row(0));
        let lap = g.add(0, 0, Kind::Outline).unwrap();
        g.add(0, 0, Kind::Extra);
        assert_eq!(kinds(&g, 0), [(1, Kind::Row(0)), (0, Kind::Outline), (0, Kind::Extra)]);
        assert_eq!(g.degree(0), 5);
        g.remove(lap);
        assert_eq!(g.degree(0), 3);
        assert_eq!(g.edge(1).map(|e| e.other(0)), None);
    }

    #[test]
    fn every_edge_is_listed_once_from_the_node_that_comes_first() {
        let mut g = nodes(2);
        assert_eq!(g.add_node(), 2);
        let edges = [g.add(1, 2, Kind::Row(0)), g.add(0, 1, Kind::Outline), g.add(2, 0, Kind::Outline), g.add(1, 1, Kind::Outline)];
        let [e12, e01, e20, e11] = edges.map(Option::unwrap);
        assert_eq!(g.all(), [(0, e01), (0, e20), (1, e12), (1, e11)]);
    }
}
