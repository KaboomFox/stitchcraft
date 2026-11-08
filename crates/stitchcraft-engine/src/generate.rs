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
//! ([`crate::generators::tatami`]), a contour, meander, circular or tartan fill, or a cross stitch.
//!
//! An element is cut to its clips, the clip paths round it, before its generator sees it (`REQ-GEN-005`): a
//! fill's area when it is built ([`region::build_clipped`]), and a stroke's subpaths where they cross a
//! clip's outline (`region::cut`). A satin column is sewn whole, as in Ink/Stitch (`SC-W0217`), and an
//! element its clips leave nothing of sews nothing (`SC-W0013`).

pub(crate) mod layered;

use std::borrow::Cow;

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Meter, Mm, Point, SplitMix64};
use stitchcraft_params::kinds::{Choice, Toggle};
use stitchcraft_params::{
    ChoiceOption, Family, ParamKind, ParamSet, ParamSpec, StitchType, Validated, find, left_to_others, params, read_param, unknown_keys,
};
use stitchcraft_plan::{MachineProfile, Rgb};

use crate::common::{CommonParams, STOP_AFTER, TRIM_AFTER};
use crate::design::{Clip, DesignSettings, Element, Hints, Join, Path, Shape};
use crate::generators::circular::{self, Circular, circular_fill};
use crate::generators::contour::{self, Contour, ContourParams, Stitching, contour_fill};
use crate::generators::cross::{self, Cross, CrossParams, cross_stitch};
use crate::generators::manual::{self, manual_stitch};
use crate::generators::meander::{self, Meander, MeanderParams, meander_fill};
use crate::generators::passes::{PathFillParams, RepeatParams};
use crate::generators::pull::PullParams;
use crate::generators::ripple::{self, Ripple, RippleParams, ripple_stitch};
use crate::generators::running::raised;
use crate::generators::running::{RunningParams, running_stitch};
use crate::generators::satin::{self, SATIN_COLUMN, SatinLengths, SatinParams, satin_stitch};
use crate::generators::tartan::{self, Tartan, TartanParams, tartan_fill};
use crate::generators::tatami::TatamiParams;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::{Fill, Layer as FillLayer, Pull, Travel, tatami_fill, top_rows};
use crate::generators::zigzag::{ZigzagStrokeParams, zigzag_stroke};
use crate::generators::{Approach, Colour, Layer, Neighbours, Stitched, method, mm};
use crate::normalize::region::{self, Region, cut};
use crate::normalize::satin::Satin;
use crate::normalize::stroke;
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

        /// The stitch the area is filled with. A tatami fill sews rows of running stitches across it, a
        /// contour fill rings that follow its outline inward, a meander fill one line that wanders through a
        /// tiling, a circular fill a spiral round its middle, and a tartan fill a plaid of rows in the
        /// colours of its sett. A fill set to Ink/Stitch's legacy fill is sewn as a tatami fill without
        /// underlay (`SC-W0314`). The other methods arrive in later versions; until then an element set to
        /// one is skipped (`SC-W0011`).
        fill_method: Choice = "tatami_fill", label "Fill method", choices FILL_METHODS;
    }
}

params! {
    /// How a stroke is sewn.
    pub struct StrokeParams for &[StitchType::RunningStitch, StitchType::RippleStitch, StitchType::ZigzagStitch, StitchType::ManualStitch];

    "Stroke" {
        /// The stitch the path is sewn with. A running stitch goes along it and a zigzag across it, and manual
        /// stitch puts a needle point on each of its nodes. A ripple stitch sews lines that spread out from
        /// the path to its target point, or along it from one rail to the other where it is drawn as a satin
        /// column's rails and rungs.
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
    /// The colour of each group, by its place, as its generator gives them ([`Stitched::colours`]): empty when
    /// every group is sewn in the element's thread (`REQ-ASM-006`).
    pub colours: Vec<Colour>,
    /// The shortest stitch it was sewn with, which finalize holds its stitches to as well.
    pub min_stitch: Mm,
    /// Which of its stitches these are: all of them, or one of the 2 layers of an element in a run of
    /// underlays first (`crate::underlays_first`). Assembly tags an underlay's stitches underlay, and puts
    /// the element's trim and stop after its top stitches.
    pub layer: Layer,
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
/// `neighbours`, with the budget's work to itself, and its stitches.
pub fn generate(element: &Element, settings: &DesignSettings, profile: &MachineProfile, neighbours: &Neighbours, budget: &Budget) -> Generation {
    generate_with(element, settings, profile, neighbours, budget, &mut budget.meter())
        .unwrap_or_else(|diagnostics| Generation { generated: None, diagnostics })
}

/// `element` generated as [`generate`] generates it, with `meter`'s work and stitches, a stitch for each of
/// its needle points (`REQ-GEN-004`); `Err` with what it says when that is more than `meter` has left:
/// `budget`'s limit for the design, which ends its planning. `SC-E0004` among them says so, naming it.
pub(crate) fn generate_with(
    element: &Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    neighbours: &Neighbours,
    budget: &Budget,
    meter: &mut Meter,
) -> Result<Generation, Vec<Diagnostic>> {
    let mut diagnostics = unknown_keys(&element.params, PARAMETERS);
    let sewn = sew(element, settings, profile, neighbours, &mut diagnostics, meter);
    let over = matches!(sewn, Err(Exhausted::Stitches));
    let generated = within(element, budget, &mut diagnostics, sewn);
    if !generated.as_ref().is_some_and(|g| g.groups.iter().any(|group| !group.is_empty())) {
        diagnostics.extend(trim_or_stop_left_out(element));
    }
    let diagnostics = named(element, diagnostics);
    if over { Err(diagnostics) } else { Ok(Generation { generated, diagnostics }) }
}

/// What a preview shows of a fill too costly to sew: where its top stitches lie ([`rows`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Rows {
    /// The segments of its top rows, or of a contour fill's path, in millimetres; `None` when it is skipped,
    /// or is not a fill.
    pub segments: Option<Vec<Segment>>,
    /// What was changed, left out or wrong, each naming the element.
    pub diagnostics: Vec<Diagnostic>,
}

/// The segments of the top rows of `element`, a fill, in a design with `settings` on the machine `profile`
/// describes, with the budget's work: where [`generate`] lays its top stitches, without the stitches, the
/// underlay or the needle's travel, for a small part of the work. A contour fill has no rows, and gives the
/// segments of its path round its rings. Its settings are read as `generate` reads them, and what they say
/// is said the same.
pub fn rows(element: &Element, settings: &DesignSettings, profile: &MachineProfile, budget: &Budget) -> Rows {
    let mut diagnostics = unknown_keys(&element.params, PARAMETERS);
    let laid = top_rows_of(element, settings, profile, &mut diagnostics, &mut budget.meter());
    let segments = within(element, budget, &mut diagnostics, laid);
    Rows { segments, diagnostics: named(element, diagnostics) }
}

/// What `run` gave, or `None`, with `SC-E0004` in `diagnostics`, when it spent `element`'s budget.
fn within<T>(element: &Element, budget: &Budget, diagnostics: &mut Vec<Diagnostic>, run: Result<Option<T>, Exhausted>) -> Option<T> {
    run.unwrap_or_else(|exhausted| {
        diagnostics.push(exhausted.diagnostic(budget, Some(element.id.clone())));
        None
    })
}

/// `diagnostics`, each naming `element`.
fn named(element: &Element, diagnostics: Vec<Diagnostic>) -> Vec<Diagnostic> {
    diagnostics.into_iter().map(|d| d.with_element(element.id.clone())).collect()
}

/// `SC-W0505` for the trim and stop that `element`, which sews nothing, sets after it: assembly has no
/// place for them. Its settings were read once already; a setting that cannot be read was reported then.
/// Automatic digitizing says it of an element it leaves out (`crate::auto`).
pub(crate) fn trim_or_stop_left_out(element: &Element) -> Option<Diagnostic> {
    let set = &element.params;
    let trim = setting::<Toggle>(set, CommonParams::SPECS, TRIM_AFTER, &mut Vec::new())?;
    let stop = setting::<Toggle>(set, CommonParams::SPECS, STOP_AFTER, &mut Vec::new())?;
    let what = match (trim, stop) {
        (true, true) => "the trim and the stop after it are",
        (true, false) => "the trim after it is",
        (false, true) => "the stop after it is",
        (false, false) => return None,
    };
    Some(Diagnostic::new(Code::TrimOrStopLeftOut, format!("This element sews no stitch, so {what} left out.")))
}

/// Whether `element` sets a stop after it, read alone, whatever stitch type uses it: the sewing order moves
/// nothing across one (`crate::auto`). A stop that can't be read stops nothing, and that is said when the
/// element's settings are read for sewing.
pub fn stops_after(element: &Element) -> bool {
    setting::<Toggle>(&element.params, CommonParams::SPECS, STOP_AFTER, &mut Vec::new()).unwrap_or(false)
}

/// The colours of the threads `element` sews in, in their order, read before it is generated, as the
/// sewing order needs them (`crate::auto`, `REQ-AUTO-024`): its own thread's, for every stitch type but a
/// tartan fill. A tartan gives its runs threads of their own ([`Stitched::colours`]), and they are read here
/// from its settings alone: its own thread for its underlay, when it has one, then the colours of its sett
/// ([`tartan::threads`]). M10.12's gradient fill will give its stops' so too.
pub(crate) fn colours(element: &Element) -> Vec<Rgb> {
    let own = element.thread.color;
    if !matches!(element.shape, Shape::Fill { .. }) || stitch_type_of(element, &DesignSettings::default()) != Some(StitchType::TartanFill) {
        return vec![own];
    }
    let underlay = setting::<Toggle>(&element.params, TatamiParams::SPECS, "fill_underlay", &mut Vec::new()).unwrap_or(true);
    tartan::threads(&element.params, own, underlay)
}

/// The value of the parameter `key` of the declaration whose parameters are `specs`, read alone from `set`,
/// whatever stitch type uses it; what is wrong with it goes to `diagnostics`.
fn setting<K: ParamKind>(set: &ParamSet, specs: &[ParamSpec], key: &str, diagnostics: &mut Vec<Diagnostic>) -> Option<K::Value> {
    read_param::<K>(set, specs.iter().find(|spec| spec.key == key), None, specs, diagnostics)
}

/// The stitch type that sews `element` in a design with `settings`, chosen by the settings that choose
/// it, read first and alone: a fill's `fill_method`, and a stroke's `satin_method` when its
/// `satin_column` is on, or else its `stroke_method`. A satin column too narrow to stitch across is sewn
/// as a stroke, as in Ink/Stitch, and `SC-W0212` says so (`REQ-SAT-015`). A fill set to Ink/Stitch's legacy
/// fill is a tatami fill, which `SC-W0314` says, and sews no underlay ([`legacy`], `REQ-FILL-005`). Each of
/// its own settings that it sets, [`LEGACY_ONLY`], is named with `SC-W0105`. `None`
/// when one of them cannot be read, which `diagnostics` says.
fn stitch_type(element: &Element, settings: &DesignSettings, diagnostics: &mut Vec<Diagnostic>) -> Option<StitchType> {
    let set = &element.params;
    let (family, specs) = match &element.shape {
        Shape::Fill { .. } => (Family::Fill, FillParams::SPECS),
        Shape::Stroke { path, width, .. } => {
            let satin = setting::<Toggle>(set, SatinParams::SPECS, SATIN_COLUMN, diagnostics)?;
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
    let chosen = method_type(family, setting::<Choice>(set, specs, family.method_param(), diagnostics)?, diagnostics)?;
    if chosen == StitchType::LegacyFill {
        // `unknown_keys` named those that no stitch type reads, and here the others are named: a ripple stitch
        // reads a `reverse` of its own, which doesn't make it the fill's.
        for key in LEGACY_ONLY.into_iter().filter(|key| set.get(key).is_some() && find(PARAMETERS, key).is_some()) {
            let message = format!(
                "`{key}` is a setting of Ink/Stitch's legacy fill, which StitchCraft does not sew; it was kept but does not change the stitches."
            );
            diagnostics.push(Diagnostic::new(Code::ParamUnknown, message));
        }
        let message = "This element's fill method is Ink/Stitch's legacy fill, which StitchCraft does not sew, so it is sewn as a tatami \
                       fill without underlay.";
        diagnostics.push(Diagnostic::new(Code::FillLegacyAsTatami, message));
    }
    Some(sewn_as(chosen))
}

/// The settings of Ink/Stitch's legacy fill that StitchCraft keeps without reading (`DEV-FILL-014`): `flip`
/// starts its rows from the other side, and `reverse` sews it backwards. Ink/Stitch's other fills read
/// neither.
const LEGACY_ONLY: [&str; 2] = ["flip", "reverse"];

/// The stitch type that sews an element set to `asked`: itself, but for Ink/Stitch's legacy fill, which
/// StitchCraft does not build and sews as a tatami fill without underlay (`REQ-FILL-005`).
pub const fn sewn_as(asked: StitchType) -> StitchType {
    match asked {
        StitchType::LegacyFill => StitchType::TatamiFill,
        other => other,
    }
}

/// Whether `set` asks for Ink/Stitch's legacy fill, which [`stitch_type`] gives a tatami fill, and which
/// sews no underlay (`REQ-FILL-005`). Read alone, as `stitch_type` reads it, which says what is wrong with it.
fn legacy(set: &ParamSet) -> bool {
    setting::<Choice>(set, FillParams::SPECS, Family::Fill.method_param(), &mut Vec::new()) == Some(StitchType::LegacyFill.id())
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
fn sew(
    element: &Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    neighbours: &Neighbours,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Generated>, Exhausted> {
    let Some(Prepared { region, common, sewing }) = prepare(element, settings, profile, diagnostics, meter)? else { return Ok(None) };
    let mut rng = SplitMix64::for_element(element.id.as_str(), common.as_ref().and_then(|common| common.random_seed));
    let stitched = match &element.shape {
        Shape::Fill { .. } => fill(&sewing, neighbours, &region, &mut rng, diagnostics, meter)?,
        Shape::Stroke { path, width, join, .. } if sewing.stitch_type.family() == Family::Satin => {
            clip_not_applied(element, diagnostics);
            satin_column(&sewing, neighbours, (path, *width, *join), &mut rng, diagnostics, meter)?
        }
        // A ripple stitch cuts its own stitches to its clips, and routes the pieces (`REQ-RIP-017`).
        Shape::Stroke { path, .. } if sewing.stitch_type == StitchType::RippleStitch => {
            ripple(&sewing, path, &element.clips, &rng, diagnostics, meter)?
        }
        Shape::Stroke { path, width, .. } => {
            let Some(path) = cut::stroke(path, &element.clips, cut_line(sewing.stitch_type), diagnostics, meter)? else { return Ok(None) };
            stroke(&sewing, &path, *width, &rng, diagnostics, meter)?
        }
    };
    let (Some(common), Some(Shared { min_stitch, .. }), Some(stitched)) = (common, sewing.shared, stitched) else { return Ok(None) };
    diagnostics.extend(stitched.warnings);
    Ok(Some(Generated { common, stitch_type: sewing.stitch_type, groups: stitched.runs, colours: stitched.colours, min_stitch, layer: Layer::Whole }))
}

/// `SC-W0217` in `diagnostics` when `element`, a satin column, is cut to clip paths, which StitchCraft doesn't
/// apply to a satin column (`REQ-GEN-005`).
fn clip_not_applied(element: &Element, diagnostics: &mut Vec<Diagnostic>) {
    if !element.clips.is_empty() {
        let message = "This satin column is cut to clip paths, which StitchCraft doesn't apply to a satin column, so it is sewn whole, as \
                       Ink/Stitch sews it.";
        diagnostics.push(Diagnostic::new(Code::SatinClipNotApplied, message));
    }
}

/// The segments of a fill's top rows, or `None` when it is skipped or is not a fill; what it says about it
/// goes to `diagnostics`.
fn top_rows_of(
    element: &Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Vec<Segment>>, Exhausted> {
    if !matches!(element.shape, Shape::Fill { .. }) {
        return Ok(None);
    }
    let Some(Prepared { region, common, sewing }) = prepare(element, settings, profile, diagnostics, meter)? else { return Ok(None) };
    // A contour, meander or circular fill has no rows: the preview shows its path, round the rings, through
    // the tile as the element's seed lays it, or along the spiral's pieces. A cross stitch fill shows its
    // crosses' bars, or its half crosses' diagonals.
    match sewing.stitch_type {
        StitchType::ContourFill => {
            let Some(contour) = contour_settings(&sewing, diagnostics) else { return Ok(None) };
            return contour::top_path(&region, &contour, meter).map(Some);
        }
        StitchType::CircularFill => {
            let Some(circular) = circular_settings(&sewing, diagnostics) else { return Ok(None) };
            return circular::top_path(&region, &circular, meter).map(Some);
        }
        StitchType::MeanderFill => {
            let Some(meander) = meander_settings(&sewing, diagnostics) else { return Ok(None) };
            let rng = SplitMix64::for_element(element.id.as_str(), common.as_ref().and_then(|common| common.random_seed));
            return meander::top_path(&region, &meander, &rng, meter).map(Some);
        }
        StitchType::CrossStitch => {
            let Some(cross) = cross_settings(&sewing, diagnostics) else { return Ok(None) };
            return cross::top_path(&region, &cross, meter).map(Some);
        }
        StitchType::TartanFill => {
            let Some(tartan) = tartan_settings(&sewing, diagnostics) else { return Ok(None) };
            return tartan::top_path(&region, &tartan, meter).map(Some);
        }
        _ => {}
    }
    let Some(fill) = fill_settings(&sewing, diagnostics) else { return Ok(None) };
    top_rows(&region, &fill, meter).map(Some)
}

/// An element read for sewing: its area, when it is a fill, the settings every stitch type shares, and how
/// its generator reads it.
struct Prepared<'a> {
    region: Region,
    common: Option<CommonParams>,
    sewing: Sewing<'a>,
}

/// `element` read for sewing, or `None` when it is skipped; what it says about it goes to `diagnostics`.
/// Its settings are read as its stitch type reads them (`REQ-PRM-004`). A fill set not to be sewn sews
/// nothing, and its area is not built; another fill's area is built first, so what is left out of it is
/// said whatever its settings.
fn prepare<'a>(
    element: &'a Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Prepared<'a>>, Exhausted> {
    let set = &element.params;
    let region = match &element.shape {
        Shape::Fill { path, rule } => {
            let Some(true) = setting::<Toggle>(set, FillParams::SPECS, "fill", diagnostics) else { return Ok(None) };
            let built = region::build_clipped(path, *rule, &element.clips, meter)?;
            diagnostics.extend(built.diagnostics);
            built.region
        }
        Shape::Stroke { .. } => Region::default(),
    };
    let Some(stitch_type) = stitch_type(element, settings, diagnostics) else { return Ok(None) };
    // A value only the node's other stitch type reads is left to it, and this element takes its default.
    let left = left_to_others(set, stitch_type, PARAMETERS);
    let set: Cow<'a, ParamSet> = if left.is_empty() {
        Cow::Borrowed(set)
    } else {
        let mut own = set.clone();
        for (key, said) in left {
            own.remove(&key);
            diagnostics.push(said);
        }
        Cow::Owned(own)
    };
    let common = kept(CommonParams::from_set_as(&set, stitch_type), diagnostics);
    let shared = common.as_ref().map(|common| Shared {
        min_stitch: shortest_stitch(common.min_stitch_length_mm, settings, profile),
        max_stitch: common.max_stitch_length_mm,
        jump: common.jump_length(settings.collapse_len),
        force_locks: common.force_lock_stitches,
    });
    let own = element.thread.color;
    Ok(Some(Prepared { region, common, sewing: Sewing { set, stitch_type, shared, hints: element.hints.clone(), own } }))
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
    /// Its settings, but those it leaves to the node's other stitch type (`REQ-PRM-006`).
    set: Cow<'a, ParamSet>,
    /// The stitch type that sews it, which its settings are read as.
    stitch_type: StitchType,
    /// What the settings every stitch type shares give it; `None` when they cannot be read, which was
    /// said.
    shared: Option<Shared>,
    /// Where its design says it starts and ends, and the point it aims at (`REQ-GEN-001`).
    hints: Hints,
    /// Its own thread's colour, which a tartan fill sews its underlay in.
    own: Rgb,
}

/// A declaration's `from_set_as`, which reads its parameters as an element of a stitch type reads them.
type FromSetAs<T> = fn(&ParamSet, StitchType) -> Result<Validated<T>, Vec<Diagnostic>>;

impl Sewing<'_> {
    /// The parameters of one declaration, read as the element's stitch type reads them, keeping their
    /// warnings and errors in `diagnostics`; `None` when they cannot be read.
    fn read<T>(&self, from_set_as: FromSetAs<T>, diagnostics: &mut Vec<Diagnostic>) -> Option<T> {
        kept(from_set_as(&self.set, self.stitch_type), diagnostics)
    }
}

/// Whether a satin column is drawn too narrow to stitch across (`REQ-SAT-015`): as one subpath or none,
/// counting the subpaths drawn as Ink/Stitch counts them, with its stroke `width` no wider than the
/// design's `min_satin_stroke_width`. It is sewn as a stroke, as in Ink/Stitch.
fn too_narrow(path: &Path, width: Mm, settings: &DesignSettings) -> bool {
    path.subpaths.len() <= 1 && width.get() <= settings.min_satin_stroke_width.get()
}

/// A fill's stitches over its `region` as `sewing` says, between its `neighbours`, varied at random by its
/// `rng`, or `None` when it is skipped. Its `fill_method` picks the generator ([`FillMethod`]).
fn fill(
    sewing: &Sewing,
    neighbours: &Neighbours,
    region: &Region,
    rng: &mut SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    // Its start and end positions take the place of the needle and the next element's offer
    // (`REQ-GEN-001`). The contour fill, which ends where its rings end, uses the start alone.
    let end = sewing.hints.end.map(Approach::Point);
    let (needle, next) = (sewing.hints.start.or(neighbours.needle), end.as_ref().or(neighbours.next.as_ref()));
    let Some(method) = FillMethod::read(sewing, diagnostics) else { return Ok(None) };
    Ok(Some(method.sew(region, needle, next, rng, meter)?))
}

/// A fill's settings, read as its `fill_method` reads them: what its generator sews from. A tatami fill's
/// needle travels in the first of the running stitch's lengths, at least twice the shortest stitch, and
/// within its tolerance, between its top rows and between the rows of its underlay (`DEV-SAT-005`), and a
/// contour fill's between the rows of its underlay.
#[derive(Clone, Debug)]
enum FillMethod {
    Tatami(Fill),
    Contour(Contour),
    Meander(Meander),
    Circular(Circular),
    Cross(Cross),
    Tartan(Tartan),
}

impl FillMethod {
    /// The settings of the fill `sewing` describes; `None` when they cannot be read, or when its fill method
    /// is not sewn yet (`SC-W0011`), which `diagnostics` says.
    fn read(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<FillMethod> {
        match sewing.stitch_type {
            StitchType::ContourFill => contour_settings(sewing, diagnostics).map(FillMethod::Contour),
            StitchType::MeanderFill => meander_settings(sewing, diagnostics).map(FillMethod::Meander),
            StitchType::CircularFill => circular_settings(sewing, diagnostics).map(FillMethod::Circular),
            StitchType::CrossStitch => cross_settings(sewing, diagnostics).map(FillMethod::Cross),
            StitchType::TartanFill => tartan_settings(sewing, diagnostics).map(FillMethod::Tartan),
            _ => fill_settings(sewing, diagnostics).map(FillMethod::Tatami),
        }
    }

    /// The fill sewn over `region` by its generator, from the `needle` to near what `next` offers, its
    /// random variation drawn from `rng`.
    fn sew(
        &self,
        region: &Region,
        needle: Option<Point>,
        next: Option<&Approach>,
        rng: &mut SplitMix64,
        meter: &mut Meter,
    ) -> Result<Stitched, Exhausted> {
        match self {
            FillMethod::Contour(contour) => contour_fill(region, contour, needle, rng, meter),
            FillMethod::Meander(meander) => meander_fill(region, meander, needle, next, rng, meter),
            FillMethod::Circular(circular) => circular_fill(region, circular, needle, next, rng, meter),
            FillMethod::Cross(cross) => cross_stitch(region, cross, needle, next, meter),
            FillMethod::Tatami(fill) => tatami_fill(region, fill, needle, next, rng, meter),
            FillMethod::Tartan(tartan) => tartan_fill(region, tartan, needle, next, rng, meter),
        }
    }

    /// Its underlay's layers, the inset of the parts they are sewn over and the needle's travel: the tatami
    /// fill's underlay, which every fill method with one sews. `None` for a cross stitch, which has none.
    fn underlay(&self) -> Option<(&[FillLayer], f64, Travel)> {
        match self {
            FillMethod::Tatami(fill) => Some((&fill.underlay, fill.inset, fill.top.travel)),
            FillMethod::Contour(contour) => Some((&contour.underlay, contour.inset, contour.travel)),
            FillMethod::Meander(meander) => Some((&meander.underlay, meander.inset, meander.travel)),
            FillMethod::Circular(circular) => Some((&circular.underlay, circular.inset, circular.travel)),
            FillMethod::Tartan(tartan) => Some((&tartan.underlay, tartan.inset, tartan.travel)),
            FillMethod::Cross(_) => None,
        }
    }

    /// The same fill without its underlay: what its top sews alone.
    fn without_underlay(&self) -> FillMethod {
        let mut top = self.clone();
        match &mut top {
            FillMethod::Tatami(fill) => fill.underlay.clear(),
            FillMethod::Contour(contour) => contour.underlay.clear(),
            FillMethod::Meander(meander) => meander.underlay.clear(),
            FillMethod::Circular(circular) => circular.underlay.clear(),
            FillMethod::Tartan(tartan) => tartan.underlay.clear(),
            FillMethod::Cross(_) => {}
        }
        top
    }
}

/// The contour fill `sewing` describes, its settings read as a contour fill reads them: its rings the row
/// spacing apart, over each part grown by `expand_mm`, after the tatami fill's underlay. `None` when they
/// cannot be read, which `diagnostics` says. Its longest stitch is raised as a tatami fill's is.
fn contour_settings(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<Contour> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let contour = sewing.read(ContourParams::from_set_as, diagnostics);
    let path = sewing.read(PathFillParams::from_set_as, diagnostics);
    let (Some(mut tatami), Some(running), Some(contour), Some(path), Some(shared)) = (tatami, running, contour, path, sewing.shared) else {
        return None;
    };
    let (longest, travel) = fill_lengths(&mut tatami, &running, shared, diagnostics)?;
    let jitter = running.enable_random_stitch_length.then_some(running.random_stitch_length_jitter_percent / 100.0);
    Some(Contour {
        strategy: contour.strategy(),
        spacing: tatami.row_spacing_mm.get(),
        corners: contour.corners(),
        clockwise: contour.clockwise,
        avoid_self_crossing: contour.avoid_self_crossing,
        smoothness: path.smoothness_mm.get(),
        stitching: Stitching {
            length: longest.get(),
            tolerance: running.running_stitch_tolerance_mm.get(),
            jitter,
            beans: path.bean_stitch_repeats,
            repeats: path.repeats,
        },
        underlay: tatami.underlay(longest, travel),
        inset: tatami.fill_underlay_inset_mm.get(),
        expand: tatami.expand_mm.get(),
        force_locks: shared.force_locks,
        travel,
    })
}

/// The meander fill `sewing` describes, its settings read as a meander fill reads them: its tile laid over
/// each part grown by `expand_mm`, after the tatami fill's underlay, its path sewn in the running stitch's
/// length and tolerance. `None` when they cannot be read, which `diagnostics` says, and `SC-W0316` when its
/// pattern is one of Ink/Stitch's tiles, for which it takes the default. Its longest stitch, its underlay's,
/// is raised as a tatami fill's is.
fn meander_settings(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<Meander> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let meander = sewing.read(MeanderParams::from_set_as, diagnostics);
    let path = sewing.read(PathFillParams::from_set_as, diagnostics);
    let (Some(mut tatami), Some(running), Some(meander), Some(path), Some(shared)) = (tatami, running, meander, path, sewing.shared) else {
        return None;
    };
    let (longest, travel) = fill_lengths(&mut tatami, &running, shared, diagnostics)?;
    let (tile, theirs) = meander.tile();
    if theirs {
        let message = format!(
            "This meander fill's pattern, `{}`, is one of Ink/Stitch's tiles, which StitchCraft doesn't have, so it wanders through the {tile} \
             tile instead.",
            meander.meander_pattern
        );
        diagnostics.push(Diagnostic::new(Code::FillMeanderTileNotOurs, message));
    }
    Some(Meander {
        tile,
        placement: meander.placement(),
        smoothness: path.smoothness_mm.get(),
        zigzag: meander.zigzag(),
        clip: meander.clip,
        beans: path.bean_stitch_repeats,
        repeats: path.repeats,
        underlay: tatami.underlay(longest, travel),
        inset: tatami.fill_underlay_inset_mm.get(),
        expand: tatami.expand_mm.get(),
        force_locks: shared.force_locks,
        travel,
    })
}

/// The circular fill `sewing` describes, its settings read as a circular fill reads them: its circles the row
/// spacing apart, or fading to the end spacing, over each part grown by `expand_mm`, after the tatami fill's
/// underlay, its spiral sewn and its needle travelling in the running stitch's length and tolerance, routed
/// as the tatami fill's rows are. `None` when they cannot be read, which `diagnostics` says. Its longest
/// stitch, its underlay's, and its running stitch's length are raised as a tatami fill's are.
fn circular_settings(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<Circular> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let path = sewing.read(PathFillParams::from_set_as, diagnostics);
    let (Some(mut tatami), Some(running), Some(path), Some(shared)) = (tatami, running, path, sewing.shared) else { return None };
    let (longest, travel) = fill_lengths(&mut tatami, &running, shared, diagnostics)?;
    let jitter = running.enable_random_stitch_length.then_some(running.random_stitch_length_jitter_percent / 100.0);
    Some(Circular {
        spacing: tatami.row_spacing_mm.get(),
        end_spacing: tatami.end_row_spacing_mm.map(Mm::get),
        stitching: Stitching { length: travel.length, tolerance: travel.tolerance, jitter, beans: path.bean_stitch_repeats, repeats: path.repeats },
        skip_last: tatami.skip_last,
        angle: tatami.angle,
        underlay: tatami.underlay(longest, travel),
        inset: tatami.fill_underlay_inset_mm.get(),
        expand: tatami.expand_mm.get(),
        force_locks: shared.force_locks,
        travel,
        target: sewing.hints.target,
    })
}

/// The cross stitch `sewing` describes, its settings read as a cross stitch reads them: its own, and the
/// `expand_mm` it shares with the tatami fill, with the element's shortest stitch. `None` when they cannot be
/// read, which `diagnostics` says.
fn cross_settings(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<Cross> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let cross = sewing.read(CrossParams::from_set_as, diagnostics);
    let (Some(tatami), Some(cross), Some(shared)) = (tatami, cross, sewing.shared) else { return None };
    Some(cross.cross(tatami.expand_mm.get(), shared.min_stitch, diagnostics))
}

/// The tartan fill `sewing` describes, its settings read as a tartan fill reads them: its own, the tatami
/// fill's rows, `skip_last`, expand and underlay, the running stitch's length and tolerance for its travel,
/// and the bean stitch of its runs. `None` when they cannot be read, or when its sett can't be, which
/// `diagnostics` says. Its longest stitch, its underlay's, and its running stitch's length are raised as a
/// tatami fill's are. Its needle travels along the outline, never under its rows, as Ink/Stitch's does.
fn tartan_settings(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<Tartan> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let path = sewing.read(PathFillParams::from_set_as, diagnostics);
    let own = sewing.read(TartanParams::from_set_as, diagnostics);
    let (Some(mut tatami), Some(running), Some(path), Some(own), Some(shared)) = (tatami, running, path, own, sewing.shared) else {
        return None;
    };
    let (longest, travel) = fill_lengths(&mut tatami, &running, shared, diagnostics)?;
    let given = tartan::Given {
        spacing: tatami.row_spacing_mm.get(),
        staggers: tatami.staggers,
        skip_last: tatami.skip_last,
        underlay: tatami.underlay(longest, travel),
        inset: tatami.fill_underlay_inset_mm.get(),
        expand: tatami.expand_mm.get(),
        longest,
        travel,
        beans: &path.bean_stitch_repeats,
        min_stitch: shared.min_stitch,
        own: sewing.own,
    };
    own.tartan(given, diagnostics)
}

/// The tatami fill `sewing` describes, its settings read as its stitch type reads them; `None` when they
/// cannot be read, or when its fill method is not sewn yet (`SC-W0011`), which `diagnostics` says. A fill
/// set to Ink/Stitch's legacy fill takes what Ink/Stitch's does: no underlay, pull compensation, random
/// stitch lengths or gap-filling rows, whatever its settings say (`REQ-FILL-005`). A longest stitch, the
/// rows' or the underlay's, shorter than twice the shortest stitch is raised to it (`SC-W0402`).
fn fill_settings(sewing: &Sewing, diagnostics: &mut Vec<Diagnostic>) -> Option<Fill> {
    let tatami = sewing.read(TatamiParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let pull = sewing.read(PullParams::from_set_as, diagnostics);
    let (Some(mut tatami), Some(mut running), Some(pull), Some(shared)) = (tatami, running, pull, sewing.shared) else { return None };
    let legacy = legacy(&sewing.set);
    if legacy {
        tatami.fill_underlay = false;
        tatami.gap_fill_rows = 0;
        running.enable_random_stitch_length = false;
    }
    if sewing.stitch_type != StitchType::TatamiFill {
        let method = sewing.stitch_type.id();
        diagnostics.push(not_yet(&format!("This element's fill method, `{method}`, is not sewn by this version of StitchCraft yet")));
        return None;
    }
    let (longest, travel) = fill_lengths(&mut tatami, &running, shared, diagnostics)?;
    let pull = if legacy { Pull::default() } else { fill_pull(&pull, diagnostics) };
    Some(tatami.fill(longest, &running, travel, pull, shared.force_locks))
}

/// A fill's longest stitch, and its underlay's, raised to twice the shortest stitch with `SC-W0402` where
/// shorter, and how its needle travels between rows: in the first of the running stitch's lengths, raised
/// so too, within its tolerance. `None`, saying so, should the longest stitch or the length be missing,
/// which the registry never leaves them.
fn fill_lengths(tatami: &mut TatamiParams, running: &RunningParams, shared: Shared, diagnostics: &mut Vec<Diagnostic>) -> Option<(Mm, Travel)> {
    // A fill's longest stitch has a default of its own, so it is never empty; the registry reads it so.
    let (Some(longest), Some(&length)) = (shared.max_stitch, running.running_stitch_length_mm.first()) else {
        diagnostics.push(Diagnostic::new(Code::InternalCheckFailed, "A fill's longest stitch or travel stitch length is missing."));
        return None;
    };
    let min = shared.min_stitch.get();
    // Rows stitched shorter than twice the shortest stitch sew stitches the plan merges, as a running stitch
    // would. Raised, each is a length, as twice a shortest stitch is.
    let longest = Mm::new(raised("max_stitch_length_mm", longest, min, diagnostics)).unwrap_or(longest);
    if tatami.fill_underlay
        && let Some(underlay) = tatami.fill_underlay_max_stitch_length_mm
    {
        tatami.fill_underlay_max_stitch_length_mm = Mm::new(raised("fill_underlay_max_stitch_length_mm", underlay, min, diagnostics)).ok();
    }
    let travel = Travel {
        length: raised("running_stitch_length_mm", length, min, diagnostics),
        tolerance: running.running_stitch_tolerance_mm.get(),
        min_stitch: min,
        underpath: tatami.underpath,
        jump: shared.jump.get(),
    };
    Some((longest, travel))
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

/// A satin column's stitches as `sewing` says, between its `neighbours`, varied at random by its `rng`, or
/// `None` when it is skipped: its path is `width` wide with `join` at its corners when it is its centre
/// line ([`SatinRead`]).
fn satin_column(
    sewing: &Sewing,
    neighbours: &Neighbours,
    shape: (&Path, Mm, Join),
    rng: &mut SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    let Some(SatinRead { rails, params, pull, lengths }) = SatinRead::read(sewing, shape, diagnostics, meter)? else { return Ok(None) };
    Ok(Some(satin_stitch(&rails, &params, &pull, lengths, neighbours, &sewing.hints, rng, meter)?))
}

/// A satin column's settings, read as its `satin_method` reads them, and its rails: what its generator sews
/// from. Its travel between underlays, and its way to its start and end, take the first of the running
/// stitch's lengths and its tolerance.
struct SatinRead {
    rails: Satin,
    params: SatinParams,
    pull: PullParams,
    lengths: SatinLengths,
}

impl SatinRead {
    /// The satin column `sewing` describes, its path `width` wide with `join` at its corners when it is its
    /// centre line; `None` when it is skipped, which `diagnostics` says.
    fn read(
        sewing: &Sewing,
        (path, width, join): (&Path, Mm, Join),
        diagnostics: &mut Vec<Diagnostic>,
        meter: &mut Meter,
    ) -> Result<Option<SatinRead>, Exhausted> {
        let Some(params) = sewing.read(SatinParams::from_set_as, diagnostics) else { return Ok(None) };
        let Some(rails) = satin::rails(path, width, join, diagnostics, meter)? else { return Ok(None) };
        let running = sewing.read(RunningParams::from_set_as, diagnostics);
        let pull = sewing.read(PullParams::from_set_as, diagnostics);
        let (Some(Shared { min_stitch, max_stitch, jump, .. }), Some(running), Some(pull)) = (sewing.shared, running, pull) else { return Ok(None) };
        // The registry reads a list of 1 length or more.
        let Some(&travel) = running.running_stitch_length_mm.first() else { return Ok(None) };
        let lengths = SatinLengths { min_stitch, max_stitch, travel, tolerance: running.running_stitch_tolerance_mm, jump };
        Ok(Some(SatinRead { rails, params, pull, lengths }))
    }
}

/// The line a stroke sewn as `stitch_type` is cut to its clips along (`REQ-GEN-005`): a manual stitch's
/// nodes, as it sews them, and another stroke's path flattened as a shape's outline is, as Ink/Stitch cuts
/// them.
pub(crate) fn cut_line(stitch_type: StitchType) -> cut::Line {
    if stitch_type == StitchType::ManualStitch { manual::nodes } else { stroke::flattened }
}

/// A stroke's stitches as `sewing` says, varied at random by streams of its `rng`, or `None` when it is
/// skipped: its `path` is `width` wide, which a zigzag stroke is. Its `stroke_method` picks the generator,
/// a running stitch for every method but manual stitch and zigzag stitch: [`sew`] sends a ripple stitch to
/// [`ripple`].
fn stroke(
    sewing: &Sewing,
    path: &Path,
    width: Mm,
    rng: &SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    let passes = sewing.read(RepeatParams::from_set_as, diagnostics);
    let Some(Shared { min_stitch, max_stitch, .. }) = sewing.shared else { return Ok(None) };
    match sewing.stitch_type {
        StitchType::ManualStitch => {
            let Some(passes) = passes else { return Ok(None) };
            Ok(Some(manual_stitch(path, max_stitch, &passes, min_stitch, meter)?))
        }
        StitchType::ZigzagStitch => {
            let (Some(zigzag), Some(passes)) = (sewing.read(ZigzagStrokeParams::from_set_as, diagnostics), passes) else { return Ok(None) };
            Ok(Some(zigzag_stroke(path, width, &zigzag, &passes, min_stitch, meter)?))
        }
        _ => {
            let (Some(running), Some(passes)) = (sewing.read(RunningParams::from_set_as, diagnostics), passes) else { return Ok(None) };
            Ok(Some(running_stitch(path, &running, &passes, min_stitch, rng, meter)?))
        }
    }
}

/// A ripple stitch's stitches along `path` as `sewing` says, cut to `clips`, its random lengths drawn from
/// streams of its `rng`, or `None` when it is skipped. It aims at its design's target point, and follows its
/// guide and anchor lines ([`ripple::guide`]). A satin column's settings are read as a ripple reads them, at
/// their defaults but for the ripple's own `reverse_rails` and `swap_satin_rails`, and
/// [`ripple::unapplied`] names those the element sets that a ripple doesn't apply.
fn ripple(
    sewing: &Sewing,
    path: &Path,
    clips: &[Clip],
    rng: &SplitMix64,
    diagnostics: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Stitched>, Exhausted> {
    let own = sewing.read(RippleParams::from_set_as, diagnostics);
    let running = sewing.read(RunningParams::from_set_as, diagnostics);
    let passes = sewing.read(RepeatParams::from_set_as, diagnostics);
    let satin = sewing.read(SatinParams::from_set_as, diagnostics);
    let (Some(own), Some(running), Some(passes), Some(satin), Some(shared)) = (own, running, passes, satin, sewing.shared) else { return Ok(None) };
    let satin = SatinParams { reverse_rails: own.reverse_rails, swap_satin_rails: own.swap_satin_rails, ..satin };
    let guide = sewing.hints.guide.as_ref().and_then(|guide| ripple::guide(guide, &own, diagnostics));
    let ripple = Ripple {
        unapplied: ripple::unapplied(&sewing.set),
        ripple: own,
        running,
        passes,
        satin,
        min_stitch: shared.min_stitch,
        jump: shared.jump,
        target: sewing.hints.target,
        guide,
        anchor: sewing.hints.anchor,
        clips: clips.to_vec(),
    };
    Ok(Some(ripple_stitch(path, &ripple, rng, meter)?))
}

/// What `element` offers the element before it to end near, read from its shape, its settings and the
/// design's `settings` alone, with the budget's work to itself (`REQ-GEN-003`): a stroke its first point,
/// a satin column its rails as they are sewn when it starts at its nearest point and otherwise its first
/// rail's start. A satin column too narrow to stitch across is a stroke (`REQ-SAT-015`). A tatami fill, a
/// fill set to Ink/Stitch's legacy fill, which is sewn as one (`REQ-FILL-005`), and a contour, meander,
/// circular, tartan or cross stitch fill start at their point nearest the needle and offer their area, their parts,
/// even when set not to be sewn, as Ink/Stitch's next element is the next in the drawing. Where its stitch
/// type is known, an
/// element offers what Ink/Stitch's would, even of a method StitchCraft does not sew yet (ADR 0014): a
/// stroke of any method its first point, as Ink/Stitch's strokes all do, and a satin column of any method
/// its rails. A fill of a method StitchCraft does not sew offers nothing, since what it would offer is not
/// known. Neither does an element whose shape cannot be read, or the settings its offer depends on: those
/// that choose its stitch type, and a satin column's. They are read as its stitch type reads them. A fill
/// of any method, or a satin column, with a start position offers that, where it starts (`REQ-GEN-001`). A
/// fill cut to its clips offers what they leave of its area, a stroke the first point of the first piece
/// they leave, and either offers nothing when they leave nothing; a satin column is sewn whole
/// (`REQ-GEN-005`).
pub fn approach(element: &Element, settings: &DesignSettings, budget: &Budget) -> Option<Approach> {
    let stitch_type = stitch_type_of(element, settings)?;
    if let Some(start) = element.hints.start
        && (matches!(element.shape, Shape::Fill { .. }) || stitch_type.family() == Family::Satin)
    {
        return Some(Approach::Point(start));
    }
    let (path, width, join) = match &element.shape {
        Shape::Fill { .. }
            if !matches!(
                stitch_type,
                StitchType::TatamiFill
                    | StitchType::ContourFill
                    | StitchType::MeanderFill
                    | StitchType::CircularFill
                    | StitchType::CrossStitch
                    | StitchType::TartanFill
            ) =>
        {
            return None;
        }
        Shape::Fill { path, rule } => {
            let parts = region::build_clipped(path, *rule, &element.clips, &mut budget.meter()).ok()?.region.parts;
            return (!parts.is_empty()).then_some(Approach::Area(parts));
        }
        Shape::Stroke { path, .. } if stitch_type.family() != Family::Satin => {
            let path = cut::stroke(path, &element.clips, cut_line(stitch_type), &mut Vec::new(), &mut budget.meter()).ok()??;
            return path.subpaths.first().map(|subpath| Approach::Point(subpath.start));
        }
        Shape::Stroke { path, width, join, .. } => (path, width, join),
    };
    let params = SatinParams::from_set_as(&element.params, stitch_type).ok()?.params;
    let meter = &mut budget.meter();
    let rails = satin::rails(path, *width, *join, &mut Vec::new(), meter).ok()??;
    if params.start_at_nearest_point {
        return Some(Approach::Shape(satin::sewn_rails(&rails, &params, meter).ok()?.into()));
    }
    satin::first_point(&rails, &params, meter).ok()?.map(Approach::Point)
}

/// The stitch type that sews `element` in a design with `settings`, as [`generate`] picks it. `None` if a
/// setting that chooses it cannot be read.
pub fn stitch_type_of(element: &Element, settings: &DesignSettings) -> Option<StitchType> {
    stitch_type(element, settings, &mut Vec::new())
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
    use crate::design::{Cap, FillRule, Segment, Subpath};

    fn element(shape: Shape, params: &[(&str, &str)]) -> Element {
        let thread = Thread::new(Rgb::new(0, 0, 0));
        Element {
            id: ElementId::new("e").unwrap(),
            name: None,
            shape,
            thread,
            params: params.iter().copied().collect(),
            automatic: false,
            hints: Default::default(),
            clips: Vec::new(),
        }
    }

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<Code> {
        diagnostics.iter().map(|d| d.code).collect()
    }

    #[test]
    fn diag_sc_w0402_a_fill_s_longest_stitch_below_twice_the_shortest_is_raised() {
        // A 10 mm square on the reference machine, whose shortest stitch is 0.3 mm: rows stitched 0.2 mm
        // long would sew stitches it merges. Its longest stitch is raised to 0.6 mm, and its underlay's
        // with it, as a running stitch's length is.
        let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
        let square = Shape::Fill { path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] }, rule: FillRule::NonZero };
        let sewn = |params: &[(&str, &str)]| {
            generate(
                &element(square.clone(), params),
                &DesignSettings::default(),
                stitchcraft_plan::profiles::REFERENCE,
                &Neighbours::default(),
                &Budget::DEFAULT,
            )
        };
        let raised = sewn(&[("max_stitch_length_mm", "0.2"), ("fill_underlay", "false")]);
        let said: Vec<String> = raised.diagnostics.iter().map(ToString::to_string).collect();
        assert_eq!(
            said,
            ["warning SC-W0402: The stitch length 0.2 mm (`max_stitch_length_mm`) is shorter than twice the shortest stitch (0.3 mm), so 0.6 mm \
              is used."]
        );
        let points: Vec<Point> = raised.generated.unwrap().groups.concat();
        let along: Vec<f64> = points.windows(2).filter(|w| (w[0].y() - w[1].y()).abs() < 1e-9).map(|w| w[0].distance(w[1])).collect();
        // Of the stitches along the rows, all but their ends' and the needle's way in are 0.6 mm long.
        let raised_ones = along.iter().filter(|length| (*length - 0.6).abs() < 1e-6).count();
        assert!(raised_ones * 10 > along.len() * 9, "{raised_ones} of {}: {along:?}", along.len());
        let underlay = sewn(&[("fill_underlay_max_stitch_length_mm", "0.25")]);
        assert_eq!(
            underlay.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>(),
            ["warning SC-W0402: The stitch length 0.25 mm (`fill_underlay_max_stitch_length_mm`) is shorter than twice the shortest stitch \
              (0.3 mm), so 0.6 mm is used."]
        );
        // Without underlay, its longest stitch says nothing; nor does a longest stitch of 3 mm. A legacy fill
        // has no underlay whatever its settings say (`REQ-FILL-005`): it says only that it is sewn so.
        assert!(sewn(&[("fill_underlay", "false"), ("fill_underlay_max_stitch_length_mm", "0.25")]).diagnostics.is_empty());
        let legacy = sewn(&[("fill_method", "legacy_fill"), ("fill_underlay_max_stitch_length_mm", "0.25")]);
        assert_eq!(codes(&legacy.diagnostics), [Code::FillLegacyAsTatami]);
        assert!(sewn(&[("max_stitch_length_mm", "3")]).diagnostics.is_empty());
    }

    #[test]
    fn a_fill_s_top_rows_are_laid_as_its_top_stitches_are_without_sewing_it() {
        let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
        let square = Shape::Fill { path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] }, rule: FillRule::NonZero };
        let laid = |shape: &Shape, params: &[(&str, &str)], budget: &Budget| {
            rows(&element(shape.clone(), params), &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, budget)
        };
        // Rows a quarter of a millimetre apart, across the 10 mm square, each one segment from side to side.
        let across =
            |rows: &Rows| rows.segments.as_ref().map(|segments| (segments.len(), segments.iter().map(|s| s.start.distance(s.end)).sum::<f64>()));
        let plain = laid(&square, &[], &Budget::DEFAULT);
        assert_eq!((across(&plain), plain.diagnostics), (Some((40, 400.0)), vec![]));
        // Grown by `expand_mm` as the top stitches are, and the generator's own rows: every top needle point
        // of the fill without its underlay lies on one of them.
        let grown = laid(&square, &[("expand_mm", "1")], &Budget::DEFAULT);
        assert_eq!(across(&grown).map(|(count, _)| count), Some(48));
        let sewn = generate(
            &element(square.clone(), &[("fill_underlay", "false")]),
            &DesignSettings::default(),
            stitchcraft_plan::profiles::REFERENCE,
            &Neighbours::default(),
            &Budget::DEFAULT,
        );
        let tops = plain.segments.unwrap_or_default();
        let on_a_row = |q: &Point| tops.iter().any(|s| (q.y() - s.start.y()).abs() < 1e-9 && q.x() >= -1e-9 && q.x() <= 10.0 + 1e-9);
        let points: Vec<Point> = sewn.generated.unwrap().groups.into_iter().flatten().collect();
        assert!(points.iter().filter(|q| on_a_row(q)).count() * 10 > points.len() * 9, "most needle points lie on the rows");
        // A contour fill has no rows: its path round its rings, 1 mm apart here, squares 10, 8, 6, 4 and 2 mm
        // across, each sewn round to 1 mm short of where it was entered, the outline from its corner at the
        // origin, and the steps between them.
        let contour = laid(&square, &[("fill_method", "contour_fill"), ("row_spacing_mm", "1")], &Budget::DEFAULT);
        let path = contour.segments.unwrap_or_default();
        let long: f64 = path.iter().map(|s| s.start.distance(s.end)).sum();
        assert!((115.0..130.0).contains(&long) && contour.diagnostics.is_empty(), "{long} mm: {:?}", contour.diagnostics);
        assert_eq!(path.first().map(|s| s.start), Some(p(0.0, 0.0)));
        assert!(path.iter().all(|s| [s.start, s.end].iter().all(|q| (0.0..=10.0).contains(&q.x()) && (0.0..=10.0).contains(&q.y()))));
        assert!(path.windows(2).all(|w| w[0].end == w[1].start), "one path");
        // Nor has a meander fill: its path through its tile, one path in the square, as the element's seed
        // lays it, so that another seed lays another over a square 20 mm a side.
        let segments = [p(20.0, 0.0), p(20.0, 20.0), p(0.0, 20.0)].map(Segment::Line).to_vec();
        let large = Shape::Fill { path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] }, rule: FillRule::NonZero };
        let meander = |seed: &str| laid(&large, &[("fill_method", "meander_fill"), ("random_seed", seed)], &Budget::DEFAULT);
        let wandered = meander("1");
        let path = wandered.segments.clone().unwrap_or_default();
        assert!(path.len() > 50 && wandered.diagnostics.is_empty(), "{} segments: {:?}", path.len(), wandered.diagnostics);
        assert!(path.windows(2).all(|w| w[0].end == w[1].start), "one path");
        assert!(path.iter().all(|s| [s.start, s.end].iter().all(|q| (0.0..=20.0).contains(&q.x()) && (0.0..=20.0).contains(&q.y()))));
        assert_ne!(meander("2").segments, wandered.segments);
        // Not a fill, a fill not sewn, a method not sewn yet, and a budget spent: no rows, and the same words
        // as generating it.
        let line = Shape::stroke(Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] });
        assert_eq!(laid(&line, &[], &Budget::DEFAULT), Rows { segments: None, diagnostics: vec![] });
        assert_eq!(laid(&square, &[("fill", "false")], &Budget::DEFAULT), Rows { segments: None, diagnostics: vec![] });
        let guided = laid(&square, &[("fill_method", "guided_fill")], &Budget::DEFAULT);
        assert_eq!((guided.segments, codes(&guided.diagnostics)), (None, vec![Code::StitchTypeNotYet]));
        // A tartan fill shows the lines of its colours, without a sett Ink/Stitch's default one's, which it
        // says, and nothing when its sett can't be read.
        let tartan = laid(&square, &[("fill_method", "tartan_fill")], &Budget::DEFAULT);
        assert!(tartan.segments.is_some_and(|segments| !segments.is_empty()));
        assert_eq!(codes(&tartan.diagnostics), [Code::FillTartanNoSett]);
        let unread = laid(&square, &[("fill_method", "tartan_fill"), ("tartan", "{")], &Budget::DEFAULT);
        assert_eq!((unread.segments, codes(&unread.diagnostics)), (None, vec![Code::FillTartanSettUnreadable]));
        let spent = laid(&square, &[], &Budget { max_stitches: 1, max_work: 10 });
        assert_eq!((spent.segments, codes(&spent.diagnostics)), (None, vec![Code::BudgetExhausted]));
        assert!(spent.diagnostics.iter().all(|d| d.element.as_ref().map(ElementId::as_str) == Some("e")));
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
            let sewing = Sewing { set: Cow::Borrowed(set), stitch_type, shared, hints: Hints::default(), own: Rgb::new(0, 0, 0) };
            fill(&sewing, &neighbours, &region, rng, diagnostics, meter).unwrap()
        };
        let mut diagnostics = Vec::new();
        let sewn = sew(&ParamSet::new(), StitchType::TatamiFill, shared(Mm::new(4.0).ok()), &mut diagnostics);
        assert_eq!((sewn.is_some(), codes(&diagnostics)), (true, vec![]));
        // A contour fill and a meander fill, each by its own generator.
        for stitch_type in [StitchType::ContourFill, StitchType::MeanderFill] {
            let sewn = sew(&ParamSet::new(), stitch_type, shared(Mm::new(4.0).ok()), &mut diagnostics);
            assert_eq!((sewn.is_some_and(|s| !s.runs.is_empty()), codes(&diagnostics)), (true, vec![]), "{stitch_type:?}");
        }
        // A tartan fill by its own, in a thread for each run, here Ink/Stitch's default sett, which it says.
        let mut said = Vec::new();
        let sewn = sew(&ParamSet::new(), StitchType::TartanFill, shared(Mm::new(4.0).ok()), &mut said);
        assert_eq!((sewn.is_some_and(|s| !s.runs.is_empty() && s.runs.len() == s.colours.len()), codes(&said)), (true, vec![Code::FillTartanNoSett]));
        // Without a longest stitch, the same bug as a tatami fill's, and settings that can't be read skip it.
        let mut said = Vec::new();
        assert!(sew(&ParamSet::new(), StitchType::TartanFill, shared(None), &mut said).is_none());
        assert_eq!(codes(&said), [Code::InternalCheckFailed]);
        let wrong = [("rows_per_thread", "many")].into_iter().collect();
        let mut said = Vec::new();
        assert!(sew(&wrong, StitchType::TartanFill, shared(Mm::new(4.0).ok()), &mut said).is_none());
        assert_eq!(codes(&said), [Code::ParamInvalid]);
        // A method not sewn yet, whose settings it does not read, wrong as they are here.
        let tatami = [("row_spacing_mm", "wide")].into_iter().collect();
        let sewn = sew(&tatami, StitchType::GuidedFill, shared(Mm::new(4.0).ok()), &mut diagnostics);
        assert_eq!((sewn.is_none(), codes(&diagnostics)), (true, vec![Code::StitchTypeNotYet]));
        // A contour fill without a longest stitch is the same bug as a tatami fill's.
        let mut diagnostics = Vec::new();
        assert!(sew(&ParamSet::new(), StitchType::ContourFill, shared(None), &mut diagnostics).is_none());
        assert_eq!(codes(&diagnostics), [Code::InternalCheckFailed]);
        // The registry gives a fill a longest stitch of its own (`ParamSpec::in_family`), so only a bug in
        // StitchCraft can leave it out.
        let mut diagnostics = Vec::new();
        let sewn = sew(&ParamSet::new(), StitchType::TatamiFill, shared(None), &mut diagnostics);
        assert_eq!((sewn.is_none(), codes(&diagnostics)), (true, vec![Code::InternalCheckFailed]));
    }

    #[test]
    fn a_tartan_s_threads_are_read_from_its_settings_and_every_other_element_s_is_its_own() {
        let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
        let square = Shape::Fill { path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] }, rule: FillRule::NonZero };
        let (black, red, blue) = (Rgb::new(0, 0, 0), Rgb::new(255, 0, 0), Rgb::new(0, 0, 255));
        assert_eq!(colours(&element(square.clone(), &[])), [black]);
        let line = Shape::stroke(Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] });
        assert_eq!(colours(&element(line, &[("fill_method", "tartan_fill")])), [black]);
        // Its underlay in its own thread, then its sett's colours; without underlay, the colours alone.
        let sett = r#"{"palette": "(#ff0000)/5 (#0000ff)/5"}"#;
        assert_eq!(colours(&element(square.clone(), &[("fill_method", "tartan_fill"), ("tartan", sett)])), [black, red, blue]);
        let bare = [("fill_method", "tartan_fill"), ("tartan", sett), ("fill_underlay", "false")];
        assert_eq!(colours(&element(square.clone(), &bare)), [red, blue]);
        // An underlay setting that can't be read is read as on, as the fill's default.
        let wrong = [("fill_method", "tartan_fill"), ("tartan", sett), ("fill_underlay", "maybe")];
        assert_eq!(colours(&element(square, &wrong)), [black, red, blue]);
    }

    #[test]
    fn each_fill_method_with_an_underlay_gives_it_and_sews_its_top_without_it() {
        // The underlay a fill in a run of underlays first sews apart, and the fill without it that sews its top.
        let shared =
            Some(Shared { min_stitch: Mm::new(0.3).unwrap(), max_stitch: Mm::new(4.0).ok(), jump: Mm::new(3.0).unwrap(), force_locks: false });
        let methods = [
            (StitchType::TatamiFill, true),
            (StitchType::ContourFill, true),
            (StitchType::MeanderFill, true),
            (StitchType::CircularFill, true),
            (StitchType::TartanFill, true),
            (StitchType::CrossStitch, false),
        ];
        for (stitch_type, underlaid) in methods {
            let set = ParamSet::new();
            let sewing = Sewing { set: Cow::Borrowed(&set), stitch_type, shared, hints: Hints::default(), own: Rgb::new(0, 0, 0) };
            let method = FillMethod::read(&sewing, &mut Vec::new()).unwrap();
            let layers = |method: &FillMethod| method.underlay().map(|(layers, inset, travel)| (layers.len(), inset, travel.min_stitch));
            assert_eq!(layers(&method), underlaid.then_some((1, 0.0, 0.3)), "{stitch_type:?}");
            assert_eq!(layers(&method.without_underlay()), underlaid.then_some((0, 0.0, 0.3)), "{stitch_type:?}");
        }
    }

    #[test]
    fn the_settings_that_choose_it_give_an_element_s_stitch_type() {
        let path = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] };
        let fill = || Shape::Fill { path: path.clone(), rule: FillRule::NonZero };
        let wide = || Shape::Stroke { path: path.clone(), width: Mm::new(3.0).unwrap(), join: Join::UNSET, cap: Cap::Butt };
        let chosen = |shape: Shape, params: &[(&str, &str)]| {
            let mut diagnostics = Vec::new();
            (stitch_type(&element(shape, params), &DesignSettings::default(), &mut diagnostics), codes(&diagnostics))
        };
        assert_eq!(chosen(fill(), &[]), (Some(StitchType::TatamiFill), vec![]));
        assert_eq!(chosen(fill(), &[("fill_method", "contour_fill")]), (Some(StitchType::ContourFill), vec![]));
        // Ink/Stitch's legacy fill is sewn as a tatami fill.
        assert_eq!(chosen(fill(), &[("fill_method", "legacy_fill")]), (Some(StitchType::TatamiFill), vec![Code::FillLegacyAsTatami]));
        assert!(legacy(&element(fill(), &[("fill_method", "legacy_fill")]).params) && !legacy(&element(fill(), &[]).params));
        assert!(StitchType::ALL.iter().all(|t| sewn_as(*t) == *t || *t == StitchType::LegacyFill));
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

    #[test]
    fn a_circular_fill_s_preview_is_the_pieces_of_its_spiral() {
        // A circular fill has no rows either: a preview that can't afford its stitches shows the pieces of its
        // spiral, circles 1 mm apart round the middle of the 10 mm square, cut by its sides.
        let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
        let square = Shape::Fill { path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] }, rule: FillRule::NonZero };
        let params = [("fill_method", "circular_fill"), ("row_spacing_mm", "1")];
        let laid = rows(&element(square, &params), &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, &Budget::DEFAULT);
        assert_eq!(laid.diagnostics, Vec::new());
        let path = laid.segments.unwrap_or_default();
        let inside = |q: Point| (-1e-9..=10.0 + 1e-9).contains(&q.x()) && (-1e-9..=10.0 + 1e-9).contains(&q.y());
        assert!(path.iter().all(|s| inside(s.start) && inside(s.end)), "{path:?}");
        // About as long as the circles inside the square: those 1 to 5 mm round its middle whole, 94 mm, and
        // what its sides leave of those 6 and 7 mm round, 10 mm.
        let long: f64 = path.iter().map(|s| s.start.distance(s.end)).sum();
        assert!((95.0..110.0).contains(&long), "{long} mm");
    }

    #[test]
    fn a_cross_stitch_fill_s_preview_is_its_crosses_diagonals() {
        // A cross stitch fill has no rows: its preview shows the 2 diagonals of each cross. A 4 mm square on
        // the canvas's 2 mm grid covers 4 cells whole, and each sews a cross.
        let segments = [p(4.0, 0.0), p(4.0, 4.0), p(0.0, 4.0)].map(Segment::Line).to_vec();
        let square = Shape::Fill { path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] }, rule: FillRule::NonZero };
        let params = [("fill_method", "cross_stitch"), ("pattern_size_mm", "2")];
        let laid = rows(&element(square, &params), &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, &Budget::DEFAULT);
        assert_eq!(laid.diagnostics, Vec::new());
        let mut diagonals: Vec<[f64; 4]> = laid.segments.unwrap().iter().map(|s| [s.start.x(), s.start.y(), s.end.x(), s.end.y()]).collect();
        diagonals.sort_by(|a, b| a.iter().zip(b).map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal));
        let cell = |x: f64, y: f64| [[x, y, x + 2.0, y + 2.0], [x, y + 2.0, x + 2.0, y]];
        let mut expected: Vec<[f64; 4]> = [cell(0.0, 0.0), cell(2.0, 0.0), cell(0.0, 2.0), cell(2.0, 2.0)].concat();
        expected.sort_by(|a, b| a.iter().zip(b).map(|(x, y)| x.total_cmp(y)).find(|o| o.is_ne()).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(diagonals, expected);
    }
}
