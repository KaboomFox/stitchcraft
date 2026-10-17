//! A live effect's geometry as JSON, written by hand.
//!
//! **Why by hand.** VectorCraft's host charges a live effect for every instruction it runs, and writing
//! its output is what limits a preview (`docs/src/design/vectorcraft-integration.md`, live-effect
//! previews). A float written the shortest way that reads back exactly takes up to 17 digits, and
//! `serde_json` takes hundreds of instructions for each byte of it. A preview needs far less: a
//! coordinate here is a whole number of thousandths of a point (0.35 µm), written with as few digits as
//! that takes, so an anchor is about 20 bytes, and each digit is one division.
//!
//! **What it writes.** `{"objects":[{"path":{"subpaths":[…]}}]}`, VectorCraft's `PathData`: each subpath
//! its anchors as `{"p":[x,y]}`, without handles, and `"closed":true` only when it is closed, since
//! VectorCraft reads a subpath as open when the key is missing.

use stitchcraft_core::Point;

use crate::geometry::in_points;

/// The opening of the output, up to its first subpath.
const START: &[u8] = br#"{"objects":[{"path":{"subpaths":["#;
/// The end of the output, after its last subpath.
const END: &[u8] = b"]}}]}";
/// Thousandths of a point: how finely coordinates are written.
const SCALE: f64 = 1000.0;

/// A live effect's geometry being written.
pub(crate) struct Geometry {
    out: Vec<u8>,
    subpaths: usize,
}

impl Geometry {
    /// Geometry with no subpath yet.
    pub(crate) fn new() -> Self {
        Geometry { out: START.to_vec(), subpaths: 0 }
    }

    /// Adds a subpath through `points`, in millimetres, closed back to its first one when `closed`. A
    /// subpath of fewer than 2 points draws nothing and is left out.
    pub(crate) fn subpath(&mut self, points: &[Point], closed: bool) {
        if points.len() < 2 {
            return;
        }
        if self.subpaths > 0 {
            self.out.push(b',');
        }
        self.subpaths += 1;
        self.out.extend_from_slice(if closed { br#"{"closed":true,"anchors":["# } else { br#"{"anchors":["# });
        for (k, point) in points.iter().enumerate() {
            if k > 0 {
                self.out.push(b',');
            }
            let [x, y] = in_points(*point);
            self.out.extend_from_slice(br#"{"p":["#);
            number(&mut self.out, x);
            self.out.push(b',');
            number(&mut self.out, y);
            self.out.extend_from_slice(b"]}");
        }
        self.out.extend_from_slice(b"]}");
    }

    /// The JSON, or `None` when no subpath was added: then the geometry draws nothing.
    pub(crate) fn finish(mut self) -> Option<Vec<u8>> {
        if self.subpaths == 0 {
            return None;
        }
        self.out.extend_from_slice(END);
        Some(self.out)
    }
}

/// Writes `value` to the nearest thousandth, ties to even, with the digits that takes: no trailing zeros
/// after the point, and no point for a whole number.
fn number(out: &mut Vec<u8>, value: f64) {
    // Coordinates lie within the working limit, 10 m, far inside what an `i64` of thousandths holds. The
    // conversion saturates, and takes a value that is not a number as 0, rather than failing.
    #[allow(clippy::cast_possible_truncation)]
    let thousandths = (value * SCALE).round_ties_even() as i64;
    if thousandths < 0 {
        out.push(b'-');
    }
    let magnitude = thousandths.unsigned_abs();
    digits(out, magnitude / 1000);
    let mut fraction = magnitude % 1000;
    if fraction == 0 {
        return;
    }
    out.push(b'.');
    let mut place = 100;
    while fraction > 0 {
        out.push(b'0' + digit(fraction / place));
        fraction %= place;
        place /= 10;
    }
}

/// Writes `n` in decimal.
fn digits(out: &mut Vec<u8>, n: u64) {
    let mut place = 1;
    while n / place >= 10 {
        place *= 10;
    }
    while place > 0 {
        out.push(b'0' + digit(n / place % 10));
        place /= 10;
    }
}

/// `n`, a decimal digit, as the byte's offset from `0`.
fn digit(n: u64) -> u8 {
    u8::try_from(n % 10).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::units::MM_PER_POINT;

    use super::*;

    fn written(value: f64) -> String {
        let mut out = Vec::new();
        number(&mut out, value);
        String::from_utf8(out).unwrap()
    }

    #[test]
    fn a_coordinate_is_written_to_the_nearest_thousandth_of_a_point_with_the_digits_it_takes() {
        assert_eq!(written(0.0), "0");
        assert_eq!(written(-0.0004), "0");
        assert_eq!(written(12.0), "12");
        assert_eq!(written(12.5), "12.5");
        assert_eq!(written(12.05), "12.05");
        assert_eq!(written(12.0051), "12.005");
        assert_eq!(written(0.0025), "0.002");
        assert_eq!(written(-3.0009), "-3.001");
        assert_eq!(written(1_000_000.25), "1000000.25");
        assert_eq!(written(28_346.456_692), "28346.457");
        assert_eq!(written(f64::NAN), "0");
    }

    #[test]
    fn subpaths_are_written_as_vectorcraft_reads_them() {
        let p = |x: f64, y: f64| Point::new(x * MM_PER_POINT, y * MM_PER_POINT).unwrap();
        let mut geometry = Geometry::new();
        geometry.subpath(&[p(1.0, 2.0), p(3.5, -4.25)], false);
        geometry.subpath(&[p(9.0, 9.0)], false);
        geometry.subpath(&[p(0.0, 0.0), p(1.0, 0.0), p(1.0, 1.0)], true);
        let text = String::from_utf8(geometry.finish().unwrap()).unwrap();
        assert_eq!(
            text,
            r#"{"objects":[{"path":{"subpaths":[{"anchors":[{"p":[1,2]},{"p":[3.5,-4.25]}]},{"closed":true,"anchors":[{"p":[0,0]},{"p":[1,0]},{"p":[1,1]}]}]}}]}"#
        );
        assert!(serde_json::from_str::<serde_json::Value>(&text).is_ok());
        assert_eq!(Geometry::new().finish(), None);
    }
}
