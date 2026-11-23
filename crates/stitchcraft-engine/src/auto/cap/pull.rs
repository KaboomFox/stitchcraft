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
//!
//! **Below the limit** (`REQ-AUTO-195`, `auto_short_stitches` `knots`). Once the cap's work above the limit is
//! done, it pulls in down to the cap, but only where a second row of stitch ends doesn't show along an edge:
//! a tight turn's knot, whose points lie closer than half the spacing on their rail, and a point whose new end
//! a later shape covers. Each moves by its first share only, as `REQ-AUTO-085` holds a column's edge, and no
//! pull makes a spot above the limit thicker: a pulled point lays its thread in the spots beside it.

use std::collections::BTreeMap;

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_plan::thickness::{Measure, PIXEL};
use stitchcraft_plan::{ElementRef, Role, SewnStitch};

use super::hot::Hot;
use super::{Cap, NEAR, Ribbon, cell};
use crate::auto::thin::Entry;
use crate::generators::satin::OFF;
use crate::normalize::near::to_side;

/// The share of a column's zigzag spacing that a point lies closer than to its rail's point before or after
/// it at a tight turn's knot (`REQ-AUTO-195`): half, 0.2 mm at the automatic columns' 0.4 mm. The outside of a
/// bend opens to 0.5 mm at the most (`REQ-AUTO-170`), so the inside's points lie this close only where the
/// rails' lengths differ by 2.5 times or more.
const KNOT: f64 = 0.5;

/// How much of a pulled point's way back, from its new end, a later shape must cover for the end not to show,
/// in millimetres: 2 pixels of the bare-fabric measure (`REQ-AUTO-195`).
const END: f64 = 2.0 * PIXEL;

/// A satin column whose crowded needle points the cap may pull in (`REQ-AUTO-171`): the shares of a stitch
/// its points move in by, in turn, its `short_stitch_inset`, and its zigzag spacing, which a point closer than
/// to its rail's point before or after it crowds.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Pullable {
    /// The shares, from 0 to 1.
    pub(crate) shares: Vec<f64>,
    /// The spacing, in millimetres.
    pub(crate) spacing: f64,
    /// Whether the cap pulls its points in below the warning's limit too, at its tight turns and under later
    /// shapes (`auto_short_stitches` `knots`, `REQ-AUTO-195`).
    pub(crate) knots: bool,
}

/// A point the cap may move, as it would move: its 2 stitches as they are and as they would be, and where
/// the point would go.
#[derive(Clone, Copy, Debug)]
pub(super) struct Move {
    pub(super) point: Entry,
    pub(super) removed: [SewnStitch; 2],
    pub(super) added: [SewnStitch; 2],
    /// Where the point is, and where it would go.
    pub(super) from: Point,
    pub(super) to: Point,
    /// Whether the point lies at a tight turn's knot: closer than [`KNOT`] of its column's spacing to its
    /// rail's point before or after it.
    pub(super) knot: bool,
    /// The point's element.
    pub(super) element: Option<ElementRef>,
}

/// What the pull-in down to `level` holds a spot above the warning's `limit` to: below the limit, no spot
/// above it gets thicker (`REQ-AUTO-195`); at the limit, `None`, as `Pass::changed` holds it.
pub(super) fn held_at(level: f64, limit: f64) -> Option<f64> {
    (level < limit).then_some(limit)
}

/// Whether a point that lowers the spot by `lowered` fills is chosen over the best so far, which lowered it by
/// `most`: it must lower it, and by more, so that of points that lower it alike the one sewn first moves.
pub(super) fn better(lowered: f64, most: Option<f64>) -> bool {
    lowered > 0.0 && most.is_none_or(|most| lowered > most)
}

impl Cap<'_, '_> {
    /// Pulls in crowded top points of the columns of `pulls` among `points` while a spot sews above `level`,
    /// one at a time: of those whose stitches pass within [`NEAR`] of the thickest spot, the one that lowers it
    /// most. A point moves by its column's shares of the way in turn, and stays once they are used up, when
    /// both its neighbours on its rail have moved before it, or when the measure refuses the move
    /// (`Pass::changed`). Where no point lowers the thickest spot, the pull-in stops, or with `skip` goes on to
    /// the next thickest (`REQ-AUTO-194`). At a spot at the warning's limit or under it, a point moves only at a
    /// knot or where a later shape covers its new end, by its first share, and while `level` is under the limit
    /// no pull makes a spot above the limit thicker (`REQ-AUTO-195`). A unit of work for each point looked at
    /// each time.
    pub(super) fn pull_in(&mut self, points: &[Entry], level: f64, skip: bool, meter: &mut Meter) -> Result<(), Exhausted> {
        let limit = self.pass.rules.limit;
        let hold = held_at(level, limit);
        // Going on past a spot, only the spots the points' stitches lay thread in are looked at ([`Hot`]).
        let mut hot = if skip { Some(Hot::of(&self.pass, points, level, meter)?) } else { None };
        loop {
            let (index, spot, near) = match hot.as_mut() {
                Some(hot) => {
                    let Some((index, spot)) = hot.next(self.pass.thickness, meter)? else { break };
                    (Some(index), spot.at, hot.near(index))
                }
                None if self.above(level, meter)? => (None, self.pass.thickness.most(Measure::Fills).at, points.to_vec()),
                None => break,
            };
            let was = self.pass.thickness.at(Measure::Fills, spot);
            let below = at_least(limit, was);
            let mut best: Option<(f64, Move)> = None;
            for point in near {
                meter.charge(1)?;
                let times = self.moves.get(&point).map_or(0, |(times, _)| *times);
                if self.refused.contains(&(point, times)) || (below && times > 0) {
                    continue;
                }
                let Some(pulled) = self.pulled(point, times, &self.moves, spot, meter)? else { continue };
                let lowered = self.lowers(&pulled, was, spot, meter)?;
                // Whether its new end shows is asked only of a point that would be chosen: it costs the most.
                if better(lowered, best.as_ref().map(|(most, _)| *most)) && !(below && !pulled.knot && self.shows(&pulled, meter)?) {
                    best = Some((lowered, pulled));
                }
            }
            let Some((_, pulled)) = best else {
                let (Some(hot), Some(index)) = (hot.as_mut(), index) else { break };
                hot.stuck(index);
                continue;
            };
            let (times, first) = self.moves.get(&pulled.point).copied().unwrap_or((0, pulled.from));
            if !self.pass.held(&pulled.removed, &pulled.added, hold, meter)? {
                self.refused.insert((pulled.point, times));
                continue;
            }
            self.pass.moved(pulled.point, pulled.to);
            if let Some(hot) = hot.as_mut() {
                // The points on either side share the moved stitches.
                let [into, out] = pulled.added;
                let (block, _) = pulled.point;
                let sharing =
                    [pulled.point, (block, out.index)].into_iter().chain(self.pass.beside(pulled.point, false, meter)?.map(|at| (block, at)));
                hot.add(self.pass.thickness, &sharing.collect::<Vec<Entry>>(), &[into, out], meter)?;
            }
            self.moves.insert(pulled.point, (times.saturating_add(1), first));
            self.places.insert((cell(spot.x()), cell(spot.y())));
            // The 2 stitches cover as they now lie, pulled in as they were: those near a spot above the cap.
            let near: Vec<(Ribbon, SewnStitch)> =
                pulled.removed.iter().zip(pulled.added).filter_map(|(was, now)| Some((self.ribbons.kill((was.block, was.index))?, now))).collect();
            for (ribbon, now) in near {
                self.ribbons.add(Ribbon::of((now.from, now.to), ribbon.cut, ribbon.at, ribbon.element).firm(ribbon.firm), meter)?;
            }
        }
        self.pulled = [self.moves.len(), self.places.len()];
        Ok(())
    }

    /// How entry `point`, moved `times` before, would move in, by the share of its column's next turn: `None`
    /// when it may not. It must be a top point of a column of the rules' `pulls` between 2 stitches of its
    /// column, one of which passes within [`NEAR`] of `spot`; closer than its column's spacing to the point
    /// before or after it on its rail; not both of whose neighbours there have moved (`moves`) while it
    /// hasn't; and its 2 stitches, moved, no shorter than the machine's shortest and no longer than its
    /// longest. It moves from where it was before it first moved towards the middle of its 2 stitches' other
    /// ends, by the share of the way: a quarter, and later half, as a column moves its crowded points in.
    pub(super) fn pulled(
        &self,
        point: Entry,
        times: usize,
        moves: &BTreeMap<Entry, (usize, Point)>,
        spot: Point,
        meter: &mut Meter,
    ) -> Result<Option<Move>, Exhausted> {
        let pass = &self.pass;
        let (block, index) = point;
        let Some((here, role, element)) = pass.stitch(point) else { return Ok(None) };
        let Some(column) = element.and_then(|element| pass.rules.pulls.get(&element)) else { return Ok(None) };
        let Some(&share) = column.shares.get(times) else { return Ok(None) };
        let ours = |at: usize| pass.stitch((block, at)).is_some_and(|(_, role, other)| role == Role::Top && other == element);
        let (Some(from), Some(to)) = (pass.beside(point, false, meter)?, pass.beside(point, true, meter)?) else { return Ok(None) };
        let (Some(into), Some(out)) = (pass.sewn(block, from, index), pass.sewn(block, index, to)) else { return Ok(None) };
        let place = pass.thickness.placed(spot);
        if role != Role::Top || !ours(from) || !ours(to) || (to_side(place, into.from, into.to) > NEAR && to_side(place, out.from, out.to) > NEAR) {
            return Ok(None);
        }
        // Its neighbours on its rail: 2 points before it and after it in its run.
        let before = pass.beside((block, from), false, meter)?.filter(|at| ours(*at));
        let after = pass.beside((block, to), true, meter)?.filter(|at| ours(*at));
        let apart = |at: Option<usize>| at.and_then(|at| pass.stitch((block, at))).map_or(f64::INFINITY, |(there, _, _)| there.distance(here));
        let nearest = apart(before).min(apart(after));
        if at_least(nearest, column.spacing * (1.0 + OFF)) {
            return Ok(None);
        }
        let moved = [before, after].into_iter().flatten().filter(|at| moves.contains_key(&(block, *at))).count();
        if times == 0 && moved == 2 {
            return Ok(None);
        }
        let middle = into.from.lerp(out.to, 0.5);
        let first = moves.get(&point).map_or(here, |(_, first)| *first);
        let to_place = pass.thickness.placed(pass.thickness.point(first.lerp(middle, share)));
        let added = [SewnStitch { to: to_place, ..into }, SewnStitch { from: to_place, ..out }];
        let (first, last, now) = (pass.at((block, from)), pass.at((block, to)), Some(pass.thickness.point(to_place)));
        if !pass.long_enough(&[(first, now, false, f64::INFINITY), (now, last, false, f64::INFINITY)]) {
            return Ok(None);
        }
        let knot = !at_least(nearest, column.spacing * KNOT);
        Ok(Some(Move { point, removed: [into, out], added, from: here, to: to_place, knot, element }))
    }

    /// Whether the new end of `pulled`'s point shows: no top stitch of a shape sewn after its column covers
    /// the first [`END`] of its way back to where it was, as the pull moves that stitch in. A unit of work for
    /// each place looked at, and for each stitch looked at there.
    pub(super) fn shows(&self, pulled: &Move, meter: &mut Meter) -> Result<bool, Exhausted> {
        let [_, out] = pulled.removed;
        let (after, element) = ((out.block, out.index), pulled.element);
        let back = pulled.from.distance(pulled.to);
        let end = SewnStitch { from: pulled.to, to: pulled.to.lerp(pulled.from, (END / back).min(1.0)), ..out };
        Ok(!self.ribbons.covers(&end, |ribbon: &Ribbon| ribbon.at > after && ribbon.element != element, false, meter)?)
    }

    /// How many fills `pulled` would take from `spot`, which sews `was`: the measure with the moved stitches,
    /// and back.
    pub(super) fn lowers(&mut self, pulled: &Move, was: f64, spot: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
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
