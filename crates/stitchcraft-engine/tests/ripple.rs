//! The ripple stitch in a plan, through the engine's public API: its target point and what it says
//! (`REQ-RIP-002`, `REQ-RIP-010`, `SC-W0406` to `SC-W0408`). Its unit cases are in
//! `src/generators/ripple/tests.rs`.

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::Point;
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Element, Hints, Path};
use stitchcraft_plan::{Role, Stitch, StitchKind};
use stitchcraft_testkit::designs::{RED, along, clipped, messages, p, planned, polylines};

/// A ripple stitch `id` along `path`, with the settings `own`.
fn ripple(path: Path, own: &[(&str, &str)]) -> Element {
    along("ripple", path, &RED, &[own, &[("stroke_method", "ripple_stitch")]].concat())
}

/// A line 20 mm long along x.
fn line() -> Path {
    polylines(&[&[(-10.0, 0.0), (10.0, 0.0)]])
}

/// A column 20 mm long along x and 4 mm wide, with a rung across its middle.
fn column() -> Path {
    polylines(&[&[(-10.0, 0.0), (10.0, 0.0)], &[(-10.0, 4.0), (10.0, 4.0)], &[(0.0, -0.5), (0.0, 4.5)]])
}

/// The needle points `outcome`'s plan sews, locks left out.
fn sewn(outcome: &PlanOutcome) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    let mine = |s: &&Stitch| s.kind == StitchKind::Normal && s.origin.role != Role::Lock;
    plan.stitches().filter(mine).map(|s| s.at).collect()
}

#[test]
fn req_rip_002_a_ripple_aims_at_the_target_point_its_design_gives() {
    // Without one, the line shrinks to its middle. With one, as Ink/Stitch's command or VectorCraft's marker
    // gives it, to that, and the ripple's last line sews it.
    let to = |target: Option<Point>| {
        let element = Element { hints: Hints { target, ..Hints::default() }, ..ripple(line(), &[("line_count", "3"), ("staggers", "0")]) };
        let outcome = planned(vec![element]);
        assert_eq!(messages(&outcome), Vec::<String>::new());
        sewn(&outcome)
    };
    assert!(to(None).iter().all(|q| q.y() == 0.0));
    let aimed = to(Some(p(0.0, 8.0)));
    assert!(aimed.iter().any(|q| q.distance(p(0.0, 8.0)) < 1e-6) && aimed.iter().any(|q| q.distance(p(0.0, 4.0)) < 1e-6), "{aimed:?}");
}

#[test]
fn req_rip_010_the_settings_of_guided_ripples_are_kept_and_named() {
    let outcome = planned(vec![ripple(line(), &[("grid_size_mm", "2"), ("rotate_ripples", "false")])]);
    let named = |key: &str| format!("warning SC-W0105: `{key}` is not a StitchCraft parameter; it was kept but does not change the stitches.");
    assert_eq!(messages(&outcome), [named("grid_size_mm"), named("rotate_ripples")]);
    assert_eq!(sewn(&outcome), sewn(&planned(vec![ripple(line(), &[])])));
}

#[test]
fn diag_sc_w0406_a_ripple_cut_to_clips_is_sewn_whole() {
    let whole = planned(vec![ripple(line(), &[])]);
    let cut = planned(vec![clipped(ripple(line(), &[]), &[&[(-5.0, -5.0), (5.0, -5.0), (5.0, 5.0), (-5.0, 5.0)]])]);
    let message =
        "warning SC-W0406: This ripple stitch is cut to clip paths, which StitchCraft doesn't apply to ripple stitch yet, so it is sewn whole.";
    assert_eq!(messages(&cut), [message]);
    assert_eq!(sewn(&cut), sewn(&whole));
}

#[test]
fn diag_sc_w0406_a_ripple_along_a_column_reads_its_rails_with_their_defaults() {
    let outcome = planned(vec![ripple(column(), &[("swap_satin_rails", "true")])]);
    let message = "warning SC-W0406: This ripple stitch sets `swap_satin_rails`, which StitchCraft doesn't apply to ripple stitch yet, so its column is read with their defaults.";
    assert_eq!(messages(&outcome), [message]);
    assert_eq!(sewn(&outcome), sewn(&planned(vec![ripple(column(), &[])])));
}

#[test]
fn diag_sc_w0407_a_ripple_of_more_than_1000_lines_is_cut_to_1000() {
    // Rails 20 mm apart, the lines at least 0.01 mm apart: 2,000 lines, cut to 1,000.
    let tall = polylines(&[&[(-10.0, 0.0), (10.0, 0.0)], &[(-10.0, 20.0), (10.0, 20.0)]]);
    let outcome = planned(vec![ripple(tall, &[("min_line_dist_mm", "0.01")])]);
    let message = "warning SC-W0407: `min_line_dist_mm` gives this ripple stitch 2000 lines, more than 1000, so it sews 1000.";
    assert_eq!(messages(&outcome).first().map(String::as_str), Some(message), "{:?}", messages(&outcome));
}

#[test]
fn diag_sc_w0408_skips_that_would_leave_no_line_leave_none_out() {
    let outcome = planned(vec![ripple(column(), &[("line_count", "4"), ("skip_start", "3"), ("skip_end", "1")])]);
    let message = "warning SC-W0408: `skip_start` and `skip_end` leave out 3 and 1 of this ripple stitch's 4 lines, which would leave none, so no line is left out.";
    assert_eq!(messages(&outcome), [message]);
    assert_eq!(sewn(&outcome), sewn(&planned(vec![ripple(column(), &[("line_count", "4")])])));
}
