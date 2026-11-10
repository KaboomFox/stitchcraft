//! The work the cut-out may spend, a budget every loop over the picture charges, so that no picture can make
//! it run on (`AGENTS.md` › Never crash). It runs out with `SC-E0004`, worded for the cut-out.

use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Meter};

/// The work the cut-out may spend: every loop over the picture charges it, and when it runs out the cut-out
/// stops with `SC-E0004`.
#[derive(Clone, Debug)]
pub struct Work {
    budget: Budget,
    meter: Meter,
}

impl Work {
    /// The work `budget` allows.
    pub const fn new(budget: Budget) -> Work {
        Work { budget, meter: budget.meter() }
    }

    /// Spends `units` of work, one for each pixel or value a loop visits: `SC-E0004` once the budget runs
    /// out.
    pub fn charge(&mut self, units: u64) -> Result<(), Diagnostic> {
        self.meter.charge(units).map_err(|_| {
            Diagnostic::new(
                Code::BudgetExhausted,
                format!(
                    "Cutting the subject out of the picture needed more than the work budget of {} units, so nothing was cut out.",
                    self.budget.max_work
                ),
            )
            .with_fix(Fix::Hint("Give a smaller picture.".to_string()))
        })
    }

    /// Spends a unit for each of `count` values a loop visits.
    pub fn charge_each(&mut self, count: usize) -> Result<(), Diagnostic> {
        self.charge(u64::try_from(count).unwrap_or(u64::MAX))
    }

    /// The work units still available.
    pub const fn left(&self) -> u64 {
        self.meter.work_left()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn work_runs_out_with_sc_e0004() {
        let mut work = Work::new(Budget { max_work: 10, ..Budget::DEFAULT });
        work.charge_each(10).unwrap();
        let said = work.charge(1).unwrap_err();
        assert_eq!(said.code, Code::BudgetExhausted);
        assert_eq!(said.message, "Cutting the subject out of the picture needed more than the work budget of 10 units, so nothing was cut out.");
        assert_eq!(said.fix.map(|fix| fix.describe()).as_deref(), Some("Give a smaller picture."));
    }
}
