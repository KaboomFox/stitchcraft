use stitchcraft_core::{ElementId, Point};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{FillRule, Path, Segment, Shape, Subpath};

/// A square fill `side` wide from (`x`, `y`) named `id`, in `colour`, with `params`.
fn square(id: &str, (x, y): (f64, f64), side: f64, colour: Rgb, params: &[(&str, &str)]) -> Element {
    let p = |x: f64, y: f64| Point::new(x, y).unwrap();
    let segments = vec![Segment::Line(p(x + side, y)), Segment::Line(p(x + side, y + side)), Segment::Line(p(x, y + side))];
    let path = Path { subpaths: vec![Subpath { start: p(x, y), segments, closed: true }] };
    let shape = Shape::Fill { path, rule: FillRule::NonZero, gradient: None };
    let params = params.iter().copied().collect();
    let (id, thread) = (ElementId::new(id).unwrap(), Thread::new(colour));
    Element { id, name: None, shape, thread, params, automatic: false, lettered: None, hints: Default::default(), clips: Vec::new() }
}

const RED: Rgb = Rgb::new(200, 0, 0);
const BLUE: Rgb = Rgb::new(0, 0, 200);

fn id(text: &str) -> ElementId {
    ElementId::new(text).unwrap()
}

#[test]
fn each_value_of_digitized_as_reads_as_what_it_says() {
    let of = |value: &str| digitized(&square("a", (0.0, 0.0), 5.0, RED, &[(DIGITIZED_AS, value)]));
    assert_eq!(
        [of("part"), of("base"), of("lettering"), of("no"), of("maybe")],
        [Some(Digitized::Part), Some(Digitized::Base), Some(Digitized::Lettering), None, None]
    );
    assert_eq!(digitized(&square("a", (0.0, 0.0), 5.0, RED, &[])), None);
}

#[test]
fn req_auto_143_the_elements_set_as_digitizing_s_own_are_marked_for_planning() {
    let elements = vec![
        square("part", (0.0, 0.0), 5.0, RED, &[(DIGITIZED_AS, PART)]),
        square("base", (0.0, 0.0), 5.0, RED, &[(DIGITIZED_AS, BASE)]),
        square("letter", (0.0, 0.0), 5.0, RED, &[(DIGITIZED_AS, LETTERING)]),
        Element { lettered: Some(Lettered::Set), ..square("set", (0.0, 0.0), 5.0, RED, &[(DIGITIZED_AS, LETTERING)]) },
        square("hand", (0.0, 0.0), 5.0, RED, &[]),
    ];
    // A piece of the base layer that planning knows already is counted once.
    let (mut base, mut made) = (vec![id("base")], BTreeSet::new());
    let given = marked(Cow::Borrowed(&elements), &mut base, &mut made);
    assert_eq!(base, vec![id("base")]);
    assert_eq!(made, [id("part"), id("base")].into_iter().collect());
    let lettered: Vec<Option<Lettered>> = given.iter().map(|element| element.lettered).collect();
    assert_eq!(lettered, [None, None, Some(Lettered::Digitized), Some(Lettered::Set), None]);
    // Elements set as nothing to letter come back as they were given.
    let (mut base, mut made) = (Vec::new(), BTreeSet::new());
    let plain = marked(Cow::Borrowed(elements.get(..2).unwrap()), &mut base, &mut made);
    assert!(matches!(plain, Cow::Borrowed(_)));
    assert_eq!(base, vec![id("base")]);
}

#[test]
fn req_auto_143_the_cap_goes_by_the_lowest_automatic_shape_then_by_the_design_s_limits() {
    let mine = Held { cap: 5.0, ..thick::by_default().unwrap() };
    let rules = Held { cap: 4.0, ..mine };
    let kept = DesignSettings { cap: Some(mine), ..DesignSettings::default() };
    let (none, some) = (BTreeSet::new(), [id("a")].into_iter().collect());
    assert_eq!(limits(Some(rules), &none, &kept), Some(rules));
    assert_eq!(limits(None, &none, &kept), Some(mine));
    assert_eq!(limits(None, &some, &DesignSettings::default()), thick::by_default());
    assert_eq!(limits(None, &none, &DesignSettings::default()), None);
}

#[test]
fn held_reads_the_auto_effect_s_names_and_writes_them_as_they_read() {
    let held = Held { cap: 3.5, overlap: 0.75, ..thick::by_default().unwrap() };
    let set: ParamSet = held.values().iter().map(|(key, value)| (*key, value.as_str())).chain([("angle", "abc"), ("auto_stitch", "bogus")]).collect();
    assert_eq!(Held::of(&set), Ok(held));
    assert_eq!(held.values().map(|(key, _)| key), Held::KEYS);
    assert_eq!(held.values()[4].1, "3.5");
    // A value that doesn't read is named.
    let bad: ParamSet = [("auto_thick_cap_fills", "lots")].into_iter().collect();
    assert_eq!(Held::of(&bad).unwrap_err().len(), 1);
}

#[test]
fn diag_sc_w0922_expand_names_the_lettering_and_the_shapes_left_out() {
    let lost = |lettering: &[&str], left_out: &[&str]| {
        let owned = |shapes: &[&str]| shapes.iter().map(|shape| (*shape).to_string()).collect::<Vec<_>>();
        lost(&owned(lettering), &owned(left_out)).map(|said| said.to_string())
    };
    assert_eq!(lost(&[], &[]), None);
    let both = lost(&["\"Black satin (1)\""], &["`vc:3`", "`vc:4`", "`vc:5`"]).unwrap();
    assert_eq!(
        both,
        "warning SC-W0922: Expand couldn't keep everything. The lettering of \"Black satin (1)\" became the columns it sews: its text can't \
         be edited as text, and lettering set in a font no longer sews its underlay as lettering does. `vc:3`, `vc:4` and `vc:5` needed more \
         work than the budget allows, and sew nothing in the expanded design."
    );
    let fix = super::lost(&["a".to_string()], &[]).unwrap().fix;
    assert_eq!(
        fix,
        Some(Fix::Hint(
            "Edit the text in the automatic design and expand it again: the traced design is kept, hidden, under the expanded parts.".to_string()
        ))
    );
    let many = lost(&[], &["a", "b", "c", "d", "e", "f", "g"]).unwrap();
    assert!(many.contains(" a, b, c, d, e and 2 more needed more work"), "{many}");
}

#[test]
fn listed_names_each_shape_up_to_its_limit() {
    let listed = |shapes: &[&str]| listed(&shapes.iter().map(|shape| (*shape).to_string()).collect::<Vec<_>>());
    assert_eq!(listed(&["a"]), "a");
    assert_eq!(listed(&["a", "b"]), "a and b");
    assert_eq!(listed(&["a", "b", "c", "d", "e"]), "a, b, c, d and e");
    assert_eq!(listed(&["a", "b", "c", "d", "e", "f"]), "a, b, c, d, e and 1 more");
}

#[test]
fn req_auto_142_an_automatic_square_expands_to_named_parts_set_as_digitizing_made_them() {
    let red = Element { automatic: true, ..square("vc:1", (0.0, 0.0), 20.0, RED, &[("auto_overlap_mm", "0.5")]) };
    let blue = Element { automatic: true, ..square("vc:2", (5.0, 5.0), 10.0, BLUE, &[]) };
    let design = Design::new(vec![red, blue], DesignSettings::default()).unwrap();
    let (expanded, said) = expand(&design, REFERENCE, &Budget::DEFAULT).unwrap();
    assert!(!said.iter().any(|said| said.code == Code::AutoExpandLost), "{said:?}");
    assert!(!expanded.elements().is_empty());
    for element in expanded.elements() {
        assert!(!element.automatic && element.lettered.is_none(), "{}", element.id);
        assert_eq!(element.params.get("auto_overlap_mm"), None, "{}", element.id);
        assert!(matches!(element.params.get(DIGITIZED_AS), Some(PART | BASE)), "{}", element.id);
        assert!(element.name.as_deref().is_some_and(|name| !name.is_empty()), "{} {:?} {:?}", element.id, element.name, element.params);
    }
    // A piece of the base layer is named after its thread.
    let base = expanded.elements().iter().find(|element| element.params.get(DIGITIZED_AS) == Some(BASE)).unwrap();
    assert!(base.name.as_deref().is_some_and(|name| name.starts_with("Base layer 1, ")), "{:?}", base.name);
    // The design keeps the limits of its lowest automatic shape, with its overlap.
    assert_eq!(expanded.settings.cap.map(|cap| cap.overlap), Some(0.5));
}

#[test]
fn req_auto_144_a_shape_digitizing_has_no_work_for_is_named_as_left_out() {
    let red = Element { automatic: true, ..square("vc:1", (0.0, 0.0), 20.0, RED, &[]) };
    let blue = Element { automatic: true, ..square("vc:2", (30.0, 0.0), 20.0, BLUE, &[]) };
    let design = Design::new(vec![red, blue], DesignSettings::default()).unwrap();
    let (expanded, said) = expand(&design, REFERENCE, &Budget { max_work: 1, ..Budget::DEFAULT }).unwrap();
    assert!(expanded.elements().is_empty());
    let lost = said.iter().find(|said| said.code == Code::AutoExpandLost).unwrap();
    assert!(lost.message.contains("`vc:1` and `vc:2` needed more work"), "{}", lost.message);
    // Naming the shapes needs more work too, and is said once, of no shape.
    assert!(said.iter().any(|said| said.code == Code::BudgetExhausted && said.element.is_none()), "{said:?}");
}
