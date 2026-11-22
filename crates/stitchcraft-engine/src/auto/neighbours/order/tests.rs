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
use crate::auto::AutoParams;
use crate::auto::neighbours::{Past, Sewn, under};
use crate::design::{Cap, DesignSettings, FillRule, Path, Segment, Subpath};

pub(super) const RED: Thread = Thread::new(Rgb::new(200, 0, 0));
pub(super) const BLUE: Thread = Thread::new(Rgb::new(0, 0, 200));
pub(super) const GREEN: Thread = Thread::new(Rgb::new(0, 160, 0));

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A path through `points`, `closed` or open.
pub(super) fn through(points: &[(f64, f64)], closed: bool) -> Path {
    let (&(x, y), rest) = points.split_first().unwrap();
    Path { subpaths: vec![Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed }] }
}

/// An element `id` of `shape`, sewn with `thread`, without settings.
pub(super) fn element(id: &str, shape: Shape, thread: &Thread) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape,
        thread: thread.clone(),
        params: ParamSet::default(),
        automatic: false,
        lettered: None,
        hints: Default::default(),
        clips: Vec::new(),
    }
}

/// An automatic area `id`, the square `size` wide from `(x, y)`, sewn with `thread`, a shape of its own.
pub(super) fn square(id: &str, (x, y): (f64, f64), size: f64, thread: &Thread) -> Sewn {
    let ring = [(x, y), (x, y + size), (x + size, y + size), (x + size, y)];
    let shape = Shape::Fill { path: through(&ring, true), rule: FillRule::NonZero, gradient: None };
    Sewn { element: element(id, shape, thread), area: Some(AutoParams::defaults()), automatic: true, reach: None, shape: usize::MAX, part: None }
}

/// An automatic running line `id` along `points`, `width` wide, sewn with `thread`, a shape of its own.
pub(super) fn running(id: &str, points: &[(f64, f64)], width: f64, thread: &Thread) -> Sewn {
    let shape = Shape::Stroke { path: through(points, false), width: Mm::new(width).unwrap(), join: Join::Round, cap: Cap::Butt };
    Sewn { element: element(id, shape, thread), area: None, automatic: true, reach: None, shape: usize::MAX, part: None }
}

/// An automatic satin column `id` along the middle of the part with `corners`, sewn with `thread`, which
/// reaches 1 mm under a shape sewn after it.
pub(super) fn column(id: &str, corners: &[(f64, f64)], thread: &Thread) -> Sewn {
    let part = Polygon { outline: corners.iter().map(|&(x, y)| p(x, y)).collect(), holes: Vec::new() };
    let middle = |a: usize, b: usize| ((corners[a].0 + corners[b].0) / 2.0, (corners[a].1 + corners[b].1) / 2.0);
    let line = running(id, &[middle(0, 3), middle(1, 2)], 2.0, thread);
    let element = Element { params: [("satin_column", "true")].into_iter().collect(), ..line.element };
    Sewn { element, reach: Some(Past { overlap: 1.0, column: 0.4 }), part: Some(part), ..line }
}

/// `sewn` in the order it sews in, each shape's paint found with `painting` and the order with `budget`;
/// its ids, and what is said of it. Each element is a shape of its own unless it says which it is a part of.
pub(super) fn ordered_with(sewn: Vec<Sewn>, painting: &Budget, budget: &Budget) -> (Vec<String>, Vec<Diagnostic>) {
    ordered_under(sewn, painting, budget, None)
}

/// [`ordered_with`], the thread `under` sewing when the elements start.
pub(super) fn ordered_under(sewn: Vec<Sewn>, painting: &Budget, budget: &Budget, under: Option<Rgb>) -> (Vec<String>, Vec<Diagnostic>) {
    let (ids, said, _) = ordered_by(sewn, painting, budget, under, Rules { last: true, once: true });
    (ids, said)
}

/// [`ordered_under`] by `rules`; and the thread the base layer sews in when the order chose another.
pub(super) fn ordered_by(sewn: Vec<Sewn>, painting: &Budget, budget: &Budget, under: Option<Rgb>, rules: Rules) -> (Vec<String>, Vec<Diagnostic>, Option<Rgb>) {
    let settings = DesignSettings::default();
    let shape = |index: usize, sewn: &Sewn| if sewn.shape == usize::MAX { index } else { sewn.shape };
    let moving: Vec<Moving> =
        sewn.iter().enumerate().map(|(index, sewn)| Moving { free: sewn.automatic, made: sewn.automatic, shape: shape(index, sewn) }).collect();
    let mut shapes = Shapes::new(sewn, &settings, painting);
    let ordered = super::ordered(&mut shapes, &moving, budget, under, rules);
    let mut said = ordered.said;
    said.append(&mut shapes.said);
    (shapes.elements.iter().map(|element| element.id.as_str().to_string()).collect(), said, ordered.under)
}

/// What `said` says, as a user reads it, each followed by the element it names, if any, in brackets.
pub(super) fn messages(said: &[Diagnostic]) -> Vec<String> {
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
        [
            "info SC-I0907: Finding where this element meets the shapes drawn after it needed more than the work budget of 5 units, so it \
             keeps its place in paint order: the elements drawn before it sew before it, and those drawn after it after it. [b1]"
                .to_string(),
            unsought_said(5, "r1"),
        ]
    );
}

#[test]
fn elements_with_settings_and_the_parts_of_a_shape_keep_their_order() {
    let fixed = |sewn: Sewn| Sewn { automatic: false, ..sewn };
    let part = |sewn: Sewn| Sewn { shape: 9, ..sewn };
    // An automatic blue square, red and blue ones with settings, and 2 parts of one red shape, the second
    // nearer: the blue one with settings waits for the red one, and the second part for the first. The red
    // ones sew first, so that the blue ones sew together.
    let sewn = vec![
        square("a", (0.0, 0.0), 10.0, &BLUE),
        fixed(square("s1", (20.0, 0.0), 10.0, &RED)),
        fixed(square("s2", (40.0, 0.0), 10.0, &BLUE)),
        part(square("p1", (100.0, 0.0), 10.0, &RED)),
        part(square("p2", (60.0, 0.0), 10.0, &RED)),
    ];
    let (ids, _) = ordered_with(sewn, &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["s1", "p1", "p2", "s2", "a"]);
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
    // to sew each time, and finding how they lie 3 units at most. The fewest thread changes aren't found
    // with as little, so each thread sews while it lasts.
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
            unsought_said(7, "r1"),
            "info SC-I0907: Choosing the order the elements sew in needed more than the work budget of 7 units, so this element and the \
             others not placed by then sew in paint order, after those placed. [b1]"
                .to_string(),
            "info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 3.".to_string(),
        ]
    );
    // With less than the first element's 4 units, nothing is placed, and the design sews in paint order.
    let (ids, said) = ordered_with(squares(), &Budget::DEFAULT, &Budget { max_work: 3, ..Budget::DEFAULT });
    assert_eq!(ids, ["r1", "b1", "r2", "b2"]);
    assert_eq!(said.len(), 2, "{said:?}");
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
    let fill = |path: Path| Shape::Fill { path, rule: FillRule::NonZero, gradient: None };
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
    let facts: Vec<Facts> = (0..3).map(|_| Facts { colours: vec![RED.color], stop: false, reach: None }).collect();
    let moving: Vec<Moving> = (0..3).map(|shape| Moving { free: true, made: true, shape }).collect();
    let mut order = Order {
        facts: &facts,
        moving: &moving,
        graph: Graph::new(3),
        borders: vec![(2, 0)],
        wished: Vec::new(),
        last: true,
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept: BTreeSet::from([1]),
        unsought: None,
        under: None,
        once: true,
        overlaps: Vec::new(),
        found: Vec::new(),
        chosen: None,
    };
    let mut sewing = Sewing::default();
    order.sew(&[vec![0], vec![1], vec![2]], &mut sewing, &Budget::DEFAULT).unwrap();
    assert_eq!(sewing.order, [0, 1, 2]);
}

#[test]
fn req_auto_012_nothing_moves_across_an_element_kept_in_its_place_and_each_sews_once() {
    // 0 overlaps 2 across 1, which keeps its place, and 2 and 3 both overlap 4, drawn after them: placing 0
    // frees 2, which waits for its run all the same, after 1; and 4 waits for 3.
    let facts: Vec<Facts> = [RED, RED, RED, BLUE, RED].iter().map(|thread| Facts { colours: vec![thread.color], stop: false, reach: None }).collect();
    let moving: Vec<Moving> = (0..5).map(|shape| Moving { free: true, made: true, shape }).collect();
    let mut graph = Graph::new(5);
    for (from, to) in [(0, 2), (2, 4), (3, 4)] {
        graph.add(from, to);
    }
    let mut order = Order {
        facts: &facts,
        moving: &moving,
        graph,
        borders: Vec::new(),
        wished: Vec::new(),
        last: true,
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept: BTreeSet::from([1]),
        unsought: None,
        under: None,
        once: true,
        overlaps: Vec::new(),
        found: Vec::new(),
        chosen: None,
    };
    let mut sewing = Sewing::default();
    order.sew(&[vec![0], vec![1], vec![2, 3, 4]], &mut sewing, &Budget::DEFAULT).unwrap();
    assert_eq!(sewing.order, [0, 1, 2, 3, 4]);
}

/// The order of elements, each sewn in the threads of its colours in turn, all free to move and with no box,
/// which the order places as drawn when no thread tells them apart; and its thread changes, then paint
/// order's.
fn threaded(colours: &[&[Rgb]]) -> (Vec<usize>, usize, usize) {
    let facts: Vec<Facts> = colours.iter().map(|colours| Facts { colours: colours.to_vec(), stop: false, reach: None }).collect();
    let moving: Vec<Moving> = (0..facts.len()).map(|shape| Moving { free: true, made: true, shape }).collect();
    let mut order = Order {
        facts: &facts,
        moving: &moving,
        graph: Graph::new(facts.len()),
        borders: Vec::new(),
        wished: Vec::new(),
        last: true,
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept: BTreeSet::new(),
        unsought: None,
        under: None,
        once: true,
        overlaps: Vec::new(),
        found: Vec::new(),
        chosen: None,
    };
    let mut sewing = Sewing::default();
    let painted: Vec<usize> = (0..facts.len()).collect();
    order.sew(std::slice::from_ref(&painted), &mut sewing, &Budget::DEFAULT).unwrap();
    let (sewn, before) = (changes(&sewing.order, &facts), changes(&painted, &facts));
    (sewing.order, sewn, before)
}

#[test]
fn req_auto_024_an_element_in_several_threads_starts_in_its_first_and_leaves_its_last_sewing() {
    let (red, blue, green) = (RED.color, BLUE.color, GREEN.color);
    // Green then red, a blue, a red: once the first is sewn, red sews on. The blue sews before them, which
    // changes thread as seldom as the blue last. In paint order the thread changes inside the first and twice
    // after it.
    assert_eq!(threaded(&[&[green, red], &[blue], &[red]]), (vec![1, 0, 2], 2, 3));
    // A red, a blue, red then blue: the third starts in red, which is sewing, and leaves blue sewing.
    assert_eq!(threaded(&[&[red], &[blue], &[red, blue]]), (vec![0, 2, 1], 1, 3));
    // Its colours stay together and in their order: one that starts in blue and ends in red waits for no
    // thread of its own, and a red sews after it. The green sews before them, as well as after.
    assert_eq!(threaded(&[&[blue], &[blue, red], &[green], &[red]]), (vec![2, 0, 1, 3], 2, 3));
    // Of one thread, nothing moves, as before.
    assert_eq!(threaded(&[&[red], &[red, red], &[red]]), (vec![0, 1, 2], 0, 0));
}

#[test]
fn req_auto_024_the_thread_changes_inside_an_element_are_between_colours_that_differ() {
    let (red, blue) = (RED.color, BLUE.color);
    let changes = |colours: &[Rgb]| Facts { colours: colours.to_vec(), stop: false, reach: None }.changes();
    assert_eq!([changes(&[]), changes(&[red]), changes(&[red, red]), changes(&[red, blue]), changes(&[red, blue, red])], [0, 0, 0, 1, 2]);
    let ends = |colours: &[Rgb]| {
        let facts = Facts { colours: colours.to_vec(), stop: false, reach: None };
        (facts.first(), facts.last())
    };
    assert_eq!(ends(&[red, blue]), (Some(red), Some(blue)));
    assert_eq!(ends(&[]), (None, None));
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
fn a_narrow_parts_line_is_boxed_round_the_part() {
    // The line along a strip 0.6 mm wide with a spike 1.4 mm high that the line leaves out: the box round the
    // part holds the spike, which the box round the line and its width doesn't.
    let corners = [(0.0, 0.0), (0.0, 0.6), (4.9, 0.6), (5.0, 2.0), (5.1, 0.6), (10.0, 0.6), (10.0, 0.0), (0.0, 0.0)];
    let part = Polygon { outline: corners.iter().map(|&(x, y)| p(x, y)).collect(), holes: Vec::new() };
    let line = Sewn { part: Some(part.clone()), ..running("line", &[(0.3, 0.3), (9.7, 0.3)], 0.6, &BLUE) };
    let shapes = Shapes::new(vec![line], &DesignSettings::default(), &Budget::DEFAULT);
    assert_eq!(reach(&shapes, 0), envelope(&[part]));
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
    let mut shapes = Shapes::new(
        vec![Sewn { element: column.clone(), area: None, automatic: false, reach: None, shape: 0, part: None }],
        &settings,
        &Budget::DEFAULT,
    );
    shapes.find(0);
    let Some((Kind::Column, parts)) = shapes.paint(0) else { panic!("{:?}", shapes.said) };
    let (paint, boxed) = (envelope(parts).unwrap(), drawn(&column).unwrap());
    assert!(paint.max_x > 74.1, "{paint:?}");
    assert!(
        boxed.min_x <= paint.min_x && boxed.min_y <= paint.min_y && paint.max_x <= boxed.max_x && paint.max_y <= boxed.max_y,
        "{paint:?} in {boxed:?}"
    );
}

/// The lowest `SC-I0907` says of a search that needed more than `work` units, for the element `id`.
fn unsought_said(work: u64, id: &str) -> String {
    format!(
        "info SC-I0907: Finding the order with the fewest thread changes needed more than the work budget of {work} units, so from this \
         element on each thread sews while an element of it is free to sew, then the thread of the first free element in paint order, and \
         every satin column sews after the fills it borders. [{id}]"
    )
}

#[test]
fn req_auto_037_the_elements_sew_in_the_order_that_changes_thread_least() {
    // A red square, a blue one, and a red one that overlaps the blue: red first, the thread of the first
    // free element, comes back to red after the blue. Blue first sews the reds one after the other.
    let squares = || vec![square("r1", (-100.0, 0.0), 10.0, &RED), square("b1", (0.0, 0.0), 10.0, &BLUE), square("r2", (5.0, 0.0), 10.0, &RED)];
    let (ids, said) = ordered_with(squares(), &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["b1", "r2", "r1"]);
    assert_eq!(messages(&said), ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2."]);
    // With less work than a row of the search costs, each thread sews while it lasts, as before M13.22, which
    // is paint order here, and that is said.
    let work = fewest::STATE - 1;
    let (ids, said) = ordered_with(squares(), &Budget::DEFAULT, &Budget { max_work: work, ..Budget::DEFAULT });
    assert_eq!(ids, ["r1", "b1", "r2"]);
    assert_eq!(messages(&said), [unsought_said(work, "r1")]);
}

#[test]
fn req_auto_037_the_thread_of_the_base_layer_sews_first() {
    // A red square and a blue one, apart: as drawn, and the blue first when blue is sewing at the start.
    let squares = || vec![square("r", (0.0, 0.0), 10.0, &RED), square("b", (50.0, 0.0), 10.0, &BLUE)];
    let (ids, said) = ordered_under(squares(), &Budget::DEFAULT, &Budget::DEFAULT, None);
    assert_eq!((ids, said.len()), (vec!["r".to_string(), "b".to_string()], 0));
    let (ids, _) = ordered_under(squares(), &Budget::DEFAULT, &Budget::DEFAULT, Some(BLUE.color));
    assert_eq!(ids, ["b", "r"]);
    // A green square with settings and a stop after it sews first, as a test sheet's half as drawn does. The
    // base layer sews after the stop, before the first element digitizing made: the blue still sews first.
    let stopped = || {
        let mut drawn = square("g", (100.0, 0.0), 10.0, &GREEN);
        drawn.automatic = false;
        drawn.element.params.set("stop_after", "true");
        let mut sewn = vec![drawn];
        sewn.extend(squares());
        sewn
    };
    let (ids, _) = ordered_under(stopped(), &Budget::DEFAULT, &Budget::DEFAULT, Some(BLUE.color));
    assert_eq!(ids, ["g", "b", "r"]);
    // With no base layer, the thread the half as drawn left is sewing, and the first in paint order next.
    let (ids, _) = ordered_under(stopped(), &Budget::DEFAULT, &Budget::DEFAULT, None);
    assert_eq!(ids, ["g", "r", "b"]);
    // A run with no element that digitizing made leaves the base layer's thread for the next: 2 squares
    // with settings keep their order, whatever is sewing.
    let set = |mut sewn: Sewn| {
        sewn.automatic = false;
        sewn
    };
    let (ids, said) = ordered_under(squares().into_iter().map(set).collect(), &Budget::DEFAULT, &Budget::DEFAULT, Some(BLUE.color));
    assert_eq!((ids, said.len()), (vec!["r".to_string(), "b".to_string()], 0));
}

#[test]
fn req_auto_037_a_satin_column_sews_after_the_fill_it_borders_where_that_costs_no_thread_change() {
    // The part of a column 2 mm wide along the right edge of a square 10 mm wide from `x`.
    let beside = |x: f64| [(x + 10.0, 0.0), (x + 10.0, 10.0), (x + 12.0, 10.0), (x + 12.0, 0.0), (x + 10.0, 0.0)];
    // A blue column drawn before the red square it borders sews after it.
    let pair = vec![column("bc", &beside(0.0), &BLUE), square("rs", (0.0, 0.0), 10.0, &RED)];
    let (ids, _) = ordered_with(pair, &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["rs", "bc"]);
    // A red square with a blue column, and apart from them a blue square with a red column: each column
    // after its square would sew a thread twice, so the red column sews with the red square, before the
    // blue square it borders, and the thread changes once.
    let columns = vec![
        square("rs", (0.0, 0.0), 10.0, &RED),
        column("bc", &beside(0.0), &BLUE),
        square("bs", (50.0, 0.0), 10.0, &BLUE),
        column("rc", &beside(50.0), &RED),
    ];
    let (ids, said) = ordered_with(columns, &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["rs", "rc", "bs", "bc"]);
    assert_eq!(messages(&said), ["info SC-I0907: The elements are sewn in an order of their own, with 1 thread change where paint order has 2."]);
    // Lines there go as the columns do (`REQ-AUTO-184`): the red line sews with the red square, and the thread
    // changes once. With `auto_thread_once` off they sew after their squares, as they are drawn, and the thread
    // changes twice (`REQ-AUTO-186`).
    let line = |id: &str, x: f64, thread: &Thread| running(id, &[(x + 11.0, 0.0), (x + 11.0, 10.0)], 2.0, thread);
    let lines = || vec![square("rs", (0.0, 0.0), 10.0, &RED), line("bl", 0.0, &BLUE), square("bs", (50.0, 0.0), 10.0, &BLUE), line("rl", 50.0, &RED)];
    let (ids, said) = ordered_with(lines(), &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!((ids, said.len()), (["rs", "rl", "bs", "bl"].map(String::from).to_vec(), 1));
    let (ids, said, _) = ordered_by(lines(), &Budget::DEFAULT, &Budget::DEFAULT, None, Rules { last: true, once: false });
    assert_eq!((ids, said.len()), (["rs", "bl", "bs", "rl"].map(String::from).to_vec(), 0));
}

#[test]
fn req_auto_037_a_satin_column_sews_after_a_parts_line_it_borders_where_that_costs_no_thread_change() {
    // A part's running line 1 mm wide along the left edge of a column's part 2 mm wide, each of a shape of its
    // own. Sewn first, the column would reach its overlap under the line's part, so it sews after the line,
    // here before the square of its thread, drawn first.
    let band = [(9.0, 0.0), (9.0, 10.0), (10.0, 10.0), (10.0, 0.0), (9.0, 0.0)];
    let part = Some(Polygon { outline: band.iter().map(|&(x, y)| p(x, y)).collect(), holes: Vec::new() });
    let line = |id: &str| Sewn { part: part.clone(), shape: 1, ..running(id, &[(9.5, 0.0), (9.5, 10.0)], 1.0, &RED) };
    let col = |id: &str| Sewn { shape: 2, ..column(id, &[(10.0, 0.0), (10.0, 10.0), (12.0, 10.0), (12.0, 0.0), (10.0, 0.0)], &BLUE) };
    let (ids, _) = ordered_with(vec![square("bs", (50.0, 0.0), 10.0, &BLUE), line("rl"), col("bc")], &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["rl", "bc", "bs"]);
    // Drawn the other way round, the column first, it still sews after the line.
    let (ids, _) = ordered_with(vec![col("bc"), line("rl"), square("rs", (50.0, 0.0), 10.0, &RED)], &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["rl", "rs", "bc"]);
    // A part's line and column of one shape keep their order, and wish nothing of each other.
    let one = |mut sewn: Sewn| {
        sewn.shape = 1;
        sewn
    };
    let (ids, _) = ordered_with(vec![one(col("bc")), one(line("rl")), square("rs", (50.0, 0.0), 10.0, &RED)], &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids, ["bc", "rl", "rs"]);
}

/// The order of the elements of `runs`, each sewn in the thread of `threads`, with `rules` of what sews
/// before what and each column of `wished` after its fill where that costs nothing, found with `budget`; and
/// the first element of the run that the search gave up at.
pub(super) fn sewn(
    threads: &[&Thread],
    runs: &[Vec<usize>],
    rules: &[(usize, usize)],
    wished: &[(usize, usize)],
    budget: &Budget,
) -> (Result<(), Exhausted>, Vec<usize>, Option<usize>) {
    let facts: Vec<Facts> = threads.iter().map(|thread| Facts { colours: vec![thread.color], stop: false, reach: None }).collect();
    let moving: Vec<Moving> = (0..facts.len()).map(|shape| Moving { free: true, made: true, shape }).collect();
    let mut graph = Graph::new(facts.len());
    for &(from, to) in rules {
        graph.add(from, to);
    }
    let mut order = Order {
        facts: &facts,
        moving: &moving,
        graph,
        borders: Vec::new(),
        wished: wished.to_vec(),
        last: true,
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept: BTreeSet::new(),
        unsought: None,
        under: None,
        once: true,
        overlaps: Vec::new(),
        found: Vec::new(),
        chosen: None,
    };
    let mut sewing = Sewing::default();
    let placed = order.sew(runs, &mut sewing, budget);
    (placed, sewing.order, order.unsought)
}

#[test]
fn req_auto_037_from_the_run_that_needs_more_work_on_the_columns_sew_after_their_fills() {
    // 2 runs, each a fill and a column of another thread that borders it, the second run's column drawn
    // first. A row of batches costs 1,000 units, and the search keeps 3 for the first run and one for the
    // second: with 3,500 units the first run is found, and the second isn't, so its column waits for its
    // fill, and the blue thread sews twice.
    let threads = [&RED, &BLUE, &BLUE, &RED];
    let runs = [vec![0, 1], vec![2, 3]];
    let wished = [(0, 1), (3, 2)];
    let with = |work: u64| sewn(&threads, &runs, &[], &wished, &Budget { max_work: work, ..Budget::DEFAULT });
    assert_eq!(with(3_500), (Ok(()), vec![0, 1, 3, 2], Some(2)));
    // With the work for both, the second run's column sews first, in the thread that is sewing, and before
    // its fill. With too little for the first run, both runs sew as before.
    assert_eq!(with(4_500), (Ok(()), vec![0, 1, 2, 3], None));
    assert_eq!(with(500), (Ok(()), vec![0, 1, 3, 2], Some(0)));
}

#[test]
fn req_auto_012_an_order_found_that_needs_more_work_to_place_leaves_the_rest_in_paint_order() {
    // 100 elements of one thread: finding their one batch takes far less work than placing them, which looks
    // at each free element each time, 5,050 units. With 2,000, the batch is found and 22 elements are placed.
    let red = RED;
    let threads = [&red; 100];
    let run: Vec<usize> = (0..100).collect();
    let (placed, order, unsought) = sewn(&threads, std::slice::from_ref(&run), &[], &[], &Budget { max_work: 2_000, ..Budget::DEFAULT });
    assert_eq!((placed.is_err(), order.len(), unsought), (true, 22, None));
    // Without work, the search fails at once, and so does finding whether the column, which sews before
    // another element, must sew before its fill.
    let (placed, order, unsought) = sewn(&[&RED, &BLUE, &RED], &[vec![0, 1, 2]], &[(1, 2)], &[(0, 1)], &Budget { max_work: 0, ..Budget::DEFAULT });
    assert_eq!((placed.is_err(), order, unsought), (true, vec![], Some(0)));
}

#[test]
fn req_auto_096_the_details_sewn_before_a_larger_area_round_them_are_counted() {
    // Details 2 and 3, each of area 0 and 1, and detail 4 of area 1: sewn 2, 0, 3, 1, 4, detail 2 sews
    // before area 0 and detail 3 before area 1, and 4 after it. Sewn 0, 4, 1, 3, 2, only detail 4 sews early.
    let details = [(0, 2), (1, 2), (1, 3), (1, 4)];
    let said = |order: &[usize]| early(&details, order).map(|said| said.message);
    assert_eq!(
        said(&[2, 0, 3, 1, 4]).as_deref(),
        Some(
            "2 of 3 details sew before a larger area round them: sewn after it, they would change thread more often, or break what \
             must sew before what."
        )
    );
    assert_eq!(
        said(&[0, 4, 1, 3, 2]).as_deref(),
        Some(
            "1 of 3 details sews before a larger area round it: sewn after it, it would change thread more often, or break what must \
             sew before what."
        )
    );
    assert_eq!(said(&[0, 1, 2, 3, 4]), None);
}

/// A green mark 4 mm square, drawn first, in a pinwheel of 4 squares 10 mm wide, red and blue, that close
/// in on it: each meets one of its sides.
fn pinwheel() -> Vec<Sewn> {
    vec![
        square("mark", (10.0, 10.0), 4.0, &GREEN),
        square("left", (0.0, 4.0), 10.0, &RED),
        square("bottom", (10.0, 0.0), 10.0, &BLUE),
        square("right", (14.0, 10.0), 10.0, &RED),
        square("top", (4.0, 14.0), 10.0, &BLUE),
    ]
}

#[test]
fn req_auto_096_closing_in_on_a_detail_is_metered_and_stops_where_the_work_runs_out() {
    // With the budget, the mark is a detail of each square and sews last, at no thread change more.
    let (ids, said) = ordered_with(pinwheel(), &Budget::DEFAULT, &Budget::DEFAULT);
    assert_eq!(ids.last().map(String::as_str), Some("mark"), "{ids:?}");
    assert!(!messages(&said).iter().any(|said| said.contains("work budget")), "{said:?}");
    // The squares close in on the mark, and the mark and the other squares on no square.
    let settings = DesignSettings::default();
    let found = || {
        let mut shapes = Shapes::new(pinwheel(), &settings, &Budget::DEFAULT);
        (0..shapes.len()).for_each(|index| shapes.find(index));
        // What the squares paint grown is found once, and kept.
        (1..shapes.len()).for_each(|index| drop(shapes.grown(index, &mut Budget::DEFAULT.meter())));
        shapes
    };
    let shapes = found();
    let facts: Vec<Facts> = (0..shapes.len()).map(|index| Facts::of(&shapes.elements[index], reach(&shapes, index))).collect();
    let mut meter = Budget::DEFAULT.meter();
    assert!(enclosed(&mut found(), &facts, 0, &mut meter).unwrap());
    assert!(!enclosed(&mut found(), &facts, 1, &mut Budget::DEFAULT.meter()).unwrap());
    // Short of the work it needs, finding it runs out and says so to the order, which keeps the mark in its
    // place: at each of the 5 shapes looked at, then as it finds the strip round the mark, and at the last
    // unit, as it finds what of the strip the squares cover. With what the squares paint grown not yet
    // found, it runs out finding that for the first.
    let work = Budget::DEFAULT.max_work - meter.work_left();
    let short = |shapes: &mut Shapes, work: u64| enclosed(shapes, &facts, 0, &mut Budget { max_work: work, ..Budget::DEFAULT }.meter()).is_err();
    for work in [0, 1, 2, 3, 4, 5, work - 1] {
        assert!(short(&mut found(), work), "{work}");
    }
    let mut painted = Shapes::new(pinwheel(), &settings, &Budget::DEFAULT);
    (0..painted.len()).for_each(|index| painted.find(index));
    assert!(short(&mut painted, 2));
    assert!(!short(&mut found(), work));
}

/// `shapes` of `sewn`, each one's paint found, the facts of each, and the work `enclosed` takes for shape
/// `index`, with what it finds.
fn closing(sewn: Vec<Sewn>, index: usize) -> (bool, u64) {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(sewn, &settings, &Budget::DEFAULT);
    (0..shapes.len()).for_each(|index| shapes.find(index));
    let facts: Vec<Facts> = (0..shapes.len()).map(|index| Facts::of(&shapes.elements[index], reach(&shapes, index))).collect();
    let mut meter = Budget::DEFAULT.meter();
    let closed = enclosed(&mut shapes, &facts, index, &mut meter).unwrap();
    (closed, Budget::DEFAULT.max_work - meter.work_left())
}

#[test]
fn req_auto_096_closing_in_grows_only_the_shapes_near_the_detail() {
    // A square 1 mm right of the mark's box is looked at, for a unit, and not grown: it lies farther than
    // twice the shapes' tolerance from the mark.
    let (closed, work) = closing(pinwheel(), 0);
    assert!(closed);
    let mut far = pinwheel();
    far.push(square("far", (15.0, 11.0), 1.0, &RED));
    assert_eq!(closing(far, 0), (true, work + 1));
}

#[test]
fn req_auto_096_a_detail_is_closed_in_and_smaller_than_the_area_it_meets() {
    // The pinwheel's mark, and a green square 4 mm wide beside a red one 10 mm wide, open all round.
    let mut beside = vec![square("small", (100.0, 0.0), 4.0, &GREEN), square("large", (90.0, 0.0), 10.0, &RED)];
    beside.extend(pinwheel());
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(beside, &settings, &Budget::DEFAULT);
    (0..shapes.len()).for_each(|index| shapes.find(index));
    let facts: Vec<Facts> = (0..shapes.len()).map(|index| Facts::of(&shapes.elements[index], reach(&shapes, index))).collect();
    let moving: Vec<Moving> = (0..shapes.len()).map(|shape| Moving { free: true, made: true, shape }).collect();
    let mut order = Order {
        facts: &facts,
        moving: &moving,
        graph: Graph::new(shapes.len()),
        borders: Vec::new(),
        wished: Vec::new(),
        last: true,
        details: Vec::new(),
        closed: BTreeMap::new(),
        kept: BTreeSet::new(),
        unsought: None,
        under: None,
        once: true,
        overlaps: Vec::new(),
        found: Vec::new(),
        chosen: None,
    };
    let mut detail = |detail: usize, other: usize| order.detail(&mut shapes, detail, other, &mut Budget::DEFAULT.meter()).unwrap();
    // The mark (2) is closed in and smaller than a square of the pinwheel (3), and no smaller than itself.
    assert!(detail(2, 3));
    assert!(!detail(2, 2));
    // The small square is smaller than the large one, and open: no detail.
    assert!(!detail(0, 1));
}

#[test]
fn req_auto_096_a_detail_paired_with_itself_sews_neither_before_nor_after_itself() {
    assert_eq!(early(&[(2, 2)], &[0, 1, 2]), None);
}
