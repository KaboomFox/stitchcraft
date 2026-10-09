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
//!
//! **The origin field** (`REQ-FMT-009`). Brother's software writes 4 bytes before the first record: a
//! long-form jump on both axes, from the corner of the box the header gives to the start, which the
//! reader skips. pyembroidery 1.4.32 to 1.5.1 leave it out, and their first record can be a long jump
//! shaped just like it. So the 4 bytes are taken as the field only when they are shaped like one and the
//! design read after them fits the box: the header's width and height, from the corner the field puts it.
//! Otherwise the data is read from its first record, with a warning, because Brother machines expect the
//! field. In files from PE-Design and pystitch the field is the design's top-left corner; some writers
//! use a larger box with the design in its middle.

use stitchcraft_plan::palette::BROTHER_PEC;
use stitchcraft_plan::{PaletteId, Thread};

use crate::decode::{Decoded, Recorder, label_text};
use crate::error::DecodeError;
use crate::quantize::Delta;

const FORMAT: &str = "PEC";
/// The PEC header and graphics header, before the stitch data.
const DATA_AT: usize = 528;
/// The origin field, which Brother's software writes before the first record.
const ORIGIN_FIELD: usize = 4;
/// Where the design may stray outside the header's box, in 0.1 mm: writers round the box.
const SLACK: i64 = 1;
/// The warning for stitch data without the origin field.
const NO_ORIGIN_FIELD: &str = "The PEC stitch data has no origin field (the 4 bytes Brother's software writes before the first \
    stitch), as pyembroidery 1.4.32 to 1.5.1 write PEC. It was read from its first record. Brother machines expect the field: \
    `stitch convert` writes the file again with it.";

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
    let header = block.get(..DATA_AT).ok_or(DecodeError::Truncated { part: "PEC header" })?;
    let name = label_text(header.get(3..19).unwrap_or_default());
    let entries = usize::from(header.get(48).copied().unwrap_or(0)) + 1;
    let indices = header.get(49..49 + entries).ok_or(DecodeError::Truncated { part: "PEC colour list" })?;
    let mut warnings = Vec::new();

    let graphics = header
        .get(514..517)
        .and_then(|b| <[u8; 3]>::try_from(b).ok())
        .map_or(0, |[a, b, c]| usize::from(a) | usize::from(b) << 8 | usize::from(c) << 16);
    let thumbnails_at = 512 + graphics;
    let end = if (DATA_AT..=block.len()).contains(&thumbnails_at) {
        thumbnails_at
    } else {
        warnings.push(format!("The thumbnail offset ({graphics}) points outside the file; stitches were read to the end of the file."));
        block.len()
    };
    let size = |at: usize| header.get(at..at + 2).and_then(|b| <[u8; 2]>::try_from(b).ok()).map_or(0, |b| i64::from(u16::from_le_bytes(b)));
    let start = data_start(block, end, (size(520), size(522)));
    if start == DATA_AT {
        warnings.push(NO_ORIGIN_FIELD.to_string());
    }
    let data = block.get(start..end).unwrap_or_default();

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
    while let Some((record, next)) = record(data, i, base + start + i)? {
        i = next;
        match record {
            Record::Change => {
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
            Record::Move { delta, flags } => {
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
    Ok(Decoded { format: String::new(), palette: Some(PaletteId::BrotherPec), name, plan: recorder.finish(), warnings })
}

/// The lowest and highest corners of the positions a design reaches, in 0.1 mm.
type Corners = ((i64, i64), (i64, i64));

/// Where the stitch data of `block` starts: after the origin field when the 4 bytes at [`DATA_AT`] are
/// shaped like one and the design read after them fits the header's box of `size` (width and height in
/// 0.1 mm), else at the first record.
///
/// Bytes shaped like the field also read as one record, a long jump, so the data after them parses
/// exactly when the data from [`DATA_AT`] does. Data that does not parse is read after the field, so
/// the error is the one a file with the field gives.
fn data_start(block: &[u8], end: usize, size: (i64, i64)) -> usize {
    let after_field = DATA_AT + ORIGIN_FIELD;
    let Some(field) = block.get(DATA_AT..after_field).and_then(origin_field) else { return DATA_AT };
    match block.get(after_field..end).map(extent) {
        Some(Ok(extent)) if !fits(field, size, extent) => DATA_AT,
        _ => after_field,
    }
}

/// The offset the origin field holds, if `bytes` are shaped like one: a long-form record on both axes
/// with the jump flag alone. Only the long form has flags, so the flags say the form too.
fn origin_field(bytes: &[u8]) -> Option<(i64, i64)> {
    let (x, flags_x, after_x) = axis(bytes, 0).ok()?;
    let (y, flags_y, _) = axis(bytes, after_x).ok()?;
    (flags_x == 0x10 && flags_y == 0x10).then_some((i64::from(x), i64::from(y)))
}

/// Whether a design with `extent` (the corners of every position it reaches, if any) fits the box of
/// `size` whose corner is at minus `field`, give or take [`SLACK`].
fn fits(field: (i64, i64), size: (i64, i64), extent: Option<Corners>) -> bool {
    let Some(((min_x, min_y), (max_x, max_y))) = extent else { return true };
    let inside = |field: i64, size: i64, min: i64, max: i64| -field <= min + SLACK && max <= size - field + SLACK;
    inside(field.0, size.0, min_x, max_x) && inside(field.1, size.1, min_y, max_y)
}

/// The corners of every position the stitch data `data` reaches from the origin, `None` without a move.
fn extent(data: &[u8]) -> Result<Option<Corners>, DecodeError> {
    let (mut i, mut x, mut y) = (0, 0_i64, 0_i64);
    let mut corners: Option<Corners> = None;
    while let Some((record, next)) = record(data, i, 0)? {
        i = next;
        if let Record::Move { delta, .. } = record {
            (x, y) = (x + i64::from(delta.dx), y + i64::from(delta.dy));
            let ((lo_x, lo_y), (hi_x, hi_y)) = corners.unwrap_or(((x, y), (x, y)));
            corners = Some(((lo_x.min(x), lo_y.min(y)), (hi_x.max(x), hi_y.max(y))));
        }
    }
    Ok(corners)
}

/// One record of PEC stitch data before its end mark.
enum Record {
    /// A move: sewn, or a jump (flag `0x10`) or trim (flag `0x20`) in long form, on either axis.
    Move { delta: Delta, flags: u8 },
    /// A colour change, or a stop: `FE B0` and a byte.
    Change,
}

/// The record at `i` of `data` and where the next one starts, or `None` at the end mark (`FF`). `at` is
/// its position in the file, for errors.
fn record(data: &[u8], i: usize, at: usize) -> Result<Option<(Record, usize)>, DecodeError> {
    let Some(&first) = data.get(i) else { return Err(DecodeError::MissingEnd) };
    match first {
        0xFF => Ok(None),
        0xFE => {
            match data.get(i + 1) {
                Some(0xB0) => {}
                Some(_) => return Err(DecodeError::BadRecord { format: FORMAT, at, byte: first }),
                None => return Err(DecodeError::Truncated { part: "stitch data" }),
            }
            data.get(i + 2).ok_or(DecodeError::Truncated { part: "stitch data" })?;
            Ok(Some((Record::Change, i + 3)))
        }
        _ => {
            let (dx, flags_x, after_x) = axis(data, i)?;
            let (dy, flags_y, after_y) = axis(data, after_x)?;
            Ok(Some((Record::Move { delta: Delta { dx, dy }, flags: flags_x | flags_y }, after_y)))
        }
    }
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
        let stitches = 22 + DATA_AT + ORIGIN_FIELD;
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
        // A colour-change marker must be followed by 0xB0. The error gives the marker's place in the file.
        let mut bad_marker = good;
        let marker = bad_marker.windows(2).position(|w| w == [0xFE, 0xB0]).unwrap();
        bad_marker[marker + 1] = 0x00;
        assert_eq!(decode(&bad_marker), Err(DecodeError::BadRecord { format: "PEC", at: marker, byte: 0xFE }));
        // A colour change needs the byte after its marker.
        let mut cut_change = golden("formats/one-stitch.pes");
        let stitches = 22 + DATA_AT + ORIGIN_FIELD;
        cut_change.splice(stitches..stitches + 3, [0xFE, 0xB0]);
        cut_change[22 + 514] -= 1;
        assert_eq!(decode(&cut_change), Err(DecodeError::Truncated { part: "stitch data" }));
    }

    #[test]
    fn a_jump_or_trim_flag_on_either_axis_marks_the_record() {
        use StitchKind::{Jump, Normal, Trim};
        // A jump flagged on x alone (x long, y short), a trim flagged on y alone, a stitch, the end; in a
        // box that fits them.
        let mut bytes = golden("formats/one-stitch.pes");
        let (pec, stitches) = (22, 22 + DATA_AT + ORIGIN_FIELD);
        bytes.splice(stitches..stitches + 3, [0x90, 0x10, 0x05, 0x03, 0xA0, 0x07, 0x00, 0x00, 0xFF]);
        bytes[pec + 514] += 6;
        bytes[pec + 520..pec + 524].copy_from_slice(&[19, 0, 12, 0]);
        let d = decode(&bytes).unwrap();
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        assert_eq!(kinds(&d), [(Jump, 1.6, 0.5), (Trim, 1.6, 0.5), (Jump, 1.9, 1.2), (Normal, 1.9, 1.2)]);
    }

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/../../conformance/fixtures/pes/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }

    #[test]
    fn req_fmt_009_pec_stitch_data_reads_with_or_without_the_origin_field() {
        use StitchKind::{Jump, Normal};
        // pyembroidery 1.5.1's own files (conformance/oracle/write_pes.py), which leave the field out. The
        // first record is a short stitch in one, and a long jump shaped like the field in the other.
        let origin = decode(&fixture("pyembroidery-origin-start.pes")).unwrap();
        assert_eq!(kinds(&origin), [(Normal, 0.0, 0.0), (Normal, 10.0, 0.0), (Normal, 10.0, 10.0)]);
        assert_eq!(origin.warnings, [NO_ORIGIN_FIELD]);
        let offset = decode(&fixture("pyembroidery-offset-start.pes")).unwrap();
        assert_eq!(kinds(&offset), [(Jump, 20.0, 15.0), (Normal, 20.0, 15.0), (Normal, 30.0, 15.0), (Normal, 30.0, 25.0)]);
        assert_eq!(offset.warnings, [NO_ORIGIN_FIELD]);
        // StitchCraft's own files have the field, as Brother's software writes it.
        assert!(decode(&golden("formats/every-command.pes")).unwrap().warnings.is_empty());
    }

    #[test]
    fn req_fmt_009_a_field_for_a_larger_box_is_the_origin_field() {
        // Some writers put the design in a 99.9 mm box, centred, and write the offset to the box's corner:
        // the header's size is the box's, and the field is 49.9 mm. The data starts where the field says.
        let mut bytes = golden("formats/one-stitch.pes");
        let pec = 22;
        bytes[pec + 520..pec + 524].copy_from_slice(&[0xE7, 0x03, 0xE7, 0x03]);
        bytes[pec + 528..pec + 532].copy_from_slice(&[0x91, 0xF3, 0x91, 0xF3]);
        let d = decode(&bytes).unwrap();
        assert_eq!(kinds(&d), [(StitchKind::Normal, 0.0, 0.0)]);
        assert!(d.warnings.is_empty(), "{:?}", d.warnings);
        // The same field over a design that does not fit the box is a jump, as pyembroidery writes one.
        bytes[pec + 520..pec + 524].copy_from_slice(&[0x0A, 0x00, 0x0A, 0x00]);
        let d = decode(&bytes).unwrap();
        assert_eq!(kinds(&d), [(StitchKind::Jump, 49.9, 49.9), (StitchKind::Normal, 49.9, 49.9)]);
        assert_eq!(d.warnings, [NO_ORIGIN_FIELD]);
    }

    #[test]
    fn req_fmt_009_the_origin_field_is_a_long_jump_on_both_axes() {
        assert_eq!(origin_field(&[0x91, 0xF3, 0x91, 0xF3]), Some((499, 499)));
        assert_eq!(origin_field(&[0x9F, 0xFF, 0x90, 0x01]), Some((-1, 1)));
        // A trim flag on either axis, or a short axis, is a record of the stitch data.
        for bytes in [[0xB0, 0x00, 0x90, 0x00], [0x90, 0x00, 0xA0, 0x00], [0x90, 0x00, 0x05, 0x00], [0x05, 0x90, 0x00, 0x00]] {
            assert_eq!(origin_field(&bytes), None, "{bytes:02X?}");
        }
        assert_eq!(origin_field(&[0x90, 0x00, 0x90]), None);
    }

    #[test]
    fn req_fmt_009_the_design_may_stray_one_unit_outside_the_box() {
        // The field puts the box's corner at (-10, -20), and the box is 100 by 200.
        let (field, size) = ((10, 20), (100, 200));
        assert!(fits(field, size, None));
        assert!(fits(field, size, Some(((-11, -21), (91, 181)))));
        for extent in [((-12, -20), (90, 180)), ((-10, -22), (90, 180)), ((-10, -20), (92, 180)), ((-10, -20), (90, 182))] {
            assert!(!fits(field, size, Some(extent)), "{extent:?}");
        }
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
