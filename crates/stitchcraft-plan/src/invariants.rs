//! The plan invariants: rules every plan satisfies before StitchCraft writes it (conformance level L0).
//!
//! The engine is supposed to produce plans that follow these rules, so a violation is a bug in whatever
//! built the plan, never a user error. The command line refuses to write a plan that violates them and
//! reports `SC-E0009` ([`Violation::diagnostic`]); the conformance suite runs the checker on every plan it
//! produces, whatever the case is about (`docs/src/design/data-model.md#plan-invariants`).
//!
//! | Requirement | Rule |
//! |---|---|
//! | REQ-PLAN-001 | Every position is inside the profile's hoop, centred on the machine origin. |
//! | REQ-PLAN-002 | While sewing, every stitch is between the profile's minimum (locks: 0.2 mm) and maximum length. |
//! | REQ-PLAN-003 | Every block sews at least one stitch; trims and stops happen where the needle is. |
//! | REQ-PLAN-005 | While sewing, a stitch never lands where the needle already is. |
//! | REQ-PLAN-006 | Colour changes plus stops fit the profile's format. |
//!
//! "While sewing" means the previous movement was a `Normal` stitch with no trim since: the stitch then
//! lays thread between two needle holes, and its length is what the machine and the fabric feel. The first
//! stitch after a jump, a trim or a thread change starts a new run and has no such length.
//!
//! The checker is linear in the plan's length and allocates only its report. Plans are bounded by the
//! stitch budget that produced them (or a reader's cap), so it needs no meter of its own; it reports at
//! most [`MAX_REPORTED`] violations so a badly broken plan cannot flood a report.

use core::fmt;

use stitchcraft_core::{Code, Diagnostic, Mm, Point};

use crate::plan::{Role, StitchKind, StitchPlan};
use crate::profile::MachineProfile;

/// Requirement ids, as in `conformance/requirements.toml` (a test checks they exist there).
pub mod req {
    /// Positions inside the hoop.
    pub const INSIDE_HOOP: &str = "REQ-PLAN-001";
    /// Stitch lengths within the profile's limits.
    pub const STITCH_LENGTH: &str = "REQ-PLAN-002";
    /// Block structure and command positions.
    pub const STRUCTURE: &str = "REQ-PLAN-003";
    /// No stitch in place.
    pub const NO_STITCH_IN_PLACE: &str = "REQ-PLAN-005";
    /// Colour changes within the format's limit.
    pub const COLOR_CHANGES: &str = "REQ-PLAN-006";
}

/// The shortest lock (tie-in, tie-off) stitch: locks are deliberately tiny back-and-forth stitches.
pub const LOCK_MIN_STITCH: Mm = Mm::from_tenths(2);

/// The most violations one check reports.
pub const MAX_REPORTED: usize = 50;

/// Lengths are compared with this slack (mm), far below the 0.1 mm resolution of machine files, so that
/// floating-point rounding in a length that is exactly at a limit is not reported.
const LENGTH_SLACK: f64 = 1e-9;

/// A broken plan invariant.
#[derive(Clone, Debug, PartialEq)]
pub struct Violation {
    /// The requirement it breaks (see [`req`]).
    pub requirement: &'static str,
    /// The colour block.
    pub block: usize,
    /// The entry within the block, when the violation concerns one.
    pub stitch: Option<usize>,
    /// What is wrong, specifically.
    pub message: String,
}

impl Violation {
    /// The `SC-E0009` diagnostic for this violation: an internal check failed, nothing was written.
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic::new(Code::InternalCheckFailed, format!("The stitch plan breaks {self}"))
    }
}

/// `REQ-PLAN-002 at block 1, entry 14: …`
impl fmt::Display for Violation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.stitch {
            Some(stitch) => write!(f, "{} at block {}, entry {}: {}", self.requirement, self.block, stitch, self.message),
            None => write!(f, "{} at block {}: {}", self.requirement, self.block, self.message),
        }
    }
}

/// Checks `plan` against the invariants for `profile`; empty when the plan is valid.
pub fn check(plan: &StitchPlan, profile: &MachineProfile) -> Vec<Violation> {
    let mut report = Report::default();
    let (half_width, half_height) = (profile.hoop.width.get() / 2.0, profile.hoop.height.get() / 2.0);
    let mut needle = Point::ORIGIN;

    for (b, block) in plan.blocks.iter().enumerate() {
        if !block.stitches.iter().any(|s| s.kind == StitchKind::Normal) {
            report.add(req::STRUCTURE, b, None, "the colour block sews no stitches".to_string());
        }
        // A thread change ends the run: the first stitch of a block starts a new one.
        let mut sewing = false;
        for (i, stitch) in block.stitches.iter().enumerate() {
            let at = stitch.at;
            if at.x().abs() > half_width || at.y().abs() > half_height {
                let message = format!(
                    "({:.2}, {:.2}) mm is outside the {} × {} mm hoop centred on the origin",
                    at.x(),
                    at.y(),
                    profile.hoop.width.get(),
                    profile.hoop.height.get()
                );
                report.add(req::INSIDE_HOOP, b, Some(i), message);
            }
            match stitch.kind {
                StitchKind::Normal => {
                    if sewing {
                        let length = needle.distance(at);
                        let min = if stitch.origin.role == Role::Lock { LOCK_MIN_STITCH } else { profile.min_stitch };
                        if length == 0.0 {
                            report.add(req::NO_STITCH_IN_PLACE, b, Some(i), "the stitch lands where the needle already is".to_string());
                        } else if length + LENGTH_SLACK < min.get() {
                            report.add(req::STITCH_LENGTH, b, Some(i), format!("the stitch is {length:.3} mm, shorter than {} mm", min.get()));
                        } else if length - LENGTH_SLACK > profile.max_stitch.get() {
                            let max = profile.max_stitch.get();
                            report.add(req::STITCH_LENGTH, b, Some(i), format!("the stitch is {length:.3} mm, longer than {max} mm"));
                        }
                    }
                    sewing = true;
                    needle = at;
                }
                StitchKind::Jump => {
                    sewing = false;
                    needle = at;
                }
                StitchKind::Trim | StitchKind::Stop => {
                    if at != needle {
                        let what = if stitch.kind == StitchKind::Trim { "trim" } else { "stop" };
                        let message = format!(
                            "the {what} is at ({:.2}, {:.2}) mm but the needle is at ({:.2}, {:.2}) mm",
                            at.x(),
                            at.y(),
                            needle.x(),
                            needle.y()
                        );
                        report.add(req::STRUCTURE, b, Some(i), message);
                    }
                    if stitch.kind == StitchKind::Trim {
                        sewing = false;
                    }
                }
            }
        }
    }

    let changes = plan.stats().changes_including_stops();
    let max = profile.format.max_color_changes();
    if changes > max {
        let message = format!("{changes} colour changes and stops, but {} records at most {max}", profile.format.name());
        report.add(req::COLOR_CHANGES, plan.blocks.len().saturating_sub(1), None, message);
    }
    report.violations
}

#[derive(Default)]
struct Report {
    violations: Vec<Violation>,
}

impl Report {
    fn add(&mut self, requirement: &'static str, block: usize, stitch: Option<usize>, message: String) {
        if self.violations.len() < MAX_REPORTED {
            self.violations.push(Violation { requirement, block, stitch, message });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::PlanBuilder;
    use crate::plan::{Provenance, Stitch};
    use crate::profiles::BROTHER_200X200;
    use crate::thread::{Rgb, Thread};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn top() -> Provenance {
        Provenance::plan(Role::Top)
    }

    /// A small valid plan: jump in, a few 2.5 mm stitches, a trim, a second colour, a stop.
    fn valid() -> PlanBuilder {
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        b.jump(p(-10.0, 0.0), Provenance::plan(Role::Travel));
        for i in 0..=4 {
            b.stitch(p(-10.0 + 2.5 * f64::from(i), 0.0), top());
        }
        b.trim(None);
        b.change_thread(Thread::new(Rgb::new(255, 0, 0)));
        b.jump(p(10.0, 10.0), Provenance::plan(Role::Travel));
        b.stitch(p(10.0, 10.0), top());
        b.stitch(p(12.0, 10.0), top());
        b.stop(None);
        b.stitch(p(14.0, 10.0), top());
        b
    }

    fn requirements(plan: &StitchPlan) -> Vec<&'static str> {
        check(plan, &BROTHER_200X200).iter().map(|v| v.requirement).collect()
    }

    #[test]
    fn a_valid_plan_passes() {
        assert_eq!(check(&valid().finish(), &BROTHER_200X200), Vec::new());
    }

    #[test]
    fn req_plan_001_positions_outside_the_hoop() {
        let mut b = valid();
        b.jump(p(100.5, 0.0), top());
        b.stitch(p(100.5, 1.0), top());
        assert_eq!(requirements(&b.finish()), vec![req::INSIDE_HOOP, req::INSIDE_HOOP]);
        let mut edge = valid();
        edge.jump(p(100.0, -100.0), top());
        edge.stitch(p(100.0, -99.0), top());
        assert_eq!(requirements(&edge.finish()), Vec::<&str>::new(), "the hoop's edge is inside");
    }

    #[test]
    fn req_plan_002_stitch_lengths() {
        let mut long = valid();
        long.stitch(p(26.1, 10.0), top());
        let report = check(&long.finish(), &BROTHER_200X200);
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].to_string(), "REQ-PLAN-002 at block 1, entry 5: the stitch is 12.100 mm, longer than 12 mm");

        let mut short = valid();
        short.stitch(p(14.2, 10.0), top());
        assert_eq!(requirements(&short.finish()), vec![req::STITCH_LENGTH]);

        let mut lock = valid();
        lock.stitch(p(14.2, 10.0), Provenance::plan(Role::Lock));
        assert_eq!(requirements(&lock.finish()), Vec::<&str>::new(), "locks may be 0.2 mm");

        let mut exact = valid();
        exact.stitch(p(26.0, 10.0), top());
        exact.stitch(p(26.3, 10.0), top());
        assert_eq!(requirements(&exact.finish()), Vec::<&str>::new(), "limits are inclusive");
    }

    #[test]
    fn runs_restart_after_jumps_trims_and_thread_changes_but_not_stops() {
        let mut b = valid();
        b.jump(p(60.0, 10.0), top());
        b.stitch(p(70.0, 10.0), top()); // first stitch after a jump: no length rule
        b.trim(None);
        b.stitch(p(90.0, 10.0), top()); // first stitch after a trim: no length rule
        assert_eq!(requirements(&b.finish()), Vec::<&str>::new());

        let mut stop = valid();
        stop.stop(None);
        stop.stitch(p(40.0, 10.0), top()); // a stop does not cut the thread: 26 mm stitch
        assert_eq!(requirements(&stop.finish()), vec![req::STITCH_LENGTH]);
    }

    #[test]
    fn req_plan_003_empty_blocks_and_misplaced_commands() {
        let mut b = valid();
        b.change_thread(Thread::new(Rgb::new(0, 0, 255)));
        b.jump(p(0.0, 0.0), top());
        let report = check(&b.finish(), &BROTHER_200X200);
        assert_eq!(report.len(), 1);
        assert_eq!(report[0].to_string(), "REQ-PLAN-003 at block 2: the colour block sews no stitches");

        let mut plan = valid().finish();
        plan.blocks[0].stitches.push(Stitch { at: p(5.0, 5.0), kind: StitchKind::Trim, origin: Provenance::plan(Role::Command) });
        assert_eq!(requirements(&plan), vec![req::STRUCTURE]);
    }

    #[test]
    fn req_plan_005_no_stitch_in_place() {
        let mut b = valid();
        b.stitch(p(14.0, 10.0), top());
        assert_eq!(requirements(&b.finish()), vec![req::NO_STITCH_IN_PLACE]);
        let mut after_jump = valid();
        after_jump.jump(p(20.0, 10.0), top());
        after_jump.stitch(p(20.0, 10.0), top());
        assert_eq!(requirements(&after_jump.finish()), Vec::<&str>::new(), "the first stitch after a jump goes down where the jump ended");
    }

    #[test]
    fn req_plan_006_colour_changes_fit_the_format() {
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        b.jump(p(0.0, 0.0), top());
        b.stitch(p(0.0, 0.0), top());
        for _ in 0..255 {
            b.stitch(p(1.0, 0.0), top());
            b.stop(None);
            b.stitch(p(0.0, 0.0), top());
        }
        assert_eq!(requirements(&b.clone().finish()), Vec::<&str>::new(), "255 changes fit PES");
        b.stop(None);
        assert_eq!(requirements(&b.finish()), vec![req::COLOR_CHANGES]);
    }

    #[test]
    fn reports_are_capped() {
        let mut b = valid();
        for i in 0..200 {
            b.stitch(p(150.0 + f64::from(i), 0.0), top());
        }
        assert_eq!(check(&b.finish(), &BROTHER_200X200).len(), MAX_REPORTED);
    }

    #[test]
    fn violations_become_internal_check_diagnostics() {
        let mut b = valid();
        b.stitch(p(14.0, 10.0), top());
        let d = check(&b.finish(), &BROTHER_200X200)[0].diagnostic();
        assert_eq!(d.code, Code::InternalCheckFailed);
        assert!(d.message.starts_with("The stitch plan breaks REQ-PLAN-005 at block 1, entry 5"));
    }

    #[test]
    fn requirement_ids_exist_in_the_requirements_file() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../conformance/requirements.toml");
        let text = std::fs::read_to_string(path).unwrap();
        for id in [req::INSIDE_HOOP, req::STITCH_LENGTH, req::STRUCTURE, req::NO_STITCH_IN_PLACE, req::COLOR_CHANGES] {
            assert!(text.contains(&format!("id = \"{id}\"")), "{id} is not in conformance/requirements.toml");
        }
    }
}
