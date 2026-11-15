//! A guided fill's guide line made ready to copy: one line, open, as Ink/Stitch makes its guide ready
//! (read at `d59c9ab`, `REQ-FILL-GDD-002`).
//!
//! **Why one line.** A copy of a guide that crosses itself crosses itself too, and the rows would cross
//! each other in the fill. So a guide that crosses or touches itself is cut where it does into pieces. A
//! piece that comes back to where it began, a loop, is left out, and 2 pieces that meet end to end where no
//! third piece meets them are joined, at most 4 times over, until one line is left. A guide still in pieces
//! after that, or one that runs back along itself, is the straight line from its first point to its last.
//!
//! **Why open.** The copies move square to the line between the guide's ends, which a closed guide doesn't
//! have. Its last 2 points are left out: the end it closes on and the point before it, as Ink/Stitch leaves
//! them out.
//!
//! **Cost.** Only the sides whose spans along x overlap are tested against each other, a unit of work for
//! each pair, and a unit for each point of each piece.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::normalize::region::geom::{Hit, hit};

/// How many times pieces are dropped and joined before the guide is given up as a straight line, as in
/// Ink/Stitch.
const ROUNDS: usize = 4;

/// The guide line `drawn`, flattened, ready to copy: one line, open; `None` when it has no length, or is
/// left with its ends at one point.
pub(super) fn prepared(drawn: &[Point], meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let line = one_line(drawn, meter)?;
    // A closed line leaves out the end it closes on and the point before it.
    let open = match line.as_slice() {
        [start, .., end] if start == end => line.get(..line.len() - 2).unwrap_or_default().to_vec(),
        _ => line,
    };
    Ok(match open.as_slice() {
        [start, .., end] if start != end => Some(open),
        _ => None,
    })
}

/// `drawn` made one line, as the module says: as it is where it doesn't meet itself, its loops left out
/// and its pieces joined where it does, and otherwise the straight line from its first point to its last.
/// A closed line that meets itself only where it closes stays closed. Nothing for a line of no points.
pub(super) fn one_line(drawn: &[Point], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let (Some(&first), Some(&last)) = (drawn.first(), drawn.last()) else { return Ok(Vec::new()) };
    Ok(match meetings(drawn, meter)? {
        Meetings::None => drawn.to_vec(),
        Meetings::Points(nodes) => joined(drawn, &nodes, meter)?.unwrap_or_else(|| vec![first, last]),
        Meetings::Along => vec![first, last],
    })
}

/// Whether `line` is simple: it meets itself nowhere but where its sides follow each other, and where it
/// closes. Costs as finding where it meets itself does.
pub(super) fn simple(line: &[Point], meter: &mut Meter) -> Result<bool, Exhausted> {
    Ok(meetings(line, meter)? == Meetings::None)
}

/// Where a line meets itself, but where its sides follow each other.
#[derive(Debug, PartialEq)]
enum Meetings {
    /// Nowhere: it is simple, or a ring.
    None,
    /// At these points: each with the side it lies on and how far along that side, sorted along the line.
    Points(Vec<(usize, f64, Point)>),
    /// Somewhere along a stretch, where it runs back along itself.
    Along,
}

/// Where `line` meets itself. 2 sides that follow each other meet at their shared point, and the first and
/// the last of a closed line at its start: nowhere else may they meet. A unit of `meter`'s work for each
/// pair of sides whose spans along x overlap.
fn meetings(line: &[Point], meter: &mut Meter) -> Result<Meetings, Exhausted> {
    let sides = line.len().saturating_sub(1);
    let closed = sides > 2 && line.first() == line.last();
    // The sides by where they start along x, each with its span.
    let mut order: Vec<(usize, f64, f64)> = line.windows(2).enumerate().map(|(i, w)| (i, w[0].x().min(w[1].x()), w[0].x().max(w[1].x()))).collect();
    order.sort_by(|a, b| a.1.total_cmp(&b.1).then(a.0.cmp(&b.0)));
    let side = |i: usize| -> Option<(Point, Point)> { Some((*line.get(i)?, *line.get(i + 1)?)) };
    let mut nodes: Vec<(usize, f64, Point)> = Vec::new();
    for (k, &(i, _, right)) in order.iter().enumerate() {
        for &(j, left, _) in order.iter().skip(k + 1) {
            if left > right {
                break;
            }
            meter.charge(1)?;
            let (lo, hi) = (i.min(j), i.max(j));
            let (Some((a, b)), Some((c, d))) = (side(lo), side(hi)) else { continue };
            let neighbours = hi == lo + 1 || (closed && lo == 0 && hi == sides - 1);
            match hit(a, b, c, d) {
                Hit::Apart => {}
                Hit::Overlap(..) => return Ok(Meetings::Along),
                // Sides that follow each other share a point, and meet nowhere else unless they overlap.
                Hit::Touch(_) | Hit::Cross(_) if neighbours => {}
                Hit::Touch(x) | Hit::Cross(x) => {
                    nodes.push((lo, a.distance(x), x));
                    nodes.push((hi, c.distance(x), x));
                }
            }
        }
    }
    if nodes.is_empty() {
        return Ok(Meetings::None);
    }
    nodes.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.total_cmp(&b.1)));
    nodes.dedup_by(|a, b| a.0 == b.0 && a.2 == b.2);
    Ok(Meetings::Points(nodes))
}

/// `line` cut at its `nodes`, its loops left out and the pieces joined where 2 meet alone, as the module says;
/// `None` when that leaves it in more than one piece, or none.
fn joined(line: &[Point], nodes: &[(usize, f64, Point)], meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let mut chains = pieces(line, nodes, meter)?;
    for _ in 0..ROUNDS {
        if chains.len() == 1 {
            break;
        }
        chains.retain(|chain| chain.first() != chain.last());
        chains = merged(chains, meter)?;
    }
    Ok(match <[Vec<Point>; 1]>::try_from(chains) {
        Ok([one]) => Some(one),
        Err(_) => None,
    })
}

/// `line` cut at its `nodes`, in order along it: each piece runs from a node, or the line's start, to the
/// next node, or the line's end.
fn pieces(line: &[Point], nodes: &[(usize, f64, Point)], meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let mut pieces = Vec::new();
    let mut piece: Vec<Point> = line.first().copied().into_iter().collect();
    let mut cuts = nodes.iter().peekable();
    for (i, point) in line.iter().enumerate().skip(1) {
        meter.charge(1)?;
        // The nodes on the side that ends at this point, in order along it.
        while let Some(&(_, _, x)) = cuts.next_if(|&&(side, _, _)| side + 1 == i) {
            if piece.last() != Some(&x) {
                piece.push(x);
            }
            pieces.push(std::mem::replace(&mut piece, vec![x]));
        }
        if piece.last() != Some(point) {
            piece.push(*point);
        }
    }
    pieces.push(piece);
    pieces.retain(|piece| piece.len() > 1);
    Ok(pieces)
}

/// `chains` with each joined to the next where one ends where the next starts and no other chain ends or
/// starts there.
fn merged(chains: Vec<Vec<Point>>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let ends: Vec<Point> = chains.iter().flat_map(|chain| chain.first().into_iter().chain(chain.last())).copied().collect();
    let mut joined: Vec<Vec<Point>> = Vec::with_capacity(chains.len());
    for chain in chains {
        meter.charge(u64::try_from(ends.len()).unwrap_or(u64::MAX))?;
        let alone = |at: Point| ends.iter().filter(|&&end| end == at).count() == 2;
        match joined.last_mut() {
            Some(last) if last.last() == chain.first() && chain.first().is_some_and(|&at| alone(at)) => last.extend(chain.into_iter().skip(1)),
            _ => joined.push(chain),
        }
    }
    Ok(joined)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn points(xy: &[(f64, f64)]) -> Vec<Point> {
        xy.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect()
    }

    fn ready(xy: &[(f64, f64)]) -> Option<Vec<(f64, f64)>> {
        prepared(&points(xy), &mut Budget::DEFAULT.meter()).unwrap().map(|line| line.iter().map(|p| (p.x(), p.y())).collect())
    }

    #[test]
    fn req_fill_gdd_002_a_simple_guide_is_copied_as_it_is() {
        let s = [(0.0, 0.0), (3.0, 1.0), (6.0, -1.0), (9.0, 0.0)];
        assert_eq!(ready(&s), Some(s.to_vec()));
    }

    #[test]
    fn req_fill_gdd_002_a_closed_guide_leaves_out_its_last_2_points() {
        // A square drawn round from the origin: its first 2 sides are left, as Ink/Stitch leaves them.
        let square = [(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0), (0.0, 0.0)];
        assert_eq!(ready(&square), Some(vec![(0.0, 0.0), (4.0, 0.0), (4.0, 4.0)]));
    }

    #[test]
    fn req_fill_gdd_002_a_guide_that_crosses_itself_once_loses_its_loop() {
        // Along x, round a loop that crosses the line at (5, 0), and on: the loop is left out, as GEOS's union
        // and line merge leave it out for Ink/Stitch (shapely 2.2 on GEOS 3.14).
        let looped = [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (5.0, 5.0), (5.0, -5.0)];
        assert_eq!(ready(&looped), Some(vec![(0.0, 0.0), (5.0, 0.0), (5.0, -5.0)]));
        // A small loop that touches the line at one of its points.
        let touched = [(0.0, 0.0), (5.0, 0.0), (6.0, 1.0), (5.0, 2.0), (4.0, 1.0), (5.0, 0.0), (10.0, 0.0)];
        assert_eq!(ready(&touched), Some(vec![(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)]));
    }

    #[test]
    fn req_fill_gdd_002_a_loop_inside_a_loop_takes_2_rounds() {
        // The outer loop crosses the line at (2, 0), and an inner loop crosses the outer one at (6, 4): the
        // inner one goes first, which closes the outer one, and then it goes.
        let nested = [(0.0, 0.0), (8.0, 0.0), (8.0, 4.0), (4.0, 4.0), (4.0, 6.0), (6.0, 6.0), (6.0, 2.0), (2.0, 2.0), (2.0, -3.0)];
        assert_eq!(ready(&nested), Some(vec![(0.0, 0.0), (2.0, 0.0), (2.0, -3.0)]));
    }

    #[test]
    fn req_fill_gdd_002_a_closed_figure_of_8_keeps_one_loop_and_opens_it() {
        // A figure of 8 from a corner: its loop through (10, 10) and (10, 0) is left out, and the other loop,
        // closed, loses its last 2 points.
        let eight = [(0.0, 0.0), (10.0, 10.0), (10.0, 0.0), (0.0, 10.0), (0.0, 0.0)];
        assert_eq!(ready(&eight), Some(vec![(0.0, 0.0), (5.0, 5.0)]));
    }

    #[test]
    fn req_fill_gdd_002_a_guide_that_cannot_be_made_one_line_is_the_line_between_its_ends() {
        // Back along itself, from (10, 0) to (5, 0): the straight line from its first point to its last.
        assert_eq!(ready(&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0), (5.0, 5.0)]), Some(vec![(0.0, 0.0), (5.0, 5.0)]));
        // Crossing itself twice, so that no piece is a loop: 2 points that 4 pieces meet at.
        let twice = [(0.0, 0.0), (10.0, 0.0), (10.0, 4.0), (3.0, 4.0), (3.0, -2.0), (7.0, -2.0), (7.0, 6.0), (12.0, 6.0)];
        assert_eq!(ready(&twice), Some(vec![(0.0, 0.0), (12.0, 6.0)]));
    }

    #[test]
    fn req_fill_gdd_002_a_guide_with_no_length_is_none() {
        assert_eq!(ready(&[]), None);
        assert_eq!(ready(&[(1.0, 1.0)]), None);
        // A figure of 8 drawn from its crossing: both its pieces are loops, and nothing is left.
        assert_eq!(ready(&[(5.0, 5.0), (10.0, 10.0), (10.0, 0.0), (5.0, 5.0), (0.0, 10.0), (0.0, 0.0), (5.0, 5.0)]), None);
    }

    #[test]
    fn finding_where_a_guide_meets_itself_is_charged_to_the_budget() {
        let s = points(&[(0.0, 0.0), (3.0, 1.0), (6.0, -1.0), (9.0, 0.0)]);
        assert!(prepared(&s, &mut Budget { max_stitches: 10, max_work: 1 }.meter()).is_err());
    }
}
