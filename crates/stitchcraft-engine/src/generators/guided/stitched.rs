//! A guided fill's row stitched: its needle points from a `staggers`-th of a stitch further along than the
//! row before's, placed as `stitch_position_method` says, as Ink/Stitch staggers and stitches its rows (read
//! at `d59c9ab`, `REQ-FILL-GDD-005`).
//!
//! **Staggered.** Row `i`, in the order the rows are taken, is smoothed again by `smoothness_mm`, and its
//! needle points start `(i / staggers) mod 1` of the longest stitch along it from its first point, which is
//! kept before them. `staggers` of 0 counts as 1, so every row starts at its first point, where Ink/Stitch
//! fails (`DEV-FILL-033`). Each row is lengthened well past the piece, so its needle points inside the piece
//! keep the stagger.
//!
//! **Placed.** A row's corners are where Ink/Stitch's running stitch takes them, at joins that turn by 45° or
//! more, so the stagger runs on through the gentler joins where a copy meets its lengthened ends. With
//! random lengths, the running stitch's random lengths, from a stream of the element's generator that the
//! row's place keys. Otherwise flexible spreads the stitches evenly between the corners, as a running stitch
//! spreads them. Flexible (stagger) sews whole stitches from the start, the stagger carried on round the
//! corners, each corner a needle point, and the last stitch whatever is left. Both split a stitch that
//! strays from the row by more than the tolerance, as a running stitch splits it.

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::Guided;
use crate::generators::running::along_row;
use crate::generators::smoothing::smooth;
use crate::normalize::along::Along;

/// Where the needle points lie along each row (`stitch_position_method`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Position {
    /// Spread evenly between the row's corners: `flexible`, the default.
    Flexible,
    /// Whole stitches from where the stagger starts the row, carried on round its corners: `flexible_stagger`.
    Stagger,
}

/// How a guided fill's rows are stitched.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowStitching {
    /// The longest stitch, in millimetres, at least twice the shortest.
    pub length: f64,
    /// How far a stitch may stray from its row, in millimetres.
    pub tolerance: f64,
    /// How much the lengths vary at random, as a share of the length: `None` for even stitches.
    pub jitter: Option<f64>,
    /// How many rows the needle points take to come back to where they started; 0 counts as 1.
    pub staggers: f64,
    /// Where the needle points lie.
    pub position: Position,
}

/// The needle points of `copy`, the `i`-th row taken, stitched as `guided` says, its random lengths, when
/// they vary, drawn from the stream of `rng` its place keys: smoothed ([`smoothed`]), then stitched with
/// its stagger counted from its place ([`stitched`]). Costs as the smoothing and the running stitch do.
pub(super) fn row(copy: &[Point], i: usize, guided: &Guided, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    #[allow(clippy::cast_precision_loss)] // Row counts are far below 2⁵².
    stitched(&smoothed(copy, guided, meter)?, i as f64, i, guided, rng, meter)
}

/// `line` smoothed by `smoothness_mm`, as a contour fill's path is; as it is without.
pub(super) fn smoothed(line: &[Point], guided: &Guided, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    if guided.smoothness > 0.0 { smooth(line, guided.smoothness, guided.travel.min_stitch, meter) } else { Ok(line.to_vec()) }
}

/// The needle points of `line` stitched as `guided` says: its first point, then the running stitch from
/// `(place / staggers) mod 1` of a longest stitch along it to its end, its random lengths, when they vary,
/// drawn from the stream of `rng` that `key` keys. A row's place is its number in the order the rows are
/// taken, and a buffer's ring's the row spacings it lies from the guide. Costs as the running stitch does.
pub(super) fn stitched(line: &[Point], place: f64, key: usize, guided: &Guided, rng: &SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some(&first) = line.first() else { return Ok(Vec::new()) };
    let RowStitching { length, tolerance, jitter, staggers, position } = guided.stitching;
    let staggers = if staggers == 0.0 { 1.0 } else { staggers };
    let phase = (place / staggers).rem_euclid(1.0) * length;
    let along = Along::new(line, meter)?;
    let rest = along.part(phase.min(along.length()), along.length());
    let lengths = (length, tolerance, guided.travel.min_stitch);
    let stitches = along_row(&rest, lengths, jitter, (rng, key), position == Position::Stagger, meter)?;
    Ok(std::iter::once(first).chain(stitches).collect())
}
