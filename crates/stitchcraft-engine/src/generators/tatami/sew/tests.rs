//! Unit tests of sewing a part's route; the conformance cases are in `tests/fill_routing.rs`.

use stitchcraft_core::Budget;

use super::*;
use crate::generators::tatami::fixture::{frame, p, rectangle};
use crate::generators::tatami::route::Node;
use crate::generators::tatami::rows::Grid;
use crate::normalize::stroke::distance_to_segment;

/// Rows 1 mm apart in stitches of 3 mm, and travel in stitches of 2 mm, nothing thinned.
fn stitching(skip_last: bool) -> Stitching {
    Stitching { grid: Grid { angle: 0.0, spacing: 1.0, end_spacing: None }, length: 3.0, staggers: 4.0, skip_last, jitter: None }
}

const TRAVEL: Travel = Travel { length: 2.0, tolerance: 0.2, min_stitch: 1e-6, underpath: false, jump: 3.0 };

/// `route` through `segments` sewn in `part`, whose rings are `rings`, as `how` says: travelling along
/// the rings and rows, from no needle, with no pull compensation and no gap rows.
fn plain<'a>(route: &'a Route, rings: &'a Rings<'a>, (segments, rows): &'a RowSegments, how: &'a Stitching, meter: &mut Meter) -> Sewing<'a> {
    let network = Rows::Straight(segments).network(&route.nodes, rings, &row_steps(route), meter).unwrap();
    let ways = Ways { route, rings, travel: TRAVEL, lines: None, network, start: None };
    Sewing { ways, stitching: how, segments, rows, pull: Pull::default(), joints: joints(segments, meter).unwrap(), gap_rows: 0 }
}

/// `sewing` travelling as `travel` says, under `lines` where there are any.
fn travelling(mut sewing: Sewing<'_>, travel: Travel, lines: Option<Underpath>) -> Sewing<'_> {
    (sewing.ways.travel, sewing.ways.lines) = (travel, lines);
    sewing
}

/// The runs `route` through `segments`, one to a row, sews in the part of `rings`, travelling along its
/// rings and rows.
fn sew(
    route: &Route,
    rings: &Rings<'_>,
    segments: &[(Point, Point)],
    stitching: &Stitching,
    meter: &mut Meter,
    warnings: &mut Vec<Diagnostic>,
) -> Vec<Vec<Point>> {
    let segments = (segments.to_vec(), (0..segments.len()).collect());
    plain(route, rings, &segments, stitching, meter).sew(&mut SplitMix64::new(1), meter, warnings).unwrap()
}

#[test]
fn with_skip_last_a_row_s_end_is_sewn_before_travel_unless_the_next_row_starts_beside_it() {
    // 3 rows across a 10 × 4 rectangle. Routed, the bottom row hands over to the middle row, which
    // starts beside its end, and the middle row to the far end of the top row, round the top corners.
    let part = rectangle(4.0);
    let segments = [1.0, 2.0, 3.0].map(|y| (p(0.0, y), p(10.0, y)));
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let route = route(&rings, Rows::Straight(&segments), None, None, meter).unwrap().unwrap();
    let stitching = stitching(true);
    let mut warnings = Vec::new();
    let runs = sew(&route, &rings, &segments, &stitching, meter, &mut warnings);
    assert_eq!((runs.len(), warnings), (1, Vec::new()));
    let run = &runs[0];
    let mut handovers = Vec::new();
    for pair in route.steps.windows(2) {
        let [Step::Row { from, to, .. }, Step::Travel { beside, .. }] = *pair else { continue };
        let (start, end) = (route.nodes[from].point, route.nodes[to].point);
        // The row's last needle point, short of its end, and what comes after it.
        let last = *row_points(start, end, &stitching, &mut SplitMix64::new(1), meter).unwrap().last().unwrap();
        let after = run.iter().position(|&q| q == last).map(|i| run[i + 1]).unwrap();
        assert_eq!(after == end, !beside, "{:?}", route.steps);
        handovers.push(beside);
    }
    assert_eq!(handovers, [true, false]);
}

#[test]
fn the_network_runs_along_the_rows_the_route_sews() {
    let steps = vec![
        Step::Travel { from: 4, to: 0, beside: false, leaves: false },
        Step::Row { segment: 1, from: 0, to: 1 },
        Step::Travel { from: 1, to: 2, beside: true, leaves: false },
        Step::Row { segment: 0, from: 3, to: 2 },
    ];
    assert_eq!(row_steps(&Route { nodes: Vec::new(), steps }), [(1, [0, 1]), (0, [3, 2])]);
}

#[test]
fn a_needle_that_finds_no_way_jumps_and_says_so_as_a_bug() {
    // A row between 2 points of the frame's outline and one between 2 points of its hole, which nothing
    // joins: the travel from the first to the second finds no way.
    let part = frame();
    let segments = [(p(0.0, 0.5), p(10.0, 0.5)), (p(4.0, 2.0), p(6.0, 2.0))];
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let route = route(&rings, Rows::Straight(&segments), None, None, meter).unwrap().unwrap();
    let mut warnings = Vec::new();
    let runs = sew(&route, &rings, &segments, &stitching(false), meter, &mut warnings);
    assert_eq!(runs.len(), 2, "the needle jumps once");
    assert_eq!((runs[0].first(), runs[1].first()), (Some(&p(0.0, 0.5)), Some(&p(6.0, 2.0))));
    assert_eq!(warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
    // With 0.5 mm of pull compensation at each end, the needle still starts on the route's first node, and
    // after the jump at the next row's lengthened start.
    let (rows, how) = ((segments.to_vec(), vec![0, 1]), stitching(false));
    let pull = Pull { mm: [0.5, 0.5], share: [0.0, 0.0] };
    let runs = Sewing { pull, ..plain(&route, &rings, &rows, &how, meter) }.sew(&mut SplitMix64::new(1), meter, &mut warnings).unwrap();
    assert_eq!((runs[0].first(), runs[1].first()), (Some(&p(0.0, 0.5)), Some(&p(6.5, 2.0))));
}

/// Whether `q` lies within `within` of `part`'s rings or in it.
fn near_or_in(part: &Polygon, q: Point, within: f64) -> bool {
    part.rings().any(|ring| ring.windows(2).any(|side| distance_to_segment(q, side[0], side[1]) <= within)) || part.covers(q)
}

#[test]
fn req_fill_tat_005_every_way_under_the_rows_lies_inside_the_part() {
    // Rows 0.25 apart across the frame at 30°, sewn in the route's order: every way the needle travels
    // lies in the part or within 0.05 mm of its rings, though smoothing cuts the lines' corners round
    // the hole.
    let part = frame();
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let how = Stitching { grid: Grid { angle: 30.0, spacing: 0.25, end_spacing: None }, ..stitching(false) };
    let rows = segments(&part, &how, meter).unwrap();
    let route = route(&rings, Rows::Straight(&rows.0), None, None, meter).unwrap().unwrap();
    let lines = Underpath::new(&part, &rings, &route.nodes, Rows::Straight(&rows.0), 30.0, meter).unwrap();
    let travel = Travel { underpath: true, ..TRAVEL };
    let mut sewing = travelling(plain(&route, &rings, &rows, &how, meter), travel, lines);
    let mut ways = 0;
    for step in &route.steps {
        match *step {
            Step::Row { segment, .. } => {
                sewing.ways.sewn(segment);
            }
            Step::Travel { from, to, .. } => {
                let way = sewing.ways.way(from, to, meter).unwrap().unwrap();
                assert_eq!((way.first(), way.last()), (Some(&route.nodes[from].point), Some(&route.nodes[to].point)));
                for pair in way.windows(2) {
                    for q in [pair[0], pair[0].lerp(pair[1], 0.5)] {
                        assert!(near_or_in(&part, q, 0.05), "{q:?} in {way:?}");
                    }
                }
                ways += 1;
            }
        }
    }
    assert!(ways > 3, "{ways}");
    // Sewn as a whole, the part is one run, which says nothing.
    let lines = Underpath::new(&part, &rings, &route.nodes, Rows::Straight(&rows.0), 30.0, meter).unwrap();
    let mut warnings = Vec::new();
    let mut sewing = travelling(plain(&route, &rings, &rows, &how, meter), travel, lines);
    assert_eq!(sewing.sew(&mut SplitMix64::new(1), meter, &mut warnings).unwrap().len(), 1);
    assert_eq!(warnings, Vec::new());
}

#[test]
fn req_fill_tat_001_a_needle_inside_leads_in_from_the_lines_nearest_node_to_the_first_row() {
    // The needle inside a 10 mm square, 3 mm from its left side, and lines under rows 1 mm apart. The
    // route starts with travel from the left side to the first row, and the lead-in takes its place: from
    // the lines' node nearest the needle to the row's start. Without that travel the route starts with
    // the row, which it keeps, and the lead-in ends at the row's start too.
    let part = rectangle(10.0);
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let how = stitching(false);
    let rows = segments(&part, &how, meter).unwrap();
    let needle = p(3.0, 4.6);
    let mut route = route(&rings, Rows::Straight(&rows.0), Some(needle), None, meter).unwrap().unwrap();
    let Some(&Step::Row { from: start, .. }) = route.steps.get(1) else { panic!("{:?}", route.steps) };
    let at = route.nodes[start].point;
    let travel = Travel { underpath: true, ..TRAVEL };
    for replaces in [true, false] {
        if !replaces {
            route.steps.remove(0);
        }
        let lines = Underpath::new(&part, &rings, &route.nodes, Rows::Straight(&rows.0), 0.0, meter).unwrap();
        let mut sewing = travelling(plain(&route, &rings, &rows, &how, meter), travel, lines);
        sewing.ways.start = Some(needle);
        let (way, replaced) = sewing.ways.lead_in(meter).unwrap().unwrap();
        // The needle is 3 mm or more from every side, so a node within 1 mm of it is well inside.
        assert!(way[0].distance(needle) < 1.0, "{:?}", way[0]);
        assert_eq!((way.last(), replaced), (Some(&at), replaces));
        let runs = sewing.sew(&mut SplitMix64::new(1), meter, &mut Vec::new()).unwrap();
        assert_eq!((runs.len(), runs[0].first()), (1, way.first()));
        let row = runs[0].iter().position(|&q| q == at).unwrap();
        assert!((runs[0][row + 1].y() - at.y()).abs() < 1e-9, "the row follows the lead-in");
    }
}

/// A 10 × 9 rectangle's rows at y = 1 to 8, each from x = 0 to 10, sewn in the order `order` gives,
/// counted from 0: the first from the left, each of the others from the side the one before ended on, with
/// travel along the outline between them. The runs it sews as `how` says, with `gap_rows` and `pull`.
fn sew_rows(order: &[usize], how: &Stitching, gap_rows: usize, pull: Pull) -> Vec<Vec<Point>> {
    let part = rectangle(9.0);
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let segments: Vec<(Point, Point)> = (1..=8).map(|y| (p(0.0, f64::from(y)), p(10.0, f64::from(y)))).collect();
    // Row k's left end is node 2k, and its right end node 2k + 1.
    let nodes = segments.iter().flat_map(|&(a, b)| [a, b]).map(|q| Node { point: q, place: rings.locate(q, |_| true, meter).unwrap().unwrap() });
    let mut steps = Vec::new();
    for (k, &row) in order.iter().enumerate() {
        let (from, to) = if k % 2 == 0 { (2 * row, 2 * row + 1) } else { (2 * row + 1, 2 * row) };
        if let Some(&Step::Row { segment, to: end, .. }) = steps.last() {
            let beside = row.abs_diff(segment) == 1;
            steps.push(Step::Travel { from: end, to: from, beside, leaves: !beside });
        }
        steps.push(Step::Row { segment: row, from, to });
    }
    let route = Route { nodes: nodes.collect(), steps };
    let rows = (segments, (0..8).collect());
    let mut warnings = Vec::new();
    let mut sewing = Sewing { pull, gap_rows, ..plain(&route, &rings, &rows, how, meter) };
    let runs = sewing.sew(&mut SplitMix64::new(1), meter, &mut warnings).unwrap();
    assert_eq!(warnings, Vec::new());
    runs
}

/// [`sew_rows`] in 2 stretches: rows 1 to `first`, travel along the outline to the end of row 8 on the
/// same side, and rows 8 down to `first + 1`.
fn two_stretches(first: usize, gap_rows: usize, pull: Pull) -> Vec<Vec<Point>> {
    let order: Vec<usize> = (0..first).chain((first..8).rev()).collect();
    sew_rows(&order, &stitching(false), gap_rows, pull)
}

/// Whether `run` passes through `points` in that order.
fn passes(run: &[Point], points: &[(f64, f64)]) -> bool {
    let mut rest = run.iter();
    points.iter().all(|&(x, y)| rest.any(|q| q.distance(p(x, y)) < 1e-9))
}

/// Whether `run` has a stitch from `a` to `b`.
fn has_stitch(run: &[Point], a: (f64, f64), b: (f64, f64)) -> bool {
    run.windows(2).any(|s| s[0].distance(p(a.0, a.1)) < 1e-9 && s[1].distance(p(b.0, b.1)) < 1e-9)
}

#[test]
fn req_fill_tat_007_leaving_a_stretch_of_more_than_3_rows_sews_the_gap_rows_past_its_last_and_comes_back() {
    // Rows 1 to 5: leaving row 5's right end, 2 rows more, 6 back from the right and 7 from the left, then
    // straight back to row 5's end, and on round the outline to row 8.
    let run = two_stretches(5, 2, Pull::default()).concat();
    assert!(passes(&run, &[(10.0, 5.0), (10.0, 6.0), (0.0, 6.0), (0.0, 7.0), (10.0, 7.0), (10.0, 5.0), (10.0, 8.0)]), "{run:?}");
    // Without gap rows, row 6 is sewn once, after rows 8 and 7.
    let plain = two_stretches(5, 0, Pull::default()).concat();
    assert!(passes(&plain, &[(10.0, 5.0), (10.0, 8.0), (0.0, 8.0), (0.0, 7.0), (10.0, 7.0), (10.0, 6.0), (0.0, 6.0)]), "{plain:?}");
    // Between them, the gap rows' needle points as any row's, and the way back's one stitch.
    let rng = &mut SplitMix64::new(0);
    let mut row = |a: (f64, f64), b: (f64, f64)| row_points(p(a.0, a.1), p(b.0, b.1), &stitching(false), rng, &mut Budget::DEFAULT.meter()).unwrap();
    let gap: Vec<Point> = [row((10.0, 6.0), (0.0, 6.0)), row((0.0, 7.0), (10.0, 7.0)), vec![p(10.0, 5.0)]].concat();
    let from = run.iter().position(|q| *q == p(10.0, 5.0)).unwrap() + 1;
    assert_eq!(run[from..from + gap.len()], gap);
    assert_eq!(run.len(), plain.len() + gap.len());
}

#[test]
fn req_fill_tat_007_a_stretch_of_4_rows_has_gap_rows_and_one_of_3_has_none() {
    // The route's first row counts as reached by travel, as Ink/Stitch's route always starts with travel to
    // it, of no length here. After rows 1 to 4, rows 5 and 6, then back to row 4's left end and on up the
    // outline to row 8.
    let run = two_stretches(4, 2, Pull::default()).concat();
    assert!(passes(&run, &[(0.0, 4.0), (0.0, 5.0), (10.0, 5.0), (10.0, 6.0), (0.0, 6.0), (0.0, 4.0), (0.0, 8.0)]), "{run:?}");
    // Rows 1 to 3, then rows 8 down to 4, which end the route.
    assert_eq!(two_stretches(3, 2, Pull::default()), two_stretches(3, 0, Pull::default()));
}

#[test]
fn req_fill_tat_007_the_travel_to_the_end_from_beside_it_ends_no_stretch() {
    // A 10 × 9 rectangle's rows at y = 0 to 8, routed to an end on its left side at y = 3.5: rows 8 down
    // to 4, travel to row 0, and rows 0 to 3 upwards, which end at row 3's left end, beside the end. The
    // travel there is one edge of the outline, and in Ink/Stitch's route such travel ends no stretch.
    let part = rectangle(9.0);
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let rows = segments(&part, &stitching(false), meter).unwrap();
    let route = route(&rings, Rows::Straight(&rows.0), None, Some(p(-5.0, 3.5)), meter).unwrap().unwrap();
    let Some(&Step::Travel { from, beside, leaves, .. }) = route.steps.last() else { panic!("{:?}", route.steps) };
    assert_eq!((route.nodes[from].point, beside, leaves), (p(0.0, 3.0), true, false));
    let mut sewn = |route: &Route, skip_last| {
        let how = stitching(skip_last);
        let mut sewing = Sewing { gap_rows: 2, ..plain(route, &rings, &rows, &how, meter) };
        sewing.sew(&mut SplitMix64::new(1), meter, &mut Vec::new()).unwrap().concat()
    };
    // Leaving the first stretch, rows 3 and 2 fill its gap. Ending the second, none do: the needle comes
    // along row 3 to its end and goes straight on to the end.
    let heights = |run: &[Point]| run[run.len() - 3..].iter().map(|q| q.y()).collect::<Vec<_>>();
    assert_eq!(heights(&sewn(&route, false)), [3.0, 3.0, 3.5]);
    // Were that travel more than a handover, rows 4 and 5 would fill the gap past row 3 first, and the
    // needle would come back down from row 5.
    let mut leaving = route.clone();
    if let Some(Step::Travel { leaves, .. }) = leaving.steps.last_mut() {
        *leaves = true;
    }
    assert_eq!(heights(&sewn(&leaving, false)), [5.0, 3.0, 3.5]);
    // With `skip_last`, the travel still sews row 3's end, as there is no next row to hand over to.
    assert!(sewn(&route, true).contains(&p(0.0, 3.0)));
}

#[test]
fn req_fill_tat_007_past_the_part_s_end_there_are_no_gap_rows_and_the_needle_sews_as_without() {
    // Rows 4 to 8 upwards, then 1 to 3: past row 8 the part ends. With `skip_last` and stitches longer
    // than the rows, the needle leaves the stretch well short of row 8's end, and goes there in one stitch.
    let (order, how) = ([3, 4, 5, 6, 7, 0, 1, 2], Stitching { length: 20.0, ..stitching(true) });
    let run = sew_rows(&order, &how, 2, Pull::default()).concat();
    assert_eq!(run, sew_rows(&order, &how, 0, Pull::default()).concat());
    let end = run.iter().position(|q| *q == p(10.0, 8.0)).unwrap();
    assert!(run[end - 1].distance(run[end]) > TRAVEL.length, "{run:?}");
}

#[test]
fn req_fill_tat_006_rows_and_gap_rows_reach_past_their_ends_and_travel_joins_the_ends() {
    // 0.5 mm past each row's left end and 0.25 mm plus a tenth of its 10 mm past its right end: rows
    // from x = -0.5 to 11.25, gap rows too, each handing over straight to the next.
    let pull = Pull { mm: [0.5, 0.25], share: [0.0, 0.1] };
    let run = two_stretches(5, 2, pull).concat();
    let ends = [(11.25, 5.0), (11.25, 6.0), (-0.5, 6.0), (-0.5, 7.0), (11.25, 7.0), (11.25, 5.0), (11.25, 8.0), (-0.5, 8.0)];
    assert!(passes(&run, &ends), "{run:?}");
    assert!(run.iter().all(|q| (-0.5 - 1e-9..=11.25 + 1e-9).contains(&q.x())), "{run:?}");
    // Each handover is one stitch, from a row's lengthened end to the next row's lengthened start.
    for (end, start) in [((11.25, 1.0), (11.25, 2.0)), ((-0.5, 2.0), (-0.5, 3.0)), ((11.25, 7.0), (11.25, 6.0))] {
        assert!(has_stitch(&run, end, start), "{end:?} to {start:?}: {run:?}");
    }
    // 12 mm past each row's left end, more than the row's own 10 mm: each row is still sewn from the end the
    // route enters it by, row 1 rightwards and row 2 back.
    let far = two_stretches(5, 0, Pull { mm: [12.0, 0.0], share: [0.0, 0.0] }).concat();
    assert!(passes(&far, &[(-12.0, 1.0), (10.0, 1.0), (10.0, 2.0), (-12.0, 2.0)]), "{far:?}");
}

#[test]
fn where_no_line_leads_the_needle_runs_along_the_rings_and_rows() {
    // A hole 0.2 mm across in the middle of a triangle the lines leave between them: the part is small,
    // so they lie 1 mm and 0.71 mm apart, and none reaches the hole. From the hole's left side the needle
    // runs along the row through it to the outline, and up the outline's left side.
    let hole = vec![p(4.44, 2.02), p(4.64, 2.02), p(4.64, 2.22), p(4.44, 2.22), p(4.44, 2.02)];
    let part = Polygon { holes: vec![hole], ..rectangle(4.0) };
    let meter = &mut Budget::DEFAULT.meter();
    let rings = Rings::new(&part, meter).unwrap();
    let points = [p(4.44, 2.12), p(0.0, 2.12), p(0.0, 1.0)];
    let nodes = points.map(|q| Node { point: q, place: rings.locate(q, |_| true, meter).unwrap().unwrap() }).to_vec();
    let route = Route { nodes, steps: Vec::new() };
    let lines = Underpath::new(&part, &rings, &route.nodes, Rows::Straight(&[(points[1], points[0])]), 0.0, meter).unwrap().unwrap();
    assert_eq!(lines.way(0, 2, meter).unwrap(), None, "no line reaches the hole");
    let network = Network::new(&route.nodes, &rings, &[[1, 0]], meter).unwrap();
    let (travel, how, rows) = (Travel { underpath: true, ..TRAVEL }, stitching(false), (vec![(points[1], points[0])], vec![0]));
    let mut sewing = travelling(plain(&route, &rings, &rows, &how, meter), travel, Some(lines));
    sewing.ways.network = network;
    assert_eq!(sewing.ways.way(0, 2, meter).unwrap(), Some(points.to_vec()));
}

/// What [`turned`] makes of the run through `run`, with a shortest stitch of 0.3 mm.
fn turns_kept(run: &[(f64, f64)]) -> Vec<Point> {
    turned(vec![run.iter().map(|&(x, y)| p(x, y)).collect()], 0.3, &mut Budget::DEFAULT.meter()).unwrap().remove(0)
}

/// Whether `run` is the needle points `points`, to a nanometre.
fn is(run: &[Point], points: &[(f64, f64)]) -> bool {
    run.len() == points.len() && run.iter().zip(points).all(|(q, &(x, y))| q.distance(p(x, y)) < 1e-9)
}

#[test]
fn req_fill_tat_014_a_turn_shorter_than_the_shortest_stitch_becomes_a_stitch_that_long() {
    // 3 rows 0.25 mm apart, sewn right, left and right: each turn's 2 points move 0.025 mm apart along the
    // side, and the turn is then 0.3 mm long.
    let rows = turns_kept(&[(0.0, 0.0), (10.0, 0.0), (10.0, 0.25), (0.0, 0.25), (0.0, 0.5), (10.0, 0.5)]);
    assert!(is(&rows, &[(0.0, 0.0), (10.0, -0.025), (10.0, 0.275), (0.0, 0.225), (0.0, 0.525), (10.0, 0.5)]), "{rows:?}");
    // The same left of and above the origin, and with its rows upright.
    let rows = turns_kept(&[(-20.0, -5.0), (-10.0, -5.0), (-10.0, -4.75), (-20.0, -4.75), (-20.0, -4.5), (-10.0, -4.5)]);
    assert!(is(&rows, &[(-20.0, -5.0), (-10.0, -5.025), (-10.0, -4.725), (-20.0, -4.775), (-20.0, -4.475), (-10.0, -4.5)]), "{rows:?}");
    let rows = turns_kept(&[(-5.0, -20.0), (-5.0, -10.0), (-4.75, -10.0), (-4.75, -20.0), (-4.5, -20.0), (-4.5, -10.0)]);
    assert!(is(&rows, &[(-5.0, -20.0), (-5.025, -10.0), (-4.725, -10.0), (-4.775, -20.0), (-4.475, -20.0), (-4.5, -10.0)]), "{rows:?}");
    // Rows at 45°: the turn runs square to them, and its points move along it.
    let (side, by) = (0.25 * std::f64::consts::FRAC_1_SQRT_2, 0.025 * std::f64::consts::FRAC_1_SQRT_2);
    let rows = turns_kept(&[(0.0, 0.0), (10.0, 10.0), (10.0 + side, 10.0 - side), (side, -side)]);
    assert!(is(&rows, &[(0.0, 0.0), (10.0 - by, 10.0 + by), (10.0 + side + by, 10.0 - side - by), (side, -side)]), "{rows:?}");
}

#[test]
fn req_fill_tat_014_only_a_stitch_between_2_rows_sewn_opposite_ways_is_a_turn() {
    // A turn as long as the shortest stitch, a short stitch between 2 that run the same way or square to
    // each other, a turn of no length, and a run too short to have a turn between 2 rows: all stay.
    let kept: [&[(f64, f64)]; 5] = [
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 0.3), (0.0, 0.3)],
        &[(0.0, 0.0), (5.0, 0.0), (5.2, 0.0), (10.0, 0.0)],
        &[(0.0, 0.0), (0.0, 1.0), (0.2, 1.0), (5.0, 1.0)],
        &[(0.0, 0.0), (9.9, 0.0), (10.0, 0.0), (10.0, 0.0), (0.0, 0.0)],
        &[(0.0, 0.0), (10.0, 0.0), (10.0, 0.25)],
    ];
    for run in kept {
        assert!(is(&turns_kept(run), run), "{run:?}: {:?}", turns_kept(run));
    }
}

#[test]
fn req_fill_tat_014_a_needle_point_too_near_a_turn_is_left_out_for_it() {
    // 0.1 mm before the turn, a row's last needle point would have the turn's first point thinned out: it
    // is left out. 0.4 mm before it, it stays.
    let near = turns_kept(&[(0.0, 0.0), (9.9, 0.0), (10.0, 0.0), (10.0, 0.25), (0.0, 0.25)]);
    assert!(is(&near, &[(0.0, 0.0), (10.0, -0.025), (10.0, 0.275), (0.0, 0.25)]), "{near:?}");
    let far = turns_kept(&[(0.0, 0.0), (9.6, 0.0), (10.0, 0.0), (10.0, 0.25), (0.0, 0.25)]);
    assert!(is(&far, &[(0.0, 0.0), (9.6, 0.0), (10.0, -0.025), (10.0, 0.275), (0.0, 0.25)]), "{far:?}");
    // The run's first needle point stays, however near.
    let first = turns_kept(&[(9.9, 0.0), (10.0, 0.0), (10.0, 0.25), (0.0, 0.25)]);
    assert!(is(&first, &[(9.9, 0.0), (10.0, -0.025), (10.0, 0.275), (0.0, 0.25)]), "{first:?}");
    // So does the second point of the turn before, which a row 0.2 mm long leaves near the next turn.
    let short = turns_kept(&[(0.0, 0.0), (10.0, 0.0), (10.0, 0.25), (9.8, 0.25), (9.8, 0.5), (20.0, 0.5)]);
    assert!(is(&short, &[(0.0, 0.0), (10.0, -0.025), (10.0, 0.275), (9.8, 0.225), (9.8, 0.525), (20.0, 0.5)]), "{short:?}");
}

#[test]
fn req_fill_tat_014_keeping_turns_costs_a_unit_for_each_needle_point() {
    let run = || vec![vec![p(0.0, 0.0), p(10.0, 0.0), p(10.0, 0.25), p(0.0, 0.25), p(0.0, 0.5), p(10.0, 0.5)]];
    let spend = |work: u64| turned(run(), 0.3, &mut Budget { max_stitches: 1, max_work: work }.meter()).is_ok();
    assert!(spend(6) && !spend(5));
}
