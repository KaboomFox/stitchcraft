//! The area a stroke paints, as rings for a fill (`REQ-AUTO-004`).
//!
//! A stroke `w` wide paints, along each segment of its path, the rectangle `w/2` to each side of it. Where 2
//! segments meet, the rectangles overlap inside the corner and leave a notch outside it, which the stroke's
//! join fills: a triangle for a bevel, the triangle and the point beyond it for a miter no longer than its
//! limit allows (`limit` times `w/2` from the corner, SVG's `stroke-miterlimit`), and the slice of a disc
//! `w/2` across for a round join. An open subpath ends as its cap says: square at the path's end (butt), a
//! square reaching `w/2` past it, or half a disc. A closed subpath turns its last corner too, and has no
//! ends. So the area is the union of those pieces, each a ring: the fill joins them with the nonzero rule,
//! all wound the same way.
//!
//! The path is first flattened within the shapes' tolerance ([`SHAPE_TOLERANCE`]), and an arc is drawn
//! with chords that stray no further from it, so the area is the stroke's within that tolerance. Only
//! additions, multiplications, square roots and `stitchcraft_core::math` are used, so every platform
//! gets the same points (`docs/src/design/determinism.md`).

use stitchcraft_core::math;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::design::{Cap, Join, Path, Segment, Subpath};
use crate::normalize::stroke::{self, SHAPE_TOLERANCE};

/// The most chords one arc is drawn with: far more than any tolerance needs for a stroke a machine sews.
const MAX_CHORDS: u32 = 256;

/// The area the stroke along `path`, `width` millimetres wide, paints with `join` at its corners and `cap`
/// at its open ends: rings that bound it under the nonzero rule, each wound counter-clockwise in y-up axes.
/// Each point of the path flattened, and each point of a ring, costs one unit of `meter`.
pub(crate) fn stroke_area(path: &Path, width: f64, join: Join, cap: Cap, meter: &mut Meter) -> Result<Path, Exhausted> {
    let half = width / 2.0;
    let flat = stroke::flatten(path, SHAPE_TOLERANCE, meter)?;
    let mut rings = Rings { rings: Vec::new(), half, meter };
    for (subpath, piece) in path.subpaths.iter().zip(&flat.pieces) {
        let points = piece.points.as_slice();
        match points {
            [] => {}
            // A subpath that does not move: a dot, as SVG draws it, round or square, and nothing for a butt cap.
            [dot] => match cap {
                Cap::Butt => {}
                Cap::Round => rings.arc(*dot, (1.0, 0.0), std::f64::consts::TAU)?,
                Cap::Square => rings.push(&[(-half, -half), (half, -half), (half, half), (-half, half)].map(|(x, y)| at(*dot, x, y)))?,
            },
            _ => {
                for pair in points.windows(2) {
                    if let [a, b] = pair {
                        rings.side(*a, *b)?;
                    }
                }
                for corner in points.windows(3) {
                    if let [a, b, c] = corner {
                        rings.join(*a, *b, *c, join)?;
                    }
                }
                if subpath.closed {
                    // The corner where it closes: its last segment into its first. A closed piece ends where
                    // it starts, so its last point repeats its first.
                    if let (Some(&[a, b]), Some(&[_, c])) = (points.get(points.len().saturating_sub(2)..), points.get(..2)) {
                        rings.join(a, b, c, join)?;
                    }
                } else if let (Some(&[a, b]), Some(&[y, z])) = (points.get(..2), points.get(points.len().saturating_sub(2)..)) {
                    rings.cap(b, a, cap)?;
                    rings.cap(y, z, cap)?;
                }
            }
        }
    }
    Ok(Path { subpaths: rings.rings })
}

/// `p` moved by (`dx`, `dy`); `p` itself if that falls off the plane.
fn at(p: Point, dx: f64, dy: f64) -> Point {
    Point::new(p.x() + dx, p.y() + dy).unwrap_or(p)
}

/// The unit vector from `a` to `b`, `None` when they are the same point.
fn direction(a: Point, b: Point) -> Option<(f64, f64)> {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length = math::hypot(dx, dy);
    (length > 0.0).then(|| (dx / length, dy / length))
}

/// The rings of a stroke's area as they are made, each closed and wound the same way.
struct Rings<'m> {
    rings: Vec<Subpath>,
    /// Half the stroke's width.
    half: f64,
    meter: &'m mut Meter,
}

impl Rings<'_> {
    /// Adds the ring through `points`, wound counter-clockwise in y-up axes; nothing for one with no area.
    fn push(&mut self, points: &[Point]) -> Result<(), Exhausted> {
        self.meter.charge(points.len() as u64)?;
        let doubled: f64 = points.iter().zip(points.iter().cycle().skip(1)).map(|(p, q)| p.x() * q.y() - q.x() * p.y()).sum();
        let mut ring = points.to_vec();
        if doubled < 0.0 {
            ring.reverse();
        }
        if let Some((&start, rest)) = ring.split_first()
            && doubled != 0.0
        {
            self.rings.push(Subpath { start, segments: rest.iter().map(|&q| Segment::Line(q)).collect(), closed: true });
        }
        Ok(())
    }

    /// The rectangle the stroke paints along the segment from `a` to `b`.
    fn side(&mut self, a: Point, b: Point) -> Result<(), Exhausted> {
        let Some((dx, dy)) = direction(a, b) else { return Ok(()) };
        let (nx, ny) = (-dy * self.half, dx * self.half);
        self.push(&[at(a, nx, ny), at(b, nx, ny), at(b, -nx, -ny), at(a, -nx, -ny)])
    }

    /// What `join` fills outside the corner at `b`, where the segment from `a` meets the one to `c`.
    fn join(&mut self, a: Point, b: Point, c: Point, join: Join) -> Result<(), Exhausted> {
        let (Some(d1), Some(d2)) = (direction(a, b), direction(b, c)) else { return Ok(()) };
        let cross = d1.0 * d2.1 - d1.1 * d2.0;
        let dot = d1.0 * d2.0 + d1.1 * d2.1;
        if cross == 0.0 && dot > 0.0 {
            return Ok(());
        }
        // The side the corner opens on, away from the turn; a turn straight back opens on both, where only
        // a round join paints more than the 2 rectangles.
        let outside = if cross > 0.0 { -1.0 } else { 1.0 };
        let normal = |(dx, dy): (f64, f64)| (-dy * outside, dx * outside);
        let (n1, n2) = (normal(d1), normal(d2));
        let h = self.half;
        let (p1, p2) = (at(b, n1.0 * h, n1.1 * h), at(b, n2.0 * h, n2.1 * h));
        match join {
            Join::Bevel => self.push(&[b, p1, p2]),
            Join::Round => {
                // From the first side to the second, the short way round, or forward when it turns straight back.
                let turn =
                    if cross == 0.0 { std::f64::consts::PI * -outside } else { math::atan2(n1.0 * n2.1 - n1.1 * n2.0, n1.0 * n2.0 + n1.1 * n2.1) };
                self.arc(b, n1, turn)
            }
            Join::Miter { limit } => {
                // The point where the 2 sides meet lies 1 / cos(turn / 2) half widths from the corner.
                let cos_half = math::hypot(n1.0 + n2.0, n1.1 + n2.1) / 2.0;
                if cos_half > 0.0 && 1.0 / cos_half <= limit {
                    let reach = h / (cos_half * cos_half) / 2.0;
                    let tip = at(b, (n1.0 + n2.0) * reach, (n1.1 + n2.1) * reach);
                    self.push(&[b, p1, tip, p2])
                } else {
                    self.push(&[b, p1, p2])
                }
            }
        }
    }

    /// The cap at `end`, where the segment from `from` ends.
    fn cap(&mut self, from: Point, end: Point, cap: Cap) -> Result<(), Exhausted> {
        let Some((dx, dy)) = direction(from, end) else { return Ok(()) };
        let h = self.half;
        let (nx, ny) = (-dy * h, dx * h);
        match cap {
            Cap::Butt => Ok(()),
            Cap::Square => {
                let (fx, fy) = (dx * h, dy * h);
                self.push(&[at(end, nx, ny), at(end, nx + fx, ny + fy), at(end, fx - nx, fy - ny), at(end, -nx, -ny)])
            }
            // Half a disc, from one side round the end to the other.
            Cap::Round => self.arc(end, (-dy, dx), -std::f64::consts::PI),
        }
    }

    /// The slice of the disc half the stroke's width across, centred on `centre`, from the unit vector `from`
    /// turning by `turn` radians, as a ring through the centre: chords that stray at most
    /// [`SHAPE_TOLERANCE`] from the arc.
    fn arc(&mut self, centre: Point, from: (f64, f64), turn: f64) -> Result<(), Exhausted> {
        let h = self.half;
        if !(h > 0.0 && turn.is_finite()) {
            return Ok(());
        }
        // A chord of angle a strays h (1 - cos(a / 2)) from its arc.
        let step = if SHAPE_TOLERANCE < h { 2.0 * math::acos(1.0 - SHAPE_TOLERANCE / h) } else { std::f64::consts::FRAC_PI_2 };
        // A whole number from 1 to the most, held there before it is cast.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let chords = (turn.abs() / step).ceil().clamp(1.0, f64::from(MAX_CHORDS)) as u32;
        let start = math::atan2(from.1, from.0);
        let whole = (turn.abs() - std::f64::consts::TAU).abs() < 1e-12;
        let mut ring = if whole { Vec::new() } else { vec![centre] };
        let last = if whole { chords - 1 } else { chords };
        for k in 0..=last {
            let (sin, cos) = math::sin_cos(start + turn * f64::from(k) / f64::from(chords));
            ring.push(at(centre, cos * h, sin * h));
        }
        self.push(&ring)
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::design::FillRule;
    use crate::normalize::region;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn line(points: &[(f64, f64)], closed: bool) -> Path {
        let (&(x, y), rest) = points.split_first().unwrap();
        Path { subpaths: vec![Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed }] }
    }

    /// The area the rings bound under the nonzero rule.
    fn area(rings: &Path) -> f64 {
        region::build(rings, FillRule::NonZero, &mut Budget::DEFAULT.meter()).unwrap().region.area()
    }

    fn painted(path: &Path, width: f64, join: Join, cap: Cap) -> f64 {
        area(&stroke_area(path, width, join, cap, &mut Budget::DEFAULT.meter()).unwrap())
    }

    #[test]
    fn every_ring_winds_the_same_way() {
        let rings = stroke_area(
            &line(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 5.0)], false),
            4.0,
            Join::Round,
            Cap::Round,
            &mut Budget::DEFAULT.meter(),
        )
        .unwrap();
        assert!(!rings.subpaths.is_empty());
        for ring in &rings.subpaths {
            let points: Vec<Point> = std::iter::once(ring.start).chain(ring.segments.iter().map(|s| s.end())).collect();
            let doubled: f64 = points.iter().zip(points.iter().cycle().skip(1)).map(|(p, q)| p.x() * q.y() - q.x() * p.y()).sum();
            assert!(doubled > 0.0 && ring.closed, "{ring:?}");
        }
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
    }

    #[test]
    fn a_dot_is_drawn_by_a_round_or_square_cap_only() {
        let dot = Path { subpaths: vec![Subpath { start: p(5.0, 5.0), segments: vec![], closed: false }] };
        assert_eq!(painted(&dot, 2.0, Join::Round, Cap::Butt), 0.0);
        assert!((painted(&dot, 2.0, Join::Round, Cap::Square) - 4.0).abs() < 1e-9);
        let disc = painted(&dot, 2.0, Join::Round, Cap::Round);
        assert!((std::f64::consts::PI - SHAPE_TOLERANCE * std::f64::consts::TAU..=std::f64::consts::PI).contains(&disc), "{disc}");
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
}
