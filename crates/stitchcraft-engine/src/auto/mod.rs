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
//! - An area is a tatami fill, with the fill's defaults (`REQ-AUTO-002`).
//! - A stroke no wider than the design's narrowest satin stroke, `min_satin_stroke_width_mm`, is a running
//!   stitch along its path (`REQ-AUTO-002`): a satin column that narrow is too narrow to stitch across
//!   (`REQ-SAT-015`).
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
//!   so that it sews and is said of as the fill of a shape so drawn: parts of it too small to sew are
//!   kept in the outline for the fill to name (`SC-W0303`). An area that would reach more than 10 m from the
//!   origin, where no design lies, is left out (`SC-W0903`).
//!
//! A stroke that paints nothing at all, every subpath a dot with a butt cap or none, sews as it would
//! without the switch, and the running stitch says what it says of it. So a design that plans without
//! the switch plans with it (`REQ-AUTO-006`).
//!
//! [`digitized`] also says how many shapes it digitized and what they sew as (`SC-I0901`). Each automatic
//! element has the budget's work to itself, as each element has for its generator; one that needs more
//! is left out with `SC-E0004`, and the rest of the design still plans (`REQ-GEN-002`).

mod area;

use std::borrow::Cow;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Labels, Meter, Mm, Point};
use stitchcraft_params::ParamSet;

use crate::common::{STOP_AFTER, TRIM_AFTER};
use crate::design::{Cap, Design, DesignSettings, Element, FillRule, Join, Path, Segment, Shape, Subpath, within_working_limit};
use crate::generators::satin::{self, SATIN_COLUMN};
use crate::generators::{TooSmall, mm, too_small};
use crate::normalize::region::{self, Polygon};
use crate::normalize::stroke;

/// The widest stroke sewn as a satin column; a wider one is filled. MC-8 confirms or changes it on the
/// machine (`docs/src/plan/machine-testing.md`).
pub const SATIN_MAX_WIDTH: Mm = Mm::from_tenths(70);

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
    /// The stitches of `shape` in a design with `settings`.
    pub fn of(shape: &Shape, settings: &DesignSettings) -> Stitches {
        match shape {
            Shape::Fill { .. } => Stitches::Fill,
            Shape::Stroke { width, .. } => Stitches::of_stroke(*width, settings),
        }
    }

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
    let micrometres = |length: Mm| (length.get() * 1000.0).round();
    micrometres(width) <= micrometres(bound)
}

/// What [`digitize`] makes of an automatic element.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Digitized {
    /// The ordinary elements that sew it, in order; none when it is left out.
    pub elements: Vec<Element>,
    /// What is said of it: `SC-I0902` for each part filled because a satin column cannot follow it,
    /// `SC-W0401` for each part that draws nothing, and `SC-W0903` for an area that reaches too far.
    pub diagnostics: Vec<Diagnostic>,
    /// The stitches it sews as, as `SC-I0901` counts them: a stroke whose every part is filled is a fill.
    /// `None` when it sews nothing, or as it would without the switch because it paints nothing.
    pub stitches: Option<Stitches>,
}

/// The elements of `design` as they are generated: each automatic element turned into the elements its
/// stitches need ([`digitize`]), the others as they are; and what was said of them: `SC-I0901` for the
/// shapes digitized, what [`digitize`] says of each, and `SC-E0004` for each one left out because it needs
/// more work than `budget` allows. A design without automatic elements comes back as it is.
pub fn digitized<'d>(design: &'d Design, budget: &Budget) -> (Cow<'d, [Element]>, Vec<Diagnostic>) {
    let elements = design.elements();
    if !elements.iter().any(|element| element.automatic) {
        return (Cow::Borrowed(elements), Vec::new());
    }
    let mut diagnostics = Vec::new();
    // The ids given out: the design's, then each part's that a stroke of several subpaths adds.
    let (mut taken, mut naming) = (Labels::default(), budget.meter());
    let mut counts = Counts::default();
    let mut out = Vec::with_capacity(elements.len());
    for element in elements {
        // A design's ids are unique (`Design::new`), so each is taken as it is, which costs no work and
        // cannot fail.
        let _ = taken.claim(element.id.as_str().to_string(), &mut naming);
    }
    for element in elements {
        if !element.automatic {
            out.push(element.clone());
            continue;
        }
        let made = match digitize(element, &design.settings, &mut budget.meter()) {
            Ok(made) => made,
            Err(exhausted) => {
                diagnostics.push(exhausted.diagnostic(budget, Some(element.id.clone())));
                continue;
            }
        };
        counts.add(made.stitches);
        diagnostics.extend(made.diagnostics);
        for part in made.elements {
            if part.id == element.id {
                out.push(part);
                continue;
            }
            // A part's own id, unless another element already has it.
            let id = taken.claim(part.id.as_str().to_string(), &mut naming).ok().and_then(|id| ElementId::new(id).ok());
            match id {
                Some(id) => out.push(Element { id, ..part }),
                None => {
                    let message = format!("A part of `{}` could not be named: it needed more work than the budget allows.", element.id);
                    diagnostics.push(Diagnostic::new(Code::BudgetExhausted, message).with_element(element.id.clone()));
                }
            }
        }
    }
    diagnostics.splice(0..0, counts.said());
    (Cow::Owned(out), diagnostics)
}

/// The ordinary elements that sew the automatic `element` of a design with `settings`, with `meter`'s
/// work, and what is said of them: itself as it is when it is an area or runs, a satin column for each of
/// its subpaths, or the fill of a part a column cannot follow, when it is a satin column, and the area it
/// paints when it is filled. A stroke that paints nothing is itself as it is.
pub fn digitize(element: &Element, settings: &DesignSettings, meter: &mut Meter) -> Result<Digitized, Exhausted> {
    let as_it_is = |stitches| Digitized { elements: vec![Element { automatic: false, ..element.clone() }], diagnostics: Vec::new(), stitches };
    let Shape::Stroke { path, width, join, cap } = &element.shape else { return Ok(as_it_is(Some(Stitches::Fill))) };
    let style = Style { width: *width, join: *join, cap: *cap };
    let made = match Stitches::of_stroke(*width, settings) {
        Stitches::Fill | Stitches::Running => return Ok(as_it_is(Some(Stitches::Running))),
        Stitches::Satin => columns(element, path, style, meter)?,
        Stitches::FilledStroke => filled(element, path, style, meter)?,
    };
    // A stroke that paints nothing sews as it would without the switch, and says what it would say then.
    if made.elements.is_empty() && made.diagnostics.iter().all(|d| d.code == Code::StrokeTooSmall) {
        return Ok(as_it_is(None));
    }
    Ok(made)
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
        meter.charge(1 + subpath.segments.len() as u64)?;
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
                made.elements.push(part(element, id, shape, column.clone()));
                satin = true;
                continue;
            }
        }
        let alone = Path { subpaths: vec![subpath.clone()] };
        match painted(&alone, style, meter)? {
            Painted::Area(area) => {
                made.diagnostics.push(cannot_follow(&id, directions.is_none()));
                made.elements.push(part(element, id, Shape::Fill { path: area, rule: FillRule::NonZero }, rest.clone()));
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
        Painted::Area(area) => {
            made.elements.push(part(element, element.id.clone(), Shape::Fill { path: area, rule: FillRule::NonZero }, element.params.clone()));
            made.stitches = Some(Stitches::FilledStroke);
        }
        Painted::Nothing => made.diagnostics.push(too_small(TooSmall::Point, 0.0).with_element(element.id.clone())),
        Painted::TooFar => made.diagnostics.push(too_far(element, style.width)),
    }
    Ok(made)
}

/// What a stroke paints.
enum Painted {
    /// An area: its outline, its parts joined, filled by the nonzero rule.
    Area(Path),
    /// Nothing: every subpath is a dot with a butt cap, or only a move.
    Nothing,
    /// An area that would reach more than 10 m from the origin, where no design lies.
    TooFar,
}

/// What the stroke along `path` drawn with `style` paints ([`area::stroke_area`]): the outline of every part
/// of its area, parts too small to sew included, so that the fill names them as it names a shape's.
fn painted(path: &Path, style: Style, meter: &mut Meter) -> Result<Painted, Exhausted> {
    let rings = area::stroke_area(path, style.width.get(), style.join, style.cap, meter)?;
    if rings.is_empty() {
        return Ok(Painted::Nothing);
    }
    if !rings.iter().flatten().all(|&p| within_working_limit(p)) {
        return Ok(Painted::TooFar);
    }
    let found = region::area(&rings, |winding| region::fills(FillRule::NonZero, winding), meter)?;
    Ok(Painted::Area(match found.parts {
        Ok(parts) => outline(&parts),
        // An area that cannot be joined is a bug in the region code; the fill then meets it on the rings and
        // says so itself.
        Err(_) => Path { subpaths: rings.iter().map(Vec::as_slice).filter_map(ring_subpath).collect() },
    }))
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
        let long = ElementId::new("é".repeat(128)).unwrap();
        let numbered = numbered(&long, 12).unwrap();
        assert!(numbered.as_str().ends_with("#12") && numbered.as_str().len() + CLAIM_ROOM <= ElementId::MAX_LEN, "{numbered}");
        assert_eq!(super::numbered(&ElementId::new("a").unwrap(), 2).unwrap().as_str(), "a#2");
    }

    #[test]
    fn the_explanation_names_the_widest_satin_column() {
        assert!(Code::AutoDigitized.explanation().contains(&format!("{} mm", SATIN_MAX_WIDTH.get())));
    }
}
