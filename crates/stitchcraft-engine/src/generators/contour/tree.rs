//! A contour fill's rings: a part's outline shrunk and its holes grown, level by level, and the tree they
//! make (`REQ-FILL-CON-001`, `docs/src/design/algorithms/fills.md` › Contour fill › Rings).
//!
//! **Levels.** Level 0 is the part's outline and holes. The next level shrinks the outline ring by the row
//! spacing, without its holes, grows each hole ring by it and keeps the outer edge of each growth, and
//! takes the grown holes, joined, from the shrunk outline. What is left falls into pieces, each an outline
//! with holes, and each piece makes a level of its own in turn, until nothing is left. Shrinking and
//! growing are the offsetter's buffers with the corners `join_style` gives ([`Corners`]), whose outlines are
//! the curves Ink/Stitch takes from shapely's `offset_curve`. Each ring is simplified within 0.01 CSS pixel,
//! and a piece smaller than 0.1 square CSS pixel is left out with its holes, as Ink/Stitch does.
//!
//! **Joined, not repaired.** Where 2 holes' growths overlap, Ink/Stitch repairs the pair as one polygon of
//! 2 parts, which keeps what they share as an island. Here they are joined (`DEV-FILL-015`).
//!
//! **The tree.** A shrunk outline ring is a child of the ring it was shrunk from. A hole ring grown from
//! others is their parent: going out from a hole, each ring is the parent of the one inside it. A hole
//! ring that grows into the outline, or into nothing, hangs under the outline ring of its level. So the
//! tree runs from the part's outline in to the middles of its pieces, and out again to each hole, and a
//! path through it reaches every ring.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::normalize::offset::{Corners, buffer_with};
use crate::normalize::region::geom::{Location, locate_in_ring, signed_area};
use crate::normalize::region::ops::{difference, union};
use crate::normalize::region::{Polygon, Unjoined};
use crate::normalize::simplify::simplify;

/// Each ring is simplified within this, in millimetres: 0.01 CSS pixel.
const SIMPLIFIED: f64 = 0.01 * MM_PER_SVG_PX;
/// A piece smaller than this, in square millimetres, is left out with its holes: 0.1 square CSS pixel.
const SMALLEST: f64 = 0.1 * MM_PER_SVG_PX * MM_PER_SVG_PX;

/// One ring of the tree.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Node {
    /// The ring, closed (its first point repeated at its end).
    pub(super) ring: Vec<Point>,
    /// The rings under it, in the order they were found.
    pub(super) children: Vec<usize>,
}

/// A part's rings as a tree, the part's outline first: the root.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Tree {
    pub(super) nodes: Vec<Node>,
}

/// The root of every tree: the part's outline.
pub(super) const ROOT: usize = 0;

impl Tree {
    /// The ring of `node`; empty for a node the tree does not have.
    pub(super) fn ring(&self, node: usize) -> &[Point] {
        self.nodes.get(node).map_or(&[], |n| n.ring.as_slice())
    }

    /// The rings under `node`.
    pub(super) fn children(&self, node: usize) -> &[usize] {
        self.nodes.get(node).map_or(&[], |n| n.children.as_slice())
    }

    /// How many rings `node` and the rings under it make.
    pub(super) fn count(&self, node: usize) -> usize {
        let (mut count, mut stack) = (0, vec![node]);
        while let Some(next) = stack.pop() {
            count += 1;
            stack.extend_from_slice(self.children(next));
        }
        count
    }
}

/// The tree of `part`'s rings `d` apart, with `corners`, each ring running clockwise as the drawing shows
/// it (y down) when `clockwise`, and the other way otherwise; and whether a level's edges failed to join
/// into rings (a bug), which leaves out what lies inside it. One unit of `meter` for each level, and what
/// the buffers and the joining cost.
pub(super) fn tree(part: &Polygon, d: f64, corners: Corners, clockwise: bool, meter: &mut Meter) -> Result<(Tree, Option<Unjoined>), Exhausted> {
    let mut building = Building { nodes: vec![Node { ring: part.outline.clone(), children: Vec::new() }], parents: vec![None] };
    let holes: Vec<usize> = part.holes.iter().map(|hole| building.add(hole.clone(), None)).collect();
    let mut failed = None;
    // Last found, first shrunk, as Ink/Stitch takes them.
    let mut levels = vec![(ROOT, holes)];
    while let Some((outline, holes)) = levels.pop() {
        meter.charge(1)?;
        match building.level(outline, &holes, d, corners, meter)? {
            Ok(found) => levels.extend(found),
            Err(unjoined) => failed = Some(unjoined),
        }
        // A hole whose growth no hole of the next level holds hangs under this level's outline.
        for &hole in &holes {
            if building.parents.get(hole).is_some_and(Option::is_none) {
                building.adopt(outline, hole);
            }
        }
    }
    let mut nodes = building.nodes;
    for node in &mut nodes {
        if (signed_area(&node.ring) > 0.0) != clockwise {
            node.ring.reverse();
        }
    }
    Ok((Tree { nodes }, failed))
}

/// A tree being built: its nodes, and the parent of each, once found.
struct Building {
    nodes: Vec<Node>,
    parents: Vec<Option<usize>>,
}

/// A level found: an outline ring and the hole rings in it.
type Level = (usize, Vec<usize>);

impl Building {
    /// Adds `ring` under `parent`, and gives its number.
    fn add(&mut self, ring: Vec<Point>, parent: Option<usize>) -> usize {
        let node = self.nodes.len();
        self.nodes.push(Node { ring, children: Vec::new() });
        self.parents.push(parent);
        if let Some(parent) = parent {
            self.adopt_child(parent, node);
        }
        node
    }

    /// Makes `parent` the parent of `child`.
    fn adopt(&mut self, parent: usize, child: usize) {
        if let Some(slot) = self.parents.get_mut(child) {
            *slot = Some(parent);
        }
        self.adopt_child(parent, child);
    }

    fn adopt_child(&mut self, parent: usize, child: usize) {
        if let Some(node) = self.nodes.get_mut(parent) {
            node.children.push(child);
        }
    }

    /// The levels inside the outline ring `outline` and its hole rings `holes`: the shrunk outline less the
    /// grown holes, in pieces, each added as an outline ring under `outline` with its holes; a piece's hole
    /// becomes the parent of the holes of `holes` it holds.
    fn level(
        &mut self,
        outline: usize,
        holes: &[usize],
        d: f64,
        corners: Corners,
        meter: &mut Meter,
    ) -> Result<Result<Vec<Level>, Unjoined>, Exhausted> {
        let ring = |node: usize| self.nodes.get(node).map_or(&[][..], |n| n.ring.as_slice());
        let Ok(shrunk) = moved(ring(outline), -d, corners, meter)? else { return Ok(Err(Unjoined)) };
        let mut grown = Vec::with_capacity(holes.len());
        for &hole in holes {
            // A hole ring runs the other way from an outline: reversed, it is the outline of what it holds.
            let reversed: Vec<Point> = ring(hole).iter().rev().copied().collect();
            let Ok(growth) = moved(&reversed, d, corners, meter)? else { return Ok(Err(Unjoined)) };
            grown.extend(growth.into_iter().take(1));
        }
        let joined = if grown.len() < 2 {
            grown
        } else {
            let Ok(parts) = union([grown.as_slice()], meter)? else { return Ok(Err(Unjoined)) };
            parts
        };
        let Ok(pieces) = difference(&shrunk, &joined, meter)? else { return Ok(Err(Unjoined)) };
        let mut found = Vec::new();
        for piece in pieces {
            if piece.area() < SMALLEST || !valid(&piece.outline) {
                continue;
            }
            let node = self.add(piece.outline, Some(outline));
            let mut piece_holes = Vec::with_capacity(piece.holes.len());
            for hole_ring in piece.holes {
                meter.charge(u64::try_from(holes.len()).unwrap_or(u64::MAX))?;
                let held: Vec<usize> =
                    holes.iter().copied().filter(|&old| self.unadopted(old) && holds(&hole_ring, ring_of(&self.nodes, old))).collect();
                let hole = self.add(hole_ring, None);
                for old in held {
                    self.adopt(hole, old);
                }
                piece_holes.push(hole);
            }
            found.push((node, piece_holes));
        }
        Ok(Ok(found))
    }

    /// Whether `node` has no parent yet.
    fn unadopted(&self, node: usize) -> bool {
        self.parents.get(node).is_some_and(Option::is_none)
    }
}

/// The ring of node `node` of `nodes`.
fn ring_of(nodes: &[Node], node: usize) -> &[Point] {
    nodes.get(node).map_or(&[], |n| n.ring.as_slice())
}

/// The outline ring `ring` buffered by `distance` with `corners`: the outlines of the parts, each
/// simplified, those that are still rings. Ink/Stitch takes a grown hole's first ring alone.
fn moved(ring: &[Point], distance: f64, corners: Corners, meter: &mut Meter) -> Result<Result<Vec<Polygon>, Unjoined>, Exhausted> {
    let polygon = Polygon { outline: ring.to_vec(), holes: Vec::new() };
    let Ok(parts) = buffer_with(&polygon, distance, corners, meter)? else { return Ok(Err(Unjoined)) };
    let mut kept = Vec::with_capacity(parts.len());
    for part in parts {
        let outline = simplify(&part.outline, SIMPLIFIED, meter)?;
        if valid(&outline) {
            kept.push(Polygon { outline, holes: Vec::new() });
        }
    }
    Ok(Ok(kept))
}

/// Whether the closed `ring` is still a ring: 3 points or more, the first repeated at its end.
fn valid(ring: &[Point]) -> bool {
    ring.len() > 3
}

/// Whether the ring `outer` holds the ring `inner`, which lies wholly inside it or wholly outside: its
/// first point lies inside.
fn holds(outer: &[Point], inner: &[Point]) -> bool {
    inner.first().is_some_and(|&p| locate_in_ring(p, outer) == Location::Interior)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::design::Join;
    use crate::normalize::fixture::part;

    fn rings_of(part: &Polygon, d: f64, corners: Corners) -> Tree {
        let (tree, failed) = tree(part, d, corners, true, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(failed, None);
        tree
    }

    /// The tree's shape: each node's children, by number.
    fn shape(tree: &Tree) -> Vec<Vec<usize>> {
        tree.nodes.iter().map(|n| n.children.clone()).collect()
    }

    #[test]
    fn a_square_s_rings_are_squares_each_inside_the_last() {
        // A 10 mm square, rings 1 mm apart: squares 8, 6, 4 and 2 mm across, each the child of the last, and
        // nothing past a square of 0 mm.
        let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[]);
        let tree = rings_of(&square, 1.0, Corners::ROUND);
        assert_eq!(shape(&tree), [vec![1], vec![2], vec![3], vec![4], vec![]]);
        for (node, side) in [(1, 8.0), (2, 6.0), (3, 4.0), (4, 2.0)] {
            let ring = tree.ring(node);
            let (min, max) = ring.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), q| (lo.min(q.x()), hi.max(q.x())));
            assert!((max - min - side).abs() < 1e-9, "ring {node}: {min}..{max}");
        }
        assert_eq!(tree.count(ROOT), 5);
        assert_eq!(tree.count(4), 1);
    }

    #[test]
    fn rings_run_clockwise_as_drawn_or_the_other_way() {
        let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[&[(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)]]);
        for clockwise in [true, false] {
            let (tree, _) = tree(&square, 1.0, Corners::ROUND, clockwise, &mut Budget::DEFAULT.meter()).unwrap();
            assert!(tree.nodes.iter().all(|n| (signed_area(&n.ring) > 0.0) == clockwise));
        }
    }

    #[test]
    fn a_hole_s_rings_grow_out_from_it_and_hang_where_they_meet_the_outline() {
        // A 20 mm square with a 2 mm hole in its middle, rings 2 mm apart. The outline shrinks to 16, 12 and
        // 8 mm, and the hole grows to 6, 10 and 14 mm (round corners). At level 3 the outline, 8 mm across,
        // lies inside the hole's growth, and nothing is left: the hole's rings hang under level 2's outline
        // ring, the largest first.
        let holed = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], &[&[(9.0, 9.0), (11.0, 9.0), (11.0, 11.0), (9.0, 11.0)]]);
        let tree = rings_of(&holed, 2.0, Corners::ROUND);
        // 0 the outline, 1 the hole; 2 the outline at 16 mm with 3 its hole at 6 mm; 4 at 12 mm with 5 at 10 mm.
        assert_eq!(shape(&tree), [vec![2], vec![], vec![4], vec![1], vec![5], vec![3]]);
        assert_eq!(tree.count(ROOT), 6);
    }

    #[test]
    fn the_growths_of_2_holes_that_overlap_are_joined_into_one_hole_ring() {
        // A 20 mm square with 2 holes 2 mm across and 1 mm apart, rings 1 mm apart. Grown by 1 mm, the holes
        // overlap, and the first level's piece has one hole ring round both: the parent of each (DEV-FILL-015).
        let holed = part(
            &[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)],
            &[&[(7.0, 9.0), (9.0, 9.0), (9.0, 11.0), (7.0, 11.0)], &[(10.0, 9.0), (12.0, 9.0), (12.0, 11.0), (10.0, 11.0)]],
        );
        let tree = rings_of(&holed, 1.0, Corners::ROUND);
        // 0 the outline, 1 and 2 the holes; 3 the outline shrunk by 1 mm, 4 the hole ring round both.
        assert_eq!(tree.children(ROOT)[0], 3);
        assert_eq!(tree.children(4), [1, 2]);
        assert_eq!(tree.nodes.iter().filter(|n| n.children.contains(&1) || n.children.contains(&2)).count(), 1);
    }

    #[test]
    fn a_level_whose_edges_do_not_join_leaves_out_what_lies_inside_it() {
        // A square 70 m from the origin lies past the 65.536 m the region's grid reaches, so its first level
        // cannot be cut into rings: the tree is the part's outline alone, and says the level failed.
        let far = part(&[(70_000.0, 0.0), (70_010.0, 0.0), (70_010.0, 10.0), (70_000.0, 10.0)], &[]);
        let (tree, failed) = tree(&far, 1.0, Corners::ROUND, true, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(failed, Some(Unjoined));
        assert_eq!(tree.nodes.len(), 1);
    }

    #[test]
    fn a_part_that_narrows_parts_in_two() {
        // 2 squares of 10 mm joined by a neck 2 mm wide: shrunk by 1.5 mm, the neck is gone and the outline
        // ring has 2 children.
        let dumbbell = part(
            &[
                (0.0, 0.0),
                (10.0, 0.0),
                (10.0, 4.0),
                (14.0, 4.0),
                (14.0, 0.0),
                (24.0, 0.0),
                (24.0, 10.0),
                (14.0, 10.0),
                (14.0, 6.0),
                (10.0, 6.0),
                (10.0, 10.0),
                (0.0, 10.0),
            ],
            &[],
        );
        let tree = rings_of(&dumbbell, 1.5, Corners::ROUND);
        assert_eq!(tree.children(ROOT).len(), 2);
    }

    #[test]
    fn mitred_rings_keep_their_corners() {
        // An L shrunk with mitred corners turns its inner corner, at (4, 4), sharply at (3, 3), where round
        // corners turn on an arc round it.
        let l = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 4.0), (4.0, 4.0), (4.0, 10.0), (0.0, 10.0)], &[]);
        let mitred = Corners { join: Join::Miter { limit: 10.0 }, quadrant_segments: 8 };
        let tree = rings_of(&l, 1.0, mitred);
        assert!(tree.ring(1).iter().any(|q| q.distance(Point::new(3.0, 3.0).unwrap()) < 1e-9), "{:?}", tree.ring(1));
        let round = rings_of(&l, 1.0, Corners { join: Join::Round, quadrant_segments: 8 });
        assert!(round.ring(1).len() > tree.ring(1).len());
    }

    #[test]
    fn a_speck_is_left_out() {
        // A square 0.2 mm across makes no ring of 0.1 square pixel: shrunk by 0.05 mm, it is 0.1 mm across,
        // 0.01 mm², more than 0.007; by 0.09 mm it is 0.02 mm across, 0.0004 mm², left out.
        let speck = part(&[(0.0, 0.0), (0.2, 0.0), (0.2, 0.2), (0.0, 0.2)], &[]);
        assert_eq!(rings_of(&speck, 0.05, Corners::ROUND).children(ROOT).len(), 1);
        assert_eq!(rings_of(&speck, 0.09, Corners::ROUND).children(ROOT).len(), 0);
        assert!(valid(&[Point::ORIGIN; 4]) && !valid(&[Point::ORIGIN; 3]));
        assert_eq!(Tree { nodes: Vec::new() }.ring(3), &[] as &[Point]);
    }
}
