//! Centre lines (`REQ-AUTO-014`): a strip's runs along its middle, a ring's closes, a junction keeps its 2
//! straightest branches as one line, a bump or a sharp turn's corner is left out, a tip is reached, and a dot
//! has none.

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, Exhausted, Meter};

use super::*;
use crate::normalize::region::geom;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

fn meter() -> Meter {
    Budget::DEFAULT.meter()
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

/// The centre lines of `part` with pieces no longer than `longest`.
fn lines(part: &Polygon, longest: f64) -> Vec<Line> {
    centre_lines(part, longest, &mut meter()).unwrap().unwrap()
}

/// `n` points round a circle of radius `r` about (`x`, `y`).
fn circle(x: f64, y: f64, r: f64, n: usize) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let (sin, cos) = sin_cos(std::f64::consts::TAU * i as f64 / n as f64);
            (x + r * cos, y + r * sin)
        })
        .collect()
}

#[test]
fn req_auto_014_a_strip_runs_along_its_middle() {
    // A strip 0.6 mm wide and 20 mm long: one line along its middle, ending within its width of each end,
    // where the circle on the last chord holds the square end.
    let strip = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 0.6), (0.0, 0.6)], &[]);
    let found = lines(&strip, 0.6);
    assert_eq!(found.len(), 1, "{found:?}");
    let line = &found[0];
    assert!(!line.closed);
    assert!(line.points.iter().all(|q| (q.y() - 0.3).abs() < 1e-9), "{line:?}");
    let xs: Vec<f64> = line.points.iter().map(|q| q.x()).collect();
    let (low, high) = (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
    assert!(low > 0.0 && low <= 0.6 && (19.4..20.0).contains(&high), "{low} {high}");
    // In order along it.
    assert!(xs.windows(2).all(|w| w[1] > w[0]) || xs.windows(2).all(|w| w[1] < w[0]), "{xs:?}");
}

#[test]
fn req_auto_014_a_ring_closes() {
    // A ring 0.6 mm wide round a circle of radius 9.7 mm: one closed line along its middle.
    let ring = part(&circle(0.0, 0.0, 10.0, 120), &[&circle(0.0, 0.0, 9.4, 120)]);
    let found = lines(&ring, 0.6);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].closed);
    assert!(found[0].points.iter().all(|q| (math::hypot(q.x(), q.y()) - 9.7).abs() < 0.05), "{found:?}");
    let length = found[0].length();
    assert!((length - std::f64::consts::TAU * 9.7).abs() < 0.5, "{length}");
}

#[test]
fn req_auto_014_at_a_junction_the_straightest_branches_are_one_line() {
    // A T: a bar 20 mm long and 0.6 mm wide, and a stem 10 mm long below its middle. The bar is one line,
    // longest and so first, and the stem ends at the junction.
    let t = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 0.6), (10.3, 0.6), (10.3, 10.6), (9.7, 10.6), (9.7, 0.6), (0.0, 0.6)], &[]);
    let found = lines(&t, 0.6);
    assert_eq!(found.len(), 2, "{found:?}");
    let (bar, stem) = (&found[0], &found[1]);
    let xs: Vec<f64> = bar.points.iter().map(|q| q.x()).collect();
    assert!(xs.iter().copied().fold(f64::INFINITY, f64::min) < 0.7 && xs.iter().copied().fold(f64::NEG_INFINITY, f64::max) > 19.3, "{bar:?}");
    assert!(bar.points.iter().all(|q| q.y() < 0.6), "{bar:?}");
    assert!(stem.points.iter().all(|q| (q.x() - 10.0).abs() < 0.31), "{stem:?}");
    let ys: Vec<f64> = stem.points.iter().map(|q| q.y()).collect();
    assert!(ys.iter().copied().fold(f64::NEG_INFINITY, f64::max) > 9.9, "{stem:?}");
    // The stem starts where the bar ended, at its end nearest: the bar runs to x = 20, and the stem's
    // nearer end is its top, by the bar.
    assert!(stem.points[0].y() < 1.0, "{stem:?}");
}

#[test]
fn req_auto_014_a_bump_of_the_outline_is_left_out() {
    // A strip with bumps 0.2 to 0.3 mm high on its sides: a round one, within the circle on the chord across
    // it, is a cap, and 2 sharp ones are tips, whose branches are left out. The line runs along the middle.
    let bumped = part(
        &[
            (0.0, 0.0),
            (4.9, 0.0),
            (5.0, -0.3),
            (5.1, 0.0),
            (9.8, 0.0),
            (10.0, -0.2),
            (10.2, 0.0),
            (20.0, 0.0),
            (20.0, 0.6),
            (15.1, 0.6),
            (15.0, 0.9),
            (14.9, 0.6),
            (0.0, 0.6),
        ],
        &[],
    );
    let found = lines(&bumped, 0.6);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].points.iter().all(|q| (q.y() - 0.3).abs() < 0.15), "{found:?}");
}

#[test]
fn req_auto_014_a_sharp_turn_is_one_line() {
    // A V: 2 arms 15 mm long and 0.6 mm wide meeting at 36°, its tip at the origin. The corner beyond where
    // their middles meet is no branch, and the line runs from one arm's end round to the other's.
    let (sin, cos) = sin_cos(math::to_radians(18.0));
    let (x, y) = (15.0 * cos, 15.0 * sin);
    // The inner edges, 0.6 mm from the outer ones, meet on the x axis 0.6 / sin 18° from the tip.
    let inset = 0.6 / cos;
    let v = part(&[(0.0, 0.0), (x, -y), (x, -y + inset), (0.6 / sin, 0.0), (x, y - inset), (x, y)], &[]);
    let found = lines(&v, 0.6);
    assert_eq!(found.len(), 1, "{found:?}");
    assert!(found[0].length() > 25.0, "{found:?}");
}

/// A graph of `points`, each a junction's middle when it has a size, joined as `links` says, in turn.
fn graph(points: &[((f64, f64), Option<f64>)], links: &[(usize, usize)]) -> Graph {
    let mut graph = Graph { points: Vec::new(), links: Vec::new(), size: Vec::new() };
    for &((x, y), size) in points {
        graph.add(p(x, y), size);
    }
    for &(a, b) in links {
        graph.join(a, b);
    }
    graph
}

#[test]
fn req_auto_014_a_junctions_bumps_are_left_out_the_shortest_first() {
    // A junction 1 mm across with a branch 10 mm long and 2 bumps, 0.3 and 0.5 mm long, the one or the other
    // found first: the shorter is left out, and the junction, left with 2 branches, is a bend, whose longer
    // bump is the line's end. Of 2 bumps as long as each other, the one found first is left out.
    let pruned = |bumps: [(f64, f64); 2]| {
        let mut bumped = graph(&[((0.0, 0.0), Some(1.0)), ((10.0, 0.0), None), (bumps[0], None), (bumps[1], None)], &[(0, 1), (0, 2), (0, 3)]);
        bumped.prune(&mut meter()).unwrap();
        bumped.links[0].clone()
    };
    assert_eq!(pruned([(0.0, 0.3), (0.0, -0.5)]), [1, 3]);
    assert_eq!(pruned([(0.0, 0.5), (0.0, -0.3)]), [1, 2]);
    assert_eq!(pruned([(0.0, 0.4), (0.0, -0.4)]), [1, 3]);
    // A bump reaches no further than twice the junction's longest edge: at a junction 0.5 mm across, a branch
    // 1.5 mm long is no bump.
    let mut long = graph(&[((0.0, 0.0), Some(0.5)), ((10.0, 0.0), None), ((-10.0, 0.0), None), ((0.0, 1.5), None)], &[(0, 1), (0, 2), (0, 3)]);
    long.prune(&mut meter()).unwrap();
    assert_eq!(long.links[0], [1, 2, 3]);
}

#[test]
fn req_auto_014_a_junctions_branches_are_paired_by_their_directions_alone() {
    // A junction with branches to (5, 0) and (-4, 1.5), and a third, 0.3 mm long, to a second junction, as
    // where 2 bars cross. Measured to where it ends, at the second junction, the short branch runs on from
    // the first more nearly straight than the second does, short as it is, and is the first's pair.
    let crossing = graph(
        &[((0.0, 0.0), Some(1.0)), ((5.0, 0.0), None), ((-4.0, 1.5), None), ((-0.3, 0.04), Some(1.0)), ((-0.3, 5.0), None), ((-0.3, -5.0), None)],
        &[(0, 1), (0, 2), (0, 3), (3, 4), (3, 5)],
    );
    assert_eq!(crossing.pairs(&mut meter()).unwrap().get(&0), Some(&(1, 3)));
    // Junctions off the origin with 3 branches in random directions, 1.5 to 5 mm long: the pair is the 2 whose
    // directions lie the widest angle apart.
    let mut rng = SplitMix64::new(20_261_028);
    for _ in 0..500 {
        let centre = (rng.next_in(-20.0, 20.0), rng.next_in(-20.0, 20.0));
        let turns: [f64; 3] = std::array::from_fn(|_| rng.next_in(0.0, std::f64::consts::TAU));
        let ends = turns.map(|turn| {
            let ((sin, cos), r) = (sin_cos(turn), rng.next_in(1.5, 5.0));
            ((centre.0 + r * cos, centre.1 + r * sin), None)
        });
        let junction = graph(&[(centre, Some(1.0)), ends[0], ends[1], ends[2]], &[(0, 1), (0, 2), (0, 3)]);
        let apart = |i: usize, j: usize| {
            let turn = (turns[j] - turns[i]).rem_euclid(std::f64::consts::TAU);
            turn.min(std::f64::consts::TAU - turn)
        };
        let widest = [(1, 2), (1, 3), (2, 3)].into_iter().max_by(|a, b| apart(a.0 - 1, a.1 - 1).total_cmp(&apart(b.0 - 1, b.1 - 1))).unwrap();
        assert_eq!(junction.pairs(&mut meter()).unwrap().get(&0), Some(&widest), "{turns:?}");
    }
    // Branches that all run the same way pair the first 2; of 2 pairs as wide apart, the first found is the
    // pair; and a branch of no length, which runs no way, pairs only where nothing else does.
    let same_way = graph(&[((0.0, 0.0), Some(1.0)), ((2.0, 0.0), None), ((3.0, 0.0), None), ((4.0, 0.0), None)], &[(0, 1), (0, 2), (0, 3)]);
    assert_eq!(same_way.pairs(&mut meter()).unwrap().get(&0), Some(&(1, 2)));
    let fork = graph(&[((0.0, 0.0), Some(1.0)), ((2.0, 0.0), None), ((-2.0, 2.0), None), ((-2.0, -2.0), None)], &[(0, 1), (0, 2), (0, 3)]);
    assert_eq!(fork.pairs(&mut meter()).unwrap().get(&0), Some(&(1, 2)));
    let still = graph(&[((0.0, 0.0), Some(1.0)), ((0.0, 0.0), None), ((5.0, 0.0), None), ((-5.0, 0.1), None)], &[(0, 1), (0, 2), (0, 3)]);
    assert_eq!(still.pairs(&mut meter()).unwrap().get(&0), Some(&(2, 3)));
}

#[test]
fn req_auto_014_a_junctions_third_branch_is_a_line_of_its_own() {
    // An H: 2 bars, each 10 mm long, with a junction at its middle, and a crossbar between the junctions. Each
    // junction pairs its bar's halves, so a line runs along each bar, and the crossbar, which no end leads to,
    // is a third line, from the first junction's third branch: one line, though its middle point comes first.
    let h = graph(
        &[
            ((5.0, 5.0), None),
            ((0.0, 5.0), Some(1.0)),
            ((10.0, 5.0), Some(1.0)),
            ((0.0, 0.0), None),
            ((0.0, 10.0), None),
            ((10.0, 0.0), None),
            ((10.0, 10.0), None),
        ],
        &[(1, 3), (1, 4), (1, 0), (2, 5), (2, 6), (2, 0)],
    );
    let pairs = h.pairs(&mut meter()).unwrap();
    let points = |line: &Line| line.points.iter().map(|q| (q.x(), q.y())).collect::<Vec<_>>();
    let found: Vec<Vec<(f64, f64)>> = h.lines(&pairs, &mut meter()).unwrap().iter().map(points).collect();
    assert_eq!(
        found,
        [vec![(0.0, 0.0), (0.0, 5.0), (0.0, 10.0)], vec![(10.0, 0.0), (10.0, 5.0), (10.0, 10.0)], vec![(0.0, 5.0), (5.0, 5.0), (10.0, 5.0)]]
    );
}

#[test]
fn req_auto_014_a_figure_8_sews_its_waist_once() {
    // A figure 8: 2 junctions 2 mm apart at its waist, a loop above them and a loop below. At the left
    // junction the waist runs on into the lower loop, and at the right one into the upper loop, so each
    // junction's third branch starts a line. The first runs round the upper loop, on through the waist and
    // back to where it started: it closes. The second runs round the lower loop and ends where it comes back
    // to the waist, which the first has sewn.
    let eight = graph(
        &[
            ((-1.0, 0.0), Some(1.0)),
            ((1.0, 0.0), Some(1.0)),
            ((-2.0, 2.0), None),
            ((0.0, 3.0), None),
            ((3.0, 0.0), None),
            ((-3.0, 0.0), None),
            ((0.0, -3.0), None),
            ((2.0, -2.0), None),
        ],
        &[(0, 1), (0, 2), (2, 3), (3, 4), (4, 1), (0, 5), (5, 6), (6, 7), (7, 1)],
    );
    let pairs = eight.pairs(&mut meter()).unwrap();
    assert_eq!(pairs, BTreeMap::from([(0, (1, 5)), (1, (0, 4))]));
    let found: Vec<(Vec<(f64, f64)>, bool)> =
        eight.lines(&pairs, &mut meter()).unwrap().iter().map(|line| (line.points.iter().map(|q| (q.x(), q.y())).collect(), line.closed)).collect();
    assert_eq!(
        found,
        [
            (vec![(-1.0, 0.0), (-2.0, 2.0), (0.0, 3.0), (3.0, 0.0), (1.0, 0.0)], true),
            (vec![(1.0, 0.0), (2.0, -2.0), (0.0, -3.0), (-3.0, 0.0), (-1.0, 0.0)], false)
        ]
    );
}

#[test]
fn req_auto_014_a_tip_is_reached() {
    // A blade 0.8 mm wide narrowing to a point 10 mm on: the line runs on to the point.
    let blade = part(&[(0.0, 0.0), (5.0, 0.0), (15.0, 0.4), (5.0, 0.8), (0.0, 0.8)], &[]);
    let found = lines(&blade, 0.4);
    assert_eq!(found.len(), 1, "{found:?}");
    // Its points lie on the region's grid, a few hundredths of a nanometre from where they are written.
    let ends = [found[0].points[0], *found[0].points.last().unwrap()];
    assert!(ends.iter().any(|q| math::hypot(q.x() - 15.0, q.y() - 0.4) < 1e-9), "{found:?}");
}

#[test]
fn req_auto_014_ends_are_found_whatever_order_the_triangles_come_in() {
    // 2 triangles either side of the chord from (0, 0) to (1, 0). The circle on the chord holds the corner
    // (0.5, -0.3), a round end whose line ends at the chord's middle, and not the corner (0.5, 3), a tip,
    // whose line runs on to it. The line is the same whichever end comes first.
    let points = vec![p(0.0, 0.0), p(0.5, -0.3), p(1.0, 0.0), p(0.5, 3.0)];
    let round = |other| Triangle { corners: [0, 1, 2], across: [None, Some(other), None] };
    let tip = |other| Triangle { corners: [2, 3, 0], across: [None, Some(other), None] };
    let line = Line { points: vec![p(0.5, 0.0), p(0.5, 3.0)], closed: false };
    for triangles in [vec![round(1), tip(0)], vec![tip(1), round(0)]] {
        let triangulation = Triangulation { points: points.clone(), triangles };
        assert_eq!(through(&triangulation, &mut meter()).unwrap(), vec![line.clone()], "{triangulation:?}");
    }
}

#[test]
fn req_auto_014_a_round_end_stops_the_line_within_half_its_width() {
    // Strips 0.8 mm wide and 10 mm long whose ends are half circles, wobbling by up to a micrometre or not,
    // or half ellipses 0.6 and 1 mm long. Points on a circle pass the Delaunay test cut into triangles in
    // many ways, the tie-break choosing one, and many of those triangles are junctions, of which caps reach 2
    // sides. The line runs along the middle and stops where the circles on the chords no longer hold the end,
    // 0.2 to 0.5 mm from it.
    let mut rng = SplitMix64::new(20_261_027);
    for (wobble, long) in [(0.0, 0.4), (1e-3, 0.4), (0.0, 0.5), (0.0, 0.3)] {
        for n in [8_u32, 12, 24, 48] {
            let mut outline = Vec::new();
            for (centre, from) in [(9.6, -90.0), (0.4, 90.0)] {
                for i in 0..=n {
                    let (sin, cos) = sin_cos(math::to_radians(from + 180.0 * f64::from(i) / f64::from(n)));
                    let r = if i == 0 || i == n { 0.4 } else { 0.4 + rng.next_in(-wobble, wobble) };
                    outline.push((centre + long / 0.4 * r * cos, 0.4 + r * sin));
                }
            }
            let found = lines(&part(&outline, &[]), 0.4);
            let case = format!("wobble {wobble}, {long} mm long, {n} points: {found:?}");
            assert_eq!(found.len(), 1, "{case}");
            let xs: Vec<f64> = found[0].points.iter().map(|q| q.x()).collect();
            let (low, high) = (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max));
            let tips = (0.4 - long, 9.6 + long);
            assert!((0.2..=0.5).contains(&(low - tips.0)) && (0.2..=0.5).contains(&(tips.1 - high)), "{low} {high}, {case}");
            assert!(found[0].points.iter().all(|q| (q.y() - 0.4).abs() < 0.15), "{case}");
        }
    }
}

#[test]
fn req_auto_014_a_crossing_runs_on_through_its_junctions() {
    // A plus of 2 bars 0.6 mm wide and 20 mm long crossing at their middles: the square where they cross is
    // 2 junction triangles, a short branch apart. Every arm is sewn to its end, and the lines together are
    // about as long as the 4 arms.
    let (a, b) = (9.7, 10.3);
    let plus = part(&[(0.0, a), (a, a), (a, 0.0), (b, 0.0), (b, a), (20.0, a), (20.0, b), (b, b), (b, 20.0), (a, 20.0), (a, b), (0.0, b)], &[]);
    let found = lines(&plus, 0.6);
    let ends: Vec<Point> = found.iter().flat_map(|line| [line.points[0], *line.points.last().unwrap()]).collect();
    for tip in [p(0.0, 10.0), p(20.0, 10.0), p(10.0, 0.0), p(10.0, 20.0)] {
        assert!(ends.iter().any(|q| q.distance(tip) < 0.6), "{tip:?} {found:?}");
    }
    let length: f64 = found.iter().map(Line::length).sum();
    assert!((38.0..41.0).contains(&length), "{length} {found:?}");
}

#[test]
fn req_auto_014_a_dot_has_no_line() {
    // A square 1 mm across in 2 triangles: the circle on the diagonal holds every corner.
    let dot = part(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)], &[]);
    assert_eq!(lines(&dot, 10.0), Vec::new());
    // A lone triangle has no chord, and no line.
    let lone = part(&[(0.0, 0.0), (1.0, 0.0), (0.5, 0.8)], &[]);
    assert_eq!(lines(&lone, 10.0), Vec::new());
}

#[test]
fn lines_sew_the_longest_first_then_the_nearest() {
    let line = |points: &[(f64, f64)], closed: bool| Line { points: points.iter().map(|&(x, y)| p(x, y)).collect(), closed };
    let long = line(&[(0.0, 0.0), (10.0, 0.0)], false);
    let near_end = line(&[(10.0, 5.0), (10.0, 1.0)], false);
    let ring = line(&[(0.0, 1.0), (2.0, 1.0), (2.0, 3.0), (0.0, 3.0)], true);
    let sewn = ordered(vec![ring, near_end, long.clone()], &mut meter()).unwrap();
    assert_eq!(sewn[0], long);
    // From (10, 0), the short line's end at (10, 1) is nearest: it is turned round.
    assert_eq!(sewn[1], line(&[(10.0, 1.0), (10.0, 5.0)], false));
    // From (10, 5), the ring's nearest point is (2, 3): it starts there.
    assert_eq!(sewn[2], line(&[(2.0, 3.0), (0.0, 3.0), (0.0, 1.0), (2.0, 1.0)], true));
    // On a tie, the line found first: of 2 lines as long as each other, and of 2 ends as near.
    let (first, second) = (line(&[(0.0, 0.0), (1.0, 0.0)], false), line(&[(5.0, 0.0), (6.0, 0.0)], false));
    assert_eq!(ordered(vec![first.clone(), second.clone()], &mut meter()).unwrap(), [first, second]);
    let (beside, ahead) = (line(&[(10.0, 1.0), (10.0, 2.0)], false), line(&[(11.0, 0.0), (12.0, 0.0)], false));
    let sewn = ordered(vec![long.clone(), beside.clone(), ahead.clone()], &mut meter()).unwrap();
    assert_eq!(sewn, [long, beside, ahead]);
}

#[test]
fn the_work_is_charged() {
    let strip = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 0.6), (0.0, 0.6)], &[]);
    let mut small = Budget { max_stitches: 10, max_work: 500 }.meter();
    assert_eq!(centre_lines(&strip, 0.6, &mut small), Err(Exhausted::Work));
}
