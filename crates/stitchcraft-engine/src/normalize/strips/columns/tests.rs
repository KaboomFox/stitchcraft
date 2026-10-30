//! Columns along centre lines (`REQ-AUTO-016`, `REQ-AUTO-017`): a band is one column between its sides, a
//! ring a closed one, an arm of a T or the column after a corner ends under the one beside it, and the rungs
//! lie square to the column.

use stitchcraft_core::Budget;
use stitchcraft_core::math::{self, sin_cos};

use super::super::Graph;
use super::*;
use crate::normalize::fixture::part;
use crate::normalize::region::geom;
use crate::normalize::stroke::segments;
use crate::normalize::triangulation::Triangulation;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
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

/// How long the pieces of `part`'s edges are cut, as automatic digitizing cuts them: as long as it is wide on
/// average.
fn piece(part: &Polygon) -> f64 {
    let perimeter: f64 = part.rings().flat_map(segments).map(|(a, b)| a.distance(b)).sum();
    2.0 * part.area() / perimeter
}

/// The columns of `part`, `None` when its triangles make none, or the triangulation's failure.
fn made(part: &Polygon) -> Result<Option<Vec<Column>>, Broken> {
    columns(part, piece(part), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The columns of `part`.
fn found(part: &Polygon) -> Vec<Column> {
    made(part).unwrap().unwrap()
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
    // A band whose end is an edge 10° or 15° from square to it ends along that edge, its rails at its 2 ends;
    // one 25° or 30° from square ends at its farthest corner, where its rails meet. (The way the edge is
    // measured against tilts a little towards the slant, so the band's own 20° isn't the bound.)
    for (slant, square) in [(10.0, true), (15.0, true), (25.0, false), (30.0, false)] {
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
    // A band turned 30°, its column's way slanting, ends along its square ends: its rails run from corner to
    // corner.
    let (sin, cos) = sin_cos(math::to_radians(30.0));
    let corners = [(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (0.0, 3.0)].map(|(x, y)| (x * cos - y * sin, x * sin + y * cos));
    let [column] = found(&part(&corners, &[])).try_into().unwrap();
    for rail in &column.rails {
        for end in [rail.first().unwrap(), rail.last().unwrap()] {
            assert!(corners.iter().any(|&(x, y)| same(*end, p(x, y))), "{end:?} {:?}", column.rails);
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
    // The outline of a column 10 mm long and 3 mm wide, its end at x = 10 cut in 2 pieces, the line going
    // along x, and the end's triangles beginning at a chord across it, 3 mm long. However the end's arc runs,
    // and wherever on the edge the end reaches farthest, it ends along the whole edge: from an end of it, on
    // round the ring past the arc, and from the point between its pieces, both ways; but not along an edge
    // longer than twice the chord.
    let ring_points = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 1.5), p(10.0, 3.0), p(0.0, 3.0)];
    let triangulation = crate::normalize::triangulation::Triangulation { points: ring_points.to_vec(), triangles: Vec::new() };
    let graph = super::super::Graph { points: Vec::new(), nodes: Vec::new(), links: Vec::new(), size: Vec::new(), inside: BTreeMap::new() };
    let skeleton = Skeleton { triangulation, graph, walked: Vec::new() };
    let outline = Outline { rings: Vec::new(), triangle: BTreeMap::new() };
    let ring = [0, 1, 2, 3, 4];
    let place = Place { skeleton: &skeleton, owner: &[], line: 0, outline: &outline, ring: &ring };
    for arc in [&[0, 1, 2][..], &[2, 3, 4], &[1, 2, 3]] {
        assert_eq!(place.farthest(arc, (1.0, 0.0), 3.0), Some((1, 3)), "{arc:?}");
    }
    // Tilted 30° from the edge's normal, the end reaches farthest at a corner.
    let (sin, cos) = sin_cos(math::to_radians(30.0));
    assert_eq!(place.farthest(&[1, 2, 3], (cos, sin), 3.0), Some((3, 3)));
    // Beyond a chord of 1.45 mm, the 3 mm edge is too long, and runs along the column: the end is at its
    // farthest point.
    assert_eq!(place.farthest(&[1, 2, 3], (1.0, 0.0), 1.45), Some((1, 1)));
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
    // Started near another point, it starts at the rung nearest it, as a ring still, its rungs from there.
    let near = column.restarted(p(-10.0, 0.0), &mut Budget::DEFAULT.meter()).unwrap();
    assert!(near.rails[0][0].distance(p(-10.0, 0.0)) < 2.0 && near.closed, "{:?}", near.rails[0][0]);
    assert!(near.rungs.len() == column.rungs.len() && near.rungs[0][0].distance(near.rails[0][0]) < 4.0, "{:?}", near.rungs);
    // Turned round, a ring is itself.
    assert_eq!(column.reversed(), column);
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
    // The bar's line is the line as walked, end to end. The stem's stops at the chord its column ends at, under
    // the bar, short of the junction's middle that its walked line runs on to.
    assert_eq!(bar.line, bar.walked);
    let mut short = stem.walked.points.clone();
    if under_end == 0 {
        short.remove(0);
    } else {
        short.pop();
    }
    assert_eq!(stem.line.points, short);
    // Turned round, the stem runs from its other end, its rails, rungs and lines the other way, and ends under
    // the bar at its other end. Turned round again, it is itself.
    let turned = stem.reversed();
    let back = |points: &[Point]| points.iter().rev().copied().collect::<Vec<_>>();
    assert_eq!(turned.rails, [back(&stem.rails[0]), back(&stem.rails[1])]);
    assert_eq!(turned.rungs, stem.rungs.iter().rev().copied().collect::<Vec<_>>());
    assert_eq!((turned.line.points.clone(), turned.walked.points.clone()), (back(&stem.line.points), back(&stem.walked.points)));
    assert_eq!((turned.under, turned.closed), ([stem.under[1], stem.under[0]], false));
    assert_eq!(turned.reversed(), *stem);
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

#[test]
fn a_part_whose_column_would_touch_itself_at_a_point_makes_none() {
    // A ring whose round hole touches its outline at a point, 4 mm wide on the far side: its line runs round
    // from one side of that point to the other, and its column would touch itself there. It makes no column.
    let outline = arc(0.0, 0.0, 4.0, 32, 0.0, std::f64::consts::TAU);
    let hole = arc(2.0, 0.0, 2.0, 16, 0.0, std::f64::consts::TAU);
    assert_eq!(made(&part(&outline[..32], &[&hole[..16]])), Ok(None));
}

#[test]
fn a_triangulation_that_loses_its_shape_is_said_to() {
    // A ring whose hole pokes out through an edge of its outline, 0.05 mm, which no region has: its
    // triangulation loses its shape, as a bug would make it.
    let outline = arc(50.0, 0.0, 4.0, 20, std::f64::consts::PI / 20.0, std::f64::consts::TAU);
    let hole = arc(51.6, 0.0, 2.4, 40, 0.0, std::f64::consts::TAU);
    assert!(matches!(made(&part(&outline[..20], &[&hole[..40]])), Err(Broken)));
}

#[test]
fn a_rings_column_is_the_same_whichever_of_its_outlines_comes_first() {
    // The ring's outlines found the other way about, its hole's first, make the same column. 2 outlines that
    // go the same way round, which no ring's column has, make none.
    let outer = arc(0.0, 0.0, 10.0, 24, 0.0, std::f64::consts::TAU);
    let inner = arc(0.0, 0.0, 7.0, 24, 0.0, std::f64::consts::TAU);
    let shape = part(&outer[..24], &[&inner[..24]]);
    let mut meter = Budget::DEFAULT.meter();
    let skeleton = skeleton(&shape, piece(&shape), Bumps::Tips, &mut meter).unwrap().unwrap();
    let lines = unlooped(&cornered(&skeleton, &skeleton.walked));
    let [walked] = lines.as_slice() else { panic!("{} lines", lines.len()) };
    let owner = owners(&skeleton, &lines, &mut meter).unwrap().unwrap();
    let found = outline(&skeleton, &owner, 0, &mut meter).unwrap().unwrap();
    let made_with = |rings: Vec<Vec<usize>>| {
        let outline = Outline { rings, triangle: found.triangle.clone() };
        ring(&skeleton, &owner, 0, &outline, skeleton.line(walked), &mut Budget::DEFAULT.meter()).unwrap()
    };
    let [one, other] = [found.rings[0].clone(), found.rings[1].clone()];
    let column = made_with(vec![one.clone(), other.clone()]);
    assert!(column.as_ref().is_some_and(|column| column.closed));
    assert_eq!(made_with(vec![other, one.clone()]), column);
    assert_eq!(made_with(vec![one.clone(), one]), None);
}

#[test]
fn a_ring_starts_at_the_nearest_rung_where_its_hole_is_nearest() {
    // A square ring 10 mm across round a square hole 4 mm across, and 2 rungs: one slanting from (6, 10) to
    // (3, 7), nearest where the last column ended, and one straight across from (5, 0) to (5, 3). The hole's
    // corner (7, 7) lies nearer (6, 10) than (3, 7) does, so a satin column would start the hole there and not
    // at the slanting rung: the ring starts at the straight one, and the slanting one is its rung.
    let closed = |corners: &[(f64, f64)]| geom::closed(&corners.iter().map(|&(x, y)| p(x, y)).collect::<Vec<_>>());
    let rails = [closed(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]), closed(&[(3.0, 3.0), (7.0, 3.0), (7.0, 7.0), (3.0, 7.0)])];
    let slanting = Rung { points: [p(6.0, 10.0), p(3.0, 7.0)], at: [24.0, 12.0], off: 0.0 };
    let straight = Rung { points: [p(5.0, 0.0), p(5.0, 3.0)], at: [5.0, 2.0], off: 0.0 };
    let ring = Ring { rails, lengths: [40.0, 16.0], rungs: vec![slanting, straight] };
    let start = |ring: &Ring| started(ring, p(6.0, 10.0), &mut Budget::DEFAULT.meter()).unwrap();
    let Some(Started { rails, rungs }) = start(&ring) else { panic!("no start") };
    assert_eq!((rails[0][0], rails[1][0]), (p(5.0, 0.0), p(5.0, 3.0)));
    assert_eq!(rungs, [slanting.points]);
    // With the slanting rung alone, it starts nowhere.
    assert!(start(&Ring { rungs: vec![slanting], ..ring }).is_none());
    // And a rail of one point has no length to start round.
    assert_eq!(from(&[p(1.0, 1.0)], 0.0, p(1.0, 1.0)), None);
}

#[test]
fn the_rails_run_from_the_start_to_the_end_on_either_side() {
    // A ring of 8 places. Ends from place 0 to 1 and from 4 to 5: one rail runs from 1 to 4, the other from 0
    // back round to 5, both from the start. Ends at a point each, at places 6 and 2: both rails run from 6,
    // one each way round the ring, to 2.
    let ring: Vec<usize> = (10..18).collect();
    assert_eq!(rails(&ring, (0, 1), (4, 5)), Some([vec![11, 12, 13, 14], vec![10, 17, 16, 15]]));
    assert_eq!(rails(&ring, (6, 6), (2, 2)), Some([vec![16, 17, 10, 11, 12], vec![16, 15, 14, 13, 12]]));
    // Ends that overlap, ends that meet, which would leave a rail of one point, and an end the wrong way round
    // make none.
    assert_eq!(rails(&ring, (0, 3), (2, 4)), None);
    assert_eq!(rails(&ring, (0, 2), (2, 3)), None);
    assert_eq!(rails(&ring, (0, 1), (5, 4)), None);
}

#[test]
fn rails_that_run_opposite_ways_give_no_rungs() {
    // A band's rails as found give its rungs. With one turned round, each corner's rail runs the other way from
    // the edge across from it: the column has no way there, and no rung is drawn.
    let band = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (0.0, 3.0)], &[]);
    let mut meter = Budget::DEFAULT.meter();
    let skeleton = skeleton(&band, piece(&band), Bumps::Tips, &mut meter).unwrap().unwrap();
    let lines = unlooped(&cornered(&skeleton, &skeleton.walked));
    let owner = owners(&skeleton, &lines, &mut meter).unwrap().unwrap();
    let [column] = found(&band).try_into().unwrap();
    let place = |q: &Point| skeleton.triangulation.points.iter().position(|&p| same(p, *q)).unwrap();
    let [one, other] = column.rails.map(|rail| rail.iter().map(place).collect::<Vec<_>>());
    assert!(!rungs(&skeleton, &owner, 0, &[one.clone(), other.clone()], &mut meter).unwrap().is_empty());
    let turned: Vec<usize> = other.into_iter().rev().collect();
    assert!(rungs(&skeleton, &owner, 0, &[one, turned], &mut meter).unwrap().is_empty());
}

#[test]
fn a_rung_meets_the_edge_at_a_right_angle_to_the_way_or_at_its_nearer_end() {
    // From (5, 3), its rail running along x, to an edge along x below it: the rung runs straight down. It meets
    // the edge from (4, 0) to (8, 0) a quarter of the way along, at (5, 0), 1 mm along it. The edge from (0, 0)
    // to (2, 0) it misses: it meets the edge's line at (5, 0), so it ends at the edge's nearer end, 2 mm along
    // it, 3 mm short.
    let (corner, rail) = (p(5.0, 3.0), (p(4.0, 3.0), p(6.0, 3.0)));
    assert_eq!(meet(corner, rail, (p(4.0, 0.0), p(8.0, 0.0))), Some((p(5.0, 0.0), 1.0, 0.0)));
    assert_eq!(meet(corner, rail, (p(0.0, 0.0), p(2.0, 0.0))), Some((p(2.0, 0.0), 2.0, 3.0)));
    // An edge that runs back the other way, a rail of no length at the corner, or an edge of none: no rung.
    assert_eq!(meet(corner, rail, (p(2.0, 0.0), p(0.0, 0.0))), None);
    assert_eq!(meet(corner, (corner, corner), (p(0.0, 0.0), p(2.0, 0.0))), None);
    assert_eq!(meet(corner, rail, (p(2.0, 0.0), p(2.0, 0.0))), None);
}

/// A skeleton of no triangles whose graph has the points `points`, each with its junction's size if it is a
/// junction's middle, for the tests of what becomes of its lines.
fn drawn(points: &[((f64, f64), Option<f64>)]) -> Skeleton {
    let mut graph = Graph { points: Vec::new(), nodes: Vec::new(), links: Vec::new(), size: Vec::new(), inside: BTreeMap::new() };
    for (i, &((x, y), size)) in points.iter().enumerate() {
        graph.add(p(x, y), size, Node::Junction(i));
    }
    Skeleton { triangulation: Triangulation { points: Vec::new(), triangles: Vec::new() }, graph, walked: Vec::new() }
}

/// The lines that a line through all of `points` in order, `closed` or not, is cut into at its corners.
fn cut(points: &[((f64, f64), Option<f64>)], closed: bool) -> Vec<Walked> {
    cornered(&drawn(points), &[Walked { nodes: (0..points.len()).collect(), closed }])
}

/// An open line through the points `nodes`.
fn open(nodes: &[usize]) -> Walked {
    Walked { nodes: nodes.to_vec(), closed: false }
}

#[test]
fn req_auto_016_a_ring_is_cut_at_its_corners_when_it_has_2_or_more() {
    // A closed line round a square, with junctions 1 mm across at its corners, turns by 90° at each: it is cut
    // into 4 lines, each from a corner to the next. Round a lens, whose 2 tips turn it by 118°, it is cut into
    // 2. Round a drop, with one corner, it stays a ring: cut there, it would still go round its hole.
    let corner = Some(1.0);
    let square = [
        ((0.0, 0.0), corner),
        ((5.0, 0.0), None),
        ((10.0, 0.0), corner),
        ((10.0, 5.0), None),
        ((10.0, 10.0), corner),
        ((5.0, 10.0), None),
        ((0.0, 10.0), corner),
        ((0.0, 5.0), None),
    ];
    assert_eq!(cut(&square, true), [open(&[0, 1, 2]), open(&[2, 3, 4]), open(&[4, 5, 6]), open(&[6, 7, 0])]);
    // Walked from the middle of a side, it is cut at the same corners, from the first it comes to.
    let mut turned = square;
    turned.rotate_left(7);
    assert_eq!(cut(&turned, true), [open(&[1, 2, 3]), open(&[3, 4, 5]), open(&[5, 6, 7]), open(&[7, 0, 1])]);
    let lens = [((0.0, 0.0), corner), ((5.0, 3.0), None), ((10.0, 0.0), corner), ((5.0, -3.0), None)];
    assert_eq!(cut(&lens, true), [open(&[0, 1, 2]), open(&[2, 3, 0])]);
    let drop = [((0.0, 0.0), corner), ((5.0, 3.0), None), ((9.0, 2.0), None), ((10.0, 0.0), None), ((9.0, -2.0), None), ((5.0, -3.0), None)];
    assert_eq!(cut(&drop, true), [Walked { nodes: (0..6).collect(), closed: true }]);
}

#[test]
fn req_auto_016_of_2_corners_near_each_other_only_the_sharper_cuts_the_line() {
    // A flat-ended V's 2 corners, junctions 2 mm across 1 mm apart, mirror images of each other and so exactly
    // as sharp: the line is cut at the first only. The second sharper, it is cut there.
    let v = [((-5.0, 10.0), None), ((-0.5, 0.0), Some(2.0)), ((0.5, 0.0), Some(2.0)), ((5.0, 10.0), None)];
    assert_eq!(cut(&v, false), [open(&[0, 1]), open(&[1, 2, 3])]);
    let sharper = [((-5.0, 10.0), None), ((-0.5, 0.0), Some(2.0)), ((0.5, 0.0), Some(2.0)), ((3.0, 10.0), None)];
    assert_eq!(cut(&sharper, false), [open(&[0, 1, 2]), open(&[2, 3])]);
    // As far apart as twice the larger junction is across, 2 corners each cut it.
    let apart = [((-5.0, 10.0), None), ((-1.0, 0.0), Some(1.0)), ((1.0, 0.0), Some(1.0)), ((5.0, 10.0), None)];
    assert_eq!(cut(&apart, false), [open(&[0, 1]), open(&[1, 2]), open(&[2, 3])]);
}

#[test]
fn req_auto_016_a_line_that_turns_by_60_degrees_stays_one_line() {
    // A line through a junction 1 mm across, its ways from there (2, 0) and (-1, y), 120° apart: y is a double
    // as near √3 as makes the cosine between them come to exactly -0.5. The line turns by 60°, as sharply as
    // a column sews, and stays one line. A little more sharply, it is cut there.
    let turned = |x: f64| [((2.0, 0.0), None), ((0.0, 0.0), Some(1.0)), ((x, 1.732_050_807_568_877_4), None)];
    assert_eq!(cut(&turned(-1.0), false), [open(&[0, 1, 2])]);
    assert_eq!(cut(&turned(-0.99), false), [open(&[0, 1]), open(&[1, 2])]);
}

#[test]
fn a_crossbar_that_ends_under_2_columns_stops_short_of_both_junctions() {
    // An H whose crossbar, 3 mm wide, ends under both bars: its line stops at the chords its column ends at,
    // short of both junctions' middles that its walked line reaches.
    let h = part(
        &[
            (0.0, 0.0),
            (3.0, 0.0),
            (3.0, 8.5),
            (10.0, 8.5),
            (10.0, 0.0),
            (13.0, 0.0),
            (13.0, 20.0),
            (10.0, 20.0),
            (10.0, 11.5),
            (3.0, 11.5),
            (3.0, 20.0),
            (0.0, 20.0),
        ],
        &[],
    );
    let columns = found(&h);
    let crossbar = columns.iter().find(|column| column.under.iter().flatten().count() == 2).unwrap();
    let walked = &crossbar.walked.points;
    assert_eq!(crossbar.line.points, walked[1..walked.len() - 1]);
}

#[test]
fn the_end_of_a_line_at_a_chord_lies_beyond_it() {
    // A line that ends at a chord between triangles 5 and 6, its last piece in 6, a junction's middle where it
    // bends before it: its own end lies in 5, beyond the chord, and runs the way of its last piece. With 5
    // another line's, it ends under that line, at the chord. With only the chord and the bend, it has no chord
    // past the bend to end at, and ends at that chord.
    let mut graph = Graph { points: Vec::new(), nodes: Vec::new(), links: Vec::new(), size: Vec::new(), inside: BTreeMap::new() };
    let chord = graph.add(p(10.0, 0.0), None, Node::Chord { triangles: (5, 6), corners: (0, 1) });
    let bend = graph.add(p(8.0, 0.0), Some(1.0), Node::Junction(6));
    let before = graph.add(p(5.0, 1.0), None, Node::Chord { triangles: (6, 7), corners: (1, 2) });
    graph.join(chord, bend, 6);
    graph.join(bend, before, 6);
    let skeleton = Skeleton { triangulation: Triangulation { points: Vec::new(), triangles: Vec::new() }, graph, walked: Vec::new() };
    let mine = [0; 8];
    let Some((End::Own { corners, beyond, way }, false)) = end(&skeleton, &mine, 0, &[chord, bend]) else { panic!("no own end") };
    assert_eq!((corners, beyond, way), ((0, 1), 5, (2.0, 0.0)));
    let theirs = [0, 0, 0, 0, 0, 1, 0, 0];
    let Some((End::Cut { corners, under }, false)) = end(&skeleton, &theirs, 0, &[chord, bend]) else { panic!("no cut") };
    assert_eq!((corners, under), ((0, 1), 1));
}

#[test]
fn the_end_of_a_line_takes_in_the_bends_on_its_way_up_to_the_lines_middle() {
    // A line along x from one junction's middle, in triangle 0, to another's, in 5, through chords and bends in
    // turn, as along a ribbon toothed on both sides: chord k between triangles k and k + 1 and bend k in
    // triangle k. Its end takes in the bends from its first junction on up to the line's middle, and lies
    // beyond the chord there, the third. Where a third line branches off the second bend, the end stops
    // before it, beyond the second chord.
    let walked = |branch: bool| {
        let mut graph = Graph { points: Vec::new(), nodes: Vec::new(), links: Vec::new(), size: Vec::new(), inside: BTreeMap::new() };
        let nodes: Vec<usize> = (0..11_usize)
            .map(|k| {
                let node = if k % 2 == 1 { Node::Chord { triangles: (k / 2, k / 2 + 1), corners: (k, k + 1) } } else { Node::Junction(k / 2) };
                graph.add(p(k as f64, 0.0), Some(1.0), node)
            })
            .collect();
        for pair in nodes.windows(2) {
            graph.join(pair[0], pair[1], 0);
        }
        if branch {
            let off = graph.add(p(4.0, 1.0), None, Node::Chord { triangles: (2, 6), corners: (20, 21) });
            graph.join(nodes[4], off, 2);
        }
        (Skeleton { triangulation: Triangulation { points: Vec::new(), triangles: Vec::new() }, graph, walked: Vec::new() }, nodes)
    };
    let mine = [0; 7];
    let (skeleton, nodes) = walked(false);
    let Some((End::Own { corners, beyond, .. }, false)) = end(&skeleton, &mine, 0, &nodes) else { panic!("no end") };
    assert_eq!((corners, beyond), ((5, 6), 2));
    let (skeleton, nodes) = walked(true);
    let Some((End::Own { corners, beyond, way }, false)) = end(&skeleton, &mine, 0, &nodes) else { panic!("no end") };
    assert_eq!((corners, beyond, way), ((3, 4), 1, (-1.0, 0.0)));
}

#[test]
fn the_points_of_a_line_that_comes_back_to_a_junction_split_into_a_line_and_a_ring() {
    let walked = |nodes: &[usize]| Walked { nodes: nodes.to_vec(), closed: false };
    let ring = |nodes: &[usize]| Walked { nodes: nodes.to_vec(), closed: true };
    // Back to point 2 after it: the line up to it, and the ring from it round back to it.
    assert_eq!(unlooped(&[walked(&[0, 1, 2, 3, 4, 5, 2])]), [walked(&[0, 1, 2]), ring(&[2, 3, 4, 5])]);
    // The shortest that is cut: a line of 2 points up to it, and a ring of 3.
    assert_eq!(unlooped(&[walked(&[0, 1, 2, 3, 1])]), [walked(&[0, 1]), ring(&[1, 2, 3])]);
    // A ring of 2 points would be no ring, a line that comes back to where it started has no line up to its
    // ring, and a ring is one already: each stays as it is.
    for line in [walked(&[0, 1, 2, 1]), walked(&[0, 1, 2, 3, 0]), ring(&[0, 1, 2])] {
        assert_eq!(unlooped(std::slice::from_ref(&line)), [line]);
    }
}

#[test]
fn a_ring_rail_started_at_a_point_runs_round_back_to_it() {
    // A square rail 10 mm a side, from (0, 0) round back to it. Started at (10, 5), 15 mm along it, it runs
    // from that point round back to it. Started at a corner, the corner is its ends, and nowhere between.
    // Started past its length, it starts on its last side.
    let square = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)];
    let started = |at: f64, point: Point| from(&square, at, point).unwrap();
    assert_eq!(started(15.0, p(10.0, 5.0)), [p(10.0, 5.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0), p(10.0, 0.0), p(10.0, 5.0)]);
    assert_eq!(started(20.0, p(10.0, 10.0)), [p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)]);
    assert_eq!(started(45.0, p(0.0, 5.0)), [p(0.0, 5.0), p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 5.0)]);
}

/// What `test` finds on a ring through `points` in order, each the point of its own place.
fn on_ring<R>(points: &[(f64, f64)], test: impl FnOnce(&Place) -> R) -> R {
    let skeleton = Skeleton {
        triangulation: Triangulation { points: points.iter().map(|&(x, y)| p(x, y)).collect(), triangles: Vec::new() },
        graph: Graph { points: Vec::new(), nodes: Vec::new(), links: Vec::new(), size: Vec::new(), inside: BTreeMap::new() },
        walked: Vec::new(),
    };
    let outline = Outline { rings: Vec::new(), triangle: BTreeMap::new() };
    let ring: Vec<usize> = (0..points.len()).collect();
    test(&Place { skeleton: &skeleton, owner: &[], line: 0, outline: &outline, ring: &ring })
}

#[test]
fn a_point_is_in_line_within_a_millionth_of_a_millimetre() {
    // Through (0, 0) and (10, 0): (20, 0.0000005) lies on the line, (20, 0.000002) off it. Through 2 places at
    // one point there is no line.
    on_ring(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.000_000_5), (20.0, 0.000_002)], |place| {
        assert!(place.in_line(0, 1, 2) && !place.in_line(0, 1, 3) && !place.in_line(0, 0, 2));
    });
}

#[test]
fn an_end_lies_along_the_squarest_short_edge_at_its_farthest_point() {
    // A band 10 mm long whose end, past (10, 0), is an edge `slant`° from square to the band, the line going
    // `way` there and the end's triangles beginning at a chord 5 mm long: the end is the edge, places 1 to 2,
    // when the edge is within 20° of square, and the far corner, place 2, otherwise, however long the way is
    // measured.
    let slanted = |slant: f64, way: (f64, f64)| {
        let rise = 3.0 * math::tan(math::to_radians(slant));
        on_ring(&[(0.0, 0.0), (10.0, 0.0), (10.0 + rise, 3.0), (0.0, 3.0)], |place| place.farthest(&[1, 2], way, 5.0))
    };
    assert_eq!(slanted(15.0, (2.0, 0.0)), Some((1, 2)));
    assert_eq!(slanted(25.0, (0.5, 0.0)), Some((2, 2)));
    // An edge longer than twice the chord runs along the column: the end is the first of its points as far
    // along the way. The 3 mm edge is just longer than twice a chord of 1.45 mm.
    assert_eq!(on_ring(&[(0.0, 0.0), (10.0, 0.0), (10.0, 3.0), (0.0, 3.0)], |place| place.farthest(&[1, 2], (1.0, 0.0), 1.45)), Some((1, 1)));
    // The same band turned 30°, its way slanting: its square end is its end.
    let (sin, cos) = sin_cos(math::to_radians(30.0));
    let turned = [(0.0, 0.0), (10.0, 0.0), (10.0, 3.0), (0.0, 3.0)].map(|(x, y)| (x * cos - y * sin, x * sin + y * cos));
    assert_eq!(on_ring(&turned, |place| place.farthest(&[1, 2], (2.0 * cos, 2.0 * sin), 5.0)), Some((1, 2)));
    // A chevron whose 2 edges from its farthest point lie 15° and 10° from square: the end is the squarer.
    let chevron = [(0.0, 0.0), (10.0, 0.0), (10.8, 3.0), (10.27, 6.0), (0.0, 6.0)];
    assert_eq!(on_ring(&chevron, |place| place.farthest(&[1, 2, 3], (1.0, 0.0), 5.0)), Some((2, 3)));
}
