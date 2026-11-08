//! Ripple stitch (milestone M10.8): lines that spread out from a stroke, sewn as one running stitch, as
//! Ink/Stitch sews its ripple stitch (`docs/src/design/algorithms/strokes.md` › Ripple stitch, `REQ-RIP-001`
//! to `REQ-RIP-010`).
//!
//! **Why.** A ripple covers a shape lightly with lines that follow its outline: rings round a point, a
//! shell's ribs, a band that fades from one edge to the other. Every line is a running stitch, so it sews
//! quickly and lies light on the fabric.
//!
//! **Steps.**
//!
//! 1. **The stroke's lines** are its subpaths, flattened as a shape's outline is, that move. 2 or more are a
//!    satin column's rails and rungs, read by the satin column's own code. 1 open line ripples towards the
//!    target point, and 1 closed line spirals in to it.
//! 2. **Measures.** Every line runs through a point of each measure, a pair of points. Along a column they
//!    are the pairs a satin column places every `running_stitch_tolerance_mm`, without compensation
//!    (`satin::ripple_pairs`). Towards a target point they are the needle points of the line's running
//!    stitch, its outline (`running::outline`), each paired with the target point (the `lines` module).
//! 3. **Count, places and lines.** `line_count`, or what `min_line_dist_mm` gives with its parity kept. Line
//!    `k` of `n` lies at `(k / (n − 1))^exponent` of the way along each measure, flipped with
//!    `flip_exponent`.
//! 4. **Bending.** A closed line's lines, the point join's and those along a column without `flip_copies`
//!    bend into the next line, and the last is left out. Then `skip_start` and `skip_end` leave lines out.
//! 5. **Sewing** (the `sew` module), one line at a time with `flip_copies`, back and forth, joined flat or at
//!    a point and staggered, or all as one running stitch.
//! 6. **`reverse`**, then repeats and bean stitch ([`passes::sew`]).
//!
//! A ripple starts and ends where its stitches do, whatever its neighbours, as every stroke does. What
//! M10.11 sews, guided ripples and clips, is left out with `SC-W0406`.

mod lines;
mod sew;
#[cfg(test)]
mod tests;

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::units::at_least;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point};
use stitchcraft_params::kinds::{Choice, LengthPair, Toggle};
use stitchcraft_params::{ParamSet, StitchType, params, read_param};

use self::lines::{Count, Kind};
use self::sew::Sewing;
use crate::design::Path;
use crate::generators::passes::{self, RepeatParams, Turn};
use crate::generators::running::{self, RunningParams};
use crate::generators::satin::{self, PUSH_COMPENSATION, SatinParams};
use crate::generators::{Stitched, TooSmall, mm, settled, too_small};
use crate::normalize::satin::{Shape, recognize};
use crate::normalize::stroke::{self, SHAPE_TOLERANCE};

/// The stitch types that read a ripple's own settings.
const RIPPLE: &[StitchType] = &[StitchType::RippleStitch];

/// The keys of a satin column's settings that turn or swap its rails, which a ripple along a column doesn't
/// apply yet ([`unapplied`]).
const REVERSE_RAILS: &str = "reverse_rails";
/// See [`REVERSE_RAILS`].
const SWAP_SATIN_RAILS: &str = "swap_satin_rails";

params! {
    /// Ripple stitch: copies of a stroke's line that shrink towards a point, or lines along a satin column from
    /// one rail to the other, sewn as one running stitch.
    pub struct RippleParams for RIPPLE;

    "Ripple" {
        /// How many lines the ripple sews. The first is the stroke's own line, or a column's first rail, and
        /// the last the target point, or the second rail. With `min_line_dist_mm`, this keeps only the
        /// count's parity, odd or even.
        line_count: Count = "10", label "Lines", range (1, 1000);

        /// How far apart the lines are at least. The count is then the shortest distance from the stroke's line
        /// to the target point, or across the column, divided by this and rounded up. Where that differs in
        /// parity from `line_count`, it is 1 less, so that the ripple ends on the same side however large it
        /// is drawn. Empty or 0: `line_count` gives the count.
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
    /// How a satin column reads the rails of a ripple along a column: its defaults, as a ripple applies none
    /// of its settings yet.
    pub satin: SatinParams,
    /// The element's shortest stitch.
    pub min_stitch: Mm,
    /// The point it aims at, when its design gives one (`REQ-RIP-002`).
    pub target: Option<Point>,
    /// The keys of the satin column's settings that the element gives values other than their defaults, which
    /// a ripple along a column doesn't apply yet ([`unapplied`]).
    pub unapplied: Vec<&'static str>,
    /// Whether clips cut it, which it doesn't apply yet: it is sewn whole.
    pub clipped: bool,
}

/// The ripple stitch `ripple` along `path`, its random lengths drawn from streams of `rng`, the element's
/// generator: one run, its repeats and bean stitch included. Every point of every line costs a unit of
/// `meter`'s work, and every needle point a stitch.
pub fn ripple_stitch(path: &Path, ripple: &Ripple, rng: &SplitMix64, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    settled(meter, |meter| sewn(path, ripple, rng, meter))
}

/// The keys of the settings of a satin column's reading of the rails that `set` gives values other than their
/// defaults, read as a satin column reads them: `reverse_rails`, `swap_satin_rails` and
/// `push_compensation_mm`. Ink/Stitch reads them for a ripple along a column. StitchCraft names them
/// (`SC-W0406`) until M10.11 applies them. A value that doesn't read is named too.
pub fn unapplied(set: &ParamSet) -> Vec<&'static str> {
    let spec = |key: &str| SatinParams::SPECS.iter().find(|spec| spec.key == key);
    let column = Some(StitchType::SatinColumn);
    let reverse = read_param::<Choice>(set, spec(REVERSE_RAILS), column, SatinParams::SPECS, &mut Vec::new());
    let swap = read_param::<Toggle>(set, spec(SWAP_SATIN_RAILS), column, SatinParams::SPECS, &mut Vec::new());
    let push = read_param::<LengthPair>(set, spec(PUSH_COMPENSATION), column, SatinParams::SPECS, &mut Vec::new());
    let defaults = [
        (REVERSE_RAILS, reverse == Some("automatic")),
        (SWAP_SATIN_RAILS, swap == Some(false)),
        (PUSH_COMPENSATION, push.is_some_and(|pair| pair.iter().all(|side| side.get() == 0.0))),
    ];
    defaults.into_iter().filter(|(_, default)| !default).map(|(key, _)| key).collect()
}

/// [`ripple_stitch`], before its stitches are settled.
fn sewn(path: &Path, ripple: &Ripple, rng: &SplitMix64, meter: &mut Meter) -> Result<Stitched, Exhausted> {
    let mut warnings = Vec::new();
    let min = ripple.min_stitch.get();
    let own = &ripple.ripple;
    let pattern = running::pattern("running_stitch_length_mm", &ripple.running.running_stitch_length_mm, min, &mut warnings);
    let tolerance = ripple.running.running_stitch_tolerance_mm.get();
    if ripple.clipped {
        warnings.push(not_sewn_yet(
            "This ripple stitch is cut to clip paths, which StitchCraft doesn't apply to ripple stitch yet, so it is sewn whole.",
            "Draw the ripple's line where it should sew: Ink/Stitch sews only what lies inside the clips.",
        ));
    }
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
    let (kind, measures) = match lines.as_slice() {
        [(index, points)] => {
            let Some(subpath) = path.subpaths.get(*index) else { return Ok(nothing(warnings)) };
            let outline = match running::outline(subpath, &pattern, (tolerance, min), meter)? {
                Ok(outline) => outline,
                Err(why) => {
                    warnings.push(too_small(why, min));
                    return Ok(nothing(warnings));
                }
            };
            let target = lines::target(ripple.target, points, meter)?;
            (lines::kind_of(points), outline.into_iter().map(|point| [point, target]).collect())
        }
        _ => {
            let recognition = recognize(path, meter)?;
            warnings.extend(recognition.warnings);
            if !ripple.unapplied.is_empty() {
                warnings.push(rails_not_applied(&ripple.unapplied));
            }
            let Ok(Shape::Rails(column)) = &recognition.shape else { return Ok(nothing(warnings)) };
            (Kind::Column, satin::ripple_pairs(column, &ripple.satin, tolerance, &mut warnings, meter)?)
        }
    };
    if measures.is_empty() {
        return Ok(nothing(warnings));
    }
    let (point, flip_copies) = (own.join_style == "1", own.flip_copies);
    let Count { lines: count, cut } =
        lines::count(own, lines::shortest(measures.iter().map(|[a, b]| a.distance(*b))), (kind == Kind::Spiral || point, kind == Kind::Column));
    if let Some(given) = cut {
        warnings.push(capped(given, count));
    }
    let places = lines::places(count, own.exponent, own.flip_exponent);
    let mut ripples = lines::lines(&measures, &places, meter)?;
    if let Some(flip) = kind.bend(point, flip_copies) {
        ripples = lines::bent(&ripples, flip, meter)?;
    }
    let skips = [own.skip_start, own.skip_end].map(|skip| usize::try_from(skip).unwrap_or(usize::MAX));
    let total_lines = ripples.len();
    let (kept, skip_start) = lines::skipped(ripples, skips);
    if skip_start.is_none() {
        warnings.push(skips_all(skips, total_lines));
    }
    let sewing = Sewing {
        pattern: &pattern,
        tolerance,
        min,
        jitter: ripple.running.enable_random_stitch_length.then_some(ripple.running.random_stitch_length_jitter_percent / 100.0),
        rng,
        staggers: own.staggers,
        point,
        skip_start: skip_start.unwrap_or(0),
    };
    let mut run = if flip_copies && kind != Kind::Spiral { sew::one_at_a_time(&kept, &sewing, meter)? } else { sew::one_run(&kept, &sewing, meter)? };
    if !run.iter().any(|point| run.first().is_some_and(|first| first != point)) {
        warnings.push(Diagnostic::new(Code::StrokeTooSmall, "Every line of this ripple stitch is one point, so it is not stitched."));
        return Ok(nothing(warnings));
    }
    if own.reverse {
        run.reverse();
    }
    let sewn = passes::sew(&run, ripple.passes.repeats, &ripple.passes.bean_stitch_repeats, Turn::Listed, meter)?;
    Ok(Stitched { runs: vec![sewn], colours: Vec::new(), warnings })
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

/// `SC-W0406` for the satin column's settings `keys` that a ripple along a column doesn't apply yet.
fn rails_not_applied(keys: &[&str]) -> Diagnostic {
    let named: Vec<String> = keys.iter().map(|key| format!("`{key}`")).collect();
    let named = match named.split_last() {
        Some((last, rest)) if !rest.is_empty() => format!("{} and {last}", rest.join(", ")),
        _ => named.concat(),
    };
    let message =
        format!("This ripple stitch sets {named}, which StitchCraft doesn't apply to ripple stitch yet, so its column is read with their defaults.");
    not_sewn_yet(&message, "Draw the rails running the same way, the first where the ripple starts, and leave these settings at their defaults.")
}

/// `SC-W0407`: `min_line_dist_mm` gives `given` lines, more than [`lines::MOST`], and the ripple sews `count`.
fn capped(given: f64, count: usize) -> Diagnostic {
    let message = format!("`min_line_dist_mm` gives this ripple stitch {given:.0} lines, more than {}, so it sews {count}.", lines::MOST);
    Diagnostic::new(Code::RippleLinesCut, message).with_fix(Fix::Hint("Set a larger `min_line_dist_mm`.".to_string()))
}

/// `SC-W0408`: `skips`, `skip_start` and `skip_end`, would leave out every one of the ripple's `total` lines,
/// and leave out none.
fn skips_all([start, end]: [usize; 2], total: usize) -> Diagnostic {
    let message = format!(
        "`skip_start` and `skip_end` leave out {start} and {end} of this ripple stitch's {total} lines, which would leave none, so no line is left out."
    );
    Diagnostic::new(Code::RippleSkipsAll, message).with_fix(Fix::Hint("Lower `skip_start` or `skip_end`.".to_string()))
}
