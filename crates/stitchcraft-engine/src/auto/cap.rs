//! The cap at the end of planning (`REQ-AUTO-100`, `docs/src/design/image-to-embroidery.md` › The cap at
//! the end of planning): where a spot of a design digitized automatically still sews above the cap once the
//! last pass ([`super::thin`]) is done, needle points leave the plan's runs.
//!
//! The order the blocks sew in moves where fills start and end and which shape reaches under which, so a
//! spot far from any one rule can pile up thread, and no rule of a shape or a line foresees it. The cap
//! looks at the finished plan alone: its stitches with their roles and elements, and the measure. A needle
//! point taken out of a run joins its 2 stitches into one. That takes thread only where the run turns at
//! the point, and it cuts no run, so it adds no jump, trim or thread change, and every needle point left is
//! one of the plan's, in its order.
//!
//! A point goes only where other stitches cover what its stitches covered: a stitch covers the places
//! within half a thread of it, as the bare-fabric measure paints it, so what shows keeps its colour and no
//! fabric is bared. The cap changes nothing of a satin column (`REQ-AUTO-112`, `REQ-AUTO-114`): it takes none
//! of its top stitches, and a column's stitch covers a place only as the fabric pulls the column in, shortened
//! by the overlap of areas at the column's edges, as the edges' cover is measured (`REQ-AUTO-007`). The rows
//! under a column's edge are what keeps it covered once pulled. And where taking a point makes a stitch
//! beside it turn where it ran straight on, the part that the pull then moves in must stay covered. The kinds go in this order, each through the plan from its end, so that a later shape
//! loses its thread before an earlier one is judged under it ([`Kind`]): underlay and travel under later
//! top stitches, a shape's top under later shapes, and a shape's own top where it is doubled.
//!
//! **One measure.** The cap takes the last pass's measure and keeps it up to date stitch by stitch, as the
//! pass does, with the pass's own helpers. Only the stitches near a spot above the cap are looked at, found
//! once: taking thread only lowers spots.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Exhausted, Meter, Point, math};
use stitchcraft_plan::thickness::{Measure, Over, PIXEL, STRAIGHT_ON, Spot, THREAD, Thickness, Zone};
use stitchcraft_plan::{ElementRef, Role, SewnStitch, StitchPlan};

use super::thin::{Entry, Pass, Rules, taken_out};
use crate::normalize::near::to_side;

/// How far from a stitch the places it covers lie, in millimetres: half a thread, as the bare-fabric measure
/// paints a stitch.
pub const RIBBON: f64 = THREAD / 2.0;

/// How far apart the places along a stitch whose cover is checked lie, in millimetres: a pixel of the
/// bare-fabric measure.
const STEP: f64 = PIXEL;

/// How near a spot above the cap a needle point's stitch in or out comes for the point to be looked at, in
/// millimetres: a window of the measure.
const NEAR: f64 = 1.0;

/// How far round a spot above the cap the top stitches that may cover a point's stitches are gathered, in
/// millimetres: [`NEAR`] and the reach of the stitches beside a point. A place farther off counts as not
/// covered, so the cap keeps the point.
const REACH: f64 = 2.5;

/// The least thread a needle point taken out must take, in millimetres: a point on a straight run takes
/// none.
const LEAST: f64 = 0.05;

/// How many times each kind goes through the plan at the most. A point taken out can leave the one before
/// it free to go: a first time took nearly all of it on every design measured.
const ROUNDS: usize = 4;

/// The side of a cell of the cover's index of top stitches, in millimetres.
const CELL: f64 = 1.0;

/// The kinds of needle point the cap takes, in the order it takes them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Kind {
    /// An underlay or travel point, when top stitches sewn after it cover all of the joined stitch: nobody
    /// sees it, and the thread above it already lies flat on the fabric.
    Hidden,
    /// A shape's top point, when the top stitches of shapes sewn after it cover all that its 2 stitches
    /// covered: nobody sees it either, but it is the overlap that keeps fabric from showing between 2
    /// shapes when the fabric pulls, so it comes second.
    Under,
    /// A shape's top point, when its own other top stitches and those of shapes sewn after it cover all that
    /// its 2 stitches covered: it shows, so it goes last, and only where it is doubled.
    Doubled,
}

impl Kind {
    /// Every kind, in order.
    const ALL: [Kind; 3] = [Kind::Hidden, Kind::Under, Kind::Doubled];

    /// Its place among [`Kind::ALL`].
    const fn at(self) -> usize {
        match self {
            Kind::Hidden => 0,
            Kind::Under => 1,
            Kind::Doubled => 2,
        }
    }

    /// Whether a needle point of `role`, and the one after it, may be of this kind.
    fn takes(self, role: Role) -> bool {
        match self {
            Kind::Hidden => matches!(role, Role::Underlay | Role::Travel),
            Kind::Under | Kind::Doubled => role == Role::Top,
        }
    }
}

/// What the cap took from a plan.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Capped {
    /// The thread taken of each kind, by [`Kind::at`], in millimetres.
    pub(crate) taken: [f64; 3],
    /// How many needle points of each kind were taken out, by [`Kind::at`].
    pub(crate) times: [usize; 3],
    /// What is above the cap after.
    pub(crate) left: Over,
    /// The thickest spot after.
    pub(crate) thickest: Spot,
}

impl Capped {
    /// Whether it took any thread.
    pub(crate) fn took(&self) -> bool {
        self.times.iter().any(|times| *times > 0)
    }
}

/// Takes needle points of the elements `made` out of `plan` where `thickness`, its measure, has a spot above
/// the cap of `rules`, and keeps the measure up to date; `None` when no spot is above the cap, or no needle
/// point of the elements `made` is near one. The other
/// elements' stitches, those a shape's settings sew as drawn, stay as they are, and cover and lay thread
/// all the same. So do the top stitches of the satin columns of `rules` (`REQ-AUTO-112`): their underlay is
/// the cap's to take. A unit of work for each stitch
/// and needle point looked at, each step along a stitch measured, and each top stitch looked at for a
/// place's cover. A cap out of work may leave the plan and the measure part changed, so its caller works on
/// copies and keeps what the last pass left.
pub(crate) fn capped(
    plan: &mut StitchPlan,
    thickness: &mut Thickness,
    rules: &Rules,
    made: &BTreeSet<ElementRef>,
    meter: &mut Meter,
) -> Result<Option<Capped>, Exhausted> {
    if thickness.most(Measure::Fills).value <= rules.cap {
        return Ok(None);
    }
    let columns = rules.columns;
    let sewn = thickness.sewn();
    meter.charge(units(sewn.len()))?;
    let element =
        |stitch: &SewnStitch| plan.blocks.get(stitch.block).and_then(|block| block.stitches.get(stitch.index)).and_then(|entry| entry.origin.element);
    let near = thickness.near(rules.cap, NEAR, meter)?;
    // A column's top stitches are the surface a person sees: the cap takes none of them, so it changes
    // neither their angle nor their edge.
    let takes = |stitch: &SewnStitch| {
        element(stitch).is_some_and(|element| made.contains(&element) && !(stitch.role == Role::Top && columns.contains(&element)))
    };
    let points = candidates(sewn, &near, takes);
    // A design whose spots above the cap lie only in shapes sewn by their own settings has nothing the cap
    // may take, and the work of the cover is spared.
    if points.is_empty() {
        return Ok(None);
    }
    let reach = thickness.near(rules.cap, REACH, meter)?;
    meter.charge(units(sewn.len()))?;
    let (mut ribbons, mut longest) = (Ribbons::default(), BTreeMap::new());
    for (at, stitch) in sewn.iter().enumerate() {
        let element = element(stitch);
        let most: &mut f64 = longest.entry((element, stitch.role == Role::Top)).or_default();
        *most = most.max(stitch.length());
        if stitch.role == Role::Top && reach.touches(stitch) {
            // A column's stitch, pulled in by the overlap at each end where the thread turns.
            let before = at.checked_sub(1).and_then(|before| sewn.get(before)).is_some_and(|before| straight_on(before, stitch));
            let after = sewn.get(at.saturating_add(1)).is_some_and(|after| straight_on(stitch, after));
            let column = element.is_some_and(|element| columns.contains(&element));
            let cut = |straight: bool| if straight || !column { 0.0 } else { rules.overlap };
            if let Some(ribbon) = Ribbon::of((stitch.from, stitch.to), (cut(before), cut(after)), (stitch.block, stitch.index), element) {
                ribbons.add(ribbon, meter)?;
            }
        }
    }
    let mut cap = Cap { pass: Pass::new(plan, thickness, rules), ribbons, longest, taken: [0.0; 3], times: [0; 3] };
    for kind in Kind::ALL {
        for _ in 0..ROUNDS {
            if !cap.above(meter)? || cap.round(kind, &points, meter)? == 0 {
                break;
            }
        }
    }
    let Cap { pass, taken, times, .. } = cap;
    let (plan, thickness, gone) = pass.ended();
    taken_out(plan, &gone);
    thickness.resewn(plan, meter)?;
    let left = thickness.over(Measure::Fills, rules.cap, meter)?;
    Ok(Some(Capped { taken, times, left, thickest: thickness.most(Measure::Fills) }))
}

/// The needle points of `sewn`, the plan's stitches, that the cap looks at: those of the elements `made`
/// whose stitch in or out touches `near`, the cells near a spot above the cap, in the plan's order.
fn candidates(sewn: &[SewnStitch], near: &Zone, made: impl Fn(&SewnStitch) -> bool) -> Vec<Entry> {
    let mut points = BTreeSet::new();
    let mut before: Option<&SewnStitch> = None;
    for stitch in sewn {
        if near.touches(stitch) {
            // The point the stitch ends at, and the one it starts at: the end of the stitch before it in its
            // block.
            let start = before.filter(|before| before.block == stitch.block);
            points.extend([Some(stitch), start].into_iter().flatten().filter(|point| made(point)).map(|point| (point.block, point.index)));
        }
        before = Some(stitch);
    }
    points.into_iter().collect()
}

/// The plan being capped, and what is known of it.
struct Cap<'p, 'r> {
    pass: Pass<'p, 'r>,
    /// The top stitches near a spot above the cap, as the cap leaves them.
    ribbons: Ribbons,
    /// The longest stitch of each element, of its top stitches and of the others.
    longest: BTreeMap<(Option<ElementRef>, bool), f64>,
    taken: [f64; 3],
    times: [usize; 3],
}

impl Cap<'_, '_> {
    /// Whether a spot is still above the cap. A unit of work for each spot.
    fn above(&self, meter: &mut Meter) -> Result<bool, Exhausted> {
        let (columns, rows) = self.pass.thickness.size();
        meter.charge(units(columns.saturating_mul(rows)))?;
        Ok(self.pass.thickness.most(Measure::Fills).value > self.pass.rules.cap)
    }

    /// Goes once through `points`, from the plan's end, taking those of `kind` that may go; how many it took.
    fn round(&mut self, kind: Kind, points: &[Entry], meter: &mut Meter) -> Result<usize, Exhausted> {
        let mut count = 0;
        for &point in points.iter().rev() {
            meter.charge(1)?;
            if !self.pass.gone.contains(&point) && self.taken_out(point, kind, meter)? {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Takes the needle point at entry `(block, at)` out of its run when it is of `kind` and may go: its 2
    /// stitches become one.
    fn taken_out(&mut self, (block, at): Entry, kind: Kind, meter: &mut Meter) -> Result<bool, Exhausted> {
        let pass = &self.pass;
        let Some((_, role, element)) = pass.stitch((block, at)) else { return Ok(false) };
        if !kind.takes(role) {
            return Ok(false);
        }
        let (Some(from), Some(to)) = (pass.beside((block, at), false, meter)?, pass.beside((block, at), true, meter)?) else { return Ok(false) };
        let Some((_, next, next_element)) = pass.stitch((block, to)) else { return Ok(false) };
        // An underlay or travel point goes between 2 stitches of either, a top point between 2 of its shape.
        let fits = if kind == Kind::Hidden { kind.takes(next) } else { next == Role::Top && next_element == element };
        let (Some(into), Some(out)) = (pass.sewn(block, from, at), pass.sewn(block, at, to)) else { return Ok(false) };
        if !fits || into.from_role == Role::Lock {
            return Ok(false);
        }
        let joined = SewnStitch { block, index: to, from: into.from, to: out.to, role: next, from_role: into.from_role };
        let (length, was) = (joined.length(), into.length() + out.length());
        let longest = self.longest.get(&(element, role == Role::Top)).copied().unwrap_or(0.0);
        let shortest = pass.rules.shortest.min(into.length()).min(out.length());
        if !at_least(was - length, LEAST) || !at_least(longest, length) || !at_least(length, shortest) {
            return Ok(false);
        }
        // The plan keeps its own needle points, off the machine's grid, and a plan's checks measure those
        // (`REQ-PLAN-002`): there too the joined stitch is no shorter than the machine's shortest or the
        // shorter of the 2, and no longer than the machine's longest.
        let (Some(first), Some(middle), Some(last)) = (pass.at((block, from)), pass.at((block, at)), pass.at((block, to))) else { return Ok(false) };
        let (shorter, joined_length) = (first.distance(middle).min(middle.distance(last)), first.distance(last));
        if !at_least(joined_length, pass.rules.shortest.min(shorter)) || !at_least(pass.rules.longest, joined_length) {
            return Ok(false);
        }
        if pass.thickness.hottest(&into, meter)?.max(pass.thickness.hottest(&out, meter)?) <= pass.rules.cap {
            return Ok(false);
        }
        let after = (block, to);
        let covered = match kind {
            Kind::Hidden => self.ribbons.covers(&joined, |ribbon| ribbon.at > after, meter)?,
            Kind::Under => {
                let counts = |ribbon: &Ribbon| ribbon.at > after && ribbon.element != element;
                self.ribbons.covers(&into, counts, meter)? && self.ribbons.covers(&out, counts, meter)?
            }
            Kind::Doubled => {
                let counts = |ribbon: &Ribbon| ribbon.at != (block, at) && ribbon.at != after && (ribbon.element == element || ribbon.at > after);
                self.ribbons.covers(&into, counts, meter)? && self.ribbons.covers(&out, counts, meter)?
            }
        };
        // A joined top stitch lays its thread only where its shape's own top thread or a later shape's lies: a
        // stitch cut across a bend would show on a neighbour (`REQ-AUTO-026`).
        let covered =
            covered && (kind == Kind::Hidden || self.ribbons.covers(&joined, |ribbon| ribbon.element == element || ribbon.at > after, meter)?);
        // The top stitches beside the 2 that ran straight on into them turn into the joined one, and the pull
        // moves their needle holes in: what they then leave must be covered too.
        let overlap = self.pass.rules.overlap;
        let before = match pass.beside((block, from), false, meter)? {
            Some(start) => {
                pass.sewn(block, start, from).filter(|before| before.role == Role::Top && straight_on(before, &into) && !straight_on(before, &joined))
            }
            None => None,
        };
        let next = match pass.beside((block, to), true, meter)? {
            Some(end) => pass.sewn(block, to, end).filter(|next| next.role == Role::Top && straight_on(&out, next) && !straight_on(&joined, next)),
            None => None,
        };
        let others = |ribbon: &Ribbon| {
            ribbon.at != (block, at)
                && ribbon.at != after
                && Some(ribbon.at) != before.map(|b| (b.block, b.index))
                && Some(ribbon.at) != next.map(|n| (n.block, n.index))
        };
        for (side, start) in [(before, false), (next, true)] {
            let Some(side) = side else { continue };
            let length = side.length();
            let t = if length > 0.0 { (overlap / length).min(1.0) } else { 0.0 };
            let part =
                if start { SewnStitch { to: side.from.lerp(side.to, t), ..side } } else { SewnStitch { from: side.to.lerp(side.from, t), ..side } };
            if !self.ribbons.covers(&part, others, meter)? {
                return Ok(false);
            }
        }
        if !covered || !self.pass.changed(&[into, out], &[joined], meter)? {
            return Ok(false);
        }
        self.pass.gone.insert((block, at));
        if role == Role::Top {
            self.ribbons.kill((block, at));
            self.ribbons.kill(after);
            // The joined stitch turns at both its ends.
            if let Some(ribbon) = Ribbon::of((joined.from, joined.to), (0.0, 0.0), after, element) {
                self.ribbons.add(ribbon, meter)?;
            }
        }
        // The stitches beside it, pulled in where they now turn.
        for (side, start) in [(before, false), (next, true)] {
            let Some(was) = side.and_then(|side| self.ribbons.kill((side.block, side.index))) else { continue };
            let cut = if start { (overlap, was.cut.1) } else { (was.cut.0, overlap) };
            if let Some(ribbon) = Ribbon::of(was.whole, cut, was.at, was.element) {
                self.ribbons.add(ribbon, meter)?;
            }
        }
        if let (Some(taken), Some(times)) = (self.taken.get_mut(kind.at()), self.times.get_mut(kind.at())) {
            *taken += was - length;
            *times += 1;
        }
        Ok(true)
    }
}

/// A top stitch, as it covers the fabric.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ribbon {
    /// Where it starts and ends, pulled in.
    from: Point,
    to: Point,
    /// The stitch as sewn, and how far the pull moves it in at its start and its end.
    whole: (Point, Point),
    cut: (f64, f64),
    /// The entry it ends at.
    at: Entry,
    element: Option<ElementRef>,
    /// Whether it is still sewn: a needle point taken out ends the 2 stitches beside it.
    alive: bool,
}

impl Ribbon {
    /// The stitch from `from` to `to` that ends at entry `at`, of `element`, pulled in by `cut` at its start and
    /// its end; `None` when the pull leaves nothing of it.
    fn of((from, to): (Point, Point), cut: (f64, f64), at: Entry, element: Option<ElementRef>) -> Option<Ribbon> {
        let (start, end) = pulled(from, to, cut.0, cut.1)?;
        Some(Ribbon { from: start, to: end, whole: (from, to), cut, at, element, alive: true })
    }
}

/// The top stitches near a spot above the cap, by the cells of [`CELL`] they cover.
#[derive(Debug, Default)]
struct Ribbons {
    ribbons: Vec<Ribbon>,
    cells: BTreeMap<(i64, i64), Vec<usize>>,
    by_entry: BTreeMap<Entry, usize>,
}

impl Ribbons {
    /// Adds `ribbon` to the cells it covers. A unit of work for each cell.
    fn add(&mut self, ribbon: Ribbon, meter: &mut Meter) -> Result<(), Exhausted> {
        let span = |a: f64, b: f64| (cell(a.min(b) - RIBBON), cell(a.max(b) + RIBBON));
        let ((x0, x1), (y0, y1)) = (span(ribbon.from.x(), ribbon.to.x()), span(ribbon.from.y(), ribbon.to.y()));
        let count = (x1 - x0 + 1).saturating_mul(y1 - y0 + 1);
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
        let at = self.ribbons.len();
        for x in x0..=x1 {
            for y in y0..=y1 {
                self.cells.entry((x, y)).or_default().push(at);
            }
        }
        self.by_entry.insert(ribbon.at, at);
        self.ribbons.push(ribbon);
        Ok(())
    }

    /// Ends the stitch that ends at entry `at`, and gives it back.
    fn kill(&mut self, at: Entry) -> Option<Ribbon> {
        let ribbon = self.by_entry.remove(&at).and_then(|index| self.ribbons.get_mut(index))?;
        ribbon.alive = false;
        Some(*ribbon)
    }

    /// Whether every place along `stitch`, each [`STEP`], lies under a stitch still sewn that `counts`. A
    /// unit of work for each place, and for each stitch looked at.
    fn covers(&self, stitch: &SewnStitch, counts: impl Fn(&Ribbon) -> bool, meter: &mut Meter) -> Result<bool, Exhausted> {
        let steps = whole(stitch.length() / STEP).max(1);
        for step in 0..=steps {
            let place = stitch.from.lerp(stitch.to, step as f64 / steps as f64);
            let near = self.cells.get(&(cell(place.x()), cell(place.y()))).map_or(&[][..], Vec::as_slice);
            meter.charge(units(near.len()).saturating_add(1))?;
            let under = |ribbon: &&Ribbon| ribbon.alive && counts(ribbon) && to_side(place, ribbon.from, ribbon.to) <= RIBBON;
            if !near.iter().filter_map(|index| self.ribbons.get(*index)).any(|ribbon| under(&ribbon)) {
                return Ok(false);
            }
        }
        Ok(true)
    }
}

/// Whether the thread runs straight on from `first` into `next`, the stitch sewn after it: from where it ends,
/// turning by less than [`STRAIGHT_ON`], so that the pull of the fabric doesn't move the needle hole between
/// them.
fn straight_on(first: &SewnStitch, next: &SewnStitch) -> bool {
    let (u, v) = ((first.to.x() - first.from.x(), first.to.y() - first.from.y()), (next.to.x() - next.from.x(), next.to.y() - next.from.y()));
    let lengths = first.length() * next.length();
    first.block == next.block && first.to == next.from && lengths > 0.0 && (u.0 * v.0 + u.1 * v.1) / lengths >= math::cos(STRAIGHT_ON.to_radians())
}

/// The stitch from `from` to `to` shortened by `start` at its start and `end` at its end, in millimetres, as
/// the pull moves its needle holes in; `None` when that leaves nothing of it.
fn pulled(from: Point, to: Point, start: f64, end: f64) -> Option<(Point, Point)> {
    let length = from.distance(to);
    (length > start + end).then(|| (from.lerp(to, start / length), from.lerp(to, (length - end) / length)))
}

/// The cell of the cover's index that `value`, in millimetres, lies in along one axis.
#[allow(clippy::cast_possible_truncation)] // a saturating cast of a whole number: a plan is 500 mm across at most
fn cell(value: f64) -> i64 {
    (value / CELL).floor() as i64
}

/// `value` rounded up to a whole number of steps: 0 below 0 and for NaN.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a saturating cast of a whole number
fn whole(value: f64) -> usize {
    value.ceil().max(0.0) as usize
}

/// `count` as work units.
fn units(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}

/// Every kind's thread taken, in words, for the note: those of none are left out.
pub(crate) fn kinds(capped: &Capped) -> String {
    let names = ["underlay and travel", "top stitches under later shapes", "a shape's own doubled top stitches"];
    let said: Vec<String> = Kind::ALL
        .iter()
        .zip(names)
        .filter_map(|(kind, name)| {
            let (taken, times) = (capped.taken.get(kind.at())?, capped.times.get(kind.at())?);
            (*times > 0).then(|| format!("{taken:.1} mm of {name} in {}", super::thick::places(*times)))
        })
        .collect();
    said.join(", ")
}

#[cfg(test)]
mod tests;
