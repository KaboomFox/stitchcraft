//! Building plans one entry at a time.
//!
//! [`PlanBuilder`] tracks the needle so commands always happen where the needle is, and keeps the block
//! structure valid by construction: there is always a current block, and a new thread starts a new one.
//! What it cannot know — whether a stitch is too long for a machine, whether a block ended up empty — is
//! the invariant checker's job ([`crate::invariants`]).

use stitchcraft_core::{ElementId, Point};

use crate::plan::{ColorBlock, ElementRef, Provenance, Role, Stitch, StitchKind, StitchPlan};
use crate::thread::Thread;

/// Why a plan could not be built.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlanError {
    /// More elements than an [`ElementRef`] can address (over four billion).
    #[error("a plan can refer to at most {} elements", u32::MAX)]
    TooManyElements,
}

/// Builds a [`StitchPlan`], tracking the needle position.
#[derive(Clone, Debug)]
pub struct PlanBuilder {
    done: Vec<ColorBlock>,
    current: ColorBlock,
    elements: Vec<ElementId>,
    needle: Point,
}

impl PlanBuilder {
    /// A plan whose first block is sewn with `thread`. The needle starts at the machine origin.
    pub fn new(thread: Thread) -> Self {
        PlanBuilder { done: Vec::new(), current: ColorBlock { thread, stitches: Vec::new() }, elements: Vec::new(), needle: Point::ORIGIN }
    }

    /// Registers `id` and returns the reference stitches use for it.
    pub fn element(&mut self, id: ElementId) -> Result<ElementRef, PlanError> {
        let element = ElementRef::from_index(self.elements.len()).ok_or(PlanError::TooManyElements)?;
        self.elements.push(id);
        Ok(element)
    }

    /// Ends the current block; what follows is sewn with `thread`.
    pub fn change_thread(&mut self, thread: Thread) {
        let finished = core::mem::replace(&mut self.current, ColorBlock { thread, stitches: Vec::new() });
        self.done.push(finished);
    }

    /// Moves to `at` and sews there.
    pub fn stitch(&mut self, at: Point, origin: Provenance) {
        self.push(at, StitchKind::Normal, origin);
    }

    /// Moves to `to` without sewing.
    pub fn jump(&mut self, to: Point, origin: Provenance) {
        self.push(to, StitchKind::Jump, origin);
    }

    /// Cuts the thread where the needle is.
    pub fn trim(&mut self, element: Option<ElementRef>) {
        self.push(self.needle, StitchKind::Trim, Provenance { element, role: Role::Command });
    }

    /// Pauses for the operator where the needle is.
    pub fn stop(&mut self, element: Option<ElementRef>) {
        self.push(self.needle, StitchKind::Stop, Provenance { element, role: Role::Command });
    }

    /// Where the needle is now.
    pub const fn needle(&self) -> Point {
        self.needle
    }

    /// The finished plan. Check it with [`crate::invariants::check`] before writing it.
    pub fn finish(mut self) -> StitchPlan {
        self.done.push(self.current);
        StitchPlan { blocks: self.done, elements: self.elements }
    }

    fn push(&mut self, at: Point, kind: StitchKind, origin: Provenance) {
        self.current.stitches.push(Stitch { at, kind, origin });
        self.needle = at;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::thread::Rgb;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn commands_happen_where_the_needle_is() {
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        let e = b.element(ElementId::new("ts:line").unwrap()).unwrap();
        b.jump(p(1.0, 2.0), Provenance::new(e, Role::Travel));
        b.stitch(p(3.0, 2.0), Provenance::new(e, Role::Top));
        b.trim(Some(e));
        b.change_thread(Thread::new(Rgb::new(255, 0, 0)));
        b.stop(None);
        let plan = b.finish();
        assert_eq!(plan.blocks.len(), 2);
        let trim = plan.blocks[0].stitches[2];
        assert_eq!((trim.kind, trim.at, trim.origin.role), (StitchKind::Trim, p(3.0, 2.0), Role::Command));
        assert_eq!(plan.blocks[1].stitches[0].at, p(3.0, 2.0), "a thread change does not move the needle");
        assert_eq!(plan.element(e).map(ElementId::as_str), Some("ts:line"));
        let entries: Vec<(Rgb, bool)> = plan.color_entries().iter().map(|e| (e.thread.color, e.stop)).collect();
        assert_eq!(entries, vec![(Rgb::new(0, 0, 0), false), (Rgb::new(255, 0, 0), false), (Rgb::new(255, 0, 0), true)]);
    }

    #[test]
    fn stats_count_every_kind() {
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        b.jump(p(1.0, 0.0), Provenance::plan(Role::Travel));
        b.stitch(p(1.0, 0.0), Provenance::plan(Role::Top));
        b.stitch(p(3.0, 0.0), Provenance::plan(Role::Top));
        b.stop(None);
        b.trim(None);
        b.change_thread(Thread::new(Rgb::new(9, 9, 9)));
        b.stitch(p(4.0, 0.0), Provenance::plan(Role::Top));
        let stats = b.finish().stats();
        assert_eq!((stats.stitches, stats.jumps, stats.trims, stats.stops, stats.color_changes), (3, 1, 1, 1, 1));
        assert_eq!(stats.changes_including_stops(), 2);
    }
}
