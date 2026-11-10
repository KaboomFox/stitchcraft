//! The subject's box when the user gives none, from U²-Net-p's map (ADR-0019, decision 3).
//!
//! **The input.** U²-Net-p takes the picture squeezed to [`SIZE`] × [`SIZE`] px, by Lanczos's filter, its
//! values divided by the largest of them and normalized with ImageNet's mean and deviation, as rembg, whose
//! conversion of the author's model the cut-out runs, prepares it. Its map says how likely each of those
//! pixels is to belong to the main subject.
//!
//! **The box.** The map is stretched to run from 0 to 1, and its pixels of a half or more fall into pieces,
//! each the pixels that touch one another, corners included. The piece with the most of the map in it,
//! its values added up, is the subject: its box, scaled back to the picture, is the box MobileSAM cuts the
//! subject out of. A smaller piece, such as a person or a shed far from the dog, is left to the box's
//! inside or outside as it falls. A map with no piece is no subject found (`SC-E1202`), and the user can
//! give the box with `--subject-box`.

use image::imageops::{FilterType, resize};
use stitchcraft_core::{Code, Diagnostic, Fix};

use crate::marks::PixelBox;
use crate::picture::Picture;
use crate::work::Work;

/// The side of U²-Net-p's input and of its map, in pixels.
pub const SIZE: u32 = 320;
/// ImageNet's mean and deviation of each colour, as fractions of the largest value.
const MEAN: [f32; 3] = [0.485, 0.456, 0.406];
const DEVIATION: [f32; 3] = [0.229, 0.224, 0.225];
/// The share of the map at which a pixel belongs to a piece.
pub const THRESHOLD: f32 = 0.5;

/// U²-Net-p's input for `picture`: 3 × [`SIZE`] × [`SIZE`] values, red, then green, then blue, each row by row.
pub fn input(picture: &Picture, work: &mut Work) -> Result<Vec<f32>, Diagnostic> {
    let view = picture.view().ok_or_else(|| bug("the picture's pixels"))?;
    work.charge(picture.pixels())?;
    let small = resize(&view, SIZE, SIZE, FilterType::Lanczos3);
    let pixels = small.as_raw();
    work.charge_each(pixels.len())?;
    let largest = pixels.chunks_exact(4).flat_map(|pixel| pixel.iter().take(3)).copied().max().unwrap_or(0).max(1);
    let largest = f32::from(largest);
    let plane = pixels.len() / 4;
    let mut values = vec![0.0; 3 * plane];
    for (channel, ((mean, deviation), out)) in MEAN.iter().zip(DEVIATION).zip(values.chunks_exact_mut(plane)).enumerate() {
        for (value, pixel) in out.iter_mut().zip(pixels.chunks_exact(4)) {
            let colour = pixel.get(channel).copied().unwrap_or(0);
            *value = (f32::from(colour) / largest - mean) / deviation;
        }
    }
    Ok(values)
}

/// The box round the piece of `map`, U²-Net-p's [`SIZE`] × [`SIZE`] values, stretched to run from 0 to 1,
/// with the most of the map in it, scaled to a picture `width` × `height` px; `None` when no value reaches
/// [`THRESHOLD`] ([the module](self)).
pub fn subject_box(map: &[f32], width: u32, height: u32, work: &mut Work) -> Result<Option<PixelBox>, Diagnostic> {
    let side = usize::try_from(SIZE).unwrap_or(0);
    if map.len() != side * side {
        return Err(bug(&format!("U²-Net-p's map of {} values", map.len())));
    }
    work.charge_each(map.len())?;
    let (low, high) = map.iter().fold((f32::INFINITY, f32::NEG_INFINITY), |(low, high), &value| (low.min(value), high.max(value)));
    let range = (high - low).max(1e-6);
    let stretched: Vec<f32> = map.iter().map(|value| (value - low) / range).collect();
    let mut seen = vec![false; stretched.len()];
    let mut best: Option<(f32, [usize; 4])> = None;
    let mut stack = Vec::new();
    for start in 0..stretched.len() {
        if seen.get(start).copied().unwrap_or(true) || stretched.get(start).is_none_or(|value| *value < THRESHOLD) {
            continue;
        }
        let (mut mass, mut bounds) = (0.0, [side, side, 0, 0]);
        if let Some(mark) = seen.get_mut(start) {
            *mark = true;
        }
        stack.push(start);
        while let Some(at) = stack.pop() {
            work.charge(1)?;
            let (x, y) = (at % side, at / side);
            mass += stretched.get(at).copied().unwrap_or(0.0);
            let [x0, y0, x1, y1] = bounds;
            bounds = [x0.min(x), y0.min(y), x1.max(x + 1), y1.max(y + 1)];
            for (nx, ny) in neighbours(x, y, side) {
                let next = ny * side + nx;
                if !seen.get(next).copied().unwrap_or(true) && stretched.get(next).is_some_and(|value| *value >= THRESHOLD) {
                    if let Some(mark) = seen.get_mut(next) {
                        *mark = true;
                    }
                    stack.push(next);
                }
            }
        }
        if best.is_none_or(|(most, _)| mass > most) {
            best = Some((mass, bounds));
        }
    }
    Ok(best.map(|(_, [x0, y0, x1, y1])| {
        let across = |value: usize, length: u32| f64::from(u32::try_from(value).unwrap_or(SIZE)) * f64::from(length) / f64::from(SIZE);
        PixelBox { x0: across(x0, width), y0: across(y0, height), x1: across(x1, width), y1: across(y1, height) }
    }))
}

/// The pixels next to `(x, y)` in a grid `side` across, corners included.
fn neighbours(x: usize, y: usize, side: usize) -> impl Iterator<Item = (usize, usize)> {
    let range = move |at: usize| at.saturating_sub(1)..=(at + 1).min(side.saturating_sub(1));
    range(y).flat_map(move |ny| range(x).map(move |nx| (nx, ny))).filter(move |&(nx, ny)| (nx, ny) != (x, y))
}

/// `SC-E1202`: U²-Net-p found no subject in `picture`.
pub fn not_found(picture: &Picture) -> Diagnostic {
    Diagnostic::new(
        Code::CutoutNoSubject,
        format!("U²-Net-p found no subject in the picture, {} × {} px: no part of its map reaches a half.", picture.width(), picture.height()),
    )
    .with_fix(Fix::Hint("Give the subject's box with --subject-box X0,Y0,X1,Y1, in the picture's pixels.".to_string()))
}

/// A failed check of what this crate makes itself: a bug, `SC-E0009`.
pub(crate) fn bug(what: &str) -> Diagnostic {
    Diagnostic::new(Code::InternalCheckFailed, format!("The subject cut-out made {what}, which it can't use: a bug in StitchCraft."))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    /// A map of [`SIZE`] × [`SIZE`] values, `value(x, y)` each.
    fn map(value: impl Fn(usize, usize) -> f32) -> Vec<f32> {
        let side = usize::try_from(SIZE).unwrap();
        (0..side * side).map(|at| value(at % side, at / side)).collect()
    }

    #[test]
    fn req_cli_014_the_box_is_the_one_round_the_piece_with_the_most_of_the_map() {
        let mut work = Work::new(Budget::DEFAULT);
        // A large faint piece and a small bright one: the most of the map is the large one's.
        let faint_and_bright = map(|x, y| {
            if (10..110).contains(&x) && (20..220).contains(&y) {
                0.6
            } else if (200..230).contains(&x) && (40..70).contains(&y) {
                1.0
            } else {
                0.0
            }
        });
        let found = subject_box(&faint_and_bright, 1200, 1600, &mut work).unwrap().unwrap();
        // The box round x 10 to 109 and y 20 to 219, scaled to 1,200 × 1,600 px: 3.75 px and 5 px a value.
        assert_eq!(found, PixelBox { x0: 37.5, y0: 100.0, x1: 412.5, y1: 1100.0 });
        // The same pieces, the bright one larger: it holds more of the map.
        let larger_bright = map(|x, y| {
            if (10..110).contains(&x) && (20..220).contains(&y) {
                0.6
            } else if (200..300).contains(&x) && (40..240).contains(&y) {
                1.0
            } else {
                0.0
            }
        });
        let found = subject_box(&larger_bright, 320, 320, &mut work).unwrap().unwrap();
        assert_eq!(found, PixelBox { x0: 200.0, y0: 40.0, x1: 300.0, y1: 240.0 });
    }

    #[test]
    fn req_cli_014_pixels_that_touch_at_a_corner_are_one_piece_and_a_half_belongs() {
        let mut work = Work::new(Budget::DEFAULT);
        // 2 squares meeting at a corner, and a third apart: the first 2 are one piece.
        let corners = map(|x, y| {
            let square = |x0: usize, y0: usize| (x0..x0 + 10).contains(&x) && (y0..y0 + 10).contains(&y);
            if square(0, 0) || square(10, 10) || square(100, 100) { 1.0 } else { 0.0 }
        });
        assert_eq!(subject_box(&corners, 320, 320, &mut work).unwrap(), Some(PixelBox { x0: 0.0, y0: 0.0, x1: 20.0, y1: 20.0 }));
        // Stretched from 0 to 1, a value half way belongs, and one just below doesn't.
        let half = map(|x, y| match (x, y) {
            (0, 0) => 0.0,
            (5, 5) => 1.0,
            (6, 6) => 0.5,
            (7, 7) => 0.499,
            _ => 0.0,
        });
        assert_eq!(subject_box(&half, 320, 320, &mut work).unwrap(), Some(PixelBox { x0: 5.0, y0: 5.0, x1: 7.0, y1: 7.0 }));
        // A map that is the same everywhere stretches to nothing: no subject.
        assert_eq!(subject_box(&map(|_, _| 0.7), 320, 320, &mut work).unwrap(), None);
        // Stretched, a map between 0.2 and 0.4 holds a piece all the same.
        let dim = map(|x, _| if x < 100 { 0.4 } else { 0.2 });
        assert_eq!(subject_box(&dim, 320, 320, &mut work).unwrap(), Some(PixelBox { x0: 0.0, y0: 0.0, x1: 100.0, y1: 320.0 }));
    }

    #[test]
    fn req_cli_014_a_map_reaching_the_grids_edges_gives_a_box_reaching_the_pictures() {
        let mut work = Work::new(Budget::DEFAULT);
        let whole = map(|x, y| if (x, y) == (0, 0) { 0.0 } else { 1.0 });
        assert_eq!(subject_box(&whole, 1200, 1786, &mut work).unwrap(), Some(PixelBox { x0: 0.0, y0: 0.0, x1: 1200.0, y1: 1786.0 }));
        let mut neighbours: Vec<(usize, usize)> = super::neighbours(319, 0, 320).collect();
        neighbours.sort_unstable();
        assert_eq!(neighbours, [(318, 0), (318, 1), (319, 1)]);
        assert_eq!(super::neighbours(5, 5, 320).count(), 8);
    }

    #[test]
    fn a_map_of_another_size_or_a_budget_that_runs_out_is_said() {
        let said = subject_box(&[0.0; 10], 320, 320, &mut Work::new(Budget::DEFAULT)).unwrap_err();
        assert_eq!(said.code, Code::InternalCheckFailed);
        assert_eq!(said.message, "The subject cut-out made U²-Net-p's map of 10 values, which it can't use: a bug in StitchCraft.");
        let mut few = Work::new(Budget { max_work: 102_400 + 10, ..Budget::DEFAULT });
        let said = subject_box(&map(|x, _| if x < 100 { 1.0 } else { 0.0 }), 320, 320, &mut few).unwrap_err();
        assert_eq!(said.code, Code::BudgetExhausted);
    }

    #[test]
    fn req_cli_014_u2netps_input_is_the_picture_squeezed_and_normalized_as_rembg_prepares_it() {
        // A picture of 2 colours side by side, 640 × 100 px: squeezed to 320 × 320, the left half red. Their
        // red is the same, so Lanczos's filter, which overshoots at an edge, keeps the largest value at 200.
        let mut rgba = Vec::new();
        for _y in 0..100 {
            for x in 0..640 {
                rgba.extend(if x < 320 { [200, 0, 0, 255] } else { [200, 100, 50, 255] });
            }
        }
        let picture = Picture::new(640, 100, rgba).unwrap();
        let values = input(&picture, &mut Work::new(Budget::DEFAULT)).unwrap();
        assert_eq!(values.len(), 3 * 320 * 320);
        // Divided by the largest value, 200, and normalized as ImageNet's colours are.
        let at = |channel: usize, x: usize, y: usize| values[channel * 320 * 320 + y * 320 + x];
        let expected = |colour: f32, channel: usize| (colour / 200.0 - MEAN[channel]) / DEVIATION[channel];
        for (channel, colour) in [200.0, 0.0, 0.0].into_iter().enumerate() {
            assert!((at(channel, 10, 50) - expected(colour, channel)).abs() < 1e-6, "red half, channel {channel}");
        }
        for (channel, colour) in [200.0, 100.0, 50.0].into_iter().enumerate() {
            assert!((at(channel, 300, 300) - expected(colour, channel)).abs() < 1e-6, "right half, channel {channel}");
        }
    }

    #[test]
    fn diag_sc_e1202_u2netp_finding_no_subject_suggests_the_box() {
        let picture = Picture::new(2, 3, vec![0; 24]).unwrap();
        let said = not_found(&picture);
        assert_eq!(said.to_string(), "error SC-E1202: U²-Net-p found no subject in the picture, 2 × 3 px: no part of its map reaches a half.");
        assert_eq!(
            said.fix.map(|fix| fix.describe()).as_deref(),
            Some("Give the subject's box with --subject-box X0,Y0,X1,Y1, in the picture's pixels.")
        );
    }

    #[test]
    fn req_cli_014_a_piece_at_a_half_is_found_and_the_first_of_2_equal_pieces_is_kept() {
        let mut work = Work::new(Budget::DEFAULT);
        // 3 values of exactly a half in a row are more of the map than 1 value of 1 alone.
        let halves = map(|x, y| {
            if y == 10 && (10..13).contains(&x) {
                0.5
            } else if (x, y) == (200, 200) {
                1.0
            } else {
                0.0
            }
        });
        let found = subject_box(&halves, 320, 320, &mut work).unwrap().unwrap();
        assert_eq!(found, PixelBox { x0: 10.0, y0: 10.0, x1: 13.0, y1: 11.0 });
        // 2 pieces with as much of the map each: the first met is kept.
        let equal = map(|x, y| if (x, y) == (10, 10) || (x, y) == (200, 200) { 1.0 } else { 0.0 });
        let found = subject_box(&equal, 320, 320, &mut work).unwrap().unwrap();
        assert_eq!(found, PixelBox { x0: 10.0, y0: 10.0, x1: 11.0, y1: 11.0 });
    }
}
