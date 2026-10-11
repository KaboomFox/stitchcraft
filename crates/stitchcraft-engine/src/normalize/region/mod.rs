//! The area a fill covers: its subpaths as rings, filled by the drawing's fill rule, in parts with holes
//! (`REQ-FILL-001`, `REQ-FILL-002`).
//!
//! A fill should sew what the drawing shows, so its area is the area an SVG renderer paints. Each
//! subpath is flattened within a tenth of a CSS pixel ([`stroke::SHAPE_TOLERANCE`]), as Ink/Stitch
//! flattens it, and closed. Where subpaths cross, touch or run along each other they are cut, and every
//! face between the cuts counts how many times the subpaths wind round it (`arrange`): the fill rule
//! fills a face whose count is not 0 (`nonzero`, SVG's default) or is odd (`evenodd`). The edges between
//! filled and empty faces become the parts' rings (`assemble`).
//!
//! Ink/Stitch builds a fill's area differently: the subpath of the largest area is the outline and every
//! other subpath a hole, whatever the fill rule, and a shape that is not valid is repaired by the even-odd
//! rule. Where the drawing's rule fills a part that the even-odd rule leaves empty, StitchCraft fills it
//! and says so (`SC-I0306`, `DEV-FILL-001`). A subpath of fewer than 3 distinct points bounds nothing and
//! is left out, where Ink/Stitch makes it a tiny triangle (`DEV-FILL-002`). Ink/Stitch's decisions about
//! sewing are kept: a part of 3 square CSS pixels or less is too small for a row and is left out, but
//! with a warning (`SC-W0303`), a fill under 20 square CSS pixels is better sewn otherwise (`SC-W0304`),
//! and a fill in several parts is sewn one part after another (`SC-W0307`).
//!
//! Each ring of a part starts at the point the drawing reaches first; outlines run clockwise and holes
//! counter-clockwise in y-up axes (counter-clockwise and clockwise on screen, where y points down). The
//! parts come in the order the drawing reaches them.

mod arrange;
mod assemble;
pub(crate) mod geom;

use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_core::{Code, Diagnostic, Exhausted, Fix, Meter, Point};

use crate::design::{FillRule, Path};
use crate::normalize::stroke;

/// Points this near each other, in millimetres, are one point: a crossing found this near a point already
/// found is that point, and a row's stretch this short is no segment.
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
    /// Parts left out, a fill too small, a fill in parts, and where the fill rule differs from the even-odd
    /// rule (`SC-W0303`, `SC-W0304`, `SC-W0307`, `SC-I0306`), and `SC-E0009` should the rings not join.
    pub diagnostics: Vec<Diagnostic>,
}

/// The region the subpaths of `path` bound under `rule`. One unit of `meter` for each point, segment and
/// cut, each pair of segments whose spans along x overlap, each half edge, and each ring and face compared.
pub fn build(path: &Path, rule: FillRule, meter: &mut Meter) -> Result<Built, Exhausted> {
    let flat = stroke::flatten(path, stroke::SHAPE_TOLERANCE, meter)?;
    let rings: Vec<Vec<Point>> =
        flat.pieces.into_iter().map(|piece| geom::closed(&piece.points)).filter(|ring| geom::without_repeats(ring).len() >= 4).collect();
    let arrangement = arrange::cut(&rings, meter)?;
    let faces = arrangement.faces(meter)?;
    let filled = |w: i64| match rule {
        FillRule::NonZero => w != 0,
        FillRule::EvenOdd => w.rem_euclid(2) == 1,
    };
    let inside = |half: usize| faces.right_of.get(half).and_then(|&f| faces.winding.get(f)).is_some_and(|&w| filled(w));
    let kept: Vec<bool> = (0..2 * arrangement.edges.len()).map(|h| inside(h) && !inside(h ^ 1)).collect();
    let joined = assemble::parts(&arrangement, &kept, meter)?;
    // Rings that do not join are a bug: the region is left empty, and SC-E0009 says so.
    let mut diagnostics: Vec<Diagnostic> = joined.as_ref().err().copied().map(assemble::Unjoined::diagnostic).into_iter().collect();
    let parts = joined.unwrap_or_default();
    let ring = |points: &[usize]| geom::closed(&points.iter().map(|&i| arrangement.point(i)).collect::<Vec<_>>());
    let (mut kept_parts, mut left_out) = (Vec::new(), Vec::new());
    for part in parts {
        let polygon = Polygon { outline: ring(&part.outline), holes: part.holes.iter().map(|hole| ring(hole)).collect() };
        if polygon.area() > TINY_PART { kept_parts.push(polygon) } else { left_out.push(polygon) }
    }
    let region = Region { parts: kept_parts };
    diagnostics.extend(tiny_parts(&left_out, region.parts.is_empty()));
    if region.parts.is_empty() && left_out.is_empty() {
        diagnostics.push(Diagnostic::new(Code::FillPartsTooSmall, "This fill bounds no area under its fill rule, so it sews nothing."));
    }
    let area = region.area();
    if area > 0.0 && area < SMALL_FILL {
        let message = format!(
            "This fill covers {} mm², less than {} mm², so a running stitch round it or a satin column across it would probably sew it \
             better.",
            square_mm(area),
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
    if rule == FillRule::NonZero && faces.winding.iter().any(|&w| w != 0 && w.rem_euclid(2) == 0) {
        let message = "Subpaths of this fill wind round some of it twice the same way, which its fill rule (nonzero) fills and the even-odd \
                       rule leaves empty. It stays in the fill, as the drawing shows it; Ink/Stitch would leave it empty.";
        diagnostics.push(
            Diagnostic::new(Code::FillRuleNotEvenOdd, message)
                .with_fix(Fix::Hint("To leave it empty, set the fill rule to even-odd, or draw the inner subpath the other way round.".to_string())),
        );
    }
    Ok(Built { region, diagnostics })
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
