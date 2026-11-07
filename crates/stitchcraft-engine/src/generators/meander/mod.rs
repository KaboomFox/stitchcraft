//! Meander fill (milestone M7.8): a line that wanders through a network of short lines without passing a
//! point twice, as a quilter's meander does (`docs/src/design/algorithms/fills.md` › Meander fill,
//! `REQ-FILL-MEA-001` to `REQ-FILL-MEA-005`).
//!
//! **Why a network.** A meander covers an area with one line that never crosses itself, and turns where the
//! lines of its tile meet. The tile's lines lying in a part make a network, which the line wanders through
//! point by point, so it fills the part evenly with turns of the tile's shape.
//!
//! **Steps.** A meander fill sews its parts as a contour fill sews them, each with the tatami fill's
//! underlay first and grown by `expand_mm` for its top (the shared part loop, `tatami::sew::parts_sewn`),
//! each ending nearest the next part, the last nearest the next element. Over each piece of a part:
//!
//! 1. The tile is laid over the piece, and its lines inside the piece make the network (`network`). A piece
//!    with no network is sewn round its outline (`SC-W0317`).
//! 2. The path wanders through the network from the needle to where the piece ends (`path`).
//! 3. The path is smoothed, made a zigzag, kept inside the part as drawn when `clip` is on, and sewn as a
//!    running stitch, then bean stitch and the repeats, each of which sews everything so far again the
//!    other way round ([`passes::doubled`]).

mod network;
mod path;
mod tiles;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};
use stitchcraft_params::{Origin, StitchType, params};

use crate::generators::contour::line;
use crate::generators::passes::{self, Turn};
use crate::generators::running::along_line;
use crate::generators::smoothing::smooth;
use crate::generators::tatami::clamp::clamp;
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::{Layer, Parts, Travel, parts_sewn, pieces};
use crate::generators::{Approach, Stitched, settled};
use crate::normalize::region::{Polygon, Region};
pub use network::Placement;

/// The stitch types a meander fill's own settings apply to.
const MEANDER: &[StitchType] = &[StitchType::MeanderFill];

params! {
    /// Meander fill: a line that wanders through a network of short lines, the sides of a tiling, without
    /// passing a point twice, as a quilter's meander does. It covers the area evenly with one line that
    /// never crosses itself.
    pub struct MeanderParams for MEANDER;

    "Meander" {
        /// The tiling whose sides the line wanders along: hexagons, squares, triangles, or 2 or 3 kinds of
        /// polygon. Its sides are 2 mm long at a scale of 100 %. A fill set to one of Ink/Stitch's tiles,
        /// which StitchCraft doesn't have, wanders through the honeycomb (`SC-W0316`).
        meander_pattern: Choice = "honeycomb", label "Pattern", choices tiles::PATTERNS,
            origin Origin::InkStitchDeviates { deviation: "DEV-FILL-018" };

        /// How far the tiling is turned, in degrees clockwise as the drawing shows it.
        meander_angle: Angle = "0", label "Pattern angle";

        /// How large the tiling is, in percent: 100 makes its sides 2 mm long. 2 percentages separated by a
        /// space scale it across and down apart. Each is at least 1.
        meander_scale_percent: PercentPair = "100", label "Pattern scale", range (1.0, 10000.0);

        /// Keep the line inside the shape as drawn, before `expand_mm` grew it, where smoothing or the zigzag
        /// would take it out.
        clip: Toggle = "false", label "Clip to the shape";

        /// Sew the line as a zigzag across it, the needle going this far along the line from one side to the
        /// other and back. 0 sews it as a running stitch.
        zigzag_spacing_mm: Length = "0", label "Zigzag spacing", range (0.0, 100.0);

        /// How wide the zigzag is, from one side of the line to the other.
        zigzag_width_mm: Length = "3", label "Zigzag width", range (0.0, 100.0);
    }
}

impl MeanderParams {
    /// The tile `meander_pattern` names, and whether it names one of Ink/Stitch's, for which it gives the
    /// default.
    pub fn tile(&self) -> (&'static str, bool) {
        let ours = tiles::PATTERNS.iter().any(|t| t.offered && t.id == self.meander_pattern);
        (if ours { self.meander_pattern } else { tiles::DEFAULT }, !ours)
    }

    /// How the tile is laid: its scale across and down, each at least 1 %, and its angle in radians.
    pub fn placement(&self) -> Placement {
        let [across, down] = self.meander_scale_percent;
        Placement { scale: (across.max(1.0) / 100.0, down.max(1.0) / 100.0), angle: self.meander_angle.to_radians() }
    }

    /// The zigzag the line is sewn as: its spacing and width; `None` for a running stitch.
    pub fn zigzag(&self) -> Option<(f64, f64)> {
        (self.zigzag_spacing_mm.get() > 0.0).then(|| (self.zigzag_spacing_mm.get(), self.zigzag_width_mm.get()))
    }
}

/// How a meander fill is sewn: its tile and how it is laid, how its line is sewn, and the part loop's
/// settings, as the tatami fill's underlay, inset, expand and travel give them.
#[derive(Clone, Debug, PartialEq)]
pub struct Meander {
    /// The tile, by its id.
    pub tile: &'static str,
    /// How the tile is laid.
    pub placement: Placement,
    /// How much the path is smoothed, in millimetres; 0 for not at all.
    pub smoothness: f64,
    /// The zigzag's spacing and width, or `None` for a running stitch.
    pub zigzag: Option<(f64, f64)>,
    /// Whether the path is kept inside the part as drawn.
    pub clip: bool,
    /// How many times each stitch goes back and forth, taken in turn along the stitches.
    pub beans: Vec<u32>,
    /// How many times the path is sewn over, doubling: `k` sews `2^(k − 1)` passes.
    pub repeats: u32,
    /// The underlay's layers, the tatami fill's.
    pub underlay: Vec<Layer>,
    /// How far each part is shrunk for its underlay, in millimetres.
    pub inset: f64,
    /// How far each part is grown for its path, in millimetres.
    pub expand: f64,
    /// Whether locks part the element's groups (`force_lock_stitches`).
    pub force_locks: bool,
    /// The needle's travel, whose running stitch the path is sewn with: its length and tolerance, the
    /// shortest stitch, and the jump length, past which a piece is a run of its own.
    pub travel: Travel,
}

/// The stitches of the meander fill `meander` over `region`: in each part the tatami fill's underlay, then
/// its path over each piece of the part grown by `expand_mm`, from the needle where the elements before
/// left it to near the next part, the last near the `next` element. Each piece's lines and pairs are drawn
/// from `rng` as it came. Work is charged to `meter` for every cell, line, point, search step and stitch,
/// and a stitch for every needle point.
pub fn meander_fill(
    region: &Region,
    meander: &Meander,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, meander, needle, next, rng, meter))
}

/// [`meander_fill`], before its stitches are settled.
fn sewn(
    region: &Region,
    meander: &Meander,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let mut tally = Tally::default();
    // The paths draw from the element's generator as it came, whatever the underlay drew from it, so that
    // they are the paths `top_path` shows.
    let base = rng.clone();
    let mut top = |piece: &Polygon,
                   part: &Polygon,
                   needle: Option<Point>,
                   end: Option<Point>,
                   _: &mut SplitMix64,
                   meter: &mut Meter,
                   _: &mut Vec<Diagnostic>| {
        let Some(points) = tally.path(piece, meander, needle, end, &base, meter)? else {
            return Ok(vec![along_line(&piece.outline, meander.travel.length, meander.travel.tolerance, meander.travel.min_stitch, meter)?]);
        };
        Ok(vec![stitched(&points, part, meander, meter)?])
    };
    let parts = Parts {
        underlay: &meander.underlay,
        inset: meander.inset,
        expand: meander.expand,
        force_locks: meander.force_locks,
        top: meander.travel,
        ends_near: true,
    };
    let sewn = parts_sewn(region, &parts, needle, next, rng, meter, &mut top)?;
    let mut warnings = sewn.warnings;
    warnings.extend(tally.said());
    warnings.extend(sewn.tally.said());
    Ok(Stitched { runs: sewn.runs, colours: Vec::new(), warnings })
}

/// The path of the `meander` fill over `region`, each part grown by its `expand` as [`meander_fill`] grows
/// it, as the segments it runs along, each piece's from its outline's first point, as a fill sews it where
/// the needle is not yet anywhere: where its line runs, without the stitches, the underlay or the needle's
/// travel between parts. A preview too costly to sew shows it, as it shows a tatami fill's top rows. Work
/// is charged to `meter` as for sewing.
pub fn top_path(region: &Region, meander: &Meander, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let (mut tally, mut segments) = (Tally::default(), Vec::new());
    for polygon in &region.parts {
        for piece in &pieces(polygon, meander.expand, &mut Vec::new(), meter)?.0 {
            let points = tally.path(piece, meander, None, None, rng, meter)?.unwrap_or_default();
            segments.extend(points.iter().zip(points.iter().skip(1)).map(|(&start, &end)| Segment { start, end }));
        }
    }
    Ok(segments)
}

/// What sewing a meander fill's pieces counted, for its diagnostics: the pieces, and those with no
/// network, sewn round their outline.
#[derive(Default)]
struct Tally {
    pieces: usize,
    outlined: usize,
}

impl Tally {
    /// The path of `meander` over `piece` as its points, from the needle, or its outline's first point, to
    /// `end`, or the outline's point half way round from the start; `None` when the piece has no network.
    /// Its lines and pairs are drawn from `rng` as it is, as every piece's are, so that a piece's path
    /// depends on nothing but the piece, its start and end and the seed: not on where the design lies, nor
    /// on the other pieces or the order they are sewn in.
    fn path(
        &mut self,
        piece: &Polygon,
        meander: &Meander,
        needle: Option<Point>,
        end: Option<Point>,
        rng: &SplitMix64,
        meter: &mut Meter,
    ) -> Result<Option<Vec<Point>>, Exhausted> {
        self.pieces += 1;
        let rings = Rings::new(piece, meter)?;
        // A tile StitchCraft doesn't have is sewn as the default, as `MeanderParams::tile` reads one.
        let tile = tiles::tile(meander.tile).or_else(|| tiles::tile(tiles::DEFAULT));
        let network = tile.map(|tile| network::network(piece, &rings, &tile, meander.placement, meter)).transpose()?.flatten();
        let Some(network) = network else {
            self.outlined += 1;
            return Ok(None);
        };
        let start = needle.or_else(|| piece.outline.first().copied()).unwrap_or(Point::ORIGIN);
        let end = end.unwrap_or_else(|| across(&piece.outline, start));
        path::path(&network, start, end, &mut rng.clone(), meter).map(Some)
    }

    /// `SC-W0317` for the pieces with no network, sewn round their outline.
    fn said(&self) -> Option<Diagnostic> {
        let message = match (self.outlined, self.pieces) {
            (0, _) => return None,
            (1, 1) => "This meander fill is too small for any line of its tile, so it is sewn as a running stitch round its outline.".to_string(),
            (1, _) => "1 of this meander fill's parts is too small for any line of its tile, so it is sewn as a running stitch round its outline."
                .to_string(),
            (n, _) => format!(
                "{n} of this meander fill's parts are too small for any line of its tile, so each is sewn as a running stitch round its outline."
            ),
        };
        Some(Diagnostic::new(Code::FillMeanderTooSmall, message))
    }
}

/// The point of the ring `outline` half way round it from its point nearest `start`.
fn across(outline: &[Point], start: Point) -> Point {
    let long = line::length(outline);
    let at = (line::project(outline, start) + long / 2.0) % long.max(f64::MIN_POSITIVE);
    line::point_at(outline, at).unwrap_or(start)
}

/// The stitches of the path `points` over the part `part`, as drawn: smoothed, made a zigzag, kept inside
/// the part when `clip` is on, sewn as a running stitch unless it zigzags and is not clipped, then bean
/// stitch and the repeats.
fn stitched(points: &[Point], part: &Polygon, meander: &Meander, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Travel { length, tolerance, min_stitch, .. } = meander.travel;
    let mut run = if meander.smoothness > 0.0 { smooth(points, meander.smoothness, min_stitch, meter)? } else { points.to_vec() };
    if let Some((spacing, width)) = meander.zigzag {
        run = zigzag(&along_line(&run, spacing / 2.0, tolerance, min_stitch, meter)?, width);
    }
    if meander.clip {
        run = clamp(&run, &Rings::new(part, meter)?, meter)?;
    }
    if meander.clip || meander.zigzag.is_none() {
        run = along_line(&run, length, tolerance, min_stitch, meter)?;
    }
    let run = passes::sew(&run, 1, &meander.beans, Turn::Listed, meter)?;
    passes::doubled(run, meander.repeats, meter)
}

/// `points` moved across their line in turn, half `width` to the right of the way to the next point as
/// drawn, y down, and then to the left, the last point kept where it is: as Ink/Stitch's meander zigzags.
/// A point where the line has no length is kept too.
fn zigzag(points: &[Point], width: f64) -> Vec<Point> {
    let half = width / 2.0;
    let mut out = points.to_vec();
    for (i, pair) in points.windows(2).enumerate() {
        let &[a, b] = pair else { continue };
        let long = a.distance(b);
        if long <= 0.0 {
            continue;
        }
        let side = if i % 2 == 1 { -half } else { half };
        // Right of the way from a to b as drawn, y down: the direction turned a quarter turn clockwise as
        // drawn, (x, y) to (-y, x).
        let (dx, dy) = ((b.x() - a.x()) / long, (b.y() - a.y()) / long);
        if let Some(p) = out.get_mut(i) {
            *p = Point::new(a.x() - dy * side, a.y() + dx * side).unwrap_or(a);
        }
    }
    out
}

#[cfg(test)]
mod tests;
