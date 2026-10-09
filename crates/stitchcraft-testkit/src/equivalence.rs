//! When two plans make a machine do the same thing.
//!
//! A machine file cannot say everything a plan says. PES spells a trim as a flag on the next move; DST
//! spells it as three jumps; long moves become several records; a stop is a colour change to the same
//! thread. So a round trip through a format does not give back the *same* plan — it gives back a plan
//! that makes the machine do the same thing. [`events`] is that "same thing": where the needle goes
//! down, where the thread is cut, where the machine pauses, in machine units (REQ-FMT-002, REQ-FMT-003).
//!
//! DST cannot even say every cut a plan makes, or leaves out: its machines cut the thread before three or
//! more jumps in a row (REQ-FMT-008). [`dst_events`] is what they do with a plan written as DST.
//!
//! Positions are rounded and DST records counted here with this module's own code, not the writers': an
//! oracle that shared the writers' arithmetic could not catch a bug in it.

use stitchcraft_core::Point;
use stitchcraft_formats::dst::{JUMPS_FOR_TRIM, RECORD_LIMIT};
use stitchcraft_plan::{StitchKind, StitchPlan};

/// Something a machine does that can be seen on the fabric or felt at the machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Event {
    /// The needle goes down here (0.1 mm units).
    Down(i32, i32),
    /// The thread is cut with the needle here.
    Cut(i32, i32),
    /// The machine pauses for a thread change or a stop with the needle here.
    Pause(i32, i32),
}

/// The machine-visible behaviour of `plan`, in a canonical order:
///
/// - jumps leave no event of their own (they only move where the next one happens);
/// - a thread change and a stop are both a pause (formats differ in which one they can record);
/// - between two needle-downs, cuts and pauses at one spot are unordered and a second cut is
///   redundant, so they are sorted and duplicate cuts dropped;
/// - cuts after the last needle-down change nothing on the fabric and are dropped.
pub fn events(plan: &StitchPlan) -> Vec<Event> {
    let mut raw = Vec::new();
    let mut needle = (0, 0);
    for (b, block) in plan.blocks.iter().enumerate() {
        if b > 0 {
            raw.push(Event::Pause(needle.0, needle.1));
        }
        for stitch in &block.stitches {
            let at = units(stitch.at);
            match stitch.kind {
                StitchKind::Normal => {
                    raw.push(Event::Down(at.0, at.1));
                    needle = at;
                }
                StitchKind::Jump => needle = at,
                StitchKind::Trim => raw.push(Event::Cut(needle.0, needle.1)),
                StitchKind::Stop => raw.push(Event::Pause(needle.0, needle.1)),
            }
        }
    }
    canonical(raw)
}

/// What a machine does with `plan` written as DST: [`events`], with the thread cut where DST machines cut
/// it. DST has no trim command; its machines cut the thread before every run of [`JUMPS_FOR_TRIM`] or more
/// jump records, where something was sewn since the thread was last cut or changed (REQ-FMT-008), and
/// StitchCraft spells a trim as such a run. So the thread is cut where such a run starts — before a jump
/// of three or more records too — and a trim with nothing sewn since the last cut or pause cuts nothing.
///
/// The records of a move of (dx, dy) are the fewest of at most [`RECORD_LIMIT`] along each axis; a jump
/// that goes nowhere takes none, a trim [`JUMPS_FOR_TRIM`], and a sewn move's last record sews, so the
/// others are jumps. A pause is a colour-change record, which ends a run of jumps.
pub fn dst_events(plan: &StitchPlan) -> Vec<Event> {
    let mut machine = DstMachine::default();
    for (b, block) in plan.blocks.iter().enumerate() {
        if b > 0 {
            machine.pause();
        }
        for stitch in &block.stitches {
            let at = units(stitch.at);
            let records = dst_records(at.0 - machine.needle.0, at.1 - machine.needle.1);
            match stitch.kind {
                StitchKind::Normal => {
                    machine.jumps(records - 1);
                    machine.end_run();
                    machine.raw.push(Event::Down(at.0, at.1));
                    machine.sewn = true;
                }
                StitchKind::Jump if at == machine.needle => {}
                StitchKind::Jump => machine.jumps(records),
                StitchKind::Trim => machine.jumps(JUMPS_FOR_TRIM),
                StitchKind::Stop => machine.pause(),
            }
            machine.needle = at;
        }
    }
    machine.end_run();
    canonical(machine.raw)
}

/// A DST machine reading records: where the needle is, whether it sewed since the thread was last cut or
/// changed, and the jump records in a row so far, with where they started.
#[derive(Default)]
struct DstMachine {
    raw: Vec<Event>,
    needle: (i32, i32),
    sewn: bool,
    run: usize,
    run_start: (i32, i32),
}

impl DstMachine {
    fn jumps(&mut self, records: usize) {
        if records > 0 && self.run == 0 {
            self.run_start = self.needle;
        }
        self.run += records;
    }

    /// A record that is not a jump: a long enough run before it cut the thread where it started.
    fn end_run(&mut self) {
        if self.run >= JUMPS_FOR_TRIM && self.sewn {
            self.raw.push(Event::Cut(self.run_start.0, self.run_start.1));
            self.sewn = false;
        }
        self.run = 0;
    }

    /// A colour-change record: a thread change or a stop, which DST cannot tell apart.
    fn pause(&mut self) {
        self.end_run();
        self.raw.push(Event::Pause(self.needle.0, self.needle.1));
        self.sewn = false;
    }
}

/// The DST records a move of (`dx`, `dy`) units takes: the fewest of at most [`RECORD_LIMIT`] along each
/// axis, and one for a move that goes nowhere.
fn dst_records(dx: i32, dy: i32) -> usize {
    let limit = RECORD_LIMIT.unsigned_abs();
    let records = dx.unsigned_abs().div_ceil(limit).max(dy.unsigned_abs().div_ceil(limit)).max(1);
    usize::try_from(records).unwrap_or(usize::MAX)
}

/// `raw` in the canonical order [`events`] describes.
fn canonical(raw: Vec<Event>) -> Vec<Event> {
    let last_down = raw.iter().rposition(|e| matches!(e, Event::Down(..)));
    let mut canonical = Vec::with_capacity(raw.len());
    let mut run: Vec<Event> = Vec::new();
    for (i, event) in raw.into_iter().enumerate() {
        if let Event::Down(..) = event {
            flush(&mut run, &mut canonical);
            canonical.push(event);
        } else if !(matches!(event, Event::Cut(..)) && last_down.is_none_or(|d| i > d)) {
            run.push(event);
        }
    }
    flush(&mut run, &mut canonical);
    canonical
}

fn flush(run: &mut Vec<Event>, out: &mut Vec<Event>) {
    run.sort();
    run.dedup_by(|a, b| a == b && matches!(a, Event::Cut(..)));
    out.append(run);
}

/// `point` in 0.1 mm, rounded half to even.
fn units(point: Point) -> (i32, i32) {
    let round = |mm: f64| {
        let units = (mm * 10.0).round_ties_even();
        // Test plans stay far inside ±10 m, so the conversion is exact.
        #[allow(clippy::cast_possible_truncation)]
        let units = units as i32;
        units
    };
    (round(point.x()), round(point.y()))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;
    use crate::plans;

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    #[test]
    fn jumps_vanish_and_commands_are_canonical() {
        let top = Provenance::plan(Role::Top);
        let mut a = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        a.stitch(p(1.0, 0.0), top);
        a.trim(None);
        a.change_thread(Thread::new(Rgb::new(1, 1, 1)));
        a.jump(p(5.0, 0.0), top);
        a.stitch(p(5.0, 0.0), top);
        a.trim(None);
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        b.stitch(p(1.0, 0.0), top);
        b.stop(None);
        b.trim(None);
        b.trim(None);
        b.jump(p(3.0, 0.0), top);
        b.jump(p(5.0, 0.0), top);
        b.stitch(p(5.0, 0.0), top);
        let (a, b) = (events(&a.finish()), events(&b.finish()));
        assert_eq!(a, vec![Event::Down(10, 0), Event::Cut(10, 0), Event::Pause(10, 0), Event::Down(50, 0)]);
        assert_eq!(a, b);
    }

    #[test]
    fn dst_machines_cut_before_three_jumps_in_a_row_after_sewing() {
        // Of the canonical long jumps, only the 30 mm one between stitches is cut, where it starts.
        let plan = plans::long_jumps();
        let mut expected = events(&plan);
        let at = expected.iter().position(|e| *e == Event::Down(300, 0)).unwrap();
        expected.insert(at, Event::Cut(0, 0));
        assert_eq!(dst_events(&plan), expected);
        // A plan whose every cut DST can say is sewn as planned.
        assert_eq!(dst_events(&plans::every_command()), events(&plans::every_command()));
    }

    #[test]
    fn a_dst_cut_happens_where_its_run_of_jumps_starts_and_only_after_sewing() {
        let top = Provenance::plan(Role::Top);
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        b.stitch(p(1.0, 0.0), top);
        // A trim after a jump: the jump and the trim's own jumps are one run, cut before the jump.
        b.jump(p(5.0, 0.0), top);
        b.trim(None);
        b.stitch(p(6.0, 0.0), top);
        // A trim right after a thread change: the thread is cut already.
        b.change_thread(Thread::new(Rgb::new(1, 1, 1)));
        b.trim(None);
        b.stitch(p(7.0, 0.0), top);
        // A sewn move of four records is three jumps and a stitch: cut before it.
        b.stitch(p(47.0, 0.0), top);
        // Two records are not enough; a jump that goes nowhere takes none.
        b.jump(p(47.0, 0.0), top);
        b.jump(p(71.2, 0.0), top);
        b.stitch(p(71.2, 0.0), top);
        assert_eq!(
            dst_events(&b.finish()),
            vec![
                Event::Down(10, 0),
                Event::Cut(10, 0),
                Event::Down(60, 0),
                Event::Pause(60, 0),
                Event::Down(70, 0),
                Event::Cut(70, 0),
                Event::Down(470, 0),
                Event::Down(712, 0),
            ]
        );
        assert_eq!((dst_records(0, 0), dst_records(121, -121), dst_records(-122, 0), dst_records(0, 243)), (1, 1, 2, 3));
    }
}
