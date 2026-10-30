//! The order's parts the cases in `tests/auto_order.rs` can't reach: what becomes of an element whose
//! neighbours, or whose paint, need more work than the budget allows, and of an order that does
//! (`REQ-AUTO-012`); the box round a shape, which holds what it paints; and the graph of what sews first.
//! The elements with settings and the parts of a shape keep their order here too, as they do there: the
//! cases here run most of the order's code, and `cargo xtask coverage` counts a function's lines in the
//! kind of test that runs most of them.

use stitchcraft_core::{Budget, ElementId, Mm};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::Thread;

use super::*;
use crate::auto::neighbours::{Sewn, under};
use crate::design::{Cap, DesignSettings, FillRule, Path, Segment, Subpath};

const RED: Thread = Thread::new(Rgb::new(200, 0, 0));
const BLUE: Thread = Thread::new(Rgb::new(0, 0, 200));

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A path through `points`, `closed` or open.
fn through(points: &[(f64, f64)], closed: bool) -> Path {
    let (&(x, y), rest) = points.split_first().unwrap();
    Path { subpaths: vec![Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed }] }
}

/// An element `id` of `shape`, sewn with `thread`, without settings.
fn element(id: &str, shape: Shape, thread: &Thread) -> Element {
    Element { id: ElementId::new(id).unwrap(), name: None, shape, thread: thread.clone(), params: ParamSet::default(), automatic: false }
}

/// An automatic area `id`, the square `size` wide from `(x, y)`, sewn with `thread`, a shape of its own.
fn square(id: &str, (x, y): (f64, f64), size: f64, thread: &Thread) -> Sewn {
    let ring = [(x, y), (x, y + size), (x + size, y + size), (x + size, y)];
    let shape = Shape::Fill { path: through(&ring, true), rule: FillRule::NonZero };
    Sewn { element: element(id, shape, thread), area: true, automatic: true, shape: usize::MAX }
}

/// An automatic running line `id` along `points`, `width` wide, sewn with `thread`, a shape of its own.
fn running(id: &str, points: &[(f64, f64)], width: f64, thread: &Thread) -> Sewn {
    let shape = Shape::Stroke { path: through(points, false), width: Mm::new(width).unwrap(), join: Join::Round, cap: Cap::Butt };
    Sewn { element: element(id, shape, thread), area: false, automatic: true, shape: usize::MAX }
}

/// `sewn` in the order it sews in, each shape's paint found with `painting` and the order with `budget`;
/// its ids, and what is said of it. Each element is a shape of its own unless it says which it is a part of.
fn ordered_with(sewn: Vec<Sewn>, painting: &Budget, budget: &Budget) -> (Vec<String>, Vec<Diagnostic>) {
    let settings = DesignSettings::default();
    let shape = |index: usize, sewn: &Sewn| if sewn.shape == usize::MAX { index } else { sewn.shape };
    let moving: Vec<Moving> = sewn.iter().enumerate().map(|(index, sewn)| Moving { free: sewn.automatic, shape: shape(index, sewn) }).collect();
    let mut shapes = Shapes::new(sewn, &settings, painting);
    let mut said = super::ordered(&mut shapes, &moving, budget);
    said.append(&mut shapes.said);
    (shapes.elements.iter().map(|element| element.id.as_str().to_string()).collect(), said)
}

/// What `said` says, as a user reads it, each followed by the element it names, if any, in brackets.
fn messages(said: &[Diagnostic]) -> Vec<String> {
    said.iter().map(|d| d.element.as_ref().map_or_else(|| d.to_string(), |id| format!("{d} [{id}]"))).collect()
}

#[test]
fn req_auto_012_nothing_moves_across_an_element_whose_neighbours_need_more_work_than_its_budget() {
    // A red square, 2 blue ones that overlap, and a red one far from them: with the budget, the reds sew one
    // after the other.
    let squares = || {
        vec![
            square("r1", (-100.0, 0.0), 10.0, &RED),
            square("b1", (0.0, 0.0), 10.0, &BLUE),
            square("b2", (5.0, 0.0), 10.0, &BLUE),
            square("r2", (100.0, 0.0), 10.0, &RED),
        ]
    };
    let (ids, _) = ordered_with(squares(), &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["r1", "r2", "b1", "b2"]);
    // With 5 units, finding how `b1` lies beside `b2` needs more: `b1` keeps its place, and `r2` doesn't move
    // before it, far from it as it is. Placing the 4 takes the 5 units.
    let (ids, said) = ordered_with(squares(), &Budget::DEFAULT, &Budget { max_work: 5, ..Budget::DEFAULT });
    assert_eq!(ids, ["r1", "b1", "b2", "r2"]);
    assert_eq!(
        messages(&said),
        ["info SC-I0907: Finding where this element meets the shapes drawn after it needed more than the work budget of 5 units, so it keeps \
          its place in paint order: the elements drawn before it sew before it, and those drawn after it after it. [b1]"]
    );
}

#[test]
fn elements_with_settings_and_the_parts_of_a_shape_keep_their_order() {
    let fixed = |sewn: Sewn| Sewn { automatic: false, ..sewn };
    let part = |sewn: Sewn| Sewn { shape: 9, ..sewn };
    // An automatic blue square, red and blue ones with settings, and 2 parts of one red shape, the second
    // nearer: the blue one with settings waits for the red one, and the second part for the first.
    let sewn = vec![
        square("a", (0.0, 0.0), 10.0, &BLUE),
        fixed(square("s1", (20.0, 0.0), 10.0, &RED)),
        fixed(square("s2", (40.0, 0.0), 10.0, &BLUE)),
        part(square("p1", (100.0, 0.0), 10.0, &RED)),
        part(square("p2", (60.0, 0.0), 10.0, &RED)),
    ];
    let (ids, _) = ordered_with(sewn, &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["a", "s1", "p1", "p2", "s2"]);
    // With settings only, nothing moves, and nothing is said.
    let sewn = vec![
        fixed(square("s1", (0.0, 0.0), 10.0, &RED)),
        fixed(square("s2", (20.0, 0.0), 10.0, &BLUE)),
        fixed(square("s3", (40.0, 0.0), 10.0, &RED)),
    ];
    let (ids, said) = ordered_with(sewn, &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!((ids, said.len()), (vec!["s1".to_string(), "s2".to_string(), "s3".to_string()], 0));
}

#[test]
fn req_auto_012_an_order_that_needs_more_work_than_the_budget_sews_the_elements_not_placed_in_paint_order() {
    // 4 squares apart, red, blue, red, blue: placing the 2 reds takes 7 units, a unit for each element free
    // to sew each time, and finding how they lie 3 units at most.
    let squares = || {
        vec![
            square("r1", (0.0, 0.0), 10.0, &RED),
            square("b1", (20.0, 0.0), 10.0, &BLUE),
            square("r2", (40.0, 0.0), 10.0, &RED),
            square("b2", (60.0, 0.0), 10.0, &BLUE),
        ]
    };
    let (ids, said) = ordered_with(squares(), &Budget::DEFAULT, &Budget { max_work: 7, ..Budget::DEFAULT });
    assert_eq!(ids, ["r1", "r2", "b1", "b2"]);
    assert_eq!(
        messages(&said),
        [
            "info SC-I0907: Choosing the order the elements sew in needed more than the work budget of 7 units, so this element and the \
             others not placed by then sew in paint order, after those placed. [b1]",
            "info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 3.",
        ]
    );
    // With less than the first element's 4 units, nothing is placed, and the design sews in paint order.
    let (ids, said) = ordered_with(squares(), &Budget::DEFAULT, &Budget { max_work: 3, ..Budget::DEFAULT });
    assert_eq!(ids, ["r1", "b1", "r2", "b2"]);
    assert_eq!(said.len(), 1, "{said:?}");
}

#[test]
fn req_auto_012_a_line_whose_paint_needs_more_work_than_the_budget_keeps_its_place_beside_the_shapes_near_it() {
    // A line along the bottom of a square, drawn before it: with the budget, it sews after the square it
    // borders.
    let drawn = || vec![running("line", &[(0.0, -0.25), (10.0, -0.25)], 0.5, &BLUE), square("square", (0.0, 0.0), 10.0, &RED)];
    let (ids, _) = ordered_with(drawn(), &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["square", "line"]);
    // With 10 units, what it covers is not found, and it keeps its place, as does the square, whose region
    // isn't built either.
    let (ids, said) = ordered_with(drawn(), &Budget { max_work: 10, ..Budget::DEFAULT }, &Budget::DEFAULT);
    assert_eq!(ids, ["line", "square"]);
    assert_eq!(
        messages(&said),
        ["info SC-I0907: Finding what this line covers needed more than the work budget of 10 units, so it keeps its place in paint order \
          beside the shapes near it. [line]"]
    );
}

#[test]
fn an_area_reaches_under_no_line_sewn_after_it() {
    // A line across the square's top edge, sewn after it: the square is sewn whole under it.
    let sewn = vec![square("square", (0.0, 0.0), 10.0, &RED), running("line", &[(-5.0, 10.0), (15.0, 10.0)], 0.5, &BLUE)];
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(sewn, &settings, &Budget::DEFAULT);
    shapes.find(1);
    assert!(matches!(shapes.paint(1), Some((Kind::Line, _))));
    assert!(under::reached(&mut shapes, &Budget::DEFAULT).is_empty());
    assert!(!shapes.changed[0]);
}

#[test]
fn a_shape_whose_paint_cant_be_found_is_boxed_by_its_path() {
    let line = |join: Join| Shape::Stroke { path: through(&[(0.0, 0.0), (10.0, 0.0)], false), width: Mm::new(2.0).unwrap(), join, cap: Cap::Butt };
    let boxed = |shape: Shape| drawn(&element("e", shape, &RED)).map(|e| [e.min_x, e.min_y, e.max_x, e.max_y]);
    // A miter limited to 4 reaches, clipped, √17 times half the width, and any other corner or cap half the
    // width times √2, as a square cap's corner does.
    let clipped = 17.0_f64.sqrt();
    assert_eq!(boxed(line(Join::Miter { limit: 4.0 })), Some([-clipped, -clipped, 10.0 + clipped, clipped]));
    assert_eq!(boxed(line(Join::Miter { limit: 1.0 })), Some([-SQRT_2, -SQRT_2, 10.0 + SQRT_2, SQRT_2]));
    assert_eq!(boxed(line(Join::Round)), Some([-SQRT_2, -SQRT_2, 10.0 + SQRT_2, SQRT_2]));
    let fill = |path: Path| Shape::Fill { path, rule: FillRule::NonZero };
    assert_eq!(boxed(fill(through(&[(0.0, 0.0), (10.0, 0.0), (10.0, 5.0)], true))), Some([0.0, 0.0, 10.0, 5.0]));
    assert_eq!(boxed(fill(Path::default())), None);
}

#[test]
fn runs_end_at_each_element_nothing_moves_across() {
    let cut = |stops: &'static [bool]| move |index: usize| stops[index];
    let live = [0, 1, 2, 3, 4];
    assert_eq!(runs(&live, cut(&[false, true, false, false, true])), [vec![0], vec![1], vec![2, 3], vec![4]]);
    assert_eq!(runs(&live, cut(&[true, false, false, false, false])), [vec![0], vec![1, 2, 3, 4]]);
    // An element left out is in no run.
    assert_eq!(runs(&[0, 2], cut(&[false, false, false])), [vec![0, 2]]);
}

#[test]
fn a_line_waits_for_no_fill_it_borders_beyond_an_element_kept_in_its_place() {
    // A line, an element kept in its place, and a fill the line borders, each a run of its own: the line
    // sews first, as it is drawn, and doesn't wait for the fill.
    let facts: Vec<Facts> = (0..3).map(|_| Facts { colour: RED.color, stop: false, reach: None }).collect();
    let moving: Vec<Moving> = (0..3).map(|shape| Moving { free: true, shape }).collect();
    let mut order = Order { facts: &facts, moving: &moving, graph: Graph::new(3), borders: vec![(2, 0)], kept: BTreeSet::from([1]) };
    let mut sewing = Sewing::default();
    order.sew(&[vec![0], vec![1], vec![2]], &mut sewing, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(sewing.order, [0, 1, 2]);
}

#[test]
fn the_graph_says_what_must_sew_before_what() {
    let mut graph = Graph::new(4);
    graph.add(0, 1);
    graph.add(0, 1);
    graph.add(1, 2);
    graph.add(0, 2);
    // An edge added twice holds back once.
    assert_eq!((graph.waiting(1), graph.waiting(2), graph.waiting(3)), (1, 2, 0));
    let mut meter = Budget::DEFAULT.meter();
    assert!(graph.reaches(0, 2, &mut meter).unwrap() && graph.reaches(1, 2, &mut meter).unwrap());
    assert!(!graph.reaches(2, 0, &mut meter).unwrap() && !graph.reaches(0, 3, &mut meter).unwrap());
    // A unit for each edge followed.
    assert!(graph.reaches(0, 3, &mut Budget { max_work: 2, ..Budget::DEFAULT }.meter()).is_err());
    assert_eq!(graph.release(0), [1]);
    assert_eq!(graph.release(1), [2]);
}

#[test]
fn boxes_are_apart_by_their_gap_and_their_centres() {
    let boxed = |x0: f64, y0: f64, x1: f64, y1: f64| Envelope { min_x: x0, min_y: y0, max_x: x1, max_y: y1 };
    let a = boxed(0.0, 0.0, 10.0, 10.0);
    assert_eq!(gap(&a, &boxed(13.0, 14.0, 20.0, 20.0)), 9.0 + 16.0);
    assert_eq!(gap(&boxed(13.0, 14.0, 20.0, 20.0), &a), 9.0 + 16.0);
    assert_eq!(gap(&a, &boxed(5.0, 5.0, 30.0, 30.0)), 0.0);
    assert_eq!(apart(&a, &boxed(10.0, 0.0, 30.0, 4.0)), 15.0 * 15.0 + 3.0 * 3.0);
}

#[test]
fn an_area_looks_for_no_lines_paint() {
    // A square, and after it a line far from it that zigzags through 200 points, whose paint takes far more
    // work to find than the square's region: the square reaches under no line, so it doesn't look for what
    // the line paints, and says nothing of it.
    let zigzag: Vec<(f64, f64)> = (0..200).map(|k| (500.0 + f64::from(k) * 0.5, f64::from(k % 2))).collect();
    let settings = DesignSettings::default();
    let sewn = vec![square("square", (0.0, 0.0), 10.0, &RED), running("line", &zigzag, 0.5, &BLUE)];
    let mut shapes = Shapes::new(sewn, &settings, &Budget { max_work: 1_000, ..Budget::DEFAULT });
    assert!(shapes.region(0).is_some(), "the square's region is built");
    assert!(under::reached(&mut shapes, &Budget::DEFAULT).is_empty());
    assert!(shapes.said.is_empty(), "{:?}", shapes.said);
    assert!(shapes.painted[1].is_none(), "what the line paints was looked for");
}

#[test]
fn a_columns_box_holds_the_corners_of_its_clipped_miters() {
    // A satin column 7 mm wide with settings, drawn as one path that turns back sharply at (60, 0), its miter
    // limit 4: its rails' miter there is clipped 14 mm from the corner, 4 times half its width, and the
    // clipped miter's corners lie further from it, one of them past x = 74.1.
    let path = through(&[(0.0, 0.0), (60.0, 0.0), (0.0, 10.0)], false);
    let shape = Shape::Stroke { path, width: Mm::new(7.0).unwrap(), join: Join::Miter { limit: 4.0 }, cap: Cap::Butt };
    let column = Element { params: [("satin_column", "true")].into_iter().collect(), ..element("column", shape, &RED) };
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(vec![Sewn { element: column.clone(), area: false, automatic: false, shape: 0 }], &settings, &Budget::DEFAULT);
    shapes.find(0);
    let Some((Kind::Column, parts)) = shapes.paint(0) else { panic!("{:?}", shapes.said) };
    let (paint, boxed) = (envelope(parts).unwrap(), drawn(&column).unwrap());
    assert!(paint.max_x > 74.1, "{paint:?}");
    assert!(
        boxed.min_x <= paint.min_x && boxed.min_y <= paint.min_y && paint.max_x <= boxed.max_x && paint.max_y <= boxed.max_y,
        "{paint:?} in {boxed:?}"
    );
}
