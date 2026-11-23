//! Thickness before threading (`REQ-AUTO-189`): which plans the guard may plan again.

use stitchcraft_core::{Budget, ElementId, Point};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, Thread};

use super::planned;
use crate::design::{Design, DesignSettings, Element, FillRule, Hints, Path, Segment, Shape, Subpath};

const NAVY: Rgb = Rgb::new(29, 53, 87);
const CREAM: Rgb = Rgb::new(244, 232, 201);
const GOLD: Rgb = Rgb::new(232, 169, 0);

/// An automatic area `id` of the rectangles `rings`, in `color`, whose warning's limit of thread is 0.5 fills:
/// every plan of it sews above that limit.
fn area(id: &str, rings: &[[f64; 4]], color: Rgb) -> Element {
    let ring = |&[x0, y0, x1, y1]: &[f64; 4], hole: bool| {
        let p = |x: f64, y: f64| Point::new(x, y).unwrap();
        let corners = if hole { [p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0)] } else { [p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1)] };
        Subpath { start: corners[0], segments: corners[1..].iter().map(|&at| Segment::Line(at)).collect(), closed: true }
    };
    let subpaths = rings.iter().enumerate().map(|(at, rect)| ring(rect, at > 0)).collect();
    let params: ParamSet = [("auto_thick_warn_fills", "0.5"), ("auto_fill_outline", "false")].into_iter().collect();
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: Path { subpaths }, rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(color),
        params,
        automatic: true,
        lettered: None,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

/// Whether the plan of `elements` may be planned again with each line after its fills.
fn guarded(elements: Vec<Element>) -> bool {
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    planned(&design, REFERENCE, &Budget::DEFAULT, false).1.is_some()
}

#[test]
fn req_auto_189_an_order_the_strict_rules_allow_is_planned_once() {
    // A navy and a gold square apart: their order threads each thread once and is one that the strict rules
    // allow, so the design, above its limit, isn't planned again.
    assert!(!guarded(vec![area("navy", &[[0.0, 0.0, 20.0, 20.0]], NAVY), area("gold", &[[40.0, 0.0, 60.0, 20.0]], GOLD)]));
    // The sew-out piece: its navy line sews with the navy fill, before the cream fill it borders, which the
    // strict rules don't allow, so the design, above its limit, may be planned again.
    assert!(guarded(vec![
        area("dark", &[[2.0, 2.0, 58.0, 42.0]], NAVY),
        area("line", &[[15.2, 11.2, 44.8, 32.8], [16.0, 12.0, 44.0, 32.0]], NAVY),
        area("light", &[[16.0, 12.0, 44.0, 32.0]], CREAM),
        area("gold", &[[62.0, 32.0, 70.0, 42.0]], GOLD),
    ]));
}
