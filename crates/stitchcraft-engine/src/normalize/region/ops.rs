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
//! part grown alone or parts shrunk, which stay apart, are not joined again.

use stitchcraft_core::{Exhausted, Meter, Point};

use super::{Polygon, Unjoined, area};
use crate::normalize::offset::buffer;

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

/// The part of `a` inside `b`.
pub(crate) fn intersection(a: &[Polygon], b: &[Polygon], meter: &mut Meter) -> Result<Parts, Exhausted> {
    if a.is_empty() || b.is_empty() {
        return Ok(Ok(Vec::new()));
    }
    let rings: Vec<Vec<Point>> = rings_of(a).chain(rings_of(b)).collect();
    Ok(area(&rings, |w| w == -2, meter)?.parts)
}

/// The part of `a` outside `b`.
pub(crate) fn difference(a: &[Polygon], b: &[Polygon], meter: &mut Meter) -> Result<Parts, Exhausted> {
    if a.is_empty() || b.is_empty() {
        return Ok(Ok(a.to_vec()));
    }
    let reversed = rings_of(b).map(|mut ring| {
        ring.reverse();
        ring
    });
    let rings: Vec<Vec<Point>> = rings_of(a).chain(reversed).collect();
    Ok(area(&rings, |w| w == -1, meter)?.parts)
}

/// `parts`, a region, grown by `distance`, or shrunk by a negative distance, with round corners: each part
/// buffered, and the parts grown joined.
pub(crate) fn buffered(parts: &[Polygon], distance: f64, meter: &mut Meter) -> Result<Parts, Exhausted> {
    let mut moved = Vec::new();
    for part in parts {
        let Ok(polygons) = buffer(part, distance, meter)? else { return Ok(Err(Unjoined)) };
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
fn rings_of(parts: &[Polygon]) -> impl Iterator<Item = Vec<Point>> + '_ {
    parts.iter().flat_map(Polygon::rings).map(<[Point]>::to_vec)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

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
