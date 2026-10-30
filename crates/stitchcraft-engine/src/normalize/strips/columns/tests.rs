//! Columns along centre lines (`REQ-AUTO-016`, `REQ-AUTO-017`): a band is one column between its sides, a
//! ring a closed one, an arm of a T or the column after a corner ends under the one beside it, and the rungs
//! lie square to the column.

use stitchcraft_core::Budget;
use stitchcraft_core::math::{self, sin_cos};

use super::*;
use crate::normalize::region::geom;
use crate::normalize::stroke::segments;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A part with the outline `outline` and the holes `holes`, wound as a region winds them.
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

/// `n` steps of a circle of radius `r` about (`x`, `y`), from `from` radians round `turn` radians, both ends
/// included.
fn arc(x: f64, y: f64, r: f64, n: usize, from: f64, turn: f64) -> Vec<(f64, f64)> {
    (0..=n)
        .map(|i| {
            let (sin, cos) = sin_cos(from + turn * i as f64 / n as f64);
            (x + r * cos, y + r * sin)
        })
        .collect()
}

/// The columns of `part`, its edges cut into pieces as long as it is wide on average, as automatic digitizing
/// cuts them.
fn found(part: &Polygon) -> Vec<Column> {
    let perimeter: f64 = part.rings().flat_map(segments).map(|(a, b)| a.distance(b)).sum();
    columns(part, 2.0 * part.area() / perimeter, &mut Budget::DEFAULT.meter()).unwrap().unwrap().unwrap()
}

/// Whether `a` and `b` are the same point, to within the region's grid.
fn same(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-9
}

/// The direction of the rung `[a, b]`, in degrees from the x axis, between 0 and 180.
fn heading([a, b]: [Point; 2]) -> f64 {
    math::to_degrees(math::atan2(b.y() - a.y(), b.x() - a.x())).rem_euclid(180.0)
}

#[test]
fn req_auto_016_a_band_is_one_column_between_its_sides() {
    // A band 3 mm wide and 20 mm long: its rails are its long sides, both the same way, its square ends are
    // its column's ends, and its rungs cross it square.
    let band = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (0.0, 3.0)], &[]);
    let [column] = found(&band).try_into().unwrap();
    assert!(!column.closed && column.under == [None, None]);
    let [one, other] = &column.rails;
    let ends = |rail: &[Point]| (*rail.first().unwrap(), *rail.last().unwrap());
    let (start, end) = (ends(one), ends(other));
    // Each rail runs the length of a side, from the same end.
    assert!(one.iter().all(|q| q.y() == one[0].y()) && other.iter().all(|q| q.y() == other[0].y()), "{column:?}");
    assert_eq!((one[0].y() - other[0].y()).abs(), 3.0);
    assert!((start.0.x() - end.0.x()).abs() < 1e-9 && (start.1.x() - end.1.x()).abs() < 1e-9, "{column:?}");
    assert_eq!((start.0.x() - start.1.x()).abs(), 20.0);
    assert!(!column.rungs.is_empty() && column.rungs.iter().all(|&rung| (heading(rung) - 90.0).abs() < 1e-6), "{:?}", column.rungs);
    // The line runs along the middle.
    assert!(column.line.points.iter().all(|q| (q.y() - 1.5).abs() < 1e-9), "{:?}", column.line);
}

#[test]
fn req_auto_016_a_round_end_ends_where_it_reaches_farthest() {
    // A band 3 mm wide whose ends are half circles of 16 steps: each end of the column lies at the end's
    // farthest point, or along the step of the circle there, which is within 20° of square to the band.
    let mut outline = arc(20.0, 1.5, 1.5, 16, -std::f64::consts::FRAC_PI_2, std::f64::consts::PI);
    outline.extend(arc(0.0, 1.5, 1.5, 16, std::f64::consts::FRAC_PI_2, std::f64::consts::PI));
    let [column] = found(&part(&outline, &[])).try_into().unwrap();
    let step = 1.5 * 2.0 * math::sin(std::f64::consts::PI / 32.0);
    for rail in &column.rails {
        for end in [rail.first().unwrap(), rail.last().unwrap()] {
            let farthest = if end.x() < 10.0 { p(-1.5, 1.5) } else { p(21.5, 1.5) };
            assert!(end.distance(farthest) <= step + 1e-9, "{end:?} from {farthest:?}");
        }
    }
}

#[test]
fn req_auto_016_a_square_end_ends_along_its_edge() {
    // A band whose end is an edge 10° from square to it ends along that edge, its rails at its 2 ends; one
    // 30° from square ends at its farthest corner, where its rails meet.
    for (slant, square) in [(10.0, true), (30.0, false)] {
        let rise = 3.0 * math::tan(math::to_radians(slant));
        let band = part(&[(0.0, 0.0), (20.0, 0.0), (20.0 + rise, 3.0), (0.0, 3.0)], &[]);
        let [column] = found(&band).try_into().unwrap();
        let far: Vec<Point> = column
            .rails
            .iter()
            .map(|rail| *[rail.first().unwrap(), rail.last().unwrap()].into_iter().max_by(|a, b| a.x().total_cmp(&b.x())).unwrap())
            .collect();
        if square {
            assert!(
                same(far[0], p(20.0, 0.0)) && same(far[1], p(20.0 + rise, 3.0)) || same(far[1], p(20.0, 0.0)) && same(far[0], p(20.0 + rise, 3.0)),
                "{far:?}"
            );
        } else {
            assert!(far.iter().all(|&q| same(q, p(20.0 + rise, 3.0))), "{slant}°: {far:?}");
        }
    }
    // Half a ring 3 mm wide ends square, along its ends' edges, each cut in 2 pieces by the points along it:
    // where the end reaches farthest at the point between them, it ends along the whole edge, not one piece,
    // which would leave the other on a rail.
    let mut outline = arc(20.0, 20.0, 12.0, 36, 0.0, std::f64::consts::PI);
    outline.extend(arc(20.0, 20.0, 9.0, 36, std::f64::consts::PI, -std::f64::consts::PI));
    let [column] = found(&part(&outline, &[])).try_into().unwrap();
    for rail in &column.rails {
        for end in [rail.first().unwrap(), rail.last().unwrap()] {
            let radius = end.distance(p(20.0, 20.0));
            assert!((radius - 12.0).abs() < 1e-9 || (radius - 9.0).abs() < 1e-9, "{end:?} {:?}", column.rails);
        }
    }
}

#[test]
fn req_auto_016_an_end_along_an_edge_takes_the_whole_straight_edge() {
    // The outline of a column 10 mm long and 3 mm wide, its end at x = 10 cut in 2 pieces, and the line going
    // along x. However the end's arc runs, and wherever on the edge the end reaches farthest, it ends along
    // the whole edge: from an end of it, on round the ring past the arc, and from the point between its
    // pieces, both ways; but not along an edge longer than an end may be.
    let ring_points = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 1.5), p(10.0, 3.0), p(0.0, 3.0)];
    let triangulation = crate::normalize::triangulation::Triangulation { points: ring_points.to_vec(), triangles: Vec::new() };
    let graph = super::super::Graph { points: Vec::new(), nodes: Vec::new(), links: Vec::new(), size: Vec::new(), inside: BTreeMap::new() };
    let skeleton = Skeleton { triangulation, graph, walked: Vec::new() };
    let outline = Outline { rings: Vec::new(), triangle: BTreeMap::new() };
    let ring = [0, 1, 2, 3, 4];
    let place = Place { skeleton: &skeleton, owner: &[], line: 0, outline: &outline, ring: &ring };
    for arc in [&[0, 1, 2][..], &[2, 3, 4], &[1, 2, 3]] {
        assert_eq!(place.farthest(arc, (1.0, 0.0), 6.0), Some((1, 3)), "{arc:?}");
    }
    // Tilted 30° from the edge's normal, the end reaches farthest at a corner.
    let (sin, cos) = sin_cos(math::to_radians(30.0));
    assert_eq!(place.farthest(&[1, 2, 3], (cos, sin), 6.0), Some((3, 3)));
    // An edge longer than the longest an end may be runs along the column: the end is at its farthest point.
    assert_eq!(place.farthest(&[1, 2, 3], (1.0, 0.0), 2.9), Some((1, 1)));
}

#[test]
fn req_auto_016_a_ring_is_a_closed_column_started_at_a_rung() {
    // A ring 10 mm round the outside and 7 mm round its hole: one closed column, its rails the outside and
    // the hole, the same way round, each ending where it starts, the hole's start its point nearest the
    // outside's start, and its rungs along the ring's radius.
    let outer = arc(0.0, 0.0, 10.0, 24, 0.0, std::f64::consts::TAU);
    let inner = arc(0.0, 0.0, 7.0, 24, 0.0, std::f64::consts::TAU);
    let ring = part(&outer[..24], &[&inner[..24]]);
    let [column] = found(&ring).try_into().unwrap();
    assert!(column.closed);
    let [out, hole] = &column.rails;
    assert!(same(out[0], *out.last().unwrap()) && same(hole[0], *hole.last().unwrap()));
    assert!(out.iter().all(|q| (q.distance(Point::ORIGIN) - 10.0).abs() < 0.2) && hole.iter().all(|q| (q.distance(Point::ORIGIN) - 7.0).abs() < 0.2));
    let turn = |rail: &[Point]| geom::signed_area(rail).signum();
    assert_eq!(turn(out), turn(hole));
    assert!(hole.iter().all(|q| q.distance(out[0]) >= hole[0].distance(out[0]) - 1e-12), "{:?}", (out[0], hole[0]));
    for &[a, b] in &column.rungs {
        let radial = math::to_degrees(math::atan2(a.y(), a.x()) - math::atan2(b.y(), b.x())).abs();
        assert!(radial < 8.0, "{a:?} {b:?}");
    }
    // Started near another point, it starts at the rung nearest it, as a ring still.
    let near = column.restarted(p(-10.0, 0.0), &mut Budget::DEFAULT.meter()).unwrap();
    assert!(near.rails[0][0].distance(p(-10.0, 0.0)) < 2.0 && near.closed, "{:?}", near.rails[0][0]);
}

#[test]
fn req_auto_016_the_stem_of_a_t_ends_under_its_bar() {
    // A T: a bar 20 mm long and 3 mm wide, a stem 3 mm wide and 12 mm long below its middle. The bar is one
    // column, whose rail on the stem's side crosses the stem's top, and the stem another, which ends there,
    // under the bar.
    let t = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (11.5, 3.0), (11.5, 15.0), (8.5, 15.0), (8.5, 3.0), (0.0, 3.0)], &[]);
    let columns = found(&t);
    let [a, b] = columns.as_slice() else { panic!("{columns:?}") };
    let (stem, bar, stem_index, bar_index) = if a.line.points.iter().any(|q| q.y() > 10.0) { (a, b, 0, 1) } else { (b, a, 1, 0) };
    assert!(stem.under.contains(&Some(bar_index)) && bar.under == [None, None], "{columns:?}");
    assert_ne!(stem_index, bar_index);
    // The stem's end under the bar is the chord across its top, from one of its sides to the other.
    let under_end = if stem.under[0].is_some() { 0 } else { 1 };
    let end = |rail: &Vec<Point>| if under_end == 0 { rail[0] } else { *rail.last().unwrap() };
    let mut corners = [end(&stem.rails[0]), end(&stem.rails[1])];
    corners.sort_by(|a, b| a.x().total_cmp(&b.x()));
    assert!(same(corners[0], p(8.5, 3.0)) && same(corners[1], p(11.5, 3.0)), "{corners:?}");
    // The bar's rail on that side goes straight across from the stem's one side to its other.
    let side = bar.rails.iter().find(|rail| rail.iter().all(|q| q.y() == 3.0)).unwrap();
    assert!(
        side.windows(2).any(|w| same(w[0], p(11.5, 3.0)) && same(w[1], p(8.5, 3.0)) || same(w[0], p(8.5, 3.0)) && same(w[1], p(11.5, 3.0))),
        "{side:?}"
    );
}

#[test]
fn req_auto_016_an_arm_twice_as_long_as_its_junction_is_a_column_of_its_own() {
    // A T whose bar's arms are 4 mm wide and 10.5 mm long on either side of a stem 4 mm wide, cut into
    // pieces 3.6 mm long, as wide as it is on average: a narrow part's line would take an arm as a bump, no
    // longer than twice its junction triangle's longest edge; each arm here is a column of its own or runs on
    // into the other, and nothing goes round it.
    let t = part(&[(5.0, 20.0), (30.0, 20.0), (30.0, 24.0), (19.5, 24.0), (19.5, 40.0), (15.5, 40.0), (15.5, 24.0), (5.0, 24.0)], &[]);
    let columns = found(&t);
    assert_eq!(columns.len(), 2, "{columns:?}");
    for column in &columns {
        // Neither column's rails go round the end of an arm: each side of a column is straight but where it
        // crosses another's end.
        for rail in &column.rails {
            let left = rail.iter().map(|q| q.x()).fold(f64::MAX, f64::min);
            let up = rail.iter().filter(|q| q.x() < 15.5 - 1e-9).map(|q| q.y()).fold(f64::NAN, f64::max);
            assert!(left >= 5.0 - 1e-9 && (up.is_nan() || up <= 24.0 + 1e-9), "{rail:?}");
        }
    }
}

/// How far the farthest point of `column`'s rails lies from its line as walked.
fn reach(column: &Column) -> f64 {
    let from_line = |q: Point| {
        let line = &column.walked.points;
        line.windows(2)
            .map(|w| {
                let (a, b) = (w[0], w[1]);
                let span = a.distance(b);
                let t = (((q.x() - a.x()) * (b.x() - a.x()) + (q.y() - a.y()) * (b.y() - a.y())) / (span * span)).clamp(0.0, 1.0);
                q.distance(a.lerp(b, t))
            })
            .fold(f64::INFINITY, f64::min)
    };
    column.rails.iter().flatten().map(|&q| from_line(q)).fold(0.0, f64::max)
}

#[test]
fn req_auto_016_a_crossbar_is_no_bump_of_the_stem_it_crosses() {
    // A t: a stem 3 mm wide, and a crossbar 3 mm wide across it, its arms 3 and 4 mm long. Each arm is a
    // column of its own, or runs on through the stem, and each column keeps to its own strip, its rails no
    // farther from its line than the strip is wide: none goes out along another's arm, as a stem that took
    // the arms for bumps would, with stitches 10 mm long across both.
    let t = part(
        &[
            (3.0, 0.0),
            (6.0, 0.0),
            (6.0, 14.0),
            (10.0, 14.0),
            (10.0, 17.0),
            (6.0, 17.0),
            (6.0, 21.0),
            (3.0, 21.0),
            (3.0, 17.0),
            (0.0, 17.0),
            (0.0, 14.0),
            (3.0, 14.0),
        ],
        &[],
    );
    let columns = found(&t);
    assert_eq!(columns.len(), 3, "{columns:?}");
    assert_eq!(columns.iter().filter(|c| c.under == [None, None]).count(), 1, "{columns:?}");
    for column in &columns {
        assert!(reach(column) <= 3.0, "{} {column:?}", reach(column));
    }
}

#[test]
fn req_auto_016_a_stub_beyond_a_junction_is_a_column_that_is_all_end() {
    // A bar 3 mm wide with a stub 4 mm wide and 2 mm long below its middle, or above it, its end cut in 2
    // pieces: the stub's line runs from the middle of the junction triangle between its end's 2 caps to the
    // bar's junction, and the chord between the 2 is both its end under the bar and where its own end begins.
    // Its end goes round the stub's end, whichever way round the outline passes the chord, and its rails are
    // the stub's sides.
    let below = [(0.0, 2.0), (6.0, 2.0), (6.0, 0.0), (10.0, 0.0), (10.0, 2.0), (16.0, 2.0), (16.0, 5.0), (0.0, 5.0)];
    for outline in [below.to_vec(), below.iter().map(|&(x, y)| (x, 5.0 - y)).collect()] {
        let columns = found(&part(&outline, &[]));
        let [bar, short] = columns.as_slice() else { panic!("{columns:?}") };
        assert!(bar.under == [None, None] && short.under.contains(&Some(0)), "{columns:?}");
        let mut sides: Vec<[Point; 2]> = short.rails.iter().map(|rail| [rail[0], *rail.last().unwrap()]).collect();
        sides.sort_by(|a, b| a[0].x().total_cmp(&b[0].x()));
        assert!(sides[0].iter().all(|q| q.x() == 6.0) && sides[1].iter().all(|q| q.x() == 10.0), "{sides:?}");
    }
}

#[test]
fn req_auto_016_a_branch_to_a_tip_is_a_bump_only_where_it_ends_at_its_junction() {
    // A stem 3 mm wide, a crossbar 3 mm wide to one side of it, and below that a foot 4 mm long cut at a slant
    // to a sharp corner: the stem's line runs on most nearly straight into the foot, to its corner, and the
    // crossbar ends under it. The foot is no bump, short as it is: taken for one, the crossbar's column would
    // cover it, with stitches 5.5 mm out from its line.
    let foot = part(&[(0.0, 0.0), (3.0, 1.2), (3.0, 20.0), (0.0, 20.0), (0.0, 7.0), (-10.0, 7.0), (-10.0, 4.0), (0.0, 4.0)], &[]);
    let columns = found(&foot);
    let [crossbar, stem] = columns.as_slice() else { panic!("{columns:?}") };
    assert!(crossbar.under.contains(&Some(1)) && stem.under == [None, None], "{columns:?}");
    assert!(stem.rails.iter().all(|rail| same(*rail.last().unwrap(), p(0.0, 0.0))), "{stem:?}");
    assert!(columns.iter().all(|column| reach(column) <= 3.0), "{columns:?}");
    // A wide square end's 2 sharp corners are branches to tips from the junction beside the end: the line runs
    // on into one, and the other, which ends at its junction, is a bump, which the column's end covers.
    let wedge = part(&[(0.0, -0.5), (20.0, -2.5), (20.0, 2.5), (0.0, 0.5)], &[]);
    let [column] = found(&wedge).try_into().unwrap();
    let ends: Vec<Point> = column.rails.iter().map(|rail| *rail.first().unwrap()).collect();
    assert!(ends.iter().any(|&q| same(q, p(20.0, 2.5))) && ends.iter().any(|&q| same(q, p(20.0, -2.5))), "{ends:?}");
}

#[test]
fn req_auto_016_after_a_sharp_corner_the_column_ends_under_the_one_before() {
    // An L, 3 mm wide, its arms 15 mm long: it turns 90° at its corner, more than 60°, and is 2 columns. One
    // covers the corner, and the other ends under it.
    let l = part(&[(0.0, 0.0), (15.0, 0.0), (15.0, 3.0), (3.0, 3.0), (3.0, 15.0), (0.0, 15.0)], &[]);
    let columns = found(&l);
    let [a, b] = columns.as_slice() else { panic!("{columns:?}") };
    let covering = [a, b].into_iter().position(|c| c.under == [None, None]).unwrap();
    let other = 1 - covering;
    assert!(columns[other].under.contains(&Some(covering)), "{columns:?}");
    // The column that covers the corner has its outside corner, (0, 0), between its rails' ends.
    let corner = &columns[covering];
    let reaches = corner.rails.iter().flatten().any(|&q| same(q, p(0.0, 0.0)));
    assert!(reaches, "{corner:?}");
    // A gentle bend of 30° is one column.
    let (sin, cos) = sin_cos(math::to_radians(30.0));
    let bent = part(
        &[
            (-15.0, 0.0),
            (0.0, 0.0),
            (15.0 * cos, 15.0 * sin),
            (15.0 * cos - 3.0 * sin, 15.0 * sin + 3.0 * cos),
            (-3.0 * sin / (1.0 + cos), 3.0),
            (-15.0, 3.0),
        ],
        &[],
    );
    assert_eq!(found(&bent).len(), 1);
}

#[test]
fn req_auto_016_a_flat_ended_v_is_cut_once_at_its_point() {
    // A V whose arms, 3 mm wide, meet in a flat end: 2 corners, side by side, cut the line once, and the arms
    // are 2 columns, one ending under the other.
    let v = part(&[(0.0, 15.0), (3.0, 15.0), (6.0, 3.0), (9.0, 15.0), (12.0, 15.0), (7.5, 0.0), (4.5, 0.0)], &[]);
    let columns = found(&v);
    assert_eq!(columns.len(), 2, "{columns:?}");
    assert!(columns.iter().any(|c| c.under.iter().flatten().count() == 1));
}

#[test]
fn req_auto_016_a_line_that_comes_back_to_its_junction_is_cut_into_a_line_and_a_ring() {
    // A b: a stem 3 mm wide down to a bowl 3 mm wide round a round hole 6 mm across, the stem's sides running
    // on into the bowl's, so that the stem's line runs on down into the bowl, round it and back to where it
    // left the stem. It is cut there, into the stem, which ends under the bowl, and the bowl, a ring round the
    // hole.
    let mut outline = arc(6.0, 6.0, 6.0, 30, std::f64::consts::PI, 5.0 * std::f64::consts::FRAC_PI_3);
    outline.extend([(3.0, 24.0), (0.0, 24.0)]);
    let hole = arc(6.0, 6.0, 3.0, 24, 0.0, std::f64::consts::TAU);
    let b = part(&outline, &[&hole[..24]]);
    let columns = found(&b);
    let [one, other] = columns.as_slice() else { panic!("{columns:?}") };
    let (stem, bowl) = if one.closed { (other, one) } else { (one, other) };
    assert!(bowl.closed && !stem.closed, "{columns:?}");
    let bowl_index = usize::from(std::ptr::eq(bowl, other));
    assert!(stem.under.contains(&Some(bowl_index)) && bowl.under == [None, None], "{columns:?}");
}

#[test]
fn req_auto_016_a_dot_has_no_column() {
    // A square 1 mm across in 2 triangles: the circle on its diagonal holds every corner, so the caps of its
    // ends meet, and it has no line.
    let dot = part(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(columns(&dot, 10.0, &mut Budget::DEFAULT.meter()).unwrap().unwrap(), Some(Vec::new()));
}

#[test]
fn req_auto_017_rungs_lie_square_to_the_column_along_a_curve_and_a_taper() {
    // Half a ring, 3 mm wide: each rung runs along the radius through its middle.
    let mut outline = arc(0.0, 0.0, 12.0, 24, 0.0, std::f64::consts::PI);
    outline.extend(arc(0.0, 0.0, 9.0, 24, std::f64::consts::PI, -std::f64::consts::PI));
    let [column] = found(&part(&outline, &[])).try_into().unwrap();
    assert!(column.rungs.len() > 10, "{:?}", column.rungs);
    for &[a, b] in &column.rungs {
        let middle = a.lerp(b, 0.5);
        let radial = (heading([a, b]) - math::to_degrees(math::atan2(middle.y(), middle.x())).rem_euclid(180.0)).abs();
        assert!(radial.min(180.0 - radial) < 4.0, "{a:?} {b:?}");
    }
    // A wedge from 1 mm to 5 mm wide over 20 mm: each rung lies square to its middle line, the x axis.
    let wedge = part(&[(0.0, -0.5), (20.0, -2.5), (20.0, 2.5), (0.0, 0.5)], &[]);
    let [column] = found(&wedge).try_into().unwrap();
    assert!(!column.rungs.is_empty());
    for &rung in &column.rungs {
        assert!((heading(rung) - 90.0).abs() < 1e-6, "{rung:?}");
    }
}

#[test]
fn req_auto_017_of_rungs_too_near_each_other_the_one_nearer_square_is_kept() {
    // 4 rungs across rails 10 mm long, each 3 mm long: drawn 1 % past the rails, rungs closer than 0.06 mm on
    // a rail would cross. Of the first 2, 0.05 mm apart, the one nearer a right angle stays; the third shares
    // its point on the second rail with the fourth, which lies nearer a right angle; rungs at a rail's end go.
    let rung = |a: f64, b: f64, off: f64| Rung { points: [p(a, 0.0), p(b, 3.0)], at: [a, b], off };
    let kept = ordered(
        vec![rung(2.0, 2.0, 0.3), rung(2.05, 2.05, 0.1), rung(5.0, 6.0, 0.2), rung(6.0, 6.0, 0.0), rung(0.0, 0.0, 0.0), rung(10.0, 10.0, 0.0)],
        [10.0, 10.0],
        None,
    );
    assert_eq!(kept, [rung(2.05, 2.05, 0.1), rung(6.0, 6.0, 0.0)]);
    // As near, the first stays.
    assert_eq!(ordered(vec![rung(2.0, 2.0, 0.0), rung(2.05, 2.05, 0.0)], [10.0, 10.0], None), [rung(2.0, 2.0, 0.0)]);
    // A better rung takes the last one's place only where it lies far enough past the one before: the third
    // lies 0.03 mm past the first on the second rail, and the second stays.
    let crowded = [rung(1.0, 1.0, 0.0), rung(1.1, 1.1, 0.5), rung(1.12, 1.03, 0.1)];
    assert_eq!(ordered(crowded.to_vec(), [10.0, 10.0], None), crowded[..2]);
    // Where it does, it does.
    assert_eq!(
        ordered(vec![rung(1.0, 1.0, 0.0), rung(1.1, 1.1, 0.5), rung(1.15, 1.15, 0.1)], [10.0, 10.0], None),
        [rung(1.0, 1.0, 0.0), rung(1.15, 1.15, 0.1)]
    );
}
