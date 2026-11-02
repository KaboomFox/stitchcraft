//! A circular fill's spiral cut by its part: the stretches of the spiral inside the part, its pieces, as
//! GEOS cuts a line by a polygon for Ink/Stitch (`REQ-FILL-CIR-002`).
//!
//! **Where it is cut.** The spiral is cut at every point where it meets one of the part's rings: where it
//! crosses one, touches one, or starts or stops running along one. Each stretch between 2 such points lies
//! inside the part or outside it, and the stretches inside, those whose middle the part covers, are the
//! pieces, in order along the spiral and running its way. A stretch along a ring is inside. 2 pieces that
//! meet where the spiral touches a ring from inside stay 2, as GEOS leaves them, and a point where the
//! spiral only touches the part is no piece (both checked with shapely 2.2 on GEOS 3.14).
//!
//! **Spans.** The spiral comes as spans, its turns and the steps between them, which the running stitch
//! stitches one by one, as a contour fill's spiral is stitched ([`Spanned`]). A piece keeps the spans of
//! the stretch of spiral it is, cut where it starts and ends.
//!
//! **Cost.** Only the sides of the rings near each side of the spiral are looked at ([`Rings::meeting`]).
//! Where a side meets none, it lies on the side of the rings the one before ended on, and only a stretch
//! that starts where the spiral meets a ring asks whether the part covers it.

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::tatami::rings::Rings;
use crate::normalize::region::SAME;
use crate::normalize::region::geom::{Hit, hit};

/// A line in spans: its points, and where each span after the first starts, as places in the points. A
/// span runs from its start to the next span's start, or to the line's end, both included.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Spanned {
    /// The line's points.
    pub points: Vec<Point>,
    /// Where each span after the first starts, increasing, each a place in `points` after the first.
    pub starts: Vec<usize>,
}

impl Spanned {
    /// The line through `stretches`, one after another: each stretch a span, and the step from the end of
    /// one to the start of the next a span of its own where they are apart.
    pub fn joined(stretches: &[Vec<Point>]) -> Spanned {
        let mut line = Spanned::default();
        for stretch in stretches {
            let (Some(&first), Some(&last)) = (stretch.first(), line.points.last()) else {
                line.points.extend_from_slice(stretch);
                continue;
            };
            let at = line.points.len() - 1;
            if first == last {
                line.starts.push(at);
                line.points.extend_from_slice(stretch.get(1..).unwrap_or_default());
            } else {
                line.starts.extend([at, at + 1]);
                line.points.extend_from_slice(stretch);
            }
        }
        line.starts.retain(|&start| start > 0 && start + 1 < line.points.len());
        line.starts.dedup();
        line
    }

    /// Adds `more` after the line: its spans, the first starting where the line ends, which it goes on
    /// from, or with the step to it a span of its own where it starts elsewhere.
    pub fn extend(&mut self, more: &Spanned) {
        let Some(&first) = more.points.first() else { return };
        let Some(&last) = self.points.last() else {
            *self = more.clone();
            return;
        };
        let at = self.points.len() - 1;
        let skip = usize::from(first == last);
        if skip == 0 {
            self.starts.push(at);
        }
        let base = at + 1 - skip;
        self.starts.push(base);
        self.points.extend_from_slice(more.points.get(skip..).unwrap_or_default());
        self.starts.extend(more.starts.iter().map(|&start| base + start));
        self.starts.retain(|&start| start > 0 && start + 1 < self.points.len());
        self.starts.dedup();
    }

    /// The spans, each from its start to the next one's, both included.
    pub fn spans(&self) -> Vec<&[Point]> {
        let last = self.points.len().saturating_sub(1);
        let bounds: Vec<usize> = std::iter::once(0).chain(self.starts.iter().copied()).chain(std::iter::once(last)).collect();
        bounds.windows(2).filter_map(|pair| self.points.get(pair[0]..=pair[1])).filter(|span| span.len() > 1).collect()
    }

    /// How long the line is.
    fn length(&self) -> f64 {
        self.points.windows(2).map(|side| side[0].distance(side[1])).sum()
    }
}

/// The pieces of `line` inside the part whose rings are `rings`, in order along it, each running its way
/// with its spans. A piece shorter than [`SAME`] is left out. Costs as [`Rings::meeting`] and
/// [`Rings::covers`] do, and one unit of `meter` for each side of the line and each point where it meets
/// a ring.
pub(super) fn cut(line: &Spanned, rings: &Rings<'_>, meter: &mut Meter) -> Result<Vec<Spanned>, Exhausted> {
    let mut starts = vec![false; line.points.len()];
    for &start in &line.starts {
        if let Some(slot) = starts.get_mut(start) {
            *slot = true;
        }
    }
    let mut cutting = Cutting { pieces: Vec::new(), piece: None };
    // Whether the line lies inside the part where it is, and `None` where it meets a ring, from where the
    // part is asked.
    let mut inside: Option<bool> = None;
    for (i, side) in line.points.windows(2).enumerate() {
        meter.charge(1)?;
        let (p, q) = (side[0], side[1]);
        let mut meets: Vec<(f64, Point)> = Vec::new();
        rings.meeting(p, q, meter, |ring| match hit(p, q, ring.a, ring.b) {
            Hit::Cross(x) | Hit::Touch(x) => meets.push((p.distance(x), x)),
            Hit::Overlap(x, y) => meets.extend([(p.distance(x), x), (p.distance(y), y)]),
            Hit::Apart => {}
        })?;
        meter.charge(u64::try_from(meets.len()).unwrap_or(u64::MAX))?;
        meets.sort_by(|a, b| a.0.total_cmp(&b.0));
        meets.dedup_by(|a, b| a.1 == b.1);
        let mut from = p;
        let stops = meets.into_iter().map(|(_, x)| (x, true)).chain(std::iter::once((q, false)));
        for (to, meeting) in stops {
            if to != from {
                let covered = match inside {
                    Some(known) => known,
                    None => rings.covers(from.lerp(to, 0.5), meter)?,
                };
                if covered {
                    // The line's own points start their spans; a point where it meets a ring starts none.
                    let start = !meeting && starts.get(i + 1).copied().unwrap_or(false);
                    cutting.take(from, to, start);
                } else {
                    cutting.end();
                }
                inside = Some(covered);
            }
            if meeting {
                cutting.end();
                inside = None;
            }
            from = to;
        }
    }
    cutting.end();
    Ok(cutting.pieces)
}

/// The pieces as the line is cut: those ended, and the one it is in.
struct Cutting {
    pieces: Vec<Spanned>,
    piece: Option<Spanned>,
}

impl Cutting {
    /// The stretch from `from` to `to` is inside: it goes on the piece the line is in, or starts one, `to`
    /// starting a span when `start` says so.
    fn take(&mut self, from: Point, to: Point, start: bool) {
        let piece = self.piece.get_or_insert_with(|| Spanned { points: vec![from], starts: Vec::new() });
        if start {
            piece.starts.push(piece.points.len());
        }
        piece.points.push(to);
    }

    /// The piece the line is in ends, and is kept if it has any length.
    fn end(&mut self) {
        let Some(mut piece) = self.piece.take() else { return };
        piece.starts.retain(|&start| start + 1 < piece.points.len());
        if piece.length() > SAME {
            self.pieces.push(piece);
        }
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::generators::tatami::fixture::{frame, p, rectangle};
    use crate::normalize::region::Polygon;

    fn line(points: &[(f64, f64)]) -> Spanned {
        Spanned { points: points.iter().map(|&(x, y)| p(x, y)).collect(), starts: Vec::new() }
    }

    fn cut_by(part: &Polygon, line: &Spanned) -> Vec<Vec<(f64, f64)>> {
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(part, meter).unwrap();
        cut(line, &rings, meter).unwrap().iter().map(|piece| piece.points.iter().map(|q| (q.x(), q.y())).collect()).collect()
    }

    #[test]
    fn req_fill_cir_002_the_pieces_are_the_stretches_inside_in_order_along_the_line() {
        // In and out of a 10 × 4 rectangle twice, and the same line backwards: the pieces run the line's way,
        // in its order, as shapely gives them.
        let part = rectangle(4.0);
        let zigzag = [(-1.0, 1.0), (1.0, 1.0), (1.0, 5.0), (3.0, 5.0), (3.0, 1.0), (11.0, 1.0)];
        assert_eq!(cut_by(&part, &line(&zigzag)), [vec![(0.0, 1.0), (1.0, 1.0), (1.0, 4.0)], vec![(3.0, 4.0), (3.0, 1.0), (10.0, 1.0)]]);
        let back: Vec<(f64, f64)> = zigzag.iter().rev().copied().collect();
        assert_eq!(cut_by(&part, &line(&back)), [vec![(10.0, 1.0), (3.0, 1.0), (3.0, 4.0)], vec![(1.0, 4.0), (1.0, 1.0), (0.0, 1.0)]]);
        // A line wholly inside is one piece, and one wholly outside none.
        assert_eq!(cut_by(&part, &line(&[(1.0, 1.0), (2.0, 2.0)])), [vec![(1.0, 1.0), (2.0, 2.0)]]);
        assert!(cut_by(&part, &line(&[(-1.0, -1.0), (-2.0, 5.0)])).is_empty());
    }

    #[test]
    fn a_line_touching_a_ring_from_inside_is_2_pieces_and_a_touch_from_outside_none() {
        // Down to the bottom side at (5, 0) and back up: 2 pieces meeting there, as GEOS cuts it.
        let part = rectangle(4.0);
        assert_eq!(cut_by(&part, &line(&[(3.0, 2.0), (5.0, 0.0), (7.0, 2.0)])), [vec![(3.0, 2.0), (5.0, 0.0)], vec![(5.0, 0.0), (7.0, 2.0)]]);
        // Outside, up to (5, 0) and back down: nothing.
        assert!(cut_by(&part, &line(&[(3.0, -2.0), (5.0, 0.0), (7.0, -2.0)])).is_empty());
        // Along the bottom side, out of the part on both sides: the stretch along it is a piece.
        assert_eq!(cut_by(&part, &line(&[(-1.0, 0.0), (11.0, 0.0)])), [vec![(0.0, 0.0), (10.0, 0.0)]]);
    }

    #[test]
    fn a_hole_cuts_the_line_too() {
        // The frame's hole spans x = 4 to 6 at y = 2: a line across the frame there is 2 pieces.
        assert_eq!(cut_by(&frame(), &line(&[(-1.0, 2.0), (11.0, 2.0)])), [vec![(0.0, 2.0), (4.0, 2.0)], vec![(6.0, 2.0), (10.0, 2.0)]]);
    }

    #[test]
    fn a_piece_no_longer_than_same_is_left_out() {
        // Points SAME apart are one point: inside a square round the origin, a line half of SAME long or SAME
        // long is no piece, and one twice SAME long is one.
        let square = Polygon { outline: vec![p(-1.0, -1.0), p(-1.0, 1.0), p(1.0, 1.0), p(1.0, -1.0), p(-1.0, -1.0)], holes: Vec::new() };
        for (long, pieces) in [(0.5 * SAME, 0), (SAME, 0), (2.0 * SAME, 1)] {
            assert_eq!(cut_by(&square, &line(&[(0.0, 0.0), (long, 0.0)])).len(), pieces, "{long}");
        }
    }

    #[test]
    fn a_piece_keeps_the_spans_of_its_stretch_of_line() {
        // 3 stretches across the rectangle, the second starting where the first ends and the third apart:
        // spans start at (5, 1), at (6, 1) for the step, and at (7, 1).
        let stretches = vec![vec![p(-1.0, 1.0), p(5.0, 1.0)], vec![p(5.0, 1.0), p(6.0, 1.0)], vec![p(7.0, 1.0), p(11.0, 1.0)]];
        let joined = Spanned::joined(&stretches);
        assert_eq!(joined.starts, [1, 2, 3]);
        let meter = &mut Budget::DEFAULT.meter();
        let part = rectangle(4.0);
        let rings = Rings::new(&part, meter).unwrap();
        let pieces = cut(&joined, &rings, meter).unwrap();
        assert_eq!(pieces.len(), 1);
        let spans: Vec<Vec<f64>> = pieces[0].spans().iter().map(|span| span.iter().map(|q| q.x()).collect()).collect();
        assert_eq!(spans, [vec![0.0, 5.0], vec![5.0, 6.0], vec![6.0, 7.0], vec![7.0, 10.0]]);
    }

    #[test]
    fn a_span_has_2_points_at_least() {
        // A stretch of one point starts no span where it starts or ends the line.
        let ends = Spanned::joined(&[vec![p(0.0, 0.0)], vec![p(1.0, 0.0), p(2.0, 0.0)], vec![p(3.0, 0.0)]]);
        assert_eq!((ends.points.len(), ends.starts), (4, vec![1, 2]));
        // A line of one point has no span.
        assert!(Spanned { points: vec![p(0.0, 0.0)], starts: Vec::new() }.spans().is_empty());
    }

    #[test]
    fn a_line_extended_by_one_point_starts_no_span_there() {
        // By the point it ends at, a line is extended by nothing. By a point elsewhere, the step to it is the
        // line's last span, which starts where the line ended: the point itself starts none.
        let mut a = line(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0)]);
        let before = a.clone();
        a.extend(&line(&[(2.0, 0.0)]));
        assert_eq!(a, before);
        a.extend(&line(&[(3.0, 1.0)]));
        assert_eq!((a.points.len(), a.starts), (4, vec![2]));
    }

    #[test]
    fn a_line_of_one_point_extended_starts_no_span_at_its_first_point() {
        // No span starts at a line's first point. A point extended by a line that starts elsewhere: the step
        // from the point to the line is the first span, and the line's own starts after it. A point extended by
        // a line that starts there: the line alone, with its own spans.
        let mut a = line(&[(0.0, 0.0)]);
        a.extend(&line(&[(1.0, 0.0), (2.0, 0.0)]));
        assert_eq!((a.points.len(), a.starts), (3, vec![1]));
        let mut b = line(&[(1.0, 0.0)]);
        b.extend(&line(&[(1.0, 0.0), (2.0, 0.0)]));
        assert_eq!(b, line(&[(1.0, 0.0), (2.0, 0.0)]));
    }

    #[test]
    fn a_piece_starts_no_span_at_its_last_point() {
        // Inside the rectangle, a line whose spans start at (5, 1) and at its last point, (6, 1): a span from
        // the last point would be that point alone, so its piece starts only the first.
        let part = rectangle(4.0);
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let spanned = Spanned { starts: vec![1, 2], ..line(&[(1.0, 1.0), (5.0, 1.0), (6.0, 1.0)]) };
        assert_eq!(cut(&spanned, &rings, meter).unwrap(), [Spanned { starts: vec![1], ..spanned.clone() }]);
    }

    #[test]
    fn lines_extend_with_their_spans() {
        let mut a = line(&[(0.0, 0.0), (1.0, 0.0)]);
        a.extend(&Spanned { points: vec![p(1.0, 0.0), p(2.0, 0.0), p(3.0, 0.0)], starts: vec![1] });
        assert_eq!((a.points.len(), a.starts.clone()), (4, vec![1, 2]));
        a.extend(&line(&[(5.0, 0.0), (6.0, 0.0)]));
        assert_eq!(a.starts, [1, 2, 3, 4]);
        let mut empty = Spanned::default();
        empty.extend(&a);
        assert_eq!(empty, a);
    }

    #[test]
    fn cutting_is_charged_to_the_budget() {
        let part = rectangle(4.0);
        let meter = &mut Budget::DEFAULT.meter();
        let rings = Rings::new(&part, meter).unwrap();
        let zigzag = line(&[(-1.0, 1.0), (1.0, 1.0), (1.0, 5.0), (3.0, 5.0), (3.0, 1.0), (11.0, 1.0)]);
        assert!(cut(&zigzag, &rings, &mut Budget { max_stitches: 1, max_work: 3 }.meter()).is_err());
    }
}
