//! The area a fill covers: its subpaths as rings, filled by the drawing's fill rule, in parts with holes
//! (`REQ-FILL-001`, `REQ-FILL-002`).
//!
//! A fill should sew what the drawing shows, so its area is the area an SVG renderer paints. Each
//! subpath is flattened within a tenth of a CSS pixel ([`stroke::SHAPE_TOLERANCE`]), as Ink/Stitch
//! flattens it, and closed. Where subpaths cross, touch or run along each other they are cut, on a grid
//! fine enough that only rounding errors fall within a cell, so that the cuts hold however rounding left
//! the drawing (`snap`), and every face between the cuts counts how many times the subpaths wind round it
//! (`arrange`): the fill rule fills a face whose count is not 0 (`nonzero`, SVG's default) or is odd
//! (`evenodd`). The edges between filled and empty faces become the parts' rings (`assemble`).
//!
//! Ink/Stitch builds a fill's area differently: the subpath of the largest area is the outline and every
//! other subpath a hole, whatever the fill rule, and a shape that is not valid is repaired. The repair
//! merges the subpaths' lines, a stretch drawn twice into one, and fills the faces they bound by how
//! deeply they nest. That is the even-odd rule, but where subpaths run along each other: a face beside the
//! outside nests in nothing, and is always filled. StitchCraft follows the drawing (`DEV-FILL-001`), and
//! says where that differs from Ink/Stitch: where the nonzero rule fills a face inside that the even-odd
//! rule leaves empty (`SC-I0306`), and where the fill rule leaves a face beside the outside empty, which a
//! stretch drawn twice bounds (`SC-I0313`). A subpath of fewer than 3 distinct points bounds nothing and
//! is left out, where Ink/Stitch makes it a tiny triangle (`DEV-FILL-002`). Ink/Stitch's decisions about
//! sewing are kept: a part of 3 square CSS pixels or less is too small for a row and is left out, but
//! with a warning (`SC-W0303`), a fill under 20 square CSS pixels is better sewn otherwise (`SC-W0304`),
//! and a fill in parts is sewn one part after another (`SC-W0307`).
//!
//! Each ring of a part starts at the point the drawing reaches first; outlines run clockwise and holes
//! counter-clockwise in y-up axes (counter-clockwise and clockwise on screen, where y points down). The
//! parts come in the order the drawing reaches them.
//!
//! The area some rings bound, cut and joined so, is one function (`area`), which a part grown or shrunk
//! also takes its area from: the rings are then its offset curves (`normalize::offset`). Regions combined,
//! as automatic digitizing combines its neighbouring shapes, take theirs from it too (`ops`).

mod arrange;
mod assemble;
pub(crate) mod geom;
pub(crate) mod ops;
mod snap;

pub(crate) use assemble::Unjoined;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::design::{FillRule, Path};
use crate::normalize::stroke;

/// Points this near each other, in millimetres, are one point to the code that sews a region: a row's
/// stretch this short is no segment, a point this near a row lies on it, and parts whose least x lie this
/// near are level. It is some 30 times the grid the region's points lie on (`snap`).
pub(crate) const SAME: f64 = 1e-9;

/// A part this small or smaller, in square millimetres, is left out: 3 square CSS pixels, a speck about
/// half a millimetre across, too small for a row of stitches (Ink/Stitch's limit).
pub const TINY_PART: f64 = 3.0 * MM_PER_SVG_PX * MM_PER_SVG_PX;

/// A fill smaller than this, in square millimetres, is better sewn as a running stitch or a satin
/// column: 20 square CSS pixels (Ink/Stitch's limit).
pub const SMALL_FILL: f64 = 20.0 * MM_PER_SVG_PX * MM_PER_SVG_PX;

/// One part of a fill's area: an outline and the holes in it, each a closed ring (its first point
/// repeated at its end), in millimetres.
#[derive(Clone, Debug, PartialEq)]
pub struct Polygon {
    /// The outline, clockwise in y-up axes, from the point of it the drawing reaches first.
    pub outline: Vec<Point>,
    /// The holes, counter-clockwise, in the order the drawing reaches them.
    pub holes: Vec<Vec<Point>>,
}

impl Polygon {
    /// Its rings: the outline, then the holes.
    pub(crate) fn rings(&self) -> impl Iterator<Item = &[Point]> {
        std::iter::once(self.outline.as_slice()).chain(self.holes.iter().map(Vec::as_slice))
    }

    /// Whether `p` lies in the part or on one of its rings, decided exactly.
    pub(crate) fn covers(&self, p: Point) -> bool {
        match geom::locate_in_ring(p, &self.outline) {
            geom::Location::Boundary => true,
            geom::Location::Exterior => false,
            geom::Location::Interior => self.holes.iter().all(|hole| geom::locate_in_ring(p, hole) != geom::Location::Interior),
        }
    }

    /// The area it covers: its outline's less its holes', in square millimetres.
    pub fn area(&self) -> f64 {
        geom::signed_area(&self.outline).abs() - self.holes.iter().map(|hole| geom::signed_area(hole).abs()).sum::<f64>()
    }
}

/// A fill's area, as parts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Region {
    /// The parts, in the order the drawing reaches them.
    pub parts: Vec<Polygon>,
}

impl Region {
    /// The area it covers, in square millimetres.
    pub fn area(&self) -> f64 {
        self.parts.iter().map(Polygon::area).sum()
    }
}

/// A fill's region and what is worth saying about it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Built {
    /// The region.
    pub region: Region,
    /// Parts left out, a fill too small, a fill in parts, and where the area differs from Ink/Stitch's
    /// (`SC-W0303`, `SC-W0304`, `SC-W0307`, `SC-I0306`, `SC-I0313`), or `SC-E0009` alone should the area
    /// not be built, a bug.
    pub diagnostics: Vec<Diagnostic>,
}

/// The region the subpaths of `path` bound under `rule`. One unit of `meter` for each point, each edge
/// bent and each point tested against it, each pair of edges whose spans along x overlap, each half edge,
/// each ring compared with a face or an outline, and each point walked to find a ring's bounds or its place
/// among the faces and outlines (`REQ-FILL-004`).
pub fn build(path: &Path, rule: FillRule, meter: &mut Meter) -> Result<Built, Exhausted> {
    let flat = stroke::flatten(path, stroke::SHAPE_TOLERANCE, meter)?;
    let rings: Vec<Vec<Point>> =
        flat.pieces.into_iter().map(|piece| geom::closed(&piece.points)).filter(|ring| geom::without_repeats(ring).len() >= 4).collect();
    Ok(built(area(&rings, |w| fills(rule, w), meter)?, rule))
}

/// `p` on the grid a region's points lie on ([`snap::GRID`]), where coordinates' differences are exact
/// floats. A point beyond the grid's reach, some 65 m from the origin, where no design lies, is left as it
/// is: the exact predicates still decide its sides, at more cost.
pub(crate) fn on_grid(p: Point) -> Point {
    snap::Node::of(p).map_or(p, snap::Node::point)
}

/// Whether `rule` fills a face whose winding number is `w`.
pub(crate) fn fills(rule: FillRule, w: i64) -> bool {
    match rule {
        FillRule::NonZero => w != 0,
        FillRule::EvenOdd => w.rem_euclid(2) == 1,
    }
}

/// The region of the area found under `rule`, and what is worth saying about it. An area that could not be
/// built is a bug: the region is left empty, and only `SC-E0009` is said.
fn built(area: Area, rule: FillRule) -> Built {
    let parts = match area.parts {
        Ok(parts) => parts,
        Err(unjoined) => return Built { region: Region::default(), diagnostics: vec![unjoined.diagnostic()] },
    };
    let (kept_parts, left_out): (Vec<Polygon>, Vec<Polygon>) = parts.into_iter().partition(|polygon| polygon.area() > TINY_PART);
    let region = Region { parts: kept_parts };
    let mut diagnostics: Vec<Diagnostic> = tiny_parts(&left_out, region.parts.is_empty()).into_iter().collect();
    if region.parts.is_empty() && left_out.is_empty() {
        diagnostics.push(Diagnostic::new(Code::FillPartsTooSmall, "This fill bounds no area under its fill rule, so it sews nothing."));
    }
    let covered = region.area();
    if covered > 0.0 && covered < SMALL_FILL {
        let message = format!(
            "This fill covers {} mm², less than {} mm², so a running stitch round it or a satin column across it would probably sew it \
             better.",
            square_mm(covered),
            square_mm(SMALL_FILL)
        );
        diagnostics.push(
            Diagnostic::new(Code::FillSmall, message)
                .with_fix(Fix::Hint("Sew it as a running stitch or a satin column, or draw it larger.".to_string())),
        );
    }
    if region.parts.len() > 1 {
        let message = format!("This fill's area falls into {} parts that are sewn one after another, with a jump between each.", region.parts.len());
        diagnostics.push(
            Diagnostic::new(Code::FillPartsApart, message)
                .with_fix(Fix::Hint("To choose the order the parts are sewn in, break the fill apart into one element for each part.".to_string())),
        );
    }
    // Across the edge between a face and the outside, the winding number changes by the face's own: by an
    // even number for a face the even-odd rule leaves empty, and by 0 for one the nonzero rule leaves
    // empty. Rings then run along that edge both ways, or twice the same way, and Ink/Stitch's repair, which
    // merges them into one edge, fills the face.
    let faces = || area.windings.iter().zip(&area.beside_outside).take(area.windings.len().saturating_sub(1));
    if rule == FillRule::NonZero && faces().any(|(&w, &beside)| w != 0 && w.rem_euclid(2) == 0 && !beside) {
        let message = "Subpaths of this fill wind round some of it twice the same way, which its fill rule (nonzero) fills and the even-odd \
                       rule leaves empty. It stays in the fill, as the drawing shows it; Ink/Stitch would leave it empty.";
        diagnostics.push(
            Diagnostic::new(Code::FillRuleNotEvenOdd, message)
                .with_fix(Fix::Hint("To leave it empty, set the fill rule to even-odd, or draw the inner subpath the other way round.".to_string())),
        );
    }
    if faces().any(|(&w, &beside)| beside && !fills(rule, w)) {
        let message = "Subpaths of this fill run along each other at its edge, around a part its fill rule leaves empty, as the drawing \
                       shows it. Ink/Stitch counts a stretch drawn twice once, and would fill that part.";
        diagnostics.push(
            Diagnostic::new(Code::FillStretchDrawnTwice, message)
                .with_fix(Fix::Hint("To sew it alike in both, draw the outline round the empty part instead of a subpath along it.".to_string())),
        );
    }
    Built { region, diagnostics }
}

/// The area some rings bound: the faces they cut the plane into whose winding numbers the rule given says
/// are filled, as parts with holes.
pub(crate) struct Area {
    /// The parts, unless the edges between filled and empty faces did not join into rings (a bug).
    pub(crate) parts: Result<Vec<Polygon>, Unjoined>,
    /// Every face's winding number; the last face is outside everything.
    pub(crate) windings: Vec<i64>,
    /// For every face, whether it lies beside the outside: an edge has one on each side.
    pub(crate) beside_outside: Vec<bool>,
}

/// The area `rings` (each open or closed, in the direction drawn) bound: the faces they cut the plane into
/// whose winding numbers `filled` says are filled ([`arrange`]), joined into parts ([`assemble`]). One unit
/// of `meter` for each point, each edge bent and each point tested against it, each pair of edges whose
/// spans along x overlap, each half edge, each ring compared with a face or an outline, and each point
/// walked to find a ring's bounds or its place among the faces and outlines (`REQ-FILL-004`).
pub(crate) fn area(rings: &[Vec<Point>], filled: impl Fn(i64) -> bool, meter: &mut Meter) -> Result<Area, Exhausted> {
    let arrangement = match arrange::cut(rings, meter)? {
        Ok(arrangement) => arrangement,
        Err(unjoined) => return Ok(Area { parts: Err(unjoined), windings: Vec::new(), beside_outside: Vec::new() }),
    };
    let faces = arrangement.faces(meter)?;
    let inside = |half: usize| faces.right_of.get(half).and_then(|&f| faces.winding.get(f)).is_some_and(|&w| filled(w));
    let kept: Vec<bool> = (0..2 * arrangement.edges.len()).map(|h| inside(h) && !inside(h ^ 1)).collect();
    let ring = |points: &[usize]| geom::closed(&points.iter().map(|&i| arrangement.point(i)).collect::<Vec<_>>());
    let parts = assemble::parts(&arrangement, &kept, meter)?.map(|parts| {
        parts.iter().map(|part| Polygon { outline: ring(&part.outline), holes: part.holes.iter().map(|hole| ring(hole)).collect() }).collect()
    });
    let outside = faces.winding.len().saturating_sub(1);
    let mut beside_outside = vec![false; faces.winding.len()];
    // The faces on the right of each edge's 2 halves.
    let halves = &faces.right_of;
    for (&a, &b) in halves.iter().step_by(2).zip(halves.iter().skip(1).step_by(2)) {
        for (face, other) in [(a, b), (b, a)] {
            if other == outside
                && let Some(beside) = beside_outside.get_mut(face)
            {
                *beside = true;
            }
        }
    }
    Ok(Area { parts, windings: faces.winding, beside_outside })
}

/// `SC-W0303` for the parts left out, each too small to sew, located at the first; `all` when they were
/// the whole fill.
fn tiny_parts(left_out: &[Polygon], all: bool) -> Option<Diagnostic> {
    let first = left_out.first()?;
    let limit = square_mm(TINY_PART);
    let message = match (left_out.len(), all) {
        (1, true) => format!(
            "This fill covers only {} mm², {limit} mm² or less, too little for a row of stitches, so it is left out.",
            square_mm(first.area())
        ),
        (1, false) => format!("1 part of this fill covers {limit} mm² or less, too little for a row of stitches, so it is left out."),
        (n, true) => format!("All {n} parts of this fill cover {limit} mm² or less each, too little for a row of stitches, so they are left out."),
        (n, false) => format!("{n} parts of this fill cover {limit} mm² or less each, too little for a row of stitches, so they are left out."),
    };
    let at = first.outline.first().copied().unwrap_or(Point::ORIGIN);
    Some(
        Diagnostic::new(Code::FillPartsTooSmall, message)
            .located(at)
            .with_fix(Fix::Hint("Draw the parts larger, or sew them as a running stitch.".to_string())),
    )
}

/// An area in square millimetres, as messages give it: 2 decimal places, without trailing zeros.
fn square_mm(area: f64) -> String {
    crate::generators::mm(area)
}

#[cfg(test)]
mod tests;
