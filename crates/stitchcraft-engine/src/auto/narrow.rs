//! The parts of an automatic area sorted by the rules ([`sorted`], `auto::rules`): the lines, each sewn as a
//! running stitch along its centre line, tripled from 0.5 mm wide (`REQ-AUTO-013` to `REQ-AUTO-015`), the
//! parts sewn as columns (`auto::columns`, `REQ-AUTO-016`), and the rest, which stay the area's fill: its
//! broad parts, its compact and uneven parts, and its specks.
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Which stitch a shape gets, and
//! `docs/src/design/algorithms/centre-lines.md` › Width along the line. A line traced from a picture is an
//! area, and filled, an area that narrow sews rows no longer than it is wide wherever they cross it. So a
//! part of an area that is narrower than the design's narrowest satin stroke everywhere sews as a running
//! stitch along its middle, as a stroke that narrow does:
//!
//! - **Narrow.** A part is narrower than a width everywhere when, shrunk by half that width, it leaves no
//!   area: no circle that wide fits inside it ([`within`]). It is shrunk with the polygon buffer that sews a
//!   fill's inset underlay (`ops::buffered`), by half of the width taken half a micrometre wider, so that a
//!   part as wide as the width, give or take a rounding error, is narrow, as a stroke is when its width is
//!   compared to the micrometre. A part whose area is more than its perimeter times that half, plus π times
//!   its square for each hole past the first, holds a wider circle somewhere: no part that narrow covers
//!   more. It isn't shrunk, which would only confirm it at the cost of its whole outline.
//! - **Sorted.** A speck stays in the area to be merged into a neighbour (`REQ-AUTO-008`). A part narrower
//!   than the design's narrowest satin stroke everywhere is a line. One wider than the widest satin column
//!   somewhere is an area, split into its fill and the narrow pieces that leave it (`auto::split`), each
//!   sorted in its turn. Any other goes by its columns: compact or uneven, it stays in the fill, an uneven
//!   part without its tails, which leave it as lines. Each part's rule is recorded for `SC-I0912`
//!   ([`Parts::fired`]).
//! - **A line.** Its centre lines ([`strips`]) come from its triangulation, its edges cut into pieces no
//!   longer than it is wide on average, twice its area over its perimeter, or [`FINEST`] if that is longer.
//!   A part whose centre lines have no length, a dot, stays in the area.
//! - **Its width** is that of the largest circle that fits inside it centred on its centre lines, twice the
//!   distance from their points to its outline and holes, averaged along them, and its spread is how much
//!   that varies ([`spread`]). The width says whether the part is a triple run ([`super::tripled`]), and both
//!   say whether it is compact or uneven. The part's edges are put in the cells of a grid as fine as the
//!   pieces it is cut into ([`Edges`]), and each point's nearest edge is looked for in the cells round it,
//!   ring by ring, until no edge farther out could be nearer: a narrow part's edges lie a cell or so from
//!   its middle, so measuring each point costs a few units of work, not one for each edge of the part.

use std::collections::BTreeMap;

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Meter, Mm, Point};

use super::columns::{self, Columns, Made};
use super::rules::{Asked, Bounds, Breadth, Fired, Measures, RuleId, choose};
use super::split;
use super::{AutoParams, no_area};
use crate::normalize::region::Polygon;
use crate::normalize::region::ops::{self, area_of};
use crate::normalize::strips::{self, Column, Line};
use crate::normalize::stroke::{distance_to_segment, segments};

/// The shortest piece a narrow part's edges are cut into for its triangulation, in millimetres: a part
/// thinner than this on average is cut no finer, which bounds its points by its perimeter.
const FINEST: f64 = 0.1;

/// A narrow part of an area.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Narrow {
    /// The part.
    pub(super) part: Polygon,
    /// Its centre lines, in the order they are sewn.
    pub(super) lines: Vec<Line>,
    /// Its width along its centre lines, on average.
    pub(super) width: Mm,
}

/// An area's parts, sorted by the rules.
#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct Parts {
    /// The parts that stay the area's fill: its broad parts, its compact and uneven parts, and its specks.
    pub(super) filled: Vec<Polygon>,
    /// Its parts sewn along their middles, narrow ones and those sewn as columns, in the order of its parts.
    pub(super) along: Vec<Along>,
    /// `SC-E0009` for each part whose width or centre line could not be found, a bug, and `SC-I0902` for each
    /// part whose satin columns are refused; the part stays in the fill.
    pub(super) said: Vec<Diagnostic>,
    /// The rule each part sewed by, and each piece that left a fill, in the order they were judged.
    pub(super) fired: Vec<Fired>,
}

/// A part sewn along its middle.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Along {
    /// A narrow part, a running stitch along its centre lines.
    Narrow(Narrow),
    /// A part no wider than a satin column, sewn as columns.
    Columns(Columns),
}

/// `parts`, the automatic area `id`'s, sorted by the rules (`auto::rules`) under `bounds`, its shape's, whose
/// settings of the rules are `rules`: the lines, with their centre lines, the parts sewn as columns, with
/// their columns, and the rest, which is filled. Finding a part's width, centre line and columns is part of
/// the work of digitizing the area, and costs `meter`'s.
pub(super) fn sorted(id: &ElementId, parts: Vec<Polygon>, bounds: &Bounds, rules: &AutoParams, meter: &mut Meter) -> Result<Parts, Exhausted> {
    let mut sorted = Parts::default();
    for part in parts {
        sort(id, part, bounds, rules, Sorting::Part, &mut sorted, meter)?;
    }
    Ok(sorted)
}

/// What is sorted.
#[derive(Clone, Copy)]
enum Sorting<'c> {
    /// A part of the area, which splits into its broad and narrow parts when it is wider than a satin column
    /// somewhere, and into its fill and its tails when it is uneven.
    Part,
    /// A narrow piece of a part split, cut along these chords where it meets the broad parts, which stays in
    /// the fill when it is wider than a satin column somewhere, and splits into its fill and its tails when it
    /// is uneven.
    Piece(&'c [[Point; 2]]),
    /// A tail of an uneven part, cut along these chords where it meets the part's fill: it is split no
    /// further, and stays in the fill unless it sews as a line or as columns.
    Tail(&'c [[Point; 2]]),
}

impl Sorting<'_> {
    /// The chords the piece was cut along: none for a part.
    fn cuts(&self) -> &[[Point; 2]] {
        match self {
            Sorting::Part => &[],
            Sorting::Piece(cuts) | Sorting::Tail(cuts) => cuts,
        }
    }
}

/// `part`, the automatic area `id`'s, sorted into `sorted` by the rules under `bounds`, with `meter`'s work,
/// and the rule it sewed by recorded: an area split into its fill and the narrow pieces that leave it
/// ([`split`]) when `sorting` says it is a part of the area, and an uneven part into its fill and its tails
/// unless it is a tail itself.
fn sort(
    id: &ElementId,
    part: Polygon,
    bounds: &Bounds,
    rules: &AutoParams,
    sorting: Sorting,
    sorted: &mut Parts,
    meter: &mut Meter,
) -> Result<(), Exhausted> {
    meter.charge(1)?;
    let fired = |rule: RuleId, measures: Option<Measures>| Fired { rule, measures };
    match lines(&part, bounds, meter)? {
        Found::Lines(lines, rule) => {
            let edges = Edges::of(&part, piece(&part), meter)?;
            match width(&edges, &lines, meter)? {
                Some(width) => {
                    let measures = Measures { width: width.get(), ..Measures::of_breadth(part.area(), Breadth::Narrow) };
                    sorted.fired.push(fired(rule, Some(measures)));
                    sorted.along.push(Along::Narrow(Narrow { part, lines, width }));
                }
                None => {
                    sorted.fired.push(fired(RuleId::Compact, None));
                    sorted.filled.push(part);
                }
            }
        }
        Found::Columns => match columns::columns(&part, bounds, rules, sorting.cuts(), meter)? {
            Made::Columns { sewn, rule, measures } => {
                sorted.fired.push(fired(rule, Some(measures)));
                sorted.along.push(Along::Columns(Columns { part, sewn }));
            }
            Made::Filled(RuleId::Uneven, measures) if !matches!(sorting, Sorting::Tail(_)) => {
                sorted.fired.push(fired(RuleId::Uneven, measures));
                // Its tails, narrower than a satin stroke, leave the fill as lines.
                let Some(split) = split::tails(&part, bounds.narrowest.get(), bounds.min_area, meter)? else { return unsplit(sorted, part, id) };
                sorted.filled.extend(split.fill);
                for tail in split.narrow {
                    sort(id, tail.part, bounds, rules, Sorting::Tail(&tail.cuts), sorted, meter)?;
                }
            }
            Made::Filled(rule, measures) => {
                sorted.fired.push(fired(rule, measures));
                sorted.filled.push(part);
            }
            Made::Long(stitch, measures) => {
                sorted.fired.push(fired(RuleId::Refused, Some(measures)));
                kept(sorted, part, columns::long(id, stitch, bounds));
            }
            Made::Crowded(most, measures) => {
                sorted.fired.push(fired(RuleId::Refused, Some(measures)));
                kept(sorted, part, columns::piled(id, most));
            }
            Made::Unmade => {
                sorted.fired.push(fired(RuleId::Refused, None));
                kept(sorted, part, columns::unmade(id, bounds));
            }
            Made::Broken => kept(sorted, part, broken(id, LOST_COLUMNS)),
        },
        Found::Broad(measures) if matches!(sorting, Sorting::Part) => {
            sorted.fired.push(fired(RuleId::Area, Some(measures)));
            let (broad, keep) = (bounds.max_satin.get(), bounds.min_fill);
            let Some(split) = split::split(&part, broad, keep, bounds.min_area, meter)? else { return unsplit(sorted, part, id) };
            sorted.filled.extend(split.fill);
            for piece in split.narrow {
                sort(id, piece.part, bounds, rules, Sorting::Piece(&piece.cuts), sorted, meter)?;
            }
        }
        Found::Broad(measures) => {
            sorted.fired.push(fired(RuleId::Area, Some(measures)));
            sorted.filled.push(part);
        }
        Found::Filled(rule, measures) => {
            sorted.fired.push(fired(rule, Some(measures)));
            sorted.filled.push(part);
        }
        Found::Broken(failed) => kept(sorted, part, broken(id, failed)),
    }
    Ok(())
}

/// `part`, the automatic area `id`'s, kept in `sorted`'s fill: its fill and the pieces that leave it couldn't
/// be found, a bug, which is said.
fn unsplit(sorted: &mut Parts, part: Polygon, id: &ElementId) -> Result<(), Exhausted> {
    kept(sorted, part, broken(id, UNSPLIT));
    Ok(())
}

/// `part` kept in `sorted`'s fill, and `said` said of the area: a part whose stitches couldn't be found.
fn kept(sorted: &mut Parts, part: Polygon, said: Diagnostic) {
    sorted.said.push(said);
    sorted.filled.push(part);
}

/// What becomes of a part.
enum Found {
    /// It sews along these centre lines, unless they have no length, by this rule: a line, or a part of a
    /// shape that asks to run.
    Lines(Vec<Line>, RuleId),
    /// It is no wider than a satin column, and its columns decide.
    Columns,
    /// It is wider than a satin column somewhere: an area.
    Broad(Measures),
    /// It stays in the fill by this rule: a speck, or a part of a shape that asks to be filled.
    Filled(RuleId, Measures),
    /// What could not be found of it, and why, a bug.
    Broken(&'static str),
}

/// What becomes of `part`, an area's held to `bounds`, with `meter`'s work: a speck stays in the fill, and so
/// does a part of a shape that asks to be filled; a part nowhere wider than the narrowest satin stroke, and
/// any part of a shape that asks to run, sews along its centre lines; and one no wider than the widest satin
/// column goes by its columns.
fn lines(part: &Polygon, bounds: &Bounds, meter: &mut Meter) -> Result<Found, Exhausted> {
    // A speck needs no measuring, and neither does a part of a shape that names a stitch any part can sew as.
    let unmeasured = Measures::of_breadth(part.area(), Breadth::Narrow);
    let rule = match choose(&unmeasured, bounds) {
        RuleId::Speck => return Ok(Found::Filled(RuleId::Speck, unmeasured)),
        RuleId::Asked if bounds.asked == Asked::Fill => return Ok(Found::Filled(RuleId::Asked, unmeasured)),
        RuleId::Asked => RuleId::Asked,
        _ => {
            let Some(breadth) = bounds.breadth_of(part, meter)? else { return Ok(Found::Broken(UNJOINED)) };
            let measures = Measures::of_breadth(part.area(), breadth);
            match breadth {
                Breadth::Narrow => RuleId::Line,
                Breadth::Medium => return Ok(Found::Columns),
                Breadth::Broad => return Ok(Found::Broad(measures)),
            }
        }
    };
    match strips::centre_lines(part, piece(part), meter)? {
        Ok(lines) => Ok(Found::Lines(lines, rule)),
        Err(_) => Ok(Found::Broken(LOST)),
    }
}

/// Whether `part` is narrower than `width` everywhere, measured to the micrometre: shrunk by half of the
/// width taken half a micrometre wider, it leaves no area. `None` when the shrunk part can't be joined, a
/// bug.
pub(super) fn within(part: &Polygon, width: Mm, meter: &mut Meter) -> Result<Option<bool>, Exhausted> {
    let half = half(width.get());
    let holes = part.holes.len().saturating_sub(1) as f64;
    if part.area() > perimeter(part) * half + std::f64::consts::PI * holes * half * half {
        return Ok(Some(false));
    }
    let Ok(shrunk) = ops::buffered(std::slice::from_ref(part), -half, meter)? else { return Ok(None) };
    Ok(Some(no_area(area_of(&shrunk))))
}

/// Half of `width` millimetres taken half a micrometre wider: how far a part is shrunk to tell whether it
/// is narrower than `width` everywhere, so that a part as wide as `width`, give or take a rounding error, is.
pub(super) fn half(width: f64) -> f64 {
    (width + 0.0005) / 2.0
}

/// How long the pieces of `part`'s edges are, for its triangulation and for the grid its width is measured
/// with: as long as the part is wide on average, twice its area over its perimeter, or [`FINEST`] if that is
/// longer.
pub(super) fn piece(part: &Polygon) -> f64 {
    (2.0 * part.area() / perimeter(part)).max(FINEST)
}

/// The length of `part`'s outline and holes, in millimetres.
fn perimeter(part: &Polygon) -> f64 {
    part.rings().flat_map(segments).map(|(a, b)| a.distance(b)).sum()
}

/// The width of a part along its centre lines, and how much it varies.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Spread {
    /// The width, averaged along the lines.
    pub(super) mean: Mm,
    /// The standard deviation of the width along the lines, in millimetres.
    pub(super) deviation: f64,
}

/// The width of a part along `lines`, its centre lines, whose edges `edges` holds, and how much it varies: the
/// diameter of the largest circle that fits inside it centred on each of their points, twice the point's
/// distance from the nearest edge of its outline and holes, averaged along the lines, and the standard
/// deviation of that diameter along them, which runs straight from one point of a line to the next. A line
/// that ends at a tip runs on into the corner so
/// that its stitches reach it, where the circle shrinks to nothing: the piece that leads into it is left out,
/// unless the lines have no other. Counted, it pulled a leg of a letter A 1.08 mm wide under 1 mm, and the leg
/// sewed as a line beside the other leg's satin column (`req_auto_016_a_letters_legs_sew_alike`). `None` for
/// lines of no length, a dot's. A unit of `meter`'s work for each point and edge.
pub(super) fn spread(edges: &Edges, lines: &[Line], meter: &mut Meter) -> Result<Option<Spread>, Exhausted> {
    // The width times the length, the width squared times the length, and the length, of the pieces that lead
    // into no tip, then of all of them.
    let (mut kept, mut all) = ([0.0; 3], [0.0; 3]);
    for line in lines {
        let mut across = Vec::with_capacity(line.points.len());
        for &p in &line.points {
            across.push(2.0 * edges.nearest(p, meter)?);
        }
        let last = line.points.len().saturating_sub(1);
        let closing = line.closed.then_some((last, 0));
        for (i, j) in (1..line.points.len()).map(|j| (j - 1, j)).chain(closing) {
            let (Some(a), Some(b), Some(wa), Some(wb)) = (line.points.get(i), line.points.get(j), across.get(i), across.get(j)) else { continue };
            let piece = a.distance(*b);
            // Along a piece the width runs straight from one end's to the other's.
            let sums = [piece * (wa + wb) / 2.0, piece * (wa * wa + wa * wb + wb * wb) / 3.0, piece];
            all = [all[0] + sums[0], all[1] + sums[1], all[2] + sums[2]];
            let into_tip = !line.closed && ((i == 0 && *wa <= TIP) || (j == last && *wb <= TIP));
            if !into_tip {
                kept = [kept[0] + sums[0], kept[1] + sums[1], kept[2] + sums[2]];
            }
        }
    }
    let [sum, squares, length] = if kept[2] > 0.0 { kept } else { all };
    if length <= 0.0 {
        return Ok(None);
    }
    let mean = sum / length;
    let deviation = (squares / length - mean * mean).max(0.0).sqrt();
    Ok(Mm::new(mean).ok().map(|mean| Spread { mean, deviation }))
}

/// The width of a part along `lines`, its centre lines, whose edges `edges` holds, averaged along them
/// ([`spread`]). `None` for lines of no length, a dot's.
pub(super) fn width(edges: &Edges, lines: &[Line], meter: &mut Meter) -> Result<Option<Mm>, Exhausted> {
    Ok(spread(edges, lines, meter)?.map(|spread| spread.mean))
}

/// The diameter of the largest circle centred on a point of `line`, but an open line's 2 ends, that fits
/// inside the outline whose edges `edges` holds: where that outline is widest along the line, 0 when the line
/// has no such point. A unit of `meter`'s work for each point and edge.
pub(super) fn widest(edges: &Edges, line: &Line, meter: &mut Meter) -> Result<f64, Exhausted> {
    let ends = if line.closed { 0 } else { 1 };
    let inside = line.points.get(ends..line.points.len().saturating_sub(ends)).unwrap_or_default();
    let mut widest = 0.0_f64;
    for &p in inside {
        widest = widest.max(2.0 * edges.nearest(p, meter)?);
    }
    Ok(widest)
}

/// How far the farthest corner of `outline` beside `line` lies from it, in millimetres: of the corners nearer
/// a point of the line between its ends than either end of an open line, for those no nearer the line than
/// to an end lie beyond the end, in its cap. 0 when none is. A unit of `meter`'s work for each corner and
/// piece of the line.
pub(super) fn farthest(line: &Line, outline: &Polygon, meter: &mut Meter) -> Result<f64, Exhausted> {
    let closing = if line.closed { line.points.first().zip(line.points.last()).map(|(&first, &last)| (last, first)) } else { None };
    let pieces: Vec<(Point, Point)> = segments(&line.points).chain(closing).collect();
    let ends = if line.closed { Vec::new() } else { line.points.first().into_iter().chain(line.points.last()).copied().collect() };
    let mut farthest = 0.0_f64;
    for ring in outline.rings() {
        for &p in ring {
            meter.charge(u64::try_from(pieces.len()).unwrap_or(u64::MAX))?;
            let near = pieces.iter().map(|&(a, b)| distance_to_segment(p, a, b)).fold(f64::INFINITY, f64::min);
            // An end lies on the line, no nearer the corner than the line is: the corner lies beyond it when it
            // is the line's nearest point, to within a rounding error.
            let beyond = ends.iter().any(|end| end.distance(p) <= near + ROUNDING);
            if !beyond && near.is_finite() {
                farthest = farthest.max(near);
            }
        }
    }
    Ok(farthest)
}

/// How far a part whose edges `edges` holds reaches from one of the centre lines of `columns`, its columns, on
/// both sides of the line, in radii of the widest circle that fits inside the part centred on its lines, with
/// `meter`'s work. A column's 2 rails are the part's outline on either side of its line. Each lies as far from
/// the line as its farthest corner beside the line ([`farthest`]), and the column reaches on both sides as far
/// as the nearer of its rails does. The part reaches as far as the column of its that reaches farthest. A
/// column's rails lie beside its line where the widest circle on the line touches them, so a column measures
/// 1, however its width tapers. A star measures more: its line runs from one point to another, and the points
/// that the line leaves out lie on both rails, far from it. The corner of a sharp turn lies on one rail only,
/// and the other rail keeps the turn a column. 0 for lines without a point between their ends, which no circle
/// is centred on.
pub(super) fn reach(edges: &Edges, columns: &[Column], meter: &mut Meter) -> Result<f64, Exhausted> {
    let (mut circle, mut beside) = (0.0_f64, 0.0_f64);
    for column in columns {
        circle = circle.max(widest(edges, &column.line, meter)?);
        let [one, other] = column.rails.clone().map(|rail| Polygon { outline: rail, holes: Vec::new() });
        beside = beside.max(farthest(&column.line, &one, meter)?.min(farthest(&column.line, &other, meter)?));
    }
    Ok(if circle > 0.0 { 2.0 * beside / circle } else { 0.0 })
}

/// How much nearer than an end of a line its nearest point to a corner may be, in millimetres, and the end
/// still be that point: a rounding error.
const ROUNDING: f64 = 1e-9;

/// How wide a line's end may measure and still be a tip, a corner of its part's outline, in millimetres:
/// a corner lies on the outline, and measures nothing but a rounding error.
const TIP: f64 = 1e-9;

/// The edges of a part in the cells of a square grid, each cut into pieces no longer than a cell, so that
/// the edge nearest a point is found among the cells round it.
pub(super) struct Edges {
    /// A cell's side, in millimetres.
    cell: f64,
    /// The pieces of edges that reach into each cell, by the cell's column and row.
    cells: BTreeMap<(i64, i64), Vec<(Point, Point)>>,
    /// The least and greatest column and row of a cell with a piece in it.
    span: [i64; 4],
}

impl Edges {
    /// The edges of `part`'s outline and holes in cells `cell` millimetres wide, with `meter`'s work: a unit
    /// for each piece and each cell it reaches.
    pub(super) fn of(part: &Polygon, cell: f64, meter: &mut Meter) -> Result<Edges, Exhausted> {
        let mut edges = Edges { cell, cells: BTreeMap::new(), span: [i64::MAX, i64::MAX, i64::MIN, i64::MIN] };
        for (a, b) in part.rings().flat_map(segments) {
            let pieces = (a.distance(b) / cell).ceil().max(1.0);
            let mut k = 0.0;
            while k < pieces {
                meter.charge(1)?;
                let (from, to) = (a.lerp(b, k / pieces), a.lerp(b, (k + 1.0) / pieces));
                let (x0, x1) = (edges.column(from.x().min(to.x())), edges.column(from.x().max(to.x())));
                let (y0, y1) = (edges.column(from.y().min(to.y())), edges.column(from.y().max(to.y())));
                for x in x0..=x1 {
                    for y in y0..=y1 {
                        meter.charge(1)?;
                        edges.cells.entry((x, y)).or_default().push((from, to));
                    }
                }
                let [low_x, low_y, high_x, high_y] = &mut edges.span;
                (*low_x, *low_y, *high_x, *high_y) = ((*low_x).min(x0), (*low_y).min(y0), (*high_x).max(x1), (*high_y).max(y1));
                k += 1.0;
            }
        }
        Ok(edges)
    }

    /// The column, or row, of the cells that `v` millimetres lies in. A part lies within metres of the
    /// origin, and a cell is a tenth of a millimetre or more, so the count fits.
    #[allow(clippy::cast_possible_truncation)]
    fn column(&self, v: f64) -> i64 {
        (v / self.cell).floor() as i64
    }

    /// The distance from `p` to the nearest edge, with `meter`'s work: a unit for each cell and each piece
    /// looked at. The cells are looked at in rings round `p`'s, and once the nearest piece found lies no
    /// farther than the cells beyond the ring could be, they aren't.
    pub(super) fn nearest(&self, p: Point, meter: &mut Meter) -> Result<f64, Exhausted> {
        if self.cells.is_empty() {
            return Ok(f64::INFINITY);
        }
        let (x, y) = (self.column(p.x()), self.column(p.y()));
        let [low_x, low_y, high_x, high_y] = self.span;
        let apart = |a: i64, b: i64| a.saturating_sub(b).saturating_abs();
        let last = apart(x, low_x).max(apart(high_x, x)).max(apart(y, low_y)).max(apart(high_y, y));
        let mut best = f64::INFINITY;
        let mut ring = 0;
        while ring <= last {
            for (i, j) in ring_cells(x, y, ring) {
                meter.charge(1)?;
                for &(a, b) in self.cells.get(&(i, j)).into_iter().flatten() {
                    meter.charge(1)?;
                    best = best.min(distance_to_segment(p, a, b));
                }
            }
            // Every cell beyond this ring lies at least `ring` cells from p.
            if best <= ring as f64 * self.cell {
                break;
            }
            ring += 1;
        }
        Ok(best)
    }
}

/// The cells `ring` cells round the cell in column `x` and row `y`: that cell itself for ring 0.
fn ring_cells(x: i64, y: i64, ring: i64) -> Vec<(i64, i64)> {
    if ring == 0 {
        return vec![(x, y)];
    }
    let rows = (x - ring..=x + ring).flat_map(|i| [(i, y - ring), (i, y + ring)]);
    let columns = (y - ring + 1..y + ring).flat_map(|j| [(x - ring, j), (x + ring, j)]);
    rows.chain(columns).collect()
}

/// What is said of a part whose shrinking, to tell whether it is narrow, couldn't be joined.
const UNJOINED: &str = "Finding how wide a part of this area is, the edges of a region could not be joined into rings";

/// What is said of a part whose fill and the pieces that leave it couldn't be found.
const UNSPLIT: &str =
    "Splitting a part of this area into its fill and the pieces that leave it, the edges of a region could not be joined into rings";

/// What is said of a narrow part whose triangulation lost its shape.
const LOST: &str = "Finding the centre line of a narrow part of this area, its triangulation lost its shape";

/// What is said of a part no wider than a satin column whose triangulation lost its shape.
const LOST_COLUMNS: &str = "Finding the satin columns of a part of this area, its triangulation lost its shape";

/// `SC-E0009` for a part of the automatic area `id` that `failed` says what failed of, a bug: the part stays
/// in the area's fill.
fn broken(id: &ElementId, failed: &str) -> Diagnostic {
    let message = format!("{failed}, so the part is filled. This is a bug in StitchCraft.");
    Diagnostic::new(Code::InternalCheckFailed, message).with_element(id.clone())
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::math::sin_cos;
    use stitchcraft_core::rng::SplitMix64;

    use super::*;
    use crate::normalize::region::geom;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The bounds of a shape with the rules' defaults in a design whose narrowest satin stroke is `narrowest`
    /// millimetres.
    fn bounds(narrowest: f64) -> Bounds {
        Bounds::of(&AutoParams::defaults(), Mm::new(narrowest).unwrap())
    }

    #[test]
    fn the_grid_finds_the_nearest_edge_as_measuring_every_edge_does() {
        // Star-shaped outlines with a star-shaped hole, and points in and around them: the distance found in
        // the cells round a point is the least over every edge, for cells of every size.
        let mut rng = SplitMix64::new(20_261_029);
        for case in 0..40 {
            let star = |rng: &mut SplitMix64, low: f64, high: f64, count: u64| -> Vec<Point> {
                let ring: Vec<Point> = (0..count)
                    .map(|i| {
                        let (sin, cos) = sin_cos(std::f64::consts::TAU * i as f64 / count as f64);
                        let r = rng.next_in(low, high);
                        Point::new(10.0 + r * cos, -5.0 + r * sin).unwrap()
                    })
                    .collect();
                geom::closed(&ring)
            };
            let count = 5 + rng.next_u64() % 30;
            let part = Polygon { outline: star(&mut rng, 6.0, 12.0, count), holes: vec![star(&mut rng, 1.0, 4.0, count)] };
            let cell = [0.1, 0.7, 3.0, 50.0][case % 4];
            let mut meter = Budget::DEFAULT.meter();
            let edges = Edges::of(&part, cell, &mut meter).unwrap();
            for _ in 0..50 {
                let p = Point::new(rng.next_in(-5.0, 25.0), rng.next_in(-20.0, 10.0)).unwrap();
                let every = part.rings().flat_map(segments).map(|(a, b)| distance_to_segment(p, a, b)).fold(f64::INFINITY, f64::min);
                let found = edges.nearest(p, &mut meter).unwrap();
                assert!((found - every).abs() <= 1e-12 * every.max(1.0), "{found} {every} at {p:?}, cell {cell}");
            }
        }
    }

    /// `n` points round a circle of radius `r` about (`x`, `y`), counter-clockwise in y-up axes from `turn`
    /// of a step past the x axis.
    fn circle(x: f64, y: f64, r: f64, n: usize, turn: f64) -> Vec<Point> {
        (0..n)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::TAU * (i as f64 + turn) / n as f64);
                Point::new(x + r * cos, y + r * sin).unwrap()
            })
            .collect()
    }

    /// The part of a narrow part, sewn along its centre lines.
    fn narrow_part(along: &Along) -> &Polygon {
        let Along::Narrow(narrow) = along else { panic!("{along:?} isn't narrow") };
        &narrow.part
    }

    /// A part with the outline `points`, wound clockwise in y-up axes as a region winds it.
    fn outline(points: &[(f64, f64)]) -> Polygon {
        let mut ring: Vec<Point> = points.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect();
        if geom::signed_area(&geom::closed(&ring)) > 0.0 {
            ring.reverse();
        }
        Polygon { outline: geom::closed(&ring), holes: Vec::new() }
    }

    #[test]
    fn each_kind_of_part_is_sorted_by_its_width() {
        // An area's parts, with a narrowest satin stroke of 1.5 mm:
        // - a square 10 mm across, broader than a satin column by its area alone;
        // - a speck;
        // - a strip 0.6 mm wide with an end 2 mm square: not narrow, which its area doesn't show and shrinking
        //   does, and no wider than a satin column, so sewn as columns, since its shape doesn't fill its uneven
        //   parts;
        // - a strip 0.6 mm wide, narrow;
        // - a regular octagon 1.5 mm across its corners, a dot, whose caps meet;
        // - and a ring whose hole pokes out through an edge of its outline, 0.012 mm, which no region has. Its
        //   triangulation loses its shape, as a bug would make it.
        // Each but the strips stays in the fill, and SC-E0009 names the area of the last.
        let square = outline(&[(0.0, 10.0), (10.0, 10.0), (10.0, 20.0), (0.0, 20.0)]);
        let speck = outline(&[(30.0, 0.0), (30.5, 0.0), (30.5, 0.5), (30.0, 0.5)]);
        let ended = outline(&[(0.0, 0.0), (18.0, 0.0), (18.0, -0.7), (20.0, -0.7), (20.0, 1.3), (18.0, 1.3), (18.0, 0.6), (0.0, 0.6)]);
        let strip = outline(&[(0.0, 5.0), (20.0, 5.0), (20.0, 5.6), (0.0, 5.6)]);
        let mut corners = circle(40.0, 0.0, 0.75, 8, 0.5);
        corners.reverse();
        let dot = Polygon { outline: geom::closed(&corners), holes: Vec::new() };
        let mut corners = circle(50.0, 0.0, 1.0, 20, 0.5);
        corners.reverse();
        let ring = Polygon { outline: geom::closed(&corners), holes: vec![geom::closed(&circle(50.4, 0.0, 0.6, 40, 0.0))] };
        let area = ElementId::new("area").unwrap();
        let parts = vec![square.clone(), speck.clone(), ended.clone(), strip.clone(), dot.clone(), ring.clone()];
        let even = Bounds { fill_uneven: false, ..bounds(1.5) };
        let sorted = sorted(&area, parts, &even, &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(sorted.filled, [square, speck, dot, ring]);
        let fired: Vec<RuleId> = sorted.fired.iter().map(|fired| fired.rule).collect();
        assert_eq!(fired, [RuleId::Area, RuleId::Speck, RuleId::Column, RuleId::Line, RuleId::Compact]);
        let [Along::Columns(columns), Along::Narrow(Narrow { part, lines, width })] = sorted.along.as_slice() else { panic!("{:?}", sorted.along) };
        assert_eq!(columns.part, ended);
        assert_eq!((part, lines.len()), (&strip, 1));
        assert!((width.get() - 0.6).abs() < 1e-9, "{width:?}");
        let [said] = sorted.said.as_slice() else { panic!("{:?}", sorted.said) };
        assert_eq!((said.code, said.element.as_ref()), (Code::InternalCheckFailed, Some(&area)));
        assert_eq!(said.message, format!("{LOST}, so the part is filled. This is a bug in StitchCraft."));
    }

    #[test]
    fn a_narrow_part_wider_than_a_satin_column_somewhere_stays_in_the_fill_whole() {
        // A part of an area wider than a satin column in places is split, and its tab sews as columns. A narrow
        // part of a part split is never so wide, since the split's opening takes every disc that wide; one
        // that were stays in the fill whole, and isn't split again.
        let tabbed = outline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 8.5), (32.0, 8.5), (32.0, 11.5), (20.0, 11.5), (20.0, 20.0), (0.0, 20.0)]);
        let (area, narrowest, rules) = (ElementId::new("area").unwrap(), bounds(1.0), AutoParams::defaults());
        let part = sorted(&area, vec![tabbed.clone()], &narrowest, &rules, &mut Budget::DEFAULT.meter()).unwrap();
        assert!(matches!(part.along.as_slice(), [Along::Columns(_)]), "{:?}", part.along);
        let mut piece = Parts::default();
        sort(&area, tabbed.clone(), &narrowest, &rules, Sorting::Piece(&[]), &mut piece, &mut Budget::DEFAULT.meter()).unwrap();
        let area = Measures::of_breadth(tabbed.area(), Breadth::Broad);
        assert_eq!(piece, Parts { fired: vec![Fired { rule: RuleId::Area, measures: Some(area) }], filled: vec![tabbed], ..Parts::default() });
    }

    #[test]
    fn parts_as_wide_as_a_satin_column_that_make_no_columns_stay_filled() {
        // With a narrowest satin stroke of 1.5 mm, 3 parts no wider than a satin column and not narrow:
        // - a regular octagon 3 mm across its corners, a dot, whose caps meet;
        // - a ring whose round hole touches its outline at a point, 4 mm wide on the far side, whose column would
        //   touch itself there;
        // - and a ring whose hole pokes out through an edge of its outline, 0.05 mm, which no region has. Its
        //   triangulation loses its shape, as a bug would make it.
        // Each stays in the fill. SC-I0902 says so of the second and SC-E0009 of the third, naming the area.
        let clockwise = |mut corners: Vec<Point>| {
            corners.reverse();
            geom::closed(&corners)
        };
        let dot = Polygon { outline: clockwise(circle(0.0, 0.0, 1.5, 8, 0.5)), holes: Vec::new() };
        let pinched = Polygon { outline: clockwise(circle(20.0, 0.0, 4.0, 32, 0.0)), holes: vec![geom::closed(&circle(22.0, 0.0, 2.0, 16, 0.0))] };
        let broken = Polygon { outline: clockwise(circle(50.0, 0.0, 4.0, 20, 0.5)), holes: vec![geom::closed(&circle(51.6, 0.0, 2.4, 40, 0.0))] };
        let area = ElementId::new("area").unwrap();
        let parts = vec![dot.clone(), pinched.clone(), broken.clone()];
        let sorted = sorted(&area, parts, &bounds(1.5), &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap();
        assert!(sorted.along.is_empty(), "{:?}", sorted.along);
        assert_eq!(sorted.filled, [dot, pinched, broken]);
        let [unmade, lost] = sorted.said.as_slice() else { panic!("{:?}", sorted.said) };
        assert_eq!(*unmade, columns::unmade(&area, &bounds(1.5)));
        assert_eq!((lost.code, lost.element.as_ref()), (Code::InternalCheckFailed, Some(&area)));
        assert_eq!(lost.message, format!("{LOST_COLUMNS}, so the part is filled. This is a bug in StitchCraft."));
    }

    #[test]
    fn parts_as_large_as_a_wide_satins_bound_allows_are_narrow() {
        // A part narrower than a width everywhere covers at most its perimeter times half the width, plus π
        // times that half's square for each hole past the first, and one that covers more is broad without
        // shrinking it. With a narrowest satin stroke of 6 mm, half of it is more than 1 mm: a strip 4 mm wide
        // covers more than its perimeter over that half, and a plate pierced by 12 × 12 holes 0.6 mm across,
        // 4.5 mm apart, though no circle 6 mm across fits between its holes, covers more than its perimeter
        // times the half plus, for each hole past the first, π, the half's square, or π times the half. Both
        // are within the bound, and narrow.
        let strip = outline(&[(0.0, -10.0), (30.0, -10.0), (30.0, -6.0), (0.0, -6.0)]);
        let (side, apart) = (54.3, 4.525);
        let holes = (0..144)
            .map(|k| {
                let (x, y) = ((f64::from(k % 12) + 0.5) * apart, (f64::from(k / 12) + 0.5) * apart);
                geom::closed(&circle(x, y, 0.3, 8, 0.5))
            })
            .collect();
        let plate = Polygon { holes, ..outline(&[(0.0, 0.0), (side, 0.0), (side, side), (0.0, side)]) };
        let area = ElementId::new("area").unwrap();
        let sorted = sorted(&area, vec![strip.clone(), plate.clone()], &bounds(6.0), &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap();
        assert!(sorted.filled.is_empty() && sorted.said.is_empty(), "{sorted:?}");
        assert_eq!(sorted.along.iter().map(narrow_part).collect::<Vec<_>>(), [&strip, &plate]);
    }

    #[test]
    fn req_auto_032_an_uneven_part_is_filled_and_its_tails_leave_the_fill_as_lines() {
        // A blob 3 mm square with 2 tails 0.6 mm wide, in a design whose narrowest satin stroke is 1 mm: the
        // blob stays in the fill, and each tail is a line.
        let blob = outline(&[
            (0.0, 0.0),
            (3.0, 0.0),
            (3.0, 1.2),
            (11.0, 1.2),
            (11.0, 1.8),
            (3.0, 1.8),
            (3.0, 3.0),
            (1.8, 3.0),
            (1.8, 11.0),
            (1.2, 11.0),
            (1.2, 3.0),
            (0.0, 3.0),
        ]);
        let area = ElementId::new("area").unwrap();
        let sorted = sorted(&area, vec![blob.clone()], &bounds(1.0), &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap();
        let rules: Vec<RuleId> = sorted.fired.iter().map(|fired| fired.rule).collect();
        assert_eq!((rules, sorted.along.len(), sorted.said.len()), (vec![RuleId::Uneven, RuleId::Line, RuleId::Line], 2, 0));
        let [square] = sorted.filled.as_slice() else { panic!("{:?}", sorted.filled) };
        assert!((square.area() - 9.0).abs() < 0.5, "{}", square.area());
        // A tail is split no further: one that is uneven itself stays in the fill whole.
        let mut tail = Parts::default();
        sort(&area, blob.clone(), &bounds(1.0), &AutoParams::defaults(), Sorting::Tail(&[]), &mut tail, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((tail.filled, tail.along.len(), tail.fired.len()), (vec![blob], 0, 1));
        assert_eq!(Sorting::Tail(&[[p(0.0, 0.0), p(1.0, 0.0)]]).cuts().len(), 1);
    }

    #[test]
    fn req_auto_035_a_part_whose_column_would_sew_too_long_a_stitch_stays_in_the_fill_and_says_its_length() {
        // A square 16 mm across with a round hole 8 mm across: a column round the hole would sew 7.31 mm
        // between its rails at the corners.
        let ring =
            Polygon { holes: vec![geom::closed(&circle(0.0, 0.0, 4.0, 48, 0.0))], ..outline(&[(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)]) };
        let area = ElementId::new("area").unwrap();
        let sorted = sorted(&area, vec![ring.clone()], &bounds(1.0), &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((sorted.filled, sorted.along.len()), (vec![ring], 0));
        let [Fired { rule: RuleId::Refused, measures: Some(measures) }] = sorted.fired.as_slice() else { panic!("{:?}", sorted.fired) };
        assert!((measures.width - 4.5).abs() < 0.1 && measures.breadth == Breadth::Medium, "{measures:?}");
        let [said] = sorted.said.as_slice() else { panic!("{:?}", sorted.said) };
        assert_eq!((said.code, said.element.as_ref()), (Code::AutoSatinFilled, Some(&area)));
        assert!(said.message.contains("would sew a stitch 7.31 mm long between its rails, longer than the widest satin column, 7 mm"), "{said}");
    }

    #[test]
    fn req_auto_068_a_part_whose_column_would_crowd_its_stitches_stays_in_the_fill_and_says_so() {
        // A band 2.5 mm wide bent three quarters of the way round a point 1 mm inside it: a column along it
        // would sew 3.5 mm of its outer edge to each millimetre of its inner one.
        let arc = |radius: f64, from: f64, to: f64| {
            (0..=48).map(move |i| {
                let (sin, cos) = sin_cos(from + (to - from) * f64::from(i) / 48.0);
                (radius * cos, radius * sin)
            })
        };
        let turn = 1.5 * std::f64::consts::PI;
        let band = outline(&arc(3.5, 0.0, turn).chain(arc(1.0, turn, 0.0)).collect::<Vec<_>>());
        let area = ElementId::new("area").unwrap();
        let sorted = sorted(&area, vec![band.clone()], &bounds(1.0), &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((sorted.filled, sorted.along.len()), (vec![band], 0));
        let [Fired { rule: RuleId::Refused, measures: Some(measures) }] = sorted.fired.as_slice() else { panic!("{:?}", sorted.fired) };
        assert_eq!(measures.lines, 1);
        let [said] = sorted.said.as_slice() else { panic!("{:?}", sorted.said) };
        assert_eq!((said.code, said.element.as_ref()), (Code::AutoSatinFilled, Some(&area)));
        assert!(said.message.contains("of one edge to 1 mm of the other, more than 2 mm"), "{said}");
    }

    #[test]
    fn req_auto_036_a_shape_that_asks_to_run_or_to_be_filled_sorts_every_part_so() {
        // A square 10 mm across, a strip 3 mm wide and a speck, of a shape that asks to run: the strip and the
        // square sew along their centre lines, the square's of no length, so it stays in the fill, and the
        // speck is a speck. Of a shape that asks to be filled, each stays in the fill.
        let square = outline(&[(0.0, 10.0), (10.0, 10.0), (10.0, 20.0), (0.0, 20.0)]);
        let strip = outline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (0.0, 3.0)]);
        let speck = outline(&[(30.0, 0.0), (30.5, 0.0), (30.5, 0.5), (30.0, 0.5)]);
        let area = ElementId::new("area").unwrap();
        let asked = |stitch: &str| {
            let rules = AutoParams::from_set(&[("auto_stitch", stitch)].into_iter().collect()).unwrap().params;
            let parts = vec![strip.clone(), speck.clone()];
            sorted(&area, parts, &Bounds::of(&rules, Mm::new(1.0).unwrap()), &rules, &mut Budget::DEFAULT.meter()).unwrap()
        };
        let run = asked("run");
        let [Along::Narrow(Narrow { part, lines, width })] = run.along.as_slice() else { panic!("{:?}", run.along) };
        assert!(*part == strip && lines.len() == 1 && (width.get() - 3.0).abs() < 0.1, "{width:?}");
        assert_eq!(
            (run.filled, run.fired.iter().map(|fired| fired.rule).collect::<Vec<_>>()),
            (vec![speck.clone()], vec![RuleId::Asked, RuleId::Speck])
        );
        let filled = asked("fill");
        assert!(filled.along.is_empty() && filled.filled == [strip.clone(), speck.clone()], "{filled:?}");
        assert_eq!(filled.fired.iter().map(|fired| fired.rule).collect::<Vec<_>>(), [RuleId::Asked, RuleId::Speck]);
        // A part that asks to run and has no line to run along stays in the fill.
        let rules = AutoParams::from_set(&[("auto_stitch", "run")].into_iter().collect()).unwrap().params;
        let mut dot = circle(40.0, 0.0, 0.75, 8, 0.5);
        dot.reverse();
        let dot = Polygon { outline: geom::closed(&dot), holes: Vec::new() };
        let sorted = sorted(&area, vec![dot.clone(), square], &Bounds::of(&rules, Mm::new(1.0).unwrap()), &rules, &mut Budget::DEFAULT.meter());
        let sorted = sorted.unwrap();
        assert!(sorted.filled.contains(&dot) && sorted.fired.first().map(|fired| fired.rule) == Some(RuleId::Compact), "{sorted:?}");
    }

    #[test]
    fn a_part_whose_fill_cant_be_found_stays_filled_and_that_is_said() {
        // What a part that the region code can't split, a bug that no drawn part reaches, comes to: it stays
        // in the fill, and SC-E0009 names the area.
        let square = outline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]);
        let (area, mut sorted) = (ElementId::new("area").unwrap(), Parts::default());
        unsplit(&mut sorted, square.clone(), &area).unwrap();
        assert_eq!(sorted.filled, [square]);
        let [said] = sorted.said.as_slice() else { panic!("{:?}", sorted.said) };
        assert_eq!((said.code, said.element.as_ref()), (Code::InternalCheckFailed, Some(&area)));
        assert_eq!(said.message, format!("{UNSPLIT}, so the part is filled. This is a bug in StitchCraft."));
    }

    #[test]
    fn req_auto_032_a_width_s_spread_is_its_standard_deviation_along_the_lines() {
        // A wedge 20 mm long, 2 mm wide at one end and 4 mm at the other, with its centre line along x: the
        // largest circle on the line is as wide as the wedge there, give or take the slope of its sides, so
        // the width averages near 3 mm and runs straight from one end to the other, whose standard deviation
        // is the difference of the ends over the square root of 12.
        let wedge = outline(&[(0.0, -1.0), (20.0, -2.0), (20.0, 2.0), (0.0, 1.0)]);
        let mut meter = Budget::DEFAULT.meter();
        let edges = Edges::of(&wedge, 0.5, &mut meter).unwrap();
        let line = Line { points: (1..=9).map(|i| p(2.0 * f64::from(i), 0.0)).collect(), closed: false };
        let found = spread(&edges, std::slice::from_ref(&line), &mut meter).unwrap().unwrap();
        let (from, to) = (2.0 * edges.nearest(p(2.0, 0.0), &mut meter).unwrap(), 2.0 * edges.nearest(p(18.0, 0.0), &mut meter).unwrap());
        assert!((found.mean.get() - (from + to) / 2.0).abs() < 1e-9, "{found:?}");
        assert!((found.deviation - (to - from) / 12.0_f64.sqrt()).abs() < 1e-9, "{found:?} {from} {to}");
        assert_eq!(width(&edges, std::slice::from_ref(&line), &mut meter).unwrap(), Some(found.mean));
        // A band's width doesn't vary, and lines of no length have no width.
        let band = outline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (0.0, 3.0)]);
        let edges = Edges::of(&band, 0.5, &mut meter).unwrap();
        let middle = Line { points: vec![p(5.0, 1.5), p(10.0, 1.5), p(15.0, 1.5)], closed: false };
        let even = spread(&edges, std::slice::from_ref(&middle), &mut meter).unwrap().unwrap();
        assert_eq!((even.mean.get(), even.deviation), (3.0, 0.0));
        let dot = Line { points: vec![p(5.0, 1.5)], closed: false };
        assert_eq!(spread(&edges, &[dot], &mut meter).unwrap(), None);
        // A line of one piece that leads into a tip has no other piece, so that one is measured: from the
        // tip of a triangle its width runs straight from nothing.
        let triangle = outline(&[(0.0, 0.0), (20.0, -2.0), (20.0, 2.0)]);
        let edges = Edges::of(&triangle, 0.5, &mut meter).unwrap();
        let from_the_tip = Line { points: vec![p(0.0, 0.0), p(10.0, 0.0)], closed: false };
        let widest = 2.0 * edges.nearest(p(10.0, 0.0), &mut meter).unwrap();
        let tapering = spread(&edges, &[from_the_tip], &mut meter).unwrap().unwrap();
        assert!(widest > 1.9 && (tapering.mean.get() - widest / 2.0).abs() < 1e-9, "{tapering:?} {widest}");
        assert!((tapering.deviation - widest / 12.0_f64.sqrt()).abs() < 1e-9, "{tapering:?} {widest}");
    }

    /// A star of `points` points round the origin, its points `outer` millimetres out and the corners between
    /// them `inner`, the first point straight up.
    fn star(points: usize, outer: f64, inner: f64) -> Polygon {
        let corners: Vec<(f64, f64)> = (0..2 * points)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::PI * i as f64 / points as f64);
                let r = if i % 2 == 0 { outer } else { inner };
                (r * sin, -r * cos)
            })
            .collect();
        outline(&corners)
    }

    /// How far `part` reaches from its columns' lines on both sides, and its columns.
    fn reach_of(part: &Polygon) -> (f64, Vec<Column>) {
        let mut meter = Budget::DEFAULT.meter();
        let columns = strips::columns(part, piece(part), &[], 1.0, &mut meter).unwrap().unwrap().unwrap();
        let edges = Edges::of(part, piece(part), &mut meter).unwrap();
        (reach(&edges, &columns, &mut meter).unwrap(), columns)
    }

    #[test]
    fn req_auto_050_a_part_reaches_as_far_as_the_nearer_rail_of_a_column_lies_from_its_line() {
        // A band 3 by 20 mm, and a leaf: each rail lies as far from the line as the widest circle reaches.
        let band = outline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 3.0), (0.0, 3.0)]);
        let (reached, columns) = reach_of(&band);
        assert!((reached - 1.0).abs() < 1e-6 && columns.len() == 1, "{reached}");
        // The star of a badge, 5.33 mm across with 4 points: its one line runs from a point to the point
        // opposite, 5.8 mm, and the 2 other points lie 2.45 mm from it, one on each rail, where the widest
        // circle on the line is 1.88 mm across.
        let badge = star(4, 8.0 / 3.0, 8.0_f64.sqrt() / 3.0);
        let (reached, columns) = reach_of(&badge);
        assert!((reached - 2.604).abs() < 0.01 && columns.len() == 1, "{reached}");
        // A star of 6 points whose centre lines keep one line reaches 1.61 times as far at any size, and one
        // of 5 points 1.97 to 2.62 times, by which of its points its line runs between. Where the corners round
        // a star's middle tie otherwise, it keeps 2 lines, and is uneven.
        let mut one_line = 0;
        for (points, inner, least, most) in [(5, 0.382, 1.96, 2.63), (6, 0.5, 1.61, 1.62)] {
            for outer in [2.0, 2.665, 4.0, 6.0] {
                let (reached, columns) = reach_of(&star(points, outer, inner * outer));
                if columns.len() == 1 {
                    one_line += 1;
                    assert!(reached > least && reached < most, "{points} points, {outer} mm: {reached}");
                }
            }
        }
        assert!(one_line >= 6, "{one_line}");
        // A letter V 5.7 mm tall in strokes 1.2 mm wide: its corner lies far from its line on one rail only.
        let v = outline(&[(-2.07, -5.7), (-0.8, -5.7), (0.0, -3.51), (0.8, -5.7), (2.07, -5.7), (0.0, 0.0)]);
        let (reached, _) = reach_of(&v);
        assert!(reached < 1.0, "{reached}");
        // Lines without a point between their ends hold no circle: such a part reaches nowhere.
        let mut ends_only = columns;
        for column in &mut ends_only {
            column.line.points.truncate(2);
        }
        let mut meter = Budget::DEFAULT.meter();
        let edges = Edges::of(&star(6, 6.0, 3.0), 1.0, &mut meter).unwrap();
        assert_eq!(reach(&edges, &ends_only, &mut meter), Ok(0.0));
        // Measuring costs work, the part's.
        let with = |work: u64| reach(&edges, &reach_of(&badge).1, &mut Budget { max_work: work, ..Budget::DEFAULT }.meter());
        let least = (0..100_000).find(|work| with(*work).is_ok()).unwrap();
        assert!(least > 0 && (0..least).step_by(7).all(|work| with(work) == Err(Exhausted::Work)), "{least}");
    }

    #[test]
    fn a_part_of_1_square_millimetre_is_no_speck() {
        // A strip 0.5 mm wide and 2 mm long, as large as a speck can't be: narrow, with a line along it.
        let strip = outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 0.5), (0.0, 0.5)]);
        assert_eq!(strip.area(), AutoParams::defaults().auto_min_area_mm2);
        let (area, rules) = (ElementId::new("area").unwrap(), AutoParams::defaults());
        let sorted = sorted(&area, vec![strip.clone()], &bounds(1.5), &rules, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(sorted.along.iter().map(narrow_part).collect::<Vec<_>>(), [&strip]);
    }

    #[test]
    fn a_parts_pieces_are_as_long_as_it_is_wide_on_average() {
        // Twice its area over its perimeter: 24 / 41.2 for a strip 0.6 mm wide and 20 mm long, and the finest
        // piece for a hairline.
        let strip = outline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 0.6), (0.0, 0.6)]);
        assert!((piece(&strip) - 24.0 / 41.2).abs() < 1e-12, "{}", piece(&strip));
        assert_eq!(piece(&outline(&[(0.0, 0.0), (20.0, 0.0), (20.0, 0.01), (0.0, 0.01)])), FINEST);
    }

    #[test]
    fn the_grid_holds_each_edge_cut_into_pieces_no_longer_than_a_cell() {
        // A star with a hole, its edges slanting every way: the pieces in the cells, each once, lie along the
        // part's edges, end to end, no longer than a cell, and together as long as the edges.
        let star: Vec<Point> = (0..14)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(i) / 14.0);
                let r = if i % 2 == 0 { 9.0 } else { 4.0 };
                Point::new(r * cos, r * sin).unwrap()
            })
            .collect();
        let part = Polygon { outline: geom::closed(&star), holes: vec![geom::closed(&circle(0.0, 0.0, 1.0, 9, 0.0))] };
        for cell in [0.3, 0.7, 2.0] {
            let edges = Edges::of(&part, cell, &mut Budget::DEFAULT.meter()).unwrap();
            let key = |q: Point| (q.x().to_bits(), q.y().to_bits());
            let pieces: BTreeMap<_, (Point, Point)> = edges.cells.values().flatten().map(|&(a, b)| ((key(a), key(b)), (a, b))).collect();
            let mut length = 0.0;
            for (a, b) in pieces.values() {
                assert!(a.distance(*b) <= cell * (1.0 + 1e-12), "{a:?} {b:?} longer than {cell}");
                let on = |q: Point| part.rings().flat_map(segments).any(|(u, v)| distance_to_segment(q, u, v) < 1e-9);
                assert!(on(*a) && on(*b), "{a:?} {b:?} off the part's edges");
                length += a.distance(*b);
            }
            assert!((length - perimeter(&part)).abs() < 1e-9, "{length} {}", perimeter(&part));
        }
    }

    #[test]
    fn a_line_into_a_tip_is_measured_without_its_piece_into_the_corner() {
        // A strip 1 mm wide and a line along its middle to a corner of its end: 1 mm wide for 4 mm, and the
        // piece of 0.5 mm into the corner, 0.5 mm wide on average, is left out. Alone, it is the width.
        let strip = outline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)]);
        let measured = |points: &[(f64, f64)]| {
            let line = Line { points: points.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect(), closed: false };
            let edges = Edges::of(&strip, piece(&strip), &mut Budget::DEFAULT.meter()).unwrap();
            width(&edges, &[line], &mut Budget::DEFAULT.meter()).unwrap().unwrap().get()
        };
        assert!((measured(&[(5.5, 0.5), (9.5, 0.5), (10.0, 1.0)]) - 1.0).abs() < 1e-12);
        assert!((measured(&[(10.0, 1.0), (9.5, 0.5), (5.5, 0.5)]) - 1.0).abs() < 1e-12);
        assert!((measured(&[(9.5, 0.5), (10.0, 1.0)]) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn a_parts_width_is_averaged_along_its_lines() {
        // A strip 1 mm wide and a line from 0.1 mm off its edge to its middle and along it: 0.2 mm wide at its
        // start and 1 mm after, each piece weighed by its length, the first 0.4 mm long and the second 1 mm.
        let strip = outline(&[(0.0, 0.0), (10.0, 0.0), (10.0, 1.0), (0.0, 1.0)]);
        let line = Line { points: [(5.0, 0.1), (5.0, 0.5), (6.0, 0.5)].map(|(x, y)| Point::new(x, y).unwrap()).to_vec(), closed: false };
        let edges = Edges::of(&strip, piece(&strip), &mut Budget::DEFAULT.meter()).unwrap();
        let found = width(&edges, std::slice::from_ref(&line), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert!((found.get() - (0.4 * 0.6 + 1.0) / 1.4).abs() < 1e-12, "{found:?}");
        // Widest at its middle point, 1 mm wide, its ends left out: a line of 2 points has no other.
        assert!((widest(&edges, &line, &mut Budget::DEFAULT.meter()).unwrap() - 1.0).abs() < 1e-12);
        let ends = Line { points: line.points[..2].to_vec(), closed: false };
        assert_eq!(widest(&edges, &ends, &mut Budget::DEFAULT.meter()).unwrap(), 0.0);
    }

    #[test]
    fn a_line_is_widest_where_the_largest_circle_on_it_but_at_its_ends_fits() {
        // A band 0.6 mm wide for 5 mm with a head 1.3 mm wide for 3 mm. A line along the band and into the
        // head, ending on its middle, measures the head at its end alone: 0.6 mm at its widest. Run on
        // through the head, it is 1.3 mm at its widest there, and closed into a ring, at its ends too.
        let club = outline(&[(0.0, -0.3), (5.0, -0.3), (5.0, -0.65), (8.0, -0.65), (8.0, 0.65), (5.0, 0.65), (5.0, 0.3), (0.0, 0.3)]);
        let edges = Edges::of(&club, piece(&club), &mut Budget::DEFAULT.meter()).unwrap();
        let measured = |points: &[(f64, f64)], closed: bool| {
            let line = Line { points: points.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect(), closed };
            widest(&edges, &line, &mut Budget::DEFAULT.meter()).unwrap()
        };
        assert!((measured(&[(0.5, 0.0), (2.5, 0.0), (4.5, 0.0), (6.5, 0.0)], false) - 0.6).abs() < 1e-12);
        assert!((measured(&[(0.5, 0.0), (2.5, 0.0), (6.5, 0.0), (7.35, 0.0)], false) - 1.3).abs() < 1e-12);
        assert!((measured(&[(6.5, 0.0), (2.5, 0.0), (0.5, 0.0)], true) - 1.3).abs() < 1e-12);
    }

    #[test]
    fn an_outline_reaches_from_its_line_as_far_as_its_farthest_corner_beside_it() {
        // A band 0.6 mm wide and 8 mm long, its line along its middle from 0.5 mm to 7.5 mm, and a bump on its
        // side 1.4 mm tall: the bump's corners are 1.7 mm from the line. The band's own corners lie beyond the
        // line's ends, nearer them than any point between, and without the bump it has no corner beside them.
        let line = Line { points: [(0.5, 0.0), (7.5, 0.0)].map(|(x, y)| Point::new(x, y).unwrap()).to_vec(), closed: false };
        let band = outline(&[(0.0, -0.3), (8.0, -0.3), (8.0, 0.3), (5.0, 0.3), (5.0, 1.7), (3.0, 1.7), (3.0, 0.3), (0.0, 0.3)]);
        assert!((farthest(&line, &band, &mut Budget::DEFAULT.meter()).unwrap() - 1.7).abs() < 1e-12);
        let plain = outline(&[(0.0, -0.3), (8.0, -0.3), (8.0, 0.3), (0.0, 0.3)]);
        assert_eq!(farthest(&line, &plain, &mut Budget::DEFAULT.meter()).unwrap(), 0.0);
        // Closed, the line has no ends, and the band's corners are beside it.
        let ring = Line { closed: true, ..line };
        assert!((farthest(&ring, &plain, &mut Budget::DEFAULT.meter()).unwrap() - (0.5_f64 * 0.5 + 0.3 * 0.3).sqrt()).abs() < 1e-12);
    }

    #[test]
    fn measuring_a_point_looks_at_the_cells_round_it_not_at_every_edge() {
        // A strip 100 mm long and 0.5 mm wide, its edges cut into some 400 pieces as long as it is wide. A
        // point on its middle finds its nearest edge in the cells round it, at the cost of some 20 units of
        // work, where looking at every piece would cost 400.
        let strip = outline(&[(0.0, 0.0), (100.0, 0.0), (100.0, 0.5), (0.0, 0.5)]);
        let mut meter = Budget::DEFAULT.meter();
        let edges = Edges::of(&strip, piece(&strip), &mut meter).unwrap();
        let before = meter.work_left();
        assert_eq!(edges.nearest(Point::new(50.0, 0.25).unwrap(), &mut meter), Ok(0.25));
        let cost = before - meter.work_left();
        assert!(cost < 40, "{cost}");
    }

    #[test]
    fn a_part_without_edges_is_nowhere_near() {
        let part = Polygon { outline: Vec::new(), holes: Vec::new() };
        let edges = Edges::of(&part, 1.0, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(edges.nearest(Point::ORIGIN, &mut Budget::DEFAULT.meter()), Ok(f64::INFINITY));
    }
}
