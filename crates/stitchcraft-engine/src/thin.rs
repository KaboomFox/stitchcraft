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
//! near instead, back to one that stays, never the run's first.
//!
//! **Unless 2 that stay meet.** Elements sewn straight on into each other can bring a point that stays
//! within the shortest stitch of the one that stays before it: the run's last point near its first, or a
//! tie-off's near a tie-in's, once the stitches between are left out. A stitch between them would be too
//! short for the machine, so the later one is left out after all, and the points before it stay. That
//! covers a point where the needle already is, which would sew in place. The run's first point is the
//! only one never left out.

/// `run` without the needle points too near the one before them. `too_near` says whether a stitch from one
/// point to another would be shorter than it may be (a point where the needle already is being too near),
/// and `pinned` whether a point stays wherever it is. `left_out` hears of each point left out, by the
/// stitch that fell short, from its first point to its second: into the point from the one before it, or
/// out of it to the point that stays after it.
pub(crate) fn thin<T>(run: Vec<T>, too_near: impl Fn(&T, &T) -> bool, pinned: impl Fn(&T) -> bool, mut left_out: impl FnMut(&T, &T)) -> Vec<T> {
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
            left_out(from, &point);
        } else {
            // A point that stays: the ones before it that it is too near go instead, back to one that stays,
            // unless it is too near that one too.
            let gone = kept.iter().skip(1).rev().take_while(|k| !pinned(k) && too_near(k, &point)).count();
            if let Some(stays) = kept.len().checked_sub(gone + 1).and_then(|stays| kept.get(stays)).filter(|k| too_near(k, &point)) {
                left_out(stays, &point);
            } else {
                for _ in 0..gone {
                    if let Some(k) = kept.pop() {
                        left_out(&k, &point);
                    }
                }
                kept.push(point);
            }
        }
    }
    kept
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Point;

    use super::*;

    fn p(x: f64) -> Point {
        Point::new(x, 0.0).unwrap()
    }

    /// `xs` along a line thinned to stitches of at least 1, the points at `pins` pinned: where the kept
    /// points are, and how many were left out.
    fn thinned(xs: &[f64], pins: &[usize]) -> (Vec<f64>, usize) {
        let run: Vec<(usize, Point)> = xs.iter().map(|&x| p(x)).enumerate().collect();
        let mut count = 0;
        let kept = thin(run, |a, b| a.1.distance(b.1) < 1.0, |(i, _)| pins.contains(i), |_, _| count += 1);
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
        assert_eq!(thinned(&[0.0, 1.5, 2.0], &[]), (vec![0.0, 2.0], 1));
        // Where the needle already is, the last point adds nothing.
        assert_eq!(thinned(&[0.0, 0.0], &[]), (vec![0.0], 1));
    }

    #[test]
    fn a_point_that_stays_too_near_the_one_that_stays_before_it_goes_and_the_points_between_stay() {
        // The last point too near the first: it goes, and the run ends where it starts.
        assert_eq!(thinned(&[0.0, 0.5], &[]), (vec![0.0], 1));
        assert_eq!(thinned(&[0.0, 0.6, 0.9], &[]), (vec![0.0], 2));
        // Back near the first, past a point it is too near: that point stays, and the last goes.
        assert_eq!(thinned(&[0.0, 1.0, 0.5], &[]), (vec![0.0, 1.0], 1));
        // A pinned point too near the pinned point before it.
        assert_eq!(thinned(&[0.0, 2.0, 2.5, 2.8, 5.0], &[1, 3]), (vec![0.0, 2.0, 5.0], 2));
    }

    #[test]
    fn a_pinned_point_stays_like_the_last_and_is_never_left_out_for_a_later_one() {
        assert_eq!(thinned(&[0.0, 2.0, 2.5, 4.0], &[2]), (vec![0.0, 2.5, 4.0], 1));
        // The last point, too near it, goes instead.
        assert_eq!(thinned(&[0.0, 2.0, 2.5, 2.9], &[1]), (vec![0.0, 2.0], 2));
    }
}
