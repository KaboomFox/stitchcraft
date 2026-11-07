use stitchcraft_core::{Code, ElementId, Mm};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::{Rgb, Thread};

use super::*;
use crate::design::{Cap, Clip, FillRule, Hints, Join, Path, Segment, Shape, Subpath};

pub(super) fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

pub(super) fn line(points: &[(f64, f64)]) -> Subpath {
    let (first, rest) = points.split_first().unwrap();
    Subpath { start: p(first.0, first.1), segments: rest.iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(), closed: false }
}

pub(super) fn element(id: &str, shape: Shape, params: &[(&str, &str)]) -> Element {
    Element {
        id: ElementId::new(id).unwrap(),
        name: None,
        shape,
        thread: Thread::new(Rgb::new(10, 20, 30)),
        params: params.iter().copied().collect(),
        automatic: false,
        hints: Hints::default(),
        clips: Vec::new(),
    }
}

/// A satin column drawn with `lines`, its rails and rungs, and `params`.
pub(super) fn column(id: &str, lines: &[&[(f64, f64)]], params: &[(&str, &str)]) -> Element {
    let path = Path { subpaths: lines.iter().map(|points| line(points)).collect() };
    let params = [params, &[("satin_column", "true")]].concat();
    element(id, Shape::Stroke { path, width: Mm::SVG_PX, join: Join::UNSET, cap: Cap::Butt }, &params)
}

/// A column 3 mm wide from `top` to `bottom` between x = `left` and `left` + 3, with a rung across its middle.
pub(super) fn upright(id: &str, left: f64, (top, bottom): (f64, f64)) -> Element {
    let (right, middle) = (left + 3.0, (top + bottom) / 2.0);
    column(id, &[&[(left, top), (left, bottom)], &[(right, top), (right, bottom)], &[(left - 1.0, middle), (right + 1.0, middle)]], &[])
}

/// A column 3 mm tall from `left` to `right` between y = `top` and `top` + 3, with a rung across its middle.
pub(super) fn bar(id: &str, (left, right): (f64, f64), top: f64) -> Element {
    let (bottom, middle) = (top + 3.0, (left + right) / 2.0);
    column(id, &[&[(left, top), (right, top)], &[(left, bottom), (right, bottom)], &[(middle, top - 1.0), (middle, bottom + 1.0)]], &[])
}

/// A running stitch through `points`.
pub(super) fn stroke(id: &str, points: &[(f64, f64)], params: &[(&str, &str)]) -> Element {
    element(id, Shape::stroke(Path { subpaths: vec![line(points)] }), params)
}

pub(super) fn routed(elements: &[Element], options: Options, ends: [Option<Point>; 2]) -> Routed {
    route(elements, &Route { tool: Tool::Satins, options, start: ends[0], end: ends[1] }, &DesignSettings::default(), &Budget::DEFAULT)
}

/// Each piece: its name, the element it comes from, whether it runs under a satin, and whether it trims.
pub(super) fn summary(routed: &Routed) -> Vec<(String, usize, bool, bool)> {
    let trims = |e: &Element| e.params.get("trim_after") == Some("true");
    routed.pieces.iter().map(|piece| (piece.element.name.clone().unwrap_or_default(), piece.from, piece.under, trims(&piece.element))).collect()
}

/// The points of each subpath of `element`'s path.
pub(super) fn lines_of(element: &Element) -> Vec<Vec<Point>> {
    element.shape.path().subpaths.iter().map(|s| std::iter::once(s.start).chain(s.segments.iter().map(|g| g.end())).collect()).collect()
}

pub(super) fn near(a: Point, (x, y): (f64, f64)) -> bool {
    a.distance(p(x, y)) < 1e-9
}

/// 2 columns as Ink/Stitch's Auto-Route Satin Columns routes them, run as a black box at d59c9ab: an upright
/// column 20 mm tall, from y = 5 to 25 between x = 5 and 8, and a bar to its right from x = 15 to 25.
pub(super) fn two() -> Vec<Element> {
    vec![
        column("e0", &[&[(5.0, 5.0), (5.0, 25.0)], &[(8.0, 5.0), (8.0, 25.0)], &[(4.0, 15.0), (9.0, 15.0)]], &[]),
        column("e1", &[&[(15.0, 5.0), (25.0, 5.0)], &[(15.0, 8.0), (25.0, 8.0)], &[(20.0, 4.0), (20.0, 9.0)]], &[]),
    ]
}

#[test]
fn req_route_002_a_stretch_passed_twice_runs_under_the_satin_that_covers_it_as_ink_stitch_routes_it() {
    let routed = routed(&two(), Options::default(), [None, None]);
    // Ink/Stitch's route: the upright's first piece as satin, a run down its centre line to its foot, satin
    // back up over the run to the first piece, a trim, and the bar whole, trimmed after.
    assert_eq!(
        summary(&routed),
        [
            ("AutoSatin 1".to_string(), 0, false, false),
            ("AutoSatin Running Stitch 2".to_string(), 0, true, false),
            ("AutoSatin 3".to_string(), 0, false, true),
            ("AutoSatin 4".to_string(), 1, false, true),
        ]
    );
    assert_eq!(routed.replaced, [0, 1]);
    assert!(routed.diagnostics.is_empty(), "{:?}", routed.diagnostics);
    let pieces: Vec<Vec<Vec<Point>>> = routed.pieces.iter().map(|piece| lines_of(&piece.element)).collect();
    // The first piece ends at the cut at a 20th of the upright, its pair 2, 0.8 mm down, as Ink/Stitch cuts it
    // at the pair at that share of the pairs' points, and gets a rung across its start.
    assert!(near(pieces[0][0][0], (5.0, 5.0)) && near(pieces[0][0][1], (5.0, 5.8)) && near(pieces[0][1][1], (8.0, 5.8)), "{:?}", pieces[0]);
    assert_eq!(pieces[0].len(), 3);
    // The run goes down the centre line from the place 1 mm down, a piece's end, to the foot.
    let run = &pieces[1][0];
    assert!(near(run[0], (6.5, 6.0)) && near(*run.last().unwrap(), (6.5, 25.0)), "{run:?}");
    assert!(run.windows(2).all(|pair| pair[0].distance(pair[1]) >= 1e-6), "no point repeats");
    // It runs along each piece's stretch of the centre line once, from the piece's start to its end: straight down.
    assert!(run.windows(2).all(|pair| pair[1].x() == 6.5 && pair[1].y() > pair[0].y()), "{run:?}");
    // The satin back up runs from the foot to the cut, its own rung kept.
    assert!(near(pieces[2][0][0], (5.0, 25.0)) && near(pieces[2][0][1], (5.0, 5.8)) && near(pieces[2][2][0], (4.97, 15.0)), "{:?}", pieces[2]);
    // The bar is whole, as it was drawn but for its rung, drawn reaching a hundredth past each rail.
    assert!(near(pieces[3][0][0], (15.0, 5.0)) && near(pieces[3][1][1], (25.0, 8.0)) && near(pieces[3][2][0], (20.0, 4.97)), "{:?}", pieces[3]);
    // A run under a column takes no setting but its lengths, and a piece keeps every setting of its column.
    assert_eq!(routed.pieces[1].element.params, ParamSet::new());
    assert_eq!(routed.pieces[0].element.params.get("satin_column"), Some("true"));
    assert_eq!(routed.pieces[1].element.id.as_str(), "e0#2");
}

#[test]
fn req_route_001_a_column_is_cut_into_as_few_equal_pieces_as_keep_each_at_most_1_mm_long() {
    let count = |length: f64| {
        let routables =
            read::routables(&[bar("b", (0.0, length), 0.0)], &DesignSettings::default(), &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
        let routable = routables.first().unwrap();
        (routable.pieces, Along::new(&routable.line, &mut Budget::DEFAULT.meter()).unwrap().length())
    };
    // The centre line runs through the middles of the pairs, from end to end of a straight bar.
    let (pieces, length) = count(10.0);
    assert!(pieces == 10 && (length - 10.0).abs() < 1e-9, "{pieces} {length}");
    assert_eq!(count(10.5).0, 11);
    assert_eq!(count(0.5).0, 1);
    // Exactly 1 mm is 1 piece, and a hair more is 2.
    assert_eq!([0.2, 1.0, 1.0 + 1e-12, 2.0, 2.5].map(read::cut_into), [1, 1, 2, 2, 3]);
    assert_eq!(PIECE, 1.0);
    // Each piece runs from a fraction of the line to the next, the last to its very end.
    let routables = read::routables(&[bar("b", (0.0, 4.0), 0.0)], &DesignSettings::default(), &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
    let ends = routables[0].ends(&mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(ends.len(), 4);
    assert!(near(ends[0][0], (0.0, 1.5)) && near(ends[1][0], (1.0, 1.5)) && ends[1][0] == ends[0][1] && near(ends[3][1], (4.0, 1.5)), "{ends:?}");
}

#[test]
fn req_route_001_a_column_routed_whole_from_end_to_end_is_one_piece_with_its_own_rungs() {
    // A bar from the leftmost place to the rightmost: the way is every piece, sewn as one stretch.
    let routed = routed(&[bar("b", (0.0, 10.0), 0.0)], Options::default(), [None, None]);
    assert_eq!(summary(&routed), [("AutoSatin 1".to_string(), 0, false, true)]);
    let lines = lines_of(&routed.pieces[0].element);
    assert_eq!(lines.len(), 3, "its 2 rails and its rung: {lines:?}");
    assert!(near(lines[0][0], (0.0, 0.0)) && near(lines[0][1], (10.0, 0.0)) && near(lines[1][1], (10.0, 3.0)), "{lines:?}");
    assert!(near(lines[2][0], (5.0, -0.03)) && near(lines[2][1], (5.0, 3.03)), "{lines:?}");
}

#[test]
fn req_route_001_a_piece_at_an_end_of_its_column_reaches_the_end_of_its_rails_whatever_its_push_compensation() {
    // Push compensation shortens or lengthens a column at its ends, and its cuts with it: shortened by 1 mm,
    // the bar's first and last cuts lie 1 mm inside its rails' ends. Routed whole, its piece still runs from
    // end to end of the rails, and keeps the setting, which shortens it when it sews as it shortens the column.
    for push in ["1", "0.5 0", "0 0.5", "-1"] {
        let pushed = bar("b", (0.0, 10.0), 0.0).with(&[("push_compensation_mm", push)]);
        let routed = routed(&[pushed], Options::default(), [None, None]);
        assert_eq!(summary(&routed), [("AutoSatin 1".to_string(), 0, false, true)], "{push}");
        let lines = lines_of(&routed.pieces[0].element);
        let ends = [lines[0][0], lines[0][1], lines[1][0], lines[1][1]];
        assert!(ends.iter().zip([(0.0, 0.0), (10.0, 0.0), (0.0, 3.0), (10.0, 3.0)]).all(|(at, want)| near(*at, want)), "{push}: {lines:?}");
        assert_eq!(routed.pieces[0].element.params.get("push_compensation_mm"), Some(push));
    }
}

#[test]
fn a_route_s_pieces_keep_their_element_s_thread_and_a_column_s_its_clips_but_none_keeps_its_positions() {
    // The upright of `two` placed and clipped, as an Ink/Stitch file's commands and clip path place and clip
    // it. Its pieces start and end where the route cuts it. A column's piece is a copy of the column, its clip
    // with it, which leaves a satin column whole, and the run is a path of its own, which nothing clips.
    let mut elements = two();
    elements[0].hints = Hints { start: Some(p(6.5, 5.0)), end: Some(p(6.5, 25.0)), target: Some(p(6.5, 15.0)) };
    let square = Path { subpaths: vec![Subpath { closed: true, ..line(&[(0.0, 0.0), (30.0, 0.0), (30.0, 30.0), (0.0, 30.0)]) }] };
    elements[0].clips = vec![Clip::of(square, FillRule::NonZero)];
    let routed = routed(&elements, Options::default(), [None, None]);
    // Routed as `two` is: the upright's first piece, the run under it, the satin back up and the bar.
    assert_eq!(routed.pieces.iter().map(|piece| (piece.from, piece.under)).collect::<Vec<_>>(), [(0, false), (0, true), (0, false), (1, false)]);
    for piece in &routed.pieces {
        assert_eq!((&piece.element.thread, piece.element.hints), (&elements[piece.from].thread, Hints::default()), "{}", piece.element.id);
    }
    let clipped: Vec<usize> = routed.pieces.iter().map(|piece| piece.element.clips.len()).collect();
    assert_eq!(clipped, [1, 0, 1, 0]);
    assert_eq!(routed.pieces[0].element.clips, elements[0].clips);
}

#[test]
fn req_route_002_runs_take_the_column_s_lengths_and_a_piece_its_settings_but_odd_centre_walk_repeats() {
    let settings = |params: &[(&str, &str)]| {
        let mut elements = two();
        elements[0] = column("e0", &[&[(5.0, 5.0), (5.0, 25.0)], &[(8.0, 5.0), (8.0, 25.0)], &[(4.0, 15.0), (9.0, 15.0)]], params);
        let routed = routed(&elements, Options::default(), [None, None]);
        (routed.pieces[0].element.params.clone(), routed.pieces[1].element.params.clone())
    };
    let (piece, run) = settings(&[
        ("running_stitch_length_mm", "1.5"),
        ("center_walk_underlay_stitch_length_mm", "2"),
        ("min_jump_stitch_length_mm", "4"),
        ("bean_stitch_repeats", "2"),
        ("zigzag_spacing_mm", "0.5"),
    ]);
    let pairs = |set: &ParamSet| set.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>();
    assert_eq!(pairs(&run), ["bean_stitch_repeats=2", "min_jump_stitch_length_mm=4", "running_stitch_length_mm=1.5"]);
    assert_eq!(piece.get("zigzag_spacing_mm"), Some("0.5"));
    // Without its own length, a run takes the centre walk's, and then the contour's.
    assert_eq!(
        pairs(&settings(&[("center_walk_underlay_stitch_length_mm", "2"), ("contour_underlay_stitch_length_mm", "3")]).1),
        ["running_stitch_length_mm=2"]
    );
    assert_eq!(pairs(&settings(&[("contour_underlay_stitch_length_mm", "3")]).1), ["running_stitch_length_mm=3"]);
    // An odd number of centre walk repeats is lowered by 1, to no fewer than 2, and an even one kept.
    for (repeats, kept) in [("1", Some("2")), ("3", Some("2")), ("5", Some("4")), ("4", Some("4")), ("2", Some("2"))] {
        assert_eq!(settings(&[("center_walk_underlay_repeats", repeats)]).0.get("center_walk_underlay_repeats"), kept, "{repeats}");
    }
    assert_eq!(settings(&[]).0.get("center_walk_underlay_repeats"), None, "the default, 2, is even");
}

#[test]
fn req_route_002_a_stroke_among_the_columns_runs_each_time_the_route_passes_it() {
    // A stroke from the upright's foot to the bar's left end shows the needle's way, and the route runs
    // along it, with its own length, instead of jumping.
    let mut elements = two();
    elements.push(stroke("way", &[(6.5, 25.0), (15.0, 6.5)], &[("running_stitch_length_mm", "2"), ("repeats", "3")]));
    let along = routed(&elements, Options::default(), [None, None]);
    let runs: Vec<&Piece> = along.pieces.iter().filter(|piece| piece.from == 2).collect();
    assert_eq!(runs.len(), 1, "{:?}", summary(&along));
    assert!(!runs[0].under);
    assert_eq!(runs[0].element.params.iter().collect::<Vec<_>>(), [("running_stitch_length_mm", "2")]);
    let way = lines_of(&runs[0].element);
    assert!(near(way[0][0], (6.5, 25.0)) && near(way[0][1], (15.0, 6.5)), "{way:?}");
    // Drawn the other way, it runs the way the route goes.
    elements[2] = stroke("way", &[(15.0, 6.5), (6.5, 25.0)], &[]);
    let back = routed(&elements, Options::default(), [None, None]);
    let run = back.pieces.iter().find(|piece| piece.from == 2).unwrap();
    assert!(near(lines_of(&run.element)[0][0], (6.5, 25.0)), "{:?}", lines_of(&run.element));
}

#[test]
fn req_route_003_jumps_join_the_groups_that_lie_apart_as_short_in_all_as_they_can_be() {
    // 3 uprights: the middle one 5 mm right of the first and the last 6 mm right of the middle. A jump
    // from the first to the last would be 14 mm, so the jumps join the first to the middle and the middle to the
    // last, 5 and 6 mm between their centre lines, whatever order they are given in.
    let elements = [upright("a", 0.0, (0.0, 10.0)), upright("c", 17.0, (0.0, 10.0)), upright("b", 8.0, (0.0, 10.0))];
    let mut graph = Graph::new(false);
    let routables = read::routables(&elements, &DesignSettings::default(), &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap();
    for (r, routable) in routables.iter().enumerate() {
        for (index, ends) in routable.ends(&mut Budget::DEFAULT.meter()).unwrap().into_iter().enumerate() {
            graph.piece(r, index, ends);
        }
    }
    graph.join_apart(&mut Budget::DEFAULT.meter()).unwrap();
    let jumps: Vec<f64> = (0..60)
        .filter_map(|link| match graph.link(link) {
            Some(graph::Link::Jump { ends }) => Some(graph.places()[ends[0]].at.distance(graph.places()[ends[1]].at)),
            _ => None,
        })
        .collect();
    assert_eq!(jumps, [8.0, 9.0], "between the centre lines, x = 1.5, 9.5 and 18.5");
}

#[test]
fn req_route_003_with_preserve_order_the_elements_keep_their_order_and_gain_only_cuts_and_runs() {
    // Given right to left, the bars are sewn right to left, each joined to the next by its shortest jump.
    let elements = [bar("c", (40.0, 50.0), 0.0), bar("b", (20.0, 30.0), 0.0), bar("a", (0.0, 10.0), 0.0)];
    let options = Options { preserve_order: true, ..Options::default() };
    let routed = routed(&elements, options, [None, None]);
    let from: Vec<usize> = routed.pieces.iter().map(|piece| piece.from).collect();
    assert!(from.windows(2).all(|pair| pair[0] <= pair[1]), "{from:?}");
    assert_eq!(from.first(), Some(&0));
    assert_eq!(from.last(), Some(&2));
    // Without it, the route runs from the leftmost end to the rightmost.
    let free = routed_from(&elements, Options::default());
    assert_eq!(free, [2, 1, 0]);
}

/// The elements each piece of the route of `elements` comes from, in its order.
fn routed_from(elements: &[Element], options: Options) -> Vec<usize> {
    routed(elements, options, [None, None]).pieces.iter().map(|piece| piece.from).collect()
}

#[test]
fn req_route_004_a_route_starts_and_stops_at_the_ends_nearest_its_start_and_end() {
    let elements = [bar("a", (0.0, 10.0), 0.0), bar("b", (20.0, 30.0), 0.0)];
    // From the right: the route starts at the right bar's right end and ends at the left bar's left end.
    let routed = routed(&elements, Options::default(), [Some(p(31.0, 1.0)), Some(p(-1.0, 1.0))]);
    assert_eq!(routed.pieces.iter().map(|piece| piece.from).collect::<Vec<_>>(), [1, 0]);
    let first = lines_of(&routed.pieces[0].element);
    assert!(near(first[0][0], (30.0, 0.0)), "sewn from its right end: {first:?}");
    // Without them, from the leftmost to the rightmost.
    assert_eq!(routed_from(&elements, Options::default()), [0, 1]);
}

#[test]
fn req_route_004_with_preserve_order_a_route_starts_on_the_first_element_and_ends_on_the_last() {
    // The start lies at the second bar, but with Preserve order the route starts on the first.
    let elements = [bar("a", (0.0, 10.0), 0.0), bar("b", (20.0, 30.0), 0.0)];
    let options = Options { preserve_order: true, ..Options::default() };
    let routed = routed(&elements, options, [Some(p(25.0, 1.0)), None]);
    assert_eq!(routed.pieces.first().map(|piece| piece.from), Some(0));
    // It starts at the first bar's end nearest the start, and runs under it to its other end and back.
    let first = &routed.pieces[0];
    assert!(first.under && near(lines_of(&first.element)[0][0], (10.0, 1.5)), "{first:?}");
    assert_eq!(routed.pieces.last().map(|piece| piece.from), Some(1));
}

#[test]
fn req_route_004_a_single_column_with_a_start_and_no_end_runs_from_the_start_back_to_its_own_start() {
    let elements = [Element { name: Some("Bar".to_string()), ..bar("a", (0.0, 10.0), 0.0) }];
    let single = routed(&elements, Options::default(), [Some(p(6.0, 5.0)), None]);
    assert_eq!(single.pieces.len(), 2);
    // The run is a path of its own, which takes no name, though the column has one.
    let run = &single.pieces[0];
    assert!(run.under && run.element.name.is_none(), "{run:?}");
    let way = &lines_of(&run.element)[0];
    assert!(near(way[0], (6.0, 1.5)) && near(*way.last().unwrap(), (0.0, 1.5)), "{way:?}");
    // The column stays as it is, its id and name with it.
    assert_eq!(single.pieces[1].element, elements[0]);
    // A start nearest the column's start leaves the column alone.
    let alone = route(
        &elements,
        &Route { tool: Tool::Satins, options: Options::default(), start: Some(p(-3.0, 1.5)), end: None },
        &DesignSettings::default(),
        &Budget::DEFAULT,
    );
    assert_eq!(alone.pieces.iter().map(|piece| piece.element.clone()).collect::<Vec<_>>(), elements);
    // With an end too, it is routed as any route.
    let both = routed(&elements, Options::default(), [Some(p(6.0, 5.0)), Some(p(11.0, 1.5))]);
    assert_eq!(both.pieces.iter().filter(|piece| piece.under).count(), 1, "{:?}", summary(&both));
}

#[test]
fn req_route_005_a_jump_more_than_1_mm_outside_the_elements_trims_and_the_runs_after_a_trim_are_left_out() {
    // Bars end to end: the jump between their centre lines' ends trims when it is longer than 1 mm (exactly
    // 1 mm is in `sew`'s tests), and the last piece trims after it.
    let apart = |gap: f64| summary(&routed(&[bar("a", (0.0, 10.0), 0.0), bar("b", (10.0 + gap, 20.0), 0.0)], Options::default(), [None, None]));
    assert_eq!(apart(0.5).iter().map(|s| s.3).collect::<Vec<_>>(), [false, true]);
    assert_eq!(apart(2.0).iter().map(|s| s.3).collect::<Vec<_>>(), [true, true]);
    assert_eq!(TRIMMED, 1.0);
    // Routed from the bar's middle to the upright's foot, `two` runs under the bar's right half and sews the
    // bar back to its left end, jumps to the upright 1 mm down, runs up to its top and sews it down. The jump
    // trims, and the run right after it is left out.
    let runs = |options: Options| summary(&routed(&two(), options, [Some(p(20.0, 6.5)), Some(p(6.5, 25.0))])).iter().filter(|s| s.2).count();
    assert_eq!((runs(Options::default()), runs(Options { trim: false, ..Options::default() })), (1, 2));
    // Without trims, nothing trims.
    let untrimmed = routed(&two(), Options { trim: false, ..Options::default() }, [None, None]);
    assert!(summary(&untrimmed).iter().all(|s| !s.3));
}

#[test]
fn req_route_005_a_jump_within_a_stroke_s_hull_is_not_long() {
    // A stroke that winds round a bar and ends inside its own hull, 3 mm from the bar's end: the jump between
    // them runs inside the hull, and doesn't trim.
    let wound = stroke("s", &[(0.0, 0.0), (0.0, 10.0), (20.0, 10.0), (20.0, -10.0), (5.0, -10.0), (5.0, -3.0)], &[]);
    let trims = |s: Element| {
        summary(&routed(&[bar("a", (8.0, 18.0), -4.0), s], Options::default(), [None, None])).iter().map(|s| (s.1, s.3)).collect::<Vec<_>>()
    };
    assert_eq!(trims(wound), [(1, false), (0, true)]);
    // A straight stroke to the same end has a hull that is a line, outside which the jump runs.
    assert_eq!(trims(stroke("s", &[(0.0, -3.0), (5.0, -3.0)], &[])), [(1, true), (0, true)]);
}

#[test]
fn req_route_006_a_route_is_the_same_each_time_and_runs_out_of_work_without_a_piece() {
    let elements = crate::testsheets::find("TS-17C").unwrap().design().unwrap().unwrap().elements().to_vec();
    let drawn: Vec<Element> = elements.into_iter().filter(|e| e.id.as_str().starts_with("ts17c:drawn:")).collect();
    let once = routed(&drawn, Options::default(), [None, None]);
    assert_eq!(once, routed(&drawn, Options::default(), [None, None]));
    let small = Budget { max_work: 10_000, ..Budget::DEFAULT };
    let out = route(&drawn, &Route { tool: Tool::Satins, options: Options::default(), start: None, end: None }, &DesignSettings::default(), &small);
    assert!(out.pieces.is_empty() && out.replaced.is_empty());
    assert_eq!(
        out.diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["error SC-E0004: Routing these elements needed more than the work budget of 10000 units, so they are left as they were."]
    );
}

#[test]
fn req_route_007_a_route_takes_columns_and_strokes_and_leaves_the_rest_as_they_are() {
    let square = Path { subpaths: vec![Subpath { closed: true, ..line(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]) }] };
    let fill = element("fill", Shape::Fill { path: square, rule: FillRule::EvenOdd }, &[]);
    let elements = [fill, bar("a", (0.0, 10.0), 10.0), stroke("dot", &[(1.0, 1.0), (1.0, 1.0)], &[])];
    let routed = routed(&elements, Options::default(), [None, None]);
    assert_eq!(routed.replaced, [1]);
    assert_eq!(routed.pieces.iter().map(|piece| piece.from).collect::<Vec<_>>(), [1]);
    let said: Vec<(Code, String)> = routed.diagnostics.iter().map(|d| (d.code, d.element.as_ref().unwrap().to_string())).collect();
    assert_eq!(said, [(Code::RouteLeftOut, "fill".to_string()), (Code::RouteLeftOut, "dot".to_string())]);
}

#[test]
fn req_route_007_pieces_are_named_and_with_preserve_order_keep_their_column_s_name() {
    let mut elements = two();
    elements[0].name = Some("Upright".to_string());
    let names = |options: Options| summary(&routed(&elements, options, [None, None])).into_iter().map(|s| s.0).collect::<Vec<_>>();
    assert_eq!(names(Options::default()), ["AutoSatin 1", "AutoSatin Running Stitch 2", "AutoSatin 3", "AutoSatin 4"]);
    let kept = Options { preserve_order: true, ..Options::default() };
    assert_eq!(names(kept), ["Upright", "AutoSatin Running Stitch 1", "Upright", "AutoSatin 2"]);
}

#[test]
fn diag_sc_e1001_a_route_without_a_satin_column_is_refused() {
    let said = |elements: &[Element]| routed(elements, Options::default(), [None, None]).diagnostics.last().map(ToString::to_string);
    assert_eq!(
        said(&[]).as_deref(),
        Some("error SC-E1001: There is nothing to route: Auto-Route Satin Columns routes satin columns, and was given no element.")
    );
    let line = stroke("s", &[(0.0, 0.0), (5.0, 0.0)], &[]);
    assert_eq!(
        said(std::slice::from_ref(&line)).as_deref(),
        Some("error SC-E1001: There is nothing to route: Auto-Route Satin Columns routes satin columns, and the element given is none.")
    );
    assert_eq!(
        said(&[line.clone(), line]).as_deref(),
        Some("error SC-E1001: There is nothing to route: Auto-Route Satin Columns routes satin columns, and none of the 2 elements given is one.")
    );
    let refused = routed(&[], Options::default(), [None, None]);
    assert!(refused.pieces.is_empty() && refused.replaced.is_empty());
}

#[test]
fn diag_sc_i1002_an_element_the_route_does_not_take_is_named_with_why() {
    let read = |element: Element| {
        let mut said = Vec::new();
        let taken = read::routables(std::slice::from_ref(&element), &DesignSettings::default(), &mut said, &mut Budget::DEFAULT.meter());
        assert!(taken.unwrap().is_empty(), "{element:?}");
        said.into_iter().map(|d| d.to_string()).collect::<Vec<_>>()
    };
    let start = "info SC-I1002: This element is left out of the route and kept as it is: ";
    let square = Path { subpaths: vec![Subpath { closed: true, ..line(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]) }] };
    assert_eq!(
        read(element("f", Shape::Fill { path: square.clone(), rule: FillRule::NonZero }, &[])),
        [format!("{start}it is a fill, which a route doesn't take.")]
    );
    let automatic = Element { automatic: true, ..stroke("s", &[(0.0, 0.0), (5.0, 0.0)], &[]) };
    assert_eq!(read(automatic), [format!("{start}it is digitized automatically, which chooses its own stitches.")]);
    assert_eq!(read(stroke("s", &[(0.0, 0.0), (5.0, 0.0)], &[("satin_column", "maybe")])), [format!("{start}its settings don't read.")]);
    assert_eq!(read(bar("b", (0.0, 5.0), 0.0).with(&[("zigzag_spacing_mm", "wide")])), [format!("{start}its settings don't read.")]);
    // Rails that are points: what is said of them is kept but for the error, which `SC-I1002` stands for.
    let points = column("p", &[&[(1.0, 1.0), (1.0, 1.0)], &[(2.0, 2.0), (2.0, 2.0)]], &[]);
    let said = read(points);
    assert_eq!(said.last().unwrap(), &format!("{start}its rails can't be made."));
    assert!(said.len() == 3 && said[..2].iter().all(|said| said.starts_with("warning SC-W0205")), "{said:?}");
    // A column so short that it has 1 pair.
    assert_eq!(read(bar("b", (0.0, 0.05), 0.0)), [format!("{start}its centre line has no length.")]);
    assert_eq!(read(stroke("s", &[(1.0, 1.0), (1.0, 1.0)], &[])), [format!("{start}it has no length.")]);
    let closed = stroke("s", &[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0), (0.0, 0.0)], &[]);
    assert_eq!(read(closed), [format!("{start}its first subpath ends where it starts, which shows no way between 2 places.")]);
    let ring = element("r", Shape::stroke(square), &[]);
    assert_eq!(read(ring), [format!("{start}its first subpath ends where it starts, which shows no way between 2 places.")]);
}

trait With {
    /// The element with `params` set too.
    fn with(self, params: &[(&str, &str)]) -> Element;
}

impl With for Element {
    fn with(mut self, params: &[(&str, &str)]) -> Element {
        for (key, value) in params {
            self.params.set(*key, *value);
        }
        self
    }
}

#[test]
fn diag_sc_w1003_a_piece_read_back_otherwise_is_kept_and_named() {
    // The upright of `two` with rails that cross 10 mm down, and rungs 0.4 and 15 mm down. Routed as `two`
    // is, its piece from the cut 0.8 mm down to its foot has 1 rung, and rails that meet each other as well
    // as the rung: read back, its rails are taken by their length.
    let mut elements = two();
    elements[0] = column("x", &[&[(5.0, 5.0), (8.0, 25.0)], &[(8.0, 5.0), (5.0, 25.0)], &[(4.0, 5.4), (9.0, 5.4)], &[(4.0, 20.0), (9.0, 20.0)]], &[]);
    let routed = routed(&elements, Options::default(), [None, None]);
    assert_eq!(routed.pieces.len(), 4, "{:?}", summary(&routed));
    assert_eq!(routed.diagnostics.len(), 1, "{:?}", routed.diagnostics);
    let said = routed.diagnostics.first().unwrap();
    assert_eq!(
        said.to_string(),
        "warning SC-W1003: A piece of this satin column, cut for the route, would be read back with other rails or rungs, so it may sew \
         otherwise than the column does there."
    );
    assert_eq!(said.element.as_ref().map(ElementId::as_str), Some("x"));
}

#[test]
fn the_options_are_ink_stitch_s_with_their_labels_and_defaults() {
    let named: Vec<(&str, &str, bool)> = OPTIONS.iter().map(|option| (option.name, option.label, option.default)).collect();
    assert_eq!(
        named,
        [
            ("trim", "Trim jump stitches", true),
            ("preserve_order", "Preserve order of satin columns", false),
            ("keep_originals", "Keep original paths", false)
        ]
    );
    assert_eq!(Options::default(), Options { trim: true, preserve_order: false, keep_originals: false });
    let mut options = Options::default();
    for option in &OPTIONS {
        let was = option.get(&options);
        option.set(&mut options, !was);
        assert_eq!(option.get(&options), !was, "{}", option.name);
        assert!(!option.help.is_empty());
    }
    assert_eq!(options, Options { trim: false, preserve_order: true, keep_originals: true });
    assert_eq!(RouteOption::named("trim").map(|option| option.label), Some("Trim jump stitches"));
    assert!(RouteOption::named("sort").is_none());
}

#[test]
fn a_route_with_no_way_from_its_start_to_its_end_says_so() {
    assert_eq!(
        lost(Vec::new()).diagnostics.iter().map(ToString::to_string).collect::<Vec<_>>(),
        ["error SC-E0009: The route found no way from its start to its end, though its jumps join every piece."]
    );
}

#[test]
fn req_route_002_a_stroke_joining_the_ends_of_a_column_s_piece_is_sewn_and_so_is_the_piece() {
    // The stroke joins the bar's places 3 and 4 mm along, as the bar's piece between them does: the route
    // runs along the stroke and back, and sews the bar whole as satin (`DEV-ROUTE-005`).
    let elements = [bar("a", (0.0, 10.0), 0.0), stroke("s", &[(3.0, 1.5), (4.0, 1.5)], &[])];
    let both = routed(&elements, Options::default(), [None, None]);
    assert_eq!(summary(&both).iter().map(|s| (s.1, s.2)).collect::<Vec<_>>(), [(0, false), (1, false), (1, false), (0, false)]);
    let satin: Vec<Vec<Point>> = both.pieces.iter().filter(|piece| piece.from == 0).map(|piece| lines_of(&piece.element)[0].clone()).collect();
    assert!(near(satin[0][0], (0.0, 0.0)) && near(satin[0][1], (satin[1][0].x(), satin[1][0].y())) && near(satin[1][1], (10.0, 0.0)), "{satin:?}");
}
