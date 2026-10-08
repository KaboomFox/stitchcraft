//! Brother PES, version 1: a small PES header followed by the PEC block the machine sews from.
//!
//! | Offset | Bytes | Content |
//! |---|---|---|
//! | 0 | 8 | `#PES0001` |
//! | 8 | 4 | offset of the PEC block (little-endian): 22 |
//! | 12 | 2 | hoop indicator: 0 = 100 × 100 mm, 1 = 130 × 180 mm |
//! | 14 | 8 | zero: no design-editor objects |
//! | 22 | … | the PEC block (the `pec` module) |
//!
//! Machines sew from the PEC block and find it through the offset at byte 8. The PES section between is
//! for design software (PE-Design); StitchCraft writes it without design-editor objects, a form other
//! writers use too. Version 1 can only say "100 × 100" or "130 × 180" about the hoop; StitchCraft writes
//! 1 for any design larger than 100 × 100 mm. Whether a Brother machine accepts larger designs from such a
//! file is what test sheet TS-10 checks at machine checkpoint MC-1 (`docs/src/design/formats.md`); a PES
//! v6 writer with explicit hoop dimensions follows only if it does not.

mod pec;
mod read;
mod thumbnail;

pub use read::decode;

use stitchcraft_plan::{FormatId, StitchPlan};

use crate::error::EncodeError;
use crate::lower::lower;

/// The offset of the PEC block.
const PEC_OFFSET: u32 = 22;
/// The largest design, in 0.1 mm, that the small-hoop indicator covers.
const SMALL_HOOP: i32 = 1000;

/// `plan` as a PES v1 file whose design name is `name`.
pub fn encode(plan: &StitchPlan, name: &str) -> Result<Vec<u8>, EncodeError> {
    let format = FormatId::PesV1;
    let lowered = lower(plan, format.name())?;
    if lowered.changes() > format.max_color_changes() {
        return Err(EncodeError::TooManyColorChanges { format: format.name(), changes: lowered.changes(), max: format.max_color_changes() });
    }
    let (min, max) = lowered.bounds;
    let large = i64::from(max.x) - i64::from(min.x) > i64::from(SMALL_HOOP) || i64::from(max.y) - i64::from(min.y) > i64::from(SMALL_HOOP);
    let pec = pec::block(&lowered, name)?;

    let mut out = Vec::with_capacity(22 + pec.len());
    out.extend_from_slice(b"#PES0001");
    out.extend_from_slice(&PEC_OFFSET.to_le_bytes());
    out.extend_from_slice(&u16::from(large).to_le_bytes());
    out.extend_from_slice(&[0; 8]);
    out.extend_from_slice(&pec);
    Ok(out)
}
