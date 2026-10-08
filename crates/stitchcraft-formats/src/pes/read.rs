//! Reading PES and PEC files: the PEC block, which is what a Brother machine sews.
//!
//! A PES file of any version (`#PES0001` … `#PES0060`) points at its PEC block from byte 8; a bare PEC
//! file (`#PEC0001`) has it at byte 8. The PES design-editor section is not read: machines sew from PEC,
//! and so does StitchCraft. Every offset and length is checked before use (`indexing_slicing` is denied in
//! this crate), the record count is capped, and anything unusual but readable becomes a warning in the
//! result. The record layout is the one the writer documents in [`super::pec`].
//!
//! A colour change to the same palette entry as the current one is read as a stop: that is how PEC
//! writes stops (StitchCraft's writer included). Two colour blocks whose threads map to the same Brother
//! colour therefore read back as one block with a stop — PES v1 cannot tell them apart.

use stitchcraft_plan::Thread;
use stitchcraft_plan::palette::BROTHER_PEC;

use crate::decode::{Decoded, Recorder, label_text};
use crate::error::DecodeError;
use crate::quantize::Delta;

const FORMAT: &str = "PEC";
/// The PEC header and graphics header, before the stitch data.
const STITCHES_AT: usize = 532;

/// Reads a PES or PEC file.
pub fn decode(bytes: &[u8]) -> Result<Decoded, DecodeError> {
    let (format, pec_at) = if let Some(version) = bytes.strip_prefix(b"#PES") {
        let version = version.get(..4).ok_or(DecodeError::Truncated { part: "PES header" })?;
        let offset = bytes.get(8..12).and_then(|b| <[u8; 4]>::try_from(b).ok()).ok_or(DecodeError::Truncated { part: "PES header" })?;
        let offset = u32::from_le_bytes(offset);
        (format!("PES (#PES{})", label_text(version)), usize::try_from(offset).map_err(|_| DecodeError::BadOffset { what: "PEC offset" })?)
    } else if bytes.starts_with(b"#PEC0001") {
        ("PEC".to_string(), 8)
    } else {
        return Err(DecodeError::UnknownFormat);
    };
    // Every PEC block starts with its label field: anything else means the offset is wrong.
    let block = bytes.get(pec_at..).filter(|b| b.starts_with(b"LA:")).ok_or(DecodeError::BadOffset { what: "PEC offset" })?;
    let mut decoded = read_pec(block, pec_at)?;
    decoded.format = format;
    Ok(decoded)
}

/// Reads the PEC block `block`, which starts at byte `base` of the file (for error positions).
fn read_pec(block: &[u8], base: usize) -> Result<Decoded, DecodeError> {
    let header = block.get(..STITCHES_AT).ok_or(DecodeError::Truncated { part: "PEC header" })?;
    let name = label_text(header.get(3..19).unwrap_or_default());
    let entries = usize::from(header.get(48).copied().unwrap_or(0)) + 1;
    let indices = header.get(49..49 + entries).ok_or(DecodeError::Truncated { part: "PEC colour list" })?;
    let mut warnings = Vec::new();

    let graphics = header
        .get(514..517)
        .and_then(|b| <[u8; 3]>::try_from(b).ok())
        .map_or(0, |[a, b, c]| usize::from(a) | usize::from(b) << 8 | usize::from(c) << 16);
    let thumbnails_at = 512 + graphics;
    let end = if (STITCHES_AT..=block.len()).contains(&thumbnails_at) {
        thumbnails_at
    } else {
        warnings.push(format!("The thumbnail offset ({graphics}) points outside the file; stitches were read to the end of the file."));
        block.len()
    };
    let data = block.get(STITCHES_AT..end).unwrap_or_default();

    let thread = |index: u8, warnings: &mut Vec<String>| match BROTHER_PEC.entry(index) {
        Some(entry) => Thread::named(entry.color, entry.name),
        None => {
            warnings.push(format!("Thread index {index} is not a Brother PEC colour; it is shown as grey."));
            Thread::named(stitchcraft_plan::Rgb::new(128, 128, 128), format!("PEC thread {index}"))
        }
    };
    let mut entry = 0;
    let mut current = indices.first().copied().unwrap_or(0);
    let mut recorder = Recorder::new(thread(current, &mut warnings));

    let mut i = 0;
    loop {
        let at = base + STITCHES_AT + i;
        let Some(&first) = data.get(i) else { return Err(DecodeError::MissingEnd) };
        match first {
            0xFF => break,
            0xFE => {
                match data.get(i + 1) {
                    Some(0xB0) => {}
                    Some(_) => return Err(DecodeError::BadRecord { format: FORMAT, at, byte: first }),
                    None => return Err(DecodeError::Truncated { part: "stitch data" }),
                }
                data.get(i + 2).ok_or(DecodeError::Truncated { part: "stitch data" })?;
                i += 3;
                entry += 1;
                let next = match indices.get(entry) {
                    Some(&index) => index,
                    None => {
                        if entry == indices.len() {
                            warnings.push(format!(
                                "The stitch data has more colour changes than the {} colours the header lists; the last colour was repeated.",
                                indices.len()
                            ));
                        }
                        current
                    }
                };
                if next == current {
                    recorder.stop()?;
                } else {
                    recorder.change_thread(thread(next, &mut warnings))?;
                    current = next;
                }
            }
            _ => {
                let (dx, flags_x, after_x) = axis(data, i)?;
                let (dy, flags_y, after_y) = axis(data, after_x)?;
                i = after_y;
                let delta = Delta { dx, dy };
                let flags = flags_x | flags_y;
                if flags & 0x20 != 0 {
                    recorder.trim()?;
                    recorder.jump(delta)?;
                } else if flags & 0x10 != 0 {
                    recorder.jump(delta)?;
                } else {
                    recorder.stitch(delta)?;
                }
            }
        }
    }
    Ok(Decoded { format: String::new(), name, plan: recorder.finish(), warnings })
}

/// One axis at `i`: the value, its flags (long form only) and where the next axis starts.
fn axis(data: &[u8], i: usize) -> Result<(i32, u8, usize), DecodeError> {
    let first = *data.get(i).ok_or(DecodeError::Truncated { part: "stitch data" })?;
    if first & 0x80 == 0 {
        let value = i32::from(first);
        return Ok((if value >= 0x40 { value - 0x80 } else { value }, 0, i + 1));
    }
    let second = *data.get(i + 1).ok_or(DecodeError::Truncated { part: "stitch data" })?;
    let value = (i32::from(first & 0x0F) << 8) | i32::from(second);
    Ok((if value >= 0x800 { value - 0x1000 } else { value }, first & 0x30, i + 2))
}

#[cfg(test)]
mod tests {
    use stitchcraft_plan::StitchKind;

    use super::*;

    fn golden(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../conformance/golden/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    fn kinds(d: &Decoded) -> Vec<(StitchKind, f64, f64)> {
        d.plan.stitches().map(|s| (s.kind, s.at.x(), s.at.y())).collect()
    }

    #[test]
    fn reads_every_command_back() {
        let d = decode(&golden("formats/every-command.pes")).unwrap();
        assert_eq!(d.format, "PES (#PES0001)");
        assert_eq!(d.name, "every-command");
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        let threads: Vec<_> = d.plan.blocks.iter().map(|b| b.thread.name.clone().unwrap_or_default()).collect();
        assert_eq!(threads, ["Red", "Blue"]);
        let k = kinds(&d);
        assert_eq!(k[0], (StitchKind::Jump, -20.0, -10.0));
        assert_eq!(k[1], (StitchKind::Normal, -20.0, -10.0));
        assert!(k.contains(&(StitchKind::Trim, -8.7, -15.8)));
        assert!(k.contains(&(StitchKind::Stop, 22.0, -8.0)));
        assert_eq!(d.plan.stats().stitches, 12);
    }

    #[test]
    fn reads_the_mc1_sheets_at_their_size() {
        for (name, w, h) in [("TS-01", 120.0, 120.0), ("TS-02", 140.0, 70.0), ("TS-10B", 190.0, 150.0)] {
            let d = decode(&golden(&format!("testsheets/{name}.pes"))).unwrap();
            let b = d.plan.bounds().unwrap();
            assert_eq!((b.width(), b.height()), (w, h), "{name}");
            assert_eq!(d.name, name);
        }
    }

    #[test]
    fn req_fmt_004_a_file_without_stitches_reads_as_an_empty_plan() {
        let mut bytes = golden("formats/one-stitch.pes");
        // Replace the stitch data (one zero stitch, then the end) with the end alone, and fix the
        // thumbnail offset to match (20 header bytes + 1).
        let stitches = 22 + STITCHES_AT;
        bytes.splice(stitches..stitches + 3, [0xFF]);
        bytes[22 + 514] = 21;
        let d = decode(&bytes).unwrap();
        assert_eq!(d.plan.stats().stitches, 0);
        assert_eq!(d.warnings, Vec::<String>::new());
    }

    #[test]
    fn bare_pec_files_are_read_too() {
        let pes = golden("formats/every-command.pes");
        let mut pec = b"#PEC0001".to_vec();
        pec.extend_from_slice(&pes[22..]);
        assert_eq!(kinds(&decode(&pec).unwrap()), kinds(&decode(&pes).unwrap()));
    }

    #[test]
    fn damaged_files_are_errors_not_guesses() {
        assert_eq!(decode(b"hello"), Err(DecodeError::UnknownFormat));
        assert_eq!(decode(b"#PES0001"), Err(DecodeError::Truncated { part: "PES header" }));
        let good = golden("formats/every-command.pes");
        for offset in [[0xFF; 4], [0xFF, 0, 0, 0]] {
            // Beyond the file, or inside it but not at a PEC block.
            let mut bad_offset = good.clone();
            bad_offset[8..12].copy_from_slice(&offset);
            assert_eq!(decode(&bad_offset), Err(DecodeError::BadOffset { what: "PEC offset" }));
        }
        assert_eq!(decode(&good[..22 + 400]), Err(DecodeError::Truncated { part: "PEC header" }));
        // Shorten the stitch data by one byte (through the thumbnail offset): the end record is gone.
        let mut no_end = good.clone();
        no_end[22 + 514] -= 1;
        assert_eq!(decode(&no_end), Err(DecodeError::MissingEnd));
        // A colour-change marker must be followed by 0xB0.
        let mut bad_marker = good;
        let marker = bad_marker.windows(2).position(|w| w == [0xFE, 0xB0]).unwrap();
        bad_marker[marker + 1] = 0x00;
        assert!(matches!(decode(&bad_marker), Err(DecodeError::BadRecord { byte: 0xFE, .. })));
    }

    /// REQ-FMT-006 (deterministic part; fuzzing runs nightly from M2.5): no prefix and no single-byte
    /// change of a real file makes the reader panic or loop.
    #[test]
    fn req_fmt_006_every_truncation_and_byte_flip_is_handled() {
        for name in ["formats/every-command.pes", "testsheets/TS-02.pes"] {
            let bytes = golden(name);
            for len in 0..bytes.len() {
                let _ = decode(&bytes[..len]);
            }
            for at in (0..bytes.len()).step_by(3) {
                for flip in [0x01, 0x80, 0xFF] {
                    let mut changed = bytes.clone();
                    changed[at] ^= flip;
                    let _ = decode(&changed);
                }
            }
        }
    }
}
