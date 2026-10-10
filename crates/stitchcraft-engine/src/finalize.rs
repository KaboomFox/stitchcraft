//! Pipeline stages 5 and 6 (finalize and check): the assembled plan fitted to the machine, then checked.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Finalize and › Check. Generators already keep their own
//! stitches within the machine's limits; what is left for here comes from joining things up and from
//! settings the machine cannot follow:
//!
//! 1. **The shortest stitch.** Where one element's stitching runs straight on into the next, the stitch
//!    between them can be anything up to the collapse length, 0 included. A needle point less than its
//!    element's shortest stitch from the one before is left out, so the stitch runs on to the next. That
//!    is the shortest stitch the element's generator used (its own, else the design's, never below the
//!    machine's), so finalize never thins what the generator spaced. The first and last points of a run
//!    of stitches (where the needle lands, and where a jump, trim or stop follows) and lock points always
//!    stay, and leave out the ones before them instead; a point where the needle already is adds nothing
//!    and is left out too. A stitch into or out of a lock point is a lock stitch, whose shortest is
//!    0.2 mm. `SC-I0504` says how many were left out.
//! 2. **The longest stitch.** A stitch longer than the machine's is split into equal parts: a hand-placed
//!    stitch, say, or a custom lock's long step (`SC-I0703`). Each part counts against the stitch budget.
//! 3. **The machine.** Too many colour changes and stops for the machine's format is `SC-E0601`. A design
//!    larger than the hoop, reaching past its edge from an origin far from the design's middle, or with a
//!    stop position past the edge is `SC-E0701`, whatever else is true of it. Otherwise a design larger
//!    than the comfort zone is `SC-W0702`.
//! 4. **The check.** The plan invariants (`stitchcraft_plan::invariants`) must hold; a broken one is a bug
//!    in StitchCraft (`SC-E0009`), and nothing is written.

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point, Rect};
use stitchcraft_plan::invariants::{self, LOCK_MIN_STITCH};
use stitchcraft_plan::{MachineProfile, Role, Stitch, StitchKind, StitchPlan};

use crate::design::DesignSettings;
use crate::generate::shortest_stitch;
use crate::generators::mm;

/// A plan fitted to the machine and checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Finalized {
    /// The plan; `None` when the machine cannot sew it or a check failed, as the diagnostics say.
    pub plan: Option<StitchPlan>,
    /// What finalizing changed, and why the plan is missing when it is.
    pub diagnostics: Vec<Diagnostic>,
}

/// `plan` fitted to the machine `profile` describes, for a design with `settings`, then checked.
/// `shortest` is each element's shortest stitch, by its place in the plan's element table, as its
/// generator used it. Every entry costs a unit of work from `meter`, and every stitch added one of its
/// stitches.
pub fn finalize(
    mut plan: StitchPlan,
    profile: &MachineProfile,
    settings: &DesignSettings,
    shortest: &[Mm],
    meter: &mut Meter,
) -> Result<Finalized, Exhausted> {
    let mut fitted = Fitted::new(shortest, shortest_stitch(None, settings, profile), profile.max_stitch.get());
    for block in &mut plan.blocks {
        block.stitches = fitted.block(&block.stitches, meter)?;
    }
    let mut diagnostics = fitted.diagnostics();
    let changes = plan.stats().changes_including_stops();
    let max = profile.format.max_color_changes();
    if changes > max {
        let message = format!("The design has {changes} colour changes and stops, but {} records at most {max}.", profile.format.name());
        diagnostics.push(Diagnostic::new(Code::TooManyColorChanges, message));
        return Ok(Finalized { plan: None, diagnostics });
    }
    let sewn = Rect::around(plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at));
    let size = sewn.and_then(|bounds| profile.check_fit(bounds));
    let outside = match &size {
        Some(fit) if fit.code == Code::OutsideHoop => size.clone(),
        _ => match sewn.and_then(|bounds| off_centre(bounds, profile)) {
            Some(reach) => Some(reach),
            None => stop_outside(&plan, profile, meter)?,
        },
    };
    if let Some(outside) = outside {
        diagnostics.push(outside);
        return Ok(Finalized { plan: None, diagnostics });
    }
    diagnostics.extend(size);
    let violations = invariants::check(&plan, profile);
    if violations.is_empty() {
        return Ok(Finalized { plan: Some(plan), diagnostics });
    }
    diagnostics.extend(violations.iter().map(invariants::Violation::diagnostic));
    Ok(Finalized { plan: None, diagnostics })
}

/// The plan as it is fitted, and what fitting it changed.
struct Fitted<'s> {
    /// Each element's shortest stitch, by its place in the plan's element table.
    shortest: &'s [Mm],
    /// The shortest stitch of an entry that names no element.
    unnamed: Mm,
    /// The machine's longest stitch.
    longest: f64,
    /// How many needle points were left out, and the shortest stitches they fell short of: the least
    /// and the most.
    merged: usize,
    short_of: Option<(f64, f64)>,
    /// How many stitches were split.
    split: usize,
}

impl<'s> Fitted<'s> {
    fn new(shortest: &'s [Mm], unnamed: Mm, longest: f64) -> Self {
        Fitted { shortest, unnamed, longest, merged: 0, short_of: None, split: 0 }
    }

    /// A block's entries with each run of stitches fitted: the stitches between two other entries (a
    /// jump, a trim, a stop) or the block's ends.
    fn block(&mut self, entries: &[Stitch], meter: &mut Meter) -> Result<Vec<Stitch>, Exhausted> {
        let mut out = Vec::with_capacity(entries.len());
        let mut run: Vec<Stitch> = Vec::new();
        for &entry in entries {
            meter.charge(1)?;
            if entry.kind == StitchKind::Normal {
                run.push(entry);
            } else {
                self.flush(&mut run, &mut out, meter)?;
                out.push(entry);
            }
        }
        self.flush(&mut run, &mut out, meter)?;
        Ok(out)
    }

    /// Fits the run of stitches gathered so far and moves it to `out`. Its first point is where the needle
    /// lands and its last where the next entry happens, so both stay, as lock points do.
    fn flush(&mut self, run: &mut Vec<Stitch>, out: &mut Vec<Stitch>, meter: &mut Meter) -> Result<(), Exhausted> {
        let last = run.len().saturating_sub(1);
        let mut kept: Vec<Stitch> = Vec::with_capacity(run.len());
        for (i, point) in run.drain(..).enumerate() {
            let Some(&from) = kept.last() else {
                kept.push(point);
                continue;
            };
            if at_least(from.at.distance(point.at), self.floor(&from, &point)) {
                kept.push(point);
            } else if i != last && point.origin.role != Role::Lock {
                self.left_out(&point);
            } else {
                // A point that stays: the ones before it go instead, back to one that stays.
                while kept.len() > 1
                    && kept.last().is_some_and(|k| k.origin.role != Role::Lock && !at_least(k.at.distance(point.at), self.floor(k, &point)))
                {
                    kept.pop();
                    self.left_out(&point);
                }
                // Where the needle already is, it adds nothing: a stitch in place.
                if kept.last().is_some_and(|k| k.at == point.at) {
                    self.left_out(&point);
                } else {
                    kept.push(point);
                }
            }
        }
        let mut from: Option<Stitch> = None;
        for point in kept {
            if let Some(start) = from {
                self.split_into(out, start, point, meter)?;
            }
            out.push(point);
            from = Some(point);
        }
        Ok(())
    }

    /// The shortest stitch of `point`'s element, or of the design when it names none.
    fn shortest_of(&self, point: &Stitch) -> f64 {
        point.origin.element.and_then(|element| self.shortest.get(element.index())).unwrap_or(&self.unnamed).get()
    }

    /// The shortest a stitch from `from` to `to` may be: a lock stitch's, into or out of a lock point, or
    /// else the shortest stitch of `to`'s element, whose stitch it is.
    fn floor(&self, from: &Stitch, to: &Stitch) -> f64 {
        if from.origin.role == Role::Lock || to.origin.role == Role::Lock { LOCK_MIN_STITCH.get() } else { self.shortest_of(to) }
    }

    /// Counts a needle point left out because a stitch to it, or from the one before it to `next`, would
    /// be shorter than `next`'s element's shortest stitch.
    fn left_out(&mut self, next: &Stitch) {
        let shortest = self.shortest_of(next);
        self.merged += 1;
        self.short_of = Some(self.short_of.map_or((shortest, shortest), |(least, most)| (least.min(shortest), most.max(shortest))));
    }

    /// Adds the needle points that split the stitch from `start` to `end` into the fewest equal parts no
    /// longer than the machine's longest stitch, with `end`'s provenance. Each one costs a stitch.
    fn split_into(&mut self, out: &mut Vec<Stitch>, start: Stitch, end: Stitch, meter: &mut Meter) -> Result<(), Exhausted> {
        let length = start.at.distance(end.at);
        let mut parts = 1_u32;
        while !at_least(self.longest, length / f64::from(parts)) {
            meter.charge(1)?;
            parts = parts.checked_add(1).ok_or(Exhausted::Work)?;
        }
        if parts > 1 {
            self.split += 1;
        }
        for part in 1..parts {
            meter.charge_stitches(1)?;
            out.push(Stitch { at: start.at.lerp(end.at, f64::from(part) / f64::from(parts)), ..end });
        }
        Ok(())
    }

    /// `SC-I0504` and `SC-I0703`, for what was changed.
    fn diagnostics(&self) -> Vec<Diagnostic> {
        let mut diagnostics = Vec::new();
        if let Some((least, most)) = self.short_of {
            let shortest = if least < most { format!("{} to {}", mm(least), mm(most)) } else { mm(least) };
            let message = match self.merged {
                1 => format!("A needle point less than the shortest stitch ({shortest} mm) from the one before was left out."),
                n => format!("{n} needle points less than the shortest stitch ({shortest} mm) from the one before were left out."),
            };
            diagnostics.push(Diagnostic::new(Code::StitchesMerged, message));
        }
        if self.split > 0 {
            let longest = mm(self.longest);
            let message = match self.split {
                1 => format!("A stitch longer than the machine's longest stitch ({longest} mm) was split into equal parts."),
                n => format!("{n} stitches longer than the machine's longest stitch ({longest} mm) were split into equal parts."),
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
        "From its origin, which goes to the hoop's centre, the design reaches {sideways:.1} mm sideways and {upwards:.1} mm up or down, but the {} reaches {} mm and {} mm.",
        profile.name,
        mm(half_width),
        mm(half_height)
    );
    Some(Diagnostic::new(Code::OutsideHoop, message).with_fix(Fix::Hint("Move the design's origin nearer its middle.".to_string())))
}

/// `SC-E0701` for a stop position past the hoop's edge. Once the stitches fit, every entry lies where
/// the needle went down, except the jump to the stop position and the stop there: the first entry outside
/// the hoop is that. Every entry looked at costs a unit of work from `meter`.
fn stop_outside(plan: &StitchPlan, profile: &MachineProfile, meter: &mut Meter) -> Result<Option<Diagnostic>, Exhausted> {
    let (half_width, half_height) = (profile.hoop.width.get() / 2.0, profile.hoop.height.get() / 2.0);
    let mut outside: Option<Point> = None;
    for stitch in plan.stitches() {
        meter.charge(1)?;
        if !at_least(half_width, stitch.at.x().abs()) || !at_least(half_height, stitch.at.y().abs()) {
            outside = Some(stitch.at);
            break;
        }
    }
    Ok(outside.map(|at| {
        let message = format!(
            "The stop position, where the frame goes before each stop, is {:.1} mm sideways and {:.1} mm up or down from the hoop's centre, but the {} reaches {} mm and {} mm.",
            at.x().abs(),
            at.y().abs(),
            profile.name,
            mm(half_width),
            mm(half_height)
        );
        Diagnostic::new(Code::OutsideHoop, message).with_fix(Fix::Hint("Move the stop position nearer the design.".to_string()))
    }))
}
