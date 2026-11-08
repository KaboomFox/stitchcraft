//! A ripple cut to its clips (milestone M10.11, `REQ-RIP-017`), as Ink/Stitch cuts and routes its ripple
//! stitch (`docs/src/design/algorithms/strokes.md` › Ripple stitch › Clips).
//!
//! **Why routing.** A ripple's lines cross its clips' edges again and again, so the pieces left inside are
//! many and short. Sewn in the order they come along the ripple, the needle would jump between most of
//! them. Ink/Stitch routes them with its tatami fill's graph instead, travelling along the clips' outline,
//! and so does StitchCraft.
//!
//! **The pieces.** The ripple's needle points, as one line, are cut where they cross the edges of the clips'
//! area, as M8.5 cuts a stroke's line (`normalize::region::cut`). Each stretch inside is a piece.
//!
//! **The route.** Each part of the area routes its pieces as a tatami fill's rows, each sewn once, as it
//! lies ([`lines_sewn`]): from the point of its rings nearest where the ripple's first piece starts, or
//! where the needle is, to the point nearest where its last piece ends. The needle travels along the part's
//! rings between pieces, never under them, as Ink/Stitch's ripple routes with its travel graph's outline
//! alone. The parts are sewn in the order the ripple reaches them, the one where its last piece ends last,
//! each a run of its own. The travel before a part's first piece and after its last is left out, unless the
//! caller keeps it: a grid keeps the way from the ripple's end to its own start.

use stitchcraft_core::{Diagnostic, Exhausted, Meter, Point};

use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::sew::Travel;
use crate::generators::tatami::sew::lines::{Given, Marked, lines_sewn};
use crate::normalize::region::Polygon;
use crate::normalize::region::cut::inside;

/// `run` cut to the area whose parts are `parts`, and its pieces routed part by part as the module says, the
/// needle travelling as `travel` says: each part's run, in sewing order. A route starts nearest `from`, or
/// the first piece's start, and ends nearest the last piece's end. `keep` keeps the travel before a part's
/// first piece, and after its last. Empty when nothing of `run` lies inside. What a route says goes to
/// `warnings`. Costs as the cutting and the routes do.
pub(super) fn routed(
    run: &[Point],
    parts: &[Polygon],
    travel: Travel,
    (from, keep): (Option<Point>, [bool; 2]),
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let pieces = inside(run, parts, meter)?;
    let (Some(&first), Some(&last)) = (pieces.first().and_then(|piece| piece.first()), pieces.last().and_then(|piece| piece.last())) else {
        return Ok(Vec::new());
    };
    let (start, end) = (from.unwrap_or(first), last);
    // The part each piece lies in, by the middle of its first stretch, as the cut found it inside. Every
    // piece has a stretch.
    let part_of = |piece: &[Point]| -> usize {
        let middle = if let [a, b, ..] = piece { a.lerp(*b, 0.5) } else { first };
        parts.iter().position(|part| part.covers(middle)).unwrap_or(0)
    };
    let mut order: Vec<usize> = Vec::new();
    let mut lines: Vec<Vec<Vec<Point>>> = vec![Vec::new(); parts.len()];
    for piece in pieces {
        let part = part_of(&piece);
        if !order.contains(&part) {
            order.push(part);
        }
        if let Some(own) = lines.get_mut(part) {
            own.push(piece);
        }
    }
    // The part where the last piece ends sews last.
    if let Some(at) = lines.iter().position(|own| own.last().and_then(|piece| piece.last()) == Some(&end))
        && let Some(place) = order.iter().position(|part| *part == at)
    {
        let part = order.remove(place);
        order.push(part);
    }
    let given = Given { travel, angle: 0.0, skip_last: false };
    let mut groups = Vec::new();
    for part in order {
        let (Some(polygon), Some(own)) = (parts.get(part), lines.get(part)) else { continue };
        let rings = Rings::new(polygon, meter)?;
        let Some(runs) = lines_sewn(polygon, &rings, own, &given, Some(start), Some(end), meter, warnings)? else { continue };
        groups.extend(runs.into_iter().filter_map(|marked| trimmed(marked, keep)));
    }
    Ok(groups)
}

/// The needle points of `marked`, without the travel before its first piece unless `keep` says to keep it,
/// nor the travel after its last unless it says so. `None` when nothing is left.
fn trimmed(Marked { points, travel }: Marked, [lead, tail]: [bool; 2]) -> Option<Vec<Point>> {
    let travels = |k: &usize| travel.get(*k).copied().unwrap_or(false);
    let from = if lead { 0 } else { (0..points.len()).find(|k| !travels(k))? };
    let to = if tail { points.len() } else { (0..points.len()).rev().find(|k| !travels(k))? + 1 };
    let kept = points.get(from..to)?.to_vec();
    (!kept.is_empty()).then_some(kept)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_rip_017_a_route_s_travel_before_its_first_piece_and_after_its_last_is_left_out_unless_kept() {
        let points: Vec<Point> = (0..6).map(|k| Point::new(f64::from(k), 0.0).unwrap()).collect();
        // Points 0 and 1 travel to the first piece, and point 5 after the last.
        let marked = || Marked { points: points.clone(), travel: vec![true, true, false, true, false, true] };
        assert_eq!(trimmed(marked(), [false, false]), Some(points[2..5].to_vec()));
        assert_eq!(trimmed(marked(), [true, false]), Some(points[..5].to_vec()));
        assert_eq!(trimmed(marked(), [false, true]), Some(points[2..].to_vec()));
        assert_eq!(trimmed(marked(), [true, true]), Some(points.clone()));
        // Travel alone, or nothing, leaves nothing.
        assert_eq!(trimmed(Marked { points: points.clone(), travel: vec![true; 6] }, [false, true]), None);
        assert_eq!(trimmed(Marked { points: points.clone(), travel: vec![true; 6] }, [true, false]), None);
        assert_eq!(trimmed(Marked { points: Vec::new(), travel: Vec::new() }, [true, true]), None);
        // A point with no mark is a piece's.
        assert_eq!(trimmed(Marked { points: points.clone(), travel: vec![true] }, [false, false]), Some(points[1..].to_vec()));
    }
}
