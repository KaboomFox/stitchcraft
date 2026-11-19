//! Each automatic area reaching under the shapes sewn after it, or cut away under the areas that cover it
//! (`REQ-AUTO-007`), said with `SC-I0904`.
//!
//! Thread pulls stitches in from the edges of their shape, so shapes that only meet show fabric between
//! them. An automatic area therefore grows by its overlap into each area, satin column or filled line sewn
//! after it, where it touches that shape, and no further than that shape. An area digitized whole under its
//! details (`layers`) runs on under them, and sewn whole it would be sewn twice there, stiff and thick. So
//! where an area or a part's line or column sewn after it covers it, it is cut away instead, its overlap
//! inside that shape's edge, and a part of it that such shapes cover whole sews nothing. A satin column or a
//! line digitized from a stroke and sewn after an area goes over it, and the area is sewn whole under it.
//! Each area's overlap is its own setting, `auto_overlap_mm` (`REQ-AUTO-020`).
//!
//! The overlap is thread under thread, and where several lie on each other a design sews thick
//! (`REQ-AUTO-067`, `docs/src/design/image-to-embroidery.md` › Less thread where areas overlap). So an area
//! reaches no further than the thread pulls ([`Reach`]): the whole overlap along its rows, whose ends the
//! thread pulls in, and its pull compensation across them. Under a part of an area narrower than [`WIDE`]
//! overlaps it reaches [`UNDER_NARROW`] of that, so that the areas on both sides of a thin part don't fill
//! it between them. And where 2 areas would both sew under a shape sewn after them, only the one sewn first
//! does ([`parted`]). Under a satin column and under a part's line or column an area reaches its whole
//! overlap whichever way its rows run, as it did: their thread doesn't cover their ends, their bends or
//! their part's width as a fill's rows cover its area, and with half the reach there fabric showed
//! (measured on the traced dogs). Across the part of a running line it reaches on under the shape beyond
//! from where it meets that shape there. The area beyond gives the line's part up to it, and their overlap
//! is whole all the same.
//!
//! The areas are taken from the last to the first, so that each one meets the shapes sewn after it as they
//! are sewn, with their own overlaps. For the area `a`:
//!
//! - **Cut away.** The areas and the parts' lines and columns sewn after it that come near it are joined. A
//!   part of `a` that they cover whole is left out, and they are shrunk by the overlap and taken from the
//!   rest. A shape that only meets `a` is shrunk away from it and takes nothing.
//! - **Reaching under.** The shapes sewn after it are grown by [`TOUCHING`], so that a gap rounding left
//!   between edges drawn as one is bridged, and where `a` lies within them it touches them. `a` reaches
//!   into them within the overlap of there, outside what was cut away. So it reaches under a shape that
//!   comes near it without touching it only within the overlap of where it touches another, and a piece of
//!   that reach that doesn't touch what is left of `a` is left out, so that `a` gains no part of its own.
//!
//! A part's line or column (`auto::narrow`, `auto::columns`) paints its part, which covers the areas sewn
//! before it as an area does. It isn't cut away itself: one that the shapes sewn after it cover whole sews
//! nothing, as such an area doesn't, and one they cover in part sews whole.
//!
//! Only the shapes whose bounds come within [`TOUCHING`] of `a`'s are looked at, each for a unit of work, so
//! an area costs the work of its neighbours, not of the whole design. The areas are named in the order they
//! are sewn.

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Point};
use stitchcraft_params::{Family, StitchType};

use super::{Failed, Kind, Shapes, TOUCHING, envelope, joined, near, no_area};
use crate::auto::AutoParams;
use crate::generators::mm;
use crate::generators::tatami::rows::axes;
use crate::normalize::region::Polygon;
use crate::normalize::region::geom::Envelope;
use crate::normalize::region::ops::{area_of, buffered, buffered_near, difference, intersection, union};

/// The automatic areas of `shapes` reaching under the shapes sewn after them and cut away under the areas
/// that cover them, and `SC-I0904` for each area cut away whole, for each that needed more work than the
/// budget allows it and keeps the region it has, and for how many reach under others and how many are cut
/// away.
pub(super) fn reached(shapes: &mut Shapes, budget: &Budget) -> Vec<Diagnostic> {
    // The overlaps of the areas that reach under others, of those cut away, and of the columns that sew
    // past a rail.
    let (mut reaching, mut cut, mut columns) = (Vec::new(), Vec::new(), Vec::new());
    let mut said = Vec::new();
    // What each area shows and what it sews under the shapes sewn after it, once it has been taken, and the
    // shapes sewn after an area split in their wide and their narrow parts, once found.
    let mut rims: Vec<Option<Rim>> = vec![None; shapes.len()];
    let mut split: Vec<Option<Split>> = vec![None; shapes.len()];
    for index in (0..shapes.len()).rev() {
        if shapes.kind(index) == Some(Kind::Part) {
            said.extend(hidden(shapes, index, budget));
        }
        if let (Some(id), Some(overlap)) = (shapes.id(index), shapes.column(index)) {
            match widened(shapes, index, overlap, &mut budget.meter()) {
                Ok(reaches) => columns.extend(reaches.then_some(overlap)),
                Err(failed) => {
                    let (finding, consequence) =
                        ("where this satin column meets the shapes sewn after it", "it sews its pull compensation past both rails");
                    said.push(failed.said(Code::AutoUnder, finding, consequence, budget.max_work, id));
                }
            }
        }
        if shapes.region(index).is_none() {
            continue;
        }
        // An area with a region is automatic, and has its settings of the rules.
        let (Some(id), Some(rules)) = (shapes.id(index), shapes.rules(index)) else { continue };
        let reach = Reach::of(rules);
        let overlap = reach.along;
        match beside(shapes, index, reach, &mut split, &mut budget.meter()) {
            Ok(Some(found)) => {
                reaching.extend(found.reaching.then_some(overlap));
                cut.extend(found.cut.then_some(overlap));
                if found.reaching || found.cut {
                    shapes.set_region(index, found.parts);
                }
                if let Some(slot) = rims.get_mut(index) {
                    *slot = found.rim;
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
    said.extend(firsts(shapes, &mut rims, budget));
    said.extend(counted(&reaching, &cut, &columns));
    said
}

/// From the first sewn to the last, takes from what each area of `shapes` sews under the shapes sewn after
/// it, `rims`, what an area sewn before it already sews there ([`parted`]), each with `budget`'s work to
/// itself; and `SC-I0904` for each area that needed more, which sews its whole overlap.
fn firsts(shapes: &mut Shapes, rims: &mut [Option<Rim>], budget: &Budget) -> Vec<Diagnostic> {
    let mut said = Vec::new();
    for index in 0..shapes.len() {
        let Some((before, rest)) = rims.split_at_mut_checked(index) else { continue };
        let (Some(id), Some(Some(rim))) = (shapes.id(index), rest.first_mut()) else { continue };
        match parted(rim, before, &mut budget.meter()) {
            Ok(Some(parts)) => shapes.set_region(index, parts),
            Ok(None) => {}
            Err(failed) => {
                let (finding, consequence) = ("where this area's overlap lies on that of an area sewn before it", "both sew there");
                said.push(failed.said(Code::AutoUnder, finding, consequence, budget.max_work, id));
            }
        }
    }
    said
}

/// `SC-I0904` for the shape `id`, covered whole by the areas sewn after it, which sews nothing.
pub(super) fn covered(id: ElementId) -> Diagnostic {
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

/// Whether the areas and the parts' lines and columns sewn after the part's line or column `index` of
/// `shapes` cover the part whole, with `meter`'s work.
fn covered_whole(shapes: &mut Shapes, index: usize, meter: &mut Meter) -> Result<bool, Failed> {
    let Some(part) = shapes.paint(index).map(|(_, parts)| parts.to_vec()) else { return Ok(false) };
    let Some(bounds) = envelope(&part) else { return Ok(false) };
    let covering = later(shapes, index, &bounds, meter)?;
    Ok(!covering.is_empty() && no_area(area_of(&uncovered(&part, &covering, meter)?)))
}

/// What the areas and the parts' lines and columns sewn after element `index` of `shapes` that come within
/// [`TOUCHING`] of `bounds` paint, with `meter`'s work: a unit for each shape looked at. A part's line or
/// column covers no element of its own shape: each column of a part paints the whole part, and the fill of
/// the area's broad parts only meets it.
fn later(shapes: &mut Shapes, index: usize, bounds: &Envelope, meter: &mut Meter) -> Result<Vec<Polygon>, Failed> {
    let mut covering = Vec::new();
    for later in near_later(shapes, index, bounds, meter)? {
        covering.extend_from_slice(covers(shapes, index, later));
    }
    Ok(covering)
}

/// The shapes sewn after element `index` of `shapes` that come within [`TOUCHING`] of `bounds`, but the
/// lines, with `meter`'s work: a unit for each shape looked at. A line goes over the area, which is sewn
/// whole under it and reaches under none, so what it paints, which only the order needs, isn't looked for.
fn near_later(shapes: &mut Shapes, index: usize, bounds: &Envelope, meter: &mut Meter) -> Result<Vec<usize>, Failed> {
    let mut found = Vec::new();
    for later in index + 1..shapes.len() {
        meter.charge(1)?;
        if shapes.kind(later) != Some(Kind::Line)
            && !guided_apart(shapes, index, later)
            && shapes.bounds(later).is_some_and(|other| near(bounds, TOUCHING, &other))
        {
            found.push(later);
        }
    }
    Ok(found)
}

/// Whether the areas `index` and `later` of `shapes` are fills of one automatic area that guides split,
/// one of them a guided fill (`REQ-AUTO-110`): they stay one area here, as they were before the guides split
/// it, and neither reaches under the other nor is cut away under it. Their parts lay apart in one region,
/// each sewn to its own edge, and an overlap between them would lay thread on thread where none lay.
fn guided_apart(shapes: &Shapes, index: usize, later: usize) -> bool {
    let guided = |at: usize| {
        shapes.elements.get(at).is_some_and(|element| element.params.get(Family::Fill.method_param()) == Some(StitchType::GuidedFill.id()))
    };
    shapes.is_area(index) && shapes.is_area(later) && shapes.of_shape.get(index) == shapes.of_shape.get(later) && (guided(index) || guided(later))
}

/// What the shape `later` of `shapes` covers of element `index`, sewn before it: an area what it paints, and
/// a part's line or column its part, unless it is a part of that element's own shape.
fn covers(shapes: &Shapes, index: usize, later: usize) -> &[Polygon] {
    let apart = shapes.of_shape.get(later) != shapes.of_shape.get(index);
    match shapes.paint(later) {
        Some((Kind::Area, parts)) => parts,
        Some((Kind::Part, parts)) if apart => parts,
        _ => &[],
    }
}

/// A shape grown to meet an area sewn before it, by how far that area reaches under it.
#[derive(Clone, Debug, Default, PartialEq)]
struct Split {
    /// How wide a wide part is at the least, in millimetres.
    width: f64,
    /// The parts wider than that, which an area reaches its whole reach under.
    wide: Vec<Polygon>,
    /// The narrow parts, which it reaches less under.
    narrow: Vec<Polygon>,
    /// A satin column, and what a part's column sews along the middle of: its thread doesn't cover its ends
    /// and bends as a fill's rows cover its area, so an area reaches its whole overlap under it whichever
    /// way its rows run.
    open: Vec<Polygon>,
    /// What a part's running stitch sews along the middle of: a thread's width of it. An area reaches its
    /// whole overlap under it too, and across it under the shape beyond.
    lines: Vec<Polygon>,
}

/// The shape `later` of `shapes` grown by [`TOUCHING`], split in its parts at least `width` wide and its
/// narrower ones, as `found` has it or with `meter`'s work; `None` for a shape that paints nothing. A satin
/// column and a part's line or column are open whole. Of an area, the wide parts are those a disc `width`
/// across fits on: the area shrunk by half the width and grown again by as much.
fn split_of(shapes: &mut Shapes, later: usize, width: f64, found: &mut [Option<Split>], meter: &mut Meter) -> Result<Option<Split>, Failed> {
    if let Some(Some(known)) = found.get(later).filter(|known| known.as_ref().is_some_and(|known| known.width == width)) {
        return Ok(Some(known.clone()));
    }
    let Some(grown) = shapes.grown(later, meter)? else { return Ok(None) };
    // The width is what a split found once is found again by: without it the split is made again, the same.
    let mut split = Split { width, ..Split::default() };
    match shapes.kind(later) {
        Some(Kind::Part) if !shapes.satin(later) => split.lines = grown,
        Some(Kind::Column | Kind::Part) => split.open = grown,
        _ => {
            // Grown, the shape is wider by [`TOUCHING`] on each side.
            let half = width / 2.0 + TOUCHING;
            let core = joined(buffered(&grown, -half, meter))?;
            split.wide = joined(intersection(&joined(buffered(&core, half, meter))?, &grown, meter))?;
            split.narrow = joined(difference(&grown, &split.wide, meter))?;
        }
    }
    if let Some(slot) = found.get_mut(later) {
        *slot = Some(split.clone());
    }
    Ok(Some(split))
}

/// How many overlaps wide a part of an area is at the least for an area sewn before it to reach its whole
/// overlap under it: 3, so that the overlaps from both sides leave a third of a narrower part free.
pub(super) const WIDE: f64 = 3.0;

/// The share of its reach that an area reaches under a narrow shape: half.
pub(super) const UNDER_NARROW: f64 = 0.5;

/// How far an area reaches under the shapes sewn after it, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Reach {
    /// Along its rows, whose ends the thread pulls in: its overlap.
    along: f64,
    /// Across its rows: its pull compensation, and no more than along them.
    across: f64,
    /// The angle of its rows, in degrees.
    angle: f64,
}

impl Reach {
    /// The reach of an area with the settings `rules`.
    fn of(rules: &AutoParams) -> Reach {
        let along = rules.auto_overlap_mm.get();
        Reach { along, across: rules.auto_pull_compensation_mm.get().min(along), angle: rules.auto_fill_angle }
    }

    /// The reach under a narrow shape: [`UNDER_NARROW`] of it along the rows, and no more across them.
    fn narrow(self) -> Reach {
        let along = self.along * UNDER_NARROW;
        Reach { along, across: self.across.min(along), angle: self.angle }
    }

    /// `parts` grown by the reach, with `meter`'s work: every point within the ellipse round a point of
    /// `parts` whose half axes are the reach along the rows and the reach across them. The plane is
    /// squeezed along the rows until the ellipse is a circle, the parts are grown by its radius there, and
    /// the plane is stretched back.
    fn grown(self, parts: &[Polygon], meter: &mut Meter) -> Result<Vec<Polygon>, Failed> {
        if self.across >= self.along {
            return joined(buffered(parts, self.along, meter));
        }
        // No reach across the rows would squeeze the plane flat: a hundredth of a millimetre is the least.
        let (radius, stretch) = (self.across.max(LEAST), self.along / self.across.max(LEAST));
        // The way along the rows, and the way across them a quarter turn on, so that rings keep their winding.
        let ((ax, ay), _) = axes(self.angle);
        let (bx, by) = (-ay, ax);
        let squeezed = |p: &Point| Point::new((p.x() * ax + p.y() * ay) / stretch, p.x() * bx + p.y() * by).unwrap_or(*p);
        let stretched = |p: &Point| Point::new(p.x() * stretch * ax + p.y() * bx, p.x() * stretch * ay + p.y() * by).unwrap_or(*p);
        let grown = joined(buffered(&moved(parts, squeezed), radius, meter))?;
        Ok(moved(&grown, stretched))
    }
}

/// The least reach across the rows that an area's reach is grown with, in millimetres.
const LEAST: f64 = 0.01;

/// `parts` with each point moved by `to`, which keeps the rings' winding.
fn moved(parts: &[Polygon], to: impl Fn(&Point) -> Point) -> Vec<Polygon> {
    let ring = |ring: &Vec<Point>| ring.iter().map(&to).collect();
    parts.iter().map(|part| Polygon { outline: ring(&part.outline), holes: part.holes.iter().map(ring).collect() }).collect()
}

/// What an area shows and what it sews under the shapes sewn after it.
#[derive(Clone, Debug, PartialEq)]
struct Rim {
    /// What shows of it: what the areas sewn after it leave uncovered.
    shown: Vec<Polygon>,
    /// What it sews under the shapes sewn after it.
    under: Vec<Polygon>,
}

/// Takes from `rim` what the areas sewn before its area sew under the shapes sewn after them, `before`,
/// with `meter`'s work: a unit for each of them, and the work of those near. Under a shape sewn after 2
/// areas only the one sewn first sews, so that 2 overlaps never lie on each other (`REQ-AUTO-067`). The
/// area sewn first keeps its overlap whole: its rows then cross every edge it has with the shape, and an
/// edge that only the later area's overlap covered would open where the thread pulls both away from it. A
/// piece of the overlap that this cuts off from what shows of the area is left out. Returns the area's
/// region when it changes.
fn parted(rim: &mut Rim, before: &[Option<Rim>], meter: &mut Meter) -> Result<Option<Vec<Polygon>>, Failed> {
    let Some(bounds) = envelope(&rim.under) else { return Ok(None) };
    let mut theirs: Vec<&[Polygon]> = Vec::new();
    for other in before.iter().flatten() {
        meter.charge(1)?;
        if envelope(&other.under).is_some_and(|other| near(&bounds, 0.0, &other)) {
            theirs.push(other.under.as_slice());
        }
    }
    if theirs.is_empty() {
        return Ok(None);
    }
    let theirs = joined(union(theirs, meter))?;
    let kept = joined(difference(&rim.under, &theirs, meter))?;
    if no_area(area_of(&rim.under) - area_of(&kept)) {
        return Ok(None);
    }
    let parts = attached(joined(union([rim.shown.as_slice(), kept.as_slice()], meter))?, &rim.shown, meter)?;
    rim.under = joined(difference(&parts, &rim.shown, meter))?;
    Ok(Some(parts))
}

/// How far apart the places lie where a rail is looked at for a shape beside it, at most, in millimetres.
const RAIL_STEP: f64 = 0.5;

/// Makes the automatic satin column `index` of `shapes` sew `overlap` past each rail that a shape sewn after
/// it runs along for more than half the rail's length, with `meter`'s work, and says whether it has such a
/// rail (`REQ-AUTO-029`). A rail with none keeps the column's pull compensation, so the column grows no
/// wider over a shape sewn before it or over fabric. A line goes over the column, and a part's line or
/// column of the column's own shape paints the part the column is in, so neither is a shape beside it.
fn widened(shapes: &mut Shapes, index: usize, overlap: f64, meter: &mut Meter) -> Result<bool, Failed> {
    let Some(rails) = shapes.rails(index, meter)? else { return Ok(false) };
    let points: Vec<Point> = rails.iter().flatten().copied().collect();
    let bounds = Envelope::of(&points);
    let mut beside = Vec::new();
    for later in index + 1..shapes.len() {
        meter.charge(1)?;
        let own = shapes.kind(later) == Some(Kind::Part) && shapes.of_shape.get(later) == shapes.of_shape.get(index);
        if own || shapes.kind(later) == Some(Kind::Line) || !shapes.bounds(later).is_some_and(|other| near(&bounds, TOUCHING, &other)) {
            continue;
        }
        beside.extend(shapes.grown(later, meter)?.into_iter().flatten());
    }
    let [first, second] = &rails;
    let under = [runs_along(first, &beside, meter)?, runs_along(second, &beside, meter)?];
    if under.contains(&true) {
        shapes.reach_past(index, under, overlap);
    }
    Ok(under.contains(&true))
}

/// Whether the shapes `beside` cover more than half the length of `rail`, with `meter`'s work: the rail is
/// looked at in the middle of each stretch of it no longer than [`RAIL_STEP`], for a unit of work and one for
/// each shape. Each shape's bounds are found first, for a unit of work each: a shape is asked only about a
/// place within its bounds, since it covers none beyond them and asking walks its whole outline.
pub(super) fn runs_along(rail: &[Point], beside: &[Polygon], meter: &mut Meter) -> Result<bool, Exhausted> {
    meter.charge(beside.len() as u64)?;
    let bounds: Vec<Envelope> = beside.iter().map(|shape| Envelope::of(&shape.outline)).collect();
    let (mut within, mut whole) = (0.0, 0.0);
    for pair in rail.windows(2) {
        let [from, to] = pair else { continue };
        let length = from.distance(*to);
        let stretches = (length / RAIL_STEP).ceil().max(1.0);
        let mut at = 0.5;
        while at < stretches {
            meter.charge(1 + beside.len() as u64)?;
            let place = from.lerp(*to, at / stretches);
            if beside.iter().zip(&bounds).any(|(shape, bounds)| bounds.holds(place) && shape.covers(place)) {
                within += length / stretches;
            }
            at += 1.0;
        }
        whole += length;
    }
    Ok(within > 0.5 * whole)
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
    /// What it shows and what it sews under the shapes sewn after it, when it sews under any.
    rim: Option<Rim>,
}

impl Found {
    /// An area with the region `parts` that sews under no shape.
    fn apart(parts: Vec<Polygon>) -> Found {
        Found { parts, reaching: false, cut: false, rim: None }
    }
}

/// The region of the automatic area `index` of `shapes` beside the shapes sewn after it, with its `reach`
/// and `meter`'s work; `None` when the areas sewn after it cover it whole. `split` keeps each shape sewn
/// after it split in its wide and its narrow parts, for the areas sewn before it.
fn beside(shapes: &mut Shapes, index: usize, reach: Reach, split: &mut [Option<Split>], meter: &mut Meter) -> Result<Option<Found>, Failed> {
    let area = shapes.region(index).map(<[Polygon]>::to_vec).unwrap_or_default();
    let Some(bounds) = envelope(&area) else { return Ok(Some(Found::apart(area))) };
    // The areas and the parts' lines and columns sewn after it that come near it, and every area, column and
    // part's line or column that does, grown to meet it, in its wide and its narrow parts.
    let (mut covering, mut wide, mut narrow, mut open, mut lines) = (Vec::new(), Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for later in near_later(shapes, index, &bounds, meter)? {
        covering.extend_from_slice(covers(shapes, index, later));
        if let Some(parts) = split_of(shapes, later, WIDE * reach.along, split, meter)? {
            wide.extend(parts.wide);
            narrow.extend(parts.narrow);
            open.extend(parts.open);
            lines.extend(parts.lines);
        }
    }
    // Every shape that comes near it is in one of the 4, and the areas and parts among them in `covering` too.
    if wide.is_empty() && narrow.is_empty() && open.is_empty() && lines.is_empty() {
        return Ok(Some(Found::apart(area)));
    }
    // What the shapes sewn after it leave of it uncovered: none, and it sews nothing.
    let covered = joined(union([covering.as_slice()], meter))?;
    let uncovered = joined(difference(&area, &covered, meter))?;
    if no_area(area_of(&uncovered)) {
        return Ok(None);
    }
    let (wide, narrow, open) =
        (joined(union([wide.as_slice()], meter))?, joined(union([narrow.as_slice()], meter))?, joined(union([open.as_slice()], meter))?);
    let lines = joined(union([lines.as_slice()], meter))?;
    // Cut away the overlap inside their edges, but for its parts they cover whole, which only they sew. What
    // is left holds what they leave uncovered. An area sewn after it may still hold a line's part, which it
    // gives up to this one ([`parted`]): its edge is taken where the line's part ends.
    let apart = if lines.is_empty() { covered } else { joined(difference(&covered, &lines, meter))? };
    let cut_zone = joined(buffered(&apart, -reach.along, meter))?;
    let left = attached(joined(difference(&area, &cut_zone, meter))?, &uncovered, meter)?;
    let cut = !no_area(area_of(&area) - area_of(&left));
    // Reaching under the shapes sewn after it from where it touches them, outside what was cut away: its
    // reach under their wide parts, and less under their narrow ones.
    let covers = joined(union([wide.as_slice(), narrow.as_slice(), open.as_slice()], meter))?;
    let contact = joined(intersection(&area, &joined(union([covers.as_slice(), lines.as_slice()], meter))?, meter))?;
    // Across a line's part, whose thread doesn't cover it, and on under the shapes beyond it from where it
    // meets them there: its overlap with them is then whole, whatever the line's width. A shape beyond the
    // line lies outside the line's part. What an area sewn after this one already sews of the part is no
    // such shape, and neither is the part itself where a column sews along it too: the area would reach
    // across the part and then as far again. So the shapes are taken without the part, and grown to meet it
    // again.
    //
    // Growing a region is most of an area's work here, and a design of many areas does it for each. So
    // nothing is grown to meet shapes that aren't there: an area with no line's part beside it goes across
    // none, and one with no wide part, narrow part or column beside it reaches under none of that kind.
    let across = if lines.is_empty() { Vec::new() } else { joined(intersection(&joined(buffered(&contact, reach.along, meter))?, &lines, meter))? };
    let beyond = if across.is_empty() {
        Vec::new()
    } else {
        // Only the shapes' parts whose growth reaches the line's part are grown ([`buffered_near`]).
        let outside = joined(buffered_near(&joined(difference(&covers, &lines, meter))?, 2.0 * TOUCHING, &across, meter))?;
        joined(intersection(&across, &outside, meter))?
    };
    let from = if beyond.is_empty() { contact } else { joined(union([contact.as_slice(), beyond.as_slice()], meter))? };
    let under_wide = if wide.is_empty() { Vec::new() } else { joined(intersection(&reach.grown(&from, meter)?, &wide, meter))? };
    let under_narrow = if narrow.is_empty() { Vec::new() } else { joined(intersection(&reach.narrow().grown(&from, meter)?, &narrow, meter))? };
    let under_open = if open.is_empty() { Vec::new() } else { joined(intersection(&joined(buffered(&from, reach.along, meter))?, &open, meter))? };
    let reached = joined(union([across.as_slice(), under_wide.as_slice(), under_narrow.as_slice(), under_open.as_slice()], meter))?;
    let under = joined(difference(&joined(difference(&reached, &area, meter))?, &cut_zone, meter))?;
    let parts =
        if no_area(area_of(&under)) { left.clone() } else { attached(joined(union([left.as_slice(), under.as_slice()], meter))?, &left, meter)? };
    let reaching = !no_area(area_of(&parts) - area_of(&left));
    let parts = if reaching { parts } else { left };
    let rim = Rim { under: joined(difference(&parts, &uncovered, meter))?, shown: uncovered };
    Ok(Some(Found { parts, reaching, cut, rim: Some(rim) }))
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
fn counted(reaching: &[f64], cut: &[f64], columns: &[f64]) -> Option<Diagnostic> {
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
    let columns = match columns {
        [] => None,
        [one] => Some(format!("1 satin column sews {} mm past a rail, under the shape sewn after it there.", mm(*one))),
        many => Some(format!("{} satin columns sew {} mm past a rail, under the shapes sewn after them there.", many.len(), spread(many))),
    };
    let message = [reach, cut, columns].into_iter().flatten().collect::<Vec<_>>().join(" ");
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

#[cfg(test)]
mod tests;
