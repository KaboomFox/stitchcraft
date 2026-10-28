//! The constrained Delaunay triangulation: every edge of the part is an edge of the triangles, the triangles
//! cover the part and nothing else, and every edge between 2 of them passes the Delaunay test.

use stitchcraft_core::exact::{self, Circle, Side};
use stitchcraft_core::math::{hypot, sin_cos};
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Exhausted, Meter};

use super::mesh::{Failed, Mesh};
use super::*;
use crate::normalize::region::geom;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn meter() -> Meter {
    Budget::DEFAULT.meter()
}

/// A part with the outline `outline` and the holes `holes`, each closed and wound as a region winds them:
/// the outline clockwise and the holes counter-clockwise in y-up axes.
fn part(outline: &[(f64, f64)], holes: &[&[(f64, f64)]]) -> Polygon {
    let ring = |points: &[(f64, f64)], clockwise: bool| {
        let mut ring: Vec<Point> = points.iter().map(|&(x, y)| p(x, y)).collect();
        if (geom::signed_area(&geom::closed(&ring)) < 0.0) != clockwise {
            ring.reverse();
        }
        geom::closed(&ring)
    };
    Polygon { outline: ring(outline, true), holes: holes.iter().map(|hole| ring(hole, false)).collect() }
}

/// The length of the segment from `a` to `b`.
fn length(a: Point, b: Point) -> f64 {
    hypot(b.x() - a.x(), b.y() - a.y())
}

/// The triangulation of `part` with no edge longer than `longest`, checked: every triangle turns
/// counter-clockwise and lies in the part, together they cover its area, each edge between 2 of them is
/// the same edge from both sides and passes the Delaunay test, and the edges across which there is no
/// triangle are the part's edges, none longer than `longest`, as long as its rings together.
fn checked(part: &Polygon, longest: f64) -> Triangulation {
    let triangulation = triangulate(part, longest, &mut meter()).unwrap().unwrap();
    let at = |i: usize| triangulation.points[i];
    let mut area = 0.0;
    let mut outline = 0.0;
    for (i, triangle) in triangulation.triangles.iter().enumerate() {
        let [a, b, c] = triangle.corners.map(at);
        assert_eq!(exact::side(a, b, c), Side::Left, "{triangle:?}");
        area += ((b.x() - a.x()) * (c.y() - a.y()) - (b.y() - a.y()) * (c.x() - a.x())) / 2.0;
        assert!(part.covers(p((a.x() + b.x() + c.x()) / 3.0, (a.y() + b.y() + c.y()) / 3.0)), "{triangle:?}");
        for k in 0..3 {
            let (from, to) = triangle.edge(k);
            match triangle.across[k] {
                Some(j) => {
                    let other = triangulation.triangles[j];
                    let back = (0..3).find(|&m| other.across[m] == Some(i)).unwrap();
                    assert_eq!(other.edge(back), (to, from));
                    assert_ne!(exact::incircle(a, b, c, at(other.corners[back])), Circle::Inside, "{triangle:?} {other:?}");
                }
                None => {
                    assert!(length(at(from), at(to)) <= longest * (1.0 + 1e-9) + 1e-9);
                    outline += length(at(from), at(to));
                }
            }
        }
    }
    assert!((area - part.area()).abs() <= 1e-9 * part.area().max(1.0), "{area} {}", part.area());
    let rings: f64 = part.rings().map(|ring| ring.windows(2).map(|w| length(w[0], w[1])).sum::<f64>()).sum();
    assert!((outline - rings).abs() <= 1e-6, "{outline} {rings}");
    triangulation
}

#[test]
fn a_triangle_is_1_triangle_and_a_square_2() {
    let triangle = part(&[(0.0, 0.0), (4.0, 0.0), (0.0, 3.0)], &[]);
    assert_eq!(checked(&triangle, 100.0).triangles.len(), 1);
    let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[]);
    let triangulation = checked(&square, 100.0);
    assert_eq!(triangulation.points.len(), 4);
    assert_eq!(triangulation.triangles.len(), 2);
}

#[test]
fn points_are_added_along_edges_longer_than_the_longest() {
    // Each side of 10 mm in 10 pieces of 1 mm: 40 points round the square, and 38 triangles between them.
    let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[]);
    let triangulation = checked(&square, 1.0);
    assert_eq!(triangulation.points.len(), 40);
    assert_eq!(triangulation.triangles.len(), 38);
    // A length that isn't a whole number of pieces gives pieces as long as each other: 10 mm in 4 of 2.5 mm.
    assert_eq!(checked(&square, 3.0).points.len(), 16);
}

#[test]
fn a_part_with_holes_is_cut_round_them() {
    let ring = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], &[&[(5.0, 5.0), (15.0, 5.0), (15.0, 15.0), (5.0, 15.0)]]);
    checked(&ring, 2.0);
    // 2 holes, one touching the outline at a corner both have, and one touching that hole at a corner.
    let outline = [(0.0, 0.0), (10.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];
    let touching = part(&outline, &[&[(10.0, 0.0), (13.0, 5.0), (7.0, 5.0)], &[(13.0, 5.0), (16.0, 10.0), (12.0, 10.0)]]);
    checked(&touching, 100.0);
    checked(&touching, 1.5);
}

#[test]
fn a_thin_strip_is_cut_across() {
    // A strip 0.5 mm wide and 50 mm long, cut in pieces no longer than it is wide: every edge between 2
    // triangles crosses it, no longer than the diagonal of a square of its width.
    let strip = part(&[(0.0, 0.0), (50.0, 0.0), (50.0, 0.5), (0.0, 0.5)], &[]);
    let triangulation = checked(&strip, 0.5);
    for triangle in &triangulation.triangles {
        for k in 0..3 {
            if triangle.across[k].is_some() {
                let (a, b) = triangle.edge(k);
                let (a, b) = (triangulation.points[a], triangulation.points[b]);
                assert!(length(a, b) <= 0.5 * 2.0_f64.sqrt() + 1e-9, "{a:?} {b:?}");
                assert!((a.y() - b.y()).abs() > 0.4, "{a:?} {b:?}");
            }
        }
    }
}

#[test]
fn random_stars_with_holes() {
    // Star-shaped outlines of many points, with a star-shaped hole round the same centre, each point at a
    // random distance: many corners turn inwards, and many edges of the part are no edges of the Delaunay
    // triangulation of its points, so they are made.
    let mut rng = SplitMix64::new(20_261_027);
    for case in 0..60 {
        let count = 5 + rng.next_u64() % 40;
        let star = |rng: &mut SplitMix64, low: f64, high: f64| -> Vec<(f64, f64)> {
            (0..count)
                .map(|i| {
                    let (sin, cos) = sin_cos(std::f64::consts::TAU * i as f64 / count as f64);
                    let r = rng.next_in(low, high);
                    (30.0 + r * cos, 40.0 + r * sin)
                })
                .collect()
        };
        let outline = star(&mut rng, 6.0, 20.0);
        let hole = star(&mut rng, 1.0, 5.0);
        let holes: Vec<&[(f64, f64)]> = if case % 2 == 0 { vec![&hole] } else { Vec::new() };
        let longest = [0.7, 2.0, 100.0][case % 3];
        checked(&part(&outline, &holes), longest);
    }
}

#[test]
fn a_part_of_fewer_than_3_points_has_no_triangles() {
    let dot = Polygon { outline: vec![p(1.0, 1.0), p(1.0, 1.0)], holes: Vec::new() };
    assert_eq!(triangulate(&dot, 1.0, &mut meter()), Ok(Ok(Triangulation::default())));
    // Nor has a line there and back, each way a piece no longer than the longest: its ends are its points.
    let line = Polygon { outline: vec![p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.0)], holes: Vec::new() };
    assert_eq!(triangulate(&line, 1.0, &mut meter()), Ok(Ok(Triangulation::default())));
}

#[test]
fn the_box_round_a_part_holds_it_wherever_it_lies() {
    // A part below and left of the origin, and a part all at one point on whole millimetres, where rounding
    // the box's corners outwards adds nothing: the box has room round each on every side.
    let square = part(&[(-10.0, -10.0), (-5.0, -10.0), (-5.0, -5.0), (-10.0, -5.0)], &[]);
    assert_eq!(checked(&square, 100.0).triangles.len(), 2);
    let dot = Polygon { outline: vec![p(1.0, 1.0); 4], holes: Vec::new() };
    let one_point = Triangulation { points: vec![p(1.0, 1.0)], triangles: Vec::new() };
    assert_eq!(triangulate(&dot, 1.0, &mut meter()), Ok(Ok(one_point)));
}

/// The triangles of `triangulation`, each as its corners' coordinates from its least corner on, in order: the
/// same for the same triangles, however they are numbered.
fn cut(points: &[Point], triangles: impl Iterator<Item = [usize; 3]>) -> Vec<[(u64, u64); 3]> {
    let mut cut: Vec<[(u64, u64); 3]> = triangles
        .map(|corners| {
            let at = corners.map(|i| (points[i].x().to_bits(), points[i].y().to_bits()));
            let least = (0..3).min_by_key(|&k| at[k]).unwrap();
            [at[least], at[(least + 1) % 3], at[(least + 2) % 3]]
        })
        .collect();
    cut.sort_unstable();
    cut
}

#[test]
fn the_triangles_are_the_same_whatever_point_the_outline_starts_at() {
    // Parts with 4 or more points on one circle that holds none: a square, a strip cut into pieces as long as
    // it is wide, regular polygons, a ring and a strip with round ends. The Delaunay test can't choose between
    // the ways such points can be cut into triangles, and the tie-break chooses one, whatever order the points
    // go in.
    let polygon = |x: f64, r: f64, n: usize, turn: f64| -> Vec<(f64, f64)> {
        (0..n)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::TAU * (i as f64 + turn) / n as f64);
                (x + r * cos, r * sin)
            })
            .collect()
    };
    let mut round = polygon(0.0, 0.4, 24, 0.0)[6..=18].to_vec();
    round.extend(polygon(9.2, 0.4, 24, 0.0)[18..].iter().chain(&polygon(9.2, 0.4, 24, 0.0)[..=6]));
    type Ring = Vec<(f64, f64)>;
    let shapes: [(Ring, Vec<Ring>, f64); 6] = [
        (vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], Vec::new(), 100.0),
        (vec![(0.0, 0.0), (6.0, 0.0), (6.0, 1.0), (0.0, 1.0)], Vec::new(), 1.0),
        (polygon(0.0, 5.0, 8, 0.0), Vec::new(), 100.0),
        (polygon(0.0, 5.0, 12, 0.0), Vec::new(), 2.0),
        (polygon(0.0, 5.0, 16, 0.0), vec![polygon(0.0, 4.0, 16, 0.0)], 100.0),
        (round, Vec::new(), 0.4),
    ];
    for (outline, holes, longest) in shapes {
        let triangles = |outline: &[(f64, f64)], holes: &[Vec<(f64, f64)>]| {
            let holes: Vec<&[(f64, f64)]> = holes.iter().map(Vec::as_slice).collect();
            let found = checked(&part(outline, &holes), longest);
            cut(&found.points, found.triangles.iter().map(|triangle| triangle.corners))
        };
        let first = triangles(&outline, &holes);
        for start in 1..outline.len() {
            let turned = |ring: &[(f64, f64)]| [&ring[start % ring.len()..], &ring[..start % ring.len()]].concat();
            let holes: Vec<Vec<(f64, f64)>> = holes.iter().map(|hole| turned(hole)).collect();
            assert_eq!(triangles(&turned(&outline), &holes), first, "{outline:?} from {start}");
        }
    }
}

#[test]
fn points_on_one_circle_are_cut_the_same_whatever_order_they_go_in() {
    // 5 points with whole coordinates on the circle of radius 5 round the origin, in each of their 120 orders:
    // one cut of the mesh, the box's triangles with it.
    let corners = [(5.0, 0.0), (3.0, 4.0), (-4.0, 3.0), (-3.0, -4.0), (4.0, -3.0)];
    let mut first = None;
    let mut order: Vec<usize> = (0..5).collect();
    // Heap's algorithm, every order once.
    let mut counts = [0; 5];
    let mut i = 0;
    loop {
        let (mesh, _) = mesh_of(&order.iter().map(|&k| corners[k]).collect::<Vec<_>>());
        let found = cut(&mesh.points, mesh.tris.iter().map(|tri| tri.v));
        assert_eq!(first.get_or_insert_with(|| found.clone()), &found, "{order:?}");
        while i < 5 && counts[i] >= i {
            counts[i] = 0;
            i += 1;
        }
        if i == 5 {
            break;
        }
        order.swap(if i % 2 == 0 { 0 } else { counts[i] }, i);
        counts[i] += 1;
        i = 1;
    }
    // The tie-break raises the least point's lift the most, as if it lay a little further out: a square is cut
    // along the diagonal that doesn't meet its least corner by x and then by y.
    let (square, ids) = mesh_of(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
    assert!(square.edge(ids[1], ids[3], &mut meter()).unwrap().is_some());
    assert!(square.edge(ids[0], ids[2], &mut meter()).unwrap().is_none());
}

#[test]
fn the_work_is_charged() {
    let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[]);
    let mut small = Budget { max_stitches: 10, max_work: 50 }.meter();
    assert_eq!(triangulate(&square, 1.0, &mut small), Err(Exhausted::Work));
}

/// A mesh of the points `points`, each inserted, and their indices.
fn mesh_of(points: &[(f64, f64)]) -> (Mesh, Vec<usize>) {
    let mut mesh = Mesh::boxed(p(-10.0, -10.0), p(10.0, 10.0)).unwrap();
    let indices = points.iter().map(|&(x, y)| mesh.insert(p(x, y), &mut meter()).unwrap()).collect();
    (mesh, indices)
}

#[test]
fn an_edge_the_delaunay_triangulation_lacks_is_made_by_flipping() {
    // The circle through a, b and c holds no other point, so the Delaunay triangulation has the short
    // diagonal from a to c, which the edge from b to d crosses.
    let (mut mesh, ids) = mesh_of(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0), (1.0, -5.0)]);
    let [a, b, c, d] = [ids[0], ids[1], ids[2], ids[3]];
    assert!(mesh.edge(a, c, &mut meter()).unwrap().is_some());
    assert!(mesh.edge(b, d, &mut meter()).unwrap().is_none());
    mesh.constrain(b, d, &mut meter()).unwrap();
    assert!(mesh.edge(a, c, &mut meter()).unwrap().is_none());
    let (t, k) = mesh.edge(b, d, &mut meter()).unwrap().unwrap();
    assert!(mesh.tri(t).unwrap().is_fixed(k));
    // A fixed edge is never flipped, though the mesh around it is no longer Delaunay.
    let inside = vec![true; mesh.tris.len()];
    mesh.settle(&inside, &mut meter()).unwrap();
    assert!(mesh.edge(b, d, &mut meter()).unwrap().is_some());
}

#[test]
fn a_point_on_an_edge_of_the_part_cuts_it() {
    // e lies on the segment from b to d, which is made as 2 edges through it.
    let (mut mesh, ids) = mesh_of(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0), (1.0, -5.0), (1.0, -1.0)]);
    let [b, d, e] = [ids[1], ids[3], ids[4]];
    mesh.constrain(b, d, &mut meter()).unwrap();
    assert!(mesh.edge(b, d, &mut meter()).unwrap().is_none());
    for (from, to) in [(b, e), (e, d)] {
        let (t, k) = mesh.edge(from, to, &mut meter()).unwrap().unwrap();
        assert!(mesh.tri(t).unwrap().is_fixed(k), "{from} {to}");
    }
}

#[test]
fn a_point_already_there_is_that_point() {
    let (mut mesh, ids) = mesh_of(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0)]);
    assert_eq!(mesh.insert(p(1.0, 5.0), &mut meter()), Ok(ids[1]));
    // A point on an edge splits the triangles beside it, and the mesh stays whole.
    let (mut mesh, _) = mesh_of(&[(0.0, 0.0), (2.0, 0.0), (1.0, 1.0), (1.0, -1.0)]);
    let on = mesh.insert(p(1.0, 0.0), &mut meter()).unwrap();
    assert!(mesh.edge(on, 4, &mut meter()).unwrap().is_some());
}

#[test]
fn an_edge_from_a_corner_of_the_box_is_found() {
    // A point a micrometre above the edge from (0, 0) to (1, 0): in the Delaunay triangulation an edge from
    // a corner of the box runs to it across that edge, whose making flips it. The triangles round a corner of
    // the box don't close round it, and each of its edges is found from either end, so the edge is made
    // either way round.
    let mut mesh = Mesh::boxed(p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    let ids: Vec<usize> =
        [(0.0, 0.0), (1.0, 0.0), (0.5, 0.001), (0.5, 1.0)].iter().map(|&(x, y)| mesh.insert(p(x, y), &mut meter()).unwrap()).collect();
    for tri in &mesh.tris {
        for k in 0..3 {
            let (a, b) = (tri.corner(k + 1), tri.corner(k + 2));
            assert!(mesh.edge(a, b, &mut meter()).unwrap().is_some(), "{a} {b}");
            assert!(mesh.edge(b, a, &mut meter()).unwrap().is_some(), "{b} {a}");
        }
    }
    // 2 points that no triangle has both of share no edge, a corner of the box among them: the point at
    // (0, 0) cut the box's diagonal from (-2, -2) to (3, 3).
    for a in 0..mesh.points.len() {
        for b in (0..mesh.points.len()).filter(|&b| b != a) {
            if !mesh.tris.iter().any(|tri| tri.v.contains(&a) && tri.v.contains(&b)) {
                assert_eq!(mesh.edge(a, b, &mut meter()), Ok(None), "{a} {b}");
            }
        }
    }
    for (from, to) in [(ids[0], ids[1]), (ids[1], ids[0])] {
        let mut made = mesh.clone();
        made.constrain(from, to, &mut meter()).unwrap();
        assert!(made.edge(from, to, &mut meter()).unwrap().is_some(), "{from} {to}");
    }
}

#[test]
fn a_point_on_the_segment_beside_its_start_cuts_it() {
    // b lies on the segment from a to c, and is a corner of the triangles round a, as are the points on its
    // line behind a and beyond c: the walk round a meets b first as the one corner of a triangle or as the
    // other, by where it starts, and the segment is made as 2 edges through b, and no edge behind its start,
    // either way round, mirrored, and along either axis.
    for (sx, sy, upright) in [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)].into_iter().flat_map(|(x, y)| [(x, y, false), (x, y, true)]) {
        let along = |(x, y): (f64, f64)| if upright { (y * sy, x * sx) } else { (x * sx, y * sy) };
        let points = [(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (1.0, 1.0), (1.0, -1.0), (-1.0, 0.0), (3.0, 0.0)].map(along);
        let (mesh, ids) = mesh_of(&points);
        for (from, to, behind) in [(ids[0], ids[2], ids[5]), (ids[2], ids[0], ids[6])] {
            let mut made = mesh.clone();
            made.constrain(from, to, &mut meter()).unwrap();
            for (a, b) in [(from, ids[1]), (ids[1], to)] {
                let (t, k) = made.edge(a, b, &mut meter()).unwrap().unwrap();
                assert!(made.tri(t).unwrap().is_fixed(k), "{points:?} {a} {b}");
            }
            let (t, k) = made.edge(behind, from, &mut meter()).unwrap().unwrap();
            assert!(!made.tri(t).unwrap().is_fixed(k), "{points:?} {behind} {from}");
        }
    }
}

#[test]
fn a_point_added_on_an_edge_keeps_the_edges_of_the_part_round_it() {
    // Some of the sides of a quadrilateral made edges of the part, each set of them in turn, and a point added
    // at the middle of its diagonal: the 2 triangles beside the diagonal are cut into 4, and each side made
    // stays an edge of the part, from both of its triangles, while the other sides and the halves of the
    // diagonal don't. The corners go in in each of their 8 orders round it, so that each side comes in each
    // place of its triangle.
    let corners = [(0.0, 0.0), (4.0, -1.0), (5.0, 3.0), (1.0, 4.0)];
    for made in 1..16_u32 {
        for (turn, reversed) in (0..4).flat_map(|turn| [(turn, false), (turn, true)]) {
            let mut order: Vec<(f64, f64)> = (0..4).map(|k| corners[(k + turn) % 4]).collect();
            if reversed {
                order.reverse();
            }
            let (mut mesh, ids) = mesh_of(&order);
            let at = |point: (f64, f64)| ids[order.iter().position(|&q| q == point).unwrap()];
            let sides: Vec<(usize, usize)> = (0..4).filter(|k| made & (1 << k) != 0).map(|k| (at(corners[k]), at(corners[(k + 1) % 4]))).collect();
            for &(a, b) in &sides {
                mesh.constrain(a, b, &mut meter()).unwrap();
            }
            let diagonal = if mesh.edge(at(corners[0]), at(corners[2]), &mut meter()).unwrap().is_some() { (0, 2) } else { (1, 3) };
            let (a, b) = (corners[diagonal.0], corners[diagonal.1]);
            let middle = mesh.insert(p((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0), &mut meter()).unwrap();
            for (i, tri) in mesh.tris.iter().enumerate() {
                for k in 0..3 {
                    let (from, to) = (tri.corner(k + 1), tri.corner(k + 2));
                    let side = sides.iter().any(|&(a, b)| (a, b) == (from, to) || (b, a) == (from, to));
                    assert_eq!(tri.is_fixed(k), side, "sides {made:#06b}, {order:?}: triangle {i}, {from} to {to}, middle {middle}");
                }
            }
        }
    }
}

#[test]
fn an_edge_that_crosses_many_is_made_by_flipping_them_in_turn() {
    // Points either side of the segment from (0, 0) to (12, 0), in turn above and below it: the triangles
    // between them make a strip that the segment crosses.
    let mut strip = vec![(0.0, 0.0), (12.0, 0.0)];
    strip.extend((1..12).map(|i| (f64::from(i), if i % 2 == 0 { 0.6 } else { -0.6 })));
    // Points scattered either side of the segment from (0, 0) to (10, 0). A flip makes the edge from
    // (1, 0.75) to (6.5, -1.75), which crosses the segment too and is flipped again in its turn, and one
    // makes the edge from (1, 0.75) to (6.5, 0.25), beside the segment though its line crosses it, which is
    // left alone: once the segment is made, that edge's quadrilateral is not convex, and the flips would wait
    // for it to be until their bound stopped them.
    let scattered = [(0.0, 0.0), (10.0, 0.0), (6.5, 0.25), (1.0, 0.75), (2.75, 2.25), (6.5, -1.75), (0.75, -0.5)];
    for points in [&strip[..], &scattered] {
        let (mut mesh, ids) = mesh_of(points);
        mesh.constrain(ids[0], ids[1], &mut meter()).unwrap();
        let (t, k) = mesh.edge(ids[0], ids[1], &mut meter()).unwrap().unwrap();
        assert!(mesh.tri(t).unwrap().is_fixed(k));
        // Every other edge either side of it is the same edge from both sides.
        for (i, tri) in mesh.tris.iter().enumerate() {
            for k in 0..3 {
                if let Some(u) = tri.across(k) {
                    assert!(mesh.tri(u).unwrap().n.contains(&Some(i)), "{i} {u}");
                }
            }
        }
    }
}

#[test]
fn an_edge_from_a_point_to_itself_is_no_edge() {
    let (mut mesh, ids) = mesh_of(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0)]);
    let before = mesh.clone().tris;
    mesh.constrain(ids[1], ids[1], &mut meter()).unwrap();
    assert_eq!(mesh.tris, before);
    // Nor is one found. Round a corner of the box that both its triangles have, the walk turns both ways, and
    // comes back through where it started: 3 steps, one more than there are triangles.
    let boxed = Mesh::boxed(p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    assert_eq!(boxed.edge(0, 0, &mut meter()), Ok(None));
}

#[test]
fn a_walk_that_goes_round_in_a_circle_is_a_bug() {
    // The edge on the right of the box made to lead into the other triangle, which leads back across the
    // diagonal: the search for a point to the right of the box goes round them until it has taken more steps
    // than there are triangles.
    let mut mesh = Mesh::boxed(p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    mesh.tris[0].n[0] = Some(1);
    assert_eq!(mesh.insert(p(4.0, 0.5), &mut meter()), Err(Failed::Broken));
}

#[test]
fn a_point_on_every_edge_of_a_triangle_is_a_bug() {
    // A triangle made to have 2 corners at the same point: a point on the line of its other edges lies on all
    // 3 of them.
    let mut mesh = Mesh::boxed(p(0.0, 0.0), p(1.0, 1.0)).unwrap();
    mesh.tris[0].v = [0, 1, 1];
    assert_eq!(mesh.insert(p(0.5, -2.0), &mut meter()), Err(Failed::Broken));
}

#[test]
fn edges_of_the_part_that_cross_are_a_bug() {
    // 2 edges of a part never cross; a mesh asked to hold both says it lost its shape.
    let (mut mesh, ids) = mesh_of(&[(0.0, 0.0), (1.0, 5.0), (2.0, 0.0), (1.0, -5.0)]);
    mesh.constrain(ids[0], ids[2], &mut meter()).unwrap();
    assert_eq!(mesh.constrain(ids[1], ids[3], &mut meter()), Err(Failed::Broken));
}

#[test]
fn a_part_whose_edges_cross_or_dont_close_is_a_bug() {
    // A ring whose hole pokes out through an edge of its outline, and a square with a spike of no width, an
    // edge there and back, out of it or into it: no region has either. The first's edges cross, and the
    // second's spike is inside on one side and outside on the other, and the triangulation says it lost its
    // shape.
    let circle = |x: f64, r: f64, n: usize, turn: f64| -> Vec<(f64, f64)> {
        (0..n)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::TAU * (i as f64 + turn) / n as f64);
                (x + r * cos, r * sin)
            })
            .collect()
    };
    let poked = part(&circle(0.0, 1.0, 20, 0.5), &[&circle(0.4, 0.6, 40, 0.0)]);
    assert_eq!(triangulate(&poked, 0.4, &mut meter()), Ok(Err(Broken)));
    for tip in [15.0, 5.0] {
        let spiked = Polygon {
            outline: [(0.0, 0.0), (0.0, 10.0), (5.0, 10.0), (5.0, tip), (5.0, 10.0), (10.0, 10.0), (10.0, 0.0), (0.0, 0.0)]
                .map(|(x, y)| p(x, y))
                .to_vec(),
            holes: Vec::new(),
        };
        assert_eq!(triangulate(&spiked, 100.0, &mut meter()), Ok(Err(Broken)), "{tip}");
    }
}
