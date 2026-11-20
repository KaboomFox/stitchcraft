use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Mm};

use super::*;
use crate::auto::AutoParams;
use crate::auto::columns::filled;
use crate::auto::marks::Marks;
use crate::normalize::fixture::part;

/// The settings of the rules `set` gives, and their bounds in a design whose narrowest satin stroke is 1 mm.
fn rules(set: &[(&str, &str)]) -> (AutoParams, Bounds) {
    let rules = AutoParams::from_set(&set.iter().copied().collect()).unwrap().params;
    let bounds = Bounds::of(&rules, Mm::new(1.0).unwrap());
    (rules, bounds)
}

/// The bound raised to 10 mm.
const RAISED: [(&str, &str); 1] = [("auto_max_split_satin_width_mm", "10")];

/// What `part` is as a wide part of a shape with the settings `set`, with `budget`.
fn tried_with(part: &Polygon, set: &[(&str, &str)], budget: Budget) -> Result<Tried, Exhausted> {
    let (rules, bounds) = rules(set);
    let by = By { rules: &rules, open: &|_, _| Ok(true), marks: &Marks::default() };
    tried(&ElementId::new("wide").unwrap(), part, &bounds, by, &mut budget.meter())
}

/// [`tried_with`] the default budget.
fn tried_as(part: &Polygon, set: &[(&str, &str)]) -> Tried {
    tried_with(part, set, Budget::DEFAULT).unwrap()
}

/// Whether the layers take `part` of a shape with the settings `set` for a filled part, and by which rule.
fn filled_as(part: &Polygon, set: &[(&str, &str)]) -> Option<RuleId> {
    filled(part, &rules(set).1, &mut Budget::DEFAULT.meter()).unwrap()
}

/// A bar 40 mm long and `width` mm wide.
fn bar(width: f64) -> Polygon {
    part(&[(0.0, 0.0), (40.0, 0.0), (40.0, width), (0.0, width)], &[])
}

/// An L of 2 bars `width` mm wide and 40 mm long: a part of 2 centre lines.
fn corner(width: f64) -> Polygon {
    part(&[(0.0, 0.0), (40.0, 0.0), (40.0, width), (width, width), (width, 40.0), (0.0, 40.0)], &[])
}

/// `n` points of a circle of radius `r` round `(x, y)`, turning `turn` radians from `from`, both ends included.
fn arc(r: f64, (x, y): (f64, f64), n: usize, from: f64, turn: f64) -> Vec<(f64, f64)> {
    (0..=n)
        .map(|i| {
            let (sin, cos) = sin_cos(from + turn * i as f64 / n as f64);
            (x + r * cos, y + r * sin)
        })
        .collect()
}

/// A band `width` mm wide bent `turn` radians round a point `inner` mm inside it.
fn bend(inner: f64, width: f64, turn: f64) -> Polygon {
    let mut outline = arc(inner + width, (0.0, 0.0), 48, 0.0, turn);
    outline.extend(arc(inner, (0.0, 0.0), 48, turn, -turn));
    part(&outline, &[])
}

#[test]
fn req_auto_155_a_wide_column_splits_at_the_longest_satin_stitch_at_a_random_phase() {
    let mut params = ParamSet::new();
    split(&mut params);
    let set = |key: &str| params.get(key).map(str::to_string);
    assert_eq!(
        [set(MAX_STITCH_LENGTH), set(SPLIT_METHOD), set(RANDOM_SPLIT_PHASE), set(RANDOM_SPLIT_JITTER_PERCENT)],
        [Some("7".into()), Some("default".into()), Some("true".into()), Some("10".into())]
    );
    // A shape's own split settings stay its own.
    let mut own: ParamSet = [(SPLIT_METHOD, "simple"), (MAX_STITCH_LENGTH, "6")].into_iter().collect();
    split(&mut own);
    assert_eq!((own.get(SPLIT_METHOD), own.get(MAX_STITCH_LENGTH), own.get(RANDOM_SPLIT_PHASE)), (Some("simple"), Some("6"), Some("true")));
}

#[test]
fn req_auto_152_a_broad_part_no_wider_than_the_bound_is_one_split_column() {
    let Tried::Wide(wide) = tried_as(&bar(8.5), &RAISED) else { panic!("not wide") };
    assert_eq!((wide.sewn.len(), wide.measures.lines), (1, 1));
    assert!((wide.measures.width - 8.5).abs() < 0.05, "{:?}", wide.measures);
    // Off by default; wider than the bound; or a shape that asks to be filled or to run: no wide part.
    for (part, set) in [(bar(8.5), &[][..]), (bar(10.5), &RAISED[..])] {
        assert!(matches!(tried_as(&part, set), Tried::Not));
    }
    for asked in ["fill", "run"] {
        assert!(matches!(tried_as(&bar(8.5), &[RAISED[0], ("auto_stitch", asked)]), Tried::Not), "{asked}");
    }
    // A shape that asks for satin columns has its wide part sewn as one.
    assert!(matches!(tried_as(&bar(8.5), &[RAISED[0], ("auto_stitch", "satin")]), Tried::Wide(_)));
    // As wide as the bound and its tolerance, a bar is wide, and wider it isn't.
    assert!(matches!(tried_as(&bar(10.0 + crate::auto::rules::WIDE_TOLERANCE), &RAISED), Tried::Wide(_)));
    assert!(matches!(tried_as(&bar(10.01 + crate::auto::rules::WIDE_TOLERANCE), &RAISED), Tried::Not));
}

#[test]
fn req_auto_153_a_wide_part_of_2_lines_or_of_uneven_width_is_an_area() {
    assert!(matches!(tried_as(&corner(8.5), &RAISED), Tried::Not));
    // A wedge from 7.5 to 10 mm wide varies by too much for a wide part, though one line.
    let wedge = part(&[(0.0, 0.0), (40.0, 0.0), (40.0, 10.0), (0.0, 7.5)], &[]);
    assert!(matches!(tried_as(&wedge, &[RAISED[0], ("auto_max_unevenness", "0.01")]), Tried::Not));
    // A compact part is no column.
    assert!(matches!(tried_as(&part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 8.5), (0.0, 8.5)], &[]), &RAISED), Tried::Not));
}

#[test]
fn req_auto_154_a_wide_part_that_crowds_its_stitches_or_has_no_column_is_refused_and_says_why() {
    let Tried::Refused(said) = tried_as(&bend(3.0, 8.5, 1.5 * std::f64::consts::PI), &RAISED) else { panic!("not refused") };
    assert_eq!(said.code, Code::AutoSatinFilled);
    assert!(said.message.contains("of one edge to 1 mm of the other"), "{said}");
    // A disc whose hole touches its edge has no column.
    let outer: Vec<(f64, f64)> = arc(8.8, (0.0, 0.0), 32, 0.0, std::f64::consts::TAU).into_iter().take(32).collect();
    let hole: Vec<(f64, f64)> = arc(4.4, (4.4, 0.0), 16, 0.0, -std::f64::consts::TAU).into_iter().take(16).collect();
    let Tried::Refused(said) = tried_as(&part(&outer, &[&hole]), &RAISED) else { panic!("not refused") };
    assert_eq!(
        said.to_string(),
        "info SC-I0902: A part of this area is no wider than the widest split satin column, 10 mm, but no satin column can be drawn along \
         it between its edges, so it is filled."
    );
}

#[test]
fn req_auto_033_the_layers_keep_a_wide_part_s_holes() {
    // A wide part sews along its middle, so the layers don't take it for a filled part.
    assert_eq!([filled_as(&bar(8.5), &[]), filled_as(&bar(8.5), &RAISED)], [Some(RuleId::Area), None]);
    // A wide part of 2 lines, one wider than the bound, and one of a shape that asks to be filled are.
    assert_eq!([filled_as(&corner(8.5), &RAISED), filled_as(&bar(10.5), &RAISED)], [Some(RuleId::Area); 2]);
    assert_eq!(filled_as(&bar(8.5), &[RAISED[0], ("auto_stitch", "fill")]), Some(RuleId::Asked));
}

#[test]
fn req_auto_152_trying_a_wide_part_is_the_part_s_work() {
    let enough = |work: u64| tried_with(&bar(8.5), &RAISED, Budget { max_work: work, ..Budget::DEFAULT }).is_ok();
    let (mut low, mut high) = (0_u64, 1 << 24);
    while low < high {
        let middle = low + (high - low) / 2;
        if enough(middle) { high = middle } else { low = middle + 1 }
    }
    assert!(enough(low) && !enough(low - 1), "{low}");
    // With none, it stops at the part's breadth, and with all but the last unit, at its columns.
    for work in [0, low - 1] {
        assert_eq!(tried_with(&bar(8.5), &RAISED, Budget { max_work: work, ..Budget::DEFAULT }).err(), Some(Exhausted::Work), "{work}");
    }
}
