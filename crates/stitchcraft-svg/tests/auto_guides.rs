//! A guide on one shape of an automatic design (roadmap step M13.24, `REQ-AUTO-109` to `REQ-AUTO-111`): a
//! line labelled `stitch:guide`, drawn over a picture, makes the filled parts of automatic areas that it
//! crosses sew as a guided fill whose rows follow it. Every other part, the order's rules, the overlaps, the
//! base layer and the cap stay automatic, and the guide itself is never sewn.
//!
//! The pictures are drawn here in code: a white field, the picture's background, and on it one red area of
//! 3 parts, a broad part on the left, a narrow bar in the middle and a broad part on the right.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, Element, Reading, Shape};
use stitchcraft_plan::{Rgb, Thread};

/// The red of the area.
const RED: Thread = Thread::new(Rgb::new(255, 0, 0));
/// The green the guides are drawn in, a thread no shape of the pictures has.
const GREEN: Thread = Thread::new(Rgb::new(0, 255, 0));

/// The red area, one path of 3 parts: a broad part from (5, 10) to (25, 50), a bar 0.6 mm wide from (29, 10)
/// to (29.6, 40), narrower than the narrowest satin stroke and as wide everywhere, so a running stitch along
/// its middle and no thin line, and a broad part from (35, 10) to (55, 50).
const AREA: &str = r##"<path id="red" d="M 5,10 H 25 V 50 H 5 Z M 29,10 H 29.6 V 40 H 29 Z M 35,10 H 55 V 50 H 35 Z" fill="#ff0000"/>"##;

/// A guide labelled `label` along the path data `d`: a line drawn with a thin green stroke and no fill, as a
/// person draws one over a picture.
fn guide(id: &str, label: &str, d: &str) -> String {
    format!(r##"<path id="{id}" inkscape:label="{label}" d="{d}" fill="none" stroke="#00ff00" stroke-width="0.2"/>"##)
}

/// A guide down the left part, bent: it crosses that part alone.
fn down_the_left(id: &str) -> String {
    guide(id, "stitch:guide", "M 15,5 C 11,20 19,40 15,55")
}

/// The picture of the red area and `more`, on a white field 60 mm square, read with every shape digitized
/// automatically.
fn picture(more: &[String]) -> Design {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="60mm" height="60mm" viewBox="0 0 60 60"><rect id="field" x="0" y="0" width="60" height="60" fill="#ffffff"/>{AREA}{}</svg>"##,
        more.concat()
    );
    stitchcraft_svg::read_with(svg.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design
}

/// The elements `design` sews as, digitized, and what was said.
fn sewn(design: &Design) -> (Vec<Element>, Vec<Diagnostic>) {
    let (elements, said) = digitized(design, &Budget::DEFAULT);
    (elements.into_owned(), said)
}

/// Whether `element` sews a guided fill: a fill whose method is `guided_fill`, with a guide.
fn guided(element: &Element) -> bool {
    matches!(element.shape, Shape::Fill { .. }) && element.params.get("fill_method") == Some("guided_fill") && element.hints.guide.is_some()
}

/// The left and the right of the box round `element`'s outline, in millimetres.
fn across(element: &Element) -> (f64, f64) {
    let xs = element.shape.path().points().map(|p| p.x()).collect::<Vec<_>>();
    (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

/// The red elements of `elements` that sew a fill, the base layer's pieces left out.
fn red_fills(elements: &[Element]) -> Vec<&Element> {
    elements.iter().filter(|element| element.thread == RED && matches!(element.shape, Shape::Fill { .. })).collect()
}

/// Whether `said` holds a diagnostic with the code `code`, such as `SC-I0918`, of the element whose id starts
/// with `id`.
fn names(said: &[Diagnostic], code: &str, id: &str) -> bool {
    said.iter().any(|diagnostic| diagnostic.code.to_string() == code && diagnostic.element.as_ref().is_some_and(|element| element.as_str().starts_with(id)))
}

#[test]
fn req_auto_109_the_filled_parts_a_guide_crosses_sew_a_guided_fill_along_it() {
    // Without a guide, the red area's broad parts are one tatami fill, and no element is guided.
    let (plain, _) = sewn(&picture(&[]));
    assert!(red_fills(&plain).iter().all(|element| !guided(element)), "{plain:?}");
    // A guide down the left part makes that part a guided fill along it, an element of its own. The right
    // part, which the guide doesn't cross, stays the area's tatami fill under the area's name.
    let (elements, said) = sewn(&picture(&[down_the_left("guide")]));
    let fills = red_fills(&elements);
    let guided_fills: Vec<&&Element> = fills.iter().filter(|element| guided(element)).collect();
    assert_eq!(guided_fills.len(), 1, "{fills:?}");
    let (left, right) = across(guided_fills[0]);
    assert!(left > 3.0 && right < 27.0, "the guided fill sews the left part alone: {left} to {right}");
    let rest: Vec<&&Element> = fills.iter().filter(|element| !guided(element) && element.id.as_str() == "svg:red:fill").collect();
    assert_eq!(rest.len(), 1, "{fills:?}");
    assert!(across(rest[0]).0 > 33.0 && rest[0].params.get("fill_method").is_none(), "{:?}", rest[0]);
    // The guide is never sewn, and is named with what it steers.
    assert!(elements.iter().all(|element| element.thread != GREEN), "{elements:?}");
    assert!(names(&said, "SC-I0918", "svg:guide"), "{said:?}");
}

#[test]
fn req_auto_109_a_part_sewn_along_its_middle_keeps_its_stitches_when_a_guide_crosses_it() {
    // A guide across the left part and the narrow bar: the left part follows it, and the bar, a running
    // stitch along its middle by its width, stays one.
    let across_both = guide("guide", "stitch:guide", "M 8,30 C 15,25 25,35 32,30");
    let (plain, _) = sewn(&picture(&[]));
    let (elements, _) = sewn(&picture(&[across_both]));
    let strokes = |elements: &[Element]| -> Vec<(String, Shape)> {
        elements.iter().filter(|e| e.thread == RED && matches!(e.shape, Shape::Stroke { .. })).map(|e| (e.id.to_string(), e.shape.clone())).collect()
    };
    assert!(!strokes(&plain).is_empty());
    assert_eq!(strokes(&elements), strokes(&plain));
    assert_eq!(red_fills(&elements).iter().filter(|element| guided(element)).count(), 1, "{elements:?}");
}

#[test]
fn req_auto_110_a_part_two_guides_cross_follows_the_one_drawn_last() {
    // 2 guides cross the left part. It follows the second, drawn over the first, which is the one a
    // person sees on top: its guide starts where the second does.
    let first = down_the_left("first");
    let second = guide("second", "stitch:guide", "M 6,30 C 12,22 18,38 24,30");
    let (elements, _) = sewn(&picture(&[first, second]));
    let guided_fills: Vec<&Element> = elements.iter().filter(|element| guided(element)).collect();
    assert_eq!(guided_fills.len(), 1, "{elements:?}");
    let start = guided_fills[0].hints.guide.as_ref().unwrap().path.subpaths[0].start;
    assert!((start.x() - 6.0).abs() < 1e-6 && (start.y() - 30.0).abs() < 1e-6, "{start:?}");
}

#[test]
fn req_auto_110_an_area_no_guide_crosses_sews_as_without_guides() {
    // A blue square beside the red area, which the guide doesn't cross, sews element for element as
    // without the guide: the same shapes with the same settings.
    let blue = r##"<rect id="blue" x="35" y="52" width="20" height="6" fill="#0000ff"/>"##.to_string();
    let (plain, _) = sewn(&picture(std::slice::from_ref(&blue)));
    let (elements, _) = sewn(&picture(&[blue, down_the_left("guide")]));
    let of_blue = |elements: &[Element]| -> Vec<(String, Shape, String)> {
        elements.iter().filter(|e| e.id.as_str().starts_with("svg:blue")).map(|e| (e.id.to_string(), e.shape.clone(), format!("{:?}", e.params))).collect()
    };
    assert!(!of_blue(&plain).is_empty());
    assert_eq!(of_blue(&elements), of_blue(&plain));
}

#[test]
fn req_auto_111_a_guide_that_crosses_no_filled_part_is_named_and_changes_nothing() {
    // A guide over the white field alone, where no automatic area is filled, steers nothing: the picture
    // sews as without it, and SC-W0919 names it.
    let (plain, _) = sewn(&picture(&[]));
    let (elements, said) = sewn(&picture(&[guide("guide", "stitch:guide", "M 2,55 L 58,57")]));
    let shapes_of = |elements: &[Element]| elements.iter().map(|element| (element.id.to_string(), element.shape.clone())).collect::<Vec<_>>();
    assert_eq!(shapes_of(&elements), shapes_of(&plain));
    assert!(names(&said, "SC-W0919", "svg:guide"), "{said:?}");
}

#[test]
fn req_auto_111_a_guide_is_read_by_its_name_in_any_case_with_a_note() {
    // The name is read as every marker's: in any case, after white space, and a note after it is the
    // person's own.
    let (elements, _) = sewn(&picture(&[guide("guide", "  Stitch:GUIDE petal", "M 15,5 C 11,20 19,40 15,55")]));
    assert_eq!(elements.iter().filter(|element| guided(element)).count(), 1, "{elements:?}");
    assert!(elements.iter().all(|element| element.thread != GREEN), "{elements:?}");
}
