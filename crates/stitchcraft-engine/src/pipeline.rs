//! The engine's entry point: a design to a stitch plan.
//!
//! Design: `docs/src/design/engine-pipeline.md`. Every host calls [`plan`] — the command line with an SVG
//! file's design, the VectorCraft plug-in with a document's — so every host gets the same stitches for the
//! same design (`docs/src/design/architecture.md` › Hosts). Today it generates each element (M3.4–M3.6)
//! and assembles them (M3.8); finalizing against the machine and the plan check arrive in M3.9.

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_plan::{MachineProfile, StitchPlan};

use crate::assemble::{Assembled, assemble};
use crate::design::Design;
use crate::generate::{Generation, generate};

/// What planning a design gives.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanOutcome {
    /// The plan; `None` when nothing could be sewn, as the diagnostics say.
    pub plan: Option<StitchPlan>,
    /// What was changed, left out or wrong, element by element, each naming its element when it has
    /// one.
    pub diagnostics: Vec<Diagnostic>,
}

/// The stitch plan for `design`, sewn on the machine `profile` describes. Each element has `budget`'s
/// work to itself; assembly has it once more, and the design `budget`'s stitches.
pub fn plan(design: &Design, profile: &MachineProfile, budget: &Budget) -> PlanOutcome {
    let mut diagnostics = Vec::new();
    let mut generated = Vec::new();
    for element in design.elements() {
        let Generation { generated: sewn, diagnostics: said } = generate(element, &design.settings, profile, budget);
        diagnostics.extend(said);
        generated.extend(sewn.map(|sewn| (element, sewn)));
    }
    let plan = match assemble(&generated, &design.settings, &mut budget.meter()) {
        Ok(Some(Assembled { plan, warnings })) => {
            diagnostics.extend(warnings);
            Some(plan)
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
