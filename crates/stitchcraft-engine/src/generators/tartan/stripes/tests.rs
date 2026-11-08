//! The geometry's own cases: the box, the walk over a full sett, the rows' needle points, and the turns.

use stitchcraft_core::Budget;
use stitchcraft_core::math::sin_cos;

use super::*;
use crate::generators::tatami::fixture::{Question, costs, p};

const RED: Rgb = Rgb::new(255, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);

fn band(colour: Rgb, width: f64) -> Item {
    Item::Band { colour: Some(colour), width }
}

fn frame(min: f64, max: f64) -> Frame {
    Frame { min: [min, min], max: [max, max], middle: p((min + max) / 2.0, (min + max) / 2.0) }
}

#[test]
fn req_fill_trt_003_the_box_reaches_the_radius_of_the_smallest_circle_round_the_piece() {
    let meter = &mut Budget::DEFAULT.meter();
    // A square's circle is half its diagonal round its middle, its inner points and repeats aside; a
    // triangle's with all 3 corners on it; and 2 points' is half their distance.
    let square = [p(0.0, 0.0), p(40.0, 0.0), p(20.0, 20.0), p(40.0, 40.0), p(0.0, 40.0), p(0.0, 0.0)];
    assert!((enclosing_radius(&square, meter).unwrap() - 800.0_f64.sqrt()).abs() < 1e-9);
    let triangle = [p(0.0, 0.0), p(10.0, 0.0), p(5.0, 75.0_f64.sqrt())];
    assert!((enclosing_radius(&triangle, meter).unwrap() - 10.0 / 3.0_f64.sqrt()).abs() < 1e-9);
    // An obtuse triangle's circle is its longest side's, the third corner inside it.
    let obtuse = [p(0.0, 0.0), p(10.0, 0.0), p(5.0, 1.0)];
    assert!((enclosing_radius(&obtuse, meter).unwrap() - 5.0).abs() < 1e-9);
    assert_eq!(enclosing_radius(&[p(1.0, 1.0), p(1.0, 5.0)], meter).unwrap(), 2.0);
    assert_eq!(enclosing_radius(&[p(3.0, 4.0)], meter).unwrap(), 0.0);
    assert_eq!(enclosing_radius(&[], meter).unwrap(), 0.0);
    // Points round a circle of radius 3, whatever their order.
    let round: Vec<Point> = (0..37)
        .map(|k| {
            let (sin, cos) = sin_cos(f64::from(k * 97 % 360).to_radians());
            p(5.0 + 3.0 * cos, -2.0 + 3.0 * sin)
        })
        .collect();
    assert!((enclosing_radius(&round, meter).unwrap() - 3.0).abs() < 1e-9);
    // The work is charged: the hull's points, then the search, which takes them in the order the same
    // shuffle gives every time.
    assert!(enclosing_radius(&round, &mut Budget { max_work: 10, ..Budget::DEFAULT }.meter()).is_err());
    assert!(enclosing_radius(&round, &mut Budget { max_work: 40, ..Budget::DEFAULT }.meter()).is_err());
    let search: Question<'_> = Box::new(|meter| enclosing_radius(&round, meter).map(|_| ()));
    assert_eq!(costs(&[search]), [90]);
}

#[test]
fn a_circle_through_3_points_on_a_line_is_across_the_2_farthest_apart() {
    let circle = Circle::through(p(0.0, 0.0), p(2.0, 0.0), p(6.0, 0.0));
    assert_eq!(circle, Circle { middle: p(3.0, 0.0), radius: 3.0 });
    let circle = Circle::through(p(1.0, 1.0), p(1.0, 1.0), p(1.0, 1.0));
    assert_eq!(circle.radius, 0.0);
    // A point on the circle is in it, and one a millionth of a millimetre outside is not.
    let circle = Circle::through(p(0.0, 0.0), p(4.0, 0.0), p(2.0, 2.0));
    assert!(circle.holds(p(2.0, -2.0)) && !circle.holds(p(2.0, -2.000_001)));
}

#[test]
fn a_circle_through_3_points_has_its_middle_as_far_from_each() {
    let circle = Circle::through(p(0.0, 0.0), p(2.0, 0.0), p(0.0, 2.0));
    assert_eq!(circle.middle, p(1.0, 1.0));
    assert!((circle.radius - 2.0_f64.sqrt()).abs() < 1e-12, "{circle:?}");
    // A circle of no radius holds its middle.
    assert!(Circle { middle: p(1.0, 1.0), radius: 0.0 }.holds(p(1.0, 1.0)));
}

#[test]
fn a_convex_hull_keeps_the_corners_once() {
    let meter = &mut Budget::DEFAULT.meter();
    let points = [p(0.0, 0.0), p(2.0, 1.0), p(4.0, 0.0), p(4.0, 4.0), p(2.0, 2.0), p(0.0, 4.0), p(0.0, 0.0), p(2.0, 0.0)];
    let mut hull = convex_hull(&points, meter).unwrap();
    hull.sort_by(|a, b| a.x().total_cmp(&b.x()).then(a.y().total_cmp(&b.y())));
    assert_eq!(hull, [p(0.0, 0.0), p(0.0, 4.0), p(4.0, 0.0), p(4.0, 4.0)]);
    assert_eq!(convex_hull(&[p(1.0, 1.0), p(1.0, 1.0)], meter).unwrap(), [p(1.0, 1.0)]);
    // In order round the hull from its least point, as the 2 chains join: a triangle's 3 corners, and a
    // square's 4 without the points inside it and along its sides.
    assert_eq!(convex_hull(&[p(0.0, 4.0), p(4.0, 0.0), p(0.0, 0.0)], meter).unwrap(), [p(0.0, 0.0), p(4.0, 0.0), p(0.0, 4.0)]);
    let inner = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0), p(2.0, 2.0), p(2.0, 0.0), p(1.0, 3.0)];
    assert_eq!(convex_hull(&inner, meter).unwrap(), [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)]);
    // 3 points on a line are its 2 ends.
    assert_eq!(convex_hull(&[p(0.0, 0.0), p(1.0, 0.0), p(2.0, 0.0)], meter).unwrap(), [p(0.0, 0.0), p(2.0, 0.0)]);
    assert!(convex_hull(&points, &mut Budget { max_work: 7, ..Budget::DEFAULT }.meter()).is_err());
}

#[test]
fn bounds_are_the_least_and_greatest_coordinates_charged_by_the_point() {
    let meter = &mut Budget::DEFAULT.meter();
    assert_eq!(bounds(&[p(1.0, 5.0), p(-2.0, 3.0), p(4.0, -1.0)], meter).unwrap(), Some(([-2.0, -1.0], [4.0, 5.0])));
    assert_eq!(bounds(&[], meter).unwrap(), None);
    assert!(bounds(&[p(0.0, 0.0); 3], &mut Budget { max_work: 2, ..Budget::DEFAULT }.meter()).is_err());
    assert!(bounds(&[p(0.0, 0.0); 3], &mut Budget { max_work: 3, ..Budget::DEFAULT }.meter()).is_ok());
}

#[test]
fn req_fill_trt_003_a_full_sett_is_laid_in_turn_bands_of_a_colour_that_meet_made_one() {
    let meter = &mut Budget::DEFAULT.meter();
    // Red and blue 5 mm each from 0 to past 22: red at 0, 10 and 20, blue at 5 and 15.
    let walked = walk(&[band(RED, 5.0), band(BLUE, 5.0)], Direction::Warp, &frame(0.0, 22.0), meter).unwrap();
    assert_eq!(walked.bands, [(RED, vec![[0.0, 5.0], [10.0, 15.0], [20.0, 25.0]]), (BLUE, vec![[5.0, 10.0], [15.0, 20.0]])]);
    // The sett is laid until it passes the far side: a band that starts on it is laid too.
    let walked = walk(&[band(RED, 5.0), band(BLUE, 5.0)], Direction::Warp, &frame(0.0, 10.0), meter).unwrap();
    assert_eq!(walked.bands, [(RED, vec![[0.0, 5.0], [10.0, 15.0]]), (BLUE, vec![[5.0, 10.0]])]);
    // A run between 2 red bands leaves them one, and lies where they meet; an unsewn band parts them. Its
    // colour first comes as a run, so it comes first in the order of both.
    let items = [Item::Run(BLUE), band(RED, 2.0), Item::Run(BLUE), band(RED, 3.0), Item::Band { colour: None, width: 1.0 }, band(BLUE, 1.0)];
    let walked = walk(&items, Direction::Weft, &frame(0.0, 6.5), meter).unwrap();
    assert_eq!(walked.bands, [(BLUE, vec![[6.0, 7.0]]), (RED, vec![[0.0, 5.0]])]);
    assert_eq!(walked.runs, [(BLUE, vec![0.0, 2.0])]);
    // A sett without width lays nothing, and the work is charged for each step.
    assert!(walk(&[Item::Run(BLUE)], Direction::Warp, &frame(0.0, 10.0), meter).unwrap().bands.is_empty());
    assert!(walk(&[], Direction::Warp, &frame(0.0, 10.0), meter).unwrap().runs.is_empty());
    assert!(walk(&[band(RED, 0.001)], Direction::Warp, &frame(0.0, 10.0), &mut Budget { max_work: 100, ..Budget::DEFAULT }.meter()).is_err());
}

#[test]
fn req_fill_trt_004_needle_points_lie_the_longest_stitch_apart_staggered_with_the_row_s_ends() {
    let tartan = |staggers: f64| Tartan { staggers, ..super::super::tests::tartan(&[]) };
    let points = |k: usize, length: f64, tartan: &Tartan| needle_points(k, length, tartan, &mut Budget::DEFAULT.meter()).unwrap();
    // Rows 10 mm long, stitches 4 mm long, staggered by a quarter: the first row from its start, the next a
    // stitch's quarter on, and the row's ends always.
    assert_eq!(points(0, 10.0, &tartan(4.0)), [0.0, 4.0, 8.0, 10.0]);
    assert_eq!(points(1, 10.0, &tartan(4.0)), [0.0, 1.0, 5.0, 9.0, 10.0]);
    assert_eq!(points(6, 10.0, &tartan(4.0)), [0.0, 2.0, 6.0, 10.0]);
    // Staggers of 0 count as 1, and below 0 the other way round.
    assert_eq!(points(3, 10.0, &tartan(0.0)), [0.0, 4.0, 8.0, 10.0]);
    assert_eq!(points(1, 10.0, &tartan(-4.0)), [0.0, 3.0, 7.0, 10.0]);
    // A row with room for fewer than 2 needle points between its ends has its ends alone.
    assert_eq!(points(0, 3.0, &tartan(4.0)), [0.0, 3.0]);
    assert_eq!(points(3, 3.0, &tartan(4.0)), [0.0, 3.0]);
    // The meter is charged for the points before they are made: a row too long for it makes none.
    let meter = |work: u64| Budget { max_work: work, ..Budget::DEFAULT }.meter();
    assert!(needle_points(0, 1e9, &tartan(4.0), &mut meter(1_000)).is_err());
    assert_eq!(needle_points(0, 3992.0, &tartan(4.0), &mut meter(1_000)).unwrap().len(), 999);
    assert!(needle_points(0, 3996.0, &tartan(4.0), &mut meter(1_000)).is_err());
}

#[test]
fn a_count_of_steps_is_at_least_its_least_and_saturates() {
    assert_eq!((steps(3.0, 1), steps(0.0, 1), steps(-2.0, 0), steps(f64::NAN, 1)), (3, 1, 0, 1));
    assert_eq!((steps(f64::INFINITY, 0), steps(1e300, 0)), (usize::MAX, usize::MAX));
}

#[test]
fn a_turn_is_exact_for_quarter_turns_and_undone_by_its_back() {
    let quarter = Turn::new(p(1.0, 1.0), 90.0);
    assert_eq!(quarter.apply(p(2.0, 1.0)), p(1.0, 2.0));
    assert_eq!(quarter.back().apply(p(1.0, 2.0)), p(2.0, 1.0));
    assert_eq!(quarter.direction((1.0, 0.0)), (0.0, 1.0));
    let some = Turn::new(p(0.0, 0.0), 30.0);
    let q = some.back().apply(some.apply(p(3.0, -2.0)));
    assert!(q.distance(p(3.0, -2.0)) < 1e-12);
    // A direction turns as a point turns about the origin.
    let q = some.apply(p(1.0, 2.0));
    assert_eq!(some.direction((1.0, 2.0)), (q.x(), q.y()));
}

#[test]
fn a_join_is_cut_into_equal_stitches_no_longer_than_the_running_stitch() {
    let cut = |a: Point, b: Point, longest: f64| segmentized(a, b, longest, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(cut(p(0.0, 0.0), p(10.0, 0.0), 2.5), [p(0.0, 0.0), p(2.5, 0.0), p(5.0, 0.0), p(7.5, 0.0), p(10.0, 0.0)]);
    assert_eq!(cut(p(0.0, 0.0), p(1.0, 0.0), 2.5), [p(0.0, 0.0), p(1.0, 0.0)]);
    assert_eq!(cut(p(2.0, 2.0), p(2.0, 2.0), 2.5), [p(2.0, 2.0), p(2.0, 2.0)]);
    // The meter is charged for the points before they are made: one for each and one more.
    assert!(segmentized(p(0.0, 0.0), p(1e12, 0.0), 0.1, &mut Budget::DEFAULT.meter()).is_err());
    let meter = |work: u64| Budget { max_work: work, ..Budget::DEFAULT }.meter();
    assert!(segmentized(p(0.0, 0.0), p(10.0, 0.0), 2.5, &mut meter(5)).is_ok());
    assert!(segmentized(p(0.0, 0.0), p(10.0, 0.0), 2.5, &mut meter(4)).is_err());
}

/// A warp band's laying over `piece`, with [`super::super::tests::tartan`]'s settings: the running stitch
/// 2.5 mm and the longest stitch 4 mm, in a box from -10 to 20 mm both ways.
fn laying<'l>(piece: &'l Polygon, frame: &'l Frame, tartan: &'l Tartan) -> Laying<'l> {
    let reach = bounds(&piece.outline, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let rings = piece.rings().collect();
    Laying { frame, tartan, piece, reach, rings, direction: Direction::Warp, turn: Turn::new(frame.middle, 0.0) }
}

fn polygon(outline: &[Point]) -> Polygon {
    Polygon { outline: outline.to_vec(), holes: Vec::new() }
}

#[test]
fn req_fill_trt_005_a_line_ending_inside_the_piece_runs_on_along_the_band_s_edge_to_the_nearest_ring() {
    let (square, box_, tartan) = (square(), frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let laying = laying(&square, &box_, &tartan);
    let meter = &mut Budget::DEFAULT.meter();
    // A line that starts inside, 4 mm above the outline along the band's upright edge, runs on from its start
    // down to the outline in 2 stitches of 2 mm; its end on the outline stays.
    let mut line = vec![p(4.0, 4.0), p(4.0, 10.0)];
    laying.run_on(&mut line, meter).unwrap();
    assert_eq!(line, [p(4.0, 0.0), p(4.0, 2.0), p(4.0, 4.0), p(4.0, 10.0)]);
    // One that ends inside runs on from its end, up to the nearer side.
    let mut line = vec![p(6.0, 0.0), p(6.0, 7.5)];
    laying.run_on(&mut line, meter).unwrap();
    assert_eq!(line, [p(6.0, 0.0), p(6.0, 7.5), p(6.0, 10.0)]);
    // An end within 0.05 pixel of the outline, 0.0132 mm, stays as it is: 2⁻⁷ mm is within it, and 2⁻⁶ mm
    // isn't.
    let mut line = vec![p(4.0, 0.007_812_5), p(4.0, 10.0)];
    laying.run_on(&mut line, meter).unwrap();
    assert_eq!(line, [p(4.0, 0.007_812_5), p(4.0, 10.0)]);
    let mut line = vec![p(4.0, 0.015_625), p(4.0, 10.0)];
    laying.run_on(&mut line, meter).unwrap();
    assert_eq!(line, [p(4.0, 0.0), p(4.0, 0.015_625), p(4.0, 10.0)]);
}

#[test]
fn req_fill_trt_007_a_run_s_pieces_of_half_a_millimetre_or_less_are_left_out() {
    // Under the diagonal of a triangle, a warp run at x = 0.4 crosses it for 0.4 mm and is left out, and one
    // at x = 5 for 5 mm, in 2 stitches no longer than the running stitch, from the bottom of the box up.
    let triangle = polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 0.0)]);
    let (box_, tartan) = (frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let laying = laying(&triangle, &box_, &tartan);
    let meter = &mut Budget::DEFAULT.meter();
    assert_eq!(laying.run(0.4, meter).unwrap(), Vec::<Vec<Point>>::new());
    assert_eq!(laying.run(5.0, meter).unwrap(), [vec![p(5.0, 0.0), p(5.0, 2.5), p(5.0, 5.0)]]);
}

/// The square from 0 to 10 mm both ways.
fn square() -> Polygon {
    polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)])
}

/// The triangle under the line from (10, 0) to (0, 10).
fn triangle() -> Polygon {
    polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(0.0, 10.0), p(0.0, 0.0)])
}

/// Level rows across a box from -10 to 20 mm, from its left side, one at each of `ys`, with needle points
/// `step` mm apart and at their ends, each other one run back: all the warp's.
fn level(ys: &[f64], step: f64) -> Rows {
    let points: Vec<f64> = (0..).map(|j| f64::from(j) * step).take_while(|&t| t < 30.0).chain([30.0]).collect();
    Rows {
        starts: ys.iter().map(|&y| p(-10.0, y)).collect(),
        along: (1.0, 0.0),
        length: 30.0,
        points: vec![points; ys.len()],
        taken: [(0..ys.len()).map(|k| (k, k % 2 == 1)).collect(), Vec::new()],
    }
}

#[test]
fn req_fill_trt_003_the_margin_is_the_sett_s_width_or_2_rows_per_thread_of_rows_whichever_is_more() {
    // The square's smallest circle has a radius of √50 round its middle, (5, 5), and the box reaches a margin
    // further: 2 × 4 rows 0.25 mm apart are 2 mm, more than a sett 1 mm wide and less than one 3 mm wide.
    let square = square();
    let radius = 50.0_f64.sqrt();
    let reach = |margin: f64| {
        let tartan = Tartan { margin, spacing: 0.25, rows_per_thread: 4, ..super::super::tests::tartan(&[]) };
        let frame = Frame::of(&square, &tartan, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert_eq!(frame.middle, p(5.0, 5.0));
        assert!((frame.max[0] - frame.max[1]).abs() + (frame.min[0] - frame.min[1]).abs() < 1e-12, "{frame:?}");
        (frame.max[0] - 5.0 - radius, 5.0 - radius - frame.min[0])
    };
    let near = |(a, b): (f64, f64), margin: f64| (a - margin).abs() < 1e-9 && (b - margin).abs() < 1e-9;
    assert!(near(reach(1.0), 2.0), "{:?}", reach(1.0));
    assert!(near(reach(3.0), 3.0), "{:?}", reach(3.0));
}

#[test]
fn req_fill_trt_005_a_row_gives_its_stretch_inside_the_band_but_one_of_half_a_pixel_or_less() {
    let (piece, box_, tartan) = (square(), frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let laying = laying(&piece, &box_, &tartan);
    let rows = level(&[5.0], 2.0);
    let stretch = |band: [f64; 2]| laying.stretch(band, &rows.row(0).unwrap(), false);
    // From one edge of the band to the other, with the row's needle points between them but those on them.
    assert_eq!(stretch([4.0, 8.0]), Some(vec![p(4.0, 5.0), p(6.0, 5.0), p(8.0, 5.0)]));
    assert_eq!(stretch([3.0, 9.0]), Some(vec![p(3.0, 5.0), p(4.0, 5.0), p(6.0, 5.0), p(8.0, 5.0), p(9.0, 5.0)]));
    assert_eq!(laying.stretch([4.0, 8.0], &rows.row(0).unwrap(), true), Some(vec![p(8.0, 5.0), p(6.0, 5.0), p(4.0, 5.0)]));
    // Half a pixel is 0.1323 mm: a stretch of 2⁻³ mm is left out, and one of 9 × 2⁻⁶ mm kept.
    assert_eq!(stretch([5.0, 5.125]), None);
    assert_eq!(stretch([5.0, 5.140_625]), Some(vec![p(5.0, 5.0), p(5.140_625, 5.0)]));
    // A level row crosses the box only between its top and bottom sides, or on them.
    let rows = level(&[-10.5, -10.0, 5.0, 20.0, 20.5], 30.0);
    let crossing: Vec<bool> = (0..5).map(|k| laying.stretch([4.0, 8.0], &rows.row(k).unwrap(), false).is_some()).collect();
    assert_eq!(crossing, [false, true, true, true, false]);
}

#[test]
fn req_fill_trt_005_the_stretches_kept_are_those_near_the_piece_and_one_more_either_side() {
    let (piece, box_, tartan) = (square(), frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let laying = laying(&piece, &box_, &tartan);
    let meter = &mut Budget::DEFAULT.meter();
    // Stretches level with the square but past its right side don't come near it: of 5, the middle one does,
    // and one more either side is kept. Without one that comes near it, none.
    let (beside, over) = (vec![p(12.0, 5.0), p(14.0, 5.0)], vec![p(4.0, 5.0), p(6.0, 5.0)]);
    let stretches = [beside.clone(), beside.clone(), over, beside.clone(), beside.clone()];
    assert_eq!(laying.near(&stretches, meter).unwrap(), (1, 3));
    assert_eq!(laying.near(&[beside.clone(), beside], meter).unwrap(), (1, 0));
}

#[test]
fn req_fill_trt_005_a_band_s_stretches_are_joined_along_its_edge_but_at_the_rings() {
    let (box_, tartan) = (frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let meter = &mut Budget::DEFAULT.meter();
    // In the square, rows 3 mm apart cross a band from 3 to 6 mm, every other one back: one zigzag, its joins
    // 2 stitches of 1.5 mm each, run on from its ends to the outline.
    let square = square();
    let zigzag = laying(&square, &box_, &tartan).band([3.0, 6.0], &level(&[2.0, 5.0, 8.0], 30.0).taken(Direction::Warp), true, meter).unwrap();
    let joined = [p(3.0, 0.0), p(3.0, 2.0), p(6.0, 2.0), p(6.0, 3.5), p(6.0, 5.0), p(3.0, 5.0), p(3.0, 6.5), p(3.0, 8.0), p(6.0, 8.0), p(6.0, 10.0)];
    assert_eq!(zigzag, [joined]);
    // In the triangle, the second row's stretch starts on its long side, and so does the third's: each starts
    // a line of its own, and the third's, outside, is cut away.
    let triangle = triangle();
    let lines = laying(&triangle, &box_, &tartan).band([3.0, 6.0], &level(&[1.0, 4.0, 7.0], 30.0).taken(Direction::Warp), true, meter).unwrap();
    assert_eq!(lines, [vec![p(3.0, 0.0), p(3.0, 1.0), p(6.0, 1.0), p(6.0, 0.0)], vec![p(6.0, 4.0), p(3.0, 4.0), p(3.0, 5.5), p(3.0, 7.0)]]);
}

#[test]
fn req_fill_trt_005_a_stretch_s_end_within_0_005_pixel_of_the_rings_lies_on_them() {
    let (piece, box_, tartan) = (square(), frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let laying = laying(&piece, &box_, &tartan);
    let meter = &mut Budget::DEFAULT.meter();
    // 0.005 pixel is 0.0013 mm: 2⁻¹⁰ mm is within it, and 2⁻⁹ mm isn't.
    assert!(laying.on_rings(p(4.0, 0.000_976_562_5), meter).unwrap());
    assert!(!laying.on_rings(p(4.0, 0.001_953_125), meter).unwrap());
}

#[test]
fn req_fill_trt_005_a_line_runs_on_to_the_nearest_crossing_of_its_band_s_edge_with_a_ring() {
    let (box_, tartan) = (frame(-10.0, 20.0), super::super::tests::tartan(&[]));
    let meter = &mut Budget::DEFAULT.meter();
    // From (4, 5) in the triangle, a warp band's upright edge meets the long side 1 mm up, nearer than the
    // bottom 5 mm down; a weft band's level edge meets it 1 mm on, nearer than the left side 4 mm back.
    let triangle = triangle();
    let warp = laying(&triangle, &box_, &tartan);
    let weft = Laying { direction: Direction::Weft, ..laying(&triangle, &box_, &tartan) };
    assert_eq!(warp.crossing(p(4.0, 5.0), meter).unwrap(), Some(p(4.0, 6.0)));
    assert_eq!(weft.crossing(p(4.0, 5.0), meter).unwrap(), Some(p(5.0, 5.0)));
    // Of 2 as near, the one on the side the rings list first.
    let square = square();
    assert_eq!(laying(&square, &box_, &tartan).crossing(p(5.0, 5.0), meter).unwrap(), Some(p(5.0, 0.0)));
    // A side whose line crosses the edge, but not the side itself, doesn't count: with the top right corner
    // pulled out to (10, 8), the line of the side from it to (6, 6) meets x = 4 at (4, 5), off that side.
    let pulled = polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 8.0), p(6.0, 6.0), p(0.0, 6.0), p(0.0, 0.0)]);
    assert_eq!(laying(&pulled, &box_, &tartan).crossing(p(4.0, 5.25), meter).unwrap(), Some(p(4.0, 6.0)));
}
