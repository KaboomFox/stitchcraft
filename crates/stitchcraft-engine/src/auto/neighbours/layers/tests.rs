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
