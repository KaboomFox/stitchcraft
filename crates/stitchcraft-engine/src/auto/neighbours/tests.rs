//! The neighbours' parts the cases in `tests/auto_neighbours.rs` can't reach: what an area becomes whose
//! neighbours need more work than its budget allows, how much work shapes far from it cost it, how long an
//! edge a speck shares with an area, the area between a column's rails, and the bounds the shapes are found
//! near by.

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{Segment, Subpath};

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// The square from `(x0, y0)` to `(x1, y1)`, wound clockwise in y-up axes as a part is.
fn square((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Polygon {
    Polygon { outline: vec![p(x0, y0), p(x0, y1), p(x1, y1), p(x1, y0), p(x0, y0)], holes: Vec::new() }
}

/// A circle round `(cx, cy)` of radius `r` as a part of `n` points.
fn circle((cx, cy): (f64, f64), r: f64, n: u32) -> Polygon {
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
fn filled(id: &str, parts: &[Polygon]) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: outline(parts), rule: FillRule::NonZero },
        thread: Thread::new(Rgb::new(0, 0, 0)),
        params: ParamSet::default(),
        automatic: false,
    }
}

/// An automatic area `id`, the square from `(x0, y0)` to `(x1, y1)`.
fn area(id: &str, from: (f64, f64), to: (f64, f64)) -> Sewn {
    Sewn { element: filled(id, &[square(from, to)]), area: true }
}

/// The least work with which `step` says what it says with the default budget, found by halving: with a
/// unit less, it says something else.
fn least_work(step: impl Fn(&Budget) -> Vec<Diagnostic>) -> u64 {
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
fn reached(sewn: Vec<Sewn>, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(sewn, &settings, &Budget::DEFAULT);
    let said = under::reached(&mut shapes, budget);
    (shapes.into_elements(), said)
}

/// `sewn`'s specks merged with `budget`, and what is said of them.
fn merged(sewn: Vec<Sewn>, budget: &Budget) -> (Vec<Element>, Vec<Diagnostic>) {
    let settings = DesignSettings::default();
    let mut shapes = Shapes::new(sewn, &settings, &Budget::DEFAULT);
    let said = specks::merged(&mut shapes, budget);
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
fn an_area_costs_a_unit_for_each_shape_sewn_after_it_far_from_it() {
    let sewn = || vec![area("left", (0.0, 0.0), (10.0, 10.0)), area("right", (10.0, 0.0), (20.0, 10.0))];
    // A fill with settings far from them, sewn after them, whose region takes thousands of units to build.
    let with_far = || {
        let mut sewn = sewn();
        sewn.push(Sewn { element: filled("far", &[circle((100.0, 100.0), 10.0, 2_000)]), area: false });
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
        sewn.push(Sewn { element: filled("far", &[circle((100.0, 100.0), 10.0, 2_000)]), area: true });
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
    };
    // The square as it is sewn, and what is said of the column, each shape's paint found with `work`.
    let sewn = |work: u64| {
        let settings = DesignSettings::default();
        let sewn = vec![area("square", (0.0, 0.0), (10.0, 10.0)), Sewn { element: column(), area: false }];
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
            "Finding what this satin column covers needed more than the work budget of {low} units, so the areas sewn before it don't reach under it."
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
fn a_speck_rejoins_the_rest_of_its_area_or_is_left_out_when_it_shares_no_edge() {
    // A part 0.02 mm from the rest of its area, and a speck far from every area.
    let parts = Sewn { element: filled("parts", &[square((0.0, 0.0), (10.0, 10.0)), square((10.02, 0.0), (10.52, 1.0))]), area: true };
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
fn a_speck_that_shares_as_long_an_edge_with_2_areas_goes_into_the_one_sewn_first() {
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
    // Whether a rectangle `width` by `height`, the whole design, covers it.
    let covers = |width: f64, height: f64| {
        let path = outline(&[square((0.0, 0.0), (width, height))]);
        let bounds = stitchcraft_core::Rect::around([p(0.0, 0.0), p(width, height)]).unwrap();
        matches!(background::covers(&path, FillRule::NonZero, bounds, &mut Budget::DEFAULT.meter()), Ok(true))
    };
    // A little less than twice the margin across either way, the margins leave no box; a little more, they
    // leave one, which the rectangle covers.
    let twice = 2.0 * background::BACKGROUND_MARGIN;
    assert!(!covers(0.9 * twice, 10.0) && !covers(10.0, 0.9 * twice));
    assert!(covers(1.1 * twice, 10.0) && covers(10.0, 1.1 * twice));
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
