//! Machine embroidery formats (layer L2): PES (PEC) for Brother and DST for everything else.
//!
//! Writing a plan has two halves. [`lower`](crate::lower) does what every format needs — quantize each
//! position once, turn positions into moves, spell out colour changes and the end, check commands —
//! and each format module only decides how to *spell* those operations, respecting its own per-record
//! limits by splitting long moves evenly. Writers refuse rather than produce a file a machine could
//! misread: every failure is an [`EncodeError`] with a registered diagnostic code.
//!
//! Readers (hostile-input parsers that never panic) arrive in roadmap milestone M2. Design:
//! `docs/src/design/formats.md`.
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing)]

pub mod dst;
pub mod error;
pub mod label;
pub mod pes;
pub mod quantize;

mod lower;

pub use error::EncodeError;
use stitchcraft_plan::{FormatId, StitchPlan};

/// `plan` in `format`, with `name` as the design name machines show.
pub fn encode(plan: &StitchPlan, format: FormatId, name: &str) -> Result<Vec<u8>, EncodeError> {
    match format {
        FormatId::PesV1 => pes::encode(plan, name),
        FormatId::Dst => dst::encode(plan, name),
    }
}
