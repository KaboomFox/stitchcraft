//! Unit tests of the zigzag stroke's steps; the conformance cases are in `tests/zigzag.rs`.

use stitchcraft_core::Budget;
use stitchcraft_params::ParamSet;

use super::*;
use crate::design::{Segment, Subpath};

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
fn a_stroke_without_a_spacing_is_not_sewn_and_says_so() {
    // The registry reads 1 spacing or more, so only a bug in StitchCraft can leave the list empty.
    let set = ParamSet::new();
    let mut params = ZigzagStrokeParams::from_set(&set).unwrap().params;
    params.zigzag_spacing_mm.clear();
    let passes = RepeatParams::from_set(&set).unwrap().params;
    let path = Path { subpaths: vec![Subpath { start: p(0.0, 0.0), segments: vec![Segment::Line(p(5.0, 0.0))], closed: false }] };
    let mm = |value| Mm::new(value).unwrap();
    let stitched = zigzag_stroke(&path, mm(2.0), &params, &passes, mm(0.3), &mut Budget::DEFAULT.meter()).unwrap();
    assert!(stitched.runs.is_empty());
    assert_eq!(stitched.warnings.iter().map(|d| d.code).collect::<Vec<_>>(), [Code::InternalCheckFailed]);
}

#[test]
fn a_point_that_would_leave_the_numbers_stays_where_it_is() {
    let sides = Sides { left: 0.0, right: f64::MAX, cos: 1.0, sin: 0.0 };
    let at = p(-f64::MAX, 0.0);
    assert_eq!(across(at, (0.0, 1.0), false, sides), at);
    assert_eq!(across(p(1.0, 2.0), (0.0, 0.0), false, sides), p(1.0, 2.0), "no direction");
}
