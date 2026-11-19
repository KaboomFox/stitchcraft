//! A guide on one shape of an automatic design (roadmap step M13.24, `REQ-AUTO-109` to `REQ-AUTO-111`): the
//! filled parts of an automatic area that a guide crosses sew as a guided fill along it, and the overlaps,
//! the base layer and the cap stay as they are. The designs are drawn here in code, as automatic digitizing
//! is given them.

// Test code may unwrap, panic and index (clippy.toml allows it in tests, but not in their helpers).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, ElementId, Point};
use stitchcraft_engine::auto::digitized;
use stitchcraft_engine::design::mark::Mark;
use stitchcraft_engine::design::{Design, DesignSettings, Element, Path, Segment, Shape, Subpath};
use stitchcraft_testkit::designs::{BLUE, GREEN, RED, automatic, fill, in_place, unbased};
use stitchcraft_testkit::fabric::{self, GAP, Limits};
use stitchcraft_testkit::pictures;

/// A guide `id` through `points`, an open line.
fn guide(id: &str, points: &[(f64, f64)]) -> Mark {
    let point = |(x, y): (f64, f64)| Point::new(x, y).unwrap();
    let line = Subpath { start: point(points[0]), segments: points[1..].iter().map(|&at| Segment::Line(point(at))).collect(), closed: false };
    Mark { id: ElementId::new(id).unwrap(), shape: Shape::stroke(Path { subpaths: vec![line] }) }
}

fn design(elements: Vec<Element>, guides: Vec<Mark>) -> Design {
    let mut design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    design.guides = guides;
    design
}

/// The left and the right of the box round `element`'s outline, in millimetres.
fn across(element: &Element) -> (f64, f64) {
    let xs: Vec<f64> = element.shape.path().points().map(|p| p.x()).collect();
    (xs.iter().copied().fold(f64::INFINITY, f64::min), xs.iter().copied().fold(f64::NEG_INFINITY, f64::max))
}

#[test]
fn req_auto_110_the_parts_a_guide_splits_from_their_area_overlap_it_no_more_than_before() {
    // One red area of 2 squares that rounding left a hair apart, 0.005 mm, as a trace leaves the parts of
    // one colour: without a guide they are one fill, and neither reaches under the other.
    let area = || {
        automatic(fill(
            "red",
            &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 20.0), (0.0, 20.0)], &[(10.005, 0.0), (20.0, 0.0), (20.0, 20.0), (10.005, 20.0)]],
            &RED,
            &[],
        ))
    };
    let plain = design(vec![area()], Vec::new());
    let (plain, _) = digitized(&plain, &Budget::DEFAULT);
    let fills: Vec<&Element> = plain.iter().filter(|e| matches!(e.shape, Shape::Fill { .. })).collect();
    assert_eq!(fills.len(), 1, "{plain:?}");
    // A guide down the left square makes it a guided fill of its own. The 2 fills are still one area for the
    // overlaps: each keeps its own square, and neither reaches under the other.
    let guided = design(vec![area()], vec![guide("guide", &[(5.0, -1.0), (4.0, 10.0), (5.0, 21.0)])]);
    let (elements, _) = digitized(&guided, &Budget::DEFAULT);
    let fills: Vec<&Element> = elements.iter().filter(|e| matches!(e.shape, Shape::Fill { .. })).collect();
    assert_eq!(fills.len(), 2, "{elements:?}");
    let guided = fills.iter().find(|e| e.params.get("fill_method") == Some("guided_fill")).unwrap();
    let rest = fills.iter().find(|e| e.params.get("fill_method").is_none()).unwrap();
    let (guided, rest) = (across(guided), across(rest));
    assert!(guided.1 <= 10.01 && rest.0 >= 10.0, "the guided fill sews {guided:?} and the rest {rest:?}");
}

#[test]
fn req_auto_109_a_guided_fill_covers_its_edges_as_the_tatami_fill_did() {
    // A red square on a blue one, as a picture is drawn: the red rows meet the square's left and right edges
    // square on, a row spacing apart, closer than the shortest stitch. Automatic digitizing keeps a fill's
    // turns there as stitches (REQ-FILL-TAT-014), so that no row starts inside the edge.
    let picture = |guides: Vec<Mark>| {
        let blue = automatic(fill("blue", &[&[(0.0, 0.0), (40.0, 0.0), (40.0, 30.0), (0.0, 30.0)]], &BLUE, &[]));
        let red = automatic(fill("red", &[&[(10.0, 10.0), (30.0, 10.0), (30.0, 20.0), (10.0, 20.0)]], &RED, &[]));
        design(vec![blue, red], guides)
    };
    let bare = |design: &Design| {
        let plan = in_place(design).plan.unwrap();
        fabric::cover(design, &plan, GAP).bare
    };
    let plain = bare(&picture(Vec::new()));
    // A guide along the square, bent, makes the red rows a guided fill: its edges stay as covered.
    let guided = picture(vec![guide("guide", &[(8.0, 15.0), (20.0, 13.0), (32.0, 15.0)])]);
    let (elements, _) = digitized(&guided, &Budget::DEFAULT);
    assert!(elements.iter().any(|e| e.params.get("fill_method") == Some("guided_fill")), "{elements:?}");
    let with_guide = bare(&guided);
    assert!(with_guide <= plain + 0.01, "{with_guide} mm² bare with the guide, {plain} without");
}

#[test]
fn req_auto_109_a_guide_across_ragged_patches_of_a_traced_photo_steers_them_and_the_cap_holds() {
    // The running dog's ear in 6 threads, 4 ragged patches of a traced photo. A guide drawn along it, bent,
    // from its left to its right a third of the way down: the filled parts it crosses sew guided fills.
    let ear = pictures::dog_shapes("dog-running-6", &["vc:5:fill", "vc:6:fill", "vc:28:fill", "vc:50:fill"]);
    let bounds = ear.bounds().unwrap();
    let (x0, y0, x1, y1) = (bounds.min().x(), bounds.min().y(), bounds.max().x(), bounds.max().y());
    let at = |u: f64, v: f64| (x0 + u * (x1 - x0), y0 + v * (y1 - y0));
    let mut guided = ear.clone();
    guided.guides = vec![guide("along", &[at(0.0, 0.35), at(0.5, 0.25), at(1.0, 0.35)])];
    let (elements, said) = digitized(&guided, &Budget::DEFAULT);
    assert!(elements.iter().any(|e| e.params.get("fill_method") == Some("guided_fill")), "{said:?}");
    assert!(said.iter().any(|d| d.code.to_string() == "SC-I0918"), "{said:?}");
    // The cap holds, and the guided parts leave no more fabric bare than the tatami fills did.
    let measured = |design: &Design| {
        let plan = in_place(design).plan.unwrap();
        (fabric::thickness(&plan, &Limits::DEFAULT).fills.value, fabric::cover(design, &plan, GAP).bare)
    };
    let ((_, bare), (thickest, guided_bare)) = (measured(&ear), measured(&guided));
    assert!(thickest <= 4.0 + 1e-9, "{thickest:.2} fills at the thickest spot");
    assert!(guided_bare <= bare + 0.05, "{guided_bare:.3} mm² bare with the guide, {bare:.3} without");
}

#[test]
fn req_auto_109_a_guide_over_a_detail_leaves_the_area_under_it_alone() {
    // A blue square drawn over the middle of a red one, as a stacked trace draws a detail: the red is
    // digitized whole under it. A guide across the blue alone steers the blue, and the red, which shows only
    // round it, keeps its tatami fill.
    let picture = |guides: Vec<Mark>| {
        // The green field is the picture's background, which is left out.
        let field = automatic(fill("field", &[&[(-5.0, -5.0), (35.0, -5.0), (35.0, 35.0), (-5.0, 35.0)]], &GREEN, &[]));
        let red = automatic(fill("red", &[&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)]], &RED, &[]));
        let blue = automatic(fill("blue", &[&[(10.0, 10.0), (20.0, 10.0), (20.0, 20.0), (10.0, 20.0)]], &BLUE, &[]));
        design(vec![field, red, blue], guides)
    };
    let guided = picture(vec![guide("guide", &[(12.0, 15.0), (15.0, 13.0), (18.0, 15.0)])]);
    let (elements, _) = digitized(&guided, &Budget::DEFAULT);
    let guided_ids: Vec<&str> = elements.iter().filter(|e| e.params.get("fill_method") == Some("guided_fill")).map(|e| e.id.as_str()).collect();
    assert_eq!(guided_ids, ["blue"], "{elements:?}");
    // Drawn across the red where it shows too, the guide steers both.
    let both = picture(vec![guide("guide", &[(5.0, 15.0), (15.0, 13.0), (18.0, 15.0)])]);
    let (elements, _) = digitized(&both, &Budget::DEFAULT);
    assert_eq!(elements.iter().filter(|e| e.params.get("fill_method") == Some("guided_fill")).count(), 2, "{elements:?}");
}

#[test]
fn req_auto_109_the_parts_of_one_area_that_follow_one_guide_are_one_element_after_the_others() {
    // One red area of 4 squares in a row. The first guide crosses the first 2, the second the third, and
    // the fourth follows none: it is the area's fill and keeps its name, and each guide's parts are one
    // guided fill after it, numbered, in the order the guides are drawn.
    let square = |x: f64| [(x, 0.0), (x + 10.0, 0.0), (x + 10.0, 10.0), (x, 10.0)];
    let (a, b, c, d) = (square(0.0), square(15.0), square(30.0), square(45.0));
    let area = automatic(fill("red", &[&a, &b, &c, &d], &RED, &[]));
    let guides = vec![guide("first", &[(-1.0, 5.0), (12.0, 4.0), (26.0, 5.0)]), guide("second", &[(32.0, -1.0), (35.0, 5.0), (32.0, 11.0)])];
    let guided = design(vec![area], guides);
    let (elements, said) = digitized(&guided, &Budget::DEFAULT);
    let fills: Vec<(&str, Option<&str>, usize, f64)> = elements
        .iter()
        .filter(|e| matches!(e.shape, Shape::Fill { .. }))
        .map(|e| {
            let start = e.hints.guide.as_ref().map_or(f64::NAN, |guide| guide.path.subpaths[0].start.x());
            (e.id.as_str(), e.params.get("fill_method"), e.shape.path().subpaths.len(), start)
        })
        .collect();
    let mut sorted = fills.clone();
    sorted.sort_by(|x, y| x.0.cmp(y.0));
    assert_eq!(sorted.len(), 3, "{fills:?}");
    assert_eq!((sorted[0].0, sorted[0].1, sorted[0].2), ("red", None, 1), "{fills:?}");
    assert_eq!((sorted[1].0, sorted[1].1, sorted[1].2, sorted[1].3), ("red#2", Some("guided_fill"), 2, -1.0), "{fills:?}");
    assert_eq!((sorted[2].0, sorted[2].1, sorted[2].2, sorted[2].3), ("red#3", Some("guided_fill"), 1, 32.0), "{fills:?}");
    let named: Vec<String> = said.iter().filter(|d| d.code.to_string() == "SC-I0918").map(|d| d.message.clone()).collect();
    assert!(named[0].starts_with("2 filled parts of 1 automatic area") && named[1].starts_with("1 filled part of 1 automatic area"), "{named:?}");
}
