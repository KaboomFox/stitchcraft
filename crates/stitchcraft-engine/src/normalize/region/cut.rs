//! A line cut to a region: the pieces of it inside, which a stroke cut to its clips sews (`REQ-GEN-005`).
//!
//! Ink/Stitch cuts a stroke's centre line to its clip with its geometry library's intersection, and sews each
//! line of the result on its own, in the order the intersection gives them, which is the order along the
//! line. Here the line is cut wherever it meets a ring of the region, decided exactly as 2 segments meet
//! ([`geom::hit`]). Each stretch between cuts then lies wholly inside, wholly outside or along a ring, and
//! its middle says which. A stretch along a ring is kept, as the intersection keeps the boundary and a
//! viewer's clip shows it.
//!
//! A stroke's path is cut subpath by subpath ([`path`]), each taken as the line its generator follows: a
//! running stitch's and a zigzag stroke's flattened as a shape's outline is, a manual stitch's from node to
//! node, as Ink/Stitch cuts them. A subpath the clips leave whole is kept as drawn, curves and all, so it sews
//! as it would unclipped. One they cut becomes its pieces, each a subpath of lines, and a closed one is cut
//! where it starts and ends too, as the intersection cuts a closed line at its ends.

use std::borrow::Cow;

use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use super::geom::{self, Hit};
use super::{Polygon, Unjoined, clipped};
use crate::design::{Clip, Path, Segment, Subpath};

/// The line a subpath is taken as, to be cut ([`path`]): its points in order, with `meter`'s work.
pub(crate) type Line = fn(&Subpath, &mut Meter) -> Result<Vec<Point>, Exhausted>;

/// `path`, a stroke's, cut to `clips` along the lines `line` makes of its subpaths ([`path`]): as it is when
/// there are none, and `None` when they leave nothing of it, which `SC-W0013` says in `diagnostics`, or their
/// area can't be built, which `SC-E0009` says.
pub(crate) fn stroke<'p>(
    path: &'p Path,
    clips: &[Clip],
    line: Line,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Cow<'p, Path>>, Exhausted> {
    if clips.is_empty() {
        return Ok(Some(Cow::Borrowed(path)));
    }
    // Clips whose area doesn't join into rings are a bug ([`Unjoined`]), which no drawing tried makes.
    let kept = self::path(path, clips, line, meter)?;
    diagnostics.extend(kept.as_ref().err().copied().map(Unjoined::clip));
    let Ok(kept) = kept else { return Ok(None) };
    if kept.subpaths.is_empty() {
        diagnostics.push(Diagnostic::new(Code::ClippedAway, "Its clip paths leave nothing of this stroke, so it sews nothing."));
        return Ok(None);
    }
    Ok(Some(Cow::Owned(kept)))
}

/// `path`, a stroke's, cut to `clips` (`REQ-GEN-005`): of each subpath, taken as the line `line` makes of
/// it, the pieces inside every clip, in order ([`inside`]). A subpath they leave whole is kept as it is,
/// and one that is a point is kept when the point lies inside them. `Err` with [`Unjoined`] when the
/// clips' area can't be built, a bug. The work of building their area, of each line and of each cut.
pub(crate) fn path(path: &Path, clips: &[Clip], line: Line, meter: &mut Meter) -> Result<Result<Path, Unjoined>, Exhausted> {
    let Ok(parts) = clipped(clips, meter)? else { return Ok(Err(Unjoined)) };
    let mut subpaths = Vec::with_capacity(path.subpaths.len());
    for subpath in &path.subpaths {
        let points = line(subpath, meter)?;
        if let [point] = points.as_slice() {
            if parts.iter().any(|part| part.covers(*point)) {
                subpaths.push(subpath.clone());
            }
            continue;
        }
        let pieces = inside(&points, &parts, meter)?;
        match pieces.as_slice() {
            [whole] if whole.first() == points.first() && whole.last() == points.last() => subpaths.push(subpath.clone()),
            _ => subpaths.extend(pieces.iter().filter_map(|piece| lines(piece))),
        }
    }
    Ok(Ok(Path { subpaths }))
}

/// The polyline `points` as an open subpath of lines.
fn lines(points: &[Point]) -> Option<Subpath> {
    let (&start, rest) = points.split_first()?;
    Some(Subpath { start, segments: rest.iter().map(|&p| Segment::Line(p)).collect(), closed: false })
}

/// The pieces of `line` that lie in `parts`, in order along it, each a polyline of 2 points or more. A piece
/// starts and ends where the line crosses or meets a ring, or at the line's own ends. A unit of `meter` for
/// each pair of a segment of the line and a side of a ring, and for each stretch between cuts.
pub(crate) fn inside(line: &[Point], parts: &[Polygon], meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let (mut pieces, mut piece): (Vec<Vec<Point>>, Vec<Point>) = (Vec::new(), Vec::new());
    for pair in line.windows(2) {
        let &[a, b] = pair else { continue };
        if a == b {
            continue;
        }
        let mut cuts = vec![(0.0, a), (1.0, b)];
        for side in parts.iter().flat_map(Polygon::rings).flat_map(|ring| ring.windows(2)) {
            let &[c, d] = side else { continue };
            meter.charge(1)?;
            match geom::hit(a, b, c, d) {
                Hit::Apart => {}
                Hit::Cross(p) | Hit::Touch(p) => cuts.push((along(a, b, p), p)),
                Hit::Overlap(p, q) => cuts.extend([(along(a, b, p), p), (along(a, b, q), q)]),
            }
        }
        cuts.sort_by(|x, y| x.0.total_cmp(&y.0));
        cuts.dedup_by(|x, y| x.0 == y.0);
        for stretch in cuts.windows(2) {
            let &[(s, from), (t, to)] = stretch else { continue };
            meter.charge(1)?;
            if parts.iter().any(|part| part.covers(a.lerp(b, (s + t) / 2.0))) {
                if piece.last() != Some(&from) {
                    end(&mut pieces, &mut piece);
                    piece.push(from);
                }
                piece.push(to);
            } else {
                end(&mut pieces, &mut piece);
            }
        }
    }
    end(&mut pieces, &mut piece);
    Ok(pieces)
}

/// Ends `piece`, which joins `pieces` when it has a stretch.
fn end(pieces: &mut Vec<Vec<Point>>, piece: &mut Vec<Point>) {
    let done = std::mem::take(piece);
    if done.len() >= 2 {
        pieces.push(done);
    }
}

/// How far along `a`–`b` the point `p` on it lies, from 0 at `a` to 1 at `b`.
fn along(a: Point, b: Point, p: Point) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    (((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / (dx * dx + dy * dy)).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::design::Segment;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A square from `(x, y)`, `side` mm a side, as a part.
    fn square(x: f64, y: f64, side: f64) -> Polygon {
        let ring = vec![p(x, y), p(x, y + side), p(x + side, y + side), p(x + side, y), p(x, y)];
        Polygon { outline: ring, holes: Vec::new() }
    }

    fn cut(line: &[Point], parts: &[Polygon]) -> Vec<Vec<Point>> {
        inside(line, parts, &mut Budget::DEFAULT.meter()).unwrap()
    }

    #[test]
    fn a_line_is_cut_where_it_crosses_a_ring_and_keeps_the_pieces_inside_in_order() {
        let parts = [square(0.0, 0.0, 10.0), square(20.0, 0.0, 10.0)];
        // Across both squares and the gap between: 2 pieces, in order along the line.
        assert_eq!(cut(&[p(-5.0, 5.0), p(35.0, 5.0)], &parts), [vec![p(0.0, 5.0), p(10.0, 5.0)], vec![p(20.0, 5.0), p(30.0, 5.0)]]);
        // A polyline that goes out and comes back in.
        let zig = [p(5.0, 5.0), p(15.0, 5.0), p(25.0, 5.0), p(25.0, 15.0)];
        assert_eq!(cut(&zig, &parts), [vec![p(5.0, 5.0), p(10.0, 5.0)], vec![p(20.0, 5.0), p(25.0, 5.0), p(25.0, 10.0)]]);
        // Wholly inside, it is kept as it is, its points all; wholly outside, nothing.
        assert_eq!(cut(&[p(1.0, 1.0), p(5.0, 2.0), p(9.0, 9.0)], &parts), [vec![p(1.0, 1.0), p(5.0, 2.0), p(9.0, 9.0)]]);
        assert!(cut(&[p(11.0, -5.0), p(19.0, 15.0)], &parts).is_empty());
    }

    #[test]
    fn a_point_repeated_along_a_line_adds_nothing_to_its_pieces() {
        // A manual stitch's nodes may repeat a point: the step between them goes nowhere and is passed over.
        let parts = [square(0.0, 0.0, 10.0)];
        assert_eq!(cut(&[p(1.0, 1.0), p(1.0, 1.0), p(5.0, 1.0), p(15.0, 1.0)], &parts), [vec![p(1.0, 1.0), p(5.0, 1.0), p(10.0, 1.0)]]);
    }

    #[test]
    fn a_stretch_along_a_ring_is_kept_and_a_hole_cuts_the_line() {
        // Along the square's edge, and through a part with a hole.
        let parts = [square(0.0, 0.0, 10.0)];
        assert_eq!(cut(&[p(-5.0, 0.0), p(15.0, 0.0)], &parts), [vec![p(0.0, 0.0), p(10.0, 0.0)]]);
        let mut holed = square(0.0, 0.0, 30.0);
        holed.holes.push(vec![p(10.0, 10.0), p(20.0, 10.0), p(20.0, 20.0), p(10.0, 20.0), p(10.0, 10.0)]);
        assert_eq!(cut(&[p(0.0, 15.0), p(30.0, 15.0)], &[holed]), [vec![p(0.0, 15.0), p(10.0, 15.0)], vec![p(20.0, 15.0), p(30.0, 15.0)]]);
        // A line through a corner alone touches the part and keeps nothing.
        assert!(cut(&[p(-5.0, 5.0), p(5.0, -5.0)], &parts).is_empty());
    }

    /// The clip of the rectangle from (`x0`, `y0`) to (`x1`, `y1`).
    fn clip(x0: f64, y0: f64, x1: f64, y1: f64) -> Clip {
        let ring = vec![Segment::Line(p(x1, y0)), Segment::Line(p(x1, y1)), Segment::Line(p(x0, y1))];
        Clip::of(Path { subpaths: vec![Subpath { start: p(x0, y0), segments: ring, closed: true }] }, crate::design::FillRule::NonZero)
    }

    /// `subpaths` cut to `clips`, each taken as its nodes, as a manual stitch's are.
    fn cut_path(subpaths: &[Subpath], clips: &[Clip]) -> Vec<Subpath> {
        path(&Path { subpaths: subpaths.to_vec() }, clips, crate::generators::manual::nodes, &mut Budget::DEFAULT.meter()).unwrap().unwrap().subpaths
    }

    /// The open subpath of lines through `points`.
    fn open(points: &[(f64, f64)]) -> Subpath {
        lines(&points.iter().map(|&(x, y)| p(x, y)).collect::<Vec<_>>()).unwrap()
    }

    #[test]
    fn a_subpath_left_whole_is_kept_as_drawn_and_one_cut_becomes_its_pieces_of_lines() {
        let clips = [clip(0.0, 0.0, 10.0, 10.0)];
        // A curve and a closed ring inside, kept as they are; a point inside kept, and one outside left out.
        let curve = Subpath { start: p(1.0, 1.0), segments: vec![Segment::Cubic(p(2.0, 9.0), p(8.0, 9.0), p(9.0, 1.0))], closed: false };
        let ring = Subpath { start: p(2.0, 2.0), segments: vec![Segment::Line(p(8.0, 2.0)), Segment::Line(p(5.0, 8.0))], closed: true };
        let point = |x, y| Subpath { start: p(x, y), segments: Vec::new(), closed: false };
        let kept = [curve.clone(), ring.clone(), point(5.0, 5.0)];
        assert_eq!(cut_path(&[curve, ring, point(5.0, 5.0), point(15.0, 5.0)], &clips), kept);
        // Lines that start or end inside and cross out: each cut where it crosses.
        let out = open(&[(5.0, 5.0), (15.0, 5.0)]);
        assert_eq!(cut_path(&[out], &clips), [open(&[(5.0, 5.0), (10.0, 5.0)])]);
        let back = open(&[(-5.0, 5.0), (5.0, 5.0)]);
        assert_eq!(cut_path(&[back], &clips), [open(&[(0.0, 5.0), (5.0, 5.0)])]);
    }

    #[test]
    fn a_closed_subpath_is_cut_where_it_starts_too() {
        // A square ring from (5, 2), its left side outside the clip: the piece from its start to the clip's
        // edge, then the piece from the edge back to its start, as the intersection cuts a closed line.
        let ring = Subpath {
            start: p(5.0, 2.0),
            segments: vec![Segment::Line(p(8.0, 2.0)), Segment::Line(p(8.0, 8.0)), Segment::Line(p(-2.0, 8.0)), Segment::Line(p(-2.0, 2.0))],
            closed: true,
        };
        let pieces = cut_path(&[ring], &[clip(0.0, 0.0, 10.0, 10.0)]);
        assert_eq!(pieces, [open(&[(5.0, 2.0), (8.0, 2.0), (8.0, 8.0), (0.0, 8.0)]), open(&[(0.0, 2.0), (5.0, 2.0)])]);
    }

    #[test]
    fn the_work_is_charged() {
        let parts = [square(0.0, 0.0, 10.0)];
        let mut meter = Budget { max_work: 5, ..Budget::DEFAULT }.meter();
        assert!(inside(&[p(-5.0, 5.0), p(35.0, 5.0)], &parts, &mut meter).is_err(), "4 sides and the stretches cost more than 5");
    }
}
