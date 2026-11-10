//! The 3 models run by tract, from their ONNX files, on one thread (ADR-0019, decision 4).
//!
//! **Why tract.** It is written in Rust, under the MIT or Apache-2.0 licence, so `cargo-deny` checks it as it
//! checks every crate, and it builds for WASI, where `stitch` makes the golden files. It runs the ONNX files
//! as their exporters wrote them, and runs on the calling thread unless told otherwise.
//!
//! **No randomness.** tract runs ONNX's random operators, and seeds one from the computer's own randomness
//! when its model gives no seed. So before tract reads a model, its graph, the graphs nested in its nodes and
//! its functions are searched for one ([`RANDOM`]), and a model that has one is refused (`SC-E1205`): no
//! cut-out calls them. `deny.toml` admits `rand` and `getrandom` through tract's operators on that ground.
//!
//! **Shapes known before optimizing.** Each model's inputs are given their shapes, and tract optimizes the
//! model for them: U²-Net-p's and the encoder's once, the decoder's for the number of marks each prompt has,
//! which its ONNX file leaves open. Only the outputs the cut-out reads are kept: U²-Net-p's first map, which
//! fuses the others, and the decoder's map and the fit it predicts. The decoder's own scaling of its map to
//! the picture is never computed, as [`crate::segment::alpha`] does that.

use std::sync::Arc;

use stitchcraft_core::{Code, Diagnostic, Fix};
use tract_onnx::pb::{ModelProto, NodeProto};
use tract_onnx::prelude::{
    DatumExt, Framework, InferenceModel, InferenceModelExt, IntoRunnable, IntoTValue, TValue, TVec, Tensor, TractError, TypedRunnableModel,
};

use crate::Models;
use crate::pins::Ready;
use crate::salient::SIZE;
use crate::segment::{EMBEDDING_SHAPE, Embedding, LOW, LowRes, Prompt, SIDE};
use crate::work::Work;

/// ONNX's operators that draw random numbers, which no model of the cut-out may have.
pub const RANDOM: [&str; 6] = ["RandomUniform", "RandomUniformLike", "RandomNormal", "RandomNormalLike", "Multinomial", "Bernoulli"];
/// Where bugs are reported.
const ISSUES: &str = "https://github.com/KaboomFox/stitchcraft/issues";

/// The names of the 3 files, as the messages give them.
pub const U2NETP: &str = "u2netp.onnx";
/// MobileSAM's encoder's file.
pub const ENCODER: &str = "mobilesam-encoder.onnx";
/// MobileSAM's decoder's file.
pub const DECODER: &str = "mobilesam-decoder.onnx";
/// The decoder's outputs the cut-out reads.
const DECODED: [&str; 2] = ["low_res_masks", "iou_predictions"];

/// The 3 models, read and optimized by tract.
#[derive(Debug)]
pub struct Tract {
    salient: Arc<TypedRunnableModel>,
    encoder: Arc<TypedRunnableModel>,
    decoder: InferenceModel,
}

impl Tract {
    /// The models in `ready`, read, U²-Net-p and the encoder optimized for their inputs, charging `work` for
    /// each node searched for a random operator: `SC-E1204` when tract can't read or optimize one, and
    /// `SC-E1205` when one has a random operator.
    pub fn load(ready: &Ready, work: &mut Work) -> Result<Tract, Diagnostic> {
        let (side, square) = (size(SIZE), size(SIDE));
        let salient = optimized(U2NETP, read(U2NETP, &ready.u2netp, work)?, &[vec![1, 3, side, side]], Keep::First)?;
        let encoder = optimized(ENCODER, read(ENCODER, &ready.encoder, work)?, &[vec![1, 3, square, square]], Keep::All)?;
        let decoder = read(DECODER, &ready.decoder, work)?;
        Ok(Tract { salient, encoder, decoder })
    }
}

/// `value`, a side in pixels, as a length.
fn size(value: u32) -> usize {
    usize::try_from(value).unwrap_or(usize::MAX)
}

/// The model in `bytes`, the file `file`, as tract reads it: `SC-E1204` when it can't, and `SC-E1205` when it
/// has a random operator, which is looked for first, every node charging `work`.
pub fn read(file: &str, bytes: &[u8], work: &mut Work) -> Result<InferenceModel, Diagnostic> {
    let onnx = tract_onnx::onnx();
    let proto = onnx.proto_model_for_read(&mut &*bytes).map_err(|e| unrunnable(file, &e))?;
    if let Some(operator) = random_operator(&proto, work)? {
        return Err(random(file, &operator));
    }
    onnx.model_for_proto_model(&proto).map_err(|e| unrunnable(file, &e))
}

/// The first of [`RANDOM`] in `proto`: in its graph, in a graph nested in a node of it, or in one of its
/// functions, each node charging `work`.
pub fn random_operator(proto: &ModelProto, work: &mut Work) -> Result<Option<String>, Diagnostic> {
    let mut lists: Vec<&[NodeProto]> = proto.graph.iter().map(|graph| graph.node.as_slice()).collect();
    lists.extend(proto.functions.iter().map(|function| function.node.as_slice()));
    while let Some(nodes) = lists.pop() {
        work.charge_each(nodes.len())?;
        for node in nodes {
            if RANDOM.contains(&node.op_type.as_str()) {
                return Ok(Some(node.op_type.clone()));
            }
            for attribute in &node.attribute {
                lists.extend(attribute.g.iter().chain(&attribute.graphs).map(|graph| graph.node.as_slice()));
            }
        }
    }
    Ok(None)
}

/// Which of a model's outputs to keep.
#[derive(Clone, Copy)]
enum Keep<'a> {
    /// Every one.
    All,
    /// The first.
    First,
    /// These, by name.
    Named(&'a [&'a str]),
}

/// `model`, the file `file`, its inputs given `shapes` in their order and its outputs kept as `keep` says,
/// optimized by tract; `SC-E1204` when it can't be.
fn optimized(file: &str, mut model: InferenceModel, shapes: &[Vec<usize>], keep: Keep<'_>) -> Result<Arc<TypedRunnableModel>, Diagnostic> {
    let broken = |e: TractError| unrunnable(file, &e);
    for (input, shape) in shapes.iter().enumerate() {
        model.set_input_fact(input, f32::fact(shape.as_slice()).into()).map_err(broken)?;
    }
    match keep {
        Keep::All => {}
        Keep::First => {
            let first = model.output_outlets().map_err(broken)?.first().copied();
            let first = first.ok_or_else(|| unrunnable(file, &TractError::msg("it has no output")))?;
            model.select_output_outlets(&[first]).map_err(broken)?;
        }
        Keep::Named(names) => model.select_outputs_by_name(names).map_err(broken)?,
    }
    model.into_optimized().map_err(broken)?.into_runnable().map_err(broken)
}

/// What `plan`, the model `file`, gives for `inputs`, each a shape and its values: each output's values;
/// `SC-E1204` when it can't run.
fn run(file: &str, plan: &Arc<TypedRunnableModel>, inputs: &[(Vec<usize>, Vec<f32>)]) -> Result<Vec<Vec<f32>>, Diagnostic> {
    let broken = |e: TractError| unrunnable(file, &e);
    let values: TVec<TValue> = inputs
        .iter()
        .map(|(shape, values)| Tensor::from_shape(shape, values).map(IntoTValue::into_tvalue))
        .collect::<Result<_, _>>()
        .map_err(broken)?;
    let outputs = plan.run(values).map_err(broken)?;
    outputs
        .iter()
        .map(|output| output.to_plain_array_view::<f32>().map(|view| view.iter().copied().collect()))
        .collect::<Result<_, _>>()
        .map_err(broken)
}

/// The only output of a run of `file`; `SC-E1204` when it gave none.
fn only(file: &str, outputs: Vec<Vec<f32>>) -> Result<Vec<f32>, Diagnostic> {
    outputs.into_iter().next().ok_or_else(|| unrunnable(file, &TractError::msg("it gave no output")))
}

impl Models for Tract {
    fn salient(&mut self, input: &[f32]) -> Result<Vec<f32>, Diagnostic> {
        let side = size(SIZE);
        only(U2NETP, run(U2NETP, &self.salient, &[(vec![1, 3, side, side], input.to_vec())])?)
    }

    fn embed(&mut self, input: &[f32]) -> Result<Embedding, Diagnostic> {
        let square = size(SIDE);
        Embedding::new(only(ENCODER, run(ENCODER, &self.encoder, &[(vec![1, 3, square, square], input.to_vec())])?)?)
    }

    fn decode(&mut self, embedding: &Embedding, prompt: &Prompt, width: u32, height: u32) -> Result<LowRes, Diagnostic> {
        let (marks, low) = (prompt.labels.len(), size(LOW));
        let given = [
            ("image_embeddings", EMBEDDING_SHAPE.to_vec(), embedding.values().to_vec()),
            ("point_coords", vec![1, marks, 2], prompt.coords.iter().flatten().copied().collect()),
            ("point_labels", vec![1, marks], prompt.labels.clone()),
            ("mask_input", vec![1, 1, low, low], vec![0.0; low * low]),
            ("has_mask_input", vec![1], vec![0.0]),
            ("orig_im_size", vec![2], vec![float(height), float(width)]),
        ];
        // In the order of the model's own inputs, by their names.
        let model = &self.decoder;
        let names = model
            .input_outlets()
            .map_err(|e| unrunnable(DECODER, &e))?
            .iter()
            .map(|outlet| model.nodes().get(outlet.node).map(|node| node.name.as_str()));
        let inputs: Vec<(Vec<usize>, Vec<f32>)> = names
            .map(|name| given.iter().find(|(given, ..)| Some(*given) == name).map(|(_, shape, values)| (shape.clone(), values.clone())))
            .collect::<Option<_>>()
            .ok_or_else(|| unrunnable(DECODER, &TractError::msg("its inputs are not MobileSAM's decoder's")))?;
        let shapes: Vec<Vec<usize>> = inputs.iter().map(|(shape, _)| shape.clone()).collect();
        let plan = optimized(DECODER, self.decoder.clone(), &shapes, Keep::Named(&DECODED))?;
        let mut outputs = run(DECODER, &plan, &inputs)?.into_iter();
        let (Some(logits), Some(fit)) = (outputs.next(), outputs.next()) else {
            return Err(unrunnable(DECODER, &TractError::msg("it gave fewer outputs than its map and its predicted fit")));
        };
        LowRes::new(logits, fit.first().copied().unwrap_or(0.0))
    }
}

/// `n`, a side of at most [`MAX_SIDE`](crate::picture::MAX_SIDE) px, as the decoder takes it.
#[allow(clippy::cast_precision_loss)] // A side is at most 16,384 px, exact in f32.
fn float(n: u32) -> f32 {
    n as f32
}

/// `SC-E1204`: tract can't run the model `file`, and why.
fn unrunnable(file: &str, error: &TractError) -> Diagnostic {
    Diagnostic::new(Code::CutoutModelUnrunnable, format!("tract could not run the subject cut-out's model {file}: {error:#}."))
        .with_fix(Fix::Hint(format!("Please report it at {ISSUES} with this message. Without --subject, the picture plans as before.")))
}

/// `SC-E1205`: the model `file` has the random operator `operator`.
fn random(file: &str, operator: &str) -> Diagnostic {
    Diagnostic::new(
        Code::CutoutModelRandom,
        format!("The subject cut-out's model {file} has a random operator, {operator}, which would cut a picture out otherwise on each run, so it was not run."),
    )
    .with_fix(Fix::Hint(format!("The pinned models have none: please report it at {ISSUES} with this message.")))
}

#[cfg(test)]
mod tests;
