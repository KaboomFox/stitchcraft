//! The engine's entry point: a design to a stitch plan.
//!
//! Design: `docs/src/design/engine-pipeline.md`. Every host calls [`plan`] — the command line with an SVG
//! file's design, the VectorCraft plug-in with a document's — so every host gets the same stitches for the
//! same design (`docs/src/design/architecture.md` › Hosts). It generates each element, assembles them,
//! fits the plan to the machine and checks it: a plan that comes back can be written as it is.
//!
//! Elements are generated in sewing order, each with its neighbours (`REQ-GEN-003`,
//! `docs/src/design/adr/0014-generators-see-their-neighbours.md`): the last needle point of the elements
//! before it that sew any, and what the next element offers to end near.

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_plan::{MachineProfile, StitchPlan};

use crate::assemble::{Assembled, assemble};
use crate::design::Design;
use crate::finalize::{Finalized, finalize};
use crate::generate::{Generation, approach, generate};
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
/// work to itself; assembling and finalizing have it once more, and the design `budget`'s stitches.
pub fn plan(design: &Design, profile: &MachineProfile, budget: &Budget) -> PlanOutcome {
    let mut diagnostics = Vec::new();
    let mut generated = Vec::new();
    let elements = design.elements();
    let mut needle = None;
    for (i, element) in elements.iter().enumerate() {
        let neighbours = Neighbours { needle, next: elements.get(i + 1).and_then(|next| approach(next, budget)) };
        let Generation { generated: sewn, diagnostics: said } = generate(element, &design.settings, profile, &neighbours, budget);
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
