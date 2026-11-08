//! The gradient fill's own cases: its colours and their threads, the travel left out at their ends, several
//! parts, the threads read for the sewing order, and its preview.

use stitchcraft_core::{Budget, Code};

use super::*;
use crate::design::GradientStop;
use crate::generators::tatami::fixture::{p, rectangle};

const RED: Rgb = Rgb::new(237, 23, 31);
const BLUE: Rgb = Rgb::new(10, 85, 163);
const WHITE: Rgb = Rgb::new(240, 240, 240);
const BLACK: Rgb = Rgb::new(0, 0, 0);

const TRAVEL: Travel = Travel { length: 2.5, tolerance: 0.2, min_stitch: 0.3, underpath: false, jump: 3.0 };

/// A gradient along x from `x0` to `x1` through `stops`, each an offset, a colour and an opacity.
fn along(x0: f64, x1: f64, stops: &[(f64, Rgb, f64)]) -> Gradient {
    let stops = stops.iter().map(|&(offset, colour, opacity)| GradientStop { offset, colour, opacity }).collect();
    Gradient { start: p(x0, 0.0), end: p(x1, 0.0), stops }
}

/// A gradient fill coloured so, its own thread black, with Ink/Stitch's defaults and no underlay.
fn filled(colouring: Colouring) -> GradientFill {
    GradientFill {
        colouring,
        spacing: 0.3,
        stitching: Stitching { longest: 3.0, staggers: 4.0, random: None },
        skip_last: false,
        stop_at_end: false,
        underlay: Vec::new(),
        inset: 0.0,
        expand: 0.0,
        travel: TRAVEL,
        own: BLACK,
    }
}

/// A tatami fill's underlay, one layer at `angle`.
fn underlay(angle: f64) -> Vec<Layer> {
    let tatami =
        crate::generators::tatami::TatamiParams::from_set_as(&stitchcraft_params::ParamSet::new(), stitchcraft_params::StitchType::TatamiFill)
            .unwrap()
            .params;
    let mut layers = tatami.underlay(stitchcraft_core::Mm::new(3.0).unwrap(), TRAVEL);
    for layer in &mut layers {
        layer.stitching.grid.angle = angle;
    }
    layers
}

/// The region of `parts`, each a rectangle from `(x0, y0)` to `(x1, y1)`.
fn region(parts: &[(f64, f64, f64, f64)]) -> Region {
    let parts = parts
        .iter()
        .map(|&(x0, y0, x1, y1)| Polygon { outline: vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], holes: Vec::new() })
        .collect();
    Region { parts }
}

fn sewn(region: &Region, fill: &GradientFill, needle: Option<Point>) -> Stitched {
    gradient_fill(region, fill, needle, None, &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap()
}

/// Each run's thread's colour, or the element's own for none, and whether it ends a colour block.
fn threads(stitched: &Stitched, own: Rgb) -> Vec<(Rgb, bool)> {
    stitched.colours.iter().map(|colour| (colour.thread.as_ref().map_or(own, |thread| thread.color), colour.ends_block)).collect()
}

#[test]
fn req_fill_grd_006_each_colour_sews_in_its_own_thread_in_the_order_the_stops_give_them() {
    // Red, white and blue across a square 20 mm a side, and the element's own thread white: white sews in
    // it, the others in threads of their own, each a colour block, in the stops' order.
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (0.5, WHITE, 1.0), (1.0, BLUE, 1.0)]);
    let fill = GradientFill { own: WHITE, ..filled(Colouring::Gradient(gradient)) };
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &fill, None);
    assert_eq!(threads(&stitched, WHITE), [(RED, true), (WHITE, true), (BLUE, true)]);
    assert_eq!(stitched.colours.iter().map(|colour| colour.thread.as_ref().and_then(|t| t.name.clone())).collect::<Vec<_>>(), [None, None, None]);
    assert!(stitched.colours[1].thread.is_none(), "white is the element's own thread");
    // Red's rows lie on the left, blue's on the right, and white's in the middle.
    let middle = |run: &[Point]| run.iter().map(|q| q.x()).sum::<f64>() / run.len() as f64;
    let middles: Vec<f64> = stitched.runs.iter().map(|run| middle(run)).collect();
    assert!(middles[0] < 7.0 && (8.0..12.0).contains(&middles[1]) && middles[2] > 13.0, "{middles:?}");
    // Every needle point lies in the square, and no stitch is shorter than the shortest.
    for run in &stitched.runs {
        assert!(run.iter().all(|q| (-1e-9..=20.0 + 1e-9).contains(&q.x()) && (-1e-9..=20.0 + 1e-9).contains(&q.y())));
        assert!(run.windows(2).all(|w| w[0].distance(w[1]) >= 0.3 - 1e-9), "{run:?}");
    }
    assert!(stitched.warnings.is_empty(), "{:?}", stitched.warnings);
}

#[test]
fn req_fill_grd_006_a_transparent_stop_s_rows_are_left_out() {
    // Red, a transparent white in the middle and blue: red's rows thin out to the middle and blue's grow
    // from it, and no white is sewn.
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (0.5, WHITE, 0.0), (1.0, BLUE, 1.0)]);
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &filled(Colouring::Gradient(gradient)), None);
    assert_eq!(threads(&stitched, BLACK), [(RED, true), (BLUE, true)]);
    let reach = |run: &[Point]| run.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(a, b), q| (a.min(q.x()), b.max(q.x())));
    let (red, blue) = (reach(&stitched.runs[0]), reach(&stitched.runs[1]));
    assert!(red.1 < 10.5 && blue.0 > 9.5, "{red:?} {blue:?}");
    // Just above 0, a stop sews its colour.
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (0.5, WHITE, 1e-9), (1.0, BLUE, 1.0)]);
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &filled(Colouring::Gradient(gradient)), None);
    assert_eq!(threads(&stitched, BLACK), [(RED, true), (WHITE, true), (BLUE, true)]);
}

#[test]
fn req_fill_grd_001_without_a_gradient_the_fill_s_colour_fades_out_along_its_angle() {
    // A square 20 mm a side fading at 0°: its rows run down the canvas, close on the left and fewer and
    // farther apart to the right, all in the element's own thread.
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &filled(Colouring::Fade(0.0)), None);
    assert_eq!(threads(&stitched, BLACK), [(BLACK, true)]);
    let mut rows: Vec<f64> = Vec::new();
    for w in stitched.runs[0].windows(2) {
        if (w[0].x() - w[1].x()).abs() < 1e-9 && (w[0].y() - w[1].y()).abs() > 1.0 && !rows.iter().any(|x| (x - w[0].x()).abs() < 1e-6) {
            rows.push(w[0].x());
        }
    }
    rows.sort_by(f64::total_cmp);
    let left = rows.iter().filter(|&&x| x < 10.0).count();
    assert!(2 * left > 3 * (rows.len() - left), "{rows:?}");
    let gaps: Vec<f64> = rows.windows(2).map(|w| w[1] - w[0]).collect();
    assert!(gaps[0] < 0.31 && gaps[gaps.len() - 1] > 1.0, "{gaps:?}");
    // At 90° the way points down the canvas, and the rows run across it, thinning towards the bottom.
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &filled(Colouring::Fade(90.0)), None);
    let level = stitched.runs[0].windows(2).filter(|w| (w[0].y() - w[1].y()).abs() < 1e-9 && (w[0].x() - w[1].x()).abs() > 1.0).count();
    assert!(level > 100, "{level}");
    let low = stitched.runs[0].iter().filter(|q| q.y() > 10.0).count();
    assert!(low * 2 < stitched.runs[0].len(), "{low}");
}

#[test]
fn req_fill_grd_006_the_underlay_sews_in_the_first_stop_s_colour_and_ends_no_colour_block() {
    // The first stop transparent red: the underlay sews red, in a thread of its own, and ends no block;
    // only blue's rows sew after it.
    let gradient = along(0.0, 20.0, &[(0.0, RED, 0.0), (1.0, BLUE, 1.0)]);
    let fill = GradientFill { underlay: underlay(0.0), ..filled(Colouring::Gradient(gradient)) };
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &fill, None);
    assert_eq!(threads(&stitched, BLACK), [(RED, false), (BLUE, true)]);
    // An underlay in the element's own colour sews in its thread.
    let gradient = along(0.0, 20.0, &[(0.0, BLACK, 1.0), (1.0, BLUE, 1.0)]);
    let fill = GradientFill { underlay: underlay(0.0), ..filled(Colouring::Gradient(gradient)) };
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &fill, None);
    assert_eq!(threads(&stitched, BLACK), [(BLACK, false), (BLACK, true), (BLUE, true)]);
    assert!(stitched.colours[0].thread.is_none() && stitched.colours[1].thread.is_none());
    // The first colour runs on from the underlay: its first point is where the underlay ended, or the
    // travel from there along the outline.
    let (under, first) = (&stitched.runs[0], &stitched.runs[1]);
    assert!(under.last().unwrap().distance(first[0]) < 2.5 + 1e-9, "{:?} {:?}", under.last(), first[0]);
    // Without a gradient the fill's own colour sews it.
    let fill = GradientFill { underlay: underlay(90.0), ..filled(Colouring::Fade(0.0)) };
    assert_eq!(threads(&sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &fill, None), BLACK), [(BLACK, false), (BLACK, true)]);
}

#[test]
fn req_fill_grd_006_a_colour_after_the_first_starts_with_its_rows_and_each_ends_with_them() {
    // Red, white and blue, the needle at the square's top left: each colour's route starts at the outline
    // nearest the needle, and its travel to its first row and from its last row is left out.
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (0.5, WHITE, 1.0), (1.0, BLUE, 1.0)]);
    let square = region(&[(0.0, 0.0, 20.0, 20.0)]);
    let stitched = sewn(&square, &filled(Colouring::Gradient(gradient)), Some(p(0.0, 0.0)));
    // A row runs down the canvas: the first stitch of each colour and its last are along a row.
    for run in &stitched.runs {
        let upright = |a: Point, b: Point| (a.x() - b.x()).abs() < 1e-9;
        assert!(upright(run[0], run[1]) && upright(run[run.len() - 2], run[run.len() - 1]), "{:?}", &run[..3]);
    }
}

#[test]
fn req_fill_grd_007_stop_at_ending_point_keeps_the_travel_of_the_last_2_colours_to_the_end() {
    // Red, white and blue, the needle at the square's top left and the end at its bottom right: the first
    // colour ends with its rows, and the last 2 travel on to the end.
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (0.5, WHITE, 1.0), (1.0, BLUE, 1.0)]);
    let square = region(&[(0.0, 0.0, 20.0, 20.0)]);
    let fill = GradientFill { stop_at_end: true, ..filled(Colouring::Gradient(gradient)) };
    let end = Approach::Point(p(20.0, 20.0));
    let stopped = gradient_fill(&square, &fill, Some(p(0.0, 0.0)), Some(&end), &mut SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
    let ends: Vec<Point> = stopped.runs.iter().map(|run| *run.last().unwrap()).collect();
    assert!(ends[0].distance(p(20.0, 20.0)) > 1.0, "{ends:?}");
    assert!(ends[1..].iter().all(|q| q.distance(p(20.0, 20.0)) < 0.5), "{ends:?}");
}

#[test]
fn travel_is_left_out_as_ink_stitch_strips_it() {
    let marked = |travel: &[bool]| Marked { points: (0..travel.len()).map(|i| p(i as f64, 0.0)).collect(), travel: travel.to_vec() };
    let xs = |runs: Vec<Vec<Point>>| runs.iter().map(|run| run.iter().map(|q| q.x()).collect::<Vec<_>>()).collect::<Vec<_>>();
    let both = Ends { lead: true, trail: true };
    // Travel before the first point on a row and after the last.
    assert_eq!(xs(stripped(vec![marked(&[true, true, false, true, false, true, true])], both)), [vec![2.0, 3.0, 4.0]]);
    // Kept where the ends say.
    assert_eq!(xs(stripped(vec![marked(&[true, false, false, true])], Ends { lead: false, trail: true })), [vec![0.0, 1.0, 2.0]]);
    assert_eq!(xs(stripped(vec![marked(&[true, false, false, true])], Ends { lead: true, trail: false })), [vec![1.0, 2.0, 3.0]]);
    // A colour whose only point on a row is its last keeps the travel before it, and one whose only point on a
    // row is its first loses its last point alone.
    assert_eq!(xs(stripped(vec![marked(&[true, true, false])], both)), [vec![0.0, 1.0, 2.0]]);
    assert_eq!(xs(stripped(vec![marked(&[false, true, true])], both)), [vec![0.0, 1.0]]);
    // All travel: the last point goes.
    assert_eq!(xs(stripped(vec![marked(&[true, true, true])], both)), [vec![0.0, 1.0]]);
    // Several runs, as after a jump: the first loses its lead and the last its trail; a run left empty goes.
    let runs = vec![marked(&[true, false, true]), marked(&[false])];
    assert_eq!(xs(stripped(runs, both)), [vec![1.0, 2.0]]);
}

#[test]
fn req_fill_grd_008_several_parts_sew_each_colour_in_one_stretch() {
    // 2 squares side by side under one gradient: each part's red, then its blue, sorted so that red sews in
    // both parts before blue.
    let gradient = along(0.0, 50.0, &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    let fill = GradientFill { underlay: underlay(90.0), ..filled(Colouring::Gradient(gradient)) };
    let parts = region(&[(0.0, 0.0, 20.0, 20.0), (30.0, 0.0, 50.0, 20.0)]);
    let stitched = sewn(&parts, &fill, Some(p(0.0, 0.0)));
    assert_eq!(threads(&stitched, BLACK), [(RED, false), (RED, true), (RED, false), (RED, true), (BLUE, true), (BLUE, true)]);
    let left = |run: &Vec<Point>| run.iter().all(|q| q.x() <= 20.0 + 1e-9);
    assert_eq!(stitched.runs.iter().map(left).collect::<Vec<_>>(), [true, true, false, false, true, false]);
    // In one part, no sort.
    let one = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &fill, Some(p(0.0, 0.0)));
    assert_eq!(threads(&one, BLACK), [(RED, false), (RED, true), (BLUE, true)]);
}

#[test]
fn diag_sc_w0330_a_stop_passed_over_where_its_row_left_out_crosses_the_fill_is_said() {
    // A gradient of no length in the middle of the square: its blue stop is passed over, and the row after
    // red's leaves a gap.
    let gradient = Gradient { start: p(10.0, 10.0), end: p(10.0, 10.0), stops: along(0.0, 1.0, &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]).stops };
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &filled(Colouring::Gradient(gradient)), None);
    assert_eq!(stitched.warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillGradientStopPassedOver]);
    assert_eq!(
        stitched.warnings[0].message,
        "Stop 2 of this fill's gradient lies within 2 rows of the blend before, so no blend starts there and the row after that blend is left \
         out, as Ink/Stitch sews it."
    );
    // Red sews up to the middle and blue from just past it.
    assert_eq!(threads(&stitched, BLACK), [(RED, true), (BLUE, true)]);
    // A stop passed over outside the fill says nothing: the gradient's point is 15 mm left of the square.
    let gradient = Gradient { start: p(-15.0, 10.0), end: p(-15.0, 10.0), stops: along(0.0, 1.0, &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]).stops };
    let stitched = sewn(&region(&[(0.0, 0.0, 20.0, 20.0)]), &filled(Colouring::Gradient(gradient)), None);
    assert!(stitched.warnings.is_empty(), "{:?}", stitched.warnings);
    // Several are named together.
    let said = passed_over(&BTreeSet::from([1, 2, 4])).unwrap();
    assert!(said.message.starts_with("Stops 2, 3 and 5 of this fill's gradient each lie within 2 rows"), "{}", said.message);
    assert_eq!(passed_over(&BTreeSet::new()), None);
}

#[test]
fn req_fill_grd_008_the_threads_are_read_from_the_paint_before_the_fill_is_sewn() {
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (0.3, WHITE, 0.0), (0.6, BLUE, 1.0), (1.0, RED, 1.0)]);
    // The underlay's colour, the first stop's, then the colours of the stops that show, each once.
    assert_eq!(super::threads(Some(&gradient), BLACK, true), [RED, RED, BLUE]);
    assert_eq!(super::threads(Some(&gradient), BLACK, false), [RED, BLUE]);
    // A transparent first stop's colour sews the underlay all the same.
    let gradient = along(0.0, 20.0, &[(0.0, WHITE, 0.0), (1.0, BLUE, 1.0)]);
    assert_eq!(super::threads(Some(&gradient), BLACK, true), [WHITE, BLUE]);
    // All transparent and no underlay: the element's own thread.
    let gradient = along(0.0, 20.0, &[(0.0, WHITE, 0.0), (1.0, BLUE, 0.0)]);
    assert_eq!(super::threads(Some(&gradient), BLACK, false), [BLACK]);
    // Without a gradient, its own thread for both.
    assert_eq!(super::threads(None, BLACK, true), [BLACK, BLACK]);
    assert_eq!(super::threads(None, BLACK, false), [BLACK]);
}

#[test]
fn the_preview_shows_every_colour_s_rows_inside_the_fill() {
    let gradient = along(0.0, 20.0, &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    let fill = filled(Colouring::Gradient(gradient));
    let segments = top_path(&region(&[(0.0, 0.0, 20.0, 20.0)]), &fill, &SplitMix64::new(1), &mut Budget::DEFAULT.meter()).unwrap();
    // Rows 0.3 mm apart across the 20 mm square, each 20 mm long.
    let long: f64 = segments.iter().map(|s| s.start.distance(s.end)).sum();
    assert!((1300.0..1360.0).contains(&long), "{long}");
    assert!(segments.iter().all(|s| [s.start, s.end].iter().all(|q| (-1e-9..=20.0 + 1e-9).contains(&q.x()))));
}

#[test]
fn the_underlay_runs_along_the_gradient_square_to_its_rows() {
    let gradient = |end: Point| Gradient { start: p(0.0, 0.0), end, stops: Vec::new() };
    assert_eq!(underlay_angle(&gradient(p(10.0, 0.0))), 0.0);
    assert!((underlay_angle(&gradient(p(0.0, 10.0))) + 90.0).abs() < 1e-12);
    assert!((underlay_angle(&gradient(p(-5.0, -5.0))) - 135.0).abs() < 1e-12);
    // A gradient of no length points along x.
    assert_eq!(underlay_angle(&gradient(p(0.0, 0.0))), 0.0);
}

#[test]
fn a_piece_without_rows_sews_no_colour() {
    // A rectangle 10 mm wide and a gradient wholly in it, rows 0.3 mm apart: every colour sews. Without a
    // point to lay rows over, nothing.
    let gradient = along(0.0, 10.0, &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    let fill = filled(Colouring::Gradient(gradient));
    let mut passed = BTreeSet::new();
    let coloured = coloured(&rectangle(4.0), &fill, &SplitMix64::new(1), &mut passed, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(coloured.iter().map(|(colour, _)| *colour).collect::<Vec<_>>(), [RED, BLUE]);
    let empty = Polygon { outline: Vec::new(), holes: Vec::new() };
    assert!(coloured_of(&empty, &fill).is_empty());
}

fn coloured_of(piece: &Polygon, fill: &GradientFill) -> Coloured {
    coloured(piece, fill, &SplitMix64::new(1), &mut BTreeSet::new(), &mut Budget::DEFAULT.meter()).unwrap()
}
