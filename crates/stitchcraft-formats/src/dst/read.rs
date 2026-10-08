//! Reading DST files.
//!
//! DST stores moves and three commands — jump, colour change, end — and nothing else: no colours, no
//! trims, no stops. So the reader adds what the format can only imply:
//!
//! - **Trims.** A run of 2 to 8 consecutive *small* jumps (at most 1 mm along each axis) that ends where
//!   it started moves the frame nowhere, so it can only mean "trim here" (StitchCraft writes three; other
//!   writers write two to four). Such a run becomes a `Trim`; any other jump stays a jump. The size limit
//!   matters: a long jump and the long move after it are both split into pieces of about 12 mm, and two
//!   such pieces can cancel exactly — the round-trip property test found that case.
//! - **Threads.** Each block gets a placeholder thread named "thread 1", "thread 2", …; a colour change and
//!   a stop look the same in DST, so both start a new block.
//!
//! Every record is checked; the record count is capped like every reader's.

use stitchcraft_plan::{Rgb, Thread};

use super::{HEADER_LEN, displacement};
use crate::decode::{Decoded, MAX_RECORDS, Recorder, label_text};
use crate::error::DecodeError;
use crate::quantize::Delta;

const FORMAT: &str = "DST";
/// The longest jump run read as a trim.
const LONGEST_TRIM: usize = 8;
/// The largest move (0.1 mm units, per axis) of a jump in a trim run.
const TRIM_JUMP_MAX: i32 = 10;

/// Reads a DST file.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    let header = bytes.get(..HEADER_LEN).ok_or(DecodeError::Truncated { part: "DST header" })?;
    if !header.starts_with(b"LA:") {
        return Err(DecodeError::UnknownFormat);
    }
    let name = label_text(header.get(3..19).unwrap_or_default());
    let records = bytes.get(HEADER_LEN..).unwrap_or_default();
    let mut blocks = 1;
    let mut recorder = Recorder::new(placeholder(blocks));
    let mut jumps: Vec<Delta> = Vec::new();
    let mut i = 0;
    let mut ended = false;
    while let Some(record) = records.get(i..i + 3).and_then(|r| <[u8; 3]>::try_from(r).ok()) {
        let at = HEADER_LEN + i;
        i += 3;
        let [_, _, kind] = record;
        if kind == 0xF3 {
            ended = true;
            break;
        }
        let (dx, dy_up) = displacement(record);
        let delta = Delta { dx, dy: -dy_up };
        match kind & 0xC0 {
            0xC0 => {
                flush(&mut jumps, &mut recorder)?;
                if delta != Delta::ZERO {
                    recorder.jump(delta)?;
                }
                blocks += 1;
                recorder.change_thread(placeholder(blocks))?;
            }
            0x80 => {
                if jumps.len() >= MAX_RECORDS {
                    return Err(DecodeError::TooLong { max: MAX_RECORDS });
                }
                jumps.push(delta);
            }
            0x00 => {
                flush(&mut jumps, &mut recorder)?;
                recorder.stitch(delta)?;
            }
            _ => return Err(DecodeError::BadRecord { format: FORMAT, at: at + 2, byte: kind }),
        }
    }
    if !ended {
        return Err(if records.len() % 3 == 0 { DecodeError::MissingEnd } else { DecodeError::Truncated { part: "DST records" } });
    }
    flush(&mut jumps, &mut recorder)?;
    Ok(Decoded { format: FORMAT.to_string(), name, plan: recorder.finish(), warnings: Vec::new() })
}

/// Records the pending jumps: each run of 2–[`LONGEST_TRIM`] small jumps that returns to its start is a
/// trim.
fn flush(jumps: &mut Vec<Delta>, recorder: &mut Recorder) -> Result<(), DecodeError> {
    let mut k = 0;
    while let Some(rest) = jumps.get(k..).filter(|r| !r.is_empty()) {
        let mut sum = (0_i64, 0_i64);
        let mut trim = None;
        for (n, d) in rest.iter().take(LONGEST_TRIM).enumerate() {
            if d.dx.abs() > TRIM_JUMP_MAX || d.dy.abs() > TRIM_JUMP_MAX {
                break;
            }
            sum = (sum.0 + i64::from(d.dx), sum.1 + i64::from(d.dy));
            if n >= 1 && sum == (0, 0) {
                trim = Some(n + 1);
                break;
            }
        }
        match trim {
            Some(length) => {
                recorder.trim()?;
                k += length;
            }
            None => {
                if let Some(&d) = rest.first() {
                    recorder.jump(d)?;
                }
                k += 1;
            }
        }
    }
    jumps.clear();
    Ok(())
}

/// The placeholder thread of block `n` (DST stores no colours).
fn placeholder(n: usize) -> Thread {
    Thread::named(Rgb::new(0, 0, 0), format!("thread {n}"))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::StitchKind;

    use super::*;
    use crate::decode::Recorder;

    fn golden(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../conformance/golden/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn reads_every_command_back() {
        let d = decode(&golden("formats/every-command.dst")).unwrap();
        assert_eq!((d.format.as_str(), d.name.as_str()), ("DST", "every-command"));
        let stats = d.plan.stats();
        assert_eq!((stats.stitches, stats.trims), (12, 3), "every stitch, and the three trims come back as trims");
        // The stop and the thread change both read as thread changes: DST cannot tell them apart.
        assert_eq!(d.plan.blocks.len(), 3);
        let first_stitch = d.plan.stitches().find(|s| s.kind == StitchKind::Normal).map(|s| (s.at.x(), s.at.y()));
        assert_eq!(first_stitch, Some((-20.0, -10.0)), "y is flipped back to point down");
    }

    #[test]
    fn cancelling_pieces_of_long_moves_are_not_trims() {
        // Found by the round-trip property test: a long jump, then a long sewn move back. Both are split
        // into ~12 mm jumps, and the last piece of one cancels the first piece of the other exactly.
        let mut jumps = vec![Delta { dx: -118, dy: 39 }, Delta { dx: 118, dy: -39 }];
        let mut recorder = Recorder::new(placeholder(1));
        flush(&mut jumps, &mut recorder).unwrap();
        let plan = recorder.finish();
        assert_eq!((plan.stats().trims, plan.stats().jumps), (0, 2));
    }

    #[test]
    fn long_jumps_are_not_mistaken_for_trims() {
        let d = decode(&golden("testsheets/TS-01.dst")).unwrap();
        assert_eq!(d.plan.stats().trims, 7);
        let b = d.plan.bounds().unwrap();
        assert_eq!((b.width(), b.height()), (120.0, 120.0));
    }

    #[test]
    fn req_fmt_004_a_file_without_stitches_reads_as_an_empty_plan() {
        let mut bytes = golden("formats/one-stitch.dst");
        bytes.truncate(HEADER_LEN);
        bytes.extend_from_slice(&[0x00, 0x00, 0xF3]);
        assert_eq!(decode(&bytes).map(|d| d.plan.stats().stitches), Ok(0));
    }

    #[test]
    fn damaged_files_are_errors_not_guesses() {
        let good = golden("formats/every-command.dst");
        assert_eq!(decode(&good[..100]), Err(DecodeError::Truncated { part: "DST header" }));
        assert_eq!(decode(&good[..good.len() - 3]), Err(DecodeError::MissingEnd));
        assert_eq!(decode(&good[..good.len() - 1]), Err(DecodeError::Truncated { part: "DST records" }));
        let mut odd = good;
        odd[HEADER_LEN + 2] = 0x43;
        assert_eq!(decode(&odd), Err(DecodeError::BadRecord { format: "DST", at: HEADER_LEN + 2, byte: 0x43 }));
    }

    #[test]
    fn req_fmt_006_every_truncation_and_byte_flip_is_handled() {
        let bytes = golden("formats/every-command.dst");
        for len in 0..bytes.len() {
            let _ = decode(&bytes[..len]);
        }
        for at in 0..bytes.len() {
            for flip in [0x01, 0x40, 0x80, 0xFF] {
                let mut changed = bytes.clone();
                changed[at] ^= flip;
                let _ = decode(&changed);
            }
        }
    }
}
