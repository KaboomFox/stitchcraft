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
//! meets it at an end, leaves its section out.
//!
//! **Rungs added.** A column whose path draws no rungs gets rungs across its nodes, as Ink/Stitch adds
//! them: the 2nd node of one rail with the 2nd of the other, and on in order, without the rails' ends.
//! Rails with different numbers of nodes pair as many as the one with fewer has (`SC-W0210`). Rails of 2
//! nodes each get one rung near their starts, which sews them as one section: across the points 0.2 CSS
//! pixels along the straight line from each rail's first node to its last. Each rung is a tenth longer
//! than the line between its points, half at each end, and cuts each rail where it comes nearest it,
//! measured as Ink/Stitch's geometry library measures it (`normalize::near`): at a node it is the node,
//! and on a curved rail of 2 nodes it is where the rung crosses the curve, or its point nearest the rung.
//! The nodes are turned before the rails are swapped: `reverse_rails` turns the nodes of the rail drawn
//! in the place it names, first or second, and swapping the rails changes only which one sews first, as
//! in Ink/Stitch. Where the rails have different numbers of nodes, the longer rail's last nodes, in the
//! order the turning leaves them, go without a partner.
//!
//! **Push compensation** shortens or lengthens the rails at the column's start and end after they are
//! turned and before they are cut, as in Ink/Stitch (the `compensation` module). A drawn rung's points come
//! from the rails as drawn, and each cut is where its point lies along the compensated rail: a cut in a
//! part taken off falls on the rail's end, and leaves its section out. An added rung is measured against
//! the compensated rail.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::generators::satin::SatinParams;
use crate::generators::satin::compensation::pushed;
use crate::normalize::along::Along;
use crate::normalize::near::{Look, along_to_segment};
use crate::normalize::satin::{Pairing, Satin};

/// How far along the line between a rail's 2 nodes the rung added across rails of 2 nodes meets it, in
/// millimetres: 0.2 CSS pixels, Ink/Stitch's.
const NEAR_START: f64 = 0.2 * MM_PER_SVG_PX;

/// How many times as long as the line between its points a rung added across the nodes is: a tenth
/// longer, half at each end, as Ink/Stitch makes it so that it reaches the rails.
const ADDED_REACH: f64 = 1.1;

/// A section: the parts of the first and of the second rail between two neighbouring cuts.
pub(crate) type Section = [Vec<Point>; 2];

/// `satin`'s rails, swapped and turned as `params` say, with its push compensation, and cut into
/// sections. What was paired by fewer nodes than drawn, and a push compensation too long for a rail, go to
/// `warnings`. Measuring the rails costs `meter` a unit of work per point, projecting a drawn rung's point
/// onto a rail one per side of it, and measuring an added rung against a rail 2 per side.
pub(crate) fn sections(satin: &Satin, params: &SatinParams, warnings: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<Vec<Section>, Exhausted> {
    let (rails, turned) = oriented(satin, params, meter)?;
    let [first, second] = &rails;
    let push = params.push_compensation_mm.map(|mm| mm.get());
    let ((first, kept_a), (second, kept_b)) = (pushed(first, push, meter)?, pushed(second, push, meter)?);
    if kept_a || kept_b {
        warnings.push(too_long(push));
    }
    let [rail_a, rail_b] = [Along::new(&first, meter)?, Along::new(&second, meter)?];
    let (mut cuts_a, mut cuts_b) = (Vec::new(), Vec::new());
    match &satin.pairing {
        Pairing::Rungs(rungs) => {
            for &[a, b] in rungs {
                let [a, b] = if params.swap_satin_rails { [b, a] } else { [a, b] };
                cuts_a.push(rail_a.project(a, meter)?);
                cuts_b.push(rail_b.project(b, meter)?);
            }
        }
        Pairing::Nodes(nodes) => {
            for rung in added_rungs(nodes, turned, warnings) {
                cuts_a.push(along_to_segment(&first, rung, Look::FromSegment, meter)?);
                cuts_b.push(along_to_segment(&second, rung, Look::FromSegment, meter)?);
            }
        }
    }
    let (parts_a, parts_b) = (parts(&rail_a, &mut cuts_a), parts(&rail_b, &mut cuts_b));
    Ok(parts_a.into_iter().zip(parts_b).filter_map(|(a, b)| Some([a?, b?])).collect())
}

/// Where the column's first stitch is as its rails are drawn: the start of its first rail, after the swap
/// and the reversal `params` say (Ink/Stitch's first stitch of a column that does not start at its nearest
/// point). `None` when the rail has no point.
pub(crate) fn first_point(satin: &Satin, params: &SatinParams, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let [first, _] = sewn_rails(satin, params, meter)?;
    Ok(first.first().copied())
}

/// `satin`'s rails as they are sewn, swapped and turned as `params` say: the lines its neighbours measure
/// against, in the order they measure them.
pub(crate) fn sewn_rails(satin: &Satin, params: &SatinParams, meter: &mut Meter) -> Result<[Vec<Point>; 2], Exhausted> {
    Ok(oriented(satin, params, meter)?.0)
}

/// `satin`'s rails swapped and turned as `params` say, and which places, first and second, were turned.
fn oriented(satin: &Satin, params: &SatinParams, meter: &mut Meter) -> Result<([Vec<Point>; 2], [bool; 2]), Exhausted> {
    let mut rails = satin.rails.clone();
    if params.swap_satin_rails {
        rails.swap(0, 1);
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
    Ok((rails, turned))
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

/// The rungs added across a column whose rails, as the path draws them, have the `nodes`, each turned when
/// `turned` turns the rail sewn in its place: between the nodes inside the rails' ends, in order, with
/// `SC-W0210` when there are more on one rail than on the other, or for rails of 2 nodes each, between the
/// points [`NEAR_START`] along their chords. Each is [`ADDED_REACH`] times as long as the line between its
/// points, about its middle.
fn added_rungs(nodes: &[Vec<Point>; 2], turned: [bool; 2], warnings: &mut Vec<Diagnostic>) -> Vec<(Point, Point)> {
    let mut lists = nodes.clone();
    for (list, turn) in lists.iter_mut().zip(turned) {
        if turn {
            list.reverse();
        }
    }
    let [first, second] = &lists;
    let ends: Vec<(Point, Point)> = if first.len() != second.len() {
        let message = format!(
            "This satin column has no rungs, and its rails have {} and {} nodes, so they pair up only as far as the rail with fewer goes.",
            first.len(),
            second.len()
        );
        warnings.push(Diagnostic::new(Code::SatinNodesUnequal, message).with_fix(Fix::Hint("Add rungs across the column.".to_string())));
        inside(first).iter().copied().zip(inside(second).iter().copied()).collect()
    } else if first.len() <= 2 {
        near_start(first).zip(near_start(second)).into_iter().collect()
    } else {
        inside(first).iter().copied().zip(inside(second).iter().copied()).collect()
    };
    ends.into_iter().map(|(a, b)| reaching(a, b)).collect()
}

/// A rail's `nodes` without its 2 ends.
fn inside(nodes: &[Point]) -> &[Point] {
    nodes.get(1..nodes.len().saturating_sub(1)).unwrap_or_default()
}

/// The segment from `a` to `b` made [`ADDED_REACH`] times as long about its middle, as shapely scales it.
fn reaching(a: Point, b: Point) -> (Point, Point) {
    let (x, y) = ((a.x() + b.x()) / 2.0, (a.y() + b.y()) / 2.0);
    let scaled = |p: Point| Point::new(x + ADDED_REACH * (p.x() - x), y + ADDED_REACH * (p.y() - y)).unwrap_or(p);
    (scaled(a), scaled(b))
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
    fn rails_of_2_nodes_get_a_rung_across_the_points_near_their_starts() {
        let nodes = |a: [(f64, f64); 2], b: [(f64, f64); 2]| [a.map(|(x, y)| p(x, y)).to_vec(), b.map(|(x, y)| p(x, y)).to_vec()];
        // `u` a twentieth of the way from `v` past it: where a rung a tenth longer than from `u` to `v` ends.
        let past = |u: Point, v: Point| p(u.x() + 0.05 * (u.x() - v.x()), u.y() + 0.05 * (u.y() - v.y()));
        let (mut warnings, rails) = (Vec::new(), nodes([(0.0, 0.0), (3.0, 4.0)], [(0.0, 10.0), (-6.0, 18.0)]));
        let [(a, b)] = added_rungs(&rails, [false, false], &mut warnings)[..] else { panic!() };
        let (near_a, near_b) = (p(0.6 * NEAR_START, 0.8 * NEAR_START), p(-0.6 * NEAR_START, 10.0 + 0.8 * NEAR_START));
        assert!(a.distance(past(near_a, near_b)) < 1e-12 && b.distance(past(near_b, near_a)) < 1e-12, "{a:?} {b:?}");
        // The first rail turned: near its last node instead.
        let [(a, b)] = added_rungs(&rails, [true, false], &mut warnings)[..] else { panic!() };
        let near_a = p(3.0 - 0.6 * NEAR_START, 4.0 - 0.8 * NEAR_START);
        assert!(a.distance(past(near_a, near_b)) < 1e-12 && b.distance(past(near_b, near_a)) < 1e-12, "{a:?} {b:?}");
        assert!(warnings.is_empty());
        // Nodes closer together than that: the last one. Nodes at one point: that point, and a rung of no
        // length stays one.
        assert_eq!(near_start(&[p(0.0, 0.0), p(0.01, 0.0)]), Some(p(0.01, 0.0)));
        assert_eq!(near_start(&[p(5.0, 5.0), p(5.0, 5.0)]), Some(p(5.0, 5.0)));
        assert_eq!(near_start(&[]), None);
        assert_eq!(reaching(p(5.0, 5.0), p(5.0, 5.0)), (p(5.0, 5.0), p(5.0, 5.0)));
    }

    #[test]
    fn nodes_inside_the_ends_pair_in_order_as_far_as_the_rail_with_fewer_goes() {
        let rails = [vec![p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0), p(20.0, 0.0)], vec![p(20.0, 4.0), p(15.0, 4.0), p(0.0, 4.0)]];
        let mut warnings = Vec::new();
        // The second rail turned: its node at 15 goes with the first's at 5, and the first's at 10 with none.
        let [(a, b)] = added_rungs(&rails, [false, true], &mut warnings)[..] else { panic!() };
        assert!(a.distance(p(4.5, -0.2)) < 1e-12 && b.distance(p(15.5, 4.2)) < 1e-12, "{a:?} {b:?}");
        assert_eq!(warnings.len(), 1);
        // The first rail turned instead: the second's node at 15 goes with the first's at 10.
        let [(a, b)] = added_rungs(&rails, [true, false], &mut warnings)[..] else { panic!() };
        assert!(a.distance(p(9.75, -0.2)) < 1e-12 && b.distance(p(15.25, 4.2)) < 1e-12, "{a:?} {b:?}");
    }

    #[test]
    fn rails_of_2_nodes_are_cut_where_the_rung_across_their_near_start_points_comes_nearest_them() {
        // The first rail's nodes are (0, 0) and (10, 0), and its line leans right as it rises from (0, 0); the
        // second runs straight from (0, -4) to (10, -4). The rung joins the points 0.2 CSS pixels along each
        // rail's chord, at x = NEAR_START, and reaches a tenth further, half at each end: up to y = 0.2. It
        // misses the first rail, and comes nearest it from its top end, at the foot of that end on the rail's
        // first side. It crosses the second rail at its own point there.
        let first = vec![p(0.0, 0.0), p(0.02, 1.0), p(10.0, 1.0), p(10.0, 0.0)];
        let second = vec![p(0.0, -4.0), p(10.0, -4.0)];
        let nodes = [vec![p(0.0, 0.0), p(10.0, 0.0)], second.clone()];
        let satin = Satin { rails: [first, second], pairing: Pairing::Nodes(nodes) };
        let params = SatinParams::from_set(&stitchcraft_params::ParamSet::new()).unwrap().params;
        let sewn = sections(&satin, &params, &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
        let t = (NEAR_START * 0.02 + 0.2) / (0.02 * 0.02 + 1.0);
        let [a, b] = [sewn[0][0].last().copied().unwrap(), sewn[0][1].last().copied().unwrap()];
        assert!(a.distance(p(0.02 * t, t)) < 1e-12 && b.distance(p(NEAR_START, -4.0)) < 1e-12, "{a:?} {b:?}");
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
