//! Pipeline stage 3 (generate): each element to its stitch groups, by its stitch type's generator.
//!
//! Design: `docs/src/design/engine-pipeline.md` › Generate. The element's parameters are read here, once,
//! as its stitch type reads them (`REQ-PRM-004`): first the settings that choose the stitch type, each
//! alone, then the ones every stitch type shares, which assembly needs (locks, trims, stops), and its
//! stitch type's, which its generator needs. A setting the stitch type does not use is kept unread. A
//! satin column also reads the running stitch's length, for the needle's travel between its underlays,
//! and a tatami fill for its travel between rows. The stitch type picks the generator in one place,
//! [`generate`]'s table, so a new stitch type is its own module and one line here.
//!
//! A stroke whose `satin_column` setting is on is a satin column, whatever its `stroke_method` says, as in
//! Ink/Stitch, and its `satin_method` picks the generator. The element's random numbers all come from one
//! generator, seeded with its `random_seed`, or with its id when that is empty
//! ([`SplitMix64::for_element`]).
//!
//! Elements are generated in sewing order, each with its neighbours (`REQ-GEN-003`): where the elements
//! before it left the needle, and what the next one offers to end near, which [`approach`] reads from the
//! next element's shape and settings alone.
//!
//! An element that cannot be sewn is skipped, with a diagnostic that says why, and the rest of the design
//! still plans (`REQ-GEN-002`): a stitch type StitchCraft does not sew yet (`SC-W0011`), parameters it
//! cannot read (`SC-E0101`), a satin column with no rails (`SC-E0201`), or its work budget spent
//! (`SC-E0004`). Each element has the budget's work to itself, so one that runs out costs nothing but
//! itself. A fill whose `fill` setting is off sews nothing, and nothing is said of it (`REQ-FILL-003`).
//! Another fill's area is built first ([`crate::normalize::region`]), so what is left out of it is said
//! even when its settings cannot be used, and its `fill_method` then picks the generator: a tatami fill
//! ([`crate::generators::tatami`]), the only one sewn so far.

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Meter, Mm, Point, SplitMix64};
use stitchcraft_params::kinds::{Choice, Toggle};
use stitchcraft_params::{ChoiceOption, Family, ParamKind, ParamSet, ParamSpec, StitchType, Validated, params, read_param, unknown_keys};
use stitchcraft_plan::MachineProfile;

use crate::common::CommonParams;
use crate::design::{DesignSettings, Element, Join, Path, Shape};
use crate::generators::manual::manual_stitch;
use crate::generators::passes::RepeatParams;
use crate::generators::pull::PullParams;
use crate::generators::running::raised;
use crate::generators::running::{RunningParams, running_stitch};
use crate::generators::satin::{self, SatinLengths, SatinParams, satin_stitch};
use crate::generators::tatami::TatamiParams;
use crate::generators::tatami::sew::{Pull, Travel, tatami_fill};
use crate::generators::{Approach, Neighbours, Stitched, method, mm};
use crate::normalize::region::{self, Region};
use crate::registry::PARAMETERS;

/// The stroke methods `stroke_method` offers, in Ink/Stitch's order, which its files count on: Ink/Stitch
/// gives the parameter's default as the first one's place in this list.
pub const STROKE_METHODS: &[ChoiceOption] =
    &[method(StitchType::RunningStitch), method(StitchType::RippleStitch), method(StitchType::ZigzagStitch), method(StitchType::ManualStitch)];

/// The fill methods `fill_method` offers, in Ink/Stitch's order, which its files count on: Ink/Stitch gives
/// the parameter's default as the first one's place in this list.
pub const FILL_METHODS: &[ChoiceOption] = &[
    method(StitchType::TatamiFill),
    method(StitchType::CircularFill),
    method(StitchType::ContourFill),
    method(StitchType::CrossStitch),
    method(StitchType::GuidedFill),
    method(StitchType::LinearGradientFill),
    method(StitchType::MeanderFill),
    method(StitchType::TartanFill),
    method(StitchType::LegacyFill),
];

/// The stitch types of fills, one per method.
const FILLS: &[StitchType] = &[
    StitchType::TatamiFill,
    StitchType::CircularFill,
    StitchType::ContourFill,
    StitchType::CrossStitch,
    StitchType::GuidedFill,
    StitchType::LinearGradientFill,
    StitchType::MeanderFill,
    StitchType::TartanFill,
    StitchType::LegacyFill,
];

params! {
    /// How a fill is sewn.
    pub struct FillParams for FILLS;

    "Fill" {
        /// Whether the fill is sewn. Off, it sews nothing, and the shape's outline, if it has one, is sewn as
        /// its stroke settings say. The element before it still ends near it, as Ink/Stitch's does.
        fill: Toggle = "true", label "Sew the fill";

        /// The stitch the area is filled with. A tatami fill sews rows of running stitches across it. The
        /// other methods arrive in later versions; until then an element set to one is skipped
        /// (`SC-W0011`).
        fill_method: Choice = "tatami_fill", label "Fill method", choices FILL_METHODS;
    }
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
            diagnostics.push(exhausted.diagnostic(budget, Some(element.id.clone())));
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
    let set = &element.params;
    let trim = setting::<Toggle>(set, CommonParams::SPECS, "trim_after", &mut Vec::new())?;
    let stop = setting::<Toggle>(set, CommonParams::SPECS, "stop_after", &mut Vec::new())?;
    let what = match (trim, stop) {
        (true, true) => "the trim and the stop after it are",
        (true, false) => "the trim after it is",
        (false, true) => "the stop after it is",
        (false, false) => return None,
    };
    Some(Diagnostic::new(Code::TrimOrStopLeftOut, format!("This element sews no stitch, so {what} left out.")))
}

/// The value of the parameter `key` of the declaration whose parameters are `specs`, read alone from `set`,
/// whatever stitch type uses it; what is wrong with it goes to `diagnostics`.
fn setting<K: ParamKind>(set: &ParamSet, specs: &[ParamSpec], key: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<K::Value> {
    read_param::<K>(set, specs.iter().find(|spec| spec.key == key), None, specs, diagnostics)
}

/// The stitch type that sews `element` in a design with `settings`, chosen by the settings that choose
/// it, read first and alone: a fill's `fill_method`, and a stroke's `satin_method` when its
/// `satin_column` is on, or else its `stroke_method`. A satin column too narrow to stitch across is sewn
/// as a stroke, as in Ink/Stitch, and `SC-W0212` says so (`REQ-SAT-015`). `None` when one of them cannot be
/// read, which `diagnostics` says.
fn stitch_type(element: &Element, settings: &DesignSettings, diagnostics: &mut Vec<Diagnostic>) -> Option<StitchType> {
    let set = &element.params;
    let (family, specs) = match &element.shape {
        Shape::Fill { .. } => (Family::Fill, FillParams::SPECS),
        Shape::Stroke { path, width, .. } => {
            let satin = setting::<Toggle>(set, SatinParams::SPECS, "satin_column", diagnostics)?;
            if satin && too_narrow(path, *width, settings) {
                let (width, limit) = (mm(width.get()), mm(settings.min_satin_stroke_width.get()));
                let message = format!(
                    "This satin column is drawn as one path, and its stroke, {width} mm wide, is no wider than the design's limit of {limit} mm, so \
                     it is sewn as a stroke."
                );
                diagnostics.push(Diagnostic::new(Code::SatinTooNarrow, message));
                (Family::Stroke, StrokeParams::SPECS)
            } else if satin {
                (Family::Satin, SatinParams::SPECS)
            } else {
                (Family::Stroke, StrokeParams::SPECS)
            }
        }
    };
    method_type(family, setting::<Choice>(set, specs, family.method_param(), diagnostics)?, diagnostics)
}

/// The stitch type of `family` whose id is `method`, which the family's method parameter offers. Each
/// option is made from a stitch type of its family, so `None`, with `SC-E0009`, is a bug in StitchCraft.
fn method_type(family: Family, method: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<StitchType> {
    let found = StitchType::from_id(family, method);
    if found.is_none() {
        let message = format!("`{}` offers `{method}`, which is not a stitch type of {}.", family.method_param(), family.plural());
        diagnostics.push(Diagnostic::new(Code::InternalCheckFailed, message));
    }
    found
}

/// The element's stitch groups, or `None` when it is skipped; what it says about it goes to `diagnostics`.
/// Its settings are read as its stitch type reads them (`REQ-PRM-004`). A fill set not to be sewn sews
/// nothing, and its area is not built; another fill's area is built first, so what is left out of it is
/// said whatever its settings.
fn sew(
    element: &Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    neighbours: &Neighbours,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Generated>, Exhausted> {
    let set = &element.params;
    let region = match &element.shape {
        Shape::Fill { path, rule } => {
            let Some(true) = setting::<Toggle>(set, FillParams::SPECS, "fill", diagnostics) else { return Ok(None) };
            let built = region::build(path, *rule, meter)?;
            diagnostics.extend(built.diagnostics);
            built.region
        }
        Shape::Stroke { .. } => Region::default(),
    };
    let Some(stitch_type) = stitch_type(element, settings, diagnostics) else { return Ok(None) };
    let common = kept(CommonParams::from_set_as(set, stitch_type), diagnostics);
    let shared = common.as_ref().map(|common| Shared {
        min_stitch: shortest_stitch(common.min_stitch_length_mm, settings, profile),
        max_stitch: common.max_stitch_length_mm,
        jump: common.jump_length(settings.collapse_len),
        force_locks: common.force_lock_stitches,
    });
    let sewing = Sewing { set, stitch_type, shared, neighbours };
    let mut rng = SplitMix64::for_element(element.id.as_str(), common.as_ref().and_then(|common| common.random_seed));
    let stitched = match &element.shape {
        Shape::Fill { .. } => fill(&sewing, &region, &mut rng, diagnostics, meter)?,
        Shape::Stroke { path, width, join } if stitch_type.family() == Family::Satin => {
            satin_column(&sewing, (path, *width, *join), &mut rng, diagnostics, meter)?
        }
        Shape::Stroke { path, .. } => stroke(&sewing, path, &rng, diagnostics, meter)?,
    };
    let (Some(common), Some(Shared { min_stitch, .. }), Some(stitched)) = (common, shared, stitched) else { return Ok(None) };
    diagnostics.extend(stitched.warnings);
    Ok(Some(Generated { common, stitch_type, groups: stitched.runs, min_stitch }))
}

/// What the settings every stitch type shares give an element's generator.
#[derive(Clone, Copy)]
struct Shared {
    /// Its shortest stitch: the machine's, or the element's or the design's when longer.
    min_stitch: Mm,
    /// Its longest stitch, when it sets one.
    max_stitch: Option<Mm>,
    /// Its jump length ([`CommonParams::jump_length`]).
    jump: Mm,
    /// Whether locks part each group from the next (`force_lock_stitches`).
    force_locks: bool,
}

/// An element as its generator reads it.
struct Sewing<'a> {
    /// Its settings.
    set: &'a ParamSet,
    /// The stitch type that sews it, which its settings are read as.
    stitch_type: StitchType,
    /// What the settings every stitch type shares give it; `None` when they cannot be read, which was
    /// said.
    shared: Option<Shared>,
    /// Where the elements before it left the needle, and what the next one offers to end near.
    neighbours: &'a Neighbours,
}

/// A declaration's `from_set_as`, which reads its parameters as an element of a stitch type reads them.
type FromSetAs<T> = fn(&ParamSet, StitchType) -> Result<Validated<T>, Vec<Diagnostic>>;

impl Sewing<'_> {
    /// The parameters of one declaration, read as the element's stitch type reads them, keeping their
    /// warnings and errors in `diagnostics`; `None` when they cannot be read.
    fn read<T>(&self, from_set_as: FromSetAs<T>, diagnostics: &mut Vec<Diagnostic>) -> Option<T> {
        kept(from_set_as(self.set, self.stitch_type), diagnostics)
    }
}

/// Whether a satin column is drawn too narrow to stitch across (`REQ-SAT-015`): as one subpath or none,
/// counting the subpaths drawn as Ink/Stitch counts them, with its stroke `width` no wider than the
/// design's `min_satin_stroke_width`. It is sewn as a stroke, as in Ink/Stitch.
fn too_narrow(path: &Path, width: Mm, settings: &DesignSettings) -> bool {
    path.subpaths.len() <= 1 && width.get() <= settings.min_satin_stroke_width.get()
}

/// A fill's stitches over its `region` as `sewing` says, varied at random by its `rng`, or `None` when it
/// is skipped. Its `fill_method` picks the generator. A tatami fill's needle travels in the first of the
/// running stitch's lengths, at least twice the shortest stitch, and within its tolerance, between its top
/// rows and between the rows of its underlay (`DEV-SAT-005`).
fn fill(
    sewing: &Sewing,
    region: &Region,
    rng: &mut SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let pull = sewing.read(PullParams::from_set_as, diagnostics);
    let (Some(tatami), Some(running), Some(pull), Some(shared)) = (tatami, running, pull, sewing.shared) else { return Ok(None) };
    if sewing.stitch_type != StitchType::TatamiFill {
        let method = sewing.stitch_type.id();
        diagnostics.push(not_yet(&format!("This element's fill method, `{method}`, is not sewn by this version of StitchCraft yet")));
        return Ok(None);
    }
    // A fill's longest stitch has a default of its own, so it is never empty; the registry reads it so.
    let (Some(longest), Some(&length)) = (shared.max_stitch, running.running_stitch_length_mm.first()) else {
        diagnostics.push(Diagnostic::new(Code::InternalCheckFailed, "A fill's longest stitch or travel stitch length is missing."));
        return Ok(None);
    };
    let min = shared.min_stitch.get();
    let travel = Travel {
        length: raised("running_stitch_length_mm", length, min, diagnostics),
        tolerance: running.running_stitch_tolerance_mm.get(),
        min_stitch: min,
        underpath: tatami.underpath,
        jump: shared.jump.get(),
    };
    let fill = tatami.fill(longest, &running, travel, fill_pull(&pull, diagnostics), shared.force_locks);
    let neighbours = sewing.neighbours;
    Ok(Some(tatami_fill(region, &fill, neighbours.needle, neighbours.next.as_ref(), rng, meter)?))
}

/// A fill's pull compensation, as its settings give it: Ink/Stitch reads a value below 0 as 0, and so does a
/// fill here, saying so (`SC-W0312`).
fn fill_pull(pull: &PullParams, diagnostics: &mut Vec<Diagnostic>) -> Pull {
    let (mm, percent) = (pull.pull_compensation_mm.map(Mm::get), pull.pull_compensation_percent);
    let below: Vec<&str> = [("`pull_compensation_mm`", mm), ("`pull_compensation_percent`", percent)]
        .into_iter()
        .filter(|(_, values)| values.iter().any(|value| *value < 0.0))
        .map(|(key, _)| key)
        .collect();
    let message = match below.as_slice() {
        [] => None,
        [key] => Some(format!("{key} is below 0, and a fill reads it as 0, as Ink/Stitch does.")),
        keys => Some(format!("{} are below 0, and a fill reads them as 0, as Ink/Stitch does.", keys.join(" and "))),
    };
    diagnostics.extend(message.map(|message| Diagnostic::new(Code::FillPullCompensationNegative, message)));
    Pull { mm: mm.map(|value| value.max(0.0)), share: percent.map(|value| value.max(0.0) / 100.0) }
}

/// A satin column's stitches as `sewing` says, varied at random by its `rng`, or `None` when it is skipped:
/// its path is `width` wide with `join` at its corners when it is its centre line. Its `satin_method` picks
/// the generator. Its travel between underlays, and its way to its start and end, take the first of the
/// running stitch's lengths and its tolerance.
fn satin_column(
    sewing: &Sewing,
    (path, width, join): (&Path, Mm, Join),
    rng: &mut SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    let Some(params) = sewing.read(SatinParams::from_set_as, diagnostics) else { return Ok(None) };
    let Some(rails) = satin::rails(path, width, join, diagnostics, meter)? else { return Ok(None) };
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let pull = sewing.read(PullParams::from_set_as, diagnostics);
    let (Some(Shared { min_stitch, max_stitch, jump, .. }), Some(running), Some(pull)) = (sewing.shared, running, pull) else { return Ok(None) };
    // The registry reads a list of 1 length or more.
    let Some(&travel) = running.running_stitch_length_mm.first() else { return Ok(None) };
    let lengths = SatinLengths { min_stitch, max_stitch, travel, tolerance: running.running_stitch_tolerance_mm, jump };
    if sewing.stitch_type != StitchType::SatinColumn {
        let method = sewing.stitch_type.id();
        diagnostics.push(not_yet(&format!("This element's satin method, `{method}`, is not sewn by this version of StitchCraft yet")));
        return Ok(None);
    }
    Ok(Some(satin_stitch(&rails, &params, &pull, lengths, sewing.neighbours, rng, meter)?))
}

/// A stroke's stitches as `sewing` says, varied at random by streams of its `rng`, or `None` when it is
/// skipped. Its `stroke_method` picks the generator.
fn stroke(
    sewing: &Sewing,
    path: &Path,
    rng: &SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    let passes = sewing.read(RepeatParams::from_set_as, diagnostics);
    let Some(Shared { min_stitch, max_stitch, .. }) = sewing.shared else { return Ok(None) };
    match sewing.stitch_type {
        StitchType::RunningStitch => {
            let (Some(running), Some(passes)) = (sewing.read(RunningParams::from_set_as, diagnostics), passes) else { return Ok(None) };
            Ok(Some(running_stitch(path, &running, &passes, min_stitch, rng, meter)?))
        }
        StitchType::ManualStitch => {
            let Some(passes) = passes else { return Ok(None) };
            Ok(Some(manual_stitch(path, max_stitch, &passes, min_stitch, meter)?))
        }
        other => {
            let method = other.id();
            diagnostics.push(not_yet(&format!("This element's stroke method, `{method}`, is not sewn by this version of StitchCraft yet")));
            Ok(None)
        }
    }
}

/// What `element` offers the element before it to end near, read from its shape, its settings and the
/// design's `settings` alone, with the budget's work to itself (`REQ-GEN-003`): a stroke its first point,
/// a satin column its rails as they are sewn when it starts at its nearest point and otherwise its first
/// rail's start. A satin column too narrow to stitch across is a stroke (`REQ-SAT-015`). A tatami fill
/// starts at its point nearest the needle and offers its area, its parts, even when it is set not to be
/// sewn, as Ink/Stitch's next element is the next in the drawing. Where its stitch type is known, an
/// element offers what Ink/Stitch's would, even of a method StitchCraft does not sew yet (ADR 0014): a
/// stroke of any method its first point, as Ink/Stitch's strokes all do, and a satin column of any method
/// its rails. A fill of a method StitchCraft does not sew offers nothing, since what it would offer is not
/// known. Neither does an element whose shape cannot be read, or the settings its offer depends on: those
/// that choose its stitch type, and a satin column's. They are read as its stitch type reads them.
pub fn approach(element: &Element, settings: &DesignSettings, budget: &Budget) -> Option<Approach> {
    let stitch_type = stitch_type(element, settings, &mut Vec::new())?;
    let (path, width, join) = match &element.shape {
        Shape::Fill { .. } if stitch_type != StitchType::TatamiFill => return None,
        Shape::Fill { path, rule } => {
            let parts = region::build(path, *rule, &mut budget.meter()).ok()?.region.parts;
            return (!parts.is_empty()).then_some(Approach::Area(parts));
        }
        Shape::Stroke { path, .. } if stitch_type.family() != Family::Satin => {
            return path.subpaths.first().map(|subpath| Approach::Point(subpath.start));
        }
        Shape::Stroke { path, width, join } => (path, width, join),
    };
    let params = SatinParams::from_set_as(&element.params, stitch_type).ok()?.params;
    let meter = &mut budget.meter();
    let rails = satin::rails(path, *width, *join, &mut Vec::new(), meter).ok()??;
    if params.start_at_nearest_point {
        return Some(Approach::Shape(satin::sewn_rails(&rails, &params, meter).ok()?.into()));
    }
    satin::first_point(&rails, &params, meter).ok()?.map(Approach::Point)
}

/// The shortest stitch for an element whose own is `own`: the machine's, or the element's own if it is
/// longer, or else the design's if that is. With no element (`None`), the design's or the machine's.
pub fn shortest_stitch(own: Option<Mm>, settings: &DesignSettings, profile: &MachineProfile) -> Mm {
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

#[cfg(test)]
mod tests {
    use stitchcraft_core::ElementId;
    use stitchcraft_plan::{Rgb, Thread};

    use super::*;
    use crate::design::{FillRule, Segment, Subpath};

    fn element(shape: Shape, params: &[(&str, &str)]) -> Element {
        let thread = Thread::new(Rgb::new(0, 0, 0));
        Element { id: ElementId::new("e").unwrap(), name: None, shape, thread, params: params.iter().copied().collect() }
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<Code> {
        diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn a_fill_is_sewn_by_its_method_and_without_a_longest_stitch_is_a_bug() {
        let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
        let square = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] };
        let region = region::build(&square, FillRule::NonZero, &mut Budget::DEFAULT.meter()).unwrap().region;
        let shared = |max| Some(Shared { min_stitch: Mm::new(0.3).unwrap(), max_stitch: max, jump: Mm::new(3.0).unwrap(), force_locks: false });
        let neighbours = Neighbours::default();
        let sew = |set: &ParamSet, stitch_type, shared, diagnostics: &mut Vec<Diagnostic>| {
            let (rng, meter) = (&mut SplitMix64::new(1), &mut Budget::DEFAULT.meter());
            fill(&Sewing { set, stitch_type, shared, neighbours: &neighbours }, &region, rng, diagnostics, meter).unwrap()
        };
        let mut diagnostics = Vec::new();
        let sewn = sew(&ParamSet::new(), StitchType::TatamiFill, shared(Mm::new(4.0).ok()), &mut diagnostics);
        assert_eq!((sewn.is_some(), codes(&diagnostics)), (true, vec![]));
        // A method not sewn yet, whose settings it does not read, wrong as they are here.
        let tatami = [("row_spacing_mm", "wide")].into_iter().collect();
        let sewn = sew(&tatami, StitchType::ContourFill, shared(Mm::new(4.0).ok()), &mut diagnostics);
        assert_eq!((sewn.is_none(), codes(&diagnostics)), (true, vec![Code::StitchTypeNotYet]));
        // The registry gives a fill a longest stitch of its own (`ParamSpec::in_family`), so only a bug in
        // StitchCraft can leave it out.
        let mut diagnostics = Vec::new();
        let sewn = sew(&ParamSet::new(), StitchType::TatamiFill, shared(None), &mut diagnostics);
        assert_eq!((sewn.is_none(), codes(&diagnostics)), (true, vec![Code::InternalCheckFailed]));
    }

    #[test]
    fn the_settings_that_choose_it_give_an_element_s_stitch_type() {
        let path = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] };
        let fill = || Shape::Fill { path: path.clone(), rule: FillRule::NonZero };
        let wide = || Shape::Stroke { path: path.clone(), width: Mm::new(3.0).unwrap(), join: Join::UNSET };
        let chosen = |shape: Shape, params: &[(&str, &str)]| {
            let mut diagnostics = Vec::new();
            (stitch_type(&element(shape, params), &DesignSettings::default(), &mut diagnostics), codes(&diagnostics))
        };
        assert_eq!(chosen(fill(), &[]), (Some(StitchType::TatamiFill), vec![]));
        assert_eq!(chosen(fill(), &[("fill_method", "contour_fill")]), (Some(StitchType::ContourFill), vec![]));
        assert_eq!(chosen(wide(), &[]), (Some(StitchType::RunningStitch), vec![]));
        assert_eq!(chosen(wide(), &[("stroke_method", "manual_stitch")]), (Some(StitchType::ManualStitch), vec![]));
        // `satin_column` on, the satin's method; off, the stroke's, whatever the other says.
        assert_eq!(chosen(wide(), &[("satin_column", "true"), ("stroke_method", "manual_stitch")]), (Some(StitchType::SatinColumn), vec![]));
        assert_eq!(chosen(wide(), &[("satin_column", "true"), ("satin_method", "e_stitch")]), (Some(StitchType::EStitch), vec![]));
        assert_eq!(chosen(wide(), &[("satin_method", "plaid")]), (Some(StitchType::RunningStitch), vec![]));
        // A satin column too narrow to stitch across is a stroke.
        assert_eq!(chosen(Shape::stroke(path.clone()), &[("satin_column", "true")]), (Some(StitchType::RunningStitch), vec![Code::SatinTooNarrow]));
        // A setting that chooses it and cannot be read is said.
        assert_eq!(chosen(wide(), &[("satin_column", "true"), ("satin_method", "plaid")]), (None, vec![Code::ParamInvalid]));
        assert_eq!(chosen(wide(), &[("satin_column", "maybe")]), (None, vec![Code::ParamInvalid]));
        // A method of another family, which only a bug in the method lists could offer.
        let mut diagnostics = Vec::new();
        assert_eq!(method_type(Family::Stroke, "tatami_fill", &mut diagnostics), None);
        let said: Vec<String> = diagnostics.iter().map(ToString::to_string).collect();
        assert_eq!(said, ["error SC-E0009: `stroke_method` offers `tatami_fill`, which is not a stitch type of strokes."]);
    }
}
