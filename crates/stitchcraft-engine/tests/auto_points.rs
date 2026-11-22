//! Points stay points (`REQ-AUTO-183`, `REQ-SAT-032`, M13.62): a satin column that tapers to a point ends in a
//! point, without sewing thicker.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › A column's point. Each case draws its wedges in code, after
//! the review set's sunrise, whose rays are wedges 2.2 mm wide at their base and 14.7° at their tip. On `main`
//! (21f100cb) a ray drawn 0.13 mm wide 0.5 mm from its tip sews 0.90 to 0.96 mm wide there, and 1.06 to 1.11 mm
//! 1 mm in, where it is drawn 0.26 mm: each stitch reaches 0.4 mm past each rail (`auto_pull_compensation_mm`),
//! up to the pair 0.2 mm from the tip, so the column ends in a bar about 0.85 mm wide with one stitch to the tip.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::{atan2, cos, hypot, sin, tan, to_degrees, to_radians};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::Element;
use stitchcraft_plan::Role;
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, planned, polylines};
use stitchcraft_testkit::fabric;

type Pt = (f64, f64);

/// The shortest stitch of the reference machine, in millimetres (`stitchcraft_plan::profiles::REFERENCE`).
const SHORTEST: f64 = 0.3;

/// A wedge `width` mm wide at its base and `angle` degrees at its tip, its tip at `tip` and its base to the right,
/// as an automatic red area, with `params` set on it.
fn wedge(id: &str, tip: Pt, width: f64, angle: f64, params: &[(&str, &str)]) -> Element {
    let length = width / 2.0 / tan(to_radians(angle / 2.0));
    automatic(fill(id, &[&[tip, (tip.0 + length, tip.1 - width / 2.0), (tip.0 + length, tip.1 + width / 2.0)]], &RED, params))
}

/// How wide `id`'s top stitches sew `d` mm in from a tip at `tip` of a wedge pointing left: the spread across of
/// its top stitches clipped to a band of 0.2 mm each side of the line square to the wedge there, half the
/// zigzag spacing.
fn sewn_width(outcome: &PlanOutcome, id: &str, tip: Pt, d: f64) -> f64 {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{id}: {:?}", outcome.diagnostics));
    let (low, high) = (tip.0 + d - 0.2, tip.0 + d + 0.2);
    let mut ys = Vec::new();
    for s in plan.sewn_stitches().into_iter().filter(|s| s.role == Role::Top && !s.is_lock()) {
        let element = plan.blocks[s.block].stitches[s.index].origin.element.map(|e| plan.elements[e.index()].to_string());
        if element.as_deref() != Some(id) {
            continue;
        }
        let ((ax, ay), (bx, by)) = ((s.from.x(), s.from.y()), (s.to.x(), s.to.y()));
        if ax.max(bx) < low || ax.min(bx) > high {
            continue;
        }
        if (bx - ax).abs() < 1e-9 {
            ys.extend([ay, by]);
            continue;
        }
        for x in [ax.min(bx).max(low), ax.max(bx).min(high)] {
            ys.push(ay + (x - ax) / (bx - ax) * (by - ay));
        }
    }
    ys.iter().fold(f64::MIN, |m, &y| m.max(y)) - ys.iter().fold(f64::MAX, |m, &y| m.min(y))
}

/// The wedges of the sew-out piece: 3 widths at the base and 3 angles at the tip, each planned alone with its tip
/// 12 mm left of the origin.
fn wedges(params: &[(&str, &str)]) -> Vec<(Element, Pt, f64, f64)> {
    let mut out = Vec::new();
    for (row, width) in [1.5, 2.5, 3.5].into_iter().enumerate() {
        for (column, angle) in [8.0, 14.0, 20.0].into_iter().enumerate() {
            let tip = (-12.0, 0.0);
            out.push((wedge(&format!("wedge-{row}-{column}"), tip, width, angle, params), tip, width, angle));
        }
    }
    out
}

/// The drawn width of a wedge `angle` degrees at its tip, `d` mm in from it.
fn drawn(angle: f64, d: f64) -> f64 {
    2.0 * d * tan(to_radians(angle / 2.0))
}

#[test]
fn req_auto_183_a_wedge_sews_its_point_no_wider_than_drawn_near_its_tip() {
    let mut wrong = Vec::new();
    for (element, tip, width, angle) in wedges(&[]) {
        let id = element.id.to_string();
        let outcome = planned(vec![element]);
        // Over the last 2 mm the pull compensation falls to 0 at the tip: at 0.5 mm in it is a quarter of 0.4 mm
        // each side, at 1 mm a half. The band measured reaches 0.2 mm farther in, and 0.05 mm is left for where the
        // pairs fall. On main (21f100cb) the wedges sew 0.85 to 1.05 mm wide 0.5 mm in and 0.95 to 1.21 mm 1 mm in.
        for d in [0.5, 1.0] {
            let (sewn, want) = (sewn_width(&outcome, &id, tip, d), drawn(angle, d));
            if sewn > drawn(angle, d + 0.2) + 0.4 * (d + 0.2) + 0.05 {
                wrong.push(format!("{id} ({width} mm, {angle}°) sews {sewn:.2} mm wide {d} mm from its tip, drawn {want:.2}"));
            }
        }
        // Past the taper the column keeps its compensation.
        let length = width / 2.0 / tan(to_radians(angle / 2.0));
        if length > 4.0 {
            let (sewn, want) = (sewn_width(&outcome, &id, tip, 3.0), drawn(angle, 3.0));
            if (sewn - want - 0.8).abs() > 0.15 {
                wrong.push(format!("{id} sews {sewn:.2} mm wide 3 mm in, drawn {want:.2}: full compensation"));
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}

#[test]
fn req_auto_183_a_wedge_s_tip_is_covered_and_no_stitch_is_shorter_than_the_machine_sews() {
    for (element, tip, _, _) in wedges(&[]) {
        let id = element.id.to_string();
        let outcome = planned(vec![element]);
        let plan = outcome.plan.as_ref().unwrap();
        let own: Vec<_> = plan
            .sewn_stitches()
            .into_iter()
            .filter(|s| s.role == Role::Top && !s.is_lock())
            .filter(|s| plan.blocks[s.block].stitches[s.index].origin.element.map(|e| plan.elements[e.index()].to_string()).as_deref() == Some(&id))
            .collect();
        let nearest = own.iter().map(|s| hypot(s.to.x() - tip.0, s.to.y() - tip.1)).fold(f64::MAX, f64::min);
        assert!(nearest <= 0.15, "{id}: the nearest needle point lies {nearest:.2} mm from the drawn tip");
        let shortest = own.iter().map(|s| s.length()).fold(f64::MAX, f64::min);
        assert!(shortest >= SHORTEST - 1e-9, "{id}: a top stitch {shortest:.2} mm long");
    }
}

#[test]
fn req_auto_183_a_taper_of_0_keeps_today_s_end() {
    for (element, tip, width, angle) in wedges(&[("auto_point_taper_mm", "0")]) {
        let id = element.id.to_string();
        let outcome = planned(vec![element]);
        let (sewn, want) = (sewn_width(&outcome, &id, tip, 0.5), drawn(angle, 0.5));
        assert!(sewn >= want + 0.7, "{id} ({width} mm, {angle}°) sews {sewn:.2} mm wide 0.5 mm in with no taper, drawn {want:.2}");
    }
}

#[test]
fn req_auto_183_a_point_over_a_fill_sews_no_thicker_than_without_the_taper() {
    // The sunrise's rays lie on its sky: a wedge over an automatic square, its tip 3 mm inside the square.
    let sky = |params: &[(&str, &str)]| automatic(fill("sky", &[&[(0.0, 0.0), (30.0, 0.0), (30.0, 20.0), (0.0, 20.0)]], &BLUE, params));
    let thickest = |params: &[(&str, &str)]| {
        let outcome = planned(vec![sky(params), wedge("ray", (3.0, 10.0), 2.2, 14.7, params)]);
        fabric::thickness(outcome.plan.as_ref().unwrap(), &fabric::Limits::DEFAULT).fills.value
    };
    let (tapered, blunt) = (thickest(&[]), thickest(&[("auto_point_taper_mm", "0")]));
    assert!(tapered <= blunt + 1e-9, "the ray over its sky sews {tapered:.3} fills at its thickest, {blunt:.3} with a blunt end");
}

#[test]
fn req_auto_183_a_square_end_keeps_its_compensation() {
    // A band 3 mm tall with square ends: its rails don't meet at an end, so nothing of it changes.
    let band = |params: &[(&str, &str)]| planned(vec![automatic(fill("band", &[&[(5.0, 5.0), (35.0, 5.0), (35.0, 8.0), (5.0, 8.0)]], &RED, params))]);
    let points = |outcome: &PlanOutcome| -> Vec<(f64, f64)> {
        let plan = outcome.plan.as_ref().unwrap();
        plan.sewn_stitches().iter().map(|s| (s.to.x(), s.to.y())).collect()
    };
    assert_eq!(points(&band(&[])), points(&band(&[("auto_point_taper_mm", "0")])), "the band sews as it does today");
}

#[test]
fn req_sat_032_a_satin_column_s_pull_compensation_falls_to_0_over_its_point_taper() {
    // A manual satin column of 2 rails that meet at its end, 2.4 mm apart at its start, 12 mm long.
    let column = |taper: Option<&str>| {
        let mut params = vec![("satin_column", "true"), ("pull_compensation_mm", "0.4")];
        if let Some(taper) = taper {
            params.push(("point_taper_mm", taper));
        }
        planned(vec![along("satin", polylines(&[&[(17.0, 3.8), (5.0, 5.0)], &[(17.0, 6.2), (5.0, 5.0)]]), &RED, &params)])
    };
    let tip = (5.0, 5.0);
    let angle = 2.0 * to_degrees(atan2(1.2, 12.0));
    // By default a column places its pairs as Ink/Stitch does: each widened by the whole compensation.
    let ink = sewn_width(&column(None), "satin", tip, 0.5);
    assert!(ink >= drawn(angle, 0.5) + 0.75, "without a taper the point sews {ink:.2} mm wide 0.5 mm in");
    // With a taper of 2 mm, a pair 0.5 mm from the point keeps a quarter of it.
    let tapered = sewn_width(&column(Some("2")), "satin", tip, 0.5);
    assert!(tapered <= drawn(angle, 0.5) + 0.25, "with a taper of 2 mm it sews {tapered:.2} mm wide 0.5 mm in");
    // At 3 mm, past the taper, the pairs are as without one.
    let (a, b) = (sewn_width(&column(None), "satin", tip, 3.0), sewn_width(&column(Some("2")), "satin", tip, 3.0));
    assert!((a - b).abs() <= 0.02, "3 mm in: {a:.2} without a taper, {b:.2} with");
}

/// A leaf 12 mm long and 3 mm wide, its 2 sides arcs that meet in a point at each end, as the review set's
/// rules fixture draws one: an automatic satin column whose start and end are points.
fn leaf() -> Vec<Pt> {
    // Each side is an arc through the leaf's 2 points (0, 0) and (12, 0) and the middle of the other side's
    // width, 1.5 mm off the axis: a circle of radius 12.75 mm about (6, -11.25) or (6, 11.25).
    let (half, sag) = (6.0, 1.5);
    let radius = (half * half + sag * sag) / (2.0 * sag);
    let arc = |side: f64| -> Vec<Pt> {
        let centre = (half, -side * (radius - sag));
        let (from, to) = (atan2(-centre.1, -centre.0), atan2(-centre.1, 2.0 * half - centre.0));
        (0..=24)
            .map(|k| {
                let a = from + (to - from) * f64::from(k) / 24.0;
                (centre.0 + radius * cos(a), centre.1 + radius * sin(a))
            })
            .collect()
    };
    let mut ring = arc(1.0);
    ring.extend(arc(-1.0).into_iter().rev().skip(1));
    ring.pop();
    ring.iter().map(|&(x, y)| (x - 6.0, y)).collect()
}

/// How far `p` lies outside the polygon `ring`: 0 inside.
fn outside(p: Pt, ring: &[Pt]) -> f64 {
    let n = ring.len();
    let (mut inside, mut nearest) = (false, f64::MAX);
    for i in 0..n {
        let (a, b) = (ring[i], ring[(i + 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            inside = !inside;
        }
        let (dx, dy) = (b.0 - a.0, b.1 - a.1);
        let t = (((p.0 - a.0) * dx + (p.1 - a.1) * dy) / (dx * dx + dy * dy).max(1e-12)).clamp(0.0, 1.0);
        nearest = nearest.min(hypot(p.0 - a.0 - t * dx, p.1 - a.1 - t * dy));
    }
    if inside { 0.0 } else { nearest }
}

/// The farthest any lock needle point of `id` lies outside `ring`, of those within 1.5 mm of one of `tips`.
fn locks_outside(outcome: &PlanOutcome, id: &str, ring: &[Pt], tips: &[Pt]) -> f64 {
    let plan = outcome.plan.as_ref().unwrap();
    plan.sewn_stitches()
        .into_iter()
        .filter(|s| s.is_lock())
        .filter(|s| plan.blocks[s.block].stitches[s.index].origin.element.map(|e| plan.elements[e.index()].to_string()).as_deref() == Some(id))
        .map(|s| (s.to.x(), s.to.y()))
        .filter(|&(x, y)| tips.iter().any(|t| hypot(x - t.0, y - t.1) < 1.5))
        .map(|p| outside(p, ring))
        .fold(0.0, f64::max)
}

#[test]
fn req_auto_183_a_point_s_locks_lie_inside_its_shape() {
    // The lock at each point goes back along the column's last stitches, which the taper brings inside the
    // point, so no needle point of it lies farther outside the drawn shape than half a thread's width.
    let ring = leaf();
    let mut found = Vec::new();
    for taper in ["2", "0"] {
        let outcome = planned(vec![automatic(fill("leaf", &[&ring], &RED, &[("auto_point_taper_mm", taper)]))]);
        found.push(locks_outside(&outcome, "leaf", &ring, &[(-6.0, 0.0), (6.0, 0.0)]));
    }
    // Measured on 2026-11-22: 0.04 mm with the taper, 0.21 mm at 0.
    assert!(found[0] <= 0.2, "with the taper a lock needle point lies {:.2} mm outside the leaf (at 0: {:.2})", found[0], found[1]);
}
