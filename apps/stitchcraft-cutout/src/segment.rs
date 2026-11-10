//! MobileSAM's part of the cut-out: the picture as its encoder takes it, the prompt its decoder takes, and
//! the alpha its map gives (ADR-0019, decision 3).
//!
//! **As SAM prepares them.** MobileSAM keeps SAM's prompt encoder and mask decoder, and the cut-out prepares
//! its input and reads its output as SAM's own code does. The encoder takes the picture scaled, its aspect
//! kept, so that its longer side is [`SIDE`] px, its sides rounded to whole pixels ([`Frame`]), normalized
//! with SAM's mean and deviation of each colour, and padded with zeros to the square. The decoder takes the
//! marks in that square's pixels: the points of the subject with the label 1, those of what isn't it with
//! 0, and the box's top left and bottom right corners with 2 and 3 ([`Prompt`]).
//!
//! **The alpha.** The decoder gives a map of [`LOW`] × [`LOW`] values across the square, each a logit: how
//! likely, on the logistic scale, the pixel there is to be the subject. As SAM does, the map is scaled to the
//! square, the picture's part of it kept and scaled to the picture, each time bilinearly between the pixels'
//! middles, as PyTorch's `interpolate` without `align_corners` does. The logistic function of each value,
//! times 255 and rounded, is the pixel's alpha ([`alpha`]): a logit of 0 or more is an alpha of 128 or more,
//! which Image Trace keeps.

use image::imageops::{FilterType, resize};
use stitchcraft_core::{Diagnostic, math};

use crate::marks::{Marks, PixelBox, Point};
use crate::picture::Picture;
use crate::salient::bug;
use crate::work::Work;

/// The side of the encoder's square, in pixels.
pub const SIDE: u32 = 1024;
/// The side of the decoder's map, in values.
pub const LOW: u32 = 256;
/// The shape of the embedding the encoder gives: 256 channels of 64 × 64 values.
pub const EMBEDDING_SHAPE: [usize; 4] = [1, 256, 64, 64];
/// How many values the embedding holds.
pub const EMBEDDING: usize = 256 * 64 * 64;
/// SAM's mean and deviation of each colour, from 0 to 255.
const MEAN: [f32; 3] = [123.675, 116.28, 103.53];
const DEVIATION: [f32; 3] = [58.395, 57.12, 57.375];

/// How a picture lies in the encoder's square: scaled, its aspect kept, so that its longer side is [`SIDE`]
/// px, at the square's top left.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Frame {
    /// The picture's width, in its own pixels.
    pub width: u32,
    /// Its height.
    pub height: u32,
    /// Its width scaled, rounded to a whole pixel as SAM rounds it.
    pub scaled_width: u32,
    /// Its height scaled.
    pub scaled_height: u32,
}

impl Frame {
    /// The frame of a picture `width` × `height` px.
    pub fn of(width: u32, height: u32) -> Frame {
        let scale = f64::from(SIDE) / f64::from(width.max(height).max(1));
        // SAM rounds a scaled side by adding a half and keeping the whole part.
        let scaled = |side: u32| whole((f64::from(side) * scale + 0.5).floor()).clamp(1, SIDE);
        Frame { width, height, scaled_width: scaled(width), scaled_height: scaled(height) }
    }

    /// `point`, of the picture, in the square's pixels: each axis scaled as the picture's side along it is.
    pub fn to_square(&self, point: Point) -> [f32; 2] {
        let across = |value: f64, scaled: u32, side: u32| narrow(value * f64::from(scaled) / f64::from(side.max(1)));
        [across(point.x, self.scaled_width, self.width), across(point.y, self.scaled_height, self.height)]
    }
}

/// `value`, a whole number from 0 to `u32::MAX`, as one.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Clamped to u32's range first.
fn whole(value: f64) -> u32 {
    value.clamp(0.0, f64::from(u32::MAX)) as u32
}

/// `value` as the decoder takes its numbers, to the nearest `f32`.
#[allow(clippy::cast_possible_truncation)] // The nearest f32 is the point: the model computes in f32.
fn narrow(value: f64) -> f32 {
    value as f32
}

/// The encoder's input for `picture` in `frame`: 3 × [`SIDE`] × [`SIDE`] values, red, then green, then blue,
/// each row by row, the picture scaled into the top left and zeros beyond it ([the module](self)).
pub fn encoder_input(picture: &Picture, frame: &Frame, work: &mut Work) -> Result<Vec<f32>, Diagnostic> {
    let view = picture.view().ok_or_else(|| bug("the picture's pixels"))?;
    work.charge(picture.pixels())?;
    // Triangle is the bilinear filter, widened when the picture shrinks, as SAM's resize does.
    let scaled = resize(&view, frame.scaled_width, frame.scaled_height, FilterType::Triangle);
    let side = usize::try_from(SIDE).unwrap_or(0);
    let plane = side * side;
    let mut values = vec![0.0; 3 * plane];
    work.charge_each(values.len())?;
    let width = usize::try_from(frame.scaled_width).unwrap_or(0);
    for (channel, ((mean, deviation), out)) in MEAN.iter().zip(DEVIATION).zip(values.chunks_exact_mut(plane)).enumerate() {
        for (row, pixels) in out.chunks_exact_mut(side).zip(scaled.as_raw().chunks_exact(width * 4)) {
            for (value, pixel) in row.iter_mut().zip(pixels.chunks_exact(4)) {
                *value = (f32::from(pixel.get(channel).copied().unwrap_or(0)) - mean) / deviation;
            }
        }
    }
    Ok(values)
}

/// The encoder's embedding of a picture: [`EMBEDDING`] values.
#[derive(Clone, Debug, PartialEq)]
pub struct Embedding {
    values: Vec<f32>,
}

impl Embedding {
    /// The embedding `values` give; `SC-E0009` when they are not [`EMBEDDING`] of them.
    pub fn new(values: Vec<f32>) -> Result<Embedding, Diagnostic> {
        if values.len() == EMBEDDING { Ok(Embedding { values }) } else { Err(bug(&format!("an embedding of {} values", values.len()))) }
    }

    /// Its values.
    pub fn values(&self) -> &[f32] {
        &self.values
    }
}

/// What the decoder takes: each mark's place in the encoder's square, and its label.
#[derive(Clone, Debug, PartialEq)]
pub struct Prompt {
    /// Each mark's place, across and down, in the square's pixels.
    pub coords: Vec<[f32; 2]>,
    /// Each mark's label: 1 for a point of the subject, 0 for one of what isn't it, 2 and 3 for the box's
    /// top left and bottom right corners.
    pub labels: Vec<f32>,
}

impl Prompt {
    /// The decoder's prompt for the box `subject` and the points of `marks`, in `frame`: the points of the
    /// subject, then those of what isn't it, then the box's corners, as SAM's own example orders them.
    pub fn new(subject: &PixelBox, marks: &Marks, frame: &Frame) -> Prompt {
        let points = marks.subject.iter().map(|point| (point, 1.0)).chain(marks.not_subject.iter().map(|point| (point, 0.0)));
        let corners = [(Point { x: subject.x0, y: subject.y0 }, 2.0), (Point { x: subject.x1, y: subject.y1 }, 3.0)];
        let (coords, labels) =
            points.map(|(point, label)| (*point, label)).chain(corners).map(|(point, label)| (frame.to_square(point), label)).unzip();
        Prompt { coords, labels }
    }
}

/// The decoder's map of a prompt: [`LOW`] × [`LOW`] logits, row by row, and how well it says its mask fits.
#[derive(Clone, Debug, PartialEq)]
pub struct LowRes {
    logits: Vec<f32>,
    predicted_iou: f32,
}

impl LowRes {
    /// The map `logits` give, with `predicted_iou`; `SC-E0009` when they are not [`LOW`] × [`LOW`] of them.
    pub fn new(logits: Vec<f32>, predicted_iou: f32) -> Result<LowRes, Diagnostic> {
        let side = usize::try_from(LOW).unwrap_or(0);
        if logits.len() == side * side { Ok(LowRes { logits, predicted_iou }) } else { Err(bug(&format!("a map of {} values", logits.len()))) }
    }

    /// Its logits, row by row.
    pub fn logits(&self) -> &[f32] {
        &self.logits
    }

    /// How well the decoder says its mask fits the subject, from 0 to 1.
    pub const fn predicted_iou(&self) -> f32 {
        self.predicted_iou
    }
}

/// The alpha of each pixel of the picture in `frame`, row by row, from the decoder's map `low` ([the
/// module](self)).
pub fn alpha(low: &LowRes, frame: &Frame, work: &mut Work) -> Result<Vec<u8>, Diagnostic> {
    let (side, square) = (usize::try_from(LOW).unwrap_or(0), usize::try_from(SIDE).unwrap_or(0));
    let size = |value: u32| usize::try_from(value).unwrap_or(0);
    let (width, height) = (size(frame.width), size(frame.height));
    work.charge_each(square * square)?;
    let up = bilinear(low.logits(), (side, side), (square, square));
    // The picture's part of the square, at its top left.
    let (part_width, part_height) = (size(frame.scaled_width), size(frame.scaled_height));
    let part: Vec<f32> = up.chunks_exact(square).take(part_height).flat_map(|row| row.iter().take(part_width).copied()).collect();
    work.charge_each(width * height)?;
    let logits = bilinear(&part, (part_width, part_height), (width, height));
    Ok(logits.into_iter().map(|logit| byte(255.0 / (1.0 + math::exp(-f64::from(logit))))).collect())
}

/// `value`, from 0 to 255, rounded to a byte.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Clamped to a byte's range first.
fn byte(value: f64) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

/// `values`, a grid `from` across and down, scaled to `to`, bilinearly between the pixels' middles and held
/// at the edges, as PyTorch's `interpolate` does without `align_corners`.
fn bilinear(values: &[f32], from: (usize, usize), to: (usize, usize)) -> Vec<f32> {
    let ((from_width, from_height), (to_width, to_height)) = (from, to);
    let columns = taps(from_width, to_width);
    let mut out = Vec::with_capacity(to_width * to_height);
    for (y0, y1, fy) in taps(from_height, to_height) {
        let row = |y: usize| values.get(y * from_width..(y + 1) * from_width).unwrap_or_default();
        let (top, bottom) = (row(y0), row(y1));
        for &(x0, x1, fx) in &columns {
            let at = |row: &[f32], x: usize| row.get(x).copied().unwrap_or(0.0);
            let upper = at(top, x0) * (1.0 - fx) + at(top, x1) * fx;
            let lower = at(bottom, x0) * (1.0 - fx) + at(bottom, x1) * fx;
            out.push(upper * (1.0 - fy) + lower * fy);
        }
    }
    out
}

/// For each of `to` pixels along an axis of `from`: the 2 pixels of `from` it lies between and how far
/// along from the first, its middle mapped to `from`'s, clamped at 0, the second held at the last pixel.
fn taps(from: usize, to: usize) -> Vec<(usize, usize, f32)> {
    let scale = count(from) / count(to.max(1));
    let last = from.saturating_sub(1);
    (0..to)
        .map(|at| {
            let source = ((count(at) + 0.5) * scale - 0.5).max(0.0);
            let first = index(source.floor()).min(last);
            let second = (first + 1).min(last);
            (first, second, source - source.floor())
        })
        .collect()
}

/// `n` as the `f32` PyTorch's scale is computed in.
#[allow(clippy::cast_precision_loss)] // Sides and indices are below 2^24, exact in f32.
fn count(n: usize) -> f32 {
    n as f32
}

/// `value`, a whole number of 0 or more, as an index.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // Whole, 0 or more and below 2^24.
fn index(value: f32) -> usize {
    value.max(0.0) as usize
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    #[test]
    fn req_cli_014_the_frame_scales_the_longer_side_to_1024_and_rounds_as_sam_does() {
        assert_eq!(Frame::of(1200, 1600), Frame { width: 1200, height: 1600, scaled_width: 768, scaled_height: 1024 });
        // 1,200 × 1,024 / 1,786 = 688.02, rounded to 688.
        assert_eq!(Frame::of(1200, 1786), Frame { width: 1200, height: 1786, scaled_width: 688, scaled_height: 1024 });
        assert_eq!(Frame::of(4000, 10), Frame { width: 4000, height: 10, scaled_width: 1024, scaled_height: 3 });
        // A side scaled below a pixel keeps one.
        assert_eq!(Frame::of(10_000, 1).scaled_height, 1);
        // Each axis scales as its side: SAM's apply_coords.
        let frame = Frame::of(1200, 1786);
        let [x, y] = frame.to_square(Point { x: 600.0, y: 893.0 });
        assert!((x - 344.0).abs() < 1e-4 && (y - 512.0).abs() < 1e-4, "{x} {y}");
    }

    #[test]
    fn req_cli_015_the_prompt_is_the_points_then_the_boxs_corners_with_their_labels() {
        let frame = Frame::of(1200, 1600);
        let marks = Marks {
            subject_box: None,
            subject: vec![Point { x: 550.0, y: 700.0 }],
            not_subject: vec![Point { x: 800.0, y: 170.0 }, Point { x: 10.0, y: 20.0 }],
        };
        let subject = PixelBox { x0: 340.0, y0: 10.0, x1: 890.0, y1: 1205.0 };
        let prompt = Prompt::new(&subject, &marks, &frame);
        // 1,200 × 1,600 px scales by 0.64 on each axis.
        let expected = [[352.0, 448.0], [512.0, 108.8], [6.4, 12.8], [217.6, 6.4], [569.6, 771.2]];
        assert_eq!(prompt.labels, [1.0, 0.0, 0.0, 2.0, 3.0]);
        for (got, want) in prompt.coords.iter().zip(expected) {
            assert!((got[0] - want[0]).abs() < 1e-3 && (got[1] - want[1]).abs() < 1e-3, "{got:?} {want:?}");
        }
        // A box alone is its 2 corners.
        let alone = Prompt::new(&subject, &Marks::default(), &frame);
        assert_eq!((alone.coords.len(), alone.labels), (2, vec![2.0, 3.0]));
    }

    #[test]
    fn bilinear_scaling_holds_the_edges_and_meets_the_middles_as_pytorch_does() {
        // 2 values scaled to 4: the middles of the outer 2 lie outside the first and last values' middles.
        assert_eq!(bilinear(&[0.0, 1.0], (2, 1), (4, 1)), [0.0, 0.25, 0.75, 1.0]);
        // Shrunk to 1, the middle lies between them.
        assert_eq!(bilinear(&[0.0, 1.0], (2, 1), (1, 1)), [0.5]);
        // Scaled down by 2 on both axes, each value lies between 4.
        let grid: Vec<f32> = (0..16).map(|v| v as f32).collect();
        assert_eq!(bilinear(&grid, (4, 4), (2, 2)), [2.5, 4.5, 10.5, 12.5]);
        assert_eq!(bilinear(&grid, (4, 4), (4, 4)), grid);
        assert_eq!(taps(3, 3), [(0, 1, 0.0), (1, 2, 0.0), (2, 2, 0.0)]);
    }

    #[test]
    fn req_cli_014_a_logit_of_0_or_more_is_an_alpha_of_128_or_more() {
        // A map of 0 at the middle column, negative left of it and positive right, across a picture as wide
        // as the square.
        let side = usize::try_from(LOW).unwrap();
        let logits: Vec<f32> = (0..side * side).map(|at| (at % side) as f32 - 128.0).collect();
        let low = LowRes::new(logits, 0.9).unwrap();
        let frame = Frame::of(1024, 1024);
        let alpha = alpha(&low, &frame, &mut Work::new(Budget::DEFAULT)).unwrap();
        assert_eq!(alpha.len(), 1024 * 1024);
        let row = &alpha[..1024];
        let first_kept = row.iter().position(|a| *a >= 128).unwrap();
        // Scaled up 4 times, the logit at x is ((x + 0.5) / 4 − 0.5) − 128, which is 0 at x = 513.5.
        assert_eq!(first_kept, 514);
        assert_eq!((row[0], row[1023]), (0, 255));
        assert!(row[..first_kept].iter().all(|a| *a < 128) && row[first_kept..].iter().all(|a| *a >= 128));
        assert_eq!(byte(127.5), 128, "the logistic function of 0, 255 × 0.5, rounds up");
        assert_eq!((byte(-3.0), byte(300.0)), (0, 255));
    }

    #[test]
    fn req_cli_014_only_the_pictures_part_of_the_square_is_scaled_to_the_picture() {
        // The square's map is positive down to its row 200 of 256, past the picture's part, the square's top 3
        // rows of 4 here, and negative below.
        let side = usize::try_from(LOW).unwrap();
        let logits: Vec<f32> = (0..side * side).map(|at| if at / side < 200 { 10.0 } else { -10.0 }).collect();
        let low = LowRes::new(logits, 0.5).unwrap();
        // A picture 400 × 300 px fills the square's top 768 rows of 1,024.
        let frame = Frame::of(400, 300);
        assert_eq!((frame.scaled_width, frame.scaled_height), (1024, 768));
        let alpha = alpha(&low, &frame, &mut Work::new(Budget::DEFAULT)).unwrap();
        assert_eq!(alpha.len(), 400 * 300);
        assert!(alpha.iter().all(|a| *a == 255), "the negative rows lie outside the picture");
    }

    #[test]
    fn maps_and_embeddings_of_another_size_are_bugs() {
        assert!(LowRes::new(vec![0.0; 10], 0.0).unwrap_err().message.contains("a map of 10 values"));
        assert!(Embedding::new(vec![0.0; 3]).unwrap_err().message.contains("an embedding of 3 values"));
        let embedding = Embedding::new(vec![0.5; EMBEDDING]).unwrap();
        assert_eq!(embedding.values().len(), EMBEDDING);
        let low = LowRes::new(vec![0.0; 256 * 256], 0.75).unwrap();
        assert_eq!((low.logits().len(), low.predicted_iou()), (65_536, 0.75));
    }

    #[test]
    fn req_cli_014_the_encoder_input_is_the_picture_scaled_normalized_and_padded() {
        // A picture 200 × 100 px of one colour scales to 1,024 × 512 px at the square's top.
        let picture = Picture::new(200, 100, [90, 120, 150, 255].repeat(200 * 100)).unwrap();
        let frame = Frame::of(200, 100);
        let values = encoder_input(&picture, &frame, &mut Work::new(Budget::DEFAULT)).unwrap();
        assert_eq!(values.len(), 3 * 1024 * 1024);
        let at = |channel: usize, x: usize, y: usize| values[channel * 1024 * 1024 + y * 1024 + x];
        for (channel, colour) in [90.0, 120.0, 150.0].into_iter().enumerate() {
            let expected = (colour - MEAN[channel]) / DEVIATION[channel];
            assert!((at(channel, 1023, 511) - expected).abs() < 1e-5, "inside, channel {channel}");
            assert_eq!(at(channel, 0, 512), 0.0, "padding, channel {channel}");
        }
    }

    #[test]
    fn the_alpha_charges_a_unit_for_each_value_of_the_square_and_each_pixel_of_the_picture() {
        let side = usize::try_from(LOW).unwrap();
        let low = LowRes::new(vec![0.0; side * side], 0.5).unwrap();
        let mut work = Work::new(Budget::DEFAULT);
        let before = work.left();
        alpha(&low, &Frame::of(400, 300), &mut work).unwrap();
        assert_eq!(before - work.left(), 1024 * 1024 + 400 * 300);
    }
}
