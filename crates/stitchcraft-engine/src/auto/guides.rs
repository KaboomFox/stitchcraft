//! Guides on the shapes of an automatic design (roadmap step M13.24, `REQ-AUTO-109` to `REQ-AUTO-111`,
//! `docs/src/design/image-to-embroidery.md` › A guide on one shape).
//!
//! **Why.** Automatic digitizing chooses the rows of each fill by geometry, all at `auto_fill_angle`. No rule
//! can know the form that the rows of fur, a petal or a roof follow, so the person draws it: a line named
//! `stitch:guide` over the picture. Each filled part of an automatic area that the line crosses sews as a
//! guided fill whose rows follow it, and everything else stays automatic: the overlaps, the base layer, the
//! order and the cap.
//!
//! **Which parts.** A part follows a guide when a stretch of the guide's line lies inside it, found as a
//! running stitch is cut to its clips (`normalize::region::cut::inside`). Only the parts that the rules by
//! width leave filled, and no speck, follow a guide. A part sewn along its middle, as a running stitch or as
//! satin columns, keeps its stitches, and is only counted. A part that 2 guides cross follows the one drawn
//! last, which lies on top.
//!
//! **Read once.** Each guide's line is read once for a plan ([`Guides::of`]), with the budget's work to
//! itself: its first subpath, opened where it closes and flattened as a shape's outline is, as a guided fill
//! reads its guide. Testing a part costs a unit for each pair of a segment of a line and a side of the part,
//! charged to the meter of the area's element. Without guides nothing is tested and nothing is charged, so a
//! design without guides sews as it did before guides existed.
//!
//! **Said.** `SC-I0918` names each guide with the parts that follow it and the parts it crosses that keep
//! their stitches. `SC-W0919` names a guide that steers nothing.

use std::cell::Cell;

use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Exhausted, Fix, Meter, Point};

use crate::design::mark::Mark;
use crate::design::{Path, Subpath};
use crate::normalize::region::{Polygon, cut};
use crate::normalize::stroke;

/// The guides of a design as automatic digitizing tests parts against them, with what they steered.
#[derive(Debug, Default)]
pub(crate) struct Guides {
    /// Each guide whose line has a length, in the design's order.
    read: Vec<Read>,
    /// What was said of the guides whose lines don't read.
    unread: Vec<Diagnostic>,
}

/// A guide whose line has a length, and what it steered.
#[derive(Debug)]
struct Read {
    /// The guide's id.
    id: ElementId,
    /// Its path, which the guided fills it makes follow.
    path: Path,
    /// Its line: its first subpath, opened and flattened.
    line: Vec<Point>,
    /// How many areas have parts that follow it.
    areas: Cell<usize>,
    /// How many filled parts follow it.
    parts: Cell<usize>,
    /// How many parts sewn along their middles it crosses, which keep their stitches.
    kept: Cell<usize>,
}

impl Guides {
    /// `guides` read, each with `budget`'s work to itself: `SC-W0919` for each one whose line has no length,
    /// or whose reading needs more work.
    pub(crate) fn of(guides: &[Mark], budget: &Budget) -> Guides {
        let mut read = Vec::with_capacity(guides.len());
        let mut unread = Vec::new();
        for guide in guides {
            let path = guide.shape.path();
            match line(path, &mut budget.meter()) {
                Ok(Some(line)) => {
                    let (areas, parts, kept) = (Cell::new(0), Cell::new(0), Cell::new(0));
                    read.push(Read { id: guide.id.clone(), path: path.clone(), line, areas, parts, kept });
                }
                Ok(None) => unread.push(unused(&guide.id, "its line has no length.")),
                Err(_) => {
                    let why = format!("reading its line needed more than the work budget of {} units.", budget.max_work);
                    unread.push(unused(&guide.id, &why));
                }
            }
        }
        Guides { read, unread }
    }

    /// Whether no guide's line reads, when no part is tested.
    pub(crate) fn is_empty(&self) -> bool {
        self.read.is_empty()
    }

    /// The last guide, by its place among those that read, whose line has a stretch inside `part`, with
    /// `meter`'s work; `None` when none does.
    pub(crate) fn crossing(&self, part: &Polygon, meter: &mut Meter) -> Result<Option<usize>, Exhausted> {
        let mut last = None;
        for (at, guide) in self.read.iter().enumerate() {
            if !cut::inside(&guide.line, std::slice::from_ref(part), meter)?.is_empty() {
                last = Some(at);
            }
        }
        Ok(last)
    }

    /// The path of the guide at `at`, which the guided fill of the parts that follow it is given.
    pub(crate) fn path(&self, at: usize) -> Option<&Path> {
        self.read.get(at).map(|guide| &guide.path)
    }

    /// Counts `parts` filled parts of one area that follow the guide at `at`.
    pub(crate) fn steered(&self, at: usize, parts: usize) {
        if let Some(guide) = self.read.get(at) {
            guide.areas.set(guide.areas.get() + 1);
            guide.parts.set(guide.parts.get() + parts);
        }
    }

    /// Counts a part sewn along its middle that the guide at `at` crosses, which keeps its stitches.
    pub(crate) fn kept(&self, at: usize) {
        if let Some(guide) = self.read.get(at) {
            guide.kept.set(guide.kept.get() + 1);
        }
    }

    /// What is said of the guides once a design is digitized: `SC-I0918` for each guide that filled parts
    /// follow, and `SC-W0919` for each that steers nothing, or whose line didn't read. In a design with no
    /// `automatic` shape, every guide is unused.
    pub(crate) fn said(self, automatic: bool) -> Vec<Diagnostic> {
        let mut said = self.unread;
        for guide in self.read {
            let parts = guide.parts.get();
            said.push(match parts {
                _ if !automatic => unused(&guide.id, NOTHING_AUTOMATIC),
                0 => unused(&guide.id, "it crosses no filled part of an automatic area."),
                _ => followed(&guide.id, parts, guide.areas.get(), guide.kept.get()),
            });
        }
        said
    }
}

/// Why a guide in a design with no automatic shape is unused.
const NOTHING_AUTOMATIC: &str = "the design has no shape digitized automatically.";

/// The guides of `guides` in a design that has no automatic shape: `SC-W0919` for each, its line unread.
pub(crate) fn unused_all(guides: &[Mark]) -> Vec<Diagnostic> {
    guides.iter().map(|guide| unused(&guide.id, NOTHING_AUTOMATIC)).collect()
}

/// The line of a guide drawn as `path`, with `meter`'s work: its first subpath, opened where it closes and
/// flattened as a shape's outline is. `None` when it has no length.
fn line(path: &Path, meter: &mut Meter) -> Result<Option<Vec<Point>>, Exhausted> {
    let Some(first) = path.subpaths.first() else { return Ok(None) };
    let open = Subpath { closed: false, ..first.clone() };
    let points = stroke::flattened(&open, meter)?;
    let long = points.windows(2).any(|pair| matches!(pair, [a, b] if a != b));
    Ok(long.then_some(points))
}

/// `SC-W0919`: the guide `id` steers nothing, for the reason `why`.
fn unused(id: &ElementId, why: &str) -> Diagnostic {
    let message = format!("The guide `{id}` is not used, and the design sews as without it: {why}");
    let fix = Fix::Hint("Draw the guide as a line across the filled shapes whose rows should follow it.".to_string());
    Diagnostic::new(Code::AutoGuideUnused, message).with_element(id.clone()).with_fix(fix)
}

/// `SC-I0918`: `parts` filled parts of `areas` automatic areas follow the guide `id`, and it crosses `kept`
/// parts sewn along their middles, which keep their stitches.
fn followed(id: &ElementId, parts: usize, areas: usize, kept: usize) -> Diagnostic {
    let message = format!(
        "{} of {} sew as guided fills whose rows follow the guide `{id}`. It crosses {} sewn along their middles, which keep their stitches.",
        counted(parts, "filled part"),
        counted(areas, "automatic area"),
        counted(kept, "part")
    );
    let fix = Fix::Hint("To steer fewer parts, draw the guide so that it crosses only the parts whose rows should follow it.".to_string());
    Diagnostic::new(Code::AutoGuided, message).with_element(id.clone()).with_fix(fix)
}

/// `n` and `noun`, the noun plural but for 1.
fn counted(n: usize, noun: &str) -> String {
    if n == 1 { format!("1 {noun}") } else { format!("{n} {noun}s") }
}

#[cfg(test)]
mod tests;
