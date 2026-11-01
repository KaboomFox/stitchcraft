//! Small designs for the engine's conformance cases, and a way to read a plan's shape at a glance.
//!
//! Plan assembly and finalizing are tested through the engine's entry point, `stitchcraft_engine::plan`,
//! on designs of a few straight strokes. The helpers are shared so every case builds them the same way.

use stitchcraft_core::{Budget, ElementId, Mm, Point};
use stitchcraft_engine::auto::{self, AutoParams};
use stitchcraft_engine::design::{Cap, Design, DesignSettings, Element, FillRule, Path, Segment, Shape, Subpath};
use stitchcraft_engine::{PlanOutcome, plan};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, Role, StitchKind, StitchPlan, Thread};

/// A red thread.
pub const RED: Thread = Thread::new(Rgb::new(200, 0, 0));
/// A blue thread.
pub const BLUE: Thread = Thread::new(Rgb::new(0, 0, 200));

/// The point (`x`, `y`).
pub fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A straight stroke `id` from `from` along x for `length` mm, sewn with `thread` and the parameters
/// `params` (Ink/Stitch keys and values).
pub fn line(id: &str, from: (f64, f64), length: f64, thread: &Thread, params: &[(&str, &str)]) -> Element {
    stroke(id, &[(from, length)], thread, params)
}

/// A stroke `id` of straight subpaths, each from a point along x for a length.
pub fn stroke(id: &str, parts: &[((f64, f64), f64)], thread: &Thread, params: &[(&str, &str)]) -> Element {
    let subpaths =
        parts.iter().map(|&((x, y), length)| Subpath { start: p(x, y), segments: vec![Segment::Line(p(x + length, y))], closed: false }).collect();
    along(id, Path { subpaths }, thread, params)
}

/// A stroke `id` along `path`, sewn with `thread` and the parameters `params`.
pub fn along(id: &str, path: Path, thread: &Thread, params: &[(&str, &str)]) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::stroke(path),
        thread: thread.clone(),
        params: params.iter().copied().collect::<ParamSet>(),
        automatic: false,
    }
}

/// `element`, a stroke, drawn `width` millimetres wide: how wide a satin column drawn as one path is.
pub fn widened(element: Element, width: f64) -> Element {
    let Shape::Stroke { path, join, cap, .. } = element.shape else { panic!("`{}` is not a stroke", element.id) };
    Element { shape: Shape::Stroke { path, width: Mm::new(width).unwrap(), join, cap }, ..element }
}

/// `element`, a stroke, ending with `cap` where its open subpaths end.
pub fn capped(element: Element, cap: Cap) -> Element {
    let Shape::Stroke { path, width, join, .. } = element.shape else { panic!("`{}` is not a stroke", element.id) };
    Element { shape: Shape::Stroke { path, width, join, cap }, ..element }
}

/// `element` as a shape without settings that StitchCraft digitizes ([`Element::automatic`]).
pub fn automatic(element: Element) -> Element {
    Element { automatic: true, ..element }
}

/// The settings of the rules that automatic digitizing goes by for a shape that sets none, each its
/// default (`REQ-AUTO-020`).
pub fn rules() -> AutoParams {
    auto::rules(&ParamSet::new()).unwrap()
}

/// A path of open polylines, one subpath through each list of points. A list of one point is a subpath
/// that does not move.
pub fn polylines(parts: &[&[(f64, f64)]]) -> Path {
    subpaths(parts, false)
}

/// A path of closed subpaths, one through each list of points: a fill's outline and holes.
pub fn rings(parts: &[&[(f64, f64)]]) -> Path {
    subpaths(parts, true)
}

/// A path of straight subpaths, one through each list of points, `closed` or open.
fn subpaths(parts: &[&[(f64, f64)]], closed: bool) -> Path {
    let subpaths = parts
        .iter()
        .filter_map(|points| {
            let (&(x, y), rest) = points.split_first()?;
            Some(Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed })
        })
        .collect();
    Path { subpaths }
}

/// The letters [`letter`] draws.
pub const LETTERS: [char; 3] = ['T', 'H', 'E'];

/// The outline of the letter `letter`, one of [`LETTERS`], `height` mm tall and 0.6 of that wide, drawn in
/// strokes `width` mm wide from the origin: a shape for automatic digitizing's property tests, and for the
/// Ink/Stitch differential's centre lines.
pub fn letter(letter: char, height: f64, width: f64) -> Vec<(f64, f64)> {
    let (h, w, wide) = (height, width, 0.6 * height);
    let (above, below) = (h / 2.0 - w / 2.0, h / 2.0 + w / 2.0);
    match letter {
        'T' => {
            let (left, right) = (wide / 2.0 - w / 2.0, wide / 2.0 + w / 2.0);
            vec![(0.0, 0.0), (wide, 0.0), (wide, w), (right, w), (right, h), (left, h), (left, w), (0.0, w)]
        }
        'H' => vec![
            (0.0, 0.0),
            (w, 0.0),
            (w, above),
            (wide - w, above),
            (wide - w, 0.0),
            (wide, 0.0),
            (wide, h),
            (wide - w, h),
            (wide - w, below),
            (w, below),
            (w, h),
            (0.0, h),
        ],
        'E' => {
            let middle = 0.8 * wide;
            vec![
                (0.0, 0.0),
                (wide, 0.0),
                (wide, w),
                (w, w),
                (w, above),
                (middle, above),
                (middle, below),
                (w, below),
                (w, h - w),
                (wide, h - w),
                (wide, h),
                (0.0, h),
            ]
        }
        other => panic!("no letter {other}"),
    }
}

/// A fill `id` of the rings `parts`, filled by the nonzero rule, sewn with `thread` and the parameters
/// `params`.
pub fn fill(id: &str, parts: &[&[(f64, f64)]], thread: &Thread, params: &[(&str, &str)]) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: rings(parts), rule: FillRule::NonZero },
        thread: thread.clone(),
        params: params.iter().copied().collect::<ParamSet>(),
        automatic: false,
    }
}

/// `elements` planned for the reference machine, the design's origin where the coordinates are, so
/// positions read as drawn.
pub fn planned(elements: Vec<Element>) -> PlanOutcome {
    planned_with(elements, DesignSettings { origin: Some(p(0.0, 0.0)), ..DesignSettings::default() })
}

/// `elements` planned for the reference machine with `settings`.
pub fn planned_with(elements: Vec<Element>, settings: DesignSettings) -> PlanOutcome {
    plan(&Design::new(elements, settings).unwrap(), REFERENCE, &Budget::DEFAULT)
}

/// The plan in words: `J` a jump, `S` a stitch, `L` a lock stitch, `T` a trim, `P` a stop and `|` a thread
/// change, runs of stitches with their length (`S5`: five stitches).
pub fn shape(plan: &StitchPlan) -> String {
    let mut words: Vec<String> = Vec::new();
    for (i, block) in plan.blocks.iter().enumerate() {
        if i > 0 {
            words.push("|".to_string());
        }
        for stitch in &block.stitches {
            let symbol = match (stitch.kind, stitch.origin.role) {
                (StitchKind::Jump, _) => "J",
                (StitchKind::Normal, Role::Lock) => "L",
                (StitchKind::Normal, _) => "S",
                (StitchKind::Trim, _) => "T",
                (StitchKind::Stop, _) => "P",
            };
            let count = words.last().and_then(|w| w.strip_prefix(symbol)).filter(|_| matches!(symbol, "S" | "L")).and_then(|n| n.parse::<u32>().ok());
            match (count, words.last_mut()) {
                (Some(n), Some(word)) => *word = format!("{symbol}{}", n + 1),
                _ => words.push(if matches!(symbol, "S" | "L") { format!("{symbol}1") } else { symbol.to_string() }),
            }
        }
    }
    words.join(" ")
}

/// The shape of `outcome`'s plan, which must exist.
pub fn shape_of(outcome: &PlanOutcome) -> String {
    shape(outcome.plan.as_ref().unwrap())
}

/// `outcome`'s diagnostics as people read them.
pub fn messages(outcome: &PlanOutcome) -> Vec<String> {
    outcome.diagnostics.iter().map(ToString::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shapes_count_runs_of_stitches() {
        let one = planned(vec![line("a", (0.0, 0.0), 10.0, &RED, &[])]);
        assert_eq!(shape_of(&one), "J L4 S5 L4");
        let two = planned(vec![
            line("a", (0.0, 0.0), 10.0, &RED, &[("trim_after", "true")]),
            line("b", (0.0, 5.0), 10.0, &BLUE, &[("stop_after", "true")]),
        ]);
        assert_eq!(shape_of(&two), "J L4 S5 L4 T | J L4 S5 L4 P");
        assert_eq!(messages(&two), Vec::<String>::new());
    }
}
