//! The parts of an automatic area no wider than a satin column, each sewn as satin columns between its own
//! outlines (`REQ-AUTO-016` to `REQ-AUTO-018`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Stitch types by width, and
//! `docs/src/design/algorithms/centre-lines.md` › Columns. Filled, a part as wide as a satin column sews rows
//! no longer than it is wide wherever they cross it; a satin column sews it with stitches across, the way a
//! digitizer would. So a part no wider than its area's widest satin column everywhere, and not narrow
//! (`auto::narrow`), sews a column along each of its centre lines (`normalize::strips::columns`):
//!
//! - **Width.** Each column is as wide as the circles that fit inside the part centred on its line, on
//!   average, as a narrow part is, which gives its underlay and its triple run. One no wider than the
//!   design's narrowest satin stroke on average is too narrow to stitch across, and runs along its line as a
//!   running stitch instead, tripled from the narrowest triple run, unless the column is wider than that
//!   stroke somewhere along its line but at its ends, or reaches farther than that from its line beside it,
//!   each measured inside its own outline ([`runs`]). A running stitch covers a thread's width along its
//!   line, and where the column is wider, the fabric beside the line shows unless an area beside it reaches
//!   under it. Measured on average alone, a traced photo's strands of fur that widened at an end ran, and so
//!   did blobs, with only other lines beside them, and so did a column that took in a bump 1.4 mm across that
//!   its line never reaches. Where 2 strokes meet, the part is wider than either, by where the junction's
//!   triangle has its corners, and a circle inside the part measures the meeting, so that a letter and its
//!   mirror image sewed otherwise. The column's own outline crosses to the other column there. A line's end
//!   lies where it meets another line or runs into a corner, and measures that.
//! - **Order.** A column that ends under another sews first, then the longest, and each next the one with
//!   an end nearest where the last one ended, sewn from that end; a ring from its rung nearest.
//! - **Settings.** Each column's underlay is its width's ([`super::underlay`]). One that ends under another
//!   is its pull compensation longer at that end, by its push compensation, and so reaches under it.
//! - **A dot.** A part whose every line is shorter than half its width there is a dot, which stays in the
//!   fill, as one with no line does. Such a part has no way to run, as a disc or a square has none, and its
//!   lines are only what its triangles leave of its middle: corners that lie on one circle tie, and how each
//!   tie breaks depends on where the part lies and how it is drawn, so that of 2 dots alike one would sew a
//!   column across and the other a fill. A disc's or a square's lines are 0.4 times as long as it is wide,
//!   and a rectangle 3 mm wide is a column from 3.5 mm long ([`DOT`]).
//! - **Drawn.** Each column is drawn with its rails and rungs, and read back as a satin column is read, it
//!   must give them ([`drawn::from_rails`]). A part whose triangles make no column round each line, or one of
//!   whose columns can't be drawn so, stays in the area's fill, and `SC-I0902` says so. A rung added to tell
//!   the rails from the rungs goes across an end that doesn't reach under another column where one can take
//!   it; across one that does, the overlap sews as a section of its own.
//!
//! Every step costs a unit of work, a part's columns being part of the work of digitizing its area.

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Mm, Point};

use super::narrow::{Edges, farthest, piece, spread, widest, width};
use super::rules::{Bounds, Breadth, Measures, RuleId, choose, fills};
use super::{AutoParams, at_most};
use crate::design::Path;
use crate::generators::mm;
use crate::generators::satin::drawn;
use crate::normalize::region::Polygon;
use crate::normalize::strips::{self, Column, Line};
use crate::normalize::stroke::distance_to_segment;

/// A part sewn as columns, and its columns.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Columns {
    /// The part, which its columns paint.
    pub(super) part: Polygon,
    /// Its columns, in the order they are sewn.
    pub(super) sewn: Vec<Sewn>,
}

/// A column of a part as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Sewn {
    /// A satin column: the path that draws its rails and rungs, its width, and how much shorter it is made at
    /// its start, then its end (negative: longer).
    Satin { path: Path, width: Mm, push: [f64; 2] },
    /// A running stitch along the column's centre line, too narrow for satin, and the column's width.
    Running { line: Line, width: Mm },
}

/// What becomes of a part no wider than a satin column.
pub(super) enum Made {
    /// It sews as these columns, by this rule, a column's or its shape's own, with these measures.
    Columns {
        /// The columns, in the order they are sewn.
        sewn: Vec<Sewn>,
        /// The rule.
        rule: RuleId,
        /// What was measured of the part.
        measures: Measures,
    },
    /// It stays in the fill by this rule, compact or uneven, with these measures: none for a part without a
    /// centre line, which is compact.
    Filled(RuleId, Option<Measures>),
    /// One of its columns would sew a stitch this long between its rails, in millimetres, longer than the
    /// widest satin column: it is refused, and stays in the fill.
    Long(f64, Measures),
    /// Its columns can't be drawn: it is refused, and stays in the fill.
    Unmade,
    /// Its triangulation lost its shape, a bug, and it stays in the fill.
    Broken,
}

/// What is measured of `part`, no wider than a satin column, whose columns are `found` and whose edges
/// `edges` holds, with `meter`'s work: its area, its width along its columns' lines and how much it varies,
/// and how many lines it has. `None` for lines of no length or no width.
fn measured(part: &Polygon, found: &[Column], edges: &Edges, meter: &mut Meter) -> Result<Option<Measures>, Exhausted> {
    let lines: Vec<Line> = found.iter().map(|column| column.line.clone()).collect();
    let spread = spread(edges, &lines, meter)?.filter(|spread| spread.mean.get() > 0.0);
    Ok(spread.map(|spread| Measures {
        area: part.area(),
        breadth: Breadth::Medium,
        width: spread.mean.get(),
        unevenness: spread.deviation / spread.mean.get(),
        lines: found.len(),
    }))
}

/// The rule by which the rules fill `part`, of an area held to `bounds`, with `meter`'s work: an area, a
/// compact part, an uneven one, any part of a shape that asks to be filled, or a part whose columns are
/// refused for a stitch too long. `None` for a part that sews along its middle, and for one whose breadth or
/// columns can't be found, whose stitches say what failed. The layers close a filled part's holes under what
/// fills them (`REQ-AUTO-033`).
pub(in crate::auto) fn filled(part: &Polygon, bounds: &Bounds, meter: &mut Meter) -> Result<Option<RuleId>, Exhausted> {
    let Some(breadth) = bounds.breadth_of(part, meter)? else { return Ok(None) };
    let unmeasured = Measures::of_breadth(part.area(), breadth);
    let as_filled = |rule: RuleId| fills(rule, bounds).then_some(rule);
    if breadth != Breadth::Medium {
        return Ok(as_filled(choose(&unmeasured, bounds)));
    }
    let Ok(Some(found)) = strips::columns(part, piece(part), &[], bounds.narrowest.get(), meter)? else { return Ok(None) };
    let edges = Edges::of(part, piece(part), meter)?;
    // A part without a centre line of any length is a dot, which is compact.
    let measures = measured(part, &found, &edges, meter)?.unwrap_or(unmeasured);
    if let Some(rule) = as_filled(choose(&measures, bounds)) {
        return Ok(Some(rule));
    }
    Ok(too_long(&found, bounds, meter)?.map(|_| RuleId::Refused))
}

/// The longest stitch that one of the columns `found` would sew between its rails, in millimetres, when
/// that is longer than the widest satin column of `bounds` ([`longest_stitch`]), with `meter`'s work: the
/// first such column's.
fn too_long(found: &[Column], bounds: &Bounds, meter: &mut Meter) -> Result<Option<f64>, Exhausted> {
    for column in found {
        let stitch = longest_stitch(&column.rails, &column.rungs, meter)?;
        if bounds.too_long(stitch) {
            return Ok(Some(stitch));
        }
    }
    Ok(None)
}

/// The columns of `part`, an area's held to `bounds` whose settings of the rules are `rules`, cut from a
/// broad part along `cuts` (`auto::split`), with `meter`'s work: none when the rules fill it, as a compact
/// or an uneven part, or refuse its columns.
pub(super) fn columns(part: &Polygon, bounds: &Bounds, rules: &AutoParams, cuts: &[[Point; 2]], meter: &mut Meter) -> Result<Made, Exhausted> {
    let narrowest = bounds.narrowest;
    let found = match strips::columns(part, piece(part), cuts, narrowest.get(), meter)? {
        Err(_) => return Ok(Made::Broken),
        Ok(None) => return Ok(Made::Unmade),
        Ok(Some(found)) => found,
    };
    let edges = Edges::of(part, piece(part), meter)?;
    // A part without a centre line of any length is a dot, which has no way to run: it is compact.
    let Some(measures) = measured(part, &found, &edges, meter)? else { return Ok(Made::Filled(RuleId::Compact, None)) };
    let rule = choose(&measures, bounds);
    if fills(rule, bounds) {
        return Ok(Made::Filled(rule, Some(measures)));
    }
    // A satin stitch longer than the widest satin column lies loose: the part is filled instead.
    if let Some(stitch) = too_long(&found, bounds, meter)? {
        return Ok(Made::Long(stitch, measures));
    }
    let mut widths = Vec::with_capacity(found.len());
    for column in &found {
        let Some(average) = width(&edges, std::slice::from_ref(&column.line), meter)? else { return Ok(Made::Unmade) };
        // Where it meets another column the part is wider than either, and its own outline isn't.
        let outline = column.outline();
        let own = Edges::of(&outline, piece(part), meter)?;
        widths.push(Width { average, widest: widest(&own, &column.line, meter)?, farthest: farthest(&column.line, &outline, meter)? });
    }
    let mut sewn = Vec::with_capacity(found.len());
    for (index, column) in ordered(&found, meter)? {
        let Some(&measured) = widths.get(index) else { return Ok(Made::Unmade) };
        let width = measured.average;
        if runs(measured, narrowest) {
            sewn.push(Sewn::Running { line: column.walked, width });
            continue;
        }
        // A column that ends under another is its pull compensation longer there, not the overlap: pushed
        // on by 1 mm, a column skews its last stitch across what lies beside the other column (`REQ-AUTO-029`).
        let reach = rules.auto_pull_compensation_mm.get();
        let push = column.under.map(|under| if under.is_some() { -reach } else { 0.0 });
        let Ok(drawn) = drawn::from_rails(column.rails, column.rungs, push, meter)? else { return Ok(Made::Unmade) };
        sewn.push(Sewn::Satin { path: drawn.path(), width, push });
    }
    Ok(Made::Columns { sewn, rule, measures })
}

/// The longest stitch a satin column with these `rails`, both from its start, and `rungs`, each the point it
/// pairs on the first rail and then on the second, sews between its rails, in millimetres, with `meter`'s
/// work. A satin column's stitches cross it between the points that lie as far along its 2 rails, as shares
/// of their lengths from one rung to the next, so the longest lies between 2 such points, at a corner of
/// one rail or the other. The rails' starts pair up, and so do their ends. A unit of work for each corner
/// and rung, times the rails' corners.
fn longest_stitch(rails: &[Vec<Point>; 2], rungs: &[[Point; 2]], meter: &mut Meter) -> Result<f64, Exhausted> {
    let [first, second] = rails;
    let (along_first, along_second) = (lengths(first), lengths(second));
    let (Some(&end_first), Some(&end_second)) = (along_first.last(), along_second.last()) else { return Ok(0.0) };
    let corners = u64::try_from(first.len() + second.len()).unwrap_or(u64::MAX);
    // Where each section starts and ends on each rail: at the rails' starts, at each rung in order, and at
    // their ends.
    let mut cuts = vec![[0.0, 0.0]];
    for [a, b] in rungs {
        meter.charge(corners)?;
        let last = cuts.last().copied().unwrap_or([0.0, 0.0]);
        cuts.push([position(first, &along_first, *a).max(last[0]), position(second, &along_second, *b).max(last[1])]);
    }
    cuts.push([end_first, end_second]);
    let mut longest = 0.0_f64;
    for (from, to) in cuts.iter().zip(cuts.iter().skip(1)) {
        // The shares of the section at which one rail or the other turns a corner, and its 2 ends.
        let share = |along: &[f64], rail: usize| -> Vec<f64> {
            let (start, end) = (from[rail], to[rail]);
            along.iter().filter(|at| **at > start && **at < end).map(|at| (at - start) / (end - start)).collect()
        };
        let shares = [vec![0.0, 1.0], share(&along_first, 0), share(&along_second, 1)].concat();
        for t in shares {
            meter.charge(1)?;
            let (a, b) =
                (point_at(first, &along_first, from[0] + t * (to[0] - from[0])), point_at(second, &along_second, from[1] + t * (to[1] - from[1])));
            longest = longest.max(a.zip(b).map_or(0.0, |(a, b)| a.distance(b)));
        }
    }
    Ok(longest)
}

/// How far along `rail` each of its points lies, in millimetres.
fn lengths(rail: &[Point]) -> Vec<f64> {
    let mut along = Vec::with_capacity(rail.len());
    let mut length = 0.0;
    for (i, p) in rail.iter().enumerate() {
        length += i.checked_sub(1).and_then(|before| rail.get(before)).map_or(0.0, |before| before.distance(*p));
        along.push(length);
    }
    along
}

/// How far along `rail`, whose points lie `along` it, its point nearest `p` lies.
fn position(rail: &[Point], along: &[f64], p: Point) -> f64 {
    let mut best = (f64::INFINITY, 0.0);
    for ((a, b), start) in rail.iter().zip(rail.iter().skip(1)).zip(along) {
        let distance = distance_to_segment(p, *a, *b);
        if distance < best.0 {
            // The nearest point of the piece from a to b lies as far from a as p is, less its distance off the piece.
            let from_a = (a.distance(p) * a.distance(p) - distance * distance).max(0.0).sqrt().min(a.distance(*b));
            best = (distance, start + from_a);
        }
    }
    best.1
}

/// The point `at` millimetres along `rail`, whose points lie `along` it: its last point from its length on.
fn point_at(rail: &[Point], along: &[f64], at: f64) -> Option<Point> {
    let next = along.partition_point(|length| *length < at).min(rail.len().saturating_sub(1));
    let (Some(to), Some(end)) = (rail.get(next), along.get(next)) else { return None };
    let Some((from, start)) = next.checked_sub(1).and_then(|before| rail.get(before).zip(along.get(before))) else { return Some(*to) };
    // The point before lies less far along than `at`. A piece of no length is then the rail's last, with `at`
    // past its end: the fraction is infinite, and held to 1.
    Some(from.lerp(*to, ((at - start) / (end - start)).clamp(0.0, 1.0)))
}

/// A column's width along its line.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Width {
    /// The diameter of the largest circle that fits inside its part centred on each point of its line,
    /// averaged along it: its width ([`width`]).
    average: Mm,
    /// The diameter of the largest circle that fits inside its own outline centred on a point of its line but
    /// an open line's 2 ends, in millimetres ([`widest`]).
    widest: f64,
    /// How far its own outline reaches from its line beside it, in millimetres ([`farthest`]).
    farthest: f64,
}

/// Whether a column of `width` along its line is too narrow to stitch across and runs along its line instead,
/// for an area whose narrowest satin stroke is `narrowest`: no wider than that stroke on average, nor anywhere
/// along its line inside its own outline but at its ends, and its outline reaching no farther than that from
/// the line beside it, each to the micrometre.
fn runs(width: Width, narrowest: Mm) -> bool {
    let within = |length: f64| super::micrometres(length) <= super::micrometres(narrowest.get());
    at_most(width.average, narrowest) && within(width.widest) && within(width.farthest)
}

/// `columns` in the order they are sewn, each with its place among them, turned to start where it is sewn
/// from, with `meter`'s work: a column that ends under another before it, then the longest first, and each
/// next the one with an end nearest where the last one ended, sewn from that end, or a ring from its rung
/// nearest. Where the columns that end under each other go round, as a pinwheel's bars do, none can sew
/// after every column it ends under: the one that comes first in that order sews first, and the last of them
/// ends on top of it. On a tie, the column found first.
fn ordered(columns: &[Column], meter: &mut Meter) -> Result<Vec<(usize, Column)>, Exhausted> {
    let count = columns.len();
    // How many columns end under each, which must sew before it.
    let mut waiting = vec![0_usize; count];
    for column in columns {
        for &under in column.under.iter().flatten() {
            if let Some(slot) = waiting.get_mut(under) {
                *slot += 1;
            }
        }
    }
    let mut done = vec![false; count];
    let mut out: Vec<(usize, Column)> = Vec::with_capacity(count);
    let mut here: Option<Point> = None;
    // Each step sews one column.
    for _ in 0..count {
        meter.charge(u64::try_from(count).unwrap_or(u64::MAX))?;
        let free = |c: usize| !done.get(c).copied().unwrap_or(true);
        let mut ready: Vec<usize> = (0..count).filter(|&c| free(c) && waiting.get(c) == Some(&0)).collect();
        if ready.is_empty() {
            ready = (0..count).filter(|&c| free(c)).collect();
        }
        let pick = match here {
            None => ready.iter().copied().fold(None, |best: Option<(usize, f64)>, c| {
                let length = columns.get(c).map_or(0.0, |column| column.walked.length());
                if best.is_some_and(|(_, most)| most >= length) { best } else { Some((c, length)) }
            }),
            Some(here) => ready.iter().copied().fold(None, |best: Option<(usize, f64)>, c| {
                let distance = columns.get(c).map_or(f64::INFINITY, |column| nearest_end(column, here));
                if best.is_some_and(|(_, least)| least <= distance) { best } else { Some((c, distance)) }
            }),
        };
        let Some((c, _)) = pick else { break };
        let Some(column) = columns.get(c) else { break };
        let column = match here {
            Some(here) if column.closed => column.restarted(here, meter)?,
            Some(here) if end_nearer(column, here) => column.reversed(),
            _ => column.clone(),
        };
        if let Some(slot) = done.get_mut(c) {
            *slot = true;
        }
        for &under in column.under.iter().flatten() {
            if let Some(slot) = waiting.get_mut(under) {
                *slot = slot.saturating_sub(1);
            }
        }
        here = if column.closed { column.rails[0].first().copied() } else { column.line.points.last().copied() };
        out.push((c, column));
    }
    Ok(out)
}

/// How far `here` is from where `column` can start: its centre line's nearer end, or for a ring, its centre
/// line's nearest point.
fn nearest_end(column: &Column, here: Point) -> f64 {
    let points = &column.line.points;
    let candidates: Vec<&Point> = if column.closed { points.iter().collect() } else { points.first().into_iter().chain(points.last()).collect() };
    candidates.into_iter().map(|p| p.distance(here)).fold(f64::INFINITY, f64::min)
}

/// Whether the open `column`'s centre line ends nearer `here` than it starts.
fn end_nearer(column: &Column, here: Point) -> bool {
    let points = &column.line.points;
    points.first().zip(points.last()).is_some_and(|(start, end)| end.distance(here) < start.distance(here))
}

/// `SC-I0902` for a part of the automatic area `id`, held to `bounds`, that is no wider than its widest
/// satin column and whose satin columns can't be drawn: it stays in the area's fill.
pub(super) fn unmade(id: &ElementId, bounds: &Bounds) -> Diagnostic {
    let message = format!(
        "A part of this area is no wider than a satin column, {} mm, but no satin columns can be drawn along it between its edges, so it is \
         filled.",
        mm(bounds.max_satin.get())
    );
    Diagnostic::new(Code::AutoSatinFilled, message).with_element(id.clone()).with_fix(Fix::Hint(
        "To sew it as satin columns, draw it with smoother edges, or draw the columns with 2 rails and give them settings.".to_string(),
    ))
}

/// `SC-I0902` for a part of the automatic area `id`, held to `bounds`, one of whose satin columns would sew a
/// stitch `stitch` millimetres long between its rails, longer than the widest satin column: it stays in the
/// area's fill.
pub(super) fn long(id: &ElementId, stitch: f64, bounds: &Bounds) -> Diagnostic {
    let message = format!(
        "A satin column along a part of this area would sew a stitch {} mm long between its rails, longer than the widest satin column, {} \
         mm, and such a stitch lies loose, so the part is filled.",
        mm(stitch),
        mm(bounds.max_satin.get())
    );
    Diagnostic::new(Code::AutoSatinFilled, message)
        .with_element(id.clone())
        .with_fix(Fix::Hint("To sew it as satin columns, draw the columns with 2 rails each and give them settings.".to_string()))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::math::{self, sin_cos};
    use stitchcraft_core::{Budget, Code, ElementId};

    use super::*;
    use crate::normalize::fixture::part;

    /// A band `width` millimetres wide and 20 mm long.
    fn band(width: f64) -> Polygon {
        part(&[(0.0, 0.0), (20.0, 0.0), (20.0, width), (0.0, width)], &[])
    }

    /// `n` steps of a circle of radius `r` about the origin, from `from` radians round `turn` radians, both
    /// ends included.
    fn arc(r: f64, n: usize, from: f64, turn: f64) -> Vec<(f64, f64)> {
        (0..=n)
            .map(|i| {
                let (sin, cos) = sin_cos(from + turn * i as f64 / n as f64);
                (r * cos, r * sin)
            })
            .collect()
    }

    /// The columns of `part` as found, and in the order they are sewn, each with its place among those found.
    fn sewn(part: &Polygon) -> (Vec<Column>, Vec<(usize, Column)>) {
        let mut meter = Budget::DEFAULT.meter();
        let found = strips::columns(part, piece(part), &[], 1.0, &mut meter).unwrap().unwrap().unwrap();
        let order = ordered(&found, &mut meter).unwrap();
        (found, order)
    }

    /// The bounds of a shape with the rules' defaults in a design whose narrowest satin stroke is `narrowest`
    /// millimetres.
    fn bounds(narrowest: f64) -> Bounds {
        Bounds::of(&AutoParams::defaults(), Mm::new(narrowest).unwrap())
    }

    /// Where `column`'s centre line starts and ends.
    fn ends(column: &Column) -> (Point, Point) {
        (column.line.points[0], *column.line.points.last().unwrap())
    }

    #[test]
    fn req_auto_016_a_column_no_wider_than_the_narrowest_satin_stroke_runs() {
        // A band 1.5 mm wide is one column: a satin column where the narrowest satin stroke is 1 mm, and a
        // running stitch along its line, as wide as the band, where it is 2 mm.
        let made = |narrowest: f64| columns(&band(1.5), &bounds(narrowest), &AutoParams::defaults(), &[], &mut Budget::DEFAULT.meter()).unwrap();
        let Made::Columns { sewn, .. } = made(1.0) else { panic!("no columns") };
        let [Sewn::Satin { width, push, .. }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!((width.get() - 1.5).abs() < 1e-6 && *push == [0.0, 0.0], "{width:?} {push:?}");
        let Made::Columns { sewn, .. } = made(2.0) else { panic!("no columns") };
        let [Sewn::Running { width, line }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!((width.get() - 1.5).abs() < 1e-6 && line.points.iter().all(|q| (q.y() - 0.75).abs() < 1e-9), "{width:?} {line:?}");
    }

    #[test]
    fn req_auto_016_a_column_wider_than_the_narrowest_stroke_somewhere_along_its_line_is_stitched_across() {
        // A strand of fur as a traced photo has it: 0.6 mm wide for 6 mm, widening over 1 mm to a head 1.4 mm
        // wide and 3 mm long. It is under 1 mm wide on average along its line, which ran along it and left the
        // head's sides bare. Where the narrowest satin stroke is 1 mm, it is one satin column, and where that
        // stroke is 2 mm, it runs.
        let strand = part(&[(0.0, -0.3), (6.0, -0.3), (7.0, -0.7), (10.0, -0.7), (10.0, 0.7), (7.0, 0.7), (6.0, 0.3), (0.0, 0.3)], &[]);
        let made = |narrowest: f64| columns(&strand, &bounds(narrowest), &AutoParams::defaults(), &[], &mut Budget::DEFAULT.meter()).unwrap();
        let Made::Columns { sewn, .. } = made(1.0) else { panic!("no columns") };
        let [Sewn::Satin { width, .. }] = sewn.as_slice() else { panic!("{sewn:?}") };
        assert!(width.get() < 1.0, "{width:?}");
        let Made::Columns { sewn, .. } = made(2.0) else { panic!("no columns") };
        assert!(matches!(sewn.as_slice(), [Sewn::Running { .. }]), "{sewn:?}");
    }

    #[test]
    fn req_auto_016_a_column_runs_no_wider_than_the_narrowest_stroke_on_average_at_its_widest_or_reach() {
        let mm = |length: f64| Mm::new(length).unwrap();
        let measured = |average: f64, widest: f64, farthest: f64| Width { average: mm(average), widest, farthest };
        assert!(runs(measured(0.8, 1.0, 1.0), mm(1.0)));
        assert!(!runs(measured(0.8, 1.001, 0.5), mm(1.0)));
        assert!(!runs(measured(0.8, 0.9, 1.001), mm(1.0)));
        assert!(!runs(measured(1.001, 0.9, 0.5), mm(1.0)));
    }

    #[test]
    fn req_auto_016_a_narrow_letters_junctions_leave_it_running() {
        // A T 14.5 mm tall in strokes 0.9 mm wide, upright and mirrored. Where its stem meets its bar, the
        // circle at the junction's middle, on the bar's line, is up to 1.13 mm wide, by where the junction's
        // triangle has its corners, which mirroring changes. Measured there, the bar sewed a satin column one
        // way round and a triple run the other (`tests/transformed.rs`). The junction measures where the
        // strokes meet, not the bar, and both run, either way round.
        for flip in [1.0, -1.0] {
            let t = part(
                &[(0.0, 0.0), (8.0, 0.0), (8.0, 0.9), (4.45, 0.9), (4.45, 14.5), (3.55, 14.5), (3.55, 0.9), (0.0, 0.9)].map(|(x, y)| (flip * x, y)),
                &[],
            );
            let made = columns(&t, &bounds(1.0), &AutoParams::defaults(), &[], &mut Budget::DEFAULT.meter()).unwrap();
            let Made::Columns { sewn, .. } = made else { panic!("no columns") };
            assert!(sewn.len() == 2 && sewn.iter().all(|s| matches!(s, Sewn::Running { .. })), "{flip}: {sewn:?}");
        }
    }

    #[test]
    fn req_auto_016_each_column_sews_from_its_end_nearest_where_the_last_ended() {
        // An H, its bars and its crossbar 3 mm wide. The crossbar ends under both bars and sews first, then the
        // bar with an end nearest where it ended, from that end, and the other bar from its end nearest where
        // that one ended. The crossbar lies low and high, so that a bar is turned round in one H and not in
        // the other.
        let (mut turned, mut kept) = (0, 0);
        for (low, high) in [(4.0, 7.0), (13.0, 16.0)] {
            let h = part(
                &[
                    (0.0, 0.0),
                    (3.0, 0.0),
                    (3.0, low),
                    (10.0, low),
                    (10.0, 0.0),
                    (13.0, 0.0),
                    (13.0, 20.0),
                    (10.0, 20.0),
                    (10.0, high),
                    (3.0, high),
                    (3.0, 20.0),
                    (0.0, 20.0),
                ],
                &[],
            );
            let (found, order) = sewn(&h);
            let [(crossbar, first), (_, second), (_, third)] = order.as_slice() else { panic!("{order:?}") };
            assert_eq!(found[*crossbar].under.iter().flatten().count(), 2, "{found:?}");
            let here = ends(first).1;
            assert!(nearest_end(second, here) < nearest_end(third, here), "{order:?}");
            for (column, here) in [(second, here), (third, ends(second).1)] {
                let (start, end) = ends(column);
                assert!(start.distance(here) < end.distance(here), "{column:?} from {here:?}");
            }
            for (place, column) in &order {
                if *column == found[*place] { kept += 1 } else { turned += 1 }
            }
        }
        assert!(turned > 0 && kept > 0, "{turned} turned, {kept} kept");
    }

    #[test]
    fn a_column_is_as_far_as_its_nearer_end_and_a_ring_as_its_nearest_point() {
        let [column] = sewn(&band(3.0)).0.try_into().unwrap();
        let (start, end) = ends(&column);
        let (by_start, by_end) = (Point::new(start.x(), start.y() + 4.0).unwrap(), Point::new(end.x(), end.y() - 2.0).unwrap());
        assert!((nearest_end(&column, by_start) - 4.0).abs() < 1e-9 && (nearest_end(&column, by_end) - 2.0).abs() < 1e-9);
        assert!(end_nearer(&column, by_end) && !end_nearer(&column, by_start));
        // As near both ends, it keeps its way.
        let point = |x: f64, y: f64| Point::new(x, y).unwrap();
        let mut level = column;
        level.line = Line { points: vec![point(0.0, 0.0), point(10.0, 0.0)], closed: false };
        assert!(!end_nearer(&level, point(5.0, 3.0)) && end_nearer(&level, point(5.001, 3.0)));
        // A ring 3 mm wide round a hole 10 mm across, its line about 6.5 mm from the centre.
        let ring = part(&arc(8.0, 24, 0.0, std::f64::consts::TAU)[..24], &[&arc(5.0, 24, 0.0, std::f64::consts::TAU)[..24]]);
        let [ring] = sewn(&ring).0.try_into().unwrap();
        let far = nearest_end(&ring, Point::new(20.0, 0.0).unwrap());
        assert!((far - 13.5).abs() < 0.2, "{far}");
    }

    #[test]
    fn columns_that_end_under_each_other_round_a_ring_all_sew() {
        // A pinwheel: a square ring of bars 3 mm wide round a hole 9 mm across, each bar running on 3 mm past
        // the corner where the bar before it ends against its side. Each bar's column ends under the next
        // one's, round the ring, so none can sew after all those it ends under: the longest sews first, the
        // first found of as long, and then each column the last one ends under.
        let pinwheel = part(
            &[
                (-3.0, 0.0),
                (12.0, 0.0),
                (12.0, -3.0),
                (15.0, -3.0),
                (15.0, 12.0),
                (18.0, 12.0),
                (18.0, 15.0),
                (3.0, 15.0),
                (3.0, 18.0),
                (0.0, 18.0),
                (0.0, 3.0),
                (-3.0, 3.0),
            ],
            &[&[(3.0, 3.0), (12.0, 3.0), (12.0, 12.0), (3.0, 12.0)]],
        );
        let (found, order) = sewn(&pinwheel);
        assert_eq!(found.len(), 4);
        assert!(found.iter().all(|column| column.under.iter().flatten().count() == 1), "{found:?}");
        let places: Vec<usize> = order.iter().map(|(place, _)| *place).collect();
        assert_eq!(places[0], 0, "{places:?}");
        for pair in places.windows(2) {
            assert!(found[pair[0]].under.contains(&Some(pair[1])), "{places:?}");
        }
    }

    #[test]
    fn the_longest_column_that_can_sew_first_sews_first() {
        // A plus of bars 3 mm wide: the column through its middle waits for the 3 that end under it, and of
        // those the longest sews first, the first found of as long.
        let plus = part(
            &[
                (8.5, 0.0),
                (11.5, 0.0),
                (11.5, 13.5),
                (25.0, 13.5),
                (25.0, 16.5),
                (11.5, 16.5),
                (11.5, 30.0),
                (8.5, 30.0),
                (8.5, 16.5),
                (0.0, 16.5),
                (0.0, 13.5),
                (8.5, 13.5),
            ],
            &[],
        );
        let (found, order) = sewn(&plus);
        let free: Vec<usize> = (0..found.len()).filter(|&c| found.iter().all(|column| !column.under.contains(&Some(c)))).collect();
        assert!(free.len() > 1, "{found:?}");
        let length = |c: usize| found[c].walked.length();
        let longest = free.iter().copied().fold(free[0], |best, c| if length(c) > length(best) { c } else { best });
        assert_eq!(order[0].0, longest, "{order:?}");
    }

    #[test]
    fn a_ring_sewn_after_another_column_starts_at_its_rung_nearest_where_that_ended() {
        // A ring 3 mm wide round a hole 10 mm across, with a stem 3 mm wide above it, 10 mm long, and one below
        // it, 6 mm long, each ending under the ring. The stems sew first, the longer first, and then the ring,
        // from its rung nearest where the second stem ended.
        let side = math::acos(1.5 / 8.0);
        let mut outline = arc(8.0, 16, side, -2.0 * side);
        outline.extend([(1.5, -14.0), (-1.5, -14.0)]);
        outline.extend(arc(8.0, 16, side - std::f64::consts::PI, -2.0 * side));
        outline.extend([(-1.5, 18.0), (1.5, 18.0)]);
        let ringed = part(&outline, &[&arc(5.0, 24, 0.0, std::f64::consts::TAU)[..24]]);
        let (found, order) = sewn(&ringed);
        let [(_, _), (_, last), (place, ring)] = order.as_slice() else { panic!("{order:?}") };
        assert!(ring.closed && !last.closed, "{order:?}");
        let here = ends(last).1;
        let starts = found[*place].rungs.iter().map(|rung| rung[0]).chain([found[*place].rails[0][0]]);
        let nearest = starts.map(|q| q.distance(here)).fold(f64::INFINITY, f64::min);
        assert!(ring.rails[0][0].distance(here) <= nearest + 1e-9, "{:?} from {here:?}", ring.rails[0][0]);
    }

    #[test]
    fn req_auto_035_a_column_s_longest_stitch_lies_between_the_points_as_far_along_its_rails() {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let longest = |first: &[(f64, f64)], second: &[(f64, f64)], rungs: &[[(f64, f64); 2]]| {
            let rails = [first.iter().map(|&(x, y)| p(x, y)).collect(), second.iter().map(|&(x, y)| p(x, y)).collect()];
            let rungs: Vec<[Point; 2]> = rungs.iter().map(|rung| rung.map(|(x, y)| p(x, y))).collect();
            longest_stitch(&rails, &rungs, &mut Budget::DEFAULT.meter()).unwrap()
        };
        let near = |found: f64, length: f64| (found - length).abs() < 1e-9;
        // Rails 3 mm apart: 3 mm. Rails that part from 2 mm to 6 mm: 6 mm, at their ends.
        assert!(near(longest(&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)], &[]), 3.0));
        assert!(near(longest(&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 2.0), (10.0, 6.0)], &[]), 6.0));
        // A rail with a corner 8 mm from the other rail's middle, halfway along both: 8 mm, at the corner. With
        // the corner a quarter of the way along its rail, the stitch from it reaches the other rail a quarter of
        // the way along that one.
        assert!(near(longest(&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (5.0, 8.0), (10.0, 3.0)], &[]), 8.0));
        let quarter = longest(&[(0.0, 0.0), (10.0, 0.0), (20.0, 0.0)], &[(0.0, 1.0), (5.0, 9.0), (20.0, 1.0)], &[]);
        let along = 26.433_981_132_056_6_f64;
        let reached = 20.0 * (89.0_f64.sqrt() / along);
        assert!(near(quarter, math::hypot(5.0 - reached, 9.0)), "{quarter}");
        // A rung pairs a point 2 mm along one rail with one 8 mm along the other: the stitch there is as
        // long as the rung, and the sections before and after it sew no longer one.
        assert!(near(longest(&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)], &[[(2.0, 0.0), (8.0, 3.0)]]), math::hypot(6.0, 3.0)));
        // A rung that lies before the one before it pairs no point behind that one's.
        let back = longest(&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 3.0), (10.0, 3.0)], &[[(6.0, 0.0), (6.0, 3.0)], [(2.0, 0.0), (2.0, 3.0)]]);
        assert!(near(back, 3.0), "{back}");
        // Rails of one point each sew from one to the other, and a column without rails sews nothing.
        assert!(near(longest(&[(0.0, 0.0)], &[(0.0, 3.0)], &[]), 3.0));
        assert_eq!(longest(&[], &[], &[]), 0.0);
        // Where a rail's point lies, and the point that far along it: past its length, its last point.
        let rail = [p(0.0, 0.0), p(4.0, 0.0), p(4.0, 3.0)];
        let lengths = lengths(&rail);
        assert_eq!(lengths, [0.0, 4.0, 7.0]);
        let at = |q: Point| position(&rail, &lengths, q);
        assert!(near(at(p(1.0, 2.0)), 1.0) && near(at(p(6.0, 1.0)), 5.0) && near(at(p(-3.0, 0.0)), 0.0) && near(at(p(9.0, 9.0)), 7.0));
        assert_eq!(
            [point_at(&rail, &lengths, 2.0), point_at(&rail, &lengths, 5.5), point_at(&rail, &lengths, 9.0)],
            [Some(p(2.0, 0.0)), Some(p(4.0, 1.5)), Some(p(4.0, 3.0))]
        );
        assert_eq!([point_at(&rail, &lengths, 0.0), point_at(&[], &[], 1.0)], [Some(p(0.0, 0.0)), None]);
        // A point as near 2 pieces of a rail lies where the first of them has it: 5 mm along a rail that
        // comes back past it 17 mm along.
        let there_and_back = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 2.0), p(0.0, 2.0)];
        assert_eq!(position(&there_and_back, &super::lengths(&there_and_back), p(5.0, 1.0)), 5.0);
        // A point repeated has no length between its copies.
        let doubled = [p(0.0, 0.0), p(0.0, 0.0), p(2.0, 0.0)];
        assert_eq!(point_at(&doubled, &super::lengths(&doubled), 0.0), Some(p(0.0, 0.0)));
        let ended_twice = [p(0.0, 0.0), p(2.0, 0.0), p(2.0, 0.0)];
        assert_eq!(point_at(&ended_twice, &super::lengths(&ended_twice), 5.0), Some(p(2.0, 0.0)));
    }

    #[test]
    fn req_auto_031_a_part_whose_lines_have_no_width_has_no_measures() {
        // A band's column with its centre line moved onto the band's edge: no circle fits on it, so the part
        // has no width to be slender or uneven by.
        let band = band(3.0);
        let mut meter = Budget::DEFAULT.meter();
        let edges = Edges::of(&band, piece(&band), &mut meter).unwrap();
        let (found, _) = sewn(&band);
        assert!(measured(&band, &found, &edges, &mut meter).unwrap().is_some_and(|measures| measures.width > 2.9));
        let edge = Line { points: [2.0, 10.0, 18.0].map(|x| Point::new(x, 0.0).unwrap()).to_vec(), closed: false };
        let mut on_the_edge = found;
        for column in &mut on_the_edge {
            column.line = edge.clone();
        }
        assert_eq!(measured(&band, &on_the_edge, &edges, &mut meter), Ok(None));
    }

    #[test]
    fn req_auto_035_measuring_a_column_s_longest_stitch_is_the_part_s_work() {
        // Each rung and each corner costs work: with too little, the measuring stops.
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let rails = [vec![p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)], vec![p(0.0, 3.0), p(10.0, 3.0)]];
        let rungs = [[p(5.0, 0.0), p(5.0, 3.0)]];
        let with = |work: u64| longest_stitch(&rails, &rungs, &mut Budget { max_work: work, ..Budget::DEFAULT }.meter());
        let least = (0..64).find(|work| with(*work).is_ok()).unwrap();
        assert_eq!(with(least), Ok(3.0));
        assert!((0..least).all(|work| with(work) == Err(Exhausted::Work)), "{least}");
        // The rung costs the rails' 5 corners, and each of the 2 sections it makes its 2 ends. The corner the
        // rung lies at is an end of both sections, and is measured as an end alone.
        assert_eq!(least, 9);
    }

    #[test]
    fn req_auto_033_the_layers_ask_whether_the_rules_fill_a_part() {
        let filled = |part: &Polygon, set: &[(&str, &str)]| {
            let rules = AutoParams::from_set(&set.iter().copied().collect()).unwrap().params;
            filled(part, &Bounds::of(&rules, Mm::new(1.0).unwrap()), &mut Budget::DEFAULT.meter()).unwrap()
        };
        // A square 10 mm across is an area, a blob 4.5 by 3 mm compact, and a band 3 mm wide a column: the
        // first 2 are filled.
        let square = part(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], &[]);
        let blob = part(&[(0.0, 0.0), (4.5, 0.0), (4.5, 3.0), (0.0, 3.0)], &[]);
        assert_eq!([filled(&square, &[]), filled(&blob, &[]), filled(&band(3.0), &[])], [Some(RuleId::Area), Some(RuleId::Compact), None]);
        // A strip 0.6 mm wide is a line, and a disc 1.5 mm across, without a centre line, is compact.
        let dot: Vec<(f64, f64)> = arc(0.75, 8, 0.0, std::f64::consts::TAU).into_iter().take(8).collect();
        assert_eq!([filled(&band(0.6), &[]), filled(&part(&dot, &[]), &[])], [None, Some(RuleId::Compact)]);
        // A T of a thick and a thin stroke is uneven, unless its shape fills no uneven part.
        let t = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 4.0), (10.75, 4.0), (10.75, 16.0), (9.25, 16.0), (9.25, 4.0), (0.0, 4.0)], &[]);
        assert_eq!([filled(&t, &[]), filled(&t, &[("auto_fill_uneven", "false")])], [Some(RuleId::Uneven), None]);
        // A band of a shape that asks to be filled is, and of one that asks for satin columns isn't, compact
        // though it is.
        assert_eq!([filled(&band(3.0), &[("auto_stitch", "fill")]), filled(&blob, &[("auto_stitch", "satin")])], [Some(RuleId::Asked), None]);
        // A square with a round hole, whose column would sew too long a stitch at its corners, is refused, and
        // filled so.
        let hole: Vec<(f64, f64)> = arc(4.0, 48, 0.0, -std::f64::consts::TAU).into_iter().take(48).collect();
        let ring = part(&[(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)], &[&hole]);
        assert_eq!(filled(&ring, &[]), Some(RuleId::Refused));
        // A ring whose hole touches its outline makes no column, and isn't said to be filled: it keeps its hole.
        let touching: Vec<(f64, f64)> = arc(2.0, 16, 0.0, -std::f64::consts::TAU).into_iter().take(16).map(|(x, y)| (x + 2.0, y)).collect();
        let outer: Vec<(f64, f64)> = arc(4.0, 32, 0.0, std::f64::consts::TAU).into_iter().take(32).collect();
        assert_eq!(filled(&part(&outer, &[&touching]), &[]), None);
    }

    #[test]
    fn req_auto_031_a_part_s_columns_say_which_rule_it_sews_by_and_what_was_measured() {
        let made = |part: &Polygon, set: &[(&str, &str)]| {
            let rules = AutoParams::from_set(&set.iter().copied().collect()).unwrap().params;
            columns(part, &Bounds::of(&rules, Mm::new(1.0).unwrap()), &rules, &[], &mut Budget::DEFAULT.meter()).unwrap()
        };
        // A band 3 by 20 mm: a column, 3 mm wide, of one line, 6.7 times its width squared.
        let Made::Columns { sewn, rule, measures } = made(&band(3.0), &[]) else { panic!("no columns") };
        assert_eq!((sewn.len(), rule, measures.lines, measures.breadth), (1, RuleId::Column, 1, Breadth::Medium));
        assert!(
            (measures.width - 3.0).abs() < 1e-6 && (measures.slenderness() - 60.0 / 9.0).abs() < 1e-4 && measures.unevenness < 1e-6,
            "{measures:?}"
        );
        // A blob 4.5 by 3 mm: compact, and a column when its shape asks for satin columns.
        let blob = part(&[(0.0, 0.0), (4.5, 0.0), (4.5, 3.0), (0.0, 3.0)], &[]);
        let Made::Filled(RuleId::Compact, Some(measures)) = made(&blob, &[]) else { panic!("not compact") };
        assert!((measures.area - 13.5).abs() < 1e-9 && measures.slenderness() < 3.0, "{measures:?}");
        let Made::Columns { rule: RuleId::Asked, .. } = made(&blob, &[("auto_stitch", "satin")]) else { panic!("not as asked") };
        // A dot is compact.
        let dot: Vec<(f64, f64)> = arc(0.75, 8, 0.0, std::f64::consts::TAU).into_iter().take(8).collect();
        assert!(matches!(made(&part(&dot, &[]), &[]), Made::Filled(RuleId::Compact, _)));
        // A square with a round hole: its column's longest stitch, with what was measured of it.
        let hole: Vec<(f64, f64)> = arc(4.0, 48, 0.0, -std::f64::consts::TAU).into_iter().take(48).collect();
        let ring = part(&[(-8.0, -8.0), (8.0, -8.0), (8.0, 8.0), (-8.0, 8.0)], &[&hole]);
        let Made::Long(stitch, measures) = made(&ring, &[]) else { panic!("not refused") };
        assert!((stitch - 7.314).abs() < 0.001 && (measures.width - 4.5).abs() < 0.1, "{stitch} {measures:?}");
        let said = long(&ElementId::new("ring").unwrap(), stitch, &bounds(1.0));
        assert_eq!(
            said.to_string(),
            "info SC-I0902: A satin column along a part of this area would sew a stitch 7.31 mm long between its rails, longer than the widest \
             satin column, 7 mm, and such a stitch lies loose, so the part is filled."
        );
        assert_eq!((said.code, said.element.as_ref().map(|id| id.as_str())), (Code::AutoSatinFilled, Some("ring")));
        // With a widest satin column of 5 mm, a ring 12 mm across, whose longest stitch is 5.49 mm, is refused too.
        let small: Vec<(f64, f64)> = arc(3.0, 48, 0.0, -std::f64::consts::TAU).into_iter().take(48).collect();
        let ring = part(&[(-6.0, -6.0), (6.0, -6.0), (6.0, 6.0), (-6.0, 6.0)], &[&small]);
        assert!(matches!(made(&ring, &[]), Made::Columns { .. }) && matches!(made(&ring, &[("auto_max_satin_width_mm", "5")]), Made::Long(..)));
    }

    #[test]
    fn a_part_no_satin_column_can_be_drawn_along_is_said_to_stay_filled() {
        let id = ElementId::new("letters").unwrap();
        let said = unmade(&id, &bounds(1.0));
        assert_eq!((said.code, said.element.as_ref()), (Code::AutoSatinFilled, Some(&id)));
        assert_eq!(
            said.to_string(),
            "info SC-I0902: A part of this area is no wider than a satin column, 7 mm, but no satin columns can be drawn along it between \
             its edges, so it is filled."
        );
    }
}
