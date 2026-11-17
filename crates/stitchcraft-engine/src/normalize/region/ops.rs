//! Regions combined and grown: the union of regions, the part of one region inside or outside another,
//! and a region grown or shrunk by a distance, each a region again whose parts are as [`area`] makes them.
//!
//! Automatic digitizing needs them where neighbouring shapes meet (`crate::auto::neighbours`): an area
//! grows under the shapes sewn after it and is cut away under the areas that cover it. Each operation is
//! one call to [`area`], which cuts the rings of both regions where they meet and keeps the faces by how
//! many times the rings wind round them. A region's parts never overlap, and its outlines run clockwise in
//! y-up axes ([`Polygon`]), so the rings of one region wind round each point inside it -1 times and round
//! every other point, a point in a hole included, 0 times. With the rings of 2 regions, then:
//!
//! - the union keeps the faces some ring winds round (`w != 0`);
//! - the intersection keeps the faces both regions wind round (`w == -2`);
//! - the difference reverses the second region's rings, so that they wind +1 round its inside, and keeps
//!   the faces only the first region winds round (`w == -1`).
//!
//! A region grown or shrunk is each part buffered as a fill's part is ([`buffer`]), with round corners,
//! and the grown parts joined, since parts grown apart may overlap.
//!
//! Work goes only where a result needs it: with an empty region on either side, the result is known, and a
//! part grown alone or parts shrunk, which stay apart, are not joined again. Nor do parts far from the
//! other region take part in an intersection or a difference ([`kept`]): an area's neighbours lie all
//! round it, and most of each is far from the strip where it meets them.

use stitchcraft_core::{Exhausted, Meter, Point};

use super::geom::Envelope;
use super::snap::REACH;
use super::{Polygon, Unjoined, area};
use crate::normalize::offset::{Corners, buffer_with};

/// The parts of a region, or [`Unjoined`] should the edges of its area not join into rings (a bug).
pub(crate) type Parts = Result<Vec<Polygon>, Unjoined>;

/// The union of `regions`. One unit of `meter` for the work of [`area`].
pub(crate) fn union<'r>(regions: impl IntoIterator<Item = &'r [Polygon]>, meter: &mut Meter) -> Result<Parts, Exhausted> {
    let rings: Vec<Vec<Point>> = regions.into_iter().flat_map(rings_of).collect();
    if rings.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    Ok(area(&rings, |w| w != 0, meter)?.parts)
}

/// The part of `a` inside `b`. The parts of either that no part of the other comes near are left out
/// ([`kept`]): their parts don't overlap, so only a part of each covers a point of the result. One unit of
/// `meter` for the work of [`kept`] and of [`area`].
pub(crate) fn intersection(a: &[Polygon], b: &[Polygon], meter: &mut Meter) -> Result<Parts, Exhausted> {
    if a.is_empty() || b.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let Some((a, b)) = kept(a, b, |of_a, of_b| of_a && of_b, meter)? else { return Ok(Err(Unjoined)) };
    let rings: Vec<Vec<Point>> = rings_of(a).chain(rings_of(b)).collect();
    if rings.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    Ok(area(&rings, |w| w == -2, meter)?.parts)
}

/// The part of `a` outside `b`. The parts of `b` that no part of `a` comes near are left out ([`kept`]).
/// One unit of `meter` for the work of [`kept`] and of [`area`].
pub(crate) fn difference(a: &[Polygon], b: &[Polygon], meter: &mut Meter) -> Result<Parts, Exhausted> {
    if a.is_empty() || b.is_empty() {
        return Ok(Ok(a.to_vec()));
    }
    let Some((a, b)) = kept(a, b, |of_a, _| of_a, meter)? else { return Ok(Err(Unjoined)) };
    let reversed = rings_of(b).map(|mut ring| {
        ring.reverse();
        ring
    });
    let rings: Vec<Vec<Point>> = rings_of(a).chain(reversed).collect();
    Ok(area(&rings, |w| w == -1, meter)?.parts)
}

/// How far apart, in millimetres, the bounds of 2 groups of parts lie at the least for [`kept`] to take
/// them apart: 2^-20 mm, about a millionth of a millimetre, or 32,768 cells of the grid points are rounded
/// to ([`super::snap::GRID`]). Rounding moves a point by less than a cell, and bends an edge only through a
/// point within a cell of it.
const APART: f64 = 1.0 / 1_048_576.0;

/// The parts of 2 regions that an operation on both needs ([`kept`]), or `None`.
type Kept<'p> = Option<(Vec<&'p Polygon>, Vec<&'p Polygon>)>;

/// The parts of `a` and of `b` that an operation on both needs, each in its order: the parts are taken in
/// groups, a part's bounds within [`APART`] of another's of its group, and a group is kept when `keep`, told
/// whether it holds a part of `a` and a part of `b`, says so; `None` should a part left out have a point
/// beyond the grid's reach, as the whole operation would fail (a bug). The groups' bounds lie apart, so
/// their rings neither meet nor bend each other's edges when they are cut, and each group's faces, rings
/// and parts are those it would have with the others there: the points of every group are numbered in the
/// same order, and so are its parts. A group left out is one whose faces the operation keeps none of. One
/// unit of `meter` for each part and each point of its rings, and for each 2 parts whose bounds are
/// compared.
fn kept<'p>(a: &'p [Polygon], b: &'p [Polygon], keep: impl Fn(bool, bool) -> bool, meter: &mut Meter) -> Result<Kept<'p>, Exhausted> {
    let parts: Vec<&Polygon> = a.iter().chain(b).collect();
    let bounds = bounds_of(&parts, meter)?;
    let mut group = groups(&bounds, APART, meter)?;
    // Whether each group holds a part of `a`, and a part of `b`.
    let mut holds = vec![(false, false); parts.len()];
    for k in 0..parts.len() {
        let root = top(&mut group, k);
        if let Some((of_a, of_b)) = holds.get_mut(root) {
            *of_a |= k < a.len();
            *of_b |= k >= a.len();
        }
    }
    let (mut kept_a, mut kept_b) = (Vec::new(), Vec::new());
    for (k, part) in parts.into_iter().enumerate() {
        let root = top(&mut group, k);
        let (of_a, of_b) = holds.get(root).copied().unwrap_or((true, true));
        if keep(of_a, of_b) {
            if k < a.len() { kept_a.push(part) } else { kept_b.push(part) }
        } else if beyond(part, 0.0) {
            return Ok(None);
        }
    }
    Ok(Some((kept_a, kept_b)))
}

/// `parts`, a region, grown by `distance` as [`buffered`] grows it, but for the parts whose growth can't
/// come within [`APART`] of `near`: what it leaves out of the growth, an intersection with `near` leaves
/// out too ([`kept`]). The parts are taken in groups, each part's bounds grown by `distance` and [`APART`]
/// meeting another's of its group: the growths of 2 groups lie apart, and are joined apart, so the growth
/// of the groups kept is that part of the whole growth, its parts in the same order. They are joined
/// whenever the whole growth would be, from 2 parts. One unit of `meter` for the work of [`buffered`] on
/// the parts kept, for each part and each point of its rings, and for each 2 parts whose bounds are
/// compared.
pub(crate) fn buffered_near(parts: &[Polygon], distance: f64, near: &[Polygon], meter: &mut Meter) -> Result<Parts, Exhausted> {
    if distance <= 0.0 || parts.len() < 2 || near.is_empty() {
        return buffered(parts, distance, meter);
    }
    let all: Vec<&Polygon> = parts.iter().collect();
    let bounds = bounds_of(&all, meter)?;
    let mut group = groups(&bounds, distance + APART, meter)?;
    let near: Vec<&Polygon> = near.iter().collect();
    let around = bounds_of(&near, meter)?.into_iter().reduce(merged);
    let reach = distance + APART;
    let mut meets = vec![false; parts.len()];
    for (k, at) in bounds.iter().enumerate() {
        if around.is_some_and(|around| {
            at.min_x - reach <= around.max_x
                && around.min_x <= at.max_x + reach
                && at.min_y - reach <= around.max_y
                && around.min_y <= at.max_y + reach
        }) {
            let root = top(&mut group, k);
            meets.get_mut(root).into_iter().for_each(|slot| *slot = true);
        }
    }
    let mut moved = Vec::new();
    for (k, part) in parts.iter().enumerate() {
        let root = top(&mut group, k);
        if meets.get(root).copied().unwrap_or(true) {
            let Ok(polygons) = buffer_with(part, distance, Corners::ROUND, meter)? else { return Ok(Err(Unjoined)) };
            moved.extend(polygons);
        } else if beyond(part, distance) {
            return buffered(parts, distance, meter);
        }
    }
    if moved.is_empty() {
        return Ok(Ok(moved));
    }
    union([moved.as_slice()], meter)
}

/// Whether a point of `part`, grown by `distance`, may lie beyond the grid's reach, where cutting it would
/// fail (a bug).
fn beyond(part: &Polygon, distance: f64) -> bool {
    part.rings().flatten().any(|p| p.x().abs().max(p.y().abs()) + distance + APART >= REACH)
}

/// The bounds of each of `parts`. One unit of `meter` for each part and each point of its rings.
fn bounds_of(parts: &[&Polygon], meter: &mut Meter) -> Result<Vec<Envelope>, Exhausted> {
    let mut bounds = Vec::with_capacity(parts.len());
    for part in parts {
        meter.charge(1 + part.rings().map(|ring| ring.len() as u64).sum::<u64>())?;
        bounds.push(part.holes.iter().map(|hole| Envelope::of(hole)).fold(Envelope::of(&part.outline), merged));
    }
    Ok(bounds)
}

/// The bounds of both `a` and `b`.
fn merged(a: Envelope, b: Envelope) -> Envelope {
    Envelope { min_x: a.min_x.min(b.min_x), min_y: a.min_y.min(b.min_y), max_x: a.max_x.max(b.max_x), max_y: a.max_y.max(b.max_y) }
}

/// The groups of `bounds`, each grown by `by` and meeting another of its group: for each, one of its group
/// ([`top`] finds the group's own). A sweep along x compares each with those before it that reach it. One
/// unit of `meter` for each 2 compared.
fn groups(bounds: &[Envelope], by: f64, meter: &mut Meter) -> Result<Vec<usize>, Exhausted> {
    let mut order: Vec<usize> = (0..bounds.len()).collect();
    let least = |i: usize| bounds.get(i).map_or(0.0, |e| e.min_x);
    order.sort_by(|&i, &j| least(i).total_cmp(&least(j)).then(i.cmp(&j)));
    let mut group: Vec<usize> = (0..bounds.len()).collect();
    let mut open: Vec<usize> = Vec::new();
    for &i in &order {
        let Some(&at) = bounds.get(i) else { continue };
        open.retain(|&j| bounds.get(j).is_some_and(|other| other.max_x + 2.0 * by >= at.min_x));
        for &j in &open {
            meter.charge(1)?;
            if bounds.get(j).is_some_and(|other| other.min_y - 2.0 * by <= at.max_y && at.min_y - 2.0 * by <= other.max_y) {
                join(&mut group, i, j);
            }
        }
        open.push(i);
    }
    Ok(group)
}

/// The group `k` is in: the first of its chain in `group`, which then leads there at once.
fn top(group: &mut [usize], k: usize) -> usize {
    let mut root = k;
    while let Some(&up) = group.get(root).filter(|&&up| up != root) {
        root = up;
    }
    let mut at = k;
    while let Some(slot) = group.get_mut(at).filter(|slot| **slot != root) {
        at = std::mem::replace(slot, root);
    }
    root
}

/// Puts the groups of `i` and of `j` together.
fn join(group: &mut [usize], i: usize, j: usize) {
    let (i, j) = (top(group, i), top(group, j));
    if let Some(slot) = group.get_mut(i.max(j)) {
        *slot = i.min(j);
    }
}

/// `parts`, a region, grown by `distance`, or shrunk by a negative distance, with round corners: each part
/// buffered, and the parts grown joined.
pub(crate) fn buffered(parts: &[Polygon], distance: f64, meter: &mut Meter) -> Result<Parts, Exhausted> {
    buffered_with(parts, distance, Corners::ROUND, meter)
}

/// [`buffered`] with `corners`.
pub(crate) fn buffered_with(parts: &[Polygon], distance: f64, corners: Corners, meter: &mut Meter) -> Result<Parts, Exhausted> {
    let mut moved = Vec::new();
    for part in parts {
        let Ok(polygons) = buffer_with(part, distance, corners, meter)? else { return Ok(Err(Unjoined)) };
        moved.extend(polygons);
    }
    // Parts shrunk stay apart, as the parts did; parts grown may now overlap.
    if distance <= 0.0 || moved.len() < 2 {
        return Ok(Ok(moved));
    }
    union([moved.as_slice()], meter)
}

/// The area `parts` cover, in square millimetres.
pub(crate) fn area_of(parts: &[Polygon]) -> f64 {
    parts.iter().map(Polygon::area).sum()
}

/// The rings of `parts`: each part's outline, then its holes.
fn rings_of<'p>(parts: impl IntoIterator<Item = &'p Polygon>) -> impl Iterator<Item = Vec<Point>> {
    parts.into_iter().flat_map(Polygon::rings).map(<[Point]>::to_vec)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::normalize::offset::buffer;

    /// The square from `(x0, y0)` to `(x1, y1)` as a part, wound as a part is: clockwise in y-up axes.
    fn square((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Polygon {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        Polygon { outline: vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], holes: Vec::new() }
    }

    /// `outer` with the square from `(x0, y0)` to `(x1, y1)` as a hole, wound counter-clockwise.
    fn holed(outer: Polygon, (x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Polygon {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        Polygon { holes: vec![vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)]], ..outer }
    }

    fn meter() -> Meter {
        Budget::DEFAULT.meter()
    }

    fn near(got: f64, want: f64) -> bool {
        (got - want).abs() < 1e-9
    }

    #[test]
    fn the_union_joins_overlapping_and_touching_parts_and_fills_a_hole_another_covers() {
        let a = [square((0.0, 0.0), (10.0, 10.0))];
        let b = [square((5.0, 0.0), (15.0, 10.0))];
        let union2 = union([a.as_slice(), b.as_slice()], &mut meter()).unwrap().unwrap();
        assert_eq!(union2.len(), 1);
        assert!(near(area_of(&union2), 150.0), "{}", area_of(&union2));
        // A hole covered by another region is filled.
        let ring = [holed(square((0.0, 0.0), (10.0, 10.0)), (4.0, 4.0), (6.0, 6.0))];
        let plug = [square((4.0, 4.0), (6.0, 6.0))];
        let filled = union([ring.as_slice(), plug.as_slice()], &mut meter()).unwrap().unwrap();
        assert_eq!((filled.len(), filled[0].holes.len()), (1, 0));
        assert!(near(area_of(&filled), 100.0));
        // Apart, the parts stay apart.
        let far = [square((20.0, 0.0), (21.0, 1.0))];
        assert_eq!(union([a.as_slice(), far.as_slice()], &mut meter()).unwrap().unwrap().len(), 2);
        assert!(union(std::iter::empty::<&[Polygon]>(), &mut meter()).unwrap().unwrap().is_empty());
    }

    #[test]
    fn the_intersection_and_the_difference_keep_the_part_inside_and_the_part_outside() {
        let a = [holed(square((0.0, 0.0), (10.0, 10.0)), (1.0, 1.0), (2.0, 2.0))];
        let b = [square((5.0, -5.0), (15.0, 15.0))];
        let inside = intersection(&a, &b, &mut meter()).unwrap().unwrap();
        assert!(near(area_of(&inside), 50.0), "{}", area_of(&inside));
        let outside = difference(&a, &b, &mut meter()).unwrap().unwrap();
        assert!(near(area_of(&outside), 49.0), "the hole stays: {}", area_of(&outside));
        // Taken out of the middle, a region leaves a hole.
        let middle = [square((3.0, 3.0), (7.0, 7.0))];
        let ring = difference(&[square((0.0, 0.0), (10.0, 10.0))], &middle, &mut meter()).unwrap().unwrap();
        assert_eq!((ring.len(), ring[0].holes.len()), (1, 1));
        assert!(near(area_of(&ring), 84.0));
        // Regions that only touch share no area, and taking one from the other leaves it whole.
        let beside = [square((10.0, 0.0), (20.0, 10.0))];
        assert!(intersection(&[square((0.0, 0.0), (10.0, 10.0))], &beside, &mut meter()).unwrap().unwrap().is_empty());
        assert!(near(area_of(&difference(&[square((0.0, 0.0), (10.0, 10.0))], &beside, &mut meter()).unwrap().unwrap()), 100.0));
        // With nothing on either side.
        assert!(intersection(&[], &b, &mut meter()).unwrap().unwrap().is_empty());
        assert!(intersection(&b, &[], &mut meter()).unwrap().unwrap().is_empty());
        assert!(difference(&[], &b, &mut meter()).unwrap().unwrap().is_empty());
        assert_eq!(difference(&b, &[], &mut meter()).unwrap().unwrap(), b.to_vec());
    }

    #[test]
    fn a_region_grows_and_shrinks_with_round_corners_and_its_parts_join_when_they_meet() {
        // Grown by 0.4 mm, a square gains its sides 0.4 mm out and a quarter circle at each corner, drawn
        // with 16 segments, which fall short of the circle by less than a hundredth of a square millimetre.
        let one = [square((0.0, 0.0), (10.0, 10.0))];
        let grown = buffered(&one, 0.4, &mut meter()).unwrap().unwrap();
        let want = 10.0 * 10.0 + 4.0 * 0.4 * 10.0 + std::f64::consts::PI * 0.4 * 0.4;
        assert_eq!(grown.len(), 1);
        assert!((area_of(&grown) - want).abs() < 0.01, "{} against {want}", area_of(&grown));
        // 2 parts 0.5 mm apart, grown by 0.4 mm each, meet and are joined into one.
        let two = [square((0.0, 0.0), (10.0, 10.0)), square((10.5, 0.0), (20.0, 10.0))];
        let joined = buffered(&two, 0.4, &mut meter()).unwrap().unwrap();
        assert_eq!(joined.len(), 1);
        // Where they overlap counts once: a strip 0.3 mm by 10 mm, and where their round corners overlap
        // past its ends, 0.13 mm² in all.
        let apart: f64 = two.iter().map(|part| area_of(&buffered(std::slice::from_ref(part), 0.4, &mut meter()).unwrap().unwrap())).sum();
        assert!((apart - area_of(&joined) - 3.13).abs() < 0.01, "{} against {apart}", area_of(&joined));
        // Shrunk, they stay apart, each 0.4 mm inside its edges.
        let shrunk = buffered(&two, -0.4, &mut meter()).unwrap().unwrap();
        assert_eq!(shrunk.len(), 2);
        assert!(near(area_of(&shrunk), 9.2 * 9.2 + 8.7 * 9.2), "{}", area_of(&shrunk));
        // 3 parts grown into each other are one.
        let three = [square((0.0, 0.0), (10.0, 10.0)), square((10.5, 0.0), (20.0, 10.0)), square((20.5, 0.0), (30.0, 10.0))];
        assert_eq!(buffered(&three, 0.4, &mut meter()).unwrap().unwrap().len(), 1);
    }

    /// What an operation on `a` and `b` gives with every part of both cut together: `filled` keeps the faces
    /// by their winding, `b`'s rings reversed when `reverse`.
    fn whole(a: &[Polygon], b: &[Polygon], reverse: bool, filled: impl Fn(i64) -> bool) -> Parts {
        let b = rings_of(b).map(|mut ring| {
            if reverse {
                ring.reverse();
            }
            ring
        });
        let rings: Vec<Vec<Point>> = rings_of(a).chain(b).collect();
        area(&rings, filled, &mut meter()).unwrap().parts
    }

    #[test]
    fn parts_far_from_the_other_region_are_left_out_and_the_result_is_the_same() {
        // A group of 2 parts of each, one part of `a` on its own, and 2 parts of `b` far from `a`.
        let a = [square((0.0, 0.0), (10.0, 10.0)), square((40.0, 0.0), (50.0, 10.0)), square((-5.0, 0.0), (0.0, 10.0))];
        let b = [
            square((5.0, 0.0), (15.0, 10.0)),
            square((100.0, 0.0), (110.0, 10.0)),
            square((5.0, 20.0), (8.0, 24.0)),
            square((15.0, 0.0), (20.0, 10.0)),
        ];
        assert_eq!(intersection(&a, &b, &mut meter()).unwrap(), whole(&a, &b, false, |w| w == -2));
        assert_eq!(difference(&a, &b, &mut meter()).unwrap(), whole(&a, &b, true, |w| w == -1));
        assert_eq!(difference(&b, &a, &mut meter()).unwrap(), whole(&b, &a, true, |w| w == -1));
        // Parts of `b` all far from `a` are left out, and `a` is still cut on its own, as it would be.
        let far = [square((100.0, 0.0), (110.0, 10.0))];
        assert_eq!(difference(&a, &far, &mut meter()).unwrap(), whole(&a, &far, true, |w| w == -1));
        // Regions far apart share nothing, and finding so costs their bounds alone: a part and each of its 5
        // points, for each of the 2.
        let mut spent = meter();
        assert!(intersection(&a[..1], &far, &mut spent).unwrap().unwrap().is_empty());
        assert_eq!(Budget::DEFAULT.max_work - spent.work_left(), 12);
        // A part left out with a point beyond the grid's reach fails as the whole operation would.
        let beyond = [square((70_000.0, 0.0), (70_001.0, 1.0))];
        assert!(intersection(&a, &beyond, &mut meter()).unwrap().is_err());
        // Half a millimetre apart, 2 regions are apart: they cost their bounds alone.
        let mut spent = meter();
        assert!(intersection(&a[..1], &[square((10.5, 0.0), (12.0, 10.0))], &mut spent).unwrap().unwrap().is_empty());
        assert_eq!(Budget::DEFAULT.max_work - spent.work_left(), 12);
    }

    #[test]
    fn bounds_within_twice_the_distance_of_each_other_are_one_group() {
        let at = |min_x: f64, min_y: f64, max_x: f64, max_y: f64| Envelope { min_x, min_y, max_x, max_y };
        let together = |a: Envelope, b: Envelope| {
            let mut spent = meter();
            let mut group = groups(&[a, b], 0.5, &mut spent).unwrap();
            (top(&mut group, 0) == top(&mut group, 1), Budget::DEFAULT.max_work - spent.work_left())
        };
        // 1 apart along x, each grown by 0.5: they meet, and the 2 are compared once.
        assert_eq!(together(at(0.0, 0.0, 1.0, 1.0), at(2.0, 0.0, 3.0, 1.0)), (true, 1));
        assert_eq!(together(at(2.0, 0.0, 3.0, 1.0), at(0.0, 0.0, 1.0, 1.0)), (true, 1));
        // Further apart along x, they aren't compared.
        assert_eq!(together(at(0.0, 0.0, 1.0, 1.0), at(2.25, 0.0, 3.0, 1.0)), (false, 0));
        // Level along x, 1 apart along y they meet, either way round, and further apart they don't.
        assert_eq!(together(at(0.0, 0.0, 1.0, 1.0), at(0.0, 2.0, 1.0, 3.0)), (true, 1));
        assert_eq!(together(at(0.0, 2.0, 1.0, 3.0), at(0.0, 0.0, 1.0, 1.0)), (true, 1));
        assert_eq!(together(at(0.0, 0.0, 1.0, 1.0), at(0.0, 2.25, 1.0, 3.0)), (false, 1));
        assert_eq!(together(at(0.0, 2.25, 1.0, 3.0), at(0.0, 0.0, 1.0, 1.0)), (false, 1));
        // A chain of 3, the first and the last apart, is one group.
        let mut group = groups(&[at(0.0, 0.0, 1.0, 1.0), at(4.0, 0.0, 5.0, 1.0), at(2.0, 0.0, 3.0, 1.0)], 0.5, &mut meter()).unwrap();
        assert!((0..3).all(|k| top(&mut group, k) == top(&mut group, 0)));
    }

    #[test]
    fn a_region_grown_near_another_keeps_the_growth_that_reaches_it() {
        let parts = [square((0.0, 0.0), (1.0, 1.0)), square((1.5, 0.0), (2.5, 1.0)), square((50.0, 0.0), (51.0, 1.0))];
        let near = [square((0.5, 0.5), (2.0, 2.0))];
        let all = buffered(&parts, 0.4, &mut meter()).unwrap().unwrap();
        assert_eq!(all.len(), 2, "the first 2 grow into one");
        // The growth of the part far away is left out, and the rest is the same.
        let kept = buffered_near(&parts, 0.4, &near, &mut meter()).unwrap().unwrap();
        assert_eq!(kept, all[..1]);
        assert_eq!(intersection(&near, &kept, &mut meter()).unwrap(), intersection(&near, &all, &mut meter()).unwrap());
        // A part kept alone is joined, as the whole growth is.
        let one = [square((0.0, 0.0), (1.0, 1.0)), square((50.0, 0.0), (51.0, 1.0))];
        let whole_growth = buffered(&one, 0.4, &mut meter()).unwrap().unwrap();
        assert_eq!(buffered_near(&one, 0.4, &near, &mut meter()).unwrap().unwrap(), whole_growth[..1]);
        // Near nothing, nothing is grown.
        let mut spent = meter();
        let none = buffered_near(&parts, 0.4, &[square((20.0, 20.0), (21.0, 21.0))], &mut spent).unwrap().unwrap();
        assert!(none.is_empty());
        // The 3 parts' bounds and the one they are near, 6 units each, and the first 2 compared.
        assert_eq!(Budget::DEFAULT.max_work - spent.work_left(), 4 * 6 + 1);
        // Shrunk, alone, or near no region, the whole is grown.
        for (parts, distance, near) in [(&parts[..], -0.2, &near[..]), (&parts[..1], 0.4, &near[..]), (&parts[..], 0.4, &[][..])] {
            assert_eq!(buffered_near(parts, distance, near, &mut meter()).unwrap(), buffered(parts, distance, &mut meter()).unwrap());
        }
        // A part left out whose growth may pass the grid's reach is grown with the rest, and fails as the
        // whole growth does.
        let beyond = [square((0.0, 0.0), (1.0, 1.0)), square((65_535.5, 0.0), (65_535.8, 1.0))];
        assert!(buffered_near(&beyond, 0.4, &near, &mut meter()).unwrap().is_err());
        assert!(buffered(&beyond, 0.4, &mut meter()).unwrap().is_err());
        // Just short of the reach, it is grown with the rest, and the growth then stays within it.
        let edge = REACH - 0.4 - APART / 2.0;
        let short = [square((0.0, 0.0), (1.0, 1.0)), square((edge - 0.5, 0.0), (edge, 1.0))];
        assert_eq!(buffered_near(&short, 0.4, &near, &mut meter()).unwrap().unwrap().len(), 2);
    }

    #[test]
    fn a_region_reaches_another_within_the_distance_and_a_millionth_of_a_millimetre() {
        // A unit square at the origin and one far away, grown by 0.4 mm near a square just within 0.4 mm
        // and half of `APART` of the first, on each side in turn: the first is grown, the far one not.
        let parts = [square((0.0, 0.0), (1.0, 1.0)), square((50.0, 50.0), (51.0, 51.0))];
        let first = buffered(&parts, 0.4, &mut meter()).unwrap().unwrap()[..1].to_vec();
        let gap = 0.4 + APART / 2.0;
        // Each side's square, and a second one 3 mm further out, which the bounds of both take in.
        let sides = [(1.0 + gap, 0.0, 1.0, 0.0), (-1.0 - gap, 0.0, -1.0, 0.0), (0.0, 1.0 + gap, 0.0, 1.0), (0.0, -1.0 - gap, 0.0, -1.0)];
        for (x, y, dx, dy) in sides {
            let at = |by: f64| square((x + by * dx, y + by * dy), (x + by * dx + 1.0, y + by * dy + 1.0));
            let near = [at(0.0), at(3.0)];
            assert_eq!(buffered_near(&parts, 0.4, &near, &mut meter()).unwrap().unwrap(), first, "{x}, {y}");
        }
        // A part within twice 0.4 mm and `APART` of the one reached is grown with it, and one further isn't.
        let next = 1.0 + 0.8 + APART;
        let chained = [square((0.0, 0.0), (1.0, 1.0)), square((next, 0.0), (next + 1.0, 1.0))];
        let near = [square((-0.6, 0.0), (-0.2, 1.0))];
        assert_eq!(buffered_near(&chained, 0.4, &near, &mut meter()).unwrap().unwrap().len(), 2);
        let apart = [square((0.0, 0.0), (1.0, 1.0)), square((next + 3.0 * APART, 0.0), (next + 1.0, 1.0))];
        assert_eq!(buffered_near(&apart, 0.4, &near, &mut meter()).unwrap().unwrap(), first);
    }

    #[test]
    fn work_goes_only_where_a_result_needs_it() {
        // With an empty region on either side, the result is known, and costs nothing.
        let b = [square((0.0, 0.0), (10.0, 10.0))];
        let mut none = Budget { max_work: 0, ..Budget::DEFAULT }.meter();
        assert!(intersection(&[], &b, &mut none).unwrap().unwrap().is_empty());
        assert!(intersection(&b, &[], &mut none).unwrap().unwrap().is_empty());
        assert!(difference(&[], &b, &mut none).unwrap().unwrap().is_empty());
        assert_eq!(difference(&b, &[], &mut none).unwrap().unwrap(), b.to_vec());
        // A part grown alone, and parts shrunk, which stay apart, cost their buffers' work and no more.
        let used = |run: &dyn Fn(&mut Meter)| {
            let mut meter = meter();
            run(&mut meter);
            Budget::DEFAULT.max_work - meter.work_left()
        };
        let two = [square((0.0, 0.0), (10.0, 10.0)), square((10.5, 0.0), (20.0, 10.0))];
        assert_eq!(used(&|m| drop(buffered(&b, 0.4, m))), used(&|m| drop(buffer(&b[0], 0.4, m))));
        assert_eq!(used(&|m| drop(buffered(&two, -0.4, m))), used(&|m| two.iter().for_each(|part| drop(buffer(part, -0.4, m)))));
    }
}
