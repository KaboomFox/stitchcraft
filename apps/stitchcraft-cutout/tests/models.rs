//! The models themselves, run by tract on the owner's 2 photos of her dog and on a picture drawn in code: the
//! cases of `REQ-CLI-019` and the marks', which a job of its own runs (ADR-0019, decision 8;
//! `docs/src/design/conformance.md` › Cases with the models).
//!
//! **Models only.** The models are 49 MB, fetched from StitchCraft's release, so these cases are ignored in
//! the normal run with a reason of a fixed form, "models only: …", and the models' job runs this file with
//! `--ignored`. It fetches the models into the folder `STITCHCRAFT_MODELS` names, as `stitch` does, which
//! checks each file against its pin. The steps after the models have cases of their own, from the models'
//! outputs saved as fixtures, that run everywhere (`tests/photos.rs`).
//!
//! **What they check.** Each cut-out from U²-Net-p's box keeps the dog with an IoU of 0.97 or more against
//! its mask checked by hand, with at most 2 % of the dog's area kept wrongly and 3 % lost, and so does the
//! running dog's with a point on the tyre behind it that isn't the subject. On the picture drawn in code the
//! marks choose between a disc and a square that touch, within the same bounds of their exact masks. tract's
//! maps match the outputs saved as fixtures, which the steps' cases read: run with `STITCHCRAFT_CUTOUT_BLESS`
//! set, this file writes them instead.
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::disallowed_methods)]

mod common;

use common::{
    DISC, SQUARE, Score, disc_marks, drawn, mask, photo, save_logits, save_map, saved_logits, saved_map, score, square_marks, tyre_marks,
    within_bounds, work,
};
use stitchcraft_cutout::pins::{Pins, folder, ready};
use stitchcraft_cutout::run::Tract;
use stitchcraft_cutout::segment::{self, Frame};
use stitchcraft_cutout::{CutOut, Marks, Models, Picture, Prompt, cut_out, salient};

/// The models, fetched into the folder `STITCHCRAFT_MODELS` names when they aren't there, and checked.
fn tract() -> Tract {
    let ready = ready(&Pins::pinned().unwrap(), folder().as_deref()).unwrap();
    Tract::load(&ready, &mut work()).unwrap()
}

/// Whether to write the models' outputs as fixtures instead of comparing them.
fn bless() -> bool {
    std::env::var_os("STITCHCRAFT_CUTOUT_BLESS").is_some()
}

/// The cut-out of `picture`, named `name`, with `marks`, scored against `mask`.
fn scored(tract: &mut Tract, name: &str, picture: &Picture, marks: &Marks, mask: &[bool]) -> (CutOut, Score) {
    let cut = cut_out(picture, marks, tract, &mut work()).unwrap();
    let score = score(&cut.alpha, mask);
    println!("{name}, {} points: box {}, crop {:?}, {score:?}", marks.not_subject.len() + marks.subject.len(), cut.subject, cut.crop);
    (cut, score)
}

#[test]
#[ignore = "models only: the models job runs tests/models.rs"]
fn req_cli_019_the_cut_out_from_u2netps_box_keeps_each_dog_within_the_bounds() {
    let mut tract = tract();
    for name in ["dog-on-turf", "dog-running"] {
        let (_, score) = scored(&mut tract, name, &photo(name), &Marks::default(), &mask(name));
        assert!(within_bounds(&score), "{name}: {score:?}");
    }
}

#[test]
#[ignore = "models only: the models job runs tests/models.rs"]
fn req_cli_019_a_point_on_the_tyre_that_isnt_the_subject_keeps_the_running_dog_within_the_bounds() {
    let (_, score) = scored(&mut tract(), "dog-running", &photo("dog-running"), &tyre_marks(), &mask("dog-running"));
    assert!(within_bounds(&score), "{score:?}");
}

#[test]
#[ignore = "models only: the models job runs tests/models.rs"]
fn req_cli_015_on_the_picture_drawn_in_code_the_marks_choose_between_2_shapes_that_touch() {
    let (drawn, mut tract) = (drawn(), tract());
    // A point of the disc and one of the square, in U²-Net-p's box, keep the disc; a box round the square
    // keeps the square. Each is cropped to its shape, give or take 2 px.
    for (name, marks, mask, edges) in [("disc", disc_marks(), &drawn.disc, DISC), ("square", square_marks(), &drawn.square, SQUARE)] {
        let (cut, score) = scored(&mut tract, name, &drawn.picture, &marks, mask);
        assert!(within_bounds(&score), "{name}: {score:?}");
        assert!(cut.crop.iter().zip(edges).all(|(got, want)| got.abs_diff(want) <= 2), "{name}: {:?}", cut.crop);
    }
}

/// The largest difference between `got` and `saved`.
fn largest_difference(got: &[f32], saved: &[f32]) -> f32 {
    assert_eq!(got.len(), saved.len());
    got.iter().zip(saved).map(|(a, b)| (a - b).abs()).fold(0.0, f32::max)
}

#[test]
#[ignore = "models only: the models job runs tests/models.rs"]
fn req_cli_019_tract_gives_the_models_outputs_saved_for_the_pictures() {
    let mut tract = tract();
    let pictures = [
        ("dog-on-turf", photo("dog-on-turf"), vec![("auto", Marks::default())]),
        ("dog-running", photo("dog-running"), vec![("auto", Marks::default()), ("tyre", tyre_marks())]),
        ("drawn", drawn().picture, vec![("disc", disc_marks()), ("square", square_marks())]),
    ];
    for (name, picture, prompts) in pictures {
        let map = tract.salient(&salient::input(&picture, &mut work()).unwrap()).unwrap();
        let found = salient::subject_box(&map, picture.width(), picture.height(), &mut work()).unwrap().unwrap();
        let frame = Frame::of(picture.width(), picture.height());
        let embedding = tract.embed(&segment::encoder_input(&picture, &frame, &mut work()).unwrap()).unwrap();
        if bless() {
            save_map(name, &map);
        } else {
            let difference = largest_difference(&map, &saved_map(name));
            println!("{name}: U²-Net-p's map within {difference:.2e} of the one saved");
            assert!(difference <= 1e-3, "{name}: {difference}");
        }
        for (prompt, marks) in prompts {
            let subject = marks.subject_box.unwrap_or(found);
            let low = tract.decode(&embedding, &Prompt::new(&subject, &marks, &frame), picture.width(), picture.height()).unwrap();
            if bless() {
                save_logits(name, prompt, low.logits());
                continue;
            }
            let difference = largest_difference(low.logits(), &saved_logits(name, prompt));
            println!("{name}, {prompt}: the decoder's logits within {difference:.2e} of those saved, its fit predicted {:.3}", low.predicted_iou());
            assert!(difference <= 0.01, "{name} {prompt}: {difference}");
        }
    }
}
