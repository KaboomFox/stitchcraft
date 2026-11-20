//! Thin lines merged into their neighbours (`REQ-AUTO-090`, M13.18): a part of an automatic area narrower
//! than the narrowest satin stroke everywhere and ragged, as soft shading and fur trace into, merges into the
//! automatic area it shares the longest edge with, and an even line, a line beside no area and the lines of a
//! shape that turns merging off sew as lines.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::auto::{MIN_LIKENESS, as_merged, digitized};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::normalize::region;
use stitchcraft_plan::Thread;
use stitchcraft_testkit::designs::{BLUE, GREEN, RED, automatic, fill, unbased};
use stitchcraft_testkit::pictures::GOLD;

/// An automatic rectangle `id` from `(x0, y0)` to `(x1, y1)` in `thread`.
fn rect(id: &str, (x0, y0): (f64, f64), (x1, y1): (f64, f64), thread: &Thread) -> Element {
    automatic(fill(id, &[&[(x0, y0), (x1, y0), (x1, y1), (x0, y1)]], thread, &[]))
}

/// A gold line 10 mm long from `x` = 10 to the right, straight on its left and ragged on its right: 0.4 mm
/// wide every millimetre and 0.7 mm wide half way between, 0.55 mm on average and 5.5 mm² large, as a strand
/// of fur traces. `params` are its settings.
fn ragged(id: &str, params: &[(&str, &str)]) -> Element {
    strand(id, 10.0, 20, &GOLD, params)
}

/// A line as [`ragged`] draws it, from `x` = `left` to the right, `steps` half millimetres long and in
/// `thread`.
fn strand(id: &str, left: f64, steps: u32, thread: &Thread, params: &[(&str, &str)]) -> Element {
    jagged(id, left, steps, (0.4, 0.7), thread, params)
}

/// A line as [`strand`] draws it, `narrow` wide every millimetre and `wide` half way between.
fn jagged(id: &str, left: f64, steps: u32, (narrow, wide): (f64, f64), thread: &Thread, params: &[(&str, &str)]) -> Element {
    let long = f64::from(steps) * 0.5;
    let mut ring = vec![(left, 0.0), (left, long)];
    ring.extend((0..=steps).map(|i| (left + if i % 2 == 0 { narrow } else { wide }, long - f64::from(i) * 0.5)));
    automatic(fill(id, &[&ring], thread, params))
}

/// The line [`ragged`] draws, whose colour keeps no share of its parts, so that it merges wherever it
/// shares an edge (REQ-AUTO-095).
fn merging(id: &str) -> Element {
    ragged(id, &[(MIN_LIKENESS, "0")])
}

/// The elements `elements` are sewn as, in a design with the default settings, and what is said of them.
fn sewn(elements: Vec<Element>) -> (Vec<Element>, Vec<Diagnostic>) {
    let design = Design::new(unbased(elements), DesignSettings::default()).unwrap();
    let (out, said) = digitized(&design, &Budget::DEFAULT);
    (out.into_owned(), said)
}

/// The ids of `elements` and whether each is a fill, in order.
fn kinds(elements: &[Element]) -> Vec<(&str, bool)> {
    elements.iter().map(|element| (element.id.as_str(), matches!(element.shape, Shape::Fill { .. }))).collect()
}

/// The area of the fill named `id` among `elements`, in square millimetres.
fn area(elements: &[Element], id: &str) -> f64 {
    let element = elements.iter().find(|element| element.id.as_str() == id).unwrap();
    let Shape::Fill { path, rule, .. } = &element.shape else { panic!("`{id}` is no fill") };
    region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region.area()
}

/// What is said with `code` among `said`.
fn said(said: &[Diagnostic], code: Code) -> Vec<String> {
    said.iter().filter(|d| d.code == code).map(ToString::to_string).collect()
}

#[test]
fn diag_sc_i0915_a_ragged_thin_line_merges_into_the_area_it_shares_the_longest_edge_with() {
    // The line runs along `left` for 10 mm and touches `right` only at its widest places. `left` reaches
    // under nothing, so that it sews what it merged and no more.
    let left = automatic(fill("left", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]], &RED, &[("auto_overlap_mm", "0")]));
    let (out, diagnostics) = sewn(vec![left, merging("line"), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)]);
    assert_eq!(kinds(&out), [("left", true), ("right", true)]);
    assert!((area(&out, "left") - 105.5).abs() < 1e-6, "{}", area(&out, "left"));
    assert_eq!(
        said(&diagnostics, Code::AutoLineMerged),
        ["info SC-I0915: A line of this area, of 5.5 mm², is narrower than the narrowest satin stroke, so it is merged into `left`, which \
          it shares the longest edge with."]
    );
}

#[test]
fn diag_sc_i0923_the_other_style_would_sew_the_ragged_line_otherwise() {
    // The line of the case above merges in the photo style, the defaults, and the logo style keeps it.
    let design = || vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), merging("line"), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)];
    let (_, merged) = sewn(design());
    // Planned again, it says the same.
    assert_eq!(sewn(design()).1, merged);
    assert_eq!(
        said(&merged, Code::AutoStyle),
        ["info SC-I0923: The logo style (`--style logo`) would sew this design otherwise: 1 thin line merged into its neighbour would \
          sew as a line."]
    );
    let logo = [("auto_prefer", "satin"), ("auto_fill_uneven", "false"), (MIN_LIKENESS, "0")];
    let area =
        |id: &str, (x0, x1): (f64, f64), thread: &Thread| automatic(fill(id, &[&[(x0, 0.0), (x1, 0.0), (x1, 10.0), (x0, 10.0)]], thread, &logo));
    let (out, kept) = sewn(vec![area("left", (0.0, 10.0), &RED), ragged("line", &logo), area("right", (10.7, 20.0), &BLUE)]);
    assert!(kinds(&out).contains(&("line", false)), "{:?}", kinds(&out));
    let notice = kept.iter().find(|said| said.code == Code::AutoStyle).unwrap();
    assert_eq!(
        notice.to_string(),
        "info SC-I0923: The photo style (`--style photo`) would sew this design otherwise: 1 thin line sewn as a line may merge into its \
         neighbour."
    );
    assert_eq!(
        notice.fix,
        Some(Fix::Hint(
            "`--style photo` sets `auto_prefer` to `balanced` and `auto_fill_uneven` to `true` on every automatic shape. In VectorCraft, set \
             them in the dialog of StitchCraft's Auto effect."
                .to_string()
        ))
    );
}

#[test]
fn req_auto_090_an_even_thin_line_sews_as_a_line() {
    // Drawn with an even pen, 0.5 mm wide all along.
    let (out, diagnostics) = sewn(vec![
        rect("left", (0.0, 0.0), (10.0, 10.0), &RED),
        rect("line", (10.0, 0.0), (10.5, 10.0), &GOLD),
        rect("right", (10.5, 0.0), (20.0, 10.0), &BLUE),
    ]);
    assert!(kinds(&out).contains(&("line", false)), "{:?}", kinds(&out));
    assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
}

#[test]
fn req_auto_090_a_shape_that_turns_merging_off_or_names_its_stitch_keeps_its_lines() {
    for params in [[("auto_fill_uneven", "false")], [("auto_stitch", "run")]] {
        let (out, diagnostics) =
            sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), ragged("line", &params), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)]);
        assert!(kinds(&out).contains(&("line", false)), "{params:?}: {:?}", kinds(&out));
        assert!(said(&diagnostics, Code::AutoLineMerged).is_empty());
    }
}

#[test]
fn req_auto_090_a_thin_line_beside_no_area_sews_as_a_line() {
    // Whatever share of the likeness merging keeps (REQ-AUTO-095): a part of its own beside no area costs none.
    for share in ["0", "1"] {
        let (out, diagnostics) = sewn(vec![ragged("line", &[(MIN_LIKENESS, share)]), rect("far", (30.0, 0.0), (40.0, 10.0), &RED)]);
        assert!(kinds(&out).contains(&("line", false)), "{share}: {:?}", kinds(&out));
        assert!(said(&diagnostics, Code::AutoLineMerged).is_empty(), "{share}");
    }
}

#[test]
fn req_auto_090_the_design_as_merged_draws_a_merged_line_in_its_neighbours_thread() {
    let design = Design::new(
        unbased(vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), merging("line"), rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)]),
        DesignSettings::default(),
    )
    .unwrap();
    let merged = as_merged(&design, &Budget::DEFAULT).unwrap();
    let drawn: Vec<(&str, Thread)> = merged.elements().iter().map(|element| (element.id.as_str(), element.thread.clone())).collect();
    assert_eq!(drawn, [("left", RED), ("line", GOLD), ("right", BLUE), ("left~line1", RED)]);
    assert!((area(merged.elements(), "left~line1") - 5.5).abs() < 1e-6);
    // A design whose lines don't merge comes back as it is.
    let even = Design::new(unbased(vec![rect("a", (0.0, 0.0), (10.0, 10.0), &RED)]), DesignSettings::default()).unwrap();
    assert_eq!(as_merged(&even, &Budget::DEFAULT).unwrap().elements(), even.elements());
}

/// A row of areas 10 mm tall, red and blue by turns, with a line between each 2: the line `id` of
/// `lines`, `steps` half millimetres long in `thread` with `auto_min_likeness` `share`, from the bottom along
/// the area on its left. Each area is 9.3 mm wide.
fn row(lines: &[(&str, u32, &Thread, &str)]) -> Vec<Element> {
    let mut elements = vec![rect("area0", (0.0, 0.0), (10.0, 10.0), &RED)];
    for (at, &(id, steps, thread, share)) in (1_u32..).zip(lines) {
        let left = 10.0 * f64::from(at);
        elements.push(strand(id, left, steps, thread, &[(MIN_LIKENESS, share)]));
        let colour = if at % 2 == 1 { &BLUE } else { &RED };
        elements.push(rect(&format!("area{at}"), (left + 0.7, 0.0), (left + 10.0, 10.0), colour));
    }
    elements
}

/// The ids of the lines of `elements` that sew as lines, the others merged.
fn lines_kept(elements: &[Element]) -> Vec<&str> {
    kinds(elements).into_iter().filter(|(id, fill)| !fill && id.starts_with("line")).map(|(id, _)| id).collect()
}

#[test]
fn req_auto_095_a_thin_line_that_is_a_whole_part_of_its_colour_sews_as_a_line() {
    // The line is gold's one part: merged, the picture's likeness would fall to 2 thirds. By default, at 0.85,
    // it sews as a line, and at 0 it merges.
    let line = |share| ragged("line", &[(MIN_LIKENESS, share)]);
    let elements = |line| vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), line, rect("right", (10.7, 0.0), (20.0, 10.0), &BLUE)];
    let by_default = sewn(elements(ragged("line", &[]))).0;
    assert_eq!(kinds(&by_default), [("left", true), ("right", true), ("line", false)]);
    assert_eq!(kinds(&sewn(elements(line("0"))).0), [("left", true), ("right", true)]);
    let (out, diagnostics) = sewn(elements(line("0.85")));
    assert_eq!(kinds(&out), [("left", true), ("right", true), ("line", false)]);
    let line = diagnostics.iter().find(|said| said.code == Code::AutoLineMerged).unwrap();
    assert_eq!(
        (line.to_string(), line.element.as_ref().map(ToString::to_string), format!("{:?}", line.fix)),
        (
            "info SC-I0915: A line of this area, of 5.5 mm², is narrower than the narrowest satin stroke, but sews as a line: it is a part of \
             its colour by itself, and merged it would take the picture's likeness under 0.85 (`auto_min_likeness`)."
                .to_string(),
            Some("line".to_string()),
            "Some(Hint(\"To merge it, lower `auto_min_likeness`.\"))".to_string()
        )
    );
}

#[test]
fn req_auto_095_a_thin_line_that_touches_a_piece_of_its_colour_merges() {
    // Gold keeps its square, a little larger, whatever the share kept.
    let gold = rect("gold", (10.7, 0.0), (20.0, 10.0), &GOLD);
    let (out, _) = sewn(vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), ragged("line", &[(MIN_LIKENESS, "1")]), gold]);
    assert_eq!(kinds(&out), [("left", true), ("gold", true)]);
}

#[test]
fn req_auto_095_the_smallest_lines_merge_first_while_the_picture_keeps_the_strictest_share() {
    // 3 colours, gold's 4 parts the 4 lines: each merged costs gold a quarter of its likeness, and the picture a
    // twelfth. So 3 merge at 0.75 and 2 at 0.8, and of 2 shapes' shares the larger rules.
    for (shares, kept) in [(["0.75"; 4], vec!["line10"]), (["0.75", "0.75", "0.75", "0.8"], vec!["line9", "line10"])] {
        let lines = [("line9", 18), ("line7", 14), ("line10", 20), ("line8", 16)];
        let gold = GOLD;
        let lines: Vec<_> = lines.into_iter().zip(shares).map(|((id, steps), share)| (id, steps, &gold, share)).collect();
        let (out, _) = sewn(row(&lines));
        assert_eq!(lines_kept(&out), kept, "{shares:?}");
    }
}

#[test]
fn req_auto_095_a_colour_with_more_parts_gives_up_its_lines_first() {
    // 4 colours: gold's 3 lines cost a third of gold's likeness each, the green line all of green's. At 0.75
    // the picture may lose 1 colour's worth: the 3 gold lines go, though the green line is shorter.
    let (out, _) =
        sewn(row(&[("line1", 20, &GOLD, "0.75"), ("line2", 10, &GREEN, "0.75"), ("line3", 20, &GOLD, "0.75"), ("line4", 20, &GOLD, "0.75")]));
    assert_eq!(lines_kept(&out), ["line2"]);
}

#[test]
fn req_auto_095_lines_of_a_colour_that_touch_are_one_part() {
    // 2 gold lines side by side are one part, and a gold square far off another: the 2 lines merge, at a cost of
    // half of gold's likeness, a sixth of the picture's.
    let share = [(MIN_LIKENESS, "0.8")];
    let elements = vec![
        rect("left", (0.0, 0.0), (10.0, 10.0), &RED),
        ragged("line1", &share),
        strand("line2", 10.7, 20, &GOLD, &share),
        rect("right", (11.4, 0.0), (20.0, 10.0), &BLUE),
        rect("far", (40.0, 0.0), (50.0, 10.0), &GOLD),
    ];
    let (out, _) = sewn(elements);
    assert_eq!(lines_kept(&out), Vec::<&str>::new());
}

#[test]
fn req_auto_095_lines_of_2_colours_that_touch_are_2_parts() {
    // 4 colours, a gold line and a shorter green one side by side, each all of its colour. At 0.75 the picture
    // may lose 1 colour's worth: the smaller, green, merges, and gold stays.
    let elements = vec![
        rect("left", (0.0, 0.0), (10.0, 10.0), &RED),
        ragged("line1", &[(MIN_LIKENESS, "0.75")]),
        strand("line2", 10.7, 16, &GREEN, &[(MIN_LIKENESS, "0.75")]),
        rect("right", (11.4, 0.0), (20.0, 10.0), &BLUE),
    ];
    assert_eq!(lines_kept(&sewn(elements).0), ["line1"]);
}

#[test]
fn req_auto_095_lines_of_a_colour_whose_boxes_meet_but_that_dont_touch_are_2_parts() {
    // A second gold line 0.2 mm to the right of the first, its ragged side matching the first's: their boxes
    // overlap and they don't touch. 2 parts of gold, half of its likeness each: at 0.75 one merges.
    let mut ring = vec![(11.3, 10.0), (11.3, 0.0)];
    ring.extend((0..=20).map(|i| (if i % 2 == 0 { 10.6 } else { 10.9 }, f64::from(i) * 0.5)));
    let elements = vec![
        rect("left", (0.0, 0.0), (10.0, 10.0), &RED),
        ragged("line1", &[(MIN_LIKENESS, "0.75")]),
        automatic(fill("line2", &[&ring], &GOLD, &[(MIN_LIKENESS, "0.75")])),
        rect("right", (11.3, 0.0), (20.0, 10.0), &BLUE),
    ];
    // As one part, gold's only one, neither would merge.
    assert_eq!(lines_kept(&sewn(elements).0).len(), 1);
}

#[test]
fn req_auto_095_a_line_too_narrow_for_a_thread_is_no_part_and_merges() {
    // 0.2 to 0.35 mm wide, 2.75 mm²: no speck, and a disc a thread across fits nowhere in it.
    let line = jagged("line", 10.0, 20, (0.2, 0.35), &GOLD, &[(MIN_LIKENESS, "1")]);
    let elements = vec![rect("left", (0.0, 0.0), (10.0, 10.0), &RED), line, rect("right", (10.35, 0.0), (20.0, 10.0), &BLUE)];
    assert_eq!(lines_kept(&sewn(elements).0), Vec::<&str>::new());
}
