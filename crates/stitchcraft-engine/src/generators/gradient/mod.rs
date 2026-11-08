//! Linear gradient fill (milestone M10.12): rows square to a linear gradient, each in one of its colours, the
//! 2 colours of each stretch between stops sharing its rows in a fixed pattern, as Ink/Stitch sews its linear
//! gradient fill (`docs/src/design/algorithms/gradient.md`, `REQ-FILL-GRD-001` to `REQ-FILL-GRD-008`, read at
//! `d59c9ab`).
//!
//! **Why rows of whole colours.** Thread can't be mixed, so a gradient fill blends 2 colours by how many rows
//! each takes: near a stop most rows take its colour. Seen from a step away, the colours blend.
//!
//! **Steps.** A gradient fill sews its parts as a tatami fill sews them, nearest the needle first, each with
//! the tatami fill's underlay first, in the gradient's first colour, and grown by `expand_mm` for its rows.
//! Over each piece of a part:
//!
//! 1. **The rows** (module `rows`) lie square to the gradient, a longest stitch past the piece either side,
//!    and each stop lies at a row. A fill without a gradient fades its own colour out along `angle`.
//! 2. **The colours** (module `blend`): the first stop takes the rows up to its own, each next stop blends
//!    with the colour before it in the fixed pattern, and the rows after the last blend take the last colour.
//!    A transparent stop's rows are left out.
//! 3. **Each colour** in its thread, in the order the stops first give them: its rows, cut by the piece, are
//!    routed as a tatami fill's rows are, from where the needle was when the piece's colours began towards
//!    the end the next part or element offers, the needle travelling along the piece's outline between them
//!    (`tatami::sew::lines`). The travel before its first row and after its last is left out, as Ink/Stitch
//!    leaves it out ([`stripped`]).
//!
//! **Several parts.** Each part sews its underlay and its colours. Then, as Ink/Stitch sorts them, the groups
//! are ordered by where their colour first came, so that each thread sews in one stretch.
//!
//! **What shows them.** Each colour of each piece is a colour block of its thread, trimmed after when the
//! element's `trim_after` is on, and the underlay ends none ([`crate::generators::Colour`], `REQ-ASM-006`).
//! A colour of the element's own thread sews in it, named or not, and the others in threads of their colours
//! alone, so that one colour is one thread.

mod blend;
mod rows;

use std::collections::BTreeSet;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point};
use stitchcraft_plan::{Rgb, Thread};

use self::blend::{Placed, share};
pub(crate) use self::rows::Stitching;
use self::rows::{Rows, Way};
use crate::design::Gradient;
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::lines::{self, Marked, lines_sewn};
use crate::generators::tatami::sew::{Layer, Parts, PartsSewn, Tally, Travel, in_order, pieces, thinned, underlay};
use crate::generators::{Approach, Colour, Own, Stitched, made, settled};
use crate::normalize::near::nearest_pair;
use crate::normalize::region::cut::inside;
use crate::normalize::region::{Polygon, Region};

/// A linear gradient fill as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub struct GradientFill {
    /// What colours its rows take.
    pub(crate) colouring: Colouring,
    /// How far apart the rows are, at most, in millimetres.
    pub(crate) spacing: f64,
    /// How the rows are stitched: the longest stitch, staggers and random lengths.
    pub(crate) stitching: Stitching,
    /// Whether each row is sewn without its last needle point.
    pub(crate) skip_last: bool,
    /// Whether the last 2 colours keep their travel to the end (`stop_at_ending_point`).
    pub(crate) stop_at_end: bool,
    /// The underlay's layers, the tatami fill's, over each part shrunk by `inset`, and how far each part is
    /// grown for its rows, in millimetres.
    pub(crate) underlay: Vec<Layer>,
    pub(crate) inset: f64,
    pub(crate) expand: f64,
    /// The needle's travel: its running stitch, the shortest stitch and the jump length.
    pub(crate) travel: Travel,
    /// The element's own thread's colour.
    pub(crate) own: Rgb,
}

/// What colours a gradient fill's rows take.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Colouring {
    /// Those of the fill's linear gradient.
    Gradient(Gradient),
    /// Without a gradient, the element's own colour fading out along the way at this angle, in degrees
    /// clockwise as the canvas shows it.
    Fade(f64),
}

impl GradientFill {
    /// The colour its underlay sews in: its gradient's first stop's, transparent or not, or its own.
    fn underlay_colour(&self) -> Rgb {
        match &self.colouring {
            Colouring::Gradient(gradient) => gradient.stops.first().map_or(self.own, |stop| stop.colour),
            Colouring::Fade(_) => self.own,
        }
    }

    /// The way its rows lie square to.
    fn way(&self) -> Way {
        match &self.colouring {
            Colouring::Gradient(gradient) => Way::of_gradient(gradient.start, gradient.end),
            Colouring::Fade(angle) => Way::at(*angle),
        }
    }
}

/// The angle, in degrees counter-clockwise as the canvas shows it, at which rows run along `gradient` from its
/// start to its end, as its underlay's rows run unless `fill_underlay_angle` sets them: square to its top rows.
/// A gradient of no length points along x.
pub(crate) fn underlay_angle(gradient: &Gradient) -> f64 {
    let (x, y) = Way::of_gradient(gradient.start, gradient.end).along;
    stitchcraft_core::math::to_degrees(stitchcraft_core::math::atan2(-y, x))
}

/// What a group of a gradient fill's stitches is.
#[derive(Clone, Debug, PartialEq)]
struct Group {
    colour: Rgb,
    underlay: bool,
    run: Vec<Point>,
}

/// The stitches of the gradient fill `fill` over `region`, from the needle where the elements before left it
/// to near the `next` element, as the module's docs say. The underlay draws from `rng`, and random lengths
/// from its streams. Work is charged to `meter` for every row, every row a blend lays, every cut and stitch,
/// and a stitch for every needle point.
pub fn gradient_fill(
    region: &Region,
    fill: &GradientFill,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, fill, needle, next, rng, meter))
}

/// [`gradient_fill`], before its stitches are settled.
fn sewn(
    region: &Region,
    fill: &GradientFill,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let ordered = in_order(region, needle, meter)?;
    let end = next.map(|next| next.next_stitch(Own::Area(&region.parts), meter)).transpose()?.flatten();
    let parts = Parts { underlay: &fill.underlay, inset: fill.inset, expand: fill.expand, force_locks: false, top: fill.travel, ends_near: true };
    let mut said = PartsSewn { runs: Vec::new(), warnings: Vec::new(), tally: Tally { parts: ordered.len(), ..Tally::default() } };
    let (mut needle, mut groups, mut passed) = (needle, Vec::new(), BTreeSet::new());
    let below = fill.underlay_colour();
    for (i, polygon) in ordered.iter().enumerate() {
        let here = match ordered.get(i + 1) {
            Some(after) => nearest_pair(&polygon.rings().collect::<Vec<_>>(), &after.rings().collect::<Vec<_>>(), meter)?.map(|(on, _)| on),
            None => end,
        };
        let laid = thinned(underlay(&parts, polygon, &mut needle, None, &mut said, rng, meter)?, fill.travel.min_stitch);
        groups.extend(laid.into_iter().map(|run| Group { colour: below, underlay: true, run }));
        needle = groups.last().and_then(|group| group.run.last().copied()).or(needle);
        let (pieces, gone) = pieces(polygon, fill.expand, &mut said.warnings, meter)?;
        said.tally.expand_gone += usize::from(gone);
        for piece in &pieces {
            let top = top(piece, fill, (needle, here), (&*rng, &mut passed), meter, &mut said.warnings)?;
            needle = top.last().and_then(|group| group.run.last().copied()).or(needle);
            groups.extend(top);
        }
    }
    if ordered.len() > 1 {
        groups = sorted(groups);
    }
    let groups = joined(groups, fill.travel.min_stitch);
    // A colour of the element's own sews in the element's own thread, named or not, and the others in threads
    // of their colours alone: a named and an unnamed thread of one colour would be 2 threads, which a machine
    // file writes as 2 entries of its palette.
    let colours = groups
        .iter()
        .map(|group| Colour { thread: (group.colour != fill.own).then(|| Thread::new(group.colour)), ends_block: !group.underlay })
        .collect();
    let mut warnings = said.warnings;
    warnings.extend(passed_over(&passed));
    warnings.extend(said.tally.said());
    Ok(Stitched { runs: groups.into_iter().map(|group| group.run).collect(), colours, warnings })
}

/// `groups`, each run's first needle points left out where they lie nearer than `min_stitch` to where the run
/// before ended and the 2 sew in one thread, as a colour's first rows run on from the underlay in the first
/// stop's colour: finalize would leave such a point out (`DEV-FIN-001`). A run keeps its last point.
fn joined(groups: Vec<Group>, min_stitch: f64) -> Vec<Group> {
    let mut out: Vec<Group> = Vec::with_capacity(groups.len());
    for mut group in groups {
        let before = out.last().filter(|before| before.colour == group.colour).and_then(|before| before.run.last().copied());
        if let Some(end) = before {
            let near = group.run.iter().take_while(|q| !at_least(q.distance(end), min_stitch)).count();
            group.run.drain(..near.min(group.run.len().saturating_sub(1)));
        }
        out.push(group);
    }
    out
}

/// `SC-W0330` for the stops of a gradient passed over, by their numbers from 1, where a row they leave out
/// crosses the fill.
fn passed_over(stops: &BTreeSet<usize>) -> Option<Diagnostic> {
    let numbers: Vec<String> = stops.iter().map(|stop| (stop + 1).to_string()).collect();
    let (which, lies) = match numbers.as_slice() {
        [] => return None,
        [one] => (format!("Stop {one}"), "lies"),
        [rest @ .., last] => (format!("Stops {} and {last}", rest.join(", ")), "each lie"),
    };
    let message = format!(
        "{which} of this fill's gradient {lies} within 2 rows of the blend before, so no blend starts there and the row after that blend is \
         left out, as Ink/Stitch sews it."
    );
    Some(
        Diagnostic::new(Code::FillGradientStopPassedOver, message)
            .with_fix(stitchcraft_core::Fix::Hint("Move the stops at least 3 row spacings apart.".to_string())),
    )
}

/// The groups the gradient's colours sew over `piece`, each routed from `start` towards `end`, as the
/// module's docs say. A stop passed over where its row left out crosses the piece goes to `passed`, and a
/// jump the needle needs between rows to `warnings`.
fn top(
    piece: &Polygon,
    fill: &GradientFill,
    (start, end): (Option<Point>, Option<Point>),
    (rng, passed): (&SplitMix64, &mut BTreeSet<usize>),
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<Group>, Exhausted> {
    let coloured = coloured(piece, fill, rng, passed, meter)?;
    let rings = Rings::new(piece, meter)?;
    let given = lines::Given { travel: Travel { underpath: false, ..fill.travel }, angle: 0.0, skip_last: fill.skip_last };
    let count = coloured.len();
    let mut groups = Vec::new();
    for (i, (colour, lines)) in coloured.into_iter().enumerate() {
        let Some(sewn) = lines_sewn(piece, &rings, &lines, &given, start, end, meter, warnings)? else { continue };
        let ends = Ends { lead: i > 0 || fill.underlay.is_empty(), trail: i + 2 < count || !fill.stop_at_end };
        for run in stripped(sewn, ends) {
            groups.extend(thinned(vec![run], fill.travel.min_stitch).into_iter().map(|run| Group { colour, underlay: false, run }));
        }
    }
    Ok(groups)
}

/// Each colour of a piece with its rows inside the piece, in the order the colours are sewn: their stops'
/// order, without the transparent ones and those that take no row.
type Coloured = Vec<(Rgb, Vec<Vec<Point>>)>;

/// The rows of `fill` over `piece` and the colour each takes, each row cut where it leaves the piece, as
/// [`Coloured`] lists them. The stops passed over whose rows left out cross the piece go to `passed`. Work is
/// charged to `meter` for the rows, the blends and the cuts, and a stitch for each needle point.
fn coloured(piece: &Polygon, fill: &GradientFill, rng: &SplitMix64, passed: &mut BTreeSet<usize>, meter: &mut Meter) -> Result<Coloured, Exhausted> {
    let Some(rows) = Rows::lay(piece, fill.way(), fill.spacing, fill.stitching.longest, meter)? else { return Ok(Vec::new()) };
    // Each stop's row and colour, a transparent stop's none.
    let stops: Vec<(i64, Option<Rgb>)> = match &fill.colouring {
        Colouring::Gradient(gradient) => {
            let at = rows.stop_rows(gradient);
            at.into_iter().zip(&gradient.stops).map(|(row, stop)| (row, (stop.opacity > 0.0).then_some(stop.colour))).collect()
        }
        Colouring::Fade(_) => vec![(0, Some(fill.own)), (i64::try_from(rows.last).unwrap_or(i64::MAX).saturating_add(1), None)],
    };
    // The colours in the order the stops first give them, a transparent stop's among them.
    let mut keys: Vec<Option<Rgb>> = Vec::new();
    for (_, key) in &stops {
        if !keys.contains(key) {
            keys.push(*key);
        }
    }
    let placed: Vec<Placed> = stops.iter().map(|(row, key)| Placed { row: *row, colour: keys.iter().position(|k| k == key).unwrap_or(0) }).collect();
    let shares = share(&placed, keys.len(), rows.last, meter)?;
    for &(stop, row) in &shares.passed {
        let crosses = match usize::try_from(row).ok().and_then(|row| rows.ends(row)) {
            Some((a, b)) => !inside(&[a, b], std::slice::from_ref(piece), meter)?.is_empty(),
            None => false,
        };
        if crosses {
            passed.insert(stop);
        }
    }
    let mut coloured = Vec::new();
    for (key, taken) in keys.iter().zip(&shares.rows) {
        let Some(colour) = key else { continue };
        if taken.is_empty() {
            continue;
        }
        let mut lines = Vec::new();
        for &k in taken {
            let points = rows.needle_points(k, &fill.stitching, rng, meter)?;
            for line in inside(&points, std::slice::from_ref(piece), meter)? {
                made(meter, line.len())?;
                lines.push(line);
            }
        }
        coloured.push((*colour, lines));
    }
    Ok(coloured)
}

/// Which ends of a colour's stitches lose their travel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Ends {
    /// The travel before its first needle point on a row.
    lead: bool,
    /// The travel after its last.
    trail: bool,
}

/// The `runs` a colour sews without the travel `ends` says, as Ink/Stitch strips it from the colour's
/// stitches. Before: up to its first needle point on a row, unless that is its last point. After: past its
/// last needle point on a row, or its last point alone where none lies after its first. A run left without
/// points is left out.
fn stripped(mut runs: Vec<Marked>, ends: Ends) -> Vec<Vec<Point>> {
    if ends.lead
        && let Some(first) = runs.first_mut()
    {
        let rowed = first.travel.iter().take(first.travel.len().saturating_sub(1)).position(|&moving| !moving);
        if let Some(at) = rowed {
            first.points.drain(..at);
            first.travel.drain(..at);
        }
    }
    if ends.trail
        && let Some(last) = runs.last_mut()
    {
        let rowed = last.travel.iter().enumerate().skip(1).rev().find(|&(_, &moving)| !moving).map(|(at, _)| at);
        let keep = rowed.map_or(last.points.len().saturating_sub(1), |at| at + 1);
        last.points.truncate(keep);
    }
    runs.into_iter().map(|run| run.points).filter(|points| !points.is_empty()).collect()
}

/// `groups`, each ordered by where its colour first comes among them, and otherwise in their order, as
/// Ink/Stitch sorts a gradient of several parts.
fn sorted(mut groups: Vec<Group>) -> Vec<Group> {
    let colours: Vec<Rgb> = groups.iter().map(|group| group.colour).collect();
    groups.sort_by_key(|group| colours.iter().position(|c| *c == group.colour));
    groups
}

/// The segments of the rows of the gradient `fill` over `region`, each part grown by its `expand` as
/// [`gradient_fill`] grows it, every colour's: where its top stitches lie, without the stitches, the underlay
/// or the needle's travel. A preview too costly to sew shows them. Random lengths draw from `rng`'s streams.
/// Work is charged to `meter` as for sewing.
pub fn top_path(region: &Region, fill: &GradientFill, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let mut segments = Vec::new();
    for polygon in &region.parts {
        for piece in &pieces(polygon, fill.expand, &mut Vec::new(), meter)?.0 {
            for line in coloured(piece, fill, rng, &mut BTreeSet::new(), meter)?.iter().flat_map(|(_, lines)| lines) {
                segments.extend(line.windows(2).filter_map(|pair| Some(Segment { start: *pair.first()?, end: *pair.get(1)? })));
            }
        }
    }
    Ok(segments)
}

/// The colours of the threads a gradient fill of an element in the thread `own` sews in, in their order, read
/// from its paint alone, as the sewing order reads them before it is generated (`REQ-AUTO-024`): its
/// `gradient`'s first stop's colour for its underlay when it has one, then the colours of the stops that aren't
/// transparent, each once, in the order the stops give them. Without a gradient, its own colour for both. A
/// small part may sew fewer of them.
pub(crate) fn threads(gradient: Option<&Gradient>, own: Rgb, underlay: bool) -> Vec<Rgb> {
    let Some(gradient) = gradient else { return if underlay { vec![own, own] } else { vec![own] } };
    let mut colours: Vec<Rgb> = Vec::new();
    for stop in gradient.stops.iter().filter(|stop| stop.opacity > 0.0) {
        if !colours.contains(&stop.colour) {
            colours.push(stop.colour);
        }
    }
    let below = gradient.stops.first().map(|stop| stop.colour).filter(|_| underlay);
    let threads: Vec<Rgb> = below.into_iter().chain(colours).collect();
    if threads.is_empty() { vec![own] } else { threads }
}

#[cfg(test)]
mod tests;
