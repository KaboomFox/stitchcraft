//! The broad and narrow parts of a part wider than a satin column in places (`REQ-AUTO-019`): a tab, an arm
//! and a bar are narrow parts cut square where they meet the broad parts, and corners and stubs stay in the
//! fill.

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Point};

use super::*;
use crate::normalize::fixture::part;
use crate::normalize::region::ops::{area_of, union};

/// The point (`x`, `y`).
fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

/// `piece`, a narrow part of `part` whose ring runs through `points` in order, cut where it meets the broad
/// parts: its ring and its cuts.
fn cut_of(points: &[(f64, f64)], part: &Polygon) -> (Vec<Point>, Vec<[Point; 2]>) {
    let mut meter = Budget::DEFAULT.meter();
    let edges = Edges::of(part, narrow::piece(part), &mut meter).unwrap();
    let ring: Vec<Point> = points.iter().map(|&(x, y)| p(x, y)).collect();
    let piece = Polygon { outline: geom::closed(&ring), holes: Vec::new() };
    let Piece { part: piece, cuts } = cut(piece, part, &edges, &mut meter).unwrap();
    (piece.outline, cuts)
}

/// The tab of [`square_with_tab`] 12 mm long, its ring from 1 mm along its bottom side round to the square's
/// side at its top, and back through 3 points of the opening's arc, inside the square: an arc whose ends lie
/// 1 mm apart along the tab.
const TAB: [(f64, f64); 7] = [(21.0, 8.5), (32.0, 8.5), (32.0, 11.5), (20.0, 11.5), (19.4, 10.6), (19.6, 9.2), (20.4, 8.7)];

/// `part` split, with the default budget.
fn split_of(part: &Polygon) -> Split {
    split(part, &AutoParams::defaults(), &mut Budget::DEFAULT.meter()).unwrap().unwrap()
}

/// The narrow parts of `split`.
fn pieces(split: &Split) -> Vec<Polygon> {
    split.narrow.iter().map(|piece| piece.part.clone()).collect()
}

/// The least and greatest x and y of `piece`'s outline.
fn bounds(piece: &Polygon) -> [f64; 4] {
    piece.outline.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |[x0, y0, x1, y1], q| {
        [x0.min(q.x()), y0.min(q.y()), x1.max(q.x()), y1.max(q.y())]
    })
}

/// The outline of a square 20 mm a side with a tab 3 mm wide and `length` long on its right side, halfway up.
fn tab_outline(length: f64) -> [(f64, f64); 8] {
    [(0.0, 0.0), (20.0, 0.0), (20.0, 8.5), (20.0 + length, 8.5), (20.0 + length, 11.5), (20.0, 11.5), (20.0, 20.0), (0.0, 20.0)]
}

/// A square 20 mm a side with a tab 3 mm wide and `length` long on its right side, halfway up.
fn square_with_tab(length: f64) -> Polygon {
    part(&tab_outline(length), &[])
}

#[test]
fn req_auto_019_a_tab_on_a_broad_part_is_a_narrow_part_cut_square_where_it_meets_it() {
    // A square 20 mm a side with a tab 3 mm wide and 12 mm long: the tab is a narrow part, cut square across
    // its mouth along the square's side, and the square, its corners in it, is the fill. Together they are
    // the part.
    let split = split_of(&square_with_tab(12.0));
    let [tab]: [Polygon; 1] = pieces(&split).try_into().unwrap();
    let [x0, y0, x1, y1] = bounds(&tab);
    assert!((x0 - 20.0).abs() < 0.01 && x1 == 32.0 && y0 == 8.5 && y1 == 11.5, "{tab:?}");
    let mouth: Vec<Point> = tab.outline.iter().copied().filter(|q| q.x() < 20.5).collect();
    assert!(mouth.len() >= 2 && mouth.iter().all(|q| (q.x() - x0).abs() < 1e-9), "{mouth:?}");
    assert!((area_of(&split.fill) - 400.0).abs() < 0.1, "{:?}", split.fill);
    assert!((area_of(&split.fill) + tab.area() - 436.0).abs() < 1e-6);
}

#[test]
fn req_auto_019_a_corner_or_a_stub_the_opening_rounds_stays_in_the_fill() {
    // A square 20 mm a side: the opening rounds its corners, which stay in the fill, the square as it is.
    // With a tab 4 mm long, which reaches less than 1 mm² beyond 3.5 mm from the square, too short for a
    // column, the tab stays in the fill too.
    for whole in [part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (0.0, 20.0)], &[]), square_with_tab(4.0)] {
        let split = split_of(&whole);
        assert!(split.narrow.is_empty() && split.fill == [whole], "{split:?}");
    }
}

#[test]
fn req_auto_019_an_arm_is_cut_square_across_where_it_meets_the_broad_part() {
    // An L: a square 20 mm a side with an arm 5 mm wide and 20 mm long up its left side. The opening's arc
    // meets the arm's outer side 3 mm below the square's top, but the arm is cut square across from its inner
    // corner, at the square's top, and what lies below goes to the fill.
    let l = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (5.0, 20.0), (5.0, 40.0), (0.0, 40.0)], &[]);
    let split = split_of(&l);
    let [arm]: [Polygon; 1] = pieces(&split).try_into().unwrap();
    let [x0, y0, x1, y1] = bounds(&arm);
    assert!(x0 == 0.0 && x1 == 5.0 && (y0 - 20.0).abs() < 0.01 && y1 == 40.0, "{arm:?}");
    let mouth: Vec<Point> = arm.outline.iter().copied().filter(|q| q.y() < 20.5).collect();
    assert!(mouth.len() >= 2 && mouth.iter().all(|q| (q.y() - y0).abs() < 1e-9), "{mouth:?}");
    assert!((area_of(&split.fill) + arm.area() - l.area()).abs() < 1e-6);
}

#[test]
fn req_auto_019_a_bar_between_2_broad_parts_is_cut_square_at_both_ends() {
    // 2 discs 16 mm across, 30 mm apart, joined by a bar 3 mm wide: the bar is a narrow part cut square where
    // it meets each disc, and the discs are the fill, in 2 parts.
    let disc = |cx: f64| {
        let ring: Vec<(f64, f64)> = (0..64)
            .map(|i| {
                let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(i) / 64.0);
                (cx + 8.0 * cos, 8.0 * sin)
            })
            .collect();
        part(&ring, &[])
    };
    let bar = part(&[(5.0, -1.5), (25.0, -1.5), (25.0, 1.5), (5.0, 1.5)], &[]);
    let joined = union([[disc(0.0), disc(30.0), bar].as_slice()], &mut Budget::DEFAULT.meter()).unwrap().unwrap();
    let [dumbbell] = joined.as_slice() else { panic!("{joined:?}") };
    let split = split_of(dumbbell);
    let [bar]: [Polygon; 1] = pieces(&split).try_into().unwrap();
    let [x0, _, x1, _] = bounds(&bar);
    for end in [x0, x1] {
        let mouth: Vec<Point> = bar.outline.iter().copied().filter(|q| (q.x() - end).abs() < 0.5).collect();
        assert!(mouth.len() >= 2 && mouth.iter().all(|q| (q.x() - end).abs() < 1e-9), "{mouth:?}");
    }
    assert!(x0 > 7.8 && x0 < 8.0 && x1 > 22.0 && x1 < 22.2, "{bar:?}");
    assert_eq!(split.fill.len(), 2, "{:?}", split.fill);
}

#[test]
fn req_auto_019_a_cut_square_across_that_would_cross_the_outline_gives_way_to_the_chord_between_the_arcs_ends() {
    // The arc's end along the tab's bottom side lies 1 mm farther along the tab than the one at the square's
    // side, and the cut square across from it, at x = 21, would cross a hole in the part. The cut joins the
    // arc's ends instead.
    let hole = [(20.9, 9.9), (21.1, 9.9), (21.1, 10.1), (20.9, 10.1)];
    let with_hole = part(&tab_outline(12.0), &[&hole]);
    let (ring, cuts) = cut_of(&TAB, &with_hole);
    assert_eq!(ring, geom::closed(&TAB[..4].iter().map(|&(x, y)| p(x, y)).collect::<Vec<_>>()));
    assert_eq!(cuts, [[p(20.0, 11.5), p(21.0, 8.5)]]);
    // Without the hole, the cut is square across from the arc's end farther along: the chord at x = 21.
    let (ring, cuts) = cut_of(&TAB, &square_with_tab(12.0));
    assert_eq!(cuts, [[p(21.0, 11.5), p(21.0, 8.5)]]);
    assert!(ring.iter().all(|q| q.x() >= 21.0), "{ring:?}");
    // With a second hole on the chord between the arc's ends too, no cut is left: the arc stays.
    let between = [(20.45, 9.95), (20.55, 9.95), (20.55, 10.05), (20.45, 10.05)];
    let with_holes = part(&tab_outline(12.0), &[&hole, &between]);
    let (ring, cuts) = cut_of(&TAB, &with_holes);
    assert_eq!(ring, geom::closed(&TAB.iter().map(|&(x, y)| p(x, y)).collect::<Vec<_>>()));
    assert!(cuts.is_empty(), "{cuts:?}");
}

#[test]
fn req_auto_019_a_piece_with_no_more_than_the_arcs_ends_on_the_outline_is_not_cut() {
    // Only the tab's end corners lie on the part's outline, and the rest of its ring is an arc: no ring is left
    // to close with a chord, and the piece stays as it is.
    let ring = [(32.0, 8.5), (32.0, 11.5), (30.0, 11.0), (25.0, 10.0), (30.0, 9.0)];
    let (outline, cuts) = cut_of(&ring, &square_with_tab(12.0));
    assert_eq!(outline, geom::closed(&ring.iter().map(|&(x, y)| p(x, y)).collect::<Vec<_>>()));
    assert!(cuts.is_empty(), "{cuts:?}");
}

#[test]
fn req_auto_019_an_arm_drawn_the_other_way_round_is_cut_square_across_too() {
    // The L of the arm's test mirrored, its arm up its right side: the cut runs from the arm's inner corner
    // at the other end of the opening's arc.
    let l = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 40.0), (15.0, 40.0), (15.0, 20.0), (0.0, 20.0)], &[]);
    let split = split_of(&l);
    let [arm]: [Polygon; 1] = pieces(&split).try_into().unwrap();
    let [x0, y0, x1, y1] = bounds(&arm);
    assert!(x0 == 15.0 && x1 == 20.0 && (y0 - 20.0).abs() < 0.01 && y1 == 40.0, "{arm:?}");
    let mouth: Vec<Point> = arm.outline.iter().copied().filter(|q| q.y() < 20.5).collect();
    assert!(mouth.len() >= 2 && mouth.iter().all(|q| (q.y() - y0).abs() < 1e-9), "{mouth:?}");
    assert!((area_of(&split.fill) + arm.area() - l.area()).abs() < 1e-6);
}

/// What [`square`] closes the ring `points` with, kept from the point after a run round to the one before it.
fn squared(points: &[(f64, f64)]) -> Option<Vec<Point>> {
    let kept: Vec<Point> = points.iter().map(|&(x, y)| p(x, y)).collect();
    square(&kept, kept.len() - 1)
}

#[test]
fn req_auto_019_a_cut_is_square_across_only_from_an_end_more_than_a_hair_farther_along() {
    // A tab that runs right, the arc's end on its bottom side `bottom` along it, and the one on its top side
    // `top`. The way runs along the tab, and how far along it each end lies is measured in millimetres: within
    // ON, the ends lie as far along, and no chord is square across.
    let right = |bottom: f64, top: f64| squared(&[(bottom, 8.5), (32.0, 8.5), (32.0, 11.5), (top, 11.5)]);
    for (bottom, top) in [(21.0 + 0.8 * ON, 21.0), (21.0 + ON, 21.0), (21.0, 21.0 + 0.8 * ON), (21.0, 21.0 + ON)] {
        assert_eq!(right(bottom, top), None, "{bottom} {top}");
    }
    // Farther along by more, from the end farther along square across: from the bottom end, across to the top
    // side, and from the top end, across to the bottom side.
    let [.., foot] = right(21.0 + 1.2 * ON, 21.0).unwrap()[..] else { panic!() };
    assert!((foot.x() - 21.0 - 1.2 * ON).abs() < 1e-12 && foot.y() == 11.5, "{foot:?}");
    let [foot, ..] = right(21.0, 21.0 + 1.2 * ON).unwrap()[..] else { panic!() };
    assert!((foot.x() - 21.0 - 1.2 * ON).abs() < 1e-12 && foot.y() == 8.5, "{foot:?}");
    // The same of a tab that runs up.
    let up = |left: f64, right: f64| squared(&[(8.5, left), (8.5, 32.0), (11.5, 32.0), (11.5, right)]);
    assert_eq!(up(21.0 + 0.8 * ON, 21.0), None);
    assert!(up(21.0 + 1.2 * ON, 21.0).is_some());
}

#[test]
fn req_auto_019_a_cut_square_across_reaches_the_other_side_no_farther_than_twice_the_arcs_chord() {
    // The arc's ends half a millimetre apart, the bottom one 0.4 mm farther along. The top side runs on along
    // the tab, and the cut square across meets it at its second edge, which a ring keeps from there on.
    let near = squared(&[(0.4, 0.0), (5.0, 0.0), (5.0, 0.3), (0.2, 0.3), (0.0, 0.3)]).unwrap();
    assert_eq!(near.len(), 4, "{near:?}");
    assert_eq!(near[..3], [p(0.4, 0.0), p(5.0, 0.0), p(5.0, 0.3)]);
    assert!((near[3].x() - 0.4).abs() < 1e-12 && near[3].y() == 0.3, "{near:?}");
    // A top side that turns away first, up 1.2 mm, comes back over the bottom end's way only 1.5 mm from it,
    // farther than twice the arc's chord: no chord is square across.
    assert_eq!(squared(&[(0.4, 0.0), (5.0, 0.0), (5.0, 1.5), (0.3, 1.5), (0.3, 0.3), (0.0, 0.3)]), None);
}

#[test]
fn req_auto_019_a_point_of_a_piece_no_farther_than_on_from_the_outline_lies_on_it() {
    // The tab's test moved so that its bottom side lies along y = 0, where how far a point lies from it is
    // its y to the last bit. A point of the piece ON above that side lies on the outline: the arc is the only
    // run cut, square across the tab from its end on the bottom side, give or take the hair the point tilts
    // the bottom side's way by, and the point stays in the ring.
    let tabbed = part(&[(-20.0, -8.5), (0.0, -8.5), (0.0, 0.0), (12.0, 0.0), (12.0, 3.0), (0.0, 3.0), (0.0, 11.5), (-20.0, 11.5)], &[]);
    let ring = [(1.0, 0.0), (6.0, ON), (12.0, 0.0), (12.0, 3.0), (0.0, 3.0), (-0.6, 2.1), (-0.4, 0.7), (0.4, 0.2)];
    let (outline, cuts) = cut_of(&ring, &tabbed);
    let [[top, bottom]] = cuts[..] else { panic!("{cuts:?}") };
    assert!(top.distance(p(1.0, 3.0)) < 1e-6 && bottom == p(1.0, 0.0), "{cuts:?}");
    assert!(outline.contains(&p(6.0, ON)), "{outline:?}");
}

#[test]
fn req_auto_020_a_narrow_part_reaching_exactly_the_smallest_area_past_the_broad_parts_is_cut_from_the_fill() {
    // The tab of a square with a tab 6 mm long, which reaches beyond twice the opening's radius by an area
    // found as `split` finds it: with that area as the smallest, the tab is a narrow part, since only a
    // smaller area is a speck; with anything larger, it stays in the fill.
    let whole = square_with_tab(6.0);
    let mut meter = Budget::DEFAULT.meter();
    let defaults = AutoParams::defaults();
    let radius = narrow::half(defaults.auto_max_satin_width_mm);
    let buffered = |parts: &[Polygon], by: f64, meter: &mut Meter| ops::buffered(parts, by, meter).unwrap().unwrap();
    let shrunk = buffered(std::slice::from_ref(&whole), -radius, &mut meter);
    let broad = ops::intersection(&buffered(&shrunk, radius, &mut meter), std::slice::from_ref(&whole), &mut meter).unwrap().unwrap();
    let rest = ops::difference(std::slice::from_ref(&whole), &broad, &mut meter).unwrap().unwrap();
    let tab = rest.iter().max_by(|a, b| a.area().total_cmp(&b.area())).unwrap();
    let reach = buffered(&shrunk, 2.0 * radius, &mut meter);
    let beyond = area_of(&ops::difference(std::slice::from_ref(tab), &reach, &mut meter).unwrap().unwrap());
    assert!(beyond > 1.0 && beyond < tab.area(), "{beyond} of {}", tab.area());
    let split_with = |smallest: f64| {
        let rules = AutoParams { auto_min_area_mm2: smallest, ..defaults.clone() };
        split(&whole, &rules, &mut Budget::DEFAULT.meter()).unwrap().unwrap()
    };
    assert_eq!(split_with(beyond).narrow.len(), 1);
    assert!(split_with(beyond.next_up()).narrow.is_empty());
}

#[test]
fn req_auto_019_a_run_right_after_the_rings_first_point_is_found() {
    // Points 0 and 2 lie on the outline and point 1 off it: the run is the one point between them.
    let ring = [p(0.0, 0.0), p(1.0, 1.0), p(2.0, 0.0)];
    assert_eq!(run(&[false, true, false], &ring, &[]), Some((0, 2)));
    // After a point that stays, it isn't cut, and with no other point on the outline there is none.
    assert_eq!(run(&[false, true, false], &ring, &[p(0.0, 0.0)]), None);
    assert_eq!(run(&[false, true, true], &ring, &[]), None);
}

#[test]
fn req_auto_019_an_edge_at_the_level_is_no_crossing_of_it() {
    // Along x: the first edge lies at the level 1, from end to end, and the second crosses it at (1, 0).
    let edges = [(p(1.0, 0.0), p(1.0, 5.0)), (p(0.0, 0.0), p(2.0, 0.0))];
    assert_eq!(crossing(1.0, &|q: Point| q.x(), edges.into_iter()), Some((1, p(1.0, 0.0))));
}

#[test]
fn req_auto_019_a_chord_along_the_outline_from_a_corner_of_the_part_crosses_nothing() {
    // A part whose bottom side runs from its corner (0, 0) to (20, 0), and a ring that the chord from that
    // corner to (10, 0) closes, along the side: the side shares the chord's end, and runs along it without
    // crossing it.
    let part = part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 10.0), (0.0, 10.0)], &[]);
    let mut meter = Budget::DEFAULT.meter();
    let along = [p(10.0, 0.0), p(10.0, 3.0), p(0.0, 3.0), p(0.0, 0.0)];
    assert!(crosses_nothing(&along, &part, &mut meter).unwrap());
    // A ring's edge across the chord crosses it.
    let across = [p(10.0, 0.0), p(10.0, 3.0), p(5.0, -2.0), p(0.0, 3.0), p(0.0, 0.0)];
    assert!(!crosses_nothing(&across, &part, &mut meter).unwrap());
}
