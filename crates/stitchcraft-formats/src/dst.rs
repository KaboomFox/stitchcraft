//! Tajima DST: the format every embroidery machine and digitizer reads.
//!
//! A 512-byte text header, then 3-byte records. Each record moves the frame by at most ±121 units
//! (12.1 mm) per axis, written as balanced-ternary digits (1, 3, 9, 27, 81) spread over the three bytes:
//!
//! | Byte | Bit 7 | 6 | 5 | 4 | 3 | 2 | 1 | 0 |
//! |---|---|---|---|---|---|---|---|---|
//! | 1 | y+1 | y−1 | y+9 | y−9 | x−9 | x+9 | x−1 | x+1 |
//! | 2 | y+3 | y−3 | y+27 | y−27 | x−27 | x+27 | x−3 | x+3 |
//! | 3 | jump | colour change | y+81 | y−81 | x−81 | x+81 | 1 | 1 |
//!
//! **y is up** in DST, so the plan's downward y is negated. A colour change — and a stop, which DST
//! cannot tell apart — is `00 00 C3`; the end is `00 00 F3`. DST has no trim command: a trim is three
//! small jumps that cancel out, (+2, −2), (−4, +4), (+2, −2), which machines read as a trim (the sequence
//! matches the output of pystitch (MIT), observed as a black box; `NOTICE`). Moves longer than one record
//! are split evenly into jumps, with a sewn move's last piece sewn (REQ-FMT-007).
//!
//! **Header fields** (`LA` label, `ST` records before the end record, `CO` colour changes, `+X -X +Y -Y`
//! extents from the start in DST axes, `AX AY` the end position, `MX MY` zero, `PD ******`) are
//! fixed-width ASCII terminated by carriage returns, then `0x1A`, padded with spaces to 512 bytes.

use stitchcraft_plan::{FormatId, StitchPlan};

use crate::error::EncodeError;
use crate::label::label;
use crate::lower::{Op, lower, split};

/// The largest move one record makes along an axis.
pub const RECORD_LIMIT: i32 = 121;
/// The header's size.
const HEADER_LEN: usize = 512;
/// A trim: three jumps that cancel out (DST axes, y up).
const TRIM_JUMPS: [(i32, i32); 3] = [(2, -2), (-4, 4), (2, -2)];
/// Record kinds (third byte, before the 81-digits are added).
const SEW: u8 = 0x03;
const JUMP: u8 = 0x83;
const COLOR_CHANGE: [u8; 3] = [0x00, 0x00, 0xC3];
const END: [u8; 3] = [0x00, 0x00, 0xF3];

/// `plan` as a DST file whose design name is `name`.
pub fn encode(plan: &StitchPlan, name: &str) -> Result<Vec<u8>, EncodeError> {
    let format = FormatId::Dst;
    let lowered = lower(plan, format.name())?;
    if lowered.changes() > format.max_color_changes() {
        return Err(EncodeError::TooManyColorChanges { format: format.name(), changes: lowered.changes(), max: format.max_color_changes() });
    }

    let mut records = Records::default();
    for op in &lowered.ops {
        match *op {
            Op::Stitch(delta) => {
                let pieces = split(delta, RECORD_LIMIT);
                let last = pieces.len().saturating_sub(1);
                for (i, piece) in pieces.enumerate() {
                    records.moving(piece.dx, -piece.dy, if i < last { JUMP } else { SEW });
                }
            }
            Op::Jump(delta) => {
                for piece in split(delta, RECORD_LIMIT) {
                    records.moving(piece.dx, -piece.dy, JUMP);
                }
            }
            Op::Trim => {
                for (dx, dy) in TRIM_JUMPS {
                    records.moving(dx, dy, JUMP);
                }
            }
            Op::Stop | Op::ColorChange => {
                records.bytes.extend_from_slice(&COLOR_CHANGE);
                records.count += 1;
                records.color_changes += 1;
            }
            Op::End => records.bytes.extend_from_slice(&END),
        }
    }

    let mut out = header(name, &records)?;
    out.extend_from_slice(&records.bytes);
    Ok(out)
}

/// The records written so far, and what the header reports about them.
#[derive(Default)]
struct Records {
    bytes: Vec<u8>,
    /// Records before the end record.
    count: usize,
    color_changes: usize,
    /// Position in DST axes (y up), and its extremes including the start.
    at: (i32, i32),
    min: (i32, i32),
    max: (i32, i32),
}

impl Records {
    /// A record moving by (`dx`, `dy`) in DST axes; both within ±[`RECORD_LIMIT`].
    fn moving(&mut self, dx: i32, dy: i32, kind: u8) {
        self.bytes.extend_from_slice(&record(dx, dy, kind));
        self.count += 1;
        self.at = (self.at.0 + dx, self.at.1 + dy);
        self.min = (self.min.0.min(self.at.0), self.min.1.min(self.at.1));
        self.max = (self.max.0.max(self.at.0), self.max.1.max(self.at.1));
    }
}

/// One record: (`dx`, `dy`) in DST axes as balanced-ternary bits, plus `kind` in the third byte.
fn record(dx: i32, dy: i32, kind: u8) -> [u8; 3] {
    // Bits for the digits 1, 3, 9, 27, 81: (byte, bit for +1, bit for −1).
    const X: [(usize, u8, u8); 5] = [(0, 0x01, 0x02), (1, 0x01, 0x02), (0, 0x04, 0x08), (1, 0x04, 0x08), (2, 0x04, 0x08)];
    const Y: [(usize, u8, u8); 5] = [(0, 0x80, 0x40), (1, 0x80, 0x40), (0, 0x20, 0x10), (1, 0x20, 0x10), (2, 0x20, 0x10)];
    let mut bytes = [0, 0, kind];
    for (value, bits) in [(dx, X), (dy, Y)] {
        for (digit, (byte, plus, minus)) in balanced_ternary(value).into_iter().zip(bits) {
            if let Some(b) = bytes.get_mut(byte) {
                match digit {
                    1 => *b |= plus,
                    -1 => *b |= minus,
                    _ => {}
                }
            }
        }
    }
    bytes
}

/// The balanced-ternary digits of `value` (−121…121) for 1, 3, 9, 27 and 81.
fn balanced_ternary(mut value: i32) -> [i32; 5] {
    let mut digits = [0; 5];
    for digit in &mut digits {
        *digit = match value.rem_euclid(3) {
            0 => 0,
            1 => 1,
            _ => -1,
        };
        value = (value - *digit) / 3;
    }
    digits
}

/// The 512-byte header.
fn header(name: &str, records: &Records) -> Result<Vec<u8>, EncodeError> {
    let too_large = |what: &str| EncodeError::TooLarge { format: FormatId::Dst.name(), what: what.to_string() };
    let fits = |value: i64, width: u32| value < 10_i64.pow(width);
    let count = i64::try_from(records.count).unwrap_or(i64::MAX);
    if !fits(count, 7) {
        return Err(too_large("the number of records"));
    }
    let extents = [records.max.0, -records.min.0, records.max.1, -records.min.1].map(|v| i64::from(v.max(0)));
    let end = (i64::from(records.at.0), i64::from(records.at.1));
    if extents.iter().chain([&end.0.abs(), &end.1.abs()]).any(|v| !fits(*v, 5)) {
        return Err(too_large("the design's extent"));
    }
    let sign = |v: i64| if v < 0 { '-' } else { '+' };
    let mut out = Vec::with_capacity(HEADER_LEN);
    out.extend_from_slice(b"LA:");
    out.extend_from_slice(&label(name));
    let [px, mx, py, my] = extents;
    let text = format!(
        "\rST:{count:>7}\rCO:{:>3}\r+X:{px:>5}\r-X:{mx:>5}\r+Y:{py:>5}\r-Y:{my:>5}\rAX:{}{:>5}\rAY:{}{:>5}\rMX:+{:>5}\rMY:+{:>5}\rPD:******\r",
        records.color_changes,
        sign(end.0),
        end.0.abs(),
        sign(end.1),
        end.1.abs(),
        0,
        0,
    );
    out.extend_from_slice(text.as_bytes());
    out.push(0x1A);
    out.resize(HEADER_LEN, b' ');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{b:02X}")).collect::<Vec<_>>().join(" ")
    }

    /// Spec vectors (reference values observed in pystitch output; DST axes, y up).
    #[test]
    fn records_encode_balanced_ternary() {
        let cases = [
            ((100, -50, JUMP), "89 A4 97"),
            ((0, 0, SEW), "00 00 03"),
            ((30, 0, SEW), "00 05 03"),
            ((0, -30, SEW), "00 50 03"),
            ((63, 0, SEW), "04 08 07"),
            ((-64, 0, SEW), "0A 04 0B"),
            ((2, -2, JUMP), "82 41 83"),
            ((-4, 4, JUMP), "82 82 83"),
            ((-108, 39, JUMP), "20 A8 8B"),
            ((121, -121, SEW), "55 55 17"),
        ];
        for ((dx, dy, kind), expected) in cases {
            assert_eq!(hex(&record(dx, dy, kind)), expected, "({dx}, {dy})");
        }
    }

    #[test]
    fn every_value_in_range_round_trips_through_its_digits() {
        for v in -121..=121 {
            let digits = balanced_ternary(v);
            assert_eq!(digits.iter().zip([1, 3, 9, 27, 81]).map(|(d, w)| d * w).sum::<i32>(), v);
            assert!(digits.iter().all(|d| (-1..=1).contains(d)));
        }
    }
}
