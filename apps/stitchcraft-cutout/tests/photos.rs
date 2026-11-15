//! The steps after the models, on the owner's 2 photos of her dog and on a picture drawn in code, from the
//! outputs tract gave for them, saved as fixtures (ADR-0019, decision 8): the box U²-Net-p's map gives, the
//! decoder's prompt, the alpha, the crop and the file, and how each cut-out scores against its mask, checked
//! by hand for the photos and exact for the drawn picture. These run without the models, everywhere. The
//! models' job checks that tract still gives those outputs (`tests/models.rs`).
#![allow(clippy::unwrap_used, clippy::disallowed_methods, clippy::cast_possible_truncation)]

mod common;

use common::{
    CENTRE, DISC, IN_SQUARE, ROUND_SQUARE, SQUARE, Saved, TYRE, bounds, disc_marks, drawn, mask, photo, saved_logits, saved_map, score, square_marks,
    tyre_marks, within_bounds, work,
};
use stitchcraft_cutout::{BoxFrom, CutOut, Marks, Picture, PixelBox, Point, cut_out};

/// U²-Net-p's box round each dog, from its saved map: the edges of its grid's cells, each 3.75 px across,
/// and 5 px down on the turf and 5.58125 px on the run.
const TURF: PixelBox = PixelBox { x0: 352.5, y0: 0.0, x1: 881.25, y1: 1190.0 };
const RUNNING: PixelBox = PixelBox { x0: 161.25, y0: 223.25, x1: 742.5, y1: 1289.26875 };

/// The models' outputs saved for the dog `name`: U²-Net-p's map, and the decoder's logits for its box, and
/// on the running dog's photo for its box with the point on the tyre.
fn dog(name: &str) -> Saved {
    let mut decoded = vec![(vec![2.0, 3.0], saved_logits(name, "auto"))];
    if name == "dog-running" {
        decoded.push((vec![0.0, 2.0, 3.0], saved_logits(name, "tyre")));
    }
    Saved::new(saved_map(name), decoded)
}

/// The models' outputs saved for the picture drawn in code: U²-Net-p's map, and the decoder's logits for
/// the disc's marks and for the square's.
fn drawn_outputs() -> Saved {
    let decoded = vec![(vec![1.0, 0.0, 2.0, 3.0], saved_logits("drawn", "disc")), (vec![2.0, 3.0], saved_logits("drawn", "square"))];
    Saved::new(saved_map("drawn"), decoded)
}

/// The cut-out of `picture` with `marks` from the outputs `saved`, and what the models were asked.
fn cut(picture: &Picture, marks: &Marks, mut saved: Saved) -> (CutOut, Saved) {
    let cut = cut_out(picture, marks, &mut saved, &mut work()).unwrap();
    (cut, saved)
}

/// `point` scaled by `across` and `down`, as the decoder takes it.
fn scaled(point: Point, [across, down]: [f64; 2]) -> [f32; 2] {
    [(point.x * across) as f32, (point.y * down) as f32]
}

/// Whether the places `got` are `want` within a thousandth of a pixel.
fn near(got: &[[f32; 2]], want: &[[f32; 2]]) -> bool {
    got.len() == want.len() && got.iter().zip(want).all(|(a, b)| (a[0] - b[0]).abs() < 1e-3 && (a[1] - b[1]).abs() < 1e-3)
}

/// The corners of `subject`, scaled by `scale`.
fn corners(subject: PixelBox, scale: [f64; 2]) -> [[f32; 2]; 2] {
    [scaled(Point { x: subject.x0, y: subject.y0 }, scale), scaled(Point { x: subject.x1, y: subject.y1 }, scale)]
}

#[test]
fn req_cli_019_from_the_models_saved_outputs_each_dog_is_cut_out_within_the_bounds() {
    for (name, marks) in [("dog-on-turf", Marks::default()), ("dog-running", Marks::default()), ("dog-running", tyre_marks())] {
        let (cut, _) = cut(&photo(name), &marks, dog(name));
        let score = score(&cut.alpha, &mask(name));
        assert!(within_bounds(&score), "{name}, {marks:?}: {score:?}");
    }
}

#[test]
fn req_cli_014_u2netps_box_round_each_dog_is_the_box_the_decoder_takes() {
    // The turf's photo, 1,200 × 1,600 px, scales by 0.64 on each axis, and the running dog's, 1,200 × 1,786
    // px, to 688 × 1,024.
    for (name, found, scale) in [("dog-on-turf", TURF, [0.64, 0.64]), ("dog-running", RUNNING, [688.0 / 1200.0, 1024.0 / 1786.0])] {
        let (cut, models) = cut(&photo(name), &Marks::default(), dog(name));
        assert_eq!((cut.subject, cut.from, models.salient_runs), (found, BoxFrom::Found, 1), "{name}");
        assert_eq!(models.prompts.len(), 1);
        assert_eq!(models.prompts[0].labels, [2.0, 3.0]);
        assert!(near(&models.prompts[0].coords, &corners(found, scale)), "{name}: {:?}", models.prompts[0].coords);
    }
}

#[test]
fn req_cli_014_the_running_dogs_cut_out_is_the_photo_cropped_round_what_it_kept_with_its_alpha() {
    let picture = photo("dog-running");
    let (cut, models) = cut(&picture, &tyre_marks(), dog("dog-running"));
    // The point on the tyre comes first, with the label 0, and then the box's corners.
    let scale = [688.0 / 1200.0, 1024.0 / 1786.0];
    assert_eq!(models.prompts[0].labels, [0.0, 2.0, 3.0]);
    let [top_left, bottom_right] = corners(RUNNING, scale);
    assert!(near(&models.prompts[0].coords, &[scaled(TYRE, scale), top_left, bottom_right]), "{:?}", models.prompts[0].coords);
    // The crop is the box round the pixels kept.
    let kept: Vec<bool> = cut.alpha.iter().map(|a| *a >= 128).collect();
    assert_eq!((cut.crop, bounds(&kept, 1200), cut.cropped()), ([173, 226, 673, 1290], [173, 226, 673, 1290], [500, 1064]));
    // The file is the photo inside the crop, each pixel with its colour and its alpha.
    let file = image::load_from_memory(&cut.png).unwrap().into_rgba8();
    assert_eq!(file.dimensions(), (500, 1064));
    for (x, y, pixel) in file.enumerate_pixels() {
        let at = (y as usize + 226) * 1200 + x as usize + 173;
        assert_eq!((&pixel.0[..3], pixel.0[3]), (&picture.rgba()[at * 4..at * 4 + 3], cut.alpha[at]), "({x}, {y})");
    }
    assert_eq!(
        cut.said().to_string(),
        "info SC-I1206: The subject was cut out of the box 161.25,223.25,742.5,1289.26875, which U²-Net-p found: 11.3 % of the picture is kept, cropped \
         to 500 × 1064 px."
    );
}

#[test]
fn req_cli_015_on_the_picture_drawn_in_code_a_point_of_the_disc_and_one_of_the_square_keep_the_disc() {
    let drawn = drawn();
    let (cut, models) = cut(&drawn.picture, &disc_marks(), drawn_outputs());
    // U²-Net-p's box holds both shapes, and the decoder takes the points first: the disc's, then the
    // square's, each scaled by 1.6 from 640 × 480 px to 1,024 × 768.
    assert_eq!((cut.subject, cut.from), (PixelBox { x0: 80.0, y0: 120.0, x1: 560.0, y1: 400.5 }, BoxFrom::Found));
    assert_eq!(models.prompts[0].labels, [1.0, 0.0, 2.0, 3.0]);
    let [top_left, bottom_right] = corners(cut.subject, [1.6, 1.6]);
    assert!(near(&models.prompts[0].coords, &[[352.0, 416.0], [736.0, 416.0], top_left, bottom_right]), "{:?}", models.prompts[0].coords);
    assert_eq!(scaled(CENTRE, [1.6, 1.6]), [352.0, 416.0]);
    assert_eq!(scaled(IN_SQUARE, [1.6, 1.6]), [736.0, 416.0]);
    // It keeps the disc, against its exact mask, and nothing of the square: cropped to the disc's box but
    // its last column and row.
    let score = score(&cut.alpha, &drawn.disc);
    assert!(within_bounds(&score), "{score:?}");
    assert_eq!(cut.alpha.iter().zip(&drawn.square).filter(|(a, square)| **square && **a >= 128).count(), 0);
    assert_eq!((DISC, cut.crop), ([80, 120, 360, 400], [80, 120, 359, 399]));
}

#[test]
fn req_cli_015_a_box_given_is_the_box_the_decoder_takes_and_u2netp_doesnt_run() {
    let drawn = drawn();
    let (cut, models) = cut(&drawn.picture, &square_marks(), drawn_outputs());
    assert_eq!((cut.subject, cut.from, models.salient_runs), (ROUND_SQUARE, BoxFrom::Given, 0));
    assert_eq!(models.prompts[0].labels, [2.0, 3.0]);
    assert!(near(&models.prompts[0].coords, &[[560.0, 272.0], [912.0, 560.0]]), "{:?}", models.prompts[0].coords);
    // It keeps the square, against its exact mask, cropped to the square's box.
    let score = score(&cut.alpha, &drawn.square);
    assert!(within_bounds(&score), "{score:?}");
    assert_eq!((cut.crop, bounds(&drawn.square, 640)), (SQUARE, SQUARE));
}
