//! A contour fill (`REQ-FILL-CON-001` to `REQ-FILL-CON-007`): rings that follow a fill's outline inward, a
//! row spacing apart, sewn as one path of running stitches as its strategy lays them, after the tatami
//! fill's underlay. The rings themselves, how a path is laid through them and how it is stitched are
//! tested in the generator's modules (`generators::contour`); these cases sew whole designs.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Code, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, fill, messages, p, planned};

/// A disc of `radius` round (`x`, 0), as a polygon of 128 corners.
fn disc(x: f64, radius: f64) -> Vec<(f64, f64)> {
    (0..128)
        .map(|k| {
            let (sin, cos) = stitchcraft_core::math::sin_cos(std::f64::consts::TAU * f64::from(k) / 128.0);
            (x + radius * cos, radius * sin)
        })
        .collect()
}

/// A contour fill of `rings` with `own` settings, rings 1 mm apart and no underlay unless `own` says
/// otherwise, planned alone.
fn contour(rings: &[&[(f64, f64)]], own: &[(&str, &str)]) -> PlanOutcome {
    let mut params = vec![("fill_method", "contour_fill"), ("row_spacing_mm", "1"), ("fill_underlay", "false")];
    params.extend_from_slice(own);
    planned(vec![fill("contour", rings, &RED, &params)])
}

/// The needle points `outcome` sews, its locks left out.
fn needles(outcome: &PlanOutcome) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock).map(|s| s.at).collect()
}

/// The codes `outcome` said.
fn codes(outcome: &PlanOutcome) -> Vec<Code> {
    outcome.diagnostics.iter().map(|d| d.code).collect()
}

#[test]
fn req_fill_con_001_a_disc_s_rings_lie_a_row_spacing_apart_down_to_its_middle() {
    // A disc 20 mm across round (30, 0), rings 1 mm apart: every needle point lies on a ring, a whole number
    // of millimetres in from the outline, and each ring from the outline in to 1 mm across is sewn.
    let sewn = contour(&[&disc(30.0, 10.0)], &[]);
    assert_eq!(codes(&sewn), Vec::new());
    let radii: Vec<f64> = needles(&sewn).iter().map(|q| q.distance(p(30.0, 0.0))).collect();
    let on_a_ring = radii.iter().filter(|&&r| (r - r.round()).abs() < 0.05).count();
    assert!(on_a_ring * 100 >= radii.len() * 95, "{on_a_ring} of {} needle points on a ring", radii.len());
    for ring in 1..=10 {
        assert!(radii.iter().any(|&r| (r - f64::from(ring)).abs() < 0.05), "no needle point on the ring {ring} mm round");
    }
}

#[test]
fn req_fill_con_001_rings_run_clockwise_as_drawn_or_the_other_way() {
    // The outline ring's needle points, in order, turn round the middle clockwise as the drawing shows it,
    // y down, unless `clockwise` is off.
    let turning = |own: &[(&str, &str)]| {
        let points: Vec<Point> =
            needles(&contour(&[&disc(30.0, 10.0)], own)).into_iter().filter(|q| (q.distance(p(30.0, 0.0)) - 10.0).abs() < 0.05).collect();
        let angle = |q: &Point| stitchcraft_core::math::atan2(q.y(), q.x() - 30.0);
        let turns: f64 = points
            .windows(2)
            .map(|w| (angle(&w[1]) - angle(&w[0]) + 3.0 * std::f64::consts::PI) % std::f64::consts::TAU - std::f64::consts::PI)
            .sum();
        turns
    };
    // In y-down coordinates, clockwise as drawn turns from +x towards +y: the angle grows.
    assert!(turning(&[]) > 5.0, "{}", turning(&[]));
    assert!(turning(&[("clockwise", "false")]) < -5.0, "{}", turning(&[("clockwise", "false")]));
}

#[test]
fn req_fill_con_002_inner_to_outer_starts_nearest_the_needle_and_ends_on_the_outline_a_row_spacing_short() {
    // The needle starts at the origin: the disc round (30, 0) starts at its point nearest it, (20, 0), and
    // ends on its outline a row spacing short of it, having sewn the middles first.
    let points = needles(&contour(&[&disc(30.0, 10.0)], &[]));
    let (first, last) = (points[0], *points.last().unwrap());
    assert!(first.distance(p(20.0, 0.0)) < 0.01, "{first:?}");
    assert!((last.distance(p(30.0, 0.0)) - 10.0).abs() < 0.05, "{last:?}");
    assert!((last.distance(first) - 1.0).abs() < 0.1, "{last:?} is a row spacing from {first:?} along the outline");
    // The needle steps straight in to the middle first: the first ring sewn round whole is the innermost.
    let middle = points.iter().position(|q| q.distance(p(30.0, 0.0)) < 1.5).unwrap();
    assert!(points[..middle].len() <= 12, "{middle} needle points before the middle");
}

#[test]
fn req_fill_con_003_a_smoothed_path_stays_inside_the_fill() {
    // A square with a square hole, smoothed by 1 mm: no needle point leaves the square or enters the hole.
    let outline = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];
    let hole = [(7.0, 7.0), (7.0, 13.0), (13.0, 13.0), (13.0, 7.0)];
    let sewn = contour(&[&outline, &hole], &[("smoothness_mm", "1")]);
    let points = needles(&sewn);
    let inside = |q: &Point| {
        let (x, y) = (q.x(), q.y());
        (-1e-6..=20.0 + 1e-6).contains(&x)
            && (-1e-6..=20.0 + 1e-6).contains(&y)
            && !(x > 7.0 + 1e-6 && x < 13.0 - 1e-6 && y > 7.0 + 1e-6 && y < 13.0 - 1e-6)
    };
    assert!(points.iter().all(inside), "{:?}", points.iter().filter(|q| !inside(q)).collect::<Vec<_>>());
    // Smoothed, the path differs from the rings.
    assert_ne!(points, needles(&contour(&[&outline, &hole], &[])));
}

#[test]
fn req_fill_con_004_a_single_spiral_ends_in_the_middle_and_a_double_one_near_where_it_began() {
    // A turn's points lie a 20th of the longest stitch apart, 0.2 mm, and the first is the one nearest the
    // needle.
    let single = needles(&contour(&[&disc(30.0, 10.0)], &[("contour_strategy", "1")]));
    assert!(single[0].distance(p(20.0, 0.0)) < 0.2, "{:?}", single[0]);
    assert!(single.last().unwrap().distance(p(30.0, 0.0)) < 1.5, "{:?}", single.last());
    // The double spiral turns in on every other ring and back out on the others, ending on the ring inside
    // the outline, near where it began.
    let double = needles(&contour(&[&disc(30.0, 10.0)], &[("contour_strategy", "2")]));
    let last = *double.last().unwrap();
    assert!((last.distance(p(30.0, 0.0)) - 9.0).abs() < 0.1 && last.distance(p(21.0, 0.0)) < 1.0, "{last:?}");
}

#[test]
fn req_fill_con_005_a_part_too_narrow_for_its_spiral_is_sewn_inner_to_outer() {
    // A strip 1.5 mm wide has no ring inside its outline to turn to.
    let strip = [(0.0, 0.0), (30.0, 0.0), (30.0, 1.5), (0.0, 1.5)];
    let spiral = contour(&[&strip], &[("contour_strategy", "1")]);
    assert_eq!(codes(&spiral), [Code::FillContourSpiralTooShort]);
    assert_eq!(needles(&spiral), needles(&contour(&[&strip], &[])));
}

#[test]
fn req_fill_con_006_repeats_double_the_path_and_bean_stitch_triples_each_stitch() {
    let disc = disc(30.0, 10.0);
    let count = |own: &[(&str, &str)]| needles(&contour(&[&disc], own)).len();
    let once = count(&[]);
    // Each repeat sews everything so far again the other way, the needle staying where it turns.
    assert_eq!(count(&[("repeats", "2")]), 2 * once - 1);
    assert_eq!(count(&[("repeats", "3")]), 4 * once - 3);
    assert_eq!(count(&[("bean_stitch_repeats", "1")]), 3 * once - 2);
    // Random lengths sew other needle points, and the seed decides which.
    let random = |seed: &str| needles(&contour(&[&disc], &[("enable_random_stitch_length", "true"), ("random_seed", seed)]));
    assert_ne!(random("1"), needles(&contour(&[&disc], &[])));
    assert_ne!(random("1"), random("2"));
    assert_eq!(random("1"), random("1"));
}

#[test]
fn req_fill_con_007_the_tatami_fill_s_underlay_comes_first_and_the_rings_go_on_from_it() {
    // With the default underlay, rows square to the fill's angle of 0, 3 mm apart, then the rings, in one
    // run: the only jump is the one to the fill's start, as without underlay.
    let jumps = |outcome: &PlanOutcome| outcome.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count();
    let sewn = contour(&[&disc(30.0, 10.0)], &[("fill_underlay", "true")]);
    assert_eq!(jumps(&sewn), 1);
    assert_eq!(jumps(&contour(&[&disc(30.0, 10.0)], &[])), 1);
    // The underlay's rows run up and down: stitch after stitch with x the same at both ends. A ring turns at
    // every needle point, so it has no 2 such stitches one after the other. After the underlay's last row
    // come the rings, every one of them: the needle points lie a whole number of millimetres from the
    // middle, but for the few of the travel from the underlay to the rings.
    let in_rows = |points: &[Point]| -> Vec<usize> {
        let upright: Vec<bool> = points.windows(2).map(|w| (w[1].x() - w[0].x()).abs() < 1e-9).collect();
        upright.windows(2).enumerate().filter(|(_, u)| u[0] && u[1]).map(|(i, _)| i).collect()
    };
    let points = needles(&sewn);
    let rows = in_rows(&points);
    let starts = rows.iter().enumerate().filter(|&(k, &i)| k == 0 || rows[k - 1] + 1 != i).count();
    let after = rows.last().unwrap() + 3;
    let radii: Vec<f64> = points[after..].iter().map(|q| q.distance(p(30.0, 0.0))).collect();
    let on_a_ring = radii.iter().filter(|&&r| (r - r.round()).abs() < 0.05).count();
    assert!(starts >= 5 && on_a_ring * 100 >= radii.len() * 95, "{starts} rows, then {on_a_ring} of {} points on a ring", radii.len());
    for ring in 1..=10 {
        assert!(radii.iter().any(|&r| (r - f64::from(ring)).abs() < 0.05), "the ring {ring} mm round is not sewn after the underlay");
    }
    let bare = needles(&contour(&[&disc(30.0, 10.0)], &[]));
    assert_eq!(in_rows(&bare), Vec::<usize>::new());
    assert!(points.len() > bare.len());
}
