//! The area a stroke paints, as rings for a fill (`REQ-AUTO-004`).
//!
//! A stroke `w` wide paints, along each chord of its flattened path, the band `w/2` to each side of it.
//! Where 2 chords meet, the bands overlap inside the turn and leave a notch outside it. Where 2 of the
//! path's segments meet, the stroke's join fills the notch: a triangle for a bevel, the triangle and the
//! point beyond it for a miter no longer than its limit allows (`limit` times `w/2` from the corner, SVG's
//! `stroke-miterlimit`), and the slice of a disc `w/2` across for a round join. Inside a curve the stroke
//! bends smoothly, as a round join bends it. An open subpath ends as its cap says: square at the path's end
//! (butt), a square reaching `w/2` past it, or half a disc. A closed subpath has no ends, and its last
//! segment joins its first. A subpath that does not move draws a dot, a disc or a square `w` across for a
//! round or a square cap, as SVG draws it, and nothing for a butt cap or when it is only a move.
//!
//! The path is flattened within the shapes' tolerance ([`SHAPE_TOLERANCE`]), each segment on its own, and
//! each arc is drawn with chords that stray no further from it. An end or a corner is square to the chord
//! that ends there, and half the stroke's width turns the angle that chord makes with its curve into a shift
//! of the corner, so a curve is halved further where it ends the subpath or meets a corner, until that shift
//! is within the tolerance too ([`stroke::flat_segments`]). So the area is the stroke's within the
//! tolerance.
//!
//! **Rings instead of pieces.** The area is the union of those bands and pieces of joins and caps, but the
//! fill is not given them as pieces: hundreds of pieces overlapping each other cost the fill's area code
//! many times the work of their outline (`REQ-FILL-004`). An open subpath is one ring instead: its right
//! side forward, the cap at its end, its left side back and the cap at its start. A closed subpath is its 2
//! sides, each a ring. At each turn the outer side goes round the join, and the inner side in to the
//! path's point and out again. The rings are then the pieces' outlines, each wound counter-clockwise in
//! y-up axes, added together: an edge 2 pieces share runs once each way and cancels. So the rings wind
//! round each point as many times as pieces cover it, and the nonzero rule fills exactly their union,
//! however tightly the line turns or often it crosses itself.
//!
//! **Long lines in lengths.** The inner side's way in to each point of a curve and out again still costs
//! the area code work that grows with the square of the curve's points. So a line of more than
//! [`RING_CHORDS`] chords is cut into lengths of that many, each a ring of its own, square across the
//! line where it is cut, and made into its area alone. Those areas' outlines, joined, are the line's.
//!
//! Only additions, multiplications, square roots and `stitchcraft_core::math` are used, so every platform
//! gets the same points (`docs/src/design/determinism.md`).

use stitchcraft_core::math;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::design::{Cap, FillRule, Join, Path, Subpath};
use crate::normalize::region::{self, Polygon};
use crate::normalize::stroke::{self, FlatSegment, SHAPE_TOLERANCE};

/// The most chords one arc is drawn with: far more than any tolerance needs for a stroke a machine sews.
const MAX_CHORDS: u32 = 256;

/// The most chords of a line one ring follows; a longer line is cut into lengths this long.
const RING_CHORDS: usize = 32;

/// The 2 sides of a stroke, by their index in [`Walker::sides`].
const RIGHT: usize = 0;
const LEFT: usize = 1;

/// A direction, of unit length.
type Direction = (f64, f64);

/// How a length of a line ends, at a point of the path and going in a direction: with the stroke's cap where
/// the subpath ends there, or square across the line where it is cut.
#[derive(Clone, Copy)]
enum End {
    Cap(Point, Direction),
    Cut,
}

/// The area the stroke along `path`, `width` millimetres wide, paints with `join` at its corners and `cap`
/// at its open ends: rings that bound it under the nonzero rule, each closed, its first point repeated at
/// its end. Each segment, each halving of a curve, each point of a ring and the work of joining a long
/// line's lengths ([`region::area`]) cost `meter`.
pub(crate) fn stroke_area(path: &Path, width: f64, join: Join, cap: Cap, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let half = width / 2.0;
    // How far an end chord may leave its curve: at an angle whose sine shifts the corner there by the
    // tolerance at most.
    let ends = (half > SHAPE_TOLERANCE).then(|| SHAPE_TOLERANCE / half);
    let mut rings = Vec::new();
    for subpath in &path.subpaths {
        let segments = stroke::flat_segments(subpath, SHAPE_TOLERANCE, ends, meter)?;
        let lengths = {
            let mut walker = Walker { half, cap, sides: [Vec::new(), Vec::new()], start: End::Cut, lengths: Vec::new(), meter: &mut *meter };
            if walker.walk(subpath, &segments, join)? { walker.lengths } else { walker.dot(subpath)?.into_iter().map(|ring| vec![ring]).collect() }
        };
        if lengths.len() > 1 {
            for length in lengths {
                rings.extend(joined(length, meter)?);
            }
        } else {
            rings.extend(lengths.into_iter().flatten());
        }
    }
    Ok(rings)
}

/// The area the rings of `length`, a length of a longer line, bound: its parts' outlines and holes, wound
/// so that the nonzero rule fills it alongside the other lengths' rings, outlines counter-clockwise in y-up
/// axes. The rings as they are, should the area not be built.
fn joined(length: Vec<Vec<Point>>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let area = region::area(&length, |winding| region::fills(FillRule::NonZero, winding), meter)?;
    Ok(match area.parts {
        Ok(parts) => parts.iter().flat_map(Polygon::rings).map(|ring| ring.iter().rev().copied().collect()).collect(),
        Err(_) => length,
    })
}

/// `ring` with its first point repeated at its end.
fn closed(mut ring: Vec<Point>) -> Vec<Point> {
    if let Some(&first) = ring.first()
        && ring.last() != Some(&first)
    {
        ring.push(first);
    }
    ring
}

/// `p` moved by (`dx`, `dy`); `p` itself if that falls off the plane.
fn at(p: Point, dx: f64, dy: f64) -> Point {
    Point::new(p.x() + dx, p.y() + dy).unwrap_or(p)
}

/// The unit vector from `a` to `b`, `None` when they are the same point.
fn direction(a: Point, b: Point) -> Option<Direction> {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length = math::hypot(dx, dy);
    (length > 0.0).then(|| (dx / length, dy / length))
}

/// The normal to the right of `d` in y-up axes, as long as `d`.
const fn right((dx, dy): Direction) -> Direction {
    (dy, -dx)
}

/// A walk along the chords of a subpath, making the rings of its stroke's sides.
struct Walker<'m> {
    /// Half the stroke's width.
    half: f64,
    /// How the stroke ends where an open subpath ends.
    cap: Cap,
    /// The sides of the length being walked: the right's points, then the left's, each in the direction of
    /// the path.
    sides: [Vec<Point>; 2],
    /// How the length being walked starts.
    start: End,
    /// The rings of each length walked, each closed.
    lengths: Vec<Vec<Vec<Point>>>,
    meter: &'m mut Meter,
}

impl Walker<'_> {
    /// Adds `p` to `side`, unless that side is there already.
    fn push(&mut self, side: usize, p: Point) -> Result<(), Exhausted> {
        let Some(points) = self.sides.get_mut(side) else { return Ok(()) };
        if points.last() != Some(&p) {
            self.meter.charge(1)?;
            points.push(p);
        }
        Ok(())
    }

    /// Both sides at `p`, across the direction `d`.
    fn across(&mut self, p: Point, d: Direction) -> Result<(), Exhausted> {
        let (x, y) = right(d);
        let h = self.half;
        self.push(RIGHT, at(p, x * h, y * h))?;
        self.push(LEFT, at(p, -x * h, -y * h))
    }

    /// Walks the chords of `segments`, the flattened segments of `subpath`, joined by `join`: both sides
    /// along each chord, a round turn between 2 chords of a segment and `join` between 2 segments, and,
    /// round a closed subpath, from the last segment into the first; then the rings of its lengths
    /// ([`RING_CHORDS`]). A segment with no chord, a curve that comes back to its start within the
    /// tolerance, is passed over. Whether any chord has a length.
    fn walk(&mut self, subpath: &Subpath, segments: &[FlatSegment], join: Join) -> Result<bool, Exhausted> {
        let (mut here, mut heading, mut first, mut chords) = (subpath.start, None, None, 0_usize);
        for segment in segments {
            // The first chord of a segment turns from the last of the one before with the stroke's join.
            let mut bend = join;
            for &next in &segment.points {
                let Some(d) = direction(here, next) else { continue };
                match heading {
                    None => {
                        self.across(here, d)?;
                        first = Some(d);
                        self.start = if subpath.closed { End::Cut } else { End::Cap(here, d) };
                    }
                    Some(from) => {
                        self.turn(here, from, d, bend)?;
                        if chords >= RING_CHORDS {
                            // The line goes on in a length of its own, from across it here.
                            self.finish(End::Cut)?;
                            self.across(here, d)?;
                            chords = 0;
                        }
                    }
                }
                self.across(next, d)?;
                (here, heading, bend, chords) = (next, Some(d), Join::Round, chords + 1);
            }
        }
        let (Some(first), Some(last)) = (first, heading) else { return Ok(false) };
        if !subpath.closed {
            self.finish(End::Cap(here, last))?;
            return Ok(true);
        }
        // The corner where it closes: its last segment into its first.
        self.turn(here, last, first, join)?;
        if self.lengths.is_empty() {
            // A closed subpath in one length is its 2 sides, each a ring.
            let [right, mut left] = std::mem::take(&mut self.sides);
            left.reverse();
            self.lengths.push(vec![closed(right), closed(left)]);
        } else {
            self.finish(End::Cut)?;
        }
        Ok(true)
    }

    /// The turn at `p` from direction `from` to `to`, joined with `join`: the outer side goes round the join
    /// to the next chord, and the inner side in to `p` and out to it.
    fn turn(&mut self, p: Point, from: Direction, to: Direction, join: Join) -> Result<(), Exhausted> {
        let cross = from.0 * to.1 - from.1 * to.0;
        let dot = from.0 * to.0 + from.1 * to.1;
        if cross == 0.0 && dot > 0.0 {
            return Ok(());
        }
        // The side the turn opens on, away from it; a turn straight back opens on both, where only a round
        // join paints more than the bands, and it goes round the left.
        let outside = if cross > 0.0 { -1.0 } else { 1.0 };
        let (outer, inner) = if cross > 0.0 { (RIGHT, LEFT) } else { (LEFT, RIGHT) };
        let normal = |(dx, dy): Direction| (-dy * outside, dx * outside);
        let (n1, n2) = (normal(from), normal(to));
        let h = self.half;
        match join {
            Join::Bevel => {}
            Join::Round => {
                // From the first side to the second, the short way round, or forward when it turns straight
                // back.
                let turn =
                    if cross == 0.0 { std::f64::consts::PI * -outside } else { math::atan2(n1.0 * n2.1 - n1.1 * n2.0, n1.0 * n2.0 + n1.1 * n2.1) };
                self.arc(outer, p, n1, turn)?;
            }
            Join::Miter { limit } => {
                // The point where the 2 sides meet lies 1 / cos(turn / 2) half widths from the corner.
                let cos_half = math::hypot(n1.0 + n2.0, n1.1 + n2.1) / 2.0;
                if cos_half > 0.0 && 1.0 / cos_half <= limit {
                    let reach = h / (cos_half * cos_half) / 2.0;
                    self.push(outer, at(p, (n1.0 + n2.0) * reach, (n1.1 + n2.1) * reach))?;
                }
            }
        }
        self.push(outer, at(p, n2.0 * h, n2.1 * h))?;
        self.push(inner, p)?;
        self.push(inner, at(p, -n2.0 * h, -n2.1 * h))
    }

    /// Ends the length being walked with `end`: its right side, `end`, its left side back and its start
    /// make one ring.
    fn finish(&mut self, end: End) -> Result<(), Exhausted> {
        // The end, from the right side round to the left.
        self.cap(end, false)?;
        let [mut ring, left] = std::mem::take(&mut self.sides);
        ring.extend(left.into_iter().rev());
        self.sides = [ring, Vec::new()];
        // The start, from the left side round to the right.
        let start = std::mem::replace(&mut self.start, End::Cut);
        self.cap(start, true)?;
        let [ring, _] = std::mem::take(&mut self.sides);
        self.lengths.push(vec![closed(ring)]);
        Ok(())
    }

    /// Adds to the right side the points the cap of `end` draws: round past the end of a length, from its
    /// right side to its left, or round behind its `start`, from its left side to its right. None for a cut
    /// or a butt cap.
    fn cap(&mut self, end: End, start: bool) -> Result<(), Exhausted> {
        let End::Cap(p, d) = end else { return Ok(()) };
        // The way out of the line there, and the side to its right, where the cap begins.
        let (ox, oy) = if start { (-d.0, -d.1) } else { d };
        let (rx, ry) = right((ox, oy));
        let h = self.half;
        match self.cap {
            Cap::Butt => Ok(()),
            Cap::Square => {
                self.push(RIGHT, at(p, (rx + ox) * h, (ry + oy) * h))?;
                self.push(RIGHT, at(p, (ox - rx) * h, (oy - ry) * h))
            }
            Cap::Round => self.arc(RIGHT, p, (rx, ry), std::f64::consts::PI),
        }
    }

    /// The points of the arc of the disc half the stroke's width across, centred on `centre`, from the unit
    /// vector `from` turning by `turn` radians, its ends left out, added to `side`: chords that stray at
    /// most [`SHAPE_TOLERANCE`] from the arc.
    fn arc(&mut self, side: usize, centre: Point, from: Direction, turn: f64) -> Result<(), Exhausted> {
        let h = self.half;
        if !(h > 0.0 && turn.is_finite()) {
            return Ok(());
        }
        let start = math::atan2(from.1, from.0);
        let chords = chords(h, turn);
        for k in 1..chords {
            let (sin, cos) = math::sin_cos(start + turn * f64::from(k) / f64::from(chords));
            self.push(side, at(centre, cos * h, sin * h))?;
        }
        Ok(())
    }

    /// The ring of the dot that `subpath`, which does not move, draws with the stroke's cap: a disc or a
    /// square the stroke's width across, centred on its start, the square along the axes, as SVG draws it;
    /// `None` for a butt cap, and for a subpath that is only a move, which SVG never draws.
    fn dot(&mut self, subpath: &Subpath) -> Result<Option<Vec<Point>>, Exhausted> {
        let (centre, h) = (subpath.start, self.half);
        if subpath.segments.is_empty() && !subpath.closed {
            return Ok(None);
        }
        match self.cap {
            Cap::Butt => return Ok(None),
            Cap::Square => {
                for (x, y) in [(h, -h), (h, h), (-h, h), (-h, -h)] {
                    self.push(RIGHT, at(centre, x, y))?;
                }
            }
            Cap::Round => {
                self.push(RIGHT, at(centre, h, 0.0))?;
                self.arc(RIGHT, centre, (1.0, 0.0), std::f64::consts::TAU)?;
            }
        }
        let [ring, _] = std::mem::take(&mut self.sides);
        Ok(Some(closed(ring)))
    }
}

/// How many chords draw an arc of the disc `half` across turning by `turn` radians: from 1 to
/// [`MAX_CHORDS`], each straying at most [`SHAPE_TOLERANCE`] from the arc.
// The count is a whole number from 1 to the most, held there before it is cast.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn chords(half: f64, turn: f64) -> u32 {
    // A chord of angle a strays h (1 - cos(a / 2)) from its arc.
    let step = if SHAPE_TOLERANCE < half { 2.0 * math::acos(1.0 - SHAPE_TOLERANCE / half) } else { std::f64::consts::FRAC_PI_2 };
    (turn.abs() / step).ceil().clamp(1.0, f64::from(MAX_CHORDS)) as u32
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::units::MM_PER_SVG_PX;

    use super::*;
    use crate::design::{FillRule, Segment};
    use crate::normalize::region;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn line(points: &[(f64, f64)], closed: bool) -> Path {
        let (&(x, y), rest) = points.split_first().unwrap();
        Path { subpaths: vec![Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed }] }
    }

    /// The area `rings` bound under the nonzero rule.
    fn area(rings: &[Vec<Point>]) -> f64 {
        let path = Path {
            subpaths: rings
                .iter()
                .filter_map(|ring| {
                    let (&start, rest) = ring.split_first()?;
                    Some(Subpath { start, segments: rest.iter().map(|&q| Segment::Line(q)).collect(), closed: true })
                })
                .collect(),
        };
        region::build(&path, FillRule::NonZero, &mut Budget::DEFAULT.meter()).unwrap().region.area()
    }

    fn painted(path: &Path, width: f64, join: Join, cap: Cap) -> f64 {
        area(&stroke_area(path, width, join, cap, &mut Budget::DEFAULT.meter()).unwrap())
    }

    #[test]
    fn an_open_line_is_one_ring_and_a_closed_one_two() {
        let open = stroke_area(&line(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)], false), 2.0, Join::Bevel, Cap::Butt, &mut Budget::DEFAULT.meter());
        // Right side forward, round the end, left side back: the inner side goes in to the corner and out.
        let ring = vec![p(0.0, -1.0), p(10.0, -1.0), p(11.0, 0.0), p(11.0, 10.0), p(9.0, 10.0), p(9.0, 0.0), p(10.0, 0.0), p(10.0, 1.0)];
        assert_eq!(open.unwrap(), [closed(ring.into_iter().chain([p(0.0, 1.0)]).collect())]);
        let square = stroke_area(
            &line(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true),
            2.0,
            Join::Bevel,
            Cap::Round,
            &mut Budget::DEFAULT.meter(),
        );
        assert_eq!(square.unwrap().len(), 2);
    }

    #[test]
    fn a_closed_square_line_paints_a_frame_with_its_corners_joined() {
        // A 20 mm square drawn as a line 4 mm wide: the frame from 18 mm inside to 22 mm outside, its outer
        // corners as the join makes them.
        let square = line(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0), (0.0, 0.0)], true);
        let frame = 24.0 * 24.0 - 16.0 * 16.0;
        assert!((painted(&square, 4.0, Join::Miter { limit: 4.0 }, Cap::Round) - frame).abs() < 1e-6);
        assert!((painted(&square, 4.0, Join::Bevel, Cap::Butt) - (frame - 4.0 * 2.0)).abs() < 1e-6);
        // A round corner leaves out the 2 by 2 mm square's corner a quarter disc does not cover, and the
        // quarter circles' chords leave out at most the tolerance along their length.
        let round = painted(&square, 4.0, Join::Round, Cap::Butt);
        let (corners, arcs) = (4.0 * (4.0 - std::f64::consts::PI), 4.0 * math::to_radians(90.0) * 2.0);
        assert!(round <= frame - corners + 1e-6 && round >= frame - corners - SHAPE_TOLERANCE * arcs, "{round}");
        // Drawn the other way round, it paints the same.
        let backwards = line(&[(0.0, 0.0), (0.0, 20.0), (20.0, 20.0), (20.0, 0.0), (0.0, 0.0)], true);
        assert!((painted(&backwards, 4.0, Join::Miter { limit: 4.0 }, Cap::Round) - frame).abs() < 1e-6);
    }

    #[test]
    fn a_dot_is_drawn_by_a_round_or_square_cap_only() {
        // A line that does not move draws a dot; a subpath that is only a move draws nothing.
        let dot = Path { subpaths: vec![Subpath { start: p(5.0, 5.0), segments: vec![Segment::Line(p(5.0, 5.0))], closed: false }] };
        assert_eq!(painted(&dot, 2.0, Join::Round, Cap::Butt), 0.0);
        assert!((painted(&dot, 2.0, Join::Round, Cap::Square) - 4.0).abs() < 1e-9);
        let disc = painted(&dot, 2.0, Join::Round, Cap::Round);
        assert!((std::f64::consts::PI - SHAPE_TOLERANCE * std::f64::consts::TAU..=std::f64::consts::PI).contains(&disc), "{disc}");
        let moved = Path { subpaths: vec![Subpath { start: p(5.0, 5.0), segments: vec![], closed: false }] };
        assert_eq!(stroke_area(&moved, 2.0, Join::Round, Cap::Round, &mut Budget::DEFAULT.meter()).unwrap(), Vec::<Vec<Point>>::new());
    }

    #[test]
    fn a_line_that_turns_straight_back_is_rounded_by_a_round_join_only() {
        let back = line(&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0)], false);
        assert!((painted(&back, 2.0, Join::Miter { limit: 4.0 }, Cap::Butt) - 20.0).abs() < 1e-9);
        assert!((painted(&back, 2.0, Join::Bevel, Cap::Butt) - 20.0).abs() < 1e-9);
        let round = painted(&back, 2.0, Join::Round, Cap::Butt);
        let half = math::to_radians(90.0);
        assert!(round <= 20.0 + half + 1e-9 && round >= 20.0 + half - SHAPE_TOLERANCE * std::f64::consts::PI, "{round}");
    }

    #[test]
    fn a_curve_s_butt_end_is_square_to_the_curve() {
        // A quarter circle of radius 10 mm from (10, 0) to (0, 10), drawn 10 mm wide with butt caps: its
        // ends lie along the radii, from (5, 0) to (15, 0) and from (0, 5) to (0, 15).
        let k = 10.0 * 0.552_284_749_830_793_4;
        let arc = Path {
            subpaths: vec![Subpath { start: p(10.0, 0.0), segments: vec![Segment::Cubic(p(10.0, k), p(k, 10.0), p(0.0, 10.0))], closed: false }],
        };
        let rings = stroke_area(&arc, 10.0, Join::Miter { limit: 4.0 }, Cap::Butt, &mut Budget::DEFAULT.meter()).unwrap();
        let points: Vec<Point> = rings.iter().flatten().copied().collect();
        for corner in [p(5.0, 0.0), p(15.0, 0.0), p(0.0, 5.0), p(0.0, 15.0)] {
            let nearest = points.iter().map(|q| q.distance(corner)).fold(f64::MAX, f64::min);
            assert!(nearest <= SHAPE_TOLERANCE, "{corner:?}: {nearest} mm");
        }
        // And it paints the quarter of the ring from 5 to 15 mm, within the tolerance along its edges.
        let quarter = std::f64::consts::FRAC_PI_4 * (15.0 * 15.0 - 5.0 * 5.0);
        let found = area(&rings);
        assert!((found - quarter).abs() <= SHAPE_TOLERANCE * 2.0 * 10.0 * std::f64::consts::PI, "{found}, not {quarter}");
    }

    #[test]
    fn the_rings_paint_the_union_of_the_pieces() {
        // Lines that turn tightly, cross themselves and come back, against the pieces they add up to, each
        // filled on its own: their union. Every join and cap, and widths from a CSS pixel to 30 mm.
        let lines = [
            line(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)], false),
            line(&[(0.0, 0.0), (20.0, 0.0), (5.0, 10.0), (5.0, -10.0), (12.0, 3.0)], false),
            line(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0), (0.5, 0.2)], true),
            line(&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0), (5.0, 0.0), (5.0, 8.0)], false),
            line(&[(0.0, 0.0), (30.0, 0.0), (30.0, 0.2), (0.0, 0.3), (15.0, 15.0)], true),
        ];
        let joins = [Join::Bevel, Join::Round, Join::Miter { limit: 4.0 }, Join::Miter { limit: 1.2 }];
        for (path, width) in lines.iter().zip([2.0, 5.0, MM_PER_SVG_PX, 7.0, 30.0]) {
            for (join, cap) in joins.iter().zip([Cap::Butt, Cap::Round, Cap::Square, Cap::Round]) {
                assert_union(path, width, *join, cap);
            }
        }
    }

    #[test]
    fn a_long_line_is_cut_into_lengths_that_paint_it_all() {
        // A zigzag of 99 lines, more than 3 lengths' worth, open and closed: each length is a ring, and
        // their areas joined are the line's.
        let points: Vec<(f64, f64)> = (0..100).map(|i| (f64::from(i) * 0.7, if i % 2 == 0 { 0.0 } else { 2.0 })).collect();
        for closed in [false, true] {
            let path = line(&points, closed);
            for (join, cap) in [(Join::Miter { limit: 4.0 }, Cap::Round), (Join::Round, Cap::Square), (Join::Bevel, Cap::Butt)] {
                let rings = stroke_area(&path, 3.0, join, cap, &mut Budget::DEFAULT.meter()).unwrap();
                assert!(rings.len() > 3, "{closed} {join:?}: {} rings", rings.len());
                assert_union(&path, 3.0, join, cap);
            }
        }
    }

    proptest::proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(96))]

        #[test]
        fn any_polyline_s_rings_paint_the_union_of_its_pieces(
            points in proptest::collection::vec((-10i32..=10, -10i32..=10), 2..7),
            closed in proptest::bool::ANY,
            width in 1u32..=150,
            style in 0usize..12,
        ) {
            let points: Vec<(f64, f64)> = points.iter().map(|&(x, y)| (f64::from(x), f64::from(y))).collect();
            let joins = [Join::Bevel, Join::Round, Join::Miter { limit: 4.0 }, Join::Miter { limit: 10.0 }];
            let caps = [Cap::Butt, Cap::Round, Cap::Square];
            assert_union(&line(&points, closed), f64::from(width) / 10.0, joins[style % 4], caps[style / 4]);
        }
    }

    /// Checks that the rings of the stroke along `path` fill what the pieces of [`pieces`] fill together.
    fn assert_union(path: &Path, width: f64, join: Join, cap: Cap) {
        let rings = painted(path, width, join, cap);
        let union = area(&pieces(path, width, join, cap));
        assert!((rings - union).abs() < 1e-6 * union.max(1.0), "{path:?} {width} {join:?} {cap:?}: {rings} mm², not {union}");
    }

    /// The pieces the stroke along `path`, of lines only, paints, each its own ring wound counter-clockwise,
    /// made apart from the rings: a band along each line, a piece of join at each turn (its corner and what
    /// the join adds outside it) and a cap at each open end, with the dots their caps draw.
    fn pieces(path: &Path, width: f64, join: Join, cap: Cap) -> Vec<Vec<Point>> {
        let h = width / 2.0;
        let mut rings = Vec::new();
        for subpath in &path.subpaths {
            let mut points = vec![subpath.start];
            points.extend(subpath.segments.iter().map(|segment| segment.end()));
            if subpath.closed {
                points.push(subpath.start);
            }
            points.dedup();
            let n = points.len();
            if n == 1 {
                let dot = stroke_area(&Path { subpaths: vec![subpath.clone()] }, width, join, cap, &mut Budget::DEFAULT.meter()).unwrap();
                rings.extend(dot);
                continue;
            }
            for w in points.windows(2) {
                let (x, y) = right(direction(w[0], w[1]).unwrap());
                rings.push(vec![at(w[0], x * h, y * h), at(w[1], x * h, y * h), at(w[1], -x * h, -y * h), at(w[0], -x * h, -y * h)]);
            }
            let mut corners: Vec<(Point, Point, Point)> = points.windows(3).map(|w| (w[0], w[1], w[2])).collect();
            if subpath.closed && n > 2 {
                corners.push((points[n - 2], points[0], points[1]));
            }
            for (a, b, c) in corners {
                let (d1, d2) = (direction(a, b).unwrap(), direction(b, c).unwrap());
                let (cross, dot) = (d1.0 * d2.1 - d1.1 * d2.0, d1.0 * d2.0 + d1.1 * d2.1);
                if cross == 0.0 && dot > 0.0 {
                    continue;
                }
                let outside = if cross > 0.0 { -1.0 } else { 1.0 };
                let normal = |(dx, dy): Direction| (-dy * outside, dx * outside);
                let (n1, n2) = (normal(d1), normal(d2));
                let mut piece = vec![b, at(b, n1.0 * h, n1.1 * h)];
                match join {
                    Join::Bevel => {}
                    Join::Round => {
                        let turn = if cross == 0.0 { std::f64::consts::PI * -outside } else { math::atan2(cross, dot) };
                        let (start, count) = (math::atan2(n1.1, n1.0), chords(h, turn));
                        for k in 1..count {
                            let (sin, cos) = math::sin_cos(start + turn * f64::from(k) / f64::from(count));
                            piece.push(at(b, cos * h, sin * h));
                        }
                    }
                    Join::Miter { limit } => {
                        let cos_half = math::hypot(n1.0 + n2.0, n1.1 + n2.1) / 2.0;
                        if cos_half > 0.0 && 1.0 / cos_half <= limit {
                            let reach = h / (cos_half * cos_half) / 2.0;
                            piece.push(at(b, (n1.0 + n2.0) * reach, (n1.1 + n2.1) * reach));
                        }
                    }
                }
                piece.push(at(b, n2.0 * h, n2.1 * h));
                rings.push(piece);
            }
            if !subpath.closed {
                // Each end, and the direction it goes on in past the path.
                let back = direction(points[1], points[0]).unwrap();
                for (end, (dx, dy)) in [(points[0], back), (points[n - 1], direction(points[n - 2], points[n - 1]).unwrap())] {
                    let (x, y) = right((dx, dy));
                    match cap {
                        Cap::Butt => {}
                        Cap::Square => rings.push(vec![
                            at(end, x * h, y * h),
                            at(end, (x + dx) * h, (y + dy) * h),
                            at(end, (dx - x) * h, (dy - y) * h),
                            at(end, -x * h, -y * h),
                        ]),
                        Cap::Round => {
                            let mut half = vec![end];
                            let (start, count) = (math::atan2(y, x), chords(h, std::f64::consts::PI));
                            for k in 0..=count {
                                let (sin, cos) = math::sin_cos(start + std::f64::consts::PI * f64::from(k) / f64::from(count));
                                half.push(at(end, cos * h, sin * h));
                            }
                            rings.push(half);
                        }
                    }
                }
            }
        }
        rings
            .into_iter()
            .map(|mut ring| {
                let doubled: f64 = ring.iter().zip(ring.iter().cycle().skip(1)).map(|(p, q)| p.x() * q.y() - q.x() * p.y()).sum();
                if doubled < 0.0 {
                    ring.reverse();
                }
                closed(ring)
            })
            .collect()
    }
}
