//! The picture's background (`REQ-AUTO-009`), left out with `SC-W0906`.
//!
//! A traced picture's background is usually the fabric's own colour, and sewing it doubles the stitches
//! and stiffens the whole design. An image trace's background lies under every other shape and covers the
//! whole picture. So the lowest shape of a design is taken for the background when it is an automatic
//! area, a shape on another path than its own is drawn on it, and its outlines cover the box around the
//! design to within [`BACKGROUND_MARGIN`] of each side, its holes and notches aside ([`covers`]):
//!
//! - **Drawn on it.** A shape on the area's own path is the same object's: the readers make a fill and its
//!   stroke 2 elements on one path. A design of one shape with a stroke is the picture, and sews it.
//! - **Holes.** An abutting trace cuts a hole in the background for each shape drawn on it, and they are set
//!   aside.
//! - **Notches.** Where a shape touches the picture's edge, as the shapes of a tightly cropped picture do,
//!   the trace cuts a notch instead. A part of the box the outlines leave is a notch when it runs along them
//!   at least as far as along the box's sides, and the areas drawn on the lowest shape fill it, to within
//!   the distance shapes still meet across ([`TOUCHING`]). A part that runs further along the sides is no
//!   notch. A disc or a rounded square that reaches the sides leaves such parts in its corners, and is the
//!   picture, though the shapes in its corners fill the rest. The lengths are measured inside the margin, on
//!   the box less [`BACKGROUND_MARGIN`] on each side.
//!
//! The box is the one around the defining points of every shape's path ([`Design::bounds`]), the
//! background's own included, so that a lowest shape that covers less of the design, a disc in the corner
//! of a square say, is a part of the picture and is sewn. A background with settings is sewn as they say,
//! and so is one whose `auto_ignore_background` is off (`REQ-AUTO-020`).
//!
//! The background is found before anything is digitized, with the budget's work to itself. One whose
//! region its fill can't build within that work, or at all, is sewn, and its fill says why. One whose
//! outlines and notches need more work than that to compare with the box, or that the region code fails to
//! join, is sewn too, and `SC-W0906` or `SC-E0009` says so. An area drawn on it whose region can't be built
//! fills no notch, and its own fill says why.

use stitchcraft_core::{Budget, Code, Diagnostic, Exhausted, Fix, Meter, Point, Rect};

use super::{Failed, TOUCHING, envelope, joined, near, no_area, parts_of};
use crate::auto::AutoParams;
use crate::design::{Design, Element, FillRule, Path, Shape};
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops;
use crate::normalize::region::{Polygon, SAME};

/// How far inside each side of the box around the design a background's outlines may end, in millimetres:
/// what tracing leaves between the picture's edge and the background's.
pub(super) const BACKGROUND_MARGIN: f64 = 0.5;

/// The picture's background in `design`, whose lowest shape's settings of the rules are `rules`, none for a
/// shape with settings: `SC-W0906` with the index of the lowest shape, which is the background and is left
/// out, or without an index when finding out failed, so that it is sewn; `None` when the lowest shape is not
/// the background, or sews it.
pub(in crate::auto) fn background(design: &Design, rules: Option<&AutoParams>, budget: &Budget) -> Option<(Option<usize>, Diagnostic)> {
    let [lowest, drawn @ ..] = design.elements() else { return None };
    let Shape::Fill { path, rule } = &lowest.shape else { return None };
    if !rules.is_some_and(|rules| rules.auto_ignore_background) {
        return None;
    }
    let bounds = design.bounds()?;
    match covers(path, *rule, drawn, bounds, &mut budget.meter()) {
        Ok(true) => {
            let message = "This area covers the whole design under the other shapes, so it is left out as the picture's background.";
            let fix = Fix::Hint("To sew it, give it settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it.".to_string());
            Some((Some(0), Diagnostic::new(Code::AutoBackground, message).with_element(lowest.id.clone()).with_fix(fix)))
        }
        Ok(false) => None,
        Err(failed) => {
            let finding = "whether this area is the picture's background";
            Some((None, failed.said(Code::AutoBackground, finding, "it is sewn", budget.max_work, lowest.id.clone())))
        }
    }
}

/// Whether the area `path` bounds under `rule`, under the elements `drawn`, is the background of a design
/// whose box is `bounds`, with `meter`'s work ([the module](self)): a shape of `drawn` lies on another path,
/// and the area's outlines cover the box to within [`BACKGROUND_MARGIN`] of each side, its holes and notches
/// aside. A box that the margins leave empty, of a design less than twice the margin across, holds no
/// picture, and nothing covers it; neither does an area whose region can't be built, which its fill says.
pub(super) fn covers(path: &Path, rule: FillRule, drawn: &[Element], bounds: Rect, meter: &mut Meter) -> Result<bool, Failed> {
    if !others(path, drawn, meter)? {
        return Ok(false);
    }
    let Some(inner) = inside(bounds, BACKGROUND_MARGIN) else { return Ok(false) };
    let Ok(Ok(parts)) = parts_of(path, rule, meter) else { return Ok(false) };
    let outlines: Vec<Polygon> = parts.into_iter().map(|part| Polygon { outline: part.outline, holes: Vec::new() }).collect();
    // An outline inside another's hole lies inside that outline: joined, they are one region.
    let outlines = joined(ops::union([outlines.as_slice()], meter))?;
    let left = joined(ops::difference(std::slice::from_ref(&inner), &outlines, meter))?;
    if no_area(ops::area_of(&left)) {
        return Ok(true);
    }
    // What the outlines leave are notches, each running along them at least as far as along the box's sides.
    let sides = Envelope::of(&inner.outline);
    for part in &left {
        if no_area(part.area()) {
            continue;
        }
        let (along_sides, along_outlines) = lengths(part, &sides, meter)?;
        if along_outlines < along_sides {
            return Ok(false);
        }
    }
    filled(left, drawn, meter)
}

/// Whether a shape of `drawn` lies on another path than `path`, with `meter`'s work: a unit for each subpath
/// and segment of `path` for each shape compared with it. A shape on `path`, the area's own stroke say, is
/// part of the same object, and no shape drawn on it.
pub(super) fn others(path: &Path, drawn: &[Element], meter: &mut Meter) -> Result<bool, Exhausted> {
    let size: usize = path.subpaths.iter().map(|subpath| 1 + subpath.segments.len()).sum();
    for element in drawn {
        meter.charge(size as u64)?;
        if element.shape.path() != path {
            return Ok(true);
        }
    }
    Ok(false)
}

/// The box `bounds` less `by` on each side, wound clockwise in y-up axes as a part is; `None` when that
/// leaves nothing.
fn inside(bounds: Rect, by: f64) -> Option<Polygon> {
    let (low, high) = (bounds.min(), bounds.max());
    let (x0, y0, x1, y1) = (low.x() + by, low.y() + by, high.x() - by, high.y() - by);
    if x0 >= x1 || y0 >= y1 {
        return None;
    }
    let corners = [(x0, y0), (x0, y1), (x1, y1), (x1, y0), (x0, y0)].map(|(x, y)| Point::new(x, y));
    let outline = corners.into_iter().collect::<Result<Vec<Point>, _>>().ok()?;
    Some(Polygon { outline, holes: Vec::new() })
}

/// How far `part`, a part of the box whose sides are `sides` that a lowest shape's outlines leave, runs along
/// the sides, and how far along the outlines, in millimetres, with `meter`'s work: a unit for each of its
/// edges. An edge on a side, both its ends on it, runs along the side; any other along the outlines.
pub(super) fn lengths(part: &Polygon, sides: &Envelope, meter: &mut Meter) -> Result<(f64, f64), Exhausted> {
    let on = |u: f64, v: f64, side: f64| (u - side).abs() <= SAME && (v - side).abs() <= SAME;
    let on_side = |a: Point, b: Point| {
        on(a.x(), b.x(), sides.min_x) || on(a.x(), b.x(), sides.max_x) || on(a.y(), b.y(), sides.min_y) || on(a.y(), b.y(), sides.max_y)
    };
    let (mut along_sides, mut along_outlines) = (0.0, 0.0);
    for ring in part.rings() {
        for edge in ring.windows(2) {
            meter.charge(1)?;
            let [a, b] = edge else { continue };
            let length = stitchcraft_core::math::hypot(b.x() - a.x(), b.y() - a.y());
            if on_side(*a, *b) { along_sides += length } else { along_outlines += length }
        }
    }
    Ok((along_sides, along_outlines))
}

/// Whether the areas of `drawn` fill the parts `left`, with `meter`'s work: a unit for each element, one for
/// each point of those whose bounds are looked at, and the work of building and growing the regions of those
/// near the parts. Each is grown by [`TOUCHING`], so that a gap rounding left between edges drawn as one is
/// bridged, as the steps after this one grow the shapes they meet. One whose region can't be built fills
/// nothing.
fn filled(mut left: Vec<Polygon>, drawn: &[Element], meter: &mut Meter) -> Result<bool, Failed> {
    let Some(around) = envelope(&left) else { return Ok(true) };
    for element in drawn {
        meter.charge(1)?;
        let Shape::Fill { path, rule } = &element.shape else { continue };
        let points: Vec<Point> = path.points().collect();
        meter.charge(points.len() as u64)?;
        if points.is_empty() || !near(&around, TOUCHING, &Envelope::of(&points)) {
            continue;
        }
        let Ok(area) = parts_of(path, *rule, meter)? else { continue };
        let grown = joined(ops::buffered(&area, TOUCHING, meter))?;
        left = joined(ops::difference(&left, &grown, meter))?;
        if no_area(ops::area_of(&left)) {
            return Ok(true);
        }
    }
    Ok(false)
}
