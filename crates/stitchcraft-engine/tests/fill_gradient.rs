//! A linear gradient fill (`REQ-FILL-GRD-001` and `REQ-FILL-GRD-006` to `REQ-FILL-GRD-008`), sewn as whole
//! designs: the rows, each stop's row and the blends are tested in the generator's modules
//! (`generators::gradient`); these cases sew designs through every stage, with what is said of them.
//!
//! Each case fills a square from 0 to 20 mm both ways, under a gradient from red to blue along x from 0 to
//! 20 mm unless it says otherwise.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Code, Point, Severity};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Element, Gradient, GradientStop, Shape};
use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::{Rgb, Role, StitchKind, Thread};
use stitchcraft_testkit::designs::{fill, messages, p, planned};

/// The element's own thread.
const OWN: Thread = Thread::new(Rgb::new(0, 0, 0));
const RED: Rgb = Rgb::new(255, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 255);
/// The square every case fills.
const SQUARE: [(f64, f64); 4] = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];

/// A gradient from `start` to `end` through `stops`, each an offset, a colour and an opacity.
fn gradient(start: (f64, f64), end: (f64, f64), stops: &[(f64, Rgb, f64)]) -> Gradient {
    let stops = stops.iter().map(|&(offset, colour, opacity)| GradientStop { offset, colour, opacity }).collect();
    Gradient { start: p(start.0, start.1), end: p(end.0, end.1), stops }
}

/// Red to blue along x across the square.
fn red_blue() -> Gradient {
    gradient((0.0, 0.0), (20.0, 0.0), &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)])
}

/// A linear gradient fill of `parts` in `thread`, painted with `painted`, with `params` and a trim after it.
fn graded(parts: &[&[(f64, f64)]], thread: &Thread, painted: Option<Gradient>, params: &[(&str, &str)]) -> Element {
    let method = [("fill_method", "linear_gradient_fill"), ("trim_after", "true")];
    let mut element = fill("gradient", parts, thread, &[&method[..], params].concat());
    if let Shape::Fill { gradient, .. } = &mut element.shape {
        *gradient = painted;
    }
    element
}

/// Each colour block's thread and the needle points of its stitches of `role`.
fn blocks(outcome: &PlanOutcome, role: Role) -> Vec<(Rgb, Vec<Point>)> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    plan.blocks
        .iter()
        .map(|block| {
            (block.thread.color, block.stitches.iter().filter(|s| s.kind == StitchKind::Normal && s.origin.role == role).map(|s| s.at).collect())
        })
        .collect()
}

/// The colours of the blocks, in order.
fn threads(outcome: &PlanOutcome) -> Vec<Rgb> {
    blocks(outcome, Role::Top).into_iter().map(|(colour, _)| colour).collect()
}

/// What `outcome` warns of, or says is wrong: its diagnostics but its notes.
fn warnings(outcome: &PlanOutcome) -> Vec<String> {
    outcome.diagnostics.iter().filter(|d| d.code.severity() != Severity::Info).map(ToString::to_string).collect()
}

/// How many of the stitches between `points` run level and how many upright, each longer than 1 mm.
fn level_and_upright(points: &[Point]) -> (usize, usize) {
    let long = |a: f64, b: f64| (a - b).abs() > 1.0;
    let flat = |a: f64, b: f64| (a - b).abs() < 1e-9;
    let level = points.windows(2).filter(|w| flat(w[0].y(), w[1].y()) && long(w[0].x(), w[1].x())).count();
    let upright = points.windows(2).filter(|w| flat(w[0].x(), w[1].x()) && long(w[0].y(), w[1].y())).count();
    (level, upright)
}

#[test]
fn req_fill_grd_001_a_fill_sews_the_gradient_its_shape_is_painted_with() {
    // Without underlay: red's rows on the left and blue's on the right, each colour a block of its own, in
    // rows that run down the canvas, square to the gradient.
    let outcome = planned(vec![graded(&[&SQUARE], &OWN, Some(red_blue()), &[("fill_underlay", "false")])]);
    assert!(warnings(&outcome).is_empty(), "{:?}", messages(&outcome));
    assert_eq!(threads(&outcome), [RED, BLUE]);
    let middle = |points: &[Point]| points.iter().map(|q| q.x()).sum::<f64>() / points.len() as f64;
    let sewn = blocks(&outcome, Role::Top);
    assert!(middle(&sewn[0].1) < 9.0 && middle(&sewn[1].1) > 11.0, "{} {}", middle(&sewn[0].1), middle(&sewn[1].1));
    // The travel between rows along the outline aside.
    for (colour, points) in &sewn {
        let (level, upright) = level_and_upright(points);
        assert!(upright > 50 && level < 5, "{colour}: {level} level, {upright} upright");
    }
    // Turned to run down the canvas, the rows run across it.
    let down = gradient((0.0, 0.0), (0.0, 20.0), &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    let outcome = planned(vec![graded(&[&SQUARE], &OWN, Some(down), &[("fill_underlay", "false")])]);
    let sewn = blocks(&outcome, Role::Top);
    let (level, upright) = level_and_upright(&sewn[0].1);
    assert!(level > 50 && upright < 5, "{level} level, {upright} upright");
    assert!(sewn[0].1.iter().map(|q| q.y()).sum::<f64>() / (sewn[0].1.len() as f64) < 9.0);
}

#[test]
fn diag_sc_w0329_a_fill_painted_without_a_gradient_fades_its_own_colour_out_along_its_angle() {
    let outcome = planned(vec![graded(&[&SQUARE], &OWN, None, &[("fill_underlay", "false")])]);
    assert_eq!(
        messages(&outcome),
        ["warning SC-W0329: This linear gradient fill's shape is painted without a linear gradient, so its own colour fades out along its \
          angle, its rows thinning out to none, as Ink/Stitch sews it."]
    );
    assert_eq!(outcome.diagnostics[0].element.as_ref().map(|id| id.as_str()), Some("gradient"));
    assert_eq!(threads(&outcome), [OWN.color]);
    // At the angle of 0, the rows run down the canvas, more of them on the left, where it starts, than on
    // the right.
    let points = &blocks(&outcome, Role::Top)[0].1;
    let left = points.iter().filter(|q| q.x() < 10.0).count();
    assert!(2 * left > 3 * (points.len() - left), "{left} of {}", points.len());
    assert_eq!(outcome.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillGradientNone]);
}

#[test]
fn req_fill_grd_006_the_underlay_sews_in_the_first_stop_s_colour_along_the_gradient() {
    // The underlay sews in red, the first stop's colour, in the block of red's rows, and runs along the
    // gradient: level, across the top's upright rows. A trim after each colour, none after the underlay.
    let outcome = planned(vec![graded(&[&SQUARE], &OWN, Some(red_blue()), &[])]);
    assert!(warnings(&outcome).is_empty(), "{:?}", messages(&outcome));
    assert_eq!(threads(&outcome), [RED, BLUE]);
    let plan = outcome.plan.as_ref().unwrap();
    let trimmed: Vec<bool> = plan.blocks.iter().map(|block| block.stitches.iter().any(|s| s.kind == StitchKind::Trim)).collect();
    assert_eq!(trimmed, [true, true]);
    let sewn = blocks(&outcome, Role::Top);
    let (level, _) = level_and_upright(&sewn[0].1);
    assert!(level > 20, "{level} level");
    assert!(level_and_upright(&sewn[1].1).0 < 5);
    // A gradient turned 45° turns the underlay with it, up the canvas to the right as the gradient runs,
    // where the rows run down it to the right.
    let slanted = gradient((0.0, 20.0), (20.0, 0.0), &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    let along = |params: &[(&str, &str)]| {
        let outcome = planned(vec![graded(&[&SQUARE], &OWN, Some(slanted.clone()), params)]);
        let red = blocks(&outcome, Role::Top).swap_remove(0).1;
        let long = |w: &&[Point]| w[0].distance(w[1]) > 1.0;
        let up = red.windows(2).filter(long).filter(|w| ((w[1].x() - w[0].x()) + (w[1].y() - w[0].y())).abs() < 1e-6).count();
        let down = red.windows(2).filter(long).filter(|w| ((w[1].x() - w[0].x()) - (w[1].y() - w[0].y())).abs() < 1e-6).count();
        (up, down)
    };
    let ((up, down), (bare_up, bare_down)) = (along(&[]), along(&[("fill_underlay", "false")]));
    assert!(up > 20 && bare_up == 0 && down > 50 && bare_down > 50, "{up} {down} {bare_up} {bare_down}");
}

#[test]
fn req_fill_grd_006_a_colour_of_the_element_s_own_sews_in_the_element_s_own_thread() {
    // A named thread, as a document's swatch or a test sheet gives an element: the blue rows sew in it, and
    // red, the underlay's colour too, in a thread of its colour alone. A machine file then writes blue as
    // one entry of its palette, where a named and an unnamed thread of one colour would be 2 threads.
    let navy = Thread::named(BLUE, "Navy");
    let plan = planned(vec![graded(&[&SQUARE], &navy, Some(red_blue()), &[])]).plan.unwrap();
    let threads: Vec<&Thread> = plan.blocks.iter().map(|block| &block.thread).collect();
    assert_eq!(threads, [&Thread::new(RED), &navy]);
    let entries = BROTHER_PEC.assign(plan.color_entries().iter().map(|entry| entry.thread)).unwrap().entries;
    assert_eq!(entries.len(), 2);
}

#[test]
fn req_fill_grd_007_stop_at_ending_point_keeps_the_travel_of_the_last_colours_to_the_end() {
    // The next element lies beyond the square's bottom left corner: with the setting, blue, the last
    // colour, travels on to the corner, where the next element is nearest; without it, blue ends with its
    // last row, on the right.
    let next = fill("next", &[&[(-20.0, 25.0), (-10.0, 25.0), (-10.0, 35.0), (-20.0, 35.0)]], &OWN, &[]);
    let last = |params: &[(&str, &str)]| {
        let outcome = planned(vec![graded(&[&SQUARE], &OWN, Some(red_blue()), &[&[("fill_underlay", "false")], params].concat()), next.clone()]);
        let blue = blocks(&outcome, Role::Top).into_iter().find(|(colour, _)| *colour == BLUE).unwrap().1;
        *blue.last().unwrap()
    };
    let stopped = last(&[("stop_at_ending_point", "true")]);
    let free = last(&[]);
    assert!(stopped.distance(p(0.0, 20.0)) < 0.5 && free.distance(p(0.0, 20.0)) > 1.0, "{stopped:?} {free:?}");
}

#[test]
fn req_fill_grd_008_a_gradient_in_parts_sews_each_thread_once() {
    let parts: [&[(f64, f64)]; 2] = [&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], &[(30.0, 0.0), (50.0, 0.0), (50.0, 20.0), (30.0, 20.0)]];
    let across = gradient((0.0, 0.0), (50.0, 0.0), &[(0.0, RED, 1.0), (1.0, BLUE, 1.0)]);
    let outcome = planned(vec![graded(&parts, &OWN, Some(across), &[])]);
    // Both parts' underlays and red rows, then both parts' blue.
    assert_eq!(threads(&outcome), [RED, BLUE]);
    for (colour, points) in blocks(&outcome, Role::Top) {
        assert!(points.iter().any(|q| q.x() < 20.0) && points.iter().any(|q| q.x() > 30.0), "{colour}");
    }
}

#[test]
fn req_asm_008_a_gradient_sewn_in_several_threads_joins_no_run_of_underlays_first() {
    // A gradient and a square fill beside it in red, both setting `underlays_first`: the gradient sews in red
    // and blue, so it joins no run, and the fill is left alone, each sewn as without the setting.
    let first = ("underlays_first", "true");
    let beside = |more: &[(&str, &str)]| fill("beside", &[&[(25.0, 0.0), (45.0, 0.0), (45.0, 20.0), (25.0, 20.0)]], &Thread::new(RED), more);
    let set = planned(vec![graded(&[&SQUARE], &OWN, Some(red_blue()), &[first]), beside(&[first])]);
    let unset = planned(vec![graded(&[&SQUARE], &OWN, Some(red_blue()), &[]), beside(&[])]);
    assert_eq!(set.plan, unset.plan);
    let notes: Vec<(&str, bool)> = set
        .diagnostics
        .iter()
        .filter(|d| d.code == Code::UnderlaysFirstAlone)
        .map(|d| (d.element.as_ref().map_or("", |id| id.as_str()), d.message.contains("several threads")))
        .collect();
    assert_eq!(notes, [("gradient", true), ("beside", false)], "{:?}", messages(&set));
}
