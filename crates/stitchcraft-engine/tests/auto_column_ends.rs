//! A satin column's ends stay clean (`REQ-AUTO-177`, `REQ-AUTO-178`, M13.60): a column's stitches stay square to
//! it up to its end, and where the end is slanted or curved they get shorter into the corner instead of fanning;
//! a line that turns sharply sews mitred corners instead of stitches piled at the inside of the corner.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › A column's ends. Each case draws its shapes in code, after
//! the review set's drawings: a band whose ends a circle cuts (the sunrise's 5 mm sea band), a band with one end
//! slanted, a bent band with square ends and a zigzag line. Before M13.60 the cut band's stitches leaned up to 37°
//! along it within 5 mm of each end, the slanted end's up to 32°, and the zigzag's corners gave 6 `SC-W0208`.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::Code;
use stitchcraft_core::math::{atan2, hypot, sin_cos, to_degrees};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::Element;
use stitchcraft_plan::Role;
use stitchcraft_testkit::designs::{RED, along, automatic, fill, planned, polylines, widened};
use stitchcraft_testkit::fabric;

type Pt = (f64, f64);

/// An automatic red area of the ring `ring`.
fn area(id: &str, ring: &[Pt]) -> Element {
    automatic(fill(id, &[ring], &RED, &[]))
}

/// The satin stitches of the elements whose ids start with `prefix`, as (from, to): each stitch that crosses the
/// column's middle, which `side` tells (its sign changes there), to a needle point within 1 mm of the one 2
/// before it, as the top's stitches lie side by side and an underlay's zigzag doesn't. A column's underlay is
/// tagged as its top (`REQ-AUTO-120` tags only lettering's), so its stitches are told apart by where they lie.
fn stitches_of(outcome: &PlanOutcome, prefix: &str, side: impl Fn(Pt) -> f64) -> Vec<(Pt, Pt)> {
    let plan = outcome.plan.as_ref().unwrap();
    let points: Vec<Pt> = plan
        .sewn_stitches()
        .into_iter()
        .filter(|s| s.role == Role::Top && !s.is_lock())
        .filter(|s| {
            let id = plan.blocks[s.block].stitches[s.index].origin.element.map(|e| plan.elements[e.index()].to_string());
            id.is_some_and(|id| id.starts_with(prefix))
        })
        .map(|s| (s.to.x(), s.to.y()))
        .collect();
    (2..points.len())
        .filter(|&i| side(points[i - 1]) * side(points[i]) < 0.0 && hypot(points[i].0 - points[i - 2].0, points[i].1 - points[i - 2].1) < 1.0)
        .map(|i| (points[i - 1], points[i]))
        .collect()
}

/// Where a band along x lies across, as its satin stitches' needle points reach: the middle between its
/// lowest and highest point.
fn middle_y(outcome: &PlanOutcome, prefix: &str) -> f64 {
    let plan = outcome.plan.as_ref().unwrap();
    let ys: Vec<f64> = plan
        .sewn_stitches()
        .into_iter()
        .filter(|s| {
            let id = plan.blocks[s.block].stitches[s.index].origin.element.map(|e| plan.elements[e.index()].to_string());
            id.is_some_and(|id| id.starts_with(prefix))
        })
        .map(|s| s.to.y())
        .collect();
    let (low, high) = ys.iter().fold((f64::MAX, f64::MIN), |(l, h), &y| (l.min(y), h.max(y)));
    f64::midpoint(low, high)
}

/// How far each stitch at least `shortest` long whose middle lies within `reach` of either end of the band (its
/// stitches' least and greatest x) leans from square to a band that runs along x, in degrees: the most.
fn most_lean_at_ends(stitches: &[(Pt, Pt)], reach: f64, shortest: f64) -> f64 {
    let xs = stitches.iter().flat_map(|&(a, b)| [a.0, b.0]);
    let (low, high) = xs.fold((f64::MAX, f64::MIN), |(l, h), x| (l.min(x), h.max(x)));
    stitches
        .iter()
        .filter(|&&(a, b)| {
            let middle = (a.0 + b.0) / 2.0;
            (middle - low < reach || high - middle < reach) && hypot(b.0 - a.0, b.1 - a.1) >= shortest
        })
        .map(|&(a, b)| to_degrees(atan2((b.0 - a.0).abs(), (b.1 - a.1).abs())))
        .fold(0.0, f64::max)
}

/// The points of the circle about `centre` of `radius` from angle `from` to `to` (radians), `n` steps.
fn arc(centre: Pt, radius: f64, (from, to): (f64, f64), n: usize) -> Vec<Pt> {
    (0..=n)
        .map(|k| {
            let (s, c) = sin_cos(from + (to - from) * k as f64 / n as f64);
            (centre.0 + radius * c, centre.1 + radius * s)
        })
        .collect()
}

/// A band 5 mm tall along x whose 2 ends a circle of radius 30 about (30, 18) cuts, as the sunrise's sea band is
/// cut by its round border: the band reaches 2 mm farther along its bottom than along its top at each end.
fn cut_band() -> Vec<Pt> {
    let (centre, radius) = ((30.0, 18.0), 30.0);
    // Where the circle crosses y = 0 and y = 5, on its left and its right.
    let angle = |y: f64, left: bool| {
        let x = (radius * radius - (y - centre.1) * (y - centre.1)).sqrt();
        atan2(y - centre.1, if left { -x } else { x })
    };
    let mut ring = arc(centre, radius, (angle(0.0, true), angle(5.0, true)), 8);
    ring.extend(arc(centre, radius, (angle(5.0, false), angle(0.0, false)), 8));
    ring
}

#[test]
fn req_auto_177_a_band_cut_by_a_circle_sews_square_to_its_ends() {
    let outcome = planned(vec![area("band", &cut_band())]);
    let middle = middle_y(&outcome, "band");
    let stitches = stitches_of(&outcome, "band", |p| p.1 - middle);
    assert!(stitches.len() > 100, "the band sews as a satin column: {} top stitches", stitches.len());
    // Measured before M13.60: 37° within 5 mm of an end, every stitch there leaning more than 20°.
    let lean = most_lean_at_ends(&stitches, 5.0, 1.5);
    assert!(lean <= 15.0, "the band's stitches stay square up to its ends, shorter into each corner: {lean:.1}°");
}

#[test]
fn req_auto_177_a_slanted_end_sews_square_and_shorter_into_its_point() {
    let outcome = planned(vec![area("slant", &[(5.0, 5.0), (45.0, 5.0), (50.0, 10.0), (5.0, 10.0)])]);
    let middle = middle_y(&outcome, "slant");
    let stitches = stitches_of(&outcome, "slant", |p| p.1 - middle);
    assert!(stitches.len() > 100, "the band sews as a satin column: {} top stitches", stitches.len());
    // Measured before M13.60: 32° at the slanted end, 2° at the square one.
    let lean = most_lean_at_ends(&stitches, 5.0, 1.5);
    assert!(lean <= 15.0, "the stitches stay square into the slanted end's point: {lean:.1}°");
}

#[test]
fn req_auto_177_a_bent_band_with_square_ends_keeps_its_stitches() {
    // Half a ring 3 mm wide, its ends cut along radii: square to the band where it ends, as a letter C's are.
    let mut ring = arc((30.0, 30.0), 13.0, (-1.2, 1.2), 24);
    ring.extend(arc((30.0, 30.0), 10.0, (1.2, -1.2), 24));
    let outcome = planned(vec![area("bend", &ring)]);
    // Each stitch at least 1.5 mm long points at the ring's centre within 12°: the ends are left as they are.
    let plan = outcome.plan.as_ref().unwrap();
    let centre = {
        let (low, high) = plan.sewn_stitches().iter().fold(((f64::MAX, f64::MAX), (f64::MIN, f64::MIN)), |(l, h), s| {
            ((l.0.min(s.to.x()), l.1.min(s.to.y())), (h.0.max(s.to.x()), h.1.max(s.to.y())))
        });
        // The ring's centre lies 13 mm left of the band's rightmost point, level with its middle.
        (high.0 - 13.0 - 0.4, f64::midpoint(low.1, high.1))
    };
    let stitches = stitches_of(&outcome, "bend", |p| hypot(p.0 - centre.0, p.1 - centre.1) - 11.5);
    assert!(stitches.len() > 50, "the band sews as a satin column: {} top stitches", stitches.len());
    let worst = stitches
        .iter()
        .filter(|&&(a, b)| hypot(b.0 - a.0, b.1 - a.1) >= 1.5)
        .map(|&(a, b)| {
            let middle = ((a.0 + b.0) / 2.0, (a.1 + b.1) / 2.0);
            let radial = atan2(middle.1 - centre.1, middle.0 - centre.0);
            let stitch = atan2(b.1 - a.1, b.0 - a.0);
            let off = to_degrees((stitch - radial).rem_euclid(std::f64::consts::PI));
            off.min(180.0 - off)
        })
        .fold(0.0, f64::max);
    assert!(worst <= 12.0, "a square end keeps its stitches: {worst:.1}°");
}

#[test]
fn req_auto_178_a_zigzag_line_sews_mitred_corners() {
    // The review set's zigzag: 1.5 mm wide, turning 127° at each of its 3 corners.
    let points = [(5.0, 40.0), (11.0, 52.0), (17.0, 40.0), (23.0, 52.0), (29.0, 40.0)];
    let zigzag = automatic(widened(along("zigzag", polylines(&[&points]), &RED, &[]), 1.5));
    let outcome = planned(vec![zigzag]);
    let leaning: Vec<_> = outcome.diagnostics.iter().filter(|d| d.code == Code::SatinStitchesLean).collect();
    // Measured before M13.60: 6 warnings, 2 at each corner, stitches up to 57.7° along the line.
    assert!(leaning.is_empty(), "no stitch leans along the line at its corners: {leaning:?}");
    // Mitred, the corners lay no more thread than the line's middle and its ends: under the cap's 3.25 fills
    // (2.36 before M13.60, 2.81 in the phase 1 prototype).
    let thickest = fabric::thickness(outcome.plan.as_ref().unwrap(), &fabric::Limits::DEFAULT).fills.value;
    assert!(thickest < 3.25, "the corners sew no thicker than the cap: {thickest:.2} fills");
}

#[test]
fn req_auto_178_a_line_turning_60_degrees_or_less_stays_one_column() {
    let points = [(5.0, 40.0), (15.0, 40.0), (23.66, 45.0), (33.66, 45.0)];
    let line = automatic(widened(along("gentle", polylines(&[&points]), &RED, &[]), 1.5));
    let outcome = planned(vec![line]);
    let plan = outcome.plan.as_ref().unwrap();
    let ids: std::collections::BTreeSet<String> = plan.elements.iter().map(ToString::to_string).filter(|id| id.starts_with("gentle")).collect();
    assert_eq!(ids.len(), 1, "a line turning 30° sews as one column: {ids:?}");
}

#[test]
fn req_auto_177_a_parallelogram_sews_square_at_both_its_slanted_ends() {
    // Both ends slanted by 45°, each column end edge folding into the other rail.
    let outcome = planned(vec![area("slab", &[(5.0, 5.0), (45.0, 5.0), (50.0, 10.0), (10.0, 10.0)])]);
    let middle = middle_y(&outcome, "slab");
    let stitches = stitches_of(&outcome, "slab", |p| p.1 - middle);
    assert!(stitches.len() > 100, "the band sews as a satin column: {} top stitches", stitches.len());
    // Measured before M13.60: up to 29.4° at its ends.
    let lean = most_lean_at_ends(&stitches, 5.0, 1.5);
    assert!(lean <= 15.0, "the stitches stay square into both points: {lean:.1}°");
}

/// A line `width` wide along x for 17 mm, then 15 mm turned by `turn` degrees, then 17 mm along x again.
fn bent_line(width: f64, turn: f64) -> Element {
    let (s, c) = sin_cos(turn.to_radians());
    let corner = (22.0 + 15.0 * c, 20.0 + 15.0 * s);
    let points = [(5.0, 20.0), (22.0, 20.0), corner, (corner.0 + 17.0, corner.1)];
    automatic(widened(along("line", polylines(&[&points]), &RED, &[]), width))
}

#[test]
fn req_auto_178_a_wider_line_sews_mitred_corners_with_no_seam() {
    for width in [3.0, 5.0] {
        for turn in [90.0, 120.0] {
            let elements = vec![bent_line(width, turn)];
            let design = stitchcraft_engine::design::Design::new(elements.clone(), Default::default()).unwrap();
            let outcome = stitchcraft_testkit::designs::in_place(&design);
            let plan = outcome.plan.as_ref().unwrap();
            let ids: std::collections::BTreeSet<String> = plan.elements.iter().map(ToString::to_string).collect();
            assert_eq!(ids, ["line~leg1", "line~leg2", "line~leg3"].map(String::from).into(), "{width} mm, {turn}°: a column per leg");
            let leaning: Vec<_> = outcome.diagnostics.iter().filter(|d| d.code == Code::SatinStitchesLean).collect();
            assert!(leaning.is_empty(), "{width} mm, {turn}°: {leaning:?}");
            // Where the legs meet, no fabric shows: the top stitches leave no gap.
            let cover = fabric::cover(&design, plan, fabric::GAP);
            assert_eq!(cover.gaps, 0, "{width} mm, {turn}°: {cover:?}");
            // Measured on 2026-11-22: 2.95 to 3.96 fills, against 2.13 to 3.93 as one column.
            let thickest = fabric::thickness(plan, &fabric::Limits::DEFAULT).fills.value;
            assert!(thickest < 4.0, "{width} mm, {turn}°: {thickest:.2} fills");
        }
    }
}

#[test]
fn req_auto_178_a_wider_line_turning_60_degrees_stays_one_column() {
    for width in [3.0, 5.0] {
        let outcome = planned(vec![bent_line(width, 60.0)]);
        let ids: Vec<String> = outcome.plan.as_ref().unwrap().elements.iter().map(ToString::to_string).collect();
        assert_eq!(ids, ["line"], "{width} mm");
    }
}
