use stitchcraft_core::{Budget, Code, ElementId, Mm, Point};

use super::*;
use crate::design::{Cap, FillRule, Join, Path, Segment, Shape, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A mark `id` round the box from (`x0`, `y0`) to (`x1`, `y1`), drawn with a stroke and no fill.
fn mark(id: &str, around: [f64; 4]) -> Mark {
    edged(id, around, 1)
}

/// A mark `id` round the box from (`x0`, `y0`) to (`x1`, `y1`), each side cut into `n` segments, drawn with a
/// stroke and no fill.
pub(crate) fn edged(id: &str, [x0, y0, x1, y1]: [f64; 4], n: u32) -> Mark {
    let step = |from: (f64, f64), to: (f64, f64)| {
        (0..n).map(move |i| {
            let t = f64::from(i) / f64::from(n);
            (from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t)
        })
    };
    let corners = [(x0, y0), (x1, y0), (x1, y1), (x0, y1), (x0, y0)];
    let ring: Vec<(f64, f64)> = corners.windows(2).flat_map(|pair| step(pair[0], pair[1])).collect();
    let outline = Subpath { start: p(x0, y0), segments: ring.iter().skip(1).map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: true };
    let shape = Shape::Stroke { path: Path { subpaths: vec![outline] }, width: Mm::new(0.2).unwrap(), join: Join::Round, cap: Cap::Butt };
    Mark { id: ElementId::new(id).unwrap(), shape }
}

/// A square part from (`x`, `y`), `side` across.
fn part(x: f64, y: f64, side: f64) -> Polygon {
    Polygon { outline: vec![p(x, y), p(x + side, y), p(x + side, y + side), p(x, y + side)], holes: Vec::new() }
}

fn budget(work: u64) -> Budget {
    Budget { max_work: work, ..Budget::DEFAULT }
}

#[test]
fn req_auto_104_without_marks_nothing_is_tested_or_charged() {
    let marks = Marks::default();
    let mut meter = budget(0).meter();
    assert_eq!(marks.holding(&part(0.0, 0.0, 1.0), &mut meter), Ok(None));
    assert_eq!(marks.held(&[part(0.0, 0.0, 1.0)], &mut meter), Ok(vec![None]));
    assert!(marks.said(true).is_empty());
}

#[test]
fn req_auto_104_a_part_is_held_when_every_point_of_its_outline_is() {
    let marks = Marks::of(&[mark("a", [0.0, 0.0, 10.0, 10.0]), mark("b", [20.0, 0.0, 30.0, 10.0])], &Budget::DEFAULT);
    let mut meter = Budget::DEFAULT.meter();
    assert_eq!(marks.holding(&part(1.0, 1.0, 2.0), &mut meter), Ok(Some(0)));
    assert_eq!(marks.holding(&part(21.0, 1.0, 2.0), &mut meter), Ok(Some(1)));
    // Across the edge, outside both boxes, and with no outline.
    assert_eq!(marks.holding(&part(9.0, 1.0, 2.0), &mut meter), Ok(None));
    assert_eq!(marks.holding(&part(40.0, 1.0, 2.0), &mut meter), Ok(None));
    assert_eq!(marks.holding(&Polygon { outline: Vec::new(), holes: Vec::new() }, &mut meter), Ok(None));
    // In a mark's box but not in the mark: an L-shaped mark leaves its box's corner out.
    let corner = Subpath {
        start: p(0.0, 0.0),
        segments: [(10.0, 0.0), (10.0, 2.0), (2.0, 2.0), (2.0, 10.0), (0.0, 10.0)].map(|(x, y)| Segment::Line(p(x, y))).to_vec(),
        closed: true,
    };
    let shape = Shape::Fill { path: Path { subpaths: vec![corner] }, rule: FillRule::NonZero, gradient: None };
    let l = Marks::of(&[Mark { id: ElementId::new("l").unwrap(), shape }], &Budget::DEFAULT);
    assert_eq!(l.holding(&part(5.0, 5.0, 1.0), &mut meter), Ok(None));
    assert_eq!(l.holding(&part(0.5, 0.5, 1.0), &mut meter), Ok(Some(0)));
}

#[test]
fn req_auto_104_testing_a_part_costs_its_points_a_unit_a_mark_and_its_points_times_the_marks_segments() {
    let marks = Marks::of(&[mark("a", [0.0, 0.0, 10.0, 10.0]), mark("b", [20.0, 0.0, 30.0, 10.0])], &Budget::DEFAULT);
    // In b: 4 points, a unit for a (whose box doesn't hold it), a unit for b, then 4 × 5, b's ring flattened
    // into 5 points, its last its first.
    let inside_b = part(21.0, 1.0, 2.0);
    let mut meter = budget(26).meter();
    assert_eq!(marks.holding(&inside_b, &mut meter), Ok(Some(1)));
    assert!(meter.charge(1).is_err());
    for short in [3, 4, 5, 25] {
        assert!(marks.holding(&inside_b, &mut budget(short).meter()).is_err(), "{short}");
    }
    assert!(marks.held(&[inside_b], &mut budget(25).meter()).is_err());
}

#[test]
fn diag_sc_i0916_a_mark_names_the_parts_it_holds_and_what_it_kept() {
    let marks = Marks::of(&[mark("eye", [0.0, 0.0, 10.0, 10.0]), mark("one", [20.0, 0.0, 30.0, 10.0])], &Budget::DEFAULT);
    let held = marks.held(&[part(1.0, 1.0, 1.0), part(3.0, 3.0, 1.0), part(5.0, 5.0, 9.0), part(21.0, 1.0, 1.0)], &mut Budget::DEFAULT.meter());
    assert_eq!(held, Ok(vec![Some(0), Some(0), None, Some(1)]));
    marks.kept_speck(0);
    marks.kept_line(0);
    marks.kept_line(0);
    // A place no mark has counts nothing.
    marks.kept_speck(5);
    marks.kept_line(5);
    let said = marks.said(true);
    assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::AutoKept, Code::AutoKept]);
    assert_eq!(said[0].element.as_ref().map(ElementId::as_str), Some("eye"));
    assert!(
        said[0].message.contains("holds 2 parts of automatic areas wholly")
            && said[0].message.contains("Of them, 1 speck and 2 thin lines sew by their width"),
        "{}",
        said[0]
    );
    assert!(said[1].message.contains("holds 1 part of") && said[1].message.contains("Of them, 0 specks and 0 thin lines sew"), "{}", said[1]);
}

#[test]
fn diag_sc_w0917_a_mark_that_keeps_nothing_is_named_with_why() {
    let empty = Mark { id: ElementId::new("flat").unwrap(), shape: Shape::stroke(Path { subpaths: Vec::new() }) };
    let marks = Marks::of(&[mark("none", [0.0, 0.0, 10.0, 10.0]), empty], &Budget::DEFAULT);
    let said = marks.said(true);
    assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::AutoMarkUnused, Code::AutoMarkUnused]);
    assert!(said[0].message.starts_with("The mark `flat` is not used, and the design sews as without it: it encloses no area."), "{}", said[0]);
    assert!(said[1].message.ends_with("no part of an automatic area lies wholly inside it."), "{}", said[1]);
    // In a design with no automatic shape, whether its area was read or not.
    let held = Marks::of(&[mark("auto", [0.0, 0.0, 10.0, 10.0])], &Budget::DEFAULT);
    held.held(&[part(1.0, 1.0, 1.0)], &mut Budget::DEFAULT.meter()).unwrap();
    for said in [held.said(false), unused_all(&[mark("auto", [0.0, 0.0, 10.0, 10.0])], &[])] {
        assert_eq!(said.len(), 1);
        assert!(said[0].message.ends_with("the design has no shape digitized automatically."), "{}", said[0]);
        assert_eq!(said[0].element.as_ref().map(ElementId::as_str), Some("auto"));
    }
    // A mark whose area needs more work than the budget allows to read.
    let said = Marks::of(&[mark("big", [0.0, 0.0, 10.0, 10.0])], &budget(0)).said(true);
    assert_eq!(said.len(), 1);
    assert!(said[0].message.ends_with("reading its area needed more than the work budget of 0 units."), "{}", said[0]);
}
