//! Passes over a run of stitches: repeats and bean stitch, for a bolder line or a line that comes back.
//!
//! Design: `docs/src/design/algorithms/strokes.md` › Repeats and bean stitch. A run is the needle points
//! of one pass along a piece of a stroke; the generators that sew lines, running stitch, manual stitch and
//! zigzag stroke, hand theirs to [`sew`]:
//!
//! - **Repeats** sew the run k times, every other pass backwards. Each pass starts where the last one
//!   ended, so a turnaround is never a stitch in place. A running or manual stitch's pass after the first
//!   starts with the point the last ended at, which Ink/Stitch lists again, and a zigzag's with the point
//!   after it, which Ink/Stitch lists once ([`Turn`]).
//! - **Bean stitch** then sews each stitch from A to B 2b + 1 times: A to B, then b times back to A and on
//!   to B. b is taken in turn from a list, which runs on from pass to pass. Every point of a pass takes a
//!   step of it, and a point where the needle already is sews nothing: each pass after the first starts
//!   with the point the last ended at, and manual stitch lists some points again. That is Ink/Stitch's
//!   counting (it lists the turnaround point again, and the repeated point takes a step), so a file sews
//!   the same in both tools; with a two-value list it also sews each stretch of the line the same way on
//!   every pass. A zigzag stroke's passes are joined instead, and a turn takes no step.

use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_params::{StitchType, params};

use crate::generators::made;

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

/// The key of the setting that sews each stitch back and forth, which automatic digitizing sets for a triple
/// run (`crate::auto`).
pub const BEAN_STITCH_REPEATS: &str = "bean_stitch_repeats";

/// The point each pass after the first starts with, which decides how bean stitch counts its steps.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Turn {
    /// With the point the last pass ended at, which takes a step of the bean list and sews nothing:
    /// running and manual stitch, whose turns Ink/Stitch lists twice.
    Listed,
    /// With the point after it, and the turn takes no step: a zigzag stroke, whose passes Ink/Stitch joins
    /// without listing the turn again.
    Joined,
}

/// `run` sewn `repeats` times (at least once), each pass after the first starting as `turn` says, each
/// stitch then sewn 2b + 1 times with b taken in turn from `beans` (none: 0), a step for every point of
/// every pass. A point where the needle already is takes its step and sews nothing. Every needle point
/// costs one unit of `meter`'s work, and every one sewn again, by a pass after the first or by bean
/// stitch, a stitch: those of the run were spent as it was made.
pub fn sew(run: &[Point], repeats: u32, beans: &[u32], turn: Turn, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some((&start, rest)) = run.split_first() else { return Ok(Vec::new()) };
    let mut out = vec![start];
    let mut step = 0_usize;
    for pass in 0..repeats.max(1) {
        // A pass after the first starts with the point the last ended at, or the one after it; the way back
        // is the run reversed. The first pass starts after the run's start, which the needle is at.
        let skip = usize::from(turn == Turn::Joined);
        let points: Vec<Point> = match pass {
            0 => rest.to_vec(),
            _ if pass % 2 == 1 => run.iter().rev().skip(skip).copied().collect(),
            _ => run.iter().skip(skip).copied().collect(),
        };
        // A pass after the first sews every point of the run but the one the needle is at.
        if pass > 0 {
            made(meter, rest.len())?;
        }
        for to in points {
            let from = out.last().copied().unwrap_or(start);
            let bean = step.checked_rem(beans.len()).and_then(|i| beans.get(i)).copied().unwrap_or(0);
            step += 1;
            meter.charge(1)?;
            if to == from {
                continue;
            }
            out.push(to);
            for _ in 0..bean {
                meter.charge(2)?;
                meter.charge_stitches(2)?;
                out.push(from);
                out.push(to);
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    #[test]
    fn the_bean_stitch_key_is_the_registered_one() {
        assert!(super::RepeatParams::SPECS.iter().any(|spec| spec.key == super::BEAN_STITCH_REPEATS));
    }

    use super::*;

    fn xs(points: &[Point]) -> Vec<f64> {
        points.iter().map(|p| p.x()).collect()
    }

    fn run(xs: &[f64]) -> Vec<Point> {
        xs.iter().map(|x| Point::new(*x, 0.0).unwrap()).collect()
    }

    fn sewn(xs_: &[f64], repeats: u32, beans: &[u32]) -> Vec<f64> {
        xs(&sew(&run(xs_), repeats, beans, Turn::Listed, &mut Budget::DEFAULT.meter()).unwrap())
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
        // A point where the needle is takes its step, and sews nothing.
        assert_eq!(sewn(&[0.0, 1.0, 1.0, 2.0], 1, &[1, 0]), [0.0, 1.0, 0.0, 1.0, 2.0, 1.0, 2.0]);
        assert_eq!(sewn(&[0.0, 0.0, 1.0], 1, &[]), [0.0, 1.0]);
        // Two passes: steps 0 and 1 there, the turnaround step 2, steps 3 and 4 back; step 3 takes the
        // list's first value again, so the first stitch back is the bold one.
        assert_eq!(sewn(&[0.0, 1.0, 2.0], 2, &[1, 0, 0]), [0.0, 1.0, 0.0, 1.0, 2.0, 1.0, 2.0, 1.0, 0.0]);
    }

    #[test]
    fn req_run_012_joined_passes_turn_without_listing_the_turn_again() {
        let joined = |repeats, beans: &[u32]| xs(&sew(&run(&[0.0, 1.0, 2.0]), repeats, beans, Turn::Joined, &mut Budget::DEFAULT.meter()).unwrap());
        assert_eq!(joined(3, &[]), sewn(&[0.0, 1.0, 2.0], 3, &[]), "the same points without bean stitch");
        // Steps 0 and 1 there, then 2 and 3 back: the first stitch back takes step 2, the list's third value,
        // and the second takes the first again.
        assert_eq!(joined(2, &[1, 0, 0]), [0.0, 1.0, 0.0, 1.0, 2.0, 1.0, 0.0, 1.0, 0.0]);
    }

    #[test]
    fn the_budget_bounds_the_work_and_the_stitches() {
        let mut meter = Budget { max_stitches: u32::MAX, max_work: 5 }.meter();
        assert_eq!(sew(&run(&[0.0, 1.0, 2.0]), 3, &[2], Turn::Listed, &mut meter), Err(Exhausted::Work));
        // The run's own points were spent as it was made; each later pass and each bean stitch costs a
        // stitch for every needle point it sews again.
        let stitches = |max_stitches, repeats, beans: &[u32]| {
            sew(&run(&[0.0, 1.0, 2.0]), repeats, beans, Turn::Listed, &mut Budget { max_stitches, max_work: u64::MAX }.meter()).map(|out| out.len())
        };
        assert_eq!(stitches(0, 1, &[]), Ok(3));
        assert_eq!((stitches(4, 3, &[]), stitches(3, 3, &[])), (Ok(7), Err(Exhausted::Stitches)), "2 passes after the first, 2 points each");
        assert_eq!(
            (stitches(8, 1, &[2]), stitches(7, 1, &[2])),
            (Ok(11), Err(Exhausted::Stitches)),
            "bean stitch sews each of 2 stitches 4 times more"
        );
    }
}
