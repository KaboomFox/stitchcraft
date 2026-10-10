//! Offset curves: a polyline moved sideways by a distance, with its corners joined as a stroke's join
//! says (`REQ-SAT-016`).
//!
//! A satin column drawn as one path becomes 2 rails, its centre line offset by half its width to each
//! side. Ink/Stitch offsets with shapely's `offset_curve`, which is GEOS's, and the rails here are built
//! to be the same curves, so that a column sews alike in both. GEOS builds an offset in 3 steps, and so
//! does this module:
//!
//! 1. The raw curve: each segment moved sideways, the corners joined ([`generator`]).
//! 2. The edge of the stroke: the ring of raw curves around the whole polyline, both sides and round caps,
//!    cut where it crosses itself, and the parts of it on the boundary of the area it covers kept
//!    ([`boundary`]).
//! 3. The sections: the parts of that boundary that lie on the raw curve, in order along it. Inside a
//!    tight bend the raw curve loops back on itself, and the loop lies inside the stroke: the section
//!    skips it. Where the line comes back close to itself, the stroke covers part of the raw curve, and
//!    the offset falls into several sections.
//!
//! A line of 2 points is simply moved. Positive distances offset to the left of the line in y-up axes,
//! which is the right on screen, where y points down. Every curve runs the same way as the line. The
//! answers are checked against shapely's for 1,050 random lines (`conformance/oracle/offset.py`).

mod boundary;
mod generator;

pub(crate) use generator::{Meet, segments_meet};

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::design::Join;
use crate::normalize::near::to_side;

/// A raw curve's segment and a boundary piece match when both of the piece's ends lie within this
/// fraction of the distance of the segment.
const MATCH: f64 = 1e-4;

/// What offsetting a line gives.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Offset {
    /// One curve.
    Line(Vec<Point>),
    /// Several curves, in order along the line: the line comes back close to itself, and the stroke covers
    /// the offset between them.
    Apart(Vec<Vec<Point>>),
    /// Nothing: the stroke covers the whole side, or the line has no length.
    Empty,
}

/// `line` offset by `distance`, to the left for a positive distance (y-up axes), with `join` at its
/// corners.
pub(crate) fn offset(line: &[Point], distance: f64, join: Join, meter: &mut Meter) -> Result<Offset, Exhausted> {
    let mut points: Vec<Point> = Vec::with_capacity(line.len());
    for &p in line {
        if points.last() != Some(&p) {
            points.push(p);
        }
    }
    if distance == 0.0 {
        return Ok(if points.len() < 2 { Offset::Empty } else { Offset::Line(line.to_vec()) });
    }
    // Shortcuts, as GEOS takes them: the 3 steps give the same curves.
    Ok(match points.as_slice() {
        [] | [_] => Offset::Empty,
        [a, b] => Offset::Line(moved_segment(*a, *b, distance)),
        _ => stepped(&points, distance, join, meter)?,
    })
}

/// `points`, none the same as the one before, offset in the 3 steps of the module docs.
fn stepped(points: &[Point], distance: f64, join: Join, meter: &mut Meter) -> Result<Offset, Exhausted> {
    let raw = generator::raw(points, distance, join, meter)?;
    let half = distance.abs();
    let ring = generator::ring(points, half, join, meter)?;
    let mut sections = Vec::new();
    for mut ring in largest_polygon(boundary::boundary(&ring, half, meter)?) {
        if distance < 0.0 {
            ring.reverse();
            for edge in &mut ring {
                *edge = (edge.1, edge.0);
            }
        }
        sections.extend(sections_of(&ring, &raw, half * MATCH, meter)?);
    }
    sections.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut curves: Vec<Vec<Point>> = sections.into_iter().map(|(_, points)| points).collect();
    Ok(match curves.len() {
        0 => Offset::Empty,
        1 => Offset::Line(curves.pop().unwrap_or_default()),
        _ => Offset::Apart(curves),
    })
}

/// The rings of the largest area the stroke covers, as GEOS builds the polygon of its buffer and keeps the
/// largest: the shell, which runs clockwise, then the holes inside it. A ring of no area bounds nothing.
fn largest_polygon(rings: Vec<Vec<(Point, Point)>>) -> Vec<Vec<(Point, Point)>> {
    let (shells, holes): (Vec<_>, Vec<_>) =
        rings.into_iter().filter(|ring| boundary::doubled_area(ring) != 0.0).partition(|ring| boundary::doubled_area(ring) < 0.0);
    let inside = |shell: &[(Point, Point)], hole: &[(Point, Point)]| hole.first().is_some_and(|&(p, q)| boundary::encloses(shell, p.lerp(q, 0.5)));
    let covered = |shell: &[(Point, Point)]| {
        -boundary::doubled_area(shell) - holes.iter().filter(|hole| inside(shell, hole)).map(|hole| boundary::doubled_area(hole)).sum::<f64>()
    };
    let Some(shell) = shells.into_iter().reduce(|best, shell| if covered(&shell) > covered(&best) { shell } else { best }) else { return Vec::new() };
    let holes: Vec<_> = holes.into_iter().filter(|hole| inside(&shell, hole)).collect();
    std::iter::once(shell).chain(holes).collect()
}

/// The segment from `a` to `b` moved `distance` to its left (y-up axes).
fn moved_segment(a: Point, b: Point, distance: f64) -> Vec<Point> {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let len = crate::normalize::near::length(a, b);
    let (ux, uy) = (distance * dx / len, distance * dy / len);
    [a, b].iter().map(|p| Point::new(p.x() - uy, p.y() + ux).unwrap_or(*p)).collect()
}

/// The sections of the boundary `ring` that lie on the raw curve `raw`, each with its place along the
/// raw curve (a segment's index plus the fraction of the way along it), in the ring's order from the one
/// placed first.
fn sections_of(ring: &[(Point, Point)], raw: &[Point], near: f64, meter: &mut Meter) -> Result<Vec<(f64, Vec<Point>)>, Exhausted> {
    // Where along the raw curve each piece of the ring lies, if it lies on it: the last segment it matches.
    // The walk starts at the piece placed first when it was placed, before any later segment moved it on.
    let mut place: Vec<Option<f64>> = vec![None; ring.len()];
    let mut first: Option<(usize, f64)> = None;
    for (i, pair) in raw.windows(2).enumerate() {
        let (r0, r1) = (pair[0], pair[1]);
        let mut least: Option<(usize, f64)> = None;
        for (k, &(p, q)) in ring.iter().enumerate() {
            meter.charge(1)?;
            if to_side(p, r0, r1) <= near
                && to_side(q, r0, r1) <= near
                && let Some(slot) = place.get_mut(k)
            {
                let at = index_f64(i) + fraction(p, r0, r1);
                *slot = Some(at);
                if least.is_none_or(|(_, l)| at < l) {
                    least = Some((k, at));
                }
            }
        }
        if let Some((k, at)) = least
            && first.is_none_or(|(_, f)| at < f)
        {
            first = Some((k, at));
        }
    }
    let n = ring.len();
    let Some((first, _)) = first else { return Ok(Vec::new()) };
    let closed = ring.first().map(|e| e.0) == ring.last().map(|e| e.1);
    let matched = |k: usize| place.get(k).copied().flatten().is_some();
    let mut sections = Vec::new();
    // Walk once round from the first-placed piece, cutting at pieces that are not on the raw curve.
    let mut current: Option<(f64, Vec<Point>)> = None;
    for step in 0..n {
        let k = (first + step) % n;
        if !closed && k == 0 {
            // An open chain does not wrap round. At the walk's first step nothing has gathered yet.
            sections.extend(current.take());
        }
        match (matched(k), ring.get(k)) {
            (true, Some(&(p, q))) => match &mut current {
                Some((_, points)) => points.push(q),
                None => current = Some((place.get(k).copied().flatten().unwrap_or_default(), vec![p, q])),
            },
            _ => sections.extend(current.take()),
        }
    }
    sections.extend(current);
    Ok(sections)
}

/// `i` as a float: indices of points are far below 2^53.
#[allow(clippy::cast_precision_loss)]
fn index_f64(i: usize) -> f64 {
    i as f64
}

/// How far along the segment from `a` to `b` the foot of `p` lies, as a fraction from 0 to 1.
fn fraction(p: Point, a: Point, b: Point) -> f64 {
    if p == a {
        return 0.0;
    }
    if p == b {
        return 1.0;
    }
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let t = ((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / (dx * dx + dy * dy);
    // A segment of no length gives NaN: its end.
    if t.is_nan() { 1.0 } else { t.clamp(0.0, 1.0) }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::rng::SplitMix64;

    use super::*;
    use crate::normalize::fixture::cases;

    /// Shapely's offset curves for random lines (`conformance/fixtures/geometry/shapely-offset.txt`,
    /// written by `conformance/oracle/offset.py`).
    const SHAPELY: &str = include_str!("../../../../../conformance/fixtures/geometry/shapely-offset.txt");

    /// Points this close are the same: GEOS computes crossings in extended precision and arcs with the
    /// platform's sine and cosine, which can differ from the engine's in their last bits.
    const CLOSE: f64 = 1e-9;

    /// The fixture's lines whose offset is not shapely's: walks on a grid that run back exactly along
    /// themselves, or whose stroke's edge touches itself exactly. There the pieces of the edge coincide,
    /// and whether GEOS takes 2 of them to meet turns on the rounding of its own arithmetic. The engine's
    /// curves differ by a node on a straight stretch, or by a stretch where the line doubles back.
    const DOUBLED_BACK: [usize; 5] = [752, 767, 848, 852, 879];

    fn same(a: &[Point], b: &[Point]) -> bool {
        a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p.x() - q.x()).abs() <= CLOSE && (p.y() - q.y()).abs() <= CLOSE)
    }

    /// `points` without those within [`CLOSE`] of the one before: GEOS keeps a point twice where 2
    /// pieces of the stroke's edge meet a hair apart, and the engine takes them as one.
    fn once(points: &[Point]) -> Vec<Point> {
        let mut kept: Vec<Point> = Vec::new();
        for &p in points {
            if kept.last().is_none_or(|q| (p.x() - q.x()).abs() > CLOSE || (p.y() - q.y()).abs() > CLOSE) {
                kept.push(p);
            }
        }
        kept
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn short_lines_and_no_distance_are_offset_as_shapely_offsets_them() {
        let meter = &mut Budget::DEFAULT.meter();
        let line = [p(0.0, 0.0), p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)];
        // No distance: the line as it is, its repeated point too, when it has length.
        assert_eq!(offset(&line, 0.0, Join::UNSET, meter).unwrap(), Offset::Line(line.to_vec()));
        assert_eq!(offset(&line[1..3], 0.0, Join::UNSET, meter).unwrap(), Offset::Line(line[1..3].to_vec()));
        assert_eq!(offset(&line[..2], 0.0, Join::UNSET, meter).unwrap(), Offset::Empty);
        assert_eq!(offset(&line[..2], 1.0, Join::UNSET, meter).unwrap(), Offset::Empty);
        assert_eq!(offset(&line[1..3], -1.0, Join::UNSET, meter).unwrap(), Offset::Line(vec![p(0.0, -1.0), p(1.0, -1.0)]));
    }

    #[test]
    fn the_shortcuts_give_the_curves_of_the_3_steps() {
        let meter = &mut Budget::DEFAULT.meter();
        let mut rng = SplitMix64::new(20_261_010);
        let mut coordinate = || rng.next_f64() * 20.0 - 10.0;
        for case in 0..500 {
            let (a, b) = (p(coordinate(), coordinate()), p(coordinate(), coordinate()));
            let distance = coordinate() / 2.0;
            let join = [Join::UNSET, Join::Round, Join::Bevel, Join::Miter { limit: 1.5 }][case % 4];
            assert_eq!(stepped(&[a, b], distance, join, meter).unwrap(), Offset::Line(moved_segment(a, b, distance)), "{a:?} {b:?} {distance}");
            assert_eq!(stepped(&[a], distance, join, meter).unwrap(), Offset::Empty);
        }
        assert_eq!(stepped(&[], 1.0, Join::UNSET, meter).unwrap(), Offset::Empty);
    }

    #[test]
    fn a_point_s_place_along_a_segment_is_held_to_it() {
        let (a, b) = (p(0.0, 0.0), p(2.0, 0.0));
        assert_eq!([fraction(a, a, b), fraction(b, a, b), fraction(p(1.0, 1.0), a, b)], [0.0, 1.0, 0.5]);
        assert_eq!([fraction(p(-1.0, 0.0), a, b), fraction(p(3.0, 0.0), a, b), fraction(p(1.0, 0.0), a, a)], [0.0, 1.0, 1.0]);
        assert_eq!(fraction(p(-2.0, 5.0), p(1.0, 1.0), p(3.0, 5.0)), 0.5, "a slanting segment away from the origin");
    }

    #[test]
    fn the_largest_polygon_is_the_shell_covering_most_less_its_holes() {
        // Rectangles as rings of edges, a shell clockwise and a hole the other way. Shells of 40 mm² less a
        // hole of 28, of 20, of 30 less 0.5, of 50 less 1 and of 49 in turn: the first of the 2 covering
        // 49 mm² is kept.
        let rectangle = |x0: f64, y0: f64, x1: f64, y1: f64, clockwise: bool| {
            let mut corners = [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)];
            if clockwise {
                corners.reverse();
            }
            (0..4).map(|i| (corners[i], corners[(i + 1) % 4])).collect::<Vec<_>>()
        };
        let (shell, hole) = (rectangle(0.0, 0.0, 10.0, 5.0, true), rectangle(1.0, 1.0, 2.0, 2.0, false));
        let no_area = vec![(p(3.0, 3.0), p(4.0, 4.0)), (p(4.0, 4.0), p(3.0, 3.0))];
        let rings = vec![
            rectangle(20.0, 0.0, 30.0, 4.0, true),
            rectangle(21.0, 0.25, 29.0, 3.75, false),
            rectangle(40.0, 0.0, 44.0, 5.0, true),
            rectangle(60.0, 0.0, 66.0, 5.0, true),
            rectangle(61.0, 1.0, 61.5, 2.0, false),
            no_area,
            shell.clone(),
            hole.clone(),
            rectangle(50.0, 0.0, 57.0, 7.0, true),
        ];
        assert_eq!(largest_polygon(rings), [shell, hole]);
    }

    #[test]
    fn of_pieces_placed_alike_the_walk_starts_at_the_one_placed_first() {
        let meter = &mut Budget::DEFAULT.meter();
        let ring = |points: &[(f64, f64)]| points.windows(2).map(|pair| (p(pair[0].0, pair[0].1), p(pair[1].0, pair[1].1))).collect::<Vec<_>>();
        // 2 pieces of a ring touching itself start at the same place on a segment: the first in the ring's
        // order starts the walk.
        let raw = [p(0.0, 0.0), p(10.0, 0.0)];
        let touching = ring(&[(2.0, 0.0), (4.0, 0.0), (4.0, 5.0), (2.0, 0.0), (3.0, 0.0), (3.0, -5.0), (2.0, 0.0)]);
        let found = sections_of(&touching, &raw, 1e-4, meter).unwrap();
        assert_eq!(found, [(0.2, vec![p(2.0, 0.0), p(4.0, 0.0)]), (0.2, vec![p(2.0, 0.0), p(3.0, 0.0)])]);
        // A piece at the end of the first segment and one at the start of the next are both at 1: the one
        // placed on the first segment starts the walk.
        let raw = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0)];
        let corner = ring(&[(10.0, 0.0), (10.0, 5.0), (12.0, 5.0), (10.0, 0.0), (10.0, 0.0005), (8.0, -3.0), (10.0, 0.0)]);
        let found = sections_of(&corner, &raw, 1e-3, meter).unwrap();
        assert_eq!(found, [(1.0, vec![p(10.0, 0.0), p(10.0, 0.0005)]), (1.0, vec![p(10.0, 0.0), p(10.0, 5.0)])]);
    }

    #[test]
    fn an_open_chain_of_the_edge_does_not_wrap_round() {
        // No rings at all, and a chain that does not close: the walk from its piece placed first, the
        // last, does not go on into the first.
        assert!(largest_polygon(Vec::new()).is_empty());
        let raw = [p(0.0, 0.0), p(10.0, 0.0)];
        let chain = [(p(4.0, 0.0), p(6.0, 0.0)), (p(6.0, 0.0), p(6.0, 5.0)), (p(6.0, 5.0), p(0.0, 0.0)), (p(0.0, 0.0), p(2.0, 0.0))];
        let found = sections_of(&chain, &raw, 1e-4, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(found, [(0.0, vec![p(0.0, 0.0), p(2.0, 0.0)]), (0.4, vec![p(4.0, 0.0), p(6.0, 0.0)])]);
    }

    #[test]
    fn offsets_are_shapely_s() {
        let (mut differ, mut seen) = (Vec::new(), 0);
        for (number, mut words) in cases(SHAPELY) {
            seen += 1;
            let line = words.polyline();
            let distance = words.number();
            let join = match words.word() {
                "mitre" => Join::Miter { limit: words.number() },
                "round" => {
                    words.number();
                    Join::Round
                }
                _ => {
                    words.number();
                    Join::Bevel
                }
            };
            let expected = words.polylines();
            let found = offset(&line, distance, join, &mut Budget::DEFAULT.meter()).unwrap();
            let agrees = match (&found, expected.as_slice()) {
                (Offset::Empty, []) => true,
                (Offset::Line(points), [one]) => same(&once(points), &once(one)),
                (Offset::Apart(curves), many) => curves.len() == many.len() && curves.iter().zip(many).all(|(a, b)| same(&once(a), &once(b))),
                _ => false,
            };
            if !agrees {
                differ.push(number);
            }
        }
        assert_eq!(seen, 1050, "every case of the fixture is read");
        assert_eq!(differ, DOUBLED_BACK, "the fixture's lines whose offset differs");
    }
}
