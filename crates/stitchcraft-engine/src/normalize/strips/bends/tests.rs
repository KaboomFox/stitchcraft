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
    for h in [6.0, 12.0, 25.0] {
        let part = v(h);
        let perimeter: f64 = part.outline.windows(2).map(|w| w[0].distance(w[1])).sum();
        let longest = 2.0 * part.area() / perimeter;
        let found = columns(&part, longest, &[], 0.5, &mut Budget::DEFAULT.meter()).unwrap().unwrap().unwrap();
        eprintln!("h {h} columns {}", found.len());
        for c in &found {
            eprintln!(
                "  under {:?} line {:?} .. {:?} n {} rungs {}",
                c.under,
                c.line.points.first(),
                c.line.points.last(),
                c.line.points.len(),
                c.rungs.len()
            );
        }
        let one = &found[1];
        eprintln!(
            "  walked1 last {:?} walked0 {:?} .. {:?}",
            one.walked.points.last(),
            found[0].walked.points.first(),
            found[0].walked.points.last()
        );
        eprintln!("  rails1 ends {:?} {:?}", one.rails[0].last(), one.rails[1].last());
        eprintln!("  outline0 {:?}", found[0].outline().outline);
        let j = joined(one, 1, &found[0]);
        eprintln!("  joined {:?}", j.as_ref().map(|(c, at)| (c.line.points.len(), *at, c.line.length())));
        if let Some((c, at)) = &j {
            let r = cut(&part, c, Some(*at), &mut Budget::DEFAULT.meter()).unwrap();
            eprintln!("  cut {}", r.is_some());
        }
        let cut = bends(&part, &found, &mut Budget::DEFAULT.meter()).unwrap();
        eprintln!("  after {}", cut.len());
    }
}
