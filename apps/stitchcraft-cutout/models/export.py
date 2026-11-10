"""Exports MobileSAM's image encoder, and its prompt encoder and mask decoder, to the 2 ONNX files the subject
cut-out runs (ADR-0019, decision 5; apps/stitchcraft-cutout/README.md › The models).

Usage, in a virtual environment that holds requirements.txt, with MobileSAM's repository checked out at the
commit models.toml names:

    python -I export.py MOBILESAM_CHECKOUT OUT_DIR

It checks the checkout's commit and the checkpoint's size and SHA-256 against models.toml, builds MobileSAM's
`vit_t` model from the checkpoint with MobileSAM's own code, and writes OUT_DIR/mobilesam-encoder.onnx and
OUT_DIR/mobilesam-decoder.onnx with PyTorch's TorchScript exporter, opset 17:

- The encoder takes `input_image`, 1 × 3 × 1024 × 1024: the picture, its longer side 1,024 px, normalized with
  SAM's mean and deviation and padded with zeros to the square. It gives `image_embeddings`, 1 × 256 × 64 × 64.
- The decoder is MobileSAM's own `SamOnnxModel`, as its scripts/export_onnx_model.py exports it with
  `--return-single-mask`: its inputs, outputs and number of points as that script names them.

Each file is then run with onnxruntime on fixed inputs and compared with PyTorch's own result, and its size
and SHA-256 are printed. U²-Net-p isn't exported: its file is rembg's conversion of its author's checkpoint,
released as it is (models.toml).
"""

import hashlib
import subprocess
import sys
import tomllib
from pathlib import Path

HERE = Path(__file__).resolve().parent
OPSET = 17


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            digest.update(block)
    return digest.hexdigest()


def checked_checkpoint(checkout: Path) -> Path:
    """The checkpoint in `checkout`, once the commit, its size and its SHA-256 are those models.toml pins."""
    source = tomllib.loads((HERE / "models.toml").read_text(encoding="utf-8"))["source"]["mobilesam"]
    commit = subprocess.run(["git", "-C", str(checkout), "rev-parse", "HEAD"], capture_output=True, text=True, check=True).stdout.strip()
    if commit != source["commit"]:
        sys.exit(f"{checkout} is at {commit}, and models.toml pins {source['commit']}")
    checkpoint = checkout / source["checkpoint"]
    size, digest = checkpoint.stat().st_size, sha256(checkpoint)
    if (size, digest) != (source["size"], source["sha256"]):
        sys.exit(f"{checkpoint} is {size} bytes with SHA-256 {digest}, and models.toml pins {source['size']} and {source['sha256']}")
    print(f"checkpoint {checkpoint}: {size} bytes, SHA-256 {digest}, commit {commit}")
    return checkpoint


def main() -> None:
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    checkout, out = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    checkpoint = checked_checkpoint(checkout)
    out.mkdir(parents=True, exist_ok=True)

    # Imported once the checkpoint is checked, so that a wrong one is quick to tell.
    import numpy as np
    import onnxruntime
    import torch

    sys.path.insert(0, str(checkout))
    from mobile_sam import sam_model_registry
    from mobile_sam.utils.onnx import SamOnnxModel

    print(f"torch {torch.__version__}, onnxruntime {onnxruntime.__version__}, numpy {np.__version__}, Python {sys.version.split()[0]}")
    sam = sam_model_registry["vit_t"](checkpoint=str(checkpoint))
    sam.eval()
    generator = torch.Generator().manual_seed(0)

    # The image encoder alone: the picture as SAM's preprocessing leaves it.
    encoder = out / "mobilesam-encoder.onnx"
    image = torch.randn(1, 3, 1024, 1024, generator=generator)
    with torch.no_grad():
        torch.onnx.export(
            sam.image_encoder,
            (image,),
            str(encoder),
            dynamo=False,
            export_params=True,
            opset_version=OPSET,
            do_constant_folding=True,
            input_names=["input_image"],
            output_names=["image_embeddings"],
        )
        embeddings = sam.image_encoder(image)

    # The prompt encoder and mask decoder, as MobileSAM's own export script makes them with
    # --return-single-mask, with a box (labels 2 and 3) and 2 points as the traced prompt.
    decoder = out / "mobilesam-decoder.onnx"
    model = SamOnnxModel(model=sam, return_single_mask=True)
    inputs = {
        "image_embeddings": embeddings,
        "point_coords": torch.tensor([[[300.0, 200.0], [700.0, 900.0], [500.0, 500.0], [650.0, 820.0]]]),
        "point_labels": torch.tensor([[2.0, 3.0, 1.0, 0.0]]),
        "mask_input": torch.zeros(1, 1, 256, 256),
        "has_mask_input": torch.tensor([0.0]),
        "orig_im_size": torch.tensor([1600.0, 1200.0]),
    }
    with torch.no_grad():
        torch.onnx.export(
            model,
            tuple(inputs.values()),
            str(decoder),
            dynamo=False,
            export_params=True,
            opset_version=OPSET,
            do_constant_folding=True,
            input_names=list(inputs),
            output_names=["masks", "iou_predictions", "low_res_masks"],
            dynamic_axes={"point_coords": {1: "num_points"}, "point_labels": {1: "num_points"}},
        )
        expected = model(**inputs)

    # Each file, run by onnxruntime, against PyTorch.
    options = onnxruntime.SessionOptions()
    options.intra_op_num_threads = 1
    run = lambda path, feed: onnxruntime.InferenceSession(str(path), options, providers=["CPUExecutionProvider"]).run(None, feed)  # noqa: E731
    got = run(encoder, {"input_image": image.numpy()})[0]
    print(f"encoder: largest difference from PyTorch {np.abs(got - embeddings.numpy()).max():.2e}")
    got = run(decoder, {name: value.numpy() for name, value in inputs.items()})
    for name, value, reference in zip(["masks", "iou_predictions", "low_res_masks"], got, expected):
        print(f"decoder {name}: shape {list(value.shape)}, largest difference from PyTorch {np.abs(value - reference.numpy()).max():.2e}")
    for path in (encoder, decoder):
        print(f"{path.name}: {path.stat().st_size} bytes, SHA-256 {sha256(path)}")


if __name__ == "__main__":
    main()
