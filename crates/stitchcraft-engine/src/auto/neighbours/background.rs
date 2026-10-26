//! The picture's background (`REQ-AUTO-009`), left out with `SC-W0906`.
//!
//! A traced picture's background is usually the fabric's own colour, and sewing it doubles the stitches
//! and stiffens the whole design. An image trace's background lies under every other shape and covers the
//! whole picture. So the lowest shape of a design is taken for the background when it is an automatic
//! area, other shapes are drawn on it, and its outlines cover the box around the design to within
//! [`BACKGROUND_MARGIN`] of each side. Its holes are set aside, since an abutting trace cuts a hole in it
//! for each shape drawn on it. The box is the one around the defining points of every shape's path
//! ([`Design::bounds`]), the background's own included, so that a lowest shape that covers less of the
//! design, a disc in a square say, is a part of the picture and is sewn. A background with settings is
//! sewn as they say.
//!
//! The background is found before anything is digitized, with the budget's work to itself. One whose
//! region its fill can't build within that work, or at all, is sewn, and its fill says why. One whose
//! outlines need more work than that to compare with the box, or that the region code fails to join, is
//! sewn too, and `SC-W0906` or `SC-E0009` says so.

use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Meter, Point, Rect};

use super::{Failed, joined, no_area, parts_of};
use crate::design::{Design, FillRule, Path, Shape};
use crate::normalize::region::Polygon;
use crate::normalize::region::ops;

/// How far inside each side of the box around the design a background's outlines may end, in millimetres:
/// what tracing leaves between the picture's edge and the background's.
pub(super) const BACKGROUND_MARGIN: f64 = 0.5;

/// The picture's background in `design`: `SC-W0906` with the index of the lowest shape, which is the
/// background and is left out, or without an index when finding out failed, so that it is sewn; `None`
/// when the lowest shape is not the background.
pub(in crate::auto) fn background(design: &Design, budget: &Budget) -> Option<(Option<usize>, Diagnostic)> {
    let [lowest, _, ..] = design.elements() else { return None };
    let Shape::Fill { path, rule } = &lowest.shape else { return None };
    if !lowest.automatic {
        return None;
    }
    let bounds = design.bounds()?;
    match covers(path, *rule, bounds, &mut budget.meter()) {
        Ok(true) => {
            let message = "This area covers the whole design under the other shapes, so it is left out as the picture's background.";
            let fix = Fix::Hint("To sew it, give it settings: in VectorCraft, apply a StitchCraft effect to it.".to_string());
            Some((Some(0), Diagnostic::new(Code::AutoBackground, message).with_element(lowest.id.clone()).with_fix(fix)))
        }
        Ok(false) => None,
        Err(failed) => {
            let finding = "whether this area is the picture's background";
            Some((None, failed.said(Code::AutoBackground, finding, "it is sewn", budget.max_work, lowest.id.clone())))
        }
    }
}

/// Whether the outlines of the area `path` bounds under `rule`, its holes aside, cover `bounds` to within
/// [`BACKGROUND_MARGIN`] of each side, with `meter`'s work. A box that the margins leave empty, of a design
/// less than twice the margin across, holds no picture, and nothing covers it; neither does an area whose
/// region can't be built, which its fill says.
pub(super) fn covers(path: &Path, rule: FillRule, bounds: Rect, meter: &mut Meter) -> Result<bool, Failed> {
    let (low, high) = (bounds.min(), bounds.max());
    let (x0, y0, x1, y1) = (low.x() + BACKGROUND_MARGIN, low.y() + BACKGROUND_MARGIN, high.x() - BACKGROUND_MARGIN, high.y() - BACKGROUND_MARGIN);
    if x0 >= x1 || y0 >= y1 {
        return Ok(false);
    }
    let Ok(Ok(parts)) = parts_of(path, rule, meter) else { return Ok(false) };
    let outlines: Vec<Polygon> = parts.into_iter().map(|part| Polygon { outline: part.outline, holes: Vec::new() }).collect();
    // An outline inside another's hole lies inside that outline: joined, they are one region.
    let outlines = joined(ops::union([outlines.as_slice()], meter))?;
    let corners = [(x0, y0), (x0, y1), (x1, y1), (x1, y0), (x0, y0)].map(|(x, y)| Point::new(x, y));
    let Ok(outline) = corners.into_iter().collect::<Result<Vec<Point>, _>>() else { return Ok(false) };
    // The box, wound clockwise in y-up axes as a part is.
    let inner = [Polygon { outline, holes: Vec::new() }];
    Ok(no_area(ops::area_of(&joined(ops::difference(&inner, &outlines, meter))?)))
}
