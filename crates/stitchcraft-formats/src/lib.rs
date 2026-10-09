//! Machine embroidery formats (layer L2): PES (PEC) for Brother and DST for everything else.
//!
//! Writing a plan has two halves. Lowering (the `lower` module) does what every format needs — quantize each
//! position once, turn positions into moves, spell out colour changes and the end, check commands —
//! and each format module only decides how to *spell* those operations, respecting its own per-record
//! limits by splitting long moves evenly. Writers refuse rather than produce a file a machine could
//! misread: every failure is an [`EncodeError`] with a registered diagnostic code. What a format makes
//! the machine do that the plan does not say comes with the file ([`Encoded::notes`]): DST's machines
//! cut the thread before long jumps.
//!
//! Reading goes the other way: [`decode()`] recognises PES, PEC and DST by their first bytes, and each reader
//! treats the file as possibly damaged or hostile — every offset and length checked, records capped,
//! failures typed ([`DecodeError`]), never a panic. Design: `docs/src/design/formats.md`.
#![forbid(unsafe_code)]
#![deny(clippy::indexing_slicing)]

pub mod decode;
pub mod dst;
pub mod error;
pub mod label;
pub mod pes;
pub mod quantize;

mod lower;

pub use decode::Decoded;
pub use error::{DecodeError, EncodeError};
use stitchcraft_core::Diagnostic;
use stitchcraft_plan::{FormatId, StitchPlan};

/// A machine file, and what the machine will do that the plan does not say.
#[derive(Clone, Debug, PartialEq)]
pub struct Encoded {
    /// The file's bytes.
    pub bytes: Vec<u8>,
    /// What the format makes the machine do differently from the plan, such as DST's cuts before long
    /// jumps (`SC-I0605`); empty when the machine does what the plan says.
    pub notes: Vec<Diagnostic>,
}

/// Reads a machine file, recognising PES, PEC and DST by their first bytes.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    if bytes.starts_with(b"#PES") || bytes.starts_with(b"#PEC") {
        pes::decode(bytes)
    } else if bytes.starts_with(b"LA:") {
        dst::decode(bytes)
    } else {
        Err(DecodeError::UnknownFormat)
    }
}

/// `plan` in `format`, with `name` as the design name machines show.
pub fn encode(plan: &StitchPlan, format: FormatId, name: &str) -> Result<Encoded, EncodeError> {
    match format {
        FormatId::PesV1 => pes::encode(plan, name).map(|bytes| Encoded { bytes, notes: Vec::new() }),
        FormatId::Dst => dst::encode(plan, name),
    }
}
