//! Ripple stitch (milestones M10.8 and M10.11): lines that spread out from a stroke, sewn as one running
//! stitch, as Ink/Stitch sews its ripple stitch (`docs/src/design/algorithms/strokes.md` › Ripple stitch,
//! `REQ-RIP-001` to `REQ-RIP-018`).
//!
//! **Why.** A ripple covers a shape lightly with lines that follow its outline: rings round a point, a
//! shell's ribs, a band that fades from one edge to the other. Every line is a running stitch, so it sews
//! quickly and lies light on the fabric.
//!
//! **Steps.**
//!
//! 1. **The stroke's lines** are its subpaths, flattened as a shape's outline is, that move. 2 or more are a
//!    satin column's rails and rungs, read by the satin column's own code. 1 open line ripples towards the
//!    target point, and 1 closed line spirals in to it. With a guide line, 1 line's copies follow the guide
//!    instead (the `guided` module).
//! 2. **Measures.** Every line runs through a point of each measure, a pair of points. Along a column they
//!    are the pairs a satin column places every `running_stitch_tolerance_mm`, without compensation
//!    (`satin::ripple_pairs`). Towards a target point they are the needle points of the line's running
//!    stitch, its outline (`running::outline`), each paired with the target point (the `lines` module).
//!    With `manual_pattern_placement` the outline is the line's nodes, and with `grid_size_mm` its needle
//!    points lie that far apart.
//! 3. **Count, places and lines.** `line_count`, or what `min_line_dist_mm` gives with its parity kept. Line
//!    `k` of `n` lies at `(k / (n − 1))^exponent` of the way along each measure, flipped with
//!    `flip_exponent`. A guide places copies of the outline instead.
//! 4. **Bending.** A closed line's lines, the point join's and those along a column without `flip_copies`
//!    bend into the next line, and the last is left out. Then `skip_start` and `skip_end` leave lines out.
//! 5. **Sewing** (the `sew` module), one line at a time with `flip_copies`, back and forth, joined flat or at
//!    a point and staggered, or all as one running stitch, or through the lines' own points with
//!    `manual_pattern_placement`.
//! 6. **`reverse`**, then the clips cut the stitches and their pieces are routed (the `clip` module), and
//!    each run is repeated. A grid's lines across the ripple's are sewn after it (the `grid` module), or
//!    before it with `grid_first`. Then bean stitch ([`passes::sew`]).
//!
//! A ripple starts and ends where its stitches do, whatever its neighbours, as every stroke does. A ripple
//! along a column reads its rails without push compensation, which `SC-W0406` says.

mod clip;
mod grid;
mod guided;
mod lines;
mod sew;
#[cfg(test)]
mod tests;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point};
use stitchcraft_params::kinds::LengthPair;
use stitchcraft_params::{ParamSet, StitchType, params, read_param};

use self::guided::{Copies, Cut, Placing};
pub use self::guided::{RippleGuide, guide};
use self::lines::{Count, Kind};
use self::sew::Sewing;
use crate::design::{Clip, Path, Subpath};
use crate::generators::passes::{self, RepeatParams, Turn};
use crate::generators::running::{self, RunningParams};
use crate::generators::satin::{self, PUSH_COMPENSATION, SatinParams};
use crate::generators::tatami::sew::Travel;
use crate::generators::{Stitched, TooSmall, made, mm, settled, too_small};
use crate::normalize::region::clipped;
use crate::normalize::satin::{Shape, recognize};
use crate::normalize::stroke::{self, SHAPE_TOLERANCE};

/// The stitch types that read a ripple's own settings.
const RIPPLE: &[StitchType] = &[StitchType::RippleStitch];

params! {
    /// Ripple stitch: copies of a stroke's line that shrink towards a point or follow a guide line, or lines
    /// along a satin column from one rail to the other, sewn as one running stitch.
    pub struct RippleParams for RIPPLE;

    "Ripple" {
        /// How many lines the ripple sews. The first is the stroke's own line, or a column's first rail, and
        /// the last the target point, or the second rail. Along a guide, how many copies of the line it
        /// places. With `min_line_dist_mm`, this keeps only the count's parity, odd or even.
        line_count: Count = "10", label "Lines", range (1, 1000);

        /// How far apart the lines are at least. The count is then the shortest distance from the stroke's line
        /// to the target point, or across the column, or the guide's length, divided by this and rounded up.
        /// Where that differs in parity from `line_count`, it is 1 less, so that the ripple ends on the same
        /// side however large it is drawn. Empty or 0: `line_count` gives the count.
        min_line_dist_mm: OptionalLength = "", label "Least distance between lines", range (0.01, 100.0);

        /// How the lines are spaced. 1 spaces them evenly. Above 1 packs them together towards the first line,
        /// and below 1 towards the last.
        exponent: Number = "1", label "Spacing exponent", range (0.1, 100.0);

        /// Pack the lines towards the other end: the spacing the exponent gives starts from the last line.
        flip_exponent: Toggle = "false", label "Flip the spacing";

        /// How many of the first lines are left out, such as the stroke's own line.
        skip_start: Count = "0", label "Skip first lines", range (0, 1000);

        /// How many of the last lines are left out, such as the lines that crowd round the target point.
        skip_end: Count = "0", label "Skip last lines", range (0, 1000);

        /// Sew every other line backwards, so that the ripple goes back and forth. Off, every line is sewn the
        /// same way, and the needle goes straight from each line's end to the next line's start.
        flip_copies: Toggle = "true", label "Back and forth";

        /// How each line meets the next. Flat joins them with a straight run of stitches. Point bends each line
        /// into the next, so that they meet at a point, and the ripple has 1 line more to bend.
        join_style: Choice = "0", label "Join style", options ["0" => "Flat", "1" => "Point"];

        /// How many lines the needle points take to come back to where they started. Each line's needle points
        /// start a staggers-th of the stitch lengths further back than the line's before, so that lines side
        /// by side don't line them up. 0 places every line's needle points from the end it is sewn from.
        staggers: Number = "1", label "Staggers", range (-100.0, 100.0);

        /// Sew the whole ripple the other way round, from its last needle point to its first.
        reverse: Toggle = "false", label "Reverse";

        /// Sew each line through the points it is made of, and nothing between them: the stroke's nodes,
        /// copied, where a line is copied, and the stitch length is not used along the lines. A straight run
        /// of stitches no longer than the stitch length joins each line to the next.
        manual_pattern_placement: Toggle = "false", label "Sew the line's own nodes";
    }

    "Rails" {
        /// Which rails of the column the ripple runs along, or of its satin guide, are read against the way
        /// they are drawn, so that both run the same way. Automatic reverses the second rail when it runs
        /// against the first.
        reverse_rails: Choice = "automatic", label "Reverse rails",
            options ["automatic" => "Automatic", "none" => "Neither", "first" => "The first", "second" => "The second", "both" => "Both"];

        /// Make the second rail of the column, or of the satin guide, the first: the ripple starts on it.
        swap_satin_rails: Toggle = "false", label "Swap rails";
    }

    "Guide" {
        /// Where the copies lie along a guide. Line count places them as `line_count` or `min_line_dist_mm`
        /// says, spread along the guide. At the rungs places one at each point of a guide line, or at each
        /// rung of a satin guide. Adaptive places one wherever it clears the last by `min_line_dist_mm`
        /// between a satin guide's rails.
        satin_guide_pattern_position: Choice = "default", label "Copies along the guide",
            options ["default" => "Line count", "render_at_rungs" => "At the rungs", "adaptive" => "Adaptive"];

        /// Which way the copies are scaled along a guide line, and along a satin guide for a closed line:
        /// both ways, across (x), down (y), or neither.
        scale_axis: Choice = "0", label "Scale axis", options ["0" => "Both", "1" => "Across", "2" => "Down", "3" => "Neither"];

        /// How large the first copy along a guide line is, in percent of the line.
        scale_start: Percent = "100", label "First copy's size", range (-1000.0, 1000.0);

        /// How large the last copy along a guide line is, in percent of the line. The copies between grow or
        /// shrink evenly from the first's size to this.
        scale_end: Percent = "0", label "Last copy's size", range (-1000.0, 1000.0);

        /// Turn each copy as the guide turns: by the angle from the guide's start to the way it runs there,
        /// or for a closed line on a satin guide, as the rails' pairs turn from the first.
        rotate_ripples: Toggle = "true", label "Turn with the guide";
    }

    "Grid" {
        /// How far apart the lines across the ripple's lines are: the ripple then sews a grid, its own lines
        /// and lines across them through every point they share. Empty or 0: no grid.
        grid_size_mm: OptionalLength = "0", label "Grid size", range (0.01, 100.0);

        /// Sew the grid before the ripple's own lines, the whole ripple backwards.
        grid_first: Toggle = "false", label "Grid first";
    }
}

/// A ripple stitch, its settings read.
#[derive(Clone, Debug, PartialEq)]
pub struct Ripple {
    /// Its own settings.
    pub ripple: RippleParams,
    /// The running stitch each of its lines is sewn with.
    pub running: RunningParams,
    /// Its repeats and bean stitch.
    pub passes: RepeatParams,
    /// How a satin column reads the rails of a ripple along a column: its defaults but for the ripple's
    /// `reverse_rails` and `swap_satin_rails`.
    pub satin: SatinParams,
    /// The element's shortest stitch.
    pub min_stitch: Mm,
    /// The element's jump length, for the travel between the pieces its clips leave.
    pub jump: Mm,
    /// The point it aims at, when its design gives one (`REQ-RIP-002`).
    pub target: Option<Point>,
    /// The line its copies follow, when its design gives one (`REQ-RIP-012`, `REQ-RIP-013`).
    pub guide: Option<RippleGuide>,
    /// The ends of the line that sets a satin guide's copies across it (`REQ-RIP-014`).
    pub anchor: Option<[Point; 2]>,
    /// The keys of the satin column's settings that the element gives values other than their defaults, which
    /// a ripple along a column doesn't apply ([`unapplied`]).
    pub unapplied: Vec<&'static str>,
    /// The areas it is cut to (`REQ-RIP-017`).
    pub clips: Vec<Clip>,
}

/// The ripple stitch `ripple` along `path`, its random lengths drawn from streams of `rng`, the element's
/// generator: its runs, repeats and bean stitch included. Every point of every line costs a unit of
/// `meter`'s work, and every needle point a stitch.
pub fn ripple_stitch(path: &Path, ripple: &Ripple, rng: &SplitMix64, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(path, ripple, rng, meter))
}

/// The keys of the settings of a satin column's reading of the rails that `set` gives values other than their
/// defaults and a ripple doesn't apply, read as a satin column reads them: `push_compensation_mm`. Ink/Stitch
/// reads it for a ripple along a column. StitchCraft names it (`SC-W0406`). A value that doesn't read is
/// named too.
pub fn unapplied(set: &ParamSet) -> Vec<&'static str> {
    let spec = SatinParams::SPECS.iter().find(|spec| spec.key == PUSH_COMPENSATION);
    let push = read_param::<LengthPair>(set, spec, Some(StitchType::SatinColumn), SatinParams::SPECS, &mut Vec::new());
    let default = push.is_some_and(|pair| pair.iter().all(|side| side.get() == 0.0));
    if default { Vec::new() } else { vec![PUSH_COMPENSATION] }
}

/// A ripple's lines before they are bent and skipped, each line's points, the first line first, and which
/// ripple they are: with the lines a grid crosses where they are not these, and what was cut of their count.
struct Built {
    kind: Kind,
    lines: Vec<Vec<Point>>,
    grid: Option<Vec<Vec<Point>>>,
    cut: Option<Cut>,
}

/// [`ripple_stitch`], before its stitches are settled.
fn sewn(path: &Path, ripple: &Ripple, rng: &SplitMix64, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let mut warnings = Vec::new();
    let min = ripple.min_stitch.get();
    let own = &ripple.ripple;
    let pattern = running::pattern("running_stitch_length_mm", &ripple.running.running_stitch_length_mm, min, &mut warnings);
    // A grid's lines lie a grid size apart, raised as a stitch length is: the outline's needle points, or a
    // column's pairs, lie that far apart.
    let grid = own.grid_size_mm.map(|size| running::pattern("grid_size_mm", &[size], min, &mut warnings));
    let tolerance = ripple.running.running_stitch_tolerance_mm.get();
    let flat = stroke::flatten(path, SHAPE_TOLERANCE, meter)?;
    let lines: Vec<(usize, &[Point])> =
        flat.pieces.iter().enumerate().filter(|(_, piece)| piece.points.len() > 1).map(|(index, piece)| (index, piece.points.as_slice())).collect();
    if lines.len() < 2 {
        let points = flat.pieces.len() - lines.len();
        warnings.extend((0..points).map(|_| too_small(TooSmall::Point, min)));
    }
    let total: f64 = lines.iter().map(|(_, points)| length(points)).sum();
    if lines.is_empty() {
        return Ok(nothing(warnings));
    }
    if !at_least(total, min) {
        let message = format!(
            "This ripple stitch's lines are {} mm long together, shorter than the shortest stitch ({} mm), so it is not stitched.",
            mm(total),
            mm(min)
        );
        warnings.push(Diagnostic::new(Code::StrokeTooSmall, message));
        return Ok(nothing(warnings));
    }
    let point = own.join_style == "1";
    let built = match lines.as_slice() {
        [(index, points)] => {
            let Some(subpath) = path.subpaths.get(*index) else { return Ok(nothing(warnings)) };
            one_line(subpath, points, ripple, (&pattern, grid.as_deref(), rng), &mut warnings, meter)?
        }
        _ => column(path, ripple, (tolerance, grid.as_deref()), &mut warnings, meter)?,
    };
    let Some(Built { kind, mut lines, grid: mut crossed, cut }) = built else { return Ok(nothing(warnings)) };
    match cut {
        Some(Cut::Distance(given)) => warnings.push(capped(given, lines.len())),
        Some(Cut::Places(places)) => warnings.push(places_cut(places, lines.len())),
        None => {}
    }
    if let Some(flip) = kind.bend(point, own.flip_copies) {
        lines = lines::bent(&lines, flip, meter)?;
        crossed = crossed.map(|crossed| lines::bent(&crossed, flip, meter)).transpose()?;
    }
    let skips = [own.skip_start, own.skip_end].map(|skip| usize::try_from(skip).unwrap_or(usize::MAX));
    let total_lines = lines.len();
    let (kept, skip_start) = lines::skipped(lines, skips);
    if skip_start.is_none() {
        warnings.push(skips_all(skips, total_lines));
    }
    let crossed = crossed.map(|crossed| lines::skipped_as(crossed, skips, skip_start.is_some()));
    let sewing = Sewing {
        pattern: &pattern,
        tolerance,
        min,
        jitter: ripple.running.enable_random_stitch_length.then_some(ripple.running.random_stitch_length_jitter_percent / 100.0),
        rng,
        staggers: own.staggers,
        point,
        skip_start: skip_start.unwrap_or(0),
        flip: own.flip_copies,
    };
    let mut run = if own.manual_pattern_placement {
        sew::manual(&kept, own.flip_copies, pattern.first().copied().unwrap_or(2.0 * min), meter)?
    } else if own.flip_copies && kind != Kind::Spiral {
        sew::one_at_a_time(&kept, &sewing, meter)?
    } else {
        sew::one_run(&kept, &sewing, meter)?
    };
    if !run.iter().any(|point| run.first().is_some_and(|first| first != point)) {
        warnings.push(Diagnostic::new(Code::StrokeTooSmall, "Every line of this ripple stitch is one point, so it is not stitched."));
        return Ok(nothing(warnings));
    }
    if own.reverse {
        run.reverse();
    }
    // The grid crosses the lines sewn, after the skips: a column's lines at the grid's pairs, or the lines
    // themselves.
    let reverse_odd = kind != Kind::Spiral && !own.flip_copies && !point;
    let grid = grid.and_then(|_| Some(grid::lines(crossed.as_ref().unwrap_or(&kept), reverse_odd, *run.last()?)));
    let Some(runs) = finished(run, grid, ripple, &sewing, &mut warnings, meter)? else { return Ok(nothing(warnings)) };
    Ok(Stitched { runs, colours: Vec::new(), warnings })
}

/// A ripple of one line, `points`, the subpath `subpath` flattened: copies of its outline along its guide,
/// or towards its target point, the outline's needle points at the running stitch's `pattern`, or a grid's
/// spacing, placed as the element's generator `rng` places them. `None` when its outline is too small to
/// stitch, which `warnings` says.
fn one_line(
    subpath: &Subpath,
    points: &[Point],
    ripple: &Ripple,
    (pattern, grid, rng): (&[f64], Option<&[f64]>, &SplitMix64),
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Built>, Exhausted> {
    let (own, min) = (&ripple.ripple, ripple.min_stitch.get());
    let tolerance = ripple.running.running_stitch_tolerance_mm.get();
    let lengths = grid.unwrap_or(pattern);
    let kind = lines::kind_of(points);
    let outline = if own.manual_pattern_placement {
        guided::node_points(subpath)
    } else {
        match running::outline(subpath, lengths, (tolerance, min), meter)? {
            Ok(outline) => outline,
            Err(why) => {
                warnings.push(too_small(why, min));
                return Ok(None);
            }
        }
    };
    let adds = kind == Kind::Spiral || own.join_style == "1";
    if let Some(guide) = &ripple.guide {
        // Along a guide line the outline is placed again, at the same lengths, as Ink/Stitch places it.
        let placed_again = matches!(guide, RippleGuide::Line { .. }) && !own.manual_pattern_placement;
        let outline =
            if placed_again { running::along_pattern(&outline, (lengths, tolerance, min), None, (rng, 0), meter)? } else { outline.clone() };
        let placing = Placing { own, adds, closed: kind == Kind::Spiral, anchor: ripple.anchor };
        if let Some(Copies { lines, cut }) = guided::copies(&outline, guide, &placing, warnings, meter)? {
            return Ok(Some(Built { kind, lines, grid: None, cut }));
        }
    }
    let target = lines::target(ripple.target, points, meter)?;
    let measures: Vec<[Point; 2]> = outline.into_iter().map(|point| [point, target]).collect();
    let (lines, cut) = laid(&measures, own, (adds, false), meter)?;
    Ok(Some(Built { kind, lines, grid: None, cut }))
}

/// A ripple along the column `path` draws, its lines through its pairs every `tolerance` millimetres, and
/// with a `grid`, the lines the grid crosses through its pairs a grid size apart. `None` when it has no
/// column or no pair, which `warnings` says.
fn column(
    path: &Path,
    ripple: &Ripple,
    (tolerance, grid): (f64, Option<&[f64]>),
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Built>, Exhausted> {
    let recognition = recognize(path, meter)?;
    warnings.extend(recognition.warnings);
    if !ripple.unapplied.is_empty() {
        warnings.push(rails_not_applied(&ripple.unapplied));
    }
    let Ok(Shape::Rails(column)) = &recognition.shape else { return Ok(None) };
    let sections = satin::ripple_sections(column, &ripple.satin, warnings, meter)?;
    let measures = satin::ripple_pairs(&sections, tolerance, 0.0, meter)?;
    if measures.is_empty() {
        return Ok(None);
    }
    let adds = ripple.ripple.join_style == "1";
    let (lines, cut) = laid(&measures, &ripple.ripple, (adds, true), meter)?;
    let crossed = match grid.and_then(|lengths| lengths.first()) {
        Some(&size) => Some(laid(&satin::ripple_pairs(&sections, size, 0.0, meter)?, &ripple.ripple, (adds, true), meter)?.0),
        None => None,
    };
    Ok(Some(Built { kind: Kind::Column, lines, grid: crossed, cut }))
}

/// The lines through `measures` as `own` counts and places them, with what `min_line_dist_mm` gave when it
/// was cut: 1 more where `adds` says, and at least 2 for a `column`.
fn laid(
    measures: &[[Point; 2]],
    own: &RippleParams,
    (adds, column): (bool, bool),
    meter: &mut Meter,
) -> Result<(Vec<Vec<Point>>, Option<Cut>), Exhausted> {
    let Count { lines: count, cut } = lines::count(own, lines::shortest(measures.iter().map(|[a, b]| a.distance(*b))), (adds, column));
    let places = lines::places(count, own.exponent, own.flip_exponent);
    Ok((lines::lines(measures, &places, meter)?, cut.map(Cut::Distance)))
}

/// The ripple's runs, from its needle points `run` and its `grid`'s lines: cut to its clips and routed,
/// repeated, the grid's lines sewn after it as `sewing` says, all of it backwards with `grid_first`, then
/// bean-stitched. `None` when its clips leave nothing of it, which `warnings` says.
fn finished(
    run: Vec<Point>,
    grid: Option<Vec<Vec<Point>>>,
    ripple: &Ripple,
    sewing: &Sewing,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Option<Vec<Vec<Point>>>, Exhausted> {
    let own = &ripple.ripple;
    let parts = if ripple.clips.is_empty() {
        None
    } else {
        match clipped(&ripple.clips, meter)? {
            Ok(parts) => Some(parts),
            Err(unjoined) => {
                warnings.push(unjoined.clip());
                return Ok(None);
            }
        }
    };
    let travel = Travel {
        length: sewing.pattern.first().copied().unwrap_or(2.0 * sewing.min),
        tolerance: sewing.tolerance,
        min_stitch: sewing.min,
        underpath: false,
        jump: ripple.jump.get(),
    };
    // A grid keeps the travel after the ripple, which leads to it, and the travel before its own first piece.
    let mut groups = match &parts {
        Some(parts) => clip::routed(&run, parts, travel, (None, [false, grid.is_some()]), warnings, meter)?,
        None => vec![run],
    };
    if groups.is_empty() {
        warnings.push(Diagnostic::new(Code::ClippedAway, "Its clip paths leave nothing of this ripple stitch, so it sews nothing."));
        return Ok(None);
    }
    let repeats = ripple.passes.repeats;
    groups = groups.iter().map(|group| repeated(group, repeats, meter)).collect::<Result<_, _>>()?;
    if let (Some(grid), Some(&last)) = (grid, groups.last().and_then(|group| group.last())) {
        let crossing = Sewing { skip_start: 0, ..*sewing };
        let stitched = sew::one_at_a_time(&grid, &crossing, meter)?;
        match &parts {
            Some(parts) => groups.extend(clip::routed(&stitched, parts, travel, (Some(last), [true, false]), warnings, meter)?),
            None => groups.extend((!stitched.is_empty()).then_some(stitched)),
        }
        if own.grid_first {
            groups.iter_mut().for_each(|group| group.reverse());
            groups.reverse();
        }
    }
    let beans = &ripple.passes.bean_stitch_repeats;
    let runs = groups.iter().map(|group| passes::sew(group, 1, beans, Turn::Listed, meter)).collect::<Result<_, _>>()?;
    Ok(Some(runs))
}

/// `group` sewn `repeats` times, there and back, as Ink/Stitch repeats each of a ripple's groups: each pass
/// after the first is the one before reversed, and starts with the point it ended at, listed again, which
/// bean stitch counts a step of ([`passes::sew`]). A stitch for each point a pass adds.
fn repeated(group: &[Point], repeats: u32, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut sewn = group.to_vec();
    for pass in 1..repeats.max(1) {
        made(meter, group.len().saturating_sub(1))?;
        if pass % 2 == 1 {
            sewn.extend(group.iter().rev());
        } else {
            sewn.extend_from_slice(group);
        }
    }
    Ok(sewn)
}

/// A ripple that sews nothing, with what was said of it.
fn nothing(warnings: Vec<Diagnostic>) -> Stitched {
    Stitched { runs: Vec::new(), colours: Vec::new(), warnings }
}

/// The length of the polyline `points`, in millimetres.
fn length(points: &[Point]) -> f64 {
    stroke::segments(points).map(|(a, b)| a.distance(b)).sum()
}

/// `SC-W0406`: what a ripple stitch doesn't sew yet, `message`, with a `hint`.
fn not_sewn_yet(message: &str, hint: &str) -> Diagnostic {
    Diagnostic::new(Code::RippleNotSewnYet, message).with_fix(Fix::Hint(hint.to_string()))
}

/// `SC-W0406` for the satin column's settings `keys` that a ripple along a column doesn't apply.
fn rails_not_applied(keys: &[&str]) -> Diagnostic {
    let named: Vec<String> = keys.iter().map(|key| format!("`{key}`")).collect();
    let message = format!(
        "This ripple stitch sets {}, which StitchCraft doesn't apply to ripple stitch, so its column is read without it.",
        named.join(" and ")
    );
    not_sewn_yet(&message, "Draw the rails as long as the ripple should run, and leave push compensation at 0.")
}

/// `SC-W0407`: `min_line_dist_mm` gives `given` lines, more than [`lines::MOST`], and the ripple sews `count`.
fn capped(given: f64, count: usize) -> Diagnostic {
    let message = format!("`min_line_dist_mm` gives this ripple stitch {given:.0} lines, more than {}, so it sews {count}.", lines::MOST);
    Diagnostic::new(Code::RippleLinesCut, message).with_fix(Fix::Hint("Set a larger `min_line_dist_mm`.".to_string()))
}

/// `SC-W0407`: a guide has `places` places for copies, more than [`lines::MOST`], and the ripple sews `count`.
fn places_cut(places: usize, count: usize) -> Diagnostic {
    let message = format!("This ripple stitch's guide has {places} places for copies, more than {}, so it sews {count}.", lines::MOST);
    Diagnostic::new(Code::RippleLinesCut, message)
        .with_fix(Fix::Hint("Place the copies by `line_count`, or along a guide with fewer points or rungs.".to_string()))
}

/// `SC-W0408`: `skips`, `skip_start` and `skip_end`, would leave out every one of the ripple's `total` lines,
/// and leave out none.
fn skips_all([start, end]: [usize; 2], total: usize) -> Diagnostic {
    let message = format!(
        "`skip_start` and `skip_end` leave out {start} and {end} of this ripple stitch's {total} lines, which would leave none, so no line is left out."
    );
    Diagnostic::new(Code::RippleSkipsAll, message).with_fix(Fix::Hint("Lower `skip_start` or `skip_end`.".to_string()))
}
