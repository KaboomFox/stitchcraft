use stitchcraft_core::{Mm, Point};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::auto::rules::{Breadth, Measures};
use crate::auto::{PREFER, outline};
use crate::design::{FillRule, Shape};
use crate::normalize::region::Polygon;

/// The bounds of a shape with `set`, in a design whose narrowest satin stroke is 1 mm.
fn bounds(set: &[(&str, &str)]) -> Bounds {
    let rules = AutoParams::from_set(&set.iter().copied().collect::<ParamSet>()).unwrap().params;
    Bounds::of(&rules, Mm::new(1.0).unwrap())
}

/// The bounds of the photo style, the defaults, and of the logo style.
fn styles() -> (Bounds, Bounds) {
    (bounds(&[]), bounds(&[(PREFER, "satin"), (FILL_UNEVEN, "false")]))
}

/// A part sewn by `rule`, no wider than a satin column, `width` wide and with `area`, whose width varies by
/// `unevenness` along `lines` centre lines.
fn part(rule: RuleId, (area, width): (f64, f64), unevenness: f64, lines: usize) -> Fired {
    Fired { rule, measures: Some(Measures { area, breadth: Breadth::Medium, width, unevenness, lines, reach: 1.0 }) }
}

/// An automatic square with `params`.
fn square(params: &[(&str, &str)]) -> Element {
    let p = |x: f64, y: f64| Point::new(x, y).unwrap();
    let square = Polygon { outline: vec![p(0.0, 0.0), p(0.0, 5.0), p(5.0, 5.0), p(5.0, 0.0), p(0.0, 0.0)], holes: Vec::new() };
    Element {
        id: ElementId::new("square").unwrap(),
        name: None,
        shape: Shape::Fill { path: outline(&[square]), rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(Rgb::new(0, 0, 0)),
        params: params.iter().copied().collect(),
        automatic: true,
        lettered: None,
        hints: Default::default(),
        clips: Vec::new(),
    }
}

#[test]
fn the_other_style_sets_auto_fill_uneven_the_other_way() {
    assert_eq!([other(true).map(|preset| preset.name), other(false).map(|preset| preset.name)], [Some("logo"), Some("photo")]);
    let elements = [square(&[]), square(&[(FILL_UNEVEN, "false")]), square(&[(PREFER, "fill")])];
    let rules: Vec<Option<AutoParams>> = elements.iter().map(|element| crate::auto::rules(&element.params)).take(2).chain([None]).collect();
    let others = others(&elements, &rules);
    let read = |other: &Option<AutoParams>| other.as_ref().map(|rules| (rules.auto_prefer, rules.auto_fill_uneven));
    // The other style's values are set over the shape's own, and a shape without settings has none.
    assert_eq!(others.iter().map(read).collect::<Vec<_>>(), [Some(("satin", false)), Some(("balanced", true)), None]);
}

#[test]
fn a_part_sews_otherwise_when_the_other_bounds_fill_it_or_give_it_columns() {
    let (photo, logo) = styles();
    let uneven = part(RuleId::Uneven, (40.0, 2.0), 0.4, 2);
    let column = part(RuleId::Column, (40.0, 2.0), 0.4, 2);
    // Uneven in the photo style, satin columns in the logo style, and the reverse.
    assert_eq!(changed(&uneven, &photo, &logo), Some(Changed::Filled(RuleId::Uneven)));
    assert_eq!(changed(&column, &logo, &photo), Some(Changed::Along));
    // Slenderness 2.5: compact for the photo style's 3, a column for the logo style's 2.
    assert_eq!(changed(&part(RuleId::Compact, (10.0, 2.0), 0.0, 1), &photo, &logo), Some(Changed::Filled(RuleId::Compact)));
    // An even column is a column in both, and an uneven one of 1 centre line too.
    assert_eq!(changed(&part(RuleId::Column, (40.0, 2.0), 0.1, 2), &photo, &logo), None);
    assert_eq!(changed(&part(RuleId::Column, (40.0, 2.0), 0.4, 1), &photo, &logo), None);
    // A part filled in both styles, and one without measures, sew alike.
    assert_eq!(changed(&part(RuleId::Uneven, (40.0, 2.0), 0.5, 2), &photo, &bounds(&[(FILL_UNEVEN, "true"), (PREFER, "satin")])), None);
    assert_eq!(changed(&Fired { rule: RuleId::Uneven, measures: None }, &photo, &logo), None);
    // A refused column is filled here, and would be refused there again, or filled.
    assert_eq!(changed(&part(RuleId::Refused, (40.0, 2.0), 0.4, 2), &logo, &logo), None);
    assert_eq!(changed(&part(RuleId::Refused, (40.0, 2.0), 0.4, 2), &logo, &photo), None);
    // A part filled by the rule arms is filled in both styles.
    let arms = Fired { rule: RuleId::Arms, measures: Some(Measures { reach: 2.0, ..column.measures.unwrap() }) };
    assert_eq!(changed(&arms, &logo, &photo), None);
}

#[test]
fn req_auto_151_the_notice_counts_by_the_other_style_and_says_nothing_when_nothing_changes() {
    let (photo, logo) = styles();
    let id = ElementId::new("t").unwrap();
    assert!(Tally::default().said().is_empty());
    let mut alike = Tally::default();
    alike.lines(0, 0);
    alike.parts(&id, &[part(RuleId::Column, (40.0, 2.0), 0.1, 2)], (&photo, &logo), &Budget::DEFAULT);
    assert!(alike.said().is_empty());
    // In the photo style: lines merged, and parts filled by 2 rules, over 2 shapes.
    let mut tally = Tally::default();
    tally.lines(2, 0);
    let filled = [part(RuleId::Uneven, (40.0, 2.0), 0.4, 2), part(RuleId::Compact, (10.0, 2.0), 0.0, 1)];
    tally.parts(&id, &filled, (&photo, &logo), &Budget::DEFAULT);
    tally.parts(&id, &filled[..1], (&photo, &logo), &Budget::DEFAULT);
    let said = tally.said();
    assert_eq!(
        said.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["info SC-I0923: The logo style (`--style logo`) would sew this design otherwise: 2 thin lines merged into their neighbours \
         would sew as lines and 1 part filled by the rule compact and 2 by the rule uneven would sew as satin columns."]
    );
    assert_eq!(
        said.first().and_then(|said| said.fix.clone()),
        Some(Fix::Hint(
            "`--style logo` sets `auto_prefer` to `satin` and `auto_fill_uneven` to `false` on every automatic shape. In VectorCraft, set \
             them in the dialog of StitchCraft's Auto effect."
                .to_string()
        ))
    );
    // In the logo style, in the singular.
    let mut tally = Tally::default();
    tally.lines(0, 1);
    tally.parts(&id, &[part(RuleId::Column, (40.0, 2.0), 0.4, 2)], (&logo, &photo), &Budget::DEFAULT);
    assert_eq!(
        tally.said().iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["info SC-I0923: The photo style (`--style photo`) would sew this design otherwise: 1 thin line sewn as a line would merge into its \
         neighbour and 1 part sewn as satin columns would be filled."]
    );
    let mut tally = Tally::default();
    tally.lines(1, 3);
    tally.parts(&id, &[part(RuleId::Uneven, (40.0, 2.0), 0.4, 2)], (&photo, &logo), &Budget::DEFAULT);
    tally.parts(&id, &[part(RuleId::Column, (40.0, 2.0), 0.4, 2); 2], (&logo, &photo), &Budget::DEFAULT);
    assert_eq!(
        tally.said().iter().map(ToString::to_string).collect::<Vec<_>>(),
        [
            "info SC-I0923: The logo style (`--style logo`) would sew this design otherwise: 1 thin line merged into its neighbour would sew \
         as a line and 1 part filled by the rule uneven would sew as satin columns.",
            "info SC-I0923: The photo style (`--style photo`) would sew this design otherwise: 3 thin lines sewn as lines would merge into \
         their neighbours and 2 parts sewn as satin columns would be filled."
        ]
    );
}

#[test]
fn parts_that_need_more_work_than_the_budget_allows_are_said_uncounted() {
    let (photo, logo) = styles();
    let id = ElementId::new("t").unwrap();
    let parts = [part(RuleId::Uneven, (40.0, 2.0), 0.4, 2); 2];
    let said = |work: u64| {
        let mut tally = Tally::default();
        tally.parts(&id, &parts, (&photo, &logo), &Budget { max_work: work, ..Budget::DEFAULT });
        tally.said().iter().map(|said| format!("{}: {said}", said.element.as_ref().map(ToString::to_string).unwrap_or_default())).collect::<Vec<_>>()
    };
    // A unit for each part.
    assert_eq!(
        said(1),
        ["t: info SC-I0923: Finding whether the logo style would sew this shape otherwise needed more than the work budget of 1 units, so \
         its parts aren't counted."]
    );
    assert_eq!(said(2).len(), 1);
    assert!(said(2)[0].contains("2 parts filled by the rule uneven"), "{:?}", said(2));
}

#[test]
fn items_are_listed_with_and_before_the_last() {
    let items = |n: usize| (1..=n).map(|n| n.to_string()).collect::<Vec<_>>();
    assert_eq!([listed(&items(0)), listed(&items(1)), listed(&items(2)), listed(&items(3))], ["", "1", "1 and 2", "1, 2 and 3"]);
    assert_eq!([counted(1, "a", "b"), counted(0, "a", "b"), counted(2, "a", "b")], ["1 a", "0 b", "2 b"]);
}
