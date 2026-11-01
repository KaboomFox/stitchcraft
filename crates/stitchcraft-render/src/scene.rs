//! What a preview shows: the plan as the machine will sew it, on the 0.1 mm grid.
//!
//! A [`Scene`] is plain data, built by one walk over the plan, so tests can compare scenes instead of
//! pixels. It holds two things:
//!
//! - **Holes**, every place the needle goes down, in sewing order, each saying how the thread got there
//!   from the previous hole ([`Arrival`]): sewn as a stitch ([`StitchPlan::sewn_stitches`] is the one
//!   definition of that), carried loose because the frame moved without a trim, or not at all because
//!   the thread was cut. Jumps are not holes: the thread runs straight from hole to hole however the
//!   frame travelled, so where a jump stopped on the way changes nothing on the fabric.
//! - **Marks**, the trims the machine makes and the stops, where the needle is when they happen.
//!
//! **What cuts the thread is the machine's** ([`TrimSupport`], from its profile). A thread change cuts,
//! and so does a stop, which PES and DST write as a thread change: the reference PE800 cuts at both
//! (machine checkpoint MC-1). A trim cuts only on a machine that obeys the format's trim command. The
//! PE800 doesn't: it cuts no jump, trim-flagged or not (MC-1), so its scene draws a jump thread where a
//! plan or a file asks for a trim, and no trim mark. A machine that cuts long jumps by itself cuts where
//! the next hole lies farther from the last than its setting, at the last hole.
//!
//! Every position is rounded with [`Point::to_tenths`], the writers' own rounding, so a scene shows the
//! holes a machine file makes, not the ones the plan meant (REQ-RND-001). Like
//! `stitchcraft_testkit::equivalence::events`, marks describe what the machine does rather than how a
//! file spells it: two trims in a row cut once, the order of commands at one spot does not matter, and
//! a trim after the last hole cuts nothing that is still sewn — so marks are a sorted set, and trailing
//! trims are left out. A PES file flags every jump as a trim (REQ-FMT-010), so on a machine that obeys the
//! trim command its preview cuts at every jump, where its plan's preview carries jump threads; on the
//! PE800 the two are the same.

use std::collections::BTreeSet;

use stitchcraft_core::{Meter, Point};
use stitchcraft_plan::{Rgb, Role, StitchKind, StitchPlan, TrimSupport};

use crate::error::RenderError;

/// A position on the machine grid: 0.1 mm units, y down, from the hoop centre.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GridPoint {
    /// x in 0.1 mm.
    pub x: i32,
    /// y in 0.1 mm, down.
    pub y: i32,
}

impl GridPoint {
    /// `point` rounded to the machine grid.
    fn of(point: Point) -> Result<Self, RenderError> {
        let (x, y) = point.to_tenths().ok_or(RenderError::OutOfRange)?;
        Ok(GridPoint { x, y })
    }
}

/// How the thread reaches a hole from the previous hole.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Arrival {
    /// A stitch: the thread is laid from the previous hole.
    Sewn,
    /// The frame moved without sewing and the thread was not cut, so it lies loose from the previous
    /// hole: a jump thread, to be cut by hand.
    Loose,
    /// The thread was cut since the previous hole (a thread change, a stop, or a trim the machine
    /// makes), or there is no previous hole: nothing joins the two.
    Cut,
}

/// A place where the needle goes down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hole {
    /// Where.
    pub at: GridPoint,
    /// The thread's colour.
    pub color: Rgb,
    /// How the thread got here from the previous hole.
    pub arrival: Arrival,
    /// Whether the stitch is a tie-in or tie-off (lock) stitch, which the simple style marks.
    pub lock: bool,
}

/// A command, where the needle is when it happens. Stops sort before trims, so trims are drawn on top.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Mark {
    /// The machine pauses for the operator, and the thread is cut, as at a thread change.
    Stop(GridPoint),
    /// The machine cuts the thread.
    Trim(GridPoint),
}

impl Mark {
    /// Where it happens.
    pub const fn at(self) -> GridPoint {
        match self {
            Mark::Stop(at) | Mark::Trim(at) => at,
        }
    }
}

/// What a preview of a plan shows (see the module documentation).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Scene {
    /// Every hole, in sewing order.
    pub holes: Vec<Hole>,
    /// The trims and stops.
    pub marks: BTreeSet<Mark>,
}

impl Scene {
    /// The scene of `plan` on a machine that cuts as `trims` says; one unit of work per plan entry.
    pub fn of(plan: &StitchPlan, trims: TrimSupport, meter: &mut Meter) -> Result<Scene, RenderError> {
        let mut scene = Scene::default();
        let sewn = plan.sewn_stitches();
        let mut sewn = sewn.iter().map(|s| (s.block, s.index)).peekable();
        // Before the first hole there is no thread to carry: the needle starts above the fabric.
        let mut cut = true;
        let (mut cuts, mut last) = (Vec::new(), None);
        for (b, block) in plan.blocks.iter().enumerate() {
            // A thread change cuts the old thread.
            cut |= b > 0;
            for (i, stitch) in block.stitches.iter().enumerate() {
                meter.charge(1)?;
                let at = GridPoint::of(stitch.at)?;
                match stitch.kind {
                    StitchKind::Normal => {
                        let sewn_here = sewn.next_if_eq(&(b, i)).is_some();
                        let arrival = match last {
                            _ if cut => Arrival::Cut,
                            _ if sewn_here => Arrival::Sewn,
                            Some(from) if cuts_itself(trims, from, at) => {
                                cuts.push(from);
                                Arrival::Cut
                            }
                            _ => Arrival::Loose,
                        };
                        let lock = stitch.origin.role == Role::Lock;
                        scene.holes.push(Hole { at, color: block.thread.color, arrival, lock });
                        scene.marks.extend(cuts.drain(..).map(Mark::Trim));
                        (cut, last) = (false, Some(at));
                    }
                    StitchKind::Jump => {}
                    // A machine without the trim command sews on: the thread runs loose to the next hole.
                    StitchKind::Trim if trims == TrimSupport::Command => {
                        cuts.push(at);
                        cut = true;
                    }
                    StitchKind::Trim => {}
                    // Written as a thread change, which cuts.
                    StitchKind::Stop => {
                        scene.marks.insert(Mark::Stop(at));
                        cut = true;
                    }
                }
            }
        }
        Ok(scene)
    }

    /// The smallest and the largest position shown, or `None` when the scene has no holes.
    pub fn bounds(&self) -> Option<(GridPoint, GridPoint)> {
        self.holes.first()?;
        let points = self.holes.iter().map(|h| h.at).chain(self.marks.iter().map(|m| m.at()));
        points.fold(None, |bounds, p| match bounds {
            None => Some((p, p)),
            Some((lo, hi)) => Some((GridPoint { x: p.x.min(lo.x), y: p.y.min(lo.y) }, GridPoint { x: p.x.max(hi.x), y: p.y.max(hi.y) })),
        })
    }
}

/// Whether a machine that cuts as `trims` says cuts the thread itself on the way from the hole `from` to the
/// hole `to`: one that cuts jumps longer than its setting, when `to` lies farther from `from` than that.
fn cuts_itself(trims: TrimSupport, from: GridPoint, to: GridPoint) -> bool {
    let TrimSupport::LongJumps(length) = trims else { return false };
    let (dx, dy, tenths) = (f64::from(to.x - from.x), f64::from(to.y - from.y), length.get() * 10.0);
    dx * dx + dy * dy > tenths * tenths
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_plan::{PlanBuilder, Provenance, Thread};

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn g(x: i32, y: i32) -> GridPoint {
        GridPoint { x, y }
    }

    /// The scene of `plan` on a machine that obeys the trim command.
    fn scene(plan: &StitchPlan) -> Scene {
        Scene::of(plan, TrimSupport::Command, &mut Budget::DEFAULT.meter()).unwrap()
    }

    const RED: Rgb = Rgb::new(200, 0, 0);
    const BLUE: Rgb = Rgb::new(0, 0, 200);

    #[test]
    fn holes_say_how_the_thread_arrived() {
        let (top, lock, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Lock), Provenance::plan(Role::Travel));
        let mut b = PlanBuilder::new(Thread::new(RED));
        b.jump(p(1.0, 0.0), travel);
        b.stitch(p(1.0, 0.0), lock);
        b.stitch(p(2.0, 0.0), top);
        b.jump(p(5.0, 0.0), travel);
        b.jump(p(6.0, 0.0), travel);
        b.stitch(p(6.0, 0.0), top);
        b.trim(None);
        b.stitch(p(7.0, 0.0), top);
        b.stop(None);
        b.stitch(p(8.0, 0.0), top);
        b.change_thread(Thread::new(BLUE));
        b.stitch(p(9.0, 0.0), top);
        let holes: Vec<_> = scene(&b.finish()).holes.iter().map(|h| (h.at.x, h.color, h.arrival, h.lock)).collect();
        assert_eq!(
            holes,
            [
                (10, RED, Arrival::Cut, true),
                (20, RED, Arrival::Sewn, false),
                // Two jumps, no trim: one loose thread straight from 2 mm to 6 mm.
                (60, RED, Arrival::Loose, false),
                (70, RED, Arrival::Cut, false),
                // A stop cuts, as a thread change does: PES and DST write it as one.
                (80, RED, Arrival::Cut, false),
                // A thread change cuts.
                (90, BLUE, Arrival::Cut, false),
            ]
        );
    }

    #[test]
    fn positions_are_the_ones_a_machine_file_gets() {
        let mut b = PlanBuilder::new(Thread::new(RED));
        b.stitch(p(1.04, -0.25), Provenance::plan(Role::Top));
        b.stitch(p(1.25, 0.75), Provenance::plan(Role::Top));
        let at: Vec<_> = scene(&b.finish()).holes.iter().map(|h| h.at).collect();
        // Rounded half to even, like the writers: -2.5 → -2, 12.5 → 12, 7.5 → 8.
        assert_eq!(at, [g(10, -2), g(12, 8)]);
    }

    #[test]
    fn marks_are_what_the_machine_does() {
        let top = Provenance::plan(Role::Top);
        let mut b = PlanBuilder::new(Thread::new(RED));
        b.stitch(p(1.0, 0.0), top);
        b.trim(None);
        b.stop(None);
        b.trim(None);
        b.stitch(p(2.0, 0.0), top);
        b.stop(None);
        b.trim(None);
        let s = scene(&b.finish());
        // One cut at 1 mm whatever the spelling; the stop at the end stays, the trim after the last hole
        // cuts nothing that stays on the fabric.
        assert_eq!(s.marks.into_iter().collect::<Vec<_>>(), [Mark::Stop(g(10, 0)), Mark::Stop(g(20, 0)), Mark::Trim(g(10, 0))]);
    }

    #[test]
    fn bounds_cover_holes_and_marks_but_not_where_jumps_stopped() {
        let top = Provenance::plan(Role::Top);
        let mut b = PlanBuilder::new(Thread::new(RED));
        b.stitch(p(1.0, 1.0), top);
        b.jump(p(50.0, 50.0), Provenance::plan(Role::Travel));
        b.stop(None);
        b.jump(p(-3.0, 2.0), Provenance::plan(Role::Travel));
        b.stitch(p(-3.0, 2.0), top);
        assert_eq!(scene(&b.finish()).bounds(), Some((g(-30, 10), g(500, 500))));
        assert_eq!(Scene::default().bounds(), None);
    }

    #[test]
    fn far_away_positions_and_spent_budgets_are_errors() {
        let mut b = PlanBuilder::new(Thread::new(RED));
        b.stitch(p(20_000.0, 0.0), Provenance::plan(Role::Top));
        let far = b.finish();
        assert_eq!(Scene::of(&far, TrimSupport::Command, &mut Budget::DEFAULT.meter()), Err(RenderError::OutOfRange));
        let tiny = Budget { max_stitches: 10, max_work: 0 };
        assert!(matches!(Scene::of(&far, TrimSupport::Command, &mut tiny.meter()), Err(RenderError::Budget(_))));
    }

    /// A stitch at 1 mm, a trim, a jump to 20 mm and a stitch there; then a jump to 25 mm and a stitch there.
    fn trimmed_and_jumped() -> StitchPlan {
        let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
        let mut b = PlanBuilder::new(Thread::new(RED));
        b.stitch(p(1.0, 0.0), top);
        b.trim(None);
        b.jump(p(20.0, 0.0), travel);
        b.stitch(p(20.0, 0.0), top);
        b.jump(p(25.0, 0.0), travel);
        b.stitch(p(25.0, 0.0), top);
        b.finish()
    }

    /// How the thread reaches each hole of `plan` on a machine that cuts as `trims` says, and the marks.
    fn arrivals(plan: &StitchPlan, trims: TrimSupport) -> (Vec<Arrival>, Vec<Mark>) {
        let s = Scene::of(plan, trims, &mut Budget::DEFAULT.meter()).unwrap();
        (s.holes.iter().map(|h| h.arrival).collect(), s.marks.into_iter().collect())
    }

    #[test]
    fn a_machine_without_the_trim_command_leaves_a_jump_thread_where_a_trim_was_asked_for() {
        // The reference PE800 cuts no jump, trim-flagged or not (MC-1): the thread runs loose from 1 mm to
        // 20 mm, and no trim is marked.
        let plan = trimmed_and_jumped();
        assert_eq!(arrivals(&plan, TrimSupport::None), (vec![Arrival::Cut, Arrival::Loose, Arrival::Loose], vec![]));
        // A machine that obeys the command cuts there.
        assert_eq!(arrivals(&plan, TrimSupport::Command), (vec![Arrival::Cut, Arrival::Cut, Arrival::Loose], vec![Mark::Trim(g(10, 0))]));
    }

    #[test]
    fn a_machine_that_cuts_long_jumps_cuts_where_the_next_hole_is_farther_than_its_setting() {
        // Its setting 10 mm: it cuts the 19 mm jump at its start, and not the 5 mm one; it has no trim command.
        let plan = trimmed_and_jumped();
        let long = TrimSupport::LongJumps(stitchcraft_core::Mm::new(10.0).unwrap());
        assert_eq!(arrivals(&plan, long), (vec![Arrival::Cut, Arrival::Cut, Arrival::Loose], vec![Mark::Trim(g(10, 0))]));
        // A jump exactly as long as the setting is sewn on.
        let exactly = TrimSupport::LongJumps(stitchcraft_core::Mm::new(5.0).unwrap());
        assert_eq!(arrivals(&plan, exactly).0, [Arrival::Cut, Arrival::Cut, Arrival::Loose]);
    }
}
