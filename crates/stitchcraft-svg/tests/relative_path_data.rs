//! Fills whose shapes share corners and edges, written with relative path data as Inkscape writes a
//! combined path (`REQ-FILL-001`). Every coordinate is a decimal of 4 places, so the shapes tile exactly in
//! decimal arithmetic; only the reader's sums of the relative moves leave a shared corner a few units in the
//! last place off itself. Each fill covers its tiling's area, in 1 part where its shapes share edges.

// Test code may unwrap and index (clippy.toml allows it in tests); a drawing's coordinates are whole numbers
// of units of 1e-4 mm, rounded from random floats.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::cast_possible_truncation)]

use std::fmt::Write as _;

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Code, SplitMix64};
use stitchcraft_engine::design::{FillRule, Shape};
use stitchcraft_engine::normalize::region::build;
use stitchcraft_svg::read;

/// A point in whole units of 1e-4 mm.
type Units = (i64, i64);

/// `units` in millimetres, as path data writes it: at most 4 decimal places.
fn mm(units: i64) -> String {
    let sign = if units < 0 { "-" } else { "" };
    let (whole, part) = (units.unsigned_abs() / 10_000, units.unsigned_abs() % 10_000);
    if part == 0 { format!("{sign}{whole}") } else { format!("{sign}{whole}.{}", format!("{part:04}").trim_end_matches('0')) }
}

/// Path data drawing `shapes` one after another with relative moves only, as Inkscape writes a combined
/// path: each subpath's `m` from the start of the one before, where `z` leaves the pen.
fn relative(shapes: &[Vec<Units>]) -> String {
    let mut d = String::new();
    let mut pen = (0, 0);
    for shape in shapes {
        let start = shape[0];
        write!(d, "m {},{} ", mm(start.0 - pen.0), mm(start.1 - pen.1)).unwrap();
        let mut at = start;
        for &q in &shape[1..] {
            write!(d, "l {},{} ", mm(q.0 - at.0), mm(q.1 - at.1)).unwrap();
            at = q;
        }
        d.push_str("z ");
        pen = start;
    }
    d
}

fn add(a: Units, b: Units) -> Units {
    (a.0 + b.0, a.1 + b.1)
}

fn times(a: Units, k: i64) -> Units {
    (a.0 * k, a.1 * k)
}

/// `a` turned a quarter round.
fn turned(a: Units) -> Units {
    (-a.1, a.0)
}

/// About `length` mm at `angle`, in units.
fn toward(angle: f64, length: f64) -> Units {
    let (sin, cos) = sin_cos(angle);
    ((length * cos * 1e4).round() as i64, (length * sin * 1e4).round() as i64)
}

/// Twice the shape's area, in square units.
fn twice_area(shape: &[Units]) -> i128 {
    let next = shape.iter().skip(1).chain(shape.first());
    shape.iter().zip(next).map(|(a, b)| i128::from(a.0) * i128::from(b.1) - i128::from(b.0) * i128::from(a.1)).sum()
}

/// The kinds of drawing: tilings whose shapes share edges, and ones whose shapes touch only at corners.
const KINDS: [&str; 6] = ["grid", "checker", "pinwheel", "fan", "corners", "triangles"];

/// A random drawing of `kind`, and whether its shapes make 1 part.
fn drawing(kind: &str, rng: &mut SplitMix64) -> (Vec<Vec<Units>>, bool) {
    let origin = ((rng.next_in(80.0, 200.0) * 1e4).round() as i64, (rng.next_in(80.0, 200.0) * 1e4).round() as i64);
    let tau = std::f64::consts::TAU;
    match kind {
        // Squares in rows and columns, every other one, or each cut into 2 triangles.
        "grid" | "checker" | "triangles" => {
            let u = toward(rng.next_in(0.0, tau), rng.next_in(2.0, 9.0));
            let v = turned(u);
            let (n, m) = (2 + rng.next_u64() % 5, 2 + rng.next_u64() % 5);
            let mut shapes = Vec::new();
            for j in 0..m as i64 {
                for i in 0..n as i64 {
                    if kind == "checker" && (i + j) % 2 == 1 {
                        continue;
                    }
                    let o = add(origin, add(times(u, i), times(v, j)));
                    let corners = [o, add(o, u), add(add(o, u), v), add(o, v)];
                    if kind == "triangles" {
                        shapes.extend([vec![corners[0], corners[1], corners[2]], vec![corners[0], corners[2], corners[3]]]);
                    } else {
                        shapes.push(corners.to_vec());
                    }
                }
            }
            (shapes, kind != "checker")
        }
        // Triangles meeting at a point: all round it, or every other one.
        "pinwheel" | "fan" => {
            let k = 3 + rng.next_u64() as usize % 6;
            let (r, start) = (rng.next_in(5.0, 30.0), rng.next_in(0.0, tau));
            let rim: Vec<Units> = (0..2 * k)
                .map(|t| add(origin, toward(start + tau * t as f64 / (2 * k) as f64 + rng.next_in(-0.1, 0.1), r * rng.next_in(0.6, 1.0))))
                .collect();
            let shapes = (0..2 * k).filter(|t| kind == "fan" || t % 2 == 0).map(|t| vec![origin, rim[t], rim[(t + 1) % (2 * k)]]).collect();
            (shapes, kind == "fan")
        }
        // A square, and at each of its corners a triangle touching it only there.
        _ => {
            let u = toward(rng.next_in(0.0, tau), rng.next_in(4.0, 15.0));
            let v = turned(u);
            let corners = [origin, add(origin, u), add(add(origin, u), v), add(origin, v)];
            let mut shapes = vec![corners.to_vec()];
            let outward = [times(add(u, v), -1), add(u, times(v, -1)), add(u, v), add(times(u, -1), v)];
            for (&c, out) in corners.iter().zip(outward) {
                let s = rng.next_in(0.2, 0.6);
                let (side, reach) = (turned(out), ((out.0 as f64 * s) as i64, (out.1 as f64 * s) as i64));
                shapes.push(vec![c, add(c, add(reach, (side.0 / 5, side.1 / 5))), add(c, add(reach, (-side.0 / 5, -side.1 / 5)))]);
            }
            (shapes, false)
        }
    }
}

#[test]
fn req_fill_001_shapes_drawn_with_relative_moves_fill_their_area_in_1_part_where_they_share_edges() {
    let mut rng = SplitMix64::new(0xf111_2026);
    let mut wrong = Vec::new();
    for kind in KINDS {
        for case in 0..50 {
            let (mut shapes, one_part) = drawing(kind, &mut rng);
            // Each shape drawn from a random corner, either way round.
            for shape in &mut shapes {
                if rng.next_u64().is_multiple_of(2) {
                    shape.reverse();
                }
                let k = rng.next_u64() as usize % shape.len();
                shape.rotate_left(k);
            }
            let area: f64 = shapes.iter().map(|shape| twice_area(shape).unsigned_abs() as f64 / 2.0 * 1e-8).sum();
            let file = format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="400mm" height="400mm" viewBox="0 0 400 400"><path id="f" style="fill:#ff0000" d="{}"/></svg>"#,
                relative(&shapes)
            );
            let svg = read(file.as_bytes(), &Budget::DEFAULT).unwrap();
            let Shape::Fill { path, .. } = &svg.design.elements()[0].shape else { panic!("not a fill") };
            for rule in [FillRule::NonZero, FillRule::EvenOdd] {
                let built = build(path, rule, &mut Budget::DEFAULT.meter()).unwrap();
                let (covered, parts) = (built.region.area(), built.region.parts.len());
                let bug = built.diagnostics.iter().any(|d| d.code == Code::InternalCheckFailed);
                if (covered - area).abs() > 1e-6 * area || (one_part && parts != 1) || bug {
                    wrong.push(format!(
                        "{kind} {case} {rule:?}: {covered} mm² in {parts} parts, not {area} mm²{}",
                        if bug { ", SC-E0009" } else { "" }
                    ));
                }
            }
        }
    }
    assert!(wrong.is_empty(), "{wrong:#?}");
}
