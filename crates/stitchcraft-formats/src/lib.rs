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
use stitchcraft_core::{Code, Diagnostic};
use stitchcraft_plan::{FormatId, StitchKind, StitchPlan};

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
        FormatId::PesV1 => pes::encode(plan, name),
        FormatId::Dst => dst::encode(plan, name),
    }
}

/// The plan the machine file `bytes` holds, read as `stitch inspect` reads it, with the threads of `plan`,
/// the plan it was written from: what the file sews, in the design's colours, which summaries count and
/// pictures draw (`REQ-CLI-008`). DST stores no colours and PES only a palette's, so each block of the file
/// takes the thread of the plan's entry it starts at, the plan's threads counted again after each stop. A
/// file that joins 2 of the plan's blocks into one, as PES joins blocks a palette shows alike, keeps the
/// first one's thread. A file StitchCraft wrote that it cannot read is a bug in it (`SC-E0009`), so every
/// host that writes one reads it back first: the command line and the VectorCraft exporters.
pub fn read_back(bytes: &[u8], plan: &StitchPlan) -> Result<StitchPlan, Diagnostic> {
    let mut back = decode(bytes)
        .map_err(|e| {
            Diagnostic::new(Code::InternalCheckFailed, format!("The machine file StitchCraft wrote does not read: {}", e.diagnostic().message))
        })?
        .plan;
    let threads: Vec<_> = plan.color_entries().into_iter().map(|entry| entry.thread.clone()).collect();
    let mut entry = 0;
    for block in &mut back.blocks {
        if let Some(thread) = threads.get(entry) {
            block.thread = thread.clone();
        }
        entry += 1 + block.stitches.iter().filter(|s| s.kind == StitchKind::Stop).count();
    }
    Ok(back)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Point;
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;

    #[test]
    fn req_cli_008_a_file_read_back_takes_the_plan_s_threads_after_a_stop_too() {
        // A stop between stitches of one thread, then another thread: PES records the stop as a change to the
        // same colour, which reads back as a stop, and the block after it takes the thread after the stop.
        let (navy, red) = (Thread::new(Rgb::new(0, 0, 128)), Thread::new(Rgb::new(200, 0, 0)));
        let mut plan = PlanBuilder::new(navy.clone());
        let sew = |plan: &mut PlanBuilder, xs: [f64; 2]| xs.map(|x| plan.stitch(Point::new(x, 0.0).unwrap(), Provenance::plan(Role::Top)));
        sew(&mut plan, [0.0, 1.0]);
        plan.stop(None);
        sew(&mut plan, [2.0, 3.0]);
        plan.change_thread(red.clone());
        sew(&mut plan, [4.0, 5.0]);
        let plan = plan.finish();
        let back = read_back(&encode(&plan, FormatId::PesV1, "stop").unwrap().bytes, &plan).unwrap();
        assert_eq!(
            back.blocks.iter().map(|b| (&b.thread, b.stitches.iter().any(|s| s.kind == StitchKind::Stop))).collect::<Vec<_>>(),
            [(&navy, true), (&red, false)]
        );
    }

    #[test]
    fn diag_sc_e0009_a_machine_file_that_does_not_read_back_is_a_bug() {
        let plan = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0))).finish();
        let bug = read_back(b"#PES0001 cut short", &plan).unwrap_err();
        assert_eq!(bug.code, Code::InternalCheckFailed);
        assert!(bug.message.starts_with("The machine file StitchCraft wrote does not read: "), "{}", bug.message);
    }
}
