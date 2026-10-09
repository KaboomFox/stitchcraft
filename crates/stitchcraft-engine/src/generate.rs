//! Pipeline stage 3 (generate): each element to its stitch groups, by its stitch type's generator.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Generate. The element's parameters are read here, once:
//! the ones every stitch type shares, which assembly needs (locks, trims, stops), and its stitch type's,
//! which its generator needs. The stitch type picks the generator in one place, [`generate`]'s table, so a
//! new stitch type is its own module and one line here.
//!
//! An element that cannot be sewn is skipped, with a diagnostic that says why, and the rest of the design
//! still plans (`REQ-GEN-002`): a stitch type StitchCraft does not sew yet (`SC-W0011`), parameters it
//! cannot read (`SC-E0101`), or its work budget spent (`SC-E0004`). Each element has the budget's work to
//! itself, so one that runs out costs nothing but itself.

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Meter, Mm, Point, SplitMix64};
use stitchcraft_params::{ChoiceOption, Family, StitchType, Validated, params, unknown_keys};
use stitchcraft_plan::MachineProfile;

use crate::common::CommonParams;
use crate::design::{DesignSettings, Element, Shape};
use crate::generators::Stitched;
use crate::generators::manual::{ManualParams, manual_stitch};
use crate::generators::passes::RepeatParams;
use crate::generators::running::{RunningParams, running_stitch};
use crate::registry::PARAMETERS;

/// The stroke methods `stroke_method` offers, in Ink/Stitch's order, which its files count on: Ink/Stitch
/// gives the parameter's default as the first one's place in this list.
pub const STROKE_METHODS: &[ChoiceOption] =
    &[method(StitchType::RunningStitch), method(StitchType::RippleStitch), method(StitchType::ZigzagStitch), method(StitchType::ManualStitch)];

/// A stitch type as a method a settings window offers.
const fn method(stitch_type: StitchType) -> ChoiceOption {
    ChoiceOption { id: stitch_type.id(), label: stitch_type.name() }
}

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
}

/// What generating one element gives.
#[derive(Clone, Debug, PartialEq)]
pub struct Generation {
    /// The element generated; `None` when it is skipped.
    pub generated: Option<Generated>,
    /// What was changed, left out or wrong, each naming the element.
    pub diagnostics: Vec<Diagnostic>,
}

/// `element` generated for a design with `settings`, sewn on the machine `profile` describes, with the
/// budget's work to itself.
pub fn generate(element: &Element, settings: &DesignSettings, profile: &MachineProfile, budget: &Budget) -> Generation {
    let mut diagnostics = unknown_keys(&element.params, PARAMETERS);
    let generated = match sew(element, settings, profile, &mut diagnostics, &mut budget.meter()) {
        Ok(generated) => generated,
        Err(exhausted) => {
            diagnostics.push(exhausted.diagnostic(budget, None));
            None
        }
    };
    Generation { generated, diagnostics: diagnostics.into_iter().map(|d| d.with_element(element.id.clone())).collect() }
}

/// The element's stitch groups, or `None` when it is skipped; what it says about it goes to `diagnostics`.
fn sew(
    element: &Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Generated>, Exhausted> {
    let set = &element.params;
    let common = kept(CommonParams::from_set(set), diagnostics);
    let path = match &element.shape {
        Shape::Stroke(path) => path,
        Shape::Fill { .. } => {
            diagnostics.push(not_yet("This element is a fill, and this version of StitchCraft does not sew fills yet"));
            return Ok(None);
        }
    };
    let stroke = kept(StrokeParams::from_set(set), diagnostics);
    let (Some(common), Some(stroke)) = (common, stroke) else { return Ok(None) };
    let min_stitch = shortest_stitch(&common, settings, profile);
    let passes = kept(RepeatParams::from_set(set), diagnostics);
    let (stitch_type, stitched): (StitchType, Stitched) = match StitchType::from_id(Family::Stroke, stroke.stroke_method) {
        Some(StitchType::RunningStitch) => {
            let (Some(running), Some(passes)) = (kept(RunningParams::from_set(set), diagnostics), passes) else { return Ok(None) };
            let mut rng = SplitMix64::for_element(element.id.as_str(), running.random_seed.unwrap_or(0));
            (StitchType::RunningStitch, running_stitch(path, &running, &passes, min_stitch, &mut rng, meter)?)
        }
        Some(StitchType::ManualStitch) => {
            let (Some(manual), Some(passes)) = (kept(ManualParams::from_set(set), diagnostics), passes) else { return Ok(None) };
            (StitchType::ManualStitch, manual_stitch(path, &manual, &passes, min_stitch, meter)?)
        }
        _ => {
            let method = stroke.stroke_method;
            diagnostics.push(not_yet(&format!("This element's stroke method, `{method}`, is not sewn by this version of StitchCraft yet")));
            return Ok(None);
        }
    };
    diagnostics.extend(stitched.warnings);
    Ok(Some(Generated { common, stitch_type, groups: stitched.runs }))
}

/// The shortest stitch for an element: the machine's, or the element's own if it is longer, or else the
/// design's if that is.
fn shortest_stitch(common: &CommonParams, settings: &DesignSettings, profile: &MachineProfile) -> Mm {
    let machine = profile.min_stitch;
    let own = common.min_stitch_length_mm.or(settings.min_stitch_len);
    own.and_then(|own| Mm::new(own.get().max(machine.get())).ok()).unwrap_or(machine)
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
