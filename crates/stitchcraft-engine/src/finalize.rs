//! Pipeline stages 5 and 6 (finalize and check): the assembled plan fitted to the machine, then checked.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Finalize and › Check. Generators already keep their own
//! stitches within the machine's limits; what is left for here comes from joining things up and from
//! settings the machine cannot follow:
//!
//! 1. **The shortest stitch.** Where one element's stitching runs straight on into the next, the stitch
//!    between them can be anything up to the collapse length, 0 included. A needle point less than the
//!    shortest stitch from the one before is left out, so the stitch runs on to the next. The first and
//!    last points of a run of stitches (where the needle lands, and where a jump, trim or stop follows)
//!    and lock points always stay, and leave out the ones before them instead. A stitch into or out of a
//!    lock point is a lock stitch, whose shortest is 0.2 mm. `SC-I0504` says how many were left out.
//! 2. **The longest stitch.** A stitch longer than the machine's is split into equal parts: a hand-placed
//!    stitch, say, or a custom lock's long step (`SC-I0703`).
//! 3. **The machine.** Too many colour changes and stops for the machine's format is `SC-E0601`; a design
//!    larger than the hoop, or reaching past its edge from an origin far from the design's middle,
//!    `SC-E0701`; one larger than its comfort zone `SC-W0702`.
//! 4. **The check.** The plan invariants (`stitchcraft_plan::invariants`) must hold; a broken one is a bug
//!    in StitchCraft (`SC-E0009`), and nothing is written.

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Rect};
use stitchcraft_plan::invariants::{self, LOCK_MIN_STITCH};
use stitchcraft_plan::{MachineProfile, Role, Stitch, StitchKind, StitchPlan};

use crate::design::DesignSettings;
use crate::generators::mm;

/// A plan fitted to the machine and checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Finalized {
    /// The plan; `None` when the machine cannot sew it or a check failed, as the diagnostics say.
    pub plan: Option<StitchPlan>,
    /// What finalizing changed, and why the plan is missing when it is.
    pub diagnostics: Vec<Diagnostic>,
}

/// `plan` fitted to the machine `profile` describes, for a design with `settings`, then checked. Every
/// entry costs a unit of work from `meter`.
pub fn finalize(mut plan: StitchPlan, profile: &MachineProfile, settings: &DesignSettings, meter: &mut Meter) -> Result<Finalized, Exhausted> {
    let shortest = settings.min_stitch_len.and_then(|own| Mm::new(own.get().max(profile.min_stitch.get())).ok()).unwrap_or(profile.min_stitch);
    let mut fitted = Fitted::default();
    for block in &mut plan.blocks {
        block.stitches = fitted.block(&block.stitches, shortest.get(), profile.max_stitch.get(), meter)?;
    }
    let mut diagnostics = fitted.diagnostics(shortest.get(), profile.max_stitch.get());
    let changes = plan.stats().changes_including_stops();
    let max = profile.format.max_color_changes();
    if changes > max {
        let message = format!("The design has {changes} colour changes and stops, but {} records at most {max}.", profile.format.name());
        diagnostics.push(Diagnostic::new(Code::TooManyColorChanges, message));
        return Ok(Finalized { plan: None, diagnostics });
    }
    if let Some(fit) = plan.bounds().and_then(|bounds| profile.check_fit(bounds).or_else(|| off_centre(bounds, profile))) {
        let fatal = fit.code == Code::OutsideHoop;
        diagnostics.push(fit);
        if fatal {
            return Ok(Finalized { plan: None, diagnostics });
        }
    }
    let violations = invariants::check(&plan, profile);
    if violations.is_empty() {
        return Ok(Finalized { plan: Some(plan), diagnostics });
    }
    diagnostics.extend(violations.iter().map(invariants::Violation::diagnostic));
    Ok(Finalized { plan: None, diagnostics })
}

/// What fitting the plan changed.
#[derive(Default)]
struct Fitted {
    merged: usize,
    split: usize,
}

impl Fitted {
    /// A block's entries with each run of stitches fitted: the stitches between two other entries (a
    /// jump, a trim, a stop) or the block's ends.
    fn block(&mut self, entries: &[Stitch], shortest: f64, longest: f64, meter: &mut Meter) -> Result<Vec<Stitch>, Exhausted> {
        let mut out = Vec::with_capacity(entries.len());
        let mut run: Vec<Stitch> = Vec::new();
        for &entry in entries {
            meter.charge(1)?;
            if entry.kind == StitchKind::Normal {
                run.push(entry);
            } else {
                self.flush(&mut run, &mut out, shortest, longest, meter)?;
                out.push(entry);
            }
        }
        self.flush(&mut run, &mut out, shortest, longest, meter)?;
        Ok(out)
    }

    /// Fits the run of stitches gathered so far and moves it to `out`. Its first point is where the needle
    /// lands and its last where the next entry happens, so both stay, as lock points do.
    fn flush(&mut self, run: &mut Vec<Stitch>, out: &mut Vec<Stitch>, shortest: f64, longest: f64, meter: &mut Meter) -> Result<(), Exhausted> {
        let last = run.len().saturating_sub(1);
        let mut kept: Vec<Stitch> = Vec::with_capacity(run.len());
        for (i, point) in run.drain(..).enumerate() {
            let Some(&from) = kept.last() else {
                kept.push(point);
                continue;
            };
            if at_least(from.at.distance(point.at), floor(&from, &point, shortest)) {
                kept.push(point);
            } else if i != last && point.origin.role != Role::Lock {
                self.merged += 1;
            } else {
                // A point that stays: the ones before it go instead, back to one that stays.
                while kept.len() > 1
                    && kept.last().is_some_and(|k| k.origin.role != Role::Lock && !at_least(k.at.distance(point.at), floor(k, &point, shortest)))
                {
                    kept.pop();
                    self.merged += 1;
                }
                kept.push(point);
            }
        }
        let mut from: Option<Stitch> = None;
        for point in kept {
            if let Some(start) = from {
                self.split_into(out, start, point, longest, meter)?;
            }
            out.push(point);
            from = Some(point);
        }
        Ok(())
    }

    /// Adds the needle points that split the stitch from `start` to `end` into the fewest equal parts no
    /// longer than `longest`, with `end`'s provenance.
    fn split_into(&mut self, out: &mut Vec<Stitch>, start: Stitch, end: Stitch, longest: f64, meter: &mut Meter) -> Result<(), Exhausted> {
        let length = start.at.distance(end.at);
        let mut parts = 1_u32;
        while !at_least(longest, length / f64::from(parts)) {
            meter.charge(1)?;
            parts = parts.checked_add(1).ok_or(Exhausted::Work)?;
        }
        if parts > 1 {
            self.split += 1;
        }
        for part in 1..parts {
            out.push(Stitch { at: start.at.lerp(end.at, f64::from(part) / f64::from(parts)), ..end });
        }
        Ok(())
    }

    /// `SC-I0504` and `SC-I0703`, for what was changed.
    fn diagnostics(&self, shortest: f64, longest: f64) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if self.merged > 0 {
            let message = match self.merged {
                1 => format!("A needle point less than the shortest stitch ({} mm) from the one before was left out.", mm(shortest)),
                n => format!("{n} needle points less than the shortest stitch ({} mm) from the one before were left out.", mm(shortest)),
            };
            diagnostics.push(Diagnostic::new(Code::StitchesMerged, message));
        }
        if self.split > 0 {
            let message = match self.split {
                1 => format!("A stitch longer than the machine's longest stitch ({} mm) was split into equal parts.", mm(longest)),
                n => format!("{n} stitches longer than the machine's longest stitch ({} mm) were split into equal parts.", mm(longest)),
            };
            diagnostics.push(Diagnostic::new(Code::StitchesSplit, message));
        }
        diagnostics
    }
}

/// `SC-E0701` for a design that would fit the hoop but reaches past its edge, because its origin, which
/// goes to the hoop's centre, is far from its middle.
fn off_centre(bounds: Rect, profile: &MachineProfile) -> Option<Diagnostic> {
    let (half_width, half_height) = (profile.hoop.width.get() / 2.0, profile.hoop.height.get() / 2.0);
    let sideways = bounds.min().x().abs().max(bounds.max().x().abs());
    let upwards = bounds.min().y().abs().max(bounds.max().y().abs());
    if at_least(half_width, sideways) && at_least(half_height, upwards) {
        return None;
    }
    let message = format!(
        "From its origin, which goes to the hoop's centre, the design reaches {sideways:.1} mm sideways and {upwards:.1} mm up or down; the hoop of {} reaches {} mm and {} mm.",
        profile.name,
        mm(half_width),
        mm(half_height)
    );
    Some(Diagnostic::new(Code::OutsideHoop, message).with_fix(Fix::Hint("Move the design's origin nearer its middle.".to_string())))
}

/// The shortest a stitch from `from` to `to` may be: a lock stitch's, into or out of a lock point, or else
/// `shortest`.
fn floor(from: &Stitch, to: &Stitch, shortest: f64) -> f64 {
    if from.origin.role == Role::Lock || to.origin.role == Role::Lock { LOCK_MIN_STITCH.get() } else { shortest }
}
