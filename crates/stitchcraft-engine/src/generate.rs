//! Pipeline stage 3 (generate): each element to its stitch groups, by its stitch type's generator.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Generate. The element's parameters are read here, once:
//! the ones every stitch type shares, which assembly needs (locks, trims, stops), and its stitch type's,
//! which its generator needs. A satin column also reads the running stitch's length, for the needle's
//! travel between its underlays. The stitch type picks the generator in one place, [`generate`]'s table,
//! so a new stitch type is its own module and one line here.
//!
//! A stroke whose `satin_column` setting is on is a satin column, whatever its `stroke_method` says, as in
//! Ink/Stitch, and its `satin_method` picks the generator.
//!
//! Elements are generated in sewing order, each with its neighbours (`REQ-GEN-003`): where the elements
//! before it left the needle, and what the next one offers to end near, which [`approach`] reads from the
//! next element's shape and settings alone.
//!
//! An element that cannot be sewn is skipped, with a diagnostic that says why, and the rest of the design
//! still plans (`REQ-GEN-002`): a stitch type StitchCraft does not sew yet (`SC-W0011`), parameters it
//! cannot read (`SC-E0101`), a satin column with no rails (`SC-E0201`), or its work budget spent
//! (`SC-E0004`). Each element has the budget's work to itself, so one that runs out costs nothing but
//! itself.

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Meter, Mm, Point, SplitMix64};
use stitchcraft_params::{ChoiceOption, Family, StitchType, Validated, params, unknown_keys};
use stitchcraft_plan::MachineProfile;

use crate::common::CommonParams;
use crate::design::{DesignSettings, Element, Path, Shape};
use crate::generators::manual::manual_stitch;
use crate::generators::passes::RepeatParams;
use crate::generators::running::{RunningParams, running_stitch};
use crate::generators::satin::{self, SatinLengths, SatinParams, satin_stitch};
use crate::generators::{Approach, Neighbours, Stitched, method, mm};
use crate::normalize::satin::Shape as SatinShape;
use crate::registry::PARAMETERS;

/// The stroke methods `stroke_method` offers, in Ink/Stitch's order, which its files count on: Ink/Stitch
/// gives the parameter's default as the first one's place in this list.
pub const STROKE_METHODS: &[ChoiceOption] =
    &[method(StitchType::RunningStitch), method(StitchType::RippleStitch), method(StitchType::ZigzagStitch), method(StitchType::ManualStitch)];

params! {
    /// How a stroke is sewn.
    pub struct StrokeParams for &[StitchType::RunningStitch, StitchType::RippleStitch, StitchType::ZigzagStitch, StitchType::ManualStitch];

    "Stroke" {
        /// The stitch the path is sewn with: a running stitch along it, or a needle point on each of its
        /// nodes (manual stitch). Ripple and zigzag stitches arrive in later versions; until then an
        /// element set to one is skipped (`SC-W0011`).
        stroke_method: Choice = "running_stitch", label "Method", choices STROKE_METHODS;
    }
}

/// An element, generated: the settings assembly needs, and its stitch groups.
#[derive(Clone, Debug, PartialEq)]
pub struct Generated {
    /// The settings every stitch type shares.
    pub common: CommonParams,
    /// Its stitch type.
    pub stitch_type: StitchType,
    /// Its groups: the needle points of each part a jump may separate from the next, in sewing order.
    pub groups: Vec<Vec<Point>>,
    /// The shortest stitch it was sewn with, which finalize holds its stitches to as well.
    pub min_stitch: Mm,
}

/// What generating one element gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Generation {
    /// The element generated; `None` when it is skipped.
    pub generated: Option<Generated>,
    /// What was changed, left out or wrong, each naming the element.
    pub diagnostics: Vec<Diagnostic>,
}

/// `element` generated for a design with `settings`, sewn on the machine `profile` describes, between its
/// `neighbours`, with the budget's work to itself.
pub fn generate(element: &Element, settings: &DesignSettings, profile: &MachineProfile, neighbours: &Neighbours, budget: &Budget) -> Generation {
    let mut diagnostics = unknown_keys(&element.params, PARAMETERS);
    let generated = match sew(element, settings, profile, neighbours, &mut diagnostics, &mut budget.meter()) {
        Ok(generated) => generated,
        Err(exhausted) => {
            diagnostics.push(exhausted.diagnostic(budget, None));
            None
        }
    };
    if !generated.as_ref().is_some_and(|g| g.groups.iter().any(|group| !group.is_empty())) {
        diagnostics.extend(left_out(element));
    }
    Generation { generated, diagnostics: diagnostics.into_iter().map(|d| d.with_element(element.id.clone())).collect() }
}

/// `SC-W0505` for the trim and stop that `element`, which sews nothing, sets after it: assembly has no
/// place for them. Its settings were read once already; a setting that cannot be read was reported then.
fn left_out(element: &Element) -> Option<Diagnostic> {
    let common = CommonParams::from_set(&element.params).ok()?.params;
    let what = match (common.trim_after, common.stop_after) {
        (true, true) => "the trim and the stop after it are",
        (true, false) => "the trim after it is",
        (false, true) => "the stop after it is",
        (false, false) => return None,
    };
    Some(Diagnostic::new(Code::TrimOrStopLeftOut, format!("This element sews no stitch, so {what} left out.")))
}

/// The element's stitch groups, or `None` when it is skipped; what it says about it goes to `diagnostics`.
fn sew(
    element: &Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    neighbours: &Neighbours,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Generated>, Exhausted> {
    let set = &element.params;
    let common = kept(CommonParams::from_set(set), diagnostics);
    let (path, width) = match &element.shape {
        Shape::Stroke { path, width, .. } => (path, *width),
        Shape::Fill { .. } => {
            diagnostics.push(not_yet("This element is a fill, and this version of StitchCraft does not sew fills yet"));
            return Ok(None);
        }
    };
    let Some(satin_params) = kept(SatinParams::from_set(set), diagnostics) else { return Ok(None) };
    let lengths = common.as_ref().map(|common| Lengths {
        min_stitch: shortest_stitch(common.min_stitch_length_mm, settings, profile),
        max_stitch: common.max_stitch_length_mm,
        jump: common.jump_length(settings.collapse_len),
    });
    let mut rng = SplitMix64::for_element(element.id.as_str(), common.as_ref().and_then(|common| common.random_seed).unwrap_or(0));
    let narrow = satin_params.satin_column && too_narrow(path, width, settings);
    if narrow {
        let (width, limit) = (mm(width.get()), mm(settings.min_satin_stroke_width.get()));
        let message = format!(
            "This satin column is drawn as one path, and its stroke, {width} mm wide, is no wider than the design's limit of {limit} mm, so it \
             is sewn as a stroke."
        );
        diagnostics.push(Diagnostic::new(Code::SatinTooNarrow, message));
    }
    let sewn = if satin_params.satin_column && !narrow {
        satin_column(element, path, &satin_params, lengths, neighbours, &mut rng, diagnostics, meter)?
    } else {
        stroke(element, path, lengths, &mut rng, diagnostics, meter)?
    };
    let (Some(common), Some(Lengths { min_stitch, .. }), Some((stitch_type, stitched))) = (common, lengths, sewn) else { return Ok(None) };
    diagnostics.extend(stitched.warnings);
    Ok(Some(Generated { common, stitch_type, groups: stitched.runs, min_stitch }))
}

/// The stitch lengths the settings every stitch type shares give an element.
#[derive(Clone, Copy)]
struct Lengths {
    /// Its shortest stitch: the machine's, or the element's or the design's when longer.
    min_stitch: Mm,
    /// Its longest stitch, when it sets one.
    max_stitch: Option<Mm>,
    /// Its jump length ([`CommonParams::jump_length`]).
    jump: Mm,
}

/// Whether a satin column is drawn too narrow to stitch across (`REQ-SAT-015`): as one subpath or none,
/// counting the subpaths drawn as Ink/Stitch counts them, with its stroke `width` no wider than the
/// design's `min_satin_stroke_width`. It is sewn as a stroke, as in Ink/Stitch.
fn too_narrow(path: &Path, width: Mm, settings: &DesignSettings) -> bool {
    path.subpaths.len() <= 1 && width.get() <= settings.min_satin_stroke_width.get()
}

/// A satin column's stitches by its `satin_method`, with the element's stitch `lengths`, between its
/// `neighbours`, varied at random by its `rng`, or `None` when it is skipped (`lengths` is `None` when the
/// settings every stitch type shares cannot be read). Its travel between underlays, and its way to its
/// start and end, take the first of the running stitch's lengths and its tolerance.
#[allow(clippy::too_many_arguments)]
fn satin_column(
    element: &Element,
    path: &Path,
    params: &SatinParams,
    lengths: Option<Lengths>,
    neighbours: &Neighbours,
    rng: &mut SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<(StitchType, Stitched)>, Exhausted> {
    let rails = match satin::shape(path, diagnostics, meter)? {
        None => return Ok(None),
        Some(SatinShape::CentreLine) => {
            diagnostics.push(not_yet("This element is a satin column drawn as its centre line, which this version of StitchCraft does not sew yet"));
            return Ok(None);
        }
        Some(SatinShape::Rails(rails)) => rails,
    };
    let running = kept(RunningParams::from_set(&element.params), diagnostics);
    let (Some(Lengths { min_stitch, max_stitch, jump }), Some(running)) = (lengths, running) else { return Ok(None) };
    // The registry reads a list of 1 length or more.
    let Some(&travel) = running.running_stitch_length_mm.first() else { return Ok(None) };
    let lengths = SatinLengths { min_stitch, max_stitch, travel, tolerance: running.running_stitch_tolerance_mm, jump };
    match StitchType::from_id(Family::Satin, params.satin_method) {
        Some(StitchType::SatinColumn) => Ok(Some((StitchType::SatinColumn, satin_stitch(&rails, params, lengths, neighbours, rng, meter)?))),
        _ => {
            let method = params.satin_method;
            diagnostics.push(not_yet(&format!("This element's satin method, `{method}`, is not sewn by this version of StitchCraft yet")));
            Ok(None)
        }
    }
}

/// A stroke's stitches by its `stroke_method`, with the element's stitch `lengths`, varied at random by its
/// `rng`, or `None` when it is skipped (`lengths` is `None` when the settings every stitch type shares
/// cannot be read).
fn stroke(
    element: &Element,
    path: &Path,
    lengths: Option<Lengths>,
    rng: &mut SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<(StitchType, Stitched)>, Exhausted> {
    let set = &element.params;
    let stroke = kept(StrokeParams::from_set(set), diagnostics);
    let (Some(stroke), Some(Lengths { min_stitch, max_stitch, .. })) = (stroke, lengths) else { return Ok(None) };
    let passes = kept(RepeatParams::from_set(set), diagnostics);
    match StitchType::from_id(Family::Stroke, stroke.stroke_method) {
        Some(StitchType::RunningStitch) => {
            let (Some(running), Some(passes)) = (kept(RunningParams::from_set(set), diagnostics), passes) else { return Ok(None) };
            Ok(Some((StitchType::RunningStitch, running_stitch(path, &running, &passes, min_stitch, rng, meter)?)))
        }
        Some(StitchType::ManualStitch) => {
            let Some(passes) = passes else { return Ok(None) };
            Ok(Some((StitchType::ManualStitch, manual_stitch(path, max_stitch, &passes, min_stitch, meter)?)))
        }
        _ => {
            let method = stroke.stroke_method;
            diagnostics.push(not_yet(&format!("This element's stroke method, `{method}`, is not sewn by this version of StitchCraft yet")));
            Ok(None)
        }
    }
}

/// What `element` offers the element before it to end near, read from its shape, its settings and the
/// design's `settings` alone, with the budget's work to itself (`REQ-GEN-003`): a stroke its first point,
/// a satin column its rails as they are sewn when it starts at its nearest point and otherwise its first
/// rail's start. A satin column too narrow to stitch across is a stroke (`REQ-SAT-015`). A fill offers
/// nothing until fills are sewn, and neither does an element whose shape or settings cannot be read.
pub fn approach(element: &Element, settings: &DesignSettings, budget: &Budget) -> Option<Approach> {
    let Shape::Stroke { path, width, .. } = &element.shape else { return None };
    let params = SatinParams::from_set(&element.params).ok()?.params;
    if !params.satin_column || too_narrow(path, *width, settings) {
        return path.subpaths.first().map(|subpath| Approach::Point(subpath.start));
    }
    let meter = &mut budget.meter();
    let SatinShape::Rails(rails) = satin::shape(path, &mut Vec::new(), meter).ok()?? else { return None };
    if params.start_at_nearest_point {
        return Some(Approach::Shape(satin::sewn_rails(&rails, &params, meter).ok()?.into()));
    }
    satin::first_point(&rails, &params, meter).ok()?.map(Approach::Point)
}

/// The shortest stitch for an element whose own is `own`: the machine's, or the element's own if it is
/// longer, or else the design's if that is. With no element (`None`), the design's or the machine's.
pub(crate) fn shortest_stitch(own: Option<Mm>, settings: &DesignSettings, profile: &MachineProfile) -> Mm {
    let machine = profile.min_stitch;
    own.or(settings.min_stitch_len).and_then(|own| Mm::new(own.get().max(machine.get())).ok()).unwrap_or(machine)
}

/// The parameters `read` gives, keeping their warnings; `None`, keeping their errors, when they cannot be
/// read.
fn kept<T>(read: Result<Validated<T>, Vec<Diagnostic>>, diagnostics: &mut Vec<Diagnostic>) -> Option<T> {
    match read {
        Ok(Validated { params, warnings }) => {
            diagnostics.extend(warnings);
            Some(params)
        }
        Err(problems) => {
            diagnostics.extend(problems);
            None
        }
    }
}

/// `SC-W0011`: `why` the element is not sewn.
fn not_yet(why: &str) -> Diagnostic {
    Diagnostic::new(Code::StitchTypeNotYet, format!("{why}, so it is skipped."))
}
