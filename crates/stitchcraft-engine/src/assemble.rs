//! Pipeline stage 4 (plan assembly): the elements' stitch groups joined, in the order the elements sew, into
//! one stitch plan, with the lock stitches, jumps, trims, stops and thread changes between them.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Plan assembly. The needle sews straight on from one
//! group to the next only when nothing separates them: the same thread, a move no longer than the
//! collapse length (or the earlier element's `min_jump_stitch_length_mm`), no forced locks, no trim or
//! stop. Anything else ends the group with its tie-off and starts the next with a jump and its tie-in, as
//! `ties` allows ([`crate::locks`] sews them). That is where locks go, and the only place (`REQ-LCK-001`).
//!
//! **Several threads in one element** (M10.5). Each group is sewn in its own thread, which its generator
//! gives it ([`crate::generators::Colour`]), or else in its element's. The thread changes inside an element
//! as it changes between 2 elements, by colour, with the tie-off, the new colour block, the jump and the
//! tie-in (`REQ-ASM-006`). With the element's `trim_after` on, each colour block the generator ends is
//! trimmed after, as Ink/Stitch's plan trims after each colour of a tartan or a gradient, and the stop
//! comes only after the element's last group (`REQ-ASM-007`). The plan's first colour block takes the
//! thread of the first group sewn.
//!
//! **Underlays first** (M10.15). An element of a run is given as 2 layers, its underlay and its top, each in
//! its place in the run's passes ([`Layer`]), and each joined to what is before it as any 2 groups are. The
//! element is one element of the plan, whose stitches are in both places. Its underlay's stitches are tagged
//! underlay, and its trim and its stop come after its top stitches alone (`REQ-ASM-009`).
//!
//! The plan is built in the design's coordinates. Finalize moves it so the design's origin, or the centre
//! of its stitches, is at the hoop's centre (`REQ-ASM-005`), once it has left out the points the machine
//! cannot sew. What the machine cannot sew — a stitch too long or too short for it, a design too large for
//! the hoop — is finalize's to fix or report (M3.9).

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::units::at_least;
use stitchcraft_core::{Diagnostic, ElementId, Exhausted, Meter, Mm, Point};
use stitchcraft_params::StitchType;
use stitchcraft_plan::{ElementRef, PlanBuilder, Provenance, Role, StitchPlan, Thread};

use crate::design::{DesignSettings, Element};
use crate::generate::Generated;
use crate::generators::Layer;
use crate::locks::{Lock, tie_in, tie_off};

/// A design's stitch plan, before finalize, and what assembly says about it.
#[derive(Clone, Debug, PartialEq)]
pub struct Assembled {
    /// The plan, in the design's coordinates: finalize moves it to the hoop's centre.
    pub plan: StitchPlan,
    /// What the locks changed or could not sew as set, each naming its element.
    pub warnings: Vec<Diagnostic>,
    /// Each element's shortest stitch, by its place in the plan's element table: the one its generator
    /// used, for finalize to hold its stitches to.
    pub shortest: Vec<Mm>,
}

/// The plan for the generated `elements`, in the order they sew, in a design with `settings`; `None` when no
/// element has a stitch. An element of a run of underlays first is there twice, its underlay and its top,
/// each where it sews. Every needle point costs a unit of work and a stitch from `meter`.
pub fn assemble(elements: &[(&Element, Generated)], settings: &DesignSettings, meter: &mut Meter) -> Result<Option<Assembled>, Exhausted> {
    let parts: Vec<(&Element, &Generated, Vec<Threaded>)> = elements
        .iter()
        .map(|(element, generated)| (*element, generated, threaded(element, generated)))
        .filter(|(_, _, groups)| !groups.is_empty())
        .collect();
    let Some(first) = parts.first().and_then(|(_, _, groups)| groups.first()) else { return Ok(None) };
    let mut assembly = Assembly {
        builder: PlanBuilder::new(first.thread.clone()),
        thread: first.thread.clone(),
        open: None,
        settings,
        warnings: Vec::new(),
        settings_said: BTreeSet::new(),
    };
    let mut shortest = Vec::with_capacity(parts.len());
    // Each element's reference, which both layers of an element of a run share.
    let mut references: BTreeMap<&ElementId, ElementRef> = BTreeMap::new();
    for (element, generated, groups) in &parts {
        let reference = match references.get(&element.id) {
            Some(&reference) => reference,
            None => {
                // Each element registered has a stitch, so the stitch budget runs out long before element
                // references do. `ok_or` takes a value: a closure here would never run, and coverage counts a
                // closure that never runs as an untested line.
                let reference = assembly.builder.element(element.id.clone()).ok().ok_or(Exhausted::Stitches)?;
                shortest.push(generated.min_stitch);
                references.insert(&element.id, reference);
                reference
            }
        };
        let role = if generated.layer == Layer::Underlay { Role::Underlay } else { Role::Top };
        for (index, group) in groups.iter().enumerate() {
            assembly.join(Group { element, generated, stitches: group.stitches, thread: group.thread, reference, role }, meter)?;
            // The trim after the element's last group is `after`'s, with its stop.
            if group.ends_block && generated.common.trim_after && index + 1 < groups.len() {
                assembly.trim(reference, meter)?;
            }
        }
        // An underlay's element sews its top later: its trim and its stop come after that.
        if generated.layer != Layer::Underlay {
            assembly.after(generated, reference, meter)?;
        }
    }
    assembly.tie_off(meter)?;
    let Assembly { builder, warnings, .. } = assembly;
    Ok(Some(Assembled { plan: builder.finish(), warnings, shortest }))
}

/// One of an element's groups that sews a stitch, with the thread it is sewn in and whether it ends a
/// colour block.
struct Threaded<'a> {
    stitches: &'a [Point],
    thread: &'a Thread,
    ends_block: bool,
}

/// The groups of `generated`, `element`'s, that sew a stitch, in their order: each in the thread its
/// generator gives it, or else in the element's, and ending a colour block when its generator says so
/// (`REQ-ASM-006`).
fn threaded<'a>(element: &'a Element, generated: &'a Generated) -> Vec<Threaded<'a>> {
    let colour = |index: usize| generated.colours.get(index);
    let groups = generated.groups.iter().enumerate().filter(|(_, stitches)| !stitches.is_empty());
    groups
        .map(|(index, stitches)| Threaded {
            stitches,
            thread: colour(index).and_then(|colour| colour.thread.as_ref()).unwrap_or(&element.thread),
            ends_block: colour(index).is_some_and(|colour| colour.ends_block),
        })
        .collect()
}

/// One group, with its element, the thread it is sewn in and the role of its stitches.
#[derive(Clone, Copy)]
struct Group<'a> {
    element: &'a Element,
    generated: &'a Generated,
    stitches: &'a [Point],
    thread: &'a Thread,
    reference: ElementRef,
    role: Role,
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
    /// locks, and no farther than the element's jump length ([`jump_length`](crate::common::CommonParams::jump_length)).
    fn sews_on(&self, distance: f64, settings: &DesignSettings) -> bool {
        let common = &self.generated.common;
        let limit = common.jump_length(settings.collapse_len);
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
    /// The warnings about locks' settings kept so far, by element.
    settings_said: BTreeSet<(String, String)>,
}

impl<'a> Assembly<'a> {
    /// Sews `group`: straight on from the open group if nothing separates them, else after a jump.
    fn join(&mut self, group: Group<'a>, meter: &mut Meter) -> Result<(), Exhausted> {
        // Threads are compared by colour, as Ink/Stitch compares them: a name alone changes nothing.
        if group.thread.color != self.thread.color {
            self.tie_off(meter)?;
            self.thread = group.thread.clone();
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
        self.stitch(group.stitches, Provenance::new(group.reference, group.role), meter)?;
        self.open = Some(group);
        Ok(())
    }

    /// The trim after a colour block of an element sewn in several threads, but its last: the open group's
    /// tie-off, then the trim (`REQ-ASM-007`).
    fn trim(&mut self, reference: ElementRef, meter: &mut Meter) -> Result<(), Exhausted> {
        self.tie_off(meter)?;
        self.builder.trim(Some(reference));
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

    /// The lock's needle points, keeping its warnings, named after `group`'s element. A warning about the
    /// lock's settings, which has no place, is the same at every group of the element: it is kept once.
    fn lock(&mut self, lock: Lock, group: Group<'_>) -> Vec<Point> {
        for warning in lock.warnings {
            if warning.at.is_some() || self.settings_said.insert((group.element.id.as_str().to_string(), warning.to_string())) {
                self.warnings.push(warning.with_element(group.element.id.clone()));
            }
        }
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
