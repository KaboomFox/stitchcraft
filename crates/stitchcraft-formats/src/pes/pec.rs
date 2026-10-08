//! The PEC block: what a Brother machine actually sews from.
//!
//! Layout, from public format descriptions; where they leave a detail open, the choice matches the output
//! of pystitch (MIT), observed as a black box, and is marked "observed" (`NOTICE`):
//!
//! | Offset | Bytes | Content |
//! |---|---|---|
//! | 0 | 3 | `LA:` |
//! | 3 | 16 | label, padded with spaces |
//! | 19 | 1 | carriage return |
//! | 20 | 12 | spaces |
//! | 32 | 4 | `FF 00 06 26`: thumbnail width in bytes (6) and height (38) |
//! | 36 | 12 | spaces |
//! | 48 | 1 | number of colour entries − 1 |
//! | 49 | 463 | one Brother palette index per colour entry, padded with spaces |
//! | 512 | 2 | `00 00` |
//! | 514 | 3 | bytes from offset 512 to the thumbnails (little-endian) |
//! | 517 | 3 | `31 FF F0` (observed constant) |
//! | 520 | 2 + 2 | design width and height in 0.1 mm (little-endian) |
//! | 524 | 2 + 2 | `E0 01`, `B0 01` (observed constants, 480 and 432) |
//! | 528 | 2 + 2 | big-endian `0x9000 \| (−min x)` and `0x9000 \| (−min y)`, 12-bit two's complement: the origin as seen from the design's top-left corner (observed) |
//! | 532 | … | stitch data |
//! | … | 228 per picture | thumbnails ([`super::thumbnail`]) |
//!
//! **Stitch data.** Each move is written per axis, x then y. A *short* axis is one byte, 7-bit two's
//! complement; StitchCraft uses it for −63…62, the range observed in reference files. A *long* axis is two
//! bytes: `0x80 | flags | high nibble`, then the low byte of a 12-bit two's complement value (±2047). Flags:
//! `0x10` jump, `0x20` trim (a trim-flagged jump). A sewn move uses short form on each axis that fits and
//! long form without flags on the others; a jump is long on both axes with a flag. A colour change — and a
//! stop, which PEC records as a change to the same thread — is `FE B0` then a byte alternating `02`, `01`,
//! starting with `02`. The end is `FF`.
//!
//! **Trims.** PEC cannot cut in place: a trim rides on the next move. The next jump gets the trim flag; a
//! sewn move becomes a trim-flagged jump to its target followed by a zero-length stitch there, so the needle
//! still goes down where the plan says. A trim with no move after it (before the end) needs no record:
//! the machine stops there.

use stitchcraft_plan::FormatId;

use crate::error::EncodeError;
use crate::label::label;
use crate::lower::{Lowered, Op, split};
use crate::quantize::Delta;

use super::thumbnail;

/// The format's name in errors.
const FORMAT: &str = "PES v1";
/// The largest move one long-form axis records.
pub(crate) const LONG_LIMIT: i32 = 2047;
/// Long-form flag bits.
const LONG: u8 = 0x80;
const JUMP: u8 = 0x10;
const TRIM: u8 = 0x20;
/// The header before the graphics header.
const HEADER_LEN: usize = 512;
/// Bytes from offset 512 to the stitch data (the graphics header).
const GRAPHICS_HEADER_LEN: usize = 20;

/// The PEC block for `lowered`.
pub(crate) fn block(lowered: &Lowered, name: &str) -> Result<Vec<u8>, EncodeError> {
    let stitches = stitch_data(&lowered.ops);
    let mut out = Vec::with_capacity(HEADER_LEN + GRAPHICS_HEADER_LEN + stitches.len() + thumbnail::BYTES * (1 + lowered.entries.len()));

    out.extend_from_slice(b"LA:");
    out.extend_from_slice(&label(name));
    out.push(b'\r');
    out.extend_from_slice(&[b' '; 12]);
    out.extend_from_slice(&[0xFF, 0x00, 0x06, 0x26]);
    out.extend_from_slice(&[b' '; 12]);
    let last_entry = u8::try_from(lowered.changes()).map_err(|_| too_many(lowered.changes()))?;
    out.push(last_entry);
    let palette = FormatId::PesV1.palette().map(|id| id.palette()).ok_or(EncodeError::NoPaletteMatch { palette: "PES" })?;
    for color in &lowered.entries {
        let entry = palette.nearest(*color).ok_or(EncodeError::NoPaletteMatch { palette: palette.name })?;
        out.push(entry.index);
    }
    out.resize(HEADER_LEN, b' ');

    let to_thumbnails = GRAPHICS_HEADER_LEN + stitches.len();
    let to_thumbnails = u32::try_from(to_thumbnails).ok().filter(|n| *n < 1 << 24).ok_or_else(|| too_large("the stitch data"))?;
    let [a, b, c, _] = to_thumbnails.to_le_bytes();
    out.extend_from_slice(&[0x00, 0x00, a, b, c, 0x31, 0xFF, 0xF0]);
    let (min, max) = lowered.bounds;
    let extent = |lo: i32, hi: i32, what: &str| u16::try_from(i64::from(hi) - i64::from(lo)).map_err(|_| too_large(what));
    out.extend_from_slice(&extent(min.x, max.x, "the design's width")?.to_le_bytes());
    out.extend_from_slice(&extent(min.y, max.y, "the design's height")?.to_le_bytes());
    out.extend_from_slice(&[0xE0, 0x01, 0xB0, 0x01]);
    for coordinate in [min.x, min.y] {
        let [low, high, _, _] = (0x9000 | (coordinate.saturating_neg() & 0x0FFF)).to_le_bytes();
        out.extend_from_slice(&[high, low]);
    }
    out.extend_from_slice(&stitches);
    out.extend_from_slice(&thumbnail::thumbnails(lowered));
    Ok(out)
}

fn too_many(changes: usize) -> EncodeError {
    EncodeError::TooManyColorChanges { format: FORMAT, changes, max: FormatId::PesV1.max_color_changes() }
}

fn too_large(what: &str) -> EncodeError {
    EncodeError::TooLarge { format: FORMAT, what: what.to_string() }
}

/// The stitch data for `ops` (see the module docs).
pub(crate) fn stitch_data(ops: &[Op]) -> Vec<u8> {
    let mut out = Vec::with_capacity(ops.len() * 2);
    let mut pending_trim = false;
    let mut marker_two = true;
    for op in ops {
        match *op {
            Op::Stitch(delta) if pending_trim => {
                jump(&mut out, delta, TRIM);
                sewn(&mut out, Delta::ZERO);
                pending_trim = false;
            }
            Op::Stitch(delta) => {
                let pieces = split(delta, LONG_LIMIT);
                let last = pieces.len().saturating_sub(1);
                for (i, piece) in pieces.enumerate() {
                    if i < last {
                        long(&mut out, piece, JUMP);
                    } else {
                        sewn(&mut out, piece);
                    }
                }
            }
            Op::Jump(delta) => {
                jump(&mut out, delta, if pending_trim { TRIM } else { JUMP });
                pending_trim = false;
            }
            Op::Trim => pending_trim = true,
            Op::Stop | Op::ColorChange => {
                out.extend_from_slice(&[0xFE, 0xB0, if marker_two { 0x02 } else { 0x01 }]);
                marker_two = !marker_two;
            }
            Op::End => out.push(0xFF),
        }
    }
    out
}

/// A jump of any length: long records, the first with `first_flag`, the rest with the jump flag.
fn jump(out: &mut Vec<u8>, delta: Delta, first_flag: u8) {
    for (i, piece) in split(delta, LONG_LIMIT).enumerate() {
        long(out, piece, if i == 0 { first_flag } else { JUMP });
    }
}

/// A sewn move within the long-form range: short form on each axis that fits, long form on the others.
fn sewn(out: &mut Vec<u8>, delta: Delta) {
    for v in [delta.dx, delta.dy] {
        if (-63..=62).contains(&v) {
            let [byte, ..] = (v & 0x7F).to_le_bytes();
            out.push(byte);
        } else {
            axis_long(out, v, 0);
        }
    }
}

/// Both axes in long form with `flag`.
fn long(out: &mut Vec<u8>, delta: Delta, flag: u8) {
    axis_long(out, delta.dx, flag);
    axis_long(out, delta.dy, flag);
}

/// One axis in long form: `0x80 | flag | high nibble`, low byte (12-bit two's complement).
fn axis_long(out: &mut Vec<u8>, v: i32, flag: u8) {
    let [low, high, ..] = (v & 0x0FFF).to_le_bytes();
    out.extend_from_slice(&[LONG | flag | high, low]);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(dx: i32, dy: i32) -> Delta {
        Delta { dx, dy }
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
    }

    /// Spec vectors: each operation and its bytes (the reference values were observed in pystitch output).
    #[test]
    fn encodes_each_operation() {
        let cases: [(&[Op], &str); 11] = [
            (&[Op::Jump(d(100, 50))], "90 64 90 32"),
            (&[Op::Stitch(d(0, 0))], "00 00"),
            (&[Op::Stitch(d(30, -30))], "1E 62"),
            (&[Op::Stitch(d(62, -63))], "3E 41"),
            (&[Op::Stitch(d(63, 0))], "80 3F 00"),
            (&[Op::Stitch(d(-64, 0))], "8F C0 00"),
            (&[Op::Stitch(d(200, 100))], "80 C8 80 64"),
            (&[Op::Trim, Op::Jump(d(171, 0))], "A0 AB A0 00"),
            (&[Op::Jump(d(-540, -195))], "9D E4 9F 3D"),
            (&[Op::ColorChange, Op::Stop, Op::ColorChange], "FE B0 02 FE B0 01 FE B0 02"),
            (&[Op::End], "FF"),
        ];
        for (ops, expected) in cases {
            assert_eq!(hex(&stitch_data(ops)), expected, "{ops:?}");
        }
    }

    #[test]
    fn a_trim_rides_on_the_next_move() {
        // Trim, then a sewn move: trim-flagged jump to the target, then a zero stitch there.
        assert_eq!(hex(&stitch_data(&[Op::Trim, Op::Stitch(d(30, 0))])), "A0 1E A0 00 00 00");
        // Trim, colour change, jump: the jump after the change carries the trim.
        assert_eq!(hex(&stitch_data(&[Op::Trim, Op::ColorChange, Op::Jump(d(30, 0))])), "FE B0 02 A0 1E A0 00");
        // A trim before the end needs no record.
        assert_eq!(hex(&stitch_data(&[Op::Trim, Op::End])), "FF");
        // Only the first piece of a split jump carries the trim.
        assert_eq!(hex(&stitch_data(&[Op::Trim, Op::Jump(d(3000, 0))])), "A5 DC A0 00 95 DC 90 00");
    }

    /// REQ-FMT-007: moves beyond ±2047 split within the limit; a long sewn move is jumps, then the stitch.
    #[test]
    fn long_moves_are_split() {
        assert_eq!(hex(&stitch_data(&[Op::Jump(d(3000, 0))])), "95 DC 90 00 95 DC 90 00");
        assert_eq!(hex(&stitch_data(&[Op::Stitch(d(-3000, 10))])), "9A 24 90 05 8A 24 05");
    }
}
