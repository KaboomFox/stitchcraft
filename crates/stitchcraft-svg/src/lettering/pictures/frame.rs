//! The geometry text in pictures measures with (`REQ-AUTO-063`, `REQ-AUTO-065`): whether a point lies in a
//! marker's area, and where a point lies along and across the line a text follows.
//!
//! **Why a line for every marker.** Letters stand on a line, level or bent. Measured along and across that
//! line, their height and their width are the same 2 numbers whichever way the line runs, so a level word
//! and a word on an arc are sized and placed by one rule. A marker that is an area gives a level line, and a
//! marker that is a line gives itself.
//!
//! Points are pairs of millimetres, the y axis down, as a design's are. Across a line, a distance is counted
//! towards the letters' tops: to the left of the way the line runs, which is up on a line that runs to the
//! right.

use stitchcraft_core::math;

/// A point, in millimetres.
pub(super) type At = (f64, f64);

/// Whether `p` lies inside the area the closed polylines `rings` enclose, by the even-odd rule: a ray from
/// it to the right crosses their edges an odd number of times.
pub(super) fn inside(rings: &[Vec<At>], (x, y): At) -> bool {
    let mut odd = false;
    for ring in rings {
        let Some(&last) = ring.last() else { continue };
        let mut from = last;
        for &to in ring {
            let (a, b) = (from, to);
            from = to;
            // The edge crosses the ray's height, and does so to the right of the point.
            if (a.1 > y) != (b.1 > y) && x < a.0 + (y - a.1) / (b.1 - a.1) * (b.0 - a.0) {
                odd = !odd;
            }
        }
    }
    odd
}

/// The area the closed polyline `ring` encloses, in square millimetres, whichever way it runs.
pub(super) fn area(ring: &[At]) -> f64 {
    let Some(&last) = ring.last() else { return 0.0 };
    let mut from = last;
    let mut twice = 0.0;
    for &to in ring {
        twice += from.0 * to.1 - to.0 * from.1;
        from = to;
    }
    (twice / 2.0).abs()
}

/// The length of the polyline `points`.
pub(super) fn length(points: &[At]) -> f64 {
    points.windows(2).map(|pair| if let [a, b] = pair { math::hypot(b.0 - a.0, b.1 - a.1) } else { 0.0 }).sum()
}

/// The line a text follows: a polyline with how far along it each point lies.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Line {
    /// Its points, no 2 in a row the same: 2 or more.
    points: Vec<At>,
    /// How far along the line each point lies, from 0.
    far: Vec<f64>,
}

impl Line {
    /// The line through `points`, or `None` when they give it no length.
    pub(super) fn of(points: &[At]) -> Option<Line> {
        let mut kept: Vec<At> = Vec::with_capacity(points.len());
        let mut far = Vec::with_capacity(points.len());
        let mut length = 0.0;
        for &point in points {
            if let Some(&before) = kept.last() {
                let step = math::hypot(point.0 - before.0, point.1 - before.1);
                if step <= 0.0 {
                    continue;
                }
                length += step;
            }
            kept.push(point);
            far.push(length);
        }
        (kept.len() >= 2).then_some(Line { points: kept, far })
    }

    /// Its length.
    pub(super) fn length(&self) -> f64 {
        self.far.last().copied().unwrap_or_default()
    }

    /// How many segments it has.
    pub(super) fn segments(&self) -> usize {
        self.points.len().saturating_sub(1)
    }

    /// Segment `index`: its start, how far along the line that lies, its length and the unit vector it runs
    /// in.
    fn segment(&self, index: usize) -> (At, f64, f64, At) {
        let at = |index: usize| self.points.get(index).copied().unwrap_or_default();
        let (a, b) = (at(index), at(index + 1));
        let from = self.far.get(index).copied().unwrap_or_default();
        let span = self.far.get(index + 1).copied().unwrap_or_default() - from;
        (a, from, span, ((b.0 - a.0) / span, (b.1 - a.1) / span))
    }

    /// Where `p` lies by the line: how far along it the nearest point of the line lies, and how far `p` is
    /// from that point, counted towards the letters' tops. The line's first and last segments go on past its
    /// ends, so a point before its start lies a negative distance along it.
    pub(super) fn locate(&self, p: At) -> (f64, f64) {
        let last = self.segments().saturating_sub(1);
        let mut best = (f64::INFINITY, 0.0, 0.0);
        for index in 0..=last {
            let (a, from, span, (ux, uy)) = self.segment(index);
            let (dx, dy) = (p.0 - a.0, p.1 - a.1);
            let mut along = dx * ux + dy * uy;
            if index > 0 {
                along = along.max(0.0);
            }
            if index < last {
                along = along.min(span);
            }
            let (qx, qy) = (a.0 + along * ux, a.1 + along * uy);
            let apart = math::hypot(p.0 - qx, p.1 - qy);
            if apart < best.0 {
                // To the left of the way the segment runs, as a picture shows it, the distance is positive.
                best = (apart, from + along, apart.copysign(dx * uy - dy * ux));
            }
        }
        (best.1, best.2)
    }

    /// The segment that `far` along the line lies in, or past which it lies.
    fn holding(&self, far: f64) -> usize {
        self.far.partition_point(|at| *at <= far).clamp(1, self.segments().max(1)) - 1
    }

    /// The point `far` along the line and `across` it, towards the letters' tops.
    fn beside_at(&self, far: f64, across: f64) -> At {
        let (a, from, _, (ux, uy)) = self.segment(self.holding(far));
        (a.0 + (far - from) * ux + across * uy, a.1 + (far - from) * uy - across * ux)
    }

    /// The line `across` millimetres beside this one, towards the letters' tops, from `from` along it to
    /// `to`: its ends, and between them each corner of this line pushed out along the line that halves the
    /// corner. Either end may lie past an end of this line.
    pub(super) fn beside(&self, across: f64, from: f64, to: f64) -> Vec<At> {
        let mut points = vec![self.beside_at(from, across)];
        for index in 1..self.segments() {
            let (corner, far, _, (ux, uy)) = self.segment(index);
            if far <= from || far >= to {
                continue;
            }
            let (_, _, _, (vx, vy)) = self.segment(index - 1);
            // The 2 segments' normals, added. Where the line turns straight back they cancel, and the
            // later segment's serves.
            let (nx, ny) = (uy + vy, -ux - vx);
            let size = math::hypot(nx, ny);
            let (nx, ny) = if size > 0.0 { (nx / size, ny / size) } else { (uy, -ux) };
            points.push((corner.0 + across * nx, corner.1 + across * ny));
        }
        points.push(self.beside_at(to, across));
        points
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn near(a: At, b: At) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn req_auto_063_a_point_is_inside_an_area_by_the_even_odd_rule() {
        let square = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let hole = vec![(4.0, 4.0), (6.0, 4.0), (6.0, 6.0), (4.0, 6.0)];
        let rings = [square.clone(), hole];
        assert!(inside(&rings, (1.0, 1.0)) && inside(&rings, (9.0, 5.0)) && inside(&rings, (5.0, 9.5)));
        assert!(!inside(&rings, (5.0, 5.0)), "in the hole");
        for outside in [(-1.0, 5.0), (11.0, 5.0), (5.0, -1.0), (5.0, 11.0)] {
            assert!(!inside(&rings, outside), "{outside:?}");
        }
        // A point level with a corner is counted once: the edge that ends there doesn't cross its height.
        assert!(inside(&[vec![(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)]], (1.0, 5.0)));
        assert!(!inside(&[vec![(0.0, 0.0), (10.0, 5.0), (0.0, 10.0)]], (10.5, 5.0)));
        assert!(!inside(&[Vec::new()], (0.0, 0.0)) && !inside(&[], (0.0, 0.0)));
        assert_eq!(area(&square), 100.0);
        let back: Vec<At> = square.iter().rev().copied().collect();
        assert_eq!(area(&back), 100.0);
        assert_eq!(area(&[(1.0, 1.0), (4.0, 1.0), (1.0, 5.0)]), 6.0);
        assert_eq!(area(&[]), 0.0);
        assert_eq!(length(&[(0.0, 0.0), (3.0, 4.0), (3.0, 10.0)]), 11.0);
        assert_eq!(length(&[(1.0, 1.0)]), 0.0);
    }

    #[test]
    fn req_auto_065_a_point_is_found_along_and_across_the_line() {
        // To the right, down, and to the right again.
        let line = Line::of(&[(0.0, 0.0), (10.0, 0.0), (10.0, 0.0), (10.0, 10.0), (20.0, 10.0)]).unwrap();
        assert_eq!((line.length(), line.segments()), (30.0, 3));
        // Up on the page is towards the tops on a line that runs to the right.
        assert_eq!(line.locate((4.0, -3.0)), (4.0, 3.0));
        assert_eq!(line.locate((4.0, 2.0)), (4.0, -2.0));
        assert_eq!(line.locate((4.0, 0.0)), (4.0, 0.0));
        // Beside the stretch that runs down, right of it is towards the tops.
        assert_eq!(line.locate((12.0, 5.0)), (15.0, 2.0));
        assert_eq!(line.locate((9.0, 5.0)), (15.0, -1.0));
        // Past the ends, the first and the last segments go on.
        assert_eq!(line.locate((-5.0, -1.0)), (-5.0, 1.0));
        assert_eq!(line.locate((26.0, 11.0)), (36.0, -1.0));
        // Outside a corner, the corner is the nearest point, and the distance is to it.
        let (along, across) = line.locate((13.0, -4.0));
        assert_eq!((along, across), (10.0, 5.0));
        // Of 2 points of the line as near, the earlier counts.
        assert_eq!(line.locate((5.0, 5.0)).0, 5.0);
        assert!(Line::of(&[(1.0, 1.0), (1.0, 1.0)]).is_none() && Line::of(&[]).is_none());
    }

    #[test]
    fn req_auto_065_the_line_beside_follows_the_corners() {
        let line = Line::of(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]).unwrap();
        // 2 mm towards the tops: above the first stretch, right of the second, the corner pushed out along
        // the line that halves it.
        let beside = line.beside(2.0, 0.0, 20.0);
        let half = 2.0 / 2.0_f64.sqrt();
        assert_eq!(beside.len(), 3);
        assert!(near(beside[0], (0.0, -2.0)) && near(beside[1], (10.0 + half, -half)) && near(beside[2], (12.0, 10.0)), "{beside:?}");
        // Past the ends and below the line, with no corner between the ends asked for.
        let below = line.beside(-1.0, -4.0, 6.0);
        assert!(below.len() == 2 && near(below[0], (-4.0, 1.0)) && near(below[1], (6.0, 1.0)), "{below:?}");
        let past = line.beside(0.0, 12.0, 25.0);
        assert!(past.len() == 2 && near(past[0], (10.0, 2.0)) && near(past[1], (10.0, 15.0)), "{past:?}");
        // A corner exactly at an end asked for isn't given twice, and is the start of the stretch after it.
        assert_eq!(line.beside(0.0, 10.0, 12.0).len(), 2);
        assert!(near(line.beside(1.0, 10.0, 12.0)[0], (11.0, 0.0)));
        assert_eq!(line.beside(0.0, 5.0, 10.0).len(), 2);
        // A line that turns straight back keeps the later stretch's side.
        let back = Line::of(&[(0.0, 0.0), (10.0, 0.0), (0.0, 0.0)]).unwrap();
        assert!(near(back.beside(1.0, 5.0, 15.0)[1], (10.0, 1.0)));
    }
}
