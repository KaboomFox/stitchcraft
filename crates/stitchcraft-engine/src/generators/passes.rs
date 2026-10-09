//! Passes over a run of stitches: repeats and bean stitch, for a bolder line or a line that comes back.
//!
//! Design: `docs/src/design/algorithms/strokes.md` › Repeats and bean stitch. A run is the needle points
//! of one pass along a piece of a stroke; the generators that sew lines (running stitch now, manual stitch
//! in M3.6) hand theirs to [`sew`]:
//!
//! - **Repeats** sew the run k times, every other pass backwards. Each pass starts where the last one
//!   ended, so a turnaround is never a stitch in place.
//! - **Bean stitch** then sews each stitch from A to B 2b + 1 times: A to B, then b times back to A and on
//!   to B. b is taken in turn from a list, which runs on from pass to pass, each turnaround taking one
//!   step of it. That is Ink/Stitch's counting (it repeats the turnaround point, and the repeated point
//!   takes a step), so a file sews the same in both tools; with a two-value list it also sews each stretch
//!   of the line the same way on every pass.

use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_params::{StitchType, params};

params! {
    /// Repeats and bean stitch: sewing a line more than once, for a bolder line or one that comes back.
    pub struct RepeatParams for &[StitchType::RunningStitch, StitchType::RippleStitch, StitchType::ZigzagStitch, StitchType::ManualStitch];

    "Repeats" {
        /// How many times to sew along the path: 2 goes there and back, 3 there, back and there again. An
        /// even number ends where the path starts, which suits a line that must come back.
        repeats: Count = "1", label "Repeats", range (1, 100),
            applies &[StitchType::RunningStitch, StitchType::RippleStitch, StitchType::ZigzagStitch];

        /// How many times to go back and forth over each stitch: 1 sews every stitch three times (there,
        /// back and there again), 2 five times. Several numbers separated by spaces are taken in turn along
        /// the stitches: "1 0" makes every other stitch bold.
        bean_stitch_repeats: CountList = "0", label "Bean stitch", range (0, 10);
    }
}

/// `run` sewn `repeats` times (at least once), each stitch then sewn 2b + 1 times with b taken in turn from
/// `beans` (none: 0). Every needle point costs one unit of `meter`.
pub fn sew(run: &[Point], repeats: u32, beans: &[u32], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some((&start, rest)) = run.split_first() else { return Ok(Vec::new()) };
    let mut out = vec![start];
    let mut step = 0_usize;
    for pass in 0..repeats.max(1) {
        // The way back: the run reversed. Its first point is where the last pass ended.
        let points: Vec<Point> = if pass % 2 == 0 { rest.to_vec() } else { run.iter().rev().skip(1).copied().collect() };
        if pass > 0 {
            // The turnaround takes a step of the list.
            step += 1;
        }
        for to in points {
            let from = out.last().copied().unwrap_or(start);
            let bean = step.checked_rem(beans.len()).and_then(|i| beans.get(i)).copied().unwrap_or(0);
            out.push(to);
            meter.charge(1)?;
            for _ in 0..bean {
                meter.charge(2)?;
                out.push(from);
                out.push(to);
            }
            step += 1;
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn xs(points: &[Point]) -> Vec<f64> {
        points.iter().map(|p| p.x()).collect()
    }

    fn run(xs: &[f64]) -> Vec<Point> {
        xs.iter().map(|x| Point::new(*x, 0.0).unwrap()).collect()
    }

    fn sewn(xs_: &[f64], repeats: u32, beans: &[u32]) -> Vec<f64> {
        xs(&sew(&run(xs_), repeats, beans, &mut Budget::DEFAULT.meter()).unwrap())
    }

    #[test]
    fn passes_turn_round_without_a_stitch_in_place() {
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 1, &[]), [0.0, 1.0, 2.0]);
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 2, &[]), [0.0, 1.0, 2.0, 1.0, 0.0]);
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 3, &[]), [0.0, 1.0, 2.0, 1.0, 0.0, 1.0, 2.0]);
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 0, &[]), [0.0, 1.0, 2.0], "at least once");
        assert_eq!(sewn(&[], 3, &[1]), Vec::<f64>::new());
    }

    #[test]
    fn bean_stitch_goes_back_and_forth_with_the_list_running_on() {
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 1, &[1]), [0.0, 1.0, 0.0, 1.0, 2.0, 1.0, 2.0]);
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 1, &[0, 2]), [0.0, 1.0, 2.0, 1.0, 2.0, 1.0, 2.0]);
        // Two passes: steps 0 and 1 there, the turnaround step 2, steps 3 and 4 back; step 3 takes the
        // list's first value again, so the first stitch back is the bold one.
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 2, &[1, 0, 0]), [0.0, 1.0, 0.0, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0]);
    }

    #[test]
    fn the_budget_bounds_the_work() {
        let mut meter = Budget { max_stitches: 1, max_work: 5 }.meter();
        assert_eq!(sew(&run(&[0.0, 1.0, 2.0]), 3, &[2], &mut meter), Err(Exhausted::Work));
    }
}
