//! `REQ-GEN-005`: an element's clips cut what it sews. A fill sews the part of its area inside every clip, a
//! stroke the pieces of its lines inside them, each on its own, and a satin column is sewn whole, which
//! `SC-W0217` names. An element they leave nothing of sews nothing, which `SC-W0013` names. These cases sew
//! whole designs with the clips the readers give their elements (`crates/stitchcraft-svg/tests/clips.rs`).

// Test code may unwrap, index and panic (clippy.toml allows the first 2 in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::panic)]

use stitchcraft_core::{Budget, Code, Point};
use stitchcraft_engine::PlanOutcome;
use stitchcraft_engine::design::{Clip, ClipShape, Design, DesignSettings, Element, FillRule, Path, Segment, Subpath};
use stitchcraft_engine::generate::approach;
use stitchcraft_engine::generators::Approach;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Role, Stitch, StitchKind};
use stitchcraft_testkit::designs::{RED, along, automatic, clipped, fill, messages, p, planned, polylines, rings, widened};

/// How far a needle point may lie past a clip's edge: the cut is exact, and the needle points of a row or a
/// line that ends there are placed on it, rounded to the machine's tenth of a millimetre.
const EDGE: f64 = 0.06;

/// The needle points the element `id` sews in `outcome`'s plan, locks left out: none when it sews nothing.
fn sewn_by(outcome: &PlanOutcome, id: &str) -> Vec<Point> {
    let plan = outcome.plan.as_ref().unwrap_or_else(|| panic!("{:?}", messages(outcome)));
    let named = |s: &Stitch| s.origin.element.and_then(|e| plan.elements.get(e.index())).is_some_and(|e| e.as_str() == id);
    plan.stitches().filter(|s| s.kind == StitchKind::Normal && s.origin.role != Role::Lock && named(s)).map(|s| s.at).collect()
}

/// The box round `points`: the least and the greatest x and y.
fn bounds(points: &[Point]) -> [f64; 4] {
    let (xs, ys) = (points.iter().map(|q| q.x()), points.iter().map(|q| q.y()));
    [xs.clone().fold(f64::MAX, f64::min), ys.clone().fold(f64::MAX, f64::min), xs.fold(f64::MIN, f64::max), ys.fold(f64::MIN, f64::max)]
}

/// Whether the box `got` is the box `want` within `slack` millimetres on each side.
fn about(got: [f64; 4], want: [f64; 4], slack: f64) -> bool {
    got.iter().zip(want).all(|(g, w)| (g - w).abs() <= slack)
}

/// A rectangle from (`x0`, `y0`) to (`x1`, `y1`), as the rings of a fill or a clip.
fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> [(f64, f64); 4] {
    [(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A square fill 20 mm a side round (0, 10), with the settings `own`.
fn square(own: &[(&str, &str)]) -> Element {
    fill("square", &[&rect(-10.0, 0.0, 10.0, 20.0)], &RED, own)
}

/// A running stitch, or another stroke as `own` says, along the x axis from -30 to 30.
fn axis(own: &[(&str, &str)]) -> Element {
    along("axis", polylines(&[&[(-30.0, 0.0), (30.0, 0.0)]]), &RED, own)
}

#[test]
fn req_gen_005_a_fill_sews_what_its_clips_leave_of_its_area() {
    // A clip over the square's right half, from y 5 to 15: its rows, and its underlay inside them, cover
    // that and nothing else. 2 clips cut it to what lies inside both.
    let half = clipped(square(&[]), &[&rect(0.0, 5.0, 30.0, 15.0)]);
    let outcome = planned(vec![half]);
    assert!(messages(&outcome).is_empty(), "{:?}", messages(&outcome));
    let sewn = sewn_by(&outcome, "square");
    assert!(about(bounds(&sewn), [0.0, 5.0, 10.0, 15.0], EDGE), "{:?}", bounds(&sewn));
    let both = clipped(clipped(square(&[]), &[&rect(0.0, 5.0, 30.0, 15.0)]), &[&rect(-30.0, 0.0, 5.0, 30.0)]);
    let sewn = sewn_by(&planned(vec![both]), "square");
    assert!(about(bounds(&sewn), [0.0, 5.0, 5.0, 15.0], EDGE), "{:?}", bounds(&sewn));
}

#[test]
fn req_gen_005_a_clip_is_the_area_of_its_shapes_together() {
    // 2 shapes that overlap: the fill sews what lies in either, the overlap among it (DEV-SVG-016).
    let shape = |x0, x1| ClipShape { path: rings(&[&rect(x0, 0.0, x1, 20.0)]), rule: FillRule::NonZero };
    let both = Clip { shapes: vec![shape(-10.0, 0.0), shape(-5.0, 5.0)] };
    let sewn = sewn_by(&planned(vec![Element { clips: vec![both], ..square(&[]) }]), "square");
    assert!(about(bounds(&sewn), [-10.0, 0.0, 5.0, 20.0], EDGE), "{:?}", bounds(&sewn));
    assert!(sewn.iter().any(|q| (-4.0..-1.0).contains(&q.x())), "the overlap is sewn");
    // Each shape's own rule says what is inside it: a square round a square drawn the same way round is a
    // ring by the even-odd rule, and filled by the nonzero rule.
    let holed = |rule| Clip::of(rings(&[&rect(-10.0, 0.0, 10.0, 20.0), &rect(-5.0, 5.0, 5.0, 15.0)]), rule);
    let in_hole = |q: &Point| (-4.5..4.5).contains(&q.x()) && (5.5..14.5).contains(&q.y());
    let even = sewn_by(&planned(vec![Element { clips: vec![holed(FillRule::EvenOdd)], ..square(&[]) }]), "square");
    assert!(!even.is_empty() && !even.iter().any(in_hole));
    let nonzero = sewn_by(&planned(vec![Element { clips: vec![holed(FillRule::NonZero)], ..square(&[]) }]), "square");
    assert!(nonzero.iter().any(in_hole));
}

#[test]
fn req_gen_005_a_fill_is_cut_before_expand_mm_grows_it() {
    // Ink/Stitch cuts the shape first, and `expand_mm` then grows what is left, past the clip's edge.
    let grown = clipped(square(&[("expand_mm", "1"), ("fill_underlay", "false")]), &[&rect(0.0, 5.0, 30.0, 15.0)]);
    let sewn = sewn_by(&planned(vec![grown]), "square");
    assert!(about(bounds(&sewn), [-1.0, 4.0, 11.0, 16.0], 0.3), "{:?}", bounds(&sewn));
}

#[test]
fn diag_sc_w0013_an_element_its_clips_leave_nothing_of_sews_nothing() {
    // A clip beside the square, a clip with no area, and 2 clips that don't meet: the fill sews nothing, and
    // its trim with it. The line after it still sews.
    let after = along("after", polylines(&[&[(-10.0, 30.0), (10.0, 30.0)]]), &RED, &[]);
    let empty = Clip::default();
    let cases = [
        clipped(square(&[("trim_after", "true")]), &[&rect(20.0, 0.0, 40.0, 20.0)]),
        Element { clips: vec![empty], ..square(&[("trim_after", "true")]) },
        clipped(clipped(square(&[("trim_after", "true")]), &[&rect(-10.0, 0.0, 0.0, 20.0)]), &[&rect(1.0, 0.0, 10.0, 20.0)]),
    ];
    for element in cases {
        let outcome = planned(vec![element.clone(), after.clone()]);
        let said = messages(&outcome);
        assert!(sewn_by(&outcome, "square").is_empty(), "{:?}", element.clips);
        assert!(said.iter().any(|m| m.contains("SC-W0013") && m.contains("leave nothing of this fill")), "{said:?}");
        assert!(said.iter().any(|m| m.contains("SC-W0505")), "{said:?}");
        assert!(!sewn_by(&outcome, "after").is_empty());
    }
    // A stroke too.
    let away = clipped(axis(&[]), &[&rect(-5.0, 10.0, 5.0, 20.0)]);
    let outcome = planned(vec![away, after]);
    assert!(sewn_by(&outcome, "axis").is_empty());
    assert!(messages(&outcome).iter().any(|m| m.contains("SC-W0013") && m.contains("leave nothing of this stroke")), "{:?}", messages(&outcome));
}

#[test]
fn req_gen_005_a_stroke_is_cut_where_it_crosses_its_clips_and_each_piece_sews_on_its_own() {
    // The axis crosses 2 squares of one clip: a running stitch sews the piece in each, in order along it, and
    // nothing between them, which the needle jumps.
    let squares: [&[(f64, f64)]; 2] = [&rect(-20.0, -5.0, -10.0, 5.0), &rect(10.0, -5.0, 20.0, 5.0)];
    let outcome = planned(vec![clipped(axis(&[]), &squares)]);
    assert!(messages(&outcome).is_empty(), "{:?}", messages(&outcome));
    let sewn = sewn_by(&outcome, "axis");
    assert!(sewn.iter().all(|q| (10.0 - EDGE..=20.0 + EDGE).contains(&q.x().abs())), "{sewn:?}");
    assert_eq!((sewn[0], *sewn.last().unwrap()), (p(-20.0, 0.0), p(20.0, 0.0)));
    assert!(sewn.iter().any(|q| q.x() == -10.0) && sewn.iter().any(|q| q.x() == 10.0), "each piece ends on its cut: {sewn:?}");
    // A manual stitch is cut from node to node, and sews a needle point where its line crosses the clip's
    // edge; a zigzag stroke's zigzag stays within the clip along the line, its width across it uncut.
    let clip = [&rect(-10.0, -5.0, 10.0, 5.0)[..]];
    let manual = along("manual", polylines(&[&[(-30.0, 0.0), (0.0, 0.0), (30.0, 0.0)]]), &RED, &[("stroke_method", "manual_stitch")]);
    let sewn = sewn_by(&planned(vec![clipped(manual, &clip)]), "manual");
    assert_eq!(sewn, [p(-10.0, 0.0), p(0.0, 0.0), p(10.0, 0.0)]);
    let zigzag = clipped(axis(&[("stroke_method", "zigzag_stitch")]), &clip);
    let sewn = sewn_by(&planned(vec![widened(zigzag, 2.0)]), "axis");
    assert!(about(bounds(&sewn), [-10.0, -1.0, 10.0, 1.0], 0.2), "{:?}", bounds(&sewn));
}

#[test]
fn req_gen_005_a_manual_stitch_is_cut_from_node_to_node_and_another_stroke_along_its_curves() {
    // A curve from (-30, 0) to (30, 0) that bulges 7.5 mm up, cut to x from -10 to 10. A manual stitch sews
    // its nodes, so its line runs straight between them and is cut there, on the x axis. A running stitch
    // follows the curve, so it is cut on the bulge, and every needle point lies on it, well above the axis.
    let bulge = Path {
        subpaths: vec![Subpath {
            start: p(-30.0, 0.0),
            segments: vec![Segment::Cubic(p(-20.0, -10.0), p(20.0, -10.0), p(30.0, 0.0))],
            closed: false,
        }],
    };
    let clip = [&rect(-10.0, -20.0, 10.0, 5.0)[..]];
    let manual = clipped(along("manual", bulge.clone(), &RED, &[("stroke_method", "manual_stitch")]), &clip);
    // The machine's longest stitch splits its 20 mm in 2, on the axis too.
    assert_eq!(sewn_by(&planned(vec![manual]), "manual"), [p(-10.0, 0.0), p(0.0, 0.0), p(10.0, 0.0)]);
    let running = sewn_by(&planned(vec![clipped(along("running", bulge, &RED, &[]), &clip)]), "running");
    assert!(running.len() > 3 && running.iter().all(|q| q.y() < -5.0 && q.x().abs() <= 10.0 + EDGE), "{running:?}");
}

#[test]
fn req_gen_005_a_clipped_fill_that_bounds_no_area_says_so_as_an_unclipped_one_does() {
    // A ring that runs out and back along one line bounds no area, clipped or not: the fill says that, not
    // that its clips leave nothing of it.
    let flat = fill("flat", &[&[(-10.0, 0.0), (10.0, 0.0)]], &RED, &[]);
    let outcome =
        planned(vec![clipped(flat, &[&rect(-20.0, -20.0, 20.0, 20.0)]), along("after", polylines(&[&[(0.0, 30.0), (10.0, 30.0)]]), &RED, &[])]);
    let said = messages(&outcome);
    assert!(said.iter().any(|m| m.contains("SC-W0303")) && !said.iter().any(|m| m.contains("SC-W0013")), "{said:?}");
}

#[test]
fn req_gen_005_a_subpath_its_clips_leave_whole_sews_as_drawn() {
    // A curve inside its clip, and a second subpath outside it: the curve sews as it does unclipped, its
    // points flattened as a running stitch flattens them, not as the cut would.
    let curve = Subpath { start: p(-10.0, 0.0), segments: vec![Segment::Cubic(p(-5.0, -8.0), p(5.0, 8.0), p(10.0, 0.0))], closed: false };
    let outside = Subpath { start: p(-10.0, 30.0), segments: vec![Segment::Line(p(10.0, 30.0))], closed: false };
    let alone = along("curve", Path { subpaths: vec![curve.clone()] }, &RED, &[]);
    let both = along("curve", Path { subpaths: vec![curve, outside] }, &RED, &[]);
    let clip = [&rect(-15.0, -15.0, 15.0, 15.0)[..]];
    assert_eq!(sewn_by(&planned(vec![clipped(both, &clip)]), "curve"), sewn_by(&planned(vec![alone]), "curve"));
}

#[test]
fn diag_sc_w0217_a_clipped_satin_column_is_sewn_whole() {
    let rails = polylines(&[&[(-20.0, 0.0), (20.0, 0.0)], &[(-20.0, 6.0), (20.0, 6.0)]]);
    let column = along("column", rails, &RED, &[("satin_column", "true")]);
    let whole = sewn_by(&planned(vec![column.clone()]), "column");
    let outcome = planned(vec![clipped(column, &[&rect(-5.0, -5.0, 5.0, 10.0)])]);
    assert_eq!(sewn_by(&outcome, "column"), whole);
    assert!(matches!(messages(&outcome).as_slice(), [m] if m.contains("SC-W0217")), "{:?}", messages(&outcome));
}

#[test]
fn req_gen_005_a_clipped_element_offers_what_its_clips_leave_of_it() {
    // The element before a clipped fill ends nearest what is left of its area, and before a clipped stroke
    // at its first piece's start. One clipped away offers nothing.
    let settings = DesignSettings::default();
    let offer = |element: &Element| approach(element, &settings, &Budget::DEFAULT);
    let Some(Approach::Area(parts)) = offer(&clipped(square(&[]), &[&rect(0.0, 5.0, 30.0, 15.0)])) else { panic!("no area") };
    let corners: Vec<Point> = parts.iter().flat_map(|part| part.outline.iter().copied()).collect();
    assert_eq!(bounds(&corners), [0.0, 5.0, 10.0, 15.0]);
    let squares: [&[(f64, f64)]; 2] = [&rect(-20.0, -5.0, -10.0, 5.0), &rect(10.0, -5.0, 20.0, 5.0)];
    assert_eq!(offer(&clipped(axis(&[]), &squares)), Some(Approach::Point(p(-20.0, 0.0))));
    assert_eq!(offer(&clipped(axis(&[]), &[&rect(-5.0, 10.0, 5.0, 20.0)])), None);
    assert_eq!(offer(&clipped(square(&[]), &[&rect(20.0, 0.0, 40.0, 20.0)])), None);
}

#[test]
fn req_gen_005_an_automatic_shape_whose_clips_need_more_work_than_there_is_is_left_out_and_said() {
    // Cutting a shape to its clips is the first work automatic digitizing does for it, within the budget's
    // work: with almost none to spend, the square is left out for it, by name.
    let area = automatic(clipped(square(&[]), &[&rect(0.0, 5.0, 30.0, 15.0)]));
    let design = Design::new(vec![area], DesignSettings::default()).unwrap();
    let outcome = stitchcraft_engine::plan(&design, REFERENCE, &Budget { max_stitches: 1_000, max_work: 5 });
    let named = |d: &&stitchcraft_core::Diagnostic| d.code == Code::BudgetExhausted && d.element.as_ref().is_some_and(|e| e.as_str() == "square");
    assert!(outcome.diagnostics.iter().any(|d| named(&d)), "{:?}", messages(&outcome));
}

#[test]
fn req_gen_005_an_automatic_shape_is_cut_to_its_clips_before_it_is_digitized() {
    // A line 3 mm wide is a satin column: cut first, its column runs from one edge of the clip to the other,
    // where a column of the whole line would be sewn whole. An area is filled where its clip leaves it, and
    // one its clip leaves nothing of is left out.
    let clip = [&rect(-10.0, -5.0, 10.0, 5.0)[..]];
    let line = automatic(clipped(widened(axis(&[]), 3.0), &clip));
    let outcome = planned(vec![line]);
    assert!(messages(&outcome).iter().all(|m| m.contains("SC-I0901")), "{:?}", messages(&outcome));
    // Its pull compensation widens it a little across the line, not along it.
    let sewn = sewn_by(&outcome, "axis");
    assert!(about(bounds(&sewn), [-10.0, -1.9, 10.0, 1.9], 0.01), "{:?}", bounds(&sewn));
    let area = automatic(clipped(square(&[]), &[&rect(0.0, 5.0, 30.0, 15.0)]));
    let sewn = sewn_by(&planned(vec![area]), "square");
    assert!(about(bounds(&sewn), [0.0, 5.0, 10.0, 15.0], 0.3), "{:?}", bounds(&sewn));
    let away = automatic(clipped(square(&[]), &[&rect(20.0, 0.0, 40.0, 20.0)]));
    let after = along("after", polylines(&[&[(-10.0, 30.0), (10.0, 30.0)]]), &RED, &[]);
    let outcome = planned(vec![away, after]);
    assert!(sewn_by(&outcome, "square").is_empty());
    assert!(messages(&outcome).iter().any(|m| m.contains("SC-W0013")), "{:?}", messages(&outcome));
}
