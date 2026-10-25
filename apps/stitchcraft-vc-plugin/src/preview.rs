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
//! **Where the numbers come from.** Spike M0.6 measured reading JSON at 512 instructions a byte, and M6.3
//! the rest, in VectorCraft's host, on the shapes the contract tests run. Each constant here is the
//! largest measured, with a margin. The contract tests fail when a level takes more than VectorCraft
//! allows, which is when a constant needs measuring again (`docs/src/design/vectorcraft-integration.md`,
//! live-effect previews).

use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generate::{generate, rows, stitch_type_of};
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_engine::normalize::satin::{Shape as Column, recognize};
use stitchcraft_engine::normalize::stroke::{SHAPE_TOLERANCE, flatten};
use stitchcraft_params::{Family, StitchType};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::{Effect, effect_error, effect_lines, effect_output, effect_ribbons};

use crate::dialog::own;
use crate::manifest::{Param, Schema};

/// The instructions VectorCraft lets any live effect run (`Limits::default()` in its `crates/plugins`,
/// which the contract tests check).
pub const FUEL_BASE: u64 = 50_000_000;
/// The instructions VectorCraft adds for each byte of a live effect's input.
pub const FUEL_PER_BYTE: u64 = 4_000;
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

/// How a family's manifest declares `preview_level`: a choice of the levels its previews offer, auto by
/// default, in a section of its own, with its help and the page that explains the levels.
pub(crate) fn param(family: Family) -> Param {
    let schema = Schema::Choice { options: Level::offered(family).iter().map(|level| level.id()).collect(), default: Level::Auto.id() };
    let mut param = own(schema, "Preview", HELP, PAGE);
    param.ui.group = Some("Preview");
    param
}

/// What `preview_level` does, for its help.
const HELP: &str = "How much of the stitches the effect shows. Exact shows every stitch, rows a fill's top rows, and outline the \
                    object's path, hatched across a fill, which costs least. Auto shows as much as VectorCraft lets a live effect \
                    work out. The machine file has every stitch, whichever is chosen.";

/// The page that explains the levels.
const PAGE: &str = "design/vectorcraft-integration.html#live-effect-previews-within-v1-budgets";

/// The instructions a live effect has for its work, given `input` and `params` bytes to read: what
/// VectorCraft allows, less reading and setting up.
pub fn spendable(input: usize, params: usize) -> u64 {
    let bytes = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    FUEL_BASE
        .saturating_add(FUEL_PER_BYTE.saturating_mul(bytes(input)))
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
    match preview(family, effect, spend) {
        Shown::Drawn(_, json) => json,
        Shown::Unsewn(why) => effect_error(&why),
        Shown::TooCostly => effect_error(&too_costly(Level::read(effect.preview_level.as_deref()), family)),
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
    let generation = generate(element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &budget);
    if spent(&generation.diagnostics) {
        return Shown::TooCostly;
    }
    let groups = generation.generated.map(|generated| generated.groups).unwrap_or_default();
    // A stroke's and a satin column's needle points are an anchor each. Sewing them does not count them.
    if priced.is_some_and(|(spend, _)| !spend.tenths(1).writes(groups.iter().map(Vec::len).sum(), anchors(family))) {
        return Shown::TooCostly;
    }
    effect_output(family, &groups).map_or_else(|| Shown::Unsewn(no_stitch(generation.diagnostics)), |json| Shown::Drawn(Level::Exact, json))
}

/// The top rows of `element`, a fill, within `spend`, a ribbon for each segment: nine tenths for laying
/// them, and a tenth for writing them.
fn fill_rows(element: &Element, spend: Spend) -> Shown {
    let laid = rows(element, &DesignSettings::default(), REFERENCE, &spend.tenths(9).work(ROWS_PER_UNIT));
    let Some(segments) = laid.segments.filter(|_| !spent(&laid.diagnostics)) else {
        return if spent(&laid.diagnostics) { Shown::TooCostly } else { Shown::Unsewn(no_stitch(laid.diagnostics)) };
    };
    if !spend.tenths(1).writes(segments.len(), RIBBON_ANCHORS) {
        return Shown::TooCostly;
    }
    effect_ribbons(segments.iter().map(|segment| (segment.start, segment.end)))
        .map_or_else(|| Shown::Unsewn(no_stitch(laid.diagnostics)), |json| Shown::Drawn(Level::Rows, json))
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
/// the cheaper levels to pick instead. Auto has tried them all.
fn too_costly(level: Level, family: Family) -> Diagnostic {
    let cheaper: Vec<&str> = match level {
        Level::Auto => Vec::new(),
        _ => Level::offered(family).iter().skip_while(|offered| **offered != level).skip(1).map(|offered| offered.id()).collect(),
    };
    let message = format!("Its {} preview needs more work than VectorCraft lets a live effect do, so the effect leaves it as it is.", level.id());
    let hint = match cheaper.as_slice() {
        [] => "The command line's preview shows its stitches.".to_string(),
        [one] => format!("Set its preview level to `{one}`."),
        more => format!("Set its preview level to `{}`.", more.join("` or `")),
    };
    Diagnostic::new(Code::VcEffectUnchanged, message).with_fix(Fix::Hint(hint))
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};
    use stitchcraft_engine::registry::PARAMETERS;
    use stitchcraft_params::Kind;
    use stitchcraft_vectorcraft::read_effect;

    use super::*;
    use crate::testing::{input, params, rails, square};

    /// `family`'s parameters at their defaults, at preview `level`.
    fn params_with(family: Family, level: &str) -> Vec<u8> {
        params(family, &[("preview_level", json!(level))])
    }

    /// What `family`'s effect shows of `path` at `level`, with what VectorCraft would let it spend.
    fn shown(family: Family, path: &Value, level: &str) -> Shown {
        let (input, params) = (input(path), params_with(family, level));
        let effect = read_effect(family, &input, &params, &Budget::DEFAULT).unwrap();
        preview(family, &effect, spendable(input.len(), params.len()))
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
        let said = |family: Family, level: Level| effect_error(&too_costly(level, family));
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
        assert_eq!(text(show(Family::Fill, &effect, spendable(input.len(), params.len()))), text(said(Family::Fill, Level::Rows)));
        let auto = read_effect(Family::Fill, &input, &params_with(Family::Fill, "auto"), &Budget::DEFAULT).unwrap();
        assert_eq!(text(show(Family::Fill, &auto, 0)), text(said(Family::Fill, Level::Auto)));
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
        assert_eq!(spendable(1_000, 2_000), FUEL_BASE + FUEL_PER_BYTE * 1_000 - READ_PER_BYTE * 3_000 - SETUP);
        assert_eq!(spendable(0, usize::MAX), 0);
        // Saturating, never overflowing: reading as many bytes as there are costs all there is.
        assert_eq!(spendable(usize::MAX, 0), 0);
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
