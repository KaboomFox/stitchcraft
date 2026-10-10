//! The rings of a fill's area: the edges between faces it covers and faces it does not, joined into an
//! outline and holes for each part.
//!
//! Each such edge is kept running with the covered face on its right, so outlines come out clockwise and
//! holes counter-clockwise in y-up axes. Where several kept edges meet at a point, which edge follows
//! which decides whether parts touching there stay apart and whether a hole touching its outline stays a
//! hole. As GEOS builds polygons, the edges are linked twice:
//!
//! 1. Round each point, each edge arriving is followed by the next edge leaving, turning
//!    counter-clockwise from the way back. Parts that only touch at the point get rings of their own; an
//!    outline and a hole that touch become one ring through the point twice.
//! 2. Within each such ring, at a point it passes twice, each edge arriving is followed by the edge
//!    leaving just clockwise of the way back. That cuts it into rings that pass no point twice: the
//!    outline, and the holes touching it.
//!
//! A clockwise ring is an outline and takes the holes cut from it; any other hole goes to the smallest
//! outline around it. Each ring starts at its point the drawing reaches first, and the parts and their
//! holes come in that order too.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter};

use super::arrange::Arrangement;
use super::geom::{Envelope, Location, closed, locate_in_ring, signed_area};

/// One part of a fill's area: rings as numbered points of the arrangement, each open (its first point not
/// repeated at its end).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Part {
    /// The outline, clockwise in y-up axes.
    pub(crate) outline: Vec<usize>,
    /// The holes, counter-clockwise.
    pub(crate) holes: Vec<Vec<usize>>,
}

/// The kept edges could not be joined into rings: an edge with no edge to follow it. The arrangement's
/// faces always join, so this is a bug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Unjoined;

impl Unjoined {
    /// What to say when it happens (`SC-E0009`): the fill's region is left empty.
    pub(crate) fn diagnostic(self) -> Diagnostic {
        let message = "The edges of this fill's area could not be joined into rings, so it is not sewn. This is a bug in StitchCraft.";
        Diagnostic::new(Code::InternalCheckFailed, message)
    }
}

/// The parts whose rings run along the half edges marked in `kept` (indexed as [`Arrangement::ends`]),
/// each with the covered face on its right. One unit of `meter` per half edge.
pub(crate) fn parts(arrangement: &Arrangement, kept: &[bool], meter: &mut Meter) -> Result<Result<Vec<Part>, Unjoined>, Exhausted> {
    let halves = 2 * arrangement.edges.len();
    meter.charge(u64::try_from(halves).unwrap_or(u64::MAX))?;
    let is_kept = |h: usize| kept.get(h).copied().unwrap_or(false);
    let next_ccw = arrangement.stars(|h| is_kept(h) || is_kept(h ^ 1));
    let ccw = |h: usize| next_ccw.get(h).copied().unwrap_or(h);
    let max_next = link_maximal(halves, &is_kept, &ccw);
    let Some(rings) = minimal_rings(halves, &is_kept, &ccw, &max_next, arrangement) else { return Ok(Err(Unjoined)) };
    Ok(Ok(join(rings, arrangement)))
}

/// Step 1: for each kept half arriving at a point, the kept half that leaves next, turning
/// counter-clockwise from the way back. Round a point of the boundary the halves arriving and leaving
/// alternate, as filled and empty faces do: just counter-clockwise of each half leaving comes the way back
/// of a half arriving, and then the next half leaving, which the arriving half is linked to. Where they do
/// not alternate (never, from an arrangement), the arriving half is left unlinked, and its ring does not
/// close in step 2.
fn link_maximal(halves: usize, kept: &impl Fn(usize) -> bool, ccw: &impl Fn(usize) -> usize) -> Vec<Option<usize>> {
    let mut next: Vec<Option<usize>> = vec![None; halves];
    for leaving in (0..halves).filter(|&h| kept(h)) {
        let back = ccw(leaving);
        let after = ccw(back);
        let link = (kept(back ^ 1) && kept(after)).then_some(after);
        next.get_mut(back ^ 1).into_iter().for_each(|slot| *slot = link);
    }
    next
}

/// Step 2: the rings that pass no point twice, as points in order, grouped by the ring of step 1 they
/// were cut from.
fn minimal_rings(
    halves: usize,
    kept: &impl Fn(usize) -> bool,
    ccw: &impl Fn(usize) -> usize,
    max_next: &[Option<usize>],
    arrangement: &Arrangement,
) -> Option<Vec<Vec<Vec<usize>>>> {
    let mut max_ring: Vec<Option<usize>> = vec![None; halves];
    let mut starts = Vec::new();
    for start in (0..halves).filter(|&h| kept(h)) {
        if max_ring.get(start).copied().flatten().is_some() {
            continue;
        }
        let id = starts.len();
        starts.push(start);
        let mut h = start;
        // Each step marks a half not marked before, or stops: at most one step per half.
        loop {
            *max_ring.get_mut(h).filter(|slot| slot.is_none())? = Some(id);
            h = max_next.get(h).copied().flatten()?;
            if h == start {
                break;
            }
        }
    }
    let mut min_next: Vec<Option<usize>> = vec![None; halves];
    let mut used = vec![false; halves];
    let mut groups = Vec::with_capacity(starts.len());
    for (id, &start) in starts.iter().enumerate() {
        let in_ring = |h: usize| max_ring.get(h).copied().flatten() == Some(id);
        let mut h = start;
        loop {
            link_minimal_at(h, &in_ring, ccw, &mut min_next);
            h = max_next.get(h).copied().flatten()?;
            if h == start {
                break;
            }
        }
        let mut rings = Vec::new();
        let mut h = start;
        loop {
            if !used.get(h).copied().unwrap_or(true) {
                rings.push(ring_points(h, &min_next, &mut used, arrangement)?);
            }
            h = max_next.get(h).copied().flatten()?;
            if h == start {
                break;
            }
        }
        groups.push(rings);
    }
    Some(groups)
}

/// Links to `leaving` the half of the ring arriving at its start that comes next round the point,
/// turning counter-clockwise from `leaving`: that arriving half then leaves by the half just clockwise of
/// its way back. Called for every half of the ring, it links every arriving half once, as the ring's
/// halves arriving and leaving alternate round each point.
fn link_minimal_at(leaving: usize, in_ring: &impl Fn(usize) -> bool, ccw: &impl Fn(usize) -> usize, min_next: &mut [Option<usize>]) {
    let mut out = ccw(leaving);
    while out != leaving && !in_ring(out ^ 1) {
        out = ccw(out);
    }
    // With no half arriving (never, round a point of a ring), nothing is linked and the ring does not close.
    if out != leaving
        && let Some(slot) = min_next.get_mut(out ^ 1)
    {
        *slot = Some(leaving);
    }
}

/// The points of the ring that starts with half `start`, in order: where each of its halves starts.
fn ring_points(start: usize, min_next: &[Option<usize>], used: &mut [bool], arrangement: &Arrangement) -> Option<Vec<usize>> {
    let mut points = Vec::new();
    let mut h = start;
    // Each step uses a half not used before, or stops: at most one step per half.
    loop {
        *used.get_mut(h).filter(|used| !**used)? = true;
        points.push(arrangement.ends(h)?.0);
        h = min_next.get(h).copied().flatten()?;
        if h == start {
            return Some(points);
        }
    }
}

/// The parts: each clockwise ring an outline with the holes cut from its ring of step 1, and each other
/// hole in the smallest outline around it. Rings start at their first-numbered point, and holes and parts
/// come in the order of those points.
fn join(groups: Vec<Vec<Vec<usize>>>, arrangement: &Arrangement) -> Vec<Part> {
    let polyline = |ring: &[usize]| closed(&ring.iter().map(|&i| arrangement.point(i)).collect::<Vec<_>>());
    let mut parts: Vec<Part> = Vec::new();
    let mut free: Vec<Vec<usize>> = Vec::new();
    for rings in groups {
        let (mut outlines, mut holes) = (Vec::new(), Vec::new());
        for ring in rings {
            let area = signed_area(&polyline(&ring));
            if area < 0.0 {
                outlines.push(ring);
            } else if area > 0.0 {
                holes.push(ring);
            }
        }
        match (outlines.pop(), outlines.is_empty()) {
            (Some(outline), true) => parts.push(Part { outline, holes }),
            (outline, _) => {
                // No outline, or (never, for rings cut from one) several: every ring stands on its own.
                parts.extend(outline.into_iter().chain(outlines).map(|outline| Part { outline, holes: Vec::new() }));
                free.extend(holes);
            }
        }
    }
    for hole in free {
        let env = Envelope::of(&polyline(&hole));
        // An outline is around the hole when its bounds cover the hole's and a point of the hole off the
        // outline lies inside it.
        let around = |part: &Part| {
            let outline = polyline(&part.outline);
            Envelope::of(&outline).covers(&env)
                && hole
                    .iter()
                    .find(|p| !part.outline.contains(p))
                    .is_some_and(|&p| locate_in_ring(arrangement.point(p), &outline) == Location::Interior)
        };
        // Of the outlines around it, the innermost: the smallest.
        let size = |part: &Part| signed_area(&polyline(&part.outline)).abs();
        let innermost =
            parts.iter().enumerate().filter(|(_, part)| around(part)).min_by(|(_, a), (_, b)| size(a).total_cmp(&size(b))).map(|(i, _)| i);
        if let Some(part) = innermost.and_then(|i| parts.get_mut(i)) {
            part.holes.push(hole);
        }
    }
    for part in &mut parts {
        start_at_first(&mut part.outline);
        part.holes.iter_mut().for_each(|hole| start_at_first(hole));
        part.holes.sort_by_key(|hole| hole.first().copied());
    }
    parts.sort_by_key(|part| part.outline.first().copied());
    parts
}

/// Turns `ring` to start at its first-numbered point.
fn start_at_first(ring: &mut [usize]) {
    if let Some(k) = (0..ring.len()).min_by_key(|&i| ring.get(i).copied()) {
        ring.rotate_left(k);
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::{Budget, Point};

    use super::*;
    use crate::normalize::region::arrange::cut;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The parts of the area the rings wind round at least once, each ring as its points.
    fn filled(rings: &[Vec<Point>]) -> Vec<(Vec<Point>, Vec<Vec<Point>>)> {
        let mut meter = Budget::DEFAULT.meter();
        let arrangement = cut(rings, &mut meter).unwrap();
        let faces = arrangement.faces(&mut meter).unwrap();
        let inside = |h: usize| faces.winding.get(faces.right_of[h]).is_some_and(|&w| w != 0);
        let kept: Vec<bool> = (0..2 * arrangement.edges.len()).map(|h| inside(h) && !inside(h ^ 1)).collect();
        let found = parts(&arrangement, &kept, &mut meter).unwrap().unwrap();
        let points = |ring: &[usize]| ring.iter().map(|&i| arrangement.point(i)).collect::<Vec<_>>();
        found.iter().map(|part| (points(&part.outline), part.holes.iter().map(|h| points(h)).collect())).collect()
    }

    fn ring(points: &[(f64, f64)]) -> Vec<Point> {
        points.iter().map(|&(x, y)| p(x, y)).collect()
    }

    #[test]
    fn an_outline_runs_clockwise_from_its_first_point_and_a_hole_the_other_way() {
        let outline = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let hole = ring(&[(2.0, 2.0), (2.0, 4.0), (4.0, 4.0), (4.0, 2.0)]);
        assert_eq!(
            filled(&[outline, hole]),
            [(ring(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0)]), vec![ring(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)])],)]
        );
    }

    #[test]
    fn parts_touching_at_a_point_stay_apart() {
        // A bow tie: 2 triangles meeting where the outline crosses itself.
        let found = filled(&[ring(&[(0.0, 0.0), (4.0, 4.0), (4.0, 0.0), (0.0, 4.0)])]);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0], (ring(&[(0.0, 0.0), (0.0, 4.0), (2.0, 2.0)]), vec![]));
        assert_eq!(found[1], (ring(&[(4.0, 4.0), (4.0, 0.0), (2.0, 2.0)]), vec![]));
    }

    #[test]
    fn a_hole_touching_its_outline_stays_a_hole() {
        let outline = ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let hole = ring(&[(5.0, 0.0), (4.0, 2.0), (6.0, 2.0)]);
        let found = filled(&[outline, hole]);
        assert_eq!(
            found,
            [(ring(&[(0.0, 0.0), (0.0, 10.0), (10.0, 10.0), (10.0, 0.0), (5.0, 0.0)]), vec![ring(&[(5.0, 0.0), (6.0, 2.0), (4.0, 2.0)])],)]
        );
    }

    #[test]
    fn holes_in_holes_are_parts_again_in_the_order_drawn() {
        let rings = [
            ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]),
            ring(&[(2.0, 2.0), (2.0, 8.0), (8.0, 8.0), (8.0, 2.0)]),
            ring(&[(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)]),
        ];
        let found = filled(&rings);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1.len(), 1);
        assert_eq!(found[1], (ring(&[(4.0, 4.0), (4.0, 6.0), (6.0, 6.0), (6.0, 4.0)]), vec![]));
    }

    #[test]
    fn a_hole_apart_from_every_outline_goes_to_the_smallest_around_it() {
        // Four squares inside each other, drawn turning one way and the other in turn: 2 parts, each with
        // a hole, and the innermost hole lies in both outlines.
        let rings = [
            ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]),
            ring(&[(1.0, 1.0), (1.0, 9.0), (9.0, 9.0), (9.0, 1.0)]),
            ring(&[(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)]),
            ring(&[(3.0, 3.0), (3.0, 7.0), (7.0, 7.0), (7.0, 3.0)]),
        ];
        let found = filled(&rings);
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].1, [ring(&[(1.0, 1.0), (9.0, 1.0), (9.0, 9.0), (1.0, 9.0)])]);
        assert_eq!(found[1].1, [ring(&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)])]);
    }

    #[test]
    fn outlines_cut_from_one_ring_each_stand_on_their_own() {
        // Never from an arrangement: 2 outlines in one group are each a part, and the group's hole goes to
        // the outline around it. The squares' points are numbered 0 to 3, 4 to 7 and 8 to 11.
        let rings = [
            ring(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]),
            ring(&[(5.0, 0.0), (9.0, 0.0), (9.0, 4.0), (5.0, 4.0)]),
            ring(&[(1.0, 1.0), (2.0, 1.0), (2.0, 2.0), (1.0, 2.0)]),
        ];
        let arrangement = cut(&rings, &mut Budget::DEFAULT.meter()).unwrap();
        let found = join(vec![vec![vec![3, 2, 1, 0], vec![7, 6, 5, 4], vec![8, 9, 10, 11]]], &arrangement);
        assert_eq!(found, [Part { outline: vec![0, 3, 2, 1], holes: vec![vec![8, 9, 10, 11]] }, Part { outline: vec![4, 7, 6, 5], holes: vec![] }]);
    }

    #[test]
    fn halves_that_do_not_alternate_are_left_unlinked() {
        // Never from an arrangement: round a point, a half leaving followed by 2 halves arriving, and a half
        // arriving round a point with nothing else. Neither arriving half is linked.
        let kept = |h: usize| h == 0 || h == 3;
        let ccw = |h: usize| match h {
            0 => 2,
            2 => 1,
            1 => 0,
            other => other,
        };
        assert_eq!(link_maximal(4, &kept, &ccw), [None; 4]);
    }

    #[test]
    fn rings_of_no_area_are_neither_outlines_nor_holes() {
        // Never from an arrangement: a ring there and back inside a square is left out.
        let rings = [ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]), ring(&[(2.0, 2.0), (3.0, 3.0)])];
        let arrangement = cut(&rings, &mut Budget::DEFAULT.meter()).unwrap();
        let found = join(vec![vec![vec![3, 2, 1, 0]], vec![vec![4, 5]]], &arrangement);
        assert_eq!(found, [Part { outline: vec![0, 3, 2, 1], holes: vec![] }]);
    }

    #[test]
    fn rings_start_at_their_first_numbered_point() {
        let mut ring = vec![3, 1, 2];
        start_at_first(&mut ring);
        assert_eq!(ring, [1, 2, 3]);
        let mut empty: Vec<usize> = vec![];
        start_at_first(&mut empty);
        assert!(empty.is_empty());
    }

    #[test]
    fn edges_that_do_not_join_are_a_bug_said_as_such() {
        // Both halves of one edge kept: neither has a half to follow it.
        let rings = [ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])];
        let mut meter = Budget::DEFAULT.meter();
        let arrangement = cut(&rings, &mut meter).unwrap();
        let kept = vec![true, true, false, false, false, false, false, false];
        let unjoined = parts(&arrangement, &kept, &mut meter).unwrap().unwrap_err();
        assert_eq!(unjoined.diagnostic().code, Code::InternalCheckFailed);
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let rings = [ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)])];
        let mut meter = Budget::DEFAULT.meter();
        let arrangement = cut(&rings, &mut meter).unwrap();
        let kept = vec![false, true, false, true, false, true, false, true];
        assert!(parts(&arrangement, &kept, &mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
        assert_eq!(parts(&arrangement, &kept, &mut meter).unwrap().unwrap().len(), 1);
    }
}
