//! A tatami fill sewn: its parts one at a time, in each part the layers of its underlay and then its top
//! rows, each layer's rows in the order `route` gives, with running stitches along the needle's way
//! between them (`REQ-FILL-TAT-001`, `REQ-FILL-TAT-008`, `REQ-FILL-TAT-011`).
//!
//! **Parts.** Ink/Stitch sews a fill's parts nearest the needle first, a part the needle is in being 0
//! away, and with no needle, from the part farthest left. Each part ends at its point nearest the next
//! part, and the last at its point nearest the fill's next stitch, found on the fill's parts as Ink/Stitch
//! finds it (the `generators::next` module). A part is a group of its own, which assembly joins to
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
//! ended (`DEV-FILL-008`). Ink/Stitch sews each piece's top rows as a group of its own, so with
//! `force_lock_stitches` each is a run of its own here, which assembly ties off and jumps to, as Ink/Stitch's
//! stitch plan does. Where shrinking leaves nothing, the part itself is sewn, as Ink/Stitch sews it, and
//! `SC-W0310` or `SC-W0309` says so.
//!
//! **Pull compensation.** The top rows reach past their segments' ends by `pull_compensation_mm` plus
//! `pull_compensation_percent` of the segment's length, at its start and at its end (`REQ-FILL-TAT-006`),
//! moved as a satin's stitches are (`generators::pull`). Where 2 segments of a row meet end to end, the row
//! going on in the part, both end there as they are, so the row is sewn once through the point. The part,
//! its route and the needle's ways stay as they are, and each way starts where the row before it ends and
//! ends where the next row starts, both lengthened. Ink/Stitch rebuilds the part round the lengthened rows instead, which can fill its holes and
//! drop a piece of it (`DEV-FILL-009`). Underlay is not compensated, as in Ink/Stitch.
//!
//! **Gap-fill rows.** Where the route leaves a stretch of more than 3 rows for another, as Ink/Stitch's gap
//! fill finds (`route::Step::Travel`), the top rows sew `gap_fill_rows` more past its last row, back and
//! forth, and come back to where it ended (the `gaps` module, `REQ-FILL-TAT-007`). The hop from each row to
//! the next, and the way back, run between the rows' ends as they lie in the part, kept inside it as travel
//! is (`clamp`). Underlay has none, as in Ink/Stitch.
//!
//! **The shortest stitch.** Rows the row spacing apart start and end nearer each other than the shortest
//! stitch a machine sews well, and a row's first grid point can lie just past its start. A needle point
//! nearer the one before it than the shortest stitch is left out, so the stitch runs on to the next, as
//! Ink/Stitch's stitch plan leaves out points nearer than its own shortest stitch. A run's last point stays,
//! and the points before it that it is too near go instead. It is finalize's rule, the same function
//! (`crate::thin`), applied here so that the generator hands finalize stitches it has nothing to thin.

pub(crate) mod lines;

use std::collections::BTreeSet;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::{MM_PER_SVG_PX, at_least};
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};

use super::clamp::clamp;
use super::gaps::gap_rows;
use super::graph::NodeId;
use super::rings::Rings;
use super::route::{Route, Rows, Step, route};
use super::rows::{Segment, rows};
use super::stitches::{Stitching, row_points};
use super::travel::{Network, between};
use super::underpath::Underpath;
use crate::generators::pull::offset;
use crate::generators::running::along_line;
use crate::generators::smoothing::smooth;
use crate::generators::{Approach, Own, Stitched, covers, settled};
use crate::normalize::near::{key, nearest_pair, nearest_to_point};
use crate::normalize::offset::buffer;
use crate::normalize::region::{Polygon, Region, Unjoined};
use crate::thin::thin;

/// How much the needle's way under the rows is smoothed, in millimetres: Ink/Stitch smooths travel by 2 pixels.
const TRAVEL_SMOOTHNESS: f64 = 2.0 * MM_PER_SVG_PX;

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
    /// Whether locks part the element's groups (`force_lock_stitches`): then each piece of top rows after
    /// a part's first is a run of its own, however near, which assembly ties off and jumps to.
    pub force_locks: bool,
}

/// The stitches of the tatami `fill` over `region`: in each part the layers of its underlay in order, each
/// back to where it started, then its top rows, from the needle where the elements before left it to near
/// the `next` element. Random lengths draw from `rng`. Each part is a run, unless a layer with no row, or a
/// piece a part is cut into, starts farther from the needle than the jump length. Work is charged to
/// `meter` for every ring point, row, node, edge, step and stitch, and a stitch for every needle point.
pub fn tatami_fill(
    region: &Region,
    fill: &Fill,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, fill, needle, next, rng, meter))
}

/// [`tatami_fill`], before its stitches are settled.
fn sewn(
    region: &Region,
    fill: &Fill,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let (mut tops, mut outlined) = (0, 0);
    let mut top = |piece: &Polygon,
                   _: &Polygon,
                   needle: Option<Point>,
                   here: Option<Point>,
                   rng: &mut SplitMix64,
                   meter: &mut Meter,
                   warnings: &mut Vec<Diagnostic>| {
        let part = Part { polygon: piece, rings: Rings::new(piece, meter)? };
        let (runs, rowless) = part.sew(&fill.top, needle, here, rng, meter, warnings)?;
        (tops, outlined) = (tops + 1, outlined + usize::from(rowless));
        Ok(runs)
    };
    let parts = Parts {
        underlay: &fill.underlay,
        inset: fill.inset,
        expand: fill.expand,
        force_locks: fill.force_locks,
        top: fill.top.travel,
        ends_near: true,
    };
    let sewn = parts_sewn(region, &parts, needle, next, rng, meter, &mut top)?;
    let mut warnings = sewn.warnings;
    warnings.extend(too_thin(outlined, tops));
    warnings.extend(sewn.tally.said());
    Ok(Stitched { runs: sewn.runs, warnings })
}

/// What a fill sews over each part of its region but its own top: its underlay's layers over the part shrunk
/// by `inset`, then its top over the part grown by `expand`, for a tatami fill and a contour fill alike.
pub(crate) struct Parts<'f> {
    /// The underlay's layers, in the order they are sewn.
    pub(crate) underlay: &'f [Layer],
    /// How far each part is shrunk for its underlay, in millimetres; grown where it is negative.
    pub(crate) inset: f64,
    /// How far each part is grown for its top, in millimetres; shrunk where it is negative.
    pub(crate) expand: f64,
    /// Whether locks part the element's groups: then each piece of a part's top after its first is a run of
    /// its own.
    pub(crate) force_locks: bool,
    /// The top's travel: the jump length past which a piece starting away from the needle is a run of its
    /// own, and the shortest stitch the runs are thinned to.
    pub(crate) top: Travel,
    /// Whether the top of each part ends near the next part, and the last near the next element: a tatami
    /// fill's does, a contour fill's ends where its rings end.
    pub(crate) ends_near: bool,
}

/// A fill's top over one piece of a part (the first polygon), the part as drawn being the second: the runs
/// it sews from the needle (the first point), to end near the second where it has one, drawing from the
/// generator and charging the meter, and what it says.
pub(crate) type Top<'t> = dyn FnMut(&Polygon, &Polygon, Option<Point>, Option<Point>, &mut SplitMix64, &mut Meter, &mut Vec<Diagnostic>) -> Result<Vec<Vec<Point>>, Exhausted>
    + 't;

/// What [`parts_sewn`] sewed: the runs, what was said, and what was counted for the diagnostics that close a
/// fill's.
pub(crate) struct PartsSewn {
    pub(crate) runs: Vec<Vec<Point>>,
    pub(crate) warnings: Vec<Diagnostic>,
    pub(crate) tally: Tally,
}

/// The runs of a fill over `region` as `parts` says, each piece's top sewn by `top`: the parts nearest the
/// needle first, in each the layers of its underlay in order, each back to where it started, then its top
/// over each piece of the part grown, from where the last ended. Runs nearer each other than the jump length
/// go on as one, and each part's runs are thinned to the shortest stitch.
pub(crate) fn parts_sewn(
    region: &Region,
    parts: &Parts,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
    top: &mut Top,
) -> Result<PartsSewn, Exhausted> {
    let ordered = in_order(region, needle, meter)?;
    let end = if parts.ends_near { next.map(|next| next.next_stitch(Own::Area(&region.parts), meter)).transpose()?.flatten() } else { None };
    let mut needle = needle;
    let (mut runs, mut warnings) = (Vec::new(), Vec::new());
    let mut tally = Tally { parts: ordered.len(), ..Tally::default() };
    for (i, polygon) in ordered.iter().enumerate() {
        let here = match (parts.ends_near, ordered.get(i + 1)) {
            (false, _) => None,
            (true, Some(after)) => nearest_pair(&rings_of(polygon), &rings_of(after), meter)?.map(|(on, _)| on),
            (true, None) => end,
        };
        let mut sewn = Vec::new();
        if !parts.underlay.is_empty() {
            let (pieces, gone) = pieces(polygon, -parts.inset, &mut warnings, meter)?;
            tally.inset_gone += usize::from(gone);
            for piece in &pieces {
                let part = Part { polygon: piece, rings: Rings::new(piece, meter)? };
                for layer in parts.underlay {
                    let (layer_runs, rowless) = part.sew(layer, needle, None, rng, meter, &mut warnings)?;
                    tally.bare += usize::from(rowless);
                    needle = layer_runs.iter().rev().find_map(|run| run.last().copied()).or(needle);
                    append(&mut sewn, layer_runs, layer.travel.jump);
                }
            }
        }
        let (pieces, gone) = pieces(polygon, parts.expand, &mut warnings, meter)?;
        tally.expand_gone += usize::from(gone);
        for (k, piece) in pieces.iter().enumerate() {
            let top_runs = top(piece, polygon, needle, here, rng, meter, &mut warnings)?;
            needle = top_runs.iter().rev().find_map(|run| run.last().copied()).or(needle);
            if k > 0 && parts.force_locks {
                sewn.extend(top_runs);
            } else {
                append(&mut sewn, top_runs, parts.top.jump);
            }
        }
        let sewn: Vec<Vec<Point>> = sewn
            .into_iter()
            .map(|run| thin(run, |a, b| !at_least(a.distance(*b), parts.top.min_stitch), |_| false, |_, _| ()))
            .filter(|run| !run.is_empty())
            .collect();
        needle = sewn.iter().rev().find_map(|run| run.last().copied()).or(needle);
        runs.extend(sewn);
    }
    Ok(PartsSewn { runs, warnings, tally })
}

/// The segments of the top rows of the tatami `fill` over `region`, each part grown by its `expand` as
/// [`tatami_fill`] grows it, in order across each piece's rows: where the top stitches lie, without the
/// stitches, the underlay or the needle's travel. A preview too costly to sew shows them. Work is charged
/// to `meter` for every ring point, row and edge a row is cut against.
pub fn top_rows(region: &Region, fill: &Fill, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let mut segments = Vec::new();
    for polygon in &region.parts {
        for piece in &pieces(polygon, fill.expand, &mut Vec::new(), meter)?.0 {
            segments.extend(rows(piece, &fill.top.stitching.grid, meter)?.into_iter().flat_map(|row| row.segments));
        }
    }
    Ok(segments)
}

/// What sewing a fill's parts counted, for the diagnostics that close its own.
#[derive(Default)]
pub(crate) struct Tally {
    /// The parts sewn.
    parts: usize,
    /// The layers of underlay that found no row.
    bare: usize,
    /// The parts that `expand` and that `inset` left nothing of.
    expand_gone: usize,
    inset_gone: usize,
}

impl Tally {
    /// What the counts say: `SC-I0308` for the layers of underlay with no row, then `SC-W0309` and `SC-W0310`
    /// for the parts `expand_mm` and `fill_underlay_inset_mm` shrank away.
    pub(crate) fn said(&self) -> impl Iterator<Item = Diagnostic> {
        let parts = self.parts;
        underlay_too_thin(self.bare)
            .into_iter()
            .chain(shrunk_away(Code::FillExpandShrinksAway, "expand_mm", "its top rows are", self.expand_gone, parts))
            .chain(shrunk_away(Code::FillUnderlayInsetShrinksAway, "fill_underlay_inset_mm", "its underlay is", self.inset_gone, parts))
    }
}

/// The pieces `polygon` grown by `distance` falls into, or `polygon` itself where that leaves nothing, as
/// Ink/Stitch sews it; and whether it left nothing. Where the grown area's edges do not join into rings, a
/// bug that `warnings` gets, `polygon` itself.
pub(crate) fn pieces(polygon: &Polygon, distance: f64, warnings: &mut Vec<Diagnostic>, meter: &mut Meter) -> Result<(Vec<Polygon>, bool), Exhausted> {
    let grown = buffer(polygon, distance, meter)?;
    let joined = grown.is_ok();
    warnings.extend(grown.as_ref().err().copied().map(Unjoined::grown));
    let pieces = grown.unwrap_or_default();
    Ok(if pieces.is_empty() { (vec![polygon.clone()], joined) } else { (pieces, false) })
}

/// `code` for the `gone` parts of `parts` that `key` shrank away entirely, whose `what` sewn over all of
/// the part.
pub(crate) fn shrunk_away(code: Code, key: &str, what: &str, gone: usize, parts: usize) -> Option<Diagnostic> {
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
        let joints = joints(&segments, meter)?;
        let across = Rows::Straight(&segments);
        let Some(route) = route(rings, across, start, end, meter)? else {
            return Ok((vec![along_line(&part.outline, travel.length, travel.tolerance, travel.min_stitch, meter)?], true));
        };
        let ways = Ways::new(part, rings, &route, across, stitching.grid.angle, *travel, start, meter)?;
        let mut sewing = Sewing { ways, stitching, segments: &segments, rows: &rows, pull: *pull, joints, gap_rows: *gap_rows };
        Ok((sewing.sew(rng, meter, warnings)?, false))
    }
}

/// A part's rings as polylines: the outline, then the holes.
fn rings_of(part: &Polygon) -> Vec<&[Point]> {
    part.rings().collect()
}

/// `region`'s parts in the order they are sewn: nearest `needle` first, or with no needle, by how far left
/// they reach. Ink/Stitch sorts them so with a stable sort, over the order its geometry library gives
/// them in, which in every case tried was by their point of least x, and of least y among the points level
/// with it. Parts that tie go in that order here too (`REQ-FILL-TAT-008`). A cross stitch fill takes its
/// parts in this order too (`generators::cross`).
pub(crate) fn in_order<'a>(region: &'a Region, needle: Option<Point>, meter: &mut Meter) -> Result<Vec<&'a Polygon>, Exhausted> {
    let mut keyed = Vec::with_capacity(region.parts.len());
    for part in &region.parts {
        let (x, y) = leftmost(part);
        let key = match needle {
            Some(p) if covers(part, p, meter)? => 0.0,
            Some(p) => nearest_to_point(&rings_of(part), p, meter)?.map_or(f64::INFINITY, |(_, d)| d),
            None => x,
        };
        keyed.push(((key, x, y), part));
    }
    keyed.sort_by(|(a, ..), (b, ..)| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)).then(a.2.total_cmp(&b.2)));
    Ok(keyed.into_iter().map(|(_, part)| part).collect())
}

/// The point of least x of `part`, and of least y among the points level with it.
fn leftmost(part: &Polygon) -> (f64, f64) {
    part.outline
        .iter()
        .map(|q| (q.x(), q.y()))
        .fold((f64::INFINITY, f64::INFINITY), |least, q| if q.0.total_cmp(&least.0).then(q.1.total_cmp(&least.1)).is_lt() { q } else { least })
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

/// The points where 2 of the row `segments` meet end to end, as keys: where a row meets the outline without
/// leaving the part, touching it from inside or running along it, it is split there (`REQ-FILL-TAT-010`) and
/// goes on. One unit of `meter` for each segment.
fn joints(segments: &[(Point, Point)], meter: &mut Meter) -> Result<BTreeSet<(u64, u64)>, Exhausted> {
    let (mut ends, mut joints) = (BTreeSet::new(), BTreeSet::new());
    for &(start, end) in segments {
        meter.charge(1)?;
        for p in [start, end] {
            if !ends.insert(key(p)) {
                joints.insert(key(p));
            }
        }
    }
    Ok(joints)
}

/// Each row the route sews, in the order sewn: its number, and the node it is sewn from and the node it is
/// sewn to.
fn row_steps(route: &Route) -> Vec<(usize, [NodeId; 2])> {
    route.steps.iter().filter_map(|step| if let Step::Row { segment, from, to } = *step { Some((segment, [from, to])) } else { None }).collect()
}

/// The needle's ways between a part's rows as its route sews them: under the rows not sewn yet, or along the
/// rings and rows, a tatami fill's straight rows or a circular fill's lines alike.
pub(crate) struct Ways<'a> {
    route: &'a Route,
    rings: &'a Rings<'a>,
    travel: Travel,
    /// The lines under the rows, with `underpath` where they cross the part.
    lines: Option<Underpath>,
    /// The ways along the part's rings and rows.
    network: Network,
    /// Where the needle was before the part.
    start: Option<Point>,
}

impl<'a> Ways<'a> {
    /// The ways `route` takes over `part`, whose rings are `rings`, between the rows `across`: the lines under
    /// them, at `angle`, when `travel` runs under the rows, and the network of rings and rows. The needle
    /// was at `start` before the part. Costs as [`Underpath::new`] and [`Rows::network`] do.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        part: &Polygon,
        rings: &'a Rings<'a>,
        route: &'a Route,
        across: Rows<'_>,
        angle: f64,
        travel: Travel,
        start: Option<Point>,
        meter: &mut Meter,
    ) -> Result<Ways<'a>, Exhausted> {
        let lines = if travel.underpath { Underpath::new(part, rings, &route.nodes, across, angle, meter)? } else { None };
        let network = across.network(&route.nodes, rings, &row_steps(route), meter)?;
        Ok(Ways { route, rings, travel, lines, network, start })
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
        clamp(&smooth(way, TRAVEL_SMOOTHNESS, self.travel.min_stitch, meter)?, self.rings, meter)
    }

    /// Row `segment` is sewn: the lines across it close.
    fn sewn(&mut self, segment: usize) {
        if let Some(lines) = &mut self.lines {
            lines.sew(segment);
        }
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

    /// The running stitches along `way`, in the travel's length and within its tolerance.
    fn stitched(&self, way: &[Point], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        along_line(way, self.travel.length, self.travel.tolerance, self.travel.min_stitch, meter)
    }
}

/// A part as it is sewn.
struct Sewing<'a> {
    ways: Ways<'a>,
    stitching: &'a Stitching,
    /// The part's row segments, each from its start to its end, and the row of each, counted across.
    segments: &'a [(Point, Point)],
    rows: &'a [usize],
    /// How far the rows reach past their ends.
    pull: Pull,
    /// The points where 2 segments of a row meet ([`joints`]), as keys.
    joints: BTreeSet<(u64, u64)>,
    /// How many rows to sew again where the route leaves a stretch of rows.
    gap_rows: usize,
}

/// The stretch of rows the route is sewing, for gap-fill rows: how many of its rows came after travel, as
/// Ink/Stitch counts them, whether the needle has travelled since the last row, the last row's segment with
/// its row of the grid, and the last segment sewn before it in another row. That segment says which way the
/// stretch goes. Ink/Stitch takes the way from the row before the last stretch of travel, and a row sewn on
/// through a joint, 2 segments of one row, has none between its segments.
struct Stretch {
    rows: usize,
    after_travel: bool,
    last: Option<(usize, Option<usize>)>,
    before: Option<usize>,
}

impl Stretch {
    /// The stretch a route starts with. Its first row counts as reached by travel: Ink/Stitch's route always
    /// starts with travel, from its node nearest the needle, of no length where that is the first row's start.
    const START: Self = Self { rows: 0, after_travel: true, last: None, before: None };

    /// Row segment `segment`, of the grid's row `row`, is sewn.
    fn row(&mut self, segment: usize, row: Option<usize>) {
        self.rows += usize::from(self.after_travel);
        self.after_travel = false;
        if let Some((last, _)) = self.last.filter(|&(_, last_row)| last_row != row) {
            self.before = Some(last);
        }
        self.last = Some((segment, row));
    }

    /// The needle travels, and the travel `leaves` the stretch where Ink/Stitch's gap fill finds more than
    /// one edge of travel in a row ([`super::route::Step::Travel`]): the last row's segment and the last one
    /// before it in another row, when the stretch had more than 3 rows.
    fn travel(&mut self, leaves: bool) -> Option<[usize; 2]> {
        self.after_travel = true;
        if !leaves {
            return None;
        }
        let rows = std::mem::take(&mut self.rows);
        match (self.last, self.before) {
            (Some((last, _)), Some(before)) if rows > 3 => Some([last, before]),
            _ => None,
        }
    }
}

impl Sewing<'_> {
    /// The runs the route sews: one, unless the needle has to jump, which is a bug that a diagnostic added
    /// to `warnings` says, once for the part.
    fn sew(&mut self, rng: &mut SplitMix64, meter: &mut Meter, warnings: &mut Vec<Diagnostic>) -> Result<Vec<Vec<Point>>, Exhausted> {
        let route = self.ways.route;
        let point = |node: usize| route.nodes.get(node).map(|n| n.point);
        // Each step's row as it is sewn, between its segment's lengthened ends.
        let sewn: Vec<Option<[Point; 2]>> = route.steps.iter().map(|step| self.row(step)).collect();
        let start_of = |i: usize| sewn.get(i).copied().flatten().map(|[a, _]| a);
        let row_before = |i: usize| i.checked_sub(1).and_then(|i| sewn.get(i).copied().flatten());
        let mut runs = Vec::new();
        let mut run: Vec<Point> = Vec::new();
        let mut jumped = false;
        let lead = self.ways.lead_in(meter)?;
        let skipped = usize::from(lead.as_ref().is_some_and(|(_, replaces)| *replaces));
        let mut stretch = Stretch::START;
        match lead {
            Some((mut way, _)) => {
                moved(&mut way, None, start_of(skipped));
                run.extend(self.ways.stitched(&way, meter)?);
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
                    self.ways.sewn(segment);
                    stretch.row(segment, self.rows.get(segment).copied());
                }
                Step::Travel { from, to, beside, leaves } => {
                    if let (Some(rows), Some(ended)) = (stretch.travel(leaves), row_before(i)) {
                        self.fill_gap(&mut run, rows, ended, rng, meter)?;
                    }
                    let next = start_of(i + 1);
                    match self.ways.way(from, to, meter)? {
                        Some(mut way) => {
                            moved(&mut way, row_before(i).map(|[_, b]| b), next);
                            let stitches = self.ways.stitched(&way, meter)?;
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

    /// The row `step` sews, between its segment's ends lengthened by the pull compensation, from the end the
    /// route enters it by; `None` for travel.
    fn row(&self, step: &Step) -> Option<[Point; 2]> {
        let Step::Row { segment, from, .. } = *step else { return None };
        let (&segment, entry) = (self.segments.get(segment)?, self.ways.route.nodes.get(from)?.point);
        Some(self.lengthened(segment, forward(entry, segment)))
    }

    /// `segment` lengthened by the pull compensation at its ends, from its start where `forward` and from its
    /// end otherwise. An end where 2 segments of the row meet, the row going on in the part, is not
    /// lengthened, so the row is sewn once there.
    fn lengthened(&self, (start, end): (Point, Point), forward: bool) -> [Point; 2] {
        let Pull { mm: [mm_start, mm_end], share: [share_start, share_end] } = self.pull;
        let at = |p: Point, mm: f64, share: f64| if self.joints.contains(&key(p)) { (0.0, 0.0) } else { (mm, share) };
        let ((mm_start, share_start), (mm_end, share_end)) = (at(start, mm_start, share_start), at(end, mm_end, share_end));
        let [start, end] = offset([start, end], [mm_start, mm_end], [share_start, share_end]);
        if forward { [start, end] } else { [end, start] }
    }

    /// Sews onto `run` the rows that fill the gap after the row of segment `last`, which the route sewed
    /// after `before`'s, the last in another row, from `a` to `b`: back and forth from it, each lengthened as
    /// the rows are, with a hop from each to the next, and one back to `b`. Where no row lies past it, the
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
        // The first row after the last runs back. Where the needle is, before the rows are lengthened.
        let forward = forward(a, segment);
        let ended = if forward { segment.1 } else { segment.0 };
        let mut at = ended;
        for (k, (start, end)) in extra.into_iter().enumerate() {
            let ahead = (k % 2 == 1) == forward;
            let [from, to] = self.lengthened((start, end), ahead);
            let (enters, leaves) = if ahead { (start, end) } else { (end, start) };
            self.hop(run, [at, enters], from, meter)?;
            extend(run, row_points(from, to, self.stitching, rng, meter)?);
            at = leaves;
        }
        self.hop(run, [at, ended], b, meter)
    }

    /// Running stitches from the end of `run` to `to`, where the next row starts, lengthened: along the way
    /// between the 2 rows' `ends` as they lie in the part, kept inside it as travel is.
    fn hop(&self, run: &mut Vec<Point>, ends: [Point; 2], to: Point, meter: &mut Meter) -> Result<(), Exhausted> {
        let mut way = clamp(&ends, self.ways.rings, meter)?;
        moved(&mut way, run.last().copied(), Some(to));
        extend(run, self.ways.stitched(&way, meter)?);
        Ok(())
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
