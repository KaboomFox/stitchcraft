//! A design digitized automatically covers the fabric whatever its colour (`REQ-AUTO-027` to
//! `REQ-AUTO-029`, `docs/src/design/image-to-embroidery.md` › Covering the fabric): no gap a thread wide
//! inside it, one base layer under all of it, and neighbours that overlap by 1 mm. The cases measure the
//! test pictures, the traced pear and pieces of the traced dogs as `tests/auto_fabric.rs` does, and small
//! shapes drawn here whose numbers can be read off. `tests/auto_photos.rs` measures the whole dogs.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_plan::{Role, SewnStitch, StitchPlan, Thread};
use stitchcraft_testkit::designs::{BLUE, RED, automatic, fill, in_place};
use stitchcraft_testkit::fabric::{self, BARE, GAP};
use stitchcraft_testkit::pictures::{self, Trace};

/// The base layer's element.
const BASE: &str = "stitchcraft:base";
/// How far the base layer lies inside the design's outer shape by default, in millimetres.
const INSET: f64 = 0.6;
/// The overlap by default, in millimetres.
const OVERLAP: f64 = 1.0;

/// A design of automatic `elements`.
fn design(elements: Vec<Element>) -> Design {
    Design::new(elements.into_iter().map(automatic).collect(), DesignSettings::default()).unwrap()
}

/// A rectangle from (`x`, `y`), `wide` by `tall`, as a fill in `thread`.
fn rect(id: &str, (x, y): (f64, f64), (wide, tall): (f64, f64), thread: &Thread) -> Element {
    fill(id, &[&[(x, y), (x + wide, y), (x + wide, y + tall), (x, y + tall)]], thread, &[])
}

/// `design`'s plan with its stitches where it draws its shapes.
fn plan(design: &Design) -> StitchPlan {
    let outcome = in_place(design);
    outcome.plan.unwrap_or_else(|| panic!("{:?}", outcome.diagnostics))
}

/// The stitches of `plan` that the element `id` sews, its parts' among them (`id#2` and so on), locks left
/// out, each with its place among the plan's stitches.
fn stitches(plan: &StitchPlan, id: &str) -> Vec<(usize, SewnStitch)> {
    let named = |stitch: &SewnStitch| {
        let element = plan.blocks.get(stitch.block)?.stitches.get(stitch.index)?.origin.element?;
        Some(plan.element(element)?.to_string())
    };
    let own = |name: String| name == id || name.strip_prefix(id).is_some_and(|rest| rest.starts_with('#'));
    plan.sewn_stitches().into_iter().enumerate().filter(|(_, stitch)| stitch.role != Role::Lock && named(stitch).is_some_and(own)).collect()
}

/// The least and greatest x and y of `stitches`' needle points.
fn span(stitches: &[(usize, SewnStitch)]) -> [f64; 4] {
    let points: Vec<Point> = stitches.iter().flat_map(|(_, stitch)| [stitch.from, stitch.to]).collect();
    let (xs, ys): (Vec<f64>, Vec<f64>) = points.iter().map(|point| (point.x(), point.y())).unzip();
    let (least, most) = (|of: &[f64]| of.iter().copied().fold(f64::MAX, f64::min), |of: &[f64]| of.iter().copied().fold(f64::MIN, f64::max));
    [least(&xs), most(&xs), least(&ys), most(&ys)]
}

/// Each test picture, the traced pear and the pieces of the traced dogs that `tests/auto_fabric.rs`
/// measures, by name.
fn pictures() -> Vec<(String, Design)> {
    let mut designs: Vec<(String, Design)> = pictures::all()
        .into_iter()
        .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        .map(|picture| (picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap()))
        .collect();
    designs.push(("pear".to_string(), pictures::pear()));
    designs.push(("paw".to_string(), pictures::dog_shapes("dog-on-turf-6", &["vc:69:fill"])));
    designs.push(("ear".to_string(), pictures::dog_shapes("dog-running-6", &["vc:5:fill", "vc:6:fill", "vc:28:fill", "vc:50:fill"])));
    designs
}

#[test]
fn req_auto_027_digitized_pictures_leave_no_gap_a_thread_wide_and_little_bare() {
    let mut found = Vec::new();
    for (name, design) in pictures() {
        let cover = fabric::cover(&design, &plan(&design), GAP);
        if cover.gaps > 0 || cover.bare_share() > BARE {
            found.push(format!(
                "{name}: {} gaps of {:.2} mm², the largest {:.3} mm² at {:?}; bare {:.2} of {:.0} mm², {:.3} %, most in {:?}",
                cover.gaps,
                cover.in_gaps,
                cover.largest,
                cover.at,
                cover.bare,
                cover.design,
                100.0 * cover.bare_share(),
                cover.shapes.first()
            ));
        }
    }
    assert!(found.is_empty(), "{found:#?}");
}

#[test]
fn req_auto_027_on_black_fabric_nothing_shows_through_a_gap_between_neighbours() {
    // A white square beside a black one, as a blaze beside a nose: whatever the fabric's colour, no thread
    // of any role leaves a gap where they meet, and what is open is no more than what is bare.
    let white = Thread::new(stitchcraft_plan::Rgb::new(240, 240, 240));
    let black = Thread::new(stitchcraft_plan::Rgb::new(0, 0, 0));
    let design = design(vec![rect("blaze", (0.0, 0.0), (12.0, 12.0), &white), rect("nose", (12.0, 2.0), (8.0, 8.0), &black)]);
    let plan = plan(&design);
    let cover = fabric::cover(&design, &plan, GAP);
    assert_eq!((cover.gaps, cover.open_gaps), (0, 0), "{cover:?}");
    assert!(cover.open <= cover.bare, "{cover:?}");
    // The base layer lies under both, so the fabric is covered twice where they meet: by the square sewn
    // first, which reaches under the other, and by the base layer.
    let base = stitches(&plan, BASE);
    assert!(
        base.iter().any(|(_, stitch)| stitch.from.x().min(stitch.to.x()) < 12.0 && stitch.from.x().max(stitch.to.x()) > 12.0),
        "no base stitch crosses x = 12"
    );
}

#[test]
fn req_auto_028_one_base_layer_sews_first_under_the_designs_outer_shape() {
    // 2 rectangles that meet at x = 20, red 20 by 20 mm and blue 30 by 20 mm: one outer shape, 50 by 20 mm.
    let pair = design(vec![rect("red", (0.0, 0.0), (20.0, 20.0), &RED), rect("blue", (20.0, 0.0), (30.0, 20.0), &BLUE)]);
    let plan = plan(&pair);
    let base = stitches(&plan, BASE);
    assert!(!base.is_empty(), "the plan has no element {BASE}: {:?}", plan.elements);
    // Every stitch of it is underlay, and it sews before every stitch of the shapes.
    assert!(base.iter().all(|(_, stitch)| stitch.role == Role::Underlay));
    let last = base.iter().map(|(at, _)| *at).max().unwrap();
    let first = ["red", "blue"].iter().flat_map(|id| stitches(&plan, id)).map(|(at, _)| at).min().unwrap();
    assert!(last < first, "the base layer's last stitch is the plan's {last}th, and a shape's first its {first}th");
    // It lies inside the outer shape by 0.6 mm and reaches that far on every side: its rows end on its
    // outline, level ones on the left and right sides and those at 60° on all 4.
    let [left, right, top, bottom] = span(&base);
    for (side, at, wanted) in [("left", left, INSET), ("right", right, 50.0 - INSET), ("top", top, INSET), ("bottom", bottom, 20.0 - INSET)] {
        assert!((at - wanted).abs() < 0.05, "the base layer's {side} side lies at {at:.2}, not {wanted:.2}");
    }
    // It is one shape across the edge where the rectangles meet: its stitches cross that edge.
    assert!(base.iter().any(|(_, stitch)| stitch.from.x().min(stitch.to.x()) < 20.0 && stitch.from.x().max(stitch.to.x()) > 20.0));
    // Its thread paints the most: blue, 600 mm² to red's 400.
    let block = plan.blocks.get(base[0].1.block).unwrap();
    assert_eq!(block.thread.color, BLUE.color);
    // Digitized, it is a tatami fill with the parameters of Ink/Stitch's Knockdown Fill at a stitch length of
    // 3 mm, and the 2 fills on it sew no underlay of their own.
    let (elements, _) = auto::digitized(&pair, &Budget::DEFAULT);
    let element = elements.iter().find(|element| element.id.to_string() == BASE).unwrap();
    let wanted = [
        ("max_stitch_length_mm", "3"),
        ("row_spacing_mm", "2.5981"),
        ("staggers", "2"),
        ("underpath", "false"),
        ("fill_underlay", "true"),
        ("fill_underlay_angle", "60 -60"),
        ("fill_underlay_max_stitch_length_mm", "3"),
        ("fill_underlay_row_spacing_mm", "2.5981"),
        ("underlay_underpath", "false"),
    ];
    for (key, value) in wanted {
        assert_eq!(element.params.get(key), Some(value), "{key}");
    }
    assert_eq!(elements.first().map(|element| element.id.to_string()).as_deref(), Some(BASE));
    for id in ["red", "blue"] {
        let fill = elements.iter().find(|element| element.id.to_string() == id).unwrap();
        assert_eq!(fill.params.get("fill_underlay"), Some("false"), "{id}");
    }
}

#[test]
fn req_auto_028_the_base_layer_keeps_a_hole_and_leaves_a_part_narrower_than_twice_its_inset() {
    // A square 30 mm a side with a hole 6 mm a side, and a strip 1 mm wide and 10 mm long from its right
    // side: Ink/Stitch's knockdown at an offset of -1 mm grows such a hole to 8 mm.
    let ring: [&[(f64, f64)]; 2] = [&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)], &[(12.0, 12.0), (12.0, 18.0), (18.0, 18.0), (18.0, 12.0)]];
    let holed = design(vec![fill("holed", &ring, &RED, &[]), rect("strip", (30.0, 14.5), (10.0, 1.0), &RED)]);
    let plan = plan(&holed);
    let base = stitches(&plan, BASE);
    assert!(!base.is_empty(), "the plan has no element {BASE}: {:?}", plan.elements);
    // No base stitch comes nearer the hole than 0.6 mm, neither its ends nor its middle. The offset rounds the
    // hole's corners, and a stitch along a rounded corner cuts it by 0.05 mm at most.
    let apart = |at: f64| (12.0 - at).max(at - 18.0).max(0.0);
    let near = |p: Point| stitchcraft_core::math::hypot(apart(p.x()), apart(p.y())) < INSET - 0.05;
    assert!(base.iter().all(|(_, stitch)| !near(stitch.from) && !near(stitch.to) && !near(stitch.from.lerp(stitch.to, 0.5))));
    // The strip is narrower than twice the inset, so the base layer ends 0.6 mm inside the square's side.
    let [left, right, top, bottom] = span(&base);
    assert!((left - INSET).abs() < 0.05 && (right - (30.0 - INSET)).abs() < 0.05, "{left} to {right}");
    assert!((top - INSET).abs() < 0.05 && (bottom - (30.0 - INSET)).abs() < 0.05, "{top} to {bottom}");
}

#[test]
fn req_auto_029_an_area_reaches_1_mm_under_the_neighbour_sewn_after_it() {
    let stripes = pictures::stripes();
    let stripes = Design::new(stripes.elements, DesignSettings::default()).unwrap();
    let plan = plan(&stripes);
    // Each pair of stripes that meet, with the x of their edge: the one sewn first reaches 1 mm past the
    // edge, and the one sewn after it ends its rows at the edge.
    for (left, right, edge) in [("red", "blue", 15.0), ("blue", "gold", 30.0)] {
        let (a, b) = (stitches(&plan, left), stitches(&plan, right));
        let (past_left, past_right) = (span(&a)[1] - edge, edge - span(&b)[0]);
        let left_first = a.first().unwrap().0 < b.first().unwrap().0;
        let (under, over) = if left_first { (past_left, past_right) } else { (past_right, past_left) };
        assert!((under - OVERLAP).abs() < 0.1, "{left} and {right}: the stripe sewn first reaches {under:.2} mm under the other");
        assert!(over < 0.05, "{left} and {right}: the stripe sewn after reaches {over:.2} mm past the edge");
    }
}

#[test]
fn req_auto_029_a_satin_column_reaches_1_mm_under_the_neighbour_sewn_after_it_and_no_wider_elsewhere() {
    // 2 strips 3 mm wide and 30 mm long that meet at x = 3: each sews as a satin column along y.
    let strips = design(vec![rect("red", (0.0, 0.0), (3.0, 30.0), &RED), rect("blue", (3.0, 0.0), (3.0, 30.0), &BLUE)]);
    let plan = plan(&strips);
    let (red, blue) = (stitches(&plan, "red"), stitches(&plan, "blue"));
    let ([red_left, red_right, ..], [blue_left, blue_right, ..]) = (span(&red), span(&blue));
    let red_first = red.first().unwrap().0 < blue.first().unwrap().0;
    // Past its outer side each column reaches its pull compensation, 0.4 mm, whichever sews first.
    assert!((-red_left - 0.4).abs() < 0.1 && (blue_right - 6.4).abs() < 0.1, "{red_left} and {blue_right}");
    // Where they meet, the one sewn first reaches 1 mm under the other, which sews 0.4 mm over it.
    let (under, over) = if red_first { (red_right - 3.0, 3.0 - blue_left) } else { (3.0 - blue_left, red_right - 3.0) };
    assert!((under - OVERLAP).abs() < 0.1, "the column sewn first reaches {under:.2} mm under the other");
    assert!((over - 0.4).abs() < 0.1, "the column sewn after reaches {over:.2} mm over the first");
}
