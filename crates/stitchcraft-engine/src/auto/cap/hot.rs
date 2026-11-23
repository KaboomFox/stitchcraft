//! The spots the cap goes on to (`REQ-AUTO-194`, `docs/src/design/image-to-embroidery.md` › The cap below the
//! limit): where the pull-in or the last resort goes on past a spot it can't lower, it works only on the spots
//! that its points' stitches lay thread in, thickest first.
//!
//! A point lowers a spot only by taking thread out of it, so only a spot that one of its stitches lays thread in
//! can be lowered by it. A design whose shapes sewn by their own settings sew far above the limit has thousands
//! of such spots that no point of the cap's reaches. Going through every spot of the measure for each of them,
//! and through every point for each spot, cost a test sheet 25 times its plan's work. So the spots are kept in
//! order of their thread, with the points whose stitches lay thread in each, and a spot whose thread changed is
//! put back in its new place when it comes up. The order is the measure's: of 2 spots as thick, the later one
//! first, as [`Thickness::most`] names it.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Exhausted, Meter};
use stitchcraft_plan::SewnStitch;
use stitchcraft_plan::thickness::{Measure, Spot, Thickness};

use crate::auto::thin::{Entry, Pass};

/// The spots above a level that a set of points may lower, thickest first, each with those points.
#[derive(Debug, Default)]
pub(super) struct Hot {
    /// The spots, by their thread's bits and their place in the measure: a spot's thread is never under 0, so
    /// the bits keep the order of the values.
    queue: BTreeSet<(u64, usize)>,
    /// The points whose stitches lay thread in each spot.
    near: BTreeMap<usize, BTreeSet<Entry>>,
    /// The points that may be moved or taken.
    points: BTreeSet<Entry>,
    /// The spots none of the points lowered.
    stuck: BTreeSet<usize>,
    /// The level: a spot at it or under it is done.
    level: f64,
}

impl Hot {
    /// The spots above `level` that the stitches into and out of `points` lay thread in, in `pass`'s plan. A
    /// unit of work for each point, and for each step along each stitch.
    pub(super) fn of(pass: &Pass, points: &[Entry], level: f64, meter: &mut Meter) -> Result<Hot, Exhausted> {
        let mut hot = Hot { points: points.iter().copied().collect(), level, ..Hot::default() };
        for &point in points {
            meter.charge(1)?;
            let (block, index) = point;
            let (from, to) = (pass.beside(point, false, meter)?, pass.beside(point, true, meter)?);
            let stitches: Vec<SewnStitch> =
                [from.and_then(|from| pass.sewn(block, from, index)), to.and_then(|to| pass.sewn(block, index, to))].into_iter().flatten().collect();
            hot.add(pass.thickness, &[point], &stitches, meter)?;
        }
        Ok(hot)
    }

    /// Notes that `stitches`, as the plan now sews them, lay thread in their spots for `points`, those of them
    /// the cap may move or take, and puts each such spot above the level in its place. A unit of work for each
    /// step along each stitch.
    pub(super) fn add(&mut self, thickness: &Thickness, points: &[Entry], stitches: &[SewnStitch], meter: &mut Meter) -> Result<(), Exhausted> {
        for stitch in stitches {
            for (spot, fills) in thickness.spots(stitch, meter)? {
                let near = self.near.entry(spot).or_default();
                near.extend(points.iter().filter(|point| self.points.contains(point)));
                if fills > self.level && !self.stuck.contains(&spot) {
                    self.queue.insert((fills.to_bits(), spot));
                }
            }
        }
        Ok(())
    }

    /// The thickest spot above the level that isn't stuck, by its place in the measure, as `thickness` now
    /// measures it; `None` when there is none. It stays in its place until it is stuck or its thread changes. A
    /// unit of work for each spot looked at.
    pub(super) fn next(&mut self, thickness: &Thickness, meter: &mut Meter) -> Result<Option<(usize, Spot)>, Exhausted> {
        while let Some(&(bits, at)) = self.queue.last() {
            meter.charge(1)?;
            let now = thickness.at_spot(Measure::Fills, at).filter(|spot| spot.value > self.level && !self.stuck.contains(&at));
            match now {
                Some(spot) if spot.value.to_bits() == bits => return Ok(Some((at, spot))),
                _ => {
                    self.queue.remove(&(bits, at));
                    if let Some(spot) = now {
                        self.queue.insert((spot.value.to_bits(), at));
                    }
                }
            }
        }
        Ok(None)
    }

    /// The points whose stitches lay thread in the spot `at`, in the plan's order.
    pub(super) fn near(&self, at: usize) -> Vec<Entry> {
        self.near.get(&at).map(|near| near.iter().copied().collect()).unwrap_or_default()
    }

    /// Notes that no point lowered the spot `at`: it isn't looked at again.
    pub(super) fn stuck(&mut self, at: usize) {
        self.stuck.insert(at);
    }
}
