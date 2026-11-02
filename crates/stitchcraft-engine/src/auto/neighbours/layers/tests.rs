use stitchcraft_core::math::sin_cos;

use super::*;
use crate::auto::neighbours::tests::{circle, filled, least_work, square};
use crate::design::DesignSettings;
use crate::normalize::fixture::part;

#[test]
fn the_summary_counts_the_areas_flattened_and_those_given_whole() {
    assert!(summary(0, 0).is_none());
    let said = |flattened, whole| summary(flattened, whole).map(|d| d.to_string()).unwrap_or_default();
    assert_eq!(said(1, 0), "info SC-I0908: 1 area sews only what shows of it under the areas drawn over it.");
    assert_eq!(
        said(2, 3),
        "info SC-I0908: 2 areas sew only what shows of them under the areas drawn over them. 3 areas are digitized whole under the areas \
         that fill their holes, which sew after them."
    );
    assert_eq!(said(0, 1), "info SC-I0908: 1 area is digitized whole under the areas that fill its holes, which sew after it.");
}

/// An automatic area's layer whose region as drawn is `parts`, with the default settings of the rules.
fn layer_of(parts: Vec<Polygon>) -> Layer {
    let bounds = envelope(&parts).unwrap();
    Layer { rules: AutoParams::defaults(), shown: parts.clone(), drawn: parts, bounds, closed: Vec::new(), unfound: false }
}

#[test]
fn an_area_whose_layers_were_not_found_fills_no_hole() {
    // An area under a square with a hole, its flattening out of work: it shows as drawn, under the whole
    // square, and is no detail of it, which would move the square before it and sew it whole under it.
    let under = Layer { unfound: true, ..layer_of(vec![part(&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)], &[])]) };
    let square =
        layer_of(vec![part(&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)], &[&[(10.0, 10.0), (10.0, 20.0), (20.0, 20.0), (20.0, 10.0)]])]);
    let layers = [Some(under), Some(square)];
    let holes = details(&layers, &boxes(&layers), 1, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(holes.is_empty(), "{:?}", holes.iter().map(|hole| &hole.details).collect::<Vec<_>>());
}

/// The automatic area `id` of `parts`.
fn automatic(id: &str, parts: &[Polygon]) -> Element {
    Element { automatic: true, ..filled(id, parts) }
}

/// The square from (18, 18) to (22, 22).
fn small() -> Polygon {
    square((18.0, 18.0), (22.0, 22.0))
}

/// A disc of `n` points 30 mm across round (20, 20), with [`small`] for a hole.
fn holed_disc(n: u32) -> Polygon {
    let ring: Vec<(f64, f64)> = (0..n)
        .map(|k| {
            let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(k) / f64::from(n));
            (20.0 + 15.0 * cos, 20.0 + 15.0 * sin)
        })
        .collect();
    part(&ring, &[&[(18.0, 18.0), (22.0, 18.0), (22.0, 22.0), (18.0, 22.0)]])
}

#[test]
fn an_area_that_cant_be_given_what_it_sews_is_sewn_as_drawn_and_not_counted_whole() {
    // A disc of many points with a hole that the square drawn after it fills. Finding the hole's details looks
    // only at the parts near it, and what the disc sews, its whole outline joined, takes the most work. With
    // a unit less than that, it is sewn as drawn, which is said, and isn't counted as digitized whole.
    let design = Design::new(vec![automatic("disc", &[holed_disc(2_000)]), automatic("square", &[small()])], DesignSettings::default()).unwrap();
    let rules = [Some(AutoParams::defaults()), Some(AutoParams::defaults())];
    let said = |budget: &Budget| flattened(&design, &rules, false, budget).said;
    let messages = |said: &[Diagnostic]| said.iter().map(|said| said.message.clone()).collect::<Vec<_>>();
    assert_eq!(messages(&said(&Budget::DEFAULT)), ["1 area is digitized whole under the areas that fill its holes, which sew after it."]);
    let least = least_work(said);
    let flat = flattened(&design, &rules, false, &Budget { max_work: least - 1, ..Budget::DEFAULT });
    let [said] = flat.said.as_slice() else { panic!("{:?}", messages(&flat.said)) };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoLayers, Some("disc")));
    assert_eq!(
        said.message,
        format!("Finding what this area sews needed more than the work budget of {} units, so it is sewn as it is drawn.", least - 1)
    );
    assert!(matches!(flat.elements.first(), Some((0, Cow::Borrowed(_)))));
}

#[test]
fn a_hole_costs_a_unit_for_each_part_far_from_it() {
    // A disc with a hole, and an area drawn after it of the square that fills the hole and, or not, a disc
    // of many points far from it. The far disc's box doesn't meet the hole's: looking at it costs a unit.
    let cost = |far: bool| {
        let mut parts = vec![small()];
        parts.extend(far.then(|| circle((100.0, 100.0), 10.0, 2_000)));
        let layers = [Some(layer_of(vec![holed_disc(64)])), Some(layer_of(parts))];
        let mut meter = Budget::DEFAULT.meter();
        let holes = details(&layers, &boxes(&layers), 0, &mut meter).unwrap();
        assert_eq!(holes.iter().map(|hole| hole.details.clone()).collect::<Vec<_>>(), [BTreeSet::from([1])]);
        Budget::DEFAULT.max_work - meter.work_left()
    };
    assert_eq!(cost(true), cost(false) + 1);
}

#[test]
fn moving_an_area_back_costs_2_units_for_each_element_to_find_where_it_and_its_details_sew() {
    // A disc drawn after the square that fills its hole, which it moves back past, and squares far from both
    // drawn after them.
    let cost = |far: usize| {
        let mut elements = vec![automatic("square", &[small()]), automatic("disc", &[holed_disc(64)])];
        elements.extend((0..far).map(|k| automatic(&format!("far-{k}"), &[square((100.0 + 10.0 * k as f64, 0.0), (105.0 + 10.0 * k as f64, 5.0))])));
        let mut layers: Vec<Option<Layer>> = elements.iter().map(|_| None).collect();
        layers[0] = Some(layer_of(vec![small()]));
        let closed = vec![Hole { part: 0, hole: 0, details: BTreeSet::from([0]) }];
        layers[1] = Some(Layer { closed, ..layer_of(vec![holed_disc(64)]) });
        let mut order: Vec<usize> = (0..elements.len()).collect();
        let mut meter = Budget::DEFAULT.meter();
        move_back(&elements, &layers, &mut order, 1, &mut meter).unwrap();
        assert_eq!(order[..2], [1, 0]);
        Budget::DEFAULT.max_work - meter.work_left()
    };
    assert_eq!(cost(10), cost(0) + 2 * 10);
}

/// `design` flattened with `budget`, each automatic area with the default settings of the rules.
fn flattened_with<'d>(design: &'d Design, budget: &Budget) -> Flattened<'d> {
    let rules: Vec<Option<AutoParams>> = design.elements().iter().map(|_| Some(AutoParams::defaults())).collect();
    flattened(design, &rules, false, budget)
}

#[test]
fn an_area_whose_holes_need_more_work_than_its_budget_allows_keeps_them() {
    // A square with a round hole of many points, and the disc that fills it drawn after it. Finding that the
    // disc fills the hole shrinks the disc, the most work of any step: with a unit less, the square keeps its
    // hole, which is said, and sews as it is drawn.
    let disc = circle((20.0, 20.0), 10.0, 2_000);
    let hole: Vec<(f64, f64)> = disc.outline.iter().map(|point| (point.x(), point.y())).collect();
    let square = part(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)], &[&hole]);
    let design = Design::new(vec![automatic("square", &[square]), automatic("disc", &[disc])], DesignSettings::default()).unwrap();
    let least = least_work(|budget| flattened_with(&design, budget).said);
    let flat = flattened_with(&design, &Budget { max_work: least - 1, ..Budget::DEFAULT });
    let [said] = flat.said.as_slice() else { panic!("{:?}", flat.said) };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoLayers, Some("square")));
    assert_eq!(said.message, format!("Finding the holes of this area needed more than the work budget of {} units, so it keeps them.", least - 1));
    assert!(matches!(flat.elements.first(), Some((0, Cow::Borrowed(_)))));
}

#[test]
fn an_area_whose_move_needs_more_work_than_its_budget_allows_keeps_its_place_and_its_holes() {
    // A square drawn after the square that fills its hole, with discs of many points far from both drawn
    // between them: moving back, it looks at each disc, the most work of any step. With a unit less, it keeps
    // its place, which is said, and its hole, whose detail sews before it.
    let mut elements = vec![automatic("detail", &[small()])];
    elements.extend((0..12_u32).map(|k| automatic(&format!("disc-{k}"), &[circle((100.0 + 10.0 * f64::from(k), 100.0), 3.0, 400)])));
    elements.push(automatic(
        "square",
        &[part(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)], &[&[(18.0, 18.0), (22.0, 18.0), (22.0, 22.0), (18.0, 22.0)]])],
    ));
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    assert_eq!(flattened_with(&design, &Budget::DEFAULT).elements.first().map(|(index, _)| *index), Some(13), "moved first");
    let least = least_work(|budget| flattened_with(&design, budget).said);
    let flat = flattened_with(&design, &Budget { max_work: least - 1, ..Budget::DEFAULT });
    let [said] = flat.said.as_slice() else { panic!("{:?}", flat.said) };
    assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoLayers, Some("square")));
    assert_eq!(said.message, format!("Finding where this area sews needed more than the work budget of {} units, so it keeps its place.", least - 1));
    assert!(matches!(flat.elements.last(), Some((13, Cow::Borrowed(_)))));
}

#[test]
fn an_area_whose_part_that_shows_needs_more_work_than_its_budget_allows_is_sewn_as_drawn_or_left_out() {
    // A square and 300 squares far from it drawn after it: finding what of it shows looks past each, a unit
    // each, the most work of any step, twice what a square sews. With a unit less, it is sewn as it is drawn,
    // in its place, which is said; or, the picture's background, it is left out whole.
    let mut elements = vec![automatic("square", &[square((0.0, 0.0), (10.0, 10.0))])];
    elements.extend((0..300_u32).map(|k| {
        let x = 20.0 + 10.0 * f64::from(k);
        automatic(&format!("far-{k}"), &[square((x, 0.0), (x + 5.0, 5.0))])
    }));
    let design = Design::new(elements, DesignSettings::default()).unwrap();
    let rules: Vec<Option<AutoParams>> = design.elements().iter().map(|_| Some(AutoParams::defaults())).collect();
    for (background, consequence, first) in
        [(false, "it is sewn as it is drawn, in its place", 0), (true, "it is left out whole as the picture's background", 1)]
    {
        let least = least_work(|budget| flattened(&design, &rules, background, budget).said);
        let flat = flattened(&design, &rules, background, &Budget { max_work: least - 1, ..Budget::DEFAULT });
        let [said] = flat.said.as_slice() else { panic!("{:?}", flat.said) };
        assert_eq!((said.code, said.element.as_ref().map(ElementId::as_str)), (Code::AutoLayers, Some("square")));
        assert_eq!(
            said.message,
            format!("Finding what of this area shows needed more than the work budget of {} units, so {consequence}.", least - 1)
        );
        assert!(matches!(flat.elements.first(), Some((index, Cow::Borrowed(_))) if *index == first), "{background}");
    }
}

#[test]
fn the_lowest_shape_told_to_be_the_background_is_left_out_though_it_is_no_layer() {
    // Told that the lowest shape is the picture's background, `flattened` leaves it out whatever it is: here a
    // shape with settings, no automatic area, of which nothing is kept.
    let design =
        Design::new(vec![filled("background", &[square((0.0, 0.0), (40.0, 40.0))]), automatic("square", &[small()])], DesignSettings::default())
            .unwrap();
    let flat = flattened(&design, &[None, Some(AutoParams::defaults())], true, &Budget::DEFAULT);
    assert_eq!((flat.elements.iter().map(|(index, _)| *index).collect::<Vec<_>>(), flat.kept), (vec![1], 0));
}

#[test]
fn a_part_in_a_hole_is_a_detail_wider_than_touching() {
    // A hole 10 mm wide and 30 mm tall, and a strip along its left side in it, as large as the smallest area
    // or larger: 0.04 mm wide, no wider than TOUCHING, a hairline, it is no detail; 0.1 mm wide, it is. A
    // square beside the hole, whose box meets the hole's, lies outside it, and is no detail.
    let hole = [square((10.0, 10.0), (20.0, 40.0))];
    let is_detail = |part: Polygon| detail(&hole, &[part], 1.0, &mut Budget::DEFAULT.meter()).unwrap();
    assert!(!is_detail(square((10.0, 10.0), (10.04, 40.0))));
    assert!(is_detail(square((10.0, 10.0), (10.1, 40.0))));
    assert!(!is_detail(square((20.0, 10.0), (25.0, 15.0))));
}

#[test]
fn a_hole_closes_when_its_details_leave_no_gap_in_it_wider_than_touching() {
    // A square with a hole 20 mm wide, and a square in the hole drawn after it, a gap all round between them:
    // 0.02 mm wide, a hairline that rounding leaves, the hole closes under it; 0.1 mm wide, fabric shows
    // through, and the hole stays open.
    let closes = |gap: f64| {
        let holed = part(&[(0.0, 0.0), (40.0, 0.0), (40.0, 40.0), (0.0, 40.0)], &[&[(10.0, 10.0), (10.0, 30.0), (30.0, 30.0), (30.0, 10.0)]]);
        let layers = [Some(layer_of(vec![holed])), Some(layer_of(vec![square((10.0 + gap, 10.0 + gap), (30.0 - gap, 30.0 - gap))]))];
        !details(&layers, &boxes(&layers), 0, &mut Budget::DEFAULT.meter()).unwrap().is_empty()
    };
    assert!(closes(0.02));
    assert!(!closes(0.1));
}

#[test]
fn an_area_with_no_hole_closed_has_no_move_to_make_and_costs_no_work() {
    // 2 squares side by side, neither with a hole closed: without any work, they keep their places, and
    // nothing is said.
    let parts = [square((0.0, 0.0), (10.0, 10.0)), square((10.0, 0.0), (20.0, 10.0))];
    let elements: Vec<Element> = parts.iter().enumerate().map(|(k, part)| automatic(&format!("square-{k}"), std::slice::from_ref(part))).collect();
    let mut layers: Vec<Option<Layer>> = parts.iter().map(|part| Some(layer_of(vec![part.clone()]))).collect();
    let (mut order, mut said) = (vec![0, 1], Vec::new());
    moved(&elements, &mut layers, &mut order, &Budget { max_work: 0, ..Budget::DEFAULT }, &mut said);
    assert_eq!((order, said), (vec![0, 1], Vec::new()));
}
