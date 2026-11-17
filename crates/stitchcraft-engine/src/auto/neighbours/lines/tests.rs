use std::collections::BTreeSet;

use super::*;
use crate::auto::neighbours::tests::{filled, square};
use crate::design::DesignSettings;
use crate::normalize::fixture::part;

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

/// A square 20 mm wide from the origin with a strand of fur 10 mm long on its top, from `x` = 4 up, straight
/// on its left and ragged on its right as [`strand`] is: one part, which sorting splits into its fill and the
/// strand, a thin piece.
fn tailed() -> Polygon {
    let mut ring = vec![(0.0, 0.0), (20.0, 0.0), (20.0, 20.0)];
    ring.extend((0..=20).map(|i| (if i % 2 == 0 { 4.4 } else { 4.7 }, 20.0 + f64::from(i) * 0.5)));
    ring.extend([(4.0, 30.0), (4.0, 20.0), (0.0, 20.0)]);
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
    let out = merged(flattened, &[], &rules, NARROWEST, budget).layers;
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
    let Merged { layers: out, merges, sorted } = merged(flattened, &[], &rules, NARROWEST, &Budget::DEFAULT);
    let said: Vec<String> = out.said.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["info SC-I0915: A line of this area, of 5.5 mm², is narrower than the narrowest satin stroke, so it is joined to the rest of this \
          area, which it shares the longest edge with."]
    );
    assert_eq!(merges.iter().map(|merge| merge.into).collect::<Vec<_>>(), [0]);
    // A whole line joined to the rest of its own area changes what the area sorts (`REQ-AUTO-092`).
    assert!(sorted.is_empty(), "{sorted:?}");
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
    // The line looks at each of the 60 pieces of `right`'s side beside it, which costs more than finding that
    // it is a line, and `left` is a plain square, which costs little.
    let elements = || {
        vec![
            auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]),
            auto("line", &[strand(10.0)], &[]),
            auto("right", &[finely(10.7, 20.0, 60)], &[]),
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
        // Each step's shortfall lasts thousands of units: finding the line costs some 7,600, and finding where
        // it goes some 11,000. `given` and `into` are run out of work at every unit below.
        work += 97;
    }
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        ["info SC-I0915: Finding the thin lines of this area", "info SC-I0915: Finding where to merge this line"]
    );
}

#[test]
fn an_area_needs_the_work_of_every_step_of_finding_its_lines_or_it_gives_none() {
    let rules = AutoParams::defaults();
    let id = ElementId::new("line").unwrap();
    let mut work = 0;
    let (sorted, given) = loop {
        match given(&id, &[strand(10.0)], NARROWEST, &rules, &|_, _| Ok(true), &mut Budget { max_work: work, ..Budget::DEFAULT }.meter()) {
            Ok(found) => break found,
            Err(_) => work += 1,
        }
    };
    assert!(work > 0);
    assert_eq!((sorted.along.len(), given.map(|given| (given.rest.len(), given.lines.len()))), (1, Some((0, 1))));
}

#[test]
fn a_line_needs_the_work_of_every_step_of_its_merge_or_it_is_not_merged() {
    // Beside `left`, and a hair from it, which bridges the gap with a second union.
    for left in [10.0, 10.02] {
        let line = Line { from: 1, part: strand(left), whole: true, at: 0 };
        let areas = || vec![area(&auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]), Some(&AutoParams::defaults()), &Budget::DEFAULT), None];
        let mut work = 0;
        let place = loop {
            let mut areas = areas();
            match into(&mut areas, &line, &mut Budget { max_work: work, ..Budget::DEFAULT }.meter()) {
                Ok(place) => break place,
                // Short of work, nothing is changed.
                Err(_) => assert!(areas.first().unwrap().as_ref().is_some_and(|area| !area.changed && area.parts.len() == 1), "{work}"),
            }
            work += 1;
        };
        assert_eq!(place, Some(0), "{left}");
        assert!(work > 0);
    }
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
    let (ids, said) = run(vec![auto("around", &[around], &[]), auto("line", &[strand(10.0)], &[])], &Budget::DEFAULT);
    assert_eq!((ids, said), (vec!["around".to_string(), "line".to_string()], Vec::<String>::new()));
}

#[test]
fn a_line_beside_no_area_stays_in_its_area_when_another_line_of_it_merges() {
    let elements = vec![auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]), auto("lines", &[strand(10.0), strand(40.0)], &[])];
    let (flattened, rules) = flattened(elements);
    let Merged { layers: out, merges, .. } = merged(flattened, &[], &rules, NARROWEST, &Budget::DEFAULT);
    assert_eq!(merges.len(), 1);
    // `lines` keeps its far line, and sews it.
    let Some((_, element)) = out.elements.get(1) else { panic!("{} elements", out.elements.len()) };
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("no fill") };
    let parts = parts_of(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert_eq!((element.id.as_str(), parts.len()), ("lines", 1));
    assert!((ops::area_of(&parts) - 5.5).abs() < 1e-6, "{}", ops::area_of(&parts));
}

/// The rules `parts` sewed by, by key.
fn rules_of(parts: &Parts) -> Vec<&'static str> {
    parts.fired.iter().map(|fired| fired.rule.key()).collect()
}

#[test]
fn req_auto_091_a_thin_piece_of_a_broader_part_merges_into_the_area_beside_it() {
    // `beside` runs along the strand's straight side for 10 mm. The strand meets the rest of `tailed` along
    // the chord it was cut by, 0.4 mm.
    let elements = vec![auto("tailed", &[tailed()], &[]), auto("beside", &[square((-10.0, 20.0), (4.0, 30.0))], &[])];
    let (flattened, rules) = flattened(elements);
    let Merged { layers: out, merges, sorted } = merged(flattened, &[], &rules, NARROWEST, &Budget::DEFAULT);
    let said: Vec<String> = out.said.iter().map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["info SC-I0915: A thin piece of this area, of 5.48 mm², would sew as a running stitch, so it is merged into `beside`, which it \
          shares the longest edge with."]
    );
    assert_eq!(merges.iter().map(|merge| merge.into).collect::<Vec<_>>(), [1]);
    let ids: Vec<&str> = out.elements.iter().map(|(_, element)| element.id.as_str()).collect();
    assert_eq!(ids, ["tailed", "beside"]);
    // `tailed` took nothing in and sews as it was sorted, less the strand and its rule (`REQ-AUTO-092`).
    // `beside` took the strand in, and is sorted again.
    let kept = sorted.get(&0).unwrap();
    assert_eq!((kept.along.len(), kept.judged.len(), rules_of(kept)), (0, 0, vec!["area"]));
    assert!(!sorted.contains_key(&1), "{:?}", sorted.keys());
}

#[test]
fn req_auto_091_a_thin_piece_beside_no_other_area_sews_as_it_was_sorted_and_is_not_said() {
    let (flattened, rules) = flattened(vec![auto("tailed", &[tailed()], &[])]);
    let Merged { layers: out, merges, sorted } = merged(flattened, &[], &rules, NARROWEST, &Budget::DEFAULT);
    assert!(out.said.is_empty() && merges.is_empty(), "{:?}", out.said);
    // Its own area is the one it shares the longest edge with: it goes back and keeps its place.
    let kept = sorted.get(&0).unwrap();
    assert_eq!((kept.along.len(), kept.judged.clone(), rules_of(kept)), (1, vec![1], vec!["area", "line"]));
}

#[test]
fn req_auto_092_an_area_that_takes_no_line_in_sews_as_it_was_sorted() {
    // `plain` has no line, and `left` takes `line` in.
    let elements = vec![
        auto("plain", &[square((50.0, 0.0), (60.0, 10.0))], &[]),
        auto("left", &[square((0.0, 0.0), (10.0, 10.0))], &[]),
        auto("line", &[strand(10.0)], &[]),
    ];
    let (flattened, rules) = flattened(elements);
    let Merged { sorted, .. } = merged(flattened, &[], &rules, NARROWEST, &Budget::DEFAULT);
    assert_eq!(sorted.keys().copied().collect::<Vec<_>>(), [0]);
    let bounds = Bounds::of(&AutoParams::defaults(), NARROWEST);
    let again = narrow::sorted(&ElementId::new("plain").unwrap(), vec![square((50.0, 0.0), (60.0, 10.0))], &bounds, &AutoParams::defaults(), &|_, _| Ok(true), &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(sorted.get(&0), Some(&again));
}

#[test]
fn the_parts_left_drop_the_rules_of_the_lines_that_left_and_keep_the_others_in_place() {
    use crate::auto::rules::{Fired, RuleId};
    let fired = |rule| Fired { rule, measures: None };
    let narrow = |x: f64| Along::Narrow(narrow::Narrow { part: square((x, 0.0), (x + 0.5, 10.0)), lines: Vec::new(), width: Mm::new(0.5).unwrap() });
    // Rules: an area, a line, a compact part, a line, a line. The lines sew along their middles at 0, 1 and 2.
    let parts = Parts {
        along: vec![narrow(0.0), narrow(1.0), narrow(2.0)],
        fired: vec![fired(RuleId::Area), fired(RuleId::Line), fired(RuleId::Compact), fired(RuleId::Column), fired(RuleId::Line)],
        judged: vec![1, 3, 4],
        ..Parts::default()
    };
    assert_eq!(without(parts.clone(), &BTreeSet::new()), parts);
    let left = without(parts, &BTreeSet::from([0]));
    assert_eq!((rules_of(&left), left.along, left.judged), (vec!["area", "compact", "column", "line"], vec![narrow(1.0), narrow(2.0)], vec![2, 3]));
}
