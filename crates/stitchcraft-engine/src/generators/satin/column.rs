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
//! (`SC-W0210`). Rails of 2 nodes each are cut once near their starts, which sews them as one section:
//! where the point 0.2 CSS pixels along the straight line from each rail's first node to its last lies
//! along the rail, as Ink/Stitch places the rung it adds there.
//!
//! **Push compensation** shortens or lengthens the rails at the column's start and end after they are
//! turned and before they are cut, as in Ink/Stitch (the `compensation` module). The points that say where
//! to cut come from the rails as drawn, and each cut is where its point lies along the compensated rail:
//! a cut in a part taken off falls on the rail's end, and leaves its section out.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::generators::satin::SatinParams;
use crate::generators::satin::compensation::pushed;
use crate::normalize::along::Along;
use crate::normalize::satin::{Pairing, Satin};

/// How far along the line between a rail's 2 nodes the point is that says where to cut it, in
/// millimetres: 0.2 CSS pixels, Ink/Stitch's.
const NEAR_START: f64 = 0.2 * 25.4 / 96.0;

/// A section: the parts of the first and of the second rail between two neighbouring cuts.
pub(crate) type Section = [Vec<Point>; 2];

/// `satin`'s rails, swapped and turned as `params` say, with its push compensation, and cut into
/// sections. What was paired by fewer nodes than drawn, and a push compensation too long for a rail, go to
/// `warnings`. Measuring the rails costs `meter` a unit of work per point, and projecting a cut onto a
/// rail one per side of it.
pub(crate) fn sections(satin: &Satin, params: &SatinParams, warnings: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<Vec<Section>, Exhausted> {
    let mut rails = satin.rails.clone();
    let mut pairing = satin.pairing.clone();
    if params.swap_satin_rails {
        rails.swap(0, 1);
        match &mut pairing {
            Pairing::Rungs(rungs) => rungs.iter_mut().for_each(|rung| rung.swap(0, 1)),
            Pairing::Nodes(nodes) => nodes.swap(0, 1),
        }
    }
    let turned = match params.reverse_rails {
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
    let push = params.push_compensation_mm.map(|mm| mm.get());
    let ((first, kept_a), (second, kept_b)) = (pushed(first, push, meter)?, pushed(second, push, meter)?);
    if kept_a || kept_b {
        warnings.push(too_long(push));
    }
    let [rail_a, rail_b] = [Along::new(&first, meter)?, Along::new(&second, meter)?];
    let (mut cuts_a, mut cuts_b) = (Vec::new(), Vec::new());
    for [a, b] in pairs(&pairing, warnings) {
        cuts_a.push(rail_a.project(a, meter)?);
        cuts_b.push(rail_b.project(b, meter)?);
    }
    let (parts_a, parts_b) = (parts(&rail_a, &mut cuts_a), parts(&rail_b, &mut cuts_b));
    Ok(parts_a.into_iter().zip(parts_b).filter_map(|(a, b)| Some([a?, b?])).collect())
}

/// `SC-W0211`, for a push compensation of `start` and `end` millimetres that would leave too little of a
/// rail.
fn too_long([start, end]: [f64; 2]) -> Diagnostic {
    let message = format!(
        "This satin column's push compensation, {start} mm at its start and {end} mm at its end, would leave less than 0.13 mm of a rail, so that rail keeps its length."
    );
    Diagnostic::new(Code::SatinPushTooLong, message).with_fix(Fix::Hint("Lower `push_compensation_mm`.".to_string()))
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

/// The points that `pairing` says go together: each rung's, or the rails' nodes inside their ends, with
/// `SC-W0210` when there are more nodes on one rail than on the other.
fn pairs(pairing: &Pairing, warnings: &mut Vec<Diagnostic>) -> Vec<[Point; 2]> {
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
        return near_start(first).zip(near_start(second)).map(|(a, b)| vec![[a, b]]).unwrap_or_default();
    }
    let inside = |nodes: &[Point]| nodes.get(1..nodes.len().saturating_sub(1)).unwrap_or_default().to_vec();
    inside(first).into_iter().zip(inside(second)).map(|(a, b)| [a, b]).collect()
}

/// The point [`NEAR_START`] along the straight line from the first of a rail's `nodes` to its last, held
/// to the last; `None` without nodes.
fn near_start(nodes: &[Point]) -> Option<Point> {
    let (first, last) = (*nodes.first()?, *nodes.last()?);
    Some(first.lerp(last, NEAR_START / first.distance(last)))
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

    #[test]
    fn rails_of_2_nodes_pair_near_their_starts_on_the_lines_between_their_nodes() {
        let nodes = |a: [(f64, f64); 2], b: [(f64, f64); 2]| Pairing::Nodes([a.map(|(x, y)| p(x, y)).to_vec(), b.map(|(x, y)| p(x, y)).to_vec()]);
        let mut warnings = Vec::new();
        let [[a, b]] = pairs(&nodes([(0.0, 0.0), (3.0, 4.0)], [(0.0, 10.0), (-6.0, 18.0)]), &mut warnings)[..] else { panic!() };
        assert!(a.distance(p(0.6 * NEAR_START, 0.8 * NEAR_START)) < 1e-15 && b.distance(p(-0.6 * NEAR_START, 10.0 + 0.8 * NEAR_START)) < 1e-15);
        // Nodes closer together than that: the last one. Nodes at one point: that point.
        let [[a, b]] = pairs(&nodes([(0.0, 0.0), (0.01, 0.0)], [(5.0, 5.0), (5.0, 5.0)]), &mut warnings)[..] else { panic!() };
        assert_eq!((a, b), (p(0.01, 0.0), p(5.0, 5.0)));
        assert!(warnings.is_empty());
        assert_eq!(near_start(&[]), None);
    }

    #[test]
    fn a_rail_is_backwards_when_its_tenths_lie_nearer_the_other_rail_turned() {
        let rails = |a: &[(f64, f64)], b: &[(f64, f64)]| [a.iter().map(|&(x, y)| p(x, y)).collect(), b.iter().map(|&(x, y)| p(x, y)).collect()];
        let backwards = |a: &[(f64, f64)], b: &[(f64, f64)]| backwards(&rails(a, b), &mut Budget::DEFAULT.meter()).unwrap();
        // Rails whose verdict turns on where along them the points are taken: each tenth from the start
        // to 9 tenths, of both rails, and the second rail's from its end when it is turned.
        assert!(backwards(&[(9.0, 4.0), (0.0, 8.0), (1.0, 6.0)], &[(0.0, 4.0), (1.0, 1.0)]));
        assert!(!backwards(&[(1.0, 6.0), (9.0, 6.0), (6.0, 1.0)], &[(0.0, 2.0), (6.0, 1.0), (4.0, 5.0)]));
        assert!(backwards(&[(6.0, 6.0), (5.0, 8.0), (9.0, 5.0)], &[(1.0, 7.0), (6.0, 10.0)]));
        // As near one way as the other: not turned, as in Ink/Stitch. The tenths fall on the rails'
        // points, so the distances are exactly equal.
        let across: Vec<(f64, f64)> = (0..=10).map(|i| (10.0 - f64::from(i), 5.0)).collect();
        let down: Vec<(f64, f64)> = (0..=10).map(|i| (20.0, f64::from(i))).collect();
        assert!(!backwards(&across, &down));
    }
}
