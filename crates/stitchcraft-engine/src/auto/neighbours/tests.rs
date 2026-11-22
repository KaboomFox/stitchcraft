//! The neighbours' parts the cases in `tests/auto_neighbours.rs` can't reach: what an area becomes whose
//! neighbours need more work than its budget allows, how much work shapes far from it cost it, how long an
//! edge a speck shares with an area, the area between a column's rails, and the bounds the shapes are found
//! near by. The layers' own tests (`layers::tests`) share its shapes and `least_work`.

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Code, Diagnostic, Mm};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{Cap, Join, Segment, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The square from `(x0, y0)` to `(x1, y1)`, wound clockwise in y-up axes as a part is.
pub(super) fn square((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Polygon {
    Polygon { outline: vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], holes: Vec::new() }
}

/// A circle round `(cx, cy)` of radius `r` as a part of `n` points.
pub(super) fn circle((cx, cy): (f64, f64), r: f64, n: u32) -> Polygon {
    let mut outline: Vec<Point> = (0..n)
        .map(|k| {
            let (sin, cos) = sin_cos(-std::f64::consts::TAU * f64::from(k) / f64::from(n));
            p(cx + r * cos, cy + r * sin)
        })
        .collect();
    outline.push(outline[0]);
    Polygon { outline, holes: Vec::new() }
}

/// A fill `id` of `parts`, without settings.
pub(super) fn filled(id: &str, parts: &[Polygon]) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: outline(parts), rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(Rgb::new(0, 0, 0)),
        params: ParamSet::default(),
        automatic: false,
        lettered: None,
        hints: Default::default(),
        clips: Vec::new(),
    }
}

/// An automatic area `id`, the square from `(x0, y0)` to `(x1, y1)`.
pub(super) fn area(id: &str, from: (f64, f64), to: (f64, f64)) -> Sewn {
    area_with(id, from, to, &[])
}

/// An automatic area `id`, the square from `(x0, y0)` to `(x1, y1)`, with the settings `params` of the rules.
pub(super) fn area_with(id: &str, from: (f64, f64), to: (f64, f64), params: &[(&str, &str)]) -> Sewn {
    let rules = AutoParams::from_set(&params.iter().copied().collect()).unwrap().params;
    Sewn { element: filled(id, &[square(from, to)]), area: Some(rules), automatic: true, reach: None, shape: 0, part: None }
}

/// The least work with which `step` says what it says with the default budget, found by halving: with a
/// unit less, it says something else.
pub(super) fn least_work(step: impl Fn(&Budget) -> Vec<Diagnostic>) -> u64 {
    let full = step(&Budget::DEFAULT);
    let enough = |work: u64| step(&Budget { max_work: work, ..Budget::DEFAULT }) == full;
    let (mut low, mut high) = (0, 1 << 24);
    assert!(enough(high), "the step costs less than {high} units");
    while high - low > 1 {
        let middle = (low + high) / 2;
        if enough(middle) { high = middle } else { low = middle }
    }
    high
}

/// `sewn` beside the shapes sewn after each area, each automatic area's region built with the default
/// budget and its neighbours found with `budget`, and what is said of them.
pub(super) fn reached(sewn: Vec<Sewn>, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(sewn, &settings, &Budget::DEFAULT);
    let said = under::reached(&mut shapes, budget);
    (shapes.into_elements(), said)
}

/// `sewn`'s specks merged with `budget`, and what is said of them.
fn merged(sewn: Vec<Sewn>, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    kept(sewn, &Marks::default(), budget)
}

/// `sewn`'s specks merged with `budget` but those `marks` keep, and what is said of them.
fn kept(sewn: Vec<Sewn>, marks: &Marks, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(sewn, &settings, &Budget::DEFAULT);
    let said = specks::merged(&mut shapes, marks, budget);
    (shapes.into_elements(), said)
}

#[test]
fn an_area_whose_neighbours_need_more_work_than_its_budget_allows_is_sewn_as_it_is_and_said_to_be() {
    let sewn = || vec![area("left", (0.0, 0.0), (10.0, 10.0)), area("right", (10.0, 0.0), (20.0, 10.0))];
    let least = least_work(|budget| reached(sewn(), budget).1);
    // With a unit less, `left` is named, and sewn as it was.
    let (elements, said) = reached(sewn(), &Budget { max_work: least - 1, ..Budget::DEFAULT });
    let [said] = said.as_slice() else { panic!("{said:?}") };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoUnder, Some("left")));
    assert_eq!(
        said.message,
        format!(
            "Finding where this area meets the shapes sewn after it needed more than the work budget of {} units, so it neither reaches \
             under them nor is cut away under them.",
            least - 1
        )
    );
    assert_eq!(
        elements.iter().map(|element| element.shape.clone()).collect::<Vec<_>>(),
        sewn().into_iter().map(|s| s.element.shape).collect::<Vec<_>>()
    );
}

#[test]
fn a_narrow_line_covered_whole_sews_nothing_unless_finding_that_needs_more_work_than_its_budget_allows() {
    // The running stitch along a strip 0.6 mm wide, and an area sewn after it. Covering the strip in part,
    // the area leaves the line whole. Covering it whole, it leaves nothing of it with the budget, and with a
    // unit less than finding that needs, the line sews whole and is named.
    let strip = square((2.0, 4.0), (8.0, 4.6));
    let middle = Subpath { start: p(2.3, 4.3), segments: vec![Segment::Line(p(7.7, 4.3))], closed: false };
    let shape = Shape::Stroke { path: Path { subpaths: vec![middle] }, width: Mm::new(0.6).unwrap(), join: Join::Round, cap: Cap::Round };
    let line = Element { shape, ..filled("line", &[]) };
    let sewn = |to: (f64, f64)| {
        let line = Sewn { element: line.clone(), area: None, automatic: true, reach: None, shape: 0, part: Some(strip.clone()) };
        vec![line, area("cover", (0.0, 0.0), to)]
    };
    let ids = |elements: &[Element]| elements.iter().map(|element| element.id.to_string()).collect::<Vec<_>>();
    let (elements, said) = reached(sewn((5.0, 10.0)), &Budget::DEFAULT);
    assert_eq!(ids(&elements), ["line", "cover"], "{said:?}");
    let (elements, said) = reached(sewn((10.0, 10.0)), &Budget::DEFAULT);
    assert_eq!(ids(&elements), ["cover"], "{said:?}");
    let least = least_work(|budget| reached(sewn((10.0, 10.0)), budget).1);
    let (elements, said) = reached(sewn((10.0, 10.0)), &Budget { max_work: least - 1, ..Budget::DEFAULT });
    let [said] = said.as_slice() else { panic!("{said:?}") };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoUnder, Some("line")));
    assert_eq!(
        said.message,
        format!(
            "Finding where this area meets the shapes sewn after it needed more than the work budget of {} units, so it is sewn whole.",
            least - 1
        )
    );
    assert_eq!(ids(&elements), ["line", "cover"]);
}

#[test]
fn an_area_covered_whole_by_an_area_sewn_after_it_sews_nothing_and_is_said_to() {
    // As the cases of `tests/auto_neighbours.rs` find it digitized: here beside its neighbours alone.
    let (elements, said) = reached(vec![area("hidden", (2.0, 2.0), (5.0, 5.0)), area("cover", (0.0, 0.0), (10.0, 10.0))], &Budget::DEFAULT);
    let [said] = said.as_slice() else { panic!("{said:?}") };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoUnder, Some("hidden")));
    assert_eq!(said.message, "This area is covered by the areas sewn after it, so it sews nothing.");
    let ids: Vec<&str> = elements.iter().map(|element| element.id.as_str()).collect();
    assert_eq!(ids, ["cover"]);
}

#[test]
fn an_area_costs_a_unit_for_each_shape_sewn_after_it_far_from_it() {
    let sewn = || vec![area("left", (0.0, 0.0), (10.0, 10.0)), area("right", (10.0, 0.0), (20.0, 10.0))];
    // A fill with settings far from them, sewn after them, whose region takes thousands of units to build.
    let with_far = || {
        let mut sewn = sewn();
        sewn.push(Sewn {
            element: filled("far", &[circle((100.0, 100.0), 10.0, 2_000)]),
            area: None,
            automatic: false,
            reach: None,
            shape: 0,
            part: None,
        });
        sewn
    };
    let (alone, far) = (least_work(|budget| reached(sewn(), budget).1), least_work(|budget| reached(with_far(), budget).1));
    assert_eq!(far, alone + 1, "a look at the far fill's bounds");
}

#[test]
fn a_speck_costs_a_unit_for_each_area_far_from_it() {
    let sewn = || vec![area("big", (0.0, 0.0), (10.0, 10.0)), area("speck", (10.0, 0.0), (10.5, 0.5))];
    let with_far = || {
        let mut sewn = sewn();
        sewn.push(Sewn {
            element: filled("far", &[circle((100.0, 100.0), 10.0, 2_000)]),
            area: Some(AutoParams::defaults()),
            automatic: true,
            reach: None,
            shape: 0,
            part: None,
        });
        sewn
    };
    let (alone, far) = (least_work(|budget| merged(sewn(), budget).1), least_work(|budget| merged(with_far(), budget).1));
    assert_eq!(far, alone + 1, "a look at the far area's bounds");
}

#[test]
fn a_column_whose_area_needs_more_work_than_its_budget_allows_is_said_and_reached_under_by_none() {
    // A square, and after it a satin column with settings along its top edge, its rails of 201 points.
    let rail = |y: f64| {
        let mut points = (0..=200).map(|k| p(f64::from(k) * 0.05, y));
        let start = points.next().unwrap();
        Subpath { start, segments: points.map(Segment::Line).collect(), closed: false }
    };
    let column = || Element {
        id: ElementId::new("column").unwrap(),
        name: None,
        shape: Shape::stroke(Path { subpaths: vec![rail(10.0), rail(13.0)] }),
        thread: Thread::new(Rgb::new(0, 0, 0)),
        params: [("satin_column", "true")].into_iter().collect(),
        automatic: false,
        lettered: None,
        hints: Default::default(),
        clips: Vec::new(),
    };
    // The square as it is sewn, and what is said of the column, each shape's paint found with `work`.
    let sewn = |work: u64| {
        let settings = DesignSettings::default();
        let sewn = vec![
            area("square", (0.0, 0.0), (10.0, 10.0)),
            Sewn { element: column(), area: None, automatic: false, reach: None, shape: 0, part: None },
        ];
        let mut shapes = Shapes::new(sewn, &settings, &Budget { max_work: work, ..Budget::DEFAULT });
        let _ = under::reached(&mut shapes, &Budget::DEFAULT);
        let said = std::mem::take(&mut shapes.said);
        (shapes.into_elements().remove(0).shape, said)
    };
    let drawn = filled("square", &[square((0.0, 0.0), (10.0, 10.0))]).shape;
    // The least work with which the square reaches under the column.
    let (mut low, mut high) = (0, 1 << 24);
    assert_ne!(sewn(high).0, drawn);
    while high - low > 1 {
        let middle = (low + high) / 2;
        if sewn(middle).0 == drawn { low = middle } else { high = middle }
    }
    // With a unit less, what the column covers is not found, and that is said.
    let (shape, said) = sewn(low);
    assert_eq!(shape, drawn);
    let [said] = said.as_slice() else { panic!("{said:?}") };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoUnder, Some("column")));
    assert_eq!(
        said.message,
        format!(
            "Finding what this satin column covers needed more than the work budget of {low} units, so the areas sewn before it don't reach under \
             it, and it keeps its place in paint order beside the shapes near it."
        )
    );
}

#[test]
fn a_speck_whose_neighbours_need_more_work_than_its_budget_allows_is_left_out_and_said_to_be() {
    let sewn = || vec![area("big", (0.0, 0.0), (10.0, 10.0)), area("speck", (10.0, 0.0), (10.5, 0.5))];
    let (elements, said) = merged(sewn(), &Budget { max_work: 10, ..Budget::DEFAULT });
    let [said] = said.as_slice() else { panic!("{said:?}") };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoSpeck, Some("speck")));
    assert_eq!(said.message, "Finding where to merge this speck needed more than the work budget of 10 units, so it is left out.");
    assert_eq!(elements.iter().map(|element| element.id.as_str()).collect::<Vec<_>>(), ["big"]);
}

#[test]
fn req_auto_104_a_speck_a_mark_holds_stays_and_one_too_costly_to_test_merges_and_is_said_to() {
    let sewn = || vec![area("big", (0.0, 0.0), (10.0, 10.0)), area("speck", (10.0, 0.0), (10.5, 0.5))];
    let ids = |elements: &[Element]| elements.iter().map(|element| element.id.to_string()).collect::<Vec<_>>();
    let near = |n| Marks::of(&[crate::auto::marks::tests::edged("eye", [9.9, -0.1, 10.6, 0.6], n)], &Budget::DEFAULT);
    let (elements, said) = kept(sewn(), &near(1), &Budget::DEFAULT);
    assert_eq!((ids(&elements), said.len()), (vec!["big".to_string(), "speck".to_string()], 0), "{said:?}");
    // Its 5 points against a mark of 4,000 segments cost more than 1,000 units.
    let (elements, said) = kept(sewn(), &near(1_000), &Budget { max_work: 1_000, ..Budget::DEFAULT });
    assert_eq!(ids(&elements), ["big"]);
    assert_eq!((said[0].code, said[0].element.as_ref().map(ElementId::as_str)), (Code::AutoKept, Some("speck")));
    assert_eq!(
        said[0].message,
        "Finding whether a mark holds this speck needed more than the work budget of 1000 units, so it merges as if there were no mark."
    );
    assert_eq!(said[1].code, Code::AutoSpeck);
}

#[test]
fn a_speck_rejoins_the_rest_of_its_area_or_is_left_out_when_it_shares_no_edge() {
    // A part 0.02 mm from the rest of its area, and a speck far from every area.
    let parts = Sewn {
        element: filled("parts", &[square((0.0, 0.0), (10.0, 10.0)), square((10.02, 0.0), (10.52, 1.0))]),
        area: Some(AutoParams::defaults()),
        automatic: true,
        reach: None,
        shape: 0,
        part: None,
    };
    let (_, said) = merged(vec![parts, area("lone", (30.0, 30.0), (30.5, 30.5))], &Budget::DEFAULT);
    assert_eq!(
        said.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(),
        [
            "A part of this area, of 0.5 mm², is too small to sew, so it is joined to the rest of this area, which it shares the longest edge with.",
            "This area of 0.25 mm² is too small to sew and shares no edge with an automatic area, so it is left out.",
        ]
    );
}

#[test]
fn a_speck_that_shares_as_long_an_edge_with_2_areas_goes_into_the_one_drawn_first() {
    // `a` and `b` are drawn alike, one on the other, and the speck lies beside both.
    let sewn = vec![area("a", (0.0, 0.0), (10.0, 10.0)), area("b", (0.0, 0.0), (10.0, 10.0)), area("speck", (10.0, 0.0), (10.5, 0.5))];
    let (_, said) = merged(sewn, &Budget::DEFAULT);
    assert_eq!(
        said.iter().map(|d| d.message.as_str()).collect::<Vec<_>>(),
        ["This area of 0.25 mm² is too small to sew, so it is merged into `a`, which it shares the longest edge with."]
    );
}

#[test]
fn a_speck_shares_the_stretch_of_an_areas_rings_that_lies_within_its_zone() {
    let mut meter = Budget::DEFAULT.meter();
    // From (1, 1) to (7, 4), a segment crosses the one from (2, 5) to (6, -1) 22/48 of the way along; it
    // stops short of a shorter one, and runs alongside one beside it.
    assert_eq!(specks::crossing(p(1.0, 1.0), p(7.0, 4.0), p(2.0, 5.0), p(6.0, -1.0)), Some(22.0 / 48.0));
    assert_eq!(specks::crossing(p(1.0, 1.0), p(7.0, 4.0), p(2.0, 5.0), p(3.0, 3.5)), None);
    assert_eq!(specks::crossing(p(1.0, 1.0), p(7.0, 4.0), p(1.0, 2.0), p(7.0, 5.0)), None);
    // From (1, 2) to (11, 7), a segment runs within the band from x = 3 to 6 for 3 tenths of its length.
    let band = [square((3.0, 0.0), (6.0, 10.0))];
    let length = stitchcraft_core::math::hypot(10.0, 5.0);
    let inside = specks::within(p(1.0, 2.0), p(11.0, 7.0), &band, &mut meter).ok().unwrap();
    assert!((inside - 0.3 * length).abs() < 1e-9, "{inside} against {}", 0.3 * length);
    // The square's right side runs within the zone from y = 4 to 6: 2 mm of its rings.
    let zone = [square((9.0, 4.0), (11.0, 6.0))];
    let shared = specks::shared(&[square((0.0, 0.0), (10.0, 10.0))], &zone, &envelope(&zone).unwrap(), &mut meter).ok().unwrap();
    assert!((shared - 2.0).abs() < 1e-9, "{shared}");
}

#[test]
fn a_design_less_than_twice_the_margin_across_has_no_background() {
    // Whether a rectangle `width` by `height`, the whole design, covers it under a dot drawn on it.
    let covers = |width: f64, height: f64| {
        let path = outline(&[square((0.0, 0.0), (width, height))]);
        let dot = filled("dot", &[square((0.0, 0.0), (0.1, 0.1))]);
        let bounds = stitchcraft_core::Rect::around([p(0.0, 0.0), p(width, height)]).unwrap();
        matches!(background::covers(&path, FillRule::NonZero, &[dot], bounds, &mut Budget::DEFAULT.meter()), Ok(true))
    };
    // Up to twice the margin across either way, the margins leave no box; a little more, they leave one,
    // which the rectangle covers.
    let twice = 2.0 * background::BACKGROUND_MARGIN;
    assert!(!covers(0.9 * twice, 10.0) && !covers(10.0, 0.9 * twice) && !covers(twice, 10.0) && !covers(10.0, twice));
    assert!(covers(1.1 * twice, 10.0) && covers(10.0, 1.1 * twice));
}

#[test]
fn a_sliver_beside_a_side_runs_along_it_further_than_along_the_background_and_has_no_area() {
    // The sliver a background whose left side leans 40 nm over 20 mm across the margin's edge leaves inside
    // the margin (`req_auto_009_a_background_whose_side_leans_across_the_margins_edge_is_left_out`): were it
    // measured as a notch, it would make the background part of the picture.
    let lean = 4e-8;
    let sliver = Polygon {
        outline: [(0.5, 0.5), (0.5, 19.5), (0.5 + 0.975 * lean, 19.5), (0.5 + 0.025 * lean, 0.5), (0.5, 0.5)].map(|(x, y)| p(x, y)).to_vec(),
        holes: Vec::new(),
    };
    let sides = Envelope { min_x: 0.5, min_y: 0.5, max_x: 29.5, max_y: 19.5 };
    let (along_sides, along_outlines) = background::lengths(&sliver, &sides, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(along_sides > along_outlines && no_area(sliver.area()), "{along_sides} {along_outlines} {}", sliver.area());
}

#[test]
fn a_part_the_background_leaves_runs_along_the_sides_its_edges_lie_on_and_along_the_outlines_elsewhere() {
    let sides = Envelope { min_x: 0.0, min_y: 0.0, max_x: 30.0, max_y: 20.0 };
    let lengths = |part: &Polygon| background::lengths(part, &sides, &mut Budget::DEFAULT.meter()).unwrap();
    let triangle = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| Polygon {
        outline: vec![p(a.0, a.1), p(b.0, b.1), p(c.0, c.1), p(a.0, a.1)],
        holes: Vec::new(),
    };
    // In each corner, 3 and 4 mm along the 2 sides it lies on, and the 5 mm between them along the outlines.
    for corner in [
        triangle((0.0, 0.0), (0.0, 4.0), (3.0, 0.0)),
        triangle((30.0, 0.0), (27.0, 0.0), (30.0, 4.0)),
        triangle((30.0, 20.0), (30.0, 16.0), (27.0, 20.0)),
        triangle((0.0, 20.0), (3.0, 20.0), (0.0, 16.0)),
    ] {
        let (along_sides, along_outlines) = lengths(&corner);
        assert!((along_sides - 7.0).abs() < 1e-9 && (along_outlines - 5.0).abs() < 1e-9, "{corner:?}: {along_sides} {along_outlines}");
    }
    // A square in the middle of each side, 2 mm along it and 6 along the outlines, and 4 more with a hole
    // 1 mm across in its middle.
    for (from, to) in [((0.0, 9.0), (2.0, 11.0)), ((28.0, 9.0), (30.0, 11.0)), ((14.0, 0.0), (16.0, 2.0)), ((14.0, 18.0), (16.0, 20.0))] {
        let (along_sides, along_outlines) = lengths(&square(from, to));
        assert!((along_sides - 2.0).abs() < 1e-9 && (along_outlines - 6.0).abs() < 1e-9, "{from:?}: {along_sides} {along_outlines}");
        let middle = ((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0);
        let hole = square((middle.0 - 0.5, middle.1 - 0.5), (middle.0 + 0.5, middle.1 + 0.5)).outline;
        let (_, around_hole) = lengths(&Polygon { holes: vec![hole], ..square(from, to) });
        assert!((around_hole - 10.0).abs() < 1e-9, "{from:?}: {around_hole}");
    }
}

#[test]
fn comparing_shapes_with_the_areas_path_costs_a_unit_for_each_of_its_subpaths_and_segments() {
    // 2 closed subpaths of 3 segments each: 8 units for each shape compared with the path.
    let parts = [square((0.0, 0.0), (10.0, 10.0)), square((20.0, 0.0), (30.0, 10.0))];
    let own = filled("own", &parts);
    let drawn = [own.clone(), own.clone(), filled("other", &[square((2.0, 2.0), (4.0, 4.0))])];
    let others =
        |work: u64, drawn: &[Element]| background::others(own.shape.path(), drawn, &mut Budget { max_work: work, ..Budget::DEFAULT }.meter());
    assert_eq!(others(16, &drawn[..2]), Ok(false));
    assert!(others(15, &drawn[..2]).is_err());
    assert_eq!(others(24, &drawn), Ok(true));
    assert!(others(23, &drawn).is_err());
}

#[test]
fn a_regions_edges_not_joined_are_said_as_a_bug() {
    let said = Failed::Unjoined.said(
        Code::AutoUnder,
        "where this area meets the shapes sewn after it",
        "it is sewn as drawn",
        7,
        ElementId::new("a").unwrap(),
    );
    assert_eq!(said.code, Code::InternalCheckFailed);
    assert_eq!(
        said.message,
        "Finding where this area meets the shapes sewn after it, the edges of a region could not be joined into rings, so it is sewn as \
         drawn. This is a bug in StitchCraft."
    );
}

#[test]
fn a_columns_area_lies_between_its_rails_as_they_are_sewn() {
    let mut meter = Budget::DEFAULT.meter();
    // Open rails, running the same way, 3 mm apart and then opening out to 5.
    let open = [vec![p(0.0, 0.0), p(5.0, 0.0), p(10.0, 0.0)], vec![p(0.0, 3.0), p(5.0, 3.0), p(10.0, 5.0)]];
    let parts = between(&open, &mut meter).unwrap().unwrap();
    assert!((ops::area_of(&parts) - (15.0 + 20.0)).abs() < 1e-9, "{}", ops::area_of(&parts));
    // Closed rails round a ring, running the same way: the ring between them.
    let ring = |x0: f64, x1: f64| vec![p(x0, x0), p(x1, x0), p(x1, x1), p(x0, x1), p(x0, x0)];
    let parts = between(&[ring(0.0, 10.0), ring(2.0, 8.0)], &mut meter).unwrap().unwrap();
    assert_eq!((parts.len(), parts[0].holes.len()), (1, 1));
    assert!((ops::area_of(&parts) - 64.0).abs() < 1e-9, "{}", ops::area_of(&parts));
}

#[test]
fn bounds_are_near_within_the_distance_given() {
    let a = Envelope::of(&[p(0.0, 0.0), p(10.0, 10.0)]);
    // Beside, above: each way round, 0.4 mm apart.
    for b in [Envelope::of(&[p(10.4, 0.0), p(20.0, 10.0)]), Envelope::of(&[p(0.0, 10.4), p(10.0, 20.0)])] {
        assert!(near(&a, 0.4, &b) && !near(&a, 0.39, &b) && near(&b, 0.4, &a) && !near(&b, 0.39, &a), "{b:?}");
    }
    assert!(envelope(&[]).is_none());
    assert_eq!(envelope(&[square((1.0, 2.0), (3.0, 4.0))]), Some(Envelope::of(&[p(1.0, 2.0), p(3.0, 4.0)])));
}

#[test]
fn a_speck_is_smaller_than_the_smallest_area() {
    // One exactly as large is sewn, and one the least bit smaller is a speck.
    let smallest = AutoParams::defaults().auto_min_area_mm2;
    assert!(!speck(smallest, smallest) && speck(smallest.next_down(), smallest) && !speck(smallest.next_up(), smallest));
}

#[test]
fn a_column_whose_neighbours_need_more_work_than_the_budget_keeps_its_pull_compensation_and_says_so() {
    // An automatic satin column 3 mm wide, its rails of 21 points, and a square sewn after it along its
    // second rail.
    let rail = |y: f64| {
        let mut points = (0..=20).map(|k| p(f64::from(k) * 0.5, y));
        let start = points.next().unwrap();
        Subpath { start, segments: points.map(Segment::Line).collect(), closed: false }
    };
    let column = Element {
        id: ElementId::new("column").unwrap(),
        shape: Shape::stroke(Path { subpaths: vec![rail(10.0), rail(13.0)] }),
        params: [("satin_column", "true"), ("pull_compensation_mm", "0.4")].into_iter().collect(),
        ..filled("column", &[])
    };
    // The column's pull compensation and what is said, its neighbours found with `work`.
    let reached = |work: u64| {
        let column =
            Sewn { element: column.clone(), area: None, automatic: true, reach: Some(Past { overlap: 1.0, column: 0.4 }), shape: 0, part: None };
        let settings = DesignSettings::default();
        let mut shapes = Shapes::new(vec![column, area("square", (0.0, 13.0), (10.0, 23.0))], &settings, &Budget::DEFAULT);
        let said = under::reached(&mut shapes, &Budget { max_work: work, ..Budget::DEFAULT });
        (shapes.into_elements().remove(0).params.get("pull_compensation_mm").map(str::to_string), said)
    };
    // With the default budget it sews 1 mm past the rail the square runs along, and 0.4 mm past the other.
    let (pull, said) = reached(Budget::DEFAULT.max_work);
    assert!(matches!(pull.as_deref(), Some("0.4 1" | "1 0.4")), "{pull:?}");
    let [counted] = said.as_slice() else { panic!("{said:?}") };
    assert_eq!(counted.to_string(), "info SC-I0904: 1 satin column sews 1 mm past a rail, under the shape sewn after it there.");
    // With any less than that takes, it keeps its pull compensation, and that is said of it.
    let least = least_work(|budget| reached(budget.max_work).1);
    assert!(least > 20, "{least}");
    for work in 0..least {
        let (pull, said) = reached(work);
        assert_eq!(pull.as_deref(), Some("0.4"), "{work}");
        let [said] = said.as_slice() else { panic!("{work}: {said:?}") };
        assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoUnder, Some("column")), "{work}");
        assert!(said.message.ends_with("so it sews its pull compensation past both rails."), "{work}: {}", said.message);
    }
}

/// A satin column 3 mm wide with settings of its own, its rails from x = 2 to 8 at y = 4 and y = 7, as
/// automatic digitizing makes one with an overlap of 1 mm: of the part `part` of its shape, or a line's.
fn column_made(part: Option<Polygon>) -> Sewn {
    let rail = |y: f64| Subpath { start: p(2.0, y), segments: vec![Segment::Line(p(8.0, y))], closed: false };
    let shape = Shape::stroke(Path { subpaths: vec![rail(4.0), rail(7.0)] });
    let element = Element { shape, params: [("satin_column", "true")].into_iter().collect(), ..filled("column", &[]) };
    Sewn { element, area: None, automatic: true, reach: Some(Past { overlap: 1.0, column: 0.4 }), shape: 0, part }
}

/// What `said` says of columns that sew past a rail.
fn past_a_rail(said: &[Diagnostic]) -> Vec<&str> {
    said.iter().map(|said| said.message.as_str()).filter(|message| message.contains("past a rail")).collect()
}

#[test]
fn a_parts_column_covered_whole_sews_nothing_and_is_not_said_to_sew_past_a_rail() {
    let strip = square((2.0, 4.0), (8.0, 7.0));
    let sewn = |to: (f64, f64)| vec![column_made(Some(strip.clone())), area("cover", (0.0, 0.0), to)];
    let ids = |elements: &[Element]| elements.iter().map(|element| element.id.to_string()).collect::<Vec<_>>();
    // An area sewn after the column covers its rail at y = 4 and not the one at y = 7: the column sews, and
    // 1 mm past the rail the area runs along.
    let (elements, said) = reached(sewn((10.0, 5.0)), &Budget::DEFAULT);
    assert_eq!(ids(&elements), ["column", "cover"], "{said:?}");
    assert_eq!(past_a_rail(&said), ["1 satin column sews 1 mm past a rail, under the shape sewn after it there."]);
    // The area covers the column whole: it is left out, and no rail of it is counted.
    let (elements, said) = reached(sewn((10.0, 10.0)), &Budget::DEFAULT);
    assert_eq!(ids(&elements), ["cover"], "{said:?}");
    assert_eq!(past_a_rail(&said), Vec::<&str>::new());
}

#[test]
fn a_line_sewn_after_a_column_along_its_rail_is_no_shape_beside_it() {
    // A line 2 mm wide along the column's rail at y = 7, sewn after the column: it goes over the column, so
    // the column keeps its pull compensation past that rail.
    let along = Subpath { start: p(2.0, 7.0), segments: vec![Segment::Line(p(8.0, 7.0))], closed: false };
    let shape = Shape::Stroke { path: Path { subpaths: vec![along] }, width: Mm::new(2.0).unwrap(), join: Join::Round, cap: Cap::Round };
    let line = Sewn { element: Element { shape, ..filled("line", &[]) }, area: None, automatic: true, reach: None, shape: 1, part: None };
    let (elements, said) = reached(vec![column_made(None), line], &Budget::DEFAULT);
    assert_eq!(past_a_rail(&said), Vec::<&str>::new());
    assert_eq!(elements[0].params.get("pull_compensation_mm"), None, "{:?}", elements[0].params);
    // An area there is one: the column sews 1 mm past the rail it runs along.
    let (_, said) = reached(vec![column_made(None), area("beside", (2.0, 7.0), (8.0, 12.0))], &Budget::DEFAULT);
    assert_eq!(past_a_rail(&said), ["1 satin column sews 1 mm past a rail, under the shape sewn after it there."]);
}

#[test]
fn a_rail_is_run_along_where_shapes_cover_more_than_half_of_its_length() {
    let meter = || Budget::DEFAULT.meter();
    let rail = [p(0.0, 0.0), p(0.0, 30.0)];
    let band = |from: f64, to: f64| square((-1.0, from), (1.0, to));
    // The rail is looked at every 0.5 mm, from 0.25 mm on. 2 shapes away from its middle cover 18 and 16 of
    // those 60 places, 17 mm of its 30 mm.
    assert!(under::runs_along(&rail, &[band(0.0, 8.9), band(19.1, 26.9)], &mut meter()).unwrap());
    // 18 and 12 places are 15 mm, half the rail and not more.
    assert!(!under::runs_along(&rail, &[band(0.0, 8.9), band(19.1, 25.1)], &mut meter()).unwrap());
    // A shape whose bounds hold the whole rail covers only what it covers: an L whose foot crosses the rail's
    // first millimetre, and whose upright stands beside it, 1 mm away.
    let corner = [p(-2.0, 0.0), p(-2.0, 30.0), p(-1.0, 30.0), p(-1.0, 1.0), p(2.0, 1.0), p(2.0, 0.0), p(-2.0, 0.0)];
    assert!(!under::runs_along(&rail, &[Polygon { outline: corner.to_vec(), holes: Vec::new() }], &mut meter()).unwrap());
    assert!(!under::runs_along(&[p(0.0, 0.0), p(0.0, 2.0)], &[band(-1.0, 1.0)], &mut meter()).unwrap());
    // No shape runs along a rail with none beside it, and none along a rail of no length.
    assert!(!under::runs_along(&rail, &[], &mut meter()).unwrap());
    assert!(!under::runs_along(&[p(0.0, 0.0), p(0.0, 0.0)], &[band(-1.0, 1.0)], &mut meter()).unwrap());
    // A rail 1 mm long is looked at in 2 places, each for a unit of work and one for each shape, after a unit
    // for each shape's bounds.
    let least = |shapes: &[Polygon]| {
        let enough =
            |work: &u64| under::runs_along(&[p(0.0, 0.0), p(0.0, 1.0)], shapes, &mut Budget { max_work: *work, ..Budget::DEFAULT }.meter()).is_ok();
        (0..20).find(enough)
    };
    assert_eq!((least(&[]), least(&[band(0.0, 1.0)]), least(&[band(0.0, 1.0), band(5.0, 6.0)])), (Some(2), Some(5), Some(8)));
    // A shape is asked only about a place within its bounds: of the 2 places, 0.25 and 0.75 mm along, a band
    // that ends between them is asked about the first. Its 5 points are looked at for its bounds, and once more.
    geom::looked_at::take();
    assert!(!under::runs_along(&[p(0.0, 0.0), p(0.0, 1.0)], &[band(0.0, 0.4)], &mut meter()).unwrap());
    assert_eq!(geom::looked_at::take(), 5 + 5);
}

#[test]
fn req_auto_029_a_column_sews_the_overlap_past_a_rail_and_keeps_the_other_rails_own_pull_compensation() {
    let settings = DesignSettings::default();
    let past = |pull: Option<&str>, under: [bool; 2]| {
        let mut column = column_made(None);
        if let Some(pull) = pull {
            column.element.params.set(PULL_COMPENSATION, pull);
        }
        let mut shapes = Shapes::new(vec![column], &settings, &Budget::DEFAULT);
        shapes.reach_past(0, under.map(|under| under.then_some(1.5)));
        shapes.elements[0].params.get(PULL_COMPENSATION).map(str::to_string)
    };
    let past = |pull: Option<&str>, under: [bool; 2]| past(pull, under).unwrap();
    // One length is each rail's, and a column with none has 0 on each.
    assert_eq!(past(Some("0.4"), [true, false]), "1.5 0.4");
    assert_eq!(past(None, [false, true]), "0 1.5");
    // A length for each rail: the rail it sews past takes the overlap, and the other keeps its own.
    assert_eq!(past(Some("0 0.4"), [true, false]), "1.5 0.4");
    assert_eq!(past(Some("0 0.4"), [false, true]), "0 1.5");
    assert_eq!(past(Some("0.2 0.4"), [true, true]), "1.5 1.5");
    // A pull compensation that doesn't read, or out of its range, stays as it is, for planning to say so.
    assert_eq!(past(Some("0 0.4 1"), [true, false]), "0 0.4 1");
    assert_eq!(past(Some("20"), [true, false]), "20");
}

#[test]
fn req_auto_167_each_rail_sews_past_by_its_own_reach() {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(vec![column_made(None)], &settings, &Budget::DEFAULT);
    shapes.reach_past(0, [Some(0.4), Some(1.0)]);
    assert_eq!(shapes.elements[0].params.get(PULL_COMPENSATION), Some("0.4 1"));
}

#[test]
fn a_columns_rails_are_found_once_for_what_it_paints_and_where_it_sews_past_a_rail() {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(vec![column_made(None)], &settings, &Budget::DEFAULT);
    let rails = |shapes: &mut Shapes| {
        let mut meter = Budget::DEFAULT.meter();
        let rails = shapes.rails(0, &mut meter).unwrap();
        (rails, Budget::DEFAULT.max_work - meter.work_left())
    };
    let (first, work) = rails(&mut shapes);
    assert!(first.is_some() && work > 0, "{first:?}, {work}");
    // Asked for again, they are the same for no work, also once the column sews past a rail.
    shapes.reach_past(0, [Some(1.0), None]);
    assert_eq!(rails(&mut shapes), (first.clone(), 0));
    assert_eq!(rails_of(&shapes.elements[0], shapes.stitch_types[0].unwrap(), &mut Budget::DEFAULT.meter()).unwrap(), first);
    // What the column paints lies between them: with its rails known it is found with less work, the same.
    let painted = |work: u64, known: bool| {
        let mut shapes = Shapes::new(vec![column_made(None)], &settings, &Budget { max_work: work, ..Budget::DEFAULT });
        if known {
            shapes.rails(0, &mut Budget::DEFAULT.meter()).unwrap();
        }
        shapes.find(0);
        shapes.paint(0).map(|(_, parts)| parts.to_vec())
    };
    let least = |known: bool| (0..Budget::DEFAULT.max_work).find(|&work| painted(work, known).is_some()).unwrap();
    assert!(least(true) < least(false), "{} and {}", least(true), least(false));
    assert_eq!(painted(least(true), true), painted(Budget::DEFAULT.max_work, false));
    // An element with no rails has none, found once too.
    let mut shapes = Shapes::new(vec![area("square", (0.0, 0.0), (10.0, 10.0))], &settings, &Budget::DEFAULT);
    assert_eq!(rails(&mut shapes), (None, 0));
}
