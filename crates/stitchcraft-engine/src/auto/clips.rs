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
    // Each point of a cut shape lies on its shape or on a clip, which the design checked, so an error is a bug.
    let mut cut = Design::new(elements, design.settings).map_err(|bug| said.push(bug)).ok()?;
    cut.sort_colours = design.sort_colours;
    Some(cut)
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

#[cfg(test)]
mod tests {
    use stitchcraft_core::{ElementId, Point};
    use stitchcraft_params::ParamSet;
    use stitchcraft_plan::{Rgb, Thread};

    use super::*;
    use crate::design::{Clip, Path, Segment, Subpath};

    /// A square fill `side` wide from (`x`, `y`) named `id`, its edges from the middle of its top as a curve
    /// would be drawn: as 4 quadratic curves that are straight, so a cut of it would show as lines.
    fn square(id: &str, (x, y): (f64, f64), side: f64, automatic: bool, clips: Vec<Clip>) -> Element {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let corners = [p(x + side, y), p(x + side, y + side), p(x, y + side), p(x, y)];
        let mut from = p(x + side / 2.0, y);
        let mut segments = Vec::new();
        for corner in corners {
            segments.push(Segment::Quad(from.lerp(corner, 0.5), corner));
            from = corner;
        }
        segments.push(Segment::Line(p(x + side / 2.0, y)));
        let path = Path { subpaths: vec![Subpath { start: p(x + side / 2.0, y), segments, closed: true }] };
        let shape = Shape::Fill { path, rule: FillRule::NonZero };
        let (id, thread) = (ElementId::new(id).unwrap(), Thread::new(Rgb::new(0, 0, 0)));
        Element { id, name: None, shape, thread, params: ParamSet::default(), automatic, hints: Default::default(), clips }
    }

    #[test]
    fn req_gen_005_only_an_automatic_shape_with_clips_is_cut_before_it_is_digitized() {
        // The automatic square without clips, and the clipped square that isn't automatic, keep their curves:
        // the generators cut the second as they sew it. The automatic clipped square becomes its outline.
        let clip = |x: f64| Clip::of(square("clip", (x, 0.0), 6.0, false, Vec::new()).shape.path().clone(), FillRule::NonZero);
        let elements = vec![
            square("free", (0.0, 0.0), 10.0, true, Vec::new()),
            square("drawn", (20.0, 0.0), 10.0, false, vec![clip(22.0)]),
            square("cut", (40.0, 0.0), 10.0, true, vec![clip(42.0)]),
        ];
        let design = Design::new(elements.clone(), crate::design::DesignSettings::default()).unwrap();
        let cut = super::cut(&design, &Budget::DEFAULT, &mut Vec::new()).unwrap();
        assert_eq!(cut.elements()[..2], elements[..2]);
        let Shape::Fill { path, .. } = &cut.elements()[2].shape else { panic!("{:?}", cut.elements()[2]) };
        assert!(path.subpaths.iter().flat_map(|s| &s.segments).all(|s| matches!(s, Segment::Line(_))), "{path:?}");
        assert!(cut.elements()[2].clips.is_empty());
    }
}
