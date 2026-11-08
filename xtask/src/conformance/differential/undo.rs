//! Deviations undone in Ink/Stitch's file (`docs/src/design/conformance.md` › L3): the machine file Ink/Stitch
//! writes, as it would read had Ink/Stitch done as StitchCraft does where an entry of the deviations ledger
//! explains a difference ([`undone`]).
//!
//! **Why undo.** An excuse alone lets a measure differ by any amount. Measured again on the file undone, the
//! measure must agree, so an excuse explains the difference it names and hides no other. A deviation is
//! undone here when the file alone shows where it is, without Ink/Stitch's plan: a record pystitch writes, a
//! thread's name, a stitch fitted to the machine, a needle point sewn where nothing is drawn.

use stitchcraft_core::Point;
use stitchcraft_plan::{Rgb, Stitch, StitchKind, StitchPlan, profiles};

/// The deviation by which pystitch, writing Ink/Stitch's PES files, writes jumps that go nowhere.
pub const NOWHERE: &str = "DEV-FMT-001";
/// The deviation by which the 2 programs name a thread by other entries of Brother's palette.
pub const PALETTE: &str = "DEV-THREAD-001";
/// The deviation by which StitchCraft fits stitches to the machine: it leaves out those shorter than its
/// shortest and splits those longer than its longest.
pub const FITTED: &str = "DEV-FIN-001";
/// The deviation by which a move-to with nothing drawn after it is no subpath, where Ink/Stitch sews a
/// needle point there.
pub const LONE: &str = "DEV-SVG-011";

/// Ink/Stitch's file `theirs` with the deviation `deviation` undone, as the file would read had Ink/Stitch
/// done as StitchCraft does, whose file is `ours`; `None` for a deviation the differential can't undo,
/// which excuses its measures as they are.
///
/// - [`NOWHERE`]: each jump that goes nowhere at the start of a colour block or right after a stop, where
///   Ink/Stitch's plan has no trim, and the trim it carries, as every jump of a PES file does. StitchCraft
///   writes no such record (`REQ-FMT-010`). Within a block, Ink/Stitch's plan jumps where the needle is after
///   lock stitches an element forces (`force_lock_stitches`), and after a trim the plan asks for, which
///   pystitch writes alike. StitchCraft's file has the trims the plan asks for, so of the jumps that go
///   nowhere within blocks, the first ones are left out with their trims, as many as Ink/Stitch's file then
///   has trims more than StitchCraft's.
/// - [`PALETTE`]: each thread named by StitchCraft's entry, when both files change thread at the same
///   colour blocks: as many blocks, and 2 of them of one thread in one file just when they are in the other.
///   Then only the entries chosen differ. Otherwise the file stays as it is, and the threads still differ.
/// - [`FITTED`]: each needle point nearer the one before than the reference machine's shortest stitch,
///   0.3 mm, left out, but the first and the last of a run of stitches, and each stitch longer than its
///   longest, 12 mm, split into as few equal parts as are no longer, as StitchCraft fits them (`REQ-FIN-001`).
///   StitchCraft
///   keeps some needle points this leaves out, such as a lock's, so the stitches agree within their
///   tolerance and not exactly.
/// - [`LONE`]: each run of stitches that sews at one point, every needle point of it nearer its first than
///   the reference machine's shortest stitch, with the jumps that lead to it and the trim before them. A trim
///   or a stop where it was moves to the needle point before it. The jumps that go nowhere this leaves, as
///   after a thread change, are left out as for [`NOWHERE`], and the file is moved so the middle of its
///   bounds stays where it was, as Ink/Stitch centres a design.
pub fn undone(deviation: &str, theirs: &StitchPlan, ours: &StitchPlan) -> Option<StitchPlan> {
    let shortest = profiles::REFERENCE.min_stitch.get();
    match deviation {
        NOWHERE => Some(without_jumps_nowhere(theirs, ours)),
        PALETTE => Some(named_as(theirs, ours)),
        FITTED => Some(split(&without_short(theirs, shortest), profiles::REFERENCE.max_stitch.get())),
        LONE => Some(in_middle_of(without_jumps_nowhere(&without_lone_points(theirs, shortest), ours), theirs)),
        _ => None,
    }
}

/// `plan` without each run of stitches whose needle points all lie nearer its first than `shortest`
/// millimetres, nor the jumps that lead to it and the trim before them. A trim or a stop where such a run
/// was is moved to the needle point before it, or left out when there is none ([`undone`]).
fn without_lone_points(plan: &StitchPlan, shortest: f64) -> StitchPlan {
    let mut plan = plan.clone();
    let mut at = Lone { needle: None, left: None, shortest };
    for block in &mut plan.blocks {
        let mut kept: Vec<Stitch> = Vec::with_capacity(block.stitches.len());
        let mut run: Vec<Stitch> = Vec::new();
        for &stitch in &block.stitches {
            if stitch.kind == StitchKind::Normal {
                run.push(stitch);
                continue;
            }
            at.end(&mut kept, &mut run);
            match (at.left.filter(|&left| at.near(stitch.at, left)), at.needle) {
                (None, _) => kept.push(stitch),
                (Some(_), Some(needle)) => kept.push(Stitch { at: needle, ..stitch }),
                (Some(_), None) => {}
            }
        }
        at.end(&mut kept, &mut run);
        block.stitches = kept;
    }
    plan
}

/// Where [`without_lone_points`] is: the last needle point it kept, and the point of a run it just left out.
struct Lone {
    needle: Option<Point>,
    left: Option<Point>,
    shortest: f64,
}

impl Lone {
    /// Whether `a` lies nearer `b` than the shortest stitch.
    fn near(&self, a: Point, b: Point) -> bool {
        (a.x() - b.x()).hypot(a.y() - b.y()) < self.shortest
    }

    /// Ends the run of stitches `run`: added to `kept`, unless it sews at one point. Then it is left out, and
    /// so are the jumps at the end of `kept` that lead to it and the trim before them.
    fn end(&mut self, kept: &mut Vec<Stitch>, run: &mut Vec<Stitch>) {
        let Some(first) = run.first().map(|s| s.at) else {
            return;
        };
        if !run.iter().all(|s| self.near(s.at, first)) {
            self.needle = run.last().map(|s| s.at);
            self.left = None;
            kept.append(run);
            return;
        }
        run.clear();
        while kept.last().is_some_and(|s| s.kind == StitchKind::Jump) {
            kept.pop();
        }
        if kept.last().is_some_and(|s| s.kind == StitchKind::Trim) {
            kept.pop();
        }
        self.left = Some(first);
    }
}

/// `plan` moved so the middle of its bounds is where `was`'s is ([`undone`]).
fn in_middle_of(mut plan: StitchPlan, was: &StitchPlan) -> StitchPlan {
    let (Some(was), Some(is)) = (was.bounds(), plan.bounds()) else {
        return plan;
    };
    let (dx, dy) = (was.center().x() - is.center().x(), was.center().y() - is.center().y());
    for stitch in plan.blocks.iter_mut().flat_map(|block| block.stitches.iter_mut()) {
        if let Ok(at) = Point::new(stitch.at.x() + dx, stitch.at.y() + dy) {
            stitch.at = at;
        }
    }
    plan
}

/// `plan` without each needle point nearer the one before than `shortest` millimetres, but the first and the
/// last of a run of stitches ([`undone`]).
fn without_short(plan: &StitchPlan, shortest: f64) -> StitchPlan {
    let mut plan = plan.clone();
    for block in &mut plan.blocks {
        let mut kept: Vec<Stitch> = Vec::with_capacity(block.stitches.len());
        let mut needle: Option<Point> = None;
        let stitch_at = |i: Option<usize>| i.and_then(|i| block.stitches.get(i)).is_some_and(|s| s.kind == StitchKind::Normal);
        for (i, &stitch) in block.stitches.iter().enumerate() {
            let inside = stitch.kind == StitchKind::Normal && stitch_at(i.checked_sub(1)) && stitch_at(i.checked_add(1));
            let near = needle.is_some_and(|from| (stitch.at.x() - from.x()).hypot(stitch.at.y() - from.y()) < shortest);
            if inside && near {
                continue;
            }
            if !stitch.kind.is_command() {
                needle = Some(stitch.at);
            }
            kept.push(stitch);
        }
        block.stitches = kept;
    }
    plan
}

/// `theirs` with each block's thread `ours`' block's, when both change thread at the same blocks ([`undone`]).
fn named_as(theirs: &StitchPlan, ours: &StitchPlan) -> StitchPlan {
    let pattern = |plan: &StitchPlan| -> Vec<usize> {
        let colours: Vec<Rgb> = plan.blocks.iter().map(|block| block.thread.color).collect();
        colours.iter().map(|c| colours.iter().position(|first| first == c).unwrap_or_default()).collect()
    };
    let mut named = theirs.clone();
    if pattern(theirs) == pattern(ours) {
        for (block, our) in named.blocks.iter_mut().zip(&ours.blocks) {
            block.thread = our.thread.clone();
        }
    }
    named
}

/// `plan` with each stitch longer than `longest` millimetres split into as few equal parts as are no longer
/// ([`undone`]).
fn split(plan: &StitchPlan, longest: f64) -> StitchPlan {
    let mut plan = plan.clone();
    let mut needle: Option<Point> = None;
    for block in &mut plan.blocks {
        let mut kept: Vec<Stitch> = Vec::with_capacity(block.stitches.len());
        for &stitch in &block.stitches {
            if let (StitchKind::Normal, Some(from)) = (stitch.kind, needle) {
                let (dx, dy) = (stitch.at.x() - from.x(), stitch.at.y() - from.y());
                let parts = (dx.hypot(dy) / longest).ceil();
                // A count of parts, at most the stitch's length over the longest: well within usize.
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let parts = if parts.is_finite() && parts > 1.0 { parts as usize } else { 1 };
                for part in 1..parts {
                    #[allow(clippy::cast_precision_loss)] // A count of parts, far below 2^52.
                    let t = part as f64 / parts as f64;
                    if let Ok(at) = Point::new(from.x() + dx * t, from.y() + dy * t) {
                        kept.push(Stitch { at, ..stitch });
                    }
                }
            }
            if !stitch.kind.is_command() {
                needle = Some(stitch.at);
            }
            kept.push(stitch);
        }
        block.stitches = kept;
    }
    plan
}

/// `plan` without the jumps that go nowhere where Ink/Stitch's plan has no trim, nor the trims they carry
/// ([`undone`]): at the start of a colour block or right after a stop, and within a block the first ones, as
/// many as `plan` then has trims more than `ours`.
fn without_jumps_nowhere(plan: &StitchPlan, ours: &StitchPlan) -> StitchPlan {
    let started = dropping_nowhere(plan, |starts, _| starts);
    let mut more = started.stats().trims.saturating_sub(ours.stats().trims);
    dropping_nowhere(&started, |_, carried| {
        let dropped = carried && more > 0;
        more -= usize::from(dropped);
        dropped
    })
}

/// `plan` without each jump that goes nowhere that `drops` leaves out, nor the trim it carries, as every
/// jump of a PES file but the first does. `drops` hears whether the jump starts a colour block or comes right
/// after a stop, and whether it carries a trim.
fn dropping_nowhere(plan: &StitchPlan, mut drops: impl FnMut(bool, bool) -> bool) -> StitchPlan {
    let mut plan = plan.clone();
    let mut needle = None;
    for block in &mut plan.blocks {
        let mut kept: Vec<Stitch> = Vec::with_capacity(block.stitches.len());
        for &stitch in &block.stitches {
            if stitch.kind == StitchKind::Jump && needle == Some(stitch.at) {
                // The trim a PES file's jump carries, and what comes before it.
                let carried = kept.last().is_some_and(|before| before.kind == StitchKind::Trim);
                let before = kept.len().saturating_sub(usize::from(carried));
                let starts = kept.get(..before).and_then(<[_]>::last).is_none_or(|before| before.kind == StitchKind::Stop);
                if drops(starts, carried) {
                    kept.truncate(before);
                    continue;
                }
            }
            if !stitch.kind.is_command() {
                needle = Some(stitch.at);
            }
            kept.push(stitch);
        }
        block.stitches = kept;
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::super::{Measure, Measured, compare};
    use super::*;
    use crate::util;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn jumps_that_go_nowhere_where_the_plan_has_no_trim_are_undone_with_the_trims_they_carry() {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let (red, blue) = (Thread::new(Rgb::new(237, 23, 31)), Thread::new(Rgb::new(11, 61, 145)));
        let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
        // As StitchCraft reads Ink/Stitch's PES files back: every jump but the first carries a trim.
        let mut builder = PlanBuilder::new(red.clone());
        builder.jump(p(0.0, 0.0), travel);
        builder.stitch(p(0.0, 0.0), top);
        builder.stitch(p(10.0, 0.0), top);
        // A thread change where the needle is: undone.
        builder.change_thread(blue);
        builder.trim(None);
        builder.jump(p(10.0, 0.0), travel);
        builder.stitch(p(10.0, 5.0), top);
        // A stop where the needle is: undone.
        builder.stop(None);
        builder.trim(None);
        builder.jump(p(10.0, 5.0), travel);
        builder.stitch(p(10.0, 9.0), top);
        // A trim the plan asked for, and a jump that goes nowhere after it: StitchCraft writes it too.
        builder.trim(None);
        builder.jump(p(10.0, 9.0), travel);
        builder.stitch(p(14.0, 9.0), top);
        // A thread change that moves: kept.
        builder.change_thread(red);
        builder.trim(None);
        builder.jump(p(20.0, 9.0), travel);
        builder.stitch(p(20.0, 12.0), top);
        let plan = builder.finish();
        assert_eq!((plan.stats().trims, plan.stats().jumps), (4, 5));
        let without = undone(NOWHERE, &plan, &plan).unwrap();
        assert_eq!((without.stats().trims, without.stats().jumps), (2, 3));
        let kinds = |block: &stitchcraft_plan::ColorBlock| block.stitches.iter().map(|s| s.kind).collect::<Vec<_>>();
        let (n, j, t, s) = (StitchKind::Normal, StitchKind::Jump, StitchKind::Trim, StitchKind::Stop);
        assert_eq!(without.blocks.iter().map(kinds).collect::<Vec<_>>(), [vec![j, n, n], vec![n, s, n, t, j, n], vec![t, j, n]]);
        assert_eq!(without.stats().stitches, plan.stats().stitches);
        // A deviation the differential can't undo excuses as it is.
        assert_eq!(undone("DEV-LCK-001", &plan, &plan), None);
    }

    #[test]
    fn jumps_that_go_nowhere_within_a_block_are_undone_as_far_as_ink_stitch_s_file_trims_more() {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let red = Thread::new(Rgb::new(237, 23, 31));
        let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
        // As DejaVu's Ä in Ink/Stitch's file: twice, lock stitches an element forces, then a jump where the
        // needle is, which pystitch writes with a trim. Then a jump where the needle is without one.
        let mut builder = PlanBuilder::new(red.clone());
        builder.jump(p(0.0, 0.0), travel);
        builder.stitch(p(0.0, 0.0), top);
        builder.stitch(p(10.0, 0.0), top);
        for y in [5.0, 9.0] {
            builder.trim(None);
            builder.jump(builder.needle(), travel);
            builder.stitch(p(10.0, y), top);
        }
        builder.jump(p(10.0, 9.0), travel);
        builder.stitch(p(14.0, 9.0), top);
        let theirs = builder.finish();
        // StitchCraft's file, with `trims` trims where its needle moves.
        let ours = |trims: u32| {
            let mut builder = PlanBuilder::new(red.clone());
            builder.stitch(p(0.0, 0.0), top);
            for i in 1..=trims {
                builder.trim(None);
                builder.jump(p(f64::from(i), 0.0), travel);
                builder.stitch(p(f64::from(i), 0.0), top);
            }
            builder.finish()
        };
        let counts = |trims: u32| {
            let undone = undone(NOWHERE, &theirs, &ours(trims)).unwrap();
            (undone.stats().trims, undone.stats().jumps)
        };
        // With no trim in StitchCraft's file both go, with 1 the first goes, and with 2 or more neither.
        assert_eq!([0, 1, 2, 3].map(counts), [(0, 2), (1, 3), (2, 4), (2, 4)]);
        let kinds = |plan: &StitchPlan| plan.blocks[0].stitches.iter().map(|s| (s.kind, s.at.y())).collect::<Vec<_>>();
        let (n, j, t) = (StitchKind::Normal, StitchKind::Jump, StitchKind::Trim);
        assert_eq!(
            kinds(&undone(NOWHERE, &theirs, &ours(1)).unwrap()),
            [(j, 0.0), (n, 0.0), (n, 0.0), (n, 5.0), (t, 5.0), (j, 5.0), (n, 9.0), (j, 9.0), (n, 9.0)]
        );
        // A needle point Ink/Stitch sews alone is left out with what leads to it, then these jumps go as here.
        assert_eq!(undone(LONE, &theirs, &ours(0)).unwrap().stats().trims, 0);
    }

    #[test]
    fn every_deviation_the_differential_undoes_is_in_the_ledger() {
        let ledger = crate::deviations::parse(&util::read(&util::root().join(crate::deviations::LEDGER)).unwrap()).unwrap();
        assert!(ledger.iter().any(|d| d.id == NOWHERE && d.requirements.iter().any(|r| r == "REQ-FMT-010")));
        assert!(ledger.iter().any(|d| d.id == PALETTE && d.requirements.iter().any(|r| r == "REQ-THREAD-001")));
        assert!(ledger.iter().any(|d| d.id == FITTED && d.requirements.iter().any(|r| r == "REQ-FIN-001")));
        assert!(ledger.iter().any(|d| d.id == LONE && d.requirements.iter().any(|r| r == "REQ-SVG-001")));
    }

    /// A plan of a block for each of `threads`, each block a stitch at the origin and one `long` mm right.
    fn blocks(threads: &[Rgb], long: f64) -> StitchPlan {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let top = Provenance::plan(Role::Top);
        let mut builder = PlanBuilder::new(Thread::new(threads[0]));
        for (i, &thread) in threads.iter().enumerate() {
            if i > 0 {
                builder.change_thread(Thread::new(thread));
            }
            builder.stitch(p(0.0, 0.0), top);
            builder.stitch(p(long, 0.0), top);
        }
        builder.finish()
    }

    #[test]
    fn threads_named_by_other_entries_are_named_as_ours_when_they_change_at_the_same_blocks() {
        let (red, vermilion, blue, ultramarine) = (Rgb::new(237, 23, 31), Rgb::new(254, 55, 15), Rgb::new(11, 61, 145), Rgb::new(38, 38, 200));
        let ours = blocks(&[vermilion, blue, vermilion], 5.0);
        let theirs = blocks(&[red, ultramarine, red], 5.0);
        let named = undone(PALETTE, &theirs, &ours).unwrap();
        assert_eq!(named.blocks.iter().map(|b| b.thread.color).collect::<Vec<_>>(), [vermilion, blue, vermilion]);
        assert!(compare(&Measured::of(&ours).unwrap(), &Measured::of(&named).unwrap()).iter().find(|row| row.0 == Measure::Threads).unwrap().3);
        // Where they change thread at other blocks, or have more of them, the threads stay theirs.
        for other in [blocks(&[red, ultramarine, ultramarine], 5.0), blocks(&[red, ultramarine], 5.0), blocks(&[red, red, red], 5.0)] {
            assert_eq!(undone(PALETTE, &other, &ours).unwrap(), other);
        }
    }

    #[test]
    fn stitches_longer_than_the_machine_s_longest_are_split_into_equal_parts() {
        let red = Rgb::new(237, 23, 31);
        // 30 mm: 3 parts of 10 mm. 12 mm, the longest, stays whole.
        let split = undone(FITTED, &blocks(&[red], 30.0), &blocks(&[red], 30.0)).unwrap();
        let xs: Vec<f64> = split.blocks[0].stitches.iter().filter(|s| s.kind == StitchKind::Normal).map(|s| s.at.x()).collect();
        assert_eq!(xs, [0.0, 10.0, 20.0, 30.0]);
        let whole = blocks(&[red], 12.0);
        assert_eq!(undone(FITTED, &whole, &whole).unwrap(), whole);
        // A jump is never split, and the stitch after it is measured from where it went.
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let mut builder = PlanBuilder::new(Thread::new(red));
        builder.stitch(p(0.0, 0.0), Provenance::plan(Role::Top));
        builder.jump(p(50.0, 0.0), Provenance::plan(Role::Travel));
        builder.stitch(p(50.0, 0.0), Provenance::plan(Role::Top));
        builder.stitch(p(63.0, 0.0), Provenance::plan(Role::Top));
        let jumped = undone(FITTED, &builder.finish(), &whole).unwrap();
        let kinds: Vec<(StitchKind, f64)> = jumped.blocks[0].stitches.iter().map(|s| (s.kind, s.at.x())).collect();
        let (n, j) = (StitchKind::Normal, StitchKind::Jump);
        assert_eq!(kinds, [(n, 0.0), (j, 50.0), (n, 50.0), (n, 56.5), (n, 63.0)]);
    }

    #[test]
    fn a_run_that_sews_at_one_point_is_undone_with_the_jumps_and_trim_that_lead_to_it() {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let (red, blue) = (Thread::new(Rgb::new(237, 23, 31)), Thread::new(Rgb::new(11, 61, 145)));
        let (top, travel) = (Provenance::plan(Role::Top), Provenance::plan(Role::Travel));
        // As Kum Tsoan relief's V: a line, then a needle point 20 mm off where a lone move-to is, and the next
        // thread where the line ended.
        let mut builder = PlanBuilder::new(red);
        builder.jump(p(0.0, 0.0), travel);
        builder.stitch(p(0.0, 0.0), top);
        builder.stitch(p(10.0, 0.0), top);
        builder.trim(None);
        builder.jump(p(0.0, -20.0), travel);
        builder.stitch(p(0.0, -20.0), top);
        builder.stitch(p(0.1, -20.0), top);
        builder.change_thread(blue);
        builder.trim(None);
        builder.jump(p(10.0, 0.0), travel);
        builder.stitch(p(10.0, 0.0), top);
        builder.stitch(p(10.0, 10.0), top);
        let plan = builder.finish();
        let measured = Measured::of(&plan).unwrap();
        assert_eq!((measured.stitches, measured.size, measured.trims, measured.jumps), (6, [10.0, 30.0], 2, 3));
        let without = undone(LONE, &plan, &plan).unwrap();
        let kinds = |block: &stitchcraft_plan::ColorBlock| block.stitches.iter().map(|s| s.kind).collect::<Vec<_>>();
        let (n, j) = (StitchKind::Normal, StitchKind::Jump);
        assert_eq!(without.blocks.iter().map(kinds).collect::<Vec<_>>(), [vec![j, n, n], vec![n, n]]);
        // The middle of the bounds stays where it was: the line and the next thread move up by 10 mm.
        let measured = Measured::of(&without).unwrap();
        assert_eq!((measured.stitches, measured.size, measured.place, measured.trims, measured.jumps), (4, [10.0, 10.0], [5.0, -5.0], 0, 1));
        // Inside a block, the trim after the lone point cuts where the needle was before it.
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(237, 23, 31)));
        builder.stitch(p(0.0, 0.0), top);
        builder.stitch(p(10.0, 0.0), top);
        builder.trim(None);
        builder.jump(p(0.0, -20.0), travel);
        builder.stitch(p(0.0, -20.0), top);
        builder.trim(None);
        builder.jump(p(0.0, 10.0), travel);
        builder.stitch(p(0.0, 10.0), top);
        builder.stitch(p(10.0, 10.0), top);
        let without = undone(LONE, &builder.finish(), &plan).unwrap();
        let records: Vec<(StitchKind, f64, f64)> = without.blocks[0].stitches.iter().map(|s| (s.kind, s.at.x(), s.at.y() + 0.0)).collect();
        let t = StitchKind::Trim;
        // The middle of the bounds stays at y = -5 mm, so the lines at 0 and 10 mm move up to -10 and 0.
        assert_eq!(records, [(n, 0.0, -10.0), (n, 10.0, -10.0), (t, 10.0, -10.0), (j, 0.0, 0.0), (n, 0.0, 0.0), (n, 10.0, 0.0)]);
        // A run of more than one place stays.
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(237, 23, 31)));
        builder.stitch(p(0.0, 0.0), top);
        builder.stitch(p(0.3, 0.0), top);
        assert_eq!(undone(LONE, &builder.finish(), &plan).unwrap().stats().stitches, 2);
    }

    #[test]
    fn needle_points_nearer_than_the_machine_s_shortest_stitch_are_left_out_but_a_run_s_last() {
        use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};
        let top = Provenance::plan(Role::Top);
        let mut builder = PlanBuilder::new(Thread::new(Rgb::new(237, 23, 31)));
        for x in [0.0, 0.2, 0.25, 1.0, 1.2, 2.0, 2.1] {
            builder.stitch(p(x, 0.0), top);
        }
        builder.jump(p(9.0, 0.0), Provenance::plan(Role::Travel));
        builder.stitch(p(9.1, 0.0), top);
        builder.stitch(p(10.0, 0.0), top);
        let plan = builder.finish();
        let fitted = undone(FITTED, &plan, &plan).unwrap();
        let kept: Vec<(StitchKind, f64)> = fitted.blocks[0].stitches.iter().map(|s| (s.kind, s.at.x())).collect();
        let (n, j) = (StitchKind::Normal, StitchKind::Jump);
        // 0.2 and 0.25 lie within 0.3 mm of 0, and 1.2 of 1.0; 2.1 ends its run, and 9.1, 0.1 mm from where the
        // jump went, starts one.
        assert_eq!(kept, [(n, 0.0), (n, 1.0), (n, 2.0), (n, 2.1), (j, 9.0), (n, 9.1), (n, 10.0)]);
    }
}
