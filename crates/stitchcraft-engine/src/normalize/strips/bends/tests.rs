//! Cases of a column cut at a bend where 2 strokes meet (`REQ-AUTO-115`).

use stitchcraft_core::Budget;

use super::*;
use crate::normalize::fixture::part;
use crate::normalize::strips::columns;

/// A V 12 mm tall whose legs narrow from 0.2 of its height to 0.14 where they meet in a flat foot.
fn v(height: f64) -> Polygon {
    let ring = [(0.0, 0.0), (0.2, 0.0), (0.42, 0.72), (0.64, 0.0), (0.84, 0.0), (0.49, 1.0), (0.35, 1.0)];
    part(&ring.map(|(x, y)| (x * height, y * height)), &[])
}

fn found(part: &Polygon) -> Vec<Column> {
    columns(part, 1.8, &[], 0.5, &mut Budget::DEFAULT.meter()).unwrap().unwrap().unwrap()
}

#[test]
fn probe() {
    if std::env::var_os("BENDS_DEBUG").is_none() {
        return;
    }
    for h in [6.0, 12.0] {
        let part = v(h);
        let perimeter: f64 = part.outline.windows(2).map(|w| w[0].distance(w[1])).sum();
        let longest = 2.0 * part.area() / perimeter;
        let found = columns(&part, longest, &[], 1.0, &mut Budget::DEFAULT.meter()).unwrap().unwrap().unwrap();
        eprintln!("h {h} columns {}", found.len());
        let cut = bends(&part, &found, 0.4, &mut |_, _| Ok(false), &mut Budget::DEFAULT.meter()).unwrap();
        eprintln!("  after {}", cut.len());
    }
}

#[test]
fn debug_letter() {
    let (Some(set), Some(name)) = (std::env::var_os("BENDS_SET"), std::env::var_os("BENDS_LETTER")) else { return };
    let text = std::fs::read_to_string(set).unwrap();
    let mut rings: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut on = false;
    for line in text.lines() {
        let mut words = line.split_whitespace();
        match words.next() {
            Some("letter") => on = words.next() == name.to_str(),
            Some("ring") if on => rings.push(
                words
                    .map(|w| {
                        let (x, y) = w.split_once(',').unwrap();
                        (x.parse().unwrap(), y.parse().unwrap())
                    })
                    .collect(),
            ),
            _ => {}
        }
    }
    let holes: Vec<&[(f64, f64)]> = rings[1..].iter().map(Vec::as_slice).collect();
    let part = part(&rings[0], &holes);
    let perimeter: f64 = part.rings().map(|r| r.windows(2).map(|w| w[0].distance(w[1])).sum::<f64>()).sum();
    let longest = (2.0 * part.area() / perimeter).max(0.1);
    let found = columns(&part, longest, &[], 0.5, &mut Budget::DEFAULT.meter()).unwrap().unwrap().unwrap();
    for (i, c) in found.iter().enumerate() {
        eprintln!("column {i} under {:?} line {:?} .. {:?} closed {}", c.under, c.line.points.first(), c.line.points.last(), c.closed);
    }
    let cut = bends(&part, &found, 0.4, &mut |_, _| Ok(false), &mut Budget::DEFAULT.meter()).unwrap();
    eprintln!("after {}", cut.len());
}
