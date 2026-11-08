use stitchcraft_core::Budget;

use super::*;
use crate::design::{FillRule, Path, Segment, Subpath};
use crate::route::tests::{bar, element, line, p, stroke};

/// The strokes of `elements` an auto-run route takes, read and cut as `break_up` says, and what is said.
fn read_all(elements: &[Element], break_up: bool) -> (Vec<Stroke>, Vec<String>) {
    let mut said = Vec::new();
    let strokes = strokes(elements, &DesignSettings::default(), break_up, &mut said, &mut Budget::DEFAULT.meter()).unwrap();
    (strokes, said.iter().map(ToString::to_string).collect())
}

/// Each stroke's pieces, their ends as pairs of coordinates.
fn pieces_of(strokes: &[Stroke]) -> Vec<Vec<[(f64, f64); 2]>> {
    strokes.iter().map(|stroke| stroke.pieces.iter().map(|[a, b]| [(a.x(), a.y()), (b.x(), b.y())]).collect()).collect()
}

/// The pieces of the strokes through `lines`, each one stroke's single subpath, cut with Add nodes at
/// intersections on.
fn cut_lines(lines: &[&[(f64, f64)]]) -> Vec<Vec<[(f64, f64); 2]>> {
    let elements: Vec<Element> = lines.iter().enumerate().map(|(k, points)| stroke(&format!("s{k}"), points, &[])).collect();
    let (strokes, said) = read_all(&elements, true);
    assert!(said.is_empty(), "{said:?}");
    pieces_of(&strokes)
}

/// The places a stroke's pieces start and end at, in order: its first point and then the end of each piece.
fn places(pieces: &[[(f64, f64); 2]]) -> Vec<(f64, f64)> {
    pieces.first().map(|first| first[0]).into_iter().chain(pieces.iter().map(|piece| piece[1])).collect()
}

#[test]
fn req_route_009_auto_run_takes_strokes_of_every_method_and_leaves_the_rest_as_they_are() {
    let square = Path { subpaths: vec![Subpath { closed: true, ..line(&[(0.0, 0.0), (5.0, 0.0), (5.0, 5.0)]) }] };
    let elements = [
        element("fill", Shape::Fill { path: square, rule: FillRule::EvenOdd }, &[]),
        bar("column", (0.0, 10.0), 10.0),
        Element { automatic: true, ..stroke("auto", &[(0.0, 20.0), (5.0, 20.0)], &[]) },
        stroke("method", &[(0.0, 30.0), (5.0, 30.0)], &[("stroke_method", "sideways")]),
        stroke("repeats", &[(0.0, 40.0), (5.0, 40.0)], &[("repeats", "many")]),
        stroke("dot", &[(1.0, 50.0), (1.0, 50.0)], &[]),
        stroke("running", &[(0.0, 60.0), (5.0, 60.0)], &[("repeats", "4")]),
        stroke("manual", &[(0.0, 70.0), (5.0, 70.0)], &[("stroke_method", "manual_stitch")]),
        stroke("zigzag", &[(0.0, 80.0), (5.0, 80.0)], &[("stroke_method", "zigzag_stitch")]),
        stroke("ripple", &[(0.0, 90.0), (5.0, 90.0)], &[("stroke_method", "ripple_stitch")]),
    ];
    let (strokes, said) = read_all(&elements, true);
    let taken: Vec<(usize, StitchType, u32)> = strokes.iter().map(|stroke| (stroke.element, stroke.stitch_type, stroke.repeats)).collect();
    assert_eq!(
        taken,
        [(6, StitchType::RunningStitch, 4), (7, StitchType::ManualStitch, 1), (8, StitchType::ZigzagStitch, 1), (9, StitchType::RippleStitch, 1)]
    );
    let start = "info SC-I1002: This element is left out of the route and kept as it is: ";
    let why = [
        "it is a fill, which a route doesn't take",
        "it is a satin column, which Auto-Route Running Stitch doesn't take",
        "it is digitized automatically, which chooses its own stitches",
        "its settings don't read",
        "its settings don't read",
        "it has no length",
    ];
    assert_eq!(said, why.map(|why| format!("{start}{why}.")));
}

#[test]
fn req_route_009_a_stroke_s_lines_are_its_subpaths_flattened_or_a_manual_stitch_s_nodes() {
    // A quarter circle as a cubic curve, then a straight subpath, then a subpath of one point twice over.
    let k = 10.0 * 0.552_284_749_830_793_6;
    let curve = Subpath { start: p(0.0, 10.0), segments: vec![Segment::Cubic(p(0.0, 10.0 - k), p(10.0 - k, 0.0), p(10.0, 0.0))], closed: false };
    let path = Path { subpaths: vec![curve, line(&[(20.0, 0.0), (20.0, 0.0), (30.0, 0.0)]), line(&[(40.0, 0.0), (40.0, 0.0)])] };
    let read = |params: &[(&str, &str)]| pieces_of(&read_all(&[element("s", Shape::stroke(path.clone()), params)], false).0);
    // Flattened within 0.1 CSS pixels, the curve is many pieces, each a chord of it, from its start to its end.
    let flat = read(&[]);
    let curve_pieces = &flat[0][..flat[0].len() - 1];
    assert!(curve_pieces.len() > 8, "{curve_pieces:?}");
    assert_eq!((curve_pieces[0][0], curve_pieces.last().unwrap()[1]), ((0.0, 10.0), (10.0, 0.0)));
    assert!(curve_pieces.windows(2).all(|pair| pair[0][1] == pair[1][0]), "each piece starts where the one before ends");
    // The straight subpath is one piece, its repeated point left out, and the point drawn twice is no line.
    assert_eq!(flat[0].last(), Some(&[(20.0, 0.0), (30.0, 0.0)]));
    // A manual stitch's lines are its nodes: the curve is one piece from its start to its end.
    assert_eq!(read(&[("stroke_method", "manual_stitch")]), [vec![[(0.0, 10.0), (10.0, 0.0)], [(20.0, 0.0), (30.0, 0.0)]]]);
}

#[test]
fn req_route_010_a_line_is_cut_where_it_crosses_or_touches_itself_in_order_along_it() {
    // It crosses itself once: as Ink/Stitch's geometry library nodes it, from its start to the crossing, round
    // the loop back to the crossing, and on to its end.
    let pieces = cut_lines(&[&[(32.0, 10.0), (44.0, 18.0), (44.0, 8.0), (34.0, 16.0)]]);
    let at = |t: f64| (32.0 + 12.0 * t, 10.0 + 8.0 * t);
    let x = at(19.0 / 44.0);
    let found = places(&pieces[0]);
    assert_eq!(found.len(), 6, "{found:?}");
    let want = [(32.0, 10.0), x, (44.0, 18.0), (44.0, 8.0), x, (34.0, 16.0)];
    assert!(found.iter().zip(want).all(|(f, w)| (f.0 - w.0).abs() < 1e-9 && (f.1 - w.1).abs() < 1e-9), "{found:?}");
    assert_eq!(found[1], found[4], "one point where it crosses itself");
    // A point of it on one of its own sides: the side is cut there.
    let touching = cut_lines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (5.0, 0.0), (5.0, -5.0)]]);
    assert_eq!(places(&touching[0]), [(0.0, 0.0), (5.0, 0.0), (10.0, 0.0), (10.0, 5.0), (5.0, 0.0), (5.0, -5.0)]);
    // A line that runs back along itself is cut where it turns back over itself, and keeps both stretches.
    assert_eq!(
        cut_lines(&[&[(0.0, 0.0), (10.0, 0.0), (5.0, 0.0)]]),
        [vec![[(0.0, 0.0), (5.0, 0.0)], [(5.0, 0.0), (10.0, 0.0)], [(10.0, 0.0), (5.0, 0.0)]]]
    );
    // A closed line meets itself only at its ends, and a line that turns without crossing itself isn't cut.
    assert_eq!(cut_lines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 0.0)]])[0].len(), 3);
    assert_eq!(cut_lines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)]])[0].len(), 3);
    // Without Add nodes at intersections, it is cut nowhere.
    let (uncut, _) = read_all(&[stroke("s", &[(32.0, 10.0), (44.0, 18.0), (44.0, 8.0), (34.0, 16.0)], &[])], false);
    assert_eq!(uncut[0].pieces.len(), 3);
}

#[test]
fn req_route_010_lines_are_cut_where_they_cross_touch_or_run_along_each_other_at_one_point() {
    // A cross: both cut where they cross, at one point.
    assert_eq!(
        cut_lines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(4.0, -3.0), (4.0, 3.0)]]),
        [vec![[(0.0, 0.0), (4.0, 0.0)], [(4.0, 0.0), (10.0, 0.0)]], vec![[(4.0, -3.0), (4.0, 0.0)], [(4.0, 0.0), (4.0, 3.0)]]]
    );
    // Slanted lines cross at a point both are cut at, to the last bit: Ink/Stitch's cuts can miss each other.
    let slanted = cut_lines(&[&[(10.3, 31.7), (47.9, 36.1)], &[(12.5, 40.2), (40.4, 14.9)]]);
    assert_eq!(slanted[0][0][1], slanted[1][0][1]);
    // A line that ends on another cuts it there, and a line through a node of another cuts it at that node.
    assert_eq!(cut_lines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(5.0, 0.0), (5.0, 6.0)]])[0], [[(0.0, 0.0), (5.0, 0.0)], [(5.0, 0.0), (10.0, 0.0)]]);
    assert_eq!(
        cut_lines(&[&[(0.0, 0.0), (5.0, 0.0), (10.0, 0.0)], &[(5.0, -2.0), (5.0, 6.0)]])[0],
        [[(0.0, 0.0), (5.0, 0.0)], [(5.0, 0.0), (10.0, 0.0)]]
    );
    // Lines that run along each other are each cut where the other ends, so the stretch they share is a piece
    // of each between the same 2 places (`DEV-ROUTE-005`).
    let along = cut_lines(&[&[(0.0, 0.0), (10.0, 0.0)], &[(6.0, 0.0), (14.0, 0.0)]]);
    assert_eq!(along, [vec![[(0.0, 0.0), (6.0, 0.0)], [(6.0, 0.0), (10.0, 0.0)]], vec![[(6.0, 0.0), (10.0, 0.0)], [(10.0, 0.0), (14.0, 0.0)]]]);
    // Lines that meet in 2 stretches are cut at both, where Ink/Stitch cuts them at neither (`DEV-ROUTE-008`).
    let twice = cut_lines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(2.0, 0.0), (5.0, 0.0), (8.0, 4.0), (12.0, 0.0), (15.0, 0.0)]]);
    assert_eq!(places(&twice[0]), [(0.0, 0.0), (2.0, 0.0), (5.0, 0.0), (12.0, 0.0), (15.0, 0.0), (20.0, 0.0)]);
}

#[test]
fn req_route_010_lines_that_pass_within_13_2_mm_are_cut_at_their_nearest_points() {
    assert!((NEAR - 50.0 * 25.4 / 96.0).abs() < 1e-12, "{NEAR}");
    // Each is cut at its point nearest the other: the post at its end, which cuts nothing, the rail below it.
    // The rail is 16 mm long, so that its distance from the post is worked out without rounding.
    let apart = |gap: f64| cut_lines(&[&[(0.0, 0.0), (16.0, 0.0)], &[(5.0, gap), (5.0, gap + 5.0)]])[0].len();
    assert_eq!(apart(5.0), 2);
    assert_eq!(cut_lines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(5.0, 5.0), (5.0, 10.0)]])[0], [[(0.0, 0.0), (5.0, 0.0)], [(5.0, 0.0), (20.0, 0.0)]]);
    assert_eq!(cut_lines(&[&[(5.0, 5.0), (5.0, 10.0)], &[(0.0, 0.0), (20.0, 0.0)]])[1], [[(0.0, 0.0), (5.0, 0.0)], [(5.0, 0.0), (20.0, 0.0)]]);
    // 13.2 mm apart, exactly 50 CSS pixels, they are cut; a hair farther, they aren't.
    assert_eq!((apart(NEAR), apart(NEAR.next_up())), (2, 1));
    // Of equally near points, the one nearest the other's start: the rail is cut below the bar's start, and
    // below its first side's start where 2 of its sides are as near.
    let below = |bar: &[(f64, f64)]| places(&cut_lines(&[&[(0.0, 0.0), (20.0, 0.0)], bar])[0])[1];
    assert_eq!((below(&[(5.0, 5.0), (15.0, 5.0)]), below(&[(15.0, 5.0), (5.0, 5.0)])), ((5.0, 0.0), (15.0, 0.0)));
    assert_eq!(below(&[(5.0, 5.0), (10.0, 5.0), (15.0, 5.0)]), (5.0, 0.0));
    // Boxes within 13.2 mm of each other hold lines that may pass farther apart: the slanted line passes the
    // rail's end 15.8 mm off, and neither is cut.
    let slanted: &[(f64, f64)] = &[(25.0, 30.0), (35.0, 5.0)];
    assert!(boxes_apart([0.0, 0.0, 20.0, 0.0], [25.0, 5.0, 35.0, 30.0]) < NEAR);
    assert_eq!(cut_lines(&[&[(0.0, 0.0), (20.0, 0.0)], slanted]), [vec![[(0.0, 0.0), (20.0, 0.0)]], vec![[(25.0, 30.0), (35.0, 5.0)]]]);
    // Lines that touch are cut only where they touch.
    assert_eq!(cut_lines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(5.0, 0.0), (8.0, 6.0)]])[0].len(), 2);
}

#[test]
fn req_route_010_cuts_a_millionth_of_a_millimetre_apart_are_one_point() {
    // 3 lines through (10.1, 10.3), each worked out from its slope: they cross each other a rounding error
    // apart, and all 3 are cut at one point.
    let centre = (10.1, 10.3);
    let through = |(dx, dy): (f64, f64)| [(centre.0 - 7.0 * dx, centre.1 - 7.0 * dy), (centre.0 + 9.0 * dx, centre.1 + 9.0 * dy)];
    let lines = [through((0.6, 0.8)), through((0.96, -0.28)), through((-0.28, 0.96))];
    let pieces = cut_lines(&[&lines[0], &lines[1], &lines[2]]);
    let met: Vec<(f64, f64)> = pieces.iter().map(|pieces| pieces[0][1]).collect();
    assert!(met.iter().all(|at| *at == met[0]) && (met[0].0 - centre.0).abs() < 1e-9, "{met:?}");
    // A stretch between 2 cuts however short is kept, where Ink/Stitch leaves out one of 0.1 CSS pixels or less
    // (`DEV-ROUTE-008`).
    let close = cut_lines(&[&[(0.0, 0.0), (20.0, 0.0)], &[(5.0, -1.0), (5.0, 1.0)], &[(5.01, -1.0), (5.01, 1.0)]]);
    assert_eq!(close[0][1], [(5.0, 0.0), (5.01, 0.0)]);
}

#[test]
fn diag_sc_w1004_more_lines_than_the_cap_are_cut_by_none() {
    // 2 lines that cross, and more lines 20 mm apart, out of reach of each other.
    let routed = |count: usize| {
        let mut elements = vec![stroke("a", &[(-10.0, -5.0), (-10.0, 5.0)], &[]), stroke("b", &[(-15.0, 0.0), (-5.0, 0.0)], &[])];
        elements.extend((0..count - 2).map(|k| {
            stroke(&format!("s{k}"), &[(20.0 * (k % 40) as f64, 20.0 * (k / 40) as f64), (20.0 * (k % 40) as f64 + 1.0, 20.0 * (k / 40) as f64)], &[])
        }));
        read_all(&elements, true)
    };
    let (cut, said) = routed(MOST_LINES);
    assert!(said.is_empty() && cut[0].pieces.len() == 2, "{said:?}");
    let (uncut, said) = routed(MOST_LINES + 1);
    assert_eq!(uncut[0].pieces.len(), 1);
    assert_eq!(
        said,
        ["warning SC-W1004: There are 1001 lines to route, more than the 1000 that are cut where they cross and pass near each other, so \
             they are routed as drawn, each cut only where it crosses itself."]
    );
}

#[test]
fn a_line_is_cut_at_its_own_points_once_and_a_cut_at_an_end_changes_nothing() {
    let mut line = Line::new(0, vec![p(0.0, 0.0), p(4.0, 0.0), p(10.0, 0.0)]);
    assert_eq!((line.along.clone(), line.bounds), (vec![0.0, 4.0, 10.0], [0.0, 0.0, 10.0, 0.0]));
    // At the point between 2 sides, from either of them; at a point past a side's end, at that end.
    line.cut(0, p(4.0, 0.0));
    line.cut(1, p(4.0, 0.0));
    line.cut(1, p(10.0, 0.0));
    line.cut(1, p(10.0f64.next_up(), 0.0));
    line.cut(0, p(0.0, 0.0));
    line.cut(1, p(7.0, 0.0));
    line.cut(9, p(7.0, 0.0));
    assert_eq!(line.cuts.iter().map(|cut| cut.0).collect::<Vec<_>>(), [4.0, 4.0, 10.0, 10.0, 0.0, 7.0]);
    let stretches = line.stretches();
    assert_eq!(stretches, [vec![p(0.0, 0.0), p(4.0, 0.0)], vec![p(4.0, 0.0), p(7.0, 0.0)], vec![p(7.0, 0.0), p(10.0, 0.0)]]);
    // A cut a hair off the line, as a point taken through the snap can be, is a point of the stretches it ends
    // and starts, in place of the line's point as far along it, and at the line's end, a stretch on to it.
    let mut off = Line::new(0, vec![p(0.0, 0.0), p(4.0, 0.0), p(10.0, 0.0)]);
    off.cut(0, p(4.0, 1e-7));
    off.cut(1, p(10.0, 1e-7));
    assert_eq!(off.cuts.iter().map(|cut| cut.0).collect::<Vec<_>>(), [4.0, 10.0]);
    let (hair, end) = (p(4.0, 1e-7), p(10.0, 1e-7));
    assert_eq!(off.stretches(), [vec![p(0.0, 0.0), hair], vec![hair, end], vec![end, p(10.0, 0.0)]]);
    let mut longer = Line::new(0, vec![p(0.0, 0.0), p(4.0, 0.0), p(10.0, 0.0), p(12.0, 0.0)]);
    longer.cut(0, hair);
    assert_eq!(longer.stretches(), [vec![p(0.0, 0.0), hair], vec![hair, p(10.0, 0.0), p(12.0, 0.0)]]);
    // 2 cuts at one point a hair apart along the line leave no stretch between them.
    let mut twice = Line::new(0, vec![p(0.0, 0.0), p(10.0, 0.0)]);
    twice.cuts = vec![(5.0, p(5.0, 0.0)), (5.000_000_1, p(5.0, 0.0))];
    assert_eq!(twice.stretches(), [vec![p(0.0, 0.0), p(5.0, 0.0)], vec![p(5.0, 0.0), p(10.0, 0.0)]]);
}

#[test]
fn sides_meet_where_they_cross_where_one_ends_on_the_other_and_where_they_overlap() {
    let meet = |s: [(f64, f64); 2], t: [(f64, f64); 2]| {
        meetings((p(s[0].0, s[0].1), p(s[1].0, s[1].1)), (p(t[0].0, t[0].1), p(t[1].0, t[1].1))).iter().map(|q| (q.x(), q.y())).collect::<Vec<_>>()
    };
    assert_eq!(meet([(0.0, 0.0), (10.0, 0.0)], [(4.0, -3.0), (4.0, 3.0)]), [(4.0, 0.0)]);
    assert_eq!(meet([(0.0, 0.0), (10.0, 0.0)], [(4.0, 0.0), (4.0, 3.0)]), [(4.0, 0.0)]);
    assert_eq!(meet([(4.0, -3.0), (4.0, 0.0)], [(0.0, 0.0), (10.0, 0.0)]), [(4.0, 0.0)]);
    assert_eq!(meet([(0.0, 0.0), (10.0, 0.0)], [(10.0, 0.0), (12.0, 3.0)]), [(10.0, 0.0)]);
    assert_eq!(meet([(0.0, 0.0), (10.0, 0.0)], [(6.0, 0.0), (14.0, 0.0)]), [(6.0, 0.0), (10.0, 0.0)]);
    assert_eq!(meet([(6.0, 0.0), (14.0, 0.0)], [(0.0, 0.0), (10.0, 0.0)]), [(10.0, 0.0), (6.0, 0.0)]);
    // Slanted: where they cross, worked out from the 4 ends, and where one ends on the other, that end only.
    let crossing = meet([(1.0, 2.0), (9.0, 6.0)], [(3.0, 7.0), (6.0, 1.0)]);
    assert!(crossing.len() == 1 && (crossing[0].0 - 4.6).abs() < 1e-12 && (crossing[0].1 - 3.8).abs() < 1e-12, "{crossing:?}");
    assert_eq!(meet([(0.0, 0.0), (10.0, 10.0)], [(5.0, 5.0), (8.0, 2.0)]), [(5.0, 5.0)]);
    // Apart, side by side, in one line apart, or crossing the other's line beyond its end: nowhere.
    assert!(meet([(0.0, 0.0), (10.0, 0.0)], [(4.0, 1.0), (4.0, 3.0)]).is_empty());
    assert!(meet([(0.0, 0.0), (10.0, 2.0)], [(0.0, 1.0), (10.0, 3.0)]).is_empty());
    assert!(meet([(0.0, 0.0), (10.0, 0.0)], [(11.0, 0.0), (14.0, 0.0)]).is_empty());
    assert!(meet([(0.0, 0.0), (4.0, 0.0)], [(2.0, 3.0), (8.0, -3.0)]).is_empty());
    assert!(meet([(2.0, 3.0), (8.0, -3.0)], [(0.0, 0.0), (4.0, 0.0)]).is_empty());
}

#[test]
fn boxes_are_as_far_apart_as_their_nearest_corners_or_sides() {
    let unit = [0.0, 0.0, 1.0, 1.0];
    assert_eq!(boxes_apart(unit, [4.0, 5.0, 6.0, 7.0]), 5.0);
    assert_eq!(boxes_apart([4.0, 5.0, 6.0, 7.0], unit), 5.0);
    assert_eq!((boxes_apart(unit, [3.0, 0.5, 4.0, 2.0]), boxes_apart(unit, [0.5, -4.0, 2.0, -2.0])), (2.0, 2.0));
    assert_eq!(boxes_apart(unit, [0.5, 0.5, 3.0, 3.0]), 0.0);
}
