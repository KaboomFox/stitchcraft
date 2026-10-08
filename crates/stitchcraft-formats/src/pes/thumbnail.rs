//! The PEC thumbnails: 48 × 38 one-bit pictures that machines show when you choose a design.
//!
//! A PEC block ends with one picture of the whole design followed by one per colour entry (stops
//! included). Each picture is 38 rows of 6 bytes; within a byte the leftmost pixel is the least
//! significant bit. Public descriptions leave the bit order open; it matches the output of pystitch
//! (MIT), observed as a black box (`NOTICE`). Each picture has a rounded frame, as machine-made files
//! do, and the stitches are drawn inside it at one scale for all pictures, so the per-colour pictures line
//! up with the whole.

use crate::lower::{Lowered, Op};
use crate::quantize::Units;

/// Pixels per row.
pub(crate) const WIDTH: i64 = 48;
/// Rows.
pub(crate) const HEIGHT: i64 = 38;
/// Bytes per row.
const STRIDE: usize = 6;
/// Bytes per picture.
pub(crate) const BYTES: usize = STRIDE * 38;

/// The drawing area inside the frame, inclusive: x 4–43, y 4–33.
const AREA: (i64, i64, i64, i64) = (4, 4, 43, 33);

/// One 48 × 38 one-bit picture.
#[derive(Clone)]
struct Picture([u8; BYTES]);

impl Picture {
    /// An empty picture with the rounded frame.
    fn framed() -> Picture {
        let mut p = Picture([0; BYTES]);
        for x in 4..=43 {
            p.set(x, 1);
            p.set(x, 36);
        }
        for y in 4..=33 {
            p.set(1, y);
            p.set(46, y);
        }
        for (x, y) in [(3, 2), (2, 3), (44, 2), (45, 3), (2, 34), (3, 35), (45, 34), (44, 35)] {
            p.set(x, y);
        }
        p
    }

    fn set(&mut self, x: i64, y: i64) {
        if !(0..WIDTH).contains(&x) || !(0..HEIGHT).contains(&y) {
            return;
        }
        let (Ok(column), Ok(row)) = (usize::try_from(x), usize::try_from(y)) else { return };
        if let Some(byte) = self.0.get_mut(row * STRIDE + column / 8) {
            *byte |= 1 << (column % 8);
        }
    }

    /// A line from `a` to `b` (Bresenham; integer-only, so identical everywhere).
    fn line(&mut self, a: (i64, i64), b: (i64, i64)) {
        let (mut x, mut y) = a;
        let (dx, dy) = ((b.0 - x).abs(), -(b.1 - y).abs());
        let (sx, sy) = (if x < b.0 { 1 } else { -1 }, if y < b.1 { 1 } else { -1 });
        let mut error = dx + dy;
        loop {
            self.set(x, y);
            if (x, y) == b {
                break;
            }
            let doubled = 2 * error;
            if doubled >= dy {
                error += dy;
                x += sx;
            }
            if doubled <= dx {
                error += dx;
                y += sy;
            }
        }
    }
}

/// Maps machine units into the drawing area, keeping the aspect ratio and centring the design.
struct Scale {
    min: Units,
    num: i64,
    den: i64,
    offset: (i64, i64),
}

impl Scale {
    fn new((min, max): (Units, Units)) -> Scale {
        let (w, h) = (i64::from(max.x) - i64::from(min.x), i64::from(max.y) - i64::from(min.y));
        let (area_w, area_h) = (AREA.2 - AREA.0, AREA.3 - AREA.1);
        // The scale is area_w / w or area_h / h, whichever is smaller (compared without dividing).
        let (num, den) = if w == 0 && h == 0 {
            (0, 1)
        } else if area_w * h <= area_h * w {
            (area_w, w)
        } else {
            (area_h, h)
        };
        let offset = ((area_w - w * num / den) / 2, (area_h - h * num / den) / 2);
        Scale { min, num, den, offset }
    }

    fn pixel(&self, at: Units) -> (i64, i64) {
        let x = (i64::from(at.x) - i64::from(self.min.x)) * self.num / self.den;
        let y = (i64::from(at.y) - i64::from(self.min.y)) * self.num / self.den;
        (AREA.0 + self.offset.0 + x, AREA.1 + self.offset.1 + y)
    }
}

/// The thumbnail bytes for a lowered plan: the whole design, then one picture per colour entry.
pub(crate) fn thumbnails(lowered: &Lowered) -> Vec<u8> {
    let scale = Scale::new(lowered.bounds);
    let mut pictures = vec![Picture::framed(); 1 + lowered.entries.len()];
    let mut at = Units::default();
    let mut entry = 1;
    for op in &lowered.ops {
        match *op {
            Op::Stitch(d) => {
                let next = Units { x: at.x + d.dx, y: at.y + d.dy };
                let (a, b) = (scale.pixel(at), scale.pixel(next));
                for index in [0, entry] {
                    if let Some(picture) = pictures.get_mut(index) {
                        picture.line(a, b);
                    }
                }
                at = next;
            }
            Op::Jump(d) => at = Units { x: at.x + d.dx, y: at.y + d.dy },
            Op::Stop | Op::ColorChange => entry += 1,
            Op::Trim | Op::End => {}
        }
    }
    pictures.iter().flat_map(|p| p.0).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::quantize::Delta;

    fn row(bytes: &[u8], picture: usize, y: usize) -> String {
        let start = picture * BYTES + y * STRIDE;
        (0..48).map(|x| if bytes[start + x / 8] >> (x % 8) & 1 == 1 { '#' } else { '.' }).collect()
    }

    fn lowered(ops: Vec<Op>, entries: usize, bounds: (Units, Units)) -> Lowered {
        Lowered { ops, entries: vec![stitchcraft_plan::Rgb::new(0, 0, 0); entries], bounds }
    }

    #[test]
    fn frames_are_rounded_and_bits_run_least_significant_first() {
        let l = lowered(vec![Op::Stitch(Delta::ZERO), Op::End], 1, (Units::default(), Units::default()));
        let bytes = thumbnails(&l);
        assert_eq!(bytes.len(), 2 * BYTES);
        assert_eq!(row(&bytes, 0, 0), "................................................");
        assert_eq!(row(&bytes, 0, 1), "....########################################....");
        assert_eq!(row(&bytes, 0, 2), "...#........................................#...");
        assert_eq!(row(&bytes, 0, 3), "..#..........................................#..");
        assert_eq!(row(&bytes, 0, 4).get(..2), Some(".#"));
        assert_eq!(&bytes[STRIDE..2 * STRIDE], &[0xF0, 0xFF, 0xFF, 0xFF, 0xFF, 0x0F]);
    }

    #[test]
    fn stitches_are_drawn_to_scale_per_colour() {
        // A horizontal line, a colour change, a vertical line: 100 × 100 units.
        let ops = vec![
            Op::Jump(Delta { dx: -50, dy: -50 }),
            Op::Stitch(Delta { dx: 100, dy: 0 }),
            Op::ColorChange,
            Op::Stitch(Delta { dx: 0, dy: 100 }),
            Op::End,
        ];
        let l = lowered(ops, 2, (Units { x: -50, y: -50 }, Units { x: 50, y: 50 }));
        let bytes = thumbnails(&l);
        assert_eq!(bytes.len(), 3 * BYTES);
        // The design is square, so it is 29 px wide, centred: x 9–38, y 4–33.
        assert_eq!(row(&bytes, 0, 4), ".#.......##############################.......#.");
        assert_eq!(row(&bytes, 1, 4), ".#.......##############################.......#.");
        assert_eq!(row(&bytes, 2, 4), ".#....................................#.......#.");
        assert_eq!(row(&bytes, 2, 20), ".#....................................#.......#.");
    }
}
