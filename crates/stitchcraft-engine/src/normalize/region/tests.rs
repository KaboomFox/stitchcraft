//! The region against the winding rule itself: for random rings drawn on a coarse grid, where they cross,
//! touch and run along each other often, every point tested lies in the region exactly when the rings
//! wind round it as the fill rule asks.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::exact::{self, Side};
use stitchcraft_core::{Budget, Point};

use super::geom::{Location, locate_in_ring, on_segment};
use super::*;
use crate::design::{Segment, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A path of closed subpaths through `rings`' points.
fn path(rings: &[Vec<Point>]) -> Path {
    Path {
        subpaths: rings
            .iter()
            .filter_map(|ring| {
                let (&start, rest) = ring.split_first()?;
                Some(Subpath { start, segments: rest.iter().map(|&q| Segment::Line(q)).collect(), closed: true })
            })
            .collect(),
    }
}

fn region(rings: &[Vec<Point>], rule: FillRule) -> Built {
    build(&path(rings), rule, &mut Budget::DEFAULT.meter()).unwrap()
}

/// How many times the closed rings wind round `q`, counted directly.
fn winding_at(rings: &[Vec<Point>], q: Point) -> i64 {
    let mut w = 0;
    for ring in rings {
        let closed_ring = geom::closed(ring);
        for pair in closed_ring.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            if a.y() <= q.y() {
                if b.y() > q.y() && exact::side(a, b, q) == Side::Left {
                    w += 1;
                }
            } else if b.y() <= q.y() && exact::side(a, b, q) == Side::Right {
                w -= 1;
            }
        }
    }
    w
}

/// Whether `q` lies in the region: inside a part's outline and outside its holes.
fn contains(region: &Region, q: Point) -> bool {
    region.parts.iter().any(|part| {
        locate_in_ring(q, &part.outline) == Location::Interior && part.holes.iter().all(|hole| locate_in_ring(q, hole) == Location::Exterior)
    })
}

/// Whether `q` lies on a segment of the rings, or within rounding of one: a crossing is worked out in
/// floating point, so the edges that end there may pass a hair to either side of the segments they cut.
fn on_rings(rings: &[Vec<Point>], q: Point) -> bool {
    rings.iter().any(|ring| geom::closed(ring).windows(2).any(|w| on_segment(q, w[0], w[1]) || stroke::distance_to_segment(q, w[0], w[1]) < 1e-9))
}

fn rings_strategy() -> impl Strategy<Value = Vec<Vec<Point>>> {
    let point = (0_i32..12, 0_i32..12).prop_map(|(x, y)| p(f64::from(x), f64::from(y)));
    prop::collection::vec(prop::collection::vec(point, 3..7), 1..4)
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(400))]

    #[test]
    fn points_lie_in_the_region_as_the_rings_wind_round_them(rings in rings_strategy(), even_odd in any::<bool>()) {
        let rule = if even_odd { FillRule::EvenOdd } else { FillRule::NonZero };
        let built = region(&rings, rule);
        // Only a part left out for its size may leave a point the rule fills outside the region.
        let left_out = built.diagnostics.iter().any(|d| d.code == Code::FillPartsTooSmall);
        let grid = (0..24).flat_map(|i| (0..24).map(move |j| p(f64::from(i) * 0.5 + 0.13, f64::from(j) * 0.5 + 0.29)));
        for q in grid.filter(|&q| !on_rings(&rings, q)) {
            let w = winding_at(&rings, q);
            let expected = if even_odd { w.rem_euclid(2) == 1 } else { w != 0 };
            let inside = contains(&built.region, q);
            prop_assert!(inside == expected || (expected && left_out), "{q:?} winds {w}, and lies in the region: {inside}");
        }
        for part in &built.region.parts {
            prop_assert!(geom::signed_area(&part.outline) < 0.0, "outlines run clockwise in y-up axes");
            prop_assert!(part.holes.iter().all(|hole| geom::signed_area(hole) > 0.0), "holes the other way");
            for ring in std::iter::once(&part.outline).chain(&part.holes) {
                let open = &ring[..ring.len() - 1];
                let mut sorted: Vec<(u64, u64)> = open.iter().map(|q| (q.x().to_bits(), q.y().to_bits())).collect();
                sorted.sort_unstable();
                let distinct = sorted.len();
                sorted.dedup();
                prop_assert_eq!(sorted.len(), distinct, "a ring passes no point twice");
            }
        }
    }
}

#[test]
fn the_fill_rule_decides_rings_inside_rings_drawn_the_same_way() {
    let outer = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    let inner = vec![p(3.0, 3.0), p(7.0, 3.0), p(7.0, 7.0), p(3.0, 7.0)];
    let nonzero = region(&[outer.clone(), inner.clone()], FillRule::NonZero);
    assert_eq!(nonzero.region.parts.len(), 1);
    assert!(nonzero.region.parts[0].holes.is_empty());
    assert_eq!(nonzero.region.area(), 100.0);
    assert_eq!(nonzero.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillRuleNotEvenOdd]);
    let even_odd = region(&[outer.clone(), inner.clone()], FillRule::EvenOdd);
    assert_eq!(even_odd.region.area(), 84.0);
    assert!(even_odd.diagnostics.is_empty());
    // Drawn the other way round, the inner ring is a hole under both rules.
    let mut turned = inner;
    turned.reverse();
    let nonzero = region(&[outer, turned], FillRule::NonZero);
    assert_eq!(nonzero.region.area(), 84.0);
    assert!(nonzero.diagnostics.is_empty());
}

#[test]
fn subpaths_of_fewer_than_3_points_bound_nothing() {
    let square = vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)];
    let built = region(&[square.clone(), vec![p(1.0, 1.0)], vec![p(2.0, 2.0), p(3.0, 2.0)]], FillRule::NonZero);
    assert_eq!(built, region(&[square], FillRule::NonZero));
    // With nothing bounded at all, the fill says so.
    let none = region(&[], FillRule::NonZero);
    assert!(none.region.parts.is_empty());
    assert_eq!(none.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillPartsTooSmall]);
}

#[test]
fn rings_start_where_the_drawing_reaches_them_first() {
    // A square drawn counter-clockwise from its second corner: turned clockwise, it still starts there.
    let built = region(&[vec![p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(0.0, 0.0)]], FillRule::NonZero);
    assert_eq!(built.region.parts[0].outline, [p(4.0, 0.0), p(0.0, 0.0), p(0.0, 4.0), p(4.0, 4.0), p(4.0, 0.0)]);
}

#[test]
fn parts_of_exactly_3_square_pixels_are_left_out_and_fills_of_exactly_20_are_not_small() {
    // Ink/Stitch keeps parts larger than 3 square pixels, and calls fills smaller than 20 small.
    let rectangle = |w: f64| vec![p(0.0, 0.0), p(w, 0.0), p(w, MM_PER_SVG_PX), p(0.0, MM_PER_SVG_PX)];
    let three = region(&[rectangle(3.0 * MM_PER_SVG_PX)], FillRule::NonZero);
    assert!(three.region.parts.is_empty());
    assert_eq!(three.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillPartsTooSmall]);
    let twenty = region(&[rectangle(20.0 * MM_PER_SVG_PX)], FillRule::NonZero);
    assert_eq!(twenty.region.area(), SMALL_FILL);
    assert!(twenty.diagnostics.is_empty());
}

#[test]
fn a_part_covers_its_inside_and_its_rings_but_not_its_holes() {
    let square = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    let hole = vec![p(4.0, 4.0), p(6.0, 4.0), p(6.0, 6.0), p(4.0, 6.0)];
    let built = region(&[square, hole], FillRule::EvenOdd);
    let part = &built.region.parts[0];
    let covers = |x, y| part.covers(p(x, y));
    assert!(covers(2.0, 2.0) && covers(0.0, 5.0) && covers(4.0, 5.0), "inside, on the outline, on the hole's ring");
    assert!(!covers(5.0, 5.0) && !covers(11.0, 5.0), "in the hole, outside");
}

#[test]
fn holes_come_in_the_order_drawn() {
    // A square with 2 holes, the right one drawn first.
    let square = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    let right = vec![p(6.0, 4.0), p(8.0, 4.0), p(8.0, 6.0), p(6.0, 6.0)];
    let left = vec![p(2.0, 4.0), p(4.0, 4.0), p(4.0, 6.0), p(2.0, 6.0)];
    let built = region(&[square, right, left], FillRule::EvenOdd);
    let firsts: Vec<Point> = built.region.parts[0].holes.iter().map(|hole| hole[0]).collect();
    assert_eq!(firsts, [p(6.0, 4.0), p(2.0, 4.0)]);
}

#[test]
fn the_budget_bounds_the_work() {
    let rings = [vec![p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]];
    // Flattening, cutting, the faces and the rings each spend some of it.
    let needed =
        (1..200).find(|&work| build(&path(&rings), FillRule::NonZero, &mut Budget { max_stitches: 1, max_work: work }.meter()).is_ok()).unwrap();
    for work in [1, needed / 4, needed / 2, needed - 1] {
        assert!(build(&path(&rings), FillRule::NonZero, &mut Budget { max_stitches: 1, max_work: work }.meter()).is_err(), "{work}");
    }
    assert!(needed > 30, "{needed}");
}
