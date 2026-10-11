//! The area a fill covers (`REQ-FILL-001`, `REQ-FILL-002`): what the drawing shows under its fill rule,
//! cut exactly where its subpaths cross, touch or run along each other, in parts with holes. Parts too
//! small to sew are left out with a warning, and small fills and fills in parts are pointed out.
//!
//! Fills are not sewn until the rest of roadmap M5, but a fill element's area is built already, so its
//! warnings come out of the plan, ahead of `SC-W0011`.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, ElementId, Point};
use stitchcraft_engine::design::{Element, FillRule, Path, Segment, Shape, Subpath};
use stitchcraft_engine::normalize::region::{Built, Polygon, TINY_PART, build};
use stitchcraft_params::ParamSet;
use stitchcraft_testkit::designs::{RED, line, messages, p, planned};

/// A path of closed subpaths, one through each list of points.
fn rings(parts: &[&[(f64, f64)]]) -> Path {
    let subpaths = parts
        .iter()
        .filter_map(|points| {
            let (&(x, y), rest) = points.split_first()?;
            Some(Subpath { start: p(x, y), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: true })
        })
        .collect();
    Path { subpaths }
}

/// The corners of the square from (`x`, `y`) of side `s`, counter-clockwise in y-up axes.
fn square(x: f64, y: f64, s: f64) -> [(f64, f64); 4] {
    [(x, y), (x + s, y), (x + s, y + s), (x, y + s)]
}

/// The same square drawn the other way round.
fn turned(x: f64, y: f64, s: f64) -> [(f64, f64); 4] {
    [(x, y), (x, y + s), (x + s, y + s), (x + s, y)]
}

fn region(path: &Path, rule: FillRule) -> Built {
    build(path, rule, &mut Budget::DEFAULT.meter()).unwrap()
}

/// A fill element `id` covering `path` under `rule`.
fn fill(id: &str, path: Path, rule: FillRule) -> Element {
    Element { id: ElementId::new(id).unwrap(), name: None, shape: Shape::Fill { path, rule }, thread: RED, params: ParamSet::default() }
}

/// What planning `element` beside a plain stroke says.
fn said(element: Element) -> Vec<String> {
    messages(&planned(vec![element, line("ok", (0.0, 40.0), 10.0, &RED, &[])]))
}

fn points(ring: &[(f64, f64)]) -> Vec<Point> {
    ring.iter().map(|&(x, y)| p(x, y)).collect()
}

#[test]
fn req_fill_001_the_fill_rule_says_what_is_filled() {
    // A ring inside a ring, both drawn the same way round: the nonzero rule fills the middle, the even-odd
    // rule leaves it empty. Drawn the other way round, the inner ring is a hole under both.
    let same = rings(&[&square(0.0, 0.0, 10.0), &square(3.0, 3.0, 4.0)]);
    let nonzero = region(&same, FillRule::NonZero).region;
    assert_eq!((nonzero.parts.len(), nonzero.parts[0].holes.len(), nonzero.area()), (1, 0, 100.0));
    let even_odd = region(&same, FillRule::EvenOdd).region;
    assert_eq!((even_odd.parts.len(), even_odd.parts[0].holes.len(), even_odd.area()), (1, 1, 84.0));
    let opposite = rings(&[&square(0.0, 0.0, 10.0), &turned(3.0, 3.0, 4.0)]);
    assert_eq!(region(&opposite, FillRule::NonZero).region, even_odd);
    // Overlapping squares: the nonzero rule fills them as one, the even-odd rule leaves the overlap out.
    let overlap = rings(&[&square(0.0, 0.0, 10.0), &square(5.0, 5.0, 10.0)]);
    let one = region(&overlap, FillRule::NonZero).region;
    assert_eq!((one.parts.len(), one.area()), (1, 175.0));
    let two = region(&overlap, FillRule::EvenOdd).region;
    assert_eq!((two.parts.len(), two.area()), (2, 150.0));
}

#[test]
fn req_fill_001_crossings_touches_and_shared_edges_are_cut_exactly() {
    // A bow tie: 2 triangles that meet where the outline crosses itself, each its own part.
    let bow = region(&rings(&[&[(0.0, 0.0), (20.0, 20.0), (20.0, 0.0), (0.0, 20.0)]]), FillRule::NonZero).region;
    assert_eq!(bow.parts.iter().map(Polygon::area).collect::<Vec<_>>(), [100.0, 100.0]);
    assert_eq!(bow.parts[0].outline, points(&[(0.0, 0.0), (0.0, 20.0), (10.0, 10.0), (0.0, 0.0)]));
    // Squares sharing an edge are one part; the edge between them is inside.
    let side_by_side = region(&rings(&[&square(0.0, 0.0, 10.0), &square(10.0, 0.0, 10.0)]), FillRule::NonZero).region;
    assert_eq!(side_by_side.parts.len(), 1);
    assert_eq!(side_by_side.parts[0].outline.len(), 7, "6 corners, the edge's ends among them, and the first again");
    // A hole touching its outline at a point stays a hole.
    let touching = region(&rings(&[&square(0.0, 0.0, 10.0), &[(5.0, 0.0), (6.0, 2.0), (4.0, 2.0)]]), FillRule::EvenOdd).region;
    assert_eq!((touching.parts.len(), touching.parts[0].holes.len()), (1, 1));
}

#[test]
fn req_fill_001_rings_turn_one_way_and_start_where_the_drawing_reaches_them_first() {
    // An outline drawn counter-clockwise in y-up axes from its top right corner, and a hole drawn
    // counter-clockwise too, under the even-odd rule: the outline is turned clockwise, still from that
    // corner, and the hole keeps its way round, from its first point.
    let path = rings(&[&[(10.0, 10.0), (0.0, 10.0), (0.0, 0.0), (10.0, 0.0)], &[(6.0, 4.0), (6.0, 6.0), (4.0, 6.0), (4.0, 4.0)]]);
    let found = region(&path, FillRule::EvenOdd).region;
    assert_eq!(found.parts[0].outline, points(&[(10.0, 10.0), (10.0, 0.0), (0.0, 0.0), (0.0, 10.0), (10.0, 10.0)]));
    assert_eq!(found.parts[0].holes, [points(&[(6.0, 4.0), (6.0, 6.0), (4.0, 6.0), (4.0, 4.0), (6.0, 4.0)])]);
    // Parts come in the order drawn.
    let parts = region(&rings(&[&square(20.0, 0.0, 10.0), &square(0.0, 0.0, 10.0)]), FillRule::NonZero).region;
    assert_eq!(parts.parts[0].outline[0], p(20.0, 0.0));
}

#[test]
fn req_fill_001_curves_are_flattened_within_a_tenth_of_a_pixel() {
    // A circle of radius 10 mm in 4 cubic Béziers: its area within the flattening's reach of π r².
    let k = 0.552_284_749_830_793_4 * 10.0;
    let quarter = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| Segment::Cubic(p(a.0, a.1), p(b.0, b.1), p(c.0, c.1));
    let circle = Path {
        subpaths: vec![Subpath {
            start: p(10.0, 0.0),
            segments: vec![
                quarter((10.0, k), (k, 10.0), (0.0, 10.0)),
                quarter((-k, 10.0), (-10.0, k), (-10.0, 0.0)),
                quarter((-10.0, -k), (-k, -10.0), (0.0, -10.0)),
                quarter((k, -10.0), (10.0, -k), (10.0, 0.0)),
            ],
            closed: true,
        }],
    };
    let area = region(&circle, FillRule::NonZero).region.area();
    let pi = std::f64::consts::PI;
    // Chords cut off at most a tenth of a pixel (0.026 mm) all round, and the Bézier circle bulges by
    // 0.03 % of its radius.
    assert!(area < pi * 100.0 * 1.0006 && area > pi * 100.0 - 2.0 * pi * 10.0 * 0.03, "{area}");
}

#[test]
fn req_fill_001_subpaths_of_fewer_than_3_points_bound_nothing() {
    let with_strays = rings(&[&square(0.0, 0.0, 10.0), &[(5.0, 5.0)], &[(2.0, 2.0), (8.0, 8.0)]]);
    assert_eq!(region(&with_strays, FillRule::EvenOdd), region(&rings(&[&square(0.0, 0.0, 10.0)]), FillRule::EvenOdd));
    let nothing = region(&rings(&[&[(0.0, 0.0), (5.0, 0.0)]]), FillRule::NonZero);
    assert!(nothing.region.parts.is_empty() && nothing.diagnostics.is_empty());
}

#[test]
fn req_fill_002_parts_too_small_to_sew_are_left_out_and_said() {
    let tiny = TINY_PART.sqrt() * 0.9;
    let found = region(&rings(&[&square(0.0, 0.0, 10.0), &square(20.0, 0.0, tiny)]), FillRule::NonZero);
    assert_eq!(found.region.parts.len(), 1);
    assert_eq!(found.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillPartsTooSmall]);
    assert_eq!(found.diagnostics[0].at, Some(p(20.0, 0.0)));
    // Just over the limit is kept, and small.
    let small = TINY_PART.sqrt() * 1.01;
    let kept = region(&rings(&[&square(0.0, 0.0, small)]), FillRule::NonZero);
    assert_eq!(kept.region.parts.len(), 1);
    assert_eq!(kept.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::FillSmall]);
}

#[test]
fn diag_sc_w0303_parts_left_out_are_counted() {
    let tiny = TINY_PART.sqrt() * 0.9;
    assert_eq!(
        said(fill("specks", rings(&[&square(0.0, 0.0, 10.0), &square(20.0, 0.0, tiny), &square(30.0, 0.0, tiny)]), FillRule::NonZero)),
        [
            "warning SC-W0303: 2 parts of this fill cover 0.21 mm² or less each, too little for a row of stitches, so they are left out.",
            "warning SC-W0011: This element is a fill, and this version of StitchCraft does not sew fills yet, so it is skipped.",
        ]
    );
    assert_eq!(
        said(fill("speck", rings(&[&square(0.0, 0.0, 10.0), &square(20.0, 0.0, tiny)]), FillRule::NonZero))[0],
        "warning SC-W0303: 1 part of this fill covers 0.21 mm² or less, too little for a row of stitches, so it is left out."
    );
    assert_eq!(
        said(fill("dot", rings(&[&square(0.0, 0.0, 0.2)]), FillRule::NonZero))[0],
        "warning SC-W0303: This fill covers only 0.04 mm², 0.21 mm² or less, too little for a row of stitches, so it is left out."
    );
    assert_eq!(
        said(fill("dots", rings(&[&square(0.0, 0.0, 0.2), &square(5.0, 0.0, 0.2)]), FillRule::NonZero))[0],
        "warning SC-W0303: All 2 parts of this fill cover 0.21 mm² or less each, too little for a row of stitches, so they are left out."
    );
}

#[test]
fn diag_sc_w0304_a_small_fill_is_named_with_its_area() {
    assert_eq!(
        said(fill("small", rings(&[&square(0.0, 0.0, 1.0)]), FillRule::NonZero))[0],
        "warning SC-W0304: This fill covers 1 mm², less than 1.4 mm², so a running stitch round it or a satin column across it would \
         probably sew it better."
    );
}

#[test]
fn diag_sc_i0306_the_nonzero_rule_filling_what_the_even_odd_rule_leaves_empty_is_said() {
    let same = rings(&[&square(0.0, 0.0, 10.0), &square(3.0, 3.0, 4.0)]);
    assert_eq!(
        said(fill("filled", same.clone(), FillRule::NonZero))[0],
        "info SC-I0306: Subpaths of this fill wind round some of it twice the same way, which its fill rule (nonzero) fills and the \
         even-odd rule leaves empty. It stays in the fill, as the drawing shows it; Ink/Stitch would leave it empty."
    );
    // Under the even-odd rule, or with the hole drawn the other way round, there is nothing to say.
    assert_eq!(said(fill("even-odd", same, FillRule::EvenOdd)).len(), 1);
    assert_eq!(said(fill("turned", rings(&[&square(0.0, 0.0, 10.0), &turned(3.0, 3.0, 4.0)]), FillRule::NonZero)).len(), 1);
}

#[test]
fn diag_sc_w0307_a_fill_in_parts_is_said_to_be_sewn_part_by_part() {
    assert_eq!(
        said(fill("parts", rings(&[&square(0.0, 0.0, 10.0), &square(20.0, 0.0, 10.0)]), FillRule::NonZero)),
        [
            "warning SC-W0307: This fill's area falls into 2 parts that are sewn one after another, with a jump between each.",
            "warning SC-W0011: This element is a fill, and this version of StitchCraft does not sew fills yet, so it is skipped.",
        ]
    );
    // One part says nothing.
    assert_eq!(said(fill("one", rings(&[&square(0.0, 0.0, 10.0)]), FillRule::NonZero)).len(), 1);
}
