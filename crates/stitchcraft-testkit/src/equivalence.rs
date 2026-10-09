//! When two plans make a machine do the same thing.
//!
//! A machine file cannot say everything a plan says. PES spells a trim as a flag on the next move; DST
//! spells it as three jumps; long moves become several records; a stop is a colour change to the same
//! thread. So a round trip through a format does not give back the *same* plan — it gives back a plan
//! that makes the machine do the same thing. [`events`] is that "same thing": where the needle goes
//! down, where the thread is cut, where the machine pauses, in machine units (REQ-FMT-002, REQ-FMT-003).
//!
//! Positions are rounded here with this module's own code, not the writers': an oracle that shared the
//! writers' quantization could not catch a bug in it.

use stitchcraft_core::Point;
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
}
