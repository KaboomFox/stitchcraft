//! What a live effect shows, within what VectorCraft lets it spend.
//!
//! **The budget.** VectorCraft lets a live effect run 50 million instructions, and 4,000 more for each byte
//! of its input, and stops it there ([`FUEL_BASE`], [`FUEL_PER_BYTE`]). The engine counts its own work in
//! units, and the contract tests measure what a unit costs in VectorCraft's host
//! (`compat/vc-contract`): a few hundred to a few thousand instructions, by the kind of work
//! ([`per_unit`]). So the effect turns the instructions it has left, after reading its input, into a
//! budget of units and of needle points for the engine. When the engine runs out of one, the effect shows
//! less, rather than being stopped by VectorCraft.
//!
//! **Levels** (`preview_level`, [`Level`]). *Exact* shows every stitch. *Rows* shows a fill's top rows,
//! one ribbon for each row segment, which costs a small part of sewing it: where its stitches lie and
//! which way they run, without the stitches, the underlay or the needle's travel. *Outline* shows the
//! object's path, as lines for a stroke or a satin column and as ribbons for a fill, with the fill's
//! direction hatched across it every few millimetres. *Auto*, the default, shows the most the budget
//! allows: exact, then rows for a fill, then the outline, each given its share of the budget, so that the
//! next always has what it needs when one runs out. A level the user picks gets the whole budget, and
//! when that is not enough the effect says which level to pick instead.
//!
//! **Where the numbers come from.** Spike M0.6 measured reading JSON at 512 instructions a byte. M6.3
//! measured the rest on the shapes the contract tests run, every one in VectorCraft's host, and each
//! constant here is the largest it measured, with a margin. The contract tests fail when a level takes
//! more than VectorCraft allows, which is when a constant needs measuring again
//! (`docs/src/design/vectorcraft-integration.md`, live-effect previews).

use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generate::{generate, rows};
use stitchcraft_engine::generators::Neighbours;
use stitchcraft_engine::normalize::satin::{Shape as Column, recognize};
use stitchcraft_engine::normalize::stroke::{SHAPE_TOLERANCE, flatten};
use stitchcraft_params::Family;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_vectorcraft::{Effect, effect_error, effect_lines, effect_output, effect_ribbons};

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
/// What writing one anchor costs: 16 instructions a byte measured, and about 22 bytes an anchor.
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

/// The instructions a live effect has for its work, given `input` and `params` bytes to read: what
/// VectorCraft allows, less reading and setting up.
pub fn spendable(input: usize, params: usize) -> u64 {
    let bytes = |n: usize| u64::try_from(n).unwrap_or(u64::MAX);
    FUEL_BASE
        .saturating_add(FUEL_PER_BYTE.saturating_mul(bytes(input)))
        .saturating_sub(READ_PER_BYTE.saturating_mul(bytes(input).saturating_add(bytes(params))))
        .saturating_sub(SETUP)
}

/// What a level showed.
enum Shown {
    /// The geometry it draws.
    Drawn(Vec<u8>),
    /// The object sews nothing, or cannot be sewn: why.
    Unsewn(Diagnostic),
    /// It needed more than its share of the budget.
    TooCostly,
}

/// The preview of `effect`, of `family`, within `spend` instructions: the geometry VectorCraft draws, or
/// why it draws the object as it is.
pub fn show(family: Family, effect: &Effect, spend: u64) -> Vec<u8> {
    let element = &effect.element;
    if let Some(unseen) = centre_line(family, element, spend) {
        return effect_error(&unseen);
    }
    let level = Level::read(effect.preview_level.as_deref());
    let tenths = |n: u64| spend.saturating_mul(n) / 10;
    let shown = match (level, family) {
        (Level::Exact, _) => exact(family, element, spend),
        (Level::Rows, Family::Fill) => fill_rows(element, spend),
        // A stroke's and a satin column's previews offer no rows.
        (Level::Outline | Level::Rows, _) => outline(family, element, spend),
        (Level::Auto, Family::Fill) => {
            exact(family, element, tenths(7)).or(|| fill_rows(element, tenths(2))).or(|| outline(family, element, tenths(1)))
        }
        (Level::Auto, Family::Stroke | Family::Satin) => exact(family, element, tenths(9)).or(|| outline(family, element, tenths(1))),
    };
    match shown {
        Shown::Drawn(json) => json,
        Shown::Unsewn(why) => effect_error(&why),
        Shown::TooCostly => effect_error(&too_costly(level, family)),
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

/// Every stitch of `element`, of `family`, within `spend` instructions: nine tenths for the engine's work
/// and a tenth for writing the needle points.
fn exact(family: Family, element: &Element, spend: u64) -> Shown {
    let anchors = if family == Family::Fill { 4 } else { 1 };
    let budget = Budget { max_work: spend / 10 * 9 / per_unit(family, element), max_stitches: points(spend / 10 / (PER_ANCHOR * anchors)) };
    let generation = generate(element, &DesignSettings::default(), REFERENCE, &Neighbours::default(), &budget);
    if spent(&generation.diagnostics) {
        return Shown::TooCostly;
    }
    let groups = generation.generated.map(|generated| generated.groups).unwrap_or_default();
    effect_output(family, &groups).map_or_else(|| Shown::Unsewn(no_stitch(generation.diagnostics)), Shown::Drawn)
}

/// The top rows of `element`, a fill, within `spend` instructions, a ribbon for each segment.
fn fill_rows(element: &Element, spend: u64) -> Shown {
    let budget = Budget { max_work: spend / 10 * 9 / ROWS_PER_UNIT, max_stitches: u32::MAX };
    let laid = rows(element, &DesignSettings::default(), REFERENCE, &budget);
    let Some(segments) = laid.segments.filter(|_| !spent(&laid.diagnostics)) else {
        return if spent(&laid.diagnostics) { Shown::TooCostly } else { Shown::Unsewn(no_stitch(laid.diagnostics)) };
    };
    if u64::try_from(segments.len()).unwrap_or(u64::MAX).saturating_mul(4 * PER_ANCHOR) > spend / 10 {
        return Shown::TooCostly;
    }
    effect_ribbons(segments.iter().map(|segment| (segment.start, segment.end)))
        .map_or_else(|| Shown::Unsewn(no_stitch(laid.diagnostics)), Shown::Drawn)
}

/// The path of `element`, of `family`, within `spend` instructions: a stroke's and a satin column's as
/// lines, a fill's as ribbons, hatched every [`HATCH_MM`] in the fill's direction when that fits too.
fn outline(family: Family, element: &Element, spend: u64) -> Shown {
    let mut meter = Budget { max_work: spend / 2 / ROWS_PER_UNIT, max_stitches: u32::MAX }.meter();
    let Ok(flat) = flatten(element.shape.path(), SHAPE_TOLERANCE, &mut meter) else { return Shown::TooCostly };
    let lines = flat.pieces.iter().map(|piece| piece.points.as_slice());
    let drawn = match family {
        Family::Stroke | Family::Satin => effect_lines(lines),
        Family::Fill => {
            let edges = lines.flat_map(|line| line.windows(2)).filter_map(|w| Some((*w.first()?, *w.get(1)?)));
            let mut hatched = element.clone();
            hatched.params.set("row_spacing_mm", HATCH_MM.to_string());
            hatched.params.set("end_row_spacing_mm", "");
            let budget = Budget { max_work: spend / 4 / ROWS_PER_UNIT, max_stitches: u32::MAX };
            let hatch = rows(&hatched, &DesignSettings::default(), REFERENCE, &budget).segments.unwrap_or_default();
            effect_ribbons(edges.chain(hatch.iter().map(|segment| (segment.start, segment.end))))
        }
    };
    drawn.map_or(Shown::TooCostly, Shown::Drawn)
}

/// The instructions the engine runs for each unit of work it charges, sewing an element of `family`,
/// measured in VectorCraft's host with a margin: a fill's travel under its rows (`underpath`) is work at
/// about 1,900, and the rest of a fill's at up to 2,800.
fn per_unit(family: Family, element: &Element) -> u64 {
    match family {
        Family::Stroke => 1_100,
        Family::Satin => 5_500,
        Family::Fill if element.params.get("underpath") == Some("false") => 3_200,
        Family::Fill => 2_200,
    }
}

/// The instructions for each unit of work laying a fill's rows, reading or flattening a path.
const ROWS_PER_UNIT: u64 = 3_200;

/// The budget for reading a live effect's object, with `spend` instructions: a tenth of them.
pub fn reading(spend: u64) -> Budget {
    Budget { max_work: spend / 10 / ROWS_PER_UNIT, max_stitches: u32::MAX }
}

/// `SC-I0810` when `element`, of `family`, is a satin column drawn as one path, its centre line: the
/// column is as wide as its stroke, which VectorCraft does not give a live effect. Telling takes a tenth
/// of `spend`; a path too long to tell within it is sewn, and the levels' budgets bound that.
fn centre_line(family: Family, element: &Element, spend: u64) -> Option<Diagnostic> {
    let (Family::Satin, Shape::Stroke { path, .. }) = (family, &element.shape) else { return None };
    let Ok(Column::CentreLine { .. }) = recognize(path, &mut reading(spend).meter()).ok()?.shape else { return None };
    let message = "This satin column is drawn as one path, so it is as wide as its stroke, which VectorCraft does not give a live effect: \
                   the effect leaves it as it is.";
    Some(Diagnostic::new(Code::VcEffectUnchanged, message).with_fix(Fix::Hint("To see its stitches in VectorCraft, draw it with 2 rails.".into())))
}

/// `n` needle points, at most as many as a budget counts.
fn points(n: u64) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
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
    use super::*;

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
