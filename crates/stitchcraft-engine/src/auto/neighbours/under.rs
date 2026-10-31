//! Each automatic area reaching under the shapes sewn after it, or cut away under the areas that cover it
//! (`REQ-AUTO-007`), said with `SC-I0904`.
//!
//! Thread pulls stitches in from the edges of their shape, so shapes that only meet show fabric between
//! them. An automatic area therefore grows by its overlap into each area, satin column or filled line sewn
//! after it, where it touches that shape, and no further than that shape. A stacked trace's lower colour runs
//! on under the upper ones, and sewn whole it would be sewn twice there, stiff and thick. So where an area
//! sewn after it covers it, it is cut away instead, its overlap inside that area's edge, and a part of it
//! that such areas cover whole sews nothing. A stacked trace then sews the areas of an abutting one. A
//! column or a line sewn after an area goes over it, and the area is sewn whole under it. Each area's
//! overlap is its own setting, `auto_overlap_mm` (`REQ-AUTO-020`).
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
//! A part's line or column (`auto::narrow`, `auto::columns`) is reached under as a satin column is. It isn't
//! cut away: one that the areas sewn after it cover whole sews nothing, as such an area doesn't, and one they
//! cover in part sews whole.
//!
//! Only the shapes whose bounds come within [`TOUCHING`] of `a`'s are looked at, each for a unit of work, so
//! an area costs the work of its neighbours, not of the whole design. The areas are named in the order they
//! are sewn.

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Meter};

use super::{Failed, Kind, Shapes, TOUCHING, envelope, joined, near, no_area};
use crate::generators::mm;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops::{area_of, buffered, difference, intersection, union};

/// The automatic areas of `shapes` reaching under the shapes sewn after them and cut away under the areas
/// that cover them, and `SC-I0904` for each area cut away whole, for each that needed more work than the
/// budget allows it and keeps the region it has, and for how many reach under others and how many are cut
/// away.
pub(super) fn reached(shapes: &mut Shapes, budget: &Budget) -> Vec<Diagnostic> {
    // The overlaps of the areas that reach under others, and of those cut away.
    let (mut reaching, mut cut) = (Vec::new(), Vec::new());
    let mut said = Vec::new();
    for index in (0..shapes.len()).rev() {
        if shapes.kind(index) == Some(Kind::Part) {
            said.extend(hidden(shapes, index, budget));
            continue;
        }
        if shapes.region(index).is_none() {
            continue;
        }
        // An area with a region is automatic, and has its settings of the rules.
        let (Some(id), Some(rules)) = (shapes.id(index), shapes.rules(index)) else { continue };
        let overlap = rules.auto_overlap_mm.get();
        match beside(shapes, index, overlap, &mut budget.meter()) {
            Ok(Some(found)) => {
                reaching.extend(found.reaching.then_some(overlap));
                cut.extend(found.cut.then_some(overlap));
                if found.reaching || found.cut {
                    shapes.set_region(index, found.parts);
                }
            }
            Ok(None) => {
                said.push(covered(id));
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
    said.extend(counted(&reaching, &cut));
    said
}

/// `SC-I0904` for the shape `id`, covered whole by the areas sewn after it, which sews nothing.
fn covered(id: ElementId) -> Diagnostic {
    let fix =
        Fix::Hint("To sew it as it is drawn, give it settings: in VectorCraft, apply StitchCraft's Fill, Stroke or Satin effect to it.".to_string());
    Diagnostic::new(Code::AutoUnder, "This area is covered by the areas sewn after it, so it sews nothing.").with_element(id).with_fix(fix)
}

/// Leaves out the part's line or column `index` of `shapes` when the areas sewn after it cover the part whole,
/// with the budget's work, and says so; one that needs more work than that is sewn whole, and that is said.
fn hidden(shapes: &mut Shapes, index: usize, budget: &Budget) -> Option<Diagnostic> {
    let id = shapes.id(index)?;
    match covered_whole(shapes, index, &mut budget.meter()) {
        Ok(false) => None,
        Ok(true) => {
            shapes.leave_out(index);
            Some(covered(id))
        }
        Err(failed) => Some(failed.said(Code::AutoUnder, "where this area meets the shapes sewn after it", "it is sewn whole", budget.max_work, id)),
    }
}

/// Whether the areas sewn after the part's line or column `index` of `shapes` cover the part whole, with
/// `meter`'s work.
fn covered_whole(shapes: &mut Shapes, index: usize, meter: &mut Meter) -> Result<bool, Failed> {
    let Some(part) = shapes.paint(index).map(|(_, parts)| parts.to_vec()) else { return Ok(false) };
    let Some(bounds) = envelope(&part) else { return Ok(false) };
    let (covering, _) = later(shapes, index, &bounds, false, meter)?;
    Ok(!covering.is_empty() && no_area(area_of(&uncovered(&part, &covering, meter)?)))
}

/// The areas sewn after element `index` of `shapes` that come within [`TOUCHING`] of `bounds`, and when
/// `grown`, every area, column and part's line or column that does, grown to meet it, with `meter`'s work: a
/// unit for each shape looked at, and the work of growing those near.
fn later(shapes: &mut Shapes, index: usize, bounds: &Envelope, grown: bool, meter: &mut Meter) -> Result<(Vec<Polygon>, Vec<Polygon>), Failed> {
    let (mut covering, mut touching) = (Vec::new(), Vec::new());
    for later in index + 1..shapes.len() {
        meter.charge(1)?;
        // A line goes over the area, which is sewn whole under it and reaches under none, so what it paints,
        // which only the order needs, isn't looked for.
        if shapes.kind(later) == Some(Kind::Line) || !shapes.bounds(later).is_some_and(|other| near(bounds, TOUCHING, &other)) {
            continue;
        }
        if let Some((Kind::Area, parts)) = shapes.paint(later) {
            covering.extend_from_slice(parts);
        }
        if grown {
            touching.extend(shapes.grown(later, meter)?.into_iter().flatten());
        }
    }
    Ok((covering, touching))
}

/// What `covering`, the areas sewn after a shape, leave uncovered of `part`, with `meter`'s work.
fn uncovered(part: &[Polygon], covering: &[Polygon], meter: &mut Meter) -> Result<Vec<Polygon>, Failed> {
    let covered = joined(union([covering], meter))?;
    joined(difference(part, &covered, meter))
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

/// The region of the automatic area `index` of `shapes` beside the shapes sewn after it, with its `overlap`
/// and `meter`'s work; `None` when the areas sewn after it cover it whole.
fn beside(shapes: &mut Shapes, index: usize, overlap: f64, meter: &mut Meter) -> Result<Option<Found>, Failed> {
    let area = shapes.region(index).map(<[Polygon]>::to_vec).unwrap_or_default();
    let Some(bounds) = envelope(&area) else { return Ok(Some(Found { parts: area, reaching: false, cut: false })) };
    // The areas sewn after it that come near it, and every area, column and part's line or column that does,
    // grown to meet it.
    let (covering, touching) = later(shapes, index, &bounds, true, meter)?;
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
    // Cut away the overlap inside their edges, but for its parts they cover whole, which only they sew. What
    // is left holds what they leave uncovered.
    let cut_zone = joined(buffered(&covered, -overlap, meter))?;
    let left = attached(joined(difference(&area, &cut_zone, meter))?, &uncovered, meter)?;
    let cut = !no_area(area_of(&area) - area_of(&left));
    // Reaching the overlap under the shapes sewn after it from where it touches them, outside what was cut
    // away.
    let touching = joined(union([touching.as_slice()], meter))?;
    let contact = joined(intersection(&area, &touching, meter))?;
    let reach = joined(intersection(&joined(buffered(&contact, overlap, meter))?, &touching, meter))?;
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

/// `SC-I0904` for the areas that reach under the shapes sewn after them, by the overlaps in `reaching`, and
/// the areas cut away under the areas sewn after them, by the overlaps in `cut`; `None` when there are
/// neither.
fn counted(reaching: &[f64], cut: &[f64]) -> Option<Diagnostic> {
    let reach = match reaching {
        [] => None,
        [one] => Some(format!("1 area reaches {} mm under the shapes sewn after it that it touches.", mm(*one))),
        many => Some(format!("{} areas reach {} mm under the shapes sewn after them that they touch.", many.len(), spread(many))),
    };
    let cut = match cut {
        [] => None,
        [one] => Some(format!("1 area is cut away under the areas sewn after it, {} mm inside their edges.", mm(*one))),
        many => Some(format!("{} areas are cut away under the areas sewn after them, {} mm inside their edges.", many.len(), spread(many))),
    };
    let message = [reach, cut].into_iter().flatten().collect::<Vec<_>>().join(" ");
    (!message.is_empty()).then(|| Diagnostic::new(Code::AutoUnder, message))
}

/// The overlaps `overlaps`, at least one, as a message gives them in millimetres: the one they share, or the
/// least to the most.
fn spread(overlaps: &[f64]) -> String {
    let least = overlaps.iter().copied().fold(f64::INFINITY, f64::min);
    let most = overlaps.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (least, most) = (mm(least), mm(most));
    if least == most { least } else { format!("{least} to {most}") }
}
