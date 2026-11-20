//! Wide columns: a part of an area, or a line, wider than the widest satin column and no wider than the widest
//! split satin column (`auto_max_split_satin_width_mm`) that is otherwise a good column sews as one satin
//! column whose stitches are split (`REQ-AUTO-152` to `REQ-AUTO-156`).
//!
//! Design: `docs/src/design/image-to-embroidery.md` › Wide columns. A satin stitch longer than about 7 mm
//! lies loose and snags, so a wider part is filled. The trade sews a border or a bar up to about 10 mm wide as
//! satin anyway, its long stitches split into shorter ones, which keeps satin's sheen where a fill reads flat.
//! The splits fall at random places, so that the needle holes of neighbouring stitches don't sew a seam along
//! the column: split into equal parts, nearly every split point lines up with the next.
//!
//! - **Off by default.** The setting's default is the longest satin stitch, at which no part is wide
//!   (`Bounds::max_split`), so every design sews as it did until a person raises it. A wide column sews
//!   thicker than a fill and with about half as much thread again, which a sew-out has to confirm first.
//! - **Only a good column.** A wide part has one centre line and an even width (`REQ-AUTO-153`), and is held
//!   to every other rule of a column: where 2 wide columns meet each fans its split stitches into the other,
//!   and a swelling column splits its stitches unevenly. Its stitches aren't refused for their length, since
//!   they are split, but a column that can't be drawn or that crowds its stitches is (`REQ-AUTO-154`).
//! - **Only a whole part.** A piece cut from a broader part keeps the rules it had, so that the rule adds no
//!   seam to a fill.

use stitchcraft_core::{Code, Diagnostic, ElementId, Exhausted, Fix, Meter};
use stitchcraft_params::ParamSet;

use super::columns::{self, Made, Sewn};
use super::narrow::{By, within};
use super::params::LONGEST_SATIN_STITCH;
use super::rules::{Asked, Bounds, Measures, RuleId};
use crate::common::MAX_STITCH_LENGTH;
use crate::generators::mm;
use crate::generators::satin::{RANDOM_SPLIT_JITTER_PERCENT, RANDOM_SPLIT_PHASE, SPLIT_METHOD};
use crate::normalize::region::Polygon;

/// How far a wide column's split may move at random, in percent of the longest satin stitch: the splits of
/// neighbouring stitches then fall apart, and only 6 to 9 % of them line up with the next (measured on bars
/// 7.5 to 10 mm wide, `docs/src/design/image-to-embroidery.md` › Wide columns).
pub const SPLIT_JITTER_PERCENT: f64 = 10.0;

/// `params`, a wide column's, with its stitches split as `REQ-AUTO-155` says: at the longest satin stitch, by
/// the default method, at a random phase with [`SPLIT_JITTER_PERCENT`] of jitter. A setting of the shape's
/// own stays: a person who split its stitches another way keeps that way.
pub(super) fn split(params: &mut ParamSet) {
    let jitter = SPLIT_JITTER_PERCENT.to_string();
    let longest = LONGEST_SATIN_STITCH.to_string();
    for (key, value) in [(MAX_STITCH_LENGTH, longest.as_str()), (SPLIT_METHOD, "default"), (RANDOM_SPLIT_PHASE, "true"), (RANDOM_SPLIT_JITTER_PERCENT, &jitter)] {
        if params.get(key).is_none() {
            params.set(key, value);
        }
    }
}

/// A wide part's columns, and what was measured of it.
pub(super) struct Wide {
    /// Its columns, in the order they are sewn: one satin column.
    pub(super) sewn: Vec<Sewn>,
    /// What was measured of it.
    pub(super) measures: Measures,
}

/// What a broad part is as a wide part.
pub(super) enum Tried {
    /// It sews as one satin column whose stitches are split.
    Wide(Wide),
    /// It isn't wide: it is wider than the bound somewhere, or no column of one even line, or no wide part can
    /// be, by its shape's settings. It is an area, as before.
    Not,
    /// Its column is refused, which this says: it is an area.
    Refused(Diagnostic),
}

/// What `part`, a broad part of the automatic area `id` held to `bounds` and sorted `by`, is as a wide part,
/// with `meter`'s work: its columns are drawn held to [`Bounds::wide`], and kept when they are one satin column
/// along one line of even width.
pub(super) fn tried(id: &ElementId, part: &Polygon, bounds: &Bounds, by: By<'_>, meter: &mut Meter) -> Result<Tried, Exhausted> {
    let (Some(split), Some(wide)) = (bounds.max_split, bounds.wide()) else { return Ok(Tried::Not) };
    if !matches!(bounds.asked, Asked::Auto | Asked::Satin) {
        return Ok(Tried::Not);
    }
    if within(part, wide.max_satin, meter)? != Some(true) {
        return Ok(Tried::Not);
    }
    Ok(match columns::columns(part, &wide, by.rules, &[], by.open, meter)? {
        Made::Columns { sewn, rule: RuleId::Column | RuleId::Asked, measures }
            if bounds.one_even_line(measures.lines, measures.unevenness)
                && sewn.iter().all(|sewn| matches!(sewn, Sewn::Satin { .. })) =>
        {
            Tried::Wide(Wide { sewn, measures })
        }
        Made::Crowded(most, _) => Tried::Refused(columns::piled(id, most)),
        Made::Unmade => Tried::Refused(unmade(id, split.get())),
        _ => Tried::Not,
    })
}

/// `SC-I0902` for a part of the automatic area `id` no wider than the widest split satin column, `split`
/// millimetres, along which no satin column can be drawn between its edges: it is filled.
fn unmade(id: &ElementId, split: f64) -> Diagnostic {
    let message = format!(
        "A part of this area is no wider than the widest split satin column, {} mm, but no satin column can be drawn along it between its \
         edges, so it is filled.",
        mm(split)
    );
    Diagnostic::new(Code::AutoSatinFilled, message)
        .with_element(id.clone())
        .with_fix(Fix::Hint("To sew it as a satin column, draw it with smoother edges, or draw the column with 2 rails and give it settings.".to_string()))
}

#[cfg(test)]
mod tests;
