//! Thickness before threading (`REQ-AUTO-189`, `docs/src/design/image-to-embroidery.md` › Each thread threaded
//! once): a design threaded once that sews above the warning's limit is planned again with each line after the
//! fills it borders, and sews that way when that keeps it at the limit or under it.
//!
//! With `auto_thread_once` on, a line may sew before a fill it borders so that its thread is threaded once
//! (`REQ-AUTO-184`). The fill's rows then end on the line, where they would have ended under it, and what
//! the last pass and the cap may take of them changes: a row's end under a later line is hidden, one over an
//! earlier line shows. Where several fills and lines meet, that can raise a spot above the limit. The owner's
//! first concern is whether her machine sews the design, so the thickness limit outweighs the threadings
//! saved. Only a design above the limit, in an order that the strict rules wouldn't allow, pays for the second
//! plan: an order they allow is itself a strict order with as few thread changes (`Sewing::once`).

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_plan::StitchPlan;
use stitchcraft_plan::thickness::{Measure, Thickness};

use crate::PlanOutcome;

/// `outcome`, the plan of a design that threaded each thread once where its shapes allow it, or else the one
/// `strict` plans with each line after the fills it borders, when `outcome`'s plan sews above `limit` fills
/// at a spot and that plan sews at `limit` or under it everywhere: then it comes back with `SC-W0927`, which
/// says how many more times its threads are threaded. A plan is measured only when the thickness warning
/// (`SC-W0913`) is said of it, each measure with `budget`'s work, and a design whose plan can't be measured
/// or made again sews as it was planned.
pub(crate) fn under(outcome: PlanOutcome, limit: f64, strict: impl FnOnce() -> PlanOutcome, budget: &Budget) -> PlanOutcome {
    let thickest = |plan: &StitchPlan| match Thickness::of(plan, &mut budget.meter()) {
        Ok(Some(thickness)) => Some(thickness.most(Measure::Fills).value),
        _ => None,
    };
    let warned = |outcome: &PlanOutcome| outcome.diagnostics.iter().any(|said| said.code == Code::AutoThick);
    let Some(once) = outcome.plan.as_ref().filter(|_| warned(&outcome)).and_then(thickest) else { return outcome };
    if once <= limit {
        return outcome;
    }
    let mut again = strict();
    let Some(strict) = again.plan.as_ref().and_then(|plan| thickest(plan).map(|most| (plan, most))) else { return outcome };
    let (Some(threaded), (plan, most)) = (outcome.plan.as_ref().map(threadings), strict) else { return outcome };
    if most > limit {
        return outcome;
    }
    let more = threadings(plan).saturating_sub(threaded);
    again.diagnostics.push(Diagnostic::new(Code::AutoThreadedForThickness, said(once, most, limit, more, threadings(plan))));
    again
}

/// How many times the threads of `plan` are threaded: once for each run of blocks one after another in one
/// thread, as a stop needs no threading.
pub(crate) fn threadings(plan: &StitchPlan) -> usize {
    let mut last = None;
    plan.blocks
        .iter()
        .filter(|block| {
            let new = last != Some(block.thread.color);
            last = Some(block.thread.color);
            new
        })
        .count()
}

/// What `SC-W0927` says: the thickest spot threaded once, `once` fills, and with each line after its fills,
/// `most`, against `limit`, and the `more` threadings that costs, of `all`.
fn said(once: f64, most: f64, limit: f64, more: usize, all: usize) -> String {
    let times = |count: usize| if count == 1 { "1 more time".to_string() } else { format!("{count} more times") };
    format!(
        "Threaded once, a spot of the design sews {once:.2} fills, above the limit of {limit}. Sewn with each line after the fills it borders, the design sews {most:.2} fills at the most, so it sews that way: its threads are threaded {}, {all} times in all.",
        times(more)
    )
}

#[cfg(test)]
mod tests;
