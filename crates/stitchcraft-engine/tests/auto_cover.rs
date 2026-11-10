//! A design digitized automatically covers the fabric whatever its colour (`REQ-AUTO-027` to
//! `REQ-AUTO-029`, `docs/src/design/image-to-embroidery.md` › Covering the fabric): no gap a thread wide
//! inside it, one base layer under all of it, and neighbours that overlap by 1 mm. The cases measure the
//! test pictures, the traced pear and pieces of the traced dogs as `tests/auto_fabric.rs` does, and small
//! shapes drawn here whose numbers can be read off. `tests/auto_photos.rs` measures the whole dogs.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, Point};
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

/// `element` with the settings `params` added.
fn with(mut element: Element, params: &[(&str, &str)]) -> Element {
    for (key, value) in params {
        element.params.set(*key, *value);
    }
    element
}

/// `design` digitized with `work` units of work for each step: the ids of its base layer's pieces, what is
/// said with `code`, and each other fill's `fill_underlay`.
fn based(design: &Design, work: u64, code: Code) -> (Vec<String>, Vec<String>, Vec<Option<String>>) {
    let (elements, said) = auto::digitized(design, &Budget { max_work: work, ..Budget::DEFAULT });
    let (base, rest): (Vec<&Element>, Vec<&Element>) = elements.iter().partition(|element| element.id.as_str().starts_with(BASE));
    let said: Vec<String> = said.iter().filter(|d| d.code == code).map(Diagnostic::to_string).collect();
    let underlays = rest.iter().map(|element| element.params.get("fill_underlay").map(str::to_string)).collect();
    (base.iter().map(|element| element.id.to_string()).collect(), said, underlays)
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
    // A column 4.4 mm wide of the dog's blaze ends under another: pushed on by the overlap of 1 mm, its last
    // stitches skewed and left 3.2 mm² bare at (41.1, 42.4).
    designs.push(("blaze".to_string(), pictures::dog_shapes("dog-running-6", &["vc:81:fill", "vc:66:fill", "vc:56:fill", "vc:38:fill"])));
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
    // The base layer's rows end 0.6 mm from the hole. The offset rounds the hole's corners, and the stitches
    // that travel along the hole's edge between rows stray up to 0.2 mm from it, a fill's travel tolerance,
    // so no base stitch comes nearer the hole than 0.4 mm, neither its ends nor its middle.
    let apart = |at: f64| (12.0 - at).max(at - 18.0).max(0.0);
    let near = |p: Point| stitchcraft_core::math::hypot(apart(p.x()), apart(p.y())) < INSET - 0.2;
    let nearer: Vec<[Point; 2]> = base
        .iter()
        .filter(|(_, stitch)| near(stitch.from) || near(stitch.to) || near(stitch.from.lerp(stitch.to, 0.5)))
        .map(|(_, stitch)| [stitch.from, stitch.to])
        .collect();
    assert!(nearer.is_empty(), "{nearer:?}");
    // The strip is narrower than twice the inset, so no base stitch lies on it, and the base layer ends 0.6
    // mm inside the square's sides. Where the strip leaves the right side the edge bulges towards it: the
    // outer shape, grown by 0.2 mm, is 1.4 mm wide there, and the place 0.8 mm from both of its corners at
    // x = 30.2 lies 0.387 mm to their left, the root of 0.8² − 0.7².
    let [left, right, top, bottom] = span(&base);
    let bulge = 30.2 - 0.387;
    assert!((left - INSET).abs() < 0.05 && (right - bulge).abs() < 0.05, "{left} to {right}");
    let past = |p: Point| p.x() > 30.0 - INSET + 0.05 && (p.y() - 15.0).abs() > 0.7;
    assert!(base.iter().all(|(_, stitch)| !past(stitch.from) && !past(stitch.to)), "{left} to {right}");
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

#[test]
fn diag_sc_i0909_says_the_base_layers_area_pieces_and_thread() {
    let work = Budget::DEFAULT.max_work;
    // 2 squares 20 mm a side, 10 mm apart: 2 pieces 18.8 mm a side, in red, which paints as much as blue and
    // is drawn first.
    let apart = design(vec![rect("red", (0.0, 0.0), (20.0, 20.0), &RED), rect("blue", (30.0, 0.0), (20.0, 20.0), &BLUE)]);
    let (base, said, underlays) = based(&apart, work, Code::AutoBase);
    assert_eq!(base, [BASE, "stitchcraft:base#2"]);
    assert_eq!(said, ["info SC-I0909: A base layer of 706.88 mm² in 2 pieces sews first under the design, in the thread #c80000."]);
    assert_eq!(underlays, [Some("false".to_string()), Some("false".to_string())]);
    let (_, diagnostics) = auto::digitized(&apart, &Budget::DEFAULT);
    let hint = diagnostics.iter().find(|d| d.code == Code::AutoBase).unwrap();
    assert!(format!("{:?}", hint.fix).contains("turn `auto_base_layer` off"), "{:?}", hint.fix);
    assert_eq!((Code::AutoBase.id(), hint.element.as_ref()), ("SC-I0909", None));
    // One square: 1 piece.
    let one = design(vec![rect("red", (0.0, 0.0), (20.0, 20.0), &RED)]);
    assert_eq!(
        based(&one, work, Code::AutoBase).1,
        ["info SC-I0909: A base layer of 353.44 mm² in 1 piece sews first under the design, in the thread #c80000."]
    );
    // A square 2 mm a side sews as a dot, filled. 0.6 mm inside its sides nothing is left that is as large
    // as the smallest area: it has no base layer, that is said, and it keeps the underlay of its settings.
    let dot = design(vec![rect("dot", (0.0, 0.0), (2.0, 2.0), &RED)]);
    let (base, said, underlays) = based(&dot, work, Code::AutoBase);
    let none =
        "info SC-I0909: No part of the design is wide enough for a base layer, so it sews none, and each fill sews the underlay of its settings.";
    assert_eq!((base, said, underlays), (Vec::new(), vec![none.to_string()], vec![None]));
    // An element that has the base layer's id already keeps it, and the base layer's steps aside.
    let named = design(vec![rect(BASE, (0.0, 0.0), (20.0, 20.0), &RED)]);
    let (elements, _) = auto::digitized(&named, &Budget::DEFAULT);
    assert_eq!(elements.iter().map(|element| element.id.as_str()).collect::<Vec<_>>(), ["stitchcraft:base~2", BASE]);
}

#[test]
fn diag_sc_w0910_a_base_layer_that_needs_more_work_than_the_budget_is_left_out() {
    let pair = design(vec![rect("red", (0.0, 0.0), (20.0, 20.0), &RED)]);
    // The least work that lays the base layer, found by halving. With less, where its shape is what needs
    // more, the design sews without one, that is said, and each fill keeps its underlay: with each of the
    // 64 budgets below the least, and with every 37th below those.
    let laid = |work: u64| !based(&pair, work, Code::AutoBase).0.is_empty();
    let (mut low, mut enough) = (0_u64, 1 << 20);
    assert!(laid(enough));
    while enough - low > 1 {
        let middle = (low + enough) / 2;
        if laid(middle) { enough = middle } else { low = middle }
    }
    let mut short = 0;
    for work in (1..enough).filter(|work| enough - work <= 64 || work % 37 == 0) {
        let (base, said, underlays) = based(&pair, work, Code::AutoBaseLeftOut);
        if said.is_empty() {
            continue;
        }
        short += 1;
        let wanted = format!(
            "warning SC-W0910: Finding the shape of the base layer needed more than the work budget of {work} units, so the design sews no base layer, and each fill sews the underlay of its settings."
        );
        assert_eq!((base, said), (Vec::new(), vec![wanted]), "{work}");
        assert!(underlays.iter().all(|underlay| underlay.as_deref() != Some("false")), "{work}: {underlays:?}");
    }
    assert!(short > 0, "no budget up to {enough} leaves the base layer out");
    assert_eq!(Code::AutoBaseLeftOut.id(), "SC-W0910");
}

#[test]
fn req_auto_028_the_base_layer_goes_by_the_settings_of_the_lowest_shape() {
    let work = Budget::DEFAULT.max_work;
    let ring: [&[(f64, f64)]; 2] = [&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)], &[(12.0, 12.0), (12.0, 18.0), (18.0, 18.0), (18.0, 12.0)]];
    let holed =
        |settings: &[(&str, &str)]| design(vec![with(fill("holed", &ring, &RED, &[]), settings), rect("beside", (30.0, 0.0), (10.0, 30.0), &BLUE)]);
    // Off: no base layer, nothing said of one, and the fills keep their underlays.
    let (base, said, underlays) = based(&holed(&[("auto_base_layer", "false")]), work, Code::AutoBase);
    assert_eq!((base, said, underlays), (Vec::new(), Vec::new(), vec![None, None]));
    // The setting of a shape above the lowest isn't the design's.
    let above = design(vec![
        rect("low", (0.0, 0.0), (20.0, 20.0), &RED),
        with(rect("high", (20.0, 0.0), (20.0, 20.0), &BLUE), &[("auto_base_layer", "false")]),
    ]);
    assert_eq!(based(&above, work, Code::AutoBase).0, [BASE]);
    // The area of the base layer, as SC-I0909 gives it.
    let area = |settings: &[(&str, &str)]| -> f64 {
        let said = based(&holed(settings), work, Code::AutoBase).1;
        let said = said[0].strip_prefix("info SC-I0909: A base layer of ").unwrap();
        said.split(' ').next().unwrap().parse().unwrap()
    };
    // 40 by 30 mm with a hole 6 mm a side. The outer shape is grown by 0.2 mm and then moved in by 0.8 mm,
    // which leaves 38.8 by 28.8 mm and a hole 7.2 mm a side with corners rounded at 0.8 mm. Without the
    // hole, the whole rectangle.
    let rounded = |radius: f64| (4.0 - std::f64::consts::PI) * radius * radius;
    assert!((area(&[]) - (38.8 * 28.8 - 7.2 * 7.2 + rounded(0.8))).abs() < 0.02, "{}", area(&[]));
    let whole = area(&[("auto_base_keep_holes", "false")]);
    assert!((whole - 38.8 * 28.8).abs() < 0.01, "{whole}");
    // An offset of -1 mm lies 1 mm inside, and one of 0.5 mm that far outside, its corners rounded.
    let inside = area(&[("auto_base_offset_mm", "-1")]);
    assert!((inside - (38.0 * 28.0 - 8.0 * 8.0 + rounded(1.2))).abs() < 0.02, "{inside}");
    let outside = area(&[("auto_base_offset_mm", "0.5"), ("auto_base_keep_holes", "false")]);
    assert!((outside - (41.0 * 31.0 - rounded(0.5))).abs() < 0.02, "{outside}");
    // A stitch length of 2 mm: rows 2 mm times √3/2 apart, in the underlay too.
    let shorter = holed(&[("auto_base_stitch_length_mm", "2")]);
    let (elements, _) = auto::digitized(&shorter, &Budget::DEFAULT);
    let base = elements.iter().find(|element| element.id.as_str() == BASE).unwrap();
    for (key, value) in [
        ("max_stitch_length_mm", "2"),
        ("row_spacing_mm", "1.7321"),
        ("fill_underlay_max_stitch_length_mm", "2"),
        ("fill_underlay_row_spacing_mm", "1.7321"),
    ] {
        assert_eq!(base.params.get(key), Some(value), "{key}");
    }
}

#[test]
fn req_auto_028_a_piece_with_no_room_for_a_row_and_a_design_of_columns_have_no_base_layer() {
    let work = Budget::DEFAULT.max_work;
    // A square 20 mm a side, and 2 small ones apart from it that sew as dots, filled. The one 3.7 mm a side
    // leaves a piece 2.5 mm a side, on which a disc as wide as the row spacing, 2.6 mm, doesn't fit: it is
    // left out. The one 3.9 mm a side leaves a piece 2.7 mm a side, which sews.
    let dots = design(vec![
        rect("big", (0.0, 0.0), (20.0, 20.0), &RED),
        rect("small", (30.0, 0.0), (3.7, 3.7), &RED),
        rect("larger", (40.0, 0.0), (3.9, 3.9), &RED),
    ]);
    let (base, said, _) = based(&dots, work, Code::AutoBase);
    assert_eq!(base, [BASE, "stitchcraft:base#2"]);
    assert_eq!(said, ["info SC-I0909: A base layer of 360.73 mm² in 2 pieces sews first under the design, in the thread #c80000."]);
    let (elements, _) = auto::digitized(&dots, &Budget::DEFAULT);
    let second = elements.iter().find(|element| element.id.as_str() == "stitchcraft:base#2").unwrap();
    let xs: Vec<f64> = second.shape.path().points().map(|point| point.x()).collect();
    assert!(xs.iter().all(|x| (40.6 - 1e-6..=43.3 + 1e-6).contains(x)), "{xs:?}");
    // A band 3 mm wide alone sews as a satin column with the underlay of its width: no base layer, and
    // nothing is said of one. Beside a square it is part of the design's outer shape, but its piece, 1.8 mm
    // wide, has no room for a row.
    let band = || rect("band", (0.0, 30.0), (30.0, 3.0), &RED);
    assert_eq!(based(&design(vec![band()]), work, Code::AutoBase), (Vec::new(), Vec::new(), vec![None]));
    let beside = design(vec![rect("big", (0.0, 0.0), (20.0, 20.0), &RED), band()]);
    assert_eq!(based(&beside, work, Code::AutoBase).0, [BASE]);
}

#[test]
fn req_auto_029_a_column_reaches_under_a_neighbour_that_runs_along_more_than_half_of_its_rail() {
    // A strip 3 mm wide and 30 mm long, a satin column along y, and a blue one beside it from y = 0, sewn
    // after it: `along` mm of the red column's right rail.
    let reach = |along: f64| {
        let strips = design(vec![rect("red", (0.0, 0.0), (3.0, 30.0), &RED), rect("blue", (3.0, 0.0), (3.0, along), &BLUE)]);
        let (elements, said) = auto::digitized(&strips, &Budget::DEFAULT);
        let red = elements.iter().find(|element| element.id.as_str() == "red").unwrap();
        let said: Vec<String> = said.iter().filter(|d| d.code == Code::AutoUnder).map(|d| d.message.clone()).collect();
        (red.params.get("pull_compensation_mm").map(str::to_string), said)
    };
    // Along 14 mm, less than half, the column keeps its pull compensation on both rails. Along 16 mm it
    // sews 1 mm past the rail beside the rectangle, and 0.4 mm past the other.
    let (pull, said) = reach(14.0);
    assert_eq!(pull.as_deref(), Some("0.4"));
    assert!(said.iter().all(|message| !message.contains("past a rail")), "{said:?}");
    let (pull, said) = reach(16.0);
    assert!(matches!(pull.as_deref(), Some("1 0.4" | "0.4 1")), "{pull:?}");
    assert!(said.iter().any(|message| message.ends_with("1 satin column sews 1 mm past a rail, under the shape sewn after it there.")), "{said:?}");
    // 2 columns with such a neighbour, one with an overlap of its own.
    let two = design(vec![
        rect("red", (0.0, 0.0), (3.0, 30.0), &RED),
        rect("blue", (3.0, 0.0), (3.0, 30.0), &BLUE),
        with(rect("gold", (40.0, 0.0), (3.0, 30.0), &RED), &[("auto_overlap_mm", "0.8")]),
        rect("navy", (43.0, 0.0), (3.0, 30.0), &BLUE),
    ]);
    let (_, said) = auto::digitized(&two, &Budget::DEFAULT);
    let said: Vec<&str> = said.iter().filter(|d| d.code == Code::AutoUnder).map(|d| d.message.as_str()).collect();
    assert!(
        said.iter().any(|message| message.ends_with("2 satin columns sew 0.8 to 1 mm past a rail, under the shapes sewn after them there.")),
        "{said:?}"
    );
}
