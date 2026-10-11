//! Exact geometric predicates: which side of a line a point lies on, decided without rounding error.
//!
//! The sign of `(b − a) × (c − a)` says whether `c` lies left of the line from `a` to `b`, right of it, or
//! on it. In floating point the products round, and for points nearly in line the computed sign can be
//! wrong. An offset curve or an overlay built on a wrong sign can tear: a corner is turned the wrong way,
//! or two crossing lines are taken to miss. Geometry libraries such as GEOS decide these signs exactly, or
//! nearly so, and StitchCraft follows the curves they build.
//!
//! The usual case is decided in plain floating point, when the result is farther from zero than its
//! largest possible rounding error. Otherwise the determinant is computed exactly, as a sum of floats
//! whose terms do not overlap, by the error-free transformations J. R. Shewchuk describes in *Adaptive
//! Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates* (1997). Only `+`, `−` and
//! `×` are used, so the answer is the same on every platform. Inputs are the finite coordinates of
//! [`Point`], far from overflow.

use crate::units::Point;

/// Where a point lies relative to a directed line.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// To the left: the turn from the line to the point is counter-clockwise in y-up axes.
    Left,
    /// On the line.
    On,
    /// To the right: clockwise.
    Right,
}

/// Shewchuk's first error bound on the plain determinant, as a multiple of `|left| + |right|`: (3 + 16ε)ε,
/// with ε = 2^-53, the largest relative error of one rounding.
const ERROR_BOUND: f64 = (3.0 + 16.0 * EPSILON) * EPSILON;

/// 2^-53, half the gap between 1 and the next `f64`.
const EPSILON: f64 = f64::EPSILON / 2.0;

/// Which side of the line from `a` to `b` the point `c` lies on, decided exactly.
pub fn side(a: Point, b: Point, c: Point) -> Side {
    let (abx, aby) = (b.x() - a.x(), b.y() - a.y());
    let (acx, acy) = (c.x() - a.x(), c.y() - a.y());
    let (left, right) = (abx * acy, aby * acx);
    let det = left - right;
    let bound = ERROR_BOUND * (left.abs() + right.abs());
    let sign = if det.abs() <= bound { exact_determinant(a, b, c) } else { det };
    if sign > 0.0 {
        Side::Left
    } else if sign < 0.0 {
        Side::Right
    } else {
        Side::On
    }
}

/// The determinant `(b − a) × (c − a)` as its exact sum's largest term, which has the sum's sign.
fn exact_determinant(a: Point, b: Point, c: Point) -> f64 {
    let ab = [two_diff(b.x(), a.x()), two_diff(b.y(), a.y())];
    let ac = [two_diff(c.x(), a.x()), two_diff(c.y(), a.y())];
    let mut sum = Vec::with_capacity(16);
    // (abx)(acy) − (aby)(acx), each factor the exact sum of 2 floats: 8 exact products a side.
    for (x, y, sign) in [(ab[0], ac[1], 1.0), (ab[1], ac[0], -1.0)] {
        for p in [x.0, x.1] {
            for q in [y.0, y.1] {
                let (high, low) = two_product(p, q);
                grow(&mut sum, sign * low);
                grow(&mut sum, sign * high);
            }
        }
    }
    sum.iter().rev().copied().find(|term| *term != 0.0).unwrap_or(0.0)
}

/// Adds `value` to `sum`, an expansion whose terms do not overlap and grow in magnitude; it stays one.
fn grow(sum: &mut Vec<f64>, value: f64) {
    let mut carry = value;
    for term in sum.iter_mut() {
        let (high, low) = two_sum(carry, *term);
        *term = low;
        carry = high;
    }
    sum.push(carry);
}

/// `a + b` as a float and its exact rounding error.
fn two_sum(a: f64, b: f64) -> (f64, f64) {
    let x = a + b;
    let b_virtual = x - a;
    let a_virtual = x - b_virtual;
    (x, (a - a_virtual) + (b - b_virtual))
}

/// `a − b` as a float and its exact rounding error.
fn two_diff(a: f64, b: f64) -> (f64, f64) {
    let x = a - b;
    let b_virtual = a - x;
    let a_virtual = x + b_virtual;
    (x, (a - a_virtual) + (b_virtual - b))
}

/// `a × b` as a float and its exact rounding error, by Dekker's splitting into 26-bit halves.
fn two_product(a: f64, b: f64) -> (f64, f64) {
    let x = a * b;
    let (a_high, a_low) = split(a);
    let (b_high, b_low) = split(b);
    let error = x - a_high * b_high - a_low * b_high - a_high * b_low;
    (x, a_low * b_low - error)
}

/// `a` as the sum of 2 floats of at most 26 significant bits each.
fn split(a: f64) -> (f64, f64) {
    const SPLITTER: f64 = 134_217_729.0; // 2^27 + 1
    let c = SPLITTER * a;
    let high = c - (c - a);
    (high, a - high)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The sign of `(b − a) × (c − a)` for integer coordinates, exactly, in 128-bit integers.
    fn exact(a: (i64, i64), b: (i64, i64), c: (i64, i64)) -> Side {
        let det = i128::from(b.0 - a.0) * i128::from(c.1 - a.1) - i128::from(b.1 - a.1) * i128::from(c.0 - a.0);
        match det.signum() {
            1 => Side::Left,
            -1 => Side::Right,
            _ => Side::On,
        }
    }

    #[test]
    fn plain_cases() {
        assert_eq!(side(p(0.0, 0.0), p(1.0, 0.0), p(0.5, 1.0)), Side::Left);
        assert_eq!(side(p(0.0, 0.0), p(1.0, 0.0), p(0.5, -1.0)), Side::Right);
        assert_eq!(side(p(0.0, 0.0), p(1.0, 1.0), p(3.0, 3.0)), Side::On);
        assert_eq!(side(p(2.0, 2.0), p(2.0, 2.0), p(3.0, 5.0)), Side::On, "a line of no length has no sides");
    }

    #[test]
    fn nearly_in_line_points_are_decided_exactly() {
        // Points one unit of the last place off a line, where the plain products round to the same value.
        let (a, b) = (p(0.5, 0.5), p(12.0, 12.0));
        let on = p(24.0, 24.0);
        let above = p(24.0, f64::from_bits(24.0_f64.to_bits() + 1));
        let below = p(24.0, f64::from_bits(24.0_f64.to_bits() - 1));
        assert_eq!((side(a, b, on), side(a, b, above), side(a, b, below)), (Side::On, Side::Left, Side::Right));
        // The classic failure: the plain determinant of these is 0, or of the wrong sign.
        let (a, b, c) = (p(0.1, 0.1), p(0.3, 0.3), p(0.5, 0.5));
        let plain = (b.x() - a.x()) * (c.y() - a.y()) - (b.y() - a.y()) * (c.x() - a.x());
        let truth = side(a, b, c);
        assert!(truth != Side::On || plain == 0.0, "{plain}");
    }

    #[test]
    fn points_a_few_units_of_the_last_place_off_a_line_are_decided_exactly() {
        // The classroom example of Kettner, Mehlhorn, Pion, Schirra and Yap (2004): p = (0.5 + iε, 0.5 + jε),
        // with ε = 2^-53, against q = (12, 12) and r = (24, 24). The coordinates' differences round, and the
        // plain determinant is 0 or of the wrong sign for many of these points. p lies left of the line from
        // q to r when j > i, on it when j = i, and right of it when j < i. Scaling by a power of 2 changes no
        // side, but changes the sizes the error bound is measured against.
        let epsilon = f64::EPSILON / 2.0;
        let mut wrong_plain = 0;
        for scale in [1.0 / 1_099_511_627_776.0, 1.0, 1_099_511_627_776.0] {
            let at = |x: f64, y: f64| p(x * scale, y * scale);
            let (q, r) = (at(12.0, 12.0), at(24.0, 24.0));
            for i in 0..64 {
                for j in 0..64 {
                    let point = at(0.5 + f64::from(i) * epsilon, 0.5 + f64::from(j) * epsilon);
                    let (truth, reversed) = match j.cmp(&i) {
                        std::cmp::Ordering::Greater => (Side::Left, Side::Right),
                        std::cmp::Ordering::Equal => (Side::On, Side::On),
                        std::cmp::Ordering::Less => (Side::Right, Side::Left),
                    };
                    for (a, b, c) in [(point, q, r), (q, r, point), (r, point, q)] {
                        assert_eq!(side(a, b, c), truth, "{i} {j} at {scale}");
                    }
                    assert_eq!(side(q, point, r), reversed, "{i} {j} at {scale}");
                    let plain = (q.x() - point.x()) * (r.y() - point.y()) - (q.y() - point.y()) * (r.x() - point.x());
                    let plain_side = if plain > 0.0 {
                        Side::Left
                    } else if plain < 0.0 {
                        Side::Right
                    } else {
                        Side::On
                    };
                    wrong_plain += usize::from(plain_side != truth);
                }
            }
        }
        // The plain determinant is wrong at 2,164 of the 4,096 points of each scale, so the test reaches the
        // cases the exact sum is for.
        assert_eq!(wrong_plain, 3 * 2_164);
    }

    #[test]
    fn the_error_bound_is_shewchuks() {
        // (3 + 16ε)ε multiplied out; both are exact in f64. A looser bound only sends more cases to the exact
        // sum, which gives the same signs, so no other test notices it.
        let epsilon = f64::EPSILON / 2.0;
        assert_eq!(ERROR_BOUND, 3.0 * epsilon + 16.0 * epsilon * epsilon);
    }

    #[test]
    fn random_integer_points_agree_with_exact_integers() {
        // Integers up to 2^52 are exact floats; their differences and products fit 128-bit integers.
        let mut rng = SplitMix64::new(20_261_010);
        let mut big = || i64::try_from(rng.next_u64() >> 12).unwrap() - (1 << 51);
        for case in 0..20_000 {
            let a = (big(), big());
            let b = (big(), big());
            // Every other case lies nearly on the line: a point between a and b, nudged by a few units.
            let c = if case % 2 == 0 {
                (big(), big())
            } else {
                let t = i128::from(big().rem_euclid(1 << 20));
                let along = |s: i64, e: i64| i64::try_from(i128::from(s) + (i128::from(e) - i128::from(s)) * t / (1 << 20)).unwrap();
                (along(a.0, b.0) + case % 3 - 1, along(a.1, b.1))
            };
            let f = |q: (i64, i64)| p(q.0 as f64, q.1 as f64);
            assert_eq!(side(f(a), f(b), f(c)), exact(a, b, c), "{a:?} {b:?} {c:?}");
        }
    }
}
