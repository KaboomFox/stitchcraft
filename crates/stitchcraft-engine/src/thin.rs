//! The shortest stitch: a needle point too near the one before it is left out, so that the stitch runs on
//! to the next point (`docs/src/design/engine-pipeline.md` › Finalize).
//!
//! **Why one copy.** Finalize applies the rule where one element's stitching runs straight on into the
//! next. A generator that joins pieces of its own, as a tatami fill joins its rows and the travel between
//! them, applies it too, so that it hands finalize stitches with nothing left to thin. Both call this
//! function, so the rule cannot drift apart between them.
//!
//! **The ends stay.** A run's first point is where the needle lands, and its last is where the next jump,
//! trim or stop happens, or where the next element starts nearest. Both stay, and so do the points the
//! caller pins (finalize's lock points). A point that stays leaves out the points before it that it is too
//! near instead, back to one that stays, never the run's first. A point that stays where the needle already
//! is would sew in place, and is left out.

use stitchcraft_core::Point;

/// `run` without the needle points too near the one before them. `at` says where a point is, `too_near`
/// whether a stitch from one point to another would be shorter than it may be, and `pinned` whether a
/// point stays wherever it is. `left_out` hears of each point left out, by the point whose stitch fell
/// short: the point itself, or the point that stays and leaves out the ones before it.
pub(crate) fn thin<T>(
    run: Vec<T>,
    at: impl Fn(&T) -> Point,
    too_near: impl Fn(&T, &T) -> bool,
    pinned: impl Fn(&T) -> bool,
    mut left_out: impl FnMut(&T),
) -> Vec<T> {
    let last = run.len().saturating_sub(1);
    let mut kept: Vec<T> = Vec::with_capacity(run.len());
    for (i, point) in run.into_iter().enumerate() {
        let Some(from) = kept.last() else {
            kept.push(point);
            continue;
        };
        if !too_near(from, &point) {
            kept.push(point);
        } else if i != last && !pinned(&point) {
            left_out(&point);
        } else {
            // A point that stays: the ones before it go instead, back to one that stays.
            while kept.len() > 1 && kept.last().is_some_and(|k| !pinned(k) && too_near(k, &point)) {
                kept.pop();
                left_out(&point);
            }
            // Where the needle already is, it adds nothing: a stitch in place.
            if kept.last().is_some_and(|k| at(k) == at(&point)) {
                left_out(&point);
            } else {
                kept.push(point);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64) -> Point {
        Point::new(x, 0.0).unwrap()
    }

    /// `xs` along a line thinned to stitches of at least 1, the points at `pins` pinned: where the kept
    /// points are, and how many were left out.
    fn thinned(xs: &[f64], pins: &[usize]) -> (Vec<f64>, usize) {
        let run: Vec<(usize, Point)> = xs.iter().map(|&x| p(x)).enumerate().collect();
        let mut count = 0;
        let kept = thin(run, |&(_, q)| q, |a, b| a.1.distance(b.1) < 1.0, |(i, _)| pins.contains(i), |_| count += 1);
        (kept.into_iter().map(|(_, q)| q.x()).collect(), count)
    }

    #[test]
    fn a_point_too_near_the_one_before_is_left_out() {
        assert_eq!(thinned(&[0.0, 0.5, 1.2, 1.5, 3.0], &[]), (vec![0.0, 1.2, 3.0], 2));
        assert_eq!(thinned(&[], &[]), (vec![], 0));
        assert_eq!(thinned(&[4.0], &[]), (vec![4.0], 0));
    }

    #[test]
    fn the_last_point_stays_and_leaves_out_the_points_before_it_instead_but_never_the_first() {
        assert_eq!(thinned(&[0.0, 2.0, 2.5, 2.9], &[]), (vec![0.0, 2.9], 2));
        assert_eq!(thinned(&[0.0, 0.5], &[]), (vec![0.0, 0.5], 0));
        assert_eq!(thinned(&[0.0, 1.5, 2.0], &[]), (vec![0.0, 2.0], 1));
        // Where the needle already is, the last point adds nothing.
        assert_eq!(thinned(&[0.0, 0.0], &[]), (vec![0.0], 1));
    }

    #[test]
    fn a_pinned_point_stays_like_the_last_and_is_never_left_out_for_a_later_one() {
        assert_eq!(thinned(&[0.0, 2.0, 2.5, 4.0], &[2]), (vec![0.0, 2.5, 4.0], 1));
        assert_eq!(thinned(&[0.0, 2.0, 2.5, 2.9], &[1]), (vec![0.0, 2.0, 2.9], 1));
    }
}
