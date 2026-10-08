//! Reading machine files back into stitch plans.
//!
//! Every reader feeds the same `Recorder`, which turns machine events — a sewn move, a jump, a trim, a
//! stop, a thread change — into a [`StitchPlan`]. The recorder owns the limits every reader needs: at
//! most [`MAX_RECORDS`] records (the stitch budget's limit, so a hostile file cannot exhaust memory) and
//! positions within ±10 m. Readers are literal: each record becomes one plan entry, so `stitch inspect`
//! shows what is in the file, not a cleaned-up version of it.

use stitchcraft_core::units::MACHINE_LIMIT;
use stitchcraft_core::{Budget, Point};
use stitchcraft_plan::{PaletteId, PlanBuilder, Provenance, Role, StitchPlan, Thread};

use crate::error::DecodeError;
use crate::quantize::{Delta, Units};

/// The most records a reader accepts: the stitch budget of one design.
pub const MAX_RECORDS: usize = Budget::DEFAULT.max_stitches as usize;

/// A machine file, read.
#[derive(Clone, Debug, PartialEq)]
pub struct Decoded {
    /// The format and version, as people say it: `PES (#PES0001)`, `PEC`, `DST`.
    pub format: String,
    /// The palette the file's thread colours are indices into; `None` when the format stores no colours
    /// (DST), so the plan's threads are placeholders.
    pub palette: Option<PaletteId>,
    /// The design name stored in the file.
    pub name: String,
    /// The stitches, one plan entry per record.
    pub plan: StitchPlan,
    /// Things in the file that are unusual but readable (an unknown thread index, a bad thumbnail
    /// offset), in the order they were found.
    pub warnings: Vec<String>,
}

/// Builds a plan from machine events, enforcing the limits every reader needs.
pub(crate) struct Recorder {
    builder: PlanBuilder,
    at: Units,
    records: usize,
}

impl Recorder {
    /// A recorder whose first block is sewn with `thread`, the needle at the origin.
    pub fn new(thread: Thread) -> Self {
        Recorder { builder: PlanBuilder::new(thread), at: Units::default(), records: 0 }
    }

    /// A sewn move.
    pub fn stitch(&mut self, delta: Delta) -> Result<(), DecodeError> {
        let at = self.advance(delta)?;
        self.builder.stitch(at, Provenance::plan(Role::Top));
        Ok(())
    }

    /// A move without sewing.
    pub fn jump(&mut self, delta: Delta) -> Result<(), DecodeError> {
        let at = self.advance(delta)?;
        self.builder.jump(at, Provenance::plan(Role::Travel));
        Ok(())
    }

    /// A trim where the needle is.
    pub fn trim(&mut self) -> Result<(), DecodeError> {
        self.count()?;
        self.builder.trim(None);
        Ok(())
    }

    /// A stop where the needle is.
    pub fn stop(&mut self) -> Result<(), DecodeError> {
        self.count()?;
        self.builder.stop(None);
        Ok(())
    }

    /// A change to `thread`.
    pub fn change_thread(&mut self, thread: Thread) -> Result<(), DecodeError> {
        self.count()?;
        self.builder.change_thread(thread);
        Ok(())
    }

    /// The plan read so far.
    pub fn finish(self) -> StitchPlan {
        self.builder.finish()
    }

    fn count(&mut self) -> Result<(), DecodeError> {
        self.records += 1;
        if self.records > MAX_RECORDS { Err(DecodeError::TooLong { max: MAX_RECORDS }) } else { Ok(()) }
    }

    fn advance(&mut self, delta: Delta) -> Result<Point, DecodeError> {
        self.count()?;
        let x = i64::from(self.at.x) + i64::from(delta.dx);
        let y = i64::from(self.at.y) + i64::from(delta.dy);
        let within = |v: i64| v.abs() <= i64::from(MACHINE_LIMIT);
        if !(within(x) && within(y)) {
            return Err(DecodeError::OutOfRange);
        }
        // Within ±MACHINE_LIMIT, so both fit an i32.
        self.at = Units { x: i32::try_from(x).map_err(|_| DecodeError::OutOfRange)?, y: i32::try_from(y).map_err(|_| DecodeError::OutOfRange)? };
        Ok(Point::from_tenths(self.at.x, self.at.y))
    }
}

/// The design name in a 16-byte label field: printable ASCII, trailing spaces removed.
pub(crate) fn label_text(field: &[u8]) -> String {
    field.iter().map(|&b| if (0x20..0x7F).contains(&b) { char::from(b) } else { '?' }).collect::<String>().trim_end().to_string()
}
