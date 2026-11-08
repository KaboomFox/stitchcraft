//! A tartan fill (`REQ-FILL-TRT-003` to `REQ-FILL-TRT-010`), sewn as whole designs: the sett is read and laid
//! in the generator's modules (`generators::tartan`); these cases sew designs through every stage, with what
//! is said of them.
//!
//! Each case's square spans -20 to 20 mm both ways. Its middle is the origin, and the smallest circle round
//! it has a radius of 28.28 mm, half its diagonal. With a sett 10 mm wide, the box the stripes are laid over
//! reaches 38.28 mm from the middle, so the warp's first stripe starts at x = -38.28 and the weft's at
//! y = -38.28. The rows at -45° run up and to the right, each along a line where x + y is the same: the box's
//! side, 76.57 mm, in 307 equal steps of 0.2494 mm, which lie 0.2494 × √2 mm apart in x + y, the first at
//! x + y = -38.28 × √2.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests), and rounds a row's
// place across the box to its number.
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic, clippy::cast_possible_truncation)]

use std::f64::consts::SQRT_2;

use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Hints};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, Role, StitchKind, Thread};
use stitchcraft_testkit::designs::{fill, messages, p, planned};

/// The element's own thread, which its underlay sews in.
const OWN: Thread = Thread::new(Rgb::new(0, 0, 0));
const RED: Rgb = Rgb::new(255, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);
/// The square every case fills.
const SQUARE: [(f64, f64); 4] = [(-20.0, -20.0), (20.0, -20.0), (20.0, 20.0), (-20.0, 20.0)];
/// Red and blue stripes 5 mm wide, repeated: a sett 10 mm wide.
const RED_BLUE: &str = r#"{"palette": "(#ff0000)5 (#0000ff)5", "symmetry": false}"#;
/// How far the box reaches from the square's middle for a sett 10 mm wide.
const REACH: f64 = 28.284_271_247_461_9 + 10.0;
/// How far apart the rows lie in x + y, and where the first does.
const STEP: f64 = 2.0 * REACH / 307.0 * SQRT_2;
const FIRST: f64 = -REACH * SQRT_2;
/// What a point may be off by.
const NEAR: f64 = 1e-6;

/// A tartan fill of the square in the element's own thread, trimmed after, with `params` too.
fn tartan(params: &[(&str, &str)]) -> Element {
    fill("tartan", &[&SQUARE], &OWN, &[&[("fill_method", "tartan_fill"), ("trim_after", "true")], params].concat())
}

/// Each colour block's thread and the needle points of its stitches, but the locks and the needle's travel
/// between elements.
fn blocks(outcome: &PlanOutcome) -> Vec<(Rgb, Vec<Point>)> {
    let plan = outcome.plan.as_ref().unwrap();
    plan.blocks
        .iter()
        .map(|block| {
            let points = block.stitches.iter().filter(|s| s.kind == StitchKind::Normal && s.origin.role == Role::Top).map(|s| s.at).collect();
            (block.thread.color, points)
        })
        .collect()
}

/// The colours of the blocks, in order.
fn threads(outcome: &PlanOutcome) -> Vec<Rgb> {
    blocks(outcome).into_iter().map(|(colour, _)| colour).collect()
}

/// The needle points of the block in `colour`.
fn sewn_in(outcome: &PlanOutcome, colour: Rgb) -> Vec<Point> {
    blocks(outcome).into_iter().find(|(c, _)| *c == colour).map(|(_, points)| points).unwrap()
}

/// Whether `q` lies on the square's outline.
fn on_outline(q: Point) -> bool {
    (q.x().abs() - 20.0).abs() < NEAR || (q.y().abs() - 20.0).abs() < NEAR
}

/// Whether `q` lies within a millimetre of the square's outline.
fn by_outline(q: Point) -> bool {
    20.0 - q.x().abs() < 1.0 || 20.0 - q.y().abs() < 1.0
}

/// Whether `v`, along x or y, lies in a band from `from` mm past each multiple of 10 mm from the box's side
/// at `-reach - offset` to 5 mm further on, its edges included.
fn in_band(v: f64, reach: f64, offset: f64, from: f64) -> bool {
    let at = (v + reach + offset - from).rem_euclid(10.0);
    at <= 5.0 + NEAR || at >= 10.0 - NEAR
}

/// Whether `v` lies on an edge of such a band.
fn on_edge(v: f64, from: f64) -> bool {
    let at = (v + REACH - from).rem_euclid(5.0);
    !(NEAR..=5.0 - NEAR).contains(&at)
}

/// The row whose line `q` lies on, and whether it lies on one.
fn row(q: Point) -> (i64, bool) {
    let k = (q.x() + q.y() - FIRST) / STEP;
    (k.round() as i64, (k - k.round()).abs() < NEAR)
}

#[test]
fn req_fill_trt_003_each_colour_sews_in_its_bands_laid_from_the_box_s_sides() {
    // Red from the box's left and top sides, blue 5 mm on, in turn: each needle point of a colour lies in its
    // bands one way or the other, but the needle's travel along the outline.
    let outcome = planned(vec![tartan(&[("tartan", RED_BLUE)])]);
    assert_eq!(threads(&outcome), [OWN.color, RED, BLUE]);
    for (colour, from) in [(RED, 0.0), (BLUE, 5.0)] {
        let points: Vec<Point> = sewn_in(&outcome, colour).into_iter().filter(|q| !on_outline(*q)).collect();
        assert!(points.len() > 1000, "{}", points.len());
        let out: Vec<&Point> = points.iter().filter(|q| !in_band(q.x(), REACH, 0.0, from) && !in_band(q.y(), REACH, 0.0, from)).collect();
        assert!(out.is_empty(), "{colour}: {out:?}");
    }
    // Moved 3 mm right and 2 mm down, the box's left and top sides move out by as much, and the bands with them.
    let moved = sett_with(r#", "offset_x": 3, "offset_y": -2"#);
    let outcome = planned(vec![tartan(&[("tartan", &moved)])]);
    let red: Vec<Point> = sewn_in(&outcome, RED).into_iter().filter(|q| !on_outline(*q)).collect();
    assert!(red.iter().all(|q| in_band(q.x(), REACH, 3.0, 0.0) || in_band(q.y(), REACH, 2.0, 0.0)));
    assert!(!red.iter().all(|q| in_band(q.x(), REACH, 0.0, 0.0) || in_band(q.y(), REACH, 0.0, 0.0)));
}

/// [`RED_BLUE`] with more settings.
fn sett_with(more: &str) -> String {
    format!(r#"{{"palette": "(#ff0000)5 (#0000ff)5", "symmetry": false{more}}}"#)
}

#[test]
fn req_fill_trt_004_rows_lie_a_spacing_apart_and_go_to_the_warp_and_the_weft_in_turns() {
    let outcome = planned(vec![tartan(&[("tartan", RED_BLUE)])]);
    let red = sewn_in(&outcome, RED);
    // Inside the bands, every needle point lies on a row.
    let inner: Vec<Point> = red.iter().copied().filter(|q| !by_outline(*q) && !on_edge(q.x(), 0.0) && !on_edge(q.y(), 0.0)).collect();
    assert!(inner.len() > 500, "{}", inner.len());
    assert!(inner.iter().all(|q| row(*q).1), "{:?}", inner.iter().find(|q| !row(**q).1));
    // In a red warp band over a blue weft band, red sews the warp's rows, 2 of every 4; over a red weft band
    // across a blue warp band, the weft's, the other 2.
    let (mut warp, mut weft) = (0, 0);
    for q in &inner {
        let (k, _) = row(*q);
        let (in_warp, in_weft) = (in_band(q.x(), REACH, 0.0, 0.0), in_band(q.y(), REACH, 0.0, 0.0));
        if in_warp && !in_weft {
            assert!(k.rem_euclid(4) < 2, "{q:?} on row {k}");
            warp += 1;
        } else if in_weft && !in_warp {
            assert!(k.rem_euclid(4) >= 2, "{q:?} on row {k}");
            weft += 1;
        }
    }
    assert!(warp > 100 && weft > 100, "{warp} {weft}");
    // With a row per thread, they go in turns of 1.
    let outcome = planned(vec![tartan(&[("tartan", RED_BLUE), ("rows_per_thread", "1")])]);
    for q in sewn_in(&outcome, RED).iter().filter(|q| !by_outline(**q) && !on_edge(q.x(), 0.0) && !on_edge(q.y(), 0.0)) {
        let (k, _) = row(*q);
        let (in_warp, in_weft) = (in_band(q.x(), REACH, 0.0, 0.0), in_band(q.y(), REACH, 0.0, 0.0));
        assert!(!(in_warp && !in_weft) || k.rem_euclid(2) == 0, "{q:?} on row {k}");
        assert!(!(in_weft && !in_warp) || k.rem_euclid(2) == 1, "{q:?} on row {k}");
    }
}

#[test]
fn req_fill_trt_004_at_an_angle_of_0_the_rows_run_level_along_the_weft_s_stripes() {
    // Level rows from the box's left side, its height of 76.57 mm in 307 equal steps from its top: every needle
    // point of a colour inside the square lies on one. A weft row lies in a band or misses it, and red sews
    // its weft's bands across the blue warp bands too.
    let outcome = planned(vec![tartan(&[("tartan", RED_BLUE), ("tartan_angle", "0")])]);
    let step = 2.0 * REACH / 307.0;
    let red: Vec<Point> = sewn_in(&outcome, RED).into_iter().filter(|q| !on_outline(*q)).collect();
    assert!(red.len() > 500, "{}", red.len());
    let level = |q: &Point| {
        let k = (q.y() + REACH) / step;
        (k - k.round()).abs() < NEAR
    };
    assert!(red.iter().all(level), "{:?}", red.iter().find(|q| !level(q)));
    let across = red.iter().filter(|q| in_band(q.y(), REACH, 0.0, 0.0) && !in_band(q.x(), REACH, 0.0, 0.0)).count();
    assert!(across > 100, "{across}");
}

#[test]
fn req_fill_trt_004_a_row_s_needle_points_lie_the_element_s_longest_stitch_apart() {
    // The stitches along each row, from one needle point to the next on the same row: a red stretch crosses
    // its band over 7.07 mm, long enough for whole stitches.
    let along = |outcome: &PlanOutcome| -> Vec<f64> {
        let red = sewn_in(outcome, RED);
        red.windows(2).filter(|w| row(w[0]).1 && row(w[1]).1 && row(w[0]).0 == row(w[1]).0).map(|w| w[0].distance(w[1])).collect()
    };
    let near = |lengths: &[f64], length: f64| lengths.iter().filter(|l| (*l - length).abs() < 0.01).count();
    // By default the rows' stitches are 4 mm long, as Ink/Stitch's fill's are.
    let default = along(&planned(vec![tartan(&[("tartan", RED_BLUE)])]));
    assert!(near(&default, 4.0) > 100, "{}", near(&default, 4.0));
    // The element's own longest stitch sets them: 3 mm, and none longer, but where a needle point closer than
    // the shortest stitch to the one before is left out.
    let three = along(&planned(vec![tartan(&[("tartan", RED_BLUE), ("max_stitch_length_mm", "3")])]));
    assert!(near(&three, 3.0) > 100, "{}", near(&three, 3.0));
    assert_eq!(near(&three, 4.0), 0);
    assert!(three.iter().all(|l| *l < 3.0 + 0.5 + NEAR), "{:?}", three.iter().copied().fold(0.0, f64::max));
}

#[test]
fn req_fill_trt_005_a_band_s_rows_are_joined_along_its_edges() {
    let outcome = planned(vec![tartan(&[("tartan", RED_BLUE)])]);
    let red = sewn_in(&outcome, RED);
    // Away from the outline, every needle point lies on a row or on a band's edge, where the rows are joined.
    for q in red.iter().filter(|q| !by_outline(**q)) {
        assert!(row(*q).1 || on_edge(q.x(), 0.0) || on_edge(q.y(), 0.0), "{q:?}");
    }
    // Each join runs along the edge, in stitches no longer than the running stitch, 2.5 mm. A row's last
    // needle point nearer its end than the shortest stitch is left out, and the stitch from the point before
    // it runs to the join.
    let joins: Vec<f64> = red
        .windows(2)
        .filter(|w| !by_outline(w[0]) && !by_outline(w[1]))
        .filter(|w| {
            let upright = (w[0].x() - w[1].x()).abs() < NEAR && on_edge(w[0].x(), 0.0);
            let level = (w[0].y() - w[1].y()).abs() < NEAR && on_edge(w[0].y(), 0.0);
            upright || level
        })
        .map(|w| w[0].distance(w[1]))
        .collect();
    assert!(joins.len() > 100, "{}", joins.len());
    assert!(joins.iter().all(|&d| d <= 2.5 + NEAR), "{joins:?}");
}

#[test]
fn req_fill_trt_006_the_underlay_then_each_colour_in_its_own_thread_starting_and_ending_with_its_rows() {
    let outcome = planned(vec![tartan(&[("tartan", RED_BLUE)])]);
    let plan = outcome.plan.as_ref().unwrap();
    assert_eq!(threads(&outcome), [OWN.color, RED, BLUE]);
    // A trim after each colour, as trim_after asks, and none after the underlay, which ends no colour block.
    let trimmed: Vec<bool> = plan.blocks.iter().map(|block| block.stitches.iter().any(|s| s.kind == StitchKind::Trim)).collect();
    assert_eq!(trimmed, [false, true, true]);
    // Each colour starts and ends with its rows, in its bands, the travel before and after them left out.
    for (colour, from) in [(RED, 0.0), (BLUE, 5.0)] {
        let points = sewn_in(&outcome, colour);
        for q in [points[0], *points.last().unwrap()] {
            assert!(in_band(q.x(), REACH, 0.0, from) || in_band(q.y(), REACH, 0.0, from), "{colour}: {q:?}");
        }
    }
    // Without underlay, the colours alone.
    let bare = planned(vec![tartan(&[("tartan", RED_BLUE), ("fill_underlay", "false")])]);
    assert_eq!(threads(&bare), [RED, BLUE]);
}

#[test]
fn req_fill_trt_006_a_colour_of_the_element_s_own_sews_in_the_element_s_own_thread() {
    // A named thread, as a document's swatch or a test sheet gives an element: the blue rows sew in it too,
    // and red in a thread of its colour alone. A machine file then writes blue as one entry of its palette,
    // where a named and an unnamed thread of one colour would be 2 threads and 2 entries.
    let navy = Thread::named(BLUE, "Navy");
    let element = fill("tartan", &[&SQUARE], &navy, &[("fill_method", "tartan_fill"), ("trim_after", "true"), ("tartan", RED_BLUE)]);
    let plan = planned(vec![element]).plan.unwrap();
    let threads: Vec<&Thread> = plan.blocks.iter().map(|block| &block.thread).collect();
    assert_eq!(threads, [&navy, &Thread::new(RED), &navy]);
    let entries = BROTHER_PEC.assign(plan.color_entries().iter().map(|entry| entry.thread)).unwrap().entries;
    assert_eq!(entries.first().map(|e| e.index), entries.last().map(|e| e.index));
}

#[test]
fn req_fill_trt_007_narrow_stripes_sew_as_runs_after_the_fills_with_bean_stitch() {
    // Blue 0.5 mm wide sews as a run, red's sett mirrored: red 5, then a run in a red band of 0.75, 5.75 mm in
    // all, the run 5.25 mm from each repeat's start. The sett is 5 mm wide, so the box reaches 33.28 mm.
    let runs = r#"{"palette": "(#ff0000)/5 (#0000ff)/0.5"}"#;
    let reach = 28.284_271_247_461_9 + 5.0;
    let on_run = |v: f64| {
        let at = (v + reach - 5.25).rem_euclid(5.75);
        !(NEAR..=5.75 - NEAR).contains(&at)
    };
    let plain = planned(vec![tartan(&[("tartan", runs)])]);
    assert_eq!(threads(&plain), [OWN.color, RED, BLUE]);
    let blue = sewn_in(&plain, BLUE);
    assert!(
        blue.iter().all(|q| on_run(q.x()) || on_run(q.y()) || on_outline(*q)),
        "{:?}",
        blue.iter().find(|q| !(on_run(q.x()) || on_run(q.y()) || on_outline(**q)))
    );
    // In equal stitches: no longer than the running stitch along the warp, and the longest stitch, 4 mm, along
    // the weft.
    let upright: Vec<f64> = blue.windows(2).filter(|w| on_run(w[0].x()) && (w[0].x() - w[1].x()).abs() < NEAR).map(|w| w[0].distance(w[1])).collect();
    let level: Vec<f64> = blue.windows(2).filter(|w| on_run(w[0].y()) && (w[0].y() - w[1].y()).abs() < NEAR).map(|w| w[0].distance(w[1])).collect();
    assert!(upright.iter().all(|&d| d <= 2.5 + NEAR) && level.iter().all(|&d| d <= 4.0 + NEAR));
    assert!(level.iter().any(|&d| d > 2.5 + NEAR), "{level:?}");
    // With bean stitch, each stitch of a run is sewn there, back and there again, but the travel's.
    let beaned = planned(vec![tartan(&[("tartan", runs), ("bean_stitch_repeats", "1")])]);
    let bean = sewn_in(&beaned, BLUE);
    let back = |points: &[Point]| points.windows(3).filter(|w| w[0].distance(w[2]) < NEAR && w[0].distance(w[1]) > NEAR).count();
    assert!(back(&bean) > 2 * blue.len() / 3, "{} of {}", back(&bean), bean.len());
    assert_eq!(back(&blue), 0);
}

#[test]
fn req_fill_trt_008_stop_at_ending_point_keeps_each_colour_s_travel_to_the_end() {
    let end = p(20.0, 20.0);
    let ended = |stop: &str| {
        let element =
            Element { hints: Hints { end: Some(end), ..Hints::default() }, ..tartan(&[("tartan", RED_BLUE), ("stop_at_ending_point", stop)]) };
        planned(vec![element])
    };
    // On, every colour starts where the needle was when the piece's colours began, and ends at the end.
    let stopped = ended("true");
    let (red, blue) = (sewn_in(&stopped, RED), sewn_in(&stopped, BLUE));
    assert!(red.last().unwrap().distance(end) < NEAR && blue.last().unwrap().distance(end) < NEAR, "{:?} {:?}", red.last(), blue.last());
    assert!(red[0].distance(blue[0]) < NEAR, "{:?} {:?}", red[0], blue[0]);
    // Off, a colour ends with its rows, and the next starts where it ended.
    let plain = ended("false");
    let (red, blue) = (sewn_in(&plain, RED), sewn_in(&plain, BLUE));
    assert!(red.last().unwrap().distance(end) > 1.0, "{:?}", red.last());
    assert!(red.len() < sewn_in(&stopped, RED).len());
    assert!(blue[0].distance(red[red.len() - 1]) < 30.0);
}

#[test]
fn req_fill_trt_009_a_tartan_in_parts_sews_each_thread_once() {
    let parts: [&[(f64, f64)]; 2] =
        [&[(-30.0, -10.0), (-10.0, -10.0), (-10.0, 10.0), (-30.0, 10.0)], &[(10.0, -10.0), (30.0, -10.0), (30.0, 10.0), (10.0, 10.0)]];
    let element = fill("parts", &parts, &OWN, &[("fill_method", "tartan_fill"), ("tartan", RED_BLUE), ("trim_after", "true")]);
    let outcome = planned(vec![element]);
    // Both parts' underlays, then both parts' red, then both parts' blue.
    assert_eq!(threads(&outcome), [OWN.color, RED, BLUE]);
    for (colour, points) in blocks(&outcome) {
        assert!(points.iter().any(|q| q.x() < 0.0) && points.iter().any(|q| q.x() > 0.0), "{colour}");
    }
}

#[test]
fn req_fill_trt_010_a_herringbone_sews_straight_rows_until_m10_7() {
    let straight = planned(vec![tartan(&[("tartan", RED_BLUE)])]);
    let herringbone = planned(vec![tartan(&[("tartan", RED_BLUE), ("herringbone_width_mm", "3")])]);
    assert_eq!(herringbone.plan, straight.plan);
    assert_eq!(
        messages(&herringbone),
        ["warning SC-W0324: This tartan's herringbone is 3 mm wide, and StitchCraft sews herringbones from M10.7, so its rows are sewn without one."]
    );
    assert!(straight.diagnostics.is_empty(), "{:?}", messages(&straight));
}

#[test]
fn a_tartan_s_diagnostics_name_it_and_one_it_can_t_read_is_skipped() {
    // Its sett can't be read: skipped, the next element sewn all the same.
    let next = fill("next", &[&[(30.0, 0.0), (40.0, 0.0), (40.0, 10.0), (30.0, 10.0)]], &OWN, &[]);
    let outcome = planned(vec![tartan(&[("tartan", r#"{"palette": "(#ff0000)5 | (#0000ff)5 | (#00ff00)5"}"#)]), next]);
    assert_eq!(
        messages(&outcome),
        [
            "error SC-E0323: This tartan fill's sett can't be read, as its code has more than one `|`, so it is skipped.",
            "warning SC-W0505: This element sews no stitch, so the trim after it is left out."
        ]
    );
    assert_eq!(outcome.diagnostics[0].element.as_ref().map(|id| id.as_str()), Some("tartan"));
    assert_eq!(threads(&outcome), [OWN.color]);
    // Without a sett, Ink/Stitch's default one, and a colour of the code that can't be read left out.
    let outcome = planned(vec![tartan(&[])]);
    assert_eq!(outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillTartanNoSett]);
    assert_eq!(threads(&outcome), [OWN.color, Rgb::new(0x10, 0x10, 0x10)]);
    let outcome = planned(vec![tartan(&[("tartan", r#"{"palette": "(#ff0000)5 (cloth)5"}"#)])]);
    assert_eq!(outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillTartanColourUnreadable]);
    assert_eq!(threads(&outcome), [OWN.color, RED]);
}

#[test]
fn rows_too_many_for_the_work_budget_end_the_element_with_sc_e0004() {
    let design = Design::new(vec![tartan(&[("tartan", RED_BLUE)])], DesignSettings::default()).unwrap();
    let outcome = stitchcraft_engine::plan(&design, REFERENCE, &Budget { max_work: 20_000, ..Budget::DEFAULT });
    assert!(outcome.diagnostics.iter().any(|d| d.code == Code::BudgetExhausted), "{:?}", messages(&outcome));
}
