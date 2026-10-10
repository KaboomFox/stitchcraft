//! A satin column's rails, turned the way they are sewn and cut into sections
//! (`docs/src/design/algorithms/satin.md` › Orientation and › Correspondence).
//!
//! **Orientation.** The stitches go across from one rail to the other, and the column is sewn from the
//! rails' starts to their ends, so the rails must run the same way. `swap_satin_rails` first makes the
//! second rail the first, which sews first in each pair. `reverse_rails` then turns rails round: `first`,
//! `second` or `both`, or with `automatic`, the second when that brings the rails closer together, as
//! Ink/Stitch decides it. Points at every tenth of each rail's length, from its start to 90 %, are paired
//! with the second rail forwards and then backwards, and the second rail turns when the backward
//! distances add up to less.
//!
//! **Sections.** Every rung cuts each rail at the distance along it of the rung's point on it. Each rail
//! is cut at its own distances, in order, and its n-th part goes with the other rail's n-th part: a
//! section, sewn as one stretch. A part of no length, where two rungs meet a rail at one point or a rung
//! meets it at an end, leaves its section out. A column whose path draws no rungs cuts its rails at their
//! nodes instead, as Ink/Stitch does: the 2nd node of one rail with the 2nd of the other, and on in order,
//! without the rails' ends. Rails with different numbers of nodes pair as many as the one with fewer has
//! (`SC-W0210`), and rails of 2 nodes each are cut once, 0.2 CSS pixels from their starts, which sews them
//! as one section.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::normalize::along::Along;
use crate::normalize::satin::{Pairing, Satin};

/// Where rails of 2 nodes each are cut, in millimetres from their starts: 0.2 CSS pixels, Ink/Stitch's.
const NEAR_START: f64 = 0.2 * 25.4 / 96.0;

/// A section: the parts of the first and of the second rail between two neighbouring cuts.
pub(crate) type Section = [Vec<Point>; 2];

/// `satin`'s rails, swapped when `swap` says so and turned as `reverse` says, cut into sections. What was
/// paired by fewer nodes than drawn goes to `warnings`. Measuring the rails costs `meter` a unit of work per
/// point, and projecting a cut onto a rail one per side of it.
pub(crate) fn sections(
    satin: &Satin,
    swap: bool,
    reverse: &str,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Section>, Exhausted> {
    let mut rails = satin.rails.clone();
    let mut pairing = satin.pairing.clone();
    if swap {
        rails.swap(0, 1);
        match &mut pairing {
            Pairing::Rungs(rungs) => rungs.iter_mut().for_each(|rung| rung.swap(0, 1)),
            Pairing::Nodes(nodes) => nodes.swap(0, 1),
        }
    }
    let turned = match reverse {
        "first" => [true, false],
        "second" => [false, true],
        "both" => [true, true],
        "automatic" => [false, backwards(&rails, meter)?],
        _ => [false, false],
    };
    for (rail, turn) in rails.iter_mut().zip(turned) {
        if turn {
            rail.reverse();
        }
    }
    if let Pairing::Nodes(nodes) = &mut pairing {
        for (nodes, turn) in nodes.iter_mut().zip(turned) {
            if turn {
                nodes.reverse();
            }
        }
    }
    let [first, second] = &rails;
    let along = [Along::new(first, meter)?, Along::new(second, meter)?];
    let [rail_a, rail_b] = &along;
    let (mut cuts_a, mut cuts_b) = (Vec::new(), Vec::new());
    for [a, b] in pairs(&pairing, &along, warnings) {
        cuts_a.push(rail_a.project(a, meter)?);
        cuts_b.push(rail_b.project(b, meter)?);
    }
    let (parts_a, parts_b) = (parts(rail_a, &mut cuts_a), parts(rail_b, &mut cuts_b));
    Ok(parts_a.into_iter().zip(parts_b).filter_map(|(a, b)| Some([a?, b?])).collect())
}

/// Whether the second of `rails` runs against the first, as Ink/Stitch judges it: the distances between
/// points at the same tenth of each rail's length add up to more than with the second rail backwards.
fn backwards(rails: &[Vec<Point>; 2], meter: &mut Meter) -> Result<bool, Exhausted> {
    let [first, second] = rails;
    let (a, b) = (Along::new(first, meter)?, Along::new(second, meter)?);
    let (mut forwards, mut back) = (0.0, 0.0);
    for tenth in 0..10 {
        let t = f64::from(tenth) / 10.0;
        let on_a = a.point(t * a.length());
        forwards += on_a.distance(b.point(t * b.length()));
        back += on_a.distance(b.point((1.0 - t) * b.length()));
    }
    Ok(forwards > back)
}

/// The points that `pairing` says go together, on the rails `along`: each rung's, or the rails' nodes
/// inside their ends, with `SC-W0210` when there are more nodes on one rail than on the other.
fn pairs(pairing: &Pairing, along: &[Along; 2], warnings: &mut Vec<Diagnostic>) -> Vec<[Point; 2]> {
    let nodes = match pairing {
        Pairing::Rungs(rungs) => return rungs.clone(),
        Pairing::Nodes(nodes) => nodes,
    };
    let [first, second] = nodes;
    if first.len() != second.len() {
        let message = format!(
            "This satin column has no rungs, and its rails have {} and {} nodes, so they pair up only as far as the rail with fewer goes.",
            first.len(),
            second.len()
        );
        warnings.push(Diagnostic::new(Code::SatinNodesUnequal, message).with_fix(Fix::Hint("Add rungs across the column.".to_string())));
    } else if first.len() <= 2 {
        let [a, b] = along;
        return vec![[a.point(NEAR_START), b.point(NEAR_START)]];
    }
    let inside = |nodes: &[Point]| nodes.get(1..nodes.len().saturating_sub(1)).unwrap_or_default().to_vec();
    inside(first).into_iter().zip(inside(second)).map(|(a, b)| [a, b]).collect()
}

/// The parts of the rail `along` between its `cuts`, in order along it; a part of no length is `None`.
fn parts(along: &Along, cuts: &mut [f64]) -> Vec<Option<Vec<Point>>> {
    cuts.sort_by(f64::total_cmp);
    let length = along.length();
    let ends: Vec<f64> = std::iter::once(0.0).chain(cuts.iter().map(|cut| cut.clamp(0.0, length))).chain(std::iter::once(length)).collect();
    ends.iter().zip(ends.iter().skip(1)).map(|(from, to)| (from < to).then(|| along.part(*from, *to))).collect()
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn a_rail_is_cut_into_its_parts_between_the_cuts_in_order() {
        let points = [p(0.0, 0.0), p(10.0, 0.0)];
        let along = Along::new(&points, &mut Budget::DEFAULT.meter()).unwrap();
        let mut cuts = vec![7.0, 3.0];
        assert_eq!(
            parts(&along, &mut cuts),
            [Some(vec![p(0.0, 0.0), p(3.0, 0.0)]), Some(vec![p(3.0, 0.0), p(7.0, 0.0)]), Some(vec![p(7.0, 0.0), p(10.0, 0.0)])]
        );
        // Cuts at an end, and two at one point, leave parts of no length.
        let mut cuts = vec![0.0, 5.0, 5.0, 10.0];
        assert_eq!(parts(&along, &mut cuts), [None, Some(vec![p(0.0, 0.0), p(5.0, 0.0)]), None, Some(vec![p(5.0, 0.0), p(10.0, 0.0)]), None]);
    }
}
