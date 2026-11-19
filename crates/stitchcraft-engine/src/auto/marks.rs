//! Marks that keep parts (roadmap step M13.39, `REQ-AUTO-104` to `REQ-AUTO-107`,
//! `docs/src/design/image-to-embroidery.md` › Marks that keep parts).
//!
//! **Why.** Automatic digitizing decides by geometry alone which small parts are specks to merge and which
//! thin lines merge into their neighbours. Geometry can't know which small part a person cares about, so a
//! person draws a mark round it. A part of an automatic area that lies wholly inside a mark sews by its
//! width where a rule would merge it: the width rules ([`super::narrow`]) measure a marked speck instead of
//! leaving it in its area's fill, and the steps of specks and of thin lines (`neighbours`) don't take it
//! away. A part partly inside is decided as if there were no mark, since cutting it at the mark's edge
//! would sew an edge the picture doesn't have.
//!
//! **Read once.** Each mark's area is read once for a plan ([`Marks::of`]), with the budget's work to
//! itself. Testing a part costs a unit for each mark it is looked at for, and when the box round the part
//! lies in the box round the mark, a unit for each of its points and each segment of the mark, charged to
//! the meter of the step that asks. Without marks nothing is tested and nothing is charged, so a design
//! without marks sews as it did before marks existed.
//!
//! **Said.** `SC-I0916` names each mark with the parts wholly inside it and how many of them sew that
//! would have merged. `SC-W0917` names a mark that keeps nothing.

use std::cell::Cell;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Meter};

use super::guides::Guides;
use crate::design::mark::{Area, Mark, bounds};
use crate::normalize::region::Polygon;

/// The marks of a design as automatic digitizing tests parts against them, with what they kept.
#[derive(Debug, Default)]
pub(crate) struct Marks {
    /// Each mark whose area reads, in the design's order.
    read: Vec<Read>,
    /// What was said of the marks whose areas don't read.
    unread: Vec<Diagnostic>,
    /// The design's guides, read beside the marks (`REQ-AUTO-109`).
    pub(crate) guides: Guides,
}

/// A mark whose area reads, and what it kept.
#[derive(Debug)]
struct Read {
    /// The mark's id.
    id: ElementId,
    /// Its area.
    area: Area,
    /// How many parts lie wholly inside it, as the thin lines' step finds the areas' parts.
    held: Cell<usize>,
    /// How many specks inside it sew by their width.
    specks: Cell<usize>,
    /// How many thin lines inside it sew as lines.
    lines: Cell<usize>,
}

impl Marks {
    /// `marks` read, each with `budget`'s work to itself: `SC-W0917` for each one whose area doesn't read, or
    /// whose reading needs more work.
    pub(crate) fn of(marks: &[Mark], budget: &Budget) -> Marks {
        let mut read = Vec::with_capacity(marks.len());
        let mut unread = Vec::new();
        for mark in marks {
            match Area::of(&mark.shape, &mut budget.meter()) {
                Ok(Ok(area)) => read.push(Read { id: mark.id.clone(), area, held: Cell::new(0), specks: Cell::new(0), lines: Cell::new(0) }),
                Ok(Err(why)) => unread.push(unused(&mark.id, why)),
                Err(_) => {
                    let why = format!("reading its area needed more than the work budget of {} units.", budget.max_work);
                    unread.push(unused(&mark.id, &why));
                }
            }
        }
        Marks { read, unread, guides: Guides::default() }
    }

    /// The marks with the guides `guides` read beside them ([`Guides::of`]).
    pub(crate) fn guided(self, guides: &[Mark], budget: &Budget) -> Marks {
        Marks { guides: Guides::of(guides, budget), ..self }
    }

    /// The first mark that holds every point of `part`'s outline, by its place among the marks that read,
    /// with `meter`'s work; `None` when none does, and without work when there are no marks.
    pub(crate) fn holding(&self, part: &Polygon, meter: &mut Meter) -> Result<Option<usize>, Exhausted> {
        if self.read.is_empty() {
            return Ok(None);
        }
        let points = u64::try_from(part.outline.len()).unwrap_or(u64::MAX);
        meter.charge(points)?;
        let Some(around) = bounds(part.outline.iter().map(|p| (p.x(), p.y()))) else { return Ok(None) };
        for (at, mark) in self.read.iter().enumerate() {
            meter.charge(1)?;
            if !mark.area.may_hold(around) {
                continue;
            }
            meter.charge(points.saturating_mul(mark.area.cost()))?;
            if part.outline.iter().all(|p| mark.area.holds((p.x(), p.y()))) {
                return Ok(Some(at));
            }
        }
        Ok(None)
    }

    /// For each of `parts`, the mark that holds it ([`Marks::holding`]), each part held counted for the
    /// mark, with `meter`'s work.
    pub(crate) fn held(&self, parts: &[Polygon], meter: &mut Meter) -> Result<Vec<Option<usize>>, Exhausted> {
        let mut held = Vec::with_capacity(parts.len());
        for part in parts {
            let at = self.holding(part, meter)?;
            if let Some(mark) = at.and_then(|at| self.read.get(at)) {
                mark.held.set(mark.held.get() + 1);
            }
            held.push(at);
        }
        Ok(held)
    }

    /// Counts a speck that the mark at `at` keeps.
    pub(crate) fn kept_speck(&self, at: usize) {
        if let Some(mark) = self.read.get(at) {
            mark.specks.set(mark.specks.get() + 1);
        }
    }

    /// Counts a thin line that the mark at `at` keeps.
    pub(crate) fn kept_line(&self, at: usize) {
        if let Some(mark) = self.read.get(at) {
            mark.lines.set(mark.lines.get() + 1);
        }
    }

    /// What is said of the marks once a design is digitized: `SC-I0916` for each mark that holds a part,
    /// and `SC-W0917` for each that doesn't, or whose area didn't read, then what is said of the guides
    /// ([`Guides::said`]). In a design with no `automatic` shape, every mark and every guide is unused.
    pub(crate) fn said(self, automatic: bool) -> Vec<Diagnostic> {
        let mut said = self.unread;
        for mark in self.read {
            let held = mark.held.get();
            said.push(match held {
                _ if !automatic => unused(&mark.id, "the design has no shape digitized automatically."),
                0 => unused(&mark.id, "no part of an automatic area lies wholly inside it."),
                _ => kept(&mark.id, held, mark.specks.get(), mark.lines.get()),
            });
        }
        said.extend(self.guides.said(automatic));
        said
    }
}

/// The marks of `marks` and the guides of `guides` in a design that has no automatic shape: `SC-W0917` for
/// each mark, its area unread, then `SC-W0919` for each guide.
pub(crate) fn unused_all(marks: &[Mark], guides: &[Mark]) -> Vec<Diagnostic> {
    let mut said: Vec<Diagnostic> = marks.iter().map(|mark| unused(&mark.id, "the design has no shape digitized automatically.")).collect();
    said.extend(super::guides::unused_all(guides));
    said
}

/// `SC-W0917`: the mark `id` keeps nothing, for the reason `why`.
fn unused(id: &ElementId, why: &str) -> Diagnostic {
    let message = format!("The mark `{id}` is not used, and the design sews as without it: {why}");
    let fix = Fix::Hint("Draw the mark round the whole part, a little larger than it, over a shape digitized automatically.".to_string());
    Diagnostic::new(Code::AutoMarkUnused, message).with_element(id.clone()).with_fix(fix)
}

/// `SC-I0916`: the mark `id` holds `held` parts wholly, of which `specks` specks and `lines` thin lines sew
/// that would have merged.
fn kept(id: &ElementId, held: usize, specks: usize, lines: usize) -> Diagnostic {
    let message = format!(
        "The mark `{id}` holds {} of automatic areas wholly. Of them, {} and {} sew by their width, where without the mark they would merge into a neighbour.",
        counted(held, "part"),
        counted(specks, "speck"),
        counted(lines, "thin line")
    );
    let fix = Fix::Hint("To let the specks of a trace's noise merge, draw the mark tighter round the part.".to_string());
    Diagnostic::new(Code::AutoKept, message).with_element(id.clone()).with_fix(fix)
}

/// `n` and `noun`, the noun plural but for 1.
fn counted(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

#[cfg(test)]
pub(crate) mod tests;
