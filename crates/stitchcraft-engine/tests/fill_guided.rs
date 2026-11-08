//! A guided fill (milestone M10.9, `REQ-FILL-GDD-001` to `REQ-FILL-GDD-007`) in a plan: its settings read, its
//! underlay before its rows, and what it sews without a guide. Where the copies lie, how they are staggered
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

#[test]
fn req_fill_gdd_001_a_strategy_not_sewn_yet_is_named_and_skipped() {
    for (id, name) in [("1", "parallel offset"), ("2", "buffer")] {
        let outcome = planned(vec![guided(Some(&SLANT), &[("guided_fill_strategy", id)]), line("after", (0.0, 30.0), 10.0, &RED, &[])]);
        assert_eq!(
            messages(&outcome),
            [format!(
                "warning SC-W0011: This element's fill method, `guided_fill`, with its strategy `{id}` ({name}), is not sewn by this version of \
                 StitchCraft yet, so it is skipped."
            )]
        );
    }
}
