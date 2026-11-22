//! Shapes whose width swells and tapers (`REQ-AUTO-174`, `REQ-AUTO-175`, M13.59): a shape that swells and tapers
//! sews as one area, twins sew alike, and a broad shape's tapering points stay in its fill.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Shapes whose width swells and tapers. Each case draws its shapes in
//! code, after the review set's drawings: 2 mirrored leaves, a lake as a half lens, a crescent, a lollipop, a
//! bar, a ring and a ray. Before M13.59 the leaves and the lake sewed as satin columns, and the crescent as a
//! fill with a satin column at each tip.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::sin_cos;
use stitchcraft_engine::design::Element;
use stitchcraft_plan::Role;
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, outlined, planned, rules_said};

type Pt = (f64, f64);

/// The quadratic Bézier curve from `a` to `b` pulled towards `c`, as `n` points without `b`.
fn quad(a: Pt, c: Pt, b: Pt, n: usize) -> Vec<Pt> {
    (0..n)
        .map(|k| {
            let t = k as f64 / n as f64;
            let (u, v, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t), t * t);
            (u * a.0 + v * c.0 + w * b.0, u * a.1 + v * c.1 + w * b.1)
        })
        .collect()
}

/// A lens from `a` to `b`, its 2 sides pulled towards `c` and `d`, as the review set's leaves are drawn.
fn lens(a: Pt, c: Pt, b: Pt, d: Pt) -> Vec<Pt> {
    let mut ring = quad(a, c, b, 40);
    ring.extend(quad(b, d, a, 40));
    ring
}

/// A circle about `centre` of `radius`, as `n` points.
fn circle(centre: Pt, radius: f64, n: usize) -> Vec<Pt> {
    (0..n)
        .map(|k| {
            let (s, c) = sin_cos(std::f64::consts::TAU * k as f64 / n as f64);
            (centre.0 + radius * c, centre.1 + radius * s)
        })
        .collect()
}

/// An automatic red area of the rings `parts`.
fn area(id: &str, parts: &[&[Pt]]) -> Element {
    automatic(fill(id, parts, &RED, &[]))
}

/// What `SC-I0912` says of each automatic shape of `elements`, planned together, by the shape's id.
fn said(elements: Vec<Element>) -> Vec<(String, String)> {
    rules_said(&planned(elements))
}

/// The shapes of `said` whose parts sew by a rule other than `rule`, with what is said of them.
fn not_by<'s>(said: &'s [(String, String)], rule: &str) -> Vec<&'s (String, String)> {
    said.iter().filter(|(_, message)| !message.contains(&format!("by the rule {rule} "))).collect()
}

/// The 2 leaves of the review set's flower, mirrored about the stem: 15 mm long, 5.4 mm wide.
fn leaves() -> [Vec<Pt>; 2] {
    [lens((28.5, 45.0), (20.0, 40.0), (14.0, 46.0), (21.0, 51.0)), lens((29.0, 50.0), (37.0, 44.0), (44.0, 49.0), (37.0, 55.0))]
}

#[test]
fn req_auto_174_a_leaf_and_its_twin_are_filled_whole() {
    let [left, right] = leaves();
    let said = said(vec![area("leaf-left", &[&left]), area("leaf-right", &[&right])]);
    // Drawn as polylines both sew as columns today. In the review set's flower, drawn with curves, the left
    // leaf's column was refused for crowding (2.11 mm to 1 mm), and the right one sewed as a column.
    assert_eq!(said.len(), 2, "{said:?}");
    assert!(not_by(&said, "uneven").is_empty(), "both leaves are filled by the rule uneven: {said:?}");
}

#[test]
fn req_auto_174_a_lens_4_mm_wide_is_filled_and_its_outline_reaches_its_points() {
    // The demo badge's lake: a flat top 30 mm long, a curved bottom 5.5 mm below it at the middle.
    let mut lake = vec![(0.0, 0.0), (30.0, 0.0)];
    lake.extend(quad((30.0, 0.0), (15.0, 11.0), (0.0, 0.0), 40).into_iter().skip(1));
    let outcome = planned(vec![outlined(fill("lake", &[&lake], &RED, &[]))]);
    let said = rules_said(&outcome);
    assert!(not_by(&said, "uneven").is_empty(), "the lake is filled: {said:?}");
    // The outline round the fill (`REQ-AUTO-051`) draws both points: a needle point within 0.15 mm of each.
    let stitches = outcome.plan.unwrap().sewn_stitches();
    for tip in [(0.0, 0.0), (30.0, 0.0)] {
        let nearest = stitches.iter().map(|s| hypot(s.to.x() - tip.0, s.to.y() - tip.1)).fold(f64::MAX, f64::min);
        assert!(nearest <= 0.15, "a needle point {nearest:.2} mm from the point at {tip:?}");
    }
}

/// The distance from the origin to (`x`, `y`).
fn hypot(x: f64, y: f64) -> f64 {
    stitchcraft_core::math::hypot(x, y)
}

#[test]
fn req_auto_174_even_parts_and_parts_narrower_than_a_fills_narrowest_piece_keep_their_columns() {
    // A guard that passes before M13.59 too: what keeps about one width, and a ray 2 mm at its widest.
    let bar = [(0.0, 0.0), (15.0, 0.0), (15.0, 3.0), (0.0, 3.0)];
    let (outer, mut hole) = (circle((30.0, 30.0), 8.0, 96), circle((30.0, 30.0), 5.5, 96));
    hole.reverse();
    let ray = [(0.0, 50.0), (9.0, 51.0), (0.0, 52.0)];
    let said = said(vec![area("bar", &[&bar]), area("ring", &[&outer, &hole]), area("ray", &[&ray])]);
    assert_eq!(said.len(), 3, "{said:?}");
    assert!(not_by(&said, "column").is_empty(), "each keeps its column: {said:?}");
}

#[test]
fn req_auto_175_a_crescents_tips_stay_in_its_fill() {
    // 30 mm tip to tip, 9 mm wide at its middle: broad, and its tips narrower than 4 mm.
    let mut crescent = quad((0.0, 0.0), (-28.0, 15.0), (0.0, 30.0), 60);
    crescent.extend(quad((0.0, 30.0), (-10.0, 15.0), (0.0, 0.0), 60));
    let said = said(vec![area("crescent", &[&crescent])]);
    let [(_, message)] = said.as_slice() else { panic!("one shape: {said:?}") };
    // Its broad middle sews by the rule area, and each tip, kept in the fill, by the rule uneven.
    assert!(message.contains("area") && !message.contains("column"), "one fill to its points: {message}");
}

#[test]
fn req_auto_175_a_crescents_outline_turns_at_each_point() {
    // The crescent of the night drawing, on a disc of sky, both with their outlines. The outline is one line
    // round the crescent: at each point it comes in along one edge and leaves along the other, and no other
    // needle point of it lies within 0.5 mm of the point. Its stitches are 2.5 mm long, so the needle points
    // before and after the point lie on the 2 edges, more than 0.5 mm apart.
    let mut crescent = quad((0.0, 0.0), (-28.0, 15.0), (0.0, 30.0), 60);
    crescent.extend(quad((0.0, 30.0), (-10.0, 15.0), (0.0, 0.0), 60));
    let sky = circle((-8.0, 15.0), 22.0, 120);
    let outcome = planned(vec![outlined(fill("sky", &[&sky], &BLUE, &[])), outlined(fill("crescent", &[&crescent], &RED, &[]))]);
    let plan = outcome.plan.unwrap();
    let outline = plan.elements.iter().position(|id| id.as_str() == "crescent#2").unwrap();
    let points: Vec<Pt> = plan
        .blocks
        .iter()
        .flat_map(|block| &block.stitches)
        .filter(|stitch| stitch.origin.element.map(|element| element.index()) == Some(outline) && stitch.origin.role == Role::Top)
        .map(|stitch| (stitch.at.x(), stitch.at.y()))
        .collect();
    for tip in [(0.0, 0.0), (0.0, 30.0)] {
        let near: Vec<usize> = (0..points.len()).filter(|&k| hypot(points[k].0 - tip.0, points[k].1 - tip.1) <= 0.5).collect();
        let [at] = near.as_slice() else { panic!("one needle point of the outline at {tip:?}: {near:?} of {points:?}") };
        assert!(hypot(points[*at].0 - tip.0, points[*at].1 - tip.1) < 0.01, "{:?} at {tip:?}", points[*at]);
        let (before, after) = (points[at.checked_sub(1).unwrap()], points[at + 1]);
        assert!(hypot(before.0 - after.0, before.1 - after.1) > 0.5, "the outline turns at {tip:?}: {before:?} {after:?}");
    }
}

#[test]
fn req_auto_175_an_even_piece_still_leaves_the_fill() {
    // A guard that passes before M13.59 too: a lollipop's stick, 2.4 mm wide, sews as a column over its disc.
    // The disc's edge from 285° round to 255°, then the stick, which meets the disc where it is 2.4 mm across.
    let mut lollipop: Vec<Pt> = (0..=66)
        .map(|k| {
            let (s, c) = sin_cos((285.0 + 5.0 * f64::from(k)).to_radians());
            (10.0 + 6.0 * c, 10.0 + 6.0 * s)
        })
        .collect();
    lollipop.extend([(8.8, 4.121), (8.8, -11.0), (11.2, -11.0), (11.2, 4.121)]);
    let said = said(vec![area("lollipop", &[&lollipop])]);
    let [(_, message)] = said.as_slice() else { panic!("one shape: {said:?}") };
    assert!(message.contains("column"), "its stick leaves the fill: {message}");
}

#[test]
fn req_auto_175_a_shape_that_asks_for_satin_keeps_its_tips_columns() {
    let mut crescent = quad((0.0, 0.0), (-28.0, 15.0), (0.0, 30.0), 60);
    crescent.extend(quad((0.0, 30.0), (-10.0, 15.0), (0.0, 0.0), 60));
    let said = said(vec![automatic(fill("crescent", &[&crescent], &RED, &[("auto_stitch", "satin")]))]);
    let [(_, message)] = said.as_slice() else { panic!("one shape: {said:?}") };
    assert!(!message.contains("uneven"), "its tips leave the fill as it asks: {message}");
}
