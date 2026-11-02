//! Automatic shapes cut to their clips before they are digitized (`REQ-GEN-005`).
//!
//! An ordinary element is cut to its clips as it is generated ([`crate::generate`]), but automatic digitizing
//! measures a shape's width, flattens the layers and finds what shows of each shape before anything is
//! generated. So an automatic shape is cut first: an area to what its clips leave of it, which then is its
//! outline, and a line to its pieces inside them, each a subpath of its own, cut as a running stitch is. What
//! is digitized, what it hides of the shapes under it, and the columns it sews then follow what shows. A cut
//! end of a line takes the line's cap, as an end drawn there would, where a viewer cuts the painted line
//! along the clip's edge. A shape its clips leave nothing of is left out (`SC-W0013`), with a trim or stop
//! after it (`SC-W0505`).

use stitchcraft_core::{Budget, Diagnostic};

use crate::design::{Design, Element, FillRule, Shape};
use crate::generate::trim_or_stop_left_out;
use crate::normalize::region::{self, cut};
use crate::normalize::stroke;

use super::outline;

/// `design` with each automatic element cut to its clips, those it leaves nothing of left out, and what is
/// said of them in `said`; `None` when no automatic element has clips. Each element has the budget's work
/// to itself, and one that needs more is left out with `SC-E0004`, as one that needs more to digitize is.
pub(super) fn cut(design: &Design, budget: &Budget, said: &mut Vec<Diagnostic>) -> Option<Design> {
    if !design.elements().iter().any(|element| element.automatic && !element.clips.is_empty()) {
        return None;
    }
    let mut elements = Vec::with_capacity(design.elements().len());
    for element in design.elements() {
        if !element.automatic || element.clips.is_empty() {
            elements.push(element.clone());
            continue;
        }
        let mut told = Vec::new();
        match shown(element, &mut told, budget) {
            Some(shape) => elements.push(Element { shape, clips: Vec::new(), ..element.clone() }),
            None => told.extend(trim_or_stop_left_out(element)),
        }
        said.extend(told.into_iter().map(|d| d.with_element(element.id.clone())));
    }
    match Design::new(elements, design.settings) {
        Ok(mut cut) => {
            cut.sort_colours = design.sort_colours;
            Some(cut)
        }
        // Each point of a cut shape lies on its shape or on a clip, which the design checked, so this is a bug.
        Err(bug) => {
            said.push(bug);
            None
        }
    }
}

/// What `element`'s clips leave of its shape, or `None` when they leave nothing, which `told` says, as it
/// says when the work runs out.
fn shown(element: &Element, told: &mut Vec<Diagnostic>, budget: &Budget) -> Option<Shape> {
    let meter = &mut budget.meter();
    let left = match &element.shape {
        Shape::Fill { path, rule } => region::build_clipped(path, *rule, &element.clips, meter).map(|built| {
            if built.region.parts.is_empty() {
                told.extend(built.diagnostics);
                return None;
            }
            Some(Shape::Fill { path: outline(&built.region.parts), rule: FillRule::NonZero })
        }),
        Shape::Stroke { path, width, join, cap } => cut::stroke(path, &element.clips, stroke::flattened, told, meter)
            .map(|cut| cut.map(|path| Shape::Stroke { path: path.into_owned(), width: *width, join: *join, cap: *cap })),
    };
    left.unwrap_or_else(|exhausted| {
        told.push(exhausted.diagnostic(budget, Some(element.id.clone())));
        None
    })
}
