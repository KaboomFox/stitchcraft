//! The cut-out itself: the picture with MobileSAM's alpha, cropped to the box round what it keeps, as a PNG
//! file (ADR-0019, decision 1).
//!
//! **What it keeps.** A pixel is kept when its alpha is 128 or more, which is what Image Trace keeps, so the
//! crop holds the whole trace and nothing more. A picture that is transparent already keeps its own alpha
//! where it is lower. Inside the crop every pixel keeps its colour, with its alpha from 0 to 255, so the
//! cut-out shows its soft edges in any viewer.
//!
//! **Cropped.** `file.place` fits the PNG file into the hoop or the size asked for, so cropped to the
//! subject, the subject is what fits. A cut-out that keeps no pixel is no subject found (`SC-E1202`), and
//! the user can mark the subject with a box or points.

use std::fmt::Write as _;

use image::ImageEncoder;
use image::codecs::png::PngEncoder;
use stitchcraft_core::{Code, Diagnostic, Fix};

use crate::BoxFrom;
use crate::marks::PixelBox;
use crate::picture::Picture;
use crate::salient::bug;
use crate::work::Work;

/// The alpha from which Image Trace keeps a pixel.
pub const KEPT: u8 = 128;

/// A picture's subject, cut out.
#[derive(Clone, Debug, PartialEq)]
pub struct CutOut {
    /// The cut-out, a PNG file of 8-bit red, green, blue and alpha.
    pub png: Vec<u8>,
    /// The box round the pixels kept, in the picture's whole pixels: left, top, right and bottom edges.
    pub crop: [u32; 4],
    /// The box MobileSAM cut the subject out of.
    pub subject: PixelBox,
    /// Where that box came from.
    pub from: BoxFrom,
    /// Each pixel's alpha, across the whole picture, row by row.
    pub alpha: Vec<u8>,
    /// The picture's width and height, in pixels.
    pub size: [u32; 2],
}

impl CutOut {
    /// `picture` with `alpha`, each pixel's, cropped to the box round the pixels it keeps; MobileSAM cut the
    /// subject out of the box `subject`, which came `from` the user or U²-Net-p. `SC-E1202` when no pixel is
    /// kept.
    pub fn new(picture: &Picture, alpha: &[u8], subject: PixelBox, from: BoxFrom, work: &mut Work) -> Result<CutOut, Diagnostic> {
        let (width, height) = (picture.width(), picture.height());
        let columns = usize::try_from(width).unwrap_or(0);
        if u64::try_from(alpha.len()).ok() != Some(picture.pixels()) {
            return Err(bug(&format!("an alpha of {} pixels for a picture of {}", alpha.len(), picture.pixels())));
        }
        work.charge(picture.pixels())?;
        // The picture's own alpha, where it is lower.
        let alpha: Vec<u8> =
            alpha.iter().zip(picture.rgba().chunks_exact(4)).map(|(a, pixel)| (*a).min(pixel.get(3).copied().unwrap_or(0))).collect();
        let mut bounds: Option<[usize; 4]> = None;
        for (at, _) in alpha.iter().enumerate().filter(|(_, a)| **a >= KEPT) {
            let (x, y) = (at % columns, at / columns);
            bounds = Some(match bounds {
                None => [x, y, x + 1, y + 1],
                Some([x0, y0, x1, y1]) => [x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)],
            });
        }
        let Some([x0, y0, x1, y1]) = bounds else { return Err(nothing_kept(&subject, from)) };
        let mut rgba = Vec::with_capacity((x1 - x0) * (y1 - y0) * 4);
        for (row, alphas) in picture.rgba().chunks_exact(columns * 4).zip(alpha.chunks_exact(columns)).skip(y0).take(y1 - y0) {
            let pixels = row.chunks_exact(4).zip(alphas).skip(x0).take(x1 - x0);
            rgba.extend(pixels.flat_map(|(pixel, a)| pixel.iter().take(3).copied().chain([*a])));
        }
        let crop = [x0, y0, x1, y1].map(|edge| u32::try_from(edge).unwrap_or(u32::MAX));
        let [left, top, right, bottom] = crop;
        let mut png = Vec::new();
        PngEncoder::new(&mut png)
            .write_image(&rgba, right - left, bottom - top, image::ExtendedColorType::Rgba8)
            .map_err(|e| bug(&format!("a cut-out PNG encoder refused ({e})")))?;
        Ok(CutOut { png, crop, subject, from, alpha, size: [width, height] })
    }

    /// How many pixels it keeps: those of an alpha of [`KEPT`] or more.
    pub fn kept(&self) -> usize {
        self.alpha.iter().filter(|a| **a >= KEPT).count()
    }

    /// Its width and height, cropped, in pixels.
    pub fn cropped(&self) -> [u32; 2] {
        let [left, top, right, bottom] = self.crop;
        [right - left, bottom - top]
    }

    /// `SC-I1206`: what the cut-out kept, and the box it came from, to correct it by.
    pub fn said(&self) -> Diagnostic {
        let [width, height] = self.size;
        let share = 100.0 * count(self.kept()) / (f64::from(width) * f64::from(height)).max(1.0);
        let [cropped_width, cropped_height] = self.cropped();
        let mut message = format!("The subject was cut out of the box {}, which {}: ", self.subject, from(self.from));
        let _ = write!(message, "{share:.1} % of the picture is kept, cropped to {cropped_width} × {cropped_height} px.");
        Diagnostic::new(Code::CutoutMade, message).with_fix(Fix::Hint(
            "To correct it, give another box with --subject-box, or mark points with --subject-point and --not-subject.".to_string(),
        ))
    }
}

/// Who gave the box, completing "which …".
const fn from(from: BoxFrom) -> &'static str {
    match from {
        BoxFrom::Given => "--subject-box gave",
        BoxFrom::Found => "U²-Net-p found",
    }
}

/// `n`, a count of pixels, as a number to divide.
#[allow(clippy::cast_precision_loss)] // A picture holds at most 50,000,000 pixels, exact in f64.
fn count(n: usize) -> f64 {
    n as f64
}

/// `SC-E1202`: MobileSAM kept no pixel of the box `subject`, which came `from` the user or U²-Net-p.
fn nothing_kept(subject: &PixelBox, from_whom: BoxFrom) -> Diagnostic {
    Diagnostic::new(Code::CutoutNoSubject, format!("MobileSAM kept no pixel of the picture in the box {subject}, which {}.", from(from_whom)))
        .with_fix(Fix::Hint(
            "Give the subject's box with --subject-box X0,Y0,X1,Y1, or mark a point of it with --subject-point X,Y, in the picture's pixels."
                .to_string(),
        ))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    /// A picture 6 × 4 px, each pixel's red its x and green its y, opaque.
    fn picture() -> Picture {
        let rgba = (0..4).flat_map(|y: u8| (0..6).flat_map(move |x: u8| [x, y, 9, 255])).collect();
        Picture::new(6, 4, rgba).unwrap()
    }

    const BOX: PixelBox = PixelBox { x0: 1.0, y0: 1.0, x1: 5.0, y1: 4.0 };

    #[test]
    fn req_cli_014_the_cut_out_is_cropped_to_the_box_round_the_pixels_of_an_alpha_of_128_or_more() {
        // Kept: (2, 1) at 128 and (4, 2) at 255; (1, 1) at 127 is not.
        let mut alpha = vec![0; 24];
        alpha[6 + 2] = 128;
        alpha[12 + 4] = 255;
        alpha[6 + 1] = 127;
        let cut = CutOut::new(&picture(), &alpha, BOX, BoxFrom::Given, &mut Work::new(Budget::DEFAULT)).unwrap();
        assert_eq!((cut.crop, cut.cropped(), cut.kept()), ([2, 1, 5, 3], [3, 2], 2));
        let decoded = image::load_from_memory(&cut.png).unwrap().into_rgba8();
        assert_eq!(decoded.dimensions(), (3, 2));
        // Each pixel keeps its colour, with its alpha.
        assert_eq!(decoded.get_pixel(0, 0).0, [2, 1, 9, 128]);
        assert_eq!(decoded.get_pixel(2, 1).0, [4, 2, 9, 255]);
        assert_eq!(decoded.get_pixel(1, 0).0, [3, 1, 9, 0]);
        assert_eq!((cut.size, cut.subject, cut.from), ([6, 4], BOX, BoxFrom::Given));
    }

    #[test]
    fn req_cli_014_a_picture_transparent_already_keeps_its_own_alpha_where_it_is_lower() {
        let mut rgba = picture().rgba().to_vec();
        // (3, 2) is transparent in the picture itself.
        rgba[(12 + 3) * 4 + 3] = 0;
        let picture = Picture::new(6, 4, rgba).unwrap();
        let mut alpha = vec![0; 24];
        alpha[12 + 3] = 255;
        alpha[12 + 4] = 200;
        let cut = CutOut::new(&picture, &alpha, BOX, BoxFrom::Found, &mut Work::new(Budget::DEFAULT)).unwrap();
        assert_eq!((cut.crop, cut.kept()), ([4, 2, 5, 3], 1));
        assert_eq!((cut.alpha[12 + 3], cut.alpha[12 + 4]), (0, 200));
    }

    #[test]
    fn diag_sc_i1206_a_cut_out_says_its_box_and_what_it_kept() {
        let mut alpha = vec![0; 24];
        alpha[..12].fill(255);
        let cut = CutOut::new(&picture(), &alpha, BOX, BoxFrom::Found, &mut Work::new(Budget::DEFAULT)).unwrap();
        let said = cut.said();
        assert_eq!(
            said.to_string(),
            "info SC-I1206: The subject was cut out of the box 1,1,5,4, which U²-Net-p found: 50.0 % of the picture is kept, cropped to 6 × 2 px."
        );
        assert_eq!(
            said.fix.map(|fix| fix.describe()).as_deref(),
            Some("To correct it, give another box with --subject-box, or mark points with --subject-point and --not-subject.")
        );
        let given = CutOut { from: BoxFrom::Given, ..cut };
        assert!(given.said().message.starts_with("The subject was cut out of the box 1,1,5,4, which --subject-box gave: "));
    }

    #[test]
    fn diag_sc_e1202_a_cut_out_that_keeps_no_pixel_is_no_subject_found() {
        let said = CutOut::new(&picture(), &[127; 24], BOX, BoxFrom::Found, &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert_eq!(said.to_string(), "error SC-E1202: MobileSAM kept no pixel of the picture in the box 1,1,5,4, which U²-Net-p found.");
        assert_eq!(
            said.fix.map(|fix| fix.describe()).as_deref(),
            Some("Give the subject's box with --subject-box X0,Y0,X1,Y1, or mark a point of it with --subject-point X,Y, in the picture's pixels.")
        );
        // An alpha of another size is a bug of the cut-out's own.
        let bug = CutOut::new(&picture(), &[255; 23], BOX, BoxFrom::Found, &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert_eq!(bug.code, Code::InternalCheckFailed);
    }
}
