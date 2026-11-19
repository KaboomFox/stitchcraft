use stitchcraft_core::{Budget, Mm, Point};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{Cap, FillRule, Hints, Join, Segment, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A path through `points`, closed or open.
fn path(points: &[(f64, f64)], closed: bool) -> Path {
    let (first, rest) = points.split_first().unwrap();
    Path { subpaths: vec![Subpath { start: p(first.0, first.1), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed }] }
}

fn fill(points: &[(f64, f64)]) -> Shape {
    Shape::Fill { path: path(points, true), rule: FillRule::NonZero, gradient: None }
}

fn stroke(points: &[(f64, f64)], closed: bool, width: f64) -> Shape {
    Shape::Stroke { path: path(points, closed), width: Mm::new(width).unwrap(), join: Join::Round, cap: Cap::Butt }
}

fn element(id: &str, name: Option<&str>, shape: Shape) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: name.map(str::to_string),
        shape,
        thread: Thread::new(Rgb::new(0, 255, 0)),
        params: ParamSet::default(),
        automatic: true,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

const BOX: [(f64, f64); 4] = [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)];

#[test]
fn req_auto_104_a_name_is_read_as_every_markers() {
    assert_eq!(note("stitch:keep", KEEP), Some(""));
    assert_eq!(note("  Stitch:KEEP\t left eye  ", KEEP), Some("left eye"));
    for other in ["stitch:keeps", "stitch:kee", "a stitch:keep", "", "stitch:text keep"] {
        assert_eq!(note(other, KEEP), None, "{other}");
    }
}

#[test]
fn req_auto_104_marks_are_taken_out_of_the_elements_one_for_each_object() {
    let elements = vec![
        element("a", None, fill(&BOX)),
        element("m:fill", Some("stitch:keep"), fill(&BOX)),
        element("m:stroke", Some("stitch:keep"), stroke(&BOX, true, 0.2)),
        element("b", Some("stitch:keep eye"), stroke(&BOX, true, 0.2)),
        element("l:fill", Some("stitch:keep"), fill(&[(0.0, 0.0), (5.0, 5.0)])),
        element("l:stroke", Some("stitch:keep"), stroke(&[(0.0, 0.0), (5.0, 5.0)], false, 2.0)),
        element("c", Some("stitch:text a"), fill(&BOX)),
    ];
    let (kept, marks) = taken_out(elements);
    fn ids<'i>(ids: Vec<&'i ElementId>) -> Vec<&'i str> {
        ids.into_iter().map(ElementId::as_str).collect()
    }
    assert_eq!(ids(kept.iter().map(|element| &element.id).collect()), ["a", "c"]);
    // A closed object's fill stands for it, and an open line's stroke, whose width is its band. The mark
    // named with a note is a mark of its own, though its path is the one before.
    assert_eq!(ids(marks.iter().map(|mark| &mark.id).collect()), ["m:fill", "b", "l:stroke"]);
    assert!(matches!(marks[2].shape, Shape::Stroke { .. }));
    // 2 marks of 2 paths in a row stay 2.
    let (_, two) = taken_out(vec![element("x", Some("stitch:keep"), fill(&BOX)), element("y", Some("stitch:keep"), stroke(&BOX[..3], true, 1.0))]);
    assert_eq!(two.len(), 2);
}

#[test]
fn req_auto_104_an_area_holds_what_its_outline_encloses() {
    let area = Area::of(&stroke(&BOX, true, 0.2), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert!(area.holds((5.0, 2.5)) && !area.holds((11.0, 2.5)) && !area.holds((5.0, 6.0)));
    // A unit for each of the 5 points of its flattened ring, which ends where it starts.
    assert_eq!(area.cost(), 5);
    assert_eq!(area.line().length(), 10.0);
    // Its box, at its edges and past them.
    assert!(area.may_hold([0.0, 0.0, 10.0, 5.0]) && area.may_hold([1.0, 1.0, 2.0, 2.0]));
    for outside in [[-0.1, 0.0, 1.0, 1.0], [0.0, -0.1, 1.0, 1.0], [0.0, 0.0, 10.1, 1.0], [0.0, 0.0, 1.0, 5.1]] {
        assert!(!area.may_hold(outside), "{outside:?}");
    }
    assert_eq!(Area::of(&fill(&[(0.0, 0.0), (10.0, 0.0)]), &mut Budget::DEFAULT.meter()).unwrap(), Err("it encloses no area."));
    assert_eq!(Area::of(&fill(&[(0.0, 0.0), (0.0, 0.0)]), &mut Budget::DEFAULT.meter()).unwrap(), Err("it encloses no area."));
    let none = Shape::Fill { path: Path { subpaths: Vec::new() }, rule: FillRule::NonZero, gradient: None };
    assert_eq!(Area::of(&none, &mut Budget::DEFAULT.meter()).unwrap(), Err("it encloses no area."));
}

#[test]
fn req_auto_106_a_line_with_a_stroke_holds_the_band_along_it() {
    let area = Area::of(&stroke(&[(0.0, 0.0), (10.0, 0.0)], false, 2.0), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert!(area.holds((0.0, 1.0)) && area.holds((10.0, -1.0)) && area.holds((5.0, 0.0)));
    assert!(!area.holds((5.0, 1.1)) && !area.holds((-0.1, 0.0)) && !area.holds((10.1, 0.0)));
    assert_eq!(area.cost(), 1);
    // Its box reaches half the stroke past the line.
    assert!(area.may_hold([-1.0, -1.0, 11.0, 1.0]) && !area.may_hold([-1.1, 0.0, 1.0, 0.0]) && !area.may_hold([0.0, 0.0, 11.1, 0.0]));
    assert!(!area.may_hold([0.0, -1.1, 1.0, 0.0]) && !area.may_hold([0.0, 0.0, 1.0, 1.1]));
    assert_eq!(Area::of(&stroke(&[(1.0, 1.0), (1.0, 1.0)], false, 2.0), &mut Budget::DEFAULT.meter()).unwrap(), Err("its line has no length."));
}

#[test]
fn req_auto_104_reading_an_area_charges_the_meter() {
    let mut meter = Budget { max_work: 0, ..Budget::DEFAULT }.meter();
    assert!(Area::of(&fill(&BOX), &mut meter).is_err());
    assert!(flat(&path(&BOX, true), &mut meter).is_err());
    assert_eq!(bounds([(1.0, 2.0), (-1.0, 4.0)].into_iter()), Some([-1.0, 2.0, 1.0, 4.0]));
    assert_eq!(bounds(std::iter::empty()), None);
}
