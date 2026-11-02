//! Lines sewn as a fill's rows: a circular fill's pieces of spiral, each stitched already, routed as a
//! tatami fill's rows are and sewn as they are (`REQ-FILL-CIR-004`).
//!
//! **Why tatami's routing.** Ink/Stitch routes a circular fill's pieces with its tatami fill's graph, from
//! their ends, and moves its needle between them with its tatami fill's travel. So each piece is a row of
//! the route here ([`Rows::Lines`]): sewn once, in the order a tatami fill's rows with those ends would be,
//! the needle travelling under the pieces not sewn yet, along lines at 45° and square to the fill's angle,
//! or with `underpath` off along the rings and the pieces themselves ([`Ways`]).
//!
//! **Sewn as they are.** A piece is sewn from the end the route enters it by, every needle point it was
//! stitched with, and with `skip_last` without its last, as Ink/Stitch sews them. The travel to the next
//! piece then keeps its first stitch, the piece's end, unless the next piece starts beside it, as a tatami
//! fill's travel does with `skip_last`.
//!
//! **Travel marked.** Each needle point the needle travels to is marked, so that bean stitch leaves the
//! travel out ([`crate::generators::passes::beaned`]). Ink/Stitch's travel ends on the next piece's first
//! point, which the piece then lists again. Both are kept, so that bean stitch counts its steps as
//! Ink/Stitch's does, and the point listed twice sews nothing.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use super::{Travel, Ways, moved};
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::route::{Route, Rows, Step, route};
use crate::normalize::region::Polygon;

/// How a fill's lines are sewn: the needle's travel between them, the angle the lines it travels along
/// under them are turned from, and whether each line's last needle point is left out.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Given {
    /// How the needle travels between lines.
    pub travel: Travel,
    /// The fill's angle, in degrees, which the lines under the rows are turned from.
    pub angle: f64,
    /// Whether each line is sewn without its last needle point (`skip_last`).
    pub skip_last: bool,
}

/// A run of needle points, each marked when the needle travels to it.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Marked {
    /// The needle points, in the order sewn.
    pub points: Vec<Point>,
    /// For each point, whether the needle travels to it.
    pub travel: Vec<bool>,
}

impl Marked {
    /// Adds `points`, each marked as `travel` says.
    fn extend(&mut self, points: impl IntoIterator<Item = Point>, travel: bool) {
        for p in points {
            self.points.push(p);
            self.travel.push(travel);
        }
    }
}

/// The runs that `lines`, each stitched already, sew over `part`, whose rings are `rings`, as `given` says:
/// routed from the point of the rings nearest `start` to the point nearest `end`, or back to where they
/// started without an `end`. One run, unless the needle has to jump, which is a bug that `warnings` gets.
/// `None` when there is no line. Costs as the route, the needle's ways and their running stitches do.
#[allow(clippy::too_many_arguments)]
pub(crate) fn lines_sewn(
    part: &Polygon,
    rings: &Rings<'_>,
    lines: &[Vec<Point>],
    given: &Given,
    start: Option<Point>,
    end: Option<Point>,
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Option<Vec<Marked>>, Exhausted> {
    let across = Rows::Lines(lines);
    let Some(route) = route(rings, across, start, end, meter)? else { return Ok(None) };
    let mut ways = Ways::new(part, rings, &route, across, given.angle, given.travel, start, meter)?;
    let point = |node: usize| route.nodes.get(node).map(|n| n.point);
    // Each step's line as it is sewn, from the end the route enters it by.
    let sewn: Vec<Option<Vec<Point>>> = route.steps.iter().map(|step| entered(step, lines, &route)).collect();
    let line = |i: usize| sewn.get(i).and_then(Option::as_ref);
    let start_of = |i: usize| line(i).and_then(|l| l.first().copied());
    let end_before = |i: usize| i.checked_sub(1).and_then(line).and_then(|l| l.last().copied());
    let (mut runs, mut run, mut jumped) = (Vec::new(), Marked::default(), false);
    let lead = ways.lead_in(meter)?;
    let skipped = usize::from(lead.as_ref().is_some_and(|(_, replaces)| *replaces));
    match lead {
        Some((mut way, _)) => {
            moved(&mut way, None, start_of(skipped));
            run.extend(ways.stitched(&way, meter)?, true);
        }
        None => {
            if let Some(Step::Travel { from, .. }) = route.steps.first() {
                run.extend(point(*from), true);
            }
        }
    }
    for (i, step) in route.steps.iter().enumerate().skip(skipped) {
        match *step {
            Step::Row { segment, .. } => {
                let Some(points) = line(i) else { continue };
                run.extend(points.iter().take(points.len().saturating_sub(usize::from(given.skip_last))).copied(), false);
                ways.sewn(segment);
            }
            Step::Travel { from, to, beside, .. } => {
                let next = start_of(i + 1);
                match ways.way(from, to, meter)? {
                    Some(mut way) => {
                        moved(&mut way, end_before(i), next);
                        let stitches = ways.stitched(&way, meter)?;
                        let keep_first = given.skip_last && !(beside && next.is_some());
                        run.extend(stitches.into_iter().skip(usize::from(!keep_first)), true);
                    }
                    None => {
                        jumped = true;
                        runs.push(std::mem::take(&mut run));
                        run.extend(next.or(point(to)), true);
                    }
                }
            }
        }
    }
    runs.push(run);
    if jumped {
        warnings.push(Diagnostic::new(Code::InternalCheckFailed, "The needle found no way between 2 pieces of this fill, so it jumps there."));
    }
    Ok(Some(runs))
}

/// The line the row `step` sews, from the end the route enters it by: the end nearer the node it is sewn
/// from. `None` for travel.
fn entered(step: &Step, lines: &[Vec<Point>], route: &Route) -> Option<Vec<Point>> {
    let Step::Row { segment, from, .. } = *step else { return None };
    let (line, entry) = (lines.get(segment)?, route.nodes.get(from)?.point);
    let (first, last) = (line.first()?, line.last()?);
    let mut line = line.clone();
    if entry.distance(*last) < entry.distance(*first) {
        line.reverse();
    }
    Some(line)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{self, p, rectangle};

    const TRAVEL: Travel = Travel { length: 2.0, tolerance: 0.2, min_stitch: 1e-6, underpath: false, jump: 3.0 };

    /// The right half of the circle of `radius` round (`x`, 2), from its bottom to its top, a point every 10°.
    fn arc(x: f64, radius: f64) -> Vec<Point> {
        (0..=18)
            .map(|k| {
                let (sin, cos) = stitchcraft_core::math::sin_cos(f64::from(k * 10 - 90).to_radians());
                p(x + radius * cos, 2.0 + radius * sin)
            })
            .collect()
    }

    fn sewn(part: &Polygon, lines: &[Vec<Point>], given: &Given) -> (Vec<Marked>, Vec<Diagnostic>) {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(part, meter).unwrap();
        let mut warnings = Vec::new();
        let runs = lines_sewn(part, &rings, lines, given, None, None, meter, &mut warnings).unwrap().unwrap();
        (runs, warnings)
    }

    #[test]
    fn req_fill_cir_004_each_line_is_sewn_once_as_it_is_and_travel_is_marked() {
        // 2 arcs across a 10 × 4 rectangle, each from its bottom side to its top side: each is sewn once,
        // every point of it, from one end or the other, and the needle travels between them along the
        // outline, every point of that travel marked.
        let part = rectangle(4.0);
        let lines = [arc(5.0, 2.0), arc(2.0, 2.0)];
        let given = Given { travel: TRAVEL, angle: 0.0, skip_last: false };
        let (runs, warnings) = sewn(&part, &lines, &given);
        assert_eq!((runs.len(), warnings), (1, Vec::new()));
        let Marked { points, travel } = &runs[0];
        assert_eq!(points.len(), travel.len());
        let sewn = |line: &[Point]| {
            let reversed: Vec<Point> = line.iter().rev().copied().collect();
            points.windows(line.len()).filter(|w| *w == line || *w == reversed.as_slice()).count()
        };
        assert_eq!(lines.iter().map(|line| sewn(line)).collect::<Vec<_>>(), [1, 1]);
        // Every point that is no line's is travel.
        let on_a_line = |q: &Point| lines.iter().any(|line| line.contains(q));
        for (q, &travels) in points.iter().zip(travel) {
            assert!(travels || on_a_line(q), "{q:?} is neither travel nor a line's");
        }
        assert!(travel.iter().any(|&t| t), "the needle travels between the lines");
    }

    #[test]
    fn with_skip_last_each_line_s_last_point_is_left_out_unless_travel_starts_there() {
        // One arc: its last point is left out, and with nothing after it, nothing sews it.
        let part = rectangle(4.0);
        let lines = [arc(5.0, 2.0)];
        let (full, _) = sewn(&part, &lines, &Given { travel: TRAVEL, angle: 0.0, skip_last: false });
        let (short, _) = sewn(&part, &lines, &Given { travel: TRAVEL, angle: 0.0, skip_last: true });
        assert_eq!(short[0].points.len() + 1, full[0].points.len());
        assert_eq!(&full[0].points[..short[0].points.len()], short[0].points.as_slice());
    }

    #[test]
    fn with_skip_last_a_line_s_end_is_sewn_only_as_travel_away_from_it() {
        // The arc round x = 2 is sewn down from (2, 4), and the needle travels on from its end round the
        // outline's left side to the other arc's top: not beside it, so the travel keeps its first stitch,
        // the arc's end, marked as travel. The arc round x = 5 is sewn last, down from (5, 4), and its end,
        // (5, 0), is left out: the needle is there only where the route starts.
        let part = rectangle(4.0);
        let lines = [arc(5.0, 2.0), arc(2.0, 2.0)];
        let (runs, _) = sewn(&part, &lines, &Given { travel: TRAVEL, angle: 0.0, skip_last: true });
        let Marked { points, travel } = &runs[0];
        let at = points.iter().position(|&q| q == p(2.0, 0.0)).unwrap();
        assert!(travel[at] && points[at - 1] == lines[1][1], "{points:?}");
        assert_eq!(points.last(), Some(&lines[0][1]), "{points:?}");
        assert!(points.iter().rposition(|&q| q == lines[0][0]) < Some(2), "{points:?}");
    }

    #[test]
    fn from_a_needle_inside_the_part_the_needle_comes_in_along_the_lines_under_them() {
        // The 2 arcs across the 10 × 4 rectangle, the lines under them on, and the needle inside the part
        // left of both: it starts at the node of the lines nearest it, travelling from there to the route's
        // start, and every point it travels to before the first line is marked.
        let part = rectangle(4.0);
        let lines = [arc(5.0, 2.0), arc(2.0, 2.0)];
        let given = Given { travel: Travel { underpath: true, ..TRAVEL }, angle: 0.0, skip_last: false };
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let needle = p(0.8, 2.0);
        let runs = lines_sewn(&part, &rings, &lines, &given, Some(needle), None, meter, &mut Vec::new()).unwrap().unwrap();
        let Marked { points, travel } = &runs[0];
        // That node lies inside the part, off its outline, where the route's first node lies.
        let off_the_outline = |q: Point| q.x() > 0.1 && q.x() < 9.9 && q.y() > 0.1 && q.y() < 3.9;
        assert!(points[0].distance(needle) < 1.0 && off_the_outline(points[0]) && travel[0], "{points:?}");
        let first_line = points.iter().position(|q| lines.iter().any(|line| line.contains(q))).unwrap();
        assert!(first_line > 0 && travel[..first_line].iter().all(|&t| t), "{points:?}");
    }

    #[test]
    fn a_needle_that_finds_no_way_between_2_lines_jumps_and_says_so_as_a_bug() {
        // A line between 2 points of the frame's outline and one between 2 points of its hole, which nothing
        // joins: the travel from the first to the second finds no way, and the needle jumps to the second.
        let part = fixture::frame();
        let lines = [vec![p(0.0, 0.5), p(10.0, 0.5)], vec![p(4.0, 2.0), p(6.0, 2.0)]];
        let (runs, warnings) = sewn(&part, &lines, &Given { travel: TRAVEL, angle: 0.0, skip_last: false });
        assert_eq!(runs.len(), 2, "the needle jumps once");
        assert!(lines[1].contains(&runs[1].points[0]), "{:?}", runs[1].points);
        assert_eq!(warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
    }

    #[test]
    fn no_line_sews_nothing() {
        let part = rectangle(4.0);
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let given = Given { travel: TRAVEL, angle: 0.0, skip_last: false };
        assert_eq!(lines_sewn(&part, &rings, &[], &given, None, None, meter, &mut Vec::new()).unwrap(), None);
    }

    #[test]
    fn a_closed_line_is_sewn_as_it_runs() {
        // A line round a triangle in the rectangle, from (2, 1) back to it: the route enters it at (2, 1),
        // where both its ends lie, as near the one as the other, and it is sewn from its first point as it
        // runs, not turned round.
        let part = rectangle(4.0);
        let closed = [vec![p(2.0, 1.0), p(4.0, 1.0), p(3.0, 3.0), p(2.0, 1.0)]];
        let (runs, _) = sewn(&part, &closed, &Given { travel: TRAVEL, angle: 0.0, skip_last: false });
        assert!(runs[0].points.ends_with(&closed[0]), "{:?}", runs[0].points);
    }
}
