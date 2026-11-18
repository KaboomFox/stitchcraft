//! The measured guard of details last (`REQ-AUTO-098`), said with `SC-I0907`.
//!
//! A detail sewn after the larger areas round it (`REQ-AUTO-096`) keeps their rows off it, but it moves
//! fills in the sewing order. A fill that moves starts and ends elsewhere, its own travel runs elsewhere
//! under its rows, and it reaches under other neighbours. On the traced dogs that raised the thickest spot
//! by up to 0.8 fills, at spots where no detail lies, and no rule of geometry told which wishes do it: of
//! the details whose rim the larger area covers less than a share, any share held one dog and raised
//! another. So the plans are measured instead. When the details' wishes change the order, the design is
//! planned both ways, and the plan that sews them last is kept only when its thickest spot, its jump
//! stitches and its thread changes are each no more than the other plan's. The owner's machine has one
//! needle: each thread change is a rethread by hand and each jump a cut, and a thick spot breaks needles,
//! so a detail sewn last is worth none of them.
//!
//! Both plans charge the budget as any plan does, each element with its own work. The thickest spots are
//! compared in hundredths of a fill, as the records keep them, so that the order in which the same stitches
//! are summed can't tell 2 plans apart.

use stitchcraft_core::{Code, Diagnostic};
use stitchcraft_plan::StitchPlan;

use crate::pipeline::PlanOutcome;

/// How a plan measures, as the guard compares it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Measured {
    /// Its thickest spot, in fills (`stitchcraft_plan::thickness`).
    pub(crate) thickest: f64,
    /// Its jump stitches.
    pub(crate) jumps: usize,
    /// Its thread changes.
    pub(crate) changes: usize,
}

impl Measured {
    /// How `plan` measures, its thickest spot `thickest` fills.
    pub(crate) fn of(plan: &StitchPlan, thickest: f64) -> Measured {
        let stats = plan.stats();
        Measured { thickest, jumps: stats.jumps, changes: stats.color_changes }
    }

    /// Whether it is no worse than `other` by any measure, the thickest spots in hundredths of a fill.
    fn within(&self, other: &Measured) -> bool {
        hundredths(self.thickest) <= hundredths(other.thickest) && self.jumps <= other.jumps && self.changes <= other.changes
    }
}

/// `fills` in hundredths, rounded.
fn hundredths(fills: f64) -> f64 {
    (fills * 100.0).round()
}

/// What planning made of a design, and how its plan measures: `None` when it made none.
pub(crate) struct Planned {
    /// The plan and what was said of it.
    pub(crate) outcome: PlanOutcome,
    /// How the plan measures.
    pub(crate) measured: Option<Measured>,
}

/// Of `last`, the plan that sews the details last, and `unwished`, the plan made without that rule, the one
/// kept: `last` when it measures no worse than `unwished` by any measure, or when `unwished` has no plan,
/// and `unwished` otherwise, which `SC-I0907` says.
pub(crate) fn kept(last: Planned, unwished: Planned) -> PlanOutcome {
    match (last.measured, unwished.measured) {
        (Some(with), Some(without)) if with.within(&without) => last.outcome,
        (_, None) => last.outcome,
        (with, Some(without)) => {
            let mut outcome = unwished.outcome;
            outcome.diagnostics.push(worse(with, &without));
            outcome
        }
    }
}

/// `SC-I0907` saying the details sew in the order found without their rule, the plan that sews them last
/// measuring `with`, or made with none, and the plan kept `without`.
fn worse(with: Option<Measured>, without: &Measured) -> Diagnostic {
    let message = match with {
        Some(with) => format!(
            "Sewn after the larger areas round them, the details made the plan worse: its thickest spot {:.2} fills against {:.2}, {} jump \
             stitches against {} and {} thread changes against {}. They sew in the order found without that rule.",
            with.thickest, without.thickest, with.jumps, without.jumps, with.changes, without.changes
        ),
        None => "No plan came of sewing the details after the larger areas round them, so they sew in the order found without that rule.".to_string(),
    };
    Diagnostic::new(Code::AutoOrder, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plan that measures `thickest`, `jumps` and `changes`, or none, with a note `note` of its own.
    fn planned(measured: Option<(f64, usize, usize)>, note: &str) -> Planned {
        let diagnostics = vec![Diagnostic::new(Code::AutoOrder, note)];
        let measured = measured.map(|(thickest, jumps, changes)| Measured { thickest, jumps, changes });
        Planned { outcome: PlanOutcome { plan: None, diagnostics }, measured }
    }

    /// The notes of the plan [`kept`] keeps of `last` and `unwished`.
    fn kept_of(last: Option<(f64, usize, usize)>, unwished: Option<(f64, usize, usize)>) -> Vec<String> {
        kept(planned(last, "last"), planned(unwished, "unwished")).diagnostics.into_iter().map(|said| said.message).collect()
    }

    #[test]
    fn req_auto_098_the_plan_that_sews_the_details_last_is_kept_only_when_it_is_no_worse() {
        let without = Some((4.69, 57, 8));
        // As good by every measure, and better by one: the details sew last.
        for with in [(4.69, 57, 8), (4.68, 57, 8), (4.69, 56, 8), (4.69, 57, 7), (4.694, 57, 8)] {
            assert_eq!(kept_of(Some(with), without), ["last"], "{with:?}");
        }
        // Worse by any one, the thickest spot by a hundredth of a fill: the plan without their rule.
        for with in [(4.70, 57, 8), (4.696, 57, 8), (4.69, 58, 8), (4.69, 57, 9)] {
            assert_eq!(kept_of(Some(with), without).first().map(String::as_str), Some("unwished"), "{with:?}");
        }
        let said = kept_of(Some((5.49, 58, 9)), without);
        assert_eq!(
            said.get(1).map(String::as_str),
            Some(
                "Sewn after the larger areas round them, the details made the plan worse: its thickest spot 5.49 fills against 4.69, 58 jump \
                 stitches against 57 and 9 thread changes against 8. They sew in the order found without that rule."
            )
        );
        // No plan without the rule: the one with it. None with it: the one without, which says so.
        assert_eq!(kept_of(Some((9.0, 99, 99)), None), ["last"]);
        assert_eq!(kept_of(None, None), ["last"]);
        assert_eq!(
            kept_of(None, without),
            ["unwished", "No plan came of sewing the details after the larger areas round them, so they sew in the order found without that rule."]
        );
    }
}
