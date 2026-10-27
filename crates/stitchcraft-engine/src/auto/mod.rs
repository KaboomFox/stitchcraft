//! Automatic digitizing: the stitches of a shape without settings, chosen by its width.
//!
//! Design: `docs/src/design/image-to-embroidery.md`. A host marks a shape *automatic* when it has no settings
//! and the user asked for automatic digitizing (`--auto`): a VectorCraft object with no StitchCraft effect,
//! an SVG element with no `inkstitch:` attribute (`REQ-AUTO-001`). Before anything is generated,
//! [`digitized`] turns each automatic element into ordinary elements whose settings say what to sew, so
//! every stage after it, and every host, treats them as it treats a shape set by hand. It keeps the
//! element's other settings, such as the trim after it that an Ink/Stitch command asks for; a shape sewn
//! as several elements keeps its trim and stop for the last.
//!
//! The stitches follow the shape's width at the size it sews ([`Stitches`]), measured to the micrometre,
//! so that a width written in another unit, which converts to a bound give or take a rounding error, is
//! that bound:
//!
//! - An area is a tatami fill, with the fill's defaults (`REQ-AUTO-002`), apart from its narrow parts. A
//!   part narrower than the design's narrowest satin stroke, `min_satin_stroke_width_mm`, everywhere is a
//!   running stitch along its centre line (`narrow`, `REQ-AUTO-013`), sewn after the rest of the area and
//!   named after it with `#2`, `#3` and so on, or the area's own name when nothing of it is filled.
//! - A stroke no wider than the design's narrowest satin stroke is a running stitch along its path
//!   (`REQ-AUTO-002`): a satin column that narrow is too narrow to stitch across (`REQ-SAT-015`).
//! - A running stitch is a triple run, bean stitch 1, from [`TRIPLE_RUN_WIDTH`] wide (`REQ-AUTO-015`): a
//!   stroke as wide as it is drawn, a narrow part as wide as the circles that fit inside it along its
//!   centre line.
//! - A stroke up to [`SATIN_MAX_WIDTH`] wide is a satin column as wide as the stroke, its path the
//!   column's centre line (`REQ-AUTO-003`, `REQ-SAT-016`). A centre line is one subpath, so a stroke of
//!   several sews a column for each, in order, the second and later named after the stroke with `#2`, `#3`
//!   and so on. A traced line's ends are round or square, and the cap is part of the line drawn: an open
//!   end with such a cap reaches half the stroke's width past the path's end, as far as the cap does, along
//!   the direction the path ends in. A subpath that a column cannot follow, because it turns more tightly
//!   than the stroke is wide or is shorter than a CSS pixel, and a dot that a round or square cap draws,
//!   are filled instead, as a wider stroke is (`SC-I0902`). Whether a column can be made is found as the
//!   satin column finds it, from the rails it would make; a subpath that draws nothing, a dot with a
//!   butt cap, is left out (`SC-W0401`).
//! - A wider stroke is the tatami fill of the area it paints (`area`, `REQ-AUTO-004`): satin stitches
//!   longer than that are loose. The fill is given that area's outline, already joined into one region,
//!   so that it sews and is said of as the fill of a shape so drawn. Its parts too small to sew are kept in
//!   the outline, and are specks then, merged into a neighbour or left out (`SC-I0905`). An area that would
//!   reach more than 10 m from the origin, where no design lies, is left out (`SC-W0903`).
//!
//! A stroke that paints nothing at all, every subpath a dot with a butt cap or none, sews as it would
//! without the switch, and the running stitch says what it says of it. So a design that plans without
//! the switch plans with it (`REQ-AUTO-006`).
//!
//! [`digitized`] also says how many shapes it digitized and what they sew as (`SC-I0901`). Each automatic
//! element has the budget's work to itself, as each element has for its generator; one that needs more
//! is left out with `SC-E0004`, and the rest of the design still plans (`REQ-GEN-002`).
//!
//! The shapes of a traced picture meet, and [`digitized`] then sews them as neighbours (`auto::neighbours`):
//! the lowest shape is left out when it is the picture's background (`REQ-AUTO-009`), specks merge into
//! the areas beside them (`REQ-AUTO-008`), the elements sew in an order of their own, with few thread
//! changes (`REQ-AUTO-010`), and each automatic area reaches under the shapes sewn after it, or is cut away
//! under the areas that cover it (`REQ-AUTO-007`). A design that asks for its threads sorted
//! (`--sort-colours`, [`Design::sort_colours`]) has every element ordered so, its elements with settings
//! too, with automatic elements or without (`REQ-AUTO-011`). A narrow part's line paints the part to its
//! neighbours, and the areas sewn before it reach under it.

mod area;
mod narrow;
mod neighbours;

pub use neighbours::{OVERLAP, SMALLEST_AREA, TOUCHING};
use neighbours::{Sewn, no_area, parts_of};

use std::borrow::Cow;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Labels, Meter, Mm, Point};
use stitchcraft_params::ParamSet;

use crate::common::{STOP_AFTER, TRIM_AFTER};
use crate::design::{Cap, Design, DesignSettings, Element, FillRule, Join, Path, Segment, Shape, Subpath, within_working_limit};
use crate::generators::passes::BEAN_STITCH_REPEATS;
use crate::generators::satin::{self, SATIN_COLUMN};
use crate::generators::{TooSmall, mm, too_small};
use crate::normalize::region::{self, Polygon, Unjoined};
use crate::normalize::strips::Line;
use crate::normalize::stroke;

/// The widest stroke sewn as a satin column; a wider one is filled. MC-8 confirms or changes it on the
/// machine (`docs/src/plan/machine-testing.md`).
pub const SATIN_MAX_WIDTH: Mm = Mm::from_tenths(70);

/// The width from which a running stitch that automatic digitizing makes is a triple run, each stitch sewn 3
/// times (bean stitch 1): a running stitch sewn once is as fine as the thread. MC-8 confirms or changes it.
pub const TRIPLE_RUN_WIDTH: Mm = Mm::from_tenths(5);

/// Room left in an id made for a column of a stroke of several subpaths, for the `~2` a clash adds
/// ([`Labels::claim`]).
const CLAIM_ROOM: usize = 12;

/// The settings that act after an element: a shape sewn as several elements keeps them for its last.
const AFTER: [&str; 2] = [TRIM_AFTER, STOP_AFTER];

/// The stitches automatic digitizing gives a shape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stitches {
    /// A tatami fill of the area.
    Fill,
    /// A running stitch along the stroke's path.
    Running,
    /// A satin column as wide as the stroke, along its path.
    Satin,
    /// A tatami fill of the area the stroke paints.
    FilledStroke,
}

impl Stitches {
    /// The stitches of a stroke `width` wide in a design with `settings`: a running stitch no wider than
    /// the design's narrowest satin stroke, a satin column no wider than [`SATIN_MAX_WIDTH`], and a fill
    /// wider than both, each width measured to the micrometre.
    pub fn of_stroke(width: Mm, settings: &DesignSettings) -> Stitches {
        if at_most(width, settings.min_satin_stroke_width) {
            Stitches::Running
        } else if at_most(width, SATIN_MAX_WIDTH) {
            Stitches::Satin
        } else {
            Stitches::FilledStroke
        }
    }

    /// Whether the stitches cover the stroke's width, as a satin column or a fill does, so that what a host
    /// draws across a stroke matters to them: its alignment and its width profile, or how unevenly its
    /// transforms stretch it (`REQ-AUTO-005`).
    pub const fn across(self) -> bool {
        matches!(self, Stitches::Satin | Stitches::FilledStroke)
    }
}

/// Whether `width` is no wider than `bound`, both measured to the micrometre.
fn at_most(width: Mm, bound: Mm) -> bool {
    micrometres(width.get()) <= micrometres(bound.get())
}

/// `length` millimetres in whole micrometres.
fn micrometres(length: f64) -> f64 {
    (length * 1000.0).round()
}

/// Whether a running stitch `width` millimetres wide is a triple run: from [`TRIPLE_RUN_WIDTH`], to the
/// micrometre.
fn tripled(width: f64) -> bool {
    micrometres(width) >= micrometres(TRIPLE_RUN_WIDTH.get())
}

/// What [`digitize`] makes of an automatic element.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Digitized {
    /// The ordinary elements that sew it, in order; none when it is left out.
    pub elements: Vec<Element>,
    /// For each of the elements, in order, the narrow part of an area that it sews along the centre line
    /// of, which it paints to its neighbours; `None` for any other element.
    pub narrow: Vec<Option<Polygon>>,
    /// What is said of it: `SC-I0902` for each part filled because a satin column cannot follow it,
    /// `SC-W0401` for each part that draws nothing, `SC-W0903` for an area that reaches too far, and
    /// `SC-E0009` for a part of an area whose width or centre line could not be found, a bug.
    pub diagnostics: Vec<Diagnostic>,
    /// The stitches it sews as, as `SC-I0901` counts them: a stroke whose every part is filled is a fill,
    /// and an area whose every part runs is a running stitch. `None` when it sews nothing, or as it would
    /// without the switch because it paints nothing.
    pub stitches: Option<Stitches>,
}

impl Digitized {
    /// Adds `element`, which sews along the centre line of the narrow part `narrow` when it has one.
    fn add(&mut self, element: Element, narrow: Option<Polygon>) {
        self.elements.push(element);
        self.narrow.push(narrow);
    }
}

/// The elements of `design` as they are generated, in the order they sew: each automatic element turned
/// into the elements its stitches need ([`digitize`]) and named, the others as they are, and the elements
/// then sewn as neighbours (`auto::neighbours`); and what was said of them: `SC-I0901` for the shapes
/// digitized, what [`digitize`] says of each, `SC-E0004` for each one left out because digitizing and naming
/// it needs more work than `budget` allows an element, and what is said of the background, the specks, the
/// order and the overlaps (`SC-W0906`, `SC-I0905`, `SC-I0907`, `SC-I0904`). A design without automatic
/// elements that doesn't ask for its threads sorted comes back as it is.
pub fn digitized<'d>(design: &'d Design, budget: &Budget) -> (Cow<'d, [Element]>, Vec<Diagnostic>) {
    let elements = design.elements();
    if !design.sort_colours && !elements.iter().any(|element| element.automatic) {
        return (Cow::Borrowed(elements), Vec::new());
    }
    let mut diagnostics = Vec::new();
    // The ids given out: the design's, then each part's that a stroke of several subpaths adds. A design's
    // ids are unique (`Design::new`), so each is taken as it is, which costs no work and cannot fail.
    let mut taken = Labels::default();
    for element in elements {
        let _ = taken.claim(element.id.as_str().to_string(), &mut budget.meter());
    }
    // The picture's background is left out before anything is digitized.
    let background = neighbours::background(design, budget);
    let lowest = background.as_ref().and_then(|(lowest, _)| *lowest);
    let mut counts = Counts::default();
    let mut sewn = Vec::with_capacity(elements.len());
    for (index, element) in elements.iter().enumerate() {
        if lowest == Some(index) {
            continue;
        }
        if !element.automatic {
            sewn.push(Sewn { element: element.clone(), area: false, automatic: false, shape: index, narrow: None });
            continue;
        }
        let mut meter = budget.meter();
        let made = match digitize(element, &design.settings, &mut meter).and_then(|made| named(made, element, &mut taken, &mut meter)) {
            Ok(made) => made,
            Err(exhausted) => {
                diagnostics.push(exhausted.diagnostic(budget, Some(element.id.clone())));
                continue;
            }
        };
        counts.add(made.stitches);
        diagnostics.extend(made.diagnostics);
        let part = |(element, narrow): (Element, Option<Polygon>)| Sewn {
            area: matches!(element.shape, Shape::Fill { .. }),
            element,
            automatic: true,
            shape: index,
            narrow,
        };
        sewn.extend(made.elements.into_iter().zip(made.narrow).map(part));
    }
    diagnostics.splice(0..0, counts.said());
    diagnostics.extend(background.map(|(_, said)| said));
    let (out, said) = neighbours::beside(sewn, &design.settings, budget, design.sort_colours);
    diagnostics.extend(said);
    (Cow::Owned(out), diagnostics)
}

/// `made`, the parts of the automatic `element`, named for the plan with `meter`'s work: each keeps its id,
/// the element's own for the first, unless another element of the design has it, when [`Labels::claim`]
/// steps it aside. Naming the parts is part of the work of digitizing the element, so an element whose
/// parts need more work to name than is left is left out whole, as one that needs more to digitize is.
fn named(made: Digitized, element: &Element, taken: &mut Labels, meter: &mut Meter) -> Result<Digitized, Exhausted> {
    let mut parts = Vec::with_capacity(made.elements.len());
    for part in made.elements {
        if part.id == element.id {
            parts.push(part);
            continue;
        }
        let label = taken.claim(part.id.as_str().to_string(), meter)?;
        // A label a clash steps aside to fits in an id: `numbered` leaves room for a suffix of 11 digits,
        // and a clash steps only past labels already taken, far fewer than 10^11.
        let id = ElementId::new(label).ok().ok_or(Exhausted::Work)?;
        parts.push(Element { id, ..part });
    }
    Ok(Digitized { elements: parts, ..made })
}

/// The ordinary elements that sew the automatic `element` of a design with `settings`, with `meter`'s
/// work, and what is said of them: an area's fill and a running stitch for each of its narrow parts, a
/// stroke that runs as it is, tripled from [`TRIPLE_RUN_WIDTH`], a satin column for each of a stroke's
/// subpaths, or the fill of a part a column cannot follow, when it is a satin column, and the area it
/// paints when it is filled. A stroke that paints nothing is itself as it is.
pub fn digitize(element: &Element, settings: &DesignSettings, meter: &mut Meter) -> Result<Digitized, Exhausted> {
    let as_it_is = |stitches| itself(element, stitches);
    let Shape::Stroke { path, width, join, cap } = &element.shape else { return areas(element, settings, meter) };
    let style = Style { width: *width, join: *join, cap: *cap };
    let made = match Stitches::of_stroke(*width, settings) {
        Stitches::Fill | Stitches::Running => {
            let mut made = as_it_is(Some(Stitches::Running));
            if tripled(width.get()) {
                for element in &mut made.elements {
                    element.params.set(BEAN_STITCH_REPEATS, "1");
                }
            }
            return Ok(made);
        }
        Stitches::Satin => columns(element, path, style, meter)?,
        Stitches::FilledStroke => filled(element, path, style, meter)?,
    };
    // A stroke that paints nothing sews as it would without the switch, and says what it would say then.
    if made.elements.is_empty() && made.diagnostics.iter().all(|d| d.code == Code::StrokeTooSmall) {
        return Ok(as_it_is(None));
    }
    Ok(made)
}

/// The elements that sew `element`, an automatic area of a design with `settings`, with `meter`'s work:
/// itself as it is when no part of it is narrow, or else the fill of its other parts, if any, and a running
/// stitch along the centre lines of each narrow part, tripled from [`TRIPLE_RUN_WIDTH`] (`narrow`). The first
/// keeps the area's name and those after it are numbered, and a trim or a stop after the area follows the
/// last. An area whose region the region code can't join, a bug, is itself as it is, and its fill says so.
fn areas(element: &Element, settings: &DesignSettings, meter: &mut Meter) -> Result<Digitized, Exhausted> {
    let as_it_is = itself(element, Some(Stitches::Fill));
    let Shape::Fill { path, rule } = &element.shape else { return Ok(as_it_is) };
    let Ok(parts) = parts_of(path, *rule, meter)? else { return Ok(as_it_is) };
    let sorted = narrow::sorted(&element.id, parts, settings.min_satin_stroke_width, meter)?;
    if sorted.narrow.is_empty() {
        return Ok(Digitized { diagnostics: sorted.said, ..as_it_is });
    }
    let (rest, after) = split_after(&element.params);
    let mut made = Digitized { diagnostics: sorted.said, ..Digitized::default() };
    made.stitches = Some(if sorted.filled.is_empty() { Stitches::Running } else { Stitches::Fill });
    if !sorted.filled.is_empty() {
        let shape = Shape::Fill { path: outline(&sorted.filled), rule: FillRule::NonZero };
        made.add(part(element, element.id.clone(), shape, rest.clone()), None);
    }
    for narrow in sorted.narrow {
        let id = if made.elements.is_empty() { Some(element.id.clone()) } else { numbered(&element.id, made.elements.len() + 1) };
        // A numbered id fits by construction ([`numbered`]); one that did not would only leave the part out.
        let Some(id) = id else { continue };
        let mut params = rest.clone();
        if tripled(narrow.width.get()) {
            params.set(BEAN_STITCH_REPEATS, "1");
        }
        let path = Path { subpaths: narrow.lines.iter().filter_map(subpath).collect() };
        let shape = Shape::Stroke { path, width: narrow.width, join: Join::Round, cap: Cap::Round };
        made.add(part(element, id, shape, params), Some(narrow.part));
    }
    keep_after(&mut made.elements, &after);
    Ok(made)
}

/// The automatic `element` sewn as itself, an ordinary element, counted as `stitches`.
fn itself(element: &Element, stitches: Option<Stitches>) -> Digitized {
    Digitized { elements: vec![Element { automatic: false, ..element.clone() }], narrow: vec![None], diagnostics: Vec::new(), stitches }
}

/// The centre line `line` as a subpath.
fn subpath(line: &Line) -> Option<Subpath> {
    let (&start, rest) = line.points.split_first()?;
    Some(Subpath { start, segments: rest.iter().map(|&p| Segment::Line(p)).collect(), closed: line.closed })
}

/// How a stroke is drawn across its path.
#[derive(Clone, Copy)]
struct Style {
    width: Mm,
    join: Join,
    cap: Cap,
}

/// The satin columns of `element`, a stroke along `path` drawn with `style`: one for each subpath, whose
/// open ends reach as far as their caps do, each with the settings of `element` and `satin_column` on. A
/// subpath a column cannot follow is the fill of the area it paints, with `SC-I0902`, and one that draws
/// nothing is left out, with `SC-W0401`.
fn columns(element: &Element, path: &Path, style: Style, meter: &mut Meter) -> Result<Digitized, Exhausted> {
    let (rest, after) = split_after(&element.params);
    let mut column = rest.clone();
    column.set(SATIN_COLUMN, "true");
    let mut made = Digitized::default();
    let (mut satin, mut fill) = (false, false);
    for (n, subpath) in path.subpaths.iter().enumerate() {
        // A unit for the subpath; its segments cost what the code that walks them charges.
        meter.charge(1)?;
        let id = if n == 0 { Some(element.id.clone()) } else { numbered(&element.id, n + 1) };
        // A numbered id fits by construction ([`numbered`]); one that did not would only leave the part out.
        let Some(id) = id else { continue };
        // A subpath that does not move is a dot, which no column sews.
        let directions = stroke::end_directions(subpath, meter)?;
        if let Some(directions) = directions {
            let line = Path { subpaths: vec![centre_line(subpath, directions, style)] };
            // The column's rails as the satin column makes them; what it would say of them it says itself.
            if satin::rails(&line, style.width, style.join, &mut Vec::new(), meter)?.is_some() {
                let shape = Shape::Stroke { path: line, width: style.width, join: style.join, cap: style.cap };
                made.add(part(element, id, shape, column.clone()), None);
                satin = true;
                continue;
            }
        }
        let alone = Path { subpaths: vec![subpath.clone()] };
        match painted(&alone, style, meter)? {
            Painted::Area { rings, parts } => {
                made.diagnostics.push(cannot_follow(&id, directions.is_none()));
                made.add(part(element, id, Shape::Fill { path: fill_path(parts, &rings), rule: FillRule::NonZero }, rest.clone()), None);
                fill = true;
            }
            Painted::Nothing => made.diagnostics.push(too_small(TooSmall::Point, 0.0).with_element(element.id.clone())),
            Painted::TooFar => made.diagnostics.push(too_far(element, style.width)),
        }
    }
    keep_after(&mut made.elements, &after);
    made.stitches = if satin { Some(Stitches::Satin) } else { fill.then_some(Stitches::FilledStroke) };
    Ok(made)
}

/// The centre line of the satin column along `subpath`, drawn with `style`, which starts and ends in the
/// `directions` given: the subpath, its open ends reaching as far as a round or a square cap does,
/// straight on.
fn centre_line(subpath: &Subpath, (start, end): stroke::Ends, style: Style) -> Subpath {
    // A butt cap ends at the path's end, and a closed subpath has no end.
    if subpath.closed || style.cap == Cap::Butt {
        return subpath.clone();
    }
    let reach = style.width.get() / 2.0;
    let finish = subpath.segments.last().map_or(subpath.start, |s| s.end());
    let (Some(before), Some(after)) = (along(subpath.start, start, -reach), along(finish, end, reach)) else { return subpath.clone() };
    let mut segments = Vec::with_capacity(subpath.segments.len() + 2);
    segments.push(Segment::Line(subpath.start));
    segments.extend(subpath.segments.iter().copied());
    segments.push(Segment::Line(after));
    Subpath { start: before, segments, closed: false }
}

/// The point `by` millimetres from `at` in `direction` (of any length), backwards for a negative `by`.
fn along(at: Point, (dx, dy): (f64, f64), by: f64) -> Option<Point> {
    let length = stitchcraft_core::math::hypot(dx, dy);
    (length > 0.0).then(|| Point::new(at.x() + dx / length * by, at.y() + dy / length * by).ok()).flatten()
}

/// `element`, a stroke along `path` drawn with `style`, as the fill of the area it paints; left out with
/// `SC-W0903` when that area reaches too far.
fn filled(element: &Element, path: &Path, style: Style, meter: &mut Meter) -> Result<Digitized, Exhausted> {
    let mut made = Digitized::default();
    match painted(path, style, meter)? {
        Painted::Area { rings, parts } => {
            let shape = Shape::Fill { path: fill_path(parts, &rings), rule: FillRule::NonZero };
            made.add(part(element, element.id.clone(), shape, element.params.clone()), None);
            made.stitches = Some(Stitches::FilledStroke);
        }
        Painted::Nothing => made.diagnostics.push(too_small(TooSmall::Point, 0.0).with_element(element.id.clone())),
        Painted::TooFar => made.diagnostics.push(too_far(element, style.width)),
    }
    Ok(made)
}

/// What a stroke paints.
enum Painted {
    /// An area: the rings round the pieces of the stroke, and every part of the area they bound by the
    /// nonzero rule, unless the region code could not join them (a bug).
    Area { rings: Vec<Vec<Point>>, parts: Result<Vec<Polygon>, Unjoined> },
    /// Nothing: every subpath is a dot with a butt cap, or only a move.
    Nothing,
    /// An area that would reach more than 10 m from the origin, where no design lies.
    TooFar,
}

/// What the stroke along `path` drawn with `style` paints ([`area::stroke_area`]): every part of its area,
/// parts too small to sew included, so that they are specks as a shape's are. A wide line is filled with it,
/// and the sewing order finds a line's neighbours by it (`auto::neighbours`).
fn painted(path: &Path, style: Style, meter: &mut Meter) -> Result<Painted, Exhausted> {
    let rings = area::stroke_area(path, style.width.get(), style.join, style.cap, meter)?;
    if rings.is_empty() {
        return Ok(Painted::Nothing);
    }
    if !rings.iter().flatten().all(|&p| within_working_limit(p)) {
        return Ok(Painted::TooFar);
    }
    let parts = region::area(&rings, |winding| region::fills(FillRule::NonZero, winding), meter)?.parts;
    Ok(Painted::Area { rings, parts })
}

/// The path a fill is given for the area that `rings` bound: the outline of each of its `parts`. An area
/// that cannot be joined is a bug in the region code; the fill is then given the rings, meets it there and
/// says so itself.
fn fill_path(parts: Result<Vec<Polygon>, Unjoined>, rings: &[Vec<Point>]) -> Path {
    match parts {
        Ok(parts) => outline(&parts),
        Err(_) => Path { subpaths: rings.iter().map(Vec::as_slice).filter_map(ring_subpath).collect() },
    }
}

/// The rings of `parts` as a path: each part's outline, then its holes, wound as [`Polygon`] winds them,
/// so that the nonzero rule leaves the holes empty.
fn outline(parts: &[Polygon]) -> Path {
    Path { subpaths: parts.iter().flat_map(Polygon::rings).filter_map(ring_subpath).collect() }
}

/// The closed ring `ring`, its first point repeated at its end, as a closed subpath.
fn ring_subpath(ring: &[Point]) -> Option<Subpath> {
    let (&start, rest) = ring.split_first()?;
    let rest = rest.strip_suffix(&[start]).unwrap_or(rest);
    Some(Subpath { start, segments: rest.iter().map(|&q| Segment::Line(q)).collect(), closed: true })
}

/// An ordinary element that sews `shape` as a part of `element`, named `id`, with `params`.
fn part(element: &Element, id: ElementId, shape: Shape, params: ParamSet) -> Element {
    Element { id, name: element.name.clone(), shape, thread: element.thread.clone(), params, automatic: false }
}

/// `params` without the settings that act after an element, and those settings.
fn split_after(params: &ParamSet) -> (ParamSet, Vec<(String, String)>) {
    let rest = params.iter().filter(|(key, _)| !AFTER.contains(key)).collect();
    let after = params.iter().filter(|(key, _)| AFTER.contains(key)).map(|(key, value)| (key.to_string(), value.to_string())).collect();
    (rest, after)
}

/// Gives the last of `elements` the settings `after` it.
fn keep_after(elements: &mut [Element], after: &[(String, String)]) {
    if let Some(last) = elements.last_mut() {
        for (key, value) in after {
            last.params.set(key.as_str(), value.as_str());
        }
    }
}

/// `SC-I0902` for the part `id` of a line, filled because a satin column cannot follow it: a `dot` its cap
/// draws, or a line that turns too tightly or is too short.
fn cannot_follow(id: &ElementId, dot: bool) -> Diagnostic {
    let message = if dot {
        "This part of the line is a dot, which its cap draws and a satin column cannot sew, so the dot is filled."
    } else {
        "A satin column cannot follow this part of the line, which turns more tightly than the line is wide or is shorter than a CSS \
         pixel, so the area it paints is filled."
    };
    Diagnostic::new(Code::AutoSatinFilled, message)
        .with_element(id.clone())
        .with_fix(Fix::Hint("To sew it as a satin column, draw the line with gentler turns or narrower.".to_string()))
}

/// `SC-W0903` for `element`, a line `width` wide whose area reaches too far to fill.
fn too_far(element: &Element, width: Mm) -> Diagnostic {
    let message = format!(
        "This line is {} mm wide, and the area it paints would reach more than 10 m from the origin, beyond what machine files can \
         record, so it is left out.",
        mm(width.get())
    );
    Diagnostic::new(Code::AutoAreaTooLarge, message)
        .with_element(element.id.clone())
        .with_fix(Fix::Hint("Check the line's stroke width.".to_string()))
}

/// `id` with `#n` after it, shortened as needed to leave room for the `~2` a clash adds.
fn numbered(id: &ElementId, n: usize) -> Option<ElementId> {
    let suffix = format!("#{n}");
    let room = ElementId::MAX_LEN.saturating_sub(suffix.len() + CLAIM_ROOM);
    let mut base = String::new();
    for c in id.as_str().chars() {
        if base.len() + c.len_utf8() > room {
            break;
        }
        base.push(c);
    }
    ElementId::new(base + &suffix).ok()
}

/// How many shapes were digitized, by the stitches they sew as, as `SC-I0901` names them.
#[derive(Default)]
struct Counts {
    running: usize,
    satin: usize,
    fills: usize,
}

impl Counts {
    fn add(&mut self, stitches: Option<Stitches>) {
        let count = match stitches {
            None => return,
            Some(Stitches::Running) => &mut self.running,
            Some(Stitches::Satin) => &mut self.satin,
            Some(Stitches::Fill | Stitches::FilledStroke) => &mut self.fills,
        };
        *count = count.saturating_add(1);
    }

    /// `SC-I0901`, saying how many shapes were digitized and how each sews; `None` when none were.
    fn said(&self) -> Option<Diagnostic> {
        let kinds: Vec<(usize, &str, &str)> = [
            (self.running, "a running stitch", "running stitches"),
            (self.satin, "a satin column", "satin columns"),
            (self.fills, "a tatami fill", "tatami fills"),
        ]
        .into_iter()
        .filter(|(count, ..)| *count > 0)
        .collect();
        let total: usize = kinds.iter().map(|(count, ..)| count).sum();
        let how = match kinds.as_slice() {
            [] => return None,
            [(1, one, _)] => format!("it sews as {one}"),
            [(2, _, many)] => format!("both sew as {many}"),
            [(_, _, many)] => format!("all sew as {many}"),
            [(count, one, many), rest @ ..] => {
                let (count, one, many) = (*count, *one, *many);
                let form = |count: usize, one: &str, many: &str| if count == 1 { one.to_string() } else { many.to_string() };
                let verb = if count == 1 { "sews" } else { "sew" };
                let mut parts = vec![format!("{count} {verb} as {}", form(count, one, many))];
                parts.extend(rest.iter().map(|&(count, one, many)| format!("{count} as {}", form(count, one, many))));
                // At least 2 parts: "a and b", or "a, b and c".
                let last = parts.pop().unwrap_or_default();
                format!("{} and {last}", parts.join(", "))
            }
        };
        let shapes = if total == 1 { "1 shape is".to_string() } else { format!("{total} shapes are") };
        Some(Diagnostic::new(Code::AutoDigitized, format!("{shapes} digitized automatically: {how}.")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_id_numbered_for_a_column_fits_with_room_for_a_clash() {
        // As long as an id can be, it is shortened just enough to leave the room, and never within a
        // character.
        for (long, kept) in [("a".repeat(256), 241), ("é".repeat(128), 240)] {
            let numbered = numbered(&ElementId::new(long.as_str()).unwrap(), 12).unwrap();
            assert_eq!(numbered.as_str(), format!("{}#12", &long[..kept]));
            assert!(numbered.as_str().len() + CLAIM_ROOM <= ElementId::MAX_LEN);
        }
        assert_eq!(numbered(&ElementId::new("a").unwrap(), 2).unwrap().as_str(), "a#2");
    }

    #[test]
    fn a_fill_is_given_the_outline_of_its_area_or_else_its_rings() {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let triangle = |points: [(f64, f64); 3]| {
            let [start, rest @ ..] = points.map(|(x, y)| p(x, y));
            Path { subpaths: vec![Subpath { start, segments: rest.into_iter().map(Segment::Line).collect(), closed: true }] }
        };
        // A triangle wound counter-clockwise in y-up axes.
        let rings = [vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 0.0)]];
        let found = region::area(&rings, |winding| region::fills(FillRule::NonZero, winding), &mut Budget::DEFAULT.meter()).unwrap();
        // Joined: the area's outline, wound clockwise as a `Polygon` winds it.
        assert_eq!(fill_path(found.parts, &rings), triangle([(0.0, 0.0), (10.0, 10.0), (10.0, 0.0)]));
        // An area that cannot be joined, a bug in the region code: the rings as they are.
        assert_eq!(fill_path(Err(Unjoined), &rings), triangle([(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]));
    }

    #[test]
    fn the_explanation_names_the_widest_satin_column() {
        assert!(Code::AutoDigitized.explanation().contains(&format!("{} mm", SATIN_MAX_WIDTH.get())));
    }
}
