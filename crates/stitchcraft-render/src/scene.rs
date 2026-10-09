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
//! - **Marks**, the trims and stops, where the needle is when they happen.
//!
//! Every position is rounded with [`Point::to_tenths`], the writers' own rounding, so a scene shows the
//! holes a machine file makes, not the ones the plan meant (REQ-RND-001). Like
//! `stitchcraft_testkit::equivalence::events`, marks describe what the machine does rather than how a
//! file spells it: two trims in a row cut once, the order of commands at one spot does not matter, and
//! a trim after the last hole cuts nothing that is still sewn — so marks are a sorted set, and trailing
//! trims are left out. A PES file spells a trim as a flag on the next move and so cannot even record
//! one at the end; this is why its preview and its plan's preview are still the same.

use std::collections::BTreeSet;

use stitchcraft_core::{Meter, Point};
use stitchcraft_plan::{Rgb, Role, StitchKind, StitchPlan};

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
    /// The thread was cut since the previous hole (a trim or a thread change), or there is no previous
    /// hole: nothing joins the two.
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
    /// The machine pauses for the operator.
    Stop(GridPoint),
    /// The thread is cut.
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
    /// The scene of `plan`; one unit of work per plan entry.
    pub fn of(plan: &StitchPlan, meter: &mut Meter) -> Result<Scene, RenderError> {
        let mut scene = Scene::default();
        let sewn = plan.sewn_stitches();
        let mut sewn = sewn.iter().map(|s| (s.block, s.index)).peekable();
        // Before the first hole there is no thread to carry: the needle starts above the fabric.
        let mut cut = true;
        let mut trims = Vec::new();
        for (b, block) in plan.blocks.iter().enumerate() {
            // A thread change cuts the old thread.
            cut |= b > 0;
            for (i, stitch) in block.stitches.iter().enumerate() {
                meter.charge(1)?;
                let at = GridPoint::of(stitch.at)?;
                match stitch.kind {
                    StitchKind::Normal => {
                        let arrival = match sewn.next_if_eq(&(b, i)) {
                            Some(_) => Arrival::Sewn,
                            None if cut => Arrival::Cut,
                            None => Arrival::Loose,
                        };
                        let lock = stitch.origin.role == Role::Lock;
                        scene.holes.push(Hole { at, color: block.thread.color, arrival, lock });
                        scene.marks.extend(trims.drain(..).map(Mark::Trim));
                        cut = false;
                    }
                    StitchKind::Jump => {}
                    StitchKind::Trim => {
                        trims.push(at);
                        cut = true;
                    }
                    StitchKind::Stop => {
                        scene.marks.insert(Mark::Stop(at));
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

    fn scene(plan: &StitchPlan) -> Scene {
        Scene::of(plan, &mut Budget::DEFAULT.meter()).unwrap()
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
                // A stop keeps the run going.
                (80, RED, Arrival::Sewn, false),
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
        assert_eq!(Scene::of(&far, &mut Budget::DEFAULT.meter()), Err(RenderError::OutOfRange));
        let tiny = Budget { max_stitches: 10, max_work: 0 };
        assert!(matches!(Scene::of(&far, &mut tiny.meter()), Err(RenderError::Budget(_))));
    }
}
