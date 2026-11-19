//! The last pass over a design digitized automatically: thread taken where a spot still sews above the
//! cap (`REQ-AUTO-074`, `docs/src/design/image-to-embroidery.md` › The thickest spot).
//!
//! The rules of digitizing lower what a single shape piles up by itself, but where 5 to 8 elements meet at
//! one spot, each laying a little, no rule of one shape knows the sum. So the finished plan is measured,
//! and at each spot above the cap (`auto_thick_cap_fills`) the pass takes out thread that is hidden or does no visible work, in this
//! order: travel that goes out and back on itself; lock stitches beyond one back and forth; the base
//! layer's row ends under places that later shapes cover; and a shape's row ends under later shapes, moved
//! back along their rows. It never takes a top stitch that shows, and never cuts a run of stitches, which
//! would leave a jump thread: thread leaves a spot only where the stitching doubles back on itself, or where
//! the end of a row can move.
//!
//! **Hidden** is read from the later shapes' own top thread: a place is hidden when the top stitches of
//! the elements sewn after it lay at least [`COVERED`] fill there, measured as the plan is measured. The
//! pass goes through the plan from its end, so what it takes from a later shape is gone before an earlier
//! one is judged hidden under it.
//!
//! What the pass leaves above the cap, the cap at the end of planning takes ([`super::cap`]), with the
//! helpers of this pass: it marks needle points gone and keeps the measure up to date in the same way.
//!
//! **One measure.** The measure that the warning `SC-W0913` gives is made once. The pass keeps it up to
//! date stitch by stitch (`Thickness::add`, `Thickness::remove`) and hands it on, so that a design is not
//! measured twice, since VectorCraft's export has few instructions to spare. Only the stitches near a spot
//! above the cap are looked at again (`Thickness::near`).

use std::collections::BTreeSet;

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Exhausted, Meter, Point};
use stitchcraft_plan::invariants::LOCK_MIN_STITCH;
use stitchcraft_plan::thickness::{Measure, Over, Spot, Thickness};
use stitchcraft_plan::{ElementRef, Role, SewnStitch, StitchKind, StitchPlan};

/// How close to where it was 2 needle points before the stitching must come back to have gone out and back
/// on itself, in millimetres: a lane of the measure, a quarter of the thread's width.
pub const BACK: f64 = 0.1;

/// The longest turn between 2 rows at a row's end, in millimetres: the rows of the open base layer lie
/// 0.8 mm apart.
pub const TURN: f64 = 1.0;

/// How nearly opposite 2 rows run beside a turn: the cosine of the angle between them is this or less.
const OPPOSITE: f64 = -0.9;

/// How many fills of later shapes' top thread hide a place.
pub const COVERED: f64 = 1.0;

/// How far round a spot above the cap the stitches are looked at again, in millimetres: a row's end that
/// moves lies within a window and the overlap of the spot.
const NEAR: f64 = 2.0;

/// The shortest move of a row's end worth making, in millimetres: a step of the machine's grid.
const LEAST_MOVE: f64 = 0.1;

/// The kinds of thread the pass takes, in the order it takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// Travel that goes out and back on itself.
    Travel,
    /// Lock stitches beyond one back and forth.
    Locks,
    /// The base layer's row ends under places later shapes cover.
    Base,
    /// A shape's row ends under later shapes.
    Rims,
}

impl Kind {
    /// Every kind, in order.
    const ALL: [Kind; 4] = [Kind::Travel, Kind::Locks, Kind::Base, Kind::Rims];

    /// Its place among [`Kind::ALL`].
    const fn at(self) -> usize {
        match self {
            Kind::Travel => 0,
            Kind::Locks => 1,
            Kind::Base => 2,
            Kind::Rims => 3,
        }
    }
}

/// What the pass took from a plan.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Thinned {
    /// The thread taken of each kind, by [`Kind::at`], in millimetres.
    pub(crate) taken: [f64; 4],
    /// How many times thread of each kind was taken, by [`Kind::at`].
    pub(crate) times: [usize; 4],
    /// What was above the cap before.
    pub(crate) above: Over,
    /// What is above the cap after.
    pub(crate) left: Over,
    /// The thickest spot after.
    pub(crate) thickest: Spot,
}

impl Thinned {
    /// Whether it took any thread.
    pub(crate) fn took(&self) -> bool {
        self.times.iter().any(|times| *times > 0)
    }
}

/// What the pass, and the cap after it, need to know of a design.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Rules<'r> {
    /// The most thread, in fills, that a spot of a millimetre may sew (`auto_thick_cap_fills`,
    /// `REQ-AUTO-099`): the pass takes thread where a spot has more.
    pub(crate) cap: f64,
    /// The base layer's pieces, by their places in the plan's elements.
    pub(crate) base: &'r BTreeSet<ElementRef>,
    /// The satin columns that digitizing made, by their places in the plan's elements: their top stitches are
    /// the surface a person sees, and the thread under their edges keeps those covered once the fabric pulls
    /// them in (`REQ-AUTO-112`, `REQ-AUTO-114`).
    pub(crate) columns: &'r BTreeSet<ElementRef>,
    /// How far a row's end may move back along its row, in millimetres: the overlap of areas.
    pub(crate) overlap: f64,
    /// The machine's shortest stitch, in millimetres, which no stitch the pass shortens goes under.
    pub(crate) shortest: f64,
    /// The machine's longest stitch, in millimetres, which no stitch the pass joins goes over.
    pub(crate) longest: f64,
}

/// Takes thread from `plan` where `thickness`, its measure, has a spot above the cap of `rules`, and keeps the
/// measure up to date; `None` when no spot is above the cap. A unit of work for each stitch looked at, and each
/// step along a stitch measured.
pub(crate) fn thinned(plan: &mut StitchPlan, thickness: &mut Thickness, rules: &Rules, meter: &mut Meter) -> Result<Option<Thinned>, Exhausted> {
    if thickness.most(Measure::Fills).value <= rules.cap {
        return Ok(None);
    }
    let above = thickness.over(Measure::Fills, rules.cap, meter)?;
    let mut pass = Pass::new(plan, thickness, rules);
    // The stitches near a spot above the cap are found once: the pass only lowers spots, so those of the
    // spots above it later are among them.
    let near = pass.near(meter)?;
    pass.doubled_back(&near, meter)?;
    for kind in [Kind::Base, Kind::Rims] {
        pass.row_ends(kind, &near, meter)?;
    }
    let Pass { plan, thickness, gone, taken, times, .. } = pass;
    taken_out(plan, &gone);
    thickness.resewn(plan, meter)?;
    let left = thickness.over(Measure::Fills, rules.cap, meter)?;
    Ok(Some(Thinned { taken, times, above, left, thickest: thickness.most(Measure::Fills) }))
}

/// Takes the entries `gone`, by block and index, out of `plan`.
pub(super) fn taken_out(plan: &mut StitchPlan, gone: &BTreeSet<Entry>) {
    // Only the blocks that lost a stitch are gone through again.
    let blocks: BTreeSet<usize> = gone.iter().map(|(block, _)| *block).collect();
    for (block, stitches) in plan.blocks.iter_mut().enumerate().filter(|(block, _)| blocks.contains(block)) {
        let mut index = 0;
        stitches.stitches.retain(|_| {
            index += 1;
            !gone.contains(&(block, index - 1))
        });
    }
}

/// A plan being thinned: entries are marked gone, and moved in place.
pub(super) struct Pass<'p, 'r> {
    plan: &'p mut StitchPlan,
    /// The plan's measure, kept up to date.
    pub(super) thickness: &'p mut Thickness,
    /// What the pass needs to know of the design.
    pub(super) rules: &'p Rules<'r>,
    /// The entries taken out, by block and index.
    pub(super) gone: BTreeSet<(usize, usize)>,
    taken: [f64; 4],
    times: [usize; 4],
}

/// An entry of the plan, by block and index.
pub(super) type Entry = (usize, usize);

impl<'p, 'r> Pass<'p, 'r> {
    /// A pass over `plan`, whose measure is `thickness`, that has taken nothing yet.
    pub(super) fn new(plan: &'p mut StitchPlan, thickness: &'p mut Thickness, rules: &'p Rules<'r>) -> Pass<'p, 'r> {
        Pass { plan, thickness, rules, gone: BTreeSet::new(), taken: [0.0; 4], times: [0; 4] }
    }

    /// The pass's plan, its measure and the entries it took out.
    pub(super) fn ended(self) -> (&'p mut StitchPlan, &'p mut Thickness, BTreeSet<Entry>) {
        (self.plan, self.thickness, self.gone)
    }
}

impl Pass<'_, '_> {
    /// The entry `at` when it is a stitch still sewn: its needle point's place on the measure's grid
    /// (`Thickness::placed`), its role and its element.
    pub(super) fn stitch(&self, (block, index): Entry) -> Option<(Point, Role, Option<ElementRef>)> {
        let entry = self.plan.blocks.get(block)?.stitches.get(index)?;
        (entry.kind == StitchKind::Normal && !self.gone.contains(&(block, index)))
            .then(|| (self.thickness.placed(entry.at), entry.origin.role, entry.origin.element))
    }

    /// The stitch still sewn next to `at` in its run, after it when `later`, else before it: a jump or a
    /// trim ends a run, a stop does not. A unit of work for each entry passed.
    pub(super) fn beside(&self, (block, index): Entry, later: bool, meter: &mut Meter) -> Result<Option<usize>, Exhausted> {
        let Some(stitches) = self.plan.blocks.get(block).map(|block| &block.stitches) else { return Ok(None) };
        let mut at = index;
        loop {
            meter.charge(1)?;
            at = match (later, at.checked_sub(1)) {
                (true, _) => at + 1,
                (false, Some(before)) => before,
                (false, None) => return Ok(None),
            };
            let Some(entry) = stitches.get(at) else { return Ok(None) };
            match entry.kind {
                StitchKind::Normal if !self.gone.contains(&(block, at)) => return Ok(Some(at)),
                StitchKind::Jump | StitchKind::Trim => return Ok(None),
                StitchKind::Normal | StitchKind::Stop => {}
            }
        }
    }

    /// Where entry `at` puts its needle point in the plan, as the plan's checks measure it.
    fn at(&self, (block, index): Entry) -> Option<Point> {
        self.plan.blocks.get(block)?.stitches.get(index).map(|entry| entry.at)
    }

    /// Whether every stitch between the needle points of `stitches`, as the plan has them, is as long as a
    /// plan's checks ask, the machine's shortest stitch or a lock's (`LOCK_MIN_STITCH`) for a stitch into or
    /// out of a lock point, given with it, or as the stitch it takes the place of when that was shorter, and
    /// no longer than the machine's longest: the pass makes no stitch the machine can't sew.
    fn long_enough(&self, stitches: &[(Option<Point>, Option<Point>, bool, f64)]) -> bool {
        stitches.iter().all(|&(from, to, lock, was)| {
            let least = if lock { LOCK_MIN_STITCH.get() } else { self.rules.shortest };
            from.zip(to)
                .map(|(from, to)| from.distance(to))
                .is_some_and(|length| at_least(length, least.min(was)) && at_least(self.rules.longest, length))
        })
    }

    /// The stitch from entry `from` to entry `to` of `block`, as it is measured.
    pub(super) fn sewn(&self, block: usize, from: usize, to: usize) -> Option<SewnStitch> {
        let ((start, from_role, _), (end, role, _)) = (self.stitch((block, from))?, self.stitch((block, to))?);
        Some(SewnStitch { block, index: to, from: start, to: end, role, from_role })
    }

    /// The 3 stitches between the needle points of the entries `at` of `block`, in turn, as they are measured.
    fn three(&self, block: usize, [a, b, c, d]: [usize; 4]) -> Option<[SewnStitch; 3]> {
        Some([self.sewn(block, a, b)?, self.sewn(block, b, c)?, self.sewn(block, c, d)?])
    }

    /// Takes `removed` out of the measure and puts `added` in, unless that would make a spot that `added`
    /// lays thread in thicker than both the cap and what it was: then the measure stays as it was, and
    /// `false` is returned. Fills within [`LENGTH_SLACK`](stitchcraft_core::units::LENGTH_SLACK) of each
    /// other count as equal, as lengths do.
    pub(super) fn changed(&mut self, removed: &[SewnStitch], added: &[SewnStitch], meter: &mut Meter) -> Result<bool, Exhausted> {
        let hottest = |thickness: &Thickness, meter: &mut Meter| -> Result<f64, Exhausted> {
            added.iter().try_fold(0.0_f64, |most, stitch| Ok(most.max(thickness.hottest(stitch, meter)?)))
        };
        let before = hottest(self.thickness, meter)?;
        for stitch in removed {
            self.thickness.remove(stitch, meter)?;
        }
        for stitch in added {
            self.thickness.add(stitch, meter)?;
        }
        let after = hottest(self.thickness, meter)?;
        if at_least(self.rules.cap.max(before), after) {
            return Ok(true);
        }
        for stitch in added {
            self.thickness.remove(stitch, meter)?;
        }
        for stitch in removed {
            self.thickness.add(stitch, meter)?;
        }
        Ok(false)
    }

    /// Counts `removed` less `added` as thread of `kind` taken.
    fn took(&mut self, kind: Kind, removed: &[SewnStitch], added: &[SewnStitch]) {
        let length = |stitches: &[SewnStitch]| stitches.iter().map(SewnStitch::length).sum::<f64>();
        if let (Some(taken), Some(times)) = (self.taken.get_mut(kind.at()), self.times.get_mut(kind.at())) {
            *taken += length(removed) - length(added);
            *times += 1;
        }
    }

    /// The stitches of the plan as measured, near a spot above the cap, by their entries.
    fn near(&self, meter: &mut Meter) -> Result<Vec<SewnStitch>, Exhausted> {
        let zone = self.thickness.near(self.rules.cap, NEAR, meter)?;
        let sewn = self.thickness.sewn();
        meter.charge(u64::try_from(sewn.len()).unwrap_or(u64::MAX))?;
        Ok(sewn.iter().filter(|stitch| zone.touches(stitch)).copied().collect())
    }

    /// Takes travel that goes out and back on itself, and lock stitches beyond one back and forth, where they
    /// lay thread in a spot above the cap: the 2 needle points of the way out and back go, and the stitch
    /// before them runs on to the one after, when that is as long as the machine's shortest stitch.
    fn doubled_back(&mut self, near: &[SewnStitch], meter: &mut Meter) -> Result<(), Exhausted> {
        for &stitch in near {
            let (block, here) = (stitch.block, stitch.index);
            let Some((_, role, _)) = self.stitch((block, here)) else { continue };
            let kind = match role {
                Role::Travel => Kind::Travel,
                Role::Lock => Kind::Locks,
                Role::Top | Role::Underlay | Role::Command => continue,
            };
            let Some(before) = self.beside((block, here), false, meter)? else { continue };
            let Some(back) = self.beside((block, here), true, meter)? else { continue };
            let Some(on) = self.beside((block, back), true, meter)? else { continue };
            let Some([out, turn, next]) = self.three(block, [before, here, back, on]) else { continue };
            let same = self.stitch((block, back)).is_some_and(|(_, other, _)| other == role);
            if !same || out.from.distance(turn.to) > BACK || (kind == Kind::Locks && self.locks((block, here), meter)? < 4) {
                continue;
            }
            if self.thickness.hottest(&out, meter)? <= self.rules.cap
                || !self.long_enough(&[(
                    self.at((block, before)),
                    self.at((block, on)),
                    out.from_role == Role::Lock || next.role == Role::Lock,
                    f64::INFINITY,
                )])
            {
                continue;
            }
            let removed = [out, turn, next];
            let added = [SewnStitch { from: out.from, from_role: out.from_role, ..next }];
            if self.changed(&removed, &added, meter)? {
                self.gone.extend([(block, here), (block, back)]);
                self.took(kind, &removed, &added);
            }
        }
        Ok(())
    }

    /// How many stitches of the lock that entry `at` belongs to are still sewn: the lock stitches before and
    /// after it in its run, without a stitch of another role between. A unit of work for each entry passed.
    fn locks(&self, (block, at): Entry, meter: &mut Meter) -> Result<usize, Exhausted> {
        let mut count = 1;
        for later in [false, true] {
            let mut here = at;
            while let Some(next) = self.beside((block, here), later, meter)? {
                if !self.stitch((block, next)).is_some_and(|(_, role, _)| role == Role::Lock) {
                    break;
                }
                count += 1;
                here = next;
            }
        }
        Ok(count)
    }

    /// Moves the row ends of `kind`, the base layer's or the other shapes', back along their rows where they
    /// lie under later shapes and lay thread in a spot above the cap. The plan is gone through from its end:
    /// the later shapes' top stitches, as the pass leaves them, are measured on a blank of the measure as it
    /// goes, and an element's own are added once its stitches are passed.
    fn row_ends(&mut self, kind: Kind, near: &[SewnStitch], meter: &mut Meter) -> Result<(), Exhausted> {
        let mut cover = self.thickness.blank();
        let mut run: Vec<Entry> = Vec::new();
        let mut owner = None;
        for stitch in near.iter().rev() {
            let element =
                self.plan.blocks.get(stitch.block).and_then(|block| block.stitches.get(stitch.index)).and_then(|entry| entry.origin.element);
            if owner != Some((stitch.block, element)) {
                self.covering(&mut cover, &run, meter)?;
                run.clear();
                owner = Some((stitch.block, element));
            }
            run.push((stitch.block, stitch.index));
            self.row_end((stitch.block, stitch.index), kind, &cover, meter)?;
        }
        Ok(())
    }

    /// Adds the top stitches that end at the entries of `run`, as they are now, to `cover`.
    fn covering(&self, cover: &mut Thickness, run: &[Entry], meter: &mut Meter) -> Result<(), Exhausted> {
        for &(block, at) in run {
            // A satin column's thread hides no row end: a row's end under a column is the overlap that keeps the
            // column's edge covered once the fabric pulls it in (`REQ-AUTO-114`).
            let column = |element: Option<ElementRef>| element.is_some_and(|element| self.rules.columns.contains(&element));
            if !self.stitch((block, at)).is_some_and(|(_, role, element)| role == Role::Top && !column(element)) {
                continue;
            }
            if let Some(stitch) = self.beside((block, at), false, meter)?.and_then(|before| self.sewn(block, before, at)) {
                cover.add(&stitch, meter)?;
            }
        }
        Ok(())
    }

    /// Moves the row end at entry `at` and the next row's start back along their rows, when `at` is the end
    /// of a row of `kind` whose turn lies under `cover` and lays thread in a spot above the cap. Rows lie
    /// on each side of a turn of up to [`TURN`], running nearly opposite ways; each moves by the overlap at
    /// the most, or half as far when only that is hidden, and every stitch keeps the machine's shortest.
    fn row_end(&mut self, (block, at): Entry, kind: Kind, cover: &Thickness, meter: &mut Meter) -> Result<(), Exhausted> {
        let Some((end, role, element)) = self.stitch((block, at)) else { return Ok(()) };
        let based = element.is_some_and(|element| self.rules.base.contains(&element));
        let wanted = match kind {
            Kind::Base => based && role == Role::Underlay,
            _ => !based && role == Role::Top,
        };
        if !wanted {
            return Ok(());
        }
        let Some(from) = self.beside((block, at), false, meter)? else { return Ok(()) };
        let Some(start) = self.beside((block, at), true, meter)? else { return Ok(()) };
        let Some(to) = self.beside((block, start), true, meter)? else { return Ok(()) };
        let same = [from, start, to].iter().all(|&entry| self.stitch((block, entry)).is_some_and(|(_, r, e)| r == role && e == element));
        let Some([row, turn, next]) = self.three(block, [from, at, start, to]) else { return Ok(()) };
        let (into, out) = (row.length(), next.length());
        // A row of no length leaves no reach, so no move divides by its length.
        let reach = self.rules.overlap.min(into - self.rules.shortest).min(out - self.rules.shortest);
        if !same || turn.length() > TURN || !opposite(&row, &next) || reach < LEAST_MOVE {
            return Ok(());
        }
        if self.thickness.hottest(&turn, meter)? <= self.rules.cap {
            return Ok(());
        }
        let hidden = |place: Point| cover.at_place(Measure::Fills, place) >= COVERED;
        // A place between 2 others, on the grid.
        let between = |place: Point| self.thickness.placed(self.thickness.point(place));
        let moved = |reach: f64| (between(row.to.lerp(row.from, reach / into)), between(next.from.lerp(next.to, reach / out)));
        let Some((back, on)) = [reach, reach / 2.0].into_iter().map(moved).find(|(back, on)| [end, turn.to, *back, *on].into_iter().all(hidden))
        else {
            return Ok(());
        };
        let (first, last, was) = (self.at((block, from)), self.at((block, to)), self.at((block, at)).zip(self.at((block, start))));
        let was = was.map_or(0.0, |(end, start)| end.distance(start));
        let (to_back, to_on) = (Some(self.thickness.point(back)), Some(self.thickness.point(on)));
        if !self.long_enough(&[(first, to_back, false, f64::INFINITY), (to_back, to_on, false, was), (to_on, last, false, f64::INFINITY)]) {
            return Ok(());
        }
        let removed = [row, turn, next];
        let added = [SewnStitch { to: back, ..row }, SewnStitch { from: back, to: on, ..turn }, SewnStitch { from: on, ..next }];
        if self.changed(&removed, &added, meter)? {
            for (index, point) in [(at, back), (start, on)] {
                if let Some(entry) = self.plan.blocks.get_mut(block).and_then(|block| block.stitches.get_mut(index)) {
                    entry.at = self.thickness.point(point);
                }
            }
            self.took(kind, &removed, &added);
        }
        Ok(())
    }
}

/// Whether stitches `a` and `b` run nearly opposite ways: the cosine of the angle between them is
/// [`OPPOSITE`] or less. A stitch of no length runs every way.
fn opposite(a: &SewnStitch, b: &SewnStitch) -> bool {
    let dot = (a.to.x() - a.from.x()) * (b.to.x() - b.from.x()) + (a.to.y() - a.from.y()) * (b.to.y() - b.from.y());
    dot <= OPPOSITE * a.length() * b.length()
}

/// Every kind's thread taken, in words, for the note: those of none are left out.
pub(crate) fn kinds(thinned: &Thinned) -> String {
    let names = ["travel sewn out and back", "lock stitches", "the base layer's row ends", "row ends under later shapes"];
    let said: Vec<String> = Kind::ALL
        .iter()
        .zip(names)
        .filter_map(|(kind, name)| {
            let (taken, times) = (thinned.taken.get(kind.at())?, thinned.times.get(kind.at())?);
            (*times > 0).then(|| format!("{taken:.1} mm of {name} in {}", super::thick::places(*times)))
        })
        .collect();
    said.join(", ")
}

#[cfg(test)]
mod tests;
