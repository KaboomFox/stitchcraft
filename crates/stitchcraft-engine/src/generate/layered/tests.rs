//! What the order of a run reads of an element before it is generated, and how an element of a run is read
//! once for its 2 layers (`REQ-ASM-008`, `REQ-ASM-009`).

use stitchcraft_core::{ElementId, Point};
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{FillRule, Join, Path, Segment, Subpath};
use crate::generators::Approach;

/// An element `shape` with `params`, in red.
fn element(shape: Shape, params: &[(&str, &str)]) -> Element {
    Element {
        id: ElementId::new("e").unwrap(),
        name: None,
        shape,
        thread: Thread::new(Rgb::new(200, 0, 0)),
        params: params.iter().copied().collect(),
        automatic: false,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A square 10 mm a side, filled.
fn square(params: &[(&str, &str)]) -> Element {
    let segments = [p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0)].map(Segment::Line).to_vec();
    element(
        Shape::Fill {
            path: Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments, closed: true }] },
            rule: FillRule::NonZero,
            gradient: None,
        },
        params,
    )
}

/// A satin column of the subpaths `rails`, with `params`.
fn satin(rails: Vec<Subpath>, params: &[(&str, &str)]) -> Element {
    let mut params = params.to_vec();
    params.push(("satin_column", "true"));
    let shape = Shape::Stroke { path: Path { subpaths: rails }, width: Mm::new(1.0).unwrap(), join: Join::UNSET, cap: crate::design::Cap::Butt };
    element(shape, &params)
}

/// A straight rail from (0, `y`) to (`length`, `y`).
fn rail(y: f64, length: f64) -> Subpath {
    Subpath { start: p(0.0, y), segments: vec![Segment::Line(p(length, y))], closed: false }
}

/// A satin column 20 mm long and 4 mm wide between 2 rails, with `params`.
fn column(params: &[(&str, &str)]) -> Element {
    satin(vec![rail(0.0, 20.0), rail(4.0, 20.0)], params)
}

/// A line 20 mm long, with `params`.
fn line(params: &[(&str, &str)]) -> Element {
    element(
        Shape::stroke(Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(20.0, 0.0))], closed: false }] }),
        params,
    )
}

#[test]
fn req_asm_008_an_element_reads_when_every_setting_its_stitch_type_reads_reads() {
    let settings = DesignSettings::default();
    assert!(reads(&line(&[]), &settings));
    assert!(!reads(&line(&[("running_stitch_length_mm", "long")]), &settings));
    // A setting its stitch type doesn't read is kept unread, whatever it says.
    assert!(reads(&line(&[("row_spacing_mm", "wide")]), &settings));
    // Nor one its settings hide: the length variation of random lengths that are off.
    assert!(reads(&line(&[("random_stitch_length_jitter_percent", "lots")]), &settings));
    assert!(!reads(&line(&[("random_stitch_length_jitter_percent", "lots"), ("enable_random_stitch_length", "true")]), &settings));
    // Blank text is the default.
    assert!(reads(&line(&[("running_stitch_length_mm", " ")]), &settings));
    // A setting that chooses the stitch type.
    assert!(!reads(&line(&[("satin_column", "maybe")]), &settings));
    // A value only the node's other stitch type reads is left to it: a zigzag stroke's 2 spacings under a
    // meander fill.
    assert!(reads(&square(&[("fill_method", "meander_fill"), ("zigzag_spacing_mm", "1 2")]), &settings));
    assert!(!reads(&square(&[("fill_method", "meander_fill"), ("zigzag_spacing_mm", "wide")]), &settings));
}

#[test]
fn req_asm_009_a_satin_column_or_a_fill_with_its_underlay_on_has_an_underlay() {
    let settings = DesignSettings::default();
    let has = |element: Element| has_underlay(&element, &settings);
    assert!(!has(column(&[])));
    for underlay in ["center_walk_underlay", "contour_underlay", "zigzag_underlay"] {
        assert!(has(column(&[(underlay, "true")])), "{underlay}");
        assert!(has(column(&[(underlay, "true"), ("satin_method", "e_stitch")])), "{underlay}");
    }
    // A fill's is on by default, for every method with one.
    for method in ["tatami_fill", "contour_fill", "meander_fill", "circular_fill", "guided_fill", "tartan_fill"] {
        assert!(has(square(&[("fill_method", method)])), "{method}");
    }
    // A guided fill whose strategy is not sewn yet sews none.
    assert!(!has(square(&[("fill_method", "guided_fill"), ("guided_fill_strategy", "1")])));
    assert!(!has(square(&[("fill_underlay", "false")])));
    assert!(!has(square(&[("fill_method", "cross_stitch")])));
    // Ink/Stitch's legacy fill is sewn without underlay, and a fill set not to be sewn sews none.
    assert!(!has(square(&[("fill_method", "legacy_fill")])));
    assert!(!has(square(&[("fill", "false")])));
    // Strokes have none, and nor has a satin column too narrow to stitch across, which is sewn as a stroke.
    assert!(!has(line(&[("contour_underlay", "true")])));
    assert!(!has(line(&[("satin_column", "true"), ("contour_underlay", "true")])));
    // Nor has an element whose stitch type doesn't read.
    assert!(!has(line(&[("satin_column", "maybe")])));
}

/// `element` read for its layers in a design without settings, with what reading it says.
fn read(element: &Element) -> (Option<Layered<'_>>, Vec<String>) {
    let mut diagnostics = Vec::new();
    let read =
        Layered::read(element, &DesignSettings::default(), stitchcraft_plan::profiles::REFERENCE, &mut diagnostics, &mut Budget::DEFAULT.meter());
    (read.unwrap(), diagnostics.iter().map(ToString::to_string).collect())
}

#[test]
fn req_asm_009_an_element_is_read_once_for_its_layers() {
    // A satin column is placed as it is read, and a fill's settings and area are read.
    let satin_column = column(&[("contour_underlay", "true")]);
    let (layered, said) = read(&satin_column);
    assert!(matches!(layered.as_ref().map(|l| &l.sews), Some(Sews::Satin(_))) && said.is_empty(), "{said:?}");
    let fill = square(&[]);
    let (layered, said) = read(&fill);
    assert!(matches!(layered.as_ref().map(|l| &l.sews), Some(Sews::Fill(..))) && said.is_empty(), "{said:?}");
    // A column too small to stitch is left out whole, and one without rails skipped, each said.
    let speck = satin(vec![rail(0.0, 0.1), rail(0.2, 0.1)], &[("center_walk_underlay", "true")]);
    let (layered, said) = read(&speck);
    assert!(layered.is_none() && said.len() == 1 && said[0].starts_with("warning SC-W0215:"), "{said:?}");
    let dot = |x: f64| Subpath { start: p(x, 0.0), segments: Vec::new(), closed: false };
    let dots = satin(vec![dot(0.0), dot(1.0)], &[("contour_underlay", "true")]);
    let (layered, said) = read(&dots);
    assert!(layered.is_none() && said.iter().any(|s| s.starts_with("error SC-E0201:")), "{said:?}");
    // A stroke has no underlay, and the order never asks for one: reading one is a bug, which is said.
    let stroke = line(&[]);
    let (layered, said) = read(&stroke);
    assert!(layered.is_none());
    assert_eq!(said, ["error SC-E0009: A stroke has no underlay to sew before its top stitches."]);
    // Settings that don't read skip it.
    let unread = square(&[("row_spacing_mm", "wide")]);
    assert!(read(&unread).0.is_none());
}

#[test]
fn req_asm_009_a_fill_s_underlay_starts_at_its_start_position_and_its_top_ends_at_its_end_position() {
    // A square that starts at its corner (10, 10) and ends at (0, 10): its underlay starts nearest the
    // start, whatever the needle, and its top ends nearest the end, whatever comes next.
    let mut fill = square(&[]);
    fill.hints = Hints { start: Some(p(10.0, 10.0)), end: Some(p(0.0, 10.0)), target: None, ..Hints::default() };
    let mut layered = read(&fill).0.unwrap();
    let neighbours = Neighbours { needle: Some(p(-5.0, -5.0)), next: Some(Approach::Point(p(20.0, -5.0))) };
    let (meter, diagnostics) = (&mut Budget::DEFAULT.meter(), &mut Vec::new());
    let underlay = layered.sew(Layer::Underlay, &neighbours, diagnostics, meter).unwrap();
    let first = underlay.groups.iter().flatten().next().copied().unwrap();
    assert!(first.distance(p(10.0, 10.0)) < 2.0, "{first:?}");
    let top = layered.sew(Layer::Top, &neighbours, diagnostics, meter).unwrap();
    let last = top.groups.iter().flatten().last().copied().unwrap();
    assert!(last.distance(p(0.0, 10.0)) < 2.0, "{last:?}");
    assert_eq!((underlay.layer, top.layer, diagnostics.len()), (Layer::Underlay, Layer::Top, 0));
}

#[test]
fn req_asm_009_a_satin_column_s_underlay_starts_at_its_start_position_and_its_top_ends_at_its_end_position() {
    // A column 20 mm long that starts and ends at its left end, between a needle and a next element both past
    // its right end. Its underlay starts at the start position and ends at the right end, nearest what the
    // next layer offers, not at the end position. Its top starts at the right end, nearest the needle, not at
    // the start position, and ends at the end position.
    let mut satin_column = column(&[("contour_underlay", "true"), ("zigzag_underlay", "true")]);
    satin_column.hints = Hints { start: Some(p(0.0, 2.0)), end: Some(p(0.0, 2.0)), target: None, ..Hints::default() };
    let mut layered = read(&satin_column).0.unwrap();
    let neighbours = Neighbours { needle: Some(p(25.0, 2.0)), next: Some(Approach::Point(p(25.0, 2.0))) };
    let (meter, diagnostics) = (&mut Budget::DEFAULT.meter(), &mut Vec::new());
    let underlay = layered.sew(Layer::Underlay, &neighbours, diagnostics, meter).unwrap();
    let top = layered.sew(Layer::Top, &neighbours, diagnostics, meter).unwrap();
    let ends = |generated: &Generated| {
        let points: Vec<Point> = generated.groups.iter().flatten().copied().collect();
        (points.first().copied().unwrap(), points.last().copied().unwrap())
    };
    let ((under_first, under_last), (top_first, top_last)) = (ends(&underlay), ends(&top));
    assert!(under_first.x() < 2.0 && under_last.x() > 18.0, "{under_first:?} to {under_last:?}");
    assert!(top_first.x() > 18.0 && top_last.x() < 2.0, "{top_first:?} to {top_last:?}");
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn req_gen_004_each_layer_spends_the_design_s_stitches_and_one_short_ends_planning() {
    let settings = DesignSettings::default();
    let (budget, neighbours) = (Budget::DEFAULT, Neighbours::default());
    let satin_column = column(&[("contour_underlay", "true")]);
    let sewn = |meter: Meter| underlay(&satin_column, &settings, stitchcraft_plan::profiles::REFERENCE, &neighbours, &budget, meter);
    // With the stitches it needs, its underlay spends what it gives, and its top what it gives.
    let (generation, layered) = sewn(budget.meter()).unwrap();
    let made = generation.generated.unwrap().groups.iter().map(Vec::len).sum::<usize>();
    assert!(made > 10, "{made} needle points of underlay");
    let top = layered.unwrap().top(&neighbours, &budget, budget.max_stitches).unwrap();
    assert!(top.generated.unwrap().groups.iter().map(Vec::len).sum::<usize>() > made);
    // One stitch is too few for its placing: planning ends, with SC-E0004 naming it.
    let Err(said) = sewn(Budget { max_stitches: 1, ..budget }.meter()) else { panic!("1 stitch was enough") };
    assert_eq!(said.iter().map(|d| (d.code, d.element.as_ref().map(ElementId::as_str))).collect::<Vec<_>>(), [(Code::BudgetExhausted, Some("e"))]);
    // No stitch left for its top ends planning there too.
    let (_, layered) = sewn(budget.meter()).unwrap();
    let Err(said) = layered.unwrap().top(&neighbours, &budget, 0) else { panic!("its top made no stitch") };
    assert_eq!(said.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::BudgetExhausted]);
}

#[test]
fn req_gen_002_a_layer_whose_work_runs_out_skips_its_element() {
    let settings = DesignSettings::default();
    let (budget, neighbours) = (Budget::DEFAULT, Neighbours::default());
    let fill = square(&[("trim_after", "true")]);
    let sewn = |meter: Meter| underlay(&fill, &settings, stitchcraft_plan::profiles::REFERENCE, &neighbours, &budget, meter).unwrap();
    // Out of work in its underlay: skipped, held for nothing, and its trim left out.
    let (generation, layered) = sewn(Budget { max_work: 10, ..budget }.meter());
    let said: Vec<Code> = generation.diagnostics.iter().map(|d| d.code).collect();
    assert_eq!((generation.generated.is_none(), layered.is_none(), said), (true, true, vec![Code::BudgetExhausted, Code::TrimOrStopLeftOut]));
    // Out of work in its top: skipped there, and the plan leaves out its underlay with it, so the element
    // sews nothing and its trim is left out too, as when it is skipped whole.
    let (_, layered) = sewn(budget.meter());
    let mut layered = layered.unwrap();
    layered.work = 10;
    let top = layered.top(&neighbours, &budget, budget.max_stitches).unwrap();
    let said = top.diagnostics.iter().map(|d| d.code).collect::<Vec<_>>();
    assert_eq!((top.generated, said), (None, vec![Code::BudgetExhausted, Code::TrimOrStopLeftOut]));
}
