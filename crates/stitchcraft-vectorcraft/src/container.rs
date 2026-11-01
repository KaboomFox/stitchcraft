//! A `.vectorcraft` file's JSON: unpacked when the file is compressed, and cut before the data format 4
//! can keep after it.
//!
//! **Why the first NUL byte.** Format 4 keeps images and other large data after the JSON, behind the
//! marker `\n\0VCBLOBS\0` (VectorCraft's `crates/format`). JSON text never holds a NUL byte, so the JSON
//! ends where the first one is, one byte before it. The data after the marker is never needed: it is
//! not unpacked, and a plain file's can be as large as it likes.
//!
//! **Why pieces.** A compressed file is unpacked a piece at a time, up to the marker and never past
//! [`MAX_BYTES`] of JSON, so that a small file that unpacks to a huge one costs no more than the limit.
//!
//! **Why a part.** A bug-report bundle needs what reading the file needs, and no more: [`read_part`]
//! leaves out the data after a plain file's JSON, its images among them, which reading never looks at.

use std::borrow::Cow;
use std::io::Read;

use flate2::read::GzDecoder;
use stitchcraft_core::{Code, Diagnostic, Meter};

/// The most JSON a `.vectorcraft` file may hold, unpacked: 64 MiB, as much as an SVG file may be.
pub const MAX_BYTES: usize = 64 << 20;

/// The first bytes of a gzip stream.
const GZIP: [u8; 2] = [0x1f, 0x8b];

/// What separates the JSON from the data after it (format 4).
const MARKER: &[u8] = b"\n\0VCBLOBS\0";

/// How many bytes are unpacked at a time.
const PIECE: usize = 64 << 10;

/// Bytes read, unpacked or copied for each unit of work.
pub(crate) const BYTES_PER_UNIT: usize = 64;

/// The JSON of the `.vectorcraft` file `bytes`, charged to `meter`: `SC-E0806` when the file is larger than
/// the adapter reads, its compressed stream is damaged or its data after the JSON is not behind the
/// marker; `SC-E0004` when the budget runs out.
pub(crate) fn json<'a>(bytes: &'a [u8], meter: &mut Meter) -> Result<Cow<'a, [u8]>, Diagnostic> {
    let json = if bytes.starts_with(&GZIP) {
        let unpacked = unpack(bytes, meter)?;
        let end = end_of_json(&unpacked)?;
        let mut unpacked = unpacked;
        unpacked.truncate(end);
        Cow::Owned(unpacked)
    } else {
        let end = end_of_json(bytes)?;
        Cow::Borrowed(bytes.get(..end).unwrap_or_default())
    };
    if json.len() > MAX_BYTES {
        return Err(too_large());
    }
    charge(meter, json.len())?;
    Ok(json)
}

/// The part of the `.vectorcraft` file `bytes` that [`read`](crate::read) reads, which reads as the file
/// does: a plain file's JSON, before the marker and the data after it. A compressed file, and one whose JSON does not end
/// at the marker, are read whole, so their part is the file.
pub fn read_part(bytes: &[u8]) -> &[u8] {
    if bytes.starts_with(&GZIP) {
        return bytes;
    }
    end_of_json(bytes).ok().and_then(|end| bytes.get(..end)).unwrap_or(bytes)
}

/// Unpacks the gzip stream `bytes` up to the end of the marker, whichever piece it ends in, and a little
/// past the limit at most.
fn unpack(bytes: &[u8], meter: &mut Meter) -> Result<Vec<u8>, Diagnostic> {
    let mut decoder = GzDecoder::new(bytes);
    let mut unpacked = Vec::new();
    let mut piece = vec![0; PIECE];
    // Where the first NUL byte is, the marker's second byte.
    let mut nul = None;
    loop {
        let n = decoder.read(&mut piece).map_err(|e| unreadable(&format!("Its compressed data does not unpack: {e}.")))?;
        let Some(read) = piece.get(..n).filter(|read| !read.is_empty()) else { return Ok(unpacked) };
        charge(meter, n)?;
        let start = unpacked.len();
        unpacked.extend_from_slice(read);
        nul = nul.or_else(|| read.iter().position(|&b| b == 0).map(|i| start + i));
        // The marker starts a byte before the NUL; a NUL first has no marker, and reads as damage.
        match nul {
            Some(at) if unpacked.len() >= at + MARKER.len() - 1 => return Ok(unpacked),
            None if unpacked.len() > MAX_BYTES => return Err(too_large()),
            _ => {}
        }
    }
}

/// Where the JSON in `bytes` ends: at the marker before the data after it, or at the end. Only as many
/// bytes are searched as the most JSON read and the marker's first two bytes.
fn end_of_json(bytes: &[u8]) -> Result<usize, Diagnostic> {
    let searched = bytes.get(..MAX_BYTES + 2).unwrap_or(bytes);
    let Some(nul) = searched.iter().position(|&b| b == 0) else { return Ok(bytes.len()) };
    match nul.checked_sub(1) {
        Some(end) if bytes.get(end..end + MARKER.len()) == Some(MARKER) => Ok(end),
        _ => Err(unreadable(&format!("It has a NUL byte at offset {nul}, where VectorCraft keeps none but in the marker before its data."))),
    }
}

/// Charges `meter` for reading `len` bytes.
fn charge(meter: &mut Meter, len: usize) -> Result<(), Diagnostic> {
    meter.charge(1 + (len / BYTES_PER_UNIT) as u64).map_err(|_| crate::document::exhausted())
}

/// `SC-E0806` for a file that unpacks to more JSON than the adapter reads.
fn too_large() -> Diagnostic {
    unreadable(&format!("Its JSON is larger than the {} MiB StitchCraft reads.", MAX_BYTES >> 20))
}

/// `SC-E0806`: the file is not a VectorCraft document StitchCraft can read, because of `what`.
pub(crate) fn unreadable(what: &str) -> Diagnostic {
    Diagnostic::new(Code::VcUnreadable, format!("The file could not be read as a VectorCraft document. {what}"))
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use flate2::Compression;
    use flate2::write::GzEncoder;
    use stitchcraft_core::Budget;

    use super::*;

    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }

    fn read(bytes: &[u8]) -> Result<Vec<u8>, Diagnostic> {
        json(bytes, &mut Budget::DEFAULT.meter()).map(|json| json.into_owned())
    }

    #[test]
    fn the_json_ends_at_the_marker_in_plain_and_compressed_files() {
        let tail = [&b"{\"a\": 1}"[..], MARKER, &[7, 0, 8, 255]].concat();
        for bytes in [tail.clone(), gzip(&tail)] {
            assert_eq!(read(&bytes).unwrap(), b"{\"a\": 1}");
        }
        for bytes in [b"{}".to_vec(), gzip(b"{}")] {
            assert_eq!(read(&bytes).unwrap(), b"{}");
        }
    }

    #[test]
    fn the_marker_is_found_where_unpacking_cuts_it() {
        // JSON that ends near the end of the first piece unpacked, so that a piece ends inside the marker.
        for len in PIECE - MARKER.len() - 2..=PIECE + 2 {
            let json = [&b"{"[..], &vec![b' '; len - 2], b"}"].concat();
            let file = [&json[..], MARKER, &[1, 0, 2]].concat();
            assert_eq!(read(&gzip(&file)).unwrap(), json, "JSON of {len} bytes");
        }
    }

    #[test]
    fn unpacking_stops_at_the_end_of_the_marker() {
        // JSON whose marker ends where the first piece unpacked ends, then a megabyte of data after it: the
        // data is never unpacked, so what the first piece and the JSON cost is enough.
        let text = [&b"{"[..], &vec![b' '; PIECE - MARKER.len() - 2], b"}"].concat();
        let file = gzip(&[&text[..], MARKER, &vec![7; 1 << 20]].concat());
        let mut meter = Budget { max_stitches: 1, max_work: 2 * (1 + PIECE / BYTES_PER_UNIT) as u64 }.meter();
        assert_eq!(json(&file, &mut meter).unwrap(), text);
    }

    #[test]
    fn diag_sc_e0806_a_nul_byte_outside_the_marker_is_damage() {
        // Plain and compressed, a NUL byte first among them, with no byte before it to start a marker.
        for plain in [b"{\"a\": \0}".to_vec(), b"\0{}".to_vec(), b"\0".to_vec(), [&b"{}"[..], b"\n\0VCBLOB\0"].concat()] {
            for bytes in [gzip(&plain), plain] {
                let refusal = read(&bytes).unwrap_err();
                assert_eq!(refusal.code, Code::VcUnreadable, "{refusal}");
                assert!(refusal.message.contains("NUL byte"), "{refusal}");
            }
        }
    }

    #[test]
    fn diag_sc_e0806_a_damaged_stream_is_refused() {
        let mut damaged = gzip(b"{\"layers\": []}");
        damaged.truncate(damaged.len() - 9);
        assert!(read(&damaged).unwrap_err().message.contains("does not unpack"));
    }

    #[test]
    fn diag_sc_e0806_more_json_than_is_read_is_refused() {
        let too_large = |result: Result<Vec<u8>, Diagnostic>| result.is_err_and(|e| e.message.contains("larger than the 64 MiB"));
        // A small compressed file that unpacks to far more than the limit stops just past it: unpacking
        // 1,025 pieces of 64 KiB costs 1,050,625 units.
        let bomb = gzip(&vec![b' '; 2 * MAX_BYTES]);
        assert!(bomb.len() < 1 << 20);
        assert!(too_large(json(&bomb, &mut Budget { max_stitches: 1, max_work: 1_100_000 }.meter()).map(Cow::into_owned)));
        // As much JSON as is read, plain or compressed, is read whole, and with the marker after it.
        let most = vec![b' '; MAX_BYTES];
        assert_eq!(read(&most).unwrap().len(), MAX_BYTES);
        assert_eq!(read(&gzip(&most)).unwrap().len(), MAX_BYTES);
        assert_eq!(read(&[&most[..], MARKER, b"data"].concat()).unwrap().len(), MAX_BYTES);
        assert!(too_large(read(&[&most[..], b" "].concat())));
        // A NUL byte past that much JSON is past the limit, whatever follows it.
        assert!(too_large(read(&[&most[..], b"  \0"].concat())));
    }

    #[test]
    fn the_part_a_bundle_keeps_reads_as_the_file_does() {
        let plain = [&b"{\"a\": 1}"[..], MARKER, &[7, 0, 8, 255]].concat();
        assert_eq!(read_part(&plain), b"{\"a\": 1}");
        // Read whole: a file without the marker, a compressed one, and one with a NUL byte of its own.
        let damaged = b"{\"a\": \0}".to_vec();
        for whole in [b"{}".to_vec(), gzip(&plain), damaged] {
            assert_eq!(read_part(&whole), whole.as_slice());
        }
        for bytes in [plain, gzip(b"{}"), b"{\"a\": \0}".to_vec(), [&vec![b' '; MAX_BYTES][..], b" "].concat()] {
            assert_eq!(read(read_part(&bytes)), read(&bytes));
        }
    }

    #[test]
    fn reading_is_charged_by_the_byte() {
        let mut meter = Budget { max_stitches: 1, max_work: 10 }.meter();
        assert_eq!(json(&[b' '; 640], &mut meter).unwrap_err().code, Code::BudgetExhausted);
        let mut meter = Budget { max_stitches: 1, max_work: 11 }.meter();
        assert!(json(&[b' '; 640], &mut meter).is_ok());
    }
}
