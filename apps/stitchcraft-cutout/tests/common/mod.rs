//! What the cut-out's integration tests share: the fixtures (`conformance/fixtures/cutout/`), a picture drawn
//! in code, how a cut-out is scored against a mask, the models' outputs saved as fixtures, and a stand-in
//! that gives them back.
//!
//! **The picture drawn in code** ([`drawn`]) is a red disc on grey stripes, touching a blue square on its
//! right. Its edges are hard, so its masks are exact, and the marks choose between 2 shapes that touch.
//!
//! **The saved outputs** are 16-bit greyscale PNG files, read and written with the `image` crate the cut-out
//! decodes pictures with: U²-Net-p's map of 320 × 320 values from 0 to 1, each times 65,535, and MobileSAM's
//! decoder's map of 256 × 256 logits, each times 256 plus 32,768, which keeps logits from −128 to 128 within
//! 1/512 of themselves. The models' job writes them with `STITCHCRAFT_CUTOUT_BLESS` set, from what tract
//! gives, and compares what tract gives with them otherwise.
// Test code may unwrap, print and read files, and each test file uses a part of this one.
#![allow(clippy::unwrap_used, clippy::disallowed_methods, clippy::print_stdout, clippy::cast_possible_truncation, clippy::cast_sign_loss, dead_code)]

use std::path::PathBuf;

use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_cutout::segment::{EMBEDDING, LOW};
use stitchcraft_cutout::{Embedding, LowRes, Marks, Models, Picture, PixelBox, Point, Prompt, Work};

/// The fixtures of the cut-out.
pub fn fixture(name: &str) -> PathBuf {
    PathBuf::from(format!("{}/../../conformance/fixtures/cutout/{name}", env!("CARGO_MANIFEST_DIR")))
}

/// The work a cut-out may spend: the command line's.
pub fn work() -> Work {
    Work::new(Budget::DEFAULT)
}

/// The photo `name`, as the cut-out decodes it.
pub fn photo(name: &str) -> Picture {
    Picture::decode(&std::fs::read(fixture(&format!("{name}.jpg"))).unwrap(), &mut work()).unwrap()
}

/// The mask of the photo `name`, checked by hand: whether each pixel is the dog, row by row.
pub fn mask(name: &str) -> Vec<bool> {
    let mask = image::open(fixture(&format!("{name}-mask.png"))).unwrap().into_luma8();
    mask.as_raw().iter().map(|value| *value >= 128).collect()
}

/// The picture drawn in code, 640 × 480 px, and the exact masks of its 2 shapes, row by row.
pub struct Drawn {
    pub picture: Picture,
    pub disc: Vec<bool>,
    pub square: Vec<bool>,
}

/// The drawn picture's disc: its centre and radius, and the box round its pixels.
pub const CENTRE: Point = Point { x: 220.0, y: 260.0 };
pub const RADIUS: f64 = 140.0;
pub const DISC: [u32; 4] = [80, 120, 360, 400];
/// The drawn picture's square, whose left edge touches the disc's right.
pub const SQUARE: [u32; 4] = [360, 180, 560, 340];
/// A point in the middle of the square.
pub const IN_SQUARE: Point = Point { x: 460.0, y: 260.0 };
/// A box round the square, 10 px from its edges.
pub const ROUND_SQUARE: PixelBox = PixelBox { x0: 350.0, y0: 170.0, x1: 570.0, y1: 350.0 };

/// The disc's marks: a point at its centre, and one in the square, which isn't the subject, in U²-Net-p's
/// box.
pub fn disc_marks() -> Marks {
    Marks { subject: vec![CENTRE], not_subject: vec![IN_SQUARE], ..Marks::default() }
}

/// The square's marks: a box round it alone.
pub fn square_marks() -> Marks {
    Marks { subject_box: Some(ROUND_SQUARE), ..Marks::default() }
}

/// A point on the near tyre behind the running dog, which isn't the subject.
pub const TYRE: Point = Point { x: 640.0, y: 720.0 };

/// The running dog's marks with the point on the tyre, in U²-Net-p's box.
pub fn tyre_marks() -> Marks {
    Marks { not_subject: vec![TYRE], ..Marks::default() }
}

/// The picture drawn in code ([the module](self)): a pixel is the disc's when its middle lies within the
/// radius, and the square's when it is in the square and not the disc's. The stripes are 16 px high.
pub fn drawn() -> Drawn {
    let (width, height) = (640, 480);
    let (mut rgba, mut disc, mut square) = (Vec::new(), Vec::new(), Vec::new());
    for y in 0..height {
        for x in 0..width {
            let (across, down) = (f64::from(x) + 0.5 - CENTRE.x, f64::from(y) + 0.5 - CENTRE.y);
            let in_disc = across * across + down * down <= RADIUS * RADIUS;
            let [x0, y0, x1, y1] = SQUARE;
            let in_square = !in_disc && (x0..x1).contains(&x) && (y0..y1).contains(&y);
            let colour = match (in_disc, in_square, y / 16 % 2) {
                (true, _, _) => [200, 40, 40],
                (_, true, _) => [40, 60, 200],
                (_, _, 0) => [235, 235, 235],
                _ => [200, 212, 225],
            };
            rgba.extend(colour.into_iter().chain([255]));
            disc.push(in_disc);
            square.push(in_square);
        }
    }
    Drawn { picture: Picture::new(width, height, rgba).unwrap(), disc, square }
}

/// The box round the pixels of `mask`, a picture `width` px across: left, top, right and bottom edges.
pub fn bounds(mask: &[bool], width: usize) -> [u32; 4] {
    let mut edges = [u32::MAX, u32::MAX, 0, 0];
    for (at, _) in mask.iter().enumerate().filter(|(_, kept)| **kept) {
        let (x, y) = (u32::try_from(at % width).unwrap(), u32::try_from(at / width).unwrap());
        edges = [edges[0].min(x), edges[1].min(y), edges[2].max(x + 1), edges[3].max(y + 1)];
    }
    edges
}

/// How a cut-out's pixels kept compare with a mask's: the intersection over the union, and the shares of
/// the mask's area kept wrongly and lost wrongly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Score {
    pub iou: f64,
    pub kept_wrongly: f64,
    pub lost_wrongly: f64,
}

/// `alpha`, each pixel's, against `mask`: a pixel is kept at an alpha of 128 or more.
pub fn score(alpha: &[u8], mask: &[bool]) -> Score {
    assert_eq!(alpha.len(), mask.len());
    let (mut both, mut kept_only, mut mask_only, mut subject) = (0u64, 0u64, 0u64, 0u64);
    for (a, m) in alpha.iter().zip(mask) {
        let kept = *a >= 128;
        both += u64::from(kept && *m);
        kept_only += u64::from(kept && !*m);
        mask_only += u64::from(!kept && *m);
        subject += u64::from(*m);
    }
    let share = |n: u64, of: u64| n as f64 / of as f64;
    Score { iou: share(both, both + kept_only + mask_only), kept_wrongly: share(kept_only, subject), lost_wrongly: share(mask_only, subject) }
}

/// Whether `score` is within `REQ-CLI-019`'s bounds: an IoU of 0.97 or more, at most 2 % of the subject's
/// area kept wrongly and 3 % lost.
pub fn within_bounds(score: &Score) -> bool {
    score.iou >= 0.97 && score.kept_wrongly <= 0.02 && score.lost_wrongly <= 0.03
}

/// A map saved as 16-bit greyscale, `side` × `side` values, each `value` × `scale` + `offset`, rounded.
fn save(name: &str, values: &[f32], side: u32, scale: f32, offset: f32) {
    let pixels: Vec<u16> = values.iter().map(|value| (value * scale + offset).round().clamp(0.0, 65_535.0) as u16).collect();
    image::ImageBuffer::<image::Luma<u16>, Vec<u16>>::from_raw(side, side, pixels).unwrap().save(fixture(name)).unwrap();
}

/// A map saved by [`save`].
fn saved(name: &str, scale: f32, offset: f32) -> Vec<f32> {
    let map = image::open(fixture(name)).unwrap().into_luma16();
    map.as_raw().iter().map(|value| (f32::from(*value) - offset) / scale).collect()
}

/// U²-Net-p's map of the photo `name`, saved.
pub fn saved_map(name: &str) -> Vec<f32> {
    saved(&format!("{name}-u2netp.png"), 65_535.0, 0.0)
}

/// Saves U²-Net-p's `map` of the photo `name`.
pub fn save_map(name: &str, map: &[f32]) {
    save(&format!("{name}-u2netp.png"), map, 320, 65_535.0, 0.0);
}

/// MobileSAM's decoder's logits for the prompt `prompt` of the photo `name`, saved.
pub fn saved_logits(name: &str, prompt: &str) -> Vec<f32> {
    saved(&format!("{name}-{prompt}-logits.png"), 256.0, 32_768.0)
}

/// Saves the decoder's `logits` for the prompt `prompt` of the photo `name`.
pub fn save_logits(name: &str, prompt: &str, logits: &[f32]) {
    save(&format!("{name}-{prompt}-logits.png"), logits, LOW, 256.0, 32_768.0);
}

/// The models' outputs saved for one picture, given back as the models gave them: U²-Net-p's map, and the
/// decoder's logits for the prompt of each set of labels. The encoder's embedding isn't saved, as the
/// decoder's outputs are.
pub struct Saved {
    pub map: Vec<f32>,
    pub decoded: Vec<(Vec<f32>, Vec<f32>)>,
    /// How many times U²-Net-p was run.
    pub salient_runs: usize,
    /// The prompts the decoder was given.
    pub prompts: Vec<Prompt>,
}

impl Saved {
    /// The outputs `map` and `decoded`, given back.
    pub fn new(map: Vec<f32>, decoded: Vec<(Vec<f32>, Vec<f32>)>) -> Saved {
        Saved { map, decoded, salient_runs: 0, prompts: Vec::new() }
    }
}

impl Models for Saved {
    fn salient(&mut self, input: &[f32]) -> Result<Vec<f32>, Diagnostic> {
        assert_eq!(input.len(), 3 * 320 * 320);
        self.salient_runs += 1;
        Ok(self.map.clone())
    }

    fn embed(&mut self, input: &[f32]) -> Result<Embedding, Diagnostic> {
        assert_eq!(input.len(), 3 * 1024 * 1024);
        Embedding::new(vec![0.0; EMBEDDING])
    }

    fn decode(&mut self, _: &Embedding, prompt: &Prompt, _: u32, _: u32) -> Result<LowRes, Diagnostic> {
        self.prompts.push(prompt.clone());
        let (_, logits) = self.decoded.iter().find(|(labels, _)| *labels == prompt.labels).unwrap();
        LowRes::new(logits.clone(), 0.0)
    }
}
