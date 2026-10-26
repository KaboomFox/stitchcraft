//! Automatic areas beside the shapes around them (`REQ-AUTO-007` to `REQ-AUTO-009`). Design:
//! `docs/src/design/image-to-embroidery.md` › Neighbours.
//!
//! A traced picture is shapes of flat colour that meet: abutting, as VectorCraft's Abutting method traces
//! them, each area with a hole where the next is, or stacked, each colour a whole shape over the ones under
//! it. Sewn as drawn, they sew badly in 3 ways, and digitizing mends each in its own step, in this order:
//!
//! 1. **The background** ([`background()`]). The lowest shape is often the picture's background, the
//!    fabric's own colour. It is left out before anything is digitized (`SC-W0906`).
//! 2. **Specks** ([`specks`]). Noise in the picture leaves areas of a pixel or two. A fill smaller than
//!    [`SMALLEST_AREA`] sews a knot, and a hole where it was shows fabric. Each speck goes into the
//!    automatic area it shares the longest edge with, or is left out when it shares none (`SC-I0905`).
//! 3. **Overlaps** ([`under`]). Thread pulls stitches in from the edges of their shape, so shapes that
//!    only meet show fabric between them. Each automatic area reaches [`OVERLAP`] under the shapes sewn
//!    after it, where it touches them. Where an area sewn after it covers it, as a stacked trace's upper
//!    colours cover the lower ones, it is cut away instead, [`OVERLAP`] inside that area's edge, so that it
//!    is sewn once (`SC-I0904`).
//!
//! Only automatic areas change: the fills automatic digitizing made, of shapes without settings. A shape
//! with settings is sewn as they say, and a column or a line sewn after an area goes over it. The regions
//! are combined with [`crate::normalize::region::ops`]. An area whose region changed is given its new
//! outline, and one that didn't keeps its path as drawn. A fill that bounds no area is left to its fill,
//! which says so.
//!
//! **Work.** Each automatic area has the budget's work to itself for its specks and for its overlaps, and
//! costs the work of the shapes near it, and a unit for each other shape it looks past. What another shape
//! paints is found once, with the budget's work to itself, as each automatic area's region is built. An
//! area whose neighbours need more work than its budget allows, or whose regions the region code fails to
//! join (a bug, `SC-E0009`), keeps the region it has, and that is said. A shape whose paint can't be found
//! within its budget paints nothing for its neighbours, and says so itself, since its generator needs that
//! work too: all but the area between a satin column's rails, which only the neighbours need, and which is
//! said when it can't be found.

mod background;
mod specks;
mod under;

pub(super) use background::background;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Meter, Point};
use stitchcraft_params::{Family, StitchType, Validated};

use super::outline;
use crate::design::{DesignSettings, Element, FillRule, Path, Shape};
use crate::generate::stitch_type_of;
use crate::generators::satin::{self, SatinParams};
use crate::normalize::region::geom::{self, Envelope};
use crate::normalize::region::ops::{self, Parts};
use crate::normalize::region::{self, Polygon};
use crate::normalize::stroke;

/// How far an automatic area reaches under the shapes sewn after it that it touches, and how far inside the
/// edge of an area sewn after it that covers it the area is cut away, in millimetres. MC-8 confirms or
/// changes it on the machine (`docs/src/plan/machine-testing.md`).
pub const OVERLAP: f64 = 0.4;

/// The smallest area sewn, in square millimetres: an automatic area, or a part of one, smaller than this
/// is a speck. MC-8 confirms or changes it.
pub const SMALLEST_AREA: f64 = 1.0;

/// How far apart 2 shapes may be and still touch, in millimetres: twice the tolerance their curves are
/// flattened to, so that a gap rounding left between edges drawn as one is closed.
pub const TOUCHING: f64 = 2.0 * stroke::SHAPE_TOLERANCE;

/// An area this small or smaller, in square millimetres, is no area: a sliver that rounding leaves where
/// regions only touch.
const NO_AREA: f64 = 1e-6;

/// Whether `area` square millimetres is no area ([`NO_AREA`]).
fn no_area(area: f64) -> bool {
    area <= NO_AREA
}

/// An element as automatic digitizing sews it.
pub(super) struct Sewn {
    /// The element.
    pub(super) element: Element,
    /// Whether it is an automatic area: a fill that automatic digitizing made.
    pub(super) area: bool,
}

/// `sewn`, its specks merged into their neighbours and its areas reaching under the shapes sewn after them,
/// and what is said of them.
pub(super) fn beside(sewn: Vec<Sewn>, settings: &DesignSettings, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let mut shapes = Shapes::new(sewn, settings, budget);
    let mut said = specks::merged(&mut shapes, budget);
    said.extend(under::reached(&mut shapes, budget));
    said.append(&mut shapes.said);
    (shapes.into_elements(), said)
}

/// What a shape sewn after an automatic area is to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// An area: the automatic area is cut away under it where it covers the automatic area, and reaches
    /// under it where they meet.
    Area,
    /// A satin column: the automatic area reaches under it where they meet, and is sewn whole under it.
    Column,
}

/// Why a step could not be taken.
#[derive(Debug)]
enum Failed {
    /// It needed more work than the budget allows it.
    Work,
    /// The region code could not join the edges of a region into rings (a bug).
    Unjoined,
}

impl From<Exhausted> for Failed {
    fn from(_: Exhausted) -> Self {
        Failed::Work
    }
}

impl Failed {
    /// What is said of the element `id` when finding `finding` failed, so that `consequence`: the `work`
    /// a step may use, under `code`, or `SC-E0009` for a region not joined.
    fn said(&self, code: Code, finding: &str, consequence: &str, work: u64, id: ElementId) -> Diagnostic {
        let diagnostic = match self {
            Failed::Work => Diagnostic::new(code, format!("Finding {finding} needed more than the work budget of {work} units, so {consequence}.")),
            Failed::Unjoined => Diagnostic::new(
                Code::InternalCheckFailed,
                format!("Finding {finding}, the edges of a region could not be joined into rings, so {consequence}. This is a bug in StitchCraft."),
            ),
        };
        diagnostic.with_element(id)
    }
}

/// The parts of a region, or why there are none.
fn joined(parts: Result<Parts, Exhausted>) -> Result<Vec<Polygon>, Failed> {
    parts?.map_err(|_| Failed::Unjoined)
}

/// What an element paints, as a shape sewn after an automatic area: its kind and region, or `None` for a
/// line, and for a shape whose region cannot be built.
type Painted = Option<(Kind, Vec<Polygon>)>;

/// The elements of a design as automatic digitizing sews them, with the regions their neighbours need.
struct Shapes<'s> {
    elements: Vec<Element>,
    /// Whether each element is an automatic area.
    area: Vec<bool>,
    /// Each automatic area's region as it now is: `None` when it could not be built, or bounds no area,
    /// which leaves the area as it is drawn.
    regions: Vec<Option<Vec<Polygon>>>,
    /// Whether each automatic area's region changed, so that it is given its new outline.
    changed: Vec<bool>,
    /// Whether each element is left out.
    removed: Vec<bool>,
    /// What each element that is no automatic area paints, once found.
    painted: Vec<Option<Painted>>,
    /// The bounds of what each element paints, once found.
    bounds: Vec<Option<Option<Envelope>>>,
    /// What each element paints grown by [`TOUCHING`], once found.
    grown: Vec<Option<Vec<Polygon>>>,
    /// What is said of the shapes whose paint could not be found, which only the neighbours need.
    said: Vec<Diagnostic>,
    settings: &'s DesignSettings,
    /// The budget each shape's paint is found with.
    budget: Budget,
}

impl<'s> Shapes<'s> {
    /// The elements of `sewn`, each automatic area's region built with `budget`'s work to itself. An area
    /// that needs more, whose region the region code fails to join, or that bounds no area, is sewn as it is
    /// drawn, and its fill says what it says of it then.
    fn new(sewn: Vec<Sewn>, settings: &'s DesignSettings, budget: &Budget) -> Self {
        let count = sewn.len();
        let (elements, area): (Vec<Element>, Vec<bool>) = sewn.into_iter().map(|sewn| (sewn.element, sewn.area)).unzip();
        let regions = elements
            .iter()
            .zip(&area)
            .map(|(element, &area)| match &element.shape {
                Shape::Fill { path, rule } if area => parts_of(path, *rule, &mut budget.meter()).ok()?.ok().filter(|parts| !parts.is_empty()),
                _ => None,
            })
            .collect();
        Shapes {
            elements,
            area,
            regions,
            changed: vec![false; count],
            removed: vec![false; count],
            painted: vec![None; count],
            bounds: vec![None; count],
            grown: vec![None; count],
            said: Vec::new(),
            settings,
            budget: *budget,
        }
    }

    /// How many elements there are.
    fn len(&self) -> usize {
        self.elements.len()
    }

    /// The id of element `index`.
    fn id(&self, index: usize) -> Option<ElementId> {
        self.elements.get(index).map(|element| element.id.clone())
    }

    /// The region of the automatic area `index` as it now is, unless it has none or is left out.
    fn region(&self, index: usize) -> Option<&[Polygon]> {
        if self.removed.get(index).copied().unwrap_or(true) {
            return None;
        }
        self.regions.get(index)?.as_deref()
    }

    /// Gives the automatic area `index` the region `parts`: none leaves it out, and some bring it back.
    fn set_region(&mut self, index: usize, parts: Vec<Polygon>) {
        if let Some(removed) = self.removed.get_mut(index) {
            *removed = parts.is_empty();
        }
        if let (Some(region), Some(changed)) = (self.regions.get_mut(index), self.changed.get_mut(index)) {
            (*region, *changed) = (Some(parts), true);
        }
        if let (Some(bounds), Some(grown)) = (self.bounds.get_mut(index), self.grown.get_mut(index)) {
            (*bounds, *grown) = (None, None);
        }
    }

    /// What element `index` paints, as a shape sewn after an automatic area, once [`Self::find`] found it:
    /// an automatic area its region as it now is, another fill its region, a satin column the area between
    /// its rails. `None` for a line, for a shape left out, and for a shape whose region cannot be built.
    fn paint(&self, index: usize) -> Option<(Kind, &[Polygon])> {
        if self.area.get(index).copied().unwrap_or(false) {
            return self.region(index).map(|parts| (Kind::Area, parts));
        }
        // Only automatic areas are left out, so what another element paints is there to find.
        let (kind, parts) = self.painted.get(index)?.as_ref()?.as_ref()?;
        Some((*kind, parts.as_slice()))
    }

    /// Finds what element `index` paints, with the budget's work to itself, unless it is an automatic area,
    /// whose region is known, or was found before.
    fn find(&mut self, index: usize) {
        if self.area.get(index).copied().unwrap_or(true) || self.painted.get(index).is_some_and(Option::is_some) {
            return;
        }
        let Some(element) = self.elements.get(index) else { return };
        let mut meter = self.budget.meter();
        let painted = match stitch_type_of(element, self.settings).map_or(Ok(None), |kind| paints(element, kind, &mut meter)) {
            Ok(painted) => painted,
            Err(failed) => {
                let consequence = "the areas sewn before it don't reach under it";
                self.said.push(failed.said(Code::AutoUnder, "what this satin column covers", consequence, self.budget.max_work, element.id.clone()));
                None
            }
        };
        if let Some(slot) = self.painted.get_mut(index) {
            *slot = Some(painted);
        }
    }

    /// The bounds of what element `index` paints.
    fn bounds(&mut self, index: usize) -> Option<Envelope> {
        if let Some(Some(known)) = self.bounds.get(index) {
            return *known;
        }
        self.find(index);
        let found = self.paint(index).and_then(|(_, parts)| envelope(parts));
        if let Some(slot) = self.bounds.get_mut(index) {
            *slot = Some(found);
        }
        found
    }

    /// What element `index` paints, grown by [`TOUCHING`] with `meter`'s work, so that a shape that comes
    /// that near meets it.
    fn grown(&mut self, index: usize, meter: &mut Meter) -> Result<Option<Vec<Polygon>>, Failed> {
        if let Some(Some(grown)) = self.grown.get(index) {
            return Ok(Some(grown.clone()));
        }
        self.find(index);
        let Some((_, parts)) = self.paint(index) else { return Ok(None) };
        let grown = joined(ops::buffered(parts, TOUCHING, meter))?;
        if let Some(slot) = self.grown.get_mut(index) {
            *slot = Some(grown.clone());
        }
        Ok(Some(grown))
    }

    /// The elements as they are sewn: each automatic area whose region changed given its outline, and the
    /// elements left out left out.
    fn into_elements(self) -> Vec<Element> {
        let Shapes { elements, regions, changed, removed, .. } = self;
        elements
            .into_iter()
            .zip(regions)
            .zip(changed.into_iter().zip(removed))
            .filter_map(|((element, region), (changed, removed))| match (removed, changed, region) {
                (true, ..) => None,
                (false, true, Some(parts)) => Some(Element { shape: Shape::Fill { path: outline(&parts), rule: FillRule::NonZero }, ..element }),
                _ => Some(element),
            })
            .collect()
    }
}

/// What `element`, sewn as `stitch_type`, paints for the automatic areas sewn before it, with `meter`'s
/// work: a fill its region, a satin column the area between its rails as they are sewn, swapped and turned
/// as its settings say, and nothing for any other stitch type. A fill whose region, or a column whose
/// settings or rails, can't be read or built within the work paints nothing, and says why itself, since its
/// generator needs them too.
fn paints(element: &Element, stitch_type: StitchType, meter: &mut Meter) -> Result<Painted, Failed> {
    match (stitch_type.family(), &element.shape) {
        (Family::Fill, Shape::Fill { path, rule }) => Ok(parts_of(path, *rule, meter).ok().and_then(Result::ok).map(|parts| (Kind::Area, parts))),
        (Family::Satin, Shape::Stroke { path, width, join, .. }) => {
            let Ok(Validated { params, .. }) = SatinParams::from_set_as(&element.params, stitch_type) else { return Ok(None) };
            let Ok(Some(column)) = satin::rails(path, *width, *join, &mut Vec::new(), meter) else { return Ok(None) };
            let Ok(rails) = satin::sewn_rails(&column, &params, meter) else { return Ok(None) };
            Ok(Some((Kind::Column, joined(between(&rails, meter))?)))
        }
        _ => Ok(None),
    }
}

/// The region that `path` bounds under `rule`, every part kept, however small: the area a fill's region is
/// built from ([`region::build`]), before the parts too small for a row are left out.
fn parts_of(path: &Path, rule: FillRule, meter: &mut Meter) -> Result<Parts, Exhausted> {
    let flat = stroke::flatten(path, stroke::SHAPE_TOLERANCE, meter)?;
    let rings: Vec<Vec<Point>> =
        flat.pieces.into_iter().map(|piece| geom::closed(&piece.points)).filter(|ring| geom::without_repeats(ring).len() >= 4).collect();
    Ok(region::area(&rings, |w| region::fills(rule, w), meter)?.parts)
}

/// The area between a satin column's `rails`, running the same way as they are sewn: the ring along the
/// first and back along the second. Rails that close make it a ring round the first, over to the second's
/// start, round the second the other way and back, which bounds the ring between them: the stretch over
/// and back adds nothing.
fn between(rails: &[Vec<Point>; 2], meter: &mut Meter) -> Result<Parts, Exhausted> {
    let [first, second] = rails;
    let ring: Vec<Point> = first.iter().chain(second.iter().rev()).copied().collect();
    Ok(region::area(&[ring], |w| w != 0, meter)?.parts)
}

/// The bounds of `parts`' outlines; `None` for no parts.
fn envelope(parts: &[Polygon]) -> Option<Envelope> {
    let points: Vec<Point> = parts.iter().flat_map(|part| part.outline.iter().copied()).collect();
    (!points.is_empty()).then(|| Envelope::of(&points))
}

/// Whether bounds `a`, grown by `by` on every side, and bounds `b` overlap, edges included.
fn near(a: &Envelope, by: f64, b: &Envelope) -> bool {
    a.min_x - by <= b.max_x && b.min_x <= a.max_x + by && a.min_y - by <= b.max_y && b.min_y <= a.max_y + by
}

#[cfg(test)]
mod tests;
