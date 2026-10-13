//! A tatami fill sewn: its parts one at a time, in each part the layers of its underlay and then its top
//! rows, each layer's rows in the order `route` gives, with running stitches along the needle's way
//! between them (`REQ-FILL-TAT-001`, `REQ-FILL-TAT-008`, `REQ-FILL-TAT-011`).
//!
//! **Parts.** Ink/Stitch sews a fill's parts nearest the needle first, a part the needle is in being 0
//! away, and with no needle, from the part farthest left. Each part ends at its point nearest the next
//! part, and the last at the point of the fill nearest the next element: its first stitch, or for an
//! element that starts nearest the needle, its shape. A part is a group of its own, which assembly joins to
//! the next as it joins any 2 groups, with a jump and the locks and trims the settings ask for, or
//! straight on when they are close (`SC-W0307` says the fill is in parts).
//!
//! **Rows.** A part's segments are taken row by row, across the rows, and in each row by how far their
//! start lies from the corner of the part's bounding box where x and y are least, as Ink/Stitch takes them.
//! Each is sewn with its needle points ([`super::stitches`]) from the end the route enters it by.
//!
//! **Travel.** Between rows the needle runs under the rows not sewn yet (`underpath`, the `underpath` module),
//! in running stitches of the running stitch length, within its tolerance (`running::along_line`). With
//! `underpath` off, in a part the lines miss, and between 2 nodes no line joins, as to a hole small enough to
//! lie between the lines, it runs along the part's rings and rows instead (the `travel` module), as
//! Ink/Stitch's runs along the outline. The travel's first stitch lands on the row's end, which the row has sewn
//! already, so it is left out, as in Ink/Stitch. With `skip_last`, the row's end is not sewn, and the
//! travel keeps its first stitch, unless the next row starts beside it. Rows and rings join every node of a
//! part, so a way along them always leads on; were none found, the needle would jump, and `SC-E0009` would
//! say so as a bug.
//!
//! **No rows.** A part too thin for any row is sewn as a running stitch round its outline, from the
//! outline's start, as Ink/Stitch sews it (`SC-W0305`).
//!
//! **A needle inside the part.** With the lines, the part starts at their node nearest the needle, as
//! Ink/Stitch's does: for a needle on the rings or outside the part, the route's start, and for one inside,
//! as after an inset underlay, a node inside, from which the needle travels under the rows to the route's
//! start, or on to the end of its first stretch of travel.
//!
//! **Underlay.** Before its top rows each part sews its underlay, a [`Layer`] for each angle, in the order
//! given (`REQ-FILL-TAT-011`). A layer is sewn as the top rows are, with rows, travel and no row as above, but
//! from the point of the part's rings nearest the needle back to that point, as Ink/Stitch sews a fill
//! with nothing to end near. The next layer, and then the top rows, start where the last ended, so the
//! part stays one run. A layer with no row goes round the outline from its start, as the top rows do
//! (`SC-I0308`), and that start can lie away from the needle. The needle goes on to it as Ink/Stitch's
//! stitch plan joins a layer to the next: straight on within the element's jump length, and farther in
//! a run of its own, which assembly reaches with a jump. The join is made here because Ink/Stitch never
//! forces locks after a layer of underlay, where assembly would after any group with
//! `force_lock_stitches`.
//!
//! **Inset and expand.** The underlay covers the part shrunk by its inset, and the top rows the part grown by
//! `expand` (`REQ-FILL-TAT-012`), as `normalize::offset` grows and shrinks it. Shrinking can cut a
//! part into pieces, sewn one after another in their order of least x: every layer on a piece, then the
//! next piece's, each from the point nearest the needle, and each piece's top rows from where the last
//! ended (`DEV-FILL-008`). Where shrinking leaves nothing, the part itself is sewn, as Ink/Stitch sews it,
//! and `SC-W0310` or `SC-W0309` says so.
//!
//! **Pull compensation.** The top rows reach past their segments' ends by `pull_compensation_mm` plus
//! `pull_compensation_percent` of the segment's length, at its start and at its end (`REQ-FILL-TAT-006`),
//! moved as a satin's stitches are (`generators::pull`). The part, its route and the needle's ways stay
//! as they are, and each way starts where the row before it ends and ends where the next row starts, both
//! lengthened. Ink/Stitch rebuilds the part round the lengthened rows instead, which can fill its holes and
//! drop a piece of it (`DEV-FILL-009`). Underlay is not compensated, as in Ink/Stitch.
//!
//! **Gap-fill rows.** Where the route leaves a stretch of more than 3 rows for another, the top rows sew
//! `gap_fill_rows` more past its last row, back and forth, and come back to where it ended (the `gaps`
//! module, `REQ-FILL-TAT-007`). Underlay has none, as in Ink/Stitch.
//!
//! **The shortest stitch.** Rows the row spacing apart start and end nearer each other than the shortest
//! stitch a machine sews well, and a row's first grid point can lie just past its start. A needle point
//! nearer the one before it than the shortest stitch is left out, so the stitch runs on to the next, as
//! Ink/Stitch's stitch plan leaves out points nearer than its own shortest stitch. A run's last point stays,
//! and the points before it that it is too near go instead. It is finalize's rule, the same function
//! (`crate::thin`), applied here so that the generator hands finalize stitches it has nothing to thin.

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use super::clamp::clamp;
use super::gaps::gap_rows;
use super::graph::NodeId;
use super::rings::Rings;
use super::route::{Route, Step, route};
use super::rows::rows;
use super::stitches::{Stitching, row_points};
use super::travel::{Network, between};
use super::underpath::{Underpath, smooth};
use crate::generators::pull::offset;
use crate::generators::running::along_line;
use crate::generators::{Approach, Stitched};
use crate::normalize::near::{nearest_pair, nearest_to_lines, nearest_to_point};
use crate::normalize::offset::buffer;
use crate::normalize::region::{Polygon, Region, Unjoined};
use crate::thin::thin;

/// How the needle travels between rows: in stitches this long, within this tolerance of the way, none
/// shorter than the shortest stitch, in millimetres, and under the rows or along the rings. And how far it
/// goes straight on to a layer that starts away from it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Travel {
    /// The running stitch's length.
    pub length: f64,
    /// How far a stitch may stray from the way.
    pub tolerance: f64,
    /// The shortest stitch.
    pub min_stitch: f64,
    /// Whether travel runs under the rows not sewn yet (`underpath`).
    pub underpath: bool,
    /// The element's jump length: a layer that starts no farther than this from the needle goes on from it
    /// with a straight stitch, and one farther away is a run of its own.
    pub jump: f64,
}

/// How far each row reaches past the ends of its segment, to make up for the thread pulling the fabric in
/// along it: millimetres past the segment's start and past its end, each with a share of the segment's
/// length added (`pull_compensation_mm`, `pull_compensation_percent`).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pull {
    /// Millimetres past the start and past the end.
    pub mm: [f64; 2],
    /// Shares of the segment's length past the start and past the end.
    pub share: [f64; 2],
}

/// One layer of a tatami fill: its top rows, or a layer of its underlay.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Layer {
    /// Where the rows lie and how they are stitched.
    pub stitching: Stitching,
    /// How the needle travels between them.
    pub travel: Travel,
    /// How far the rows reach past their ends.
    pub pull: Pull,
    /// How many rows to sew again where the route leaves a stretch of rows, an even number.
    pub gap_rows: usize,
}

/// A tatami fill as it is sewn: in each part, its underlay's layers over the part shrunk by `inset`, then
/// its top rows over the part grown by `expand`.
#[derive(Clone, Debug, PartialEq)]
pub struct Fill {
    /// The top rows.
    pub top: Layer,
    /// How far each part is grown for its top rows, in millimetres; shrunk where it is negative.
    pub expand: f64,
    /// The underlay's layers, in the order they are sewn.
    pub underlay: Vec<Layer>,
    /// How far each part is shrunk for its underlay, in millimetres; grown where it is negative.
    pub inset: f64,
}

/// The stitches of the tatami `fill` over `region`: in each part the layers of its underlay in order, each
/// back to where it started, then its top rows, from the needle where the elements before left it to near
/// the `next` element. Random lengths draw from `rng`. Each part is a run, unless a layer with no row, or a
/// piece a part is cut into, starts farther from the needle than the jump length. Work is charged to
/// `meter` for every ring point, row, node, edge, step and stitch.
pub fn tatami_fill(
    region: &Region,
    fill: &Fill,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let parts = in_order(region, needle, meter)?;
    let end = ending(region, next, meter)?;
    let mut needle = needle;
    let (mut runs, mut warnings) = (Vec::new(), Vec::new());
    let mut tally = Tally::default();
    for (i, polygon) in parts.iter().enumerate() {
        let here = match parts.get(i + 1) {
            Some(after) => nearest_pair(&rings_of(polygon), &rings_of(after), meter)?.map(|(on, _)| on),
            None => end,
        };
        let mut sewn = Vec::new();
        if !fill.underlay.is_empty() {
            let (pieces, gone) = pieces(polygon, -fill.inset, &mut warnings, meter)?;
            tally.inset_gone += usize::from(gone);
            for piece in &pieces {
                let part = Part { polygon: piece, rings: Rings::new(piece, meter)? };
                for layer in &fill.underlay {
                    let (layer_runs, rowless) = part.sew(layer, needle, None, rng, meter, &mut warnings)?;
                    tally.bare += usize::from(rowless);
                    needle = layer_runs.iter().rev().find_map(|run| run.last().copied()).or(needle);
                    append(&mut sewn, layer_runs, layer.travel.jump);
                }
            }
        }
        let (pieces, gone) = pieces(polygon, fill.expand, &mut warnings, meter)?;
        tally.expand_gone += usize::from(gone);
        for piece in &pieces {
            let part = Part { polygon: piece, rings: Rings::new(piece, meter)? };
            let (top_runs, rowless) = part.sew(&fill.top, needle, here, rng, meter, &mut warnings)?;
            (tally.tops, tally.outlined) = (tally.tops + 1, tally.outlined + usize::from(rowless));
            needle = top_runs.iter().rev().find_map(|run| run.last().copied()).or(needle);
            append(&mut sewn, top_runs, fill.top.travel.jump);
        }
        let sewn: Vec<Vec<Point>> = sewn
            .into_iter()
            .map(|run| thin(run, |a, b| !at_least(a.distance(*b), fill.top.travel.min_stitch), |_| false, |_| ()))
            .filter(|run| !run.is_empty())
            .collect();
        needle = sewn.iter().rev().find_map(|run| run.last().copied()).or(needle);
        runs.extend(sewn);
    }
    warnings.extend(too_thin(tally.outlined, tally.tops));
    warnings.extend(underlay_too_thin(tally.bare));
    let parts = parts.len();
    warnings.extend(shrunk_away(Code::FillExpandShrinksAway, "expand_mm", "its top rows are", tally.expand_gone, parts));
    warnings.extend(shrunk_away(Code::FillUnderlayInsetShrinksAway, "fill_underlay_inset_mm", "its underlay is", tally.inset_gone, parts));
    Ok(Stitched { runs, warnings })
}

/// What sewing a fill counted, for its diagnostics.
#[derive(Default)]
struct Tally {
    /// The pieces sewn with top rows, and those of them that found no row and went round the outline.
    tops: usize,
    outlined: usize,
    /// The layers of underlay that found no row.
    bare: usize,
    /// The parts that `expand` and that `inset` left nothing of.
    expand_gone: usize,
    inset_gone: usize,
}

/// The pieces `polygon` grown by `distance` falls into, or `polygon` itself where that leaves nothing, as
/// Ink/Stitch sews it; and whether it left nothing. Where the grown area's edges do not join into rings, a
/// bug that `warnings` gets, `polygon` itself.
fn pieces(polygon: &Polygon, distance: f64, warnings: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<(Vec<Polygon>, bool), Exhausted> {
    let grown = buffer(polygon, distance, meter)?;
    let joined = grown.is_ok();
    warnings.extend(grown.as_ref().err().copied().map(Unjoined::grown));
    let pieces = grown.unwrap_or_default();
    Ok(if pieces.is_empty() { (vec![polygon.clone()], joined) } else { (pieces, false) })
}

/// `code` for the `gone` parts of `parts` that `key` shrank away entirely, whose `what` sewn over all of
/// the part.
fn shrunk_away(code: Code, key: &str, what: &str, gone: usize, parts: usize) -> Option<Diagnostic> {
    let message = match (gone, parts) {
        (0, _) => return None,
        (1, 1) => format!("`{key}` shrinks this fill away entirely, so {what} sewn over all of it."),
        (1, _) => format!("`{key}` shrinks 1 of this fill's parts away entirely, so {what} sewn over all of that part."),
        (n, _) => format!("`{key}` shrinks {n} of this fill's parts away entirely, so on each {what} sewn over all of the part."),
    };
    Some(Diagnostic::new(code, message))
}

/// `SC-W0305` for the `outlined` pieces of the `tops` sewn with top rows that found no row and went round
/// the outline: the parts of the fill, or the pieces `expand_mm` cut them into.
fn too_thin(outlined: usize, parts: usize) -> Option<Diagnostic> {
    let message = match (outlined, parts) {
        (0, _) => return None,
        (1, 1) => "This fill is too thin for a row of stitches at its row spacing, so it is sewn as a running stitch round its outline.".to_string(),
        (1, _) => "1 of this fill's parts is too thin for a row of stitches at its row spacing, so it is sewn as a running stitch round its \
                   outline."
            .to_string(),
        (n, _) => format!(
            "{n} of this fill's parts are too thin for a row of stitches at its row spacing, so each is sewn as a running stitch round its \
             outline."
        ),
    };
    Some(Diagnostic::new(Code::FillTooThin, message))
}

/// `SC-I0308` for the `bare` layers of underlay that found no row and went round their part's outline.
fn underlay_too_thin(bare: usize) -> Option<Diagnostic> {
    let message = match bare {
        0 => return None,
        1 => "1 layer of this fill's underlay finds no row at the underlay's row spacing, so it is sewn as a running stitch round its part's \
              outline."
            .to_string(),
        n => format!(
            "{n} layers of this fill's underlay find no row at the underlay's row spacing, so each is sewn as a running stitch round its part's \
             outline."
        ),
    };
    Some(Diagnostic::new(Code::FillUnderlayTooThin, message))
}

/// Adds the runs of the next layer to `runs`. Its first run goes on from the last of `runs` when it starts
/// no farther than `jump` from where that one ends, as a layer starts where the last ended, or as
/// Ink/Stitch sews on to a layer that starts near; otherwise it is a run of its own.
fn append(runs: &mut Vec<Vec<Point>>, more: Vec<Vec<Point>>, jump: f64) {
    let mut more = more.into_iter();
    let Some(first) = more.next() else { return };
    match runs.last_mut() {
        Some(last) if last.last().zip(first.first()).is_some_and(|(a, b)| at_least(jump, a.distance(*b))) => extend(last, first),
        _ => runs.push(first),
    }
    runs.extend(more);
}

/// A part, with its rings measured once for all its layers.
struct Part<'a> {
    polygon: &'a Polygon,
    rings: Rings<'a>,
}

impl Part<'_> {
    /// The runs `layer` sews over the part, from the point of its rings nearest `start` to the point
    /// nearest `end`, or back to where it started without an `end`, and whether it found no row and went
    /// round the outline. A jump the route needs is said in `warnings`.
    fn sew(
        &self,
        layer: &Layer,
        start: Option<Point>,
        end: Option<Point>,
        rng: &mut SplitMix64,
        meter: &mut Meter,
        warnings: &mut Vec<Diagnostic>,
    ) -> Result<(Vec<Vec<Point>>, bool), Exhausted> {
        let Layer { stitching, travel, pull, gap_rows } = layer;
        let (part, rings) = (self.polygon, &self.rings);
        let (segments, rows) = segments(part, stitching, meter)?;
        let Some(route) = route(rings, &segments, start, end, meter)? else {
            return Ok((vec![along_line(&part.outline, travel.length, travel.tolerance, travel.min_stitch, meter)?], true));
        };
        let lines = if travel.underpath { Underpath::new(part, rings, &route.nodes, &segments, stitching.grid.angle, meter)? } else { None };
        let network = Network::new(&route.nodes, rings, &row_ends(&route), meter)?;
        let mut sewing = Sewing {
            route: &route,
            rings,
            stitching,
            travel: *travel,
            lines,
            network,
            start,
            segments: &segments,
            rows: &rows,
            pull: *pull,
            gap_rows: *gap_rows,
        };
        Ok((sewing.sew(rng, meter, warnings)?, false))
    }
}

/// A part's rings as polylines: the outline, then the holes.
fn rings_of(part: &Polygon) -> Vec<&[Point]> {
    part.rings().collect()
}

/// `region`'s parts in the order they are sewn: nearest `needle` first, or with no needle, by how far left
/// they reach; the drawing's order on a tie.
fn in_order<'a>(region: &'a Region, needle: Option<Point>, meter: &mut Meter) -> Result<Vec<&'a Polygon>, Exhausted> {
    let mut keyed = Vec::with_capacity(region.parts.len());
    for part in &region.parts {
        let key = match needle {
            Some(p) if covers(part, p, meter)? => 0.0,
            Some(p) => nearest_to_point(&rings_of(part), p, meter)?.map_or(f64::INFINITY, |(_, d)| d),
            None => part.outline.iter().map(|q| q.x()).fold(f64::INFINITY, f64::min),
        };
        keyed.push((key, part));
    }
    keyed.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(keyed.into_iter().map(|(_, part)| part).collect())
}

/// Where the fill ends: its point nearest what the `next` element offers. A point in the fill is itself.
/// Of a shape, the first line whose start lies in the fill ends it there, and otherwise the fill's point
/// nearest the shape, as Ink/Stitch's geometry library finds it.
fn ending(region: &Region, next: Option<&Approach>, meter: &mut Meter) -> Result<Option<Point>, Exhausted> {
    let rings: Vec<&[Point]> = region.parts.iter().flat_map(rings_of).collect();
    match next {
        None => Ok(None),
        Some(Approach::Point(p)) => {
            for part in &region.parts {
                if covers(part, *p, meter)? {
                    return Ok(Some(*p));
                }
            }
            Ok(nearest_to_point(&rings, *p, meter)?.map(|(on, _)| on))
        }
        Some(Approach::Shape(lines)) => {
            for part in &region.parts {
                for start in lines.iter().filter_map(|line| line.first()) {
                    if covers(part, *start, meter)? {
                        return Ok(Some(*start));
                    }
                }
            }
            nearest_to_lines(&rings, &lines.iter().map(Vec::as_slice).collect::<Vec<_>>(), meter)
        }
    }
}

/// Whether `p` lies in `part` or on its rings. One unit of `meter` for each of their points.
fn covers(part: &Polygon, p: Point, meter: &mut Meter) -> Result<bool, Exhausted> {
    meter.charge(rings_of(part).iter().map(|ring| u64::try_from(ring.len()).unwrap_or(u64::MAX)).fold(0, u64::saturating_add))?;
    Ok(part.covers(p))
}

/// A part's row segments, each from its start to its end, and the row of each, counted across.
type RowSegments = (Vec<(Point, Point)>, Vec<usize>);

/// The part's row segments, row by row across the rows and in each row by how far their start lies from
/// the corner of the part's bounding box where x and y are least, each from its start to its end; and the
/// row of each, counted across.
fn segments(part: &Polygon, stitching: &Stitching, meter: &mut Meter) -> Result<RowSegments, Exhausted> {
    let corner = part.outline.iter().fold((f64::INFINITY, f64::INFINITY), |(x, y), q| (x.min(q.x()), y.min(q.y())));
    let corner = Point::new(corner.0, corner.1).unwrap_or(Point::ORIGIN);
    let (mut segments, mut in_row) = (Vec::new(), Vec::new());
    for (i, row) in rows(part, &stitching.grid, meter)?.into_iter().enumerate() {
        let mut pieces = row.segments;
        pieces.sort_by(|a, b| a.start.distance(corner).total_cmp(&b.start.distance(corner)));
        in_row.extend(std::iter::repeat_n(i, pieces.len()));
        segments.extend(pieces.into_iter().map(|piece| (piece.start, piece.end)));
    }
    Ok((segments, in_row))
}

/// The ends of each row segment the route sews, in the order sewn.
fn row_ends(route: &Route) -> Vec<[NodeId; 2]> {
    route.steps.iter().filter_map(|step| if let Step::Row { from, to, .. } = *step { Some([from, to]) } else { None }).collect()
}

/// A part as it is sewn.
struct Sewing<'a> {
    route: &'a Route,
    rings: &'a Rings<'a>,
    stitching: &'a Stitching,
    travel: Travel,
    /// The lines under the rows, with `underpath` where they cross the part.
    lines: Option<Underpath>,
    /// The ways along the part's rings and rows.
    network: Network,
    /// Where the needle was before the part.
    start: Option<Point>,
    /// The part's row segments, each from its start to its end, and the row of each, counted across.
    segments: &'a [(Point, Point)],
    rows: &'a [usize],
    /// How far the rows reach past their ends.
    pull: Pull,
    /// How many rows to sew again where the route leaves a stretch of rows.
    gap_rows: usize,
}

/// The stretch of rows the route is sewing, for gap-fill rows: how many of its rows came after travel, as
/// Ink/Stitch counts them, whether the needle has travelled since the last row, and the segments of the last
/// row and the one before it.
struct Stretch {
    rows: usize,
    after_travel: bool,
    last: [Option<usize>; 2],
}

impl Stretch {
    /// The stretch a route starts with. Its first row counts as reached by travel: Ink/Stitch's route always
    /// starts with travel, from its node nearest the needle, of no length where that is the first row's start.
    const START: Self = Self { rows: 0, after_travel: true, last: [None, None] };

    /// Row segment `segment` is sewn.
    fn row(&mut self, segment: usize) {
        self.rows += usize::from(self.after_travel);
        self.after_travel = false;
        let [last, _] = self.last;
        self.last = [Some(segment), last];
    }

    /// The needle travels from node `from` to node `to`, which one edge joins (`beside`) when the travel
    /// goes no farther than the next node: from a row to the next beside it, or from the last row to the
    /// part's end beside it, as one edge of Ink/Stitch's route. Other travel leaves the stretch: the last
    /// row's segment and the one before it when the stretch had more than 3 rows.
    fn travel(&mut self, from: NodeId, to: NodeId, beside: bool) -> Option<[usize; 2]> {
        self.after_travel = true;
        if beside || from == to {
            return None;
        }
        let rows = std::mem::take(&mut self.rows);
        match self.last {
            [Some(last), Some(before)] if rows > 3 => Some([last, before]),
            _ => None,
        }
    }
}

impl Sewing<'_> {
    /// The runs the route sews: one, unless the needle has to jump, which is a bug that a diagnostic added
    /// to `warnings` says, once for the part.
    fn sew(&mut self, rng: &mut SplitMix64, meter: &mut Meter, warnings: &mut Vec<Diagnostic>) -> Result<Vec<Vec<Point>>, Exhausted> {
        let route = self.route;
        let point = |node: usize| route.nodes.get(node).map(|n| n.point);
        // Each step's row as it is sewn, between its segment's lengthened ends.
        let sewn: Vec<Option<[Point; 2]>> = route.steps.iter().map(|step| self.row(step)).collect();
        let start_of = |i: usize| sewn.get(i).copied().flatten().map(|[a, _]| a);
        let row_before = |i: usize| i.checked_sub(1).and_then(|i| sewn.get(i).copied().flatten());
        let mut runs = Vec::new();
        let mut run: Vec<Point> = Vec::new();
        let mut jumped = false;
        let lead = self.lead_in(meter)?;
        let skipped = usize::from(lead.as_ref().is_some_and(|(_, replaces)| *replaces));
        let mut stretch = Stretch::START;
        match lead {
            Some((mut way, _)) => {
                moved(&mut way, None, start_of(skipped));
                run.extend(along_line(&way, self.travel.length, self.travel.tolerance, self.travel.min_stitch, meter)?);
            }
            None => {
                if let Some(Step::Travel { from, .. }) = route.steps.first() {
                    run.extend(point(*from));
                }
            }
        }
        for (i, step) in route.steps.iter().enumerate().skip(skipped) {
            match *step {
                Step::Row { segment, .. } => {
                    let Some([a, b]) = sewn.get(i).copied().flatten() else { continue };
                    extend(&mut run, row_points(a, b, self.stitching, rng, meter)?);
                    if let Some(lines) = &mut self.lines {
                        lines.sew(segment);
                    }
                    stretch.row(segment);
                }
                Step::Travel { from, to, beside } => {
                    if let (Some(rows), Some(ended)) = (stretch.travel(from, to, beside), row_before(i)) {
                        self.fill_gap(&mut run, rows, ended, rng, meter)?;
                    }
                    let next = start_of(i + 1);
                    match self.way(from, to, meter)? {
                        Some(mut way) => {
                            moved(&mut way, row_before(i).map(|[_, b]| b), next);
                            let stitches = along_line(&way, self.travel.length, self.travel.tolerance, self.travel.min_stitch, meter)?;
                            let keep_first = self.stitching.skip_last && !(beside && next.is_some());
                            extend(&mut run, stitches.into_iter().skip(usize::from(!keep_first)));
                        }
                        None => {
                            jumped = true;
                            runs.push(std::mem::take(&mut run));
                            run.extend(next.or(point(to)));
                        }
                    }
                }
            }
        }
        runs.push(run);
        if jumped {
            warnings.push(Diagnostic::new(Code::InternalCheckFailed, "The needle found no way between 2 rows of this fill, so it jumps there."));
        }
        Ok(runs)
    }

    /// The needle's way from node `from` to node `to`: under the rows the cheapest way along the lines,
    /// smoothed and kept inside the part, and where no line joins them [`between`]'s way along the rings and
    /// rows, `None` where none leads either.
    fn way(&self, from: NodeId, to: NodeId, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
        if let Some(lines) = &self.lines
            && let Some(way) = lines.way(from, to, meter)?
        {
            return Ok(Some(self.smoothed(&way, meter)?));
        }
        between(&self.route.nodes, self.rings, &self.network, from, to, meter)
    }

    /// A way along the lines, smoothed and kept inside the part.
    fn smoothed(&self, way: &[Point], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        clamp(&smooth(way, self.travel.min_stitch, meter)?, self.rings, meter)
    }

    /// The row `step` sews, between its segment's ends lengthened by the pull compensation, from the end the
    /// route enters it by; `None` for travel.
    fn row(&self, step: &Step) -> Option<[Point; 2]> {
        let Step::Row { segment, from, .. } = *step else { return None };
        let (&segment, entry) = (self.segments.get(segment)?, self.route.nodes.get(from)?.point);
        Some(self.lengthened(segment, forward(entry, segment)))
    }

    /// `segment` lengthened by the pull compensation at both ends, from its start where `forward` and from
    /// its end otherwise.
    fn lengthened(&self, (start, end): (Point, Point), forward: bool) -> [Point; 2] {
        let [start, end] = offset([start, end], self.pull.mm, self.pull.share);
        if forward { [start, end] } else { [end, start] }
    }

    /// Sews onto `run` the rows that fill the gap after the row of segment `last`, which the route sewed
    /// after `before`'s, from `a` to `b`: back and forth from it, each lengthened as the rows are, with
    /// straight stitches from each to the next, and straight back to `b`. Where no row lies past it, the
    /// part ends there, and nothing is sewn.
    fn fill_gap(
        &self,
        run: &mut Vec<Point>,
        [last, before]: [usize; 2],
        [a, b]: [Point; 2],
        rng: &mut SplitMix64,
        meter: &mut Meter,
    ) -> Result<(), Exhausted> {
        let extra = gap_rows(self.segments, self.rows, last, before, self.gap_rows, meter)?;
        let Some(&segment) = self.segments.get(last).filter(|_| !extra.is_empty()) else { return Ok(()) };
        // The first row after the last runs back.
        let forward = forward(a, segment);
        for (k, row) in extra.into_iter().enumerate() {
            let [from, to] = self.lengthened(row, (k % 2 == 1) == forward);
            self.straight(run, from, meter)?;
            extend(run, row_points(from, to, self.stitching, rng, meter)?);
        }
        self.straight(run, b, meter)
    }

    /// Running stitches straight on from the end of `run` to `to`.
    fn straight(&self, run: &mut Vec<Point>, to: Point, meter: &mut Meter) -> Result<(), Exhausted> {
        let way: Vec<Point> = run.last().copied().into_iter().chain([to]).collect();
        extend(run, along_line(&way, self.travel.length, self.travel.tolerance, self.travel.min_stitch, meter)?);
        Ok(())
    }

    /// Where Ink/Stitch's fill starts with the lines: at the node of the lines nearest the needle, from
    /// which it travels to the route's first node, or on to the end of the route's first stretch of travel,
    /// which this way then takes the place of. A needle on the rings or outside the part is nearest the
    /// route's first node, and needs no such way. The way, smoothed and kept inside the part, and whether
    /// it takes the place of the first stretch; `None` without the lines or a needle, or where no line
    /// leads from the nearest node.
    fn lead_in(&self, meter: &mut Meter) -> Result<Option<(Vec<Point>, bool)>, Exhausted> {
        let (Some(lines), Some(needle), Some(&first)) = (&self.lines, self.start, self.route.steps.first()) else { return Ok(None) };
        let (from, to, replaces) = match first {
            Step::Travel { from, to, .. } => (from, to, true),
            Step::Row { from, .. } => (from, from, false),
        };
        let Some(nearest) = lines.nearest(needle, meter)?.filter(|&node| node != from) else { return Ok(None) };
        let Some(way) = lines.way(nearest, to, meter)? else { return Ok(None) };
        Ok(Some((self.smoothed(&way, meter)?, replaces)))
    }
}

/// Whether a row sewn from `from` runs from its segment's start to its end: whether `from` lies no farther
/// from the start than from the end. A row starts at an end of its segment, or past it by the pull
/// compensation, which is never below 0 for a fill.
fn forward(from: Point, (start, end): (Point, Point)) -> bool {
    from.distance(start) <= from.distance(end)
}

/// `way` with its first point moved to `first` and its last to `last`, where given: travel from where a
/// row ends, lengthened, to where the next starts.
fn moved(way: &mut [Point], first: Option<Point>, last: Option<Point>) {
    if let (Some(p), Some(q)) = (first, way.first_mut()) {
        *q = p;
    }
    if let (Some(p), Some(q)) = (last, way.last_mut()) {
        *q = p;
    }
}

/// Adds `points` to `run`, each unless it is where the run already is.
fn extend(run: &mut Vec<Point>, points: impl IntoIterator<Item = Point>) {
    for p in points {
        if run.last() != Some(&p) {
            run.push(p);
        }
    }
}

#[cfg(test)]
mod tests;
