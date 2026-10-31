//! A file's bytes as VectorCraft takes them in JSON: base64, padded (RFC 4648, section 4).
//!
//! **Why here.** VectorCraft decodes an exporter's file from it, and a command such as `file.place` is
//! given a file's bytes so (`dataBase64`, VectorCraft's `docs/mcp.md`). The plug-in's exporters, the `stitch`
//! command, which gives VectorCraft's command line a picture to trace (ADR-0015), and the compatibility gate
//! all write it, and this is the one encoder they share.

/// `bytes` in base64, padded (RFC 4648, section 4), as VectorCraft decodes an exporter's file, and the bytes
/// of a file a command is given.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3).saturating_mul(4));
    for chunk in bytes.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|i| chunk.get(i).copied().unwrap_or(0));
        // Each sextet's bits from 2 bytes add up, as they don't overlap.
        let sextets = [a >> 2, ((a & 0b11) << 4) + (b >> 4), ((b & 0b1111) << 2) + (c >> 6), c & 0b11_1111];
        // A chunk of n bytes fills n + 1 digits, and padding the rest.
        for (i, sextet) in sextets.into_iter().enumerate() {
            out.push(if i <= chunk.len() { digit(sextet) } else { '=' });
        }
    }
    out
}

/// The base64 digit of `sextet`, a number from 0 to 63.
fn digit(sextet: u8) -> char {
    char::from(match sextet {
        0..=25 => b'A' + sextet,
        26..=51 => b'a' + (sextet - 26),
        52..=61 => b'0' + (sextet - 52),
        62 => b'+',
        _ => b'/',
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `text` decoded from base64, padded, or `None` when it isn't base64.
    fn decoded(text: &str) -> Option<Vec<u8>> {
        let value = |c: u8| (0..64).find(|&v| digit(v) == char::from(c));
        let mut out = Vec::new();
        for chunk in text.as_bytes().chunks(4) {
            let padding = chunk.iter().filter(|&&c| c == b'=').count();
            let n = chunk.iter().try_fold(0u32, |n, &c| Some(n << 6 | u32::from(if c == b'=' { 0 } else { value(c)? })))?;
            out.extend(n.to_be_bytes().iter().skip(1).take(3 - padding));
        }
        Some(out)
    }

    #[test]
    fn files_are_written_in_padded_base64() {
        // RFC 4648's test vectors (section 10): the encodings of "foobar"'s first 0 to 6 letters.
        let encodings = ["", "Zg==", "Zm8=", "Zm9v", "Zm9vYg==", "Zm9vYmE=", "Zm9vYmFy"];
        for (letters, text) in encodings.into_iter().enumerate() {
            assert_eq!(base64(&b"foobar"[..letters]), text);
        }
        // Every byte, at each place in a group of 3, and every digit.
        let all: Vec<u8> = (0..=255).chain((0..=255).rev()).chain([0, 255, 128]).collect();
        for skip in 0..3 {
            assert_eq!(decoded(&base64(&all[skip..])).as_deref(), Some(&all[skip..]));
        }
        let digits: String = (0..64).map(digit).collect();
        assert_eq!(digits, "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/");
    }
}
