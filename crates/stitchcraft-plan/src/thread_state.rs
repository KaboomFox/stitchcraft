//! The machine's state as it reads a plan, entry by entry: where the needle is, whether a run of stitches
//! is open, and whether the thread has sewn since it was last cut.
//!
//! Several things read a plan as the machine does: the stitch-length rules ([`crate::plan::StitchPlan::sewn_stitches`]),
//! the plan invariants ([`crate::invariants`]), the engine's check of the locks and, through
//! `sewn_stitches`, the preview's scene. Each once kept its own copy of this model, and the copies
//! disagreed at a stop (M13.56). This type is the one copy, so they can't disagree again. Design:
//! `docs/src/design/data-model.md` › The thread's state.
//!
//! A run is the needle points sewn between 2 places where the thread may be cut: the plan's start, a
//! thread change, a jump, a trim, a stop and the end (`REQ-PLAN-010`: the machine cuts at a stop, and PES
//! and DST write it as a thread change). The thread is cut, for `REQ-PLAN-009`, at the plan's start, a
//! thread change, a trim and a stop. A jump moves the needle but leaves the thread attached.
//!
//! The type does no work of its own: each call takes one entry, so whoever loops over a plan bounds the work.

use stitchcraft_core::Point;

use crate::plan::{Role, SewnStitch, Stitch, StitchKind};

/// What one entry did as the machine read it ([`ThreadState::enter`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Entered {
    /// A `Normal` entry that lays thread from the hole before it.
    Sewn(SewnStitch),
    /// A `Normal` entry that starts a run: the needle goes down with no thread laid from a hole.
    Started,
    /// A jump: the needle moves up, and any run ends.
    Jumped,
    /// A trim, and whether a stitch was sewn since the thread was last cut (`REQ-PLAN-009`).
    Trimmed {
        /// Whether a `Normal` entry came since the plan's start, the last thread change, trim or stop.
        after_sewing: bool,
    },
    /// A stop: the machine cuts there, and any run ends.
    Stopped,
}

/// The machine's state between 2 entries of a plan (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThreadState {
    needle: Point,
    needle_role: Role,
    in_run: bool,
    sewn_since_cut: bool,
}

impl Default for ThreadState {
    /// The plan's start: the needle at the origin, up, and the thread cut.
    fn default() -> Self {
        Self { needle: Point::ORIGIN, needle_role: Role::Travel, in_run: false, sewn_since_cut: false }
    }
}

impl ThreadState {
    /// Where the needle is: the last `Normal` or `Jump` entry's position, or the origin before any.
    pub fn needle(&self) -> Point {
        self.needle
    }

    /// A thread change, before a colour block's first entry: the run ends, and the thread is cut. The needle
    /// stays where it is.
    pub fn change_thread(&mut self) {
        (self.in_run, self.sewn_since_cut) = (false, false);
    }

    /// Reads `stitch`, entry `index` of colour block `block`, and says what it did.
    pub fn enter(&mut self, block: usize, index: usize, stitch: &Stitch) -> Entered {
        match stitch.kind {
            StitchKind::Normal => {
                let (from, from_role) = (self.needle, self.needle_role);
                let entered = if self.in_run {
                    Entered::Sewn(SewnStitch { block, index, from, to: stitch.at, role: stitch.origin.role, from_role })
                } else {
                    Entered::Started
                };
                (self.needle, self.needle_role, self.in_run, self.sewn_since_cut) = (stitch.at, stitch.origin.role, true, true);
                entered
            }
            StitchKind::Jump => {
                (self.needle, self.in_run) = (stitch.at, false);
                Entered::Jumped
            }
            StitchKind::Trim => {
                let after_sewing = self.sewn_since_cut;
                self.change_thread();
                Entered::Trimmed { after_sewing }
            }
            StitchKind::Stop => {
                self.change_thread();
                Entered::Stopped
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan::Provenance;

    fn entry(kind: StitchKind, x: f64, role: Role) -> Stitch {
        Stitch { at: Point::new(x, 0.0).unwrap(), kind, origin: Provenance::plan(role) }
    }

    /// What each entry of `entries` did, from the plan's start, a thread change before entry `change`.
    fn read(entries: &[Stitch], change: usize) -> (Vec<Entered>, ThreadState) {
        let mut state = ThreadState::default();
        let mut said = Vec::new();
        for (i, stitch) in entries.iter().enumerate() {
            if i == change {
                state.change_thread();
            }
            said.push(state.enter(0, i, stitch));
        }
        (said, state)
    }

    #[test]
    fn each_entry_moves_the_state_as_the_machine_does() {
        use StitchKind::{Jump, Normal, Stop, Trim};
        let entries = [
            entry(Trim, 0.0, Role::Command),
            entry(Jump, 1.0, Role::Travel),
            entry(Normal, 1.0, Role::Lock),
            entry(Normal, 3.0, Role::Top),
            entry(Stop, 3.0, Role::Command),
            entry(Normal, 5.0, Role::Top),
            entry(Trim, 5.0, Role::Command),
            entry(Trim, 5.0, Role::Command),
            entry(Jump, 7.0, Role::Travel),
            entry(Trim, 7.0, Role::Command),
            entry(Normal, 9.0, Role::Top),
            entry(Normal, 11.0, Role::Top),
        ];
        let (said, state) = read(&entries, 11);
        let sewn = |index: usize, from: f64, to: f64, role: Role, from_role: Role| {
            Entered::Sewn(SewnStitch { block: 0, index, from: Point::new(from, 0.0).unwrap(), to: Point::new(to, 0.0).unwrap(), role, from_role })
        };
        assert_eq!(
            said,
            [
                Entered::Trimmed { after_sewing: false },
                Entered::Jumped,
                Entered::Started,
                sewn(3, 1.0, 3.0, Role::Top, Role::Lock),
                Entered::Stopped,
                Entered::Started,
                Entered::Trimmed { after_sewing: true },
                Entered::Trimmed { after_sewing: false },
                Entered::Jumped,
                Entered::Trimmed { after_sewing: false },
                Entered::Started,
                Entered::Started,
            ],
            "a stop ends the run (REQ-PLAN-010); a jump moves the needle but cuts nothing; a thread change ends the run"
        );
        assert_eq!(state.needle(), Point::new(11.0, 0.0).unwrap());
        let (_, after_jump) = read(&entries[..2], usize::MAX);
        assert_eq!(after_jump.needle(), Point::new(1.0, 0.0).unwrap(), "a jump moves the needle");
        let (_, after_trim) = read(&entries[..7], usize::MAX);
        assert_eq!(after_trim.needle(), Point::new(5.0, 0.0).unwrap(), "a trim leaves it");
    }

    #[test]
    fn the_plan_starts_with_the_needle_up_at_the_origin() {
        let start = ThreadState::default();
        assert_eq!(start.needle(), Point::ORIGIN);
        let mut state = start;
        state.enter(0, 0, &entry(StitchKind::Normal, 2.0, Role::Top));
        assert_eq!(state.enter(0, 1, &entry(StitchKind::Trim, 2.0, Role::Command)), Entered::Trimmed { after_sewing: true });
    }
}
