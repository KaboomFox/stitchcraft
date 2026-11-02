//! Contour fill (milestone M7.7): rings that follow a fill's outline inward, like growth rings, sewn as one
//! path of running stitches (`docs/src/design/algorithms/fills.md` › Contour fill, `REQ-FILL-CON-001` to
//! `REQ-FILL-CON-007`).
//!
//! **Why rings.** Rows across a shape show its direction; rings follow its outline, so a contour fill
//! suits round and flowing shapes, and sews as one unbroken line, with no travel between rows.
//!
//! **Steps.** A contour fill sews its parts as a tatami fill sews them, nearest the needle first, each with
//! the tatami fill's underlay first and grown by `expand_mm` for its top (the shared part loop,
//! `tatami::sew::parts_sewn`). Over each piece of a part:
//!
//! 1. Its rings are found, a row spacing apart, with the corners `join_style` gives, and make a tree
//!    (`tree`).
//! 2. A path is laid through them as `contour_strategy` says: inner to outer round each ring (`inner`),
//!    smoothed by `smoothness_mm` and kept inside the piece, or a spiral, single or double (`spiral`). A
//!    piece too narrow for its spiral is sewn inner to outer (`SC-I0315`). Rings the path does not reach
//!    are left out (`SC-W0311`).
//! 3. The path is sewn as a running stitch of the longest stitch, within the running stitch's tolerance, at
//!    random lengths when they are on, then bean stitch, then the repeats, each of which sews everything so
//!    far again the other way round ([`passes::doubled`]).
//!
//! **Stretches.** The path is kept as its stretches: along one ring between steps, or one turn of a spiral.
//! Each stretch, and each step between them, is stitched as a span of its own. The running stitch spreads a
//! span's stitches evenly and splits those that stray from a curve too tight for them (`DEV-RUN-002`): over
//! a whole spiral, from its wide outer turns to its tight middle, that halved the inner stitches, and sewed
//! some 30 % more stitches than Ink/Stitch, whose stitches each shorten to fit. A ring, or a turn, curves
//! about as tightly all along. A smoothed path is cut again as far along it as its stretches met.
//!
//! The rings start where the needle is: the start command's point, the underlay's last stitch, or the
//! origin. The end command is read but not used: the path ends where its strategy ends it.

mod inner;
pub(crate) mod line;
mod spiral;
mod tree;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};
use stitchcraft_params::{StitchType, params};

use crate::design::Join;
use crate::generators::passes::{self, Turn};
use crate::generators::running::along_line_varied;
use crate::generators::smoothing::smooth;
use crate::generators::tatami::clamp::clamp;
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::{Layer, Parts, Travel, parts_sewn, pieces};
use crate::generators::{Stitched, settled};
use crate::normalize::offset::Corners;
use crate::normalize::region::{Polygon, Region};

/// The stitch types a contour fill's own settings apply to.
const CONTOUR: &[StitchType] = &[StitchType::ContourFill];

/// The mitre limit of a contour fill's mitred corners, in row spacings: Ink/Stitch's.
const MITRE_LIMIT: f64 = 10.0;
/// The segments to a quarter circle of its round corners: GEOS raises the 5 Ink/Stitch asks for to 8.
const ROUND_SEGMENTS: u32 = 8;

params! {
    /// Contour fill: rings that follow the fill's outline inward, like growth rings, sewn as one path of
    /// running stitches. It suits round and flowing shapes.
    pub struct ContourParams for CONTOUR;

    "Contour" {
        /// How the path runs through the rings. Inner to outer steps in to the middle first and sews each
        /// ring round on the way back out. Single spiral turns in from the outline to the middle. Double
        /// spiral turns in, and back out between the turns it made.
        contour_strategy: Choice = "0", label "Strategy",
            options ["0" => "Inner to outer", "1" => "Single spiral", "2" => "Double spiral"];

        /// How the rings turn at the shape's corners, as they move in a row spacing at a time: round,
        /// mitred (pointed, cut off 10 row spacings out) or bevelled (cut straight across).
        join_style: Choice = "0", label "Join style", options ["0" => "Round", "1" => "Mitred", "2" => "Bevelled"];

        /// With inner to outer, the rings run the other way at every other level, so that the steps from
        /// one ring to the next don't cross. Off, only the middles run the other way.
        avoid_self_crossing: Toggle = "false", label "Avoid self-crossing";

        /// The rings run clockwise as the drawing shows them. Off, counter-clockwise.
        clockwise: Toggle = "true", label "Clockwise";
    }
}

/// How a contour fill's path runs through its rings (`contour_strategy`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Strategy {
    /// Round each ring, from the middles out.
    InnerToOuter,
    /// One spiral from the outline in.
    SingleSpiral,
    /// A spiral in and one back out between its turns.
    DoubleSpiral,
}

/// How a contour fill's path is stitched: its running stitch, bean stitch and repeats.
#[derive(Clone, Debug, PartialEq)]
pub struct Stitching {
    /// The longest stitch, in millimetres.
    pub length: f64,
    /// How far a stitch may stray from the path, in millimetres.
    pub tolerance: f64,
    /// How much the lengths vary at random, as a share of the length: `None` for even stitches.
    pub jitter: Option<f64>,
    /// How many times each stitch goes back and forth, taken in turn along the stitches.
    pub beans: Vec<u32>,
    /// How many times the path is sewn over, doubling: `k` sews `2^(k − 1)` passes.
    pub repeats: u32,
}

/// A contour fill as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub struct Contour {
    /// How the path runs through the rings.
    pub strategy: Strategy,
    /// How far apart the rings are, in millimetres.
    pub spacing: f64,
    /// How the rings turn at corners.
    pub corners: Corners,
    /// Whether the rings run clockwise as drawn.
    pub clockwise: bool,
    /// Whether inner to outer's rings run the other way at every other level.
    pub avoid_self_crossing: bool,
    /// How much inner to outer's path is smoothed, in millimetres; 0 for not at all.
    pub smoothness: f64,
    /// How the path is stitched.
    pub stitching: Stitching,
    /// The underlay's layers, the tatami fill's.
    pub underlay: Vec<Layer>,
    /// How far each part is shrunk for its underlay, in millimetres.
    pub inset: f64,
    /// How far each part is grown for its rings, in millimetres.
    pub expand: f64,
    /// Whether locks part the element's groups (`force_lock_stitches`).
    pub force_locks: bool,
    /// The needle's travel: its jump length, past which a piece is a run of its own, and the shortest stitch.
    pub travel: Travel,
}

impl ContourParams {
    /// How the path runs through the rings.
    pub fn strategy(&self) -> Strategy {
        match self.contour_strategy {
            "1" => Strategy::SingleSpiral,
            "2" => Strategy::DoubleSpiral,
            _ => Strategy::InnerToOuter,
        }
    }

    /// The rings' corners, `spacing` apart: round with 8 segments to a quarter circle, mitred with a limit
    /// of 10 times the spacing, or bevelled.
    pub fn corners(&self) -> Corners {
        let join = match self.join_style {
            "1" => Join::Miter { limit: MITRE_LIMIT },
            "2" => Join::Bevel,
            _ => Join::Round,
        };
        Corners { join, quadrant_segments: ROUND_SEGMENTS }
    }
}

/// The stitches of the contour fill `contour` over `region`: in each part its underlay, then its rings,
/// from the needle where the elements before left it. Random lengths draw from streams of `rng`. Work is
/// charged to `meter` for every ring, point and stitch.
pub fn contour_fill(
    region: &Region,
    contour: &Contour,
    needle: Option<Point>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, contour, needle, rng, meter))
}

/// [`contour_fill`], before its stitches are settled.
fn sewn(region: &Region, contour: &Contour, needle: Option<Point>, rng: &mut SplitMix64, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let mut tally = Tally::default();
    let mut top =
        |piece: &Polygon, _: &Polygon, needle: Option<Point>, _: Option<Point>, rng: &mut SplitMix64, meter: &mut Meter, _: &mut Vec<Diagnostic>| {
            let stretches = tally.path(piece, contour, needle.unwrap_or(Point::ORIGIN), meter)?;
            let run = tally.stitched(&stretches, contour, rng, meter)?;
            let run = passes::sew(&run, 1, &contour.stitching.beans, Turn::Listed, meter)?;
            Ok(vec![passes::doubled(run, contour.stitching.repeats, meter)?])
        };
    let parts = Parts {
        underlay: &contour.underlay,
        inset: contour.inset,
        expand: contour.expand,
        force_locks: contour.force_locks,
        top: contour.travel,
        ends_near: false,
    };
    let sewn = parts_sewn(region, &parts, needle, None, rng, meter, &mut top)?;
    let mut warnings = sewn.warnings;
    warnings.extend(tally.said());
    warnings.extend(sewn.tally.said());
    Ok(Stitched { runs: sewn.runs, warnings })
}

/// The path of the `contour` fill over `region`, each part grown by its `expand` as [`contour_fill`] grows
/// it, as the segments it runs along: round its rings and across between them, or along a spiral's turns,
/// each piece's from its point nearest the origin, without the stitches, the underlay or the needle's
/// travel between parts. A preview too costly to sew shows it, as it shows a tatami fill's top rows. Work
/// is charged to `meter` as for sewing.
pub fn top_path(region: &Region, contour: &Contour, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let (mut tally, mut segments) = (Tally::default(), Vec::new());
    for polygon in &region.parts {
        for piece in &pieces(polygon, contour.expand, &mut Vec::new(), meter)?.0 {
            let path = tally.path(piece, contour, Point::ORIGIN, meter)?.concat();
            segments.extend(path.windows(2).map(|side| Segment { start: side[0], end: side[1] }));
        }
    }
    Ok(segments)
}

/// A spiral's stretches, one for each turn.
type Spiral = fn(&[&[Point]], f64, Point, &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted>;

/// What sewing a contour fill's pieces counted, for its diagnostics: the pieces, the rings left out, the
/// pieces too narrow for their spiral, and whether a level's edges failed to join. And the spans stitched,
/// whose number keys each one's random lengths.
#[derive(Default)]
struct Tally {
    pieces: usize,
    left_out: usize,
    short: usize,
    failed: bool,
    spans: usize,
}

impl Tally {
    /// The path through `piece`'s rings as `contour` lays it, from `start`, as its stretches.
    fn path(&mut self, piece: &Polygon, contour: &Contour, start: Point, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
        self.pieces += 1;
        let (rings, failed) = tree::tree(piece, contour.spacing, contour.corners, contour.clockwise, meter)?;
        self.failed |= failed.is_some();
        let spiral: Spiral = match contour.strategy {
            Strategy::InnerToOuter => return self.inner(&rings, piece, contour, start, meter),
            Strategy::SingleSpiral => spiral::single,
            Strategy::DoubleSpiral => spiral::double,
        };
        let (chain, left_out) = spiral::chain(&rings, meter)?;
        let chained: Vec<&[Point]> = chain.iter().map(|&node| rings.ring(node)).collect();
        let stretches = spiral(&chained, contour.stitching.length, start, meter)?;
        if stretches.iter().map(Vec::len).sum::<usize>() < 2 {
            self.short += 1;
            return self.inner(&rings, piece, contour, start, meter);
        }
        self.left_out += left_out;
        Ok(stretches)
    }

    /// The inner-to-outer path through `rings`, smoothed by the contour's smoothness, kept inside `piece`,
    /// and cut again as far along it as its stretches met.
    fn inner(
        &mut self,
        rings: &tree::Tree,
        piece: &Polygon,
        contour: &Contour,
        start: Point,
        meter: &mut Meter,
    ) -> Result<Vec<Vec<Point>>, Exhausted> {
        let path = inner::inner_to_outer(rings, contour.spacing, start, contour.avoid_self_crossing, meter)?;
        self.left_out += path.left_out;
        if contour.smoothness <= 0.0 {
            return Ok(path.stretches);
        }
        let whole = path.stretches.concat();
        let smoothed = smooth(&whole, contour.smoothness, contour.travel.min_stitch, meter)?;
        let kept = clamp(&smoothed, &Rings::new(piece, meter)?, meter)?;
        Ok(line::cut_at(&kept, &line::starts(&path.stretches)))
    }

    /// The running stitch along `stretches` as `contour` stitches it: each stretch, and each step from the end
    /// of one to the start of the next, a span of its own.
    fn stitched(&mut self, stretches: &[Vec<Point>], contour: &Contour, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let Stitching { length, tolerance, jitter, .. } = contour.stitching;
        let lengths = (length, tolerance, contour.travel.min_stitch);
        let mut run: Vec<Point> = Vec::new();
        for stretch in stretches {
            let Some(&first) = stretch.first() else { continue };
            let step = run.last().filter(|&&end| end != first).map(|&end| vec![end, first]);
            for line in step.iter().chain(std::iter::once(stretch)) {
                self.spans += 1;
                let stitches = along_line_varied(line, lengths, jitter, (rng, self.spans), meter)?;
                run.extend(stitches.into_iter().skip(usize::from(!run.is_empty())));
            }
        }
        Ok(run)
    }

    /// What the counts say: `SC-W0311` for the rings left out, `SC-I0315` for the pieces too narrow for
    /// their spiral, and `SC-E0009` should a level's edges not join into rings.
    fn said(&self) -> impl Iterator<Item = Diagnostic> {
        let left_out = match self.left_out {
            0 => None,
            1 => Some("This contour fill leaves out 1 of its rings, which its path does not reach.".to_string()),
            n => Some(format!("This contour fill leaves out {n} of its rings, which its path does not reach.")),
        };
        let short = match (self.short, self.pieces) {
            (0, _) => None,
            (1, 1) => Some("This contour fill is too narrow for its spiral, so it is sewn inner to outer.".to_string()),
            (1, _) => Some("1 of this contour fill's parts is too narrow for its spiral, so it is sewn inner to outer.".to_string()),
            (n, _) => Some(format!("{n} of this contour fill's parts are too narrow for its spiral, so each is sewn inner to outer.")),
        };
        let failed = self.failed.then_some(
            "The edges of a ring of this contour fill could not be joined into rings, so the rings inside it are left out. This is a bug in \
             StitchCraft.",
        );
        let left_out = left_out.map(|message| Diagnostic::new(Code::FillContourRingsLeftOut, message));
        let short = short.map(|message| Diagnostic::new(Code::FillContourSpiralTooShort, message));
        let failed = failed.map(|message| Diagnostic::new(Code::InternalCheckFailed, message));
        left_out.into_iter().chain(short).chain(failed)
    }
}

#[cfg(test)]
mod tests;
