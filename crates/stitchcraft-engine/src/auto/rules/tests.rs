use stitchcraft_core::Mm;
use stitchcraft_params::{Kind, ParamSet};

use super::*;

/// The bounds of a shape whose settings of the rules are `set`, in a design whose narrowest satin stroke is
/// 1 mm.
fn bounds(set: &[(&str, &str)]) -> Bounds {
    let rules = AutoParams::from_set(&set.iter().copied().collect::<ParamSet>()).unwrap().params;
    Bounds::of(&rules, Mm::new(1.0).unwrap())
}

/// A part no wider than a satin column, with these measures.
fn part(area: f64, width: f64, unevenness: f64, lines: usize) -> Measures {
    Measures { area, breadth: Breadth::Medium, width, unevenness, lines }
}

#[test]
fn req_auto_030_the_rules_are_tried_in_the_tables_order_and_each_has_a_word_of_its_own() {
    let ids: Vec<RuleId> = RULES.iter().map(|rule| rule.id).collect();
    let order = [RuleId::Asked, RuleId::Speck, RuleId::Line, RuleId::Area, RuleId::Compact, RuleId::Uneven, RuleId::Column, RuleId::Refused];
    assert_eq!(ids, order);
    let keys: Vec<&str> = RULES.iter().map(|rule| rule.id.key()).collect();
    assert_eq!(keys, ["asked", "speck", "line", "area", "compact", "uneven", "column", "refused"]);
    // Every setting a rule names is a setting of the rules, and its texts leave no placeholder unfilled.
    for rule in RULES {
        assert!(rule.bounds.iter().all(|key| AutoParams::SPECS.iter().any(|spec| spec.key == *key)), "{:?}", rule.id);
        let (is, sewn) = (filled(rule.is), filled(rule.sewn));
        assert!(!is.contains('{') && !sewn.contains('{'), "{is} {sewn}");
        assert!(rule.label.to_lowercase() == rule.id.key(), "{}", rule.label);
    }
}

#[test]
fn req_auto_030_a_rules_text_gives_each_bound_s_default_with_its_unit() {
    assert_eq!(filled("smaller than {auto_min_area_mm2}"), "smaller than 1 mm²");
    assert_eq!(filled("wider than {auto_max_satin_width_mm}, {auto_min_fill_width_mm}"), "wider than 7 mm, 4 mm");
    assert_eq!(filled("{auto_min_slenderness} times, {auto_max_unevenness} of itself"), "3 times, 0.3 of itself");
    assert_eq!(filled("stroke, {narrowest}"), "stroke, 1 mm");
    assert_eq!(filled(RULES[2].sewn), "a running stitch along its centre lines, a triple run from 0.5 mm wide");
}

#[test]
fn req_auto_030_every_rule_has_a_case_at_each_bound() {
    let defaults = bounds(&[]);
    let rule = |measures: Measures| choose(&measures, &defaults);
    // The smallest area, 1 mm²: a part exactly that large is no speck.
    assert_eq!(rule(Measures::of_breadth(1.0_f64.next_down(), Breadth::Broad)), RuleId::Speck);
    assert_eq!(rule(Measures::of_breadth(1.0, Breadth::Broad)), RuleId::Area);
    assert_eq!(rule(Measures::of_breadth(1.0, Breadth::Narrow)), RuleId::Line);
    // The least slenderness, 3, to its third decimal: a part exactly as slender is a column.
    assert_eq!(rule(part(26.9946, 3.0, 0.0, 1)), RuleId::Compact);
    assert_eq!(rule(part(26.9964, 3.0, 0.0, 1)), RuleId::Column);
    assert_eq!(rule(part(27.0, 3.0, 0.0, 1)), RuleId::Column);
    // A part without a centre line has no width, and is compact.
    assert_eq!(rule(part(9.0, 0.0, 0.0, 0)), RuleId::Compact);
    assert_eq!(part(9.0, 0.0, 0.0, 0).slenderness(), 0.0);
    assert_eq!(part(18.0, 3.0, 0.0, 1).slenderness(), 2.0);
    // The most a width varies, 0.3, to its third decimal: a part whose width varies by exactly that is even,
    // and a part of one line is held to no evenness.
    assert_eq!(rule(part(40.0, 2.0, 0.3004, 2)), RuleId::Column);
    assert_eq!(rule(part(40.0, 2.0, 0.3006, 2)), RuleId::Uneven);
    assert_eq!(rule(part(40.0, 2.0, 0.9, 1)), RuleId::Column);
    // A compact part is compact however uneven.
    assert_eq!(rule(part(8.0, 2.0, 0.9, 2)), RuleId::Compact);
}

#[test]
fn req_auto_030_a_stroke_goes_by_its_width_to_the_micrometre() {
    let defaults = bounds(&[]);
    let rule = |width: f64| choose(&Measures::of_stroke(width, &defaults), &defaults);
    assert_eq!([rule(1.0), rule(1.0004), rule(1.0006)], [RuleId::Line, RuleId::Line, RuleId::Column]);
    assert_eq!([rule(7.0), rule(7.0004), rule(7.0006)], [RuleId::Column, RuleId::Column, RuleId::Area]);
    assert_eq!(defaults.breadth(1.0006), Breadth::Medium);
    // A stroke is one line of even width, however long: never a speck, compact or uneven.
    let stroke = Measures::of_stroke(3.0, &defaults);
    assert_eq!((stroke.lines, stroke.unevenness, stroke.width, stroke.area), (1, 0.0, 3.0, f64::INFINITY));
    // A satin stitch is too long from the micrometre over the widest satin column.
    assert_eq!([defaults.too_long(7.0004), defaults.too_long(7.0006)], [false, true]);
}

#[test]
fn req_auto_030_each_bound_is_a_setting_of_the_shape() {
    // The smallest area.
    let speck = Measures::of_breadth(1.5, Breadth::Narrow);
    assert_eq!([choose(&speck, &bounds(&[])), choose(&speck, &bounds(&[(MIN_AREA, "2")]))], [RuleId::Line, RuleId::Speck]);
    // The least slenderness.
    let short = part(18.0, 3.0, 0.0, 1);
    assert_eq!([choose(&short, &bounds(&[])), choose(&short, &bounds(&[(MIN_SLENDERNESS, "2")]))], [RuleId::Compact, RuleId::Column]);
    // The most a width varies, and the switch that fills an uneven part.
    let ragged = part(40.0, 2.0, 0.4, 3);
    assert_eq!(choose(&ragged, &bounds(&[])), RuleId::Uneven);
    assert_eq!(choose(&ragged, &bounds(&[(MAX_UNEVENNESS, "0.5")])), RuleId::Column);
    assert_eq!(choose(&ragged, &bounds(&[(FILL_UNEVEN, "false")])), RuleId::Column);
    // The widest satin column, and the design's narrowest satin stroke.
    let narrower = bounds(&[(MAX_SATIN_WIDTH, "4")]);
    assert_eq!([bounds(&[]).breadth(5.0), narrower.breadth(5.0)], [Breadth::Medium, Breadth::Broad]);
    assert_eq!((narrower.max_satin.get(), narrower.min_fill), (4.0, 4.0));
    let wider = Bounds::of(&AutoParams::defaults(), Mm::new(1.5).unwrap());
    assert_eq!([bounds(&[]).breadth(1.2), wider.breadth(1.2)], [Breadth::Medium, Breadth::Narrow]);
    assert_eq!(bounds(&[(MIN_FILL_WIDTH, "2.5")]).min_fill, 2.5);
}

#[test]
fn req_auto_030_the_rules_lean_towards_satin_columns_or_fills_by_half() {
    let (satin, balanced, fill) = (bounds(&[(PREFER, "satin")]), bounds(&[(PREFER, "balanced")]), bounds(&[(PREFER, "fill")]));
    assert_eq!(balanced, bounds(&[]));
    assert_eq!((balanced.min_slenderness, balanced.max_unevenness, balanced.min_fill), (3.0, 0.3, 4.0));
    assert_eq!((satin.min_slenderness, satin.min_fill), (2.0, 6.0));
    assert_eq!((fill.min_slenderness, fill.min_fill), (4.5, 4.0 / LEAN));
    assert!((satin.max_unevenness - 0.45).abs() < 1e-12 && (fill.max_unevenness - 0.2).abs() < 1e-12, "{satin:?} {fill:?}");
    // The narrowest piece of a fill leans no wider than the widest satin column.
    assert_eq!(bounds(&[(PREFER, "satin"), (MIN_FILL_WIDTH, "6")]).min_fill, 7.0);
    // A part between the bounds sews as each lean says.
    let between = part(22.5, 3.0, 0.25, 2);
    assert_eq!([choose(&between, &satin), choose(&between, &balanced), choose(&between, &fill)], [RuleId::Column, RuleId::Compact, RuleId::Compact]);
    let ragged = part(60.0, 2.0, 0.25, 2);
    assert_eq!([choose(&ragged, &satin), choose(&ragged, &balanced), choose(&ragged, &fill)], [RuleId::Column, RuleId::Column, RuleId::Uneven]);
}

#[test]
fn req_auto_036_a_shape_that_names_its_stitch_sews_so_where_it_can() {
    let asked = |stitch: &str| bounds(&[(STITCH, stitch)]);
    assert_eq!(
        [asked("auto").asked, asked("run").asked, asked("satin").asked, asked("fill").asked],
        [Asked::Auto, Asked::Run, Asked::Satin, Asked::Fill]
    );
    // The setting's choices are the stitches a shape can ask for, and no other.
    let spec = AutoParams::SPECS.iter().find(|spec| spec.key == STITCH).unwrap();
    let Kind::Choice { options } = spec.kind else { panic!("{:?}", spec.kind) };
    assert_eq!(options.iter().map(|option| option.id).collect::<Vec<_>>(), ["auto", "run", "satin", "fill"]);
    let blob = part(8.0, 2.0, 0.0, 1);
    let (line, area) = (Measures::of_breadth(8.0, Breadth::Narrow), Measures::of_breadth(80.0, Breadth::Broad));
    // A running stitch and a fill at any breadth, satin columns where a column can be.
    for measures in [blob, line, area] {
        assert_eq!([choose(&measures, &asked("run")), choose(&measures, &asked("fill"))], [RuleId::Asked, RuleId::Asked]);
    }
    assert_eq!(
        [choose(&blob, &asked("satin")), choose(&line, &asked("satin")), choose(&area, &asked("satin"))],
        [RuleId::Asked, RuleId::Line, RuleId::Area]
    );
    // A speck is a speck whatever the shape asks.
    assert_eq!(choose(&Measures::of_breadth(0.5, Breadth::Narrow), &asked("fill")), RuleId::Speck);
    // What is filled: an area, a compact part, an uneven one, and any part of a shape that asks to be.
    let filled: Vec<bool> = RULES.iter().map(|rule| fills(rule.id, &asked("auto"))).collect();
    assert_eq!(filled, [false, false, false, true, true, true, false, false]);
    assert_eq!(
        [fills(RuleId::Asked, &asked("fill")), fills(RuleId::Asked, &asked("satin")), fills(RuleId::Asked, &asked("run"))],
        [true, false, false]
    );
}

#[test]
fn req_auto_030_a_style_is_a_named_set_of_the_rules_settings() {
    assert_eq!(PRESETS.iter().map(|preset| preset.name).collect::<Vec<_>>(), ["logo", "photo"]);
    for preset in PRESETS {
        // Each value reads, with nothing said, as the setting's own.
        let read = AutoParams::from_set(&preset.values.iter().copied().collect::<ParamSet>()).unwrap();
        assert!(read.warnings.is_empty() && !preset.help.is_empty(), "{}", preset.name);
        assert!(preset.values.iter().all(|(key, _)| AutoParams::SPECS.iter().any(|spec| spec.key == *key && spec.group != crate::auto::ADVANCED)));
    }
    // The photo style is the rules' defaults, and the logo style keeps the columns of a ragged part.
    let [logo, photo] = PRESETS.map(|preset| bounds(preset.values));
    assert_eq!(photo, bounds(&[]));
    let ragged = part(40.0, 2.0, 0.6, 3);
    assert_eq!([choose(&ragged, &logo), choose(&ragged, &photo)], [RuleId::Column, RuleId::Uneven]);
    assert_eq!(logo.min_slenderness, 2.0);
}

#[test]
fn diag_sc_i0912_names_each_part_s_rule_and_the_measures_that_decided_it() {
    let id = ElementId::new("shape").unwrap();
    let fired = |rule: RuleId, measures: Option<Measures>| Fired { rule, measures };
    assert_eq!(said(&id, &[]), None);
    let one = said(&id, &[fired(RuleId::Compact, Some(part(8.0, 2.0, 0.13, 1)))]).unwrap();
    assert_eq!((one.code, one.element.as_ref()), (Code::AutoRule, Some(&id)));
    assert_eq!(
        one.to_string(),
        "info SC-I0912: This shape sews by the rule compact (2 mm wide, slenderness 2.0, width varying by 0.13, 1 centre line)."
    );
    // What changes the choice is in the code's explanation, said once, and not with each shape.
    assert_eq!(one.fix, None);
    // A speck gives its area, a line and a stroke their width, an area and a part not measured nothing.
    let texts = [
        fired(RuleId::Speck, Some(Measures::of_breadth(0.75, Breadth::Narrow))),
        fired(RuleId::Line, Some(Measures { width: 0.6, lines: 2, ..Measures::of_breadth(4.0, Breadth::Narrow) })),
        fired(RuleId::Column, Some(Measures::of_stroke(3.0, &bounds(&[])))),
        fired(RuleId::Area, Some(Measures::of_breadth(80.0, Breadth::Broad))),
        fired(RuleId::Refused, None),
        fired(RuleId::Uneven, Some(part(40.0, 2.0, 0.4, 3))),
    ]
    .map(|fired| fired.text());
    let uneven = "uneven (2 mm wide, slenderness 10.0, width varying by 0.40, 3 centre lines)";
    assert_eq!(texts, ["speck (0.75 mm²)", "line (0.6 mm wide)", "column (3 mm wide)", "area", "refused", uneven]);
    // Several parts are numbered, the first 3 with their measures, and the rest counted by rule in the table's
    // order.
    let line = fired(RuleId::Line, None);
    let two = said(&id, &[fired(RuleId::Area, None), line]).unwrap();
    assert_eq!(two.message, "This shape's 2 parts sew by these rules, part 1: area, part 2: line.");
    let three = said(&id, &[line; 3]).unwrap();
    assert_eq!(three.message, "This shape's 3 parts sew by these rules, part 1: line, part 2: line, part 3: line.");
    let mut many = vec![line; 4];
    many.extend([fired(RuleId::Column, None), fired(RuleId::Speck, None), fired(RuleId::Column, None)]);
    let many = said(&id, &many).unwrap();
    assert!(many.message.starts_with("This shape's 7 parts sew by these rules, part 1: line, "), "{many}");
    assert!(many.message.ends_with("part 3: line, and 4 more: 1 speck, 1 line, 2 column."), "{many}");
    // VectorCraft shows 500 characters of an export's warning, and cuts a longer one without saying so: the
    // longest notice, of 3 measured parts and the rest counted under every rule, fits them with its code and
    // a long id before it.
    let mut most = vec![fired(RuleId::Uneven, Some(part(1234.5678, 6.999, 4.999, 12))); 3];
    most.extend(RULES.iter().flat_map(|rule| [fired(rule.id, None); 99]));
    let longest = said(&ElementId::new("a-layer-of-a-long-name:1234:fill").unwrap(), &most).unwrap().one_line();
    assert!(longest.ends_with(", 99 column, 99 refused.") && longest.chars().count() <= 500, "{}: {longest}", longest.chars().count());
}
