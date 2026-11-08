//! The herringbone's own cases: its bands and rows, the warp band by band, and the weft joined across the
//! bands.

use stitchcraft_core::Budget;

use super::*;
use crate::generators::tartan::Sett;
use crate::generators::tartan::sett::Item;
use crate::generators::tartan::tests::tartan;
use crate::generators::tatami::fixture::{Question, costs, p};
use crate::normalize::region::Polygon;

const RED: Rgb = Rgb::new(255, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);
const GREEN: Rgb = Rgb::new(0, 255, 0);

/// A box from `min` to `max`, turning about its middle.
fn frame(min: [f64; 2], max: [f64; 2]) -> Frame {
    Frame { min, max, middle: p((min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0) }
}

/// A herringbone `width` mm wide, its rows 0.5 mm apart, over a box 10 mm wide and 8 mm high from the origin.
fn herringbone(width: f64) -> Herringbone {
    let tartan = Tartan { herringbone: width, spacing: 0.5, ..tartan(&[]) };
    Herringbone::new(&frame([0.0, 0.0], [10.0, 8.0]), &tartan, &mut Budget::DEFAULT.meter()).unwrap()
}

/// The square from `a` to `b` both ways.
fn square(a: f64, b: f64) -> Polygon {
    Polygon { outline: vec![p(a, a), p(b, a), p(b, b), p(a, b), p(a, a)], holes: Vec::new() }
}

/// A laying of `direction`'s stripes over `piece`, whose bounds are `reach`, in a box from -10 to 20 mm both
/// ways, not turned.
fn laying<'l>(piece: &'l Polygon, reach: Bounds, frame: &'l Frame, tartan: &'l Tartan, direction: Direction) -> Laying<'l> {
    Laying { frame, tartan, piece, reach, rings: piece.rings().collect(), direction, turn: Turn::new(frame.middle, 0.0) }
}

/// `points` to a nanometre, for points that rounding moves a hair.
fn rounded(points: &[Point]) -> Vec<(f64, f64)> {
    points.iter().map(|q| ((q.x() * 1e9).round() / 1e9, (q.y() * 1e9).round() / 1e9)).collect()
}

#[test]
fn req_fill_trt_012_bands_stand_side_by_side_until_one_starts_past_the_box_s_right_side() {
    let tartan = Tartan { herringbone: 2.0, spacing: 0.5, ..tartan(&[]) };
    let meter = &mut Budget::DEFAULT.meter();
    // The gap between bands is 0.005 pixel.
    assert!((GAP - 0.001_322_916_666_666_666_7).abs() < 1e-15);
    let pitch = 2.0 + GAP;
    // A box 4 pitches wide: the fifth band would start on its right side, and isn't laid.
    let exact = Herringbone::new(&frame([0.0, 0.0], [4.0 * pitch, 8.0]), &tartan, meter).unwrap();
    assert_eq!(exact.bands, 4);
    assert_eq!(Herringbone::new(&frame([0.0, 0.0], [4.0 * pitch + 1e-9, 8.0]), &tartan, meter).unwrap().bands, 5);
    assert_eq!((exact.left(0), exact.left(3)), (0.0, 3.0 * pitch));
    // Each band's sides are as long as the box is high plus the width, 10 mm, in 20 steps of 0.5 mm.
    assert_eq!((exact.count, exact.side(1), exact.side(20)), (20, 0.5, 10.0));
    assert_eq!(exact.points.len(), 21);
    // Every band's rows are charged as if made, 5 bands of 21 rows, and the needle points of 21 rows, 3 each, or 2
    // where the staggers start a row too near its end for a point between its ends.
    let laid: Question<'_> = Box::new(|meter| Herringbone::new(&frame([0.0, 0.0], [10.0, 8.0]), &tartan, meter).map(|_| ()));
    assert_eq!(costs(&[laid]), [21 * 5 + 16 * 3 + 5 * 2]);
}

#[test]
fn req_fill_trt_012_the_first_band_s_rows_run_down_to_the_right_and_the_next_band_s_up() {
    let herringbone = herringbone(2.0);
    let ends = |j: usize, k: usize| {
        let row = herringbone.row(j, k).unwrap();
        (row.start, row.at(row.length).unwrap(), row.length)
    };
    // Row 3 of the first band: from a width above the box's top, 1.5 mm down its left side, to 1.5 mm down its
    // right side, 2√2 long.
    let (start, end, length) = ends(0, 3);
    assert_eq!((start, length), (p(0.0, -0.5), 2.0 * SQRT_2));
    assert!(end.distance(p(2.0, 1.5)) < 1e-12, "{end:?}");
    // Row 3 of the second band: from 1.5 mm down its left side up to a width above it on its right side.
    let (start, end, _) = ends(1, 3);
    assert_eq!(start, p(2.0 + GAP, 1.5));
    assert!(end.distance(p(4.0 + GAP, -0.5)) < 1e-12, "{end:?}");
    // The third band runs down again; the sides end at step 20.
    assert_eq!(herringbone.row(2, 0).unwrap().along, (FRAC_1_SQRT_2, FRAC_1_SQRT_2));
    assert_eq!(herringbone.row(1, 0).unwrap().along, (FRAC_1_SQRT_2, -FRAC_1_SQRT_2));
    assert!(herringbone.row(0, 20).is_some() && herringbone.row(0, 21).is_none());
}

#[test]
fn req_fill_trt_012_the_first_rows_of_a_turn_go_to_the_weft_in_the_first_band_and_to_the_warp_in_the_next() {
    let herringbone = herringbone(2.0);
    // 2 rows per thread: rows 0, 1, 4, 5 and so on, and every other one back, then rows 2, 3, 6, 7 and so on.
    let first = herringbone.group(0, Direction::Weft);
    assert_eq!(first.get(..5).unwrap(), [(0, false), (1, true), (4, false), (5, true), (8, false)]);
    assert_eq!(herringbone.group(0, Direction::Warp).get(..3).unwrap(), [(2, false), (3, true), (6, false)]);
    assert_eq!(herringbone.group(1, Direction::Warp), first);
    assert_eq!(herringbone.group(1, Direction::Weft), herringbone.group(0, Direction::Warp));
    assert_eq!(herringbone.group(2, Direction::Weft), first);
    // The rows taken are the band's.
    let taken = herringbone.taken(1, Direction::Warp);
    assert_eq!((taken.len(), taken.first().map(|(row, back)| (row.start, *back))), (first.len(), Some((p(2.0 + GAP, 0.0), false))));
}

#[test]
fn req_fill_trt_012_a_band_s_rows_are_taken_from_the_end_of_its_turned_side_sorted_first() {
    // Upright, or turned so that x grows along the side, from step 0; turned the other way, from the last.
    let order = |side: (f64, f64)| in_turns(3, 1, side).map(|list| list.into_iter().map(|(k, _)| k).collect::<Vec<_>>());
    assert_eq!(order((0.0, 1.0)), [vec![0, 2], vec![1, 3]]);
    assert_eq!(order((0.5, -0.8)), [vec![0, 2], vec![1, 3]]);
    assert_eq!(order((-0.5, 0.8)), [vec![2, 0], vec![3, 1]]);
    assert_eq!(order((0.0, -1.0)), [vec![2, 0], vec![3, 1]]);
    // Turned with the sett, as a herringbone's are: by 30°, x falls along the side.
    let tartan = Tartan { herringbone: 2.0, spacing: 0.5, ..tartan(&[]) };
    let turned = Tartan { sett: Sett { rotate: 30.0, ..tartan.sett.clone() }, ..tartan };
    let herringbone = Herringbone::new(&frame([0.0, 0.0], [10.0, 8.0]), &turned, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(herringbone.group(0, Direction::Weft).first(), Some(&(20, false)));
}

#[test]
fn req_fill_trt_014_a_band_s_weft_rows_near_a_stripe_are_joined_end_to_start() {
    // The first band's weft rows that, with the join after them, reach between 3 and 4 mm down: rows 5, 8, 9, 12
    // and 13, each its 2 ends, as stitches 4 mm long don't fit a row 2√2 long, joined in one stitch each.
    let herringbone = herringbone(2.0);
    let meter = &mut Budget::DEFAULT.meter();
    let zigzag = herringbone.zigzag(0, [3.0, 4.0], 2.5, meter).unwrap();
    let expected = [(2.0, 2.5), (0.0, 0.5), (0.0, 2.0), (2.0, 4.0), (2.0, 4.5), (0.0, 2.5), (0.0, 4.0), (2.0, 6.0), (2.0, 6.5), (0.0, 4.5)];
    assert_eq!(rounded(&zigzag), expected);
    // A join longer than the running stitch is cut into equal stitches: rows 1 and 4 lie 1.5 mm apart.
    let zigzag = herringbone.zigzag(0, [0.0, 0.5], 1.0, meter).unwrap();
    assert_eq!(rounded(zigzag.get(..5).unwrap()), [(0.0, -2.0), (2.0, 0.0), (2.0, 0.5), (0.0, -1.5), (0.0, -0.75)]);
    // A stripe below every row reaches none.
    assert!(herringbone.zigzag(0, [10.5, 12.0], 2.5, meter).unwrap().is_empty());
}

#[test]
fn req_fill_trt_014_a_zigzag_s_pieces_inside_a_stripe_are_those_longer_than_half_a_pixel() {
    let box_ = frame([-10.0, -10.0], [20.0, 20.0]);
    let meter = &mut Budget::DEFAULT.meter();
    let cut = |line: &[Point], stripe: [f64; 2]| clipped(line, stripe, &box_, &mut Budget::DEFAULT.meter()).unwrap();
    // A tent through a stripe from 1 to 3 mm down: its 2 sides' stretches inside, each from where it enters.
    let tent = [p(0.0, 0.0), p(4.0, 4.0), p(8.0, 0.0)];
    assert_eq!(rounded(&cut(&tent, [1.0, 3.0]).concat()), [(1.0, 1.0), (3.0, 3.0), (5.0, 3.0), (7.0, 1.0)]);
    // Points inside run on in one piece; a line that only touches the stripe gives none.
    let inside = [p(0.0, 1.5), p(1.0, 2.0), p(2.0, 1.5), p(3.0, 5.0)];
    assert_eq!(cut(&inside, [1.0, 3.0]).len(), 1);
    assert_eq!(cut(&inside, [1.0, 3.0]).first().map(Vec::len), Some(4));
    assert!(cut(&[p(0.0, 0.0), p(2.0, 1.0), p(4.0, 0.0)], [1.0, 3.0]).is_empty());
    // Half a pixel is left out, and a hair more kept.
    assert!(cut(&[p(0.0, 0.0), p(0.0, SHORTEST_STRETCH)], [-1.0, 1.0]).is_empty());
    assert_eq!(cut(&[p(0.0, 0.0), p(0.0, SHORTEST_STRETCH + 1e-9)], [-1.0, 1.0]).len(), 1);
    // The box's sides cut too: a level line through the stripe ends at its right side.
    assert_eq!(rounded(&cut(&[p(15.0, 2.0), p(25.0, 2.0)], [1.0, 3.0]).concat()), [(15.0, 2.0), (20.0, 2.0)]);
    // A point in a line twice sews nothing, and the meter is charged for each point.
    assert_eq!(cut(&[p(0.0, 2.0), p(0.0, 2.0), p(1.0, 2.0)], [1.0, 3.0]), [vec![p(0.0, 2.0), p(1.0, 2.0)]]);
    assert!(clipped(&tent, [1.0, 3.0], &box_, &mut Budget { max_work: 2, ..Budget::DEFAULT }.meter()).is_err());
    assert!(clipped(&tent, [1.0, 3.0], &box_, meter).is_ok());
}

#[test]
fn req_fill_trt_014_a_segment_s_stretch_between_bounds_is_where_it_enters_and_leaves_them() {
    let (least, most) = ([0.0, 0.0], [10.0, 10.0]);
    assert_eq!(within(p(-5.0, 5.0), p(15.0, 5.0), least, most), Some((0.25, 0.75)));
    assert_eq!(within(p(15.0, 5.0), p(-5.0, 5.0), least, most), Some((0.25, 0.75)));
    // Along a side or square to one: in when the other coordinate is, out when not.
    assert_eq!(within(p(0.0, 2.0), p(0.0, 8.0), least, most), Some((0.0, 1.0)));
    assert_eq!(within(p(10.5, 2.0), p(10.5, 8.0), least, most), None);
    assert_eq!(within(p(-0.5, 2.0), p(-0.5, 8.0), least, most), None);
    // Missing the bounds, and touching a corner.
    assert_eq!(within(p(11.0, 0.0), p(20.0, 9.0), least, most), None);
    assert_eq!(within(p(10.0, 10.0), p(12.0, 12.0), least, most), Some((0.0, 0.0)));
}

#[test]
fn req_fill_trt_014_a_band_s_pieces_are_joined_along_the_stripe_unless_an_end_lies_on_the_rings() {
    let (box_, tartan) = (frame([-10.0, -10.0], [20.0, 20.0]), tartan(&[]));
    let piece = square(0.0, 10.0);
    let laying = laying(&piece, ([0.0, 0.0], [10.0, 10.0]), &box_, &tartan, Direction::Weft);
    let meter = &mut Budget::DEFAULT.meter();
    let pieces = vec![vec![p(1.0, 3.0), p(2.0, 4.0)], vec![p(7.0, 4.0), p(6.0, 3.0)]];
    // 5 mm along the edge in 2 stitches of 2.5 mm.
    let joined_up = joined(pieces, &laying, meter).unwrap();
    assert_eq!(joined_up, [vec![p(1.0, 3.0), p(2.0, 4.0), p(4.5, 4.0), p(7.0, 4.0), p(6.0, 3.0)]]);
    // An end on the outline, or a start on it, parts them.
    let on_end = vec![vec![p(1.0, 3.0), p(2.0, 0.0)], vec![p(7.0, 4.0), p(6.0, 3.0)]];
    assert_eq!(joined(on_end, &laying, meter).unwrap().len(), 2);
    let on_start = vec![vec![p(1.0, 3.0), p(2.0, 4.0)], vec![p(10.0, 4.0), p(6.0, 3.0)]];
    assert_eq!(joined(on_start, &laying, meter).unwrap().len(), 2);
    assert!(joined(Vec::new(), &laying, meter).unwrap().is_empty());
}

#[test]
fn req_fill_trt_014_an_edge_crosses_the_piece_where_the_piece_reaches_it() {
    let reach = |top: f64, bottom: f64| ([0.0, top], [10.0, bottom]);
    assert!(crosses([2.0, 4.0], reach(4.0, 10.0)));
    assert!(!crosses([2.0, 4.0], reach(4.000_001, 10.0)));
    assert!(crosses([2.0, 4.0], reach(0.0, 2.0)));
    assert!(!crosses([2.0, 4.0], reach(0.0, 1.999_999)));
    // A piece inside the stripe meets neither edge.
    assert!(!crosses([2.0, 4.0], reach(2.5, 3.5)));
    assert!(crosses([2.0, 4.0], reach(3.0, 5.0)));
}

/// 3 bands' lines in a stripe from 10 to 12 mm down, across a box 30 mm wide: the first from the stripe's top
/// to its bottom, the second from its top back to it, the third from its top to its bottom.
fn banded() -> Vec<Vec<Vec<Point>>> {
    vec![vec![vec![p(1.0, 10.0), p(2.0, 12.0)]], vec![vec![p(11.0, 10.0), p(11.5, 11.0), p(12.0, 10.0)]], vec![vec![p(21.0, 10.0), p(22.0, 12.0)]]]
}

#[test]
fn req_fill_trt_014_upright_or_turned_less_than_a_quarter_the_bands_are_joined_from_the_right() {
    let (box_, tartan) = (frame([0.0, 0.0], [30.0, 30.0]), tartan(&[]));
    let piece = square(0.0, 30.0);
    let laying = laying(&piece, ([0.0, 0.0], [30.0, 30.0]), &box_, &tartan, Direction::Weft);
    let chain = chained(banded(), [10.0, 12.0], &laying, &mut Budget::DEFAULT.meter()).unwrap();
    // The last band's line runs on from its end nearer the box's left side, (21, 10), along the stripe's top
    // edge to the second band's line, which starts there, and from its end on to the first band's end there.
    let expected = [
        (22.0, 12.0),
        (21.0, 10.0),
        (18.5, 10.0),
        (16.0, 10.0),
        (13.5, 10.0),
        (11.0, 10.0),
        (11.5, 11.0),
        (12.0, 10.0),
        (9.8, 10.0),
        (7.6, 10.0),
        (5.4, 10.0),
        (3.2, 10.0),
        (1.0, 10.0),
        (2.0, 12.0),
    ];
    assert_eq!(rounded(&chain), expected);
}

#[test]
fn req_fill_trt_014_turned_more_than_a_quarter_the_bands_are_joined_from_the_left() {
    let (box_, tartan) = (frame([0.0, 0.0], [30.0, 30.0]), tartan(&[]));
    let piece = square(0.0, 30.0);
    let turned = Laying { turn: Turn::new(box_.middle, 180.0), ..laying(&piece, ([0.0, 0.0], [30.0, 30.0]), &box_, &tartan, Direction::Weft) };
    let chain = chained(banded(), [10.0, 12.0], &turned, &mut Budget::DEFAULT.meter()).unwrap();
    // The first band's line runs on from its end farther right, (2, 12), on the stripe's bottom edge. The second
    // band's line has no end there: it starts at its end on the edge its first point is not on, (12, 10), and
    // the join to it crosses the stripe. It ends where it began, so the last band's line starts on the top edge.
    let found = rounded(&chain);
    assert_eq!(found.get(..2).unwrap(), [(1.0, 10.0), (2.0, 12.0)]);
    assert_eq!(found.get(6..13).unwrap(), [(12.0, 10.0), (11.5, 11.0), (11.0, 10.0), (13.5, 10.0), (16.0, 10.0), (18.5, 10.0), (21.0, 10.0)]);
    assert_eq!((found.last(), found.len()), (Some(&(22.0, 12.0)), 14));
    // A quarter turn the other way joins from the left too; a quarter turn this way from the right.
    let left = Laying { turn: Turn::new(box_.middle, -90.0), ..turned };
    assert_eq!(rounded(&chained(banded(), [10.0, 12.0], &left, &mut Budget::DEFAULT.meter()).unwrap()).first(), Some(&(1.0, 10.0)));
    let right = Laying { turn: Turn::new(box_.middle, 90.0), ..left };
    assert_eq!(rounded(&chained(banded(), [10.0, 12.0], &right, &mut Budget::DEFAULT.meter()).unwrap()).first(), Some(&(22.0, 12.0)));
}

#[test]
fn req_fill_trt_014_the_first_band_s_line_turns_the_edges_round_past_their_middle() {
    let (box_, tartan) = (frame([0.0, 0.0], [30.0, 30.0]), tartan(&[]));
    let piece = square(0.0, 30.0);
    let laying = laying(&piece, ([0.0, 0.0], [30.0, 30.0]), &box_, &tartan, Direction::Weft);
    let chain = |lines: Vec<Vec<Point>>| rounded(&chained(vec![lines], [10.0, 12.0], &laying, &mut Budget::DEFAULT.meter()).unwrap());
    // Starting halfway along the edge, the edge keeps its way, and the line runs to its end farther along it.
    assert_eq!(chain(vec![vec![p(15.0, 10.0), p(14.0, 12.0)]]), [(14.0, 12.0), (15.0, 10.0)]);
    // A hair past halfway, the edge turns round, and the line runs to its end nearer the left side.
    assert_eq!(chain(vec![vec![p(15.000_001, 10.0), p(14.0, 12.0)]]), [(15.000_001, 10.0), (14.0, 12.0)]);
    // A point as near one edge as the other is on the first, the bottom one here: the line after the first,
    // which starts halfway between the edges, is sewn from its start, as the first ended on the bottom edge.
    let tie = vec![vec![p(21.0, 11.0), p(22.0, 10.0)], vec![p(2.0, 12.0), p(1.0, 10.0)]];
    let tied = chain(tie);
    assert_eq!((tied.get(..2), tied.last(), tied.len()), (Some([(1.0, 10.0), (2.0, 12.0)].as_slice()), Some(&(22.0, 10.0)), 11));
    assert!(chain(Vec::new()).is_empty());
}

#[test]
fn req_fill_trt_013_the_warp_is_laid_band_by_band_the_second_fourth_and_so_on_first() {
    // Red from 0 to 1.5 mm and blue from 1.5 to 10 mm across the square: the second band, from 2 to 4 mm, meets
    // only blue, so blue comes first, though the sett reaches red first.
    let tartan = Tartan { herringbone: 2.0, ..tartan(&[]) };
    let box_ = frame([0.0, 0.0], [20.0, 20.0]);
    let piece = square(0.0, 10.0);
    let laying = laying(&piece, ([0.0, 0.0], [10.0, 10.0]), &box_, &tartan, Direction::Warp);
    let herringbone = Herringbone::new(&box_, &tartan, &mut Budget::DEFAULT.meter()).unwrap();
    let walked = Walked { bands: vec![(RED, vec![[0.0, 1.5]]), (BLUE, vec![[1.5, 10.0]]), (GREEN, vec![[30.0, 40.0]])], runs: Vec::new() };
    let mut fills = Vec::new();
    herringbone.warp(&laying, &walked, &mut fills, &mut Budget::DEFAULT.meter()).unwrap();
    let colours: Vec<Rgb> = fills.iter().map(|(colour, _)| *colour).collect();
    assert_eq!(colours, [BLUE, RED]);
    // Every line lies in the square, and none runs on to its rings: each starts and ends on a band's edge or a
    // stripe's, or the square's.
    let inside = |q: &Point| (-1e-9..=10.0 + 1e-9).contains(&q.x()) && (-1e-9..=10.0 + 1e-9).contains(&q.y());
    assert!(fills.iter().flat_map(|(_, lines)| lines.iter().flatten()).all(inside));
}

#[test]
fn req_fill_trt_014_every_weft_colour_whose_stripes_reach_the_piece_is_listed_lines_or_none() {
    let tartan = Tartan { herringbone: 2.0, ..tartan(&[]) };
    let box_ = frame([-10.0, -10.0], [20.0, 20.0]);
    let piece = square(0.0, 10.0);
    let laying = laying(&piece, ([0.0, 0.0], [10.0, 10.0]), &box_, &tartan, Direction::Weft);
    let herringbone = Herringbone::new(&box_, &tartan, &mut Budget::DEFAULT.meter()).unwrap();
    // Red crosses the square; green only touches its bottom side, so has no line inside it; blue lies below
    // it, and a stripe of no width reaches nothing.
    let walked =
        Walked { bands: vec![(RED, vec![[2.0, 6.0]]), (GREEN, vec![[10.0, 12.0]]), (BLUE, vec![[12.0, 14.0], [5.0, 5.0]])], runs: Vec::new() };
    let mut fills = Vec::new();
    herringbone.weft(&laying, &walked, &mut fills, &mut Budget::DEFAULT.meter()).unwrap();
    let listed: Vec<(Rgb, usize)> = fills.iter().map(|(colour, lines)| (*colour, lines.len())).collect();
    assert_eq!(listed.iter().map(|(colour, _)| *colour).collect::<Vec<_>>(), [RED, GREEN]);
    assert!(listed.first().is_some_and(|(_, lines)| *lines > 0));
    assert_eq!(listed.get(1), Some(&(GREEN, 0)));
    // Red's stripe crosses the square, so its bands' lines are joined along its edges, from band to band: a line
    // reaches across more than 2 bands.
    let widest = fills.first().map(|(_, lines)| lines.iter().map(|line| reach_x(line)).fold(0.0, f64::max));
    assert!(widest.is_some_and(|x| x > 2.0 * (2.0 + GAP)), "{widest:?}");
}

#[test]
fn req_fill_trt_014_a_stripe_inside_the_piece_joins_no_band_to_the_next() {
    // A square from 3 to 5 mm inside a stripe from 0 to 8 mm: no edge crosses it, and each band's line is cut by
    // it alone.
    let tartan = Tartan { herringbone: 1.0, ..tartan(&[]) };
    let box_ = frame([-10.0, -10.0], [20.0, 20.0]);
    let piece = square(3.0, 5.0);
    let laying = laying(&piece, ([3.0, 3.0], [5.0, 5.0]), &box_, &tartan, Direction::Weft);
    let herringbone = Herringbone::new(&box_, &tartan, &mut Budget::DEFAULT.meter()).unwrap();
    let lines = herringbone.stripe(&laying, [0.0, 8.0], &mut Budget::DEFAULT.meter()).unwrap();
    // 3 bands meet it, and each line lies in one of them.
    assert!(lines.len() >= 3, "{lines:?}");
    assert!(lines.iter().all(|line| reach_x(line) <= 1.0 + 1e-9), "{lines:?}");
}

#[test]
fn req_fill_trt_014_a_full_sett_of_stripes_in_a_herringbone_charges_its_work() {
    let tartan = Tartan {
        herringbone: 2.0,
        warp: vec![Item::Band { colour: Some(RED), width: 3.0 }, Item::Band { colour: Some(BLUE), width: 3.0 }],
        weft: vec![Item::Band { colour: Some(RED), width: 3.0 }, Item::Band { colour: Some(BLUE), width: 3.0 }],
        ..tartan(&[])
    };
    let piece = square(0.0, 10.0);
    let lay: Question<'_> = Box::new(|meter| crate::generators::tartan::stripes::lay(&piece, &tartan, meter).map(|_| ()));
    let cost = *costs(&[lay]).first().unwrap();
    assert!(cost > 10_000, "{cost}");
}

/// How far across `line` reaches.
fn reach_x(line: &[Point]) -> f64 {
    let xs = line.iter().map(|q| q.x());
    xs.clone().fold(f64::NEG_INFINITY, f64::max) - xs.fold(f64::INFINITY, f64::min)
}
