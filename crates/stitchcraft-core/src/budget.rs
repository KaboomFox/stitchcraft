//! Work budgets: how StitchCraft guarantees that it finishes.
//!
//! An accidental or hostile input — a path with a million nodes, a fill with 0.001 mm row spacing —
//! must not make StitchCraft run for hours or exhaust memory, on the command line or inside
//! VectorCraft's five-second live-effect window. So every loop over input charges a [`Meter`], and when
//! the budget runs out the work stops with [`Exhausted`], which hosts report as `SC-E0004`
//! ([`Exhausted::diagnostic`]). Other elements still plan.
//!
//! Work is counted in abstract units (one per inner-loop iteration: a scanline crossing, a graph edge
//! visit, an offset step), never in time, so a budget behaves identically on every machine and the
//! result never depends on how fast the computer is (`docs/src/design/determinism.md`).

use crate::diag::{Code, Diagnostic, Fix};
use crate::element::ElementId;

/// The limits for one planning call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The most stitches one design may have.
    pub max_stitches: u32,
    /// The most work units one call may spend.
    pub max_work: u64,
}

impl Budget {
    /// Generous defaults for the command line: 2,000,000 stitches (far beyond any home-machine design)
    /// and 500,000,000 work units. Hosts with a time limit, such as the VectorCraft plug-in, pass
    /// smaller budgets.
    pub const DEFAULT: Budget = Budget { max_stitches: 2_000_000, max_work: 500_000_000 };

    /// A fresh meter for this budget.
    pub const fn meter(self) -> Meter {
        Meter { work_left: self.max_work, stitches_left: self.max_stitches }
    }
}

impl Default for Budget {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// What is left of a budget. Charging never wraps around and never panics; once a limit is reached,
/// every further charge fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Meter {
    work_left: u64,
    stitches_left: u32,
}

/// Which limit ran out.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum Exhausted {
    /// The work budget.
    #[error("the work budget is exhausted")]
    Work,
    /// The stitch budget.
    #[error("the stitch budget is exhausted")]
    Stitches,
}

impl Meter {
    /// Spends `units` of work.
    pub fn charge(&mut self, units: u64) -> Result<(), Exhausted> {
        match self.work_left.checked_sub(units) {
            Some(left) => {
                self.work_left = left;
                Ok(())
            }
            None => {
                self.work_left = 0;
                Err(Exhausted::Work)
            }
        }
    }

    /// Spends `count` stitches.
    pub fn charge_stitches(&mut self, count: u32) -> Result<(), Exhausted> {
        match self.stitches_left.checked_sub(count) {
            Some(left) => {
                self.stitches_left = left;
                Ok(())
            }
            None => {
                self.stitches_left = 0;
                Err(Exhausted::Stitches)
            }
        }
    }

    /// Work units still available.
    pub const fn work_left(&self) -> u64 {
        self.work_left
    }

    /// Stitches still available.
    pub const fn stitches_left(&self) -> u32 {
        self.stitches_left
    }
}

impl Exhausted {
    /// The `SC-E0004` diagnostic hosts show, for `element` when the work ran out while planning it.
    pub fn diagnostic(self, budget: &Budget, element: Option<ElementId>) -> Diagnostic {
        let diagnostic = match self {
            Exhausted::Work => Diagnostic::new(
                Code::BudgetExhausted,
                format!("Planning this element needed more than the work budget of {} units, so it was skipped.", budget.max_work),
            )
            .with_fix(Fix::Hint("Simplify the element (fewer nodes, wider spacing) or split it into smaller elements.".to_string())),
            Exhausted::Stitches => Diagnostic::new(
                Code::BudgetExhausted,
                format!("The design needs more than {} stitches, the limit for one design.", budget.max_stitches),
            )
            .with_fix(Fix::Hint("Split the design into several files, or use wider spacing on large fills.".to_string())),
        };
        match element {
            Some(id) => diagnostic.with_element(id),
            None => diagnostic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn charges_count_down_and_stop_at_zero() {
        let mut meter = Budget { max_stitches: 10, max_work: 100 }.meter();
        assert_eq!(meter.charge(60), Ok(()));
        assert_eq!(meter.work_left(), 40);
        assert_eq!(meter.charge(41), Err(Exhausted::Work));
        assert_eq!(meter.work_left(), 0);
        assert_eq!(meter.charge(1), Err(Exhausted::Work), "an exhausted meter stays exhausted");
        assert_eq!(meter.charge(0), Ok(()));
    }

    #[test]
    fn stitches_are_counted_separately() {
        let mut meter = Budget { max_stitches: 3, max_work: u64::MAX }.meter();
        assert_eq!(meter.charge_stitches(3), Ok(()));
        assert_eq!(meter.charge_stitches(1), Err(Exhausted::Stitches));
        assert_eq!(meter.work_left(), u64::MAX);
    }

    #[test]
    fn diag_sc_e0004_running_out_names_the_limit_and_suggests_a_fix() {
        let budget = Budget::DEFAULT;
        let id = ElementId::new("svg:path7").unwrap();
        let work = budget.meter().charge(budget.max_work + 1).unwrap_err().diagnostic(&budget, Some(id.clone()));
        assert_eq!(work.to_string(), "error SC-E0004: Planning this element needed more than the work budget of 500000000 units, so it was skipped.");
        assert_eq!(work.element, Some(id));
        assert_eq!(
            work.fix.map(|f| f.describe()).as_deref(),
            Some("Simplify the element (fewer nodes, wider spacing) or split it into smaller elements.")
        );
        let stitches = budget.meter().charge_stitches(budget.max_stitches + 1).unwrap_err().diagnostic(&budget, None);
        assert_eq!(stitches.to_string(), "error SC-E0004: The design needs more than 2000000 stitches, the limit for one design.");
        assert_eq!(stitches.fix.map(|f| f.describe()).as_deref(), Some("Split the design into several files, or use wider spacing on large fills."));
    }
}
