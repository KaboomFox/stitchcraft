//! A meander's path through its network (`REQ-FILL-MEA-003`, `docs/src/design/algorithms/fills.md` ›
//! Meander fill › The path): from the point nearest the needle to the point nearest the end, as the path
//! of fewest lines, then lengthened by detours through points it has not passed until none is left.
//!
//! **Why detours.** A path of fewest lines crosses the part, and the detours make it wander: each line, or
//! each 2 lines one after the other, is swapped for a longer way round through the points not yet passed,
//! the lines and pairs drawn at random. The path never passes a point twice, so the stitches never cross.
//!
//! **The search.** A detour is the first way a depth-first search finds, from the detour's first point to
//! its last, through points not passed, up to a number of lines. It tries the points farthest from the
//! last point first, the lowest-numbered of those as far, so each detour takes the long way round where
//! there is one and reaches into the points not passed. Taken the short way first, a detour round a
//! triangle passes all 3 of its points, and a path of such detours hems itself in: over a square 100 mm a
//! side, the tiling of hexagons and triangles was 69 % covered so, and 94 % with the long way first, in
//! a third of the search's work. Ink/Stitch's search tries them as its graph stores them, and its random
//! numbers are its own, so its path differs for one seed (`DEV-FILL-020`).

use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Exhausted, Meter, Point};

use super::network::Network;

/// The fewest and most lines of a detour that replaces one line of the path, and 2 lines.
const LINE: (usize, usize) = (2, 7);
const PAIR: (usize, usize) = (3, 10);

/// The meander through `network` from its point nearest `start` to its point nearest `end`, the next
/// nearest where that is the first, its lines and pairs drawn from `rng`, as the points it passes. One
/// unit of `meter` for each point the searches reach and each line of the path looked through.
pub(crate) fn path(network: &Network, start: Point, end: Point, rng: &mut SplitMix64, meter: &mut Meter) -> Result<Vec<Point>, Exhausted> {
    let Some(first) = nearest(network, start, None) else { return Ok(Vec::new()) };
    let Some(last) = nearest(network, end, Some(first)) else { return Ok(vec![network.point(first)]) };
    Ok(Walk::wandered(network, first, last, rng, meter)?.map_or_else(|| vec![network.point(first)], |walk| walk.points()))
}

/// The point of `network` nearest `p`, but `except`, the first of those as near.
fn nearest(network: &Network, p: Point, except: Option<usize>) -> Option<usize> {
    let mut best: Option<(usize, f64)> = None;
    for (i, q) in network.points.iter().enumerate() {
        let d = q.distance(p);
        if Some(i) != except && best.is_none_or(|(_, near)| d < near) {
            best = Some((i, d));
        }
    }
    best.map(|(i, _)| i)
}

/// A path of fewest lines through `network` from `first` to `last`, as its points; `None` when none joins
/// them. Breadth first, each point's neighbours in order. One unit of `meter` for each point reached.
fn fewest_lines(network: &Network, first: usize, last: usize, meter: &mut Meter) -> Result<Option<Vec<usize>>, Exhausted> {
    let mut from: Vec<Option<usize>> = vec![None; network.len()];
    let mut queue = std::collections::VecDeque::from([first]);
    let mut seen = vec![false; network.len()];
    if let Some(s) = seen.get_mut(first) {
        *s = true;
    }
    while let Some(p) = queue.pop_front() {
        meter.charge(1)?;
        if p == last {
            let mut points = vec![last];
            let mut at = last;
            while let Some(before) = from.get(at).copied().flatten() {
                meter.charge(1)?;
                points.push(before);
                at = before;
            }
            points.reverse();
            return Ok(Some(points));
        }
        for n in network.neighbours(p) {
            if let Some(s) = seen.get_mut(n).filter(|s| !**s) {
                *s = true;
                if let Some(f) = from.get_mut(n) {
                    *f = Some(p);
                }
                queue.push_back(n);
            }
        }
    }
    Ok(None)
}

/// A meander under way: the lines of its path, in order, and the points it has passed.
struct Walk<'n> {
    network: &'n Network,
    passed: Vec<bool>,
    lines: Vec<(usize, usize)>,
}

impl<'n> Walk<'n> {
    /// The meander through `network` from `first` to `last`, wandered as far as it goes; `None` when no
    /// path joins them.
    fn wandered(network: &'n Network, first: usize, last: usize, rng: &mut SplitMix64, meter: &mut Meter) -> Result<Option<Walk<'n>>, Exhausted> {
        let Some(fewest) = fewest_lines(network, first, last, meter)? else { return Ok(None) };
        let lines = fewest.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
        let mut walk = Walk { network, passed: vec![false; network.len()], lines };
        for &p in &fewest {
            walk.pass(p);
        }
        walk.wander(rng, meter)?;
        Ok(Some(walk))
    }

    /// The points the path passes, in order.
    fn points(&self) -> Vec<Point> {
        let mut points: Vec<Point> = self.lines.iter().map(|&(a, _)| self.network.point(a)).collect();
        points.extend(self.lines.last().map(|&(_, b)| self.network.point(b)));
        points
    }

    /// Marks point `p` passed.
    fn pass(&mut self, p: usize) {
        if let Some(passed) = self.passed.get_mut(p) {
            *passed = true;
        }
    }

    /// Lengthens the path by detours until no line and no pair of lines can be replaced: lines drawn from
    /// a list of them, which each detour's lines join, and when it is empty, pairs of lines one after the
    /// other, the first replaced putting its detour's lines in the list.
    fn wander(&mut self, rng: &mut SplitMix64, meter: &mut Meter) -> Result<(), Exhausted> {
        let mut drawn = self.lines.clone();
        loop {
            while let Some((a, b)) = pop_random(&mut drawn, rng) {
                if let Some(detour) = self.detour(a, b, None, LINE, meter)? {
                    drawn.extend(self.replace(&[(a, b)], &detour, meter)?);
                }
            }
            let mut pairs: Vec<[(usize, usize); 2]> = self.lines.windows(2).filter_map(|w| Some([*w.first()?, *w.get(1)?])).collect();
            let mut replaced = false;
            while let Some([one, two]) = pop_random(&mut pairs, rng) {
                if let Some(detour) = self.detour(one.0, two.1, Some(one.1), PAIR, meter)? {
                    drawn.extend(self.replace(&[one, two], &detour, meter)?);
                    replaced = true;
                    break;
                }
            }
            if !replaced {
                return Ok(());
            }
        }
    }

    /// The first way from `a` to `b` a depth-first search finds through points not passed, of `fewest` to
    /// `most` lines, as its points, trying the points farthest from `b` first; `skipped`, the point between
    /// a pair's lines, it does not pass.
    fn detour(
        &self,
        a: usize,
        b: usize,
        skipped: Option<usize>,
        (fewest, most): (usize, usize),
        meter: &mut Meter,
    ) -> Result<Option<Vec<usize>>, Exhausted> {
        let target = self.network.point(b);
        // Each point's neighbours a way may go on to, farthest from the target first.
        let onwards = |p: usize, way: &[usize]| -> Vec<usize> {
            let mut next: Vec<usize> = self
                .network
                .neighbours(p)
                .filter(|&n| Some(n) != skipped && !way.contains(&n) && (n == b || !self.passed.get(n).copied().unwrap_or(true)))
                .collect();
            next.sort_by(|&x, &y| self.network.point(y).distance(target).total_cmp(&self.network.point(x).distance(target)).then(x.cmp(&y)));
            next
        };
        let mut way = vec![a];
        let mut stack = vec![onwards(a, &way).into_iter()];
        while let Some(choices) = stack.last_mut() {
            meter.charge(1)?;
            let Some(n) = choices.next() else {
                stack.pop();
                way.pop();
                continue;
            };
            if n == b {
                if way.len() >= fewest {
                    way.push(b);
                    return Ok(Some(way));
                }
                continue;
            }
            // On to `n` the way has as many lines as it had points, and needs one more to reach `b`.
            if way.len() < most {
                way.push(n);
                let more = onwards(n, &way);
                stack.push(more.into_iter());
            }
        }
        Ok(None)
    }

    /// Replaces the lines `old`, one after the other along the path, by the way `detour`, whose points but
    /// its ends are passed from now on; the detour's lines. One unit of `meter` for each line looked
    /// through.
    fn replace(&mut self, old: &[(usize, usize)], detour: &[usize], meter: &mut Meter) -> Result<Vec<(usize, usize)>, Exhausted> {
        let Some(&first) = old.first() else { return Ok(Vec::new()) };
        let mut at = None;
        for (i, &line) in self.lines.iter().enumerate() {
            meter.charge(1)?;
            if line == first {
                at = Some(i);
                break;
            }
        }
        let Some(at) = at else { return Ok(Vec::new()) };
        let lines: Vec<(usize, usize)> = detour.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
        let end = (at + old.len()).min(self.lines.len());
        self.lines.splice(at..end, lines.iter().copied());
        for &p in detour {
            self.pass(p);
        }
        Ok(lines)
    }
}

/// An item of `items` taken out at random, as Ink/Stitch takes one: the draw's place along the list,
/// rounded, the last item moved to its place.
fn pop_random<T>(items: &mut Vec<T>, rng: &mut SplitMix64) -> Option<T> {
    let last = items.len().checked_sub(1)?;
    // The draw lies in [0, 1), so the place is a whole number from 0 to `last`, which `as` keeps.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
    let place = ((rng.next_f64() * last as f64).round() as usize).min(last);
    Some(items.swap_remove(place))
}

#[cfg(test)]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]
mod tests {
    use std::collections::BTreeSet;

    use stitchcraft_core::Budget;

    use super::*;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A grid of `w` by `h` points 1 apart, each joined to the points beside, above and below it.
    fn grid(w: usize, h: usize) -> Network {
        let mut network = Network::default();
        for i in 0..w {
            for j in 0..h {
                network.points.push(p(i as f64, j as f64));
                let mut next = BTreeSet::new();
                let at = |i: usize, j: usize| i * h + j;
                if i > 0 {
                    next.insert(at(i - 1, j));
                }
                if i + 1 < w {
                    next.insert(at(i + 1, j));
                }
                if j > 0 {
                    next.insert(at(i, j - 1));
                }
                if j + 1 < h {
                    next.insert(at(i, j + 1));
                }
                network.next.push(next);
            }
        }
        network
    }

    #[test]
    fn req_fill_mea_003_the_path_passes_no_point_twice_from_the_start_s_nearest_to_the_end_s() {
        let network = grid(8, 6);
        let meter = &mut Budget::DEFAULT.meter();
        let points = path(&network, p(-1.0, -1.0), p(9.0, 9.0), &mut SplitMix64::new(7), meter).unwrap();
        assert_eq!((points.first(), points.last()), (Some(&p(0.0, 0.0)), Some(&p(7.0, 5.0))));
        let distinct: BTreeSet<(i64, i64)> = points.iter().map(|q| (q.x() as i64, q.y() as i64)).collect();
        assert_eq!(distinct.len(), points.len(), "no point twice");
        // Each step is a line of the network, and the path wanders through most of it.
        assert!(points.windows(2).all(|w| (w[0].distance(w[1]) - 1.0).abs() < 1e-12));
        assert!(points.len() * 10 >= 48 * 8, "{} of 48 points", points.len());
        // The same seed gives the same path, and another seed another.
        assert_eq!(path(&network, p(-1.0, -1.0), p(9.0, 9.0), &mut SplitMix64::new(7), meter).unwrap(), points);
        assert_ne!(path(&network, p(-1.0, -1.0), p(9.0, 9.0), &mut SplitMix64::new(8), meter).unwrap(), points);
    }

    #[test]
    fn no_line_of_the_finished_path_has_a_detour_left() {
        // When the meander is done, no line of it has a way round of 2 to 7 lines through points it hasn't
        // passed, and no 2 lines a way of 3 to 10.
        let network = grid(7, 7);
        let meter = &mut Budget::DEFAULT.meter();
        let walk = Walk::wandered(&network, 0, 42, &mut SplitMix64::new(3), meter).unwrap().unwrap();
        for &(a, b) in &walk.lines {
            assert_eq!(walk.detour(a, b, None, LINE, meter).unwrap(), None, "{a}-{b}");
        }
        for pair in walk.lines.windows(2) {
            assert_eq!(walk.detour(pair[0].0, pair[1].1, Some(pair[0].1), PAIR, meter).unwrap(), None);
        }
        assert_eq!((walk.lines[0].0, walk.lines.last().unwrap().1), (0, 42));
    }

    #[test]
    fn a_detour_is_the_first_way_found_farthest_from_the_end_first() {
        // On a 3 by 3 grid with the path along its bottom from (0, 0) to (1, 0), the way round goes up the
        // left side, along the top and down the right, then back to (1, 0) through the middle: 7 lines, the
        // most a line's detour has, and every point but (2, 0).
        let network = grid(3, 3);
        let mut passed = vec![false; 9];
        passed[0] = true;
        passed[3] = true;
        let walk = Walk { network: &network, passed, lines: vec![(0, 3)] };
        let meter = &mut Budget::DEFAULT.meter();
        assert_eq!(walk.detour(0, 3, None, LINE, meter).unwrap(), Some(vec![0, 1, 2, 5, 8, 7, 4, 3]));
        // Allowed 3 lines at most, the way round the square beside the line.
        assert_eq!(walk.detour(0, 3, None, (2, 3), meter).unwrap(), Some(vec![0, 1, 4, 3]));
        // Allowed only 2 lines, there is none.
        assert_eq!(walk.detour(0, 3, None, (2, 2), meter).unwrap(), None);
    }

    #[test]
    fn an_item_is_taken_at_the_draw_s_place_along_the_list_and_the_last_moved_there() {
        // Seed 1's first draw is 0.567, 2.27 places along a list of 5: the item at place 2 is taken.
        let mut items = vec![10, 11, 12, 13, 14];
        assert_eq!(pop_random(&mut items, &mut SplitMix64::new(1)), Some(12));
        assert_eq!(items, [10, 11, 14, 13]);
        assert_eq!(pop_random(&mut Vec::<usize>::new(), &mut SplitMix64::new(1)), None);
    }

    #[test]
    fn the_nearest_point_is_the_first_of_those_as_near_but_the_one_left_out() {
        // On a 3 by 3 grid, (0.5, 0) is as near (0, 0), point 0, as (1, 0), point 3: the first is taken.
        let network = grid(3, 3);
        assert_eq!(nearest(&network, p(0.5, 0.0), None), Some(0));
        // Leaving (0, 0) out, its neighbours (0, 1) and (1, 0) are as near it: (0, 1), point 1, is taken.
        assert_eq!(nearest(&network, p(0.0, 0.0), Some(0)), Some(1));
        assert_eq!(nearest(&Network::default(), p(0.0, 0.0), None), None);
    }

    #[test]
    fn where_no_path_joins_the_ends_the_path_is_the_start_s_point() {
        // 2 squares of 4 points 10 apart, neither joined to the other: from one to the other, no way.
        let mut network = grid(2, 2);
        for i in 0..4 {
            let (point, next) = (network.points[i], network.next[i].clone());
            network.points.push(p(point.x() + 10.0, point.y()));
            network.next.push(next.into_iter().map(|n| n + 4).collect());
        }
        let meter = &mut Budget::DEFAULT.meter();
        assert_eq!(path(&network, p(0.0, 0.0), p(11.0, 1.0), &mut SplitMix64::new(1), meter).unwrap(), [p(0.0, 0.0)]);
        assert_eq!(fewest_lines(&network, 0, 7, meter).unwrap(), None);
        assert_eq!(fewest_lines(&network, 0, 3, meter).unwrap(), Some(vec![0, 1, 3]));
    }
}
