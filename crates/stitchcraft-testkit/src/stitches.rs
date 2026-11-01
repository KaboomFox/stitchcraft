//! What a plan sews, read back from its stitches, for the cases that check a test sheet against its drawing
//! (`docs/src/plan/machine-testing.md` › TS-13): where each element's needle goes in, the rungs of a satin
//! column, and the jump threads that a machine cutting no jump leaves on the fabric.
//!
//! **Needle points.** An element's top stitches, in sewing order, are cut into runs wherever the needle
//! jumps, trims or stops, the thread changes, or a stitch of another element or role comes between
//! ([`top_runs`]). Plan assembly gives all of an element's stitches but its locks the top stitches' role
//! for now, its underlay's too, so a run holds the element's underlay before its top stitches.
//!
//! **Rungs.** A satin column's stitches zigzag from rail to rail: one crosses the column at a place along it,
//! its rung, and the next comes back across it to the next place, a step further along. Each stitch turns
//! back from the one before by more than 135°, across a column wider than the step; a stitch along a rail
//! turns by less, and isn't part of the zigzag. So of 2 stitches in turn the rung is the one that advances
//! less along the column: along the way its 2 rails run there, from each of its ends to the needle point 2
//! further on the same rail. In each stretch of at least 4 stitches that each turn back so, the rungs are
//! every other stitch: those that advance less than the stitch after them more often than the others do,
//! the first stitch's on a tie ([`rungs`]). A shorter stretch is no satin: an underlay that runs to a
//! column's end and back turns back once. Where a column tapers to a point, its rungs shorten as they go,
//! and are still the stitches that advance least.
//!
//! **Jump threads.** The reference machine cuts the thread at each thread change and stop, but at no jump
//! (MC-1), and the tester snips the jump threads at each change and stop (`machine-testing.md` › Standard
//! setup). So a jump leaves a thread on the fabric from where the needle last went in to where it goes in
//! again, and each stitch of the same colour block sewn after it and before a stop that crosses it sews it
//! down ([`jump_threads`]). A stitch that only touches it, as the stitch from its end does, leaves it free.

use stitchcraft_core::Point;
use stitchcraft_plan::{Role, StitchKind, StitchPlan};

/// An element's top stitches sewn without a break ([`top_runs`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Run {
    /// The element's id.
    pub element: String,
    /// Where the needle goes in, in sewing order.
    pub needles: Vec<Point>,
}

/// `plan`'s top stitches, each element's cut into runs where the needle jumps, trims or stops, the thread
/// changes, or a stitch of another element or role comes between. A run holds every needle point of its
/// stitches, the first included.
pub fn top_runs(plan: &StitchPlan) -> Vec<Run> {
    let mut runs: Vec<Run> = Vec::new();
    for block in &plan.blocks {
        let mut open = false;
        for stitch in &block.stitches {
            let element = stitch.origin.element.and_then(|e| plan.elements.get(e.index()));
            match (stitch.kind, stitch.origin.role, element) {
                (StitchKind::Normal, Role::Top, Some(id)) => match runs.last_mut() {
                    Some(run) if open && run.element == id.as_str() => run.needles.push(stitch.at),
                    _ => {
                        runs.push(Run { element: id.as_str().to_string(), needles: vec![stitch.at] });
                        open = true;
                    }
                },
                _ => open = false,
            }
        }
    }
    runs
}

/// The rungs of the satin column whose top needle points are `needles`, in sewing order: every other stitch
/// of each stretch of at least [`ZIGZAG`] stitches that zigzags, those that advance less along the column
/// than the stitch after them more often (see the module's docs).
pub fn rungs(needles: &[Point]) -> Vec<[Point; 2]> {
    let stitches: Vec<[Point; 2]> = needles.windows(2).filter_map(|pair| Some([*pair.first()?, *pair.get(1)?])).collect();
    let mut rungs = Vec::new();
    for zigzag in zigzags(&stitches) {
        if zigzag.len() < ZIGZAG {
            continue;
        }
        let along: Vec<f64> = (0..zigzag.len()).map(|k| advance(zigzag, k)).collect();
        // Each stitch and the next vote for the place, even or odd, of the one that advances less.
        let odd = along.windows(2).enumerate().filter(|(k, pair)| (k % 2 == 1) == (pair[0] < pair[1])).count();
        let even = zigzag.len() - 1 - odd;
        rungs.extend(zigzag.iter().skip(usize::from(odd > even)).step_by(2));
    }
    rungs
}

/// How far stitch `k` of `zigzag` advances along the column, less than 0 going back: along the way its 2
/// rails run there, from each of its ends to the needle point 2 further on the same rail, or 2 back at the
/// zigzag's end. 0 where the rails go nowhere.
fn advance(zigzag: &[[Point; 2]], k: usize) -> f64 {
    let points: Vec<Point> = zigzag.first().map(|first| first[0]).into_iter().chain(zigzag.iter().map(|stitch| stitch[1])).collect();
    let step = |i: usize| match (points.get(i), points.get(i + 2)) {
        (Some(&from), Some(&to)) => direction([from, to]),
        _ => points.get(i.wrapping_sub(2)).zip(points.get(i)).map_or((0.0, 0.0), |(&from, &to)| direction([from, to])),
    };
    let (a, b) = (step(k), step(k + 1));
    let way = (a.0 + b.0, a.1 + b.1);
    let (x, y) = zigzag.get(k).map_or((0.0, 0.0), |stitch| direction(*stitch));
    let length = stitchcraft_core::math::hypot(way.0, way.1);
    if length == 0.0 { 0.0 } else { (x * way.0 + y * way.1) / length }
}

/// The fewest stitches in turn, each turning back from the one before, that are a satin's zigzag.
pub const ZIGZAG: usize = 4;

/// `stitches` cut into stretches in which each stitch turns back from the one before it by more than 135°:
/// against its direction more than across it.
fn zigzags(stitches: &[[Point; 2]]) -> Vec<&[[Point; 2]]> {
    let mut stretches = Vec::new();
    let mut start = 0;
    for k in 1..=stitches.len() {
        let turns_back = stitches.get(k - 1).zip(stitches.get(k)).is_some_and(|(before, stitch)| {
            let (u, v) = (direction(*before), direction(*stitch));
            u.0 * v.0 + u.1 * v.1 < -(u.0 * v.1 - u.1 * v.0).abs()
        });
        if !turns_back {
            stretches.extend(stitches.get(start..k));
            start = k;
        }
    }
    stretches
}

/// How far `stitch` goes, along x and along y.
fn direction([from, to]: [Point; 2]) -> (f64, f64) {
    (to.x() - from.x(), to.y() - from.y())
}

/// A jump thread left on the fabric (see the module's docs).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct JumpThread {
    /// Where the needle last went in before the jump, and where it goes in again.
    pub ends: [Point; 2],
    /// How many stops the machine made before the jump: a test sheet's half as drawn sews before its stop.
    pub after_stops: usize,
    /// How many stitches of the jump's colour block, sewn after it and before a stop, cross it.
    pub sewn_down: usize,
}

/// The jump threads `plan` leaves on the reference machine, in sewing order (see the module's docs).
pub fn jump_threads(plan: &StitchPlan) -> Vec<JumpThread> {
    let mut threads = Vec::new();
    let mut stops = 0;
    for block in &plan.blocks {
        // The stitches sewn since the block began or the last stop, and the jump threads left meanwhile,
        // each with how many of those stitches were sewn before it.
        let (mut sewn, mut left): (Vec<[Point; 2]>, Vec<(JumpThread, usize)>) = (Vec::new(), Vec::new());
        // Where the needle last went in while the thread runs from the fabric, and where it went in before a
        // jump, while it jumps.
        let (mut needle, mut out_at): (Option<Point>, Option<Point>) = (None, None);
        for stitch in &block.stitches {
            match stitch.kind {
                StitchKind::Normal => {
                    if let Some(from) = out_at.take().filter(|&from| from != stitch.at) {
                        left.push((JumpThread { ends: [from, stitch.at], after_stops: stops, sewn_down: 0 }, sewn.len()));
                    } else if let Some(from) = needle {
                        sewn.push([from, stitch.at]);
                    }
                    needle = Some(stitch.at);
                }
                StitchKind::Jump => {
                    out_at = out_at.or(needle);
                    needle = None;
                }
                // The reference machine cuts no jump, trimmed or not.
                StitchKind::Trim => {}
                StitchKind::Stop => {
                    threads.extend(settled(&mut left, &mut sewn));
                    (needle, out_at, stops) = (None, None, stops + 1);
                }
            }
        }
        threads.extend(settled(&mut left, &mut sewn));
    }
    threads
}

/// The jump threads `left`, each sewn down by the stitches of `sewn` from the one it was left before; both
/// emptied, as the tester snips the threads and no later stitch reaches them.
fn settled(left: &mut Vec<(JumpThread, usize)>, sewn: &mut Vec<[Point; 2]>) -> Vec<JumpThread> {
    let threads = left
        .drain(..)
        .map(|(thread, from)| JumpThread {
            sewn_down: sewn.get(from..).unwrap_or_default().iter().filter(|s| crosses(**s, thread.ends)).count(),
            ..thread
        })
        .collect();
    sewn.clear();
    threads
}

/// Whether the segments `a` and `b` cross: each has an end on either side of the other's line. Segments
/// that only touch, or lie along one line, don't.
fn crosses(a: [Point; 2], b: [Point; 2]) -> bool {
    let side = |[p, q]: [Point; 2], r: Point| (q.x() - p.x()) * (r.y() - p.y()) - (q.y() - p.y()) * (r.x() - p.x());
    side(a, b[0]) * side(a, b[1]) < 0.0 && side(b, a[0]) * side(b, a[1]) < 0.0
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::ElementId;
    use stitchcraft_plan::{ColorBlock, ElementRef, Provenance, Rgb, Stitch, Thread};

    use super::*;
    use crate::designs::p;

    /// An entry of `kind` at `(x, y)`, of the element at `element` in `role`, or of the plan itself.
    fn entry(kind: StitchKind, (x, y): (f64, f64), element: Option<usize>, role: Role) -> Stitch {
        let origin = match element.and_then(ElementRef::from_index) {
            Some(element) => Provenance::new(element, role),
            None => Provenance::plan(role),
        };
        Stitch { at: p(x, y), kind, origin }
    }

    /// Normal top stitches of the element at `element`, at `points`.
    fn sewn(element: usize, points: &[(f64, f64)]) -> Vec<Stitch> {
        points.iter().map(|&at| entry(StitchKind::Normal, at, Some(element), Role::Top)).collect()
    }

    /// A jump to `at`, of no element.
    fn jump(at: (f64, f64)) -> Stitch {
        entry(StitchKind::Jump, at, None, Role::Travel)
    }

    /// A plan of `blocks`, each one thread's entries, of the elements `a` and `b`.
    fn plan(blocks: Vec<Vec<Stitch>>) -> StitchPlan {
        StitchPlan {
            blocks: blocks.into_iter().map(|stitches| ColorBlock { thread: Thread::new(Rgb::new(0, 0, 0)), stitches }).collect(),
            elements: vec![ElementId::new("a").unwrap(), ElementId::new("b").unwrap()],
        }
    }

    /// The points `(x, y)`.
    fn points(at: &[(f64, f64)]) -> Vec<Point> {
        at.iter().map(|&(x, y)| p(x, y)).collect()
    }

    #[test]
    fn an_elements_top_stitches_run_until_anything_else_comes_between() {
        let mut first = sewn(0, &[(0.0, 0.0), (1.0, 0.0)]);
        // An underlay stitch of the same element, then its top stitches again: a run of their own.
        first.push(entry(StitchKind::Normal, (1.0, 1.0), Some(0), Role::Underlay));
        first.extend(sewn(0, &[(2.0, 0.0), (3.0, 0.0)]));
        // A jump of the element's own, then the element again: another run.
        first.push(entry(StitchKind::Jump, (5.0, 0.0), Some(0), Role::Top));
        first.extend(sewn(0, &[(5.0, 0.0)]));
        // Another element straight after, a stitch of no element between 2 of its own, and the element again
        // after a thread change: runs of their own.
        first.extend(sewn(1, &[(6.0, 0.0)]));
        first.push(entry(StitchKind::Normal, (7.0, 0.0), None, Role::Top));
        first.extend(sewn(1, &[(8.0, 0.0)]));
        let runs = top_runs(&plan(vec![first, sewn(1, &[(9.0, 0.0), (9.5, 0.0)])]));
        let got: Vec<(&str, usize)> = runs.iter().map(|run| (run.element.as_str(), run.needles.len())).collect();
        assert_eq!(got, [("a", 2), ("a", 2), ("a", 1), ("b", 1), ("b", 1), ("b", 2)]);
        assert_eq!(runs[1].needles, points(&[(2.0, 0.0), (3.0, 0.0)]));
    }

    #[test]
    fn a_satins_rungs_are_the_stitches_that_advance_least_along_it() {
        // Rails 4 mm apart, y = 0 and y = 4, rungs every millimetre: across at x, back to the other rail at
        // x + 1.
        let ladder: Vec<(f64, f64)> = (0..6).flat_map(|k| [(f64::from(k), 0.0), (f64::from(k), 4.0)]).collect();
        let found = rungs(&points(&ladder));
        assert_eq!(found.len(), 6);
        assert!(found.iter().all(|[a, b]| a.x() == b.x()), "{found:?}");
        // Started on its way back instead, the rungs are its odd stitches.
        let found = rungs(&points(&ladder[1..]));
        assert_eq!(found.len(), 5);
        assert!(found.iter().all(|[a, b]| a.x() == b.x()), "{found:?}");
        // 4 stitches zigzag; 3, as an underlay's turn at a column's end, don't.
        assert_eq!(rungs(&points(&ladder[..5])), [[p(0.0, 0.0), p(0.0, 4.0)], [p(1.0, 0.0), p(1.0, 4.0)]]);
        assert!(rungs(&points(&ladder[..4])).is_empty());
        // A column that tapers towards a point: its rungs shorten, and still advance least.
        let point: Vec<(f64, f64)> = (0..5).flat_map(|k| [(f64::from(k), 0.4 * f64::from(k)), (f64::from(k), 4.0 - 0.4 * f64::from(k))]).collect();
        let found = rungs(&points(&point));
        assert_eq!(found.len(), 5);
        assert!(found.iter().all(|[a, b]| a.x() == b.x()), "{found:?}");
    }

    #[test]
    fn a_satin_cut_by_a_stitch_that_doesnt_turn_back_has_rungs_on_either_side() {
        // A rung, the way back, a rung and the way back to (2, 0), then a stitch square to that one, which
        // doesn't turn back from it: it starts a stretch of its own, and is no rung. The ladder goes on from
        // where it ends.
        let needles =
            points(&[(0.0, 0.0), (0.0, 4.0), (1.0, 0.0), (1.0, 4.0), (2.0, 0.0), (6.0, 1.0), (6.0, 5.0), (7.0, 1.0), (7.0, 5.0), (8.0, 1.0)]);
        let found = rungs(&needles);
        assert_eq!(found, [[p(0.0, 0.0), p(0.0, 4.0)], [p(1.0, 0.0), p(1.0, 4.0)], [p(6.0, 1.0), p(6.0, 5.0)], [p(7.0, 1.0), p(7.0, 5.0)]]);
    }

    #[test]
    fn a_stitch_that_turns_by_135_degrees_or_less_starts_a_stretch_of_its_own() {
        // Rungs up and stitches back, the last straight down, then a stitch at 135° to it: not part of the
        // zigzag, so no rung.
        let zigzag = [(0.0, 0.0), (0.0, 4.0), (1.0, 0.0), (1.0, 4.0), (1.0, 0.0)];
        let rungs_of = |last: (f64, f64)| rungs(&points(&[&zigzag[..], &[last]].concat()));
        let both = [[p(0.0, 0.0), p(0.0, 4.0)], [p(1.0, 0.0), p(1.0, 4.0)]];
        assert_eq!(rungs_of((5.0, 4.0)), both);
        // The same turning the other way, by less: a stitch along a rail after the way back.
        assert_eq!(rungs_of((-3.0, 1.0)), both);
        // Stitches that turn by 143° each time, more than 135°, zigzag.
        assert_eq!(rungs(&points(&[(0.0, 0.0), (1.0, 2.0), (-1.0, 1.0), (0.0, 3.0), (-2.0, 2.0)])).len(), 2);
    }

    #[test]
    fn a_tie_takes_the_place_of_the_first_stitch_and_a_lone_stitch_has_no_rung() {
        // Rails 10 mm apart, each stitch advancing half a millimetre more than the one before: 2 votes each
        // way.
        let needles = points(&[(0.0, 0.0), (0.0, 10.0), (0.5, 0.0), (1.5, 10.0), (3.0, 0.0), (5.0, 10.0)]);
        assert_eq!(rungs(&needles), [[p(0.0, 0.0), p(0.0, 10.0)], [p(0.5, 0.0), p(1.5, 10.0)], [p(3.0, 0.0), p(5.0, 10.0)]]);
        // Each advancing less than the one before: 2 votes for the odd ones, 1 for the even.
        let needles = points(&[(5.0, 10.0), (3.0, 0.0), (1.5, 10.0), (0.5, 0.0), (0.0, 10.0)]);
        assert_eq!(rungs(&needles), [[p(3.0, 0.0), p(1.5, 10.0)], [p(0.5, 0.0), p(0.0, 10.0)]]);
        // Of 2 that advance as far, the second counts as advancing less: a zigzag of equal stitches, and
        // stitches back and forth along one line, whose rails go nowhere, the odd.
        let needles = points(&[(0.0, 0.0), (1.0, 10.0), (2.0, 0.0), (3.0, 10.0), (4.0, 0.0)]);
        assert_eq!(rungs(&needles), [[p(1.0, 10.0), p(2.0, 0.0)], [p(3.0, 10.0), p(4.0, 0.0)]]);
        let needles = points(&[(0.0, 0.0), (0.0, 2.0), (0.0, 0.0), (0.0, 2.0), (0.0, 0.0)]);
        assert_eq!(rungs(&needles), [[p(0.0, 2.0), p(0.0, 0.0)], [p(0.0, 2.0), p(0.0, 0.0)]]);
        // A stitch on its own, and stitches that run on, make no zigzag.
        assert!(rungs(&points(&[(0.0, 0.0), (1.0, 0.0)])).is_empty());
        assert!(rungs(&points(&[(0.0, 0.0), (1.0, 0.0), (2.0, 0.0), (2.0, 1.0), (3.0, 1.0)])).is_empty());
        assert!(rungs(&points(&[(0.0, 0.0)])).is_empty());
    }

    #[test]
    fn a_jump_leaves_a_thread_that_later_stitches_of_its_colour_sew_down() {
        // A stitch, a jump 10 mm along x, then stitches back across the jump's path at x = 5 and x = 3, and
        // one that ends on it.
        let mut stitches = sewn(0, &[(0.0, 0.0), (0.0, 1.0)]);
        stitches.push(jump((5.0, 0.0)));
        stitches.push(entry(StitchKind::Trim, (5.0, 0.0), None, Role::Command));
        stitches.extend(sewn(1, &[(10.0, 1.0), (5.0, 3.0), (5.0, -1.0), (3.0, 2.0), (3.0, 1.0)]));
        let threads = jump_threads(&plan(vec![stitches.clone()]));
        // From where the needle last went in, through the trim the machine doesn't make, to where it goes
        // in again. The stitch from there, and the last one, only touch it.
        assert_eq!(threads, [JumpThread { ends: [p(0.0, 1.0), p(10.0, 1.0)], after_stops: 0, sewn_down: 2 }]);
        // The stitches across it in another colour block, after the thread change, sew nothing down.
        let (first, second) = stitches.split_at(5);
        let threads = jump_threads(&plan(vec![first.to_vec(), second.to_vec()]));
        assert_eq!(threads, [JumpThread { ends: [p(0.0, 1.0), p(10.0, 1.0)], after_stops: 0, sewn_down: 0 }]);
        // A jump at the end of a block leaves none: the machine cuts the thread at the change.
        let (first, second) = stitches.split_at(4);
        assert!(jump_threads(&plan(vec![first.to_vec(), second.to_vec()])).is_empty());
    }

    #[test]
    fn a_stop_ends_the_jump_threads_before_it_and_a_jump_from_the_start_leaves_none() {
        // A jump to the first stitch leaves no thread, nor does one that goes nowhere. After a jump along
        // y = 1, a stop, then a stitch across the jump's path: snipped at the stop, it is sewn down by
        // nothing. After the stop, a jump from the cut thread leaves none, and the next one a thread after 1
        // stop.
        let mut stitches = vec![jump((0.0, 0.0))];
        stitches.extend(sewn(0, &[(0.0, 0.0), (0.0, 1.0)]));
        stitches.push(jump((0.0, 1.0)));
        stitches.extend(sewn(0, &[(0.0, 1.0)]));
        stitches.push(jump((4.0, 1.0)));
        stitches.extend(sewn(0, &[(4.0, 1.0)]));
        stitches.push(entry(StitchKind::Stop, (4.0, 1.0), None, Role::Command));
        stitches.push(jump((2.0, 0.0)));
        stitches.extend(sewn(1, &[(2.0, 0.0), (2.0, 2.0)]));
        stitches.push(jump((6.0, 2.0)));
        stitches.extend(sewn(1, &[(6.0, 2.0)]));
        let threads = jump_threads(&plan(vec![stitches]));
        assert_eq!(
            threads,
            [
                JumpThread { ends: [p(0.0, 1.0), p(4.0, 1.0)], after_stops: 0, sewn_down: 0 },
                JumpThread { ends: [p(2.0, 2.0), p(6.0, 2.0)], after_stops: 1, sewn_down: 0 },
            ]
        );
        // Without the stop, the stitch across sews the first thread down, and touches the second.
        let mut stitches = sewn(0, &[(0.0, 1.0)]);
        stitches.push(jump((4.0, 1.0)));
        stitches.extend(sewn(0, &[(4.0, 1.0)]));
        stitches.push(jump((2.0, 0.0)));
        stitches.extend(sewn(1, &[(2.0, 0.0), (2.0, 2.0)]));
        let threads = jump_threads(&plan(vec![stitches]));
        let got: Vec<([Point; 2], usize)> = threads.iter().map(|thread| (thread.ends, thread.sewn_down)).collect();
        assert_eq!(got, [([p(0.0, 1.0), p(4.0, 1.0)], 1), ([p(4.0, 1.0), p(2.0, 0.0)], 0)]);
    }

    #[test]
    fn segments_cross_only_when_each_has_an_end_on_either_side_of_the_other() {
        let level = [p(0.0, 0.0), p(4.0, 0.0)];
        assert!(crosses([p(2.0, -1.0), p(2.0, 1.0)], level));
        assert!(crosses(level, [p(2.0, 1.0), p(2.0, -1.0)]));
        // On its line beyond its end: the first has its ends either side of the other, the other doesn't.
        assert!(!crosses([p(6.0, -1.0), p(6.0, 1.0)], level));
        assert!(!crosses(level, [p(6.0, -1.0), p(6.0, 1.0)]));
        // Touching at an end, from either side and either way round, or along the same line.
        for touching in [[p(2.0, 0.0), p(2.0, 1.0)], [p(2.0, -1.0), p(2.0, 0.0)], [p(2.0, 1.0), p(2.0, 0.0)], [p(2.0, 0.0), p(2.0, -1.0)]] {
            assert!(!crosses(touching, level), "{touching:?}");
            assert!(!crosses(level, touching), "{touching:?}");
        }
        assert!(!crosses([p(1.0, 0.0), p(5.0, 0.0)], level));
    }
}
