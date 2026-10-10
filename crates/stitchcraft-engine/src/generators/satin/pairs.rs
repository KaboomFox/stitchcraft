//! A satin column's needle points, placed along its sections as Ink/Stitch places them
//! (`docs/src/design/algorithms/satin.md` › Top stitches).
//!
//! The stitches go across the column in pairs of needle points, one on each rail at the same fraction of
//! its section, and the column is sewn first rail, second rail, pair after pair. Each pair is meant to lie
//! the zigzag spacing from the one before it, measured across the column: at a right angle to the previous
//! pair, at whichever end is farther (the outside of a curve), or from the previous pair's first point when
//! that pair has no length.
//!
//! In each section, a pair is placed the spacing's share of the section's longer rail past the one before.
//! A pair that lands more than 5 % off the spacing moves: the step to it is scaled by how far off it is, at
//! most twice, and the first time no farther than the section's end. The first pair of a section after the
//! first is placed as far past the section's start as the previous pair is short of the spacing from it.
//! The column starts with a pair at its start, and ends with one at its end unless the last pair is within
//! 0.1 mm of it.
//!
//! With random spacing, each step's spacing is drawn at random, at each section's start and after each
//! pair, and a pair moves to lie that spacing from the one before. Pairs are measured from each other as
//! the rails place them, and each is widened by pull compensation only as it goes into the column
//! (the `compensation` module).

use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::satin::column::Section;
use crate::generators::satin::compensation::Processor;
use crate::normalize::along::Along;
use crate::normalize::stroke::distance_to_segment;

/// A pair of needle points across the column: the one on the first rail, then the one on the second.
pub(crate) type Pair = [Point; 2];

/// Lengths below this, in millimetres, count as none: a hundredth of a CSS pixel, as in Ink/Stitch.
const NO_LENGTH: f64 = 0.01 * 25.4 / 96.0;

/// How far off the spacing a pair may land, as a fraction of it, before it moves.
const OFF: f64 = 0.05;

/// How many times a pair moves at most.
const MOVES: u32 = 2;

/// The column ends with a pair at its end unless its last pair is closer to it than this, in millimetres.
const END: f64 = 0.1;

/// The pairs of needle points along `sections`, `spacing` millimetres apart, each step and each pair as
/// `processor` makes them, at a unit of `meter`'s work per point measured and per place a pair is tried at.
pub(crate) fn pairs(sections: &[Section], spacing: f64, processor: &mut Processor, meter: &mut Meter) -> Result<Vec<Pair>, Exhausted> {
    let mut pairs: Vec<Pair> = Vec::new();
    let mut last: Option<Pair> = None;
    for [first, second] in sections {
        let (a, b) = (Along::new(first, meter)?, Along::new(second, meter)?);
        let at = |fraction: f64| [a.point(fraction * a.length()), b.point(fraction * b.length())];
        let start = at(0.0);
        let mut previous = match last {
            Some(pair) => pair,
            None => {
                pairs.push(processor.widened(start));
                start
            }
        };
        let step = spacing / a.length().max(b.length()).max(NO_LENGTH);
        let mut times = processor.step();
        let mut ahead = (1.0 - (gap(start, previous) / spacing).min(1.0)) * step * times;
        let (mut done, mut tries) = (0.0, 0);
        while done + ahead <= 1.0 {
            meter.charge(1)?;
            tries += 1;
            let pair = at(done + ahead);
            let (apart, wanted) = (gap(pair, previous), spacing * times);
            if tries <= MOVES && apart > NO_LENGTH && ((wanted - apart) / wanted).abs() > OFF {
                ahead *= wanted / apart;
                if tries == 1 {
                    ahead = ahead.min(1.0 - done);
                }
                continue;
            }
            done += ahead;
            times = processor.step();
            ahead = step * times;
            tries = 0;
            previous = pair;
            pairs.push(processor.widened(pair));
        }
        last = Some(previous);
    }
    if let (Some(previous), Some(end)) = (last, sections.last().map(end_of))
        && gap(end, previous) > END
    {
        pairs.push(processor.widened(end));
    }
    Ok(pairs)
}

/// The pair at the end of `section`: its rails' last points.
fn end_of([first, second]: &Section) -> Pair {
    [first.last().copied().unwrap_or(Point::ORIGIN), second.last().copied().unwrap_or(Point::ORIGIN)]
}

/// How far `pair` lies from `previous` across the column: at a right angle to `previous`, at whichever end
/// is farther, or from `previous`'s first point when `previous` has no length.
fn gap([a, b]: Pair, [c, d]: Pair) -> f64 {
    let (x, y) = (d.x() - c.x(), d.y() - c.y());
    let length = c.distance(d);
    if length < NO_LENGTH {
        return distance_to_segment(c, a, b);
    }
    let across = |p: Point, q: Point| ((q.y() - p.y()) * x - (q.x() - p.x()) * y).abs() / length;
    across(a, c).max(across(b, d))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_core::rng::SplitMix64;
    use stitchcraft_params::ParamSet;

    use super::*;
    use crate::generators::satin::SatinParams;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The pairs along `sections`, `spacing` apart, with no compensation and no random variation.
    fn placed(sections: &[Section], spacing: f64) -> Vec<Pair> {
        let params = SatinParams::from_set(&ParamSet::new()).unwrap().params;
        let mut rng = SplitMix64::new(0);
        pairs(sections, spacing, &mut Processor::new(&params, &mut rng), &mut Budget::DEFAULT.meter()).unwrap()
    }

    #[test]
    fn pairs_are_apart_across_the_previous_pair_at_its_farther_end() {
        // The previous pair across x = 0; the next one 0.5 mm on at the top, 0.3 mm at the bottom.
        let previous = [p(0.0, 0.0), p(0.0, 4.0)];
        assert_eq!(gap([p(0.3, 0.0), p(0.5, 4.0)], previous), 0.5);
        assert_eq!(gap([p(-0.7, 0.0), p(0.2, 4.0)], previous), 0.7, "either way");
        // A previous pair of no length: the distance from its point to the new pair. Shorter than a
        // hundredth of a CSS pixel (0.0026 mm) is no length.
        assert_eq!(gap([p(3.0, -1.0), p(3.0, 1.0)], [p(0.0, 0.0), p(0.0, 0.0)]), 3.0);
        assert_eq!(gap([p(0.3, 0.0), p(0.3, 0.4)], [p(0.0, 0.0), p(0.002, 0.0)]), 0.3);
        assert_eq!(gap([p(0.3, 0.0), p(0.3, 0.4)], [p(0.0, 0.0), p(0.003, 0.0)]), 0.4);
        // Exactly a hundredth of a CSS pixel is a length, as in Ink/Stitch.
        assert!((gap([p(0.3, 0.0), p(0.3, 0.4)], [p(0.0, 0.0), p(NO_LENGTH, 0.0)]) - 0.4).abs() < 1e-12);
    }

    // The next 3 cases sit exactly on a limit, where Ink/Stitch compares strictly. Their rails run along
    // the axes and meet the needle points at their own points, so the arithmetic is exact.

    #[test]
    fn a_pair_a_hundredth_of_a_css_pixel_from_the_previous_one_stays_where_it_lands() {
        // At a spacing of 10 mm the next pair lands on the rails' ends. The first rail runs along the
        // start pair's line and the second runs exactly a hundredth of a CSS pixel across it, too near
        // to scale the step by.
        let section = [vec![p(0.0, 0.0), p(0.0, -10.0)], vec![p(0.0, 4.0), p(NO_LENGTH, 4.0)]];
        let sewn = placed(&[section], 10.0);
        assert_eq!(sewn, [[p(0.0, 0.0), p(0.0, 4.0)], [p(0.0, -10.0), p(NO_LENGTH, 4.0)]]);
    }

    #[test]
    fn a_pair_exactly_5_percent_off_the_spacing_stays_where_it_lands() {
        // Halfway, the first rail has run 20 mm along the start pair's line and the second 19 mm across
        // it: 5 % short of the 20 mm spacing.
        let section = [vec![p(0.0, 0.0), p(0.0, -20.0), p(0.0, -40.0)], vec![p(0.0, 4.0), p(19.0, 4.0), p(38.0, 4.0)]];
        let sewn = placed(&[section], 20.0);
        assert_eq!(sewn.get(1), Some(&[p(0.0, -20.0), p(19.0, 4.0)]), "{sewn:?}");
    }

    #[test]
    fn a_column_ending_exactly_0_1_mm_past_its_last_pair_gets_no_end_pair() {
        let section = [vec![p(0.0, 0.0), p(0.1, 0.0)], vec![p(0.0, 4.0), p(0.1, 4.0)]];
        let sewn = placed(&[section], 0.4);
        assert_eq!(sewn, [[p(0.0, 0.0), p(0.0, 4.0)]], "the start pair, with the end exactly 0.1 mm on");
    }

    #[test]
    fn a_straight_column_is_sewn_at_the_spacing_and_ends_at_its_end() {
        let section = [vec![p(0.0, 0.0), p(1.0, 0.0)], vec![p(0.0, 4.0), p(1.0, 4.0)]];
        let sewn = placed(&[section], 0.4);
        let x: Vec<f64> = sewn.iter().map(|[a, _]| a.x()).collect();
        assert_eq!(x.len(), 4, "{x:?}");
        assert!((x[1] - 0.4).abs() < 1e-12 && (x[2] - 0.8).abs() < 1e-12, "{x:?}");
        assert_eq!((x[0], x[3]), (0.0, 1.0), "the start, and the end 0.2 mm on");
        assert!(sewn.iter().all(|[a, b]| a.x() == b.x()), "straight across");
    }

    #[test]
    fn a_column_ending_within_0_1_mm_of_its_last_pair_gets_no_end_pair() {
        let section = [vec![p(0.0, 0.0), p(0.85, 0.0)], vec![p(0.0, 4.0), p(0.85, 4.0)]];
        let sewn = placed(&[section], 0.4);
        assert_eq!(sewn.len(), 3, "0, 0.4 and 0.8, and the end 0.05 mm past the last: {sewn:?}");
        assert!(placed(&[], 0.4).is_empty());
    }
}
