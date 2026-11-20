//! The stitch rule of a shape doesn't depend on how its outline is drawn (`REQ-AUTO-135` to `REQ-AUTO-138`,
//! M13.53): an outline drawn again within 0.05 mm, with points added along it, its points moved, started at
//! another point or drawn the other way round, sews by the rule it sewed by as drawn.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Outlines drawn 2 ways. Each case draws its
//! shapes in code: a bar, a T, an L, a wedge, a quarter ring and block letters, each 2 ways that thread can't
//! tell apart. Before M13.53 each pair sewed by 2 rules.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::math::{hypot, sin_cos};
use stitchcraft_testkit::designs::{RED, automatic, fill, letter, planned, rules_said};

/// The rules that an automatic red fill of `outline` sews its parts by, as `SC-I0912` names them.
fn rule(outline: &[(f64, f64)]) -> String {
    let said = rules_said(&planned(vec![automatic(fill("a", &[outline], &RED, &[]))]));
    let [(_, message)] = said.as_slice() else { panic!("one rule said: {said:?}") };
    match message.split_once("by the rule ") {
        Some((_, rest)) => rest.split([' ', '.']).next().unwrap().to_string(),
        None => message.clone(),
    }
}

/// `ring` with a point every `step` mm along its edges, or closer: the same outline, drawn with more points.
fn dense(ring: &[(f64, f64)], step: f64) -> Vec<(f64, f64)> {
    let mut out = Vec::new();
    for (i, &(x0, y0)) in ring.iter().enumerate() {
        let (x1, y1) = ring[(i + 1) % ring.len()];
        // A few hundred pieces at most: the cast can't truncate.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let n = (hypot(x1 - x0, y1 - y0) / step).ceil().max(1.0) as usize;
        out.extend((0..n).map(|k| {
            let t = k as f64 / n as f64;
            (x0 + (x1 - x0) * t, y0 + (y1 - y0) * t)
        }));
    }
    out
}

/// `ring` with its corner `at` moved `by` mm at `degrees` from the x axis.
fn moved(ring: &[(f64, f64)], at: usize, by: f64, degrees: f64) -> Vec<(f64, f64)> {
    let (sin, cos) = sin_cos(degrees.to_radians());
    let mut out = ring.to_vec();
    out[at] = (ring[at].0 + by * cos, ring[at].1 + by * sin);
    out
}

/// `ring` started at its corner `at`.
fn started(ring: &[(f64, f64)], at: usize) -> Vec<(f64, f64)> {
    ring[at..].iter().chain(&ring[..at]).copied().collect()
}

/// A bar 2 mm wide and 12 mm long.
const BAR: [(f64, f64); 4] = [(0.0, 0.0), (2.0, 0.0), (2.0, 12.0), (0.0, 12.0)];
/// A block T, 9 mm wide and 12 mm tall, in strokes 2 mm wide.
const T: [(f64, f64); 8] = [(0.0, 0.0), (9.0, 0.0), (9.0, 2.0), (5.5, 2.0), (5.5, 12.0), (3.5, 12.0), (3.5, 2.0), (0.0, 2.0)];
/// A block L, 8 mm wide and 12 mm tall, in strokes 2 mm wide.
const L: [(f64, f64); 6] = [(0.0, 0.0), (2.0, 0.0), (2.0, 10.0), (8.0, 10.0), (8.0, 12.0), (0.0, 12.0)];
/// A wedge 10 mm long, 2.4 mm wide at its base and 0.8 mm at its end.
const WEDGE: [(f64, f64); 4] = [(0.0, 0.0), (2.4, 0.0), (1.6, 10.0), (0.8, 10.0)];

/// A quarter of a ring between radii 5 and 7 mm, each arc in `pieces` straight pieces.
fn quarter_ring(pieces: u32) -> Vec<(f64, f64)> {
    let at = |r: f64, i: u32| {
        let (sin, cos) = sin_cos(std::f64::consts::FRAC_PI_2 * f64::from(i) / f64::from(pieces));
        (10.0 + r * cos, 10.0 + r * sin)
    };
    (0..=pieces).map(|i| at(7.0, i)).chain((0..=pieces).rev().map(|i| at(5.0, i))).collect()
}

/// `REQ-AUTO-135`: a bar whose end leans 0.02 mm sews as a satin column drawn with its 4 corners, and so it does
/// with a point every 0.1 mm along its edges. Before M13.53 the points along its end fanned its last stitches
/// into a corner, 2.98 mm of one edge to 1 mm of the other, and the bar was refused and filled.
#[test]
fn req_auto_135_points_along_a_straight_edge_change_no_rule() {
    let leaning = [(0.0, 0.0), (2.0, 0.02), (2.0, 12.0), (0.0, 12.0)];
    assert_eq!(rule(&leaning), "column");
    assert_eq!(rule(&dense(&leaning, 0.1)), "column");
    // A wedge's base fanned so too, 3.4 mm of one edge to 1 mm of the other.
    assert_eq!(rule(&WEDGE), "column");
    assert_eq!(rule(&dense(&WEDGE, 0.1)), "column");
}

/// `REQ-AUTO-135`: a quarter ring drawn with 12 straight pieces to each arc sews as it does drawn with 96, and
/// with a point every 0.1 mm. Before M13.53 the points added along its arcs made it refused, its stitches
/// crowding 2.66 mm of one edge to 1 mm of the other.
#[test]
fn req_auto_135_a_curve_drawn_with_more_points_keeps_its_rule() {
    let few = rule(&quarter_ring(12));
    assert_eq!(few, "column");
    assert_eq!(rule(&quarter_ring(96)), few);
    assert_eq!(rule(&dense(&quarter_ring(96), 0.1)), few);
}

/// `REQ-AUTO-136`: a corner 0.02 mm from square ends a bar as a square corner does. Before M13.53 a corner moved
/// so left its 2 neighbours at 89.6°, outside the circle on the end's chord: the end was a tip, the bar's
/// centre line ran into a corner, the other corner lay 1.4 times as far from the line as the bar's widest
/// circle reaches, and the bar sewed by the rule arms, as a tatami fill.
#[test]
fn req_auto_136_a_corner_a_hair_from_square_ends_a_column_as_a_square_one_does() {
    assert_eq!(rule(&BAR), "column");
    for at in 0..BAR.len() {
        for step in 0..8 {
            let degrees = 45.0 * f64::from(step);
            assert_eq!(rule(&moved(&BAR, at, 0.02, degrees)), "column", "the bar's corner {at} moved 0.02 mm at {degrees}°");
        }
    }
    // The arm of a T's bar likewise.
    assert_eq!(rule(&moved(&T, 0, 0.02, 45.0)), "column");
}

/// `REQ-AUTO-137`: an L or a T with a corner of its end moved 0.02 mm sews as satin columns, as it does drawn
/// square. Before M13.53 the column drawn along its end, read back as a satin column is read, paired one rung
/// otherwise, and the whole part was refused and filled.
#[test]
fn req_auto_137_a_corner_moved_a_hair_doesnt_refuse_a_part_of_columns() {
    assert_eq!(rule(&L), "column");
    assert_eq!(rule(&moved(&L, 1, 0.02, 45.0)), "column");
    assert_eq!(rule(&moved(&L, 1, 0.02, 315.0)), "column");
    assert_eq!(rule(&T), "column");
    for (at, degrees) in [(1, 45.0), (1, 135.0), (3, 135.0), (6, 315.0), (7, 225.0), (7, 270.0), (7, 315.0)] {
        assert_eq!(rule(&moved(&T, at, 0.02, degrees)), "column", "the T's corner {at} moved 0.02 mm at {degrees}°");
    }
}

/// `REQ-AUTO-137`: a ring started at another of its corners sews by the same rule. Before M13.53 a T whose
/// stem meets its bar 0.02 mm off square, started at its sixth corner, was refused and filled.
#[test]
fn req_auto_137_a_ring_started_at_another_corner_keeps_its_rule() {
    let joint = moved(&T, 3, 0.02, 90.0);
    let drawn = rule(&joint);
    assert_eq!(drawn, "column");
    for at in 1..joint.len() {
        assert_eq!(rule(&started(&joint, at)), drawn, "started at corner {at}");
    }
}

/// `REQ-AUTO-138`: block letters, bars and wedges drawn again in every way the other cases draw them sew by
/// the rule they sew by as drawn: with a point every 0.1 mm, with any one corner moved 0.02 mm in 8 directions,
/// started at each corner, and drawn the other way round.
#[test]
fn req_auto_138_shapes_drawn_again_within_the_tolerance_keep_their_rules() {
    let mut shapes: Vec<(String, Vec<(f64, f64)>)> =
        vec![("bar".into(), BAR.to_vec()), ("T".into(), T.to_vec()), ("L".into(), L.to_vec()), ("wedge".into(), WEDGE.to_vec())];
    for c in ['T', 'H', 'E'] {
        for (height, width) in [(6.0, 1.0), (12.0, 2.0)] {
            shapes.push((format!("{c} {height} mm"), letter(c, height, width)));
        }
    }
    let mut changed = Vec::new();
    for (name, ring) in &shapes {
        let drawn = rule(ring);
        let mut again = vec![("dense".to_string(), dense(ring, 0.1)), ("reversed".to_string(), ring.iter().rev().copied().collect())];
        for at in 0..ring.len() {
            again.push((format!("started at {at}"), started(ring, at)));
            for step in 0..8 {
                again.push((format!("corner {at} moved at {}°", 45 * step), moved(ring, at, 0.02, 45.0 * f64::from(step))));
            }
        }
        for (how, redrawn) in again {
            let got = rule(&redrawn);
            if got != drawn {
                changed.push(format!("{name}, {how}: {drawn} -> {got}"));
            }
        }
    }
    assert!(changed.is_empty(), "{} redrawings changed rule:\n{}", changed.len(), changed.join("\n"));
}
