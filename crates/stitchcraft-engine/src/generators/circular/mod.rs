//! Circular fill (milestone M7.9): a double spiral round the middle of each part, cut by the part, as
//! Ink/Stitch sews its circular fill (`docs/src/design/algorithms/fills.md` › Circular fill,
//! `REQ-FILL-CIR-001` to `REQ-FILL-CIR-006`).
//!
//! **Why a spiral.** Its rings radiate from the middle of the shape, whatever its outline, so a sun, the
//! heart of a flower or a button sews round its middle. The spiral turns in on every other circle and back
//! out between them, so the fill ends near where it began instead of stuck at the middle.
//!
//! **Steps.** A circular fill sews its parts as a tatami fill sews them, nearest the needle first, each with
//! the tatami fill's underlay first and grown by `expand_mm` for its spiral (the shared part loop,
//! `tatami::sew::parts_sewn`). Over each piece of a part:
//!
//! 1. **Circles** round its middle, its centroid: one of 0.1 CSS pixel radius, then from the row spacing
//!    out, each the row spacing past the last, until one lies 2 spacings past the piece's farthest point.
//!    With `end_row_spacing_mm`, each step changes with the radius, from the row spacing at the middle to
//!    the end spacing at the farthest point, and stays at the end spacing past it (`DEV-FILL-021`). Each
//!    circle has 64 sides, as GEOS buffers a point for Ink/Stitch: from its right-hand point, turning up as
//!    drawn.
//! 2. **The spiral** turns in from the outermost circle on every other one and back out on the others: the
//!    contour fill's double spiral (`contour::spiral`), its turns cut into points by the running stitch's
//!    length.
//! 3. **Pieces.** The piece cuts the spiral where it meets its rings, and the stretches inside are the
//!    spiral's pieces (the `cut` module).
//! 4. **Sewing.** Each piece is a running stitch of the running stitch's length, within its tolerance and
//!    at random lengths when they are on, a span for each turn of the spiral and each step between turns, as
//!    the contour fill's spiral is stitched. One piece, as a round part's spiral can be, is sewn after the
//!    way along the outline from the needle to where it starts, and before the way along the outline to
//!    the end, the shorter way round (`DEV-FILL-022`). Several are routed as a tatami fill's rows are, the
//!    needle travelling between them under those not sewn yet (`tatami::sew::lines`). Where the spiral
//!    misses the piece, which then lies between 2 of its turns, a running stitch goes round its outline
//!    (`SC-W0305`), as in Ink/Stitch.
//! 5. **Bean stitch** sews each stitch back and forth but the travel between pieces, and each **repeat**
//!    then sews everything so far again, the other way round ([`passes::doubled`]).
//!
//! **A small piece**, within a row spacing of its middle, is sewn as one circle that size round its middle,
//! a running stitch of even lengths from the circle's right-hand point, as Ink/Stitch sews it, and
//! `SC-W0318` says so.
//!
//! **The middle** is each piece's centroid. Ink/Stitch's `target_point` command moves it, which StitchCraft
//! reads with the rest of Ink/Stitch's commands and settings in M8.

mod cut;

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use self::cut::{Spanned, cut};
use crate::generators::contour::{Stitching, spiral};
use crate::generators::passes;
use crate::generators::running::{along_line, along_line_varied};
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::lines::{Given, Marked, lines_sewn};
use crate::generators::tatami::sew::{Layer, Parts, Travel, parts_sewn, pieces};
use crate::generators::{Approach, Stitched, settled};
use crate::normalize::region::{Polygon, Region};

/// A circle's sides: 16 to a quarter circle, GEOS's default when Ink/Stitch buffers a point.
const SIDES: u32 = 64;
/// The radius of the circle at the middle, in millimetres: 0.1 CSS pixel, so that the spiral ends there.
const MIDDLE: f64 = 0.1 * MM_PER_SVG_PX;

/// A circular fill as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub struct Circular {
    /// How far apart the circles are at the middle, in millimetres.
    pub spacing: f64,
    /// How far apart they are at the farthest point, for circles that fade; `None` for evenly spaced ones.
    pub end_spacing: Option<f64>,
    /// How the spiral is stitched: the running stitch's length and tolerance, random lengths, bean stitch
    /// and repeats.
    pub stitching: Stitching,
    /// Whether each piece of the spiral is sewn without its last needle point.
    pub skip_last: bool,
    /// The fill's angle, in degrees, which the lines the needle travels along under the spiral are turned
    /// from.
    pub angle: f64,
    /// The underlay's layers, the tatami fill's.
    pub underlay: Vec<Layer>,
    /// How far each part is shrunk for its underlay, in millimetres.
    pub inset: f64,
    /// How far each part is grown for its spiral, in millimetres.
    pub expand: f64,
    /// Whether locks part the element's groups (`force_lock_stitches`).
    pub force_locks: bool,
    /// The needle's travel between the pieces of the spiral: its running stitch, whether it runs under them,
    /// the shortest stitch, and the jump length, past which a piece is a run of its own.
    pub travel: Travel,
}

/// The stitches of the circular fill `circular` over `region`: in each part the tatami fill's underlay, then
/// its spiral over each piece of the part grown by `expand_mm`, from the needle where the elements before
/// left it to near the next part, the last near the `next` element. Random lengths draw from `rng` as it
/// came, as every piece's do. Work is charged to `meter` for every circle, point and stitch, and a stitch
/// for every needle point.
pub fn circular_fill(
    region: &Region,
    circular: &Circular,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, circular, needle, next, rng, meter))
}

/// [`circular_fill`], before its stitches are settled.
fn sewn(
    region: &Region,
    circular: &Circular,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let mut tally = Tally::default();
    // Each piece's random lengths draw from the element's generator as it came, as Ink/Stitch's draw from the
    // element's seed whatever the underlay drew.
    let base = rng.clone();
    let mut top = |piece: &Polygon,
                   _: &Polygon,
                   needle: Option<Point>,
                   end: Option<Point>,
                   _: &mut SplitMix64,
                   meter: &mut Meter,
                   warnings: &mut Vec<Diagnostic>| { tally.top(piece, circular, (needle, end), &base, meter, warnings) };
    let parts = Parts {
        underlay: &circular.underlay,
        inset: circular.inset,
        expand: circular.expand,
        force_locks: circular.force_locks,
        top: circular.travel,
        ends_near: true,
    };
    let sewn = parts_sewn(region, &parts, needle, next, rng, meter, &mut top)?;
    let mut warnings = sewn.warnings;
    warnings.extend(tally.said());
    warnings.extend(sewn.tally.said());
    Ok(Stitched { runs: sewn.runs, warnings })
}

/// The pieces of the spiral of the `circular` fill over `region`, each part grown by its `expand` as
/// [`circular_fill`] grows it, as the segments they run along, or the circle a small part is sewn as: where
/// the spiral lies, without the stitches, the underlay or the needle's travel. A preview too costly to sew
/// shows it, as it shows a tatami fill's top rows. Work is charged to `meter` as for sewing.
pub fn top_path(region: &Region, circular: &Circular, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let (mut tally, mut segments) = (Tally::default(), Vec::new());
    for polygon in &region.parts {
        for piece in &pieces(polygon, circular.expand, &mut Vec::new(), meter)?.0 {
            let rings = Rings::new(piece, meter)?;
            let lines = match tally.spiral(piece, &rings, circular, meter)? {
                Spiral::Small(circle) => vec![circle],
                Spiral::Pieces(pieces) => pieces.into_iter().map(|piece| piece.points).collect(),
            };
            for line in &lines {
                segments.extend(line.iter().zip(line.iter().skip(1)).map(|(&start, &end)| Segment { start, end }));
            }
        }
    }
    Ok(segments)
}

/// A piece's spiral: the circle a small piece is sewn as, or the spiral's pieces inside it.
enum Spiral {
    Small(Vec<Point>),
    Pieces(Vec<Spanned>),
}

/// What sewing a circular fill's pieces counted, for its diagnostics: the pieces, those within a row
/// spacing of their middle, and those the spiral misses. And the spans stitched, whose number keys each
/// one's random lengths.
#[derive(Default)]
struct Tally {
    pieces: usize,
    small: usize,
    missed: usize,
    spans: usize,
}

impl Tally {
    /// The spiral of `circular` over `piece`, whose rings are `rings`.
    fn spiral(&mut self, piece: &Polygon, rings: &Rings<'_>, circular: &Circular, meter: &mut Meter) -> Result<Spiral, Exhausted> {
        self.pieces += 1;
        let middle = piece.centroid(meter)?;
        let farthest = farthest(piece, middle, meter)?;
        if circular.spacing > farthest {
            self.small += 1;
            return Ok(Spiral::Small(circle(middle, circular.spacing)));
        }
        let circles = circles(middle, farthest, circular.spacing, circular.end_spacing, meter)?;
        let rings_in: Vec<&[Point]> = circles.iter().map(Vec::as_slice).collect();
        let start = circles.first().and_then(|outermost| outermost.first()).copied().unwrap_or(middle);
        let turns = spiral::double(&rings_in, circular.stitching.length, start, meter)?;
        Ok(Spiral::Pieces(cut(&Spanned::joined(&turns), rings, meter)?))
    }

    /// The runs the spiral of `circular` sews over `piece`, from the needle to near the end where there are
    /// any (`ends`), random lengths drawing from `rng`. A jump the needle needs between pieces is said in
    /// `warnings`.
    fn top(
        &mut self,
        piece: &Polygon,
        circular: &Circular,
        ends: (Option<Point>, Option<Point>),
        rng: &SplitMix64,
        meter: &mut Meter,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Vec<Vec<Point>>, Exhausted> {
        let rings = Rings::new(piece, meter)?;
        let spiral = self.spiral(piece, &rings, circular, meter)?;
        self.sew(spiral, (piece, &rings), circular, ends, rng, meter, warnings)
    }

    /// The runs `spiral` sews over `piece`, whose rings are `rings`, as [`Tally::top`] sews them.
    #[allow(clippy::too_many_arguments)]
    fn sew(
        &mut self,
        spiral: Spiral,
        (piece, rings): (&Polygon, &Rings<'_>),
        circular: &Circular,
        (needle, end): (Option<Point>, Option<Point>),
        rng: &SplitMix64,
        meter: &mut Meter,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<Vec<Vec<Point>>, Exhausted> {
        let Stitching { length, tolerance, .. } = circular.stitching;
        let min = circular.travel.min_stitch;
        let pieces = match spiral {
            Spiral::Small(circle) => return Ok(vec![finished(&along_line(&circle, length, tolerance, min, meter)?, &[], circular, meter)?]),
            Spiral::Pieces(pieces) => pieces,
        };
        match pieces.as_slice() {
            // As Ink/Stitch's, the outline sews without bean stitch or repeats.
            [] => {
                self.missed += 1;
                Ok(vec![along_line(&piece.outline, length, tolerance, min, meter)?])
            }
            [only] => {
                let mut path = Spanned::default();
                if let (Some(needle), Some(&first)) = (needle, only.points.first()) {
                    path.extend(&along_outline(rings, needle, first, meter)?);
                }
                path.extend(only);
                if let (Some(end), Some(&last)) = (end, only.points.last()) {
                    path.extend(&along_outline(rings, last, end, meter)?);
                }
                let run = self.stitched(&path, circular, rng, meter)?;
                Ok(vec![finished(&run, &[], circular, meter)?])
            }
            several => {
                let mut lines = Vec::with_capacity(several.len());
                for line in several {
                    lines.push(self.stitched(line, circular, rng, meter)?);
                }
                let given = Given { travel: circular.travel, angle: circular.angle, skip_last: circular.skip_last };
                let runs = lines_sewn(piece, rings, &lines, &given, needle, end, meter, warnings)?.unwrap_or_default();
                runs.iter().map(|Marked { points, travel }| finished(points, travel, circular, meter)).collect()
            }
        }
    }

    /// The running stitch along `line` as `circular` stitches it, a span of it at a time.
    fn stitched(&mut self, line: &Spanned, circular: &Circular, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let Stitching { length, tolerance, jitter, .. } = circular.stitching;
        let lengths = (length, tolerance, circular.travel.min_stitch);
        let mut run: Vec<Point> = Vec::new();
        for span in line.spans() {
            self.spans += 1;
            let stitches = along_line_varied(span, lengths, jitter, (rng, self.spans), meter)?;
            run.extend(stitches.into_iter().skip(usize::from(!run.is_empty())));
        }
        Ok(run)
    }

    /// What the counts say: `SC-W0318` for the pieces sewn as one circle, and `SC-W0305` for those the spiral
    /// misses, sewn round their outline.
    fn said(&self) -> impl Iterator<Item = Diagnostic> {
        let small = match (self.small, self.pieces) {
            (0, _) => None,
            (1, 1) => Some("This circular fill lies within its row spacing of its middle, so it is sewn as one circle of that radius.".to_string()),
            (1, _) => Some(
                "1 of this circular fill's parts lies within the row spacing of its middle, so it is sewn as one circle of that radius.".to_string(),
            ),
            (n, _) => Some(format!(
                "{n} of this circular fill's parts lie within the row spacing of their middle, so each is sewn as one circle of that radius."
            )),
        };
        let missed = match (self.missed, self.pieces) {
            (0, _) => None,
            (1, 1) => Some("This circular fill's spiral misses it, so it is sewn as a running stitch round its outline.".to_string()),
            (1, _) => Some("This circular fill's spiral misses 1 of its parts, so it is sewn as a running stitch round its outline.".to_string()),
            (n, _) => Some(format!("This circular fill's spiral misses {n} of its parts, so each is sewn as a running stitch round its outline.")),
        };
        let small = small.map(|message| Diagnostic::new(Code::FillCircularTooSmall, message));
        small.into_iter().chain(missed.map(|message| Diagnostic::new(Code::FillTooThin, message)))
    }
}

/// `run` with its bean stitch, but where `travel` marks the needle travelling, and then its repeats.
fn finished(run: &[Point], travel: &[bool], circular: &Circular, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let beaned = passes::beaned(run, &circular.stitching.beans, travel, meter)?;
    passes::doubled(beaned, circular.stitching.repeats, meter)
}

/// How far the point of `piece`'s rings farthest from `middle` lies from it: GEOS's discrete Hausdorff
/// distance from the piece to the point, which Ink/Stitch sizes its circles by. One unit of `meter` for
/// each point.
fn farthest(piece: &Polygon, middle: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
    let mut farthest = 0.0_f64;
    for ring in piece.rings() {
        meter.charge(u64::try_from(ring.len()).unwrap_or(u64::MAX))?;
        farthest = ring.iter().map(|p| p.distance(middle)).fold(farthest, f64::max);
    }
    Ok(farthest)
}

/// The circle of `radius` round `middle` as GEOS buffers a point: 64 sides from its right-hand point,
/// turning up as drawn first, clockwise in y-up axes, and closed.
fn circle(middle: Point, radius: f64) -> Vec<Point> {
    let step = std::f64::consts::TAU / f64::from(SIDES);
    let mut circle: Vec<Point> = (0..SIDES)
        .map(|k| {
            let (sin, cos) = sin_cos(-(f64::from(k) * step));
            Point::new(middle.x() + radius * cos, middle.y() + radius * sin).unwrap_or(middle)
        })
        .collect();
    if let Some(&first) = circle.first() {
        circle.push(first);
    }
    circle
}

/// The circles round `middle`, the outermost first: from `spacing` out, until one lies 2 spacings past
/// `farthest`, the end spacing's where there is one, and the circle at the middle last. Without `end` each
/// lies `spacing` past the last. With it, each step is `spacing` plus the share of the way from `spacing` to
/// `end` that the circle's radius is of `farthest`, as Ink/Stitch grades them, and `end` past the farthest
/// point (`DEV-FILL-021`). One unit of `meter` for each point of a circle but the one at the middle, whose
/// number of points is fixed.
fn circles(middle: Point, farthest: f64, spacing: f64, end: Option<f64>, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let stop = farthest + 2.0 * end.unwrap_or(spacing);
    let mut circles = vec![circle(middle, MIDDLE)];
    let mut radius = spacing;
    while radius < stop {
        meter.charge(u64::from(SIDES + 1))?;
        circles.push(circle(middle, radius));
        radius += match end {
            Some(end) if radius < farthest => spacing + (end - spacing) * (radius / farthest),
            Some(end) => end,
            None => spacing,
        };
    }
    circles.reverse();
    Ok(circles)
}

/// The way along the outline of the part whose rings are `rings`, from its point nearest `from` to its
/// point nearest `to`, the shorter way round, as a line of one span: how a fill of one piece joins it to
/// the needle and the end (`DEV-FILL-022`). Empty when the part has no outline. Costs as
/// [`Rings::locate`] and [`Rings::way`] do.
fn along_outline(rings: &Rings<'_>, from: Point, to: Point, meter: &mut Meter) -> Result<Spanned, Exhausted> {
    let outline = |ring: usize| ring == 0;
    let (Some(a), Some(b)) = (rings.locate(from, outline, meter)?, rings.locate(to, outline, meter)?) else { return Ok(Spanned::default()) };
    let way = rings.way(0, a.at, b.at, rings.forwards(0, a.at, b.at), meter)?;
    Ok(Spanned { points: way, starts: Vec::new() })
}

#[cfg(test)]
mod tests;
