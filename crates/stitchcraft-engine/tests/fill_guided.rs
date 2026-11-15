//! A guided fill (milestones M10.9 and M10.10, `REQ-FILL-GDD-001` to `REQ-FILL-GDD-010`) in a plan: its
//! settings read, its underlay before its rows, each strategy's rows, and what it sews without a guide. Where the copies lie, how they are staggered
//! and cut, and what the fallbacks say are tested in the generator's modules (`generators::guided`).

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Code, Mm, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Element, Guide, Join};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Role, StitchKind};
use stitchcraft_testkit::designs::{RED, fill, line, messages, planned, polylines};

/// A square 20 mm a side.
const SQUARE: [(f64, f64); 4] = [(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)];

/// A guided fill of the square with `own` settings, guided by the polyline `guide`, or by none.
fn guided(guide: Option<&[(f64, f64)]>, own: &[(&str, &str)]) -> Element {
    let mut params = vec![("fill_method", "guided_fill")];
    params.extend_from_slice(own);
    let mut element = fill("leaf", &[&SQUARE], &RED, &params);
    element.hints.guide = guide.map(|points| Guide { path: polylines(&[points]), width: Mm::ZERO, join: Join::UNSET, params: ParamSet::new() });
    element
}

/// The needle points of `role` that `outcome` sews, its locks left out.
fn sewn(outcome: &PlanOutcome, role: Role) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role == role).map(|s| s.at).collect()
}

/// The codes `outcome` said.
fn codes(outcome: &PlanOutcome) -> Vec<Code> {
    outcome.diagnostics.iter().map(|d| d.code).collect()
}

/// How long the stitches through `points` are, in all.
fn thread(points: &[Point]) -> f64 {
    points.windows(2).map(|w| w[0].distance(w[1])).sum()
}

/// A straight guide at 45° down to the right across the square, from its top left corner.
const SLANT: [(f64, f64); 2] = [(0.0, 0.0), (20.0, 20.0)];

#[test]
fn req_fill_gdd_007_each_part_sews_the_tatami_fill_s_underlay_and_then_rows_along_the_guide() {
    // The default underlay's rows run up and down the square, square to the fill's angle of 0, and then the
    // rows run along the guide, at 45°, in one run with no jump between.
    let outcome = planned(vec![guided(Some(&SLANT), &[])]);
    assert_eq!(codes(&outcome), []);
    let points = sewn(&outcome, Role::Top);
    let upright: Vec<bool> = points.windows(2).map(|w| (w[1].x() - w[0].x()).abs() < 1e-9).collect();
    let last_row = upright.windows(2).rposition(|u| u[0] && u[1]).unwrap();
    assert!(last_row > 10 && last_row < points.len() / 4, "rows to {last_row} of {}", points.len());
    let rows = &points[last_row + 2..];
    let along: f64 = rows.windows(2).filter(|w| ((w[1].x() - w[0].x()) - (w[1].y() - w[0].y())).abs() < 1e-6).map(|w| w[0].distance(w[1])).sum();
    assert!(along > 0.9 * thread(rows), "{along} of {}", thread(rows));
    let jumps = outcome.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count();
    assert_eq!(jumps, 1);
}

#[test]
fn req_fill_gdd_003_guided_fill_angle_moves_the_copies_its_way() {
    // Moved down the page rather than square to the guide, the copies of a guide at 45° lie 0.71 row
    // spacings apart: 1.41 times the rows, and as much thread.
    let square = planned(vec![guided(Some(&SLANT), &[("fill_underlay", "false")])]);
    let down = planned(vec![guided(Some(&SLANT), &[("fill_underlay", "false"), ("guided_fill_angle", "90")])]);
    let ratio = thread(&sewn(&down, Role::Top)) / thread(&sewn(&square, Role::Top));
    assert!((ratio - std::f64::consts::SQRT_2).abs() < 0.05, "{ratio}");
    // An empty angle is no angle.
    let empty = planned(vec![guided(Some(&SLANT), &[("fill_underlay", "false"), ("guided_fill_angle", "")])]);
    assert_eq!(sewn(&empty, Role::Top), sewn(&square, Role::Top));
}

#[test]
fn req_fill_gdd_005_stitch_position_method_and_smoothness_change_the_rows() {
    let plain = planned(vec![guided(Some(&SLANT), &[("fill_underlay", "false")])]);
    let stagger = planned(vec![guided(Some(&SLANT), &[("fill_underlay", "false"), ("stitch_position_method", "flexible_stagger")])]);
    assert_eq!(codes(&stagger), []);
    assert_ne!(sewn(&stagger, Role::Top), sewn(&plain, Role::Top));
    // A zigzag guide, smoothed by 2 mm: its copies' corners are rounded off, and no stitch of the rows turns
    // as sharply as the zigzag does, at 90°.
    let zigzag = [(0.0, 10.0), (5.0, 5.0), (10.0, 10.0), (15.0, 5.0), (20.0, 10.0)];
    let turns = |outcome: &PlanOutcome| {
        let top = sewn(outcome, Role::Top);
        top.windows(3)
            .filter(|w| {
                let (a, b) = ((w[1].x() - w[0].x(), w[1].y() - w[0].y()), (w[2].x() - w[1].x(), w[2].y() - w[1].y()));
                let dot = a.0 * b.0 + a.1 * b.1;
                w[0].distance(w[1]) > 0.5 && w[1].distance(w[2]) > 0.5 && dot.abs() < 0.1 * w[0].distance(w[1]) * w[1].distance(w[2])
            })
            .count()
    };
    let sharp = planned(vec![guided(Some(&zigzag), &[("fill_underlay", "false"), ("row_spacing_mm", "1")])]);
    let smooth = planned(vec![guided(Some(&zigzag), &[("fill_underlay", "false"), ("row_spacing_mm", "1"), ("smoothness_mm", "2")])]);
    assert!(turns(&sharp) > 20 && turns(&smooth) * 10 < turns(&sharp), "{} {}", turns(&sharp), turns(&smooth));
}

#[test]
fn req_fill_gdd_001_without_a_guide_line_the_fill_sews_its_own_tatami_fill() {
    let without = planned(vec![guided(None, &[("angle", "30")])]);
    let tatami = planned(vec![fill("leaf", &[&SQUARE], &RED, &[("angle", "30")])]);
    assert_eq!(codes(&without), [Code::FillGuidedNoGuide]);
    assert_eq!(sewn(&without, Role::Top), sewn(&tatami, Role::Top));
    assert_eq!(sewn(&without, Role::Underlay), sewn(&tatami, Role::Underlay));
    // A setting the tatami fill reads is said once, though both read it.
    let clamped = planned(vec![guided(None, &[("row_spacing_mm", "0.01")])]);
    assert_eq!(codes(&clamped).iter().filter(|code| **code == Code::ParamClamped).count(), 1, "{:?}", messages(&clamped));
}

/// How far `q` lies from the line through `line`.
fn away(q: Point, line: &[(f64, f64)]) -> f64 {
    let at = |(x, y): (f64, f64)| Point::new(x, y).unwrap();
    line.windows(2)
        .map(|side| {
            let (a, b) = (at(side[0]), at(side[1]));
            let along = ((q.x() - a.x()) * (b.x() - a.x()) + (q.y() - a.y()) * (b.y() - a.y())) / (a.distance(b) * a.distance(b));
            q.distance(a.lerp(b, along.clamp(0.0, 1.0)))
        })
        .fold(f64::INFINITY, f64::min)
}

/// The share of `points` that lie within 0.03 mm of `first` plus a whole number of millimetres from `line`.
fn on_rows(points: &[Point], line: &[(f64, f64)], first: f64) -> f64 {
    let on = points.iter().filter(|q| ((away(**q, line) - first).rem_euclid(1.0) - 0.5).abs() > 0.47).count();
    on as f64 / points.len() as f64
}

#[test]
fn req_fill_gdd_001_each_strategy_sews_its_rows_without_a_word() {
    let mut sewn_by = Vec::new();
    for id in ["0", "1", "2"] {
        let outcome = planned(vec![guided(Some(&SLANT), &[("guided_fill_strategy", id)])]);
        assert_eq!(codes(&outcome), [], "{id}: {:?}", messages(&outcome));
        let top = sewn(&outcome, Role::Top);
        assert!(thread(&top) > 1_500.0, "{id}: {} mm", thread(&top));
        sewn_by.push(top);
    }
    // A straight guide's offset curves are its copies, but for the ten-thousandth of a CSS pixel that row 0
    // lies off it, and a buffer's rings lie half a row spacing from both.
    assert!(sewn_by[0].len().abs_diff(sewn_by[1].len()) < 20, "{} and {}", sewn_by[0].len(), sewn_by[1].len());
    assert_ne!(sewn_by[1], sewn_by[2]);
    // A strategy that isn't one is refused, as any setting that can't be read.
    let outcome = planned(vec![guided(Some(&SLANT), &[("guided_fill_strategy", "3")]), line("after", (0.0, 30.0), 10.0, &RED, &[])]);
    assert_eq!(codes(&outcome), [Code::ParamInvalid]);
}

/// A square ring 4 mm a side in the middle of the square, drawn closed.
const RING: [(f64, f64); 5] = [(8.0, 8.0), (12.0, 8.0), (12.0, 12.0), (8.0, 12.0), (8.0, 8.0)];
/// Rows 1 mm apart, without underlay.
const APART: [(&str, &str); 2] = [("row_spacing_mm", "1"), ("fill_underlay", "false")];

#[test]
fn req_fill_gdd_009_parallel_offset_s_rows_keep_their_distance_from_the_guide() {
    // Round a closed guide, each row lies a whole number of row spacings from it, inside it and outside, round
    // its corners too. Copies of it, opened and moved across the square, don't.
    let strategy = |id: &'static str| [APART[0], APART[1], ("guided_fill_strategy", id)];
    let offset = planned(vec![guided(Some(&RING), &strategy("1"))]);
    assert_eq!(codes(&offset), [], "{:?}", messages(&offset));
    let rows = sewn(&offset, Role::Top);
    let copied = sewn(&planned(vec![guided(Some(&RING), &strategy("0"))]), Role::Top);
    // The needle's travel between the rows lies where it may, so not every needle point is on a row.
    assert!(rows.len() > 150 && on_rows(&rows, &RING, 0.0) > 0.6, "{} of {}", on_rows(&rows, &RING, 0.0), rows.len());
    assert!(on_rows(&copied, &RING, 0.0) < 0.3, "{}", on_rows(&copied, &RING, 0.0));
    // The rings inside the square are joined into the rows that leave it: one run, without a jump after the
    // one that reaches the fill.
    let jumps = offset.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count();
    assert_eq!(jumps, 1);
}

#[test]
fn req_fill_gdd_010_a_buffer_s_rows_are_rings_half_a_row_spacing_and_whole_ones_from_the_guide() {
    // Round a short line inside the square, and round 2 of them, every subpath of the guide.
    let dash = [(8.0, 10.0), (12.0, 10.0)];
    let buffered = [APART[0], APART[1], ("guided_fill_strategy", "2")];
    let outcome = planned(vec![guided(Some(&dash), &buffered)]);
    assert_eq!(codes(&outcome), [], "{:?}", messages(&outcome));
    let rings = sewn(&outcome, Role::Top);
    assert!(rings.len() > 150 && on_rows(&rings, &dash, 0.5) > 0.5, "{} of {}", on_rows(&rings, &dash, 0.5), rings.len());
    assert!(on_rows(&rings, &dash, 0.0) < 0.3, "{}", on_rows(&rings, &dash, 0.0));
    let jumps = outcome.plan.as_ref().unwrap().stitches().filter(|s| s.kind == StitchKind::Jump).count();
    assert_eq!(jumps, 1);
    let mut two = guided(Some(&dash), &buffered);
    let second = [(8.0, 15.0), (12.0, 15.0)];
    two.hints.guide.as_mut().unwrap().path = polylines(&[&dash, &second]);
    let both = sewn(&planned(vec![two]), Role::Top);
    let nearer = |q: &Point| away(*q, &dash).min(away(*q, &second));
    let on = both.iter().filter(|q| ((nearer(q) - 0.5).rem_euclid(1.0) - 0.5).abs() > 0.47).count();
    assert!(on * 2 > both.len(), "{on} of {}", both.len());
    // The copy and parallel offset strategies follow the first subpath alone.
    let mut first_only = guided(Some(&dash), &[APART[0], APART[1], ("guided_fill_strategy", "1")]);
    let one = sewn(&planned(vec![first_only.clone()]), Role::Top);
    first_only.hints.guide.as_mut().unwrap().path = polylines(&[&dash, &second]);
    assert_eq!(sewn(&planned(vec![first_only]), Role::Top), one);
}
