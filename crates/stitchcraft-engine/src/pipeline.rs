//! The engine's entry point: a design to a stitch plan.
//!
//! Design: `docs/src/design/engine-pipeline.md`. Every host calls [`plan`] — the command line with an SVG
//! file's design, the VectorCraft plug-in with a document's — so every host gets the same stitches for the
//! same design (`docs/src/design/architecture.md` › Hosts). It generates each element, assembles them,
//! fits the plan to the machine and checks it: a plan that comes back can be written as it is.
//!
//! Elements are generated in sewing order, each with its neighbours (`REQ-GEN-003`,
//! `docs/src/design/adr/0014-generators-see-their-neighbours.md`): the last needle point of the elements
//! before it that sew any, and what the next element offers to end near. Each is given the stitches the
//! ones before it left (`REQ-GEN-004`), and one that needs more ends planning there: the design needs more
//! stitches than it may have, so no plan could come of it, and making the rest would only fill memory.

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_plan::{MachineProfile, StitchPlan};

use crate::assemble::{Assembled, assemble};
use crate::design::Design;
use crate::finalize::{Finalized, finalize};
use crate::generate::{Generation, approach, generate_with};
use crate::generators::Neighbours;

/// What planning a design gives.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanOutcome {
    /// The plan, fitted to the machine and checked; `None` when it cannot be sewn, as the diagnostics
    /// say.
    pub plan: Option<StitchPlan>,
    /// What was changed, left out or wrong, element by element, each naming its element when it has
    /// one.
    pub diagnostics: Vec<Diagnostic>,
}

/// The stitch plan for `design`, sewn on the machine `profile` describes. Each element has `budget`'s
/// work to itself, and the stitches the elements before it left; assembling and finalizing have the work
/// once more, and the design `budget`'s stitches.
pub fn plan(design: &Design, profile: &MachineProfile, budget: &Budget) -> PlanOutcome {
    let mut diagnostics = Vec::new();
    let mut generated = Vec::new();
    let elements = design.elements();
    let mut needle = None;
    let mut left = budget.max_stitches;
    for (i, element) in elements.iter().enumerate() {
        let neighbours = Neighbours { needle, next: elements.get(i + 1).and_then(|next| approach(next, &design.settings, budget)) };
        let mut meter = Budget { max_stitches: left, ..*budget }.meter();
        let Generation { generated: sewn, diagnostics: said } =
            match generate_with(element, &design.settings, profile, &neighbours, budget, &mut meter) {
                Ok(generation) => generation,
                Err(said) => {
                    diagnostics.extend(said);
                    return PlanOutcome { plan: None, diagnostics };
                }
            };
        // What a skipped element made before it stopped counts for nothing.
        let made = sewn.as_ref().map_or(0, |sewn| sewn.groups.iter().map(Vec::len).sum::<usize>());
        left = left.saturating_sub(u32::try_from(made).unwrap_or(u32::MAX));
        diagnostics.extend(said);
        needle = sewn.as_ref().and_then(|sewn| sewn.groups.iter().rev().find_map(|group| group.last().copied())).or(needle);
        generated.extend(sewn.map(|sewn| (element, sewn)));
    }
    let mut meter = budget.meter();
    let finished = assemble(&generated, &design.settings, &mut meter).and_then(|assembled| match assembled {
        Some(Assembled { plan, warnings, shortest }) => {
            diagnostics.extend(warnings);
            finalize(plan, profile, &design.settings, &shortest, &mut meter).map(Some)
        }
        None => Ok(None),
    });
    let plan = match finished {
        Ok(Some(Finalized { plan, diagnostics: said })) => {
            diagnostics.extend(said);
            plan
        }
        Ok(None) => {
            let message = "The design has nothing to stitch: it has no elements, or every one was skipped.";
            diagnostics.push(Diagnostic::new(Code::NothingToStitch, message));
            None
        }
        Err(exhausted) => {
            diagnostics.push(exhausted.diagnostic(budget, None));
            None
        }
    };
    PlanOutcome { plan, diagnostics }
}
