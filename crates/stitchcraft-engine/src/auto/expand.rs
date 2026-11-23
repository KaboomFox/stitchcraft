//! Expand (roadmap step M13.26, `REQ-AUTO-142` to `REQ-AUTO-144`; `docs/src/design/image-to-embroidery.md`
//! › Expand): an automatic design made the ordinary elements digitizing makes of it, which a person edits
//! one by one, and which sew as the automatic design sewed.
//!
//! **Why the elements digitizing makes.** Planning turns every automatic element into ordinary elements
//! before it generates anything ([`super::sewing`]). Those are the elements Expand gives, each with the
//! settings that say what it sews, so no second digitizer or translation of the rules is needed.
//!
//! **What no ordinary setting says.** After digitizing, planning goes by which elements digitizing made: the
//! needle runs under later shapes between 2 of them (`REQ-AUTO-053`), every stitch of the base layer is
//! underlay (`REQ-AUTO-028`), and the cap at the end of planning thins them by the limits of the lowest
//! automatic shape (`REQ-AUTO-074`). Lettering that digitizing placed keeps its underlay and its pull
//! there (`REQ-AUTO-120`, `REQ-AUTO-121`). One setting common to every stitch effect says it of any element,
//! [`DIGITIZED_AS`], and the design keeps the limits ([`DesignSettings::cap`]). Planning reads both
//! ([`marked`], [`limits`]), so an expanded design plans to the automatic plan, and a person can set them
//! by hand or clear them on a part.
//!
//! **What is lost.** A lettering's text becomes the columns it sews, and a shape whose digitizing ran out of
//! work sews nothing in either design. One warning names them (`SC-W0922`). Expand drops every setting of the
//! Auto effect, which no stitch type reads.

use std::borrow::Cow;
use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::MachineProfile;

use super::thick::{self, Held};
use super::{AutoParams, Sewing, sewing};
use crate::common::{BASE, DIGITIZED_AS, LETTERING, PART};
use crate::design::{Design, DesignSettings, Element, Lettered};
use crate::names::{Names, base_name, thread_name};

/// How many shapes the warning names of each kind at the most, the rest counted.
const NAMED: usize = 5;

/// How an element set with [`DIGITIZED_AS`] sews.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Digitized {
    /// As a part digitizing made.
    Part,
    /// As a piece of the base layer: a part whose stitches are all underlay.
    Base,
    /// As lettering digitizing placed.
    Lettering,
}

/// What `element`'s [`DIGITIZED_AS`] says: `None` for `no`, for none, and for a value it doesn't take, which
/// reading its settings names (`SC-E0101`).
fn digitized(element: &Element) -> Option<Digitized> {
    match element.params.get(DIGITIZED_AS)? {
        PART => Some(Digitized::Part),
        BASE => Some(Digitized::Base),
        LETTERING => Some(Digitized::Lettering),
        _ => None,
    }
}

/// `elements` as planning sews them (`REQ-AUTO-143`): each set to a part or to a piece of the base layer
/// added to `made`, the elements digitizing made, and each set to a piece of the base layer to `base` too;
/// each set to lettering marked as lettering digitizing placed. Elements set as nothing come back as they
/// are, unchanged.
pub(super) fn marked<'d>(elements: Cow<'d, [Element]>, base: &mut Vec<ElementId>, made: &mut BTreeSet<ElementId>) -> Cow<'d, [Element]> {
    let mut lettering = Vec::new();
    for (at, element) in elements.iter().enumerate() {
        match digitized(element) {
            Some(Digitized::Part) => {
                made.insert(element.id.clone());
            }
            Some(Digitized::Base) => {
                made.insert(element.id.clone());
                if !base.contains(&element.id) {
                    base.push(element.id.clone());
                }
            }
            Some(Digitized::Lettering) if element.lettered.is_none() => lettering.push(at),
            Some(Digitized::Lettering) | None => {}
        }
    }
    if lettering.is_empty() {
        return elements;
    }
    let mut elements = elements.into_owned();
    for at in lettering {
        if let Some(element) = elements.get_mut(at) {
            element.lettered = Some(Lettered::Digitized);
        }
    }
    Cow::Owned(elements)
}

/// What the cap holds a design to: `rules`, its lowest automatic shape's, when it has one; else the limits
/// its `settings` keep, when any element is set as digitizing made it (`made`), or else the Auto effect's
/// defaults; and nothing for a design without either, which is not measured.
pub(super) fn limits(rules: Option<Held>, made: &BTreeSet<ElementId>, settings: &DesignSettings) -> Option<Held> {
    rules.or(settings.cap).or_else(|| if made.is_empty() { None } else { thick::by_default() })
}

/// `design` expanded (`REQ-AUTO-142`): the elements automatic digitizing makes of it, in the order they sew,
/// none of them automatic, without the Auto effect's settings, and each set as digitizing made it
/// ([`DIGITIZED_AS`]). A part without a name of its own takes its shape's, as the notices name it, with the
/// thread names of `profile`'s palette, and a piece of the base layer its thread's, `Base layer 1, Red`. The design keeps what the cap held the automatic design to
/// ([`DesignSettings::cap`]), and none of its marks or guides, which steer automatic shapes only. What was
/// said of digitizing comes back with it, and one warning names what Expand couldn't keep (`SC-W0922`).
/// Digitizing has `budget`'s work, as planning gives it, and so does naming, which names nothing when it
/// needs more, as said. The error is `Design::new`'s, which the elements of a design's digitizing never
/// give.
pub fn expand(design: &Design, profile: &MachineProfile, budget: &Budget) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
    let Sewing { elements, mut diagnostics, base, made, limits, .. } = sewing(design, budget, false);
    let order: Vec<&ElementId> = elements.iter().map(|element| &element.id).collect();
    let names = Names::given(design, (&elements, &base, &order), profile.palette.palette(), &mut budget.meter()).unwrap_or_else(|exhausted| {
        diagnostics.push(exhausted.diagnostic(budget, None));
        Names::default()
    });
    let base: BTreeSet<&ElementId> = base.iter().collect();
    let called = |id: &ElementId| names.name_of(id.as_str()).map_or_else(|| format!("`{id}`"), |name| format!("\"{name}\""));
    let mut lettering: Vec<String> = Vec::new();
    let mut parts = Vec::with_capacity(elements.len());
    let mut pieces: BTreeMap<String, usize> = BTreeMap::new();
    for element in elements.iter() {
        let digitized = if base.contains(&element.id) {
            Some(BASE)
        } else if element.lettered == Some(Lettered::Digitized) {
            Some(LETTERING)
        } else if made.contains(&element.id) {
            Some(PART)
        } else {
            None
        };
        if element.lettered.is_some() {
            let shape = called(&element.id);
            if !lettering.contains(&shape) {
                lettering.push(shape);
            }
        }
        let mut params: ParamSet = element.params.iter().filter(|(key, _)| !AutoParams::SPECS.iter().any(|spec| spec.key == *key)).collect();
        if let Some(digitized) = digitized {
            params.set(DIGITIZED_AS, digitized);
        }
        // A piece of the base layer is of no shape: it is named after its thread, numbered as a shape is.
        let piece = || {
            let thread = thread_name(&element.thread, profile.palette.palette());
            let number = pieces.entry(thread.clone()).or_insert(0);
            *number += 1;
            base_name(&thread, *number)
        };
        let own = element.name.clone().or_else(|| names.name_of(element.id.as_str()).map(str::to_string));
        let name = own.or_else(|| base.contains(&element.id).then(piece));
        parts.push(Element { name, params, lettered: None, ..element.clone() });
    }
    let mut left_out: Vec<String> = Vec::new();
    for said in diagnostics.iter().filter(|said| said.code == Code::BudgetExhausted) {
        let shape = said.element.as_ref().map(called);
        if let Some(shape) = shape.filter(|shape| !left_out.contains(shape)) {
            left_out.push(shape);
        }
    }
    diagnostics.extend(lost(&lettering, &left_out));
    let settings = DesignSettings { cap: limits, ..design.settings };
    Design::new(parts, settings).map(|expanded| (expanded, diagnostics))
}

/// The warning that names what Expand couldn't keep (`SC-W0922`): the shapes of `lettering`, which became
/// their columns, and those `left_out` for their budget; none when there are neither.
fn lost(lettering: &[String], left_out: &[String]) -> Option<Diagnostic> {
    let mut said = Vec::new();
    if !lettering.is_empty() {
        said.push(format!(
            "The lettering of {} became the columns it sews: its text can't be edited as text, and lettering set in a font no longer sews its \
             underlay as lettering does",
            listed(lettering)
        ));
    }
    if !left_out.is_empty() {
        said.push(format!("{} needed more work than the budget allows, and sew nothing in the expanded design", listed(left_out)));
    }
    if said.is_empty() {
        return None;
    }
    let message = format!("Expand couldn't keep everything. {}.", said.join(". "));
    let hint = "Edit the text in the automatic design and expand it again: the traced design is kept, hidden, under the expanded parts.";
    Some(Diagnostic::new(Code::AutoExpandLost, message).with_fix(Fix::Hint(hint.to_string())))
}

/// `shapes`, at most [`NAMED`] of them named and the rest counted.
fn listed(shapes: &[String]) -> String {
    let named: Vec<&str> = shapes.iter().take(NAMED).map(String::as_str).collect();
    let more = shapes.len().saturating_sub(NAMED);
    match (named.split_last(), more) {
        (Some((last, [])), 0) => (*last).to_string(),
        (Some((last, rest)), 0) => format!("{} and {last}", rest.join(", ")),
        (_, more) => format!("{} and {more} more", named.join(", ")),
    }
}

#[cfg(test)]
mod tests;
