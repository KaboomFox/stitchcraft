//! The rules that choose the stitches of an automatic shape from its geometry, as one table ([`RULES`],
//! `REQ-AUTO-030`), and the one place a bound is compared (`choose`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Which stitch a shape gets. Width alone chose the
//! stitch before M13.21: a blob as wide as a satin column sewed as a column bent into its corners, and a
//! ragged patch of a traced photo as several columns that fan where they meet. The choice was also spread
//! over the code that sorts a stroke, a part, a part's columns, a part's broad places and its holes, each
//! with a comparison of its own. Here it is one function of a part's measures and its shape's
//! bounds, so that the docs, the notice that names the rule a part sewed by (`SC-I0912`) and the tests at
//! each bound come from one table, and a rule can't be changed in one place and not in another.
//!
//! The rules ask only about geometry: how large a part is, how wide, how long for its width, how evenly
//! wide, how many centre lines it has, and how far it reaches from them. None asks what a picture shows, so a flower, a drawing and an
//! animal go by the same table.
//!
//! - **The bounds** are the shape's settings ([`AutoParams`]) and the design's narrowest satin stroke. One
//!   choice leans 3 of them together towards satin columns or towards fills ([`LEAN`]).
//! - **Ties.** A length is compared to the micrometre and a ratio to its third decimal, so that a value
//!   written in another unit, which converts to a bound give or take a rounding error, is that bound. A part
//!   as wide as a bound falls on its narrower side, one exactly as slender as the least slenderness is a
//!   column, and one whose width varies by exactly the most it may is even.
//! - **Asked.** A shape that names its stitch (`auto_stitch`) sews so where it can, and by the table where
//!   it can't.

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Meter, Mm};

use super::neighbours::speck;
use super::params::{
    FILL_UNEVEN, LONGEST_SATIN_STITCH, MAX_REACH, MAX_SATIN_WIDTH, MAX_SPLIT_SATIN_WIDTH, MAX_UNEVENNESS, MIN_AREA, MIN_FILL_WIDTH, MIN_SLENDERNESS,
    MIN_TRIPLE_RUN_WIDTH, PREFER, STITCH,
};
use super::{AutoParams, micrometres, narrow};
use crate::design::DesignSettings;
use crate::generators::mm;
use crate::normalize::region::Polygon;

/// The most of one rail of a satin column, in millimetres, that its stitches may sew to a millimetre of the
/// other. A straight column sews 1 mm to 1 mm and lays 1.25 fills of thread, so at 2 mm the shorter rail
/// has 2.5 fills of the column's own thread, under the 3 that planning warns at (`REQ-AUTO-062`). On the 4
/// traced dogs, the satin columns of small ragged patches sewed up to 6.6 fills by themselves where their
/// stitches met at one end of a fan (`docs/src/design/image-to-embroidery.md` › Less thread where a design
/// sews thick).
pub const MOST_CROWDING: f64 = 2.0;

/// How much wider than the widest split satin column (`auto_max_split_satin_width_mm`) a part or a line may
/// be and still sew as one, in millimetres: 50 micrometres, an eighth of the thread's width, which no stitch
/// can show. A part drawn as wide as the bound measures a few micrometres over it where the points of its
/// outline fall: a ring drawn 10 mm wide with 180 points a side sews a stitch 10.003 mm long between its
/// rails. Without it, a person who sets the bound to the width they drew would see the part filled.
pub const WIDE_TOLERANCE: f64 = 0.05;

/// Whether a satin column that sews `most` millimetres of one rail to a millimetre of the other crowds its
/// stitches, so that the column is refused: more than [`MOST_CROWDING`], to the micrometre.
pub(crate) fn crowds(most: f64) -> bool {
    micrometres(most) > micrometres(MOST_CROWDING)
}

/// A rule of the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuleId {
    /// The shape names its stitch, and the part can sew so.
    Asked,
    /// The part is smaller than the smallest area.
    Speck,
    /// The part is nowhere wider than the narrowest satin stroke.
    Line,
    /// The part is wider than the widest satin column somewhere, no wider than the widest split satin column
    /// anywhere, and a column of one centre line and even width: it sews as one satin column whose stitches
    /// are split.
    Wide,
    /// The part is wider than the widest satin column somewhere.
    Area,
    /// The part is short for its width: a blob.
    Compact,
    /// The part has several centre lines, and its width varies along them.
    Uneven,
    /// The part reaches far from one of its centre lines on both sides of it: it has arms that no line follows.
    Arms,
    /// Any other part: it sews as satin columns.
    Column,
    /// A column that can't be drawn, or that would sew a stitch longer than the widest satin column unless its
    /// stitches are split.
    Refused,
}

impl RuleId {
    /// The word for the rule in `SC-I0912` and the docs.
    pub const fn key(self) -> &'static str {
        match self {
            RuleId::Asked => "asked",
            RuleId::Speck => "speck",
            RuleId::Line => "line",
            RuleId::Wide => "wide",
            RuleId::Area => "area",
            RuleId::Compact => "compact",
            RuleId::Uneven => "uneven",
            RuleId::Arms => "arms",
            RuleId::Column => "column",
            RuleId::Refused => "refused",
        }
    }
}

/// A row of the table: what a part is, and what it sews as. A `{key}` in a text stands for the default of
/// the setting with that key, with its unit, `{narrowest}` for the design's narrowest satin stroke and
/// `{longest}` for the longest satin stitch ([`filled`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Which rule.
    pub id: RuleId,
    /// Its name in the docs' table.
    pub label: &'static str,
    /// What the part is, completing "The part is …".
    pub is: &'static str,
    /// What it sews as.
    pub sewn: &'static str,
    /// The keys of the settings that its texts name.
    pub bounds: &'static [&'static str],
}

/// The rules, in the order they are tried: the first that a part matches decides.
pub const RULES: [Rule; 10] = [
    Rule {
        id: RuleId::Asked,
        label: "Asked",
        is: "of a shape whose `auto_stitch` names a stitch, and can sew so",
        sewn: "that stitch: a running stitch along its centre lines, satin columns along them, or a tatami fill of the whole shape",
        bounds: &[STITCH],
    },
    Rule {
        id: RuleId::Speck,
        label: "Speck",
        is: "smaller than {auto_min_area_mm2}",
        sewn: "merged into the neighbour it shares the longest edge with, or left out when it shares none",
        bounds: &[MIN_AREA],
    },
    Rule {
        id: RuleId::Line,
        label: "Line",
        is: "nowhere wider than the design's narrowest satin stroke, {narrowest}",
        sewn: "a running stitch along its centre lines, a triple run from {auto_min_triple_run_width_mm} wide",
        bounds: &[MIN_TRIPLE_RUN_WIDTH],
    },
    Rule {
        id: RuleId::Wide,
        label: "Wide",
        is: "wider than {auto_max_satin_width_mm} somewhere and no wider than the widest split satin column anywhere, once a person raises \
             it from its default, {auto_max_split_satin_width_mm}, at which no part is wide. It has one centre line along which its width \
             varies by no more than {auto_max_unevenness} of itself, and is a column by the rules below",
        sewn: "one satin column whose stitches split into pieces no longer than {longest}, at random places",
        bounds: &[MAX_SPLIT_SATIN_WIDTH, MAX_UNEVENNESS],
    },
    Rule {
        id: RuleId::Area,
        label: "Area",
        is: "wider than {auto_max_satin_width_mm} somewhere",
        sewn: "a tatami fill. A piece of it narrower than {auto_min_fill_width_mm} leaves the fill when the rules below make it a line or a column, \
               unless it is one line whose width varies by more than {auto_max_unevenness} of itself",
        bounds: &[MAX_SATIN_WIDTH, MIN_FILL_WIDTH, MAX_UNEVENNESS],
    },
    Rule {
        id: RuleId::Compact,
        label: "Compact",
        is: "smaller in area than {auto_min_slenderness} times its width squared",
        sewn: "a tatami fill",
        bounds: &[MIN_SLENDERNESS, PREFER],
    },
    Rule {
        id: RuleId::Uneven,
        label: "Uneven",
        is: "of several centre lines, or of one at least {auto_min_fill_width_mm} wide somewhere, with a width that varies along them by more \
             than {auto_max_unevenness} of itself",
        sewn: "a tatami fill. A piece of a part of several lines narrower than the narrowest satin stroke leaves the fill as a line, and a part \
               of one line is filled whole",
        bounds: &[MAX_UNEVENNESS, MIN_FILL_WIDTH, FILL_UNEVEN, PREFER],
    },
    Rule {
        id: RuleId::Arms,
        label: "Arms",
        is: "reaching from one of its centre lines, on both sides of it, more than {auto_max_reach} times as far as the widest circle that fits \
             inside it on its lines",
        sewn: "a tatami fill",
        bounds: &[MAX_REACH],
    },
    Rule { id: RuleId::Column, label: "Column", is: "anything else", sewn: "satin columns along its centre lines", bounds: &[] },
    Rule {
        id: RuleId::Refused,
        label: "Refused",
        is: "a column that can't be drawn, that would sew a stitch longer than {auto_max_satin_width_mm} between its rails and isn't wide, \
             or that would sew more than 2 mm of one rail to 1 mm of the other",
        sewn: "a tatami fill",
        bounds: &[MAX_SATIN_WIDTH],
    },
];

/// `text`, a rule's, with each `{key}` replaced by the default of the setting with that key and its unit,
/// `{narrowest}` by the default of the design's narrowest satin stroke and `{longest}` by the longest satin
/// stitch.
pub fn filled(text: &str) -> String {
    let mut filled = text
        .replace("{narrowest}", &format!("{} mm", mm(DesignSettings::MIN_SATIN_STROKE_WIDTH.get())))
        .replace("{longest}", &format!("{} mm", mm(LONGEST_SATIN_STITCH)));
    for spec in AutoParams::SPECS {
        let value = format!("{} {}", spec.default, spec.kind.unit());
        filled = filled.replace(&format!("{{{}}}", spec.key), value.trim_end());
    }
    filled
}

/// A named set of values of the rules' settings: a style of digitizing, which `stitch plan --style` sets on
/// every automatic shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Preset {
    /// Its name on the command line.
    pub name: &'static str,
    /// What it is for.
    pub help: &'static str,
    /// The settings it sets, each with its value.
    pub values: &'static [(&'static str, &'static str)],
}

/// The presets. Each names values of the settings that lean the rules, and no rule of its own.
pub const PRESETS: [Preset; 2] = [
    Preset {
        name: "logo",
        help: "Flat art drawn in strokes, such as a logo, lettering or an ink drawing: more parts sew as satin columns, a part of \
               several strokes keeps its columns though its width varies, and a thin line sews as a line.",
        values: &[(PREFER, "satin"), (FILL_UNEVEN, "false")],
    },
    Preset {
        name: "photo",
        help: "A traced photo, whose shapes are patches and not strokes: the rules as they are by default, which fill a ragged patch \
               and merge a ragged thin line into the patch beside it.",
        values: &[(PREFER, "balanced"), (FILL_UNEVEN, "true")],
    },
];

/// By how much `auto_prefer` moves the least slenderness, the most a width varies and the narrowest piece of
/// a fill together: `satin` divides the first by it and multiplies the other 2, and `fill` does the reverse.
pub const LEAN: f64 = 1.5;

/// The stitch a shape asks for (`auto_stitch`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Asked {
    /// None: the rules choose.
    Auto,
    /// A running stitch along each part's centre lines.
    Run,
    /// Satin columns along them.
    Satin,
    /// A tatami fill of the whole shape.
    Fill,
}

impl Asked {
    /// The stitch that the setting's value `id` names: the rules' choice for `auto`, and for any other.
    fn named(id: &str) -> Asked {
        match id {
            "run" => Asked::Run,
            "satin" => Asked::Satin,
            "fill" => Asked::Fill,
            _ => Asked::Auto,
        }
    }
}

/// The bounds a shape's parts are held to: its settings of the rules, leaned as it prefers, and its design's
/// narrowest satin stroke.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Bounds {
    /// The design's narrowest satin stroke: a part nowhere wider is a line.
    pub(crate) narrowest: Mm,
    /// The widest satin column, and the longest stitch one sews between its rails unless they are split.
    pub(crate) max_satin: Mm,
    /// The widest split satin column, when it is wider than the longest satin stitch: `None` when no part is
    /// wide, as by default.
    pub(crate) max_split: Option<Mm>,
    /// Whether the columns' stitches are split, so that no stitch between their rails is too long: a wide
    /// part's columns ([`Bounds::wide`]).
    pub(crate) splits: bool,
    /// The narrowest piece that stays in a fill, in millimetres: no wider than the widest satin column.
    pub(crate) min_fill: f64,
    /// The least slenderness of a part sewn as columns.
    pub(crate) min_slenderness: f64,
    /// The most the width of a part of several lines varies, as a share of it.
    pub(crate) max_unevenness: f64,
    /// Whether a part more uneven is filled.
    pub(crate) fill_uneven: bool,
    /// The farthest a part sewn as columns reaches from a centre line on both sides, in radii of its widest
    /// circle.
    pub(crate) max_reach: f64,
    /// The smallest area, in square millimetres.
    pub(crate) min_area: f64,
    /// The stitch the shape asks for.
    pub(crate) asked: Asked,
}

impl Bounds {
    /// The bounds of a shape whose settings of the rules are `rules`, in a design whose narrowest satin stroke
    /// is `narrowest`.
    pub(crate) fn of(rules: &AutoParams, narrowest: Mm) -> Bounds {
        // How far each of the 3 bounds leans: towards columns, not at all, or towards fills.
        let (slender, uneven) = match rules.auto_prefer {
            "satin" => (1.0 / LEAN, LEAN),
            "fill" => (LEAN, 1.0 / LEAN),
            _ => (1.0, 1.0),
        };
        Bounds {
            narrowest,
            max_satin: rules.auto_max_satin_width_mm,
            max_split: Some(rules.auto_max_split_satin_width_mm).filter(|split| micrometres(split.get()) > micrometres(LONGEST_SATIN_STITCH)),
            splits: false,
            min_fill: (rules.auto_min_fill_width_mm.get() * uneven).min(rules.auto_max_satin_width_mm.get()),
            min_slenderness: rules.auto_min_slenderness * slender,
            max_unevenness: rules.auto_max_unevenness * uneven,
            fill_uneven: rules.auto_fill_uneven,
            max_reach: rules.auto_max_reach,
            min_area: rules.auto_min_area_mm2,
            asked: Asked::named(rules.auto_stitch),
        }
    }

    /// The bounds a part's columns are held to when it is tried as a wide part: these, with the widest satin
    /// column the widest split satin column and its tolerance ([`WIDE_TOLERANCE`]), and stitches that split.
    /// `None` when no part is wide.
    pub(crate) fn wide(&self) -> Option<Bounds> {
        let max_split = self.max_split?;
        let max_satin = Mm::new(max_split.get() + WIDE_TOLERANCE).ok()?;
        Some(Bounds { max_satin, max_split: None, splits: true, ..*self })
    }

    /// Whether a line, or a part, `width` millimetres wide with `lines` centre lines along which its width
    /// varies by `unevenness` of itself is a wide column: wider than the widest satin column, no wider than
    /// the widest split satin column and its tolerance, each to the micrometre, of one line, and as even as a
    /// column of several lines is held to.
    pub(crate) fn is_wide(&self, width: f64, lines: usize, unevenness: f64) -> bool {
        self.max_split.is_some_and(|split| {
            micrometres(width) > micrometres(self.max_satin.get())
                && micrometres(width) <= micrometres(split.get() + WIDE_TOLERANCE)
                && self.one_even_line(lines, unevenness)
        })
    }

    /// Whether a part of `lines` centre lines along which its width varies by `unevenness` of itself has one
    /// line and is as even as a column of several lines is held to, as a wide part is (`REQ-AUTO-153`).
    pub(crate) fn one_even_line(&self, lines: usize, unevenness: f64) -> bool {
        lines == 1 && thousandths(unevenness) <= thousandths(self.max_unevenness)
    }

    /// Whether a part of `lines` centre lines along which its width varies by `unevenness` of itself has one
    /// line and is more uneven than a column of several lines may be: a leaf, a lens, or a piece that falls to
    /// a point (`REQ-AUTO-174`, `REQ-AUTO-175`).
    pub(crate) fn one_uneven_line(&self, lines: usize, unevenness: f64) -> bool {
        lines == 1 && thousandths(unevenness) > thousandths(self.max_unevenness)
    }

    /// How broad a stroke `width` millimetres wide is, or a part that is that wide at its widest.
    pub(crate) fn breadth(&self, width: f64) -> Breadth {
        if micrometres(width) <= micrometres(self.narrowest.get()) {
            Breadth::Narrow
        } else if micrometres(width) <= micrometres(self.max_satin.get()) {
            Breadth::Medium
        } else {
            Breadth::Broad
        }
    }

    /// How broad `part` is, with `meter`'s work: whether a circle as wide as the narrowest satin stroke fits
    /// inside it nowhere, or one as wide as the widest satin column ([`narrow::within`]). `None` when the
    /// part, shrunk, can't be joined, a bug.
    pub(super) fn breadth_of(&self, part: &Polygon, meter: &mut Meter) -> Result<Option<Breadth>, Exhausted> {
        let Some(narrow) = narrow::within(part, self.narrowest, meter)? else { return Ok(None) };
        if narrow {
            return Ok(Some(Breadth::Narrow));
        }
        let Some(column) = narrow::within(part, self.max_satin, meter)? else { return Ok(None) };
        Ok(Some(if column { Breadth::Medium } else { Breadth::Broad }))
    }

    /// Whether a satin stitch `length` millimetres long between a column's rails is longer than the widest
    /// satin column, so that the column is refused: never when its stitches are split.
    pub(crate) fn too_long(&self, length: f64) -> bool {
        !self.splits && micrometres(length) > micrometres(self.max_satin.get())
    }
}

/// How broad a part is at its widest, against its shape's bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Breadth {
    /// Nowhere wider than the narrowest satin stroke.
    Narrow,
    /// Wider than that somewhere, and nowhere wider than the widest satin column.
    Medium,
    /// Wider than the widest satin column somewhere.
    Broad,
}

/// What is measured of a part, or of a stroke, to choose its stitches.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Measures {
    /// Its area, in square millimetres.
    pub(crate) area: f64,
    /// How broad it is at its widest.
    pub(crate) breadth: Breadth,
    /// Its width: the diameter of the circles that fit inside it centred on its centre lines, averaged
    /// along them, in millimetres. 0 for a part without a centre line, and for one whose lines weren't
    /// looked for.
    pub(crate) width: f64,
    /// How much its width varies along its centre lines: the standard deviation of the width over the width.
    pub(crate) unevenness: f64,
    /// How many centre lines it has.
    pub(crate) lines: usize,
    /// How far it reaches from one of its centre lines on both sides of the line, in radii of the widest
    /// circle that fits inside it on its lines (`auto::narrow::reach`): 1 for a column, whose rails lie as far
    /// from its line as that circle reaches. 0 for a stroke, and for a part whose columns weren't looked for.
    pub(crate) reach: f64,
    /// Whether it is as wide as the narrowest piece that stays in a fill somewhere ([`Bounds::min_fill`]):
    /// measured only for a part of one line more uneven than its bounds allow ([`Bounds::one_uneven_line`]),
    /// when uneven parts are filled, and false for any other (`REQ-AUTO-174`).
    pub(crate) fill_wide: bool,
}

impl Measures {
    /// The measures of a stroke `width` millimetres wide, held to `bounds`: one line of even width, as long
    /// as it likes.
    pub(crate) fn of_stroke(width: f64, bounds: &Bounds) -> Measures {
        Measures { area: f64::INFINITY, breadth: bounds.breadth(width), width, unevenness: 0.0, lines: 1, reach: 0.0, fill_wide: false }
    }

    /// The measures of a part of `area` square millimetres that is `breadth` broad, whose centre lines
    /// weren't looked for.
    pub(crate) fn of_breadth(area: f64, breadth: Breadth) -> Measures {
        Measures { area, breadth, width: 0.0, unevenness: 0.0, lines: 0, reach: 0.0, fill_wide: false }
    }

    /// How long it is for its width: its area over its width squared. 0 for a part without a width.
    pub(crate) fn slenderness(&self) -> f64 {
        if self.width > 0.0 { self.area / (self.width * self.width) } else { 0.0 }
    }
}

/// A ratio in thousandths, as the rules compare it.
fn thousandths(ratio: f64) -> f64 {
    (ratio * 1000.0).round()
}

/// The rule that a part, or a stroke, with `measures` sews by under `bounds`: the first of the table that it
/// matches. [`RuleId::Refused`] is found later, once its columns are drawn, and so is [`RuleId::Wide`] for a
/// part of an area: only a stroke's measures are known before ([`Measures::of_stroke`]).
pub(crate) fn choose(measures: &Measures, bounds: &Bounds) -> RuleId {
    if speck(measures.area, bounds.min_area) {
        return RuleId::Speck;
    }
    match (bounds.asked, measures.breadth) {
        (Asked::Run | Asked::Fill, _) | (Asked::Satin, Breadth::Medium) => return RuleId::Asked,
        (Asked::Auto | Asked::Satin, _) => {}
    }
    match measures.breadth {
        Breadth::Narrow => RuleId::Line,
        Breadth::Broad if bounds.is_wide(measures.width, measures.lines, measures.unevenness) => RuleId::Wide,
        Breadth::Broad => RuleId::Area,
        Breadth::Medium if thousandths(measures.slenderness()) < thousandths(bounds.min_slenderness) => RuleId::Compact,
        // A part of one line is held to the bound when it is as wide as a fill's narrowest piece somewhere
        // (`REQ-AUTO-174`): a leaf or a lens is filled whole, and a ray keeps its column.
        Breadth::Medium
            if bounds.fill_uneven
                && (measures.lines > 1 || measures.fill_wide)
                && thousandths(measures.unevenness) > thousandths(bounds.max_unevenness) =>
        {
            RuleId::Uneven
        }
        Breadth::Medium if thousandths(measures.reach) > thousandths(bounds.max_reach) => RuleId::Arms,
        Breadth::Medium => RuleId::Column,
    }
}

/// Whether a part that sews by `rule` under `bounds` is filled: an area, a compact part, an uneven one, one
/// with arms, and any part of a shape that asks to be filled.
pub(crate) fn fills(rule: RuleId, bounds: &Bounds) -> bool {
    match rule {
        RuleId::Area | RuleId::Compact | RuleId::Uneven | RuleId::Arms => true,
        RuleId::Asked => bounds.asked == Asked::Fill,
        RuleId::Speck | RuleId::Line | RuleId::Wide | RuleId::Column | RuleId::Refused => false,
    }
}

/// The rule a part of a shape sewed by, and what was measured of it, when anything was.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Fired {
    /// The rule.
    pub(crate) rule: RuleId,
    /// Its measures.
    pub(crate) measures: Option<Measures>,
}

impl Fired {
    /// The rule and the measures that decided it, for `SC-I0912`: the area of a speck, the width and the reach
    /// of a part with arms, and the width, the slenderness, how much the width varies and the lines of any other
    /// part whose lines were measured.
    fn text(&self) -> String {
        let measured = self.measures.and_then(|m| match (self.rule, m.breadth) {
            (RuleId::Speck, _) => Some(format!("{} mm²", mm(m.area))),
            (RuleId::Arms, _) => Some(format!("{} mm wide, reaching {:.2} times as far as its widest circle", mm(m.width), m.reach)),
            (_, Breadth::Medium) if m.lines > 0 && m.area.is_finite() => {
                let lines = if m.lines == 1 { "1 centre line".to_string() } else { format!("{} centre lines", m.lines) };
                Some(format!("{} mm wide, slenderness {:.1}, width varying by {:.2}, {lines}", mm(m.width), m.slenderness(), m.unevenness))
            }
            _ if m.width > 0.0 => Some(format!("{} mm wide", mm(m.width))),
            _ => None,
        });
        match measured {
            Some(measured) => format!("{} ({measured})", self.rule.key()),
            None => self.rule.key().to_string(),
        }
    }
}

/// How many parts `SC-I0912` gives the measures of: a traced shape can have dozens, and the rest are counted
/// by rule.
const LISTED: usize = 3;

/// `SC-I0912` for the automatic shape `id` whose parts sewed by `fired`, in order: the rule each part sewed
/// by, with the measures that decided it for the first [`LISTED`], and the rest counted by rule. `None` for
/// a shape with no part.
pub(super) fn said(id: &ElementId, fired: &[Fired]) -> Option<Diagnostic> {
    let message = match fired {
        [] => return None,
        [only] => format!("This shape sews by the rule {}.", only.text()),
        _ => {
            let listed: Vec<String> = fired.iter().take(LISTED).enumerate().map(|(n, part)| format!("part {}: {}", n + 1, part.text())).collect();
            let rest = fired.get(LISTED..).unwrap_or_default();
            let counted: Vec<String> = RULES
                .iter()
                .map(|rule| (rule.id.key(), rest.iter().filter(|part| part.rule == rule.id).count()))
                .filter(|(_, count)| *count > 0)
                .map(|(key, count)| format!("{count} {key}"))
                .collect();
            let more = if rest.is_empty() { String::new() } else { format!(", and {} more: {}", rest.len(), counted.join(", ")) };
            format!("This shape's {} parts sew by these rules, {}{more}.", fired.len(), listed.join(", "))
        }
    };
    Some(Diagnostic::new(Code::AutoRule, message).with_element(id.clone()))
}

#[cfg(test)]
mod tests;
