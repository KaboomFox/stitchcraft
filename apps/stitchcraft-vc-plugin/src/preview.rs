//! What a live effect shows, within what VectorCraft lets it spend.
//!
//! **The budget.** VectorCraft lets a live effect run 50 million instructions, and 4,000 more for each byte
//! of its input, and stops it there ([`FUEL_BASE`], [`FUEL_PER_BYTE`]). The engine counts its own work in
//! units, and the contract tests measure what a unit costs in VectorCraft's host (`compat/vc-contract`).
//! The effect turns the instructions it has left, after reading its input, into a budget of units for the
//! engine, and prices what it writes by the anchor. It shows less when the engine runs out, or when what
//! it would write costs more than its share, rather than be stopped.
//!
//! **Levels** (`preview_level`, [`Level`]). *Exact* shows every stitch. *Rows* shows a fill's top rows,
//! one ribbon for each row segment: where its stitches lie and which way they run, without the stitches,
//! the underlay or the needle's travel, for a small part of the work. *Outline* shows the object's path,
//! as lines for a stroke or a satin column and as ribbons for a fill, hatched in the fill's direction every
//! few millimetres. *Auto*, the default, shows a stroke or a satin column exactly and a fill by its rows,
//! and the outline when that costs too much. A level the user picks gets the whole budget, and when that
//! is not enough the effect names a cheaper one (`SC-I0810`).
//!
//! **Why auto shows a fill's rows.** A stroke's work costs about 880 instructions a unit and a satin
//! column's 4,730, on every shape measured, so a budget of units bounds their exact previews. A fill's
//! costs from 1,900 to 5,300 by the part of the work, and a 7 mm square's exact preview takes 27 million:
//! an exact preview of a fill fits only when it is small, and no budget of units can tell beforehand. So a
//! fill's exact preview is the user's to pick, with all that VectorCraft allows, which stops one that needs
//! more with its own error.
//!
//! **A satin column drawn as one path** (M6.8). It makes its rails from its centre line before it sews,
//! in many units of about 330 instructions each, which the satin column's price counts as dear as its
//! stitches, since one budget of units cannot tell them apart. One bend takes 33,100 units, so auto shows
//! the outline of a centre line that bends, and its exact preview is the user's to pick: 12.6 million
//! instructions for one bend, 25 million for a quarter circle of 17 points.
//!
//! **The Auto effect** (`REQ-VC-016`). Its auto level shows the stitches automatic digitizing chooses for
//! the object: it digitizes the object's area and line ([`stitchcraft_engine::auto::digitize`]), then shows
//! each element they become as auto shows one of its family, a fill by its rows and the others exactly,
//! and the outline when that costs too much. Exact shows every stitch, with all that VectorCraft allows.
//! VectorCraft draws what it returns with every paint of the object, so it returns ribbons when a fill
//! paints the object, and lines for its stroke otherwise. Digitizing is most of the work, from about
//! 1,450 instructions a unit for a filled square to 2,260 for a line traced as an area 0.3 mm wide,
//! measured in VectorCraft's host and priced at 2,700 (`DIGITIZE_PER_UNIT`): a square's takes 12,900
//! units, an L's 21,000 and a ring's 31,800, of the 50 million instructions a live effect has. So a small
//! square shows its rows, and a larger one, an L, a ring and most traced letters show their outlines:
//! digitizing them leaves too little for their stitches, or needs more than there is.
//!
//! **Where the numbers come from.** Spike M0.6 measured reading JSON at 512 instructions a byte, M6.3 the
//! rest and M13.7 digitizing, in VectorCraft's host, on the shapes the contract tests run. Each constant
//! here is the largest measured, with a margin. The contract tests fail when a level takes more than
//! VectorCraft allows, which is when a constant needs measuring again
//! (`docs/src/design/vectorcraft-integration.md`, live-effect previews).

use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Point};
use stitchcraft_engine::auto::digitize;
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generate::{generate, rows, stitch_type_of};
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_engine::normalize::satin::{Shape as Column, recognize};
use stitchcraft_engine::normalize::stroke::{SHAPE_TOLERANCE, flatten};
use stitchcraft_params::{Family, StitchType};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::{AutoEffect, Effect, effect_drawn, effect_error, effect_lines, effect_output, effect_ribbons};

use crate::dialog::own;
use crate::manifest::{Param, Schema};

/// The instructions VectorCraft lets any live effect run (`Limits::default()` in its `crates/plugins`,
/// which the contract tests check).
pub const FUEL_BASE: u64 = 50_000_000;
/// The instructions VectorCraft adds for each byte of a live effect's input, and of a filter's.
pub const FUEL_PER_BYTE: u64 = 4_000;
/// The instructions VectorCraft lets a heavy filter run besides those for each byte of its input, from 0.17
/// (`HEAVY_FUEL` in its `crates/plugins`, which the contract tests check): Stroke to Satin's
/// ([`crate::stroke_to_satin`]).
pub const HEAVY_FUEL: u64 = 1_000_000_000;
/// What reading JSON costs, for each byte of the input and the parameters (512 measured).
const READ_PER_BYTE: u64 = 650;
/// What a run costs before and besides its work: starting, reading the settings, the manifest's
/// defaults. At most 900,000 measured.
const SETUP: u64 = 2_000_000;
/// What writing one anchor costs: 16 instructions a byte measured, and at most 27 bytes an anchor, in a
/// fill's ribbons.
const PER_ANCHOR: u64 = 500;
/// How far apart the outline's hatch lines lie, in millimetres.
const HATCH_MM: f64 = 3.0;

/// What a preview shows: how much of the stitches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    /// The most the budget allows.
    Auto,
    /// Every stitch.
    Exact,
    /// A fill's top rows, one ribbon for each row segment.
    Rows,
    /// The object's path, hatched across a fill in the fill's direction.
    Outline,
}

impl Level {
    /// The levels a family's previews offer, the default first.
    pub fn offered(family: Family) -> &'static [Level] {
        match family {
            Family::Fill => &[Level::Auto, Level::Exact, Level::Rows, Level::Outline],
            Family::Stroke | Family::Satin => &[Level::Auto, Level::Exact, Level::Outline],
        }
    }

    /// The level's id, as `preview_level` takes it.
    pub const fn id(self) -> &'static str {
        match self {
            Level::Auto => "auto",
            Level::Exact => "exact",
            Level::Rows => "rows",
            Level::Outline => "outline",
        }
    }

    /// The level `id` names; auto for one it does not, which VectorCraft never passes, since it gives a
    /// choice one of the manifest's options.
    pub fn read(id: Option<&str>) -> Level {
        [Level::Exact, Level::Rows, Level::Outline].into_iter().find(|level| Some(level.id()) == id).unwrap_or(Level::Auto)
    }
}

/// The levels the Auto effect's previews offer, the default first. Its auto level already shows the rows of
/// each fill it makes.
pub(crate) const AUTO_LEVELS: &[Level] = &[Level::Auto, Level::Exact, Level::Outline];

/// How a family's manifest declares `preview_level`: a choice of the levels its previews offer.
pub(crate) fn param(family: Family) -> Param {
    level_param(Level::offered(family), HELP)
}

/// How the Auto effect's manifest declares `preview_level`: a choice of [`AUTO_LEVELS`].
pub(crate) fn auto_param() -> Param {
    level_param(AUTO_LEVELS, AUTO_HELP)
}

/// `preview_level` as a choice of `levels`, auto by default, in a section of its own, with `help` and the
/// page that explains the levels.
fn level_param(levels: &[Level], help: &str) -> Param {
    let schema = Schema::Choice { options: levels.iter().map(|level| level.id()).collect(), default: Level::Auto.id() };
    let mut param = own(schema, "Preview", help, PAGE);
    param.ui.group = Some("Preview");
    param
}

/// What `preview_level` does, for its help.
const HELP: &str = "How much of the stitches the effect shows. Exact shows every stitch, rows a fill's top rows, and outline the \
                    object's path, hatched across a fill, which costs least. Auto shows as much as VectorCraft lets a live effect \
                    work out. The machine file has every stitch, whichever is chosen.";

/// What the Auto effect's `preview_level` does, for its help.
const AUTO_HELP: &str = "How much of the stitches the effect shows. Exact shows every stitch automatic digitizing chooses, and outline \
                         the object's path, hatched across a fill, which costs least. Auto shows as much as VectorCraft lets a live \
                         effect work out: the stitches of each satin column and running stitch, and the top rows of each fill. The \
                         machine file has every stitch, whichever is chosen.";

/// The page that explains the levels.
const PAGE: &str = "design/vectorcraft-integration.html#live-effect-previews-within-v1-budgets";

/// The instructions a plug-in has for its work, given `input` and `params` bytes to read: what VectorCraft
/// allows, `base` besides [`FUEL_PER_BYTE`] for each byte of the input, less reading and setting up. The
/// base is [`FUEL_BASE`] for a live effect, and [`HEAVY_FUEL`] for a heavy filter.
pub fn spendable(base: u64, input: usize, params: usize) -> u64 {
    let bytes = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    base.saturating_add(FUEL_PER_BYTE.saturating_mul(bytes(input)))
        .saturating_sub(READ_PER_BYTE.saturating_mul(bytes(input).saturating_add(bytes(params))))
        .saturating_sub(SETUP)
}

/// What a preview showed.
#[derive(Debug, PartialEq)]
enum Shown {
    /// The geometry a level draws.
    Drawn(Level, Vec<u8>),
    /// The object sews nothing, or cannot be sewn: why.
    Unsewn(Diagnostic),
    /// It needed more than its share of the budget.
    TooCostly,
}

/// The preview of `effect`, of `family`, within `spend` instructions: the geometry VectorCraft draws, or
/// why it draws the object as it is.
pub fn show(family: Family, effect: &Effect, spend: u64) -> Vec<u8> {
    written(preview(family, effect, spend), effect.preview_level.as_deref(), Level::offered(family))
}

/// The preview of `effect`, the Auto effect's, within `spend` instructions (`REQ-VC-016`), as [`show`]
/// shows a family's.
pub fn show_auto(effect: &AutoEffect, spend: u64) -> Vec<u8> {
    written(auto_preview(effect, Spend(spend)), effect.preview_level.as_deref(), AUTO_LEVELS)
}

/// What VectorCraft is given for `shown`, a preview at the level `asked` names among those `offered`: the
/// geometry, or why the object is drawn as it is.
fn written(shown: Shown, asked: Option<&str>, offered: &[Level]) -> Vec<u8> {
    match shown {
        Shown::Drawn(_, json) => json,
        Shown::Unsewn(why) => effect_error(&why),
        Shown::TooCostly => effect_error(&too_costly(Level::read(asked), offered)),
    }
}

/// What [`show`] shows, and at which level.
fn preview(family: Family, effect: &Effect, spend: u64) -> Shown {
    let (element, spend) = (&effect.element, Spend(spend));
    // Given no stroke, the element's stroke is a stand-in, and stitches as wide as it would mislead.
    if !effect.stroked
        && let Some(unseen) = centre_line(family, element, spend).or_else(|| zigzag(family, element))
    {
        return Shown::Unsewn(unseen);
    }
    match (Level::read(effect.preview_level.as_deref()), family) {
        (Level::Exact, _) => exact(family, element, None),
        (Level::Rows, Family::Fill) => fill_rows(element, spend),
        // A stroke's and a satin column's previews offer no rows.
        (Level::Outline | Level::Rows, _) => outline(family, element, spend),
        (Level::Auto, Family::Fill) => fill_rows(element, spend.tenths(8)).or(|| outline(family, element, spend.tenths(2))),
        (Level::Auto, Family::Stroke | Family::Satin) => {
            exact(family, element, Some(spend.tenths(9))).or(|| outline(family, element, spend.tenths(1)))
        }
    }
}

/// Instructions a preview may spend, shared out among its parts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Spend(u64);

impl Spend {
    /// `n` tenths of it.
    fn tenths(self, n: u64) -> Spend {
        Spend(self.0.saturating_mul(n) / 10)
    }

    /// What each of `count` parts may spend of eight tenths of it, shared out equally: all eight tenths
    /// for one part, or for none.
    fn shared(self, count: usize) -> Spend {
        Spend(self.tenths(8).0 / u64::try_from(count).unwrap_or(u64::MAX).max(1))
    }

    /// A budget of the engine's work it buys, at `per_unit` instructions a unit, and of any number of
    /// needle points.
    fn work(self, per_unit: u64) -> Budget {
        Budget { max_work: self.0 / per_unit, max_stitches: u32::MAX }
    }

    /// Whether it pays for writing `count` things of `anchors` anchors each.
    fn writes(self, count: usize, anchors: u64) -> bool {
        u64::try_from(count).unwrap_or(u64::MAX).saturating_mul(anchors.saturating_mul(PER_ANCHOR)) <= self.0
    }
}

impl Shown {
    /// This, or what `cheaper` shows when this was too costly.
    fn or(self, cheaper: impl FnOnce() -> Shown) -> Shown {
        match self {
            Shown::TooCostly => cheaper(),
            shown => shown,
        }
    }
}

/// Every stitch of `element`, of `family`: within `spend`, nine tenths for the engine's work and a tenth
/// for writing the needle points, for a family whose work [`per_unit`] prices; or else within what
/// VectorCraft allows, which stops a run that needs more.
fn exact(family: Family, element: &Element, spend: Option<Spend>) -> Shown {
    let priced = spend.zip(per_unit(family));
    let budget = priced.map_or(Budget::DEFAULT, |(spend, per_unit)| spend.tenths(9).work(per_unit));
    let (groups, said) = match stitched(element, &budget) {
        Ok(stitched) => stitched,
        Err(shown) => return shown,
    };
    // A stroke's and a satin column's needle points are an anchor each. Sewing them does not count them.
    if priced.is_some_and(|(spend, _)| !spend.tenths(1).writes(groups.iter().map(Vec::len).sum(), anchors(family))) {
        return Shown::TooCostly;
    }
    effect_output(family, &groups).map_or_else(|| Shown::Unsewn(no_stitch(said)), |json| Shown::Drawn(Level::Exact, json))
}

/// What a step of a preview drew, and what working it out said; or what the preview shows instead.
type Worked<T> = Result<(T, Vec<Diagnostic>), Shown>;

/// Every stitch of `element` within `budget`, as runs of needle points, and what sewing it said; or
/// [`Shown::TooCostly`] when the budget ran out.
fn stitched(element: &Element, budget: &Budget) -> Worked<Vec<Vec<Point>>> {
    let generation = generate(element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), budget);
    if spent(&generation.diagnostics) {
        return Err(Shown::TooCostly);
    }
    Ok((generation.generated.map(|generated| generated.groups).unwrap_or_default(), generation.diagnostics))
}

/// The top rows of `element`, a fill, within `spend`, a ribbon for each segment: nine tenths for laying
/// them, and a tenth for writing them.
fn fill_rows(element: &Element, spend: Spend) -> Shown {
    let (segments, said) = match rowed(element, &spend.tenths(9).work(ROWS_PER_UNIT)) {
        Ok(rowed) => rowed,
        Err(shown) => return shown,
    };
    if !spend.tenths(1).writes(segments.len(), RIBBON_ANCHORS) {
        return Shown::TooCostly;
    }
    effect_ribbons(segments).map_or_else(|| Shown::Unsewn(no_stitch(said)), |json| Shown::Drawn(Level::Rows, json))
}

/// The top rows of `element`, a fill, laid within `budget`, as their segments, and what laying them said;
/// none when it has no rows, and [`Shown::TooCostly`] when the budget ran out.
fn rowed(element: &Element, budget: &Budget) -> Worked<Vec<(Point, Point)>> {
    let laid = rows(element, &DesignSettings::default(), REFERENCE, budget);
    if spent(&laid.diagnostics) {
        return Err(Shown::TooCostly);
    }
    let segments = laid.segments.unwrap_or_default().iter().map(|segment| (segment.start, segment.end)).collect();
    Ok((segments, laid.diagnostics))
}

/// The path of `element`, of `family`, within `spend`: a stroke's and a satin column's as lines, a fill's
/// as ribbons, hatched every [`HATCH_MM`] in the fill's direction when that fits too. Three tenths for
/// flattening the path, two for writing it, three for laying the hatch and two for writing that.
fn outline(family: Family, element: &Element, spend: Spend) -> Shown {
    let Ok(flat) = flatten(element.shape.path(), SHAPE_TOLERANCE, &mut spend.tenths(3).work(ROWS_PER_UNIT).meter()) else {
        return Shown::TooCostly;
    };
    let lines = flat.pieces.iter().map(|piece| piece.points.as_slice());
    if !spend.tenths(2).writes(flat.pieces.iter().map(|piece| piece.points.len()).sum(), anchors(family)) {
        return Shown::TooCostly;
    }
    let drawn = match family {
        Family::Stroke | Family::Satin => effect_lines(lines),
        Family::Fill => {
            let edges = lines.flat_map(|line| line.windows(2)).filter_map(|w| Some((*w.first()?, *w.get(1)?)));
            let mut hatched = element.clone();
            hatched.params.set(ROW_SPACING, HATCH_MM.to_string());
            hatched.params.set(END_ROW_SPACING, "");
            let hatch = rows(&hatched, &DesignSettings::default(), REFERENCE, &spend.tenths(3).work(ROWS_PER_UNIT)).segments.unwrap_or_default();
            let hatch = if spend.tenths(2).writes(hatch.len(), RIBBON_ANCHORS) { hatch } else { Vec::new() };
            effect_ribbons(edges.chain(hatch.iter().map(|segment| (segment.start, segment.end))))
        }
    };
    drawn.map_or(Shown::TooCostly, |json| Shown::Drawn(Level::Outline, json))
}

/// The anchors of a ribbon.
const RIBBON_ANCHORS: u64 = 4;

/// The anchors an outline writes for each point of its path: a ribbon's to a fill's edge, and one for a
/// stroke's or a satin column's line.
fn anchors(family: Family) -> u64 {
    match family {
        Family::Fill => RIBBON_ANCHORS,
        Family::Stroke | Family::Satin => 1,
    }
}

/// The instructions the engine runs for each unit of work it charges, sewing an element of `family`,
/// measured in VectorCraft's host with a margin: 880 for a stroke and 4,730 for a satin column, the same
/// on every shape measured. `None` for a fill, whose units cost from 1,900 to 5,300 instructions by the
/// part of the work, so that no one price bounds a fill's exact preview.
fn per_unit(family: Family) -> Option<u64> {
    match family {
        Family::Stroke => Some(1_100),
        Family::Satin => Some(5_500),
        Family::Fill => None,
    }
}

/// The fill settings the outline sets, by the registry's keys (a test checks them).
const ROW_SPACING: &str = "row_spacing_mm";
const END_ROW_SPACING: &str = "end_row_spacing_mm";

/// The instructions for each unit of work laying a fill's rows, reading or flattening a path.
const ROWS_PER_UNIT: u64 = 3_200;

/// The budget for reading a live effect's object, with `spend` instructions: the work a tenth of them buys.
pub fn reading(spend: u64) -> Budget {
    Spend(spend).tenths(1).work(ROWS_PER_UNIT)
}

/// The budget of the engine's work that `tenths` tenths of `spend` instructions buy, at `per_unit`
/// instructions a unit, and of any number of needle points.
pub(crate) fn work(spend: u64, tenths: u64, per_unit: u64) -> Budget {
    Spend(spend).tenths(tenths).work(per_unit)
}

/// `SC-I0810` when `element`, of `family`, is a satin column drawn as one path, its centre line, and
/// VectorCraft gave the effect no stroke: the column is as wide as its stroke (`REQ-VC-010`). Telling takes
/// a tenth of `spend`; a path too long to tell within it is sewn, and the levels' budgets bound that.
fn centre_line(family: Family, element: &Element, spend: Spend) -> Option<Diagnostic> {
    let (Family::Satin, Shape::Stroke { path, .. }) = (family, &element.shape) else { return None };
    let Ok(Column::CentreLine { .. }) = recognize(path, &mut reading(spend.0).meter()).ok()?.shape else { return None };
    let message = "This satin column is drawn as one path, as wide as its stroke, and VectorCraft gives the live effect no stroke for it: \
                   the effect leaves it as it is.";
    let hint = "Give the object a stroke as wide as the column, or draw its 2 rails and join them with Object › Compound Path › Make.";
    Some(Diagnostic::new(Code::VcEffectUnchanged, message).with_fix(Fix::Hint(hint.into())))
}

/// `SC-I0810` when `element`, of `family`, is a zigzag stroke and VectorCraft gave the effect no stroke: a
/// zigzag is as wide as its stroke (`REQ-VC-010`). The object drawn as it is shows its width.
fn zigzag(family: Family, element: &Element) -> Option<Diagnostic> {
    let (Family::Stroke, Some(StitchType::ZigzagStitch)) = (family, stitch_type_of(element, &DesignSettings::default())) else { return None };
    let message = "This zigzag stroke is as wide as its stroke, and VectorCraft gives the live effect no stroke for it: the effect leaves it \
                   as it is.";
    Some(Diagnostic::new(Code::VcEffectUnchanged, message).with_fix(Fix::Hint("Give the object a stroke as wide as the zigzag.".into())))
}

/// Whether `diagnostics` say the budget ran out.
fn spent(diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|d| d.code == Code::BudgetExhausted)
}

/// Why an element sews no stitch: the most serious of what sewing it said, the first of those, or
/// `SC-I0810` when it said nothing, as for a fill set not to be sewn.
pub(crate) fn no_stitch(diagnostics: Vec<Diagnostic>) -> Diagnostic {
    diagnostics
        .into_iter()
        .min_by_key(Diagnostic::severity)
        .unwrap_or_else(|| Diagnostic::new(Code::VcEffectUnchanged, "This object sews no stitch, so the live effect leaves it as it is."))
}

/// `SC-I0810` for a preview at `level` that needs more work than VectorCraft lets a live effect do, with
/// the cheaper levels among those `offered` to pick instead. Auto has tried them all.
fn too_costly(level: Level, offered: &[Level]) -> Diagnostic {
    let cheaper: Vec<&str> = match level {
        Level::Auto => Vec::new(),
        _ => offered.iter().skip_while(|offered| **offered != level).skip(1).map(|offered| offered.id()).collect(),
    };
    let message = format!("Its {} preview needs more work than VectorCraft lets a live effect do, so the effect leaves it as it is.", level.id());
    let hint = match cheaper.as_slice() {
        [] => "The command line's preview shows its stitches.".to_string(),
        [one] => format!("Set its preview level to `{one}`."),
        more => format!("Set its preview level to `{}`.", more.join("` or `")),
    };
    Diagnostic::new(Code::VcEffectUnchanged, message).with_fix(Fix::Hint(hint))
}

/// The instructions the engine runs for each unit of work automatic digitizing charges, in VectorCraft's
/// host, with a margin: measured in VectorCraft 0.18.0's host, about 1,450 for a filled square or an L, and
/// 2,260 for a line traced as an area 0.3 mm wide, whose centre lines digitizing finds. The contract tests
/// check that the previews it bounds finish there, these shapes' among them.
const DIGITIZE_PER_UNIT: u64 = 2_700;

/// What [`show_auto`] shows of `effect`, and at which level: at auto, the stitches automatic digitizing
/// chooses within eight tenths of `spend` ([`digitized`]), or else the outline within the rest.
fn auto_preview(effect: &AutoEffect, spend: Spend) -> Shown {
    match Level::read(effect.preview_level.as_deref()) {
        Level::Exact => digitized(effect, None),
        Level::Outline => auto_outline(effect, spend),
        // The manifest offers no rows: VectorCraft never passes them.
        Level::Auto | Level::Rows => digitized(effect, Some(spend.tenths(8))).or(|| auto_outline(effect, spend.tenths(2))),
    }
}

/// Whether a fill paints `effect`'s object, which then fills what the effect returns: its area comes first.
fn filled(effect: &AutoEffect) -> bool {
    effect.elements.first().is_some_and(|element| matches!(element.shape, Shape::Fill { .. }))
}

/// The stitches automatic digitizing chooses for `effect`'s object, as auto shows each element it becomes:
/// a fill's top rows, and a satin column's and a running stitch's every stitch. Within `spend`, digitizing
/// first, since it is most of the work, then the elements within eight tenths of what it leaves, shared out
/// equally, and writing them within two. Without, at the exact level, every stitch of each, within all that
/// VectorCraft allows, which stops a run that needs more.
fn digitized(effect: &AutoEffect, spend: Option<Spend>) -> Shown {
    let settings = DesignSettings::default();
    let budget = spend.map_or(Budget::DEFAULT, |spend| spend.work(DIGITIZE_PER_UNIT));
    let mut meter = budget.meter();
    let (mut made, mut said) = (Vec::new(), Vec::new());
    for element in &effect.elements {
        let Ok(digitized) = digitize(element, &settings, &mut meter) else { return Shown::TooCostly };
        made.extend(digitized.elements);
        said.extend(digitized.diagnostics);
    }
    let digitizing = budget.max_work.saturating_sub(meter.work_left()).saturating_mul(DIGITIZE_PER_UNIT);
    let left = spend.map(|spend| Spend(spend.0.saturating_sub(digitizing)));
    let share = left.map(|left| left.shared(made.len()));
    let mut runs: Vec<Vec<Point>> = Vec::new();
    for element in &made {
        let family = stitch_type_of(element, &settings).map_or(Family::Stroke, StitchType::family);
        let drawn = match (family, share) {
            (Family::Fill, Some(share)) => rowed(element, &share.work(ROWS_PER_UNIT))
                .map(|(segments, said)| (segments.into_iter().map(|(start, end)| vec![start, end]).collect(), said)),
            (_, share) => stitched(element, &share.zip(per_unit(family)).map_or(Budget::DEFAULT, |(share, per_unit)| share.work(per_unit))),
        };
        match drawn {
            Ok((drawn, diagnostics)) => {
                runs.extend(drawn);
                said.extend(diagnostics);
            }
            Err(shown) => return shown,
        }
    }
    let ribbons = filled(effect);
    let (count, anchors) = to_write(&runs, ribbons);
    if left.is_some_and(|left| !left.tenths(2).writes(count, anchors)) {
        return Shown::TooCostly;
    }
    let level = if spend.is_some() { Level::Auto } else { Level::Exact };
    effect_drawn(&runs, ribbons).map_or_else(|| Shown::Unsewn(no_stitch(said)), |json| Shown::Drawn(level, json))
}

/// What [`effect_drawn`] writes of `runs`, as [`Spend::writes`] prices it: with `ribbons`, a ribbon for
/// each of their segments, of [`RIBBON_ANCHORS`] anchors, and else a point, an anchor, for each point.
fn to_write(runs: &[Vec<Point>], ribbons: bool) -> (usize, u64) {
    if ribbons { (runs.iter().map(|run| run.len().saturating_sub(1)).sum(), RIBBON_ANCHORS) } else { (runs.iter().map(Vec::len).sum(), 1) }
}

/// The outline of `effect`'s object within `spend`, as a fill's when a fill paints it, hatched, and as a
/// stroke's otherwise ([`outline`]).
fn auto_outline(effect: &AutoEffect, spend: Spend) -> Shown {
    match effect.elements.first() {
        Some(element) => outline(if filled(effect) { Family::Fill } else { Family::Stroke }, element, spend),
        None => Shown::Unsewn(no_stitch(Vec::new())),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use stitchcraft_engine::registry::PARAMETERS;
    use stitchcraft_params::Kind;
    use stitchcraft_vectorcraft::{read_auto_effect, read_effect};

    use super::*;
    use crate::testing::{auto_params, input, line, painted, params, rails, square};

    /// `family`'s parameters at their defaults, at preview `level`.
    fn params_with(family: Family, level: &str) -> Vec<u8> {
        params(family, &[("preview_level", json!(level))])
    }

    /// What `family`'s effect shows of `path` at `level`, with what VectorCraft would let it spend.
    fn shown(family: Family, path: &Value, level: &str) -> Shown {
        let (input, params) = (input(path), params_with(family, level));
        let effect = read_effect(family, &input, &params, &Budget::DEFAULT).unwrap();
        preview(family, &effect, spendable(FUEL_BASE, input.len(), params.len()))
    }

    /// The level `shown` drew at.
    fn level(shown: &Shown) -> Option<Level> {
        match shown {
            Shown::Drawn(level, _) => Some(*level),
            Shown::Unsewn(_) | Shown::TooCostly => None,
        }
    }

    #[test]
    fn req_vc_003_auto_shows_the_most_the_budget_allows() {
        // A fill by its rows, a 3.5 mm one as a 90 cm one, which shows its outline when they cost too much.
        assert_eq!(level(&shown(Family::Fill, &square(10.0), "auto")), Some(Level::Rows));
        assert_eq!(level(&shown(Family::Fill, &square(1280.0), "auto")), Some(Level::Rows));
        assert_eq!(level(&shown(Family::Fill, &square(2560.0), "auto")), Some(Level::Outline));
        // A stroke and a satin column exactly, and a stroke 36 m long by its outline.
        assert_eq!(level(&shown(Family::Stroke, &square(320.0), "auto")), Some(Level::Exact));
        assert_eq!(level(&shown(Family::Satin, &rails(960.0), "auto")), Some(Level::Exact));
        assert_eq!(level(&shown(Family::Stroke, &square(25_600.0), "auto")), Some(Level::Outline));
        // A 2 m satin column, whose 10,000 needle points cost more to write than its tenth of the budget, though
        // sewing them would fit.
        assert_eq!(level(&shown(Family::Satin, &rails(6000.0), "auto")), Some(Level::Outline));
    }

    #[test]
    fn req_vc_003_a_level_the_user_picks_gets_the_whole_budget() {
        // Rows that auto's share cannot afford, the whole budget can.
        assert_eq!(level(&shown(Family::Fill, &square(1600.0), "auto")), Some(Level::Outline));
        assert_eq!(level(&shown(Family::Fill, &square(1600.0), "rows")), Some(Level::Rows));
        assert_eq!(level(&shown(Family::Fill, &square(80.0), "outline")), Some(Level::Outline));
        // A fill's exact preview has all VectorCraft allows: natively, all there is.
        assert_eq!(level(&shown(Family::Fill, &square(80.0), "exact")), Some(Level::Exact));
        // A stroke's outline is its path as lines, a fill's as ribbons, hatched.
        let Shown::Drawn(_, json) = shown(Family::Stroke, &square(80.0), "outline") else { panic!("no outline") };
        let lines: Value = serde_json::from_slice(&json).unwrap();
        assert_eq!(lines["objects"][0]["path"]["subpaths"][0]["anchors"].as_array().map(Vec::len), Some(5));
        let Shown::Drawn(_, json) = shown(Family::Fill, &square(80.0), "outline") else { panic!("no outline") };
        let ribbons: Value = serde_json::from_slice(&json).unwrap();
        // 4 sides, and the hatch every 3 mm across the 28 mm square.
        assert_eq!(ribbons["objects"][0]["path"]["subpaths"].as_array().map(Vec::len), Some(4 + 10));
    }

    #[test]
    fn diag_sc_i0810_a_preview_that_needs_more_than_vectorcraft_allows_names_a_cheaper_level() {
        let said = |family: Family, level: Level| effect_error(&too_costly(level, Level::offered(family)));
        let text = |json: Vec<u8>| serde_json::from_slice::<Value>(&json).unwrap()["error"].as_str().unwrap().to_string();
        let start = |level: &str| {
            format!("info SC-I0810: Its {level} preview needs more work than VectorCraft lets a live effect do, so the effect leaves it as it is.")
        };
        assert_eq!(text(said(Family::Fill, Level::Exact)), format!("{} Set its preview level to `rows` or `outline`.", start("exact")));
        assert_eq!(text(said(Family::Fill, Level::Rows)), format!("{} Set its preview level to `outline`.", start("rows")));
        assert_eq!(text(said(Family::Satin, Level::Exact)), format!("{} Set its preview level to `outline`.", start("exact")));
        assert_eq!(text(said(Family::Stroke, Level::Outline)), format!("{} The command line's preview shows its stitches.", start("outline")));
        assert_eq!(text(said(Family::Fill, Level::Auto)), format!("{} The command line's preview shows its stitches.", start("auto")));
        // As a run says it: rows too costly at the level picked, and auto with nothing to spend.
        let (input, params) = (input(&square(2560.0)), params_with(Family::Fill, "rows"));
        let effect = read_effect(Family::Fill, &input, &params, &Budget::DEFAULT).unwrap();
        assert_eq!(text(show(Family::Fill, &effect, spendable(FUEL_BASE, input.len(), params.len()))), text(said(Family::Fill, Level::Rows)));
        let auto = read_effect(Family::Fill, &input, &params_with(Family::Fill, "auto"), &Budget::DEFAULT).unwrap();
        assert_eq!(text(show(Family::Fill, &auto, 0)), text(said(Family::Fill, Level::Auto)));
    }

    /// What the Auto effect shows of the object VectorCraft gives as `input`, at `level`, with what
    /// VectorCraft would let it spend.
    fn shown_auto(input: &[u8], level: &str) -> Shown {
        let params = auto_params(&[("preview_level", json!(level))]);
        let effect = read_auto_effect(input, &params, &Budget::DEFAULT).unwrap();
        auto_preview(&effect, Spend(spendable(FUEL_BASE, input.len(), params.len())))
    }

    /// The subpaths of a preview drawn, and whether each is closed.
    fn closed(shown: &Shown) -> Vec<bool> {
        let Shown::Drawn(_, json) = shown else { panic!("nothing drawn: {shown:?}") };
        let drawn: Value = serde_json::from_slice(json).unwrap();
        drawn["objects"][0]["path"]["subpaths"].as_array().unwrap().iter().map(|subpath| subpath["closed"].as_bool().unwrap_or(false)).collect()
    }

    #[test]
    fn req_vc_016_auto_shows_the_stitches_automatic_digitizing_chooses_drawn_with_the_object_s_paints() {
        // A filled square 14 mm a side is filled: its top rows, as ribbons for its fill to fill.
        let filled = painted(&square(40.0), true, None);
        let rows = shown_auto(&filled, "auto");
        assert_eq!(level(&rows), Some(Level::Auto));
        assert!(closed(&rows).len() > 10 && closed(&rows).iter().all(|closed| *closed), "{rows:?}");
        // A line 3 mm wide is a satin column: its stitches, as lines for its stroke to draw.
        let stroked = painted(&line(&[(0.0, 0.0), (100.0, 0.0)]), false, Some(3.0 * 72.0 / 25.4));
        let column = shown_auto(&stroked, "auto");
        assert_eq!(level(&column), Some(Level::Auto));
        assert!(!closed(&column).is_empty() && closed(&column).iter().all(|closed| !closed), "{column:?}");
        // The same line with a fill too: its stitches as ribbons, which the fill fills, after the area's rows.
        let both = shown_auto(&painted(&line(&[(0.0, 0.0), (100.0, 0.0)]), true, Some(3.0 * 72.0 / 25.4)), "auto");
        assert!(closed(&both).iter().all(|closed| *closed), "{both:?}");
        // Exact shows every stitch, and the outline the object's path, hatched across a fill every 3 mm.
        assert_eq!((level(&shown_auto(&filled, "exact")), level(&shown_auto(&stroked, "exact"))), (Some(Level::Exact), Some(Level::Exact)));
        let outline = shown_auto(&filled, "outline");
        assert_eq!((level(&outline), closed(&outline).len()), (Some(Level::Outline), 4 + 5));
        assert_eq!(closed(&shown_auto(&stroked, "outline")), [false]);
    }

    #[test]
    fn req_vc_016_auto_shows_the_outline_of_what_costs_more_than_the_budget_allows() {
        // A filled square 90 cm a side, whose rows cost more than auto's share.
        let filled = painted(&square(2560.0), true, None);
        assert_eq!(level(&shown_auto(&filled, "auto")), Some(Level::Outline));
        // With nothing to spend, auto says why it shows nothing, and names no cheaper level.
        let params = auto_params(&[]);
        let effect = read_auto_effect(&filled, &params, &Budget::DEFAULT).unwrap();
        let said: Value = serde_json::from_slice(&show_auto(&effect, 0)).unwrap();
        let start = "info SC-I0810: Its auto preview needs more work than VectorCraft lets a live effect do";
        assert!(said["error"].as_str().is_some_and(|error| error.starts_with(start)), "{said}");
        let text = |level: Level| too_costly(level, AUTO_LEVELS).fix.map(|fix| fix.describe()).unwrap_or_default();
        assert_eq!(text(Level::Exact), "Set its preview level to `outline`.");
    }

    #[test]
    fn spending_is_shared_out_in_tenths_and_priced_in_units_and_anchors() {
        let spend = Spend(32_000);
        assert_eq!(spend.tenths(9), Spend(28_800));
        assert_eq!(Spend(u64::MAX).tenths(9), Spend(u64::MAX / 10));
        assert_eq!(spend.work(3_200), Budget { max_work: 10, max_stitches: u32::MAX });
        // 16 anchors are 8,000 instructions to write.
        assert!(Spend(8_000).writes(4, RIBBON_ANCHORS) && !Spend(7_999).writes(4, RIBBON_ANCHORS));
        assert!(Spend(8_000).writes(16, 1) && !Spend(7_999).writes(16, 1));
        assert!(!Spend(u64::MAX - 1).writes(usize::MAX, RIBBON_ANCHORS), "saturating, not wrapping");
        assert_eq!(reading(320_000), Budget { max_work: 10, max_stitches: u32::MAX });
        assert_eq!((anchors(Family::Fill), anchors(Family::Stroke), anchors(Family::Satin)), (4, 1, 1));
        // The Auto effect's elements share eight tenths of what digitizing leaves equally.
        assert_eq!((spend.shared(4), spend.shared(1), spend.shared(0)), (Spend(6_400), Spend(25_600), Spend(25_600)));
        // A run of 3 needle points is 2 ribbons, or 3 anchors of a line.
        let run = |n: u32| (0..n).map(|x| Point::new(f64::from(x), 0.0).unwrap()).collect::<Vec<_>>();
        assert_eq!((to_write(&[run(3), run(1)], true), to_write(&[run(3), run(1)], false)), ((2, RIBBON_ANCHORS), (4, 1)));
    }

    #[test]
    fn a_level_shows_nothing_whose_work_or_writing_its_share_does_not_pay_for() {
        let fill = read_effect(Family::Fill, &input(&square(1600.0)), &params_with(Family::Fill, "rows"), &Budget::DEFAULT).unwrap().element;
        // Its rows take 11,357 units of work, which nine tenths of 40.4 million instructions pay for, and
        // writing their 2,258 segments 4.5 million, a tenth of 45.2 million.
        assert_eq!(fill_rows(&fill, Spend(43_000_000)), Shown::TooCostly);
        assert_eq!(level(&fill_rows(&fill, Spend(46_000_000))), Some(Level::Rows));
        // An outline that cannot even flatten the path.
        assert_eq!(level(&outline(Family::Fill, &fill, Spend(0))), None);
    }

    #[test]
    fn a_unit_of_work_is_priced_above_what_the_contract_tests_measured() {
        // 880 instructions measured for a stroke's, 4,730 for a satin column's (`compat/vc-contract`).
        assert!(per_unit(Family::Stroke).is_some_and(|price| price > 880));
        assert!(per_unit(Family::Satin).is_some_and(|price| price > 4_730));
        // From 1,900 to 5,300 for a fill's, by the part of the work: no one price.
        assert_eq!(per_unit(Family::Fill), None);
        // Digitizing's, from 1,450 for a filled square to 2,260 for a line traced as an area 0.3 mm wide.
        const { assert!(DIGITIZE_PER_UNIT > 2_260 && DIGITIZE_PER_UNIT < 2 * 2_260) };
        // Above, but not far above: a price twice what was measured would show half the stitches it could.
        assert!(per_unit(Family::Stroke).is_some_and(|price| price < 2 * 880));
        assert!(per_unit(Family::Satin).is_some_and(|price| price < 2 * 4_730));
    }

    #[test]
    fn a_level_is_read_from_its_id_and_auto_from_anything_else() {
        for level in [Level::Auto, Level::Exact, Level::Rows, Level::Outline] {
            assert_eq!(Level::read(Some(level.id())), level);
        }
        assert_eq!((Level::read(None), Level::read(Some("sketch"))), (Level::Auto, Level::Auto));
    }

    #[test]
    fn what_an_effect_may_spend_is_vectorcraft_s_allowance_less_reading_and_setting_up() {
        assert_eq!(spendable(FUEL_BASE, 1_000, 2_000), FUEL_BASE + FUEL_PER_BYTE * 1_000 - READ_PER_BYTE * 3_000 - SETUP);
        assert_eq!(spendable(HEAVY_FUEL, 1_000, 2_000), HEAVY_FUEL + FUEL_PER_BYTE * 1_000 - READ_PER_BYTE * 3_000 - SETUP);
        assert_eq!(spendable(FUEL_BASE, 0, usize::MAX), 0);
        // Saturating, never overflowing: reading as many bytes as there are costs all there is.
        assert_eq!(spendable(FUEL_BASE, usize::MAX, 0), 0);
    }

    #[test]
    fn the_settings_the_outline_sets_are_the_registry_s() {
        let kind = |key: &str| stitchcraft_params::find_for(PARAMETERS, key, Family::Fill).map(|spec| spec.kind);
        for key in [ROW_SPACING, END_ROW_SPACING] {
            assert!(matches!(kind(key), Some(Kind::Length { .. })), "{key}");
        }
    }

    #[test]
    fn why_nothing_is_sewn_is_the_most_serious_of_what_was_said_and_the_first_of_those() {
        let said = vec![
            Diagnostic::new(Code::VcNotPrinted, "a"),
            Diagnostic::new(Code::VcGeometryUnusable, "b"),
            Diagnostic::new(Code::VcUnreadable, "c"),
            Diagnostic::new(Code::BudgetExhausted, "d"),
        ];
        assert_eq!(no_stitch(said).message, "c");
        assert_eq!(no_stitch(Vec::new()).code, Code::VcEffectUnchanged);
    }
}
