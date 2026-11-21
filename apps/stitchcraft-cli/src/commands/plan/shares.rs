//! Each thread's share of a plan's stitches (`REQ-AUTO-162`): what `stitch plan` prints under `shares`
//! and the report keeps under the same name.
//!
//! A one-needle machine is rethreaded by hand for every thread, so before sewing a person wants to see
//! which thread sews almost nothing. The list counts, it doesn't choose: no thread is called one to drop.
//! The summary and the report both take their numbers from [`shares`], so they can't disagree.
//!
//! A thread is its colour and its name. A thread the plan threads twice, sewn, changed and sewn again, is
//! one share, threaded 2 times, since it is one spool. A stop keeps the thread in the needle and is no
//! threading. The shares are listed in the order each thread is first threaded.
//!
//! The stitches counted are the plan's `Normal` entries, as the plan's own counts are (`PlanStats`):
//! underlay and lock stitches use the same thread and count. Each share is rounded to a tenth of a percent
//! by largest remainder, so that the shares as shown add up to exactly 100.0: each thread gets its tenths
//! rounded down, and the tenths left over go one each to the threads whose remainders were largest, the
//! first threaded first among equal ones. A plan without stitches has a share of 0.0 for each thread.
//!
//! The loops run over the plan the engine made, whose stitches its `Budget` already bounded, as the
//! plan's own counts do, and not over input: they charge no meter.

use std::fmt::Write as _;

use stitchcraft_plan::{StitchKind, StitchPlan, Thread};

/// Tenths of a percent in the whole: the shares, as shown, add up to this.
const WHOLE: u64 = 1000;

/// One thread's share of a plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Share<'a> {
    /// The thread.
    pub thread: &'a Thread,
    /// The `Normal` stitches it sews.
    pub stitches: u64,
    /// Its share of the plan's stitches, in tenths of a percent.
    pub tenths: u64,
    /// How many times the machine is threaded with it.
    pub threaded: u64,
}

impl Share<'_> {
    /// The share in percent, to a tenth: `tenths` as the report writes it.
    pub fn percent(&self) -> f64 {
        // At most 1000 tenths, which an f64 holds exactly.
        self.tenths as f64 / 10.0
    }
}

/// Each thread of `plan` once, in the order it is first threaded, with its stitches, its share and how
/// many times it is threaded.
pub fn shares(plan: &StitchPlan) -> Vec<Share<'_>> {
    let mut shares: Vec<Share<'_>> = Vec::new();
    for block in &plan.blocks {
        let stitches = block.stitches.iter().filter(|stitch| stitch.kind == StitchKind::Normal).count() as u64;
        match shares.iter_mut().find(|share| *share.thread == block.thread) {
            Some(share) => {
                share.stitches += stitches;
                share.threaded += 1;
            }
            None => shares.push(Share { thread: &block.thread, stitches, tenths: 0, threaded: 1 }),
        }
    }
    let total: u64 = shares.iter().map(|share| share.stitches).sum();
    if total == 0 {
        return shares;
    }
    // Each share's tenths rounded down, and what is left of each, in thousandths of a tenth of the total.
    let mut left: Vec<(u64, usize)> = Vec::with_capacity(shares.len());
    for (i, share) in shares.iter_mut().enumerate() {
        let scaled = share.stitches * WHOLE;
        share.tenths = scaled / total;
        left.push((scaled % total, i));
    }
    let given: u64 = shares.iter().map(|share| share.tenths).sum();
    // Largest remainder first; among equal ones, the thread threaded first.
    left.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    let missing = usize::try_from(WHOLE.saturating_sub(given)).unwrap_or(0);
    for &(_, i) in left.iter().take(missing) {
        if let Some(share) = shares.get_mut(i) {
            share.tenths += 1;
        }
    }
    shares
}

/// The shares, one line each, as `stitch plan` prints them under `shares`.
pub fn describe(out: &mut String, shares: &[Share<'_>]) {
    for (i, share) in shares.iter().enumerate() {
        let name = share.thread.name.as_deref().unwrap_or("unnamed");
        let stitches = if share.stitches == 1 { "stitch" } else { "stitches" };
        let threaded = match share.threaded {
            1 => "once".to_string(),
            n => format!("{n} times"),
        };
        let line = format!(
            "{}. {name} ({}): {} {stitches}, {}.{} %, threaded {threaded}",
            i + 1,
            share.thread.color,
            share.stitches,
            share.tenths / 10,
            share.tenths % 10
        );
        let _ = writeln!(out, "  {:<10}{line}", if i == 0 { "shares" } else { "" });
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role};

    use super::*;

    const RED: Rgb = Rgb::new(200, 0, 0);
    const GREEN: Rgb = Rgb::new(0, 160, 0);

    /// A plan of `blocks`: each a thread and how many stitches it sews, after a jump to its start.
    fn plan(blocks: &[(Thread, usize)]) -> StitchPlan {
        let mut plan = StitchPlan::default();
        for (thread, stitches) in blocks {
            let mut b = PlanBuilder::new(thread.clone());
            b.jump(stitchcraft_core::Point::new(0.0, 0.0).unwrap(), Provenance::plan(Role::Travel));
            for i in 0..*stitches {
                b.stitch(stitchcraft_core::Point::new(i as f64, 0.0).unwrap(), Provenance::plan(Role::Top));
            }
            plan.blocks.extend(b.finish().blocks);
        }
        plan
    }

    fn tenths(shares: &[Share<'_>]) -> Vec<u64> {
        shares.iter().map(|share| share.tenths).collect()
    }

    #[test]
    fn req_auto_162_a_thread_threaded_twice_is_one_share_threaded_2_times() {
        let (red, green) = (Thread::named(RED, "Red"), Thread::named(GREEN, "Green"));
        let plan = plan(&[(red.clone(), 30), (green.clone(), 50), (red.clone(), 20)]);
        let shares = shares(&plan);
        assert_eq!(shares.len(), 2);
        assert_eq!((shares[0].thread, shares[0].stitches, shares[0].threaded, shares[0].tenths), (&red, 50, 2, 500));
        assert_eq!((shares[1].thread, shares[1].stitches, shares[1].threaded, shares[1].tenths), (&green, 50, 1, 500));
        let mut out = String::new();
        describe(&mut out, &shares);
        assert_eq!(
            out,
            "  shares    1. Red (#c80000): 50 stitches, 50.0 %, threaded 2 times\n            2. Green (#00a000): 50 stitches, 50.0 %, threaded once\n"
        );
    }

    #[test]
    fn req_auto_162_the_same_colour_by_another_name_is_another_thread() {
        let plan = plan(&[(Thread::named(RED, "Red"), 1), (Thread::new(RED), 3)]);
        let shares = shares(&plan);
        assert_eq!(shares.iter().map(|share| share.threaded).collect::<Vec<_>>(), [1, 1]);
        assert_eq!(tenths(&shares), [250, 750]);
        let mut out = String::new();
        describe(&mut out, &shares);
        assert!(out.contains("1. Red (#c80000): 1 stitch, 25.0 %") && out.contains("2. unnamed (#c80000): 3 stitches, 75.0 %"), "{out}");
    }

    #[test]
    fn req_auto_162_a_design_in_one_thread_is_all_of_it() {
        let plan = plan(&[(Thread::new(GREEN), 7)]);
        let shares = shares(&plan);
        assert_eq!(tenths(&shares), [1000]);
        assert_eq!(shares[0].percent(), 100.0);
        assert_eq!(shares[0].stitches, 7);
    }

    #[test]
    fn req_auto_162_the_shares_as_shown_add_up_to_100_by_largest_remainder() {
        // Thirds: 333.33 tenths each, so one gets the tenth left over, the first threaded among equals.
        let threads = [Thread::new(RED), Thread::new(GREEN), Thread::new(Rgb::new(0, 0, 200))];
        let thirds = plan(&[(threads[0].clone(), 1), (threads[1].clone(), 1), (threads[2].clone(), 1)]);
        assert_eq!(tenths(&shares(&thirds)), [334, 333, 333]);
        // 1, 2 and 4 of 7: 142.86, 285.71 and 571.43 tenths. The largest remainder, .86, gets the tenth.
        let sevenths = plan(&[(threads[0].clone(), 1), (threads[1].clone(), 2), (threads[2].clone(), 4)]);
        let shares = shares(&sevenths);
        assert_eq!(tenths(&shares), [143, 286, 571]);
        assert_eq!(shares.iter().map(|share| share.tenths).sum::<u64>(), WHOLE);
        assert_eq!(shares.iter().map(Share::percent).collect::<Vec<_>>(), [14.3, 28.6, 57.1]);
        // A share too small to show is 0.0, and the rest still adds up.
        let tiny = plan(&[(threads[0].clone(), 1), (threads[1].clone(), 2999)]);
        assert_eq!(tenths(&super::shares(&tiny)), [0, 1000]);
    }

    #[test]
    fn req_auto_162_a_design_without_stitches_has_a_share_of_none() {
        assert!(shares(&StitchPlan::default()).is_empty());
        let mut out = String::new();
        describe(&mut out, &[]);
        assert!(out.is_empty());
        let plan = plan(&[(Thread::new(RED), 0)]);
        let shares = shares(&plan);
        assert_eq!((shares[0].stitches, shares[0].tenths, shares[0].threaded), (0, 0, 1));
        assert_eq!(shares[0].percent(), 0.0);
    }
}
