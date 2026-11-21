//! The figures of a thickness picture's key, drawn as straight strokes (`REQ-RND-006`).
//!
//! A key's numbers are drawn, not set in a font: the repository keeps no font, and a font's outlines would
//! bring curves, whose drawing isn't the same on every platform (see [`crate::raster`]'s determinism). So
//! each figure is up to 7 straight bars, as a pocket calculator shows it, which tiny-skia draws with
//! arithmetic alone. Only what a number of fills needs is here: the 10 digits and a decimal point.

/// A figure of the key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Glyph {
    /// A digit, 0 to 9.
    Digit(u8),
    /// The decimal point.
    Point,
}

/// The width of a digit, as a share of its height.
const WIDE: f32 = 0.55;

/// The 7 bars a digit is made of, from its top left corner, in shares of its height: top, top right, bottom
/// right, bottom, bottom left, top left and middle.
const BARS: [((f32, f32), (f32, f32)); 7] = [
    ((0.0, 0.0), (WIDE, 0.0)),
    ((WIDE, 0.0), (WIDE, 0.5)),
    ((WIDE, 0.5), (WIDE, 1.0)),
    ((0.0, 1.0), (WIDE, 1.0)),
    ((0.0, 0.5), (0.0, 1.0)),
    ((0.0, 0.0), (0.0, 0.5)),
    ((0.0, 0.5), (WIDE, 0.5)),
];

/// Which of [`BARS`] each digit lights, one bit per bar, the top's lowest.
const LIT: [u8; 10] = [0x3F, 0x06, 0x5B, 0x4F, 0x66, 0x6D, 0x7D, 0x07, 0x7F, 0x6F];

impl Glyph {
    /// The strokes the figure is drawn with, from its top left corner, in shares of its height.
    pub(crate) fn strokes(self) -> Vec<((f32, f32), (f32, f32))> {
        match self {
            Glyph::Digit(digit) => {
                let lit = LIT.get(usize::from(digit)).copied().unwrap_or(0);
                BARS.iter().enumerate().filter(|(bar, _)| lit >> bar & 1 == 1).map(|(_, stroke)| *stroke).collect()
            }
            Glyph::Point => vec![((0.0, 1.0), (0.05, 1.0))],
        }
    }
}

/// The figures of `text`, or `None` when it has a character the key has no figure for.
pub(crate) fn glyphs(text: &str) -> Option<Vec<Glyph>> {
    text.chars()
        .map(|c| match c {
            '.' => Some(Glyph::Point),
            // A decimal digit is below 10, so it fits a byte.
            _ => c.to_digit(10).and_then(|d| u8::try_from(d).ok()).map(Glyph::Digit),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn figures_light_the_bars_a_calculator_does() {
        let bars = |digit: u8| Glyph::Digit(digit).strokes().len();
        assert_eq!((0..10).map(bars).collect::<Vec<_>>(), [6, 2, 5, 5, 4, 5, 6, 3, 7, 6]);
        // A 1 is the 2 bars on the right; a 7 adds the top.
        assert_eq!(Glyph::Digit(1).strokes(), [BARS[1], BARS[2]]);
        assert_eq!(Glyph::Digit(7).strokes(), [BARS[0], BARS[1], BARS[2]]);
        assert_eq!(Glyph::Digit(4).strokes(), [BARS[1], BARS[2], BARS[5], BARS[6]]);
        assert!(Glyph::Digit(10).strokes().is_empty());
        assert_eq!(Glyph::Point.strokes().len(), 1);
    }

    #[test]
    fn a_number_is_read_into_figures() {
        assert_eq!(glyphs("2.5"), Some(vec![Glyph::Digit(2), Glyph::Point, Glyph::Digit(5)]));
        assert_eq!(glyphs("09"), Some(vec![Glyph::Digit(0), Glyph::Digit(9)]));
        for bad in ["-1", "1e9", "inf", "NaN", "½"] {
            assert_eq!(glyphs(bad), None, "{bad}");
        }
    }
}
