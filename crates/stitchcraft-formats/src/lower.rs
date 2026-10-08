//! Lowering: a stitch plan becomes one list of machine operations with quantized moves.
//!
//! Every writer needs the same first steps — quantize each position once, turn positions into moves,
//! insert a colour change between blocks and an end after the last, check that commands happen where the
//! needle is — so they happen here, once, and each format only decides how to *spell* the operations.
//! [`split`] is the other shared piece: every format limits how far one record may move, and splits
//! longer moves evenly (REQ-FMT-007).

use stitchcraft_plan::{Rgb, StitchKind, StitchPlan};

use crate::error::EncodeError;
use crate::quantize::{Delta, Units, quantize_point};

/// One machine operation. Moves are relative to the previous position, in 0.1 mm, y down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Op {
    /// Move and sew.
    Stitch(Delta),
    /// Move without sewing.
    Jump(Delta),
    /// Cut the thread here.
    Trim,
    /// Pause for the operator here; formats record it as a change to the same thread.
    Stop,
    /// Change to the next block's thread.
    ColorChange,
    /// The design is finished.
    End,
}

/// A plan ready for a writer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Lowered {
    /// The operations, ending with exactly one [`Op::End`].
    pub ops: Vec<Op>,
    /// The thread colour of every colour entry — the first block, each later block, each stop — in order.
    /// Formats that list threads list these.
    pub entries: Vec<Rgb>,
    /// The smallest and largest position the needle visits (the start at the origin excluded).
    pub bounds: (Units, Units),
}

impl Lowered {
    /// Colour changes plus stops: what a format has to record as colour changes.
    pub fn changes(&self) -> usize {
        self.entries.len().saturating_sub(1)
    }
}

/// Lowers `plan`; `format` names the writer for error messages.
pub(crate) fn lower(plan: &StitchPlan, format: &'static str) -> Result<Lowered, EncodeError> {
    if !plan.stitches().any(|s| s.kind == StitchKind::Normal) {
        return Err(EncodeError::Empty);
    }
    let mut ops = Vec::with_capacity(plan.stitches().count() + plan.blocks.len() + 1);
    let entries = plan.color_entries().iter().map(|entry| entry.thread.color).collect();
    let mut needle = Units::default();
    let mut bounds: Option<(Units, Units)> = None;

    for (b, block) in plan.blocks.iter().enumerate() {
        if b > 0 {
            ops.push(Op::ColorChange);
        }
        for (index, stitch) in block.stitches.iter().enumerate() {
            let at = quantize_point(stitch.at)
                .ok_or_else(|| EncodeError::TooLarge { format, what: format!("the position ({:.1}, {:.1}) mm", stitch.at.x(), stitch.at.y()) })?;
            match stitch.kind {
                StitchKind::Normal => ops.push(Op::Stitch(at - needle)),
                StitchKind::Jump => ops.push(Op::Jump(at - needle)),
                StitchKind::Trim | StitchKind::Stop => {
                    if at != needle {
                        let kind = if stitch.kind == StitchKind::Trim { "trim" } else { "stop" };
                        return Err(EncodeError::CommandAwayFromNeedle { kind, block: b, index });
                    }
                    ops.push(if stitch.kind == StitchKind::Trim { Op::Trim } else { Op::Stop });
                }
            }
            needle = at;
            bounds = Some(match bounds {
                None => (at, at),
                Some((min, max)) => (Units { x: min.x.min(at.x), y: min.y.min(at.y) }, Units { x: max.x.max(at.x), y: max.y.max(at.y) }),
            });
        }
    }
    ops.push(Op::End);
    // A plan with a Normal stitch has at least one position.
    let bounds = bounds.unwrap_or_default();
    Ok(Lowered { ops, entries, bounds })
}

/// `delta` as the fewest equal-as-possible moves of at most `limit` units along each axis. A zero move is
/// one zero piece. The pieces add up to `delta` exactly.
pub(crate) fn split(delta: Delta, limit: i32) -> Split {
    let limit = i64::from(limit.max(1));
    let (dx, dy) = (i64::from(delta.dx), i64::from(delta.dy));
    let ceil = |v: i64| (v.abs() + limit - 1) / limit;
    let pieces = ceil(dx).max(ceil(dy)).max(1);
    Split { dx, dy, pieces, done: 0 }
}

/// The moves [`split`] produces.
#[derive(Clone, Debug)]
pub(crate) struct Split {
    dx: i64,
    dy: i64,
    pieces: i64,
    done: i64,
}

impl Split {
    /// The distance covered after `i` of the pieces, along an axis of total `d`.
    fn covered(&self, d: i64, i: i64) -> i64 {
        d.signum() * (d.abs() * i / self.pieces)
    }
}

impl Iterator for Split {
    type Item = Delta;

    fn next(&mut self) -> Option<Delta> {
        if self.done >= self.pieces {
            return None;
        }
        let (before, after) = (self.done, self.done + 1);
        self.done = after;
        let dx = self.covered(self.dx, after) - self.covered(self.dx, before);
        let dy = self.covered(self.dy, after) - self.covered(self.dy, before);
        // Each piece is at most the per-record limit, which fits an i32.
        Some(Delta { dx: i32::try_from(dx).ok()?, dy: i32::try_from(dy).ok()? })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let left = usize::try_from(self.pieces - self.done).unwrap_or(0);
        (left, Some(left))
    }
}

impl ExactSizeIterator for Split {}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Point;
    use stitchcraft_plan::{PlanBuilder, Provenance, Role, Thread};

    use super::*;

    fn d(dx: i32, dy: i32) -> Delta {
        Delta { dx, dy }
    }

    #[test]
    fn req_fmt_007_splits_are_even_exact_and_within_the_limit() {
        for (delta, limit, count) in [(d(171, 0), 121, 2), (d(-540, 195), 121, 5), (d(3000, 0), 2047, 2), (d(0, 0), 121, 1), (d(121, -121), 121, 1)] {
            let pieces: Vec<Delta> = split(delta, limit).collect();
            assert_eq!(pieces.len(), count, "{delta:?}");
            assert_eq!(split(delta, limit).len(), count);
            let sum = pieces.iter().fold(d(0, 0), |acc, p| d(acc.dx + p.dx, acc.dy + p.dy));
            assert_eq!(sum, delta);
            assert!(pieces.iter().all(|p| p.dx.abs() <= limit && p.dy.abs() <= limit), "{pieces:?}");
            let (min, max) = pieces.iter().fold((i32::MAX, i32::MIN), |(lo, hi), p| (lo.min(p.dx.abs()), hi.max(p.dx.abs())));
            assert!(max - min <= 1, "uneven: {pieces:?}");
        }
    }

    #[test]
    fn lowering_spells_out_blocks_commands_and_the_end() {
        let p = |x, y| Point::new(x, y).unwrap();
        let top = Provenance::plan(Role::Top);
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(1, 2, 3)));
        b.jump(p(1.0, 1.0), top);
        b.stitch(p(1.0, 1.0), top);
        b.stitch(p(3.5, 1.0), top);
        b.trim(None);
        b.change_thread(Thread::new(Rgb::new(4, 5, 6)));
        b.jump(p(-2.0, -1.0), top);
        b.stitch(p(-2.0, -1.0), top);
        b.stop(None);
        b.stitch(p(-2.0, 1.0), top);
        let lowered = lower(&b.finish(), "test").unwrap();
        assert_eq!(
            lowered.ops,
            vec![
                Op::Jump(d(10, 10)),
                Op::Stitch(d(0, 0)),
                Op::Stitch(d(25, 0)),
                Op::Trim,
                Op::ColorChange,
                Op::Jump(d(-55, -20)),
                Op::Stitch(d(0, 0)),
                Op::Stop,
                Op::Stitch(d(0, 20)),
                Op::End,
            ]
        );
        assert_eq!(lowered.entries, vec![Rgb::new(1, 2, 3), Rgb::new(4, 5, 6), Rgb::new(4, 5, 6)]);
        assert_eq!(lowered.changes(), 2);
        assert_eq!(lowered.bounds, (Units { x: -20, y: -10 }, Units { x: 35, y: 10 }));
    }

    #[test]
    fn req_fmt_004_writers_refuse_empty_plans_and_misplaced_commands() {
        let mut b = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        b.jump(Point::new(1.0, 0.0).unwrap(), Provenance::plan(Role::Travel));
        assert_eq!(lower(&b.clone().finish(), "test"), Err(EncodeError::Empty));
        b.stitch(Point::new(1.0, 0.0).unwrap(), Provenance::plan(Role::Top));
        let mut plan = b.finish();
        plan.blocks[0].stitches.push(stitchcraft_plan::Stitch {
            at: Point::new(9.0, 9.0).unwrap(),
            kind: StitchKind::Stop,
            origin: Provenance::plan(Role::Command),
        });
        assert_eq!(lower(&plan, "test"), Err(EncodeError::CommandAwayFromNeedle { kind: "stop", block: 0, index: 2 }));
    }
}
