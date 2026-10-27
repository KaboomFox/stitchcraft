//! Each automatic area reaching under the shapes sewn after it, or cut away under the areas that cover it
//! (`REQ-AUTO-007`), said with `SC-I0904`.
//!
//! Thread pulls stitches in from the edges of their shape, so shapes that only meet show fabric between
//! them. An automatic area therefore grows [`OVERLAP`] into each area, satin column or filled line sewn
//! after it, where it touches that shape, and no further than that shape. A stacked trace's lower colour runs
//! on under the upper ones, and sewn whole it would be sewn twice there, stiff and thick. So where an area
//! sewn after it covers it, it is cut away instead, [`OVERLAP`] inside that area's edge, and a part of it
//! that such areas cover whole sews nothing. A stacked trace then sews the areas of an abutting one. A
//! column or a line sewn after an area goes over it, and the area is sewn whole under it.
//!
//! The areas are taken from the last to the first, so that each one meets the shapes sewn after it as they
//! are sewn, with their own overlaps. For the area `a`:
//!
//! - **Cut away.** The areas sewn after it that come near it are joined. A part of `a` that they cover
//!   whole is left out, and they are shrunk by the overlap and taken from the rest. An area that only meets
//!   `a` is shrunk away from it and takes nothing.
//! - **Reaching under.** The shapes sewn after it are grown by [`TOUCHING`], so that a gap rounding left
//!   between edges drawn as one is bridged, and where `a` lies within them it touches them. `a` reaches
//!   into them within the overlap of there, outside what was cut away. So it reaches under a shape that
//!   comes near it without touching it only within the overlap of where it touches another, and a piece of
//!   that reach that doesn't touch what is left of `a` is left out, so that `a` gains no part of its own.
//!
//! Only the shapes whose bounds come within [`TOUCHING`] of `a`'s are looked at, each for a unit of work, so
//! an area costs the work of its neighbours, not of the whole design. The areas are named in the order they
//! are sewn.

use stitchcraft_core::{Budget, Code, Diagnostic, Fix, Meter};

use super::{Failed, Kind, OVERLAP, Shapes, TOUCHING, envelope, joined, near, no_area};
use crate::generators::mm;
use crate::normalize::region::Polygon;
use crate::normalize::region::ops::{area_of, buffered, difference, intersection, union};

/// The automatic areas of `shapes` reaching under the shapes sewn after them and cut away under the areas
/// that cover them, and `SC-I0904` for each area cut away whole, for each that needed more work than the
/// budget allows it and keeps the region it has, and for how many reach under others and how many are cut
/// away.
pub(super) fn reached(shapes: &mut Shapes, budget: &Budget) -> Vec<Diagnostic> {
    let (mut reaching, mut cut) = (0_usize, 0_usize);
    let mut said = Vec::new();
    for index in (0..shapes.len()).rev() {
        if shapes.region(index).is_none() {
            continue;
        }
        let Some(id) = shapes.id(index) else { continue };
        match beside(shapes, index, &mut budget.meter()) {
            Ok(Some(found)) => {
                reaching += usize::from(found.reaching);
                cut += usize::from(found.cut);
                if found.reaching || found.cut {
                    shapes.set_region(index, found.parts);
                }
            }
            Ok(None) => {
                let fix = Fix::Hint("To sew it as it is drawn, give it settings: in VectorCraft, apply a StitchCraft effect to it.".to_string());
                let message = "This area is covered by the areas sewn after it, so it sews nothing.";
                said.push(Diagnostic::new(Code::AutoUnder, message).with_element(id).with_fix(fix));
                shapes.set_region(index, Vec::new());
            }
            Err(failed) => {
                let (finding, consequence) =
                    ("where this area meets the shapes sewn after it", "it neither reaches under them nor is cut away under them");
                said.push(failed.said(Code::AutoUnder, finding, consequence, budget.max_work, id));
            }
        }
    }
    // Taken from the last to the first, they are named in the order they are sewn.
    said.reverse();
    said.extend(counted(reaching, cut));
    said
}

/// What became of an automatic area beside the shapes sewn after it.
struct Found {
    /// Its region.
    parts: Vec<Polygon>,
    /// Whether it reaches under a shape sewn after it.
    reaching: bool,
    /// Whether it is cut away under an area sewn after it.
    cut: bool,
}

/// The region of the automatic area `index` of `shapes` beside the shapes sewn after it, with `meter`'s
/// work; `None` when the areas sewn after it cover it whole.
fn beside(shapes: &mut Shapes, index: usize, meter: &mut Meter) -> Result<Option<Found>, Failed> {
    let area = shapes.region(index).map(<[Polygon]>::to_vec).unwrap_or_default();
    let Some(bounds) = envelope(&area) else { return Ok(Some(Found { parts: area, reaching: false, cut: false })) };
    // The areas sewn after it that come near it, and every area and column that does, grown to meet it.
    let (mut covering, mut touching) = (Vec::new(), Vec::new());
    for later in index + 1..shapes.len() {
        meter.charge(1)?;
        // A line goes over the area, which is sewn whole under it and reaches under none, so what it paints,
        // which only the order needs, isn't looked for.
        if shapes.kind(later) == Some(Kind::Line) || !shapes.bounds(later).is_some_and(|other| near(&bounds, TOUCHING, &other)) {
            continue;
        }
        if let Some((Kind::Area, parts)) = shapes.paint(later) {
            covering.extend_from_slice(parts);
        }
        touching.extend(shapes.grown(later, meter)?.into_iter().flatten());
    }
    // Every shape that comes near it is in `touching`, and the areas among them in `covering` too.
    if touching.is_empty() {
        return Ok(Some(Found { parts: area, reaching: false, cut: false }));
    }
    // What the areas sewn after it leave of it uncovered: none, and it sews nothing.
    let covered = joined(union([covering.as_slice()], meter))?;
    let uncovered = joined(difference(&area, &covered, meter))?;
    if no_area(area_of(&uncovered)) {
        return Ok(None);
    }
    // Cut away 0.4 mm inside their edges, but for its parts they cover whole, which only they sew. What is
    // left holds what they leave uncovered.
    let cut_zone = joined(buffered(&covered, -OVERLAP, meter))?;
    let left = attached(joined(difference(&area, &cut_zone, meter))?, &uncovered, meter)?;
    let cut = !no_area(area_of(&area) - area_of(&left));
    // Reaching 0.4 mm under the shapes sewn after it from where it touches them, outside what was cut away.
    let touching = joined(union([touching.as_slice()], meter))?;
    let contact = joined(intersection(&area, &touching, meter))?;
    let reach = joined(intersection(&joined(buffered(&contact, OVERLAP, meter))?, &touching, meter))?;
    let under = joined(difference(&joined(difference(&reach, &area, meter))?, &cut_zone, meter))?;
    if no_area(area_of(&under)) {
        return Ok(Some(Found { parts: left, reaching: false, cut }));
    }
    let reached = attached(joined(union([left.as_slice(), under.as_slice()], meter))?, &left, meter)?;
    let reaching = !no_area(area_of(&reached) - area_of(&left));
    Ok(Some(Found { parts: if reaching { reached } else { left }, reaching, cut }))
}

/// The parts of `parts` that overlap `of`.
fn attached(parts: Vec<Polygon>, of: &[Polygon], meter: &mut Meter) -> Result<Vec<Polygon>, Failed> {
    let mut kept = Vec::with_capacity(parts.len());
    for part in parts {
        if !no_area(area_of(&joined(intersection(std::slice::from_ref(&part), of, meter))?)) {
            kept.push(part);
        }
    }
    Ok(kept)
}

/// `SC-I0904` for `reaching` areas that reach under the shapes sewn after them and `cut` areas cut away
/// under the areas sewn after them; `None` when there are neither.
fn counted(reaching: usize, cut: usize) -> Option<Diagnostic> {
    let overlap = mm(OVERLAP);
    let reach = match reaching {
        0 => None,
        1 => Some(format!("1 area reaches {overlap} mm under the shapes sewn after it that it touches.")),
        n => Some(format!("{n} areas reach {overlap} mm under the shapes sewn after them that they touch.")),
    };
    let cut = match cut {
        0 => None,
        1 => Some(format!("1 area is cut away under the areas sewn after it, {overlap} mm inside their edges.")),
        n => Some(format!("{n} areas are cut away under the areas sewn after them, {overlap} mm inside their edges.")),
    };
    let message = [reach, cut].into_iter().flatten().collect::<Vec<_>>().join(" ");
    (!message.is_empty()).then(|| Diagnostic::new(Code::AutoUnder, message))
}
