//! The elements a route takes, read: each satin column with its centre line and the pieces it is cut into,
//! and each stroke among them by its first subpath, which shows where the needle may travel
//! (`docs/src/design/routing.md` › Auto-satin, steps 1 and 2).
//!
//! **What is routed.** An element is a satin column when it sews as one, whatever its satin method, and a
//! stroke when it sews as any stroke, a satin column drawn as one path too narrow to stitch across among
//! them, as Ink/Stitch takes them (read at `d59c9ab`). A fill, an element digitized automatically, one whose
//! settings don't read, a column whose rails can't be made and a column or stroke of no length are left as
//! they are, and `SC-I1002` names each. So is a stroke whose first subpath ends where it starts, which
//! shows no way between 2 places: Ink/Stitch adds its point to the graph and never sews it, and deletes it
//! with the rest (`DEV-ROUTE-003`).
//!
//! **Pieces.** A column's centre line is cut into equal lengths, as few as keep each at most 1 mm long
//! ([`PIECE`]), each a piece from one fraction of the line to the next. A stroke is one piece, from its
//! first subpath's start to its end.
//!
//! **What hides a jump.** A jump's length outside the 2 elements it joins decides whether it is trimmed.
//! A column is its rails, lines that hide only a jump that runs along them. A stroke is the convex hull of
//! its subpaths, as Ink/Stitch takes a stroke's shape.

use stitchcraft_core::{Code, Diagnostic, Exhausted, Meter, Point, Severity};
use stitchcraft_params::Family;

use crate::design::{DesignSettings, Element, Shape};
use crate::generate::stitch_type_of;
use crate::generators::satin::cut::Cuttable;
use crate::generators::satin::{self, SatinParams};
use crate::normalize::along::Along;
use crate::normalize::stroke::{self, SHAPE_TOLERANCE};
use crate::route::trim::{Outline, hull};

/// How long a piece of a column's centre line is at most, in millimetres: 1 mm, Ink/Stitch's
/// (`PIXELS_PER_MM` CSS pixels).
pub const PIECE: f64 = 1.0;

/// An element the route takes, read.
pub(super) struct Routable {
    /// Its place among the elements given.
    pub(super) element: usize,
    /// What it is.
    pub(super) kind: Kind,
    /// The line it is cut along: a column's centre line, or a stroke's first subpath.
    pub(super) line: Vec<Point>,
    /// How many pieces it is cut into: at least 1.
    pub(super) pieces: usize,
    /// What it hides of a jump to or from it.
    pub(super) outline: Outline,
}

/// What a routed element is.
pub(super) enum Kind {
    /// A satin column, ready to cut, with its settings.
    Column(Box<(Cuttable, SatinParams)>),
    /// A stroke.
    Stroke,
}

impl Routable {
    /// Whether it is a satin column.
    pub(super) fn is_column(&self) -> bool {
        matches!(self.kind, Kind::Column(_))
    }

    /// Where each of its pieces starts and ends on its line, in order: a column's at the fractions of its
    /// line from one piece to the next, and a stroke's at its line's ends.
    pub(super) fn ends(&self, meter: &mut Meter) -> Result<Vec<[Point; 2]>, Exhausted> {
        let along = Along::new(&self.line, meter)?;
        let at = |k: usize| along.point(along.length() * self.fraction(k));
        let mut ends = Vec::with_capacity(self.pieces);
        for index in 0..self.pieces {
            meter.charge(1)?;
            ends.push([at(index), at(index + 1)]);
        }
        Ok(ends)
    }

    /// The fraction of its line where its piece `index` starts: `index` over its pieces.
    pub(super) fn fraction(&self, index: usize) -> f64 {
        index as f64 / self.pieces as f64
    }

    /// The stretch of its line from the start of piece `first` to the end of piece `last`, both included,
    /// turned to run back when `back`.
    pub(super) fn stretch(&self, first: usize, last: usize, back: bool, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let along = Along::new(&self.line, meter)?;
        let mut points = along.part(along.length() * self.fraction(first), along.length() * self.fraction(last + 1));
        if back {
            points.reverse();
        }
        Ok(points)
    }
}

/// The elements of `elements` that the route takes, in order, each read with the design's `settings`; what
/// is said of the others, and of those taken, goes to `said`. Reading charges `meter`.
pub(super) fn routables(
    elements: &[Element],
    settings: &DesignSettings,
    said: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Vec<Routable>, Exhausted> {
    let mut taken = Vec::new();
    for (index, element) in elements.iter().enumerate() {
        match read(index, element, settings, said, meter)? {
            Ok(routable) => taken.push(routable),
            Err(why) => said.push(left(element, why)),
        }
    }
    Ok(taken)
}

/// `element`, the `index`th, read as the route takes it; or why it is left as it is, the end of a
/// sentence. What its reading says goes to `said`.
fn read(
    index: usize,
    element: &Element,
    settings: &DesignSettings,
    said: &mut Vec<Diagnostic>,
    meter: &mut Meter,
) -> Result<Result<Routable, &'static str>, Exhausted> {
    let Shape::Stroke { path, width, join, .. } = &element.shape else { return Ok(Err("it is a fill, which a route doesn't take")) };
    if element.automatic {
        return Ok(Err("it is digitized automatically, which chooses its own stitches"));
    }
    let Some(stitch_type) = stitch_type_of(element, settings) else { return Ok(Err("its settings don't read")) };
    let mut own = Vec::new();
    if stitch_type.family() == Family::Satin {
        let params = match SatinParams::from_set_as(&element.params, stitch_type) {
            Ok(read) => read.params,
            Err(_) => return Ok(Err("its settings don't read")),
        };
        let rails = satin::rails(path, *width, *join, &mut own, meter)?;
        // The error that says why there are no rails is left to `SC-I1002`, as the element is left as it is.
        said.extend(own.into_iter().filter(|said| said.code.severity() != Severity::Error).map(|said| said.with_element(element.id.clone())));
        let Some(rails) = rails else { return Ok(Err("its rails can't be made")) };
        let cuttable = Cuttable::of(&rails, &params, meter)?;
        let mut line = cuttable.line();
        if let [only] = line.as_slice() {
            line.push(*only);
        }
        let length = Along::new(&line, meter)?.length();
        if length <= 0.0 {
            return Ok(Err("its centre line has no length"));
        }
        let pieces = cut_into(length);
        let outline = Outline::Lines(cuttable.rails.iter().flat_map(|rail| rail.windows(2).filter_map(|pair| pair.try_into().ok())).collect());
        return Ok(Ok(Routable { element: index, kind: Kind::Column(Box::new((cuttable, params))), line, pieces, outline }));
    }
    let flat = stroke::flatten(path, SHAPE_TOLERANCE, meter)?;
    let mut length = 0.0;
    for piece in &flat.pieces {
        length += Along::new(&piece.points, meter)?.length();
    }
    if length <= 0.0 {
        return Ok(Err("it has no length"));
    }
    let first = flat.pieces.first().map(|piece| piece.points.clone()).unwrap_or_default();
    match (first.first(), first.last()) {
        (Some(start), Some(end)) if first.len() >= 2 && start != end => {}
        _ => return Ok(Err("its first subpath ends where it starts, which shows no way between 2 places")),
    }
    let points: Vec<Point> = flat.pieces.iter().flat_map(|piece| piece.points.iter().copied()).collect();
    let outline = hull(&points, meter)?;
    Ok(Ok(Routable { element: index, kind: Kind::Stroke, line: first, pieces: 1, outline }))
}

/// How many pieces a centre line `length` mm long, more than 0, is cut into: as few as keep each at most
/// [`PIECE`] long.
pub(super) fn cut_into(length: f64) -> usize {
    // A length is finite and more than 0, so its count of pieces is a whole number of at least 1.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let count = (length / PIECE).ceil() as usize;
    count
}

/// `SC-I1002` for `element`, which the route leaves as it is because of `why`.
fn left(element: &Element, why: &str) -> Diagnostic {
    Diagnostic::new(Code::RouteLeftOut, format!("This element is left out of the route and kept as it is: {why}.")).with_element(element.id.clone())
}
