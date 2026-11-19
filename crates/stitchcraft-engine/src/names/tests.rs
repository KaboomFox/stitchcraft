//! Cases of the names of shapes (`REQ-AUTO-125`, `REQ-AUTO-126`), on elements made by hand, whose kinds
//! come from the elements given as sewn.

use stitchcraft_core::{Budget, Code, Mm};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::PaletteId;

use super::*;
use crate::design::{Cap, DesignSettings, FillRule, Hints, Join, Path, Shape};
use crate::generators::satin::SATIN_COLUMN;

const RED: Rgb = Rgb::new(200, 30, 30);
const BLUE: Rgb = Rgb::new(30, 30, 200);

fn palette() -> &'static Palette {
    PaletteId::BrotherPec.palette()
}

/// A fill `id` in a thread of `colour`, automatic when `automatic`.
fn fill(id: &str, colour: Rgb, automatic: bool) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape: Shape::Fill { path: Path { subpaths: Vec::new() }, rule: FillRule::NonZero, gradient: None },
        thread: Thread::new(colour),
        params: ParamSet::new(),
        automatic,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

/// A stroke `id` 2 mm wide in a thread of `colour`, a satin column when `satin`.
fn stroke(id: &str, colour: Rgb, satin: bool) -> Element {
    let params: ParamSet = if satin { [(SATIN_COLUMN, "true")].into_iter().collect() } else { ParamSet::new() };
    let shape = Shape::Stroke { path: Path { subpaths: Vec::new() }, width: Mm::new(2.0).unwrap(), join: Join::Miter { limit: 4.0 }, cap: Cap::Butt };
    Element { shape, params, ..fill(id, colour, true) }
}

fn named(element: Element, name: &str) -> Element {
    Element { name: Some(name.to_string()), ..element }
}

fn design(elements: Vec<Element>) -> Design {
    Design::new(elements, DesignSettings::default()).unwrap()
}

/// The names of `design`'s shapes, `sewn` as they are generated and in `order`, in the Layers panel's order.
fn given(design: &Design, sewn: &[Element], order: &[&str]) -> Names {
    let order: Vec<ElementId> = order.iter().map(|id| ElementId::new(*id).unwrap()).collect();
    let order: Vec<&ElementId> = order.iter().collect();
    let base = [ElementId::new("base").unwrap()];
    Names::given(design, (sewn, &base, &order), palette(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The names of a design of one automatic red area, `id`, which sews nothing.
pub(crate) fn one(id: &str) -> Names {
    given(&design(vec![fill(id, RED, true)]), &[], &[])
}

fn listed(names: &Names) -> Vec<(Option<&str>, &str, Kind, Option<usize>)> {
    names.shapes.iter().map(|shape| (shape.name.as_deref(), shape.element.as_str(), shape.kind, shape.place)).collect()
}

#[test]
fn req_auto_125_a_name_is_the_thread_how_the_shape_sews_and_its_number_in_brackets() {
    assert_eq!(name("Red", Kind::Fill, 3), "Red fill (3)");
    let labels = [Kind::Fill, Kind::Column, Kind::Line, Kind::LeftOut].map(Kind::label);
    assert_eq!(labels, ["fill", "column", "line", "left out"]);
    let kinds = [Some(StitchType::SatinColumn), Some(StitchType::ZigzagStitch), Some(StitchType::RunningStitch), Some(StitchType::RippleStitch)];
    assert_eq!(kinds.map(Kind::of), [Kind::Column, Kind::Column, Kind::Line, Kind::Line]);
    assert_eq!([Some(StitchType::TatamiFill), None].map(Kind::of), [Kind::Fill, Kind::Fill]);
}

#[test]
fn req_auto_125_a_thread_is_named_by_its_own_name_the_palette_s_or_its_colour() {
    let entry = palette().entries.first().unwrap();
    let own = Thread { color: RED, name: Some("Rosa's red".to_string()) };
    assert_eq!(thread_name(&own, palette()), "Rosa's red");
    // A thread of an entry's colour has the entry's name, and one without has its hex code.
    assert_eq!(thread_name(&Thread::new(entry.color), palette()), entry.name);
    assert_eq!(thread_name(&Thread::new(Rgb::new(1, 2, 3)), palette()), "#010203");
    // A name that is empty, or a marker's, is no thread's name.
    for name in ["  ", " Stitch:keep"] {
        assert_eq!(thread_name(&Thread { color: Rgb::new(1, 2, 3), name: Some(name.to_string()) }, palette()), "#010203", "{name}");
    }
    assert!(!marker("stitch") && marker("STITCH:guide"));
}

#[test]
fn req_auto_125_shapes_are_numbered_by_thread_and_kind_from_the_layers_panel_s_top_row() {
    // From the bottom: 2 red areas, a blue one, a red stroke sewn as a column, an object of a red fill and a
    // red stroke, and a red area left out.
    let elements = vec![
        fill("vc:1:fill", RED, true),
        fill("vc:2:fill", RED, true),
        fill("vc:3:fill", BLUE, true),
        stroke("vc:4:stroke", RED, true),
        fill("vc:5:fill", RED, true),
        stroke("vc:5:stroke", RED, false),
        fill("vc:6:fill", RED, true),
    ];
    let design = design(elements.clone());
    // Digitizing sewed the 2nd area as columns only, its first part with its id; the 5th's fill as a
    // fill; and the base layer.
    let sewn: Vec<Element> = vec![
        fill("base", BLUE, true),
        elements[0].clone(),
        stroke("vc:2:fill", RED, true),
        fill("vc:2:fill#2", RED, true),
        elements[2].clone(),
        elements[3].clone(),
        stroke("vc:5:stroke", RED, false),
        elements[4].clone(),
    ];
    let order = ["base", "vc:3:fill", "vc:1:fill", "vc:2:fill", "vc:4:stroke", "vc:2:fill#2", "vc:5:fill", "vc:5:stroke"];
    let names = given(&design, &sewn, &order);
    let red = thread_name(&Thread::new(RED), palette());
    let called = |kind: &str, n: u8| format!("{red} {kind} ({n})");
    let (fill1, fill2, column1, column2) = (called("fill", 1), called("fill", 2), called("column", 1), called("column", 2));
    let left = called("left out", 1);
    assert_eq!(
        listed(&names),
        [
            (Some(left.as_str()), "vc:6", Kind::LeftOut, None),
            (Some(fill1.as_str()), "vc:5", Kind::Fill, Some(5)),
            (Some(column1.as_str()), "vc:4", Kind::Column, Some(4)),
            (Some("#1e1ec8 fill (1)"), "vc:3", Kind::Fill, Some(1)),
            (Some(column2.as_str()), "vc:2", Kind::Column, Some(3)),
            (Some(fill2.as_str()), "vc:1", Kind::Fill, Some(2)),
        ]
    );
    // The sewing order, then the shape left out.
    let sewing: Vec<&str> = names.sewing_order().iter().map(|shape| shape.element.as_str()).collect();
    assert_eq!(sewing, ["vc:3", "vc:1", "vc:2", "vc:4", "vc:5", "vc:6"]);
    // An element's, a part's and a shape's own id name the shape; another names none.
    for id in ["vc:2:fill", "vc:2:fill#2", "vc:2:fill#2~3", "vc:2", "vc:2:fill~line1"] {
        assert_eq!(names.name_of(id), Some(column2.as_str()), "{id}");
    }
    assert_eq!((names.name_of("vc:7:fill"), names.of("base"), names.name_of("vc:2:fil")), (None, None, None));
    assert_eq!(names.of("vc:3").map(|shape| shape.thread), Some(BLUE));
}

#[test]
fn req_auto_125_a_shape_sews_as_its_own_part_or_else_as_its_first() {
    // Nothing sews with the red area's id, and its first part is a column; the blue area's own part, a
    // column, sews after a filled part of it.
    let design = design(vec![fill("vc:7:fill", RED, true), fill("vc:8:fill", BLUE, true)]);
    let sewn = [stroke("vc:7:fill#2", RED, true), fill("vc:7:fill#3", RED, true), fill("vc:8:fill#2", BLUE, true), stroke("vc:8:fill", BLUE, true)];
    let names = given(&design, &sewn, &[]);
    let kinds: Vec<(&str, Kind)> = names.shapes.iter().map(|shape| (shape.element.as_str(), shape.kind)).collect();
    assert_eq!(kinds, [("vc:8", Kind::Column), ("vc:7", Kind::Column)]);
}

#[test]
fn req_auto_126_own_names_are_kept_and_never_given_again() {
    let design = design(vec![
        fill("vc:1:fill", RED, true),
        named(fill("vc:2:fill", RED, true), "Lakeside"),
        fill("vc:3:fill", RED, false),
        named(fill("vc:4:fill", BLUE, true), &name(&thread_name(&Thread::new(RED), palette()), Kind::Fill, 1)),
    ]);
    let sewn = design.elements().to_vec();
    let names = given(&design, &sewn, &[]);
    let red = thread_name(&Thread::new(RED), palette());
    let (first, second) = (format!("{red} fill (1)"), format!("{red} fill (2)"));
    let listed: Vec<Option<&str>> = names.shapes.iter().map(|shape| shape.name.as_deref()).collect();
    // The shape with settings and no name has none.
    assert_eq!(listed, [Some(first.as_str()), None, Some("Lakeside"), Some(second.as_str())]);
    // A diagnostic of a named shape's element is given the name; one of no shape, or of none, isn't.
    let mut said = [
        Diagnostic::new(Code::AutoRule, "x").with_element(ElementId::new("vc:1:fill").unwrap()),
        Diagnostic::new(Code::AutoRule, "x").with_element(ElementId::new("vc:3:fill").unwrap()),
        Diagnostic::new(Code::AutoRule, "x"),
    ];
    names.tell(&mut said);
    let told: Vec<Option<&str>> = said.iter().map(|d| d.name.as_deref()).collect();
    assert_eq!(told, [Some(second.as_str()), None, None]);
}

#[test]
fn req_auto_125_naming_needs_work_at_each_step() {
    let design = design(vec![fill("vc:1:fill", RED, true), named(fill("vc:2:fill", RED, true), &name("#c81e1e", Kind::Fill, 1))]);
    let sewn = design.elements().to_vec();
    let order: Vec<&ElementId> = sewn.iter().map(|element| &element.id).collect();
    // A unit for each element of the design and each sewn, each number tried and each step placed: 2 + 2 + 2
    // + 2.
    for work in 0..8 {
        let budget = Budget { max_work: work, ..Budget::DEFAULT };
        assert!(Names::given(&design, (&sewn, &[], &order), palette(), &mut budget.meter()).is_err(), "{work}");
    }
    let names = Names::given(&design, (&sewn, &[], &order), palette(), &mut Budget { max_work: 8, ..Budget::DEFAULT }.meter()).unwrap();
    assert_eq!(names.name_of("vc:1:fill"), Some("#c81e1e fill (2)"));
}

#[test]
fn a_shape_is_its_elements_id_without_the_last_part() {
    assert_eq!([shape_of("vc:6:fill"), shape_of("svg:a:b:stroke"), shape_of("vc:6"), shape_of("base")], ["vc:6", "svg:a:b", "vc:6", "base"]);
}
