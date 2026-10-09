//! Pipeline stage 4 (plan assembly): the elements' stitch groups joined, in document order, into one stitch
//! plan, with the lock stitches, jumps, trims, stops and thread changes between them.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Plan assembly. The needle sews straight on from one
//! group to the next only when nothing separates them: the same thread, a move no longer than the
//! collapse length (or the earlier element's `min_jump_stitch_length_mm`), no forced locks, no trim or
//! stop. Anything else ends the group with its tie-off and starts the next with a jump and its tie-in, as
//! `ties` allows ([`crate::locks`] sews them). That is where locks go, and the only place (`REQ-LCK-001`).
//!
//! The plan is built in the design's coordinates, then moved so the design's origin, or the centre of
//! its stitches, is at the hoop's centre (`REQ-ASM-005`). What the machine cannot sew — a stitch too long
//! or too short for it, a design too large for the hoop — is finalize's to fix or report (M3.9).

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Diagnostic, Exhausted, Meter, Mm, Point, Rect};
use stitchcraft_params::StitchType;
use stitchcraft_plan::{ElementRef, PlanBuilder, Provenance, Role, StitchKind, StitchPlan, Thread};

use crate::design::{DesignSettings, Element};
use crate::generate::Generated;
use crate::locks::{Lock, tie_in, tie_off};

/// A design's stitch plan, before finalize, and what assembly says about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembled {
    /// The plan, in hoop coordinates.
    pub plan: StitchPlan,
    /// What the locks changed or could not sew as set, each naming its element.
    pub warnings: Vec<Diagnostic>,
    /// Each element's shortest stitch, by its place in the plan's element table: the one its generator
    /// used, for finalize to hold its stitches to.
    pub shortest: Vec<Mm>,
}

/// The plan for the generated `elements`, in document order, in a design with `settings`; `None` when no
/// element has a stitch. Every needle point costs a unit of work and a stitch from `meter`.
pub fn assemble(elements: &[(&Element, Generated)], settings: &DesignSettings, meter: &mut Meter) -> Result<Option<Assembled>, Exhausted> {
    let parts: Vec<&(&Element, Generated)> = elements.iter().filter(|(_, generated)| generated.groups.iter().any(|g| !g.is_empty())).collect();
    let Some((first, _)) = parts.first() else { return Ok(None) };
    let mut assembly =
        Assembly { builder: PlanBuilder::new(first.thread.clone()), thread: first.thread.clone(), open: None, settings, warnings: Vec::new() };
    let mut shortest = Vec::with_capacity(parts.len());
    for (element, generated) in parts {
        // Each element registered has a stitch, so the stitch budget runs out long before element
        // references do.
        let reference = assembly.builder.element(element.id.clone()).map_err(|_| Exhausted::Stitches)?;
        shortest.push(generated.min_stitch);
        let groups: Vec<&[Point]> = generated.groups.iter().filter(|g| !g.is_empty()).map(Vec::as_slice).collect();
        for stitches in &groups {
            assembly.join(Group { element, generated, stitches, reference }, meter)?;
        }
        assembly.after(generated, reference, meter)?;
    }
    assembly.tie_off(meter)?;
    let Assembly { builder, warnings, .. } = assembly;
    Ok(Some(Assembled { plan: centred(builder.finish(), settings.origin), warnings, shortest }))
}

/// One group, with its element.
#[derive(Clone, Copy)]
struct Group<'a> {
    element: &'a Element,
    generated: &'a Generated,
    stitches: &'a [Point],
    reference: ElementRef,
}

impl Group<'_> {
    /// Whether the group gets a tie-in where the needle jumps to it, and a tie-off where a jump, trim,
    /// stop, thread change or the end follows it. Manual stitch gets neither unless locks are forced.
    fn locks(&self) -> (bool, bool) {
        let common = &self.generated.common;
        let by_hand = self.generated.stitch_type == StitchType::ManualStitch && !common.force_lock_stitches;
        let tie_in = matches!(common.ties, "0" | "1") && !by_hand;
        let tie_off = (matches!(common.ties, "0" | "2") || common.force_lock_stitches) && !by_hand;
        (tie_in, tie_off)
    }

    /// Whether the needle may sew straight on from this group to a point `distance` away: no forced
    /// locks, and no farther than the element's `min_jump_stitch_length_mm` or else the design's collapse
    /// length. A setting of 0 or less is read as not set, as in Ink/Stitch (`stitchcraft_params` reads
    /// an optional length that way).
    fn sews_on(&self, distance: f64, settings: &DesignSettings) -> bool {
        let common = &self.generated.common;
        let limit = common.min_jump_stitch_length_mm.unwrap_or(settings.collapse_len);
        !common.force_lock_stitches && at_least(limit.get(), distance)
    }
}

/// The plan as it is built.
struct Assembly<'a> {
    builder: PlanBuilder,
    /// The thread of the block being sewn.
    thread: Thread,
    /// The group the needle is still sewing from, until a tie-off ends it.
    open: Option<Group<'a>>,
    settings: &'a DesignSettings,
    warnings: Vec<Diagnostic>,
}

impl<'a> Assembly<'a> {
    /// Sews `group`: straight on from the open group if nothing separates them, else after a jump.
    fn join(&mut self, group: Group<'a>, meter: &mut Meter) -> Result<(), Exhausted> {
        // Threads are compared by colour, as Ink/Stitch compares them: a name alone changes nothing.
        if group.element.thread.color != self.thread.color {
            self.tie_off(meter)?;
            self.thread = group.element.thread.clone();
            self.builder.change_thread(self.thread.clone());
        } else if let (Some(open), Some(&next)) = (self.open, group.stitches.first())
            && !open.sews_on(self.builder.needle().distance(next), self.settings)
        {
            self.tie_off(meter)?;
        }
        let tie_in = if self.open.is_none() && group.locks().0 {
            self.lock(tie_in(group.stitches, &group.generated.common, meter)?, group)
        } else {
            Vec::new()
        };
        if self.open.is_none()
            && let Some(&landing) = tie_in.first().or(group.stitches.first())
        {
            self.builder.jump(landing, Provenance::new(group.reference, Role::Travel));
        }
        self.stitch(&tie_in, Provenance::new(group.reference, Role::Lock), meter)?;
        self.stitch(group.stitches, Provenance::new(group.reference, Role::Top), meter)?;
        self.open = Some(group);
        Ok(())
    }

    /// After an element's last group: its trim and its stop, each after its tie-off, the stop after a jump
    /// to the design's stop position if it has one.
    fn after(&mut self, generated: &Generated, reference: ElementRef, meter: &mut Meter) -> Result<(), Exhausted> {
        let common = &generated.common;
        if common.trim_after || common.stop_after {
            self.tie_off(meter)?;
        }
        if common.trim_after {
            self.builder.trim(Some(reference));
        }
        if common.stop_after {
            if let Some(at) = self.settings.stop_position {
                self.builder.jump(at, Provenance::new(reference, Role::Travel));
            }
            self.builder.stop(Some(reference));
        }
        Ok(())
    }

    /// Ends the open group, with its tie-off if it gets one.
    fn tie_off(&mut self, meter: &mut Meter) -> Result<(), Exhausted> {
        let Some(open) = self.open.take() else { return Ok(()) };
        if open.locks().1 {
            let points = self.lock(tie_off(open.stitches, &open.generated.common, meter)?, open);
            self.stitch(&points, Provenance::new(open.reference, Role::Lock), meter)?;
        }
        Ok(())
    }

    /// The lock's needle points, keeping its warnings, named after `group`'s element.
    fn lock(&mut self, lock: Lock, group: Group<'_>) -> Vec<Point> {
        self.warnings.extend(lock.warnings.into_iter().map(|w| w.with_element(group.element.id.clone())));
        lock.points
    }

    /// Sews each of `points`.
    fn stitch(&mut self, points: &[Point], origin: Provenance, meter: &mut Meter) -> Result<(), Exhausted> {
        for &at in points {
            meter.charge(1)?;
            meter.charge_stitches(1)?;
            self.builder.stitch(at, origin);
        }
        Ok(())
    }
}

/// `plan` moved so that `origin`, or else the centre of the box around its stitches, is at the hoop's
/// centre. Positions within the data model's reach stay finite, moved; were one not to, it would stay.
fn centred(mut plan: StitchPlan, origin: Option<Point>) -> StitchPlan {
    let sewn = plan.stitches().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at);
    let Some(origin) = origin.or_else(|| Rect::around(sewn).map(Rect::center)) else { return plan };
    for stitch in plan.blocks.iter_mut().flat_map(|b| b.stitches.iter_mut()) {
        stitch.at = Point::new(stitch.at.x() - origin.x(), stitch.at.y() - origin.y()).unwrap_or(stitch.at);
    }
    plan
}
