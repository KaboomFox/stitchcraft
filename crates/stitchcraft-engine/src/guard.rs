//! The measured guard: a change to how a design sews is kept only where its plan measures no worse
//! (`REQ-AUTO-098`).
//!
//! Some changes help where they act and can harm elsewhere, in ways no rule of geometry foresees.
//! Sewing a detail after the fills round it (M13.40) moves fills in the sewing order, and a fill that moves
//! starts and ends elsewhere, travels elsewhere under its rows and reaches under other neighbours: on the
//! traced dogs that raised the thickest spot by up to 0.8 fills, at spots where no detail lies. A thin line
//! kept as a line for likeness (M13.44) can do the same. So such a step plans the design both ways and
//! keeps the changed plan only when none of the measures it names is higher on it than on the plain one.
//! The owner's machine has one needle: each thread change is a rethread by hand and each jump a cut, and a
//! thick spot breaks needles.
//!
//! The measures are those planning already takes, never a second measure: the thickest spot as
//! `crate::auto::thick::finished` measures it with `stitchcraft_plan::thickness`, and the jumps and thread
//! changes of `StitchPlan::stats`. The thickest spots are compared in hundredths of a fill, as the records
//! keep them, so that the order in which the same stitches are summed can't tell 2 plans apart. Both plans
//! charge the budget as any plan does, each element with its own work.
//!
//! M13.40 passes every measure ([`Worse::ALL`]), and the step for thin lines kept for likeness is to pass
//! [`Worse::Thickest`] alone.

use stitchcraft_core::Diagnostic;
use stitchcraft_plan::StitchPlan;

use crate::pipeline::PlanOutcome;

/// A measure on which the changed plan may be no higher than the plain one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Worse {
    /// The thickest spot, in hundredths of a fill.
    Thickest,
    /// The jump stitches.
    JumpThreads,
    /// The thread changes.
    ThreadChanges,
}

impl Worse {
    /// Every measure.
    pub(crate) const ALL: [Worse; 3] = [Worse::Thickest, Worse::JumpThreads, Worse::ThreadChanges];

    /// Whether `changed` is higher than `plain` on this measure.
    fn rises(self, changed: &Measured, plain: &Measured) -> bool {
        match self {
            Worse::Thickest => hundredths(changed.thickest) > hundredths(plain.thickest),
            Worse::JumpThreads => changed.jumps > plain.jumps,
            Worse::ThreadChanges => changed.changes > plain.changes,
        }
    }
}

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

/// Of `plain`, the plan made without a change, and `changed`, the plan made with it, the one kept:
/// `changed` when none of `measures` is higher on it than on `plain`, or when `plain` has no plan. Otherwise
/// `plain`, with what `said` makes of how `changed` measured, `None` when no plan came of it, and of how
/// `plain` measured.
pub(crate) fn kept(plain: Planned, changed: Planned, measures: &[Worse], said: impl FnOnce(Option<Measured>, Measured) -> Diagnostic) -> PlanOutcome {
    let Some(before) = plain.measured else { return changed.outcome };
    match changed.measured {
        Some(after) if !measures.iter().any(|measure| measure.rises(&after, &before)) => changed.outcome,
        after => {
            let mut outcome = plain.outcome;
            outcome.diagnostics.push(said(after, before));
            outcome
        }
    }
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Code;
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;

    /// A plan that measures `thickest`, `jumps` and `changes`, or none, with a note `note` of its own.
    fn planned(measured: Option<(f64, usize, usize)>, note: &str) -> Planned {
        let diagnostics = vec![Diagnostic::new(Code::AutoOrder, note)];
        let measured = measured.map(|(thickest, jumps, changes)| Measured { thickest, jumps, changes });
        Planned { outcome: PlanOutcome { plan: None, diagnostics }, measured }
    }

    /// The notes of the plan [`kept`] keeps of `plain` and `changed` by `measures`, the guard's own note
    /// giving both plans' numbers.
    fn kept_of(plain: Option<(f64, usize, usize)>, changed: Option<(f64, usize, usize)>, measures: &[Worse]) -> Vec<String> {
        let said = |after: Option<Measured>, before: Measured| Diagnostic::new(Code::AutoOrder, format!("{after:?} {before:?}"));
        kept(planned(plain, "plain"), planned(changed, "changed"), measures, said).diagnostics.into_iter().map(|said| said.message).collect()
    }

    #[test]
    fn req_auto_098_the_changed_plan_is_kept_only_when_no_measure_named_rises() {
        let plain = Some((4.69, 57, 8));
        // As good by every measure, and better by one: the changed plan.
        for changed in [(4.69, 57, 8), (4.68, 57, 8), (4.69, 56, 8), (4.69, 57, 7), (4.694, 57, 8)] {
            assert_eq!(kept_of(plain, Some(changed), &Worse::ALL), ["changed"], "{changed:?}");
        }
        // Higher by any one, the thickest spot by a hundredth of a fill: the plain plan, and the note.
        for changed in [(4.70, 57, 8), (4.696, 57, 8), (4.69, 58, 8), (4.69, 57, 9)] {
            assert_eq!(kept_of(plain, Some(changed), &Worse::ALL).first().map(String::as_str), Some("plain"), "{changed:?}");
        }
        assert_eq!(
            kept_of(plain, Some((5.49, 58, 9)), &Worse::ALL),
            ["plain", "Some(Measured { thickest: 5.49, jumps: 58, changes: 9 }) Measured { thickest: 4.69, jumps: 57, changes: 8 }"]
        );
        // Only the measures named count.
        assert_eq!(kept_of(plain, Some((4.69, 99, 99)), &[Worse::Thickest]), ["changed"]);
        assert_eq!(kept_of(plain, Some((9.0, 57, 99)), &[Worse::JumpThreads]), ["changed"]);
        assert_eq!(kept_of(plain, Some((9.0, 99, 8)), &[Worse::ThreadChanges]), ["changed"]);
        assert_eq!(kept_of(plain, Some((4.70, 57, 8)), &[Worse::Thickest]).first().map(String::as_str), Some("plain"));
        assert_eq!(kept_of(plain, Some((4.69, 58, 8)), &[Worse::JumpThreads]).first().map(String::as_str), Some("plain"));
        assert_eq!(kept_of(plain, Some((4.69, 57, 9)), &[Worse::ThreadChanges]).first().map(String::as_str), Some("plain"));
        // No plain plan: the changed one. No changed plan: the plain one, which says so.
        assert_eq!(kept_of(None, Some((9.0, 99, 99)), &Worse::ALL), ["changed"]);
        assert_eq!(kept_of(None, None, &Worse::ALL), ["changed"]);
        assert_eq!(kept_of(plain, None, &Worse::ALL), ["plain", "None Measured { thickest: 4.69, jumps: 57, changes: 8 }"]);
    }

    #[test]
    fn a_plan_measures_its_jumps_and_thread_changes_as_its_counts_say() {
        // 2 jumps and 1 thread change, in 3 stitches.
        let at = |x: f64| stitchcraft_core::Point::new(x, 0.0).unwrap();
        let top = Provenance::plan(Role::Top);
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        builder.stitch(at(0.0), top);
        builder.jump(at(5.0), top);
        builder.jump(at(10.0), top);
        builder.change_thread(Thread::new(Rgb::new(200, 0, 0)));
        builder.stitch(at(10.0), top);
        builder.stitch(at(11.0), top);
        assert_eq!(Measured::of(&builder.finish(), 2.5), Measured { thickest: 2.5, jumps: 2, changes: 1 });
    }
}
