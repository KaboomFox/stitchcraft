//! Pull compensation, shared by the stitch types that make up for the fabric: its settings, and the one
//! move it makes, a stitch's ends moved apart along it (`docs/src/design/algorithms/satin.md` ›
//! Compensation, `docs/src/design/algorithms/fills.md` › Pull compensation).
//!
//! Thread under tension pulls the fabric in along each stitch, so a stitch sews shorter than it is drawn.
//! A satin column's stitches and a tatami fill's rows are lengthened to make up for it, each end by a
//! length in millimetres plus a share of the stitch's own length. Ink/Stitch names both settings alike for
//! both stitch types and moves both with one function, so they are declared and made here once. The two
//! stitch types differ in what they lengthen, and in what a negative value does: a satin's stitches move
//! in, and a fill reads it as 0, as Ink/Stitch does (`SC-W0312`).

use stitchcraft_core::Point;
use stitchcraft_core::units::MM_PER_SVG_PX;
use stitchcraft_params::{StitchType, params};

/// The stitch types pull compensation lengthens: satin columns, one per method, and tatami fills.
const PULLED: &[StitchType] = &[StitchType::SatinColumn, StitchType::EStitch, StitchType::SStitch, StitchType::SatinZigzag, StitchType::TatamiFill];

params! {
    /// Pull compensation: how far a satin column's stitches and a tatami fill's rows reach past the
    /// shape's edge, to make up for the thread pulling the fabric in.
    pub struct PullParams for PULLED;

    "Pull compensation" {
        /// How far each end of every stitch reaches past the shape's edge: a satin column's stitches past
        /// its rails, and a fill's rows past its outline and its holes. The thread pulls the fabric in along
        /// the stitches as it sews, so a shape comes out smaller than drawn, and this makes up for it. 2
        /// values set the 2 ends: a satin's first rail's side, then the second's, or the end each fill row
        /// starts from at the fill's angle, then the other (at 0°, the left end, then the right). Negative
        /// values make a satin narrower; a fill reads them as 0, as Ink/Stitch does.
        pull_compensation_mm: LengthPair = "0", label "Pull compensation", range (-10.0, 10.0);

        /// More pull compensation, in percent of each stitch's length, added to the length above: a
        /// satin's stitches reach out by a share of the column's width there, and a fill's rows by a share
        /// of each row's length, so wide parts and long rows reach further. 2 values set the 2 ends, as
        /// above. A fill reads negative values as 0, as Ink/Stitch does.
        pull_compensation_percent: PercentPair = "0", label "Pull compensation (%)", range (-100.0, 100.0);
    }
}

/// Points closer than this, in millimetres, are one point, with no direction between them: a
/// ten-thousandth of a CSS pixel, as in Ink/Stitch.
pub(crate) const SAME_POINT: f64 = 0.0001 * MM_PER_SVG_PX;

/// `[a, b]` with its ends moved apart along it: `a` by `mm_a` millimetres plus `share_a` of the pair's
/// width, and `b` likewise, toward each other where negative. Ends that would cross meet instead, where
/// their moves divide the width. A pair of one point stays as it is.
pub(crate) fn offset([a, b]: [Point; 2], [mm_a, mm_b]: [f64; 2], [share_a, share_b]: [f64; 2]) -> [Point; 2] {
    let width = a.distance(b);
    if width < SAME_POINT {
        return [a, b];
    }
    let (mut by_a, mut by_b) = (mm_a + width * share_a, mm_b + width * share_b);
    if by_a + by_b < -width {
        let scale = -width / (by_a + by_b);
        (by_a, by_b) = (by_a * scale, by_b * scale);
    }
    [away(a, b, by_a / width), away(b, a, by_b / width)]
}

/// `p` moved straight away from `from` by `times` their distance.
pub(crate) fn away(p: Point, from: Point, times: f64) -> Point {
    Point::new(p.x() + (p.x() - from.x()) * times, p.y() + (p.y() - from.y()) * times).unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn near(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-12
    }

    #[test]
    fn a_pair_is_widened_along_itself_by_its_sides_lengths_and_shares() {
        let pair = [p(0.0, 0.0), p(0.0, 4.0)];
        let [a, b] = offset(pair, [0.2, 0.2], [0.0, 0.0]);
        assert!(near(a, p(0.0, -0.2)) && near(b, p(0.0, 4.2)), "{a:?} {b:?}");
        // A tenth of the 4 mm width on the first side, and nothing on the second.
        let [a, b] = offset(pair, [0.0, 0.0], [0.1, 0.0]);
        assert!(near(a, p(0.0, -0.4)) && near(b, p(0.0, 4.0)), "{a:?} {b:?}");
        // Inward, where negative.
        let [a, b] = offset(pair, [-0.5, 0.0], [0.0, -0.25]);
        assert!(near(a, p(0.0, 0.5)) && near(b, p(0.0, 3.0)), "{a:?} {b:?}");
    }

    #[test]
    fn ends_that_would_cross_meet_where_their_moves_divide_the_stitch() {
        // 3 mm in from the first end and 5 mm from the second: 8 mm in all across 4 mm, so they meet
        // 3/8 of the way along.
        let [a, b] = offset([p(0.0, 0.0), p(0.0, 4.0)], [-3.0, -5.0], [0.0, 0.0]);
        assert!(near(a, p(0.0, 1.5)) && near(b, p(0.0, 1.5)), "{a:?} {b:?}");
        // Exactly meeting needs no scaling.
        let [a, b] = offset([p(0.0, 0.0), p(0.0, 4.0)], [-1.0, -3.0], [0.0, 0.0]);
        assert!(near(a, p(0.0, 1.0)) && near(b, p(0.0, 1.0)), "{a:?} {b:?}");
    }

    #[test]
    fn a_pair_of_one_point_stays_as_it_is() {
        let point = [p(3.0, 3.0), p(3.0, 3.0)];
        assert_eq!(offset(point, [1.0, 1.0], [0.5, 0.5]), point);
        // Closer than a ten-thousandth of a CSS pixel (0.0000265 mm) is one point, and that far apart or
        // more is a stitch.
        let close = [p(0.0, 0.0), p(0.0, 0.000_026)];
        assert_eq!(offset(close, [1.0, 1.0], [0.0, 0.0]), close);
        for apart in [SAME_POINT, 0.000_027] {
            let [a, _] = offset([p(0.0, 0.0), p(0.0, apart)], [1.0, 1.0], [0.0, 0.0]);
            assert!((a.y() + 1.0).abs() < 1e-9, "{apart}: {a:?}");
        }
    }
}
