//! Wide columns: a satin column wider than the widest satin column, its stitches split (roadmap step M13.54,
//! `REQ-AUTO-152` to `REQ-AUTO-156`). A part of an area, or a stroke, wider than 7 mm and no wider than
//! `auto_max_split_satin_width_mm` that is otherwise a good column sews as one satin column whose stitches are
//! split at 7 mm, at a random phase so that the needle points of neighbouring stitches don't line up. The
//! designs are drawn here in code: bars, rings and strokes of known widths.
//!
//! The setting is off by default, so the cases that need the rule raise it to 10 mm, and the cases without it
//! guard that every design sews as before (`docs/src/design/image-to-embroidery.md` › Wide columns). The
//! bound is compared within its tolerance, `WIDE_TOLERANCE`, which every case at the bound uses. A wide
//! column is a part's satin column, so the area under it keeps `auto_column_overlap_mm` there (`REQ-AUTO-157`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::auto::rules::WIDE_TOLERANCE;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Segment, Shape};
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, line, unbased};

/// The setting of the step: the widest part that sews as a satin column with split stitches, in millimetres.
const WIDEST_SPLIT: &str = "auto_max_split_satin_width_mm";
/// The longest satin stitch, in millimetres: the widest satin column, at which a wide column's stitches split.
const LONGEST: f64 = 7.0;

/// A bar 50 mm long and `width` mm wide, as an automatic area with `params`.
fn bar(width: f64, params: &[(&str, &str)]) -> Element {
    automatic(fill("bar", &[&[(0.0, 0.0), (50.0, 0.0), (50.0, width), (0.0, width)]], &BLUE, params))
}

/// A ring round (40, 40) whose band is `width` mm wide about a radius of 20 mm, drawn with 180 points a side,
/// as an automatic area with `params`.
fn ring(width: f64, params: &[(&str, &str)]) -> Element {
    let circle = |r: f64, turn: f64| -> Vec<(f64, f64)> {
        (0..180)
            .map(|k| {
                let (sin, cos) = stitchcraft_core::math::sin_cos(turn * f64::from(k) * std::f64::consts::TAU / 180.0);
                (40.0 + r * cos, 40.0 + r * sin)
            })
            .collect()
    };
    let (outer, inner) = (circle(20.0 + width / 2.0, 1.0), circle(20.0 - width / 2.0, -1.0));
    automatic(fill("ring", &[&outer, &inner], &RED, params))
}

/// An L of 2 bars `width` mm wide, each 40 mm long: a part of 2 centre lines.
fn corner(width: f64, params: &[(&str, &str)]) -> Element {
    let ring = [(0.0, 0.0), (40.0, 0.0), (40.0, width), (width, width), (width, 40.0), (0.0, 40.0)];
    automatic(fill("corner", &[&ring], &BLUE, params))
}

/// What `elements` sew as, digitized with no base layer, and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

/// Whether `element` is a satin column whose stitches split at the longest satin stitch, at a random phase.
fn split(element: &Element) -> bool {
    matches!(element.shape, Shape::Stroke { .. })
        && element.params.get("satin_column") == Some("true")
        && element.params.get("max_stitch_length_mm").and_then(|v| v.parse::<f64>().ok()) == Some(LONGEST)
        && element.params.get("random_split_phase") == Some("true")
}

/// What `SC-I0912` says of the shape `id`.
fn rule(said: &[Diagnostic], id: &str) -> String {
    said.iter()
        .filter(|d| d.code == Code::AutoRule && d.element.as_ref().is_some_and(|e| e.to_string().contains(id)))
        .map(|d| d.message.clone())
        .collect()
}

#[test]
fn req_auto_152_a_bar_8_5_mm_wide_sews_as_one_split_satin_column() {
    let (out, said) = sewn(vec![bar(8.5, &[(WIDEST_SPLIT, "10")])]);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(split(&out[0]), "{:?}", out[0]);
    assert!(rule(&said, "bar").contains("wide"), "{said:?}");
}

#[test]
fn req_auto_152_a_ring_as_wide_as_the_bound_sews_as_a_split_satin_column() {
    // A ring drawn 10 mm wide measures its longest stitch a few micrometres over 10 mm between its points:
    // its stitches are split, so the length isn't a reason to refuse it (REQ-AUTO-154).
    let (out, _) = sewn(vec![ring(10.0, &[(WIDEST_SPLIT, "10")])]);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(split(&out[0]), "{:?}", out[0]);
}

#[test]
fn req_auto_152_without_the_setting_a_bar_8_5_mm_wide_is_filled() {
    let (out, _) = sewn(vec![bar(8.5, &[])]);
    assert!(out.iter().all(|e| matches!(e.shape, Shape::Fill { .. })), "{out:?}");
}

#[test]
fn req_auto_152_a_bar_wider_than_the_bound_is_filled() {
    let (out, _) = sewn(vec![bar(10.5, &[(WIDEST_SPLIT, "10")])]);
    assert!(out.iter().all(|e| matches!(e.shape, Shape::Fill { .. })), "{out:?}");
}

#[test]
fn req_auto_152_the_bound_holds_within_its_tolerance() {
    let (at, _) = sewn(vec![bar(10.0 + WIDE_TOLERANCE, &[(WIDEST_SPLIT, "10")])]);
    assert!(at.len() == 1 && split(&at[0]), "{at:?}");
    let (over, _) = sewn(vec![bar(10.01 + WIDE_TOLERANCE, &[(WIDEST_SPLIT, "10")])]);
    assert!(over.iter().all(|e| matches!(e.shape, Shape::Fill { .. })), "{over:?}");
    // A stroke too.
    let stroke =
        |width: f64| stitchcraft_testkit::designs::widened(automatic(line("border", (0.0, 0.0), 50.0, &RED, &[(WIDEST_SPLIT, "10")])), width);
    let (at, _) = sewn(vec![stroke(10.0 + WIDE_TOLERANCE)]);
    let (over, _) = sewn(vec![stroke(10.01 + WIDE_TOLERANCE)]);
    assert!(at.len() == 1 && split(&at[0]) && over.iter().all(|e| matches!(e.shape, Shape::Fill { .. })), "{at:?} {over:?}");
}

#[test]
fn req_auto_153_a_wide_part_of_2_centre_lines_is_filled() {
    let (out, _) = sewn(vec![corner(8.5, &[(WIDEST_SPLIT, "10")])]);
    assert!(out.iter().all(|e| matches!(e.shape, Shape::Fill { .. })), "{out:?}");
}

#[test]
fn req_auto_155_the_needle_points_of_neighbouring_split_stitches_do_not_line_up() {
    use stitchcraft_testkit::designs::planned;
    let outcome = planned(vec![bar(8.5, &[(WIDEST_SPLIT, "10")])]);
    let plan = outcome.plan.unwrap();
    let points: Vec<(f64, f64)> = plan.blocks.iter().flat_map(|b| b.stitches.iter()).map(|s| (s.at.x(), s.at.y())).collect();
    // A split point lies inside the bar, more than 1 mm from both rails, between 2 points far across from it.
    let splits: Vec<f64> = points
        .windows(3)
        .filter(|w| w[1].1 > 1.0 && w[1].1 < 7.5 && (w[0].1 - w[1].1).abs() > 2.0 && (w[2].1 - w[1].1).abs() > 2.0)
        .map(|w| w[1].1)
        .collect();
    assert!(splits.len() > 100, "{} split points", splits.len());
    let lined = splits.windows(2).filter(|w| (w[0] - w[1]).abs() < 0.2).count();
    // Measured with the prototype, 2026-11-20: 6 to 9 % at a random phase, 98 % split in equal parts.
    assert!(lined * 5 <= splits.len(), "{lined} of {} split points line up with the next", splits.len());
}

#[test]
fn req_auto_156_a_stroke_8_5_mm_wide_sews_as_a_split_satin_column() {
    let stroke = automatic(line("border", (0.0, 0.0), 50.0, &RED, &[(WIDEST_SPLIT, "10")]));
    let (out, said) = sewn(vec![stitchcraft_testkit::designs::widened(stroke, 8.5)]);
    assert_eq!(out.len(), 1, "{out:?}");
    assert!(split(&out[0]), "{:?}", out[0]);
    assert!(rule(&said, "border").contains("wide"), "{said:?}");
}

#[test]
fn req_auto_156_without_the_setting_a_stroke_8_5_mm_wide_fills_what_it_paints() {
    let stroke = automatic(line("border", (0.0, 0.0), 50.0, &RED, &[]));
    let (out, _) = sewn(vec![stitchcraft_testkit::designs::widened(stroke, 8.5)]);
    assert!(out.iter().all(|e| matches!(e.shape, Shape::Fill { .. })), "{out:?}");
}

#[test]
fn req_auto_157_an_area_keeps_its_column_overlap_under_a_wide_column() {
    // A square 30 mm wide, and over its right edge a bar 8.5 mm wide and 50 mm long drawn after it, which
    // sews as a split satin column: the square is cut away 1 mm inside the bar's edge by default, and 0.4 mm
    // with the setting.
    let right = |kept: &[(&str, &str)]| {
        let mut params = vec![(WIDEST_SPLIT, "10")];
        params.extend_from_slice(kept);
        let square = automatic(fill("square", &[&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)]], &RED, &params));
        let over = automatic(fill("over", &[&[(25.0, -10.0), (33.5, -10.0), (33.5, 40.0), (25.0, 40.0)]], &BLUE, &[(WIDEST_SPLIT, "10")]));
        let (out, _) = sewn(vec![square, over]);
        assert!(out.iter().any(|element| element.id.as_str().contains("over") && split(element)), "{out:?}");
        let square = out.iter().find(|element| element.id.as_str().contains("square")).unwrap();
        let Shape::Fill { path, .. } = &square.shape else { panic!("{:?}", square.shape) };
        let ends = path.subpaths.iter().flat_map(|sub| {
            std::iter::once(sub.start).chain(sub.segments.iter().map(|segment| match segment {
                Segment::Line(to) | Segment::Quad(_, to) | Segment::Cubic(_, _, to) => *to,
            }))
        });
        (ends.map(|point| point.x()).fold(f64::NEG_INFINITY, f64::max) * 1000.0).round() / 1000.0
    };
    assert_eq!(right(&[]), 26.0);
    assert_eq!(right(&[("auto_column_overlap_mm", "0.4")]), 25.4);
}
