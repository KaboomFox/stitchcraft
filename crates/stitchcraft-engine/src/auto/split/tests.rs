//! The broad and narrow parts of a part wider than a satin column in places (`REQ-AUTO-019`): a tab, an arm
//! and a bar are narrow parts cut square where they meet the broad parts, and corners and stubs stay in the
//! fill.

use stitchcraft_core::math::sin_cos;
use stitchcraft_core::{Budget, Point};

use super::*;
use crate::normalize::fixture::part;
use crate::normalize::region::ops::{area_of, union};

/// `part` split, with the default budget.
fn split_of(part: &Polygon) -> Split {
    split(part, &mut Budget::DEFAULT.meter()).unwrap().unwrap()
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

/// A square 20 mm a side with a tab 3 mm wide and `length` long on its right side, halfway up.
fn square_with_tab(length: f64) -> Polygon {
    part(&[(0.0, 0.0), (20.0, 0.0), (20.0, 8.5), (20.0 + length, 8.5), (20.0 + length, 11.5), (20.0, 11.5), (20.0, 20.0), (0.0, 20.0)], &[])
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
