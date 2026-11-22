//! Crowded needle points pulled in (`REQ-AUTO-171`, `docs/src/design/image-to-embroidery.md` › Smooth satin
//! edges): where a spot of a design digitized automatically still sews above the warning's limit once the
//! cap's 3 kinds are done, the cap moves crowded top points of the satin columns there in along their
//! stitches, before its last resort takes any point out.
//!
//! An automatic column sews every stitch from rail to rail as it is made, and keeps its spacing on the inside
//! of a bend, so that its inside edge sews as one row. Its points still crowd where it fans into a corner,
//! and where the layers under and over it reach the limit that crowding is too much thread. Only there does a
//! point move in, as digitizing programs shorten some stitches in a turn: the edge elsewhere keeps its
//! stitches full length. A moved point keeps its place in its run, so the plan keeps its jumps, trims and
//! thread changes, and the measure is kept up to date stitch by stitch, as the cap keeps it.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_plan::thickness::Measure;
use stitchcraft_plan::{Role, SewnStitch};

use super::{Cap, NEAR, Ribbon, cell};
use crate::auto::CROWDED_CLOSER_THAN;
use crate::auto::thin::Entry;
use crate::normalize::near::to_side;

/// A point the cap may move, as it would move: its 2 stitches as they are and as they would be, and where
/// the point would go.
#[derive(Clone, Copy, Debug)]
struct Move {
    point: Entry,
    removed: [SewnStitch; 2],
    added: [SewnStitch; 2],
    to: Point,
}

impl Cap<'_, '_> {
    /// Pulls in crowded top points of the columns of `pulls` among `points` while a spot sews above the
    /// warning's limit, one at a time: of those whose stitches pass within [`NEAR`] of the thickest spot,
    /// the one that lowers it most. A point moves by its column's shares of the way in turn, and stays once
    /// they are used up, when both its neighbours on its rail have moved before it, or when the measure
    /// refuses the move (`Pass::changed`). A unit of work for each point looked at each time.
    pub(super) fn pull_in(&mut self, points: &[Entry], meter: &mut Meter) -> Result<(), Exhausted> {
        let mut moves: BTreeMap<Entry, usize> = BTreeMap::new();
        let mut refused: BTreeSet<(Entry, usize)> = BTreeSet::new();
        let mut places: BTreeSet<(i64, i64)> = BTreeSet::new();
        while self.above(self.pass.rules.limit, meter)? {
            let spot = self.pass.thickness.most(Measure::Fills).at;
            let was = self.pass.thickness.at(Measure::Fills, spot);
            let mut best: Option<(f64, Move)> = None;
            for &point in points {
                meter.charge(1)?;
                let times = moves.get(&point).copied().unwrap_or(0);
                if refused.contains(&(point, times)) {
                    continue;
                }
                let Some(pulled) = self.pulled(point, times, &moves, spot, meter)? else { continue };
                let lowered = self.lowers(&pulled, was, spot, meter)?;
                if lowered > 0.0 && best.as_ref().is_none_or(|(most, _)| lowered > *most) {
                    best = Some((lowered, pulled));
                }
            }
            let Some((_, pulled)) = best else { break };
            let times = moves.get(&pulled.point).copied().unwrap_or(0);
            if !self.pass.changed(&pulled.removed, &pulled.added, meter)? {
                refused.insert((pulled.point, times));
                continue;
            }
            self.pass.moved(pulled.point, pulled.to);
            moves.insert(pulled.point, times + 1);
            places.insert((cell(spot.x()), cell(spot.y())));
            // The 2 stitches cover as they now lie, pulled in as they were.
            for (was, now) in pulled.removed.iter().zip(pulled.added) {
                if let Some(ribbon) = self.ribbons.kill((was.block, was.index)) {
                    self.ribbons.add(Ribbon::of((now.from, now.to), ribbon.cut, ribbon.at, ribbon.element).firm(ribbon.firm), meter)?;
                }
            }
        }
        self.pulled = [moves.len(), places.len()];
        Ok(())
    }

    /// How entry `point`, moved `times` before, would move in, by the share of its column's next turn: `None`
    /// when it may not. It must be a top point of a column of the rules' `pulls` between 2 stitches of its
    /// column, one of which passes within [`NEAR`] of `spot`; closer than [`CROWDED_CLOSER_THAN`] to the point
    /// before or after it on its rail; not both of whose neighbours there have moved (`moves`) while it
    /// hasn't; and its 2 stitches, moved, no shorter than the machine's shortest and no longer than its
    /// longest. It moves towards the middle of its 2 stitches' other ends.
    fn pulled(&self, point: Entry, times: usize, moves: &BTreeMap<Entry, usize>, spot: Point, meter: &mut Meter) -> Result<Option<Move>, Exhausted> {
        let pass = &self.pass;
        let (block, index) = point;
        let Some((here, role, element)) = pass.stitch(point) else { return Ok(None) };
        let Some(share) = element.and_then(|element| pass.rules.pulls.get(&element)).and_then(|shares| shares.get(times)).copied() else {
            return Ok(None);
        };
        let ours = |at: usize| pass.stitch((block, at)).is_some_and(|(_, role, other)| role == Role::Top && other == element);
        if role != Role::Top {
            return Ok(None);
        }
        let (Some(from), Some(to)) = (pass.beside(point, false, meter)?, pass.beside(point, true, meter)?) else { return Ok(None) };
        let (Some(into), Some(out)) = (pass.sewn(block, from, index), pass.sewn(block, index, to)) else { return Ok(None) };
        let place = pass.thickness.placed(spot);
        if !ours(from) || !ours(to) || (to_side(place, into.from, into.to) > NEAR && to_side(place, out.from, out.to) > NEAR) {
            return Ok(None);
        }
        // Its neighbours on its rail: 2 points before it and after it in its run.
        let before = pass.beside((block, from), false, meter)?.filter(|at| ours(*at));
        let after = pass.beside((block, to), true, meter)?.filter(|at| ours(*at));
        let apart = |at: Option<usize>| at.and_then(|at| pass.stitch((block, at))).map_or(f64::INFINITY, |(there, _, _)| there.distance(here));
        if apart(before).min(apart(after)) >= CROWDED_CLOSER_THAN {
            return Ok(None);
        }
        let moved = [before, after].into_iter().flatten().filter(|at| moves.contains_key(&(block, *at))).count();
        if times == 0 && moved == 2 {
            return Ok(None);
        }
        let middle = into.from.lerp(out.to, 0.5);
        let to_place = pass.thickness.placed(pass.thickness.point(here.lerp(middle, share)));
        let added = [SewnStitch { to: to_place, ..into }, SewnStitch { from: to_place, ..out }];
        let (first, last, now) = (pass.at((block, from)), pass.at((block, to)), Some(pass.thickness.point(to_place)));
        if !pass.long_enough(&[(first, now, false, f64::INFINITY), (now, last, false, f64::INFINITY)]) {
            return Ok(None);
        }
        Ok(Some(Move { point, removed: [into, out], added, to: to_place }))
    }

    /// How many fills `pulled` would take from `spot`, which sews `was`: the measure with the moved stitches,
    /// and back.
    fn lowers(&mut self, pulled: &Move, was: f64, spot: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
        let thickness = &mut *self.pass.thickness;
        for stitch in &pulled.removed {
            thickness.remove(stitch, meter)?;
        }
        for stitch in &pulled.added {
            thickness.add(stitch, meter)?;
        }
        let lowered = was - thickness.at(Measure::Fills, spot);
        for stitch in &pulled.added {
            thickness.remove(stitch, meter)?;
        }
        for stitch in &pulled.removed {
            thickness.add(stitch, meter)?;
        }
        Ok(lowered)
    }
}
