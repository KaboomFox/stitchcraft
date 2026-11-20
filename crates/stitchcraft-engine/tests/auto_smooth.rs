//! Outlines smoothed where thread can't show their wobble (roadmap step M13.52, `REQ-AUTO-131` to
//! `REQ-AUTO-134`). A traced picture's outlines carry many points and wobble by less than a thread is
//! wide. With `auto_smooth_mm` set, an automatic area's outline is smoothed within that distance, its corners
//! kept, an edge 2 areas share smoothed once, and no outline made to cross itself or another. The designs
//! are drawn here in code with a known wobble: a saw-tooth 0.06 mm high, a tooth every 0.5 mm.
//!
//! The pass is `stitchcraft_engine::auto`'s `smooth` module (`docs/src/design/image-to-embroidery.md` ›
//! Smoothing in StitchCraft). The setting is 0 by default, and the cases of an area with no setting, or with
//! the setting at 0, guard the outline as drawn.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss
)]

use stitchcraft_core::{Budget, Diagnostic, Fix};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Segment, Shape};
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, planned, unbased};
use stitchcraft_testkit::fabric::{GAP, cover};

/// The setting the cases set: how far an outline may move when it is smoothed, in millimetres.
const SMOOTH: &str = "auto_smooth_mm";
/// How high each tooth of the drawn saw-tooth stands, in millimetres: less than half a thread 0.4 mm wide.
const TOOTH: f64 = 0.06;
/// How far apart the teeth are, in millimetres.
const PITCH: f64 = 0.5;

/// The points of a saw-tooth from `(x0, y)` to `(x1, y)`, every other point `TOOTH` above the line, the ends
/// on it.
fn saw(x0: f64, x1: f64, y: f64) -> Vec<(f64, f64)> {
    let steps = ((x1 - x0).abs() / (PITCH / 2.0)).round() as usize;
    let dx = (x1 - x0) / steps as f64;
    (0..=steps).map(|i| (x0 + dx * i as f64, if i % 2 == 1 && i != steps { y + TOOTH } else { y })).collect()
}

/// The points of a saw-tooth from `(x, y0)` to `(x, y1)`, every other point `TOOTH` to the right of the line.
fn saw_up(x: f64, y0: f64, y1: f64) -> Vec<(f64, f64)> {
    saw(y0, y1, x).into_iter().map(|(along, across)| (across, along)).collect()
}

/// An area 20 by 10 mm whose top edge is a saw-tooth, below the area [`top`] draws over the same edge.
fn bottom(id: &str, params: &[(&str, &str)], right_saw: bool) -> Element {
    let mut ring = vec![(0.0, 0.0), (20.0, 0.0)];
    if right_saw {
        ring.extend(saw_up(20.0, 0.0, 10.0).into_iter().skip(1));
    } else {
        ring.push((20.0, 10.0));
    }
    let top_edge: Vec<(f64, f64)> = saw(0.0, 20.0, 10.0).into_iter().rev().skip(1).collect();
    ring.extend(top_edge);
    automatic(fill(id, &[&ring], &RED, params))
}

/// An area 20 by 10 mm above [`bottom`]'s, its lower edge the same saw-tooth, point for point.
fn top(id: &str, params: &[(&str, &str)]) -> Element {
    let mut ring = saw(0.0, 20.0, 10.0);
    ring.extend([(20.0, 20.0), (0.0, 20.0)]);
    automatic(fill(id, &[&ring], &BLUE, params))
}

/// The design of `elements`, with no base layer and no overlap, so that each area keeps its outline.
fn design(elements: Vec<Element>) -> Design {
    let elements = elements
        .into_iter()
        .map(|mut element| {
            element.params.set("auto_overlap_mm", "0");
            element
        })
        .collect();
    Design::new(unbased(elements), DesignSettings::default()).unwrap()
}

/// What the areas `elements` sew as, in a [`design`], and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = design(elements);
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

/// The outline points of the fills digitized from the element `id`: each fill's starts and segment ends.
fn outline(elements: &[Element], id: &str) -> Vec<(f64, f64)> {
    let mut points = Vec::new();
    for element in elements.iter().filter(|element| element.id.as_str() == id || element.id.as_str().starts_with(&format!("{id}#"))) {
        let Shape::Fill { path, .. } = &element.shape else { continue };
        for subpath in &path.subpaths {
            points.push((subpath.start.x(), subpath.start.y()));
            points.extend(subpath.segments.iter().map(|segment: &Segment| (segment.end().x(), segment.end().y())));
        }
    }
    points
}

/// How far `p` lies from the polyline through `line`.
fn distance(p: (f64, f64), line: &[(f64, f64)]) -> f64 {
    line.windows(2)
        .map(|pair| {
            let ((ax, ay), (bx, by)) = (pair[0], pair[1]);
            let (dx, dy) = (bx - ax, by - ay);
            let length2 = dx * dx + dy * dy;
            let t = if length2 > 0.0 { (((p.0 - ax) * dx + (p.1 - ay) * dy) / length2).clamp(0.0, 1.0) } else { 0.0 };
            {
                let (ex, ey) = (p.0 - ax - t * dx, p.1 - ay - t * dy);
                (ex * ex + ey * ey).sqrt()
            }
        })
        .fold(f64::INFINITY, f64::min)
}

/// The points of `outline` on the saw-tooth edge at y 10, between its ends.
fn on_the_edge(outline: &[(f64, f64)]) -> Vec<(f64, f64)> {
    outline.iter().copied().filter(|&(x, y)| (y - 10.0).abs() <= 0.5 && x > 0.5 && x < 19.5).collect()
}

/// Whether 2 segments cross, other than at an end they share.
fn cross(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let side = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0);
    let (d1, d2, d3, d4) = (side(a, b, c), side(a, b, d), side(c, d, a), side(c, d, b));
    d1 * d2 < 0.0 && d3 * d4 < 0.0
}

#[test]
fn req_auto_131_an_outline_is_smoothed_within_the_setting_and_keeps_its_corners() {
    let drawn = bottom("a", &[(SMOOTH, "0.2")], false);
    let (elements, _) = sewn(vec![drawn.clone()]);
    let smoothed = outline(&elements, "a");
    let original = outline(std::slice::from_ref(&drawn), "a");
    let closed: Vec<(f64, f64)> = original.iter().copied().chain(original.first().copied()).collect();
    for &p in &smoothed {
        assert!(distance(p, &closed) <= 0.2 + 1e-9, "{p:?} lies {} mm from the drawn outline", distance(p, &closed));
    }
    for corner in [(0.0, 0.0), (20.0, 0.0), (0.0, 10.0)] {
        assert!(
            smoothed.iter().any(|&p| (p.0 - corner.0).abs() < 1e-9 && (p.1 - corner.1).abs() < 1e-9),
            "the corner {corner:?} moved: {smoothed:?}"
        );
    }
    // The saw-tooth's 75 points away from its ends become a line or a few points along it.
    assert_eq!(on_the_edge(&original).len(), 75);
    assert!(on_the_edge(&smoothed).len() <= 4, "the edge keeps {} points", on_the_edge(&smoothed).len());
}

#[test]
fn req_auto_132_an_edge_2_areas_share_is_smoothed_once_and_they_still_meet() {
    let params = [(SMOOTH, "0.2")];
    let (elements, _) = sewn(vec![bottom("a", &params, false), top("b", &params)]);
    let (a, b) = (outline(&elements, "a"), outline(&elements, "b"));
    let (a_closed, b_closed): (Vec<_>, Vec<_>) =
        (a.iter().copied().chain(a.first().copied()).collect(), b.iter().copied().chain(b.first().copied()).collect());
    let (a_edge, b_edge) = (on_the_edge(&a), on_the_edge(&b));
    assert!(a_edge.len() <= 4 && b_edge.len() <= 4, "the shared edge keeps {} and {} points", a_edge.len(), b_edge.len());
    for &p in &a_edge {
        assert!(distance(p, &b_closed) < 1e-6, "{p:?} of the lower area's edge is off the upper area's outline");
    }
    for &p in &b_edge {
        assert!(distance(p, &a_closed) < 1e-6, "{p:?} of the upper area's edge is off the lower area's outline");
    }
}

#[test]
fn req_auto_133_an_edge_shared_with_an_area_set_off_stays_as_drawn() {
    let (lower, upper) = (bottom("a", &[(SMOOTH, "0.2")], true), top("b", &[(SMOOTH, "0")]));
    let drawn = on_the_edge(&outline(std::slice::from_ref(&lower), "a"));
    let (elements, _) = sewn(vec![lower, upper]);
    let a = outline(&elements, "a");
    // The edge the 2 areas share keeps every tooth on both sides: the upper area is to sew as drawn.
    assert_eq!(on_the_edge(&a), drawn, "the lower area's top edge moved");
    assert_eq!(on_the_edge(&outline(&elements, "b")).len(), drawn.len());
    // Its right side is its own, and is smoothed.
    let right: Vec<_> = a.iter().filter(|&&(x, y)| (x - 20.0).abs() <= 0.5 && y > 0.5 && y < 9.5).collect();
    assert!(right.len() <= 4, "the right side keeps {} points", right.len());
}

#[test]
fn req_auto_133_an_area_with_no_setting_sews_its_outline_as_drawn() {
    let drawn = bottom("a", &[], true);
    let (elements, said) = sewn(vec![drawn.clone()]);
    assert_eq!(on_the_edge(&outline(&elements, "a")), on_the_edge(&outline(std::slice::from_ref(&drawn), "a")));
    assert!(said.iter().all(|d| d.code.to_string() != "SC-I0921"), "{said:?}");
}

#[test]
fn req_auto_134_a_smoothed_outline_never_crosses_itself() {
    // A strip 0.6 mm tall whose top is the saw-tooth, filled whole and smoothed at 0.4 mm: more than half the
    // strip's height, so that its 2 long sides could meet.
    let mut ring = vec![(0.0, 9.4), (20.0, 9.4)];
    ring.extend(saw(20.0, 0.0, 10.0));
    let strip = automatic(fill("s", &[&ring], &RED, &[(SMOOTH, "0.4"), ("auto_stitch", "fill")]));
    let (elements, _) = sewn(vec![strip]);
    let smoothed = outline(&elements, "s");
    assert!(!smoothed.is_empty() && smoothed.len() < ring.len(), "the strip keeps {} of its {} points", smoothed.len(), ring.len());
    let closed: Vec<(f64, f64)> = smoothed.iter().copied().chain(smoothed.first().copied()).collect();
    let segments: Vec<_> = closed.windows(2).map(|pair| (pair[0], pair[1])).collect();
    for (i, &(a, b)) in segments.iter().enumerate() {
        for &(c, d) in segments.iter().skip(i + 2) {
            assert!(!cross(a, b, c, d), "the smoothed strip crosses itself: {a:?}-{b:?} and {c:?}-{d:?}");
        }
    }
}

#[test]
fn diag_sc_i0921_says_the_outline_points_before_and_after() {
    let params = [(SMOOTH, "0.2")];
    let (lower, upper) = (bottom("a", &params, true), top("b", &params));
    let drawn = outline(std::slice::from_ref(&lower), "a").len() + outline(std::slice::from_ref(&upper), "b").len();
    assert_eq!(drawn, 205);
    let (_, said) = sewn(vec![lower, upper]);
    let notices: Vec<&str> = said.iter().filter(|d| d.code.to_string() == "SC-I0921").map(|d| d.message.as_str()).collect();
    assert_eq!(notices.len(), 1, "{said:?}");
    // The 2 areas' outlines are drawn with 205 points, the shared edge counted once for each area.
    assert!(notices[0].contains("205"), "{}", notices[0]);
    let notice = said.iter().find(|d| d.code.to_string() == "SC-I0921").unwrap();
    assert_eq!(
        notice.to_string(),
        "info SC-I0921: The outlines of 2 automatic areas were smoothed within 0.2 mm: they are drawn with 19 points, where they had 205."
    );
    assert_eq!(
        notice.fix,
        Some(Fix::Hint("Smoothing changes which parts sew as satin columns. To sew the outlines as drawn, set auto_smooth_mm to 0.".to_string()))
    );
    assert_eq!(notice.element, None);
}

#[test]
fn req_auto_133_a_setting_of_0_digitizes_as_without_the_setting() {
    // The same areas with the setting at 0 and with no setting digitize to the same elements, and say the same.
    let unset = sewn(vec![bottom("a", &[], true), top("b", &[])]);
    let off = sewn(vec![bottom("a", &[(SMOOTH, "0")], true), top("b", &[(SMOOTH, "0")])]);
    let strip = |elements: Vec<Element>| -> Vec<Shape> { elements.into_iter().map(|element| element.shape).collect() };
    assert_eq!(strip(off.0), strip(unset.0));
    assert_eq!(off.1, unset.1);
}

/// The points of the outline of `id` among `elements`, sorted: the same outline drawn from another start or
/// the other way round has the same points.
fn sorted(elements: &[Element], id: &str) -> Vec<(u64, u64)> {
    let mut points: Vec<(u64, u64)> = outline(elements, id).into_iter().map(|(x, y)| ((x + 0.0).to_bits(), (y + 0.0).to_bits())).collect();
    points.sort_unstable();
    points.dedup();
    points
}

#[test]
fn req_auto_131_a_ring_smooths_the_same_from_any_start_and_either_way_round() {
    // The lower area of the cases, its saw-tooth on top and on its right, drawn from 3 starts and both ways.
    let Shape::Fill { path, .. } = bottom("a", &[], true).shape else { panic!("a fill") };
    let subpath = &path.subpaths[0];
    let ring: Vec<(f64, f64)> =
        std::iter::once(subpath.start).chain(subpath.segments.iter().map(|segment| segment.end())).map(|p| (p.x(), p.y())).collect();
    let ring: Vec<(f64, f64)> = if ring.first() == ring.last() { ring[..ring.len() - 1].to_vec() } else { ring };
    let smoothed = |points: &[(f64, f64)]| {
        let (elements, _) = sewn(vec![automatic(fill("a", &[points], &RED, &[(SMOOTH, "0.2")]))]);
        sorted(&elements, "a")
    };
    let first = smoothed(&ring);
    assert!(first.len() < 30, "{} points", first.len());
    for start in [1, 17, 60] {
        let mut turned = ring.clone();
        turned.rotate_left(start);
        assert_eq!(smoothed(&turned), first, "from point {start}");
        turned.reverse();
        assert_eq!(smoothed(&turned), first, "from point {start}, the other way round");
    }
}

#[test]
fn req_auto_131_smoothing_the_same_design_twice_gives_the_same_outlines() {
    let elements = || vec![bottom("a", &[(SMOOTH, "0.3")], true), top("b", &[(SMOOTH, "0.2")])];
    let (once, said_once) = sewn(elements());
    let (twice, said_twice) = sewn(elements());
    assert_eq!(once, twice);
    assert_eq!(said_once, said_twice);
}

/// The area a ring through `points` encloses.
fn area(points: &[(f64, f64)]) -> f64 {
    let closed: Vec<(f64, f64)> = points.iter().copied().chain(points.first().copied()).collect();
    closed.windows(2).map(|pair| pair[0].0 * pair[1].1 - pair[1].0 * pair[0].1).sum::<f64>().abs() / 2.0
}

#[test]
fn req_auto_131_a_small_ring_keeps_its_area_within_a_tenth() {
    // A disc 1.2 mm across drawn with 48 points, every other one 0.05 mm out: smoothed by a Gaussian of 0.5 mm,
    // most of the length round it, it would shrink to a speck. The area bound keeps it.
    let disc: Vec<(f64, f64)> = (0..48)
        .map(|i| {
            let angle = std::f64::consts::TAU * f64::from(i) / 48.0;
            let radius = if i % 2 == 0 { 0.6 } else { 0.65 };
            (5.0 + radius * angle.cos(), 5.0 + radius * angle.sin())
        })
        .collect();
    let (elements, _) = sewn(vec![automatic(fill("d", &[&disc], &RED, &[(SMOOTH, "0.5"), ("auto_stitch", "fill")]))]);
    let smoothed = outline(&elements, "d");
    let (before, after) = (area(&disc), area(&smoothed));
    assert!(smoothed.len() >= 3, "{smoothed:?}");
    assert!((after - before).abs() <= 0.1 * before, "the disc's area went from {before} to {after} mm²");
}

#[test]
fn req_auto_132_smoothed_neighbours_leave_no_more_fabric_bare_than_as_drawn() {
    // The 2 areas of the cases, planned as drawn and smoothed: the bare fabric between them is no more.
    let bare = |setting: &str| {
        let elements = design(vec![bottom("a", &[(SMOOTH, setting)], true), top("b", &[(SMOOTH, setting)])]).elements().to_vec();
        let drawn = Design::new(elements.clone(), DesignSettings::default()).unwrap();
        let plan = planned(elements).plan.unwrap();
        cover(&drawn, &plan, GAP).bare
    };
    let (as_drawn, smoothed) = (bare("0"), bare("0.2"));
    assert!(smoothed <= as_drawn + 1e-9, "{as_drawn} mm² bare as drawn, {smoothed} mm² smoothed");
}
