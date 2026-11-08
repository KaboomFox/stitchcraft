//! The geometry's own cases: the box, the walk over a full sett, the rows' needle points, and the turns.

use stitchcraft_core::Budget;
use stitchcraft_core::math::sin_cos;

use super::*;
use crate::generators::tatami::fixture::p;

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
    // The work is charged.
    assert!(enclosing_radius(&round, &mut Budget { max_work: 10, ..Budget::DEFAULT }.meter()).is_err());
    assert!(enclosing_radius(&round, &mut Budget { max_work: 40, ..Budget::DEFAULT }.meter()).is_err());
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
fn a_convex_hull_keeps_the_corners_once() {
    let meter = &mut Budget::DEFAULT.meter();
    let points = [p(0.0, 0.0), p(2.0, 1.0), p(4.0, 0.0), p(4.0, 4.0), p(2.0, 2.0), p(0.0, 4.0), p(0.0, 0.0), p(2.0, 0.0)];
    let mut hull = convex_hull(&points, meter).unwrap();
    hull.sort_by(|a, b| a.x().total_cmp(&b.x()).then(a.y().total_cmp(&b.y())));
    assert_eq!(hull, [p(0.0, 0.0), p(0.0, 4.0), p(4.0, 0.0), p(4.0, 4.0)]);
    assert_eq!(convex_hull(&[p(1.0, 1.0), p(1.0, 1.0)], meter).unwrap(), [p(1.0, 1.0)]);
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
    let square = polygon(&[p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)]);
    let (box_, tartan) = (frame(-10.0, 20.0), super::super::tests::tartan(&[]));
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
