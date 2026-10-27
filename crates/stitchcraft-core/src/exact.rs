//! Exact geometric predicates: which side of a line a point lies on, and whether it lies inside the circle
//! through 3 others, decided without rounding error.
//!
//! The sign of `(b − a) × (c − a)` says whether `c` lies left of the line from `a` to `b`, right of it, or
//! on it. In floating point the products round, and for points nearly in line the computed sign can be
//! wrong. An offset curve or an overlay built on a wrong sign can tear: a corner is turned the wrong way,
//! or two crossing lines are taken to miss. Geometry libraries such as GEOS decide these signs exactly, or
//! nearly so, and StitchCraft follows the curves they build.
//!
//! A Delaunay triangulation asks the second question of 4 points: whether the fourth lies inside the
//! circle through the other 3. It flips the edge between 2 triangles when it does, and with a wrong answer
//! for 4 points nearly on one circle it can flip the same edges back and forth forever ([`incircle`]).
//!
//! The usual case is decided in plain floating point, when the result is farther from zero than its
//! largest possible rounding error. Otherwise the determinant is computed exactly, as a sum of floats
//! whose terms do not overlap, by the error-free transformations J. R. Shewchuk describes in *Adaptive
//! Precision Floating-Point Arithmetic and Fast Robust Geometric Predicates* (1997). Only `+`, `−` and
//! `×` are used, so the answer is the same on every platform. Inputs are the finite coordinates of
//! [`Point`], far from overflow and from underflow, as the error-free products need. The exact sum costs
//! little when the coordinates' differences are exact floats, as they are for points on the grid a fill's
//! region is built on: each term is then a single product.

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
    let sign = if det.abs() <= bound {
        if left == 0.0 && right == 0.0 {
            // Far from underflow, a product is 0 only where a factor is, and a difference of 2 floats is 0
            // only where they are equal: the determinant is exactly 0. Points level or upright with each
            // other come here often, and the exact sum would only confirm it.
            0.0
        } else {
            exact_determinant(a, b, c)
        }
    } else {
        det
    };
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
    largest(&sum)
}

/// Where a point lies relative to a circle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Circle {
    /// Inside it.
    Inside,
    /// On it.
    On,
    /// Outside it.
    Outside,
}

/// Shewchuk's first error bound on the plain in-circle determinant, as a multiple of its permanent:
/// (10 + 96ε)ε.
const INCIRCLE_BOUND: f64 = (10.0 + 96.0 * EPSILON) * EPSILON;

/// Where `d` lies relative to the circle through `a`, `b` and `c`, decided exactly. The 3 points turn
/// counter-clockwise in y-up axes, as a triangle's corners are listed; for points that turn clockwise the
/// answer is the other way round. 3 points in line bound no circle, and the answer then says nothing of one.
///
/// It is the sign of the 3 × 3 determinant whose rows are each point's offset from `d` and that offset's
/// squared length: positive inside the circle.
pub fn incircle(a: Point, b: Point, c: Point, d: Point) -> Circle {
    let (adx, ady) = (a.x() - d.x(), a.y() - d.y());
    let (bdx, bdy) = (b.x() - d.x(), b.y() - d.y());
    let (cdx, cdy) = (c.x() - d.x(), c.y() - d.y());
    let (bdxcdy, cdxbdy) = (bdx * cdy, cdx * bdy);
    let (cdxady, adxcdy) = (cdx * ady, adx * cdy);
    let (adxbdy, bdxady) = (adx * bdy, bdx * ady);
    let (alift, blift, clift) = (adx * adx + ady * ady, bdx * bdx + bdy * bdy, cdx * cdx + cdy * cdy);
    let det = alift * (bdxcdy - cdxbdy) + blift * (cdxady - adxcdy) + clift * (adxbdy - bdxady);
    let permanent = (bdxcdy.abs() + cdxbdy.abs()) * alift + (cdxady.abs() + adxcdy.abs()) * blift + (adxbdy.abs() + bdxady.abs()) * clift;
    let sign = if det.abs() > INCIRCLE_BOUND * permanent { det } else { exact_incircle(a, b, c, d) };
    if sign > 0.0 {
        Circle::Inside
    } else if sign < 0.0 {
        Circle::Outside
    } else {
        Circle::On
    }
}

/// The in-circle determinant of [`incircle`] as its exact sum's largest term, which has the sum's sign. Each
/// offset is the exact sum of 2 floats, a single float when the subtraction is exact.
fn exact_incircle(a: Point, b: Point, c: Point, d: Point) -> f64 {
    let offset = |p: f64, q: f64| {
        let (x, error) = two_diff(p, q);
        let mut terms = vec![error, x];
        terms.retain(|term| *term != 0.0);
        terms
    };
    let (adx, ady) = (offset(a.x(), d.x()), offset(a.y(), d.y()));
    let (bdx, bdy) = (offset(b.x(), d.x()), offset(b.y(), d.y()));
    let (cdx, cdy) = (offset(c.x(), d.x()), offset(c.y(), d.y()));
    // The cross product p × q of 2 offsets, and an offset's squared length.
    let cross = |px: &[f64], py: &[f64], qx: &[f64], qy: &[f64]| difference(&product(px, qy), &product(py, qx));
    let lift = |x: &[f64], y: &[f64]| sum(&product(x, x), &product(y, y));
    let terms = [
        product(&lift(&adx, &ady), &cross(&bdx, &bdy, &cdx, &cdy)),
        product(&lift(&bdx, &bdy), &cross(&cdx, &cdy, &adx, &ady)),
        product(&lift(&cdx, &cdy), &cross(&adx, &ady, &bdx, &bdy)),
    ];
    largest(&terms.iter().fold(Vec::new(), |total, term| sum(&total, term)))
}

/// The exact product of the expansions `e` and `f`.
fn product(e: &[f64], f: &[f64]) -> Vec<f64> {
    let mut out = Vec::with_capacity(2 * e.len() * f.len());
    for &p in e {
        for &q in f {
            let (high, low) = two_product(p, q);
            grow(&mut out, low);
            grow(&mut out, high);
        }
    }
    out
}

/// The exact sum of the expansions `e` and `f`.
fn sum(e: &[f64], f: &[f64]) -> Vec<f64> {
    let mut out = e.to_vec();
    for &term in f {
        grow(&mut out, term);
    }
    out
}

/// The exact difference `e − f` of 2 expansions.
fn difference(e: &[f64], f: &[f64]) -> Vec<f64> {
    let negated: Vec<f64> = f.iter().map(|term| -term).collect();
    sum(e, &negated)
}

/// The largest term of the expansion `sum`, which has the sign of the whole sum: its terms do not overlap.
/// 0 for an empty sum.
fn largest(sum: &[f64]) -> f64 {
    sum.iter().rev().copied().find(|term| *term != 0.0).unwrap_or(0.0)
}

/// Adds `value` to `sum`, an expansion whose terms do not overlap and grow in magnitude; it stays one, and
/// its terms that are 0 are dropped, which keeps the sums of many products short.
fn grow(sum: &mut Vec<f64>, value: f64) {
    let mut carry = value;
    for term in sum.iter_mut() {
        let (high, low) = two_sum(carry, *term);
        *term = low;
        carry = high;
    }
    sum.push(carry);
    sum.retain(|term| *term != 0.0);
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
        // Level and upright points: a factor of each product is 0.
        assert_eq!(side(p(1.0, 2.0), p(5.0, 2.0), p(9.0, 2.0)), Side::On);
        assert_eq!(side(p(3.0, 1.0), p(3.0, 4.0), p(3.0, -6.0)), Side::On);
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

    /// An integer as limbs of 32 bits, least first: the sum of each limb times 2^(32k). Normalized, every
    /// limb lies in 0 to 2^32 but the last, which is 0 or −1 and gives the sign.
    #[derive(Clone, Debug)]
    struct Big(Vec<i128>);

    impl Big {
        /// The float `v` times 2^-`unit`, which must be a whole number: `unit` is at most `v`'s lowest bit.
        fn of(v: f64, unit: i32) -> Big {
            if v == 0.0 {
                return Big(vec![0]).normalized();
            }
            let (mantissa, exponent) = dyadic(v);
            let shift = u32::try_from(exponent - unit).unwrap();
            let mut limbs = vec![0; (shift / 32) as usize];
            limbs.push(mantissa << (shift % 32));
            Big(limbs).normalized()
        }

        fn normalized(mut self) -> Big {
            let mut carry = 0_i128;
            for limb in &mut self.0 {
                let v = *limb + carry;
                (*limb, carry) = (v.rem_euclid(1 << 32), v.div_euclid(1 << 32));
            }
            while carry != 0 && carry != -1 {
                self.0.push(carry.rem_euclid(1 << 32));
                carry = carry.div_euclid(1 << 32);
            }
            self.0.push(carry);
            self
        }

        fn plus(&self, other: &Big, sign: i128) -> Big {
            let mut limbs = vec![0; self.0.len().max(other.0.len())];
            for (k, limb) in limbs.iter_mut().enumerate() {
                *limb = self.0.get(k).copied().unwrap_or(0) + sign * other.0.get(k).copied().unwrap_or(0);
            }
            Big(limbs).normalized()
        }

        fn times(&self, other: &Big) -> Big {
            let mut limbs = vec![0; self.0.len() + other.0.len()];
            for (i, a) in self.0.iter().enumerate() {
                for (j, b) in other.0.iter().enumerate() {
                    limbs[i + j] += a * b;
                }
            }
            Big(limbs).normalized()
        }

        fn sign(&self) -> i32 {
            if self.0.last() == Some(&-1) { -1 } else { i32::from(self.0.iter().any(|&limb| limb != 0)) }
        }
    }

    /// `v` as `m`·2^`e`, `m` an odd whole number, or 0 for 0: `e` is the exponent of its lowest bit.
    fn dyadic(v: f64) -> (i128, i32) {
        let bits = v.to_bits();
        let exponent = i32::try_from((bits >> 52) & 0x7ff).unwrap();
        let fraction = i128::from(bits & ((1 << 52) - 1));
        let (mut mantissa, mut exponent) = if exponent == 0 { (fraction, -1074) } else { (fraction | (1 << 52), exponent - 1075) };
        while mantissa != 0 && mantissa % 2 == 0 {
            (mantissa, exponent) = (mantissa / 2, exponent + 1);
        }
        (if bits >> 63 == 1 { -mantissa } else { mantissa }, exponent)
    }

    /// The in-circle determinant's sign, worked out in whole numbers: every coordinate a multiple of the
    /// lowest bit any of them has.
    fn incircle_of_whole_numbers(points: [Point; 4]) -> Circle {
        let unit = points.iter().flat_map(|p| [p.x(), p.y()]).filter(|v| *v != 0.0).map(|v| dyadic(v).1).min().unwrap_or(0);
        let [a, b, c, d] = points.map(|p| (Big::of(p.x(), unit), Big::of(p.y(), unit)));
        let offset = |p: &(Big, Big)| (p.0.plus(&d.0, -1), p.1.plus(&d.1, -1));
        let (a, b, c) = (offset(&a), offset(&b), offset(&c));
        let lift = |p: &(Big, Big)| p.0.times(&p.0).plus(&p.1.times(&p.1), 1);
        let cross = |p: &(Big, Big), q: &(Big, Big)| p.0.times(&q.1).plus(&p.1.times(&q.0), -1);
        let det = lift(&a).times(&cross(&b, &c)).plus(&lift(&b).times(&cross(&c, &a)), 1).plus(&lift(&c).times(&cross(&a, &b)), 1);
        match det.sign() {
            1 => Circle::Inside,
            -1 => Circle::Outside,
            _ => Circle::On,
        }
    }

    #[test]
    fn the_whole_number_reference_counts_right() {
        let three = Big::of(3.0, 0);
        assert_eq!(three.times(&three).plus(&Big::of(9.0, 0), -1).sign(), 0);
        assert_eq!(Big::of(-0.75, -2).times(&three).sign(), -1);
        assert_eq!(Big::of(1e30, 0).plus(&Big::of(1.0, 0), -1).sign(), 1);
        assert_eq!(dyadic(-0.375), (-3, -3));
        assert_eq!(Big::of(0.0, 5).sign(), 0);
    }

    #[test]
    fn incircle_plain_cases() {
        // The circle through the corners of a right triangle has its hypotenuse for a diameter.
        let (a, b, c) = (p(0.0, 0.0), p(2.0, 0.0), p(0.0, 2.0));
        assert_eq!(incircle(a, b, c, p(1.0, 1.0)), Circle::Inside);
        assert_eq!(incircle(a, b, c, p(2.0, 2.0)), Circle::On);
        assert_eq!(incircle(a, b, c, p(3.0, 3.0)), Circle::Outside);
        // The same corners listed clockwise give the other answer, and so the corners themselves lie on it.
        assert_eq!(incircle(a, c, b, p(1.0, 1.0)), Circle::Outside);
        assert_eq!(incircle(b, c, a, p(1.0, 1.0)), Circle::Inside);
        assert_eq!(incircle(a, b, c, b), Circle::On);
        // 4 points of a circle of radius 5 with whole coordinates, and one a unit in the last place off it.
        let (a, b, c) = (p(5.0, 0.0), p(0.0, 5.0), p(-5.0, 0.0));
        assert_eq!(incircle(a, b, c, p(3.0, -4.0)), Circle::On);
        assert_eq!(incircle(a, b, c, p(3.0, f64::from_bits((-4.0_f64).to_bits() + 1))), Circle::Outside);
        assert_eq!(incircle(a, b, c, p(3.0, f64::from_bits((-4.0_f64).to_bits() - 1))), Circle::Inside);
    }

    #[test]
    fn the_incircle_error_bound_is_shewchuks() {
        // (10 + 96ε)ε multiplied out; both are exact in f64. A looser bound only sends more cases to the exact
        // sum, which gives the same signs, so no other test notices it.
        let epsilon = f64::EPSILON / 2.0;
        assert_eq!(INCIRCLE_BOUND, 10.0 * epsilon + 96.0 * epsilon * epsilon);
    }

    #[test]
    fn points_nearly_on_a_circle_are_decided_exactly() {
        // 4 points a circle's width apart, each nudged by up to 3 units of the last place: their offsets are
        // exact, and the plain determinant is often 0 or of the wrong sign. Scaling by a power of 2 changes
        // no answer.
        let mut rng = SplitMix64::new(20_261_027);
        let epsilon = f64::EPSILON / 2.0;
        let mut wrong_plain = 0;
        for scale in [1.0 / 1_099_511_627_776.0, 1.0, 1_099_511_627_776.0] {
            for _ in 0..4_000 {
                let mut nudge = || 0.5 + f64::from(i32::try_from(rng.next_u64() % 7).unwrap() - 3) * epsilon;
                let corners = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)];
                let points = corners.map(|(x, y)| p((x + nudge()) * scale, (y + nudge()) * scale));
                let truth = incircle_of_whole_numbers(points);
                let [a, b, c, d] = points;
                assert_eq!(incircle(a, b, c, d), truth, "{points:?}");
                assert_eq!(incircle(b, c, a, d), truth, "{points:?}");
                let reversed = match truth {
                    Circle::Inside => Circle::Outside,
                    Circle::Outside => Circle::Inside,
                    Circle::On => Circle::On,
                };
                assert_eq!(incircle(b, a, c, d), reversed, "{points:?}");
                wrong_plain += usize::from(plain_incircle(a, b, c, d) != truth);
            }
        }
        // The plain determinant is wrong at many of them, so the test reaches the cases the exact sum is for.
        assert!(wrong_plain > 1_000, "{wrong_plain}");
    }

    #[test]
    fn offsets_that_round_are_decided_exactly() {
        // Points of a circle of every size and place, its corners' coordinates of different magnitudes, so
        // that their offsets round and the exact sum needs their errors too, and a fourth point put on the
        // circle in floating point, where it lies a few units of the last place off it.
        let mut rng = SplitMix64::new(20_261_028);
        let mut rounded = 0;
        for case in 0..3_000 {
            let size = [1e-6, 1e-3, 1.0, 1e3][case % 4];
            let centre = (rng.next_in(-2.0, 2.0) * size * [1e-3, 1.0, 1e3][case % 3], rng.next_in(-2.0, 2.0) * size);
            let radius = size * rng.next_in(0.5, 2.0);
            let on = |turn: f64| {
                let (sin, cos) = crate::math::sin_cos(turn);
                p(centre.0 + radius * cos, centre.1 + radius * sin)
            };
            let turns: [f64; 3] = [0.0, 2.0, 4.0].map(|start| start + rng.next_in(0.0, 1.5));
            let [a, b, c] = turns.map(on);
            let d = on(rng.next_in(0.0, std::f64::consts::TAU));
            let truth = incircle_of_whole_numbers([a, b, c, d]);
            assert_eq!(incircle(a, b, c, d), truth, "{:?}", [a, b, c, d]);
            let exact_offsets = [a, b, c].iter().all(|q| two_diff(q.x(), d.x()).1 == 0.0 && two_diff(q.y(), d.y()).1 == 0.0);
            rounded += usize::from(!exact_offsets && plain_incircle(a, b, c, d) != truth);
        }
        assert!(rounded > 100, "{rounded}");
    }

    /// The in-circle determinant's sign in plain floating point.
    fn plain_incircle(a: Point, b: Point, c: Point, d: Point) -> Circle {
        let o = |q: Point| (q.x() - d.x(), q.y() - d.y());
        let ((ax, ay), (bx, by), (cx, cy)) = (o(a), o(b), o(c));
        let det = (ax * ax + ay * ay) * (bx * cy - cx * by) + (bx * bx + by * by) * (cx * ay - ax * cy) + (cx * cx + cy * cy) * (ax * by - bx * ay);
        if det > 0.0 {
            Circle::Inside
        } else if det < 0.0 {
            Circle::Outside
        } else {
            Circle::On
        }
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
