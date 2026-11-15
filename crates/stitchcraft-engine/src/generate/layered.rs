//! An element of a run of underlays first, read once and sewn as 2 layers (`docs/src/design/engine-pipeline.md`
//! › Underlays first, ADR-0018).
//!
//! **Why.** A run sews every element's underlay before any top stitch, so each element of it is sewn in 2
//! places, each layer from the needle before it to near what the next layer offers. Its settings are read
//! and its shape built once, before its underlay, and its top is sewn later from the same reading
//! ([`Layered`]). Both layers share its work budget, and each spends the design's stitches on the needle
//! points it makes. A satin column places its top stitches as it is read, and a fill draws its top's random
//! numbers from its own generator, which its underlay leaves as it was. So the top stitches take the random
//! numbers they take outside a run.
//!
//! **Which elements.** A satin column with an underlay on, and a tatami, contour, meander, circular or
//! tartan fill whose underlay is on, are sewn in 2 layers here ([`has_underlay`]). Every other element of a
//! run sews nothing first, and is generated whole in the second pass, as [`super::generate`] generates it.
//!
//! **What the order reads first.** Before any element is generated, the order reads whether each sets
//! `underlays_first` ([`sets`]), whether a trim or a stop follows it ([`ends`]), whether its settings read
//! ([`reads`]) and whether it has an underlay ([`has_underlay`]). Each is read from the element's settings
//! alone, as its stitch type reads them, and costs no work: the registry's parameters are a fixed list.

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Meter, Mm, SplitMix64};
use stitchcraft_params::kinds::Toggle;
use stitchcraft_params::{Family, ParamSpec, StitchType, left_to_others, unknown_keys};
use stitchcraft_plan::MachineProfile;

use super::{
    FillMethod, FillParams, Generated, Generation, Prepared, SatinRead, Shared, clip_not_applied, legacy, named, prepare, setting, stitch_type_of,
    trim_or_stop_left_out, within,
};
use crate::common::{CommonParams, STOP_AFTER, TRIM_AFTER, UNDERLAYS_FIRST};
use crate::design::{DesignSettings, Element, Hints, Shape};
use crate::generators::satin::{CENTER_WALK_UNDERLAY, CONTOUR_UNDERLAY, Column, SatinParams, ZIGZAG_UNDERLAY};
use crate::generators::tatami::sew::underlay_sewn;
use crate::generators::tatami::{FILL_UNDERLAY, TatamiParams, UNDERLAID};
use crate::generators::{Approach, Layer, Neighbours, Stitched};
use crate::normalize::region::Region;
use crate::registry::PARAMETERS;

/// Whether `element` sets `underlays_first`, read alone, whatever stitch type reads it. A value that doesn't
/// read sets nothing, and reading the element for sewing says so (`SC-E0101`).
pub(crate) fn sets(element: &Element) -> bool {
    setting::<Toggle>(&element.params, CommonParams::SPECS, UNDERLAYS_FIRST, &mut Vec::new()) == Some(true)
}

/// Whether a trim or a stop follows `element`, each read alone, as [`sets`] reads it.
pub(crate) fn ends(element: &Element) -> bool {
    [TRIM_AFTER, STOP_AFTER].iter().any(|key| setting::<Toggle>(&element.params, CommonParams::SPECS, key, &mut Vec::new()) == Some(true))
}

/// Whether the settings of `element`, in a design with `settings`, read as its stitch type reads them: those
/// that choose its stitch type, and every other its stitch type reads, by the registry's declarations of
/// them, but those it leaves to the node's other stitch type (`REQ-PRM-006`). An element whose settings
/// don't read is skipped with `SC-E0101` when it is generated, and is in no run.
pub(crate) fn reads(element: &Element, settings: &DesignSettings) -> bool {
    let Some(stitch_type) = stitch_type_of(element, settings) else { return false };
    let mut set = element.params.clone();
    for (key, _) in left_to_others(&element.params, stitch_type, PARAMETERS) {
        set.remove(&key);
    }
    let family = stitch_type.family();
    let reads = |spec: &ParamSpec, specs: &[ParamSpec]| {
        let text = set.get(spec.key).filter(|text| !text.trim().is_empty() && spec.read_by(stitch_type, &set, specs));
        text.is_none_or(|text| spec.in_family(family).read(text).is_ok())
    };
    PARAMETERS.iter().all(|group| group.specs.iter().all(|spec| reads(spec, group.specs)))
}

/// Whether `element`, in a design with `settings`, has an underlay to sew before its top stitches: a satin
/// column with one of its underlays on, or a fill sewn with one, whose method has the tatami fill's underlay,
/// with `fill_underlay` on. A fill set to Ink/Stitch's legacy fill has none (`REQ-FILL-005`), nor has one set
/// not to be sewn, nor a guided fill whose strategy is not sewn yet (`REQ-FILL-GDD-001`). Each setting is read
/// alone, as the element reads it.
pub(crate) fn has_underlay(element: &Element, settings: &DesignSettings) -> bool {
    let set = &element.params;
    let on = |specs: &[ParamSpec], key: &str| setting::<Toggle>(set, specs, key, &mut Vec::new()) == Some(true);
    match stitch_type_of(element, settings) {
        Some(stitch_type) if stitch_type.family() == Family::Satin => {
            [CENTER_WALK_UNDERLAY, CONTOUR_UNDERLAY, ZIGZAG_UNDERLAY].iter().any(|key| on(SatinParams::SPECS, key))
        }
        Some(stitch_type) if UNDERLAID.contains(&stitch_type) => {
            let sewn = stitch_type != StitchType::GuidedFill || super::copied(set);
            on(FillParams::SPECS, "fill") && !legacy(set) && sewn && on(TatamiParams::SPECS, FILL_UNDERLAY)
        }
        _ => false,
    }
}

/// An element of a run of underlays first, read once for its 2 layers, between its underlay and its top.
pub(crate) struct Layered<'a> {
    element: &'a Element,
    common: CommonParams,
    stitch_type: StitchType,
    min_stitch: Mm,
    hints: Hints,
    /// The element's random numbers, as its top draws them.
    rng: SplitMix64,
    sews: Sews,
    /// What is left of the element's work once its underlay is sewn: its layers share it.
    work: u64,
}

/// What an element of a run sews from.
enum Sews {
    /// A satin column, placed.
    Satin(Box<Column>),
    /// A fill's area, and its settings.
    Fill(Region, Box<FillMethod>),
}

/// `element`, in a design with `settings` on the machine `profile` describes, read once and its underlay
/// sewn between its `neighbours`, with `meter`'s work and stitches: the layer, as [`super::generate`] gives an
/// element, and the element held for its top. With the element skipped, as `generate` skips it, or left out
/// whole, no element is held, and the trim or stop after it is left out with `SC-W0505`. `Err` with what it
/// says when its stitches are more than `meter` has left, `budget`'s limit for the design, which ends its
/// planning.
pub(crate) fn underlay<'a>(
    element: &'a Element,
    settings: &DesignSettings,
    profile: &MachineProfile,
    neighbours: &Neighbours,
    budget: &Budget,
    mut meter: Meter,
) -> Result<(Generation, Option<Layered<'a>>), Vec<Diagnostic>> {
    let mut diagnostics = unknown_keys(&element.params, PARAMETERS);
    let left = meter.stitches_left();
    let sewn = Layered::read(element, settings, profile, &mut diagnostics, &mut meter).and_then(|read| {
        let Some(mut layered) = read else { return Ok(None) };
        let generated = layered.sew(Layer::Underlay, neighbours, &mut diagnostics, &mut meter)?;
        meter.settle_stitches(left, generated.groups.iter().map(Vec::len).sum())?;
        layered.work = meter.work_left();
        Ok(Some((generated, layered)))
    });
    let over = matches!(sewn, Err(Exhausted::Stitches));
    let (generated, held) = within(element, budget, &mut diagnostics, sewn).unzip();
    if held.is_none() {
        diagnostics.extend(trim_or_stop_left_out(element));
    }
    let diagnostics = named(element, diagnostics);
    if over { Err(diagnostics) } else { Ok((Generation { generated, diagnostics }, held)) }
}

impl<'a> Layered<'a> {
    /// `element` read for sewing, as [`super::generate`] reads it: its settings, its shape, and for a satin
    /// column its placing, which draws its random numbers. `None` when it is skipped, or has no underlay to
    /// sew first, which `diagnostics` says.
    fn read(
        element: &'a Element,
        settings: &DesignSettings,
        profile: &MachineProfile,
        diagnostics: &mut Vec<Diagnostic>,
        meter: &mut Meter,
    ) -> Result<Option<Layered<'a>>, Exhausted> {
        let Some(Prepared { region, common, sewing }) = prepare(element, settings, profile, diagnostics, meter)? else { return Ok(None) };
        let mut rng = SplitMix64::for_element(element.id.as_str(), common.as_ref().and_then(|common| common.random_seed));
        let sews = match &element.shape {
            Shape::Fill { .. } => FillMethod::read(&sewing, diagnostics).map(|method| Sews::Fill(region, Box::new(method))),
            Shape::Stroke { path, width, join, .. } if sewing.stitch_type.family() == Family::Satin => {
                clip_not_applied(element, diagnostics);
                let Some(SatinRead { rails, params, pull, lengths }) = SatinRead::read(&sewing, (path, *width, *join), diagnostics, meter)? else {
                    return Ok(None);
                };
                let (column, warnings) = Column::new(&rails, &params, &pull, lengths, &mut rng, meter)?;
                diagnostics.extend(warnings);
                // A column left out is left out whole: neither layer sews it.
                if let Some(left_out) = column.left_out() {
                    diagnostics.push(left_out);
                    return Ok(None);
                }
                Some(Sews::Satin(Box::new(column)))
            }
            Shape::Stroke { .. } => {
                diagnostics.push(Diagnostic::new(Code::InternalCheckFailed, "A stroke has no underlay to sew before its top stitches."));
                None
            }
        };
        let (Some(common), Some(Shared { min_stitch, .. }), Some(sews)) = (common, sewing.shared, sews) else { return Ok(None) };
        let (stitch_type, hints) = (sewing.stitch_type, element.hints.clone());
        Ok(Some(Layered { element, common, stitch_type, min_stitch, hints, rng, sews, work: 0 }))
    }

    /// Its top, sewn between its `neighbours` with what is left of its work and the design's `left` stitches,
    /// as [`underlay`] sews its underlay. With its work spent, the element is skipped (`SC-E0004`), and
    /// `None` is generated: its underlay is then left out too. Its trim or stop goes after its top, and is
    /// left out with `SC-W0505` when its top sews no stitch, as when the element is skipped whole: the element
    /// then sews nothing, as its underlay sews nothing when its top does (a column with no section).
    pub(crate) fn top(mut self, neighbours: &Neighbours, budget: &Budget, left: u32) -> Result<Generation, Vec<Diagnostic>> {
        let element = self.element;
        let mut diagnostics = Vec::new();
        let mut meter = Budget { max_stitches: left, max_work: self.work }.meter();
        let sewn = self.sew(Layer::Top, neighbours, &mut diagnostics, &mut meter).and_then(|generated| {
            meter.settle_stitches(left, generated.groups.iter().map(Vec::len).sum())?;
            Ok(Some(generated))
        });
        let over = matches!(sewn, Err(Exhausted::Stitches));
        let generated = within(element, budget, &mut diagnostics, sewn);
        if !generated.as_ref().is_some_and(|g| g.groups.iter().any(|group| !group.is_empty())) {
            diagnostics.extend(trim_or_stop_left_out(element));
        }
        let diagnostics = named(element, diagnostics);
        if over { Err(diagnostics) } else { Ok(Generation { generated, diagnostics }) }
    }

    /// Its `layer`, sewn between its `neighbours`, with what it says in `diagnostics`. The start position
    /// places the start of its underlay, its first layer, and the end position the end of its top.
    fn sew(&mut self, layer: Layer, neighbours: &Neighbours, diagnostics: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<Generated, Exhausted> {
        let stitched = match &self.sews {
            Sews::Satin(column) => column.sew(layer, neighbours, &self.hints, meter)?,
            Sews::Fill(region, method) if layer == Layer::Underlay => {
                // A fill is held for its layers only with an underlay (`has_underlay`), which a cross stitch has not.
                let Some((underlay, inset, travel)) = method.underlay() else { return Ok(self.generated(Vec::new(), Vec::new(), layer)) };
                let needle = self.hints.start.or(neighbours.needle);
                underlay_sewn(region, underlay, inset, travel, needle, neighbours.next.as_ref(), &mut self.rng, meter)?
            }
            Sews::Fill(region, method) => {
                let end = self.hints.end.map(Approach::Point);
                method.without_underlay().sew(region, neighbours.needle, end.as_ref().or(neighbours.next.as_ref()), &mut self.rng, meter)?
            }
        };
        let Stitched { runs, colours, warnings } = stitched;
        diagnostics.extend(warnings);
        Ok(self.generated(runs, colours, layer))
    }

    /// The element's `layer`, of the stitch groups `runs` in their `colours`.
    fn generated(&self, runs: Vec<Vec<stitchcraft_core::Point>>, colours: Vec<crate::generators::Colour>, layer: Layer) -> Generated {
        Generated { common: self.common.clone(), stitch_type: self.stitch_type, groups: runs, colours, min_stitch: self.min_stitch, layer }
    }
}

#[cfg(test)]
mod tests;
