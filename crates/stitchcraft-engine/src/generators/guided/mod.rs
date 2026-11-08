//! Guided fill (milestone M10.9): rows that follow a guide line drawn with the fill, copied across each part
//! of its area, as Ink/Stitch sews its guided fill with its copy strategy
//! (`docs/src/design/algorithms/guided.md`, `REQ-FILL-GDD-001` to `REQ-FILL-GDD-008`).
//!
//! **Why.** A tatami fill's rows are straight. A leaf, a flame or a ribbon reads better with rows that bend
//! with it, along a line the designer draws beside it: the guide.
//!
//! **Steps.** A guided fill sews its parts as a tatami fill sews them, nearest the needle first, each with
//! the tatami fill's underlay first and grown by `expand_mm` for its rows (the shared part loop,
//! `tatami::sew::parts_sewn`). Over each piece of a part:
//!
//! 1. **The guide** (the `line` module): its first subpath, flattened as a shape's outline is, made one line
//!    where it crosses itself and opened where it closes, then lengthened straight on at both ends, along
//!    the line between its ends, until each end lies as far out as the piece's farthest corner from it.
//! 2. **Copies** (the `copies` module): the guide moved a row spacing at a time, square to the line between
//!    its ends or at `guided_fill_angle`, from the copy nearest the piece outward both ways while the copies
//!    meet the piece's bounds, smoothed by `smoothness_mm`. Those that meet the piece are its rows, in order
//!    across it.
//! 3. **Stitches** (the `stitched` module): each row smoothed again, and stitched from a `staggers`-th of a
//!    stitch further along than the row before, as `stitch_position_method` places its needle points.
//! 4. **Cut** (the `cut` module): each stitched row is cut where it crosses the piece's rings, and the pieces
//!    inside are the lines the fill sews, routed as a tatami fill's rows are, the needle travelling under the
//!    lines not sewn yet (`tatami::sew::lines`). Bean stitch then sews each stitch back and forth, but the
//!    travel between lines.
//!
//! **Without a guide.** A piece no copy meets sews a tatami fill at the angle of the line between the guide's
//! ends (`SC-W0332`), as Ink/Stitch falls back to it. A fill with no guide, or with one that starts and ends
//! at one point once it is made one line, sews as a tatami fill with its own settings (`SC-W0331`), as
//! Ink/Stitch sews one without a guide (`DEV-FILL-034`).

mod copies;
mod cut;
mod line;
mod stitched;

use stitchcraft_core::math::{atan2, to_degrees};
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};
use stitchcraft_params::{ChoiceOption, StitchType, normalized_angle, params};

pub use self::stitched::{Position, RowStitching};
use crate::design::Path;
use crate::generators::passes;
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::rows::Grid;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::lines::{Given, Marked, lines_sewn};
use crate::generators::tatami::sew::{Fill, Layer, Parts, Pull, Travel, layer_sewn, parts_sewn, pieces, tatami_fill, too_thin, top_rows};
use crate::generators::tatami::stitches::Stitching;
use crate::generators::{Approach, Stitched, settled};
use crate::normalize::region::{Polygon, Region};
use crate::normalize::stroke;
use crate::thin::thin;

/// The stitch types the guided fill's own settings apply to.
const GUIDED: &[StitchType] = &[StitchType::GuidedFill];

/// The strategies `guided_fill_strategy` names, by Ink/Stitch's numbers. Copying is sewn. Ink/Stitch's
/// parallel offset and buffer arrive with M10.10, and a fill set to one is skipped until then (`SC-W0011`).
pub const STRATEGIES: &[ChoiceOption] =
    &[ChoiceOption::new(COPY, "Copy"), ChoiceOption::kept("1", "Parallel offset"), ChoiceOption::kept("2", "Buffer")];

/// The strategy that copies the guide, the default.
pub const COPY: &str = "0";

/// The key of the strategy setting, which the generate step reads first: a strategy not sewn yet skips the
/// element.
pub const STRATEGY: &str = "guided_fill_strategy";

params! {
    /// Guided fill: rows that follow a guide line drawn with the fill, copied across its area. The guide is a
    /// line grouped with the fill and marked as one: Ink/Stitch's guide-line marker on a stroke in an SVG
    /// file, or a path named `stitch:guide` in a VectorCraft document. A fill without one sews as a tatami
    /// fill (`SC-W0331`).
    pub struct GuidedParams for GUIDED;

    "Guided fill" {
        /// How the rows follow the guide. Copy moves copies of the guide across the fill, a row spacing
        /// apart. Ink/Stitch's parallel offset and buffer arrive in a later version, and until then a fill set
        /// to one is skipped (`SC-W0011`).
        guided_fill_strategy: Choice = "0", label "Strategy", choices STRATEGIES;

        /// The direction the guide's copies move in, in degrees clockwise from horizontal as the drawing shows
        /// it. Empty, they move square to the line between the guide's ends. A direction near the guide's own
        /// brings the rows closer together than the row spacing, and the copies then reach only as far as the
        /// guide is long.
        guided_fill_angle: OptionalAngle = "", label "Copy angle", when guided_fill_strategy == "0";

        /// Where the needle points lie along each row. Flexible spreads a row's stitches evenly between its
        /// corners. Flexible (stagger) sews whole stitches of the longest stitch from where the row's stagger
        /// starts it, round its corners, and the last one shorter, so that the rows' needle points keep their
        /// stagger.
        stitch_position_method: Choice = "flexible", label "Stitch positions",
            options ["flexible" => "Flexible", "flexible_stagger" => "Flexible (stagger)"];
    }
}

impl GuidedParams {
    /// Where the needle points lie along each row (`stitch_position_method`).
    pub fn position(&self) -> Position {
        if self.stitch_position_method == "flexible_stagger" { Position::Stagger } else { Position::Flexible }
    }
}

/// A guided fill as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub struct Guided {
    /// The path of its design's guide line; `None` without one.
    pub guide: Option<Path>,
    /// How far apart the copies are, in millimetres: `row_spacing_mm`.
    pub spacing: f64,
    /// The direction the copies move in, in degrees clockwise from the x axis as the drawing shows it;
    /// `None` for square to the line between the guide's ends.
    pub angle: Option<f64>,
    /// How much each copy is smoothed, in millimetres, once before the rows are found and once before they
    /// are stitched; 0 for not at all.
    pub smoothness: f64,
    /// How the rows are stitched.
    pub stitching: RowStitching,
    /// Whether each line is sewn without its last needle point (`skip_last`).
    pub skip_last: bool,
    /// How many times each stitch goes back and forth, taken in turn along the stitches.
    pub beans: Vec<u32>,
    /// The underlay's layers, the tatami fill's.
    pub underlay: Vec<Layer>,
    /// How far each part is shrunk for its underlay, in millimetres.
    pub inset: f64,
    /// How far each part is grown for its rows, in millimetres.
    pub expand: f64,
    /// Whether locks part the element's groups (`force_lock_stitches`).
    pub force_locks: bool,
    /// The needle's travel between the lines: its running stitch, whether it runs under them, the shortest
    /// stitch, and the jump length.
    pub travel: Travel,
    /// The tatami fill the element sews without a guide it can use, as Ink/Stitch sews a guided fill
    /// without a guide.
    pub plain: Plain,
}

/// The tatami fill a guided fill sews without a guide it can use: the element's settings read as a tatami
/// fill's, and what that reading says, said only when the fill is sewn so.
#[derive(Clone, Debug, PartialEq)]
pub struct Plain {
    /// The fill; `None` when the settings don't read as a tatami fill's, which `said` says.
    pub fill: Option<Fill>,
    /// What reading them said that reading them as a guided fill's didn't.
    pub said: Vec<Diagnostic>,
}

/// The stitches of the guided fill `guided` over `region`: in each part the tatami fill's underlay, then its
/// rows over each piece of the part grown by `expand_mm`, from the needle where the elements before left it
/// to near the next part, the last near the `next` element. Random lengths draw from `rng` as it came, every
/// row from a stream its place keys. Without a guide it can use, its tatami fill `plain`, and `SC-W0331`
/// says so. Work is charged to `meter` for every point of the guide, of its copies and of their stitches,
/// and a stitch for every needle point.
pub fn guided_fill(
    region: &Region,
    guided: &Guided,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let Some(guide) = guide_of(guided, meter)? else {
        let mut warnings = vec![no_guide(guided.guide.is_some())];
        warnings.extend(guided.plain.said.iter().cloned());
        let Some(plain) = &guided.plain.fill else { return Ok(Stitched { runs: Vec::new(), colours: Vec::new(), warnings }) };
        let mut sewn = tatami_fill(region, plain, needle, next, rng, meter)?;
        warnings.append(&mut sewn.warnings);
        return Ok(Stitched { warnings, ..sewn });
    };
    settled(meter, |meter| sewn(region, guided, &guide, needle, next, rng, meter))
}

/// [`guided_fill`] with the guide's line `guide`, before its stitches are settled.
fn sewn(
    region: &Region,
    guided: &Guided,
    guide: &Guide,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let mut tally = Tally::default();
    // Each row's random lengths draw from the element's generator as it came, as Ink/Stitch's draw from the
    // element's seed whatever the underlay drew.
    let base = rng.clone();
    let mut top =
        |piece: &Polygon,
         _: &Polygon,
         needle: Option<Point>,
         end: Option<Point>,
         rng: &mut SplitMix64,
         meter: &mut Meter,
         warnings: &mut Vec<Diagnostic>| { tally.top(piece, guided, guide, (needle, end), (&base, rng), meter, warnings) };
    let parts = Parts {
        underlay: &guided.underlay,
        inset: guided.inset,
        expand: guided.expand,
        force_locks: guided.force_locks,
        top: guided.travel,
        ends_near: true,
    };
    let sewn = parts_sewn(region, &parts, needle, next, rng, meter, Some(&mut top))?;
    let mut warnings = sewn.warnings;
    warnings.extend(tally.said());
    warnings.extend(sewn.tally.said());
    Ok(Stitched { runs: sewn.runs, colours: Vec::new(), warnings })
}

/// The lines of the guided fill `guided` over `region`, each part grown by its `expand` as [`guided_fill`]
/// grows it, as the segments they run along, or a tatami fill's rows where no copy of the guide meets a
/// piece, or without a guide it can use: where the rows lie, without the underlay or the needle's travel,
/// their random lengths drawn from streams of `rng` as sewing draws them. A preview too costly to sew shows
/// them, as it shows a tatami fill's top rows. Work is charged to `meter` as for sewing.
pub fn top_path(region: &Region, guided: &Guided, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let Some(guide) = guide_of(guided, meter)? else {
        return match &guided.plain.fill {
            Some(plain) => top_rows(region, plain, meter),
            None => Ok(Vec::new()),
        };
    };
    let mut segments = Vec::new();
    for polygon in &region.parts {
        for piece in &pieces(polygon, guided.expand, &mut Vec::new(), meter)?.0 {
            let rings = Rings::new(piece, meter)?;
            let Some(lines) = rows_of(piece, &rings, guided, &guide, rng, meter)? else {
                let fallback = Fill { top: guide.fallback(guided), expand: 0.0, underlay: Vec::new(), inset: 0.0, force_locks: false };
                segments.extend(top_rows(&Region { parts: vec![piece.clone()] }, &fallback, meter)?);
                continue;
            };
            for line in &lines {
                segments.extend(line.iter().zip(line.iter().skip(1)).map(|(&start, &end)| Segment { start, end }));
            }
        }
    }
    Ok(segments)
}

/// The guide line of `guided` as the rows copy it ([`line::prepared`]), and the angle of the line between
/// its ends as drawn; `None` when it has none, or one that starts and ends at one point once it is made one
/// line.
fn guide_of(guided: &Guided, meter: &mut Meter) -> Result<Option<Guide>, Exhausted> {
    let Some(first) = guided.guide.as_ref().and_then(|path| path.subpaths.first()) else { return Ok(None) };
    let drawn = stroke::flattened(first, meter)?;
    let (Some(&start), Some(&end)) = (drawn.first(), drawn.last()) else { return Ok(None) };
    let Some(line) = line::prepared(&drawn, meter)? else { return Ok(None) };
    // Ink/Stitch's angles turn counter-clockwise as the drawing shows it, y down.
    let angle = normalized_angle(-to_degrees(atan2(end.y() - start.y(), end.x() - start.x())));
    Ok(Some(Guide { line, angle }))
}

/// A guide line, ready to be copied: its line, which no copy has lengthened yet, and the angle of the line
/// between the ends of its first subpath as drawn, in degrees counter-clockwise from horizontal as the
/// drawing shows it, which a tatami fill falls back to.
struct Guide {
    line: Vec<Point>,
    angle: f64,
}

impl Guide {
    /// The tatami fill's rows a piece no copy of the guide meets sews: at the angle of the guide's ends, the
    /// guided fill's row spacing apart, staggered on the tatami fill's grid in its longest stitch, as
    /// Ink/Stitch falls back to a tatami fill there, without an end spacing, random lengths, gap-filling
    /// rows or pull compensation.
    fn fallback(&self, guided: &Guided) -> Layer {
        let stitching = Stitching {
            grid: Grid { angle: self.angle, spacing: guided.spacing, end_spacing: None },
            length: guided.stitching.length,
            staggers: guided.stitching.staggers,
            skip_last: guided.skip_last,
            jitter: None,
        };
        Layer { stitching, travel: guided.travel, pull: Pull::default(), gap_rows: 0 }
    }
}

/// The lines the guide's copies sew over `piece`, whose rings are `rings`: the copies that meet it,
/// stitched and cut by it, in the order they are taken ([`copies::rows`], [`stitched::row`], [`cut::cut`]).
/// Each row's random lengths draw from a stream of `rng` that its place keys. `None` when no copy leaves a
/// line inside the piece.
fn rows_of(
    piece: &Polygon,
    rings: &Rings<'_>,
    guided: &Guided,
    guide: &Guide,
    rng: &SplitMix64,
    meter: &mut Meter,
) -> Result<Option<Vec<Vec<Point>>>, Exhausted> {
    let copies = copies::rows(piece, rings, &guide.line, guided, meter)?;
    let mut lines = Vec::new();
    for (i, copy) in copies.iter().enumerate() {
        let row = stitched::row(copy, i, guided, rng, meter)?;
        lines.extend(cut::cut(&row, rings, meter)?);
    }
    Ok((!lines.is_empty()).then_some(lines))
}

/// What sewing a guided fill's pieces counted, for its diagnostics: the pieces, those no copy of the guide
/// meets, and those of them too thin for the tatami fill's rows too.
#[derive(Default)]
struct Tally {
    pieces: usize,
    missed: usize,
    outlined: usize,
}

impl Tally {
    /// The runs the guide's copies sew over `piece`, from the needle to near the end where there are any
    /// (`ends`), each row's random lengths drawing from a stream of the element's generator as it came, the
    /// first of `rngs`, and a tatami fill's from the second. A jump the needle needs between lines is said in
    /// `warnings`.
    #[allow(clippy::too_many_arguments)]
    fn top(
        &mut self,
        piece: &Polygon,
        guided: &Guided,
        guide: &Guide,
        (needle, end): (Option<Point>, Option<Point>),
        (base, rng): (&SplitMix64, &mut SplitMix64),
        meter: &mut Meter,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Vec<Vec<Point>>, Exhausted> {
        self.pieces += 1;
        let rings = Rings::new(piece, meter)?;
        let Some(lines) = rows_of(piece, &rings, guided, guide, base, meter)? else {
            self.missed += 1;
            let (runs, rowless) = layer_sewn(piece, &guide.fallback(guided), needle, end, rng, meter, warnings)?;
            self.outlined += usize::from(rowless);
            return Ok(runs);
        };
        let given = Given { travel: guided.travel, angle: guided.angle.unwrap_or(0.0), skip_last: guided.skip_last };
        let runs = lines_sewn(piece, &rings, &lines, &given, needle, end, meter, warnings)?.unwrap_or_default();
        runs.into_iter().map(|run| finished(run, guided, meter)).collect()
    }

    /// What the counts say: `SC-W0332` for the pieces no copy of the guide meets, sewn as a tatami fill, and
    /// `SC-W0305` for those of them too thin for its rows too, sewn round their outline.
    fn said(&self) -> impl Iterator<Item = Diagnostic> {
        let missed = match (self.missed, self.pieces) {
            (0, _) => None,
            (1, 1) => Some(
                "No copy of this guided fill's guide meets it, so it is sewn as a tatami fill at the angle of the line between the guide's ends."
                    .to_string(),
            ),
            (1, _) => Some(
                "No copy of this guided fill's guide meets 1 of its parts, so that part is sewn as a tatami fill at the angle of the line between \
                 the guide's ends."
                    .to_string(),
            ),
            (n, _) => Some(format!(
                "No copy of this guided fill's guide meets {n} of its parts, so each is sewn as a tatami fill at the angle of the line between the \
                 guide's ends."
            )),
        };
        missed.map(|message| Diagnostic::new(Code::FillGuidedMissed, message)).into_iter().chain(too_thin(self.outlined, self.pieces))
    }
}

/// `run`, its needle points nearer the one before than the shortest stitch left out, then sewn with its
/// bean stitch, but where it marks the needle travelling, as Ink/Stitch leaves out its short stitches before
/// its guided fill's bean stitch. Without bean stitch, `run` as it is: the fill's parts are thinned anyway.
fn finished(run: Marked, guided: &Guided, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    if guided.beans.iter().all(|&bean| bean == 0) {
        return Ok(run.points);
    }
    let min = guided.travel.min_stitch;
    let marked: Vec<(Point, bool)> = run.points.into_iter().zip(run.travel).collect();
    let kept = thin(marked, |a, b| !at_least(a.0.distance(b.0), min), |_| false, |_, _| ());
    let (points, travel): (Vec<Point>, Vec<bool>) = kept.into_iter().unzip();
    passes::beaned(&points, &guided.beans, &travel, meter)
}

/// `SC-W0331`: a guided fill without a guide it can use, sewn as a tatami fill; `drawn` when it has one that
/// starts and ends at one point once it is made one line (`DEV-FILL-034`).
fn no_guide(drawn: bool) -> Diagnostic {
    let message = if drawn {
        "This guided fill's guide line starts and ends at one point, so the fill is sewn as a tatami fill with its own settings."
    } else {
        "This guided fill has no guide line, so it is sewn as a tatami fill with its own settings."
    };
    Diagnostic::new(Code::FillGuidedNoGuide, message)
}

#[cfg(test)]
mod tests;
