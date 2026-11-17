//! Cases of a part read as strokes (`REQ-AUTO-087`, `REQ-AUTO-088`).

use stitchcraft_core::Budget;

use super::*;
use crate::normalize::fixture::part;
use crate::normalize::strips::columns;

/// The traced word's M, `height` mm tall.
fn m(height: f64) -> Polygon {
    let ring = [(0.0, 1.0), (0.0, 0.0), (0.22, 0.0), (0.46, 0.70), (0.70, 0.0), (0.92, 0.0), (0.92, 1.0), (0.74, 1.0), (0.74, 0.4385), (0.5475, 1.0), (0.3725, 1.0), (0.18, 0.4385), (0.18, 1.0)];
    let ring: Vec<(f64, f64)> = ring.iter().map(|&(x, y)| (x * height, y * height)).collect();
    part(&ring, &[])
}

/// The traced word's A, `height` mm tall.
fn a(height: f64) -> Polygon {
    let scale = |ring: &[(f64, f64)]| -> Vec<(f64, f64)> { ring.iter().map(|&(x, y)| (x * height, y * height)).collect() };
    let outline = scale(&[(0.0, 1.0), (0.35, 0.0), (0.51, 0.0), (0.86, 1.0), (0.6693, 1.0), (0.5923, 0.78), (0.2677, 0.78), (0.1907, 1.0)]);
    let hole = scale(&[(0.43, 0.3163), (0.3307, 0.6), (0.5293, 0.6)]);
    part(&outline, &[&hole])
}

#[test]
fn debug_m() {
    let part = if std::env::var("LETTER_A").is_ok() { a(10.0) } else { m(10.0) };
    let meter = &mut Budget::DEFAULT.meter();
    let found = columns(&part, 1.8, &[], 1.0, meter).unwrap().unwrap().unwrap();
    println!("found {}", found.len());
    let mut runs = Vec::new();
    for ring in std::iter::once(&part.outline).chain(&part.holes) { runs.extend(sides(ring, meter).unwrap()); }
    for r in &runs { println!("side {:?} -> {:?}", (r.a.x(), r.a.y()), (r.b.x(), r.b.y())); }
    for (i, x) in runs.iter().enumerate() { for y in runs.iter().skip(i + 1) { if let Some((b, o)) = paired(*x, *y, (1.0, 7.0)) { println!("pair {:?}-{:?} & {:?}-{:?}: held {:?}", (x.a.x(), x.a.y()), (x.b.x(), x.b.y()), (y.a.x(), y.a.y()), (y.b.x(), y.b.y()), held(&part, &b, o, meter).unwrap()); } } }
    let bands = bands(&part, &runs, (1.0, 7.0), meter).unwrap().unwrap();
    for b in &bands { println!("band {b:?}"); }
    let joined = joins(&part, &bands, 0.4, meter).unwrap();
    println!("joined {joined:?}");
    let Some(joined) = joined else { return };
    let pieces = pieces(&part, &bands, &joined, meter).unwrap();
    println!("pieces {:?}", pieces.as_ref().map(|p| p.iter().map(|q| q.len()).collect::<Vec<_>>()));
    let Some(pieces) = pieces else { return };
    for (band, mine) in bands.iter().zip(&pieces) {
        for piece in mine {
            let ring: Vec<(f64, f64)> = piece.outline.iter().map(|p| ((p.x() * 100.0).round() / 100.0, (p.y() * 100.0).round() / 100.0)).collect();
            let along: Vec<f64> = piece.outline.iter().map(|&p| (band.along(p) * 100.0).round() / 100.0).collect();
            println!("piece {ring:?}\n along {along:?}");
            let made = column(&part, piece, band, meter).unwrap();
            println!(" column {:?}", made.as_ref().map(|(r, g, _)| (r[0].len(), r[1].len(), g.len())));
        }
    }
    let cols = columns_of(&part, &bands, &joined, &pieces, meter).unwrap();
    println!("columns {:?}", cols.as_ref().map(|c| c.len()));
    for c in cols.unwrap() {
        let made = crate::generators::satin::drawn::from_rails(c.rails.clone(), c.rungs.clone(), [0.0, 0.0], meter).unwrap();
        println!("drawn {:?} rails {:?} rungs {:?}", made.as_ref().err(), c.rails.iter().map(|r| r.iter().map(|p| ((p.x()*100.0).round()/100.0, (p.y()*100.0).round()/100.0)).collect::<Vec<_>>()).collect::<Vec<_>>(), c.rungs.iter().map(|r| r.iter().map(|p| ((p.x()*100.0).round()/100.0, (p.y()*100.0).round()/100.0)).collect::<Vec<_>>()).collect::<Vec<_>>());
    }
}
