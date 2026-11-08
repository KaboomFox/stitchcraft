use stitchcraft_core::{Budget, ElementId, Mm};
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{Cap, Clip, DesignSettings, FillRule, Join};
use crate::route::tests::{lines_of, p, stroke};
use crate::route::{Route, Tool, route};

/// `elements` routed by Auto-Route Running Stitch with `options`, from `start` to `end`.
fn routed(elements: &[Element], options: Options, [start, end]: [Option<Point>; 2]) -> crate::route::Routed {
    route(elements, &Route { tool: Tool::Runs, options, start, end }, &DesignSettings::default(), &Budget::DEFAULT)
}

/// An element of a route: its name, the element it comes from, whether it runs under the others, its
/// settings as `key=value`, and its points.
type Sewn = (String, usize, bool, Vec<String>, Vec<(f64, f64)>);

/// Each element of the route ([`Sewn`]).
fn sewn(routed: &crate::route::Routed) -> Vec<Sewn> {
    routed
        .pieces
        .iter()
        .map(|piece| {
            let params = piece.element.params.iter().map(|(key, value)| format!("{key}={value}")).collect();
            let points = lines_of(&piece.element).concat().iter().map(|q| (q.x(), q.y())).collect();
            (piece.element.name.clone().unwrap_or_default(), piece.from, piece.under, params, points)
        })
        .collect()
}

/// Each element's name, and its points rounded to a ten-thousandth of a millimetre.
fn names_and_points(routed: &crate::route::Routed) -> Vec<(String, Vec<(f64, f64)>)> {
    let round = |v: f64| (v * 1e4).round() / 1e4;
    sewn(routed).into_iter().map(|(name, .., points)| (name, points.into_iter().map(|(x, y)| (round(x), round(y))).collect())).collect()
}

/// A rail, a post across it, a brace from the post to the rail, and apart a line that crosses itself,
/// as Ink/Stitch's Auto-Route Running Stitch routes them, run as a black box at d59c9ab.
fn network() -> Vec<Element> {
    vec![
        stroke("e0", &[(10.0, 32.0), (48.0, 32.0)], &[]),
        stroke("e1", &[(16.0, 12.0), (16.0, 35.0)], &[]),
        stroke("e2", &[(16.0, 18.0), (30.0, 32.0)], &[]),
        stroke("e3", &[(32.0, 10.0), (44.0, 18.0), (44.0, 8.0), (34.0, 16.0)], &[]),
    ]
}

/// The crossing of the line that crosses itself, rounded as [`names_and_points`] rounds.
const X: (f64, f64) = (37.1818, 13.4545);

/// The route [`network`]'s elements take up to the jump to the line apart, the same with and without trims.
fn to_the_jump() -> Vec<(String, Vec<(f64, f64)>)> {
    [
        ("AutoRun 1", vec![(10.0, 32.0), (16.0, 32.0)]),
        ("AutoRun Underpath 2", vec![(16.0, 32.0), (16.0, 18.0), (30.0, 32.0)]),
        ("AutoRun 3", vec![(30.0, 32.0), (16.0, 18.0)]),
        ("AutoRun Underpath 4", vec![(16.0, 18.0), (16.0, 12.0)]),
        ("AutoRun 5", vec![(16.0, 12.0), (16.0, 18.0), (16.0, 32.0)]),
        ("AutoRun Underpath 6", vec![(16.0, 32.0), (16.0, 35.0)]),
        ("AutoRun 7", vec![(16.0, 35.0), (16.0, 32.0)]),
        ("AutoRun 8", vec![(16.0, 32.0), (30.0, 32.0), (48.0, 32.0)]),
    ]
    .into_iter()
    .map(|(name, points)| (name.to_string(), points))
    .collect()
}

#[test]
fn req_route_011_a_network_is_sewn_as_ink_stitch_s_auto_route_running_stitch_sews_it() {
    // From the leftmost end along the rail to its rightmost: at the post, under the brace's way down and up
    // the post's top, and back on top; then the rail on, a jump to the line apart, and its loop and ends, each
    // out under and back on top. The run under the post and the brace is one element.
    let routed = routed(&network(), Tool::Runs.defaults(), [None, None]);
    let mut want = to_the_jump();
    want.extend(
        [
            ("AutoRun Underpath 9", vec![(44.0, 18.0), X, (32.0, 10.0)]),
            ("AutoRun 10", vec![(32.0, 10.0), X]),
            ("AutoRun Underpath 11", vec![X, (44.0, 8.0), (44.0, 18.0)]),
            ("AutoRun 12", vec![(44.0, 18.0), (44.0, 8.0), X]),
            ("AutoRun Underpath 13", vec![X, (34.0, 16.0)]),
            ("AutoRun 14", vec![(34.0, 16.0), X, (44.0, 18.0)]),
        ]
        .map(|(name, points)| (name.to_string(), points)),
    );
    assert_eq!(names_and_points(&routed), want);
    assert_eq!(routed.replaced, [0, 1, 2, 3]);
    assert!(routed.diagnostics.is_empty(), "{:?}", routed.diagnostics);
    // Each comes from the stroke it runs along, the run under 2 strokes from the last.
    let from: Vec<(usize, bool)> = routed.pieces.iter().map(|piece| (piece.from, piece.under)).collect();
    assert_eq!(&from[..4], [(0, false), (2, true), (2, false), (1, true)]);
}

#[test]
fn req_route_012_a_jump_longer_than_0_75_mm_trims_and_the_runs_under_the_others_after_it_are_left_out() {
    // As Ink/Stitch routes them with Trim jump stitches: the rail trims before the jump to the line apart, and
    // the run that would follow under the line's first stretch is left out. The route's last jump, back to
    // the rail's end, trims too. The elements are counted as they are made, without the run left out.
    let routed = routed(&network(), Options { trim: true, ..Tool::Runs.defaults() }, [None, None]);
    let mut want = to_the_jump();
    want.extend(
        [
            ("AutoRun 9", vec![(32.0, 10.0), X]),
            ("AutoRun Underpath 10", vec![X, (44.0, 8.0), (44.0, 18.0)]),
            ("AutoRun 11", vec![(44.0, 18.0), (44.0, 8.0), X]),
            ("AutoRun Underpath 12", vec![X, (34.0, 16.0)]),
            ("AutoRun 13", vec![(34.0, 16.0), X, (44.0, 18.0)]),
        ]
        .map(|(name, points)| (name.to_string(), points)),
    );
    assert_eq!(names_and_points(&routed), want);
    let trims: Vec<String> =
        sewn(&routed).into_iter().filter(|(.., params, _)| params.contains(&"trim_after=true".to_string())).map(|s| s.0).collect();
    assert_eq!(trims, ["AutoRun 8", "AutoRun 13"]);
    assert_eq!(RUN_TRIMMED, 0.75);
    // A jump exactly 0.75 mm long doesn't trim, and a hair longer does.
    let jump = |gap: f64| {
        let lines = [stroke("a", &[(-10.0, 0.0), (0.0, 0.0)], &[]), stroke("b", &[(gap, 0.0), (10.0, 0.0)], &[])];
        sewn(&routed_trimmed(&lines)).into_iter().map(|(name, .., params, _)| (name, params)).collect::<Vec<_>>()
    };
    assert_eq!(jump(0.75), [("AutoRun 1".to_string(), vec![]), ("AutoRun 2".to_string(), vec![])]);
    assert_eq!(jump(0.75f64.next_up()), [("AutoRun 1".to_string(), vec!["trim_after=true".to_string()]), ("AutoRun 2".to_string(), vec![])]);
}

/// `elements` routed with Trim jump stitches on.
fn routed_trimmed(elements: &[Element]) -> crate::route::Routed {
    routed(elements, Options { trim: true, ..Tool::Runs.defaults() }, [None, None])
}

#[test]
fn req_route_012_any_long_jump_of_2_in_a_row_trims_where_ink_stitch_looks_only_at_the_first() {
    // A rail, and below it a short line with a line 0.53 mm from its end: the route jumps to the short line,
    // on to the other, and back from it in 2 jumps, 0.53 mm and 5 mm. Ink/Stitch trims after the rail only,
    // and jumps 5.35 mm back to it from the other line untrimmed (`DEV-ROUTE-009`).
    let lines = [
        stroke("a", &[(10.0, 50.0), (50.0, 50.0)], &[]),
        stroke("b", &[(30.0, 55.0), (35.0, 60.0)], &[]),
        stroke("c", &[(29.6, 55.35), (22.0, 59.0)], &[]),
    ];
    let routed = routed_trimmed(&lines);
    let trims: Vec<(String, bool)> =
        sewn(&routed).into_iter().map(|(name, .., params, _)| (name, params.contains(&"trim_after=true".to_string()))).collect();
    let want = [("AutoRun 1", true), ("AutoRun 2", false), ("AutoRun Underpath 3", false), ("AutoRun 4", true), ("AutoRun 5", false)];
    assert_eq!(trims, want.map(|(name, trims)| (name.to_string(), trims)));
}

#[test]
fn req_route_012_without_cuts_the_lines_meet_only_at_their_nodes_as_ink_stitch_routes_them() {
    let routed = routed(&network(), Options { break_up: false, ..Tool::Runs.defaults() }, [None, None]);
    let want = [
        ("AutoRun Underpath 1", vec![(16.0, 35.0), (16.0, 12.0)]),
        ("AutoRun Underpath 2", vec![(16.0, 18.0), (30.0, 32.0)]),
        ("AutoRun 3", vec![(30.0, 32.0), (16.0, 18.0)]),
        ("AutoRun 4", vec![(16.0, 12.0), (16.0, 35.0)]),
        ("AutoRun 5", vec![(10.0, 32.0), (48.0, 32.0)]),
        ("AutoRun Underpath 6", vec![(44.0, 18.0), (32.0, 10.0)]),
        ("AutoRun 7", vec![(32.0, 10.0), (44.0, 18.0)]),
        ("AutoRun Underpath 8", vec![(44.0, 18.0), (44.0, 8.0), (34.0, 16.0)]),
        ("AutoRun 9", vec![(34.0, 16.0), (44.0, 8.0), (44.0, 18.0)]),
    ];
    assert_eq!(names_and_points(&routed), want.map(|(name, points)| (name.to_string(), points)));
}

#[test]
fn req_route_012_with_preserve_order_the_strokes_keep_their_order_as_ink_stitch_routes_them() {
    let routed = routed(&network(), Options { preserve_order: true, ..Tool::Runs.defaults() }, [None, None]);
    let want = [
        ("AutoRun 1", vec![(10.0, 32.0), (16.0, 32.0)]),
        ("AutoRun Underpath 2", vec![(16.0, 32.0), (16.0, 18.0), (30.0, 32.0), (48.0, 32.0)]),
        ("AutoRun 3", vec![(48.0, 32.0), (30.0, 32.0)]),
        ("AutoRun 4", vec![(30.0, 32.0), (16.0, 18.0)]),
        ("AutoRun Underpath 5", vec![(16.0, 18.0), (16.0, 12.0)]),
        ("AutoRun 6", vec![(16.0, 12.0), (16.0, 18.0), (16.0, 32.0)]),
        ("AutoRun Underpath 7", vec![(16.0, 32.0), (16.0, 35.0)]),
        ("AutoRun 8", vec![(16.0, 35.0), (16.0, 32.0)]),
        ("AutoRun 9", vec![(16.0, 32.0), (30.0, 32.0)]),
        ("AutoRun 10", vec![(34.0, 16.0), X]),
        ("AutoRun Underpath 11", vec![X, (32.0, 10.0)]),
        ("AutoRun 12", vec![(32.0, 10.0), X]),
        ("AutoRun Underpath 13", vec![X, (44.0, 8.0), (44.0, 18.0)]),
        ("AutoRun 14", vec![(44.0, 18.0), (44.0, 8.0), X, (44.0, 18.0)]),
    ];
    assert_eq!(names_and_points(&routed), want.map(|(name, points)| (name.to_string(), points)));
}

#[test]
fn req_route_012_with_preserve_order_every_subpath_of_a_stroke_is_sewn() {
    // The middle stroke's second subpath lies 28 mm below its first. Joined at their ends, both are sewn,
    // where Ink/Stitch joins them at a point that is no node of its graph and leaves the second unsewn
    // (`DEV-ROUTE-010`).
    let middle = Element {
        shape: Shape::stroke(Path {
            subpaths: vec![crate::route::tests::line(&[(30.0, 42.0), (60.0, 42.0)]), crate::route::tests::line(&[(40.0, 70.0), (50.0, 70.0)])],
        }),
        ..stroke("e1", &[(0.0, 0.0)], &[])
    };
    let lines = [stroke("e0", &[(10.0, 40.0), (30.0, 40.0)], &[]), middle, stroke("e2", &[(62.0, 40.0), (80.0, 40.0)], &[])];
    let routed = routed(&lines, Options { preserve_order: true, ..Tool::Runs.defaults() }, [None, None]);
    let below: Vec<&(String, Vec<(f64, f64)>)> = names_and_points(&routed)
        .iter()
        .filter(|(_, points)| points.iter().any(|q| q.1 == 70.0))
        .cloned()
        .collect::<Vec<_>>()
        .leak()
        .iter()
        .collect();
    assert_eq!(below.len(), 2, "out under and back on top: {below:?}");
    assert_eq!(routed.pieces.iter().map(|piece| piece.from).collect::<Vec<_>>().first(), Some(&0));
    assert_eq!(routed.pieces.last().map(|piece| piece.from), Some(2));
}

#[test]
fn req_route_011_each_element_has_the_settings_of_the_stroke_it_runs_along() {
    // Ink/Stitch gives the rail's first stretch and the brace's the settings of the post, whose run under the
    // others follows each, and the run under the post and the brace the brace's (`DEV-ROUTE-006`).
    let lines = [
        stroke("e0", &[(10.0, 32.0), (48.0, 32.0)], &[("repeats", "3"), ("running_stitch_length_mm", "3")]),
        stroke("e1", &[(16.0, 12.0), (16.0, 35.0)], &[("repeats", "2"), ("bean_stitch_repeats", "1"), ("running_stitch_tolerance_mm", "0.3")]),
        stroke("e2", &[(16.0, 18.0), (30.0, 32.0)], &[("stroke_method", "manual_stitch")]),
    ];
    let routed = routed(&lines, Tool::Runs.defaults(), [None, None]);
    let settings: Vec<(String, usize, Vec<String>)> = sewn(&routed).into_iter().map(|(name, from, _, params, _)| (name, from, params)).collect();
    let want: [(&str, usize, &[&str]); 9] = [
        // On top, a stroke's settings, an even number of repeats lowered by 1 and an odd one kept.
        ("AutoRun 1", 0, &["repeats=3", "running_stitch_length_mm=3"]),
        // Under the others, only the length and tolerance and a manual stitch, so the post's run and the
        // brace's are 2 elements.
        ("AutoRun Underpath 2", 1, &["running_stitch_tolerance_mm=0.3"]),
        ("AutoRun Underpath 3", 2, &["stroke_method=manual_stitch"]),
        ("AutoRun 4", 2, &["stroke_method=manual_stitch"]),
        ("AutoRun Underpath 5", 1, &["running_stitch_tolerance_mm=0.3"]),
        ("AutoRun 6", 1, &["bean_stitch_repeats=1", "repeats=1", "running_stitch_tolerance_mm=0.3"]),
        ("AutoRun Underpath 7", 1, &["running_stitch_tolerance_mm=0.3"]),
        ("AutoRun 8", 1, &["bean_stitch_repeats=1", "repeats=1", "running_stitch_tolerance_mm=0.3"]),
        ("AutoRun 9", 0, &["repeats=3", "running_stitch_length_mm=3"]),
    ];
    let want: Vec<(String, usize, Vec<String>)> =
        want.iter().map(|(name, from, params)| (name.to_string(), *from, params.iter().map(ToString::to_string).collect())).collect();
    assert_eq!(settings, want);
}

#[test]
fn req_route_011_a_run_under_the_others_keeps_its_stroke_s_underlays_first() {
    // Every stroke of the network sets `underlays_first`, and so does every element of its route, each run
    // under the others too. The route's elements in one thread are then one run of underlays first
    // (`REQ-ASM-008`). The route goes the way it goes without the setting.
    let first: Vec<Element> =
        network().into_iter().map(|line| Element { params: [("underlays_first", "true")].into_iter().collect(), ..line }).collect();
    let routed_first = routed(&first, Tool::Runs.defaults(), [None, None]);
    let without: Vec<String> = sewn(&routed_first).into_iter().filter(|(.., params, _)| *params != ["underlays_first=true"]).map(|s| s.0).collect();
    assert_eq!(without, Vec::<String>::new());
    assert_eq!(names_and_points(&routed_first), names_and_points(&routed(&network(), Tool::Runs.defaults(), [None, None])));
}

#[test]
fn req_route_011_strokes_that_differ_in_underlays_first_share_no_run_under_the_others() {
    // The rail and the post set it, and the brace doesn't. The run under the post and then the brace is 2
    // elements, and only the post's sets it. One element can't both set it and not, and a run of underlays
    // first takes in no piece of a stroke that doesn't set it.
    let first = [("underlays_first", "true")];
    let lines = [
        stroke("e0", &[(10.0, 32.0), (48.0, 32.0)], &first),
        stroke("e1", &[(16.0, 12.0), (16.0, 35.0)], &first),
        stroke("e2", &[(16.0, 18.0), (30.0, 32.0)], &[]),
    ];
    let routed = routed(&lines, Tool::Runs.defaults(), [None, None]);
    let sets: Vec<(String, usize, bool)> =
        sewn(&routed).into_iter().map(|(name, from, _, params, _)| (name, from, params == ["underlays_first=true"])).collect();
    let want = [
        ("AutoRun 1", 0, true),
        ("AutoRun Underpath 2", 1, true),
        ("AutoRun Underpath 3", 2, false),
        ("AutoRun 4", 2, false),
        ("AutoRun Underpath 5", 1, true),
        ("AutoRun 6", 1, true),
        ("AutoRun Underpath 7", 1, true),
        ("AutoRun 8", 1, true),
        ("AutoRun 9", 0, true),
    ];
    assert_eq!(sets, want.map(|(name, from, sets)| (name.to_string(), from, sets)));
}

#[test]
fn req_route_011_an_even_number_of_repeats_is_lowered_by_1_on_top_and_an_odd_one_kept() {
    let repeats = |set: &str| {
        let routed = routed(&[stroke("s", &[(0.0, 0.0), (10.0, 0.0)], &[("repeats", set)])], Tool::Runs.defaults(), [None, None]);
        routed.pieces[0].element.params.get("repeats").map(str::to_string)
    };
    assert_eq!(["1", "2", "3", "4", "100"].map(repeats), ["1", "1", "3", "3", "99"].map(|r| Some(r.to_string())));
    let unset = routed(&[stroke("s", &[(0.0, 0.0), (10.0, 0.0)], &[])], Tool::Runs.defaults(), [None, None]);
    assert_eq!(unset.pieces[0].element.params.get("repeats"), None);
}

#[test]
fn req_route_011_each_element_keeps_its_stroke_s_thread_and_stroke_and_nothing_that_places_it() {
    // The rail, the post and the brace of the network, each with a thread, a stroke, positions and a clip of
    // its own.
    let square =
        Path { subpaths: vec![Subpath { closed: true, ..crate::route::tests::line(&[(0.0, 0.0), (60.0, 0.0), (60.0, 60.0), (0.0, 60.0)]) }] };
    let dress = |mut element: Element, colour: u8, width: f64| {
        element.thread = Thread::new(Rgb::new(colour, 0, 0));
        element.shape = Shape::Stroke { path: element.shape.path().clone(), width: Mm::new(width).unwrap(), join: Join::Round, cap: Cap::Square };
        element.hints = Hints { start: Some(p(1.0, 1.0)), end: Some(p(2.0, 2.0)), target: Some(p(3.0, 3.0)), ..Hints::default() };
        element.clips = vec![Clip::of(square.clone(), FillRule::NonZero)];
        element.name = Some("Drawn".to_string());
        element
    };
    let lines: Vec<Element> = network().into_iter().take(3).zip([(10, 0.5), (20, 0.7), (30, 0.9)]).map(|(e, (c, w))| dress(e, c, w)).collect();
    let routed = routed(&lines, Tool::Runs.defaults(), [None, None]);
    for (k, piece) in routed.pieces.iter().enumerate() {
        let source = &lines[piece.from];
        let Shape::Stroke { width, join, cap, .. } = &piece.element.shape else { panic!("{k} is no stroke") };
        let Shape::Stroke { width: drawn, .. } = &source.shape else { panic!("{} is no stroke", source.id) };
        assert_eq!((&piece.element.thread, width, join, cap), (&source.thread, drawn, &Join::Round, &Cap::Square), "{k}");
        assert_eq!((piece.element.hints.clone(), piece.element.clips.len(), piece.element.automatic), (Hints::default(), 0, false), "{k}");
        assert_eq!(piece.element.id, ElementId::new(format!("{}#{}", source.id, k + 1)).unwrap());
    }
    // The run under the post and then the brace, of 2 threads, is 2 elements, each with its own.
    let under: Vec<(usize, u8)> = routed.pieces.iter().take(3).map(|piece| (piece.from, piece.element.thread.color.r)).collect();
    assert_eq!(under, [(0, 10), (1, 20), (2, 30)]);
    assert_eq!(routed.pieces.iter().take(3).map(|piece| piece.under).collect::<Vec<_>>(), [false, true, true]);
}

#[test]
fn diag_sc_e1001_a_route_without_a_running_stitch_is_refused() {
    let said = |elements: &[Element]| {
        let refused = routed(elements, Tool::Runs.defaults(), [None, None]);
        assert!(refused.pieces.is_empty() && refused.replaced.is_empty());
        refused.diagnostics.last().map(|said| (said.to_string(), said.fix.clone()))
    };
    let start = "error SC-E1001: There is nothing to route: Auto-Route Running Stitch routes running stitches, and";
    let hint = Some(stitchcraft_core::Fix::Hint("Give the running stitches to route.".to_string()));
    assert_eq!(said(&[]), Some((format!("{start} was given no element."), hint.clone())));
    let column = crate::route::tests::bar("column", (0.0, 10.0), 0.0);
    assert_eq!(said(std::slice::from_ref(&column)), Some((format!("{start} the element given is none."), hint.clone())));
    assert_eq!(said(&[column.clone(), column]), Some((format!("{start} none of the 2 elements given is one."), hint)));
}

#[test]
fn a_point_that_repeats_the_one_before_to_within_a_millionth_of_a_millimetre_is_left_out() {
    let element = stroke("s", &[(0.0, 0.0), (10.0, 0.0)], &[]);
    let routed = [Stroke { element: 0, stitch_type: StitchType::RunningStitch, repeats: 1, pieces: Vec::new() }];
    let made = |points: Vec<Point>| Made { stroke: 0, under: true, points };
    let piece =
        super::element(&made(vec![p(0.0, 0.0), p(5e-7, 0.0), p(1e-6, 0.0), p(10.0, 0.0)]), (&routed, std::slice::from_ref(&element)), 3, false)
            .unwrap();
    assert_eq!(lines_of(&piece.element), [vec![p(0.0, 0.0), p(1e-6, 0.0), p(10.0, 0.0)]]);
    assert_eq!(piece.element.name.as_deref(), Some("AutoRun Underpath 3"));
    // A run whose points are all one is none.
    assert!(super::element(&made(vec![p(0.0, 0.0), p(5e-7, 0.0)]), (&routed, std::slice::from_ref(&element)), 1, false).is_none());
}
