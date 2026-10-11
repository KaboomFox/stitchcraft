//! The rows of a tatami fill (`REQ-FILL-TAT-002`, `REQ-FILL-TAT-010`): at the fill's angle, a whole number
//! of row spacings from the design's origin, graded towards the end row spacing when one is set, and cut
//! where they meet the outline, as Ink/Stitch lays them.
//!
//! The rows are laid before they are routed and sewn (`tests/fill_routing.rs`), and their settings are
//! read and checked.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, ElementId, Point, math};
use stitchcraft_engine::design::{Element, FillRule, Path, Segment, Shape, Subpath};
use stitchcraft_engine::generators::tatami::TatamiParams;
use stitchcraft_engine::generators::tatami::rows::{Grid, Row, rows};
use stitchcraft_engine::normalize::region::{Polygon, build};
use stitchcraft_params::ParamSet;
use stitchcraft_testkit::designs::{RED, line, messages, p, planned};

/// The region's one part, of the closed polygon through `corners`.
fn part(corners: &[(f64, f64)]) -> Polygon {
    let (&(x, y), rest) = corners.split_first().unwrap();
    let path =
        Path { subpaths: vec![Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: true }] };
    build(&path, FillRule::NonZero, &mut Budget::DEFAULT.meter()).unwrap().region.parts.remove(0)
}

fn laid(part: &Polygon, grid: Grid) -> Vec<Row> {
    rows(part, &grid, &mut Budget::DEFAULT.meter()).unwrap()
}

/// A regular polygon of 60 corners round (`cx`, `cy`), as near a circle of `radius` as rows need.
fn round(cx: f64, cy: f64, radius: f64) -> Vec<(f64, f64)> {
    (0..60)
        .map(|k| {
            let (sin, cos) = math::sin_cos(f64::from(k) * std::f64::consts::TAU / 60.0);
            (cx + radius * cos, cy + radius * sin)
        })
        .collect()
}

#[test]
fn req_fill_tat_002_rows_are_the_row_spacing_apart_and_a_whole_number_of_spacings_from_the_origin() {
    // A disc at 30°, rows 0.4 apart: each row lies a whole number of spacings from the origin, measured
    // across the rows, and every segment's ends lie on its row.
    let disc = part(&round(12.3, 7.9, 5.0));
    let found = laid(&disc, Grid { angle: 30.0, spacing: 0.4, end_spacing: None });
    assert_eq!(found.len(), 25);
    let across = |q: Point| q.x() * 0.5 + q.y() * 3f64.sqrt() / 2.0;
    for (row, next) in found.iter().zip(&found[1..]) {
        assert!((next.across - row.across - 0.4).abs() < 1e-9);
    }
    for row in &found {
        assert!(((row.across / 0.4) - (row.across / 0.4).round()).abs() < 1e-9, "{}", row.across);
        for segment in &row.segments {
            assert!((across(segment.start) - row.across).abs() < 1e-9 && (across(segment.end) - row.across).abs() < 1e-9);
        }
    }
    // Two fills side by side at the same angle and spacing share their rows: where they meet, their rows
    // lie on the same lines.
    let left = laid(&part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]), Grid { angle: 15.0, spacing: 0.5, end_spacing: None });
    let right = laid(&part(&[(10.0, 0.0), (20.0, 0.0), (20.0, 10.0), (10.0, 10.0)]), Grid { angle: 15.0, spacing: 0.5, end_spacing: None });
    let shared = left.iter().filter(|row| right.iter().any(|other| (other.across - row.across).abs() < 1e-9)).count();
    assert!(shared > 10, "{shared}");
}

#[test]
fn req_fill_tat_002_graded_rows_change_from_the_row_spacing_to_the_end_spacing_over_the_height() {
    // A strip 20 mm tall, rows from 0.25 to 1 mm apart: the steps grow with the distance from the first
    // row, reach 1 mm at the height and stay there.
    let strip = part(&[(0.0, 0.0), (5.0, 0.0), (5.0, 20.0), (0.0, 20.0)]);
    let found = laid(&strip, Grid { angle: 0.0, spacing: 0.25, end_spacing: Some(1.0) });
    let steps: Vec<f64> = found.iter().zip(&found[1..]).map(|(row, next)| next.across - row.across).collect();
    assert!((steps[0] - 0.25).abs() < 1e-12);
    for (step, row) in steps.iter().zip(&found) {
        assert!((step - (0.25 + 0.75 * (row.across / 20.0).min(1.0))).abs() < 1e-9, "{step} after {}", row.across);
    }
    assert!(steps.windows(2).all(|pair| pair[1] > pair[0]), "growing");
    assert!(*steps.last().unwrap() > 0.9);
}

#[test]
fn req_fill_tat_010_rows_are_cut_where_they_meet_the_outline() {
    // A notch from the top down to the row at y = 5, its floor 2 mm wide: the row is 3 segments, the
    // floor among them, and the row along the top edge at y = 0 is one.
    let notch = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 10.0), (4.0, 10.0), (4.0, 5.0), (2.0, 5.0), (2.0, 10.0), (0.0, 10.0)]);
    let found = laid(&notch, Grid { angle: 0.0, spacing: 5.0, end_spacing: None });
    let ends = |row: &Row| row.segments.iter().map(|s| (s.start.x(), s.end.x())).collect::<Vec<_>>();
    assert_eq!(found.iter().map(|row| row.across).collect::<Vec<_>>(), [0.0, 5.0]);
    assert_eq!(ends(&found[0]), [(0.0, 6.0)]);
    assert_eq!(ends(&found[1]), [(0.0, 2.0), (2.0, 4.0), (4.0, 6.0)]);
    // A V notch whose tip touches the row: 2 segments meeting at the tip.
    let vee = part(&[(0.0, 0.0), (6.0, 0.0), (6.0, 10.0), (4.0, 10.0), (3.0, 5.0), (2.0, 10.0), (0.0, 10.0)]);
    assert_eq!(ends(&laid(&vee, Grid { angle: 0.0, spacing: 5.0, end_spacing: None })[1]), [(0.0, 3.0), (3.0, 6.0)]);
    // A triangle whose apex only touches a row: no segment there.
    let triangle = part(&[(3.0, 0.0), (6.0, 6.0), (0.0, 6.0)]);
    assert_eq!(laid(&triangle, Grid { angle: 0.0, spacing: 3.0, end_spacing: None }).iter().map(|row| row.across).collect::<Vec<_>>(), [3.0]);
}

#[test]
fn req_fill_tat_002_the_settings_are_ink_stitch_s() {
    let read = TatamiParams::from_set(&ParamSet::default()).unwrap().params;
    assert_eq!(read.grid(), Grid { angle: 0.0, spacing: 0.25, end_spacing: None });
    let set: ParamSet = [("angle", "-30"), ("row_spacing_mm", "0.4"), ("end_row_spacing_mm", "0.8")].into_iter().collect();
    assert_eq!(TatamiParams::from_set(&set).unwrap().params.grid(), Grid { angle: -30.0, spacing: 0.4, end_spacing: Some(0.8) });
    // An end row spacing of 0 is none, as Ink/Stitch reads it.
    let none: ParamSet = [("end_row_spacing_mm", "0")].into_iter().collect();
    assert_eq!(TatamiParams::from_set(&none).unwrap().params.end_row_spacing_mm, None);
}

#[test]
fn a_fill_s_settings_are_checked_before_fills_are_sewn() {
    let square = Path {
        subpaths: vec![Subpath {
            start: p(0.0, 0.0),
            segments: vec![Segment::Line(p(10.0, 0.0)), Segment::Line(p(10.0, 10.0)), Segment::Line(p(0.0, 10.0))],
            closed: true,
        }],
    };
    let params: ParamSet = [("row_spacing_mm", "0.05")].into_iter().collect();
    let fill = Element {
        id: ElementId::new("dense").unwrap(),
        name: None,
        shape: Shape::Fill { path: square, rule: FillRule::NonZero },
        thread: RED,
        params,
    };
    let said = messages(&planned(vec![fill, line("ok", (0.0, 40.0), 10.0, &RED, &[])]));
    assert!(said[0].starts_with("warning SC-W0102:") && said[0].contains("row_spacing_mm"), "{said:?}");
}
