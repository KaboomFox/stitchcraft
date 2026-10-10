//! Short stitches on curves (`docs/src/design/algorithms/satin.md` › Top stitches, step 3).
//!
//! On the inside of a tight curve, a rail's needle points crowd together, and the thread piles up where
//! they meet. Ink/Stitch insets some of them, and so does StitchCraft. On each rail, a needle point closer
//! than `short_stitch_distance_mm` to the last point left in place there moves in along its stitch, by a
//! `short_stitch_inset` percentage of the stitch's width. Points that crowd one after another take turns
//! with the percentages and start over at the first, so `15 30` insets them by 15 %, 30 %, 15 % and on. A
//! point at least that far from the last one left in place stays, and is the one the next points are
//! measured from. A distance of 0 insets none.
//!
//! The points are the compensated ones. An inset moves its point toward the other end of the stitch, as
//! negative pull compensation does.

use stitchcraft_core::Point;

use crate::generators::satin::compensation::offset;
use crate::generators::satin::pairs::Pair;

/// `pairs` with their crowded needle points inset: those closer than `distance` millimetres to the last
/// point left in place on their rail, by `insets` of their stitch's width, taking turns. No point is
/// closer than 0, and without insets a crowded point moves by none.
pub(crate) fn inset(pairs: Vec<Pair>, distance: f64, insets: &[f64]) -> Vec<Pair> {
    let (mut first, mut second) = (Rail::default(), Rail::default());
    pairs
        .into_iter()
        .map(|[a, b]| {
            let by = [first.inset(a, b, distance, insets), second.inset(b, a, distance, insets)];
            offset([a, b], by, [0.0, 0.0])
        })
        .collect()
}

/// What one rail remembers: the last point left in place there, and which inset comes next.
#[derive(Default)]
struct Rail {
    /// The last point left in place.
    last: Option<Point>,
    /// The inset the next crowded point takes.
    next: usize,
}

impl Rail {
    /// How far `point`, at one end of the stitch to `other`, moves out along it: by minus its inset when
    /// it is closer than `distance` to the last point left in place, else not at all.
    fn inset(&mut self, point: Point, other: Point, distance: f64, insets: &[f64]) -> f64 {
        if self.next >= insets.len() {
            self.next = 0;
        }
        match self.last {
            Some(last) if point.distance(last) < distance => {
                let share = insets.get(self.next).copied().unwrap_or(0.0);
                self.next += 1;
                -share * point.distance(other)
            }
            _ => {
                self.last = Some(point);
                self.next = 0;
                0.0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// Pairs 4 mm across, their first ends at `x` along y = 0 and their second ends at `x2` along y = 4.
    fn pairs(x: &[f64], x2: &[f64]) -> Vec<Pair> {
        x.iter().zip(x2).map(|(&a, &b)| [p(a, 0.0), p(b, 4.0)]).collect()
    }

    #[test]
    fn a_point_closer_than_the_distance_to_the_last_left_in_place_moves_in() {
        // The first rail's points 0.1 mm apart, the second's 1 mm apart: on the first, each point within
        // 0.25 mm of the last one left in place moves in by 10 % of its 4 mm stitch, along it.
        let first = [0.0, 0.1, 0.2, 0.3, 0.4, 0.5];
        let second = [0.0, 1.0, 2.0, 3.0, 4.0, 5.0];
        let sewn = inset(pairs(&first, &second), 0.25, &[0.1]);
        let moved: Vec<bool> = sewn.iter().zip(pairs(&first, &second)).map(|([a, _], [was, _])| *a != was).collect();
        assert_eq!(moved, [false, true, true, false, true, true], "0.3 is 0.3 mm on from 0, and 0.4, 0.5 are within 0.25 of 0.3");
        // The second pair's stitch runs from (0.1, 0) to (1, 4), 4.1 mm: its first end moves 0.41 mm along it.
        let [a, _] = sewn[1];
        assert!(a.distance(p(0.19, 0.4)) < 1e-12, "{a:?}");
        assert!(sewn.iter().zip(pairs(&first, &second)).all(|([_, b], [_, was])| *b == was), "the second rail's points are far apart");
    }

    #[test]
    fn crowded_points_take_the_insets_in_turn() {
        let first = [0.0, 0.05, 0.1, 0.15, 0.2, 0.3];
        let sewn = inset(pairs(&first, &first), 0.25, &[0.1, 0.3]);
        // Straight stitches 4 mm across: the first rail's points move up by 0.4 or 1.2 mm.
        let up: Vec<f64> = sewn.iter().map(|[a, _]| (a.y() * 1e9).round() / 1e9).collect();
        assert_eq!(up, [0.0, 0.4, 1.2, 0.4, 1.2, 0.0]);
    }

    #[test]
    fn a_point_exactly_the_distance_from_the_last_left_in_place_stays() {
        // As in Ink/Stitch: only a point closer than the distance moves.
        let even = pairs(&[0.0, 0.25, 0.5], &[0.0, 0.25, 0.5]);
        assert_eq!(inset(even.clone(), 0.25, &[0.15]), even);
    }

    #[test]
    fn a_distance_of_0_or_no_insets_leaves_every_point() {
        let crowded = pairs(&[0.0, 0.01, 0.02], &[0.0, 0.01, 0.02]);
        assert_eq!(inset(crowded.clone(), 0.0, &[0.15]), crowded);
        assert_eq!(inset(crowded.clone(), 0.25, &[]), crowded);
        assert_ne!(inset(crowded.clone(), 0.25, &[0.15]), crowded);
    }
}
