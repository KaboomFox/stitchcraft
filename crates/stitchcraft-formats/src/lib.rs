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
    decode_up_to(bytes, decode::MAX_RECORDS)
}

/// The machine file `bytes`, as [`decode()`] reads it, of at most `max` records.
fn decode_up_to(bytes: &[u8], max: usize) -> Result<Decoded, DecodeError> {
    if bytes.starts_with(b"#PES") || bytes.starts_with(b"#PEC") {
        pes::decode_up_to(bytes, max)
    } else if bytes.starts_with(b"LA:") {
        dst::decode_up_to(bytes, max)
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
/// host that writes one reads it back first: the command line and the VectorCraft exporters. It is read
/// whole, as many records as it has: no record is shorter than a byte, and a plan within the stitch budget
/// can have more records than needle points, its jumps, trims and thread changes, and DST's long moves.
pub fn read_back(bytes: &[u8], plan: &StitchPlan) -> Result<StitchPlan, Diagnostic> {
    let mut back = decode_up_to(bytes, bytes.len())
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
    use stitchcraft_core::{Budget, Point};
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    use super::*;

    #[test]
    fn req_cli_008_a_file_read_back_is_read_however_many_records_it_has() {
        // A plan as large as the stitch budget, after a jump: a record more than a reader takes from a
        // file of anyone's, and a file StitchCraft wrote, which it reads back whole. Every format reads
        // back through the same cap, so one is enough, and the test stays quick.
        let mut plan = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        plan.jump(Point::new(1.0, 0.0).unwrap(), Provenance::plan(Role::Travel));
        for i in 0..Budget::DEFAULT.max_stitches {
            plan.stitch(Point::new(if i % 2 == 0 { 1.0 } else { 2.0 }, 0.0).unwrap(), Provenance::plan(Role::Top));
        }
        let plan = plan.finish();
        let bytes = encode(&plan, FormatId::PesV1, "budget").unwrap().bytes;
        assert_eq!(decode(&bytes).map(|_| ()), Err(DecodeError::TooLong { max: decode::MAX_RECORDS }));
        let back = read_back(&bytes, &plan).unwrap();
        assert_eq!(back.stats().stitches, plan.stats().stitches);
    }

    #[test]
    fn a_reader_takes_as_many_records_as_it_is_asked_to() {
        // Stitches either side of a 30 mm jump, which DST spells as jump records in a row: a reader takes
        // the records the file reads back as, one entry each, and no fewer.
        let mut plan = PlanBuilder::new(Thread::new(Rgb::new(0, 0, 0)));
        plan.jump(Point::new(1.0, 0.0).unwrap(), Provenance::plan(Role::Travel));
        plan.stitch(Point::new(1.0, 0.0).unwrap(), Provenance::plan(Role::Top));
        plan.jump(Point::new(31.0, 0.0).unwrap(), Provenance::plan(Role::Travel));
        plan.stitch(Point::new(31.0, 0.0).unwrap(), Provenance::plan(Role::Top));
        plan.stitch(Point::new(33.0, 0.0).unwrap(), Provenance::plan(Role::Top));
        let plan = plan.finish();
        for format in FormatId::ALL {
            let bytes = encode(&plan, *format, "few").unwrap().bytes;
            let records = decode(&bytes).unwrap().plan.stitches().count();
            assert!(decode_up_to(&bytes, records).is_ok(), "{}", format.name());
            assert_eq!(decode_up_to(&bytes, records - 1).map(|_| ()), Err(DecodeError::TooLong { max: records - 1 }), "{}", format.name());
        }
    }

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
