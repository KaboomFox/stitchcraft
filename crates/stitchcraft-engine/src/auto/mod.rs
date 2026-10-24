//! Automatic digitizing: the stitches of a shape without settings, chosen by its width.
//!
//! Design: `docs/src/design/image-to-embroidery.md`. A host marks a shape *automatic* when it has no settings
//! and the user asked for automatic digitizing (`--auto`): a VectorCraft object with no StitchCraft effect,
//! an SVG element with no `inkstitch:` attribute (`REQ-AUTO-001`). Before anything is generated,
//! [`digitized`] turns each automatic element into ordinary elements whose settings say what to sew, so
//! every stage after it, and every host, treats them as it treats a shape set by hand. It keeps the
//! element's other settings, such as the trim after it that an Ink/Stitch command asks for.
//!
//! The stitches follow the shape's width at the size it sews ([`Stitches`]):
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
//!   the direction the path ends in.
//! - A wider stroke is the tatami fill of the area it paints (`area`, `REQ-AUTO-004`): satin stitches
//!   longer than that are loose. The fill is given that area's outline, already joined into one region,
//!   so that it sews and is said of as the fill of a shape so drawn.
//!
//! [`digitized`] also says how many shapes it digitized and how (`SC-I0901`). Each automatic element has
//! the budget's work to itself, as each element has for its generator; one that needs more is left out
//! with `SC-E0004`, and the rest of the design still plans (`REQ-GEN-002`).

mod area;

use std::borrow::Cow;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Labels, Meter, Mm, Point};

use crate::design::{Cap, Design, DesignSettings, Element, FillRule, Join, Path, Segment, Shape, Subpath};
use crate::generators::satin::SATIN_COLUMN;
use crate::normalize::region::{self, Region};

/// The widest stroke sewn as a satin column; a wider one is filled. MC-8 confirms or changes it on the
/// machine (`docs/src/plan/machine-testing.md`).
pub const SATIN_MAX_WIDTH: Mm = Mm::from_tenths(70);

/// Room left in an id made for a column of a stroke of several subpaths, for the `~2` a clash adds
/// ([`Labels::claim`]).
const CLAIM_ROOM: usize = 12;

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
    /// wider than both.
    pub fn of_stroke(width: Mm, settings: &DesignSettings) -> Stitches {
        if width.get() <= settings.min_satin_stroke_width.get() {
            Stitches::Running
        } else if width.get() <= SATIN_MAX_WIDTH.get() {
            Stitches::Satin
        } else {
            Stitches::FilledStroke
        }
    }

    /// Whether the stitches cover the stroke's width, as a satin column or a fill does, so that what a host
    /// draws across a stroke matters to them: its alignment and its width profile (`REQ-AUTO-005`).
    pub const fn across(self) -> bool {
        matches!(self, Stitches::Satin | Stitches::FilledStroke)
    }
}

/// The elements of `design` as they are generated: each automatic element turned into the elements its
/// stitches need ([`digitize`]), the others as they are; and what was said of them, `SC-I0901` for the
/// shapes digitized, and `SC-E0004` for each one left out because it needs more work than `budget` allows.
/// A design without automatic elements comes back as it is.
pub fn digitized<'d>(design: &'d Design, budget: &Budget) -> (Cow<'d, [Element]>, Vec<Diagnostic>) {
    let elements = design.elements();
    if !elements.iter().any(|element| element.automatic) {
        return (Cow::Borrowed(elements), Vec::new());
    }
    let mut diagnostics = Vec::new();
    // The ids given out: the design's, then each column's that a stroke of several subpaths adds.
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
        counts.add(Stitches::of(&element.shape, &design.settings));
        let mut made = made.into_iter();
        out.extend(made.next());
        for column in made {
            // A column's own id, unless another element already has it.
            let id = taken.claim(column.id.as_str().to_string(), &mut naming).ok().and_then(|id| ElementId::new(id).ok());
            match id {
                Some(id) => out.push(Element { id, ..column }),
                None => {
                    let message = format!("A column of `{}` could not be named: it needed more work than the budget allows.", element.id);
                    diagnostics.push(Diagnostic::new(Code::BudgetExhausted, message).with_element(element.id.clone()));
                }
            }
        }
    }
    diagnostics.splice(0..0, counts.said());
    (Cow::Owned(out), diagnostics)
}

/// The ordinary elements that sew the automatic `element` of a design with `settings`, with `meter`'s
/// work: itself as it is when it is an area or runs, each of its subpaths a satin column when it is one,
/// and the area it paints when it is filled.
pub fn digitize(element: &Element, settings: &DesignSettings, meter: &mut Meter) -> Result<Vec<Element>, Exhausted> {
    let ordinary = Element { automatic: false, ..element.clone() };
    let Shape::Stroke { path, width, join, cap } = &element.shape else { return Ok(vec![ordinary]) };
    match Stitches::of_stroke(*width, settings) {
        Stitches::Fill | Stitches::Running => Ok(vec![ordinary]),
        Stitches::Satin => columns(&ordinary, path, (*width, *join, *cap), meter),
        Stitches::FilledStroke => Ok(vec![filled(ordinary, path, (*width, *join, *cap), meter)?]),
    }
}

/// The satin columns of `element`, a stroke along `path` with `style`: one for each subpath, whose open ends
/// reach as far as their caps do, each with the settings of `element` and `satin_column` on.
fn columns(element: &Element, path: &Path, (width, join, cap): (Mm, Join, Cap), meter: &mut Meter) -> Result<Vec<Element>, Exhausted> {
    let mut params = element.params.clone();
    params.set(SATIN_COLUMN, "true");
    let reach = match cap {
        Cap::Butt => 0.0,
        Cap::Round | Cap::Square => width.get() / 2.0,
    };
    let mut columns = Vec::with_capacity(path.subpaths.len());
    for (n, subpath) in path.subpaths.iter().enumerate() {
        meter.charge(1 + subpath.segments.len() as u64)?;
        let id = if n == 0 { Some(element.id.clone()) } else { numbered(&element.id, n + 1) };
        // A numbered id fits by construction ([`numbered`]); one that did not would only leave the column out.
        let Some(id) = id else { continue };
        // A butt cap ends at the path's end, and a closed subpath has no end.
        let subpath = if subpath.closed || cap == Cap::Butt { subpath.clone() } else { reached(subpath, reach) };
        let shape = Shape::Stroke { path: Path { subpaths: vec![subpath] }, width, join, cap };
        columns.push(Element { id, shape, params: params.clone(), ..element.clone() });
    }
    // A stroke of no subpath is still one column, which says why it sews nothing as any column would.
    if columns.is_empty() {
        columns.push(Element { params, ..element.clone() });
    }
    Ok(columns)
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

/// `subpath` reaching `reach` millimetres further past each end, straight on in the direction it ends in.
fn reached(subpath: &Subpath, reach: f64) -> Subpath {
    let points: Vec<Point> = std::iter::once(subpath.start).chain(subpath.segments.iter().flat_map(|s| s.points())).collect();
    let end = subpath.segments.last().map_or(subpath.start, |s| s.end());
    // The direction it starts in: towards the first point that is not its start; and ends in, likewise.
    let onward = |from: Point, toward: Option<&Point>| toward.and_then(|&to| beyond(to, from, reach));
    let (Some(before), Some(after)) =
        (onward(subpath.start, points.iter().find(|&&q| q != subpath.start)), onward(end, points.iter().rev().find(|&&q| q != end)))
    else {
        return subpath.clone();
    };
    let mut segments = Vec::with_capacity(subpath.segments.len() + 2);
    segments.push(Segment::Line(subpath.start));
    segments.extend(subpath.segments.iter().copied());
    segments.push(Segment::Line(after));
    Subpath { start: before, segments, closed: false }
}

/// The point `reach` millimetres past `at`, coming from `from`: on the line from `from` through `at`.
fn beyond(from: Point, at: Point, reach: f64) -> Option<Point> {
    let (dx, dy) = (at.x() - from.x(), at.y() - from.y());
    let length = stitchcraft_core::math::hypot(dx, dy);
    (length > 0.0).then(|| Point::new(at.x() + dx / length * reach, at.y() + dy / length * reach).ok()).flatten()
}

/// `element`, a stroke along `path` with `style`, as the fill of the area it paints: that area's outline, its
/// parts joined, filled by the nonzero rule.
fn filled(element: Element, path: &Path, (width, join, cap): (Mm, Join, Cap), meter: &mut Meter) -> Result<Element, Exhausted> {
    let rings = area::stroke_area(path, width.get(), join, cap, meter)?;
    let built = region::build(&rings, FillRule::NonZero, meter)?;
    // An area that cannot be joined is a bug in the region code; the fill then meets it on the pieces and
    // says so itself.
    let joined = built.diagnostics.iter().all(|d| d.code != Code::InternalCheckFailed);
    let path = if joined { outline(&built.region) } else { rings };
    Ok(Element { shape: Shape::Fill { path, rule: FillRule::NonZero }, ..element })
}

/// The rings of `region` as a path: each part's outline, then its holes, wound as [`region::Polygon`] winds
/// them, so that the nonzero rule leaves the holes empty.
fn outline(region: &Region) -> Path {
    let subpaths = region
        .parts
        .iter()
        .flat_map(|part| part.rings())
        .filter_map(|ring| {
            let (&start, rest) = ring.split_first()?;
            // A ring repeats its first point at its end, which a closed subpath does not need.
            let rest = rest.strip_suffix(&[start]).unwrap_or(rest);
            Some(Subpath { start, segments: rest.iter().map(|&q| Segment::Line(q)).collect(), closed: true })
        })
        .collect();
    Path { subpaths }
}

/// How many shapes were digitized, by their stitches as `SC-I0901` names them.
#[derive(Default)]
struct Counts {
    running: usize,
    satin: usize,
    fills: usize,
}

impl Counts {
    fn add(&mut self, stitches: Stitches) {
        let count = match stitches {
            Stitches::Running => &mut self.running,
            Stitches::Satin => &mut self.satin,
            Stitches::Fill | Stitches::FilledStroke => &mut self.fills,
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
