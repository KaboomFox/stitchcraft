//! The ripple stitch's unit cases (`REQ-RIP-001` to `REQ-RIP-010`, `REQ-RIP-015`, `REQ-RIP-016`). Its
//! diagnostics in a plan are in `tests/ripple.rs`, and its guides, grid and clips in `tests/ripple_guided.rs`,
//! through the engine's public API. Where a guide places its copies is in `guided/tests.rs`.

use stitchcraft_core::{Budget, math};

use super::lines::{CLOSED, MOST, bent, count, kind_of, lines, places, shortest, skipped, skipped_as, target};
use super::sew::{manual, one_at_a_time};
use super::*;
use crate::design::{Join, Segment, Subpath};
use crate::normalize::stroke::distance_to_segment;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// A path of a polyline through each of `parts`, each subpath closed where `closed` says.
fn path(parts: &[&[(f64, f64)]], closed: bool) -> Path {
    let subpath = |points: &&[(f64, f64)]| Subpath {
        start: p(points[0].0, points[0].1),
        segments: points[1..].iter().map(|&(x, y)| Segment::Line(p(x, y))).collect(),
        closed,
    };
    Path { subpaths: parts.iter().map(subpath).collect() }
}

fn open(parts: &[&[(f64, f64)]]) -> Path {
    path(parts, false)
}

/// A column 20 mm long along x and 4 mm wide, its first rail along y = 0, a rung across its middle.
fn column() -> Path {
    open(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (20.0, 4.0)], &[(10.0, -0.5), (10.0, 4.5)]])
}

/// A line 10 mm long along x.
fn line() -> Path {
    open(&[&[(0.0, 0.0), (10.0, 0.0)]])
}

/// A square 10 mm a side, closed at its first corner.
fn square() -> Path {
    path(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0), (0.0, 0.0)]], true)
}

/// A ripple set as `with` says, for a machine whose shortest stitch is 0.3 mm, aiming at `target`: its
/// column's rails read with its own `reverse_rails` and `swap_satin_rails`, as the engine reads them.
fn ripple(with: &[(&str, &str)], target: Option<Point>) -> Ripple {
    let set: ParamSet = with.iter().copied().collect();
    let own = RippleParams::from_set(&set).unwrap().params;
    let satin = SatinParams::from_set_as(&set, StitchType::RippleStitch).unwrap().params;
    Ripple {
        satin: SatinParams { reverse_rails: own.reverse_rails, swap_satin_rails: own.swap_satin_rails, ..satin },
        ripple: own,
        running: RunningParams::from_set(&set).unwrap().params,
        passes: RepeatParams::from_set(&set).unwrap().params,
        min_stitch: Mm::new(0.3).unwrap(),
        jump: Mm::new(3.0).unwrap(),
        target,
        guide: None,
        anchor: None,
        unapplied: unapplied(&set),
        clips: Vec::new(),
    }
}

fn sewn(path: &Path, with: &[(&str, &str)], target: Option<Point>) -> Stitched {
    ripple_stitch(path, &ripple(with, target), &SplitMix64::new(7), &mut Budget::DEFAULT.meter()).unwrap()
}

/// The one run `path` sews as `with` says, which says nothing.
fn run(path: &Path, with: &[(&str, &str)], target: Option<Point>) -> Vec<Point> {
    let stitched = sewn(path, with, target);
    assert_eq!(stitched.warnings, Vec::new());
    assert_eq!(stitched.runs.len(), 1);
    stitched.runs.into_iter().next().unwrap()
}

/// What `stitched` says, as people read it.
fn said(stitched: &Stitched) -> Vec<String> {
    stitched.warnings.iter().map(ToString::to_string).collect()
}

fn near(a: Point, b: Point) -> bool {
    a.distance(b) < 1e-9
}

/// The coordinates of `points`, each to a thousandth of a millimetre.
fn rounded(points: &[Point]) -> Vec<(f64, f64)> {
    let round = |v: f64| (v * 1000.0).round() / 1000.0;
    points.iter().map(|q| (round(q.x()), round(q.y()))).collect()
}

/// The points `(x, y)` for each x of `xs`.
fn along_x(xs: &[f64], y: f64) -> Vec<(f64, f64)> {
    xs.iter().map(|&x| (x, y)).collect()
}

/// How lines are sewn in these cases: in stitches of 2.5 mm within 0.2 mm, the shortest 0.3 mm.
fn sewing(rng: &SplitMix64, staggers: f64, point: bool, skip_start: usize) -> Sewing<'_> {
    Sewing { pattern: &[2.5], tolerance: 0.2, min: 0.3, jitter: None, rng, staggers, point, skip_start, flip: true }
}

/// `lines` sewn one at a time as `sewing` says, to a thousandth of a millimetre.
fn one_by_one(lines: &[Vec<Point>], sewing: &Sewing) -> Vec<(f64, f64)> {
    rounded(&one_at_a_time(lines, sewing, &mut Budget::DEFAULT.meter()).unwrap())
}

#[test]
fn req_rip_001_a_stroke_of_one_line_aims_at_its_target_and_of_2_lines_follows_its_column() {
    // 1 line is closed when its ends lie nearer each other than 0.05 CSS pixels.
    assert_eq!(kind_of(&[p(0.0, 0.0), p(10.0, 0.0)]), Kind::Target);
    assert_eq!(kind_of(&[p(0.0, 0.0), p(10.0, 0.0), p(0.0, 0.0)]), Kind::Spiral);
    assert_eq!(kind_of(&[p(0.0, 0.0), p(10.0, 0.0), p(0.0, CLOSED)]), Kind::Target, "exactly 0.05 CSS pixels apart");
    assert_eq!(kind_of(&[p(0.0, 0.0), p(10.0, 0.0), p(0.0, CLOSED * 0.99)]), Kind::Spiral);
    // 0.05 CSS pixels are 0.01323 mm.
    assert_eq!([0.0132, 0.0133].map(|gap| kind_of(&[p(0.0, 0.0), p(10.0, 0.0), p(0.0, gap)])), [Kind::Spiral, Kind::Target]);
    // 2 lines are a column: its first line lies on the first rail, its last on the second, sewn back and
    // forth.
    let ripple = run(&column(), &[("line_count", "3")], None);
    let xs = [0.0, 2.5, 5.0, 7.5, 10.0, 12.5, 15.0, 17.5, 20.0];
    let back: Vec<f64> = xs.iter().rev().copied().collect();
    assert_eq!(rounded(&ripple), [along_x(&xs, 0.0), along_x(&back, 2.0), along_x(&xs, 4.0)].concat());
    // An open line shrinks towards its target point, and the last line is the target point itself.
    let towards = run(&line(), &[("line_count", "3"), ("staggers", "0")], Some(p(5.0, 5.0)));
    let line_0 = along_x(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0);
    let line_1 = along_x(&[7.5, 5.0, 2.5], 2.5);
    assert_eq!(rounded(&towards), [line_0, vec![(8.75, 1.25)], line_1, vec![(3.75, 3.75), (5.0, 5.0)]].concat());
    // A closed line spirals in to its target point in one run, round the bent lines' corners.
    let spiral = run(&square(), &[("line_count", "2")], None);
    let corners = [(0.0, 0.0), (9.375, 0.625), (8.75, 8.75), (1.875, 8.125), (2.5, 2.5), (6.875, 3.125), (6.25, 6.25), (4.375, 5.625), (5.0, 5.0)];
    let sewn = rounded(&spiral);
    let places: Vec<usize> = corners.iter().map(|corner| sewn.iter().position(|q| q == corner).unwrap()).collect();
    assert!(places.windows(2).all(|pair| pair[0] < pair[1]), "{sewn:?}");
    assert_eq!((places.first(), places.last()), (Some(&0), Some(&(sewn.len() - 1))));
}

#[test]
fn req_rip_001_a_stroke_too_small_to_sew_sews_nothing_and_says_so() {
    let point = "warning SC-W0401: A part of the stroke is a single point, so it is not stitched.";
    // A path of 2 points: each is named.
    let points = sewn(&open(&[&[(1.0, 1.0)], &[(5.0, 5.0)]]), &[], None);
    assert_eq!((points.runs.len(), said(&points)), (0, vec![point.to_string(); 2]));
    // A point beside a line: the point is named, and the line ripples.
    let beside = sewn(&open(&[&[(1.0, 1.0)], &[(0.0, 0.0), (10.0, 0.0)]]), &[("line_count", "2")], None);
    assert_eq!((beside.runs.len(), said(&beside)), (1, vec![point.to_string()]));
    // Beside a column's rails, the satin column's reading names it.
    let rails = sewn(&open(&[&[(1.0, 1.0)], &[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 4.0), (10.0, 4.0)]]), &[("line_count", "2")], None);
    assert_eq!(said(&rails), ["warning SC-W0205: Subpath 1 of this satin column is one point, so it is left out."]);
    // Lines shorter together than the shortest stitch, 0.3 mm.
    let short = sewn(&open(&[&[(0.0, 0.0), (0.1, 0.0)], &[(0.0, 0.1), (0.1, 0.1)]]), &[], None);
    let message =
        "warning SC-W0401: This ripple stitch's lines are 0.2 mm long together, shorter than the shortest stitch (0.3 mm), so it is not stitched.";
    assert_eq!((short.runs.len(), said(&short)), (0, vec![message.to_string()]));
    // A line exactly as long as the shortest stitch is sewn.
    assert_eq!(run(&open(&[&[(0.0, 0.0), (0.3, 0.0)]]), &[("line_count", "2")], None), [p(0.0, 0.0), p(0.3, 0.0)]);
    // A closed line 0.6 mm round lies all within the shortest stitch of its ends: it has no outline.
    let curl = sewn(&path(&[&[(0.0, 0.0), (0.2, 0.0), (0.2, 0.1), (0.0, 0.1), (0.0, 0.0)]], true), &[], None);
    let curled = "warning SC-W0401: A part of the stroke is 0.6 mm long, but all of it lies within the shortest stitch (0.3 mm) of its ends, so it is not stitched.";
    assert_eq!((curl.runs.len(), said(&curl)), (0, vec![curled.to_string()]));
}

#[test]
fn req_rip_001_a_ripple_whose_lines_are_single_points_sews_nothing() {
    let message = "warning SC-W0401: Every line of this ripple stitch is one point, so it is not stitched.";
    // Rails 0.05 mm long: 1 pair across them, so every line is one point.
    let stub = sewn(&open(&[&[(0.0, 0.0), (0.05, 0.0)], &[(0.0, 4.0), (0.05, 4.0)], &[(0.02, -0.5), (0.02, 4.5)]]), &[], None);
    assert_eq!((stub.runs.len(), said(&stub)), (0, vec![message.to_string()]));
    // 1 line, flipped: the target point alone, at a phase of 0, or sewn unstaggered as its one point.
    for staggers in ["1", "0"] {
        let point = sewn(&line(), &[("line_count", "1"), ("flip_exponent", "true"), ("staggers", staggers)], Some(p(5.0, 5.0)));
        assert_eq!((point.runs.len(), said(&point)), (0, vec![message.to_string()]), "{staggers}");
    }
    // A column with no section to sew says so, as a satin column does: the rung cuts the first rail at its
    // start and the second at its end.
    let crossed = open(&[&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 4.0), (10.0, 4.0)], &[(-5.0, -2.0), (15.0, 6.0)]]);
    let none = sewn(&crossed, &[], None);
    let no_section = "error SC-E0216: Every section of this satin column has no length on one of its rails, so it has nothing to sew.";
    assert_eq!((none.runs.len(), said(&none)), (0, vec![no_section.to_string()]));
}

#[test]
fn req_rip_002_the_target_point_is_the_given_one_or_the_middle_of_the_hull() {
    let meter = &mut Budget::DEFAULT.meter();
    let corners = [p(0.0, 0.0), p(10.0, 0.0), p(10.0, 10.0), p(0.0, 10.0), p(0.0, 0.0)];
    assert_eq!(target(Some(p(3.0, 4.0)), &corners, meter).unwrap(), p(3.0, 4.0));
    assert!(near(target(None, &corners, meter).unwrap(), p(5.0, 5.0)));
    // The centroid of the hull's area: a right triangle's lies a third of the way in from its sides, wherever
    // the points inside or on it are.
    let triangle = target(None, &[p(0.0, 0.0), p(6.0, 0.0), p(3.0, 3.0), p(1.0, 1.0), p(0.0, 6.0)], meter).unwrap();
    assert!(near(triangle, p(2.0, 2.0)), "{triangle:?}");
    // Points in a line: the middle of the 2 farthest apart. One point: itself.
    assert_eq!(target(None, &[p(0.0, 0.0), p(1.0, 1.0), p(10.0, 10.0)], meter).unwrap(), p(5.0, 5.0));
    assert_eq!(target(None, &[p(2.0, 3.0), p(2.0, 3.0)], meter).unwrap(), p(2.0, 3.0));
    // A line along x without a target point shrinks to its middle, and with one to it.
    let to = |target: Option<Point>| run(&line(), &[("line_count", "2"), ("staggers", "0")], target).last().copied();
    assert_eq!(to(None), Some(p(5.0, 0.0)));
    assert_eq!(to(Some(p(5.0, 8.0))), Some(p(5.0, 8.0)));
    // A column has no target point.
    assert_eq!(run(&column(), &[("line_count", "2")], Some(p(50.0, 50.0))), run(&column(), &[("line_count", "2")], None));
}

#[test]
fn req_rip_003_the_count_is_line_count_or_what_min_line_dist_mm_gives_with_its_parity() {
    let read = |with: &[(&str, &str)]| ripple(with, None).ripple;
    let plain = |lines: usize| Count { lines, cut: None };
    // `line_count`, 1 more for a closed line or the point join, at least 2 along a column.
    assert_eq!(count(&read(&[]), 7.0, (false, false)), plain(10));
    assert_eq!(count(&read(&[("line_count", "3")]), 7.0, (true, false)), plain(4));
    assert_eq!(count(&read(&[("line_count", "1")]), 7.0, (false, false)), plain(1));
    assert_eq!(count(&read(&[("line_count", "1")]), 7.0, (false, true)), plain(2));
    assert_eq!(count(&read(&[("line_count", "1")]), 7.0, (true, true)), plain(2));
    assert_eq!(count(&read(&[("line_count", "2")]), 7.0, (true, true)), plain(3));
    // A count of 0, which the registry doesn't let through, is 1.
    assert_eq!(count(&RippleParams { line_count: 0, ..read(&[]) }, 7.0, (false, false)), plain(1));
    // `min_line_dist_mm`: the shortest divided by it, rounded up, 1 less where its parity differs from
    // `line_count`'s, and at least 1.
    let by = |distance: &str, line_count: &str, shortest: f64| {
        count(&read(&[("min_line_dist_mm", distance), ("line_count", line_count)]), shortest, (false, false))
    };
    assert_eq!(by("1", "10", 10.0), plain(10));
    assert_eq!(by("1", "9", 10.0), plain(9));
    assert_eq!(by("1", "10", 10.5), plain(10));
    assert_eq!(by("1", "9", 10.5), plain(11));
    assert_eq!(by("2", "4", 8.0), plain(4));
    assert_eq!(by("1", "4", 1.5), plain(2));
    assert_eq!(by("1", "3", 0.5), plain(1));
    assert_eq!(by("1", "4", 0.0), plain(1));
    assert_eq!(by("1", "3", 0.0), plain(1));
    // At most 1,000, or 999 for an odd `line_count`, with the count it gave.
    assert_eq!(by("1", "10", 1000.0), plain(MOST));
    assert_eq!(by("1", "10", 1000.5), plain(MOST), "1,001, odd, is 1,000");
    assert_eq!(by("1", "9", 1000.0), plain(999));
    assert_eq!(by("1", "10", 1001.5), Count { lines: MOST, cut: Some(1002.0) });
    assert_eq!(by("1", "9", 1001.0), Count { lines: 999, cut: Some(1001.0) });
    assert_eq!(by("0.5", "10", 50_000.0), Count { lines: MOST, cut: Some(100_000.0) });
    // The line a closed line adds comes after the cut, and so does the least count of a column.
    let closed = count(&read(&[("min_line_dist_mm", "1"), ("line_count", "9")]), 1001.0, (true, false));
    assert_eq!(closed, Count { lines: 1000, cut: Some(1001.0) });
    assert_eq!(count(&read(&[("min_line_dist_mm", "1"), ("line_count", "3")]), 0.0, (false, true)), plain(2));
}

#[test]
fn req_rip_003_the_shortest_measure_is_ink_stitch_s_and_a_ripple_counts_its_lines_so() {
    // Ink/Stitch's shortest: a length of 0 is taken, and the next replaces it, so the last 0 stays.
    assert_eq!(shortest([3.0, 2.0, 5.0]), 2.0);
    assert_eq!(shortest([0.0, 4.0, 3.0]), 3.0);
    assert_eq!(shortest([2.0, 0.0, 5.0]), 5.0);
    assert_eq!(shortest([2.0, 5.0, 0.0]), 0.0);
    assert_eq!(shortest([]), 0.0);
    // A target point on the line: the measure there has no length, and the next one counts. A line 10 mm
    // long aiming at its middle with lines 1 mm apart has 2 lines, the line and the target point, from 2.5 mm.
    let middle = run(&line(), &[("min_line_dist_mm", "1"), ("staggers", "0")], None);
    assert_eq!(rounded(&middle), along_x(&[0.0, 2.5, 5.0, 7.5, 10.0, 7.5, 5.0], 0.0));
    // Ripple stitch counts the same: a line 10 mm long and its target 2,000 mm away, at most 2 mm apart.
    let far = Some(p(5.0, 2000.0));
    let cut = sewn(&line(), &[("min_line_dist_mm", "1"), ("line_count", "9")], far);
    let message = "warning SC-W0407: `min_line_dist_mm` gives this ripple stitch 1999 lines, more than 1000, so it sews 999.";
    assert_eq!(said(&cut), [message]);
    let kept = sewn(&line(), &[("min_line_dist_mm", "2"), ("line_count", "9")], far);
    assert_eq!((kept.warnings, kept.runs), (Vec::new(), cut.runs));
}

#[test]
fn req_rip_004_line_k_lies_at_its_place_between_the_rails_or_towards_the_target_point() {
    assert_eq!(places(0, 1.0, false), Vec::<f64>::new());
    assert_eq!(places(1, 1.0, false), [0.0]);
    assert_eq!(places(1, 2.0, true), [1.0]);
    assert_eq!(places(2, 1.0, false), [0.0, 1.0]);
    assert_eq!(places(3, 1.0, true), [0.0, 0.5, 1.0]);
    assert_eq!(places(5, 1.0, false), [0.0, 0.25, 0.5, 0.75, 1.0]);
    assert_eq!(places(5, 2.0, false), [0.0, 0.0625, 0.25, 0.5625, 1.0]);
    assert_eq!(places(5, 2.0, true), [0.0, 0.4375, 0.75, 0.9375, 1.0]);
    assert_eq!(places(3, 0.5, false), [0.0, 0.5_f64.sqrt(), 1.0]);
    // Line k runs through the point its place of the way along each measure, exactly on its ends at 0 and 1.
    let measures = [[p(0.0, 0.0), p(4.0, 8.0)], [p(10.0, 0.0), p(10.0, 2.0)], [p(3.0, 3.0), p(3.0, 3.0)]];
    let laid = lines(&measures, &[0.0, 0.25, 1.0], &mut Budget::DEFAULT.meter()).unwrap();
    let want = [[(0.0, 0.0), (10.0, 0.0), (3.0, 3.0)], [(1.0, 2.0), (10.0, 0.5), (3.0, 3.0)], [(4.0, 8.0), (10.0, 2.0), (3.0, 3.0)]];
    assert_eq!(laid, want.map(|line| line.map(|(x, y)| p(x, y)).to_vec()));
    // The 9 points must fit in the stitches left.
    let room = |stitches| lines(&measures, &[0.0, 0.25, 1.0], &mut Budget { max_stitches: stitches, max_work: u64::MAX }.meter()).map(|_| ());
    assert_eq!((room(9), room(8)), (Ok(()), Err(Exhausted::Stitches)));
    // Towards a target point, line k is the line's running stitch shrunk towards it. With `flip_copies` off,
    // one run sews every line forwards: here the needle points of the line, then of the line halfway to
    // (5, 10), then the target point, and the needle goes straight from each line to the next.
    let one_way = run(&line(), &[("line_count", "3"), ("staggers", "0"), ("flip_copies", "false")], Some(p(5.0, 10.0)));
    let joins = [(8.125, 1.25), (6.25, 2.5), (4.375, 3.75)];
    let want = [
        along_x(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0),
        joins.to_vec(),
        along_x(&[2.5, 5.0, 7.5], 5.0),
        vec![(6.667, 6.667), (5.833, 8.333), (5.0, 10.0)],
    ];
    assert_eq!(rounded(&one_way), want.concat());
    // An exponent of 2 puts the middle line a quarter of the way: 7.5 mm long, in 3 stitches.
    let packed = run(&line(), &[("line_count", "3"), ("exponent", "2"), ("staggers", "0"), ("flip_copies", "false")], Some(p(5.0, 10.0)));
    let middle = along_x(&[1.25, 3.75, 6.25, 8.75], 2.5);
    assert!(rounded(&packed).windows(4).any(|four| four == middle), "{:?}", rounded(&packed));
}

#[test]
fn req_rip_004_a_copy_of_the_line_reaches_the_target_point_only_at_place_1() {
    // An outline through the target point: each copy keeps (1 − t) of each point's distance to the target
    // point. The last copy is the target point exactly, and the others reach it only where the outline does.
    let target = p(10.0, 12.0);
    let outline = [p(0.0, 0.0), p(5.0, -3.0), p(10.0, 0.0), target, p(15.0, 3.0), p(20.0, 0.0)];
    let measures: Vec<[Point; 2]> = outline.iter().map(|&q| [q, target]).collect();
    let at = places(6, 1.0, false);
    let laid = lines(&measures, &at, &mut Budget::DEFAULT.meter()).unwrap();
    assert_eq!(laid.last(), Some(&vec![target; 6]));
    for (line, t) in laid.iter().zip(&at) {
        for (q, from) in line.iter().zip(outline) {
            assert!((q.distance(target) - (1.0 - t) * from.distance(target)).abs() < 1e-9, "{t}: {q:?}");
            assert_eq!(near(*q, target), *t == 1.0 || from == target, "{t}: {q:?}");
        }
    }
}

#[test]
fn req_rip_005_skips_leave_out_lines_at_each_end_unless_they_leave_none() {
    let lines: Vec<Vec<Point>> = (0..4).map(|k| vec![p(0.0, f64::from(k)), p(1.0, f64::from(k))]).collect();
    let ys = |kept: &[Vec<Point>]| kept.iter().map(|line| line[0].y()).collect::<Vec<_>>();
    let (kept, first) = skipped(lines.clone(), [1, 1]);
    assert_eq!((ys(&kept), first), (vec![1.0, 2.0], Some(1)));
    assert_eq!(skipped(lines.clone(), [3, 0]).0, &lines[3..]);
    assert_eq!(skipped(lines.clone(), [0, 3]).0, &lines[..1]);
    assert_eq!(skipped(lines.clone(), [0, 0]), (lines.clone(), Some(0)));
    // Skips that add up to the count or more leave every line in.
    assert_eq!(skipped(lines.clone(), [2, 2]), (lines.clone(), None));
    assert_eq!(skipped(lines.clone(), [4, 0]), (lines.clone(), None));
    assert_eq!(skipped(lines.clone(), [0, 4]), (lines.clone(), None));
    assert_eq!(skipped(lines.clone(), [usize::MAX, usize::MAX]), (lines, None));
    // Along a column of 4 lines, skipping 1 at each end sews the 2 between, the first forwards.
    let middle = run(&column(), &[("line_count", "4"), ("skip_start", "1"), ("skip_end", "1")], None);
    let xs = [0.0, 2.5, 5.0, 7.5, 10.0, 12.5, 15.0, 17.5, 20.0];
    let back: Vec<f64> = xs.iter().rev().copied().collect();
    assert_eq!(rounded(&middle), [along_x(&xs, 1.333), along_x(&back, 2.667)].concat());
    // Skips that would leave none are said, and every line is sewn.
    let all = sewn(&column(), &[("line_count", "4"), ("skip_start", "2"), ("skip_end", "2")], None);
    assert_eq!(all.runs, sewn(&column(), &[("line_count", "4")], None).runs);
    let message = "warning SC-W0408: `skip_start` and `skip_end` leave out 2 and 2 of this ripple stitch's 4 lines, which would leave none, so no line is left out.";
    assert_eq!(said(&all), [message]);
}

#[test]
fn req_rip_006_lines_bend_into_the_next_as_each_ripple_bends_them() {
    // A spiral's always, unflipped. Towards a target point, the point join's, flipped. Along a column, the
    // point join's and those without `flip_copies`, flipped when both are on.
    assert_eq!([Kind::Spiral.bend(false, true), Kind::Spiral.bend(true, false)], [Some(false); 2]);
    let towards = [Kind::Target.bend(true, true), Kind::Target.bend(true, false), Kind::Target.bend(false, false), Kind::Target.bend(false, true)];
    assert_eq!(towards, [Some(true), Some(true), None, None]);
    let along = [Kind::Column.bend(true, true), Kind::Column.bend(true, false), Kind::Column.bend(false, false), Kind::Column.bend(false, true)];
    assert_eq!(along, [Some(true), Some(false), Some(false), None]);
    // 3 lines of 3 points 1 mm apart bend into 2, each from its own start to the next line's end. Flipped,
    // the second runs from the next line's start to its own end.
    let three: Vec<Vec<Point>> = (0..3).map(|k| (0..3).map(|i| p(f64::from(i), f64::from(k))).collect()).collect();
    let meter = &mut Budget::DEFAULT.meter();
    let line_0 = vec![p(0.0, 0.0), p(1.0, 0.5), p(2.0, 1.0)];
    assert_eq!(bent(&three, false, meter).unwrap(), [line_0.clone(), vec![p(0.0, 1.0), p(1.0, 1.5), p(2.0, 2.0)]]);
    assert_eq!(bent(&three, true, meter).unwrap(), [line_0, vec![p(0.0, 2.0), p(1.0, 1.5), p(2.0, 1.0)]]);
    // Lines of 1 point stay as they are, and 1 line bends into none.
    assert_eq!(bent(&[vec![p(0.0, 0.0)], vec![p(0.0, 1.0)]], true, meter).unwrap(), [vec![p(0.0, 0.0)]]);
    assert_eq!(bent(&three[..1], false, meter).unwrap(), Vec::<Vec<Point>>::new());
}

#[test]
fn req_rip_006_lines_are_sewn_back_and_forth_and_joined_flat() {
    // Ink/Stitch at d59c9ab, run as a black box, sews this line 40 mm long in 4 lines to its middle so: the
    // line, a join of 3 stitches, the second line backwards in 11 stitches, a join of 3, the third forwards
    // in 6 and nothing at the target point. It sews the line in 17 stitches, where StitchCraft's running
    // stitch sews a whole number of stitch lengths in 16 (`DEV-RUN-002`).
    let line_40 = open(&[&[(5.0, 30.0), (45.0, 30.0)]]);
    let xs: Vec<f64> = rounded(&run(&line_40, &[("line_count", "4")], None)).iter().map(|&(x, y)| if y == 30.0 { x } else { f64::NAN }).collect();
    let line_0: Vec<f64> = (0..=16).map(|k| 5.0 + 2.5 * f64::from(k)).collect();
    let line_1 = [38.333, 35.909, 33.485, 31.061, 28.636, 26.212, 23.788, 21.364, 18.939, 16.515, 14.091, 11.667];
    let line_2 = [18.333, 20.556, 22.778, 25.0, 27.222, 29.444, 31.667];
    assert_eq!(xs, [&line_0[..], &[42.778, 40.556], &line_1, &[13.889, 16.111], &line_2].concat());
    // A long join is a straight running stitch: rails 10 mm apart in 2 lines.
    let wide = run(&open(&[&[(0.0, 0.0), (5.0, 0.0)], &[(0.0, 10.0), (5.0, 10.0)]]), &[("line_count", "2")], None);
    let join = [(5.0, 2.5), (5.0, 5.0), (5.0, 7.5)];
    assert_eq!(rounded(&wide), [along_x(&[0.0, 2.5, 5.0], 0.0), join.to_vec(), along_x(&[5.0, 2.5, 0.0], 10.0)].concat());
    // Without `flip_copies`, the lines along a column bend into each other and are sewn as one run, each
    // forwards, the needle going straight back from each line's end to the next one's start.
    let one_way = run(&column(), &[("line_count", "3"), ("flip_copies", "false")], None);
    let sewn = rounded(&one_way);
    assert_eq!((sewn.len(), sewn[0], sewn[9], sewn[17], sewn[26]), (27, (0.0, 0.0), (20.0, 2.0), (0.0, 2.0), (20.0, 4.0)));
    assert_eq!(sewn[9..18], along_x(&[20.0, 17.5, 15.0, 12.5, 10.0, 7.5, 5.0, 2.5, 0.0], 2.0));
    // Towards a target point too.
    let target = run(&line(), &[("line_count", "3"), ("flip_copies", "false")], None);
    assert_eq!(rounded(&target), along_x(&[0.0, 2.5, 5.0, 7.5, 10.0, 7.5, 5.0, 2.5, 5.0, 7.5, 5.0], 0.0));
}

#[test]
fn req_rip_006_lines_joined_at_a_point_meet_and_list_their_meeting_twice() {
    // Along a column, the point join adds a line: 4 lines bend into 3 that meet at (20, 4/3) and (0, 8/3),
    // every other one sewn from its end, a zigzag from the first rail's start to the second rail's end.
    let zigzag = run(&column(), &[("line_count", "3"), ("join_style", "1")], None);
    let turns = [p(0.0, 0.0), p(20.0, 4.0 / 3.0), p(0.0, 8.0 / 3.0), p(20.0, 4.0)];
    assert_eq!(zigzag.len(), 28);
    for (k, part) in [&zigzag[..10], &zigzag[9..19], &zigzag[18..]].into_iter().enumerate() {
        let (from, to) = (turns[k], turns[k + 1]);
        assert!(near(part[0], from) && near(part[part.len() - 1], to), "{:?}", rounded(part));
        assert!(part.iter().all(|q| distance_to_segment(*q, from, to) < 1e-9), "{:?}", rounded(part));
    }
    // The meeting points are listed twice, so each takes a step of the bean list: with "1 0", the stitch
    // after each meeting is sewn 3 times, as the one before it is.
    let beans = run(&column(), &[("line_count", "3"), ("join_style", "1"), ("bean_stitch_repeats", "1 0")], None);
    assert_eq!(beans.len(), 1 + 15 * 3 + 12);
    let after = [zigzag[9], zigzag[10], zigzag[9], zigzag[10]];
    assert!(beans.windows(4).any(|four| four == after), "{:?}", rounded(&beans));
    // Skipping the first line keeps every other line turning as before: the second still from its end.
    let skipped = run(&column(), &[("line_count", "3"), ("join_style", "1"), ("skip_start", "1")], None);
    assert_eq!(skipped, zigzag[9..]);
    // Towards a target point, the point join flips every other line whatever `flip_copies` says, and the
    // lines meet where the outline shrunk by a third and by two thirds starts and ends.
    let towards = rounded(&run(&line(), &[("line_count", "3"), ("join_style", "1")], Some(p(5.0, 5.0))));
    let at = |q: (f64, f64)| towards.iter().position(|s| *s == q).unwrap();
    assert!(at((0.0, 0.0)) < at((8.333, 1.667)) && at((8.333, 1.667)) < at((3.333, 3.333)), "{towards:?}");
    assert_eq!(towards.last(), Some(&(5.0, 5.0)));
    let one_run = rounded(&run(&line(), &[("line_count", "3"), ("join_style", "1"), ("flip_copies", "false")], Some(p(5.0, 5.0))));
    assert_eq!((one_run.first(), one_run.last()), (towards.first(), towards.last()));
}

#[test]
fn req_rip_006_a_closed_line_spirals_in_one_run_whatever_the_join_and_flip_copies() {
    let spiral = run(&square(), &[("line_count", "3")], None);
    assert_eq!((spiral.first(), spiral.last()), (Some(&p(0.0, 0.0)), Some(&p(5.0, 5.0))));
    // Turning round the target point, the needle comes nearer it with every turn.
    let corners: Vec<f64> = rounded(&spiral).iter().filter(|q| [(9.583, 0.417), (9.167, 9.167), (1.25, 8.75)].contains(q)).map(|_| 0.0).collect();
    assert_eq!(corners.len(), 3);
    let point = run(&square(), &[("line_count", "3"), ("join_style", "1"), ("flip_copies", "false")], None);
    assert_eq!(point, spiral);
}

#[test]
fn req_rip_007_each_line_s_stitches_keep_within_the_tolerance_of_the_line() {
    // A column round half a circle, 3 mm and 2 mm from its centre: stitches of 2.5 mm along its lines
    // would stray 0.26 mm from them, so each line is sewn in shorter ones.
    let arc = |r: f64| -> Vec<(f64, f64)> {
        (0..=32).map(|k| std::f64::consts::PI * f64::from(k) / 32.0).map(|a| (10.0 + r * math::cos(a), 10.0 - r * math::sin(a))).collect()
    };
    let (outer, inner) = (arc(3.0), arc(2.0));
    let bend = open(&[&outer, &inner]);
    for tolerance in [0.1, 0.2] {
        let tolerance_mm = tolerance.to_string();
        let sewn = run(&bend, &[("line_count", "5"), ("running_stitch_tolerance_mm", &tolerance_mm)], None);
        let radius = |q: Point| q.distance(p(10.0, 10.0));
        let mut stitches = 0;
        for pair in sewn.windows(2) {
            // A stitch along a line, not a join between 2: both ends as far from the centre.
            if (radius(pair[0]) - radius(pair[1])).abs() > 0.01 {
                continue;
            }
            stitches += 1;
            let sag = radius(pair[0]) - radius(pair[0].lerp(pair[1], 0.5));
            assert!(sag <= tolerance + 0.005, "{tolerance}: {:?} strays {sag}", rounded(pair));
        }
        assert!(stitches > 20, "{stitches}");
    }
}

#[test]
fn req_rip_007_the_running_stitch_a_ripple_shrinks_lies_on_its_curve_within_a_tenth_of_the_tolerance() {
    // A quarter circle of radius 10 mm about the origin, drawn as one curve whose control points lie 0.552
    // of the radius from its ends, which strays from the circle by less than 0.003 mm. The running stitch
    // that a ripple shrinks towards its target point flattens it within a tenth of the 0.2 mm tolerance and
    // places its needle points on the flattened line, so each lies within 0.02 mm of the curve.
    let k = 10.0 * 0.552_284_75;
    let quarter = Subpath { start: p(10.0, 0.0), segments: vec![Segment::Cubic(p(10.0, k), p(k, 10.0), p(0.0, 10.0))], closed: false };
    let points = crate::generators::running::outline(&quarter, &[2.5], (0.2, 0.3), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    assert!(points.len() >= 7, "{points:?}");
    let off = points.iter().map(|q| ((q.x() * q.x() + q.y() * q.y()).sqrt() - 10.0).abs()).fold(0.0, f64::max);
    assert!(off < 0.023, "a needle point lies {off} mm off the circle");
}

#[test]
fn req_rip_008_a_line_is_lengthened_back_along_its_first_segment_whichever_way_it_runs() {
    // The case below with lines that run along y: line 1 is placed from 2.25 mm before its start, back along
    // its first segment, and sewn from its end.
    let rng = SplitMix64::new(0);
    let down = [vec![p(0.0, 0.0), p(0.0, 10.0)], vec![p(1.0, 0.0), p(1.0, 10.0)]];
    let along_y = |ys: &[f64], x: f64| ys.iter().map(|&y| (x, y)).collect::<Vec<_>>();
    let want = [along_y(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0), along_y(&[10.0, 7.55, 5.1, 2.65, 0.0], 1.0)];
    assert_eq!(one_by_one(&down, &sewing(&rng, -10.0, false, 0)), want.concat());
}

#[test]
fn req_rip_008_line_i_s_needle_points_start_its_phase_before_its_start() {
    // 3 lines 10 mm long, 2 mm apart, with 3 staggers: line i is placed from (i / 3) of 2.5 mm before its
    // start. Line 1 from 0.833 mm before, in 5 stitches of 2.167 mm, sewn from its end. Line 2 from 1.667 mm
    // before, in 5 of 2.333 mm.
    let rails = open(&[&[(0.0, 0.0), (10.0, 0.0)], &[(0.0, 4.0), (10.0, 4.0)]]);
    let staggered = rounded(&run(&rails, &[("line_count", "3"), ("staggers", "3")], None));
    let want = [
        along_x(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0),
        along_x(&[10.0, 7.833, 5.667, 3.5, 1.333, 0.0], 2.0),
        along_x(&[0.0, 0.667, 3.0, 5.333, 7.667, 10.0], 4.0),
    ];
    assert_eq!(staggered, want.concat());
    // Negative staggers count from the other end: (1 / −10) mod 1 is 0.9, 2.25 mm. The needle point that
    // lands 0.2 mm from the start, nearer than the shortest stitch, is left out.
    let rng = SplitMix64::new(0);
    let two = [vec![p(0.0, 0.0), p(10.0, 0.0)], vec![p(0.0, 1.0), p(10.0, 1.0)]];
    let back_line = along_x(&[10.0, 7.55, 5.1, 2.65, 0.0], 1.0);
    assert_eq!(one_by_one(&two, &sewing(&rng, -10.0, false, 0)), [along_x(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0), back_line].concat());
    // A first segment shorter than 0.1 CSS pixels lengthens the line in proportion less: 0.01 mm of 0.026 mm
    // takes 0.472 mm of the 1.25 mm that 2 staggers give line 1.
    let short_first = [two[0].clone(), vec![p(0.0, 1.0), p(0.01, 1.0), p(10.0, 1.0)]];
    let line_1 = along_x(&[10.0, 7.906, 5.811, 3.717, 1.622, 0.0], 1.0);
    assert_eq!(one_by_one(&short_first, &sewing(&rng, 2.0, false, 0)), [along_x(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0), line_1].concat());
    // A first segment of no length lengthens the line by nothing.
    let doubled_start = [two[0].clone(), vec![p(0.0, 1.0), p(0.0, 1.0), p(10.0, 1.0)]];
    let unstaggered = along_x(&[10.0, 7.5, 5.0, 2.5, 0.0], 1.0);
    assert_eq!(one_by_one(&doubled_start, &sewing(&rng, 2.0, false, 0)), [along_x(&[0.0, 2.5, 5.0, 7.5, 10.0], 0.0), unstaggered.clone()].concat());
    // A line of 1 point is left out, and still counts: the line after it is sewn from its end.
    let after_a_point = [vec![p(0.0, 0.0)], two[1].clone()];
    assert_eq!(one_by_one(&after_a_point, &sewing(&rng, 0.0, false, 0)), unstaggered);
}

#[test]
fn req_rip_008_a_line_of_no_length_sews_its_point_unless_staggered_at_a_phase_of_0() {
    let rng = SplitMix64::new(0);
    let point = vec![p(1.0, 1.0); 3];
    assert_eq!(one_by_one(std::slice::from_ref(&point), &sewing(&rng, 0.0, false, 0)), [(1.0, 1.0)]);
    assert_eq!(one_by_one(std::slice::from_ref(&point), &sewing(&rng, 3.0, false, 0)), Vec::<(f64, f64)>::new());
    let after = one_by_one(&[vec![p(1.0, 0.0), p(1.0, 6.0)], point], &sewing(&rng, 3.0, false, 0));
    assert_eq!(after.last(), Some(&(1.0, 1.0)));
    // Towards a target point the last line is the target point: sewn with staggers of 0 or 3, and with 1 or
    // 2, where its phase is 0, left out.
    let last = |staggers: &str| run(&line(), &[("line_count", "3"), ("staggers", staggers)], Some(p(5.0, 5.0))).last().copied();
    assert_eq!([last("0"), last("3")], [Some(p(5.0, 5.0)); 2]);
    assert_eq!([last("1"), last("2")], [Some(p(2.5, 2.5)); 2]);
}

#[test]
fn req_rip_009_reverse_turns_the_ripple_then_repeats_and_bean_stitch_sew_it() {
    let forwards = run(&line(), &[("line_count", "3")], None);
    assert_eq!(rounded(&forwards), along_x(&[0.0, 2.5, 5.0, 7.5, 10.0, 7.5, 5.0, 2.5], 0.0));
    let mut backwards = run(&line(), &[("line_count", "3"), ("reverse", "true")], None);
    backwards.reverse();
    assert_eq!(backwards, forwards);
    // 2 repeats go there and back, and bean stitch sews each stitch 3 times.
    let twice = run(&line(), &[("line_count", "3"), ("repeats", "2")], None);
    assert_eq!(rounded(&twice), along_x(&[0.0, 2.5, 5.0, 7.5, 10.0, 7.5, 5.0, 2.5, 5.0, 7.5, 10.0, 7.5, 5.0, 2.5, 0.0], 0.0));
    let bold = run(&line(), &[("line_count", "3"), ("bean_stitch_repeats", "1")], None);
    assert_eq!(rounded(&bold[..7]), along_x(&[0.0, 2.5, 0.0, 2.5, 5.0, 2.5, 5.0], 0.0));
    assert_eq!(bold.len(), 7 * 3 + 1);
}

#[test]
fn req_rip_009_random_lengths_draw_from_a_stream_for_each_line() {
    let random = [("line_count", "3"), ("enable_random_stitch_length", "true")];
    let sewn = run(&column(), &random, None);
    assert_eq!(run(&column(), &random, None), sewn, "the same each time");
    // Lines 0 and 2, sewn the same way along rails as long, place their needle points differently.
    let xs = |y: f64| sewn.iter().filter(|q| q.y() == y).map(|q| q.x()).collect::<Vec<f64>>();
    assert_ne!(xs(0.0), xs(4.0));
    assert_eq!((xs(0.0).first(), xs(0.0).last(), xs(4.0).last()), (Some(&0.0), Some(&20.0), Some(&20.0)));
    // Each stitch along a line is 2.5 mm, longer or shorter by up to the jitter, 10 % by default.
    let along: Vec<f64> = sewn.windows(2).filter(|pair| pair[0].y() == pair[1].y()).map(|pair| pair[0].distance(pair[1])).collect();
    assert!(along.iter().all(|&length| length <= 2.5 * 1.1 + 1e-9), "{along:?}");
    assert!(along.iter().any(|&length| (length - 2.5).abs() > 0.01), "{along:?}");
}

#[test]
fn req_rip_010_push_compensation_along_a_column_is_named_and_not_applied() {
    // Named when it reads as other than 0, or doesn't read at all.
    let named = |pairs: &[(&str, &str)]| unapplied(&pairs.iter().copied().collect());
    assert_eq!(named(&[]), Vec::<&str>::new());
    assert_eq!(named(&[("push_compensation_mm", "0 0")]), Vec::<&str>::new());
    assert_eq!(named(&[("push_compensation_mm", "0 0.5")]), [PUSH_COMPENSATION]);
    assert_eq!(named(&[("push_compensation_mm", "0.5")]), [PUSH_COMPENSATION]);
    assert_eq!(named(&[("push_compensation_mm", "-0.5 0")]), [PUSH_COMPENSATION]);
    assert_eq!(named(&[("push_compensation_mm", "wide")]), [PUSH_COMPENSATION]);
    // The rails' own settings are the ripple's, and applied (`REQ-RIP-018`).
    assert_eq!(named(&[("reverse_rails", "both"), ("swap_satin_rails", "true")]), Vec::<&str>::new());
    assert!(SatinParams::SPECS.iter().any(|spec| spec.key == PUSH_COMPENSATION));
    // Along a column it is said, and the rails are read without it.
    let pushed = sewn(&column(), &[("line_count", "3"), ("push_compensation_mm", "1")], None);
    assert_eq!(pushed.runs, sewn(&column(), &[("line_count", "3")], None).runs);
    let message = "warning SC-W0406: This ripple stitch sets `push_compensation_mm`, which StitchCraft doesn't apply to ripple stitch, so its column is read without it.";
    assert_eq!(said(&pushed), [message]);
    // Towards a target point Ink/Stitch doesn't read it either, and nothing is said.
    assert_eq!(sewn(&line(), &[("push_compensation_mm", "1")], None).warnings, Vec::new());
}

#[test]
fn every_point_of_every_line_costs_work_and_every_needle_point_a_stitch() {
    let work = |line_count: &str| {
        let mut meter = Budget::DEFAULT.meter();
        ripple_stitch(&line(), &ripple(&[("line_count", line_count)], None), &SplitMix64::new(7), &mut meter).unwrap();
        Budget::DEFAULT.max_work - meter.work_left()
    };
    assert!(work("20") > work("10"), "{} {}", work("20"), work("10"));
    let left = |budget: Budget| {
        let mut meter = budget.meter();
        let stitched = ripple_stitch(&line(), &ripple(&[("line_count", "3")], None), &SplitMix64::new(7), &mut meter).map(|stitched| stitched.runs);
        (stitched, meter.stitches_left())
    };
    assert_eq!(left(Budget { max_stitches: 100, max_work: u64::MAX }).1, 92, "8 needle points");
    assert_eq!(left(Budget { max_stitches: 7, max_work: u64::MAX }).0, Err(Exhausted::Stitches));
    assert_eq!(left(Budget { max_stitches: 100, max_work: 20 }).0, Err(Exhausted::Work));
}

#[test]
fn req_rip_015_a_column_s_grid_lines_lose_the_lines_the_skips_leave_out() {
    let lines: Vec<Vec<Point>> = (0..4).map(|k| vec![p(0.0, f64::from(k))]).collect();
    let ys = |kept: Vec<Vec<Point>>| kept.iter().map(|line| line[0].y()).collect::<Vec<_>>();
    assert_eq!(ys(skipped_as(lines.clone(), [1, 1], true)), [1.0, 2.0]);
    assert_eq!(ys(skipped_as(lines.clone(), [0, 3], true)), [0.0]);
    // Skips that leave the ripple none leave the grid's lines all, and more than they have leave none.
    assert_eq!(ys(skipped_as(lines.clone(), [3, 2], false)), [0.0, 1.0, 2.0, 3.0]);
    assert_eq!(ys(skipped_as(lines, [3, 2], true)), Vec::<f64>::new());
}

#[test]
fn req_rip_016_lines_placed_by_hand_are_sewn_through_their_points_joined_by_even_stitches() {
    let lines = vec![vec![p(-4.0, 0.0), p(0.0, 0.0), p(4.0, 0.0)], vec![p(-4.0, 10.0), p(0.0, 10.0), p(4.0, 10.0)]];
    let sewn = |flip: bool| rounded(&manual(&lines, flip, 2.5, &mut Budget::DEFAULT.meter()).unwrap());
    // With flip_copies the first line from its end, then 10 mm in 4 stitches to the next.
    let flipped = [(4.0, 0.0), (0.0, 0.0), (-4.0, 0.0), (-4.0, 2.5), (-4.0, 5.0), (-4.0, 7.5), (-4.0, 10.0), (0.0, 10.0), (4.0, 10.0)];
    assert_eq!(sewn(true), flipped);
    // Without it, 12.81 mm in 6 stitches.
    let straight = [
        [(-4.0, 0.0), (0.0, 0.0), (4.0, 0.0)].as_slice(),
        &[(2.667, 1.667), (1.333, 3.333), (0.0, 5.0), (-1.333, 6.667), (-2.667, 8.333)],
        &[(-4.0, 10.0), (0.0, 10.0), (4.0, 10.0)],
    ];
    assert_eq!(sewn(false), straight.concat());
    // A join no longer than the stitch length is one stitch, and each point is a stitch and a unit of work.
    let near = vec![vec![p(0.0, 0.0), p(1.0, 0.0)], vec![p(1.0, 2.5), p(2.0, 2.5)]];
    let mut meter = Budget { max_stitches: 4, max_work: 4 }.meter();
    assert_eq!(rounded(&manual(&near, false, 2.5, &mut meter).unwrap()), [(0.0, 0.0), (1.0, 0.0), (1.0, 2.5), (2.0, 2.5)]);
    assert_eq!((meter.stitches_left(), meter.work_left()), (0, 0));
    assert_eq!(manual(&near, false, 2.5, &mut Budget { max_stitches: 3, max_work: 100 }.meter()), Err(Exhausted::Stitches));
    assert_eq!(manual(&near, false, 2.5, &mut Budget { max_stitches: 100, max_work: 3 }.meter()), Err(Exhausted::Work));
    // No stitch length cuts nothing.
    assert_eq!(manual(&near, false, 0.0, &mut Budget::DEFAULT.meter()).unwrap().len(), 4);
}

#[test]
fn req_rip_012_copies_along_a_guide_line_are_of_the_outline_placed_again_and_others_of_the_outline_itself() {
    // A curve's outline is its running stitch. Along a guide line, the copies are of that outline sewn again at
    // the same lengths, which moves its needle points. A satin guide's copies, and copies placed by hand, are of
    // the outline itself.
    let curve = Subpath { start: p(0.0, 0.0), segments: vec![Segment::Cubic(p(3.0, 6.0), p(7.0, -6.0), p(10.0, 0.0))], closed: false };
    let meter = &mut Budget::DEFAULT.meter();
    let points = stroke::flattened(&curve, meter).unwrap();
    let rng = SplitMix64::new(7);
    let built = |guide: RippleGuide, with: &[(&str, &str)]| {
        let ripple = Ripple { guide: Some(guide), ..ripple(with, None) };
        let built = one_line(&curve, &points, &ripple, (&[2.5], None, &rng), &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        built.lines
    };
    let copied = |outline: &[Point], guide: &RippleGuide, with: &[(&str, &str)]| {
        let own = ripple(with, None).ripple;
        let placing = Placing { own: &own, adds: false, closed: false, anchor: None };
        guided::copies(outline, guide, &placing, &mut Vec::new(), &mut Budget::DEFAULT.meter()).unwrap().unwrap().lines
    };
    let outline = running::outline(&curve, &[2.5], (0.2, 0.3), meter).unwrap().unwrap();
    let again = running::along_pattern(&outline, (&[2.5], 0.2, 0.3), None, (&rng, 0), meter).unwrap();
    assert_ne!(again, outline, "the outline placed again differs");
    let line = RippleGuide::Line { path: open(&[&[(0.0, 0.0), (0.0, 10.0)]]), nodes: false };
    let with = [("line_count", "3"), ("scale_end", "100")];
    assert_eq!(built(line.clone(), &with), copied(&again, &line, &with));
    let rails = SatinParams::from_set(&ParamSet::new()).unwrap().params;
    let satin = RippleGuide::Satin {
        path: open(&[&[(0.0, 0.0), (20.0, 0.0)], &[(0.0, 4.0), (20.0, 4.0)]]),
        width: Mm::ZERO,
        join: Join::UNSET,
        params: Box::new(rails),
    };
    assert_eq!(built(satin.clone(), &with), copied(&outline, &satin, &with));
    // By hand, the copies are of the curve's nodes.
    let by_hand = [("line_count", "3"), ("scale_end", "100"), ("manual_pattern_placement", "true")];
    assert_eq!(built(line.clone(), &by_hand), copied(&[p(0.0, 0.0), p(10.0, 0.0)], &line, &by_hand));
}
