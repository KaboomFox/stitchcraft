//! The engine's entry point: a design to a stitch plan.
//!
//! Design: `docs/src/design/engine-pipeline.md`. Every host calls [`plan`] — the command line with an SVG
//! file's design, the VectorCraft plug-in with a document's — so every host gets the same stitches for the
//! same design (`docs/src/design/architecture.md` › Hosts). It generates each element, assembles them,
//! fits the plan to the machine and checks it: a plan that comes back can be written as it is.
//!
//! The shapes without settings that the host marked automatic are digitized first ([`crate::auto`]): each
//! becomes the elements its width calls for, with the settings that say so, and from there on is sewn as
//! any element is. The elements are put in the order they sew there too: automatic elements, and every
//! element of a design that asks for its threads sorted, in an order with few thread changes.
//!
//! Then the runs of underlays first are found, in that order ([`crate::underlays_first`]): each element of
//! a run with an underlay is sewn as 2 layers, its underlay in the run's first pass and its top in the
//! second, from one reading of it ([`crate::generate::layered`]). Every other element is one step, whole.
//!
//! Steps are generated in sewing order, each with its neighbours (`REQ-GEN-003`,
//! `docs/src/design/adr/0014-generators-see-their-neighbours.md`): the last needle point of the steps
//! before it that sew any, and what the next step's element offers to end near. Each is given the stitches
//! the ones before it left (`REQ-GEN-004`), and one that needs more ends planning there: the design needs
//! more stitches than it may have, so no plan could come of it, and making the rest would only fill memory.

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_plan::{MachineProfile, StitchPlan};

use crate::assemble::{Assembled, assemble};
use crate::auto::digitized;
use crate::design::{Design, Element};
use crate::finalize::{Finalized, finalize};
use crate::generate::layered::{Layered, underlay};
use crate::generate::{Generated, Generation, approach, generate_with};
use crate::generators::{Layer, Neighbours};
use crate::underlays_first::{Candidate, alone, steps};

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
    let (elements, mut diagnostics) = digitized(design, budget);
    let candidates: Vec<Candidate> = elements.iter().map(|element| Candidate::of(element, &design.settings)).collect();
    let (steps, lone) = steps(&candidates);
    diagnostics.extend(lone.into_iter().filter_map(|(k, why)| elements.get(k).map(|element| alone(element, why))));
    let mut generated: Vec<(&Element, Generated)> = Vec::new();
    let mut needle = None;
    let mut left = budget.max_stitches;
    // The elements of a run between their underlay and their top, by their places.
    let mut held: BTreeMap<usize, Layered> = BTreeMap::new();
    for (i, step) in steps.iter().enumerate() {
        let Some(element) = elements.get(step.element) else { continue };
        let next = steps.get(i + 1).and_then(|next| elements.get(next.element)).and_then(|next| approach(next, &design.settings, budget));
        let neighbours = Neighbours { needle, next };
        let mut meter = Budget { max_stitches: left, ..*budget }.meter();
        let generation = match step.layer {
            Layer::Whole => generate_with(element, &design.settings, profile, &neighbours, budget, &mut meter),
            Layer::Underlay => underlay(element, &design.settings, profile, &neighbours, budget, meter).map(|(generation, layered)| {
                held.extend(layered.map(|layered| (step.element, layered)));
                generation
            }),
            // An element skipped at its underlay sews nothing more.
            Layer::Top => held
                .remove(&step.element)
                .map_or(Ok(Generation { generated: None, diagnostics: Vec::new() }), |layered| layered.top(&neighbours, budget, left)),
        };
        let Generation { generated: sewn, diagnostics: said } = match generation {
            Ok(generation) => generation,
            Err(said) => {
                diagnostics.extend(said);
                return PlanOutcome { plan: None, diagnostics };
            }
        };
        // A top whose work ran out skips its element: its underlay is left out too, and its stitches come back.
        if step.layer == Layer::Top && sewn.is_none() {
            let underlay = generated.iter().position(|(sewing, layer)| sewing.id == element.id && layer.layer == Layer::Underlay);
            let given = underlay.map(|at| generated.remove(at).1.groups.iter().map(Vec::len).sum::<usize>()).unwrap_or(0);
            left = left.saturating_add(u32::try_from(given).unwrap_or(u32::MAX));
        }
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
