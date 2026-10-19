//! Unit tests of the zigzag stroke's steps; the conformance cases are in `tests/zigzag.rs`.

use super::*;

fn p(x: f64, y: f64) -> Point {
    Point::new(x, y).unwrap()
}

#[test]
fn a_part_takes_an_even_count_of_half_spacings_of_at_least_2() {
    let counts = [halves(10.0, 0.2), halves(12.625, 0.25), halves(13.125, 0.25), halves(2.75, 0.25), halves(0.1, 0.25), halves(0.0, 0.25)];
    assert_eq!(counts, [50, 50, 52, 12, 2, 2]);
    // Far more than any budget allows: as many as a count holds, which the budget refuses.
    assert_eq!(halves(1e300, 1e-300), u32::MAX);
}

#[test]
fn a_direction_lies_halfway_between_its_two_lines() {
    let (a, b, c) = (p(0.0, 0.0), p(2.0, 0.0), p(2.0, 3.0));
    assert_eq!(direction(None, a, Some(b)), (2.0, 0.0));
    assert_eq!(direction(Some(b), c, None), (0.0, 3.0));
    assert_eq!(direction(Some(a), b, Some(c)), (0.5, 0.5));
    // A line with no length gives way to the other, and with neither there is no direction.
    assert_eq!(direction(Some(a), a, Some(b)), (2.0, 0.0));
    assert_eq!(direction(Some(a), b, Some(b)), (2.0, 0.0));
    assert_eq!(direction(Some(a), a, Some(a)), (0.0, 0.0));
    assert_eq!(direction(None, a, None), (0.0, 0.0));
}

#[test]
fn only_a_stroke_of_one_moving_part_with_its_ends_together_is_closed() {
    let ring = [p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.01)];
    let closed = |parts: &[&[Point]]| is_closed(parts.iter().copied());
    assert!(closed(&[&ring]));
    assert!(closed(&[&ring, &[p(5.0, 5.0)]]), "a part that does not move does not count");
    assert!(!closed(&[&ring, &ring]));
    assert!(!closed(&[&[p(0.0, 0.0), p(1.0, 0.0), p(0.0, 0.02)]]));
    assert!(!closed(&[]));
}

#[test]
fn a_point_that_would_leave_the_numbers_stays_where_it_is() {
    let sides = Sides { left: 0.0, right: f64::MAX, cos: 1.0, sin: 0.0 };
    let at = p(-f64::MAX, 0.0);
    assert_eq!(across(at, (0.0, 1.0), false, sides), at);
    assert_eq!(across(p(1.0, 2.0), (0.0, 0.0), false, sides), p(1.0, 2.0), "no direction");
}

#[test]
fn a_zigzag_names_some_stitches_shorter_than_the_shortest_stitch_and_how_to_lengthen_them() {
    let meter = &mut stitchcraft_core::Budget::DEFAULT.meter();
    let mut named = |points: &[Point], spacing: f64| {
        let mut short = Short::default();
        short.count(points, 0.3, meter).unwrap();
        short.diagnostic(spacing, 0.3).map(|d| (d.to_string(), d.fix))
    };
    // Stitches of 1, 0.2 and 1 mm, at a spacing of 1 mm, whose half is already longer than 0.3 mm: a
    // straight zigzag of any width sews, so the hint names no width.
    let (said, fix) = named(&[p(0.0, 0.0), p(1.0, 0.0), p(1.2, 0.0), p(2.2, 0.0)], 1.0).unwrap();
    assert_eq!(
        said,
        "warning SC-W0405: 1 of the zigzag's 3 stitches is shorter than the shortest stitch (0.3 mm), at 0.2 mm, so finishing leaves out a needle point of it, and the zigzag sews narrower there."
    );
    assert_eq!(fix, Some(Fix::Hint("Make the zigzag wider, or space it out.".to_string())));
    // 2 short ones of 3, at the default spacing.
    let (said, fix) = named(&[p(0.0, 0.0), p(0.25, 0.0), p(0.45, 0.0), p(2.0, 0.0)], 0.4).unwrap();
    assert_eq!(
        said,
        "warning SC-W0405: 2 of the zigzag's 3 stitches are shorter than the shortest stitch (0.3 mm), the shortest 0.2 mm long, so finishing leaves out a needle point of each, and the zigzag sews narrower there."
    );
    assert_eq!(fix, Some(Fix::Hint("Make the zigzag at least 0.23 mm wide, or space it out.".to_string())));
    // None short, none named.
    assert_eq!(named(&[p(0.0, 0.0), p(1.0, 0.0)], 0.4), None);
    // A stitch exactly as long as the shortest stitch, or a rounding error shorter, is as long as finishing
    // keeps, and is not named.
    assert_eq!(named(&[p(0.0, 0.0), p(0.3, 0.0), p(0.6 - 1e-12, 0.0)], 0.4), None);
    // Where half the spacing is exactly the shortest stitch, a straight zigzag of any width sews too, and
    // the hint names no width.
    let (_, fix) = named(&[p(0.0, 0.0), p(0.2, 0.0), p(1.2, 0.0)], 0.6).unwrap();
    assert_eq!(fix, Some(Fix::Hint("Make the zigzag wider, or space it out.".to_string())));
}
