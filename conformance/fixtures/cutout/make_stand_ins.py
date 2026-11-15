"""Writes the stand-ins for the subject cut-out's models that the runner's tests run, tiny ONNX models with the
real ones' inputs and outputs (apps/stitchcraft-cutout/src/run/tests.rs), and the model with a random
operator that SC-E1205's case refuses. Each computes something a test can tell from its output:

- stand-in-u2netp.onnx: `input.1`, 1 × 3 × 320 × 320, to 2 maps, each the logistic function of the mean of
  the 3 colours, as rembg's file gives 7 maps, the first fusing the others.
- stand-in-encoder.onnx: `input_image`, 1 × 3 × 1024 × 1024, averaged over squares of 16 × 16 px and mixed
  into 256 channels, to `image_embeddings`, 1 × 256 × 64 × 64.
- stand-in-decoder.onnx: MobileSAM's decoder's 6 inputs and 3 outputs, the number of points left open:
  `low_res_masks` is `mask_input` plus the sum of the labels less 5.5, `iou_predictions` the mean of the
  points' coordinates, and `masks` `mask_input` itself.
- random-operator.onnx: one RandomUniform node, which draws a 2 × 2 tensor.

Run with onnx (1.23.2 made the committed files): python3 -I make_stand_ins.py FOLDER
"""

import sys
from pathlib import Path

import numpy as np
import onnx
from onnx import TensorProto, helper, numpy_helper

out = Path(sys.argv[1])
F = TensorProto.FLOAT
opset = [helper.make_opsetid("", 17)]


def save(graph, name: str) -> None:
    model = helper.make_model(graph, opset_imports=opset, producer_name="StitchCraft's tests")
    model.ir_version = 8
    onnx.checker.check_model(model, full_check=True)
    onnx.save(model, out / name)


save(
    helper.make_graph(
        [
            helper.make_node("ReduceMean", ["input.1"], ["mean"], axes=[1], keepdims=1),
            helper.make_node("Sigmoid", ["mean"], ["d0"]),
            helper.make_node("Identity", ["d0"], ["d1"]),
        ],
        "stand-in-u2netp",
        [helper.make_tensor_value_info("input.1", F, [1, 3, 320, 320])],
        [helper.make_tensor_value_info("d0", F, [1, 1, 320, 320]), helper.make_tensor_value_info("d1", F, [1, 1, 320, 320])],
    ),
    "stand-in-u2netp.onnx",
)

weights = (np.arange(256 * 3, dtype=np.float32).reshape(256, 3, 1, 1) % 7 - 3) / 10
save(
    helper.make_graph(
        [
            helper.make_node("AveragePool", ["input_image"], ["pooled"], kernel_shape=[16, 16], strides=[16, 16]),
            helper.make_node("Conv", ["pooled", "mix"], ["image_embeddings"]),
        ],
        "stand-in-encoder",
        [helper.make_tensor_value_info("input_image", F, [1, 3, 1024, 1024])],
        [helper.make_tensor_value_info("image_embeddings", F, [1, 256, 64, 64])],
        initializer=[numpy_helper.from_array(weights, "mix")],
    ),
    "stand-in-encoder.onnx",
)

save(
    helper.make_graph(
        [
            helper.make_node("ReduceSum", ["point_labels", "label_axes"], ["labels"], keepdims=1),
            helper.make_node("Sub", ["labels", "offset"], ["shift"]),
            helper.make_node("Reshape", ["shift", "one_by_one"], ["shift_4d"]),
            helper.make_node("Add", ["mask_input", "shift_4d"], ["low_res_masks"]),
            helper.make_node("ReduceMean", ["point_coords"], ["coords_mean"], axes=[1, 2], keepdims=0),
            helper.make_node("Reshape", ["coords_mean", "one_by_one_2d"], ["iou_predictions"]),
            helper.make_node("Identity", ["mask_input"], ["masks"]),
        ],
        "stand-in-decoder",
        [
            helper.make_tensor_value_info("image_embeddings", F, [1, 256, 64, 64]),
            helper.make_tensor_value_info("point_coords", F, [1, "num_points", 2]),
            helper.make_tensor_value_info("point_labels", F, [1, "num_points"]),
            helper.make_tensor_value_info("mask_input", F, [1, 1, 256, 256]),
            helper.make_tensor_value_info("has_mask_input", F, [1]),
            helper.make_tensor_value_info("orig_im_size", F, [2]),
        ],
        [
            helper.make_tensor_value_info("masks", F, [1, 1, 256, 256]),
            helper.make_tensor_value_info("iou_predictions", F, [1, 1]),
            helper.make_tensor_value_info("low_res_masks", F, [1, 1, 256, 256]),
        ],
        initializer=[
            numpy_helper.from_array(np.array([1], np.int64), "label_axes"),
            numpy_helper.from_array(np.array([5.5], np.float32), "offset"),
            numpy_helper.from_array(np.array([1, 1, 1, 1], np.int64), "one_by_one"),
            numpy_helper.from_array(np.array([1, 1], np.int64), "one_by_one_2d"),
        ],
    ),
    "stand-in-decoder.onnx",
)

save(
    helper.make_graph(
        [helper.make_node("RandomUniform", [], ["noise"], shape=[2, 2], name="noise")],
        "random-operator",
        [],
        [helper.make_tensor_value_info("noise", F, [2, 2])],
    ),
    "random-operator.onnx",
)
