//! Marks that keep parts (roadmap step M13.39, `REQ-AUTO-104` to `REQ-AUTO-107`): a shape labelled
//! `stitch:keep` marks an area of a picture, and each part of an automatic area that lies wholly inside it
//! sews by its width, where automatic digitizing would merge it into a neighbour as a speck or a thin line.
//! Everything else sews as it would without the mark, and the mark itself is never sewn.
//!
//! The pictures are drawn here in code: a white field, the picture's background, a red square on it, and
//! blue specks and ragged blue lines on the square, as a trace of a photo leaves them.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::{Design, Element, Reading, Shape};
use stitchcraft_plan::{Rgb, Thread};

/// The blue of the specks and the lines.
const BLUE: Thread = Thread::new(Rgb::new(0, 0, 255));
/// The green the marks are drawn in, a thread no shape of the pictures has.
const GREEN: Thread = Thread::new(Rgb::new(0, 255, 0));

/// A blue speck 1.6 mm wide and 0.5 mm tall from (`x`, `y`), 0.8 mm² large: under the smallest area of
/// 1 mm², and narrower than the narrowest satin stroke, 1 mm.
fn speck(id: &str, x: f64, y: f64) -> String {
    format!(r##"<rect id="{id}" x="{x}" y="{y}" width="1.6" height="0.5" fill="#0000ff"/>"##)
}

/// A blue line 10 mm long from (`x`, `y`) downwards, straight on its left and ragged on its right: 0.4 mm
/// wide every millimetre and 0.7 mm half way between, as a strand of fur traces. It is narrower than the
/// narrowest satin stroke everywhere, and its width varies by far more than 0.05 of itself, so it is a
/// thin line (`REQ-AUTO-090`). The lines here start on the square's top edge: a line that lies wholly on
/// an area fills a hole of it, a detail that sews on the area whole, and never merges (`REQ-AUTO-023`).
fn line(id: &str, x: f64, y: f64) -> String {
    let mut points = vec![format!("{x},{y}"), format!("{x},{}", y + 10.0)];
    points.extend((0..=20).map(|i| format!("{},{}", x + if i % 2 == 0 { 0.4 } else { 0.7 }, y + 10.0 - f64::from(i) * 0.5)));
    format!(r##"<polygon id="{id}" points="{}" fill="#0000ff"/>"##, points.join(" "))
}

/// A mark labelled `label` round the box from (`x0`, `y0`) to (`x1`, `y1`): a rectangle drawn with a green
/// stroke and no fill, as a person draws one over a picture.
fn mark(label: &str, [x0, y0, x1, y1]: [f64; 4]) -> String {
    format!(
        r##"<rect id="mark" inkscape:label="{label}" x="{x0}" y="{y0}" width="{}" height="{}" fill="none" stroke="#00ff00" stroke-width="0.2"/>"##,
        x1 - x0,
        y1 - y0
    )
}

/// The picture of `shapes`, on a white field 60 mm square and a red square from (10, 10) to (50, 50), read
/// with every shape digitized automatically.
fn picture(shapes: &[String]) -> Design {
    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="60mm" height="60mm" viewBox="0 0 60 60"><rect id="field" x="0" y="0" width="60" height="60" fill="#ffffff"/><rect id="square" x="10" y="10" width="40" height="40" fill="#ff0000"/>{}</svg>"##,
        shapes.concat()
    );
    stitchcraft_svg::read_with(svg.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design
}

/// `design` with every automatic shape merging its thin lines whatever the likeness kept
/// (`auto_min_likeness` 0), and its marks.
fn merging(design: Design) -> Design {
    let mut elements = design.elements().to_vec();
    for element in elements.iter_mut().filter(|element| element.automatic) {
        element.params.set("auto_min_likeness", "0");
    }
    let mut merged = Design::new(elements, design.settings).unwrap();
    merged.marks = design.marks;
    merged
}

/// The elements `design` sews as, digitized, and what was said.
fn sewn(design: &Design) -> (Vec<Element>, Vec<Diagnostic>) {
    let (elements, said) = digitized(design, &Budget::DEFAULT);
    (elements.into_owned(), said)
}

/// The ids of the elements of `elements` sewn in `thread`.
fn in_thread<'e>(elements: &'e [Element], thread: &Thread) -> Vec<&'e str> {
    elements.iter().filter(|element| element.thread == *thread).map(|element| element.id.as_str()).collect()
}

/// Whether `said` holds a diagnostic with the code `code`, such as `SC-I0905`, of the area the SVG element
/// `id` is read as.
fn names(said: &[Diagnostic], code: &str, id: &str) -> bool {
    let area = format!("svg:{id}:fill");
    said.iter().any(|diagnostic| diagnostic.code.to_string() == code && diagnostic.element.as_ref().is_some_and(|element| element.as_str() == area))
}

/// Whether `element` is a running stitch: a stroke whose path is the line it sews along.
fn runs(element: &Element) -> bool {
    matches!(element.shape, Shape::Stroke { .. })
}

#[test]
fn req_auto_104_a_speck_wholly_inside_a_keep_mark_sews_and_one_outside_merges_as_before() {
    // Without a mark, both specks merge into the red square (REQ-AUTO-008).
    let plain = picture(&[speck("inner", 20.0, 20.0), speck("outer", 40.0, 40.0)]);
    let (elements, said) = sewn(&plain);
    assert!(in_thread(&elements, &BLUE).is_empty(), "{elements:?}");
    assert!(names(&said, "SC-I0905", "inner") && names(&said, "SC-I0905", "outer"), "{said:?}");
    // A mark round the inner speck keeps it: it sews by its width, a running stitch in its own blue, as a
    // part that is no speck would. The outer speck merges as it did.
    let marked = picture(&[speck("inner", 20.0, 20.0), speck("outer", 40.0, 40.0), mark("stitch:keep", [18.0, 18.0, 23.0, 22.0])]);
    let (elements, said) = sewn(&marked);
    let blue = in_thread(&elements, &BLUE);
    assert_eq!(blue.len(), 1, "{elements:?}");
    assert!(blue[0].starts_with("svg:inner") && elements.iter().any(|element| element.id.as_str() == blue[0] && runs(element)), "{elements:?}");
    assert!(!names(&said, "SC-I0905", "inner") && names(&said, "SC-I0905", "outer"), "{said:?}");
    // The mark is never sewn.
    assert!(
        in_thread(&elements, &GREEN).is_empty() && marked.elements().iter().all(|element| !element.id.as_str().starts_with("svg:mark")),
        "{elements:?}"
    );
}

#[test]
fn req_auto_105_a_thin_line_wholly_inside_a_keep_mark_sews_as_a_line_and_one_across_its_edge_merges() {
    // Without a mark, both ragged lines merge into the red square (REQ-AUTO-090). They are blue's only parts,
    // so at the default likeness of 0.85 they would sew as lines with no mark too (REQ-AUTO-095): at 0 they
    // merge, and the mark alone keeps the first.
    let lines = [line("inside", 20.0, 10.0), line("across", 35.0, 10.0)];
    let (elements, said) = sewn(&merging(picture(&lines)));
    assert!(in_thread(&elements, &BLUE).is_empty(), "{elements:?}");
    assert!(names(&said, "SC-I0915", "inside") && names(&said, "SC-I0915", "across"), "{said:?}");
    // The mark holds the first line whole and only the left of the second: the first sews as a running
    // stitch along its middle, in its blue, and the second merges as it did, since a part partly inside a
    // mark is decided as if there were none.
    let mut shapes = lines.to_vec();
    shapes.push(mark("stitch:keep", [18.0, 9.0, 35.5, 21.0]));
    let (elements, said) = sewn(&merging(picture(&shapes)));
    let blue = in_thread(&elements, &BLUE);
    assert!(!blue.is_empty() && blue.iter().all(|id| id.starts_with("svg:inside")), "{elements:?}");
    assert!(elements.iter().filter(|element| element.thread == BLUE).all(runs), "{elements:?}");
    assert!(!names(&said, "SC-I0915", "inside") && names(&said, "SC-I0915", "across"), "{said:?}");
}

#[test]
fn req_auto_106_a_mark_drawn_as_a_line_keeps_what_lies_within_half_its_stroke() {
    // A line drawn down the middle of the ragged line with a stroke 2 mm wide marks the band 1 mm either
    // side of it, between its ends, as a text marker's line does (REQ-AUTO-063).
    let band = r##"<path id="mark" inkscape:label="stitch:keep" d="M 20.5,9 L 20.5,21" fill="none" stroke="#00ff00" stroke-width="2"/>"##;
    let (elements, said) = sewn(&picture(&[line("inside", 20.0, 10.0), band.to_string()]));
    assert!(!in_thread(&elements, &BLUE).is_empty() && in_thread(&elements, &GREEN).is_empty(), "{elements:?}");
    assert!(!names(&said, "SC-I0915", "inside"), "{said:?}");
}

#[test]
fn req_auto_107_a_mark_changes_nothing_outside_the_parts_it_holds_whole() {
    // A mark that holds no part whole, here an empty corner of the square, leaves the picture sewing as it
    // does without it: the same elements, with the same shapes, in the same order.
    let shapes = [speck("inner", 20.0, 20.0), line("strand", 35.0, 20.0)];
    let (plain, _) = sewn(&picture(&shapes));
    let mut marked = shapes.to_vec();
    marked.push(mark("stitch:keep", [11.0, 40.0, 15.0, 45.0]));
    let (elements, said) = sewn(&picture(&marked));
    let shapes_of = |elements: &[Element]| elements.iter().map(|element| (element.id.to_string(), element.shape.clone())).collect::<Vec<_>>();
    assert_eq!(shapes_of(&elements), shapes_of(&plain));
    // And the mark that keeps nothing is named, so that a person knows to draw it round the part.
    assert!(
        said.iter().any(|diagnostic| diagnostic.code.to_string() == "SC-W0917"
            && diagnostic.element.as_ref().is_some_and(|element| element.as_str().starts_with("svg:mark"))),
        "{said:?}"
    );
}
