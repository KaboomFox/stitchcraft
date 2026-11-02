//! Inner to outer: one path through a contour fill's tree of rings, from the outline in to the middles and
//! back out, each ring sewn around its children (`REQ-FILL-CON-002`, `docs/src/design/algorithms/fills.md`
//! › Contour fill › Inner to outer).
//!
//! **In.** A ring starts at its point nearest where the needle comes to it. Each child is entered at the
//! first point along the ring within 1.5 row spacings of it, or within 2.05 where none is that near: where a
//! hole's growth meets an outline, rings can lie up to 2 row spacings apart. The needle steps to the
//! child's point nearest that point. A child farther than that from every point of the ring is not sewn,
//! nor any ring under it, which `SC-W0311` says.
//!
//! **Out.** The ring is sewn to each child's entry in turn, the child sewn whole with its own children,
//! and the needle comes back to the ring a row spacing past the entry. A child ends a row spacing short of
//! where it began, so the steps out of the rings spiral out from the innermost. After its last child, a ring
//! is sewn round to a row spacing short of its start, or whole when what is left of it is no longer than a
//! row spacing.
//!
//! **Directions.** A ring with no child sewn, a middle, runs the other way, which looks better. With
//! `avoid_self_crossing`, the rings instead run the other way at every other level down the tree, and none
//! is turned, so the steps between them don't cross.
//!
//! **Stretches.** The path is kept as the stretches it is sewn in, each along one ring, the needle stepping
//! straight from the end of one to the start of the next, so that each is stitched as a span of its own.
//!
//! **No recursion.** Ink/Stitch's walk calls itself for each child. Here the rings being sewn are a stack,
//! so a tree hundreds of levels deep needs no deep call stack, which a WebAssembly host runs out of.

use stitchcraft_core::{Exhausted, Meter, Point};

use super::line::{cut, first_within, length, nearest, project, roll};
use super::tree::{ROOT, Tree};
use crate::generators::tatami::sides::Sides;

/// A child is entered at the first point of its parent's ring within this many row spacings of it, or,
/// where none is that near, within [`FAR`].
const NEAR: f64 = 1.5;
const FAR: f64 = 2.05;

/// The path through a tree, as its stretches along the rings, and how many of its rings it leaves out.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Path {
    pub(super) stretches: Vec<Vec<Point>>,
    pub(super) left_out: usize,
}

/// Where a child is entered: how far along its parent's ring, at which point of the child's own, and which
/// child.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Entry {
    at: f64,
    on_child: Point,
    child: usize,
}

/// A ring being sewn around its children: what is left of it to sew, how far along it the needle was last
/// sent, its children's entries in order and the next to sew, whether the needle comes back from a child
/// and steps a row spacing on, and whether it runs in its own direction.
struct Frame {
    rest: Option<Vec<Point>>,
    sewn_to: f64,
    entries: Vec<Entry>,
    next: usize,
    back: bool,
    forward: bool,
}

/// The rings of `tree` sewn inner to outer, `d` apart, from the outline's point nearest `start`. With
/// `avoid_self_crossing`, the rings run the other way at every other level. One unit of `meter` for each
/// step, and what finding the entries costs.
pub(super) fn inner_to_outer(tree: &Tree, d: f64, start: Point, avoid_self_crossing: bool, meter: &mut Meter) -> Result<Path, Exhausted> {
    let mut walk = Walk { tree, d, avoid_self_crossing, stretches: Vec::new(), left_out: 0 };
    let mut stack: Vec<Frame> = walk.open(ROOT, start, true, meter)?.into_iter().collect();
    while let Some(frame) = stack.last_mut() {
        meter.charge(1)?;
        if frame.back {
            // Back from a child: the needle steps on a row spacing along the ring.
            frame.back = false;
            if let Some(rest) = frame.rest.take() {
                frame.rest = cut(&rest, d).1;
                frame.sewn_to += d;
            }
        }
        let Some(&entry) = frame.entries.get(frame.next) else {
            // After its last child the ring is sewn round, to a row spacing short of where it started.
            if let Some(rest) = frame.rest.take() {
                let long = length(&rest);
                walk.stretch(if long > d { cut(&rest, long - d).0.unwrap_or_default() } else { rest });
            }
            stack.pop();
            continue;
        };
        frame.next += 1;
        let (before, after) = frame.rest.as_deref().map_or((None, None), |rest| cut(rest, entry.at - frame.sewn_to));
        frame.sewn_to = entry.at;
        walk.stretch(before.unwrap_or_default());
        frame.rest = after;
        frame.back = true;
        let forward = !frame.forward;
        stack.extend(walk.open(entry.child, entry.on_child, forward, meter)?);
    }
    Ok(Path { stretches: walk.stretches, left_out: walk.left_out })
}

/// The walk through a tree: the stretches so far, and the rings left out.
struct Walk<'t> {
    tree: &'t Tree,
    d: f64,
    avoid_self_crossing: bool,
    stretches: Vec<Vec<Point>>,
    left_out: usize,
}

impl Walk<'_> {
    /// Adds `stretch` to the path, unless it has no point.
    fn stretch(&mut self, stretch: Vec<Point>) {
        if !stretch.is_empty() {
            self.stretches.push(stretch);
        }
    }

    /// Starts sewing `node`'s ring at its point nearest `start`, in its own direction when `forward` or
    /// without `avoid_self_crossing`: the frame that sews it around its children, or `None` for a ring with
    /// no child to sew, which is sewn here, the other way unless the rings alternate, to a row spacing short
    /// of where it began.
    fn open(&mut self, node: usize, start: Point, forward: bool, meter: &mut Meter) -> Result<Option<Frame>, Exhausted> {
        let mut ring = self.tree.ring(node).to_vec();
        if !forward && self.avoid_self_crossing {
            ring.reverse();
        }
        let ring = roll(&ring, project(&ring, start));
        let entries = self.entries(node, &ring, meter)?;
        if entries.is_empty() {
            let ring: Vec<Point> = if self.avoid_self_crossing { ring } else { ring.into_iter().rev().collect() };
            let long = length(&ring);
            self.stretch(if long > self.d { cut(&ring, long - self.d).0.unwrap_or_default() } else { ring });
            return Ok(None);
        }
        // The ring starts where it is entered: the first stretch is its start alone when a child is entered
        // there, and the needle steps on from it.
        self.stretch(ring.first().copied().into_iter().collect());
        Ok(Some(Frame { rest: Some(ring), sewn_to: 0.0, entries, next: 0, back: false, forward }))
    }

    /// Where `ring`, the ring of `node` started where it is sewn from, enters each of its children, in order
    /// along it; a child it cannot enter is left out with the rings under it.
    fn entries(&mut self, node: usize, ring: &[Point], meter: &mut Meter) -> Result<Vec<Entry>, Exhausted> {
        let mut entries = Vec::new();
        for &child in self.tree.children(node) {
            let child_ring = self.tree.ring(child);
            let sides = Sides::new(vec![child_ring], meter)?;
            let found = match first_within(ring, &sides, NEAR * self.d, meter)? {
                Some(found) => Some(found),
                None => first_within(ring, &sides, FAR * self.d, meter)?,
            };
            let Some((at, on_ring)) = found else {
                self.left_out += self.tree.count(child);
                continue;
            };
            let on_child = nearest(&[child_ring], on_ring, meter)?.unwrap_or(on_ring);
            entries.push(Entry { at, on_child, child });
        }
        // In order along the ring; children entered at one place keep the tree's order.
        entries.sort_by(|a, b| a.at.total_cmp(&b.at));
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::contour::tree::{Node, tree};
    use crate::normalize::fixture::part;
    use crate::normalize::offset::Corners;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn square(x0: f64, x1: f64) -> Vec<Point> {
        vec![p(x0, x0), p(x0, x1), p(x1, x1), p(x1, x0), p(x0, x0)]
    }

    /// The path's stretches end to end.
    fn flat(path: &Path) -> Vec<Point> {
        path.stretches.concat()
    }

    #[test]
    fn a_lone_ring_is_sewn_the_other_way_to_a_row_spacing_short_of_its_start() {
        let lone = Tree { nodes: vec![Node { ring: square(0.0, 4.0), children: Vec::new() }] };
        let meter = &mut Budget::DEFAULT.meter();
        // From (0, 0), the ring runs up the left side; turned, it runs along the bottom first, and stops 1
        // short of its start.
        let path = inner_to_outer(&lone, 1.0, p(0.0, 0.0), false, meter).unwrap();
        assert_eq!(path.stretches, [vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 1.0)]]);
        // Alternating, it keeps its direction.
        let path = inner_to_outer(&lone, 1.0, p(0.0, 0.0), true, meter).unwrap();
        assert_eq!(flat(&path), [p(0.0, 0.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0), p(1.0, 0.0)]);
        // From the middle of the bottom side, it starts there.
        assert_eq!(flat(&inner_to_outer(&lone, 1.0, p(2.0, -1.0), true, meter).unwrap()).first(), Some(&p(2.0, 0.0)));
    }

    #[test]
    fn a_ring_is_sewn_round_its_child_and_back_a_row_spacing_on() {
        // A square 0..4 round a square 1..3, 1 apart. The outline is entered at its start, (0, 0), within
        // 1.5 of the child, whose nearest point is (1, 1). The child, alone, runs the other way to 1 short of
        // its start, (1, 2); the needle comes back to the outline 1 past (0, 0), at (0, 1), and sews it round
        // to (1, 0).
        let tree = Tree { nodes: vec![Node { ring: square(0.0, 4.0), children: vec![1] }, Node { ring: square(1.0, 3.0), children: Vec::new() }] };
        let path = inner_to_outer(&tree, 1.0, p(0.0, 0.0), false, &mut Budget::DEFAULT.meter()).unwrap();
        // The outline's start alone, the child, then the rest of the outline: 3 stretches, the needle
        // stepping between them.
        let expected = [
            vec![p(0.0, 0.0)],
            vec![p(1.0, 1.0), p(3.0, 1.0), p(3.0, 3.0), p(1.0, 3.0), p(1.0, 2.0)],
            vec![p(0.0, 1.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0), p(1.0, 0.0)],
        ];
        assert_eq!(path, Path { stretches: expected.to_vec(), left_out: 0 });
    }

    #[test]
    fn a_child_out_of_reach_is_left_out_with_the_rings_under_it() {
        // A child 3 away from its parent everywhere, more than 2.05 row spacings of 1: it and its own child
        // are left out, and the parent is sewn alone.
        let tree = Tree {
            nodes: vec![
                Node { ring: square(0.0, 10.0), children: vec![1] },
                Node { ring: square(3.0, 7.0), children: vec![2] },
                Node { ring: square(4.0, 6.0), children: Vec::new() },
            ],
        };
        let path = inner_to_outer(&tree, 1.0, p(0.0, 0.0), false, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(path.left_out, 2);
        assert_eq!(flat(&path).len(), 5);
        // 2 away, it is entered within 2.05.
        let tree = Tree { nodes: vec![Node { ring: square(0.0, 10.0), children: vec![1] }, Node { ring: square(2.0, 8.0), children: Vec::new() }] };
        assert_eq!(inner_to_outer(&tree, 1.0, p(0.0, 0.0), false, &mut Budget::DEFAULT.meter()).unwrap().left_out, 0);
    }

    #[test]
    fn every_ring_of_a_part_is_sewn_once_round() {
        // A 20 mm square with a hole, rings 1 mm apart: the path passes within 0.01 mm of every ring's every
        // point but those within a row spacing of where the ring was left.
        let holed = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], &[&[(5.0, 5.0), (8.0, 5.0), (8.0, 8.0), (5.0, 8.0)]]);
        let meter = &mut Budget::DEFAULT.meter();
        let (tree, _) = tree(&holed, 1.0, Corners::ROUND, true, meter).unwrap();
        let path = inner_to_outer(&tree, 1.0, p(0.0, 0.0), false, meter).unwrap();
        assert_eq!(path.left_out, 0);
        let points = flat(&path);
        let sides = Sides::new(vec![&points], meter).unwrap();
        for node in &tree.nodes {
            let near = node.ring.iter().filter(|&&q| first_within(&[q, q], &sides, 0.01, meter).unwrap().is_some()).count();
            assert!(near + 2 >= node.ring.len() / 2, "{near} of {} points", node.ring.len());
        }
    }
}
