use std::collections::BTreeSet;

use super::*;
use crate::auto::MIN_LIKENESS;
use crate::auto::neighbours::tests::{filled, square};
use crate::design::DesignSettings;
use crate::normalize::fixture::part;
use stitchcraft_plan::{Rgb, Thread};

/// The narrowest satin stroke of a design that sets none.
const NARROWEST: Mm = DesignSettings::MIN_SATIN_STROKE_WIDTH;

/// A line 10 mm long from `x` = `left` to the right, straight on its left and ragged on its right: 0.4 mm
/// wide every millimetre and 0.7 mm wide half way between.
fn strand(left: f64) -> Polygon {
    let mut ring = vec![(left, 0.0), (left, 10.0)];
    ring.extend((0..=20).map(|i| (left + if i % 2 == 0 { 0.4 } else { 0.7 }, 10.0 - f64::from(i) * 0.5)));
    part(&ring, &[])
}

/// The square from `(x0, 0)` to `(x1, 10)`, its left side cut into `n` pieces, which a line along it looks
/// at one by one.
fn finely(x0: f64, x1: f64, n: u32) -> Polygon {
    let mut ring = vec![(x1, 0.0), (x1, 10.0)];
    ring.extend((0..=n).map(|i| (x0, 10.0 - 10.0 * f64::from(i) / f64::from(n))));
    part(&ring, &[])
}

/// `elements` as the layers give them, each automatic with the default settings of the rules but for those
/// in `params` by its place.
fn flattened(elements: Vec<Element>) -> (Flattened<'static>, Vec<Option<AutoParams>>) {
    let rules = elements.iter().map(|element| element.automatic.then(AutoParams::defaults)).collect();
    let elements = elements.into_iter().enumerate().map(|(index, element)| (index, Cow::Owned(element))).collect();
    (Flattened { elements, kept: 0, said: Vec::new() }, rules)
}

/// An automatic area `id` of `parts`, with `params`.
fn auto(id: &str, parts: &[Polygon], params: &[(&str, &str)]) -> Element {
    let mut element = filled(id, parts);
    element.automatic = true;
    element.params = params.iter().copied().collect();
    element
}

/// The ids of what `merged` gives, and what it says.
fn run(elements: Vec<Element>, budget: &Budget) -> (Vec<String>, Vec<String>) {
    let (flattened, rules) = flattened(elements);
    let (out, _) = merged(flattened, &rules, NARROWEST, budget);
    (out.elements.iter().map(|(_, element)| element.id.to_string()).collect(), out.said.iter().map(ToString::to_string).collect())
}

#[test]
fn a_line_whose_area_merges_whole_sews_nothing_and_its_trim_has_no_place() {
    let set = filled("set", &[square((30.0, 0.0), (40.0, 10.0))]);
    let elements = vec![auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]), auto("line", &[strand(10.0)], &[("trim_after", "true")]), set];
    let (ids, said) = run(elements, &Budget::DEFAULT);
    assert_eq!(ids, ["left", "set"]);
    assert_eq!(
        said,
        [
            "info SC-I0915: A line of this area, of 5.5 mm², is narrower than the narrowest satin stroke, so it is merged into `left`, which \
             it shares the longest edge with.",
            "warning SC-W0505: This element sews no stitch, so the trim after it is left out."
        ]
    );
}

#[test]
fn a_line_a_hair_from_the_rest_of_its_own_area_rejoins_it_across_the_gap() {
    // 0.02 mm apart, nearer than twice the shapes' tolerance.
    let (flattened, rules) = flattened(vec![auto("a", &[square((0.0, 0.0), (10.0, 10.0)), strand(10.02)], &[])]);
    let (out, merges) = merged(flattened, &rules, NARROWEST, &Budget::DEFAULT);
    let said: Vec<String> = out.said.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["info SC-I0915: A line of this area, of 5.5 mm², is narrower than the narrowest satin stroke, so it is joined to the rest of this \
          area, which it shares the longest edge with."]
    );
    assert_eq!(merges.iter().map(|merge| merge.into).collect::<Vec<_>>(), [0]);
    // One part, the gap bridged.
    let Some((_, element)) = out.elements.first() else { panic!("no element") };
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("no fill") };
    let parts = parts_of(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert_eq!(parts.len(), 1);
    assert!(ops::area_of(&parts) > 105.5, "{}", ops::area_of(&parts));
}

#[test]
fn a_line_on_a_tie_goes_into_the_area_that_sews_first() {
    // An even square's edge on each side: `right`'s is the line's ragged side, touched at its widest places
    // only, so `left` and a copy of it drawn after it tie.
    let elements = vec![
        auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]),
        auto("again", &[square((0.0, 0.0), (10.0, 10.0))], &[]),
        auto("line", &[strand(10.0)], &[]),
    ];
    let (_, said) = run(elements, &Budget::DEFAULT);
    assert!(said.first().is_some_and(|said| said.contains("merged into `left`")), "{said:?}");
}

#[test]
fn a_shape_that_needs_more_work_than_the_budget_allows_keeps_its_lines_and_says_so() {
    // The line looks at each of the 3,000 pieces of `right`'s side beside it, which costs more than finding
    // that it is a line.
    let elements = || {
        vec![
            auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]),
            auto("line", &[strand(10.0)], &[]),
            auto("right", &[finely(10.7, 20.0, 3000)], &[]),
        ]
    };
    let full = run(elements(), &Budget::DEFAULT);
    let (mut seen, mut work) = (BTreeSet::new(), 0);
    loop {
        let (ids, said) = run(elements(), &Budget { max_work: work, ..Budget::DEFAULT });
        if (ids.clone(), said.clone()) == full {
            break;
        }
        // A line that can't be merged stays where it is.
        assert_eq!(ids, ["left", "line", "right"], "{work}");
        seen.extend(said.iter().filter_map(|said| said.split(" needed more").next().map(str::to_string)));
        work += 1;
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        [
            "info SC-I0915: Finding the thin lines of this area",
            "info SC-I0915: Finding where to merge this line",
            "info SC-I0915: Finding whether this line is a part of its colour"
        ]
    );
}

#[test]
fn lines_whose_parts_need_more_work_than_the_budget_allows_sew_as_lines_and_say_so() {
    // 2 gold lines side by side between a red and a blue area: gold's one part, which sews as lines.
    let coloured = |id: &str, parts: &[Polygon], color: Rgb| Element { thread: Thread::new(color), ..auto(id, parts, &[]) };
    let elements = || {
        vec![
            coloured("left", &[square((0.0, 0.0), (10.0, 10.0))], Rgb::new(200, 0, 0)),
            coloured("line", &[strand(10.0)], Rgb::new(230, 180, 30)),
            coloured("line2", &[strand(10.7)], Rgb::new(230, 180, 30)),
            coloured("right", &[square((11.4, 0.0), (20.0, 10.0))], Rgb::new(0, 0, 200)),
        ]
    };
    let full = run(elements(), &Budget::DEFAULT);
    assert_eq!(full.1.len(), 2, "{:?}", full.1);
    assert!(full.1.iter().all(|said| said.contains("but sews as a line")), "{:?}", full.1);
    let (mut seen, mut work) = (BTreeSet::new(), 0);
    loop {
        let (ids, said) = run(elements(), &Budget { max_work: work, ..Budget::DEFAULT });
        if (ids.clone(), said.clone()) == full {
            break;
        }
        assert_eq!(ids, ["left", "line", "line2", "right"], "{work}");
        seen.extend(said.iter().filter_map(|said| said.split(" needed more").next().map(str::to_string)));
        work += 1;
    }
    assert!(seen.contains("info SC-I0915: Finding whether this line is a part of its colour"), "{seen:?}");
}

#[test]
fn a_part_is_no_speck_and_holds_a_thread() {
    let rules = AutoParams::defaults();
    let smallest = rules.auto_min_area_mm2;
    assert_eq!(
        [is_part(smallest, true, &rules), is_part(smallest - 1e-9, true, &rules), is_part(smallest * 10.0, false, &rules)],
        [true, false, false]
    );
}

#[test]
fn a_line_is_ragged_from_the_least_unevenness_on() {
    let line = strand(10.0);
    let mut meter = Budget::DEFAULT.meter();
    let cell = piece(&line);
    let lines = centre_lines(&line, cell, &mut meter).unwrap().unwrap();
    let spread = spread(&Edges::of(&line, cell, &mut meter).unwrap(), &lines, &mut meter).unwrap().unwrap();
    // The line's own unevenness: it is ragged at it, and not above it.
    let least = spread.deviation / spread.mean.get();
    assert!(ragged(&line, least, &mut Budget::DEFAULT.meter()).unwrap());
    assert!(!ragged(&line, least * 1.001, &mut Budget::DEFAULT.meter()).unwrap());
}

#[test]
fn a_line_merges_into_no_area_whose_box_only_is_near_it() {
    // An L round the line's top and right side, 1 mm and more away from it: its box holds the line.
    let around = part(&[(0.0, 12.0), (20.0, 12.0), (20.0, -5.0), (12.0, -5.0), (12.0, 11.0), (0.0, 11.0)], &[]);
    // Its colour keeps no share of its parts, so that a line merges wherever it shares an edge.
    let line = auto("line", &[strand(10.0)], &[(MIN_LIKENESS, "0")]);
    let (ids, said) = run(vec![auto("around", &[around], &[]), line], &Budget::DEFAULT);
    assert_eq!((ids, said), (vec!["around".to_string(), "line".to_string()], Vec::<String>::new()));
}

#[test]
fn a_line_beside_no_area_stays_in_its_area_when_another_line_of_it_merges() {
    let elements = vec![auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]), auto("lines", &[strand(10.0), strand(40.0)], &[])];
    let (flattened, rules) = flattened(elements);
    let (out, merges) = merged(flattened, &rules, NARROWEST, &Budget::DEFAULT);
    assert_eq!(merges.len(), 1);
    // `lines` keeps its far line, and sews it.
    let Some((_, element)) = out.elements.get(1) else { panic!("{} elements", out.elements.len()) };
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("no fill") };
    let parts = parts_of(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert_eq!((element.id.as_str(), parts.len()), ("lines", 1));
    assert!((ops::area_of(&parts) - 5.5).abs() < 1e-6, "{}", ops::area_of(&parts));
}
