//! The joining of rows that close inside a piece: its cases (`REQ-FILL-GDD-011`).

use stitchcraft_core::Budget;

use super::*;
use crate::generators::tatami::fixture::{costs, frame, p, rectangle};

fn xy(points: &[Point]) -> Vec<(f64, f64)> {
    points.iter().map(|q| ((q.x() * 1e6).round() / 1e6, (q.y() * 1e6).round() / 1e6)).collect()
}

/// The closed square from (`x0`, `y0`) to (`x1`, `y1`), from that first corner along x first: counter-clockwise
/// with y up.
fn square(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
    vec![p(x0, y0), p(x1, y0), p(x1, y1), p(x0, y1), p(x0, y0)]
}

fn turned(ring: &[Point]) -> Vec<Point> {
    ring.iter().rev().copied().collect()
}

#[test]
fn req_fill_gdd_011_a_ring_goes_into_the_ring_round_it_where_2_points_a_spacing_apart_lie_near() {
    let meter = &mut Budget::DEFAULT.meter();
    let inner = square(4.0, 1.5, 6.0, 2.5);
    let outer = square(3.5, 1.0, 6.5, 3.0);
    // The walk's first 2 points, the inner ring's start and the point 0.5 mm on, lie 0.5 mm from the outer
    // ring. The outer ring starts at the second's nearest point and loses its last 0.5 mm, and the inner one
    // follows backwards from its start to the second point.
    let joined = ring_into_ring(&inner, &outer, (0.5, 3.0, false), meter).unwrap().unwrap();
    let expected = [
        [(4.5, 1.0), (6.5, 1.0), (6.5, 3.0), (3.5, 3.0), (3.5, 1.0), (4.0, 1.0)],
        [(4.0, 1.5), (4.0, 2.5), (6.0, 2.5), (6.0, 1.5), (4.5, 1.5), (4.5, 1.0)],
    ];
    assert_eq!(xy(&joined), expected.concat());
    // Whichever way they were drawn: both are turned counter-clockwise first.
    assert_eq!(ring_into_ring(&turned(&inner), &turned(&outer), (0.5, 3.0, false), meter).unwrap().unwrap(), joined);
    // With skip_last the inner ring is cut into stitches of the longest stitch, here 0.4 mm, and its ends are
    // left out.
    let skipped = ring_into_ring(&inner, &outer, (0.5, 0.4, true), meter).unwrap().unwrap();
    assert_eq!(xy(&skipped[..6]), expected[0]);
    assert_eq!(xy(&skipped[6..9]), [(4.0, 1.833333), (4.0, 2.166667), (4.0, 2.5)]);
    assert_eq!(xy(&skipped[skipped.len() - 3..]), [(5.25, 1.5), (4.875, 1.5), (4.5, 1.0)]);
    assert_eq!(skipped.len(), 21);
    // 2½ row spacings, 1.25 mm, is too far: just past it no place is found, and just within it one is.
    let mut at = |gap: f64| ring_into_ring(&inner, &square(4.0 - gap, 1.5 - gap, 6.0 + gap, 2.5 + gap), (0.5, 3.0, false), meter).unwrap();
    assert_eq!(at(1.26), None);
    assert!(at(1.24).is_some());
    // A ring shorter than a row spacing has no 2 points to take, nor has one just a row spacing long.
    assert_eq!(ring_into_ring(&square(5.0, 2.0, 5.1, 2.1), &outer, (0.5, 3.0, false), meter).unwrap(), None);
    assert_eq!(ring_into_ring(&square(5.0, 2.0, 5.125, 2.125), &outer, (0.5, 3.0, false), meter).unwrap(), None);
    assert!(ring_into_ring(&square(5.0, 2.0, 5.25, 2.25), &outer, (0.5, 3.0, false), meter).unwrap().is_some());
    // Each of the 2 points must lie nearer than 2½ row spacings. Here the first lies just 1.25 mm from the
    // outer ring, below its top side and 0.75 and 1 mm from a step in it, and the second nearer: the rings
    // are joined from the walk's next step. The other way round, with the second point just 1.25 mm away, no
    // step of the walk finds a place.
    let stepped =
        |step: f64, left: f64, right: f64| vec![p(0.0, left), p(step, left), p(step, right), p(10.0, right), p(10.0, 6.0), p(0.0, 6.0), p(0.0, left)];
    let first_at_the_limit = ring_into_ring(&inner, &stepped(4.75, 0.25, 0.5), (0.5, 3.0, false), meter).unwrap().unwrap();
    assert!((first_at_the_limit[0].x() - (4.5 + STEP)).abs() < 1e-9 && first_at_the_limit[0].y() == 0.5, "{:?}", first_at_the_limit[0]);
    assert_eq!(ring_into_ring(&inner, &stepped(3.75, 0.5, 0.25), (0.5, 3.0, false), meter).unwrap(), None);
    // The walk goes on a CSS pixel at a time until both points are near: here from the second step, the first
    // lying 1.5 mm from a ring that is near the inner one's right half alone.
    let beside = square(5.5, 1.0, 6.5, 3.0);
    let stepped = ring_into_ring(&inner, &beside, (0.5, 3.0, false), meter).unwrap().unwrap();
    assert_eq!(xy(&stepped[..1]), [(5.5, 1.5)]);
    assert!((stepped[stepped.len() - 2].x() - (4.0 + STEP + 0.5)).abs() < 1e-9, "{:?}", xy(&stepped));
}

#[test]
fn req_fill_gdd_011_a_ring_goes_into_a_row_that_is_opened_a_row_spacing_long() {
    let meter = &mut Budget::DEFAULT.meter();
    let piece = rectangle(4.0);
    let rings = Rings::new(&piece, meter).unwrap();
    let ring = square(3.5, 1.0, 6.5, 3.0);
    // A level row 0.25 mm above the ring: opened from x = 3.5 to 4, where the ring's first 2 points lie
    // nearest, and the ring goes in from its start, the way round that keeps the row from crossing itself.
    let row = [p(-1.0, 0.75), p(11.0, 0.75)];
    let joined = ring_into_row(&rings, &ring, &row, 0.5, meter).unwrap().unwrap();
    let expected = [(-1.0, 0.75), (3.5, 0.75), (3.5, 1.0), (3.5, 3.0), (6.5, 3.0), (6.5, 1.0), (4.0, 1.0), (4.0, 0.75), (11.0, 0.75)];
    assert_eq!(xy(&joined), expected);
    // The row the other way: the ring goes in as it runs.
    let back = ring_into_row(&rings, &ring, &turned(&row), 0.5, meter).unwrap().unwrap();
    assert_eq!(xy(&back), [(11.0, 0.75), (4.0, 0.75), (4.0, 1.0), (6.5, 1.0), (6.5, 3.0), (3.5, 3.0), (3.5, 1.0), (3.5, 0.75), (-1.0, 0.75)]);
    // A ring that is a line already, joined before: it is walked as it is and closed where it is opened.
    let open = &ring[..4];
    let line = ring_into_row(&rings, open, &row, 0.5, meter).unwrap().unwrap();
    assert_eq!(xy(&line), expected);
    // 1½ row spacings, 0.75 mm, is too far, measured to the row's stretches in the piece: a row outside it has
    // none.
    let mut far = |y: f64| ring_into_row(&rings, &ring, &[p(-1.0, y), p(11.0, y)], 0.5, meter).unwrap();
    assert_eq!(far(0.24), None);
    assert!(far(0.26).is_some());
    assert_eq!(ring_into_row(&rings, &square(3.5, 0.1, 6.5, 3.0), &[p(-1.0, -0.25), p(11.0, -0.25)], 0.5, meter).unwrap(), None);
    // A ring shorter than a row spacing has no 2 points to take.
    assert_eq!(ring_into_row(&rings, &square(5.0, 0.8, 5.1, 0.9), &row, 0.5, meter).unwrap(), None);
    // Each of the 2 points must lie nearer than 1½ row spacings, here 3.75 mm with rows 2.5 mm apart. A row
    // with a step in it lies just that far from the ring's first point and nearer its second: the ring goes
    // in from the walk's next step. With the second point just that far, it finds no place.
    let wide = Polygon { outline: square(0.0, 0.0, 40.0, 40.0), holes: Vec::new() };
    let wide_rings = Rings::new(&wide, meter).unwrap();
    let big = square(10.0, 10.0, 20.0, 20.0);
    let stepped = |step: f64, left: f64, right: f64| [p(0.0, left), p(step, left), p(step, right), p(40.0, right)];
    let first_at_the_limit = ring_into_row(&wide_rings, &big, &stepped(12.25, 6.25, 7.0), 2.5, meter).unwrap().unwrap();
    assert!(first_at_the_limit.iter().any(|q| (q.x() - (10.0 + STEP)).abs() < 1e-9 && q.y() == 10.0), "{:?}", xy(&first_at_the_limit));
    assert_eq!(ring_into_row(&wide_rings, &big, &stepped(10.25, 7.0, 6.25), 2.5, meter).unwrap(), None);
}

#[test]
fn req_fill_gdd_011_2_rows_are_crossed_where_they_come_nearest() {
    let meter = &mut Budget::DEFAULT.meter();
    let one = [p(0.0, 1.0), p(10.0, 1.0)];
    // Side by side and the same way: each runs to their nearest points, their starts, and goes on along the
    // other, a row spacing on.
    let (first, second) = crossed(&one, &[p(0.0, 1.5), p(10.0, 1.5)], 0.5, meter).unwrap();
    assert_eq!((xy(&first), xy(&second)), (vec![(0.0, 1.0), (0.0, 1.5)], vec![(10.0, 1.0), (0.5, 1.0), (0.5, 1.5), (10.0, 1.5)]));
    // The second the other way: the lines across would meet, so it is turned round, and its point a row
    // spacing back counts from its end.
    let (first, second) = crossed(&one, &[p(10.0, 1.5), p(0.0, 1.5)], 0.5, meter).unwrap();
    assert_eq!((xy(&first), xy(&second)), (vec![(0.0, 1.0), (10.0, 1.0), (10.0, 1.5), (0.0, 1.5)], vec![(10.0, 1.0), (0.5, 1.5), (10.0, 1.5)]));
    // Nearest in their middles, the second the other way: its point a row spacing back lies along it.
    let bent = [p(3.0, 0.0), p(3.0, 1.0), p(10.0, 1.0)];
    let (first, second) = crossed(&bent, &[p(6.0, 1.5), p(0.0, 1.5)], 0.5, meter).unwrap();
    assert_eq!(xy(&first), [(3.0, 0.0), (3.0, 1.0), (3.0, 1.5), (0.0, 1.5)]);
    assert_eq!(xy(&second), [(10.0, 1.0), (3.5, 1.0), (3.5, 1.5), (6.0, 1.5)]);
    // Just a row spacing from its start, that point is its start, and nearer, it counts from its end.
    let (_, second) = crossed(&bent, &[p(3.5, 1.5), p(0.0, 1.5)], 0.5, meter).unwrap();
    assert_eq!(xy(&second), [(10.0, 1.0), (3.5, 1.0), (3.5, 1.5)]);
    let (_, second) = crossed(&bent, &[p(3.2, 1.5), p(0.0, 1.5)], 0.5, meter).unwrap();
    assert_eq!(xy(&second), [(10.0, 1.0), (3.5, 1.0), (0.3, 1.5), (3.2, 1.5)]);
    // A line of no point is left as it is.
    assert_eq!(crossed(&one, &[], 0.5, meter).unwrap(), (one.to_vec(), Vec::new()));
}

#[test]
fn req_fill_gdd_011_a_ring_beside_a_hole_is_joined_on_into_the_next_row() {
    let meter = &mut Budget::DEFAULT.meter();
    let piece = frame();
    let rings = Rings::new(&piece, meter).unwrap();
    let hole = piece.holes[0].clone();
    // A ring 0.25 mm left of the hole, and a row 0.2 mm left of the ring that starts inside the piece.
    let ring = square(3.0, 1.5, 3.75, 2.5);
    let inside = vec![p(2.8, 0.5), p(2.8, 3.5)];
    let pass = |rows: &mut Vec<Vec<Point>>, removed: Vec<usize>| {
        let mut pass = Pass { removed, added: Vec::new(), unjoined: Vec::new() };
        pass.ring_into_rows(&rings, ring.clone(), rows, 0.5, &mut Budget::DEFAULT.meter()).unwrap();
        pass
    };
    // The hole's ring takes it first, which meets no outline, so the row takes both, and is done with.
    let mut rows = vec![hole.clone(), inside.clone()];
    let done = pass(&mut rows, Vec::new());
    assert_eq!((done.removed, done.added.len(), done.unjoined.len()), (vec![1], 0, 0));
    assert_eq!(rows[1], inside);
    let both = [
        [(2.8, 0.5), (2.8, 2.475), (3.0, 2.475), (3.0, 1.5), (3.264583, 1.5), (4.0, 1.014583), (4.0, 1.0), (6.0, 1.0)],
        [(6.0, 3.0), (4.0, 3.0), (4.0, 1.514583), (3.75, 1.514583), (3.75, 2.5), (3.475, 2.5), (2.8, 2.975), (2.8, 3.5)],
    ];
    assert_eq!(xy(&rows[0]), both.concat());
    // A row that starts outside the piece is crossed with them into 2 rows.
    let through = vec![p(2.8, -0.5), p(2.8, 4.5)];
    let mut rows = vec![hole.clone(), through.clone()];
    let done = pass(&mut rows, Vec::new());
    assert_eq!((done.removed, done.added.len(), done.unjoined.len()), (vec![1], 1, 0));
    assert_eq!((rows[0].first(), rows[0].last()), (hole.first(), through.last()));
    assert_eq!(done.added[0].last(), through.first());
    // With no other row near, the ring joined into the hole's waits for the next pass.
    let mut rows = vec![hole.clone(), vec![p(9.0, 0.5), p(9.0, 3.5)]];
    let done = pass(&mut rows, Vec::new());
    assert_eq!((done.removed, done.added.len()), (Vec::new(), 0));
    assert_eq!(done.unjoined, [rows[0].clone()]);
    assert!(rows[0].len() > hole.len());
    // A row taken out earlier in the pass is passed over, and a row a hair more than a row spacing away is
    // not near.
    let mut rows = vec![hole.clone(), inside];
    let done = pass(&mut rows, vec![0]);
    assert_eq!((rows[0].clone(), done.unjoined), (hole, vec![rows[1].clone()]));
    assert_eq!(xy(&rows[1]), [(2.8, 0.5), (2.8, 1.5), (3.0, 1.5), (3.0, 2.5), (3.75, 2.5), (3.75, 1.5), (3.5, 1.5), (2.8, 2.0), (2.8, 3.5)]);
    let aside = |x: f64| {
        let mut rows = vec![vec![p(x, -0.5), p(x, 4.5)]];
        pass(&mut rows, Vec::new()).unjoined.len()
    };
    // The ring's left side is at x = 3: a row 0.5005 mm from it is too far, and one 0.5001 mm from it is near, a
    // thousandth of a CSS pixel being 0.00026 mm.
    assert_eq!((aside(2.4995), aside(2.4999)), (1, 0));
}

#[test]
fn req_fill_gdd_011_the_rows_are_sorted_out_and_joined() {
    let meter = &mut Budget::DEFAULT.meter();
    let piece = rectangle(4.0);
    let rings = Rings::new(&piece, meter).unwrap();
    let with = (0.5, 3.0, false);
    let level = |y: f64| vec![p(-1.0, y), p(11.0, y)];
    // Without a row inside, the rows stay as they are, one that misses the piece too.
    let rows = vec![level(1.0), level(9.0), square(1.0, 1.0, 5.0, 6.0)];
    assert_eq!(joined(&piece, &rings, rows.clone(), with, meter).unwrap(), rows);
    // With one, a row that misses the piece is left out, and a closed row that starts inside the piece starts
    // at the middle of its first stretch outside it.
    let ring = square(6.0, 1.5, 8.0, 2.5);
    let all = joined(&piece, &rings, vec![level(1.0), level(9.0), square(1.0, 1.0, 5.0, 6.0), ring.clone()], with, meter).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all[0].len() > 2 && all[0].contains(&p(8.0, 2.5)), "{:?}", xy(&all[0]));
    assert_eq!(xy(&all[1]), [(3.0, 6.0), (1.0, 6.0), (1.0, 1.0), (5.0, 1.0), (5.0, 6.0), (3.0, 6.0)]);
    // A row that starts inside the piece and doesn't close stays as it is, whatever lies outside.
    let hook = vec![p(2.0, 1.0), p(2.0, 6.0), p(4.0, 6.0), p(4.0, 2.0)];
    let kept = joined(&piece, &rings, vec![hook.clone(), ring.clone()], with, meter).unwrap();
    assert_eq!(kept.len(), 1);
    assert_eq!(kept[0].first(), hook.first());
    // 2 rings side by side, neither round the other: each goes into the row.
    let apart = joined(&piece, &rings, vec![level(1.0), square(1.0, 1.5, 3.0, 2.5), ring.clone()], with, meter).unwrap();
    assert_eq!(apart.len(), 1);
    assert!(apart[0].contains(&p(3.0, 2.5)) && apart[0].contains(&p(8.0, 2.5)), "{:?}", xy(&apart[0]));
    // A ring inside another goes into it first, smallest first, and then both into the row.
    let nested = vec![level(1.0), square(5.5, 1.5, 8.5, 3.0), square(6.0, 2.0, 8.0, 2.5)];
    let one = joined(&piece, &rings, nested.clone(), with, meter).unwrap();
    assert_eq!(one.len(), 1);
    for q in nested[1].iter().chain(&nested[2]).skip(1) {
        assert!(one[0].contains(q) || q.distance(p(5.5, 1.5)) < 0.6 || q.distance(p(6.0, 2.0)) < 0.6, "{q:?} of {:?}", xy(&one[0]));
    }
    // A ring too far inside the one round it is left out, and a ring with no row near is, after 6 tries.
    let lost = joined(&piece, &rings, vec![level(0.25), square(1.0, 0.5, 9.0, 3.5), square(4.0, 1.9, 6.0, 2.1)], with, meter).unwrap();
    assert_eq!(lost.len(), 1);
    assert!(lost[0].contains(&p(9.0, 3.5)) && !lost[0].contains(&p(6.0, 2.1)), "{:?}", xy(&lost[0]));
    assert_eq!(joined(&piece, &rings, vec![level(0.25), ring], with, meter).unwrap(), [level(0.25)]);
    // A row of fewer than 4 points that lies inside is cut into thirds and closed.
    let short = joined(&piece, &rings, vec![level(1.0), vec![p(4.0, 1.25), p(7.0, 1.25)]], with, meter).unwrap();
    assert_eq!(short.len(), 1);
    assert!(short[0].contains(&p(7.0, 1.25)) && short[0].contains(&p(6.0, 1.25)), "{:?}", xy(&short[0]));
}

#[test]
fn req_fill_gdd_011_without_a_row_that_meets_the_outline_the_piece_s_rings_take_the_rows_place() {
    let meter = &mut Budget::DEFAULT.meter();
    // 2 holes 0.5 mm apart, and a ring between them, 0.1 mm from each: it goes into the first hole's ring,
    // which meets no outline, so the 2 holes' rings are crossed into 2 rows.
    let holes = [square(4.0, 1.0, 6.0, 3.0), square(6.5, 1.0, 8.5, 3.0)];
    let piece = Polygon { holes: holes.iter().map(|hole| turned(hole)).collect(), ..rectangle(4.0) };
    let rings = Rings::new(&piece, meter).unwrap();
    let ring = square(6.1, 1.5, 6.4, 2.5);
    let rows = joined(&piece, &rings, vec![ring], (0.25, 3.0, false), meter).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[0], piece.outline);
    let first = [
        (4.0, 1.0),
        (4.0, 3.0),
        (6.0, 3.0),
        (6.0, 1.5),
        (6.1, 1.5),
        (6.1, 2.5),
        (6.4, 2.5),
        (6.5, 2.5),
        (6.5, 3.0),
        (8.5, 3.0),
        (8.5, 1.0),
        (6.5, 1.0),
    ];
    assert_eq!(xy(&rows[1]), first);
    assert_eq!(xy(&rows[2]), [(4.0, 1.0), (6.0, 1.0), (6.0, 1.25), (6.35, 1.5), (6.4, 1.5), (6.4, 2.25), (6.5, 2.25), (6.5, 1.0)]);
    // A ring within a row spacing of the outline goes into it.
    let piece = rectangle(4.0);
    let rings = Rings::new(&piece, meter).unwrap();
    let rows = joined(&piece, &rings, vec![square(2.0, 0.25, 8.0, 3.75)], (0.5, 3.0, false), meter).unwrap();
    assert_eq!(rows.len(), 1);
    assert!(rows[0].contains(&p(8.0, 3.75)) && rows[0].contains(&p(10.0, 4.0)), "{:?}", xy(&rows[0]));
}

#[test]
fn joining_runs_out_of_a_budget_of_any_less_work_than_it_takes() {
    let piece = frame();
    let rings = Rings::new(&piece, &mut Budget::DEFAULT.meter()).unwrap();
    let rows = vec![vec![p(-1.0, 0.5), p(11.0, 0.5)], square(1.0, 1.0, 3.0, 3.0), square(1.5, 1.5, 2.5, 2.5), vec![p(7.0, 2.0), p(8.0, 2.0)]];
    let through = vec![p(2.8, -0.5), p(2.8, 4.5)];
    let work = costs(&[
        Box::new(|meter| joined(&piece, &rings, rows.clone(), (0.5, 0.4, true), meter).map(|_| ())),
        Box::new(|meter| crossed(&rows[0], &through, 0.5, meter).map(|_| ())),
    ]);
    assert!(work[0] > 100 && work[1] > 4, "{work:?}");
}
