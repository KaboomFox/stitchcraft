//! Guided ripples (milestone M10.11): copies of a ripple stitch's line that follow a guide line, or lie
//! between a satin guide's rails, as Ink/Stitch places them (`docs/src/design/algorithms/strokes.md` ›
//! Ripple stitch › Guided ripples, `REQ-RIP-012` to `REQ-RIP-014`).
//!
//! **Why.** A ripple towards a target point shrinks its line to one point. A guide moves the copies
//! instead: along a path, growing or shrinking as they go, or across a band between 2 rails, so a line can
//! ripple along a letter's stroke or a leaf's vein.
//!
//! **Copies.** Each copy is the stroke's outline, its needle points or its nodes, moved, turned about a
//! point and scaled about it along x and y, in that order, as Ink/Stitch's affine helpers do it. A copy's
//! points keep the outline's order, so point `j` of every copy lies on one line across the ripple, the line
//! a grid sews.
//!
//! - **Along a guide line** (one subpath), copy `i` of `n` lies at its place along the guide,
//!   `(i / (n − 1))^exponent` of its length, flipped with `flip_exponent`, or at the guide's point `i` with
//!   `render_at_rungs`. It moves there from the guide's start, is scaled from `scale_start` to `scale_end`
//!   about that point, and with `rotate_ripples` turns as the guide turns from its start to there.
//! - **Between a satin guide's rails** (a satin column, or a guide of several subpaths), each copy is placed
//!   on a pair of points across the guide so that the outline's ends land on the pair's, or the anchor
//!   line's ends do. The pairs are the satin column's at an even spacing, its sections' ends with
//!   `render_at_rungs`, or every 0.1 CSS pixel with `adaptive`, a copy there kept when its box clears the
//!   last one's moved by the least distance. A closed line with no anchor line is centred on each pair's
//!   middle instead, scaled by the pair's width over the first pair's, and turned with it.
//!
//! Turns come from the directions of 2 lines, as the cosine and sine of the angle between them, with
//! arithmetic and square roots alone (`stitchcraft_core::math`), and a direction of no length points along x,
//! as an angle of 0 does.

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point};

use stitchcraft_params::{StitchType, toggle};

use super::RippleParams;
use super::lines::{self, Count, MOST};
use crate::design::{Guide, Join, Path, Subpath};
use crate::generators::satin::{self, SATIN_COLUMN, SatinParams, Section};
use crate::normalize::along::Along;
use crate::normalize::stroke;

/// How far along the guide its start's direction is taken, in millimetres: 0.1 CSS pixels, as Ink/Stitch
/// takes it.
const START_DIRECTION: f64 = 0.1 * MM_PER_SVG_PX;

/// How far apart the pairs an adaptive satin guide tries are, in millimetres: 0.1 CSS pixels, as in
/// Ink/Stitch.
const ADAPTIVE_STEP: f64 = 0.1 * MM_PER_SVG_PX;

/// The least distance an adaptive satin guide keeps between its copies' boxes without `min_line_dist_mm`,
/// in millimetres: 0.1 CSS pixels, as in Ink/Stitch.
const ADAPTIVE_GAP: f64 = 0.1 * MM_PER_SVG_PX;

/// The guide line of a guided ripple, as its generator reads it (`REQ-RIP-012`, `REQ-RIP-013`).
#[derive(Clone, Debug, PartialEq)]
pub enum RippleGuide {
    /// A line: the copies follow its first subpath, flattened as a shape's outline is, or through its nodes
    /// when `nodes` says so, for a guide sewn as a manual stitch.
    Line {
        /// The guide's path.
        path: Path,
        /// Whether its nodes are its points.
        nodes: bool,
    },
    /// A satin column, drawn with rails and rungs or as one path `width` wide with `join` at its corners:
    /// the copies lie between its rails, read as `params` say.
    Satin {
        /// The guide's path.
        path: Path,
        /// Its stroke's width.
        width: Mm,
        /// How its stroke turns its corners.
        join: Join,
        /// How its rails are read: its own settings, with the ripple's `reverse_rails` and
        /// `swap_satin_rails`.
        params: Box<SatinParams>,
    },
}

/// The guide a ripple's design gives it, `guide`, as its generator reads it, with the ripple's `own`
/// `reverse_rails` and `swap_satin_rails` for a satin guide's rails: a satin column when its own
/// `satin_column` is on or it has several subpaths, as Ink/Stitch reads a guide, and otherwise a line, its
/// nodes for a guide sewn as a manual stitch. `None`, with the reason in `diagnostics`, when a satin guide's
/// settings don't read.
pub fn guide(guide: &Guide, own: &RippleParams, diagnostics: &mut Vec<Diagnostic>) -> Option<RippleGuide> {
    let satin = guide.params.get(SATIN_COLUMN).and_then(toggle) == Some(true) || guide.path.subpaths.len() > 1;
    if !satin {
        let nodes = guide.params.get(STROKE_METHOD) == Some(StitchType::ManualStitch.id());
        return Some(RippleGuide::Line { path: guide.path.clone(), nodes });
    }
    match SatinParams::from_set_as(&guide.params, StitchType::SatinColumn) {
        Ok(read) => {
            diagnostics.extend(read.warnings);
            let params = SatinParams { reverse_rails: own.reverse_rails, swap_satin_rails: own.swap_satin_rails, ..read.params };
            Some(RippleGuide::Satin { path: guide.path.clone(), width: guide.width, join: guide.join, params: Box::new(params) })
        }
        Err(problems) => {
            diagnostics.extend(problems);
            diagnostics.push(unusable("its satin guide's settings don't read", "Correct the satin guide's settings."));
            None
        }
    }
}

/// The key of a guide's stroke method, whose manual stitch makes its nodes its points.
const STROKE_METHOD: &str = "stroke_method";

/// The copies a guide places, each the outline's points moved there, the first copy first, and how many
/// more there would have been where they were cut.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Copies {
    /// The copies.
    pub(super) lines: Vec<Vec<Point>>,
    /// What was cut, if anything.
    pub(super) cut: Option<Cut>,
}

/// Why a guided ripple sews fewer copies than asked for: at most [`MOST`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Cut {
    /// `min_line_dist_mm` gave this count.
    Distance(f64),
    /// The guide has this many places for copies: points of a guide line, rungs or clear places.
    Places(usize),
}

/// How the ripple's settings place the copies along a guide line, and whether its line is closed.
pub(super) struct Placing<'a> {
    /// The ripple's settings.
    pub(super) own: &'a RippleParams,
    /// Whether a closed line or the point join adds a line to the count.
    pub(super) adds: bool,
    /// Whether the stroke's line is closed.
    pub(super) closed: bool,
    /// The anchor line's ends, when the design gives one.
    pub(super) anchor: Option<[Point; 2]>,
}

/// The copies of `outline` that `guide` places as `placing` says, or `None` when it places none: what is
/// wrong with the guide then goes to `warnings`, and the ripple sews without it. Every point of every copy
/// costs a unit of `meter`'s work, and the copies must fit in the stitches left.
pub(super) fn copies(
    outline: &[Point],
    guide: &RippleGuide,
    placing: &Placing,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Copies>, Exhausted> {
    match guide {
        RippleGuide::Line { path, nodes } => {
            let Some(line) = guide_line(path, *nodes, meter)? else {
                warnings.push(unusable("its guide line has no length", DRAW_GUIDE));
                return Ok(None);
            };
            along_line(outline, &line, placing, meter).map(Some)
        }
        RippleGuide::Satin { path, width, join, params } => {
            let mut said = Vec::new();
            let rails = satin::rails(path, *width, *join, &mut said, meter)?;
            let sections = match &rails {
                Some(rails) => satin::ripple_sections(rails, params, &mut said, meter)?,
                None => Vec::new(),
            };
            warnings.extend(said);
            if sections.is_empty() {
                warnings.push(unusable("its satin guide has no rails to place copies between", DRAW_GUIDE));
                return Ok(None);
            }
            between_rails(outline, &sections, params, placing, warnings, meter)
        }
    }
}

/// The points a guide line of `path` gives: its first subpath flattened as a shape's outline is, or its
/// nodes when `nodes`; `None` when they have no length.
fn guide_line(path: &Path, nodes: bool, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let points = if nodes {
        path.subpaths.iter().map(node_points).find(|points| points.len() > 1).unwrap_or_default()
    } else {
        match path.subpaths.first() {
            Some(first) => stroke::flattened(first, meter)?,
            None => Vec::new(),
        }
    };
    let moves = points.windows(2).any(|pair| pair.first() != pair.last());
    Ok(moves.then_some(points))
}

/// The nodes of `subpath`: its start and each segment's end, and its start again where it closes elsewhere.
pub(super) fn node_points(subpath: &Subpath) -> Vec<Point> {
    let mut points: Vec<Point> = std::iter::once(subpath.start).chain(subpath.segments.iter().map(|segment| segment.end())).collect();
    if subpath.closed && points.last() != Some(&subpath.start) {
        points.push(subpath.start);
    }
    points
}

/// The copies of `outline` along the guide line `line` (`REQ-RIP-012`).
fn along_line(outline: &[Point], line: &[Point], placing: &Placing, meter: &mut Meter) -> Result<Copies, Exhausted> {
    let own = placing.own;
    let along = Along::new(line, meter)?;
    let rungs = own.satin_guide_pattern_position == RUNGS;
    let (count, cut) = if rungs { capped(line.len()) } else { counted(lines::count(own, along.length(), (placing.adds, false))) };
    let places = lines::places(count, own.exponent, own.flip_exponent);
    let scales = spaced(own.scale_start / 100.0, own.scale_end / 100.0, count);
    let start = line.first().copied().unwrap_or(Point::ORIGIN);
    let start_direction = direction(start, at(&along, START_DIRECTION));
    room(outline, count, meter)?;
    let mut copies = Vec::with_capacity(count);
    let mut previous: Option<Point> = None;
    for (i, &scale) in scales.iter().enumerate() {
        let point = if rungs { line.get(i).copied().unwrap_or(start) } else { at(&along, places.get(i).copied().unwrap_or(0.0) * along.length()) };
        let turn = match previous {
            Some(before) if own.rotate_ripples => between(start_direction, direction(before, point)),
            _ => [1.0, 0.0],
        };
        let move_by = [point.x() - start.x(), point.y() - start.y()];
        copies.push(placed(outline, Place { move_by, turn, scale: axes(own.scale_axis, scale), about: point }, meter)?);
        previous = Some(point);
    }
    Ok(Copies { lines: copies, cut })
}

/// The copies of `outline` between the rails of a satin guide cut into `sections`, whose own settings are
/// `params` (`REQ-RIP-013`, `REQ-RIP-014`); `None` when they can't be placed, which `warnings` says.
fn between_rails(
    outline: &[Point],
    sections: &[Section],
    params: &SatinParams,
    placing: &Placing,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Copies>, Exhausted> {
    let anchor = match placing.anchor {
        Some([from, to]) if from == to => {
            let hint = "Draw the anchor line across the ripple's line, from where its first end should lie to where its last should.";
            warnings.push(unusable("its anchor line's ends are one point", hint));
            None
        }
        anchor => anchor,
    };
    let ends = anchor.or_else(|| Some([*outline.first()?, *outline.last()?]));
    let Some([from, to]) = ends.filter(|_| anchor.is_some() || !placing.closed) else {
        return centred(outline, sections, params, placing, warnings, meter);
    };
    let width = from.distance(to);
    let base = direction(from, to);
    let own = placing.own;
    if own.satin_guide_pattern_position == ADAPTIVE {
        return Ok(Some(adaptive(outline, sections, (from, width, base), own, meter)?));
    }
    let (mut pairs, cut) = if own.satin_guide_pattern_position == RUNGS {
        let (lines, cut) = capped(sections.len());
        (section_ends(sections).take(lines).collect(), cut)
    } else {
        match spread(sections, params, placing, meter)? {
            Some(spread) => spread,
            None => {
                warnings.push(unusable("its satin guide's middle has no length", DRAW_GUIDE));
                return Ok(None);
            }
        }
    };
    // A guide whose rails close places its first pair again at its end, and that copy is left out, as
    // Ink/Stitch leaves it out. So is the only pair of a guide too short for 2.
    if pairs.first() == pairs.last() {
        pairs.pop();
    }
    if pairs.is_empty() {
        warnings.push(unusable("its satin guide is too short to place a copy on", DRAW_GUIDE));
        return Ok(None);
    }
    room(outline, pairs.len(), meter)?;
    let mut copies = Vec::with_capacity(pairs.len());
    for [a, b] in pairs {
        let place = Place {
            move_by: [a.x() - from.x(), a.y() - from.y()],
            turn: between(base, direction(a, b)),
            scale: [ratio(a.distance(b), width); 2],
            about: a,
        };
        copies.push(placed(outline, place, meter)?);
    }
    Ok(Some(Copies { lines: copies, cut }))
}

/// The pairs a satin guide places its copies on at an even spacing, as many as `line_count` or
/// `min_line_dist_mm` asks for along the guide's middle, with the count asked for when it was cut. `None`
/// when the guide's middle has no length.
#[allow(clippy::cast_precision_loss)] // Copy counts are far below 2⁵².
fn spread(sections: &[Section], params: &SatinParams, placing: &Placing, meter: &mut Meter) -> Result<Option<Spread>, Exhausted> {
    let middle = middle(sections, params, meter)?;
    if middle <= 0.0 {
        return Ok(None);
    }
    let (count, cut) = counted(lines::count(placing.own, middle, (placing.adds, false)));
    let spacing = middle / count.saturating_sub(1).max(1) as f64;
    Ok(Some((satin::ripple_pairs(sections, spacing, 0.0, meter)?, cut)))
}

/// Pairs a satin guide places its copies on, and what was cut of their count.
type Spread = (Vec<[Point; 2]>, Option<Cut>);

/// How long a satin guide's middle is: the line through the middles of its pairs at its own zigzag
/// spacing, as Ink/Stitch measures it to count its copies.
fn middle(sections: &[Section], params: &SatinParams, meter: &mut Meter) -> Result<f64, Exhausted> {
    let middles = satin::ripple_pairs(sections, params.zigzag_spacing_mm.get(), 0.5, meter)?;
    Ok(middles.windows(2).filter_map(|pair| Some(pair.first()?[0].distance(pair.last()?[0]))).sum())
}

/// The end of each of `sections`, as a pair: where its copies lie with `render_at_rungs`.
fn section_ends(sections: &[Section]) -> impl Iterator<Item = [Point; 2]> + '_ {
    sections.iter().filter_map(|[a, b]| Some([*a.last()?, *b.last()?]))
}

/// The copies of a closed `outline` with no anchor line between the rails cut into `sections`, each centred
/// on a pair's middle, scaled by its width over the first pair's and turned with it as `rotate_ripples`
/// says, on `scale_axis`. `None` when the first pair has no width, which `warnings` says.
fn centred(
    outline: &[Point],
    sections: &[Section],
    params: &SatinParams,
    placing: &Placing,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Copies>, Exhausted> {
    let own = placing.own;
    let Some((spread, spread_cut)) = spread(sections, params, placing, meter)? else {
        warnings.push(unusable("its satin guide's middle has no length", DRAW_GUIDE));
        return Ok(None);
    };
    let at_rungs: Vec<[Point; 2]> = section_ends(sections).collect();
    let (pairs, cut) = match at_rungs.split_last() {
        // The last section's end is the guide's end, which no rung marks.
        Some((_, rungs)) if own.satin_guide_pattern_position == RUNGS && !rungs.is_empty() => {
            let (lines, cut) = capped(rungs.len());
            (rungs.iter().take(lines).copied().collect(), cut)
        }
        _ => (spread, spread_cut),
    };
    let Some(&[a, b]) = pairs.first() else { return Ok(None) };
    let (first_width, base) = (a.distance(b), direction(a, b));
    if first_width <= 0.0 {
        warnings.push(unusable("its satin guide's first pair of points has no width", DRAW_GUIDE));
        return Ok(None);
    }
    let Some(centre) = centroid(outline) else { return Ok(None) };
    room(outline, pairs.len(), meter)?;
    let mut copies = Vec::with_capacity(pairs.len());
    for [a, b] in pairs {
        // The pair's middle, as `(a + b) / 2` gives it.
        let Ok(middle) = Point::new((a.x() + b.x()) * 0.5, (a.y() + b.y()) * 0.5) else { continue };
        let turn = if own.rotate_ripples { between(base, direction(a, b)) } else { [1.0, 0.0] };
        let scale = axes(own.scale_axis, a.distance(b) / first_width);
        copies.push(placed(outline, Place { move_by: [middle.x() - centre.x(), middle.y() - centre.y()], turn, scale, about: middle }, meter)?);
    }
    Ok(Some(Copies { lines: copies, cut }))
}

/// The copies of `outline` an adaptive satin guide places: on pairs every [`ADAPTIVE_STEP`] along
/// `sections`, each where the box round the outline, turned to lie along the anchor, clears the last copy's
/// box moved back by the least distance. `ends` are the anchor's first end, its length and its direction.
fn adaptive(
    outline: &[Point],
    sections: &[Section],
    (from, width, base): (Point, f64, [f64; 2]),
    own: &RippleParams,
    meter: &mut Meter,
) -> Result<Copies, Exhausted> {
    // The outline turned about the anchor's first end so that the anchor runs along x.
    let level = Place { move_by: [0.0, 0.0], turn: [base[0], -base[1]], scale: [1.0, 1.0], about: from };
    let outline = placed(outline, level, meter)?;
    let (low, high) = bounds(&outline);
    let gap = own.min_line_dist_mm.map_or(ADAPTIVE_GAP, Mm::get);
    let boxed = |[lx, ly]: [f64; 2], [hx, hy]: [f64; 2]| [[lx, ly], [lx, hy], [hx, hy], [hx, ly]];
    let (whole, kept) = (boxed(low, high), boxed([low[0], low[1] - gap], [high[0], high[1] - gap]));
    let pairs = satin::ripple_pairs(sections, ADAPTIVE_STEP, 0.0, meter)?;
    let (mut copies, mut placed_at): (Vec<Vec<Point>>, usize) = (Vec::new(), 0);
    let mut last: Option<[[f64; 2]; 4]> = None;
    for [a, b] in pairs {
        meter.charge(1)?;
        let place = Place { move_by: [a.x() - from.x(), a.y() - from.y()], turn: direction(a, b), scale: [ratio(a.distance(b), width); 2], about: a };
        if last.is_some_and(|last| !disjoint(&last, &place.corners(&whole))) {
            continue;
        }
        // Past the most copies, the places are only counted.
        placed_at += 1;
        if placed_at <= MOST {
            meter.room(outline.len())?;
            copies.push(placed(&outline, place, meter)?);
        }
        last = Some(place.corners(&kept));
    }
    Ok(Copies { lines: copies, cut: capped(placed_at).1 })
}

/// How many copies `points` places hold, at most [`MOST`], and the places when there are more.
fn capped(points: usize) -> (usize, Option<Cut>) {
    (points.min(MOST), (points > MOST).then_some(Cut::Places(points)))
}

/// A count of copies, and what `min_line_dist_mm` asked for when it was cut.
fn counted(Count { lines, cut }: Count) -> (usize, Option<Cut>) {
    (lines, cut.map(Cut::Distance))
}

/// Where a copy is placed: moved by `move_by`, turned about `about` by the angle whose cosine and sine are
/// `turn`, and scaled about it by `scale` along x and along y.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Place {
    move_by: [f64; 2],
    turn: [f64; 2],
    scale: [f64; 2],
    about: Point,
}

impl Place {
    /// `(x, y)` placed, as plain numbers.
    fn apply(&self, [x, y]: [f64; 2]) -> [f64; 2] {
        let [cos, sin] = self.turn;
        let (cx, cy) = (self.about.x(), self.about.y());
        let (dx, dy) = (x + self.move_by[0] - cx, y + self.move_by[1] - cy);
        let (tx, ty) = (cos * dx - sin * dy, sin * dx + cos * dy);
        [cx + self.scale[0] * tx, cy + self.scale[1] * ty]
    }

    /// The corners of a box, placed.
    fn corners(&self, corners: &[[f64; 2]; 4]) -> [[f64; 2]; 4] {
        corners.map(|corner| self.apply(corner))
    }
}

/// `outline` placed as `place` says, at a unit of `meter`'s work per point. A point placed beyond what a
/// number holds stays where it was, which only a scale no setting allows can make.
fn placed(outline: &[Point], place: Place, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    meter.charge(u64::try_from(outline.len()).unwrap_or(u64::MAX))?;
    Ok(outline
        .iter()
        .map(|&q| {
            let [x, y] = place.apply([q.x(), q.y()]);
            Point::new(x, y).unwrap_or(q)
        })
        .collect())
}

/// Whether `count` copies of `outline` fit in `meter`'s stitches left.
fn room(outline: &[Point], count: usize, meter: &mut Meter) -> Result<(), Exhausted> {
    meter.room(outline.len().saturating_mul(count))
}

/// The point `distance` along `along`, at its ends beyond them.
fn at(along: &Along, distance: f64) -> Point {
    along.point(distance.clamp(0.0, along.length()))
}

/// The direction from `a` to `b` as a unit vector, or along x where they are one point: an angle of 0, as
/// `atan2(0, 0)` is.
fn direction(a: Point, b: Point) -> [f64; 2] {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let length = (dx * dx + dy * dy).sqrt();
    if length > 0.0 { [dx / length, dy / length] } else { [1.0, 0.0] }
}

/// The turn from the direction `from` to the direction `to`, as its cosine and sine.
fn between([fx, fy]: [f64; 2], [tx, ty]: [f64; 2]) -> [f64; 2] {
    [fx * tx + fy * ty, fx * ty - fy * tx]
}

/// `scale` along the axes `scale_axis` names: "0" both, "1" x alone, "2" y alone and "3" neither.
fn axes(scale_axis: &str, scale: f64) -> [f64; 2] {
    match scale_axis {
        "1" => [scale, 1.0],
        "2" => [1.0, scale],
        "3" => [1.0, 1.0],
        _ => [scale, scale],
    }
}

/// `length` over `width`, 0 where the width is 0.
fn ratio(length: f64, width: f64) -> f64 {
    if width > 0.0 { length / width } else { 0.0 }
}

/// `n` numbers spread evenly from `start` to `end`, as `numpy.linspace` spreads them: step `k` of `n − 1`,
/// the last exactly `end`. One number is `start`.
#[allow(clippy::cast_precision_loss)] // Copy counts are far below 2⁵².
fn spaced(start: f64, end: f64, n: usize) -> Vec<f64> {
    let step = (end - start) / n.saturating_sub(1).max(1) as f64;
    (0..n).map(|k| if k + 1 == n && n > 1 { end } else { start + k as f64 * step }).collect()
}

/// The smallest and largest x and y of `points`.
fn bounds(points: &[Point]) -> ([f64; 2], [f64; 2]) {
    let first = points.first().map_or([0.0, 0.0], |q| [q.x(), q.y()]);
    points.iter().fold((first, first), |([lx, ly], [hx, hy]), q| ([lx.min(q.x()), ly.min(q.y())], [hx.max(q.x()), hy.max(q.y())]))
}

/// The centroid of the polyline `points`, as GEOS finds a line's: the middles of its segments weighed by
/// their lengths, or the mean of its points where it has no length. `None` without a point.
fn centroid(points: &[Point]) -> Option<Point> {
    let (mut total, mut x, mut y) = (0.0, 0.0, 0.0);
    for (a, b) in stroke::segments(points) {
        let length = a.distance(b);
        let middle = a.lerp(b, 0.5);
        (total, x, y) = (total + length, x + middle.x() * length, y + middle.y() * length);
    }
    if total > 0.0 {
        return Point::new(x / total, y / total).ok();
    }
    #[allow(clippy::cast_precision_loss)] // Point counts are far below 2⁵².
    let n = points.len() as f64;
    let (sx, sy) = points.iter().fold((0.0, 0.0), |(sx, sy), q| (sx + q.x(), sy + q.y()));
    Point::new(sx / n, sy / n).ok()
}

/// Whether the convex quadrilaterals `a` and `b` share no point: some side's line, or the line square to
/// it, has the one on each side of it, by their corners' shadows on it. Boxes that touch share a point.
fn disjoint(a: &[[f64; 2]; 4], b: &[[f64; 2]; 4]) -> bool {
    let sides = |corners: &[[f64; 2]; 4]| -> Vec<[f64; 2]> {
        (0..4).filter_map(|k| Some((corners.get(k)?, corners.get((k + 1) % 4)?))).map(|([ax, ay], [bx, by])| [bx - ax, by - ay]).collect()
    };
    let mut axes: Vec<[f64; 2]> = Vec::with_capacity(16);
    for [dx, dy] in sides(a).into_iter().chain(sides(b)).filter(|[dx, dy]| *dx != 0.0 || *dy != 0.0) {
        axes.extend([[dx, dy], [-dy, dx]]);
    }
    if axes.is_empty() {
        return a.first() != b.first();
    }
    let shadow = |corners: &[[f64; 2]; 4], [ux, uy]: [f64; 2]| {
        corners.iter().map(|[x, y]| x * ux + y * uy).fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), d| (low.min(d), high.max(d)))
    };
    axes.into_iter().any(|axis| {
        let ((a_low, a_high), (b_low, b_high)) = (shadow(a, axis), shadow(b, axis));
        a_high < b_low || b_high < a_low
    })
}

/// `SC-W0409`: a ripple stitch's guide or anchor line can't place its copies, because `why`, so the ripple
/// sews without it, with a `hint`.
fn unusable(why: &str, hint: &str) -> Diagnostic {
    let message = format!("This ripple stitch's guide can't be used: {why}. It is sewn without it.");
    Diagnostic::new(Code::RippleGuideUnusable, message).with_fix(Fix::Hint(hint.to_string()))
}

/// How to draw a guide that places copies.
const DRAW_GUIDE: &str = "Draw the guide as a line with length, or as a satin column of 2 rails, grouped with the ripple.";

/// `satin_guide_pattern_position`'s option that places copies at a satin guide's rungs, or at a guide
/// line's points.
const RUNGS: &str = "render_at_rungs";

/// `satin_guide_pattern_position`'s option that places copies where they clear each other.
const ADAPTIVE: &str = "adaptive";
