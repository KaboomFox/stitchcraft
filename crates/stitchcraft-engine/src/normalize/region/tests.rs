//! The region against the winding rule itself: every point tested lies in the region exactly when the
//! rings wind round it as the fill rule asks. The rings are random rings drawn on a coarse grid, where
//! they cross, touch and run along each other often, and random rings with float coordinates beside
//! near-copies of themselves, as rounding leaves shapes meant to share corners and edges: moved or turned
//! by a hair, drawn back, or through some of the same corners. The edges such rings are cut into meet only
//! at their ends, and every point is left as often as it is reached.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::exact::{self, Side};
use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Point, SplitMix64};

use super::arrange::{Arrangement, cut};
use super::geom::{Hit, Location, hit, locate_in_ring, on_segment};
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

/// Whether `q` lies on a segment of the rings, or within a nanometre of one: the region's points lie on a
/// grid finer than that, a hair from the rings'.
fn on_rings(rings: &[Vec<Point>], q: Point) -> bool {
    rings.iter().any(|ring| geom::closed(ring).windows(2).any(|w| on_segment(q, w[0], w[1]) || stroke::distance_to_segment(q, w[0], w[1]) < 1e-9))
}

/// What is wrong with the region `rings` bound under `rule`, if anything: a point of `samples` off the
/// rings that lies in the region when the rule leaves it empty or the other way round (a part left out for
/// its size excuses a point the rule fills), a ring that turns the wrong way, or a ring through a point
/// twice.
fn check(rings: &[Vec<Point>], rule: FillRule, samples: impl IntoIterator<Item = Point>) -> Result<(), String> {
    let built = region(rings, rule);
    if built.diagnostics.iter().any(|d| d.code == Code::InternalCheckFailed) {
        return Err(format!("{:?}", built.diagnostics));
    }
    let left_out = built.diagnostics.iter().any(|d| d.code == Code::FillPartsTooSmall && d.message.contains("left out"));
    for q in samples.into_iter().filter(|&q| !on_rings(rings, q)) {
        let w = winding_at(rings, q);
        let expected = match rule {
            FillRule::NonZero => w != 0,
            FillRule::EvenOdd => w.rem_euclid(2) == 1,
        };
        let inside = contains(&built.region, q);
        if inside != expected && !(expected && left_out) {
            return Err(format!("{q:?} winds {w}, and lies in the region: {inside}"));
        }
    }
    for part in &built.region.parts {
        if geom::signed_area(&part.outline) >= 0.0 || part.holes.iter().any(|hole| geom::signed_area(hole) <= 0.0) {
            return Err("an outline turns counter-clockwise, or a hole clockwise, in y-up axes".to_string());
        }
        for ring in std::iter::once(&part.outline).chain(&part.holes) {
            let open = &ring[..ring.len() - 1];
            let mut sorted: Vec<(u64, u64)> = open.iter().map(|q| (q.x().to_bits(), q.y().to_bits())).collect();
            sorted.sort_unstable();
            let distinct = sorted.len();
            sorted.dedup();
            if sorted.len() != distinct {
                return Err("a ring passes a point twice".to_string());
            }
        }
    }
    Ok(())
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
        let grid = (0..24).flat_map(|i| (0..24).map(move |j| p(f64::from(i) * 0.5 + 0.13, f64::from(j) * 0.5 + 0.29)));
        if let Err(why) = check(&rings, rule, grid) {
            prop_assert!(false, "{}", why);
        }
    }
}

/// A random ring round (`cx`, `cy`): a star of `n` corners at random angles and radii.
fn star(rng: &mut SplitMix64, (cx, cy): (f64, f64), n: usize, r: f64) -> Vec<Point> {
    let mut angles: Vec<f64> = (0..n).map(|_| rng.next_in(0.0, std::f64::consts::TAU)).collect();
    angles.sort_by(f64::total_cmp);
    angles
        .into_iter()
        .map(|a| {
            let (sin, cos) = sin_cos(a);
            p(cx + r * rng.next_in(0.3, 1.0) * cos, cy + r * rng.next_in(0.3, 1.0) * sin)
        })
        .collect()
}

/// A random walk of `n` corners in a square of side `s`, which often crosses itself.
fn walk(rng: &mut SplitMix64, n: usize, s: f64) -> Vec<Point> {
    (0..n).map(|_| p(rng.next_in(0.0, s), rng.next_in(0.0, s))).collect()
}

/// `ring` moved by (`dx`, `dy`).
fn moved(ring: &[Point], dx: f64, dy: f64) -> Vec<Point> {
    ring.iter().map(|q| p(q.x() + dx, q.y() + dy)).collect()
}

/// `ring` turned by `angle` round `c`.
fn turned(ring: &[Point], c: Point, angle: f64) -> Vec<Point> {
    let (s, k) = sin_cos(angle);
    ring.iter().map(|q| p(c.x() + (q.x() - c.x()) * k - (q.y() - c.y()) * s, c.y() + (q.x() - c.x()) * s + (q.y() - c.y()) * k)).collect()
}

/// Case `case` of the near-copies: a random ring with float coordinates, and beside it a copy moved by
/// 1e-13 to 1e-7 mm or not at all, a copy turned by a hair, a copy moved and drawn back, a ring through
/// some of its corners, another random ring, or 2 near-copies.
fn near_copies(case: usize, rng: &mut SplitMix64) -> Vec<Vec<Point>> {
    let base = if case.is_multiple_of(3) { walk(rng, 4 + case % 7, 40.0) } else { star(rng, (20.0, 20.0), 5 + case % 9, 18.0) };
    let tiny = [1e-13, 1e-11, 1e-9, 1e-7, 0.0][case % 5];
    let mut rings = vec![base.clone()];
    match (case / 5) % 6 {
        0 => rings.push(moved(&base, tiny, tiny * 0.7)),
        1 => {
            let centre = p(rng.next_in(0.0, 40.0), rng.next_in(0.0, 40.0));
            rings.push(turned(&base, centre, tiny + 1e-15));
        }
        2 => {
            let mut back = moved(&base, -tiny * 0.3, tiny);
            back.reverse();
            rings.push(back);
        }
        3 => {
            let other = star(rng, (25.0, 18.0), 6, 15.0);
            let mixed: Vec<Point> = other.iter().enumerate().map(|(k, &q)| if k % 2 == 0 { base[k % base.len()] } else { q }).collect();
            rings.push(moved(&mixed, tiny, 0.0));
        }
        4 => rings.push(walk(rng, 6, 40.0)),
        _ => {
            rings.push(moved(&base, tiny, -tiny));
            rings.push(turned(&base, p(20.0, 20.0), -tiny - 1e-15));
        }
    }
    rings
}

/// 20 × 20 points across the box round `rings`.
fn samples(rings: &[Vec<Point>]) -> Vec<Point> {
    let env = geom::Envelope::of(&rings.concat());
    let (w, h) = (env.max_x - env.min_x, env.max_y - env.min_y);
    (0..20)
        .flat_map(|i| (0..20).map(move |j| (f64::from(i), f64::from(j))))
        .map(|(i, j)| p(env.min_x + w * (i + 0.371) / 20.0, env.min_y + h * (j + 0.613) / 20.0))
        .collect()
}

#[test]
fn req_fill_001_points_lie_in_the_region_as_rings_and_their_near_copies_wind_round_them() {
    let mut rng = SplitMix64::new(0x5eed_0001);
    for case in 0..360 {
        let rings = near_copies(case, &mut rng);
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            if let Err(why) = check(&rings, rule, samples(&rings)) {
                panic!("case {case}, {rule:?}: {why}");
            }
        }
    }
}

/// What is wrong with `arrangement`, if anything: a point reached more often than it is left, or 2 edges
/// that meet anywhere but at the ends they share.
fn planar(arrangement: &Arrangement) -> Result<(), String> {
    let mut balance = vec![0_i64; arrangement.points.len()];
    for (&(from, to), &count) in arrangement.edges.iter().zip(&arrangement.count) {
        balance[from] -= count;
        balance[to] += count;
    }
    if let Some(point) = balance.iter().position(|&b| b != 0) {
        return Err(format!("{:?} is reached {} times more often than it is left", arrangement.points[point], balance[point]));
    }
    for (k, &(a, b)) in arrangement.edges.iter().enumerate() {
        for &(c, d) in &arrangement.edges[k + 1..] {
            let ends = [a, b].into_iter().filter(|e| *e == c || *e == d).count();
            let (pa, pb, pc, pd) = (arrangement.point(a), arrangement.point(b), arrangement.point(c), arrangement.point(d));
            let fine = match hit(pa, pb, pc, pd) {
                Hit::Apart => ends == 0,
                Hit::Touch(q) => ends == 1 && [pa, pb].contains(&q) && [pc, pd].contains(&q),
                Hit::Cross(_) | Hit::Overlap(..) => false,
            };
            if !fine {
                return Err(format!("the edges {pa:?}–{pb:?} and {pc:?}–{pd:?} meet away from their ends"));
            }
        }
    }
    Ok(())
}

#[test]
fn req_fill_001_rings_and_their_near_copies_are_cut_into_edges_that_meet_only_at_their_ends() {
    let mut rng = SplitMix64::new(0x5eed_0001);
    for case in 0..360 {
        let rings = near_copies(case, &mut rng);
        let arrangement = cut(&rings, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        if let Err(why) = planar(&arrangement) {
            panic!("case {case}: {why}");
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
    // Ink/Stitch keeps parts larger than 3 square pixels, and calls fills smaller than 20 small. Drawn, a
    // rectangle's corners go on the grid, which moves its area off the limit by a hair, so the parts are
    // given as found.
    let rectangle =
        |w: f64| Polygon { outline: geom::closed(&[p(0.0, 0.0), p(0.0, MM_PER_SVG_PX), p(w, MM_PER_SVG_PX), p(w, 0.0)]), holes: Vec::new() };
    let found = |parts: Vec<Polygon>| built(Area { parts: Ok(parts), windings: Vec::new(), beside_outside: Vec::new() }, FillRule::NonZero);
    let three = found(vec![rectangle(3.0 * MM_PER_SVG_PX)]);
    assert!(three.region.parts.is_empty());
    assert_eq!(three.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillPartsTooSmall]);
    let twenty = found(vec![rectangle(20.0 * MM_PER_SVG_PX)]);
    assert_eq!(twenty.region.area(), SMALL_FILL);
    assert!(twenty.diagnostics.is_empty());
}

#[test]
fn diag_sc_e0009_an_area_that_could_not_be_built_says_only_so() {
    // A bug: the region is left empty, and SC-E0009 says so, not that the fill bounds no area.
    let failed = built(Area { parts: Err(Unjoined), windings: vec![1, 2], beside_outside: vec![true, false] }, FillRule::NonZero);
    assert_eq!(failed.region, Region::default());
    assert_eq!(failed.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
    // A point beyond the grid's reach is such a bug, a design lying within 10 m of the origin.
    assert_eq!(region(&[vec![p(0.0, 0.0), p(1e5, 0.0), p(1e5, 1e5)]], FillRule::NonZero), failed);
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

#[test]
fn req_fill_001_a_corner_a_rounding_error_off_another_shape_s_corner_is_that_corner() {
    // A triangle whose corner sits on a 10 mm square's corner (10, 0) up to a picometre, as a transform or
    // relative path data leave it, drawn before the square and after it. Its side crosses the square's bottom
    // edge half a picometre short of the square's corner. Under both rules the fill is the triangle and the
    // square, touching at the corner: 125 mm² in 2 parts.
    let corner = p(10.0 + 5e-13, 1e-12);
    let triangle = vec![corner, p(corner.x() - 5.0, corner.y() - 5.0), p(15.0, -5.0)];
    let square = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    for rings in [[triangle.clone(), square.clone()], [square, triangle]] {
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            let built = region(&rings, rule);
            assert_eq!((built.region.parts.len(), built.region.area()), (2, 125.0), "{rule:?}");
        }
    }
}

#[test]
fn req_fill_001_shapes_a_rounding_error_apart_or_over_each_other_are_one_part() {
    // 2 squares of 10 mm side by side, drawn the same way round, 1e-13 or 1e-12 mm apart, or over each other
    // by 1e-12 mm, as rounding leaves shapes meant to share an edge. On the grid they share it: under both
    // rules, 1 part of 200 mm², and no sliver for SC-I0306 to speak of.
    let left = vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)];
    for gap in [1e-13, 1e-12, -1e-12] {
        let x = 10.0 + gap;
        let right = vec![p(x, 0.0), p(x + 10.0, 0.0), p(x + 10.0, 10.0), p(x, 10.0)];
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            let built = region(&[left.clone(), right.clone()], rule);
            assert_eq!((built.region.parts.len(), built.region.area()), (1, 200.0), "{gap:e} {rule:?}");
            assert_eq!(built.diagnostics, Vec::new(), "{gap:e} {rule:?}");
        }
    }
}

#[test]
fn req_fill_001_a_grid_of_squares_drawn_with_relative_moves_is_one_part() {
    // Grids of 2 to 6 by 2 to 6 squares side by side, at random angles, as one path whose corners are each
    // reached by adding a side to where the pen is, as relative path data reaches them: the corners the
    // squares share come out a few units in the last place apart. Under both rules each grid is 1 part.
    let mut rng = SplitMix64::new(5);
    for case in 0..100_u32 {
        let (n, m) = (2 + case % 5, 2 + (case / 5) % 5);
        let side = rng.next_in(0.5, 5.0);
        let (sin, cos) = sin_cos(rng.next_in(0.0, std::f64::consts::TAU));
        let (u, v) = ((side * cos, side * sin), (-side * sin, side * cos));
        let mut rings = Vec::new();
        let mut row = (rng.next_in(0.0, 50.0), rng.next_in(0.0, 50.0));
        for _ in 0..m {
            let mut pen = row;
            for _ in 0..n {
                let mut corner = pen;
                let mut ring = vec![p(corner.0, corner.1)];
                for (dx, dy) in [u, v, (-u.0, -u.1)] {
                    corner = (corner.0 + dx, corner.1 + dy);
                    ring.push(p(corner.0, corner.1));
                }
                rings.push(ring);
                pen = (pen.0 + u.0, pen.1 + u.1);
            }
            row = (row.0 + v.0, row.1 + v.1);
        }
        let area = side * side * f64::from(n * m);
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            let built = region(&rings, rule);
            assert_eq!(built.region.parts.len(), 1, "case {case} {rule:?}: {:?}", built.diagnostics);
            assert!((built.region.area() - area).abs() < 1e-9 * area, "case {case} {rule:?}: {} mm²", built.region.area());
        }
    }
}

#[test]
fn req_fill_001_shapes_sharing_a_curved_edge_fill_both() {
    // 2 shapes in one path that share a curved edge, as shapes drawn side by side and combined into one
    // path share it: the first runs (0, 0) → (20, 0) and back along a cubic to (0, 20); the second round the
    // rest of a 30 mm square and back along the same cubic the other way. Flattened from either end, the
    // curve's points can differ in their last bits. Under both rules the fill is the square: 900 mm² in 1 part.
    let mut rng = SplitMix64::new(77);
    for case in 0..100 {
        let c1 = p(20.0 + rng.next_in(-5.0, 5.0), rng.next_in(2.0, 15.0));
        let c2 = p(rng.next_in(2.0, 15.0), 20.0 + rng.next_in(-5.0, 5.0));
        let (start, end) = (p(20.0, 0.0), p(0.0, 20.0));
        let first = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(start), Segment::Cubic(c1, c2, end)], closed: true };
        let corners = [p(30.0, 0.0), p(30.0, 30.0), p(0.0, 30.0), end];
        let mut segments: Vec<Segment> = corners.iter().map(|&q| Segment::Line(q)).collect();
        segments.push(Segment::Cubic(c2, c1, start));
        let second = Subpath { start, segments, closed: true };
        let path = Path { subpaths: vec![first, second] };
        for rule in [FillRule::NonZero, FillRule::EvenOdd] {
            let built = build(&path, rule, &mut Budget::DEFAULT.meter()).unwrap();
            assert_eq!(built.region.parts.len(), 1, "case {case} {rule:?}");
            assert!((built.region.area() - 900.0).abs() < 1e-9, "case {case} {rule:?}: {} mm²", built.region.area());
        }
    }
}
