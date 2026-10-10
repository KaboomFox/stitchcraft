//! What makes up for the fabric, and what varies at random (`docs/src/design/algorithms/satin.md` ›
//! Compensation).
//!
//! Thread under tension pulls the fabric in across a satin column, so a column sews narrower than it is
//! drawn, and the stitches push the fabric out at the column's ends. **Pull compensation** moves both ends
//! of every stitch outward along the stitch: by `pull_compensation_mm`, plus `pull_compensation_percent`
//! of the stitch's width, with one value for both sides or one for each. Negative values move them inward,
//! and two ends that would cross meet instead, where their moves divide the stitch. A stitch whose ends are
//! one point has no direction to move them in and stays as it is. **Push compensation** takes
//! `push_compensation_mm` off each rail at the column's start and end, before the rails are cut into
//! sections, or adds it along the rail's first or last segment where it is negative. A rail that taking it
//! off would leave shorter than half a CSS pixel keeps its length (`SC-W0211`), as in Ink/Stitch.
//!
//! **Random variation** comes from the element's generator. On each side, a stitch's share of the width
//! is drawn between `pull_compensation_percent` less `random_width_decrease_percent` and plus
//! `random_width_increase_percent`. Each step to the next stitch is drawn between the zigzag spacing less
//! and plus `random_zigzag_spacing_percent` of it, and never below a hundredth of it. As in Ink/Stitch, the
//! spacing is drawn at each section's start and after each stitch, and the widths with each stitch, so
//! turning one kind of variation on leaves the other's draws as they were.
//!
//! **Underlays** place their pairs the same way, moved in by their insets as by negative pull
//! compensation, and draw nothing at random, as in Ink/Stitch. Turning an underlay on leaves the top
//! stitches' draws as they were.

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::satin::SatinParams;
use crate::generators::satin::pairs::Pair;
use crate::normalize::along::Along;

/// Points closer than this, in millimetres, are one point, with no direction between them: a
/// ten-thousandth of a CSS pixel, as in Ink/Stitch.
const SAME_POINT: f64 = 0.0001 * 25.4 / 96.0;

/// A rail keeps its length when push compensation would leave less of it than this, in millimetres: half
/// a CSS pixel, as in Ink/Stitch.
const SHORTEST_PUSHED: f64 = 0.5 * 25.4 / 96.0;

/// The shortest a step between stitches gets at random, as a multiple of the zigzag spacing.
const SHORTEST_STEP: f64 = 0.01;

/// What happens to each pair of needle points as it is placed, and to the steps between them, as a satin
/// column's parameters say: the pairs widened by pull compensation, with a share drawn at random, and the
/// steps drawn at random around the zigzag spacing. An underlay's pairs are moved in by its insets, with
/// nothing drawn.
pub(crate) struct Processor<'r> {
    /// Pull compensation on each side, in millimetres.
    pull: [f64; 2],
    /// The least share of a stitch's width each side moves out by: pull compensation less the random
    /// decrease, as fractions.
    least: [f64; 2],
    /// How much more than `least` each side may move out by at random, as a fraction of the width.
    range: [f64; 2],
    /// How far a step may be from the zigzag spacing at random, either way, as a fraction of it.
    jitter: f64,
    /// The element's generator; `None` for an underlay, which draws nothing.
    rng: Option<&'r mut SplitMix64>,
}

impl<'r> Processor<'r> {
    /// The processor `params` describe, drawing from `rng`.
    pub(crate) fn new(params: &SatinParams, rng: &'r mut SplitMix64) -> Processor<'r> {
        let fraction = |[a, b]: [f64; 2]| [a / 100.0, b / 100.0];
        let ([pull_a, pull_b], [decrease_a, decrease_b], [increase_a, increase_b]) = (
            fraction(params.pull_compensation_percent),
            fraction(params.random_width_decrease_percent),
            fraction(params.random_width_increase_percent),
        );
        Processor {
            pull: params.pull_compensation_mm.map(|mm| mm.get()),
            least: [pull_a - decrease_a, pull_b - decrease_b],
            range: [decrease_a + increase_a, decrease_b + increase_b],
            jitter: params.random_zigzag_spacing_percent / 100.0,
            rng: Some(rng),
        }
    }

    /// The processor of an underlay inset by `mm` millimetres plus `share` of the width on each side, which
    /// draws nothing: each step is the spacing, and each pair is moved in as negative pull compensation
    /// moves it.
    pub(crate) fn inset(mm: [f64; 2], share: [f64; 2]) -> Processor<'static> {
        Processor { pull: mm.map(|mm| -mm), least: share.map(|share| -share), range: [0.0; 2], jitter: 0.0, rng: None }
    }

    /// The next step to a stitch, as a multiple of the zigzag spacing.
    pub(crate) fn step(&mut self) -> f64 {
        let Some(rng) = self.rng.as_deref_mut() else { return 1.0 };
        (1.0 + (rng.next_f64() - 0.5) * 2.0 * self.jitter).max(SHORTEST_STEP)
    }

    /// `pair` widened by pull compensation, with each side's random share drawn.
    pub(crate) fn widened(&mut self, pair: Pair) -> Pair {
        let ([least_a, least_b], [range_a, range_b]) = (self.least, self.range);
        let shares = match self.rng.as_deref_mut() {
            Some(rng) => {
                let share_a = least_a + rng.next_f64() * range_a;
                [share_a, least_b + rng.next_f64() * range_b]
            }
            None => self.least,
        };
        offset(pair, self.pull, shares)
    }
}

/// `[a, b]` with its ends moved apart along it: `a` by `mm_a` millimetres plus `share_a` of the pair's
/// width, and `b` likewise, toward each other where negative. Ends that would cross meet instead, where
/// their moves divide the width. A pair of one point stays as it is.
pub(crate) fn offset([a, b]: Pair, [mm_a, mm_b]: [f64; 2], [share_a, share_b]: [f64; 2]) -> Pair {
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

/// `rail` with push compensation, `start` millimetres off its start and `end` off its end, or added along
/// its first or last segment where negative, and whether taking it off would have left less than half a
/// CSS pixel of the rail, which then keeps its length. Measuring the rail costs `meter` a unit of work
/// per point.
pub(crate) fn pushed(rail: &[Point], [start, end]: [f64; 2], meter: &mut Meter) -> Result<(Vec<Point>, bool), Exhausted> {
    let mut points = rail.to_vec();
    if start < 0.0
        && let Some(before) = beyond(points.iter(), -start)
    {
        points.insert(0, before);
    }
    if end < 0.0
        && let Some(after) = beyond(points.iter().rev(), -end)
    {
        points.push(after);
    }
    let (start, end) = (start.max(0.0), end.max(0.0));
    if start <= 0.0 && end <= 0.0 {
        return Ok((points, false));
    }
    let along = Along::new(&points, meter)?;
    let length = along.length();
    if start + end >= length - SHORTEST_PUSHED {
        return Ok((points, true));
    }
    Ok((along.part(start, length - end), false))
}

/// The point `by` millimetres beyond the first of `points`, straight on from the first other point after
/// it: `None` when they are all one point.
fn beyond<'p>(mut points: impl Iterator<Item = &'p Point>, by: f64) -> Option<Point> {
    let first = *points.next()?;
    let next = points.find(|p| p.distance(first) >= SAME_POINT)?;
    Some(away(first, *next, by / first.distance(*next)))
}

/// `p` moved straight away from `from` by `times` their distance.
fn away(p: Point, from: Point, times: f64) -> Point {
    Point::new(p.x() + (p.x() - from.x()) * times, p.y() + (p.y() - from.y()) * times).unwrap_or(p)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    fn near(a: Point, b: Point) -> bool {
        a.distance(b) < 1e-12
    }

    /// Whether `pushed` gave the points `want`, to within rounding, and `kept` as whether the rail kept
    /// its length.
    fn gives(pushed: (Vec<Point>, bool), want: &[(f64, f64)], kept: bool) -> bool {
        let (points, kept_length) = pushed;
        kept_length == kept && points.len() == want.len() && points.iter().zip(want).all(|(point, &(x, y))| near(*point, p(x, y)))
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

    #[test]
    fn push_compensation_takes_off_or_adds_at_each_end() {
        let rail = [p(0.0, 0.0), p(4.0, 0.0), p(10.0, 0.0)];
        let meter = &mut Budget::DEFAULT.meter();
        assert_eq!(pushed(&rail, [0.0, 0.0], meter).unwrap(), (rail.to_vec(), false));
        assert!(gives(pushed(&rail, [1.0, 2.0], meter).unwrap(), &[(1.0, 0.0), (4.0, 0.0), (8.0, 0.0)], false));
        assert!(gives(pushed(&rail, [-1.0, -2.0], meter).unwrap(), &[(-1.0, 0.0), (0.0, 0.0), (4.0, 0.0), (10.0, 0.0), (12.0, 0.0)], false));
        // Added on at one end and taken off at the other.
        assert!(gives(pushed(&rail, [-1.0, 1.0], meter).unwrap(), &[(-1.0, 0.0), (0.0, 0.0), (4.0, 0.0), (9.0, 0.0)], false));
    }

    #[test]
    fn a_rail_is_lengthened_along_its_end_segments() {
        // A bent rail, and a first segment of no length, whose direction is the next point's.
        let rail = [p(0.0, 0.0), p(0.0, 0.0), p(3.0, 4.0), p(10.0, 4.0)];
        let (points, _) = pushed(&rail, [-5.0, -1.0], &mut Budget::DEFAULT.meter()).unwrap();
        assert!(near(points[0], p(-3.0, -4.0)) && near(points[points.len() - 1], p(11.0, 4.0)), "{points:?}");
        // A rail of one point has no direction to lengthen it in.
        let dot = [p(1.0, 1.0), p(1.0, 1.0)];
        assert_eq!(pushed(&dot, [-1.0, -1.0], &mut Budget::DEFAULT.meter()).unwrap(), (dot.to_vec(), false));
    }

    #[test]
    fn a_rail_keeps_its_length_when_little_would_be_left() {
        let rail = [p(0.0, 0.0), p(1.0, 0.0)];
        let meter = &mut Budget::DEFAULT.meter();
        // 1 mm long: taking off 0.9 leaves 0.1 mm, less than half a CSS pixel (0.13 mm).
        assert_eq!(pushed(&rail, [0.5, 0.4], meter).unwrap(), (rail.to_vec(), true));
        assert!(gives(pushed(&rail, [0.4, 0.4], meter).unwrap(), &[(0.4, 0.0), (0.6, 0.0)], false));
        // Lengthening still applies.
        assert!(gives(pushed(&rail, [-1.0, 1.9], meter).unwrap(), &[(-1.0, 0.0), (0.0, 0.0), (1.0, 0.0)], true));
    }

    #[test]
    fn random_steps_and_shares_fill_their_ranges() {
        let mut rng = SplitMix64::new(7);
        let mut processor = Processor { pull: [0.0, 0.0], least: [0.1, -0.2], range: [0.2, 0.0], jitter: 0.25, rng: Some(&mut rng) };
        let (mut steps, mut sides) = (Vec::new(), Vec::new());
        for _ in 0..1000 {
            steps.push(processor.step());
            let [a, b] = processor.widened([p(0.0, 0.0), p(0.0, 10.0)]);
            assert!(near(b, p(0.0, 8.0)), "{b:?}");
            sides.push(a.y());
        }
        // Steps from 0.75 to 1.25 of the spacing, and the first side 1 to 3 mm out, each range reached
        // to within 1 % at both ends.
        let range = |values: &[f64]| values.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(*v), hi.max(*v)));
        let ((least, most), (outmost, inmost)) = (range(&steps), range(&sides));
        assert!((0.75..0.755).contains(&least) && (1.245..=1.25).contains(&most), "{least} {most}");
        assert!((-3.0..-2.98).contains(&outmost) && (-1.02..=-1.0).contains(&inmost), "{outmost} {inmost}");
        // A step never gets shorter than a hundredth of the spacing.
        let mut rng = SplitMix64::new(7);
        let mut wild = Processor { pull: [0.0, 0.0], least: [0.0, 0.0], range: [0.0, 0.0], jitter: 5.0, rng: Some(&mut rng) };
        let steps: Vec<f64> = (0..100).map(|_| wild.step()).collect();
        assert!(steps.iter().all(|step| *step >= SHORTEST_STEP) && steps.contains(&SHORTEST_STEP), "{steps:?}");
    }

    #[test]
    fn an_underlay_s_pairs_move_in_by_its_insets_and_its_steps_are_the_spacing() {
        let mut inset = Processor::inset([0.5, 1.0], [0.1, 0.0]);
        assert_eq!((inset.step(), inset.step()), (1.0, 1.0));
        // 0.5 mm plus a tenth of the 10 mm width in from the first end, and 1 mm from the second.
        let [a, b] = inset.widened([p(0.0, 0.0), p(0.0, 10.0)]);
        assert!(near(a, p(0.0, 1.5)) && near(b, p(0.0, 9.0)), "{a:?} {b:?}");
    }

    #[test]
    fn a_roll_maps_to_a_step_and_to_shares_as_in_ink_stitch() {
        // The rolls the processor takes, in order: one for a step, then one for each side of a pair.
        let mut rolls = SplitMix64::new(3);
        let (step_roll, roll_a, roll_b) = (rolls.next_f64(), rolls.next_f64(), rolls.next_f64());
        let mut rng = SplitMix64::new(3);
        let mut processor = Processor { pull: [0.0, 0.0], least: [0.1, 0.2], range: [0.3, 0.4], jitter: 0.25, rng: Some(&mut rng) };
        assert_eq!(processor.step(), 1.0 + (step_roll - 0.5) * 2.0 * 0.25);
        let [a, b] = processor.widened([p(0.0, 0.0), p(0.0, 10.0)]);
        assert!((a.y() + 10.0 * (0.1 + roll_a * 0.3)).abs() < 1e-12, "{a:?}");
        assert!((b.y() - 10.0 - 10.0 * (0.2 + roll_b * 0.4)).abs() < 1e-12, "{b:?}");
    }
}
