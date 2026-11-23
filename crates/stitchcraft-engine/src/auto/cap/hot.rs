//! The spots the cap goes on to (`REQ-AUTO-194`, `docs/src/design/image-to-embroidery.md` › The cap below the
//! limit): where the pull-in or the last resort goes on past a spot it can't lower, it works only on the spots
//! near its points' stitches, thickest first.
//!
//! A point lowers a spot only by taking thread out of it, so only a spot that one of its stitches comes near can
//! be lowered by it. A design whose shapes sewn by their own settings sew far above the limit has thousands of
//! spots that no point of the cap's comes near. Going through every spot of the measure for each of them, and
//! through every point for each spot, cost a test sheet 25 times its plan's work, and a test sheet has no room
//! for any more work at all. So the points near a spot are found from the top stitches the cap already files by
//! cells for their cover ([`Ribbons`]), in the cell of the spot and those round it, which reach farther than
//! [`NEAR`], and the spots above the level with such a stitch near are kept in order of their thread. A spot
//! whose thread changed is put back in its new place when it comes up. The order is the measure's: of 2 spots as
//! thick, the later one first, as [`Thickness::most`] names it. The points looked at for a spot are those the
//! cap looked at before it went on past spots, of those whose stitches come within [`NEAR`] of it, in the same
//! order, so the cap moves and takes the same points.

use std::collections::BTreeSet;

use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_plan::thickness::{Measure, Spot, Thickness};
use stitchcraft_plan::{ElementRef, SewnStitch};

use super::{Ribbons, cell, units};
use crate::auto::thin::{Entry, Pass};

/// The spots above a level that a set of points may lower, thickest first.
#[derive(Debug, Default)]
pub(super) struct Hot {
    /// The spots, by their thread's bits and their place in the measure: a spot's thread is never under 0, so
    /// the bits keep the order of the values.
    queue: BTreeSet<(u64, usize)>,
    /// The points that may be moved or taken.
    points: BTreeSet<Entry>,
    /// Their elements, by their places in the plan's elements: only top stitches of these can be theirs.
    elements: Vec<bool>,
    /// The spots none of the points lowered.
    stuck: BTreeSet<usize>,
    /// The level: a spot at it or under it is done.
    level: f64,
}

impl Hot {
    /// The spots above `level` of `pass`'s measure with a top stitch of the elements of `points` filed in
    /// `ribbons` near them. A unit of work for each point and each spot of the measure, and for each cell
    /// looked at.
    pub(super) fn of(pass: &Pass, ribbons: &Ribbons, points: &[Entry], level: f64, meter: &mut Meter) -> Result<Hot, Exhausted> {
        meter.charge(units(points.len()))?;
        let mut hot = Hot { points: points.iter().copied().collect(), level, ..Hot::default() };
        for element in points.iter().filter_map(|point| pass.stitch(*point)).filter_map(|(_, _, element)| element) {
            let at = element.index();
            if hot.elements.len() <= at {
                hot.elements.resize(at.saturating_add(1), false);
            }
            if let Some(ours) = hot.elements.get_mut(at) {
                *ours = true;
            }
        }
        // The cells with a live top stitch of the points' elements filed in them, or in a cell beside them.
        let mut near = BTreeSet::new();
        for (&(x, y), filed) in &ribbons.cells {
            meter.charge(units(filed.len()))?;
            if filed.iter().filter_map(|index| ribbons.ribbons.get(*index)).any(|ribbon| ribbon.alive && hot.ours(ribbon.element)) {
                near.extend((x - 1..=x + 1).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))));
            }
        }
        let thickness = &*pass.thickness;
        let values = thickness.values(Measure::Fills);
        meter.charge(units(values.len()))?;
        for (at, _) in values.iter().enumerate().filter(|(_, value)| **value > level) {
            let Some(spot) = thickness.at_spot(Measure::Fills, at) else { continue };
            let place = thickness.placed(spot.at);
            if near.contains(&(cell(place.x()), cell(place.y()))) {
                hot.queue.insert((spot.value.to_bits(), at));
            }
        }
        Ok(hot)
    }

    /// The live top stitches of the points' elements filed in the cell of `spot`, a spot of `thickness` in the
    /// plan's millimetres, and in the 8 cells round it, by their entries. A unit of work for each stitch looked
    /// at.
    fn stitches(&self, thickness: &Thickness, ribbons: &Ribbons, spot: Point, meter: &mut Meter) -> Result<BTreeSet<Entry>, Exhausted> {
        let at = thickness.placed(spot);
        let (x, y) = (cell(at.x()), cell(at.y()));
        let mut found = BTreeSet::new();
        for cell in (x - 1..=x + 1).flat_map(|x| (y - 1..=y + 1).map(move |y| (x, y))) {
            for ribbon in ribbons.cells.get(&cell).into_iter().flatten().filter_map(|index| ribbons.ribbons.get(*index)) {
                meter.charge(1)?;
                if ribbon.alive && self.ours(ribbon.element) {
                    found.insert(ribbon.at);
                }
            }
        }
        Ok(found)
    }

    /// The points near `spot`, a spot of `pass`'s measure in the plan's millimetres: of the points, those at
    /// either end of a top stitch [`Hot::stitches`] finds, in the plan's order. A unit of work for each stitch
    /// and each point looked at.
    pub(super) fn near(&self, pass: &Pass, ribbons: &Ribbons, spot: Point, meter: &mut Meter) -> Result<Vec<Entry>, Exhausted> {
        let mut near = BTreeSet::new();
        for (block, index) in self.stitches(pass.thickness, ribbons, spot, meter)? {
            let start = pass.beside((block, index), false, meter)?.map(|start| (block, start));
            near.extend([Some((block, index)), start].into_iter().flatten().filter(|point| self.points.contains(point)));
        }
        Ok(near.into_iter().collect())
    }

    /// Puts each spot above the level that `stitches`, as the plan now sews them, lay thread in in its place. A
    /// unit of work for each step along each stitch.
    pub(super) fn add(&mut self, thickness: &Thickness, stitches: &[SewnStitch], meter: &mut Meter) -> Result<(), Exhausted> {
        for stitch in stitches {
            for (spot, fills) in thickness.spots(stitch, meter)? {
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

    /// Whether `element` is one of the points'.
    fn ours(&self, element: Option<ElementRef>) -> bool {
        element.is_some_and(|element| self.elements.get(element.index()).copied().unwrap_or(false))
    }

    /// Notes that no point lowered the spot `at`: it isn't looked at again.
    pub(super) fn stuck(&mut self, at: usize) {
        self.stuck.insert(at);
    }
}
