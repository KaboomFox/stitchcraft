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

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId};
use stitchcraft_params::Family;
use stitchcraft_plan::{MachineProfile, StitchPlan};

use crate::assemble::{Assembled, assemble_routed};
use crate::auto::cap::{Pull, Pullable};
use crate::auto::thick::Pulls;
use crate::auto::{Sewing, sewing, thick};
use crate::check;
use crate::design::{Design, Element, Lettered};
use crate::finalize::{Finalized, fitted};
use crate::generate::layered::{Layered, underlay};
use crate::generate::{Generated, Generation, approach, generate_with};
use crate::generators::pull::PullParams;
use crate::generators::satin::SatinParams;
use crate::generators::{Layer, Neighbours};
use crate::moves::Routing;
use crate::names::Names;
use crate::underlays_first::{Candidate, alone, steps};

/// What planning a design gives.
#[derive(Clone, Debug, PartialEq)]
pub struct PlanOutcome {
    /// The plan, fitted to the machine and checked; `None` when it cannot be sewn, as the diagnostics
    /// say.
    pub plan: Option<StitchPlan>,
    /// What was changed, left out or wrong, element by element, each naming its element when it has
    /// one, and its shape's name when that has one.
    pub diagnostics: Vec<Diagnostic>,
    /// The design's shapes, each automatic one without a name named, and each placed in the plan's sewing
    /// order (`REQ-AUTO-125`, [`crate::names`]); none when planning ran out of work before they were.
    pub names: Names,
}

/// The stitch plan for `design`, sewn on the machine `profile` describes. Each element has `budget`'s
/// work to itself, and the stitches the elements before it left; assembling and finalizing have the work
/// once more, and the design `budget`'s stitches.
pub fn plan(design: &Design, profile: &MachineProfile, budget: &Budget) -> PlanOutcome {
    let Sewing { elements, mut diagnostics, base, made, limits, leaning } = sewing(design, budget);
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
            Layer::Whole | Layer::Base => generate_with(element, &design.settings, profile, &neighbours, budget, &mut meter),
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
                let names = named(design, (&elements, &base, &generated), profile, budget, &mut diagnostics);
                return PlanOutcome { plan: None, diagnostics, names };
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
        // Every stitch of the base layer is underlay, its top rows too (`REQ-AUTO-028`).
        let layer = |sewn: Generated| if base.contains(&element.id) { Generated { layer: Layer::Base, ..sewn } } else { sewn };
        generated.extend(sewn.map(|sewn| (element, layer(sewn))));
    }
    // The lettering that digitizing placed is in the cap's reach, as the elements it made are (`REQ-AUTO-119`).
    // A design with lettering is measured, by the rules' default limits when no automatic shape sets them,
    // and one with nothing in the cap's reach only measured (`REQ-AUTO-124`).
    let reach: BTreeSet<ElementId> = made
        .iter()
        .cloned()
        .chain(elements.iter().filter(|element| element.lettered == Some(Lettered::Digitized)).map(|element| element.id.clone()))
        .collect();
    let limits = limits.or_else(|| if elements.iter().any(|element| element.lettered.is_some()) { thick::by_default() } else { None });
    // The satin columns in the cap's reach: it leaves their top stitches as they are (`REQ-AUTO-112`). Those of
    // the lettering are pulled in by their own pull compensation (`REQ-AUTO-121`).
    let satins = || generated.iter().filter(|(element, sewn)| sewn.stitch_type.family() == Family::Satin && reach.contains(&element.id));
    let columns: BTreeSet<ElementId> = satins().map(|(element, _)| element.id.clone()).collect();
    let lettering: BTreeMap<ElementId, Pull> = satins()
        .filter(|(element, _)| element.lettered == Some(Lettered::Digitized))
        .filter_map(|(element, _)| Some((element.id.clone(), Pull::of(&PullParams::from_set(&element.params).ok()?.params))))
        .collect();
    // The columns that move no needle point in as they are made, but lettering's: the cap pulls their crowded
    // points in where a spot sews above the warning's limit, by their insets (`REQ-AUTO-171`), and with
    // `auto_short_stitches` `knots` at tight turns and under later shapes below it (`REQ-AUTO-195`).
    let pulls: Pulls = satins()
        .filter(|(element, _)| element.lettered.is_none())
        .filter_map(|(element, _)| {
            let satin = SatinParams::from_set(&element.params).ok()?.params;
            let shares = satin.short_stitch_inset.iter().map(|percent| percent / 100.0).collect();
            let column = Pullable { shares, spacing: satin.zigzag_spacing_mm.get(), knots: crate::auto::knots(&element.params) };
            (satin.short_stitch_distance_mm.get() <= 0.0).then(|| (element.id.clone(), column))
        })
        .collect();
    let names = named(design, (&elements, &base, &generated), profile, budget, &mut diagnostics);
    let mut meter = budget.meter();
    // Where shapes sewn later cover the way between 2 elements that digitizing made, the needle runs under
    // them and leaves no jump thread (`REQ-AUTO-053`).
    let mut routing = Routing::of(&generated, &made, budget);
    let finished = assemble_routed(&generated, &design.settings, routing.as_mut(), &mut meter).and_then(|assembled| match assembled {
        Some(Assembled { plan, warnings, shortest }) => {
            diagnostics.extend(warnings);
            fitted(plan, profile, &design.settings, &shortest, &mut meter).map(Some)
        }
        None => Ok(None),
    });
    diagnostics.extend(routing.and_then(|routing| routing.said(budget)));
    let plan = match finished {
        Ok(Some(Finalized { plan, diagnostics: said })) => {
            diagnostics.extend(said);
            // A design digitized automatically is thinned where a spot sews above the cap (`REQ-AUTO-074`),
            // told where it sews thick (`SC-W0913`), and is written.
            let cap = |plan: &mut StitchPlan| match &limits {
                Some(held) if reach.is_empty() => thick::measured(plan, held, (&names, &leaning), budget),
                Some(held) => thick::finished(plan, held, (&base, &reach, &columns, &lettering, &pulls), profile, (&names, &leaning), budget),
                None => Vec::new(),
            };
            let asked = check::asked(generated.iter().map(|(element, sewn)| (&element.id, sewn)));
            plan.and_then(|plan| after_finalize(plan, cap, profile, &asked, &mut diagnostics))
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
    names.tell(&mut diagnostics);
    PlanOutcome { plan, diagnostics, names }
}

/// `plan`, finalized, through the stages that come after finalize, `cap` (on an automatic design the
/// thinning pass and the cap, which take needle points out), and then the check of the plan that is
/// returned ([`check`]): a plan that breaks it is not returned, and `diagnostics` gets its `SC-E0009`.
pub(crate) fn after_finalize(
    mut plan: StitchPlan,
    cap: impl FnOnce(&mut StitchPlan) -> Vec<Diagnostic>,
    profile: &MachineProfile,
    asked: &BTreeMap<ElementId, (bool, bool)>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Option<StitchPlan> {
    diagnostics.extend(cap(&mut plan));
    check::checked(plan, profile, asked).map_err(|said| diagnostics.extend(said)).ok()
}

/// The shapes of `design` named (`REQ-AUTO-125`) by how `elements`, the elements as they are generated, sew
/// them, but for the base layer's pieces, `base`; their threads named by `profile`'s palette, and each
/// placed by the first of the `generated` steps that sews it. Naming has `budget`'s work, and one that needs
/// more names nothing: `diagnostics` then says so.
fn named(
    design: &Design,
    (elements, base, generated): (&[Element], &[ElementId], &[(&Element, Generated)]),
    profile: &MachineProfile,
    budget: &Budget,
    diagnostics: &mut Vec<Diagnostic>,
) -> Names {
    let order: Vec<&ElementId> = generated.iter().map(|(element, _)| &element.id).collect();
    Names::given(design, (elements, base, &order), profile.palette.palette(), &mut budget.meter()).unwrap_or_else(|exhausted| {
        diagnostics.push(exhausted.diagnostic(budget, None));
        Names::default()
    })
}
