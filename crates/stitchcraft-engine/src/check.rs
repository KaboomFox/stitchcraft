//! Pipeline stage 7, the check: the plan the engine returns, checked once nothing takes stitches out of it.
//!
//! Finalize checks the plan invariants (`REQ-FIN-003`), but on an automatic design the thinning pass and
//! the cap ([`crate::auto::thick`]) take needle points out after it, lock points among them. So the check
//! runs again here, on the plan that is returned, and every plan the engine returns passes it by
//! construction. It checks 2 things:
//!
//! - the plan invariants for the machine (`stitchcraft_plan::invariants`);
//! - each run's locks (`REQ-PLAN-008`): a run starts with a lock stitch where its first point's element asks
//!   for a tie-in, and ends with one where its last point's element asks for a tie-off. A run and its ends
//!   are the machine's thread state ([`ThreadState`]): the needle points sewn between the plan's start, a
//!   thread change, a jump, a trim, a stop and the end. An element asks for locks as assembly reads its
//!   settings ([`Generated::locks`]). A plan doesn't hold those settings, which is why this check is the
//!   engine's and not an invariant.
//!
//! A plan that breaks either is a bug in StitchCraft, reported as `SC-E0009` instead of returned. Nothing
//! is repaired. Both walks are linear in the plan's length, and the plan is bounded by the stitch budget
//! that made it, so the check needs no meter of its own. Its cost is in the design page's Check section.
//! Design: `docs/src/design/data-model.md` › The thread's state.

use std::collections::BTreeMap;

use stitchcraft_core::{Diagnostic, ElementId};
use stitchcraft_plan::invariants::{self, Violation};
use stitchcraft_plan::{Entered, MachineProfile, Role, Stitch, StitchPlan, ThreadState};

use crate::generate::Generated;

/// The requirement of the locks' check, as in `conformance/requirements.toml`.
pub(crate) const LOCKS: &str = "REQ-PLAN-008";

/// The locks each element asks for, a tie-in and a tie-off, by its id: from the first of its `generated`
/// steps (an element sewn as 2 layers has one setting for both).
pub(crate) fn asked<'a>(generated: impl IntoIterator<Item = (&'a ElementId, &'a Generated)>) -> BTreeMap<ElementId, (bool, bool)> {
    let mut asked = BTreeMap::new();
    for (id, sewn) in generated {
        asked.entry(id.clone()).or_insert_with(|| sewn.locks());
    }
    asked
}

/// `plan` if it passes the invariants for `profile` and has every lock `asked` sets; otherwise the
/// `SC-E0009` diagnostic of each violation, at most [`invariants::MAX_REPORTED`] of each kind.
pub(crate) fn checked(plan: StitchPlan, profile: &MachineProfile, asked: &BTreeMap<ElementId, (bool, bool)>) -> Result<StitchPlan, Vec<Diagnostic>> {
    let mut violations = invariants::check(&plan, profile);
    violations.extend(locks(&plan, asked));
    if violations.is_empty() { Ok(plan) } else { Err(violations.iter().map(Violation::diagnostic).collect()) }
}

/// Where `plan` lacks a lock that `asked` sets (`REQ-PLAN-008`). An element `asked` doesn't list, or an
/// entry that names none, asks for nothing.
pub(crate) fn locks(plan: &StitchPlan, asked: &BTreeMap<ElementId, (bool, bool)>) -> Vec<Violation> {
    let mut walk = Walk { plan, asked, violations: Vec::new() };
    let mut state = ThreadState::default();
    let mut last: Option<(usize, usize, &Stitch)> = None;
    for (b, block) in plan.blocks.iter().enumerate() {
        walk.ended(last.take());
        state.change_thread();
        for (i, stitch) in block.stitches.iter().enumerate() {
            match state.enter(b, i, stitch) {
                Entered::Started => {
                    walk.started(b, i, stitch);
                    last = Some((b, i, stitch));
                }
                Entered::Sewn(_) => last = Some((b, i, stitch)),
                Entered::Jumped | Entered::Trimmed { .. } | Entered::Stopped => walk.ended(last.take()),
            }
        }
    }
    walk.ended(last);
    walk.violations
}

/// The walk of [`locks`]: what it reads, and what it found.
struct Walk<'p> {
    plan: &'p StitchPlan,
    asked: &'p BTreeMap<ElementId, (bool, bool)>,
    violations: Vec<Violation>,
}

impl Walk<'_> {
    /// The locks the element of `stitch` asks for, and its id.
    fn asks(&self, stitch: &Stitch) -> Option<(&ElementId, (bool, bool))> {
        let id = self.plan.element(stitch.origin.element?)?;
        Some((id, *self.asked.get(id)?))
    }

    /// A run starts at `stitch`, entry `i` of block `b`.
    fn started(&mut self, b: usize, i: usize, stitch: &Stitch) {
        if let Some((id, (true, _))) = self.asks(stitch)
            && stitch.origin.role != Role::Lock
        {
            let message = format!("the run starts without the tie-in that element `{}` asks for", id.as_str());
            self.add(b, i, message);
        }
    }

    /// A run ended at `last`, if one was open.
    fn ended(&mut self, last: Option<(usize, usize, &Stitch)>) {
        let Some((b, i, stitch)) = last else { return };
        if let Some((id, (_, true))) = self.asks(stitch)
            && stitch.origin.role != Role::Lock
        {
            let message = format!("the run ends without the tie-off that element `{}` asks for", id.as_str());
            self.add(b, i, message);
        }
    }

    fn add(&mut self, block: usize, stitch: usize, message: String) {
        if self.violations.len() < invariants::MAX_REPORTED {
            self.violations.push(Violation { requirement: LOCKS, block, stitch: Some(stitch), message });
        }
    }
}

#[cfg(test)]
mod tests;
