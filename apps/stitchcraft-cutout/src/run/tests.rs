// The tests read the stand-ins for the models from the fixtures.
#![allow(clippy::disallowed_methods)]

use stitchcraft_core::Budget;
use tract_onnx::pb::{AttributeProto, FunctionProto, GraphProto};

use super::*;
use crate::marks::{Marks, PixelBox, Point};
use crate::picture::Picture;
use crate::segment::Frame;
use crate::{BoxFrom, cut_out};

/// A fixture of the cut-out's, by name (`conformance/fixtures/cutout/`).
fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!("{}/../../conformance/fixtures/cutout/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
}

/// The stand-ins for the 3 models (`make_stand_ins.py`).
fn stand_ins() -> Ready {
    Ready { u2netp: fixture("stand-in-u2netp.onnx"), encoder: fixture("stand-in-encoder.onnx"), decoder: fixture("stand-in-decoder.onnx") }
}

fn work() -> Work {
    Work::new(Budget::DEFAULT)
}

/// The logistic function of `x`.
#[allow(clippy::cast_possible_truncation)] // To the nearest f32, as the stand-in computes it.
fn logistic(x: f32) -> f32 {
    (1.0 / (1.0 + stitchcraft_core::math::exp(-f64::from(x)))) as f32
}

#[test]
fn req_cli_014_tract_runs_each_model_with_its_inputs_shaped_and_only_the_outputs_read() {
    let mut tract = Tract::load(&stand_ins(), &mut work()).unwrap();
    // U²-Net-p's stand-in gives 2 maps, and the first is kept: the logistic function of the colours' mean.
    let input: Vec<f32> = (0..3 * 320 * 320).map(|at| if at % 320 < 160 { 2.0 } else { -2.0 }).collect();
    let map = tract.salient(&input).unwrap();
    assert_eq!(map.len(), 320 * 320);
    assert!((map[0] - logistic(2.0)).abs() < 1e-6 && (map[319] - logistic(-2.0)).abs() < 1e-6, "{} {}", map[0], map[319]);
    // The encoder's stand-in averages squares of 16 × 16 px and mixes the 3 colours into 256 channels.
    let embedding = tract.embed(&vec![1.0; 3 * 1024 * 1024]).unwrap();
    let mixed = |channel: usize| (0..3).map(|colour| ((3 * channel + colour) % 7) as f32 - 3.0).sum::<f32>() / 10.0;
    for channel in [0, 1, 100, 255] {
        assert!((embedding.values()[channel * 64 * 64] - mixed(channel)).abs() < 1e-5, "channel {channel}");
    }
    // The decoder's stand-in, optimized again for each number of marks: its map is the labels' sum less 5.5,
    // and the fit it predicts the mean of the marks' coordinates.
    for (labels, sum) in [(vec![2.0, 3.0], 5.0), (vec![1.0, 2.0, 3.0], 6.0), (vec![1.0, 0.0, 1.0, 2.0, 3.0], 7.0)] {
        let coords: Vec<[f32; 2]> = (0..labels.len()).map(|at| [at as f32, 10.0]).collect();
        let mean = coords.iter().flatten().sum::<f32>() / (2 * coords.len()) as f32;
        let low = tract.decode(&embedding, &Prompt { coords, labels: labels.clone() }, 1200, 1600).unwrap();
        assert!(low.logits().iter().all(|logit| (logit - (sum - 5.5)).abs() < 1e-6), "{labels:?}");
        assert!((low.predicted_iou() - mean).abs() < 1e-5, "{labels:?}: {}", low.predicted_iou());
    }
}

#[test]
fn req_cli_014_the_cut_out_takes_the_box_u2netp_finds_and_the_decoders_map() {
    // 320 × 320 px, white on the left and black on the right: U²-Net-p's stand-in maps the left half.
    let rgba: Vec<u8> = (0..320 * 320).flat_map(|at| if at % 320 < 160 { [255, 255, 255, 255] } else { [0, 0, 0, 255] }).collect();
    let picture = Picture::new(320, 320, rgba).unwrap();
    let mut tract = Tract::load(&stand_ins(), &mut work()).unwrap();
    // A box alone sums its labels to 5: the stand-in's map is −0.5 everywhere, and nothing is kept.
    let said = cut_out(&picture, &Marks::default(), &mut tract, &mut work()).unwrap_err();
    assert_eq!(said.to_string(), "error SC-E1202: MobileSAM kept no pixel of the picture in the box 0,0,160,320, which U²-Net-p found.");
    // A point of the subject sums them to 6: 0.5 everywhere, and the whole picture is kept.
    let marks = Marks { subject: vec![Point { x: 80.0, y: 160.0 }], ..Marks::default() };
    let cut = cut_out(&picture, &marks, &mut tract, &mut work()).unwrap();
    assert_eq!((cut.subject, cut.from, cut.crop), (PixelBox { x0: 0.0, y0: 0.0, x1: 160.0, y1: 320.0 }, BoxFrom::Found, [0, 0, 320, 320]));
    assert!(cut.alpha.iter().all(|a| *a == 159), "255 × logistic(0.5), rounded");
    // A box given is the one taken (and U²-Net-p doesn't run: `tests/photos.rs`).
    let given = PixelBox { x0: 10.0, y0: 20.0, x1: 300.0, y1: 310.0 };
    let marks = Marks { subject_box: Some(given), ..marks };
    let cut = cut_out(&picture, &marks, &mut tract, &mut work()).unwrap();
    assert_eq!((cut.subject, cut.from), (given, BoxFrom::Given));
    assert_eq!(Frame::of(320, 320).to_square(Point { x: 80.0, y: 160.0 }), [256.0, 512.0]);
}

#[test]
fn diag_sc_e1205_a_model_with_a_random_operator_is_refused_before_it_runs() {
    let said = read("random-operator.onnx", &fixture("random-operator.onnx"), &mut work()).unwrap_err();
    assert_eq!(
        said.to_string(),
        "error SC-E1205: The subject cut-out's model random-operator.onnx has a random operator, RandomUniform, which would cut a picture out \
         otherwise on each run, so it was not run."
    );
    assert_eq!(
        said.fix.map(|fix| fix.describe()).as_deref(),
        Some("The pinned models have none: please report it at https://github.com/KaboomFox/stitchcraft/issues with this message.")
    );
    // Given as the decoder, it stops the models' loading.
    let ready = Ready { decoder: fixture("random-operator.onnx"), ..stand_ins() };
    assert_eq!(Tract::load(&ready, &mut work()).unwrap_err().code, Code::CutoutModelRandom);
}

/// A node of the operator `op_type`.
fn node(op_type: &str) -> NodeProto {
    NodeProto { op_type: op_type.to_string(), ..NodeProto::default() }
}

#[test]
fn every_random_operator_is_found_in_the_graph_its_nested_graphs_and_its_functions() {
    for operator in RANDOM {
        let model =
            ModelProto { graph: Some(GraphProto { node: vec![node("Conv"), node(operator)], ..GraphProto::default() }), ..ModelProto::default() };
        assert_eq!(random_operator(&model, &mut work()).unwrap().as_deref(), Some(operator));
    }
    // In a branch of an If node, as a graph and among graphs.
    let branch = GraphProto { node: vec![node("RandomNormal")], ..GraphProto::default() };
    let nested = |attribute: AttributeProto| ModelProto {
        graph: Some(GraphProto { node: vec![NodeProto { attribute: vec![attribute], ..node("If") }], ..GraphProto::default() }),
        ..ModelProto::default()
    };
    let as_graph = nested(AttributeProto { g: Some(branch.clone()), ..AttributeProto::default() });
    let among_graphs = nested(AttributeProto { graphs: vec![GraphProto::default(), branch], ..AttributeProto::default() });
    for model in [as_graph, among_graphs] {
        assert_eq!(random_operator(&model, &mut work()).unwrap().as_deref(), Some("RandomNormal"));
    }
    // In a function the model defines.
    let function =
        ModelProto { functions: vec![FunctionProto { node: vec![node("Multinomial")], ..FunctionProto::default() }], ..ModelProto::default() };
    assert_eq!(random_operator(&function, &mut work()).unwrap().as_deref(), Some("Multinomial"));
    // None, and every node is charged.
    let calm = ModelProto { graph: Some(GraphProto { node: vec![node("Conv"), node("Relu")], ..GraphProto::default() }), ..ModelProto::default() };
    let mut charged = work();
    assert_eq!(random_operator(&calm, &mut charged).unwrap(), None);
    assert_eq!(charged.left(), Budget::DEFAULT.max_work - 2);
    let mut few = Work::new(Budget { max_work: 1, ..Budget::DEFAULT });
    assert_eq!(random_operator(&calm, &mut few).unwrap_err().code, Code::BudgetExhausted);
}

#[test]
fn diag_sc_e1204_a_model_tract_can_not_read_or_run_says_what_tract_said() {
    let said = Tract::load(&Ready { u2netp: b"not a model".to_vec(), ..stand_ins() }, &mut work()).unwrap_err();
    assert_eq!(said.code, Code::CutoutModelUnrunnable);
    assert!(said.to_string().starts_with("error SC-E1204: tract could not run the subject cut-out's model u2netp.onnx: "), "{said}");
    assert_eq!(
        said.fix.map(|fix| fix.describe()).as_deref(),
        Some(
            "Please report it at https://github.com/KaboomFox/stitchcraft/issues with this message. Without --subject, the picture plans as before."
        )
    );
    // A decoder without MobileSAM's decoder's inputs, U²-Net-p's stand-in in its place.
    let mut tract = Tract::load(&Ready { decoder: fixture("stand-in-u2netp.onnx"), ..stand_ins() }, &mut work()).unwrap();
    let embedding = Embedding::new(vec![0.0; crate::segment::EMBEDDING]).unwrap();
    let said = tract.decode(&embedding, &Prompt { coords: vec![[0.0, 0.0]; 2], labels: vec![2.0, 3.0] }, 10, 10).unwrap_err();
    assert_eq!(said.message, "tract could not run the subject cut-out's model mobilesam-decoder.onnx: its inputs are not MobileSAM's decoder's.");
}
