//! Split stitches, and the order the needle sews a satin column's points in
//! (`docs/src/design/algorithms/satin.md` › Top stitches, steps 4 and 5).
//!
//! The needle goes from the first rail to the second and back, pair after pair: each stitch across the
//! column, from a pair's first point to its second, and each stitch back, from its second point to the
//! next pair's first. Where the element sets a longest stitch (`max_stitch_length_mm`), a stitch longer
//! than it is split as `split_method` says, as Ink/Stitch splits it. The ends of a stitch are its pair's
//! compensated points, and it is sewn between their insets (step 3).
//!
//! - **Default** splits a stitch into the fewest equal parts no longer than the longest stitch, and the
//!   stitch back after it into as many as the stitch across before it. `random_split_jitter_percent` moves
//!   each split up to that share of a part either way, at random. With `random_split_phase`, the splits
//!   start a random share of the longest stitch from the stitch's inset start instead, and follow at the
//!   longest stitch, longer or shorter by up to the jitter. Stitches no longer than
//!   `min_random_split_length_mm` are not split, and a split within the shortest stitch of an end is left
//!   out, unless it is the only one.
//! - **Simple** splits a stitch at whole multiples of the longest stitch from its start, and the stitch
//!   back from its end, where they lie between its insets.
//! - **Staggered** does the same from a start that moves along by a `split_staggers`-th of the longest
//!   stitch from one stitch to the next, and comes back after that many stitches.
//!
//! Random splits draw from the element's generator, after the column's pairs are placed, in the order the
//! stitches are sewn.

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use crate::generators::satin::SatinParams;
use crate::generators::satin::pairs::Pair;

/// How a satin column's stitches are split.
pub(crate) struct Splitter<'r> {
    /// Where the splits go.
    method: Method,
    /// The longest stitch, in millimetres; `None` splits none.
    length: Option<f64>,
    /// How far a split may move at random, or a part's length vary with random phase, as a fraction.
    jitter: f64,
    /// Whether splits start at a random share of the longest stitch (the default method's random phase).
    random_phase: bool,
    /// Stitches no longer than this are not split by the default method, in millimetres.
    shortest_split: f64,
    /// How many stitches staggered splits take to come back to where they started.
    staggers: f64,
    /// The element's shortest stitch, in millimetres: a random split this near an end is left out.
    min_stitch: f64,
    /// The element's generator.
    rng: &'r mut SplitMix64,
}

/// The split methods, as `split_method` names them.
#[derive(Clone, Copy, PartialEq)]
enum Method {
    Default,
    Simple,
    Staggered,
}

impl<'r> Splitter<'r> {
    /// The splitter `params` describe, for an element whose longest stitch is `max_stitch` and shortest
    /// `min_stitch`, drawing from `rng`.
    pub(crate) fn new(params: &SatinParams, max_stitch: Option<f64>, min_stitch: f64, rng: &'r mut SplitMix64) -> Splitter<'r> {
        let method = match params.split_method {
            "simple" => Method::Simple,
            "staggered" => Method::Staggered,
            _ => Method::Default,
        };
        let shortest_split = max_stitch.map_or(0.0, |length| params.min_random_split_length_mm.map_or(length, |mm| mm.get().min(length)));
        Splitter {
            method,
            length: max_stitch,
            jitter: params.random_split_jitter_percent / 100.0,
            random_phase: params.random_split_phase,
            shortest_split,
            staggers: params.split_staggers,
            min_stitch,
            rng,
        }
    }

    /// The most a short stitch's inset may move a point, in millimetres: a third of the longest stitch,
    /// where the default method splits evenly, as in Ink/Stitch.
    pub(crate) fn inset_limit(&self) -> Option<f64> {
        self.length.filter(|_| self.method == Method::Default && !self.random_phase).map(|length| length / 3.0)
    }

    /// The needle points of the column whose pairs are `pairs` and their insets `short`: each pair's inset
    /// points, with the splits of the stitch across from the first to the second, and of the stitch back to
    /// the next pair. A unit of `meter`'s work per split tried.
    pub(crate) fn sew(&mut self, pairs: &[Pair], short: &[Pair], meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let mut run = Vec::with_capacity(pairs.len() * 2);
        let mut back: Option<(Point, Point)> = None;
        let mut parts: Option<u32> = None;
        for (row, ([a, b], [inset_a, inset_b])) in (0_u32..).step_by(2).zip(pairs.iter().zip(short)) {
            if let Some((from, inset_from)) = back {
                run.extend(self.split([from, *a], [inset_from, *inset_a], parts, row, true, meter)?.0);
            }
            run.push(*inset_a);
            let (splits, across) = self.split([*a, *b], [*inset_a, *inset_b], None, row + 1, false, meter)?;
            run.extend(splits);
            run.push(*inset_b);
            (back, parts) = (Some((*b, *inset_b)), across);
        }
        Ok(run)
    }

    /// The splits of the stitch between `ends`, sewn from `inset[0]` to `inset[1]`: stitch `row` of the
    /// column, back from the next pair's end when `back`. The default method splits it into `parts` when
    /// they are given, and says how many parts it made.
    fn split(
        &mut self,
        ends: [Point; 2],
        inset: [Point; 2],
        parts: Option<u32>,
        row: u32,
        back: bool,
        meter: &mut Meter,
    ) -> Result<(Vec<Point>, Option<u32>), Exhausted> {
        let Some(length) = self.length else { return Ok((Vec::new(), None)) };
        let [a, b] = ends;
        match self.method {
            Method::Default if a.distance(b) <= self.shortest_split => Ok((Vec::new(), Some(1))),
            Method::Default if self.random_phase => Ok((self.random_phase(ends, inset, length, meter)?, None)),
            Method::Default => {
                let parts = parts.unwrap_or_else(|| fewest_parts(a.distance(b), length));
                Ok((self.even(a, b, parts, meter)?, Some(parts)))
            }
            Method::Simple => Ok((staggered(ends, inset, length, 1.0, row, back, meter)?, None)),
            Method::Staggered => Ok((staggered(ends, inset, length, self.staggers, row, back, meter)?, None)),
        }
    }

    /// The points that split the stitch from `a` to `b` into `parts` equal parts, each moved up to the
    /// jitter's share of a part either way, at random, in order along it.
    fn even(&mut self, a: Point, b: Point, parts: u32, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let mut at = Vec::new();
        for k in 1..parts {
            meter.charge(1)?;
            at.push((f64::from(k) + (self.rng.next_f64() * 2.0 - 1.0) * self.jitter) / f64::from(parts));
        }
        at.sort_by(f64::total_cmp);
        Ok(at.into_iter().map(|t| a.lerp(b, t)).collect())
    }

    /// The splits of the stitch between `ends` with a random phase, sewn from `inset[0]` to `inset[1]`: the
    /// first a random share of `length` from the inset start, then one every `length`, longer or shorter by
    /// up to the jitter at random, while they lie before the inset end. A split within the shortest stitch
    /// of an end is left out, unless it is the only one.
    fn random_phase(&mut self, ends: [Point; 2], inset: [Point; 2], length: f64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
        let ([a, b], [from, to]) = (ends, inset);
        let distance = from.distance(to);
        let mut at = Vec::new();
        let mut progress = length * self.rng.next_f64();
        while progress < distance {
            meter.charge(1)?;
            at.push(from.lerp(to, progress / distance));
            progress += length * (1.0 + self.jitter * (self.rng.next_f64() - 0.5) * 2.0);
        }
        if at.len() > 1 && at.first().is_some_and(|first| first.distance(a) <= self.min_stitch) {
            at.remove(0);
        }
        if at.len() > 1 && at.last().is_some_and(|last| last.distance(b) <= self.min_stitch) {
            at.pop();
        }
        Ok(at)
    }
}

/// The splits of the stitch between `ends`, sewn from `inset[0]` to `inset[1]` and stitch `row` of the
/// column, at whole multiples of `length` from a start that moves along by a `staggers`-th of it from one
/// stitch to the next: from the stitch's start, or from its end when it goes `back`. Only splits strictly
/// between the insets count, and a stitch no longer than `length` has none.
fn staggered(
    ends: [Point; 2],
    inset: [Point; 2],
    length: f64,
    staggers: f64,
    row: u32,
    back: bool,
    meter: &mut Meter,
) -> Result<Vec<Point>, Exhausted> {
    let ([a, b], [from, to]) = if back { ([ends[1], ends[0]], [inset[1], inset[0]]) } else { (ends, inset) };
    let distance = a.distance(b);
    if distance <= length {
        return Ok(Vec::new());
    }
    let (after, before) = (along(a, b, from), along(a, b, to));
    let mut at = Vec::new();
    let mut progress = (f64::from(row) / staggers).rem_euclid(1.0) * length;
    while progress < before {
        meter.charge(1)?;
        if after < progress {
            at.push(a.lerp(b, progress / distance));
        }
        progress += length;
    }
    if back {
        at.reverse();
    }
    Ok(at)
}

/// The fewest parts no longer than `length` that `distance` divides into, at least 1, as Ink/Stitch
/// counts them: the distance over the length, rounded up.
fn fewest_parts(distance: f64, length: f64) -> u32 {
    // A whole number of at least 1, and `as` saturates: the budget refuses as many parts as that.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let parts = (distance / length).ceil().max(1.0) as u32;
    parts
}

/// How far along the line from `a` to `b` the point of it nearest `p` is, held to the line's ends.
fn along(a: Point, b: Point, p: Point) -> f64 {
    let distance = a.distance(b);
    let (x, y) = ((b.x() - a.x()) / distance, (b.y() - a.y()) / distance);
    ((p.x() - a.x()) * x + (p.y() - a.y()) * y).clamp(0.0, distance)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_params::ParamSet;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// The satin parameters `settings` give.
    fn params(settings: &[(&str, &str)]) -> SatinParams {
        SatinParams::from_set(&settings.iter().copied().collect::<ParamSet>()).unwrap().params
    }

    #[test]
    fn insets_are_held_to_a_third_of_the_longest_stitch_only_where_the_default_method_splits_evenly() {
        let limit =
            |settings: &[(&str, &str)], longest: Option<f64>| Splitter::new(&params(settings), longest, 0.3, &mut SplitMix64::new(0)).inset_limit();
        assert_eq!(limit(&[], Some(1.5)), Some(0.5));
        assert_eq!(limit(&[], None), None);
        assert_eq!(limit(&[("random_split_phase", "true")], Some(1.5)), None);
        assert_eq!(limit(&[("split_method", "simple")], Some(1.5)), None);
        assert_eq!(limit(&[("split_method", "staggered")], Some(1.5)), None);
    }

    #[test]
    fn a_stitch_divides_into_the_fewest_parts_no_longer_than_the_longest_stitch() {
        assert_eq!([fewest_parts(7.0, 3.0), fewest_parts(6.0, 3.0), fewest_parts(0.5, 3.0), fewest_parts(0.0, 3.0)], [3, 2, 1, 1]);
        assert_eq!(fewest_parts(1e300, 1e-3), u32::MAX, "as many as the budget refuses");
    }

    #[test]
    fn the_shortest_stitch_to_split_is_the_longest_stitch_unless_set_shorter() {
        let shortest = |settings: &[(&str, &str)]| Splitter::new(&params(settings), Some(3.0), 0.3, &mut SplitMix64::new(0)).shortest_split;
        assert_eq!(shortest(&[]), 3.0);
        assert_eq!(shortest(&[("min_random_split_length_mm", "2")]), 2.0);
        assert_eq!(shortest(&[("min_random_split_length_mm", "5")]), 3.0, "never longer than the longest stitch");
    }

    #[test]
    fn staggered_splits_lie_strictly_between_the_insets() {
        // A stitch from (0, 0) to (0, 10), sewn from 1 mm in to 9.5 mm, split every 3 mm from its start: at
        // 3, 6 and 9.
        let meter = &mut Budget::DEFAULT.meter();
        let (ends, inset) = ([p(0.0, 0.0), p(0.0, 10.0)], [p(0.0, 1.0), p(0.0, 9.5)]);
        assert_eq!(ys(&staggered(ends, inset, 3.0, 1.0, 0, false, meter).unwrap()), [3.0, 6.0, 9.0]);
        // Back, every 3 mm from its end: 3 and 6 mm from it lie between the insets, and 9 does not, sewn in
        // the stitch's order.
        assert_eq!(ys(&staggered(ends, inset, 3.0, 1.0, 0, true, meter).unwrap()), [4.0, 7.0]);
        // Sewn from 3 mm in to 9 mm, the splits at 3 and 9 fall exactly on the insets and are not made.
        assert_eq!(ys(&staggered(ends, [p(0.0, 3.0), p(0.0, 9.0)], 3.0, 1.0, 0, false, meter).unwrap()), [6.0]);
        // No longer than the length: none.
        assert!(staggered([p(0.0, 0.0), p(0.0, 2.0)], [p(0.0, 0.0), p(0.0, 2.0)], 2.0, 1.0, 0, false, meter).unwrap().is_empty());
    }

    /// A splitter with the default method, `length` and `jitter`, a random phase or not, and the shortest
    /// stitch `min`, drawing from `rng`.
    fn splitter(rng: &mut SplitMix64, length: f64, jitter: f64, random_phase: bool, min: f64) -> Splitter<'_> {
        Splitter { method: Method::Default, length: Some(length), jitter, random_phase, shortest_split: length, staggers: 4.0, min_stitch: min, rng }
    }

    /// The heights of `points`, to within rounding.
    fn ys(points: &[Point]) -> Vec<f64> {
        points.iter().map(|p| (p.y() * 1e9).round() / 1e9).collect()
    }

    /// `values` to within rounding.
    fn rounded(values: &[f64]) -> Vec<f64> {
        values.iter().map(|v| (v * 1e9).round() / 1e9).collect()
    }

    #[test]
    fn even_splits_move_by_their_rolls_as_in_ink_stitch() {
        // 4 parts of a 10 mm stitch, each split moved by (2 × roll − 1) × jitter of a part, then sorted.
        let mut rolls = SplitMix64::new(5);
        let mut want: Vec<f64> = (1..4).map(|k| (f64::from(k) + (rolls.next_f64() * 2.0 - 1.0) * 0.5) * 2.5).collect();
        want.sort_by(f64::total_cmp);
        let mut rng = SplitMix64::new(5);
        let got = splitter(&mut rng, 3.0, 0.5, false, 0.3).even(p(0.0, 0.0), p(0.0, 10.0), 4, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(ys(&got), rounded(&want));
    }

    /// Where a random phase puts its splits on a stitch `distance` long: the first at `length` × roll, and
    /// each next `length` × (1 + `jitter` × (2 × roll − 1)) on, while they lie before the end.
    fn phased(seed: u64, length: f64, jitter: f64, distance: f64) -> Vec<f64> {
        let mut rolls = SplitMix64::new(seed);
        let mut at = Vec::new();
        let mut progress = length * rolls.next_f64();
        while progress < distance {
            at.push(progress);
            progress += length * (1.0 + jitter * (rolls.next_f64() - 0.5) * 2.0);
        }
        at
    }

    #[test]
    fn a_random_phase_starts_at_its_roll_and_steps_by_the_length_varied_by_the_jitter() {
        let meter = &mut Budget::DEFAULT.meter();
        let stitch = [p(0.0, 0.0), p(0.0, 10.0)];
        let want = phased(9, 2.0, 0.5, 10.0);
        assert!(want.len() >= 4, "{want:?}");
        let got = splitter(&mut SplitMix64::new(9), 2.0, 0.5, true, 0.0).random_phase(stitch, stitch, 2.0, meter).unwrap();
        assert_eq!(ys(&got), rounded(&want));
        // A split exactly at the inset end is not made: the stitch is sewn to where the third would be, and
        // ends a millimetre on.
        let third = *want.get(2).unwrap();
        let (ends, inset) = ([p(0.0, 0.0), p(0.0, third + 1.0)], [p(0.0, 0.0), p(0.0, third)]);
        let got = splitter(&mut SplitMix64::new(9), 2.0, 0.5, true, 0.0).random_phase(ends, inset, 2.0, meter).unwrap();
        assert_eq!(ys(&got), rounded(&want[..2]));
    }

    #[test]
    fn of_2_or_more_random_splits_one_within_the_shortest_stitch_of_an_end_is_left_out() {
        let meter = &mut Budget::DEFAULT.meter();
        let stitch = [p(0.0, 0.0), p(0.0, 10.0)];
        // Seed 3 starts the splits 0.23 mm in, every 2 mm, so a shortest stitch just past that leaves the
        // first out.
        let want = phased(3, 2.0, 0.0, 10.0);
        let first = *want.first().unwrap();
        let got = splitter(&mut SplitMix64::new(3), 2.0, 0.0, true, first + 0.001).random_phase(stitch, stitch, 2.0, meter).unwrap();
        assert_eq!(ys(&got), rounded(&want[1..]));
        // Alone, it stays: the stitch ends before the second.
        let one = [p(0.0, 0.0), p(0.0, first + 1.0)];
        let got = splitter(&mut SplitMix64::new(3), 2.0, 0.0, true, first + 0.001).random_phase(one, one, 2.0, meter).unwrap();
        assert_eq!(ys(&got), rounded(&want[..1]));
        // Seed 6 starts them 1.48 mm in, so the last, 9.48 mm in, is 0.52 mm from the end, and a shortest
        // stitch just past that leaves it out, but not the first.
        let want = phased(6, 2.0, 0.0, 10.0);
        let last = 10.0 - want.last().unwrap();
        let got = splitter(&mut SplitMix64::new(6), 2.0, 0.0, true, last + 0.001).random_phase(stitch, stitch, 2.0, meter).unwrap();
        assert_eq!(ys(&got), rounded(&want[..want.len() - 1]));
        // Alone, it stays.
        let one = [p(0.0, 0.0), p(0.0, want[0] + 0.1)];
        let got = splitter(&mut SplitMix64::new(6), 2.0, 0.0, true, 0.2).random_phase(one, one, 2.0, meter).unwrap();
        assert_eq!(ys(&got), rounded(&want[..1]));
    }

    #[test]
    fn a_point_s_place_along_a_line_is_held_to_its_ends() {
        let (a, b) = (p(0.0, 0.0), p(4.0, 0.0));
        assert_eq!([along(a, b, p(1.0, 3.0)), along(a, b, p(-1.0, 0.0)), along(a, b, p(9.0, -1.0))], [1.0, 0.0, 4.0]);
    }
}
