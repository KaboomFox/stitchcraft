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
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, in_place, unbased};
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
