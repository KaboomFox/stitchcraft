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
//! fabric is bared. The kinds go in this order, each through the plan from its end, so that a later shape
//! loses its thread before an earlier one is judged under it ([`Kind`]): underlay and travel under later
//! top stitches, a shape's top under later shapes, and a shape's own top where it is doubled.
//!
//! **Satin columns** (`REQ-AUTO-112`, `REQ-AUTO-114`). Below the warning's limit the cap changes nothing of
//! a column: it takes none of its top stitches, and a column's stitch covers a place only as the fabric pulls
//! the column in, shortened by the overlap of areas at the column's edges, as the edges' cover is measured
//! (`REQ-AUTO-007`). The rows under a column's edge are what keeps it covered once pulled. And where taking a
//! point makes a stitch beside it turn where it ran straight on, the part that the pull then moves in must
//! stay covered. Above the warning's limit a spot is too thick to sew well, which outweighs an edge: there the
//! 3 kinds go first, judging a column's stitches as drawn, and what is still above it after the cap loses
//! columns' top points, one at a time, those that change their column least.
//!
//! **Lettering** (`REQ-AUTO-119` to `REQ-AUTO-123`). The lettering that digitizing placed is in the cap's
//! reach, and its columns are columns. A font's columns overlap where they join by their own pull
//! compensation, so a lettering column's stitch covers a place only as that pull moves it in, as drawn or not.
//! A lettering column's underlay goes only where another of the lettering's columns covers it too, since it
//! holds up its own column's top elsewhere, and the last resort takes none of a lettering column's points.
//!
//! **Crowded points pulled in** (`REQ-AUTO-171`, the `pull` module). Where a spot still sews above the warning's
//! limit once the 3 kinds are done, crowded top points of the columns that move none in as they are made
//! move in along their stitches, before the last resort takes any point out. Neither stops at a spot it can't
//! lower: each goes on to the next spot above the limit (`REQ-AUTO-194`). With `auto_short_stitches` `knots`,
//! the default, the cap then pulls in the points of tight turns, and points whose new end a later shape
//! covers, below the limit too, down to the cap (`REQ-AUTO-195`).
//!
//! **One measure.** The cap takes the last pass's measure and keeps it up to date stitch by stitch, as the
//! pass does, with the pass's own helpers. Only the stitches near a spot above the cap are looked at, found
//! once: taking thread only lowers spots.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Budget, Exhausted, Meter, Point, math};
use stitchcraft_plan::thickness::{Measure, Over, PIXEL, STRAIGHT_ON, Spot, THREAD, Thickness, Zone};
use stitchcraft_plan::{ElementRef, Role, SewnStitch, StitchPlan};

use self::hot::Hot;
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

/// The least a lettering column's stitch is pulled in at a turn, in millimetres: a font's least pull
/// compensation, for stable fabric (`docs/src/design/image-to-embroidery.md` › The cap and lettering).
pub const LEAST_PULL: f64 = 0.2;

/// How far a lettering column's own pull compensation says the fabric pulls each end of its stitches in
/// (`REQ-AUTO-121`): `pull_compensation_mm`, and `pull_compensation_percent` of the stitch's length, the
/// larger of the 2 sides' each.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Pull {
    /// The length, in millimetres.
    pub(crate) mm: f64,
    /// The share of the stitch's length.
    pub(crate) share: f64,
}

impl Pull {
    /// The pull of a column whose settings of pull compensation are `params`.
    pub(crate) fn of(params: &crate::generators::pull::PullParams) -> Pull {
        let [a, b] = params.pull_compensation_mm.map(|mm| mm.get());
        let [c, d] = params.pull_compensation_percent;
        Pull { mm: a.max(b), share: c.max(d) / 100.0 }
    }

    /// How far a stitch `length` long is pulled in at an end where it turns: never less than [`LEAST_PULL`].
    fn at(self, length: f64) -> f64 {
        (self.mm + self.share * length).max(LEAST_PULL)
    }
}

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
    /// A satin column's top point, as a last resort where a spot still sews above the warning's limit: as a
    /// shape's own doubled top, one point at a time, of those that lower the thickest spot, the one that
    /// changes the column least (`REQ-AUTO-112`).
    Column,
}

impl Kind {
    /// Every kind, in order.
    const ALL: [Kind; 4] = [Kind::Hidden, Kind::Under, Kind::Doubled, Kind::Column];

    /// Its place among [`Kind::ALL`].
    const fn at(self) -> usize {
        match self {
            Kind::Hidden => 0,
            Kind::Under => 1,
            Kind::Doubled => 2,
            Kind::Column => 3,
        }
    }

    /// Whether a needle point of `role`, and the one after it, may be of this kind.
    fn takes(self, role: Role) -> bool {
        match self {
            Kind::Hidden => matches!(role, Role::Underlay | Role::Travel),
            Kind::Under | Kind::Doubled | Kind::Column => role == Role::Top,
        }
    }
}

/// What the cap took from a plan.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Capped {
    /// The thread taken of each kind, by [`Kind::at`], in millimetres.
    pub(crate) taken: [f64; 4],
    /// How many needle points of each kind were taken out, by [`Kind::at`].
    pub(crate) times: [usize; 4],
    /// What is above the cap after.
    pub(crate) left: Over,
    /// The thickest spot after.
    pub(crate) thickest: Spot,
    /// Whether it ran out of work, and kept the points it had taken until then.
    pub(crate) ran_out: bool,
    /// How many satin columns' top points it pulled in, and in how many places (`REQ-AUTO-172`).
    pub(crate) pulled: [usize; 2],
}

impl Capped {
    /// Whether it took any thread.
    pub(crate) fn took(&self) -> bool {
        self.times.iter().any(|times| *times > 0) || self.pulled[0] > 0
    }
}

/// Takes needle points of the elements `made` out of `plan` where `thickness`, its measure, has a spot above
/// the cap of `rules`, and keeps the measure up to date; `None` when no spot is above the cap, or no needle
/// point of the elements `made` is near one. The other
/// elements' stitches, those a shape's settings sew as drawn, stay as they are, and cover and lay thread
/// all the same. So do the top stitches of the satin columns of `rules` (`REQ-AUTO-112`): their underlay is
/// the cap's to take. A unit of work for each stitch
/// and needle point looked at, each step along a stitch measured, and each top stitch looked at for a
/// place's cover, out of `budget`'s work (`REQ-AUTO-101`). A needle point goes only once it is judged whole,
/// so a cap that runs out of work while it takes points keeps those it took, and the plan they leave is
/// measured again with the budget's work once more ([`Capped::ran_out`]). A cap out of work before it
/// takes any, or that can't measure the plan again, may leave the plan and the measure part changed, so its
/// caller works on copies and keeps what the last pass left.
pub(crate) fn capped(
    plan: &mut StitchPlan,
    thickness: &mut Thickness,
    rules: &Rules,
    made: &BTreeSet<ElementRef>,
    budget: &Budget,
) -> Result<Option<Capped>, Exhausted> {
    let mut work = budget.meter();
    let meter = &mut work;
    if thickness.most(Measure::Fills).value <= rules.cap {
        return Ok(None);
    }
    let (columns, lettering) = (rules.columns, rules.lettering);
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
    // Where a spot still sews above the warning's limit, a column's top points near it are the last resort:
    // not a lettering column's, whose look is the font's (`REQ-AUTO-123`).
    let column = |stitch: &SewnStitch| {
        element(stitch).is_some_and(|element| {
            made.contains(&element) && stitch.role == Role::Top && columns.contains(&element) && !lettering.contains_key(&element)
        })
    };
    // Of those, the columns that move no point in as they are made may have their crowded points pulled in
    // there first (`REQ-AUTO-171`).
    let pulls = |stitch: &SewnStitch| column(stitch) && element(stitch).is_some_and(|element| rules.pulls.contains_key(&element));
    let (last, hottest, pulled) = if thickness.most(Measure::Fills).value > rules.limit {
        let near = thickness.near(rules.limit, NEAR, meter)?;
        let [last, hottest, pulled] = candidates(sewn, &near, [&column, &takes, &pulls]);
        (last, hottest, pulled)
    } else {
        (Vec::new(), Vec::new(), Vec::new())
    };
    // And of those, the columns sewn with `knots` below the limit too, near a spot above the cap
    // (`REQ-AUTO-195`).
    let knots =
        |stitch: &SewnStitch| column(stitch) && element(stitch).is_some_and(|element| rules.pulls.get(&element).is_some_and(|pull| pull.knots));
    let [points, low] = candidates(sewn, &near, [&takes, &knots]);
    // A design whose spots above the cap lie only in shapes sewn by their own settings has nothing the cap
    // may take, and the work of the cover is spared.
    if points.is_empty() && last.is_empty() && low.is_empty() {
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
            // A column's stitch, pulled in by the overlap at each end where the thread turns, and a lettering
            // column's by its own pull, as drawn or not (`REQ-AUTO-121`).
            let before = at.checked_sub(1).and_then(|before| sewn.get(before)).is_some_and(|before| straight_on(before, stitch));
            let after = sewn.get(at.saturating_add(1)).is_some_and(|after| straight_on(stitch, after));
            let column = element.is_some_and(|element| columns.contains(&element));
            let pull = element.and_then(|element| lettering.get(&element));
            let by = pull.map_or(rules.overlap, |pull| pull.at(stitch.length()));
            let cut = |straight: bool| if straight || !column { 0.0 } else { by };
            let ribbon = Ribbon::of((stitch.from, stitch.to), (cut(before), cut(after)), (stitch.block, stitch.index), element);
            ribbons.add(ribbon.firm(pull.is_some()), meter)?;
        }
    }
    let mut cap = Cap {
        pass: Pass::new(plan, thickness, rules),
        ribbons,
        longest,
        taken: [0.0; 4],
        times: [0; 4],
        pulled: [0; 2],
        moves: BTreeMap::new(),
        refused: BTreeSet::new(),
        places: BTreeSet::new(),
    };
    let mut ran_out = cap.taking([&hottest, &points, &pulled], &last, meter).is_err();
    // Below the limit, down to the cap, the columns sewn with `knots` (`REQ-AUTO-195`). The measure is kept up to
    // date as above it: no second measure of the plan, whose work the test sheets have no room for.
    if !ran_out {
        ran_out = cap.pull_in(&low, rules.cap, true, meter).is_err();
    }
    let Cap { pass, taken, times, pulled, .. } = cap;
    let (plan, thickness, gone) = pass.ended();
    taken_out(plan, &gone);
    // What is left is measured with the budget's work once more. A cap out of work may have stopped part way
    // through a point's change of the measure: the plan is measured again.
    let mut again = budget.meter();
    if ran_out {
        *thickness = Thickness::of(plan, &mut again)?.ok_or(Exhausted::Work)?;
    } else {
        thickness.resewn(plan, &mut again)?;
    }
    let left = thickness.over(Measure::Fills, rules.cap, &mut again)?;
    Ok(Some(Capped { taken, times, left, thickest: thickness.most(Measure::Fills), ran_out, pulled }))
}

/// The needle points of `sewn`, the plan's stitches, that the cap looks at: for each of `made`, those of its
/// elements whose stitch in or out touches `near`, the cells near a spot above the cap, in the plan's order.
/// The plan is gone through once for them all.
fn candidates<const N: usize>(sewn: &[SewnStitch], near: &Zone, made: [&dyn Fn(&SewnStitch) -> bool; N]) -> [Vec<Entry>; N] {
    let mut points: [BTreeSet<Entry>; N] = std::array::from_fn(|_| BTreeSet::new());
    let mut before: Option<&SewnStitch> = None;
    for stitch in sewn {
        if near.touches(stitch) {
            // The point the stitch ends at, and the one it starts at: the end of the stitch before it in its
            // block.
            let start = before.filter(|before| before.block == stitch.block);
            for (points, made) in points.iter_mut().zip(made) {
                points.extend([Some(stitch), start].into_iter().flatten().filter(|point| made(point)).map(|point| (point.block, point.index)));
            }
        }
        before = Some(stitch);
    }
    points.map(|points| points.into_iter().collect())
}

/// The plan being capped, and what is known of it.
struct Cap<'p, 'r> {
    pass: Pass<'p, 'r>,
    /// The top stitches near a spot above the cap, as the cap leaves them.
    ribbons: Ribbons,
    /// The longest stitch of each element, of its top stitches and of the others.
    longest: BTreeMap<(Option<ElementRef>, bool), f64>,
    taken: [f64; 4],
    times: [usize; 4],
    /// How many points [`Cap::pull_in`] moved, and in how many places.
    pulled: [usize; 2],
    /// How often each point pulled in moved, and where it was before it first did, over every pull-in.
    moves: BTreeMap<Entry, (usize, Point)>,
    /// The points whose next move the measure refused, with how often they had moved.
    refused: BTreeSet<(Entry, usize)>,
    /// The cells of the spots points were pulled in at.
    places: BTreeSet<(i64, i64)>,
}

impl Cap<'_, '_> {
    /// Takes the points the cap may take, with `meter`'s work: above the warning's limit the 3 kinds go
    /// first, of `hottest`, judging a column's cover as drawn; then below it, of `points`, down to the cap,
    /// keeping the thread under a column's edge; then the crowded points of `pulled` are pulled in, and last
    /// the columns' top points `last` are taken, above the limit. Those 2 each stop at the first spot they
    /// can't lower, and then go again, on past such spots (`REQ-AUTO-194`).
    fn taking(&mut self, [hottest, points, pulled]: [&[Entry]; 3], last: &[Entry], meter: &mut Meter) -> Result<(), Exhausted> {
        let rules = self.pass.rules;
        for (points, hot, limit) in [(hottest, true, rules.limit), (points, false, rules.cap)] {
            for kind in [Kind::Hidden, Kind::Under, Kind::Doubled] {
                for _ in 0..ROUNDS {
                    if !self.above(limit, meter)? || self.round(kind, points, hot, meter)? == 0 {
                        break;
                    }
                }
            }
        }
        for skip in [false, true] {
            self.pull_in(pulled, rules.limit, skip, meter)?;
            self.last_resort(last, skip, meter)?;
        }
        Ok(())
    }

    /// Whether a spot is still above `limit`, in fills. A unit of work for each spot.
    fn above(&self, limit: f64, meter: &mut Meter) -> Result<bool, Exhausted> {
        let (columns, rows) = self.pass.thickness.size();
        meter.charge(units(columns.saturating_mul(rows)))?;
        Ok(self.pass.thickness.most(Measure::Fills).value > limit)
    }

    /// Takes satin columns' top points out of `points` while a spot sews above the warning's limit, one at a
    /// time: of those that may go and lower the thickest spot by half as much as the one that lowers it most,
    /// or more, the one whose joined stitch runs across the column, not along an edge, and turns least from the
    /// stitch into it. A unit of work for each point looked at each time. Only a point whose stitches lay
    /// thread at the thickest spot can lower it, and only such a point is judged further, so that each point
    /// taken costs the work of the points round its spot, not of every point near a spot above the limit
    /// (`REQ-AUTO-101`). Where no point lowers the thickest spot, it stops, or with `skip` goes on to the next
    /// thickest (`REQ-AUTO-194`).
    fn last_resort(&mut self, points: &[Entry], skip: bool, meter: &mut Meter) -> Result<(), Exhausted> {
        let limit = self.pass.rules.limit;
        let (mut points, mut taken) = (points.to_vec(), BTreeSet::new());
        // Going on past a spot, only the spots the points' stitches lay thread in are looked at ([`Hot`]), and
        // only while a spot is above the limit.
        let mut hot = if skip && self.above(limit, meter)? { Some(Hot::of(&self.pass, &self.ribbons, &points, limit, meter)?) } else { None };
        if skip && hot.is_none() {
            return Ok(());
        }
        loop {
            let (index, spot, near) = match hot.as_mut() {
                Some(hot) => {
                    let Some((index, spot)) = hot.next(&self.pass, meter)? else { break };
                    (Some(index), spot.at, hot.near(&self.pass, &self.ribbons, spot.at, meter)?)
                }
                None if self.above(limit, meter)? => (None, self.pass.thickness.most(Measure::Fills).at, points.clone()),
                None => break,
            };
            let mut may = Vec::new();
            for point in near.into_iter().filter(|point| !taken.contains(point)) {
                meter.charge(1)?;
                if let Some(change) = self.taken_out(point, Kind::Column, true, Some(spot), meter)? {
                    may.push((change, point));
                }
            }
            let Some(point) = chosen(&may) else {
                let (Some(hot), Some(index)) = (hot.as_mut(), index) else { break };
                hot.stuck(index);
                continue;
            };
            points.retain(|other| *other != point);
            taken.insert(point);
            self.taken_out(point, Kind::Column, true, None, meter)?;
            if let Some(hot) = hot.as_mut() {
                hot.taken(point);
            }
        }
        Ok(())
    }

    /// Goes once through `points`, from the plan's end, taking those of `kind` that may go; how many it took.
    fn round(&mut self, kind: Kind, points: &[Entry], last: bool, meter: &mut Meter) -> Result<usize, Exhausted> {
        let mut count = 0;
        for &point in points.iter().rev() {
            meter.charge(1)?;
            if !self.pass.gone.contains(&point) && self.taken_out(point, kind, last, None, meter)?.is_some() {
                count += 1;
            }
        }
        Ok(count)
    }

    /// Takes the needle point at entry `(block, at)` out of its run when it is of `kind` and may go, its 2
    /// stitches become one, and says how that changes the stitch into it; only says so when `asked`, with
    /// how much it lowers that spot, the thickest. A point whose 2 stitches pass farther than [`NEAR`] from
    /// the spot asked of lays no thread there, and lowers it by nothing: it isn't judged.
    fn taken_out(&mut self, (block, at): Entry, kind: Kind, hot: bool, asked: Option<Point>, meter: &mut Meter) -> Result<Option<Change>, Exhausted> {
        let pass = &self.pass;
        let Some((_, role, element)) = pass.stitch((block, at)) else { return Ok(None) };
        // A point taken out is no stitch any more (`Pass::stitch`).
        if !kind.takes(role) {
            return Ok(None);
        }
        let (Some(from), Some(to)) = (pass.beside((block, at), false, meter)?, pass.beside((block, at), true, meter)?) else { return Ok(None) };
        let Some((_, next, next_element)) = pass.stitch((block, to)) else { return Ok(None) };
        // An underlay or travel point goes between 2 stitches of either, a top point between 2 of its shape.
        let fits = if kind == Kind::Hidden { kind.takes(next) } else { next == Role::Top && next_element == element };
        let (Some(into), Some(out)) = (pass.sewn(block, from, at), pass.sewn(block, at, to)) else { return Ok(None) };
        if let Some(spot) = asked {
            let place = pass.thickness.placed(spot);
            if [&into, &out].iter().all(|stitch| to_side(place, stitch.from, stitch.to) > NEAR) {
                return Ok(None);
            }
        }
        if !fits || into.from_role == Role::Lock {
            return Ok(None);
        }
        let joined = SewnStitch { block, index: to, from: into.from, to: out.to, role: next, from_role: into.from_role };
        let (length, was) = (joined.length(), into.length() + out.length());
        let longest = self.longest.get(&(element, role == Role::Top)).copied().unwrap_or(0.0);
        let shortest = pass.rules.shortest.min(into.length()).min(out.length());
        if !at_least(was - length, LEAST) || !at_least(longest, length) || !at_least(length, shortest) {
            return Ok(None);
        }
        // The plan keeps its own needle points, off the machine's grid, and a plan's checks measure those
        // (`REQ-PLAN-002`): there too the joined stitch is no shorter than the machine's shortest or the
        // shorter of the 2, and no longer than the machine's longest.
        let (Some(first), Some(middle), Some(last)) = (pass.at((block, from)), pass.at((block, at)), pass.at((block, to))) else { return Ok(None) };
        let (shorter, joined_length) = (first.distance(middle).min(middle.distance(last)), first.distance(last));
        if !at_least(joined_length, pass.rules.shortest.min(shorter)) || !at_least(pass.rules.longest, joined_length) {
            return Ok(None);
        }
        let limit = if hot { pass.rules.limit } else { pass.rules.cap };
        if pass.thickness.hottest(&into, meter)?.max(pass.thickness.hottest(&out, meter)?) <= limit {
            return Ok(None);
        }
        // Above the warning's limit a column's cover is judged as drawn, as the cap did before columns were
        // left whole.
        let whole = hot;
        let after = (block, to);
        let covered = match kind {
            Kind::Hidden => {
                // A lettering column's underlay holds its top up, and goes only where another of the
                // lettering's columns covers it too (`REQ-AUTO-122`).
                let lettering = pass.rules.lettering;
                let other = |ribbon: &Ribbon| ribbon.element != element && ribbon.element.is_some_and(|other| lettering.contains_key(&other));
                let held = element.is_some_and(|element| lettering.contains_key(&element));
                self.ribbons.covers(&joined, |ribbon| ribbon.at > after, false, meter)?
                    && (!held || self.ribbons.covers(&joined, other, false, meter)?)
            }
            Kind::Under => {
                let counts = |ribbon: &Ribbon| ribbon.at > after && ribbon.element != element;
                self.ribbons.covers(&into, counts, false, meter)? && self.ribbons.covers(&out, counts, false, meter)?
            }
            Kind::Doubled | Kind::Column => {
                let counts = |ribbon: &Ribbon| ribbon.at != (block, at) && ribbon.at != after && (ribbon.element == element || ribbon.at > after);
                self.ribbons.covers(&into, counts, whole, meter)? && self.ribbons.covers(&out, counts, whole, meter)?
            }
        };
        // A joined top stitch lays its thread only where its shape's own top thread or a later shape's lies: a
        // stitch cut across a bend would show on a neighbour (`REQ-AUTO-026`). Above the warning's limit a spot
        // too thick to sew outweighs that, as it outweighs a column's pulled edge (`REQ-AUTO-112`).
        let covered = covered
            && (kind == Kind::Hidden
                || hot
                || self.ribbons.covers(&joined, |ribbon| ribbon.element == element || ribbon.at > after, false, meter)?);
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
            let Some(side) = side.filter(|_| !hot) else { continue };
            // A stitch of no length gives no number, and the whole of it, its one place, is what moves.
            let t = (overlap / side.length()).min(1.0);
            let part =
                if start { SewnStitch { to: side.from.lerp(side.to, t), ..side } } else { SewnStitch { from: side.to.lerp(side.from, t), ..side } };
            if !self.ribbons.covers(&part, others, whole, meter)? {
                return Ok(None);
            }
        }
        let change = Change::of(&into, &joined);
        if !covered {
            return Ok(None);
        }
        if let Some(spot) = asked {
            // How much it lowers the thickest spot: the measure with the joined stitch, and back.
            let was = self.pass.thickness.at(Measure::Fills, spot);
            let lowered = self.pass.thickness.tried(&[into, out], &[joined], meter, |thickness| was - thickness.at(Measure::Fills, spot))?;
            return Ok(Some(Change { lowered, ..change }));
        }
        if !self.pass.changed(&[into, out], &[joined], meter)? {
            return Ok(None);
        }
        self.pass.gone.insert((block, at));
        if role == Role::Top {
            self.ribbons.kill((block, at));
            self.ribbons.kill(after);
            // The joined stitch: a fill's, which no pull counts, or a column's, which turns at both its ends.
            let cut = if kind == Kind::Column { (overlap, overlap) } else { (0.0, 0.0) };
            self.ribbons.add(Ribbon::of((joined.from, joined.to), cut, after, element), meter)?;
        }
        // The stitches beside it, pulled in where they now turn.
        for (side, start) in [(before, false), (next, true)] {
            let Some(was) = side.and_then(|side| self.ribbons.kill((side.block, side.index))) else { continue };
            let cut = if start { (overlap, was.cut.1) } else { (was.cut.0, overlap) };
            self.ribbons.add(Ribbon::of(was.whole, cut, was.at, was.element).firm(was.firm), meter)?;
        }
        if let (Some(taken), Some(times)) = (self.taken.get_mut(kind.at()), self.times.get_mut(kind.at())) {
            *taken += was - length;
            *times += 1;
        }
        Ok(Some(change))
    }
}

/// How joining a needle point's 2 stitches changes the stitch into it.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Change {
    /// Whether the joined stitch turns 45° or more from it: along a column's edge, a notch in it.
    notch: bool,
    /// The cosine of the angle between them: 1 where the joined stitch runs on as the stitch did.
    cosine: f64,
    /// How many fills taking the point takes from the thickest spot.
    lowered: f64,
}

impl Change {
    /// How the stitch `joined` turns from `into`, the stitch it replaces into the point.
    fn of(into: &SewnStitch, joined: &SewnStitch) -> Change {
        let (u, v) = ((into.to.x() - into.from.x(), into.to.y() - into.from.y()), (joined.to.x() - joined.from.x(), joined.to.y() - joined.from.y()));
        let cosine = (u.0 * v.0 + u.1 * v.1) / (into.length() * joined.length());
        // A cosine of no number, from a stitch of no length, counts as a notch.
        let across = matches!(cosine.partial_cmp(&math::cos(NOTCH.to_radians())), Some(Ordering::Greater | Ordering::Equal));
        Change { notch: !across, cosine, lowered: 0.0 }
    }

    /// Whether this change is smaller than `other`: no notch where `other` has one, or else a smaller turn.
    fn less_than(self, other: Change) -> bool {
        (!self.notch && other.notch) || (self.notch == other.notch && self.cosine > other.cosine)
    }
}

/// Of the points `may`, with how taking each changes its column, those that lower the thickest spot by half as
/// much as the one that lowers it most, or more, and of those the one that changes its column least: a point
/// that leaves the spot as thick changes a column for nothing. `None` when none lowers it.
fn chosen(may: &[(Change, Entry)]) -> Option<Entry> {
    let most = may.iter().fold(0.0_f64, |most, (change, _)| most.max(change.lowered));
    let mut best: Option<(Change, Entry)> = None;
    for &(change, point) in may.iter().filter(|(change, _)| most > 0.0 && change.lowered * 2.0 >= most) {
        if best.is_none_or(|(least, _)| change.less_than(least)) {
            best = Some((change, point));
        }
    }
    best.map(|(_, point)| point)
}

/// The turn from the stitch into a point, in degrees, from which the joined stitch runs along a column's edge
/// rather than across the column.
const NOTCH: f64 = 45.0;

/// A top stitch, as it covers the fabric.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Ribbon {
    /// Where it starts and ends, pulled in; `None` when the pull leaves nothing of it.
    pulled: Option<(Point, Point)>,
    /// The stitch as sewn, and how far the pull moves it in at its start and its end.
    whole: (Point, Point),
    cut: (f64, f64),
    /// The entry it ends at.
    at: Entry,
    element: Option<ElementRef>,
    /// Whether it is still sewn: a needle point taken out ends the 2 stitches beside it.
    alive: bool,
    /// Whether it covers only as pulled in, even where the cover is judged as sewn: a lettering column's.
    firm: bool,
}

impl Ribbon {
    /// The stitch from `from` to `to` that ends at entry `at`, of `element`, pulled in by `cut` at its start and
    /// its end.
    fn of((from, to): (Point, Point), cut: (f64, f64), at: Entry, element: Option<ElementRef>) -> Ribbon {
        Ribbon { pulled: pulled(from, to, cut.0, cut.1), whole: (from, to), cut, at, element, alive: true, firm: false }
    }

    /// The ribbon, covering only as pulled in when `firm`: a lettering column's, and a stitch beside a joined
    /// one that was a lettering column's.
    fn firm(self, firm: bool) -> Ribbon {
        Ribbon { firm, ..self }
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
        let (from, to) = ribbon.whole;
        let ((x0, x1), (y0, y1)) = (span(from.x(), to.x()), span(from.y(), to.y()));
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

    /// Whether every place along `stitch`, each [`STEP`], lies under a stitch still sewn that `counts`, pulled
    /// in or, when `as_sewn`, as sewn. A unit of work for each place, and for each stitch looked at.
    fn covers(&self, stitch: &SewnStitch, counts: impl Fn(&Ribbon) -> bool, as_sewn: bool, meter: &mut Meter) -> Result<bool, Exhausted> {
        let steps = whole(stitch.length() / STEP).max(1);
        for step in 0..=steps {
            let place = stitch.from.lerp(stitch.to, step as f64 / steps as f64);
            let near = self.cells.get(&(cell(place.x()), cell(place.y()))).map_or(&[][..], Vec::as_slice);
            meter.charge(units(near.len()).saturating_add(1))?;
            let lies = |ribbon: &Ribbon| if as_sewn && !ribbon.firm { Some(ribbon.whole) } else { ribbon.pulled };
            let under =
                |ribbon: &&Ribbon| ribbon.alive && counts(ribbon) && lies(ribbon).is_some_and(|(from, to)| to_side(place, from, to) <= RIBBON);
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
    // A stitch of no length gives no number, and no number runs straight on.
    let lengths = first.length() * next.length();
    first.block == next.block && first.to == next.from && (u.0 * v.0 + u.1 * v.1) / lengths >= math::cos(STRAIGHT_ON.to_radians())
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

/// What the cap did, in words, for the note: the points it pulled in and every kind's thread it took
/// ([`kinds`]), or one of them.
pub(crate) fn done(capped: &Capped) -> String {
    let [points, at] = capped.pulled;
    let took = kinds(capped);
    let pulled = match points {
        0 => return format!("took {took}"),
        1 => format!("pulled in 1 point of a satin column in {}", super::thick::places(at)),
        _ => format!("pulled in {points} points of satin columns in {}", super::thick::places(at)),
    };
    if took.is_empty() { pulled } else { format!("{pulled} and took {took}") }
}

/// Every kind's thread taken, in words, for the note: those of none are left out.
pub(crate) fn kinds(capped: &Capped) -> String {
    let names = [
        "underlay and travel",
        "top stitches under later shapes",
        "a shape's own doubled top stitches",
        "satin columns' top stitches above the warning's limit",
    ];
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

mod hot;
mod pull;
pub(crate) use pull::Pullable;
#[cfg(test)]
mod tests;
