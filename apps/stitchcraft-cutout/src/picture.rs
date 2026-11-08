//! The picture the cut-out works on, decoded as its file stores the pixels.
//!
//! **Decoded as VectorCraft decodes it.** The `image` crate decodes a PNG or JPEG file by what its bytes
//! say it is, whatever its extension, as VectorCraft's `file.place` does. An orientation its metadata
//! gives is not applied, so the marks a user gives are in the pixels as the file stores them, and the
//! cut-out, placed by VectorCraft, lies as the picture would.
//!
//! **Bounded.** A picture is at most [`MAX_SIDE`] px on a side and [`MAX_PIXELS`] in all, and the decoder may
//! allocate no more than [`MAX_ALLOC`], so a small file that claims a huge picture can't exhaust memory. The
//! models see the picture at 320 × 320 and 1,024 px across, so a larger one would cut out no better. Every
//! loop over the picture charges the [`Work`] the cut-out may spend.

use std::io::Cursor;

use image::{ImageReader, Limits};
use stitchcraft_core::{Code, Diagnostic, Fix};

use crate::work::Work;

/// The longest side of a picture the cut-out takes, in pixels.
pub const MAX_SIDE: u32 = 16_384;
/// The most pixels a picture the cut-out takes may have: 50 megapixels.
pub const MAX_PIXELS: u64 = 50_000_000;
/// The most memory the decoder may allocate, in bytes.
pub const MAX_ALLOC: u64 = 512 * 1024 * 1024;

/// A picture: its size, and its pixels as 8-bit red, green, blue and alpha, row by row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    width: u32,
    height: u32,
    rgba: Vec<u8>,
}

impl Picture {
    /// The picture in the file `bytes`, a PNG or JPEG file by what its bytes say, decoded as it stores the
    /// pixels; `SC-E1203` when it can't be read, or is larger than the cut-out takes.
    pub fn decode(bytes: &[u8], work: &mut Work) -> Result<Picture, Diagnostic> {
        let reader = || ImageReader::new(Cursor::new(bytes)).with_guessed_format().map_err(|e| unreadable(&e.to_string()));
        // Its size, from its header, before any pixel is decoded.
        let (width, height) = reader()?.into_dimensions().map_err(|e| unreadable(&e.to_string()))?;
        let pixels = u64::from(width) * u64::from(height);
        if pixels > MAX_PIXELS {
            return Err(unreadable(&format!("it is {width} × {height} px, and the cut-out takes at most {MAX_PIXELS} pixels")));
        }
        let mut decoding = reader()?;
        let mut limits = Limits::default();
        limits.max_image_width = Some(MAX_SIDE);
        limits.max_image_height = Some(MAX_SIDE);
        limits.max_alloc = Some(MAX_ALLOC);
        decoding.limits(limits);
        let decoded = decoding.decode().map_err(|e| unreadable(&e.to_string()))?;
        work.charge(pixels)?;
        Picture::new(decoded.width(), decoded.height(), decoded.into_rgba8().into_raw())
    }

    /// A picture `width` × `height` px of the pixels `rgba`, 4 bytes each, row by row; `SC-E1203` when
    /// they aren't that many, or it has no pixel.
    pub fn new(width: u32, height: u32, rgba: Vec<u8>) -> Result<Picture, Diagnostic> {
        let expected = u64::from(width) * u64::from(height) * 4;
        if width == 0 || height == 0 || u64::try_from(rgba.len()).ok() != Some(expected) {
            return Err(unreadable(&format!("{} bytes don't make a picture {width} × {height} px of 4 bytes a pixel", rgba.len())));
        }
        Ok(Picture { width, height, rgba })
    }

    /// Its width, in pixels.
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// Its height, in pixels.
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// Its pixels, 4 bytes each, row by row.
    pub fn rgba(&self) -> &[u8] {
        &self.rgba
    }

    /// How many pixels it has.
    pub fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// The picture as `image` keeps it, to resize.
    pub(crate) fn view(&self) -> Option<image::RgbaImage> {
        image::RgbaImage::from_raw(self.width, self.height, self.rgba.clone())
    }
}

/// `SC-E1203`: the picture can't be read for the cut-out, and why.
fn unreadable(why: &str) -> Diagnostic {
    Diagnostic::new(Code::CutoutPictureUnreadable, format!("The picture could not be read for the subject cut-out: {why}."))
        .with_fix(Fix::Hint("Give a PNG or JPEG file of at most 50 megapixels, or plan the picture without --subject.".to_string()))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    /// A PNG file of `width` × `height` px, every pixel `rgba`.
    fn png(width: u32, height: u32, rgba: [u8; 4]) -> Vec<u8> {
        let image = image::RgbaImage::from_pixel(width, height, image::Rgba(rgba));
        let mut bytes = Vec::new();
        image.write_to(&mut Cursor::new(&mut bytes), image::ImageFormat::Png).unwrap();
        bytes
    }

    #[test]
    fn a_picture_is_decoded_as_its_file_stores_the_pixels_whatever_it_is_named() {
        let mut work = Work::new(Budget::DEFAULT);
        let picture = Picture::decode(&png(3, 2, [10, 20, 30, 40]), &mut work).unwrap();
        assert_eq!((picture.width(), picture.height(), picture.pixels()), (3, 2, 6));
        assert_eq!(picture.rgba().chunks(4).collect::<Vec<_>>(), vec![[10, 20, 30, 40]; 6]);
        // The picture's pixels were charged, and a budget they don't fit stops it.
        assert_eq!(work.left(), Budget::DEFAULT.max_work - 6);
        let mut small = Work::new(Budget { max_work: 5, ..Budget::DEFAULT });
        assert_eq!(Picture::decode(&png(3, 2, [0; 4]), &mut small).unwrap_err().code, stitchcraft_core::Code::BudgetExhausted);
        assert_eq!(picture.view().unwrap().dimensions(), (3, 2));
    }

    #[test]
    fn diag_sc_e1203_a_file_that_is_no_picture_is_said_with_why() {
        let said = Picture::decode(b"<svg/>", &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert_eq!(said.code, Code::CutoutPictureUnreadable);
        assert!(said.to_string().starts_with("error SC-E1203: The picture could not be read for the subject cut-out: "), "{said}");
        assert_eq!(
            said.fix.map(|fix| fix.describe()).as_deref(),
            Some("Give a PNG or JPEG file of at most 50 megapixels, or plan the picture without --subject.")
        );
        // Pixels that don't make the picture, or a picture without a pixel, are refused the same way.
        assert!(Picture::new(2, 2, vec![0; 15]).unwrap_err().message.contains("15 bytes don't make a picture 2 × 2 px"));
        assert!(Picture::new(0, 2, Vec::new()).is_err() && Picture::new(2, 0, Vec::new()).is_err());
        assert!(Picture::new(1, 1, vec![0; 4]).is_ok());
    }

    /// The CRC-32 of `bytes`, as a PNG chunk carries it.
    fn crc32(bytes: &[u8]) -> u32 {
        let mut crc = u32::MAX;
        for byte in bytes {
            crc ^= u32::from(*byte);
            for _ in 0..8 {
                crc = if crc & 1 == 1 { (crc >> 1) ^ 0xEDB8_8320 } else { crc >> 1 };
            }
        }
        !crc
    }

    /// A PNG file whose header says it is `width` × `height` px, of 8-bit RGBA pixels, and that holds none:
    /// its data chunk is empty.
    fn header_only(width: u32, height: u32) -> Vec<u8> {
        let mut header = b"IHDR".to_vec();
        header.extend(width.to_be_bytes());
        header.extend(height.to_be_bytes());
        header.extend([8, 6, 0, 0, 0]);
        let mut bytes = b"\x89PNG\r\n\x1a\n".to_vec();
        for chunk in [header, b"IDAT".to_vec()] {
            bytes.extend(u32::try_from(chunk.len() - 4).unwrap().to_be_bytes());
            bytes.extend(&chunk);
            bytes.extend(crc32(&chunk).to_be_bytes());
        }
        bytes
    }

    #[test]
    fn a_picture_larger_than_the_cut_out_takes_is_refused_from_its_header() {
        // More pixels than MAX_PIXELS, 8,000 × 6,251 px, refused before a pixel is decoded.
        let many = Picture::decode(&header_only(8_000, 6_251), &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert_eq!(many.code, Code::CutoutPictureUnreadable, "{many}");
        assert!(many.message.ends_with("it is 8000 × 6251 px, and the cut-out takes at most 50000000 pixels."), "{}", many.message);
        // Exactly MAX_PIXELS passes that check, and fails only for the pixels the file lacks.
        let most = Picture::decode(&header_only(10_000, 5_000), &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert!(!most.message.contains("the cut-out takes at most"), "{}", most.message);
        // Wider than MAX_SIDE: the decoder's limits refuse it.
        let wide = Picture::decode(&png(MAX_SIDE + 1, 1, [0; 4]), &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert_eq!(wide.code, Code::CutoutPictureUnreadable, "{wide}");
        assert!(Picture::decode(&png(MAX_SIDE, 1, [0; 4]), &mut Work::new(Budget::DEFAULT)).is_ok());
    }
}
