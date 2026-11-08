//! Tartan fill without herringbone (milestone M10.6): a plaid woven of warp and weft, each colour's rows sewn
//! in its own thread, as Ink/Stitch sews its tartan fill (`docs/src/design/algorithms/tartan.md`,
//! `REQ-FILL-TRT-001` to `REQ-FILL-TRT-010`, read at `d59c9ab`).
//!
//! **Why a twill of rows.** A woven tartan's warp and weft threads cross over and under one another, so each
//! colour shows in the stripes it is woven into, both ways. Rows slanting across both directions' stripes,
//! taken in turns by the warp and the weft, sew that look: where a warp stripe crosses a weft stripe of
//! another colour, their rows alternate, and the 2 colours mix as woven threads do.
//!
//! **Steps.** A tartan sews its parts as a tatami fill sews them, nearest the needle first, each with the
//! tatami fill's underlay first, in the element's own thread, and grown by `expand_mm` for its rows. Over
//! each piece of a part:
//!
//! 1. **The sett** (module `sett`) is read once for the element: the stripes of its palette code, scaled, its
//!    narrow stripes runs, mirrored or repeated.
//! 2. **Stripes and rows** (module `stripes`): the sett's stripes and the rows are laid over a box round the
//!    piece, turned as the sett is, and each band's rows are joined into zigzags along its edges and cut by
//!    the piece. Each colour gets the lines of its bands, the warp's first.
//! 3. **Each colour** in its thread: its lines are routed as a tatami fill's rows are, from where the colour
//!    before it ended, or the needle, to the end the next part or element offers, the needle travelling
//!    along the piece's outline between them (`tatami::sew::lines`). The travel before its first line and
//!    after its last is left out, so the colour starts and ends with its rows, unless
//!    `stop_at_ending_point` is on, which keeps it: then every colour travels from where the piece starts to
//!    where it ends.
//! 4. **The runs** come after every colour's rows, each colour's routed the same way, its travel kept, and
//!    with bean stitch, but for the travel. The last colour sewn as a fill that has runs too is sewn last of
//!    the fills, and its runs first, so the thread doesn't change between them.
//!
//! **Several parts.** Each part sews its underlay and its colours as above. Then, as Ink/Stitch sorts them,
//! the underlays and colours sewn as fills are ordered by where their colour first came, then the runs, so
//! that each thread sews in one stretch.
//!
//! **What shows them.** Each colour's run is a colour block of its thread, trimmed after when the element's
//! `trim_after` is on, and the underlay sews in the element's thread and ends none
//! ([`crate::generators::Colour`], `REQ-ASM-006`). A colour of the element's own sews in the element's
//! thread, named or not, and the others in threads of their colours alone, so that one colour is one thread.

mod params;
mod sett;
mod stripes;

pub(crate) use params::Given;
pub use params::TartanParams;
pub use sett::Sett;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Diagnostic, Exhausted, Meter, Point};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use self::sett::Item;
use crate::generators::tatami::rings::Rings;
use crate::generators::tatami::rows::Segment;
use crate::generators::tatami::sew::lines::{self, Marked, lines_sewn};
use crate::generators::tatami::sew::{Layer, Parts, PartsSewn, Tally, Travel, in_order, pieces, thinned, underlay};
use crate::generators::{Approach, Colour, Own, Stitched, passes, settled};
use crate::normalize::near::nearest_pair;
use crate::normalize::region::{Polygon, Region};

/// A tartan fill as it is sewn.
#[derive(Clone, Debug, PartialEq)]
pub struct Tartan {
    /// The sett, as read.
    pub(crate) sett: Sett,
    /// The warp's full sett and the weft's ([`Sett::full`]).
    pub(crate) warp: Vec<Item>,
    pub(crate) weft: Vec<Item>,
    /// How wide the wider direction's stripes are, in millimetres, which widens the box round each piece.
    pub(crate) margin: f64,
    /// The rows' angle to the stripes, in degrees (`tartan_angle`).
    pub(crate) angle: f64,
    /// How far apart the rows are, at most, in millimetres.
    pub(crate) spacing: f64,
    /// How far apart the needle points along a row are, in millimetres: the longest stitch.
    pub(crate) longest: f64,
    /// How many rows the needle points take to come back to where they started.
    pub(crate) staggers: f64,
    /// How many rows in turn go to the warp, then to the weft.
    pub(crate) rows_per_thread: u32,
    /// The running stitch's length, of the travel, the joins and the warp's runs, in millimetres.
    pub(crate) run_length: f64,
    /// The runs' bean stitch.
    pub(crate) beans: Vec<u32>,
    /// Whether each line is sewn without its last needle point.
    pub(crate) skip_last: bool,
    /// Whether every colour keeps its travel from the start and to the end (`stop_at_ending_point`).
    pub(crate) stop_at_end: bool,
    /// The underlay's layers, the tatami fill's, over each part shrunk by `inset`, and how far each part is
    /// grown for its rows, in millimetres.
    pub(crate) underlay: Vec<Layer>,
    pub(crate) inset: f64,
    pub(crate) expand: f64,
    /// The needle's travel: its running stitch, the shortest stitch and the jump length.
    pub(crate) travel: Travel,
    /// The element's own thread's colour, which the underlay sews in, and the stripes of that colour.
    pub(crate) own: Rgb,
}

/// What a group of a tartan's stitches is, for its colour and its place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// The underlay, in the element's thread.
    Underlay,
    /// A colour's rows.
    Fill,
    /// A colour's runs.
    Run,
}

/// A group of a tartan's stitches: a run of needle points in one colour.
#[derive(Clone, Debug, PartialEq)]
struct Group {
    colour: Rgb,
    kind: Kind,
    run: Vec<Point>,
}

/// The stitches of the tartan fill `tartan` over `region`, from the needle where the elements before left it
/// to near the `next` element, as the module's docs say. The underlay draws from `rng`. Work is charged to
/// `meter` for every step of the setts, row, band and stitch, and a stitch for every needle point.
pub fn tartan_fill(
    region: &Region,
    tartan: &Tartan,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(region, tartan, needle, next, rng, meter))
}

/// [`tartan_fill`], before its stitches are settled.
fn sewn(
    region: &Region,
    tartan: &Tartan,
    needle: Option<Point>,
    next: Option<&Approach>,
    rng: &mut SplitMix64,
    meter: &mut Meter,
) -> Result<Stitched, Exhausted> {
    let ordered = in_order(region, needle, meter)?;
    let end = next.map(|next| next.next_stitch(Own::Area(&region.parts), meter)).transpose()?.flatten();
    let parts =
        Parts { underlay: &tartan.underlay, inset: tartan.inset, expand: tartan.expand, force_locks: false, top: tartan.travel, ends_near: true };
    let mut said = PartsSewn { runs: Vec::new(), warnings: Vec::new(), tally: Tally { parts: ordered.len(), ..Tally::default() } };
    let (mut needle, mut groups) = (needle, Vec::new());
    for (i, polygon) in ordered.iter().enumerate() {
        let here = match ordered.get(i + 1) {
            Some(after) => nearest_pair(&polygon.rings().collect::<Vec<_>>(), &after.rings().collect::<Vec<_>>(), meter)?.map(|(on, _)| on),
            None => end,
        };
        let below = thinned(underlay(&parts, polygon, &mut needle, &mut said, rng, meter)?, tartan.travel.min_stitch);
        groups.extend(below.into_iter().map(|run| Group { colour: tartan.own, kind: Kind::Underlay, run }));
        needle = groups.last().and_then(|group| group.run.last().copied()).or(needle);
        let (pieces, gone) = pieces(polygon, tartan.expand, &mut said.warnings, meter)?;
        said.tally.expand_gone += usize::from(gone);
        for piece in &pieces {
            let top = top(piece, tartan, (needle, here), meter, &mut said.warnings)?;
            needle = top.last().and_then(|group| group.run.last().copied()).or(needle);
            groups.extend(top);
        }
    }
    if ordered.len() > 1 {
        groups = sorted(groups);
    }
    // A colour of the element's own sews in the element's own thread, named or not, and the others in threads
    // of their colours alone: a named and an unnamed thread of one colour would be 2 threads, which a machine
    // file writes as 2 entries of its palette.
    let colours = groups
        .iter()
        .map(|group| match group.kind {
            Kind::Underlay => Colour { thread: None, ends_block: false },
            Kind::Fill | Kind::Run => Colour { thread: (group.colour != tartan.own).then(|| Thread::new(group.colour)), ends_block: true },
        })
        .collect();
    let mut warnings = said.warnings;
    warnings.extend(said.tally.said());
    Ok(Stitched { runs: groups.into_iter().map(|group| group.run).collect(), colours, warnings })
}

/// The groups the tartan's colours sew over `piece`, from `start` towards `end` (`ends`), as the module's
/// docs say. A jump the needle needs between lines is said in `warnings`.
fn top(
    piece: &Polygon,
    tartan: &Tartan,
    (start, end): (Option<Point>, Option<Point>),
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Result<Vec<Group>, Exhausted> {
    let laid = stripes::lay(piece, tartan, meter)?;
    let (fills, runs) = joined(laid.fills, laid.runs);
    let rings = Rings::new(piece, meter)?;
    let given = lines::Given { travel: Travel { underpath: false, ..tartan.travel }, angle: 0.0, skip_last: tartan.skip_last };
    // Without a needle or an end, as for a first element without underlay or the design's last, a colour
    // starts and ends at the second point of the outline, as Ink/Stitch's does. Its geometry library rebuilds
    // the outline, which then starts one point on from where the drawing reaches it first, so that point is
    // the third here, both running the same way.
    let second = piece.outline.get(2).or(piece.outline.first()).copied();
    let min = tartan.travel.min_stitch;
    let mut groups: Vec<Group> = Vec::new();
    let mut from = start;
    for (colour, lines) in fills {
        if !tartan.stop_at_end {
            from = groups.last().and_then(|group| group.run.last().copied()).or(from);
        }
        from = from.or(second);
        let Some(sewn) = lines_sewn(piece, &rings, &lines, &given, from, end.or(second), meter, warnings)? else { continue };
        for Marked { points, travel } in sewn {
            let run = if tartan.stop_at_end { points } else { without_travel(points, &travel) };
            groups.extend(thinned(vec![run], min).into_iter().map(|run| Group { colour, kind: Kind::Fill, run }));
        }
    }
    let mut from = start;
    for (colour, lines) in runs {
        if !tartan.stop_at_end {
            from = groups.last().and_then(|group| group.run.last().copied()).or(from);
        }
        let Some(sewn) = lines_sewn(piece, &rings, &lines, &given, from, end, meter, warnings)? else { continue };
        for Marked { points, travel } in sewn {
            let run = passes::beaned(&points, &tartan.beans, &travel, meter)?;
            groups.extend(thinned(vec![run], min).into_iter().map(|run| Group { colour, kind: Kind::Run, run }));
        }
    }
    Ok(groups)
}

/// The colours sewn as fills and those with runs, each with its lines, in the order sewn: the last colour
/// sewn as a fill that has runs too moved to the end of the fills and the start of the runs, as Ink/Stitch
/// moves it, so that the thread doesn't change between its rows and its runs.
type Coloured = Vec<(Rgb, Vec<Vec<Point>>)>;

/// [`Coloured`]'s order, of `fills` and `runs` as laid.
fn joined(mut fills: Coloured, mut runs: Coloured) -> (Coloured, Coloured) {
    let shared = fills.iter().rposition(|(colour, _)| runs.iter().any(|(c, _)| c == colour));
    if let Some(at) = shared {
        let fill = fills.remove(at);
        if let Some(run_at) = runs.iter().position(|(c, _)| *c == fill.0) {
            let run = runs.remove(run_at);
            runs.insert(0, run);
        }
        fills.push(fill);
    }
    (fills, runs)
}

/// `points` without the travel before the first point the needle doesn't travel to and after the last, as
/// `travel` marks them; as they are when the needle travels to every point.
fn without_travel(points: Vec<Point>, travel: &[bool]) -> Vec<Point> {
    let first = travel.iter().position(|&moving| !moving);
    let last = travel.iter().rposition(|&moving| !moving);
    match (first, last) {
        (Some(first), Some(last)) => points.get(first..=last).map(<[Point]>::to_vec).unwrap_or(points),
        _ => points,
    }
}

/// `groups`, the underlays and colours sewn as fills first, then the runs, each ordered by where its colour
/// first comes among them, and otherwise in their order, as Ink/Stitch sorts a tartan of several parts.
fn sorted(groups: Vec<Group>) -> Vec<Group> {
    let (mut fills, mut runs): (Vec<Group>, Vec<Group>) = groups.into_iter().partition(|group| group.kind != Kind::Run);
    for list in [&mut fills, &mut runs] {
        let colours: Vec<Rgb> = list.iter().map(|group| group.colour).collect();
        list.sort_by_key(|group| colours.iter().position(|c| *c == group.colour));
    }
    fills.extend(runs);
    fills
}

/// The segments of the lines and runs of the `tartan` over `region`, each part grown by its `expand` as
/// [`tartan_fill`] grows it: where its top stitches lie, without the stitches, the underlay or the needle's
/// travel. A preview too costly to sew shows them. Work is charged to `meter` as for sewing.
pub fn top_path(region: &Region, tartan: &Tartan, meter: &mut Meter) -> Result<Vec<Segment>, Exhausted> {
    let mut segments = Vec::new();
    for polygon in &region.parts {
        for piece in &pieces(polygon, tartan.expand, &mut Vec::new(), meter)?.0 {
            let laid = stripes::lay(piece, tartan, meter)?;
            for line in laid.fills.iter().chain(&laid.runs).flat_map(|(_, lines)| lines) {
                segments.extend(line.windows(2).filter_map(|pair| Some(Segment { start: *pair.first()?, end: *pair.get(1)? })));
            }
        }
    }
    Ok(segments)
}

/// The colours of the threads a tartan of `set`, of an element in the thread `own`, sews in, in their order,
/// read from its settings alone, as the sewing order reads them before it is generated (`REQ-AUTO-024`): its
/// own for the underlay when it has one, then the sett's colours sewn as fills, the warp's in the order its
/// stripes first reach them and then the weft's, and then those of its runs, the colour they share last and
/// first. A sett that can't be read or sews nothing sews in its own thread alone, and a small part may sew
/// fewer of them.
pub(crate) fn threads(set: &ParamSet, own: Rgb, underlay: bool) -> Vec<Rgb> {
    let tartan = TartanParams::from_set_as(set, stitchcraft_params::StitchType::TartanFill).map(|read| read.params.tartan).unwrap_or_default();
    let Ok(read) = sett::read(&tartan) else { return vec![own] };
    let sett = read.sett;
    // Each direction's colours come in the order its sett first reaches them, by a band or a run, as
    // `stripes::lay` orders them; a colour sews as a fill where it has a band with a width.
    let (mut fills, mut runs): (Vec<Rgb>, Vec<Rgb>) = (Vec::new(), Vec::new());
    for items in [sett.full(&sett.warp), sett.full(&sett.weft)] {
        let mut order: Vec<Rgb> = Vec::new();
        for item in &items {
            if let Item::Band { colour: Some(colour), .. } | Item::Run(colour) = *item
                && !order.contains(&colour)
            {
                order.push(colour);
            }
        }
        for colour in order {
            let filled = items.iter().any(|item| matches!(*item, Item::Band { colour: Some(c), width } if c == colour && width > 0.0));
            let ran = items.contains(&Item::Run(colour));
            for (list, has) in [(&mut fills, filled), (&mut runs, ran)] {
                if has && !list.contains(&colour) {
                    list.push(colour);
                }
            }
        }
    }
    let as_lines = |colours: Vec<Rgb>| colours.into_iter().map(|colour| (colour, Vec::new())).collect::<Coloured>();
    let (fills, runs) = joined(as_lines(fills), as_lines(runs));
    let mut threads = if underlay { vec![own] } else { Vec::new() };
    threads.extend(fills.into_iter().chain(runs).map(|(colour, _)| colour));
    if threads.is_empty() { vec![own] } else { threads }
}

#[cfg(test)]
mod tests;
