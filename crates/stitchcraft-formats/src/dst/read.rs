//! Reading DST files.
//!
//! DST stores moves and three commands — jump, colour change, end — and nothing else: no colours, no
//! trims, no stops. So the reader adds what the format can only imply:
//!
//! - **Trims.** DST has no trim command: machines cut the thread before [`JUMPS_FOR_TRIM`] or more jump
//!   records in a row — the common setting, and pyembroidery's reading — if something was sewn since it
//!   was last cut or changed (REQ-FMT-008; `super` explains). The reader does what they do: such a run
//!   reads as a `Trim` and then its jumps; a shorter run, or one with nothing sewn before it, as jumps.
//!   A trim's own spelling at the start of a long enough run — 2 to 8 *small* jumps (at most 1 mm along
//!   each axis) that end where they started, as StitchCraft (three) and other writers spell it — moves the
//!   frame nowhere, so it is not kept as jumps. Only small ones: a long jump and the long move after it are
//!   both split into pieces of about 12 mm, and two such pieces can cancel exactly — the round-trip
//!   property test found that case — but they are real moves.
//! - **Threads.** Each block gets a placeholder thread named "thread 1", "thread 2", …; a colour change and
//!   a stop look the same in DST, so both start a new block.
//!
//! Every record is checked; the record count is capped like every reader's.

use stitchcraft_plan::{Rgb, Thread};

use super::{HEADER_LEN, JUMPS_FOR_TRIM, displacement};
use crate::decode::{Decoded, MAX_RECORDS, Recorder, label_text};
use crate::error::DecodeError;
use crate::quantize::Delta;

const FORMAT: &str = "DST";
/// The longest spelling of a trim recognised at the start of a run of jumps.
const LONGEST_TRIM: usize = 8;
/// The largest move (0.1 mm units, per axis) of a jump in a trim's spelling.
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
    // Whether a stitch was sewn since the thread was last cut or changed.
    let mut sewn = false;
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
                flush(&mut jumps, &mut sewn, &mut recorder)?;
                if delta != Delta::ZERO {
                    recorder.jump(delta)?;
                }
                blocks += 1;
                recorder.change_thread(placeholder(blocks))?;
                sewn = false;
            }
            0x80 => {
                if jumps.len() >= MAX_RECORDS {
                    return Err(DecodeError::TooLong { max: MAX_RECORDS });
                }
                jumps.push(delta);
            }
            0x00 => {
                flush(&mut jumps, &mut sewn, &mut recorder)?;
                recorder.stitch(delta)?;
                sewn = true;
            }
            _ => return Err(DecodeError::BadRecord { format: FORMAT, at: at + 2, byte: kind }),
        }
    }
    if !ended {
        return Err(if records.len() % 3 == 0 { DecodeError::MissingEnd } else { DecodeError::Truncated { part: "DST records" } });
    }
    flush(&mut jumps, &mut sewn, &mut recorder)?;
    Ok(Decoded { format: FORMAT.to_string(), palette: None, name, plan: recorder.finish(), warnings: Vec::new() })
}

/// Records the run of `jumps` that just ended: a trim first when it is [`JUMPS_FOR_TRIM`] or more long
/// and something was `sewn` since the thread was last cut or changed, then its jumps but a trim's
/// spelling at its start.
fn flush(jumps: &mut Vec<Delta>, sewn: &mut bool, recorder: &mut Recorder) -> Result<(), DecodeError> {
    let mut rest = jumps.as_slice();
    if jumps.len() >= JUMPS_FOR_TRIM {
        if *sewn {
            recorder.trim()?;
            *sewn = false;
        }
        rest = rest.get(spelled_trim(rest)..).unwrap_or_default();
    }
    for &d in rest {
        recorder.jump(d)?;
    }
    jumps.clear();
    Ok(())
}

/// How many of the first `jumps` spell a trim: 2 to [`LONGEST_TRIM`] small jumps that end where they
/// started, or none.
fn spelled_trim(jumps: &[Delta]) -> usize {
    let small = jumps.iter().take(LONGEST_TRIM).take_while(|d| d.dx.abs() <= TRIM_JUMP_MAX && d.dy.abs() <= TRIM_JUMP_MAX).count();
    let back = |n: usize| {
        let first = jumps.iter().take(n);
        first.clone().map(|d| i64::from(d.dx)).sum::<i64>() == 0 && first.map(|d| i64::from(d.dy)).sum::<i64>() == 0
    };
    (2..=small).find(|&n| back(n)).unwrap_or(0)
}

/// The placeholder thread of block `n` (DST stores no colours).
fn placeholder(n: usize) -> Thread {
    Thread::named(Rgb::new(0, 0, 0), format!("thread {n}"))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::StitchKind;

    use super::super::{JUMP, SEW, TRIM_JUMPS, record};
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

    /// A DST file of `records` (DST axes, y up), as the plan entries read from it: `S` a stitch, `J` a
    /// jump, `T` a trim, `|` a thread change.
    fn read(records: &[(i32, i32, u8)]) -> String {
        let mut bytes = b"LA:records".to_vec();
        bytes.resize(HEADER_LEN, b' ');
        for &(dx, dy, kind) in records {
            bytes.extend_from_slice(&if kind == 0xC3 { [0x00, 0x00, 0xC3] } else { record(dx, dy, kind) });
        }
        bytes.extend_from_slice(&[0x00, 0x00, 0xF3]);
        let plan = decode(&bytes).unwrap().plan;
        let blocks = plan.blocks.iter().map(|block| {
            let letter = |kind| match kind {
                StitchKind::Normal => "S",
                StitchKind::Jump => "J",
                StitchKind::Trim => "T",
                StitchKind::Stop => "P",
            };
            block.stitches.iter().map(|s| letter(s.kind)).collect::<Vec<_>>().join(" ")
        });
        blocks.collect::<Vec<_>>().join(" | ")
    }

    #[test]
    fn req_fmt_008_three_jumps_in_a_row_cut_the_thread_before_them() {
        let (sew, jump, change) = (SEW, JUMP, 0xC3);
        let stitches = [(0, 0, sew), (30, 0, sew)];
        let between = |jumps: &[(i32, i32, u8)]| [&stitches[..], jumps, &stitches[..]].concat();
        // Three or more jumps after stitches: cut before them. Two: no cut.
        assert_eq!(read(&between(&[(100, 0, jump); 3])), "S S T J J J S S");
        assert_eq!(read(&between(&[(100, 0, jump); 5])), "S S T J J J J J S S");
        assert_eq!(read(&between(&[(121, 0, jump); 2])), "S S J J S S");
        // A trim as StitchCraft spells it: three small jumps back to where they started, which go nowhere.
        let trim = TRIM_JUMPS.map(|(dx, dy)| (dx, dy, jump));
        assert_eq!(read(&between(&trim)), "S S T S S");
        assert_eq!(read(&between(&[&trim[..], &[(100, 0, jump); 2]].concat())), "S S T J J S S");
        // Two small jumps back to the start are no trim: machines count three.
        assert_eq!(read(&between(&[(2, -2, jump), (-2, 2, jump)])), "S S J J S S");
        // A trim's spelling is small jumps, up to 1 mm along each axis, that end where they started; other
        // jumps are moves, kept even where they cancel.
        for axis in [|v: i32| (v, 0), |v: i32| (0, v)] {
            let run = |moves: [i32; 3]| moves.map(|v| (axis(v).0, axis(v).1, jump));
            assert_eq!(read(&between(&run([10, -10, 5]))), "S S T J S S");
            assert_eq!(read(&between(&run([11, -11, 5]))), "S S T J J J S S");
            assert_eq!(read(&between(&run([100, -100, 50]))), "S S T J J J S S");
        }
        assert_eq!(read(&between(&[(2, 1, jump), (-2, 1, jump), (0, -2, jump)])), "S S T S S");
        assert_eq!(read(&between(&[(1, 2, jump), (1, -2, jump), (-2, 0, jump)])), "S S T S S");
        // Where nothing was sewn since the thread was cut or changed, there is nothing to cut.
        assert_eq!(read(&[&[(100, 0, jump); 3][..], &stitches[..]].concat()), "J J J S S");
        assert_eq!(read(&[&stitches[..], &[(0, 0, change)], &[(100, 0, jump); 3], &stitches[..]].concat()), "S S | J J J S S");
        assert_eq!(read(&between(&[&trim[..], &[(0, 0, change)], &trim[..]].concat())), "S S T | S S");
    }

    #[test]
    fn cancelling_pieces_of_long_moves_are_kept() {
        // Found by the round-trip property test: a long jump, then a long sewn move back. Both are split
        // into ~12 mm jumps, and the last piece of one cancels the first piece of the other exactly; they
        // are real moves, not a trim's spelling.
        for sewn in [true, false] {
            let mut jumps = vec![Delta { dx: -118, dy: 39 }, Delta { dx: 118, dy: -39 }, Delta { dx: 50, dy: 0 }];
            let (mut recorder, mut after) = (Recorder::new(placeholder(1)), sewn);
            flush(&mut jumps, &mut after, &mut recorder).unwrap();
            let plan = recorder.finish();
            assert_eq!((plan.stats().trims, plan.stats().jumps), (usize::from(sewn), 3));
        }
    }

    #[test]
    fn ts_01_reads_back_with_its_seven_trims() {
        // Each of its long jumps follows a trim, so they add none.
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
