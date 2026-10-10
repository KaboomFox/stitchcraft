//! Underlays (`docs/src/design/algorithms/satin.md` › Underlays): stitches sewn before a satin column's
//! top stitches, which hold the fabric still and lift the top stitches off it, as Ink/Stitch sews them.
//!
//! Each underlay places pairs of points across the column as the top stitches are placed, along the same
//! sections (the `pairs` module), moved in by its insets as negative pull compensation moves a pair, and
//! with nothing drawn at random (the `compensation` module).
//!
//! - The **centre walk** places pairs every `center_walk_underlay_stitch_tolerance_mm`, each moved in until
//!   its ends meet `center_walk_underlay_position` percent of the way from the first rail to the second. A
//!   running stitch of `center_walk_underlay_stitch_length_mm` follows the line through them, there and
//!   back, `center_walk_underlay_repeats` times. Ink/Stitch uses the satin's running stitch tolerance here
//!   in place of the centre walk's own (`DEV-SAT-004`).
//! - The **contour** places pairs every `contour_underlay_stitch_tolerance_mm`, inset by
//!   `contour_underlay_inset_mm` plus `contour_underlay_inset_percent` of the width, and runs a running
//!   stitch along each rail's side. Each side then stops short of the column's start by the first rail's
//!   inset in millimetres and of its end by the second's, as push compensation shortens a rail: a side too
//!   short for that keeps its length (`SC-W0206`). The first rail's side goes towards the end, and the
//!   second's back.
//! - The **zigzag** places pairs every half `zigzag_underlay_spacing_mm`, inset by its own insets, which are
//!   half the contour's when left empty. It zigzags to the column's end through one end of each pair, on
//!   either rail in turn, and back through the other ends, so each rail's points are the spacing apart
//!   each way. A stitch longer than `zigzag_underlay_max_stitch_length_mm` is split into equal parts.
//!
//! A centre walk with an odd number of repeats ends at the column's end, and the contour, the zigzag and
//! the top stitches then run from the end to the start. The needle goes straight from each part to the
//! next, in equal stitches no longer than the running stitch's length.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Mm, Point};

use crate::generators::mm;
use crate::generators::passes;
use crate::generators::running::along_line;
use crate::generators::satin::SatinParams;
use crate::generators::satin::column::Section;
use crate::generators::satin::compensation::{Processor, pushed};
use crate::generators::satin::pairs::{Pair, pairs};
use crate::generators::satin::split::evenly;

/// Whether the centre walk ends at the column's end: it is on, with an odd number of repeats. The rest of
/// the column is then sewn from its end to its start, as in Ink/Stitch.
pub(crate) fn ends_at_end(params: &SatinParams) -> bool {
    params.center_walk_underlay && params.center_walk_underlay_repeats % 2 == 1
}

/// The parts of the underlays `params` turn on, for the column cut into `sections`, in the order they are
/// sewn: the centre walk, the contour's 2 sides and the zigzag's 2 ways, each a run of needle points. The
/// walks keep to the element's shortest stitch, `min_stitch`, and what was changed goes to `warnings`.
pub(crate) fn underlays(
    sections: &[Section],
    params: &SatinParams,
    min_stitch: f64,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let mut parts = Vec::new();
    if params.center_walk_underlay {
        parts.push(centre_walk(sections, params, min_stitch, warnings, meter)?);
    }
    if params.contour_underlay {
        parts.extend(contour(sections, params, min_stitch, warnings, meter)?);
    }
    if params.zigzag_underlay {
        parts.extend(zigzag(sections, params, meter)?);
    }
    Ok(parts)
}

/// `parts` sewn one after the other, the needle going straight from where each ends to where the next
/// starts, in equal stitches no longer than `travel`.
pub(crate) fn join(parts: Vec<Vec<Point>>, travel: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let mut run: Vec<Point> = Vec::new();
    for part in parts {
        if let (Some(&from), Some(&to)) = (run.last(), part.first()) {
            run.extend(evenly(from, to, travel, meter)?);
        }
        run.extend(part);
    }
    Ok(run)
}

/// The centre walk: a running stitch along the line between the rails at its position, there and back.
fn centre_walk(
    sections: &[Section],
    params: &SatinParams,
    min_stitch: f64,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Point>, Exhausted> {
    let (position, tolerance) = (params.center_walk_underlay_position / 100.0, params.center_walk_underlay_stitch_tolerance_mm.get());
    let placed = pairs(sections, tolerance, &mut Processor::inset([0.0; 2], [position, 1.0 - position]), meter)?;
    let line: Vec<Point> = placed.iter().map(|[a, _]| *a).collect();
    let length = params.center_walk_underlay_stitch_length_mm;
    let walk = along_line(&line, "center_walk_underlay_stitch_length_mm", length, tolerance, min_stitch, warnings, meter)?;
    passes::sew(&walk, params.center_walk_underlay_repeats, &[], meter)
}

/// The contour's 2 sides, in the order they are sewn, each stopping short of the column's ends.
fn contour(
    sections: &[Section],
    params: &SatinParams,
    min_stitch: f64,
    warnings: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Vec<Point>>, Exhausted> {
    let inset = params.contour_underlay_inset_mm.map(Mm::get);
    let share = params.contour_underlay_inset_percent.map(|percent| percent / 100.0);
    let tolerance = params.contour_underlay_stitch_tolerance_mm.get();
    let placed = pairs(sections, tolerance, &mut Processor::inset(inset, share), meter)?;
    let mut side = |pick: fn(&Pair) -> Point| -> Result<(Vec<Point>, bool), Exhausted> {
        let line: Vec<Point> = placed.iter().map(pick).collect();
        let length = params.contour_underlay_stitch_length_mm;
        let run = along_line(&line, "contour_underlay_stitch_length_mm", length, tolerance, min_stitch, warnings, meter)?;
        pushed(&run, inset, meter)
    };
    let ((mut first, kept_first), (mut second, kept_second)) = (side(|[a, _]| *a)?, side(|[_, b]| *b)?);
    if kept_first || kept_second {
        warnings.push(too_short(inset));
    }
    if ends_at_end(params) {
        first.reverse();
    } else {
        second.reverse();
    }
    Ok(vec![first, second])
}

/// `SC-W0206`, for a contour underlay too short to stop `start` millimetres short of the column's start
/// and `end` of its end.
fn too_short([start, end]: [f64; 2]) -> Diagnostic {
    let message = format!(
        "This satin column's contour underlay is too short to stop {} mm short of the column's start and {} mm short of its end, so it keeps its length.",
        mm(start),
        mm(end)
    );
    Diagnostic::new(Code::SatinContourTooShort, message).with_fix(Fix::Hint("Lower `contour_underlay_inset_mm`.".to_string()))
}

/// The zigzag's 2 ways: through one end of each pair, on either rail in turn, and back through the others.
fn zigzag(sections: &[Section], params: &SatinParams, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let inset = params.zigzag_underlay_inset_mm.map_or(params.contour_underlay_inset_mm.map(|mm| mm.get() / 2.0), |pair| pair.map(Mm::get));
    let percent = params.zigzag_underlay_inset_percent.unwrap_or(params.contour_underlay_inset_percent.map(|percent| percent / 2.0));
    let spacing = params.zigzag_underlay_spacing_mm.get() / 2.0;
    let mut placed = pairs(sections, spacing, &mut Processor::inset(inset, percent.map(|percent| percent / 100.0)), meter)?;
    if ends_at_end(params) {
        placed.reverse();
    }
    let there: Vec<Point> = placed.iter().enumerate().map(|(i, &[a, b])| if i % 2 == 0 { a } else { b }).collect();
    let back: Vec<Point> = placed.iter().enumerate().rev().map(|(i, &[a, b])| if i % 2 == 0 { b } else { a }).collect();
    let Some(longest) = params.zigzag_underlay_max_stitch_length_mm else { return Ok(vec![there, back]) };
    let split = |points: Vec<Point>, meter: &mut Meter| join(points.into_iter().map(|point| vec![point]).collect(), longest.get(), meter);
    Ok(vec![split(there, meter)?, split(back, meter)?])
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn parts_are_joined_by_equal_stitches_no_longer_than_the_travel() {
        let meter = &mut Budget::DEFAULT.meter();
        let parts = vec![vec![p(0.0, 0.0), p(1.0, 0.0)], vec![], vec![p(1.0, 5.0)], vec![p(1.0, 6.0)]];
        // 5 mm from the first part to the third, in 3 stitches of 1.67 mm, then 1 mm straight on.
        let joined = join(parts, 2.0, meter).unwrap();
        let want = [p(0.0, 0.0), p(1.0, 0.0), p(1.0, 5.0 / 3.0), p(1.0, 10.0 / 3.0), p(1.0, 5.0), p(1.0, 6.0)];
        assert!(joined.len() == want.len() && joined.iter().zip(want).all(|(a, b)| a.distance(b) < 1e-12), "{joined:?}");
        assert!(join(vec![], 2.0, meter).unwrap().is_empty());
    }
}
