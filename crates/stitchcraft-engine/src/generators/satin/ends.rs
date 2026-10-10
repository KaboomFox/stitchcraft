//! Where a satin column starts and ends (`docs/src/design/algorithms/satin.md` › Start and end): near
//! where the elements before it left the needle (`start_at_nearest_point`), and near where the next
//! element starts (`end_at_nearest_point`), as Ink/Stitch does by default.
//!
//! - **The line** between the rails at `running_stitch_position`, a running stitch of the travel length
//!   within `running_stitch_tolerance_mm`, is the needle's way to the start and between the passes.
//! - **Cuts.** A place is found by its cut: of the pairs at the zigzag spacing, with no compensation and
//!   nothing random, the one with the point nearest it. The line is entered where it comes nearest the
//!   cut, and a part is cut in 2 at its own point nearest it.
//! - **The start** is the line's point nearest the needle, or the compensated outline's when only that is
//!   within the jump length. The needle follows the line from the start's cut to the first stitch's.
//! - **The end** is the compensated outline's point nearest the next stitch, unless it is within 5 CSS
//!   pixels of the line's end. Every part is cut in 2 at the end's cut: the first halves are sewn, the line
//!   is followed to its end (or, after an odd centre walk, to where the second pass starts), the second
//!   halves are sewn, and the end is stitched last.
//!
//! The pieces are joined as the underlays are, in straight stitches no longer than the travel. A point of
//! the line's way within the shortest stitch of the needle, or of the way's point before it, only routes
//! the needle, and is left out: finalize would leave it out too, with a note about a point the design never
//! drew. An end within the shortest stitch of the needle moves the needle's last point there instead of
//! adding a stitch too short to sew, so the column's last stitch, often a top stitch, is never split.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Diagnostic, Exhausted, Meter, Point};

use crate::generators::Approach;
use crate::generators::passes;
use crate::generators::running::raised;
use crate::generators::satin::column::Section;
use crate::generators::satin::compensation::Processor;
use crate::generators::satin::pairs::{Pair, pairs};
use crate::generators::satin::split::evenly;
use crate::generators::satin::underlay::{Layer, split_long, walk_line};
use crate::generators::satin::{SatinLengths, SatinParams};
use crate::normalize::along::Along;
use crate::normalize::near::{Look, along_to_segment, length, nearest_to_lines, nearest_to_point};

// Ink/Stitch measures these limits in CSS pixels.

/// An end nearer than this to the line's end changes nothing: 5 CSS pixels.
const END_NEAR: f64 = 5.0 * MM_PER_SVG_PX;
/// Where a cut's 2 points coincide, a part whose start is nearer than this to the place is all second half:
/// 0.1 CSS pixels.
const START_NEAR: f64 = 0.1 * MM_PER_SVG_PX;
/// A cut whose 2 points are nearer than this is a point: Ink/Stitch's precision grid, 1e-5 CSS pixels.
const SAME: f64 = 1e-5 * MM_PER_SVG_PX;

/// A piece of a column's run, in the order it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Piece {
    /// Needle points of the column itself: an underlay's, the top stitches', or the start.
    Sewn(Vec<Point>),
    /// The way along the line, which only routes the needle.
    Way(Vec<Point>),
    /// The end, the column's last stitch.
    End(Point),
}

/// `pieces` sewn one after the other, the needle going straight from each to the next in equal stitches no
/// longer than `travel`. A way's point within `min_stitch` of the needle, or of the way's point before it,
/// is left out, and so is a way's last point when the next piece starts within `min_stitch` of it. The end
/// takes the place of a needle point within `min_stitch` of it: the stitch to that point goes to the end
/// instead, one stitch as before. Ink/Stitch adds the end as a stitch of its own, too short to sew here.
pub(crate) fn join(pieces: Vec<Piece>, travel: f64, min_stitch: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut run: Vec<Point> = Vec::new();
    let mut after_way = false;
    for piece in pieces {
        let needle = run.last().copied();
        let (points, way) = match piece {
            Piece::Sewn(points) => (points, false),
            Piece::Way(points) => (spaced(needle, points, min_stitch), true),
            Piece::End(point) => {
                if let Some(needle) = run.last_mut().filter(|needle| needle.distance(point) < min_stitch) {
                    *needle = point;
                    continue;
                }
                (vec![point], false)
            }
        };
        let Some(&first) = points.first() else { continue };
        if after_way && run.last().is_some_and(|needle| needle.distance(first) < min_stitch) {
            run.pop();
        }
        if let Some(&from) = run.last() {
            run.extend(evenly(from, first, travel, meter)?);
        }
        run.extend(points);
        after_way = way;
    }
    Ok(run)
}

/// The points of `way` at least `min_stitch` from the needle at `needle` and from the point kept before
/// them.
fn spaced(needle: Option<Point>, way: Vec<Point>, min_stitch: f64) -> Vec<Point> {
    let mut kept: Vec<Point> = Vec::with_capacity(way.len());
    for point in way {
        if kept.last().copied().or(needle).is_none_or(|last| last.distance(point) >= min_stitch) {
            kept.push(point);
        }
    }
    kept
}

/// The line a column's needle follows to its start and between its passes, and the cuts that say where.
pub(crate) struct Guides {
    /// The running stitch along the line at `running_stitch_position`.
    line: Vec<Point>,
    /// The pairs at the zigzag spacing, with no compensation and nothing random.
    cuts: Vec<Pair>,
}

impl Guides {
    /// The guides of the column cut into `sections`, sewn as `params` and `lengths` say. The line keeps to
    /// the shortest stitch, raising its length with `SC-W0402` into `warnings` where it is shorter than
    /// twice that.
    pub(crate) fn new(
        sections: &[Section],
        params: &SatinParams,
        lengths: SatinLengths,
        warnings: &mut Vec<Diagnostic>,
        meter: &mut Meter,
    ) -> Result<Guides, Exhausted> {
        let min_stitch = lengths.min_stitch.get();
        let length = raised("running_stitch_length_mm", lengths.travel, min_stitch, warnings);
        let position = params.running_stitch_position / 100.0;
        let line = walk_line(sections, position, lengths.tolerance.get(), length, min_stitch, meter)?;
        let cuts = pairs(sections, params.zigzag_spacing_mm.get(), &mut Processor::inset([0.0; 2], [0.0; 2]), meter)?;
        Ok(Guides { line, cuts })
    }

    /// Where a column starts with the needle at `needle`: the line's point nearest it, or the point of the
    /// compensated `outline` nearest it when the line's point is farther than `jump` and the outline is not.
    pub(crate) fn start(&self, needle: Point, outline: [&[Point]; 2], jump: f64, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
        let Some((on_line, _)) = nearest_to_point(&[&self.line], needle, meter)? else { return Ok(None) };
        if length(needle, on_line) > jump
            && let Some((on_outline, near)) = nearest_to_point(&outline, needle, meter)?
            && near < jump
        {
            return Ok(Some(on_outline));
        }
        Ok(Some(on_line))
    }

    /// Where a column whose rails, swapped and turned as they are sewn, are `rails` ends before what `next`
    /// offers: the point of the compensated `outline` nearest the next stitch, the point of the rails
    /// nearest `next`. `None` when that is within 5 CSS pixels of the line's end.
    pub(crate) fn end(&self, next: &Approach, rails: [&[Point]; 2], outline: [&[Point]; 2], meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
        let next_stitch = match next {
            Approach::Point(point) => nearest_to_point(&rails, *point, meter)?.map(|(on, _)| on),
            Approach::Shape(lines) => nearest_to_lines(&rails, &lines.iter().map(Vec::as_slice).collect::<Vec<_>>(), meter)?,
        };
        let Some(next_stitch) = next_stitch else { return Ok(None) };
        let Some((end, _)) = nearest_to_point(&outline, next_stitch, meter)? else { return Ok(None) };
        let usual = self.line.last().is_some_and(|&last| length(last, end) < END_NEAR);
        Ok((!usual).then_some(end))
    }

    /// The pieces of a column with the underlays `layers` and the `top` stitches, started from `start` and
    /// ended at `end` when they are given. `odd` says the centre walk ends at the column's end.
    pub(crate) fn pieces(
        &self,
        layers: Vec<Layer>,
        top: Vec<Point>,
        start: Option<Point>,
        end: Option<Point>,
        odd: bool,
        meter: &mut Meter,
    ) -> Result<Vec<Piece>, Exhausted> {
        let mut pieces = match end {
            Some(end) => self.passes(layers, &top, end, odd, meter)?,
            None => sewn(layers, top, meter)?,
        };
        // The column's first stitch: the pieces start with what it sews, whichever pass comes first.
        let first = pieces.iter().find_map(|piece| if let Piece::Sewn(points) = piece { points.first().copied() } else { None });
        if let (Some(start), Some(first)) = (start, first) {
            let way = self.way(self.entry(start, meter)?, self.entry(first, meter)?, meter)?;
            pieces.splice(0..0, [Piece::Sewn(vec![start]), Piece::Way(way)]);
        }
        Ok(pieces)
    }

    /// The column in 2 passes: every part's first half, the way along the line, every part's second half,
    /// and the `end`.
    fn passes(&self, layers: Vec<Layer>, top: &[Point], end: Point, odd: bool, meter: &mut Meter) -> Result<Vec<Piece>, Exhausted> {
        let (mut first, mut second) = (Vec::new(), Vec::new());
        for layer in layers {
            match layer {
                Layer::Walk { way, repeats } => {
                    let (to_cut, from_cut) = self.halves(&way, end, meter)?;
                    let (one, mut two) = if odd { (from_cut, to_cut) } else { (to_cut, from_cut) };
                    two.reverse();
                    first.push(passes::sew(&one, repeats, &[], meter)?);
                    second.push(passes::sew(&two, repeats, &[], meter)?);
                }
                Layer::Contour([side_a, side_b]) => {
                    let (a_first, a_second) = self.halves(&side_a, end, meter)?;
                    let (b_first, b_second) = self.halves(&side_b, end, meter)?;
                    first.extend([a_first, b_second]);
                    second.extend([b_first, a_second]);
                }
                Layer::Zigzag { ways: [there, back], longest } => {
                    let (there_first, there_second) = self.halves(&there, end, meter)?;
                    let (back_first, back_second) = self.halves(&back, end, meter)?;
                    first.extend([split_long(there_first, longest, meter)?, split_long(back_second, longest, meter)?]);
                    second.extend([split_long(back_first, longest, meter)?, split_long(there_second, longest, meter)?]);
                }
            }
        }
        let (top_first, mut top_second) = self.halves(top, end, meter)?;
        top_second.reverse();
        first.push(top_first);
        second.push(top_second);
        let to = match second.first().and_then(|part| part.first()) {
            Some(&resume) if odd => self.entry(resume, meter)?,
            _ => Along::new(&self.line, meter)?.length(),
        };
        let way = self.way(self.entry(end, meter)?, to, meter)?;
        let mut pieces: Vec<Piece> = first.into_iter().map(Piece::Sewn).collect();
        pieces.push(Piece::Way(way));
        pieces.extend(second.into_iter().map(Piece::Sewn));
        pieces.push(Piece::End(end));
        Ok(pieces)
    }

    /// The cut through `place`: the pair with the point nearest it, the first of equally near ones.
    fn cut(&self, place: Point) -> Option<Pair> {
        let mut best: Option<(Pair, f64)> = None;
        for pair in &self.cuts {
            for &point in pair {
                let distance = length(point, place);
                if best.is_none_or(|(_, d)| distance < d) {
                    best = Some((*pair, distance));
                }
            }
        }
        best.map(|(pair, _)| pair)
    }

    /// How far along the line it comes nearest the cut through `place`.
    fn entry(&self, place: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
        match self.cut(place) {
            Some([a, b]) => along_to_segment(&self.line, (a, b), Look::FromSegment, meter),
            None => Ok(0.0),
        }
    }

    /// The line from `from` to `to` along it.
    fn way(&self, from: f64, to: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        Ok(stretch(&Along::new(&self.line, meter)?, from, to))
    }

    /// `part` cut in 2 at its point nearest the cut through `place`: the part up to there, and the part from
    /// there. Where the cut's 2 points coincide, the part is all second half when its start is within 0.1
    /// CSS pixels of `place`, and all first half otherwise, the other half its start or end.
    fn halves(&self, part: &[Point], place: Point, meter: &mut Meter) -> Result<(Vec<Point>, Vec<Point>), Exhausted> {
        let (Some(&start), Some(&last), Some([a, b])) = (part.first(), part.last(), self.cut(place)) else {
            return Ok((part.to_vec(), part.last().map(|&last| vec![last]).unwrap_or_default()));
        };
        if length(a, b) < SAME {
            return Ok(if length(start, place) < START_NEAR { (vec![start], part.to_vec()) } else { (part.to_vec(), vec![last]) });
        }
        let along = Along::new(part, meter)?;
        let at = along_to_segment(part, (a, b), Look::FromPolyline, meter)?;
        Ok((stretch(&along, 0.0, at), stretch(&along, at, along.length())))
    }
}

/// The parts of a column that nothing cuts: its underlays' and its `top` stitches, in the order they are
/// sewn.
pub(crate) fn sewn(layers: Vec<Layer>, top: Vec<Point>, meter: &mut Meter) -> Result<Vec<Piece>, Exhausted> {
    let mut pieces = Vec::new();
    for layer in layers {
        pieces.extend(layer.parts(meter)?.into_iter().map(Piece::Sewn));
    }
    pieces.push(Piece::Sewn(top));
    Ok(pieces)
}

/// The stretch of the polyline `along` from `from` to `to` along it: backwards when `to` comes first, and
/// one point when they are the same.
fn stretch(along: &Along<'_>, from: f64, to: f64) -> Vec<Point> {
    if from < to {
        along.part(from, to)
    } else if to < from {
        let mut part = along.part(to, from);
        part.reverse();
        part
    } else {
        vec![along.point(from)]
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// Guides along a straight line from (0, 3) to (10, 3), with cuts across it every 1 mm from (x, 0)
    /// to (x, 6).
    fn guides() -> Guides {
        let line = (0..=4).map(|i| p(2.5 * f64::from(i), 3.0)).collect();
        let cuts = (0..=10).map(|x| [p(f64::from(x), 0.0), p(f64::from(x), 6.0)]).collect();
        Guides { line, cuts }
    }

    #[test]
    fn ways_and_halves_follow_their_line() {
        let (g, meter) = (guides(), &mut Budget::DEFAULT.meter());
        assert_eq!(g.way(1.0, 6.0, meter).unwrap(), [p(1.0, 3.0), p(2.5, 3.0), p(5.0, 3.0), p(6.0, 3.0)]);
        assert_eq!(g.way(6.0, 1.0, meter).unwrap(), [p(6.0, 3.0), p(5.0, 3.0), p(2.5, 3.0), p(1.0, 3.0)], "backwards");
        assert_eq!(g.way(4.0, 4.0, meter).unwrap(), [p(4.0, 3.0)], "a point");
        // A place is found by its cut: 3.4 mm along is the cut at 3.
        assert_eq!(g.entry(p(3.4, 5.0), meter).unwrap(), 3.0);
        let part = [p(0.0, 1.0), p(10.0, 1.0)];
        assert_eq!(g.halves(&part, p(6.2, 0.5), meter).unwrap(), (vec![p(0.0, 1.0), p(6.0, 1.0)], vec![p(6.0, 1.0), p(10.0, 1.0)]));
        assert_eq!(g.halves(&part, p(-3.0, 0.0), meter).unwrap(), (vec![p(0.0, 1.0)], part.to_vec()), "cut at its start");
        assert_eq!(g.halves(&[], p(1.0, 0.0), meter).unwrap(), (Vec::new(), Vec::new()));
        assert_eq!(g.halves(&[p(4.0, 1.0)], p(1.0, 0.0), meter).unwrap(), (vec![p(4.0, 1.0)], vec![p(4.0, 1.0)]), "a point is both halves");
    }

    #[test]
    fn where_a_cut_is_a_point_a_part_is_not_cut() {
        // The rails meet at x = 10: the last cut's points coincide.
        let mut g = guides();
        g.cuts.push([p(11.0, 3.0), p(11.0, 3.0)]);
        let (part, meter) = ([p(11.0, 3.0), p(4.0, 3.0)], &mut Budget::DEFAULT.meter());
        assert_eq!(g.halves(&part, p(11.0, 3.01), meter).unwrap(), (vec![p(11.0, 3.0)], part.to_vec()), "its start is at the place");
        assert_eq!(g.halves(&part, p(11.0, 3.2), meter).unwrap(), (part.to_vec(), vec![p(4.0, 3.0)]), "its start is not");
    }

    #[test]
    fn with_no_cuts_nothing_is_cut() {
        let g = Guides { line: vec![p(0.0, 3.0), p(10.0, 3.0)], cuts: Vec::new() };
        let meter = &mut Budget::DEFAULT.meter();
        assert_eq!(g.entry(p(5.0, 3.0), meter).unwrap(), 0.0);
        let part = [p(0.0, 1.0), p(10.0, 1.0)];
        assert_eq!(g.halves(&part, p(5.0, 1.0), meter).unwrap(), (part.to_vec(), vec![p(10.0, 1.0)]));
    }

    #[test]
    fn a_way_s_points_near_the_needle_are_left_out() {
        let meter = &mut Budget::DEFAULT.meter();
        let way = vec![p(5.1, 3.0), p(2.5, 3.0), p(0.1, 3.0)];
        let pieces = vec![Piece::Sewn(vec![p(5.0, 3.0)]), Piece::Way(way), Piece::Sewn(vec![p(0.0, 3.0), p(0.0, 0.0)])];
        // The way's first point is 0.1 mm from the start, and its last 0.1 mm from the first stitch.
        assert_eq!(join(pieces, 10.0, 0.3, meter).unwrap(), [p(5.0, 3.0), p(2.5, 3.0), p(0.0, 3.0), p(0.0, 0.0)]);
        // A way all within the shortest stitch of the needle is left out whole.
        let pieces = vec![Piece::Sewn(vec![p(0.0, 0.0)]), Piece::Way(vec![p(0.1, 0.0)]), Piece::Sewn(vec![p(0.15, 0.0)])];
        assert_eq!(join(pieces, 10.0, 0.3, meter).unwrap(), [p(0.0, 0.0), p(0.15, 0.0)], "sewn points stay");
        // The end takes the place of a needle point near it, so the stitch to that point now ends at the
        // end: one stitch, however much longer than the travel. It is joined in straight stitches otherwise.
        let pieces = vec![Piece::Sewn(vec![p(0.0, 0.0), p(4.0, 0.0)]), Piece::End(p(4.2, 0.0))];
        assert_eq!(join(pieces, 2.5, 0.3, meter).unwrap(), [p(0.0, 0.0), p(4.2, 0.0)]);
        let pieces = vec![Piece::Sewn(vec![p(1.0, 1.0)]), Piece::End(p(1.1, 1.0))];
        assert_eq!(join(pieces, 2.5, 0.3, meter).unwrap(), [p(1.1, 1.0)], "a needle point alone too");
        let pieces = vec![Piece::Sewn(vec![p(0.0, 0.0)]), Piece::End(p(6.0, 0.0))];
        assert_eq!(join(pieces, 2.5, 0.3, meter).unwrap(), [p(0.0, 0.0), p(2.0, 0.0), p(4.0, 0.0), p(6.0, 0.0)]);
        assert_eq!(join(vec![Piece::End(p(1.0, 1.0))], 2.5, 0.3, meter).unwrap(), [p(1.0, 1.0)], "an end alone stays");
        // Exactly the shortest stitch away is not within it: the end is a stitch of its own, and a way's last
        // point is kept.
        let pieces = vec![Piece::Sewn(vec![p(0.0, 0.0), p(4.0, 0.0)]), Piece::End(p(4.25, 0.0))];
        assert_eq!(join(pieces, 10.0, 0.25, meter).unwrap(), [p(0.0, 0.0), p(4.0, 0.0), p(4.25, 0.0)]);
        let pieces = vec![Piece::Sewn(vec![p(0.0, 0.0)]), Piece::Way(vec![p(2.0, 0.0)]), Piece::Sewn(vec![p(2.25, 0.0)])];
        assert_eq!(join(pieces, 10.0, 0.25, meter).unwrap(), [p(0.0, 0.0), p(2.0, 0.0), p(2.25, 0.0)]);
    }

    #[test]
    fn a_cut_is_a_point_only_when_shorter_than_the_precision_grid() {
        let meter = &mut Budget::DEFAULT.meter();
        // A cut 1e-5 mm long, about 4 grid steps: a part crossing it is cut where it touches it.
        let mut g = guides();
        g.cuts.push([p(11.0, 3.0), p(11.0, 3.00001)]);
        let part = [p(10.0, 3.0), p(12.0, 3.0)];
        assert_eq!(g.halves(&part, p(11.0, 3.0), meter).unwrap(), (vec![p(10.0, 3.0), p(11.0, 3.0)], vec![p(11.0, 3.0), p(12.0, 3.0)]));
        // Exactly 1 grid step long is not shorter than it.
        let (a, b) = (p(11.0, 0.0), p(11.0, SAME));
        assert_eq!(length(a, b), SAME);
        let g = Guides { line: guides().line, cuts: vec![[a, b]] };
        let part = [p(10.0, 0.0), p(12.0, 0.0)];
        assert_eq!(g.halves(&part, a, meter).unwrap(), (vec![p(10.0, 0.0), p(11.0, 0.0)], vec![p(11.0, 0.0), p(12.0, 0.0)]));
        // At a point cut, a part starting exactly 0.1 CSS pixels from the place does not start there.
        let g = Guides { line: guides().line, cuts: vec![[p(0.0, 0.0), p(0.0, 0.0)]] };
        let part = [p(START_NEAR, 0.0), p(5.0, 0.0)];
        assert_eq!(length(part[0], p(0.0, 0.0)), START_NEAR);
        assert_eq!(g.halves(&part, p(0.0, 0.0), meter).unwrap(), (part.to_vec(), vec![p(5.0, 0.0)]));
    }
}
