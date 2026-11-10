# The export record of StitchCraft's cut-out models

<!-- build: release.py puts the release, the run that built it, its tools and the files' sums here -->

## Sources

**MobileSAM.** Its authors' repository, https://github.com/ChaoningZhang/MobileSAM, at commit
`f706ad9c4eb7f219c00d9050e46328518ffb65d2` (2026-05-05). Its checkpoint `weights/mobile_sam.pt` is 40,728,226
bytes, SHA-256 `6dbb90523a35330fedd7f1d3dfc66f995213d81b29a5ca8108dbcdd4e37d6c2f`. Its `LICENSE`, the Apache
License 2.0, is `LICENSE-MobileSAM.txt` here.

**U²-Net-p.** Its author's repository, https://github.com/xuebinqin/U-2-Net at commit `ac7e1c8`, gives the
checkpoint `u2netp.pth` (4.7 MB) on Google Drive (file `1rbSTGKAE-MTxBYHd-51l2hMOQPT_7EPy`) and on Baidu Pan
only, and the repository itself doesn't contain one: its `setup_model_weights.py` fetches others from Google Drive.
The machine that set the pins couldn't reach either host, so the owner chose rembg's conversion. rembg
publishes it as a release asset, and its README (commit `991927b`) lists it as u2netp's download, with
U²-Net as its source. Each build fetches it with curl from
https://github.com/danielgatis/rembg/releases/download/v0.0.0/u2netp.onnx and releases it unchanged. It is
the file StitchCraft's spike M13.13 measured. U²-Net's `LICENSE`, the Apache License 2.0, is
`LICENSE-U-2-Net.txt` here, and rembg's `LICENSE.txt`, the MIT License, is `LICENSE-rembg.txt`.

## The build

The `build` job of StitchCraft's workflow `.github/workflows/models.yml` makes the release's folder on one of
GitHub's runners. It checks MobileSAM's repository out at the commit above, and installs `requirements.txt`
with `--require-hashes` into a virtual environment of CPython 3.13. Then it runs these, where `release` is
an empty folder at first:

```sh
venv/bin/python -I export.py mobilesam release
venv/bin/python -I export.py mobilesam again        # a second export, which must give the same bytes
venv/bin/python -I release.py fetch release         # U²-Net-p's file, with curl
venv/bin/python -I release.py assemble release notes.md
```

`release.py` stops unless each of the 3 files has the size and SHA-256 that StitchCraft's
`apps/stitchcraft-cutout/models/models.toml` pins. Only then does it add the licences, `NOTICE.txt`,
`export.py`, `requirements.txt`, this record and `SHA256SUMS.txt`. The same workflow publishes the folder as
the release when StitchCraft's owner asks it to, and it never replaces a release.

`requirements.txt` pins every package `export.py` runs with, by version and the hashes of its wheels for
CPython 3.13 on Linux x86_64. torch is PyPI's Linux build, which includes CUDA's libraries and computes on the
CPU of a runner without a GPU. The table above names the versions this build ran with.

## What `export.py` does

- It checks the checkout's commit, and the checkpoint's size and SHA-256, against StitchCraft's
  `apps/stitchcraft-cutout/models/models.toml`.
- It builds MobileSAM's `vit_t` model with MobileSAM's own `sam_model_registry`, which loads the checkpoint.
- **The encoder** is `sam.image_encoder`. Its input `input_image` is 1 × 3 × 1024 × 1024, the picture
  scaled so that its longer side is 1,024 px, normalized with SAM's mean and deviation and padded with
  zeros. Its output `image_embeddings` is 1 × 256 × 64 × 64.
- **The decoder** is MobileSAM's `SamOnnxModel` with `return_single_mask`, as MobileSAM's
  `scripts/export_onnx_model.py --return-single-mask` exports it. Its inputs are `image_embeddings`,
  `point_coords`, `point_labels`, `mask_input`, `has_mask_input` and `orig_im_size`, with the number of
  points left open, and its outputs `masks`, `iou_predictions` and `low_res_masks`.
- Both are exported by PyTorch's TorchScript exporter (`dynamo=False`) at opset 17, with constant folding.

## The checks

The pins were set on 2026-11-08 from an export made by hand with this `export.py` and this
`requirements.txt`, on Linux x86_64 (an Intel Xeon at 2.1 GHz, 2 cores) with CPython 3.13.18. These checks
were made on its files. They hold for every build's files, which are the same bytes: a build stops when a
file differs from its pin.

1. **Against PyTorch.** Each file, run by onnxruntime on fixed inputs, against PyTorch's own result: the
   encoder within 9.5e-07, the decoder's `low_res_masks` within 1.3e-05, `iou_predictions` within 1.8e-07
   and `masks` within 4.4e-05. `export.py` makes this check in every build, and prints what it finds.
2. **The number of points.** The decoder picks its mask by the number of points it is given, and does so
   as PyTorch does for 2, 3 and 6 points. A box alone takes the best of the 3 masks after the first, and
   more points the first, each within 1.6e-05.
3. **Reproducible.** A second run of `export.py` wrote both files again byte for byte, and every build
   exports twice to check it.
4. **The photos.** StitchCraft's 2 test photos of the owner's dog, cut out from U²-Net-p's box with
   onnxruntime 1.31.1 on one thread by M13.13's script, and scored against their masks checked by hand:

   | Marks | Turf: IoU | kept wrongly | lost wrongly | Running: IoU | kept wrongly | lost wrongly |
   |---|---:|---:|---:|---:|---:|---:|
   | U²-Net-p's box | 0.985 | 0.4 % | 1.1 % | 0.987 | 0.4 % | 0.9 % |
   | U²-Net-p's box, and a point on the tyre that isn't the subject | | | | 0.988 | 0.5 % | 0.7 % |
   | A box drawn by hand | 0.985 | 0.4 % | 1.1 % | 0.988 | 0.2 % | 0.9 % |

   M13.13 measured 0.985 and 0.988 from U²-Net-p's box, and 0.988 with the point on the tyre. U²-Net-p
   alone keeps the tyre behind the running dog, 19.8 % of the dog's area, and these cut-outs leave it out.
5. **M13.13's files.** M13.13 measured a third-party export of MobileSAM, of unknown origin. Its
   decoder has this one's weights and approximates GELU with tanh. Its encoder has the same
   shapes and other weights, the sum of their magnitudes 1,106,155.6 against the checkpoint's 1,108,688.5,
   and its embedding of the running photo differs from this export's by up to 0.167. That explains the 0.001
   between 0.987 and 0.988.
6. **U²-Net-p** is the file M13.13 measured, by its SHA-256. Run by tract, it gives the boxes M13.13's
   tract program found: 352.5,0,881.25,1190 on turf and 161.25,223.25,742.5,1289.27 running.
7. **tract.** StitchCraft's `stitchcraft-cutout`, with tract 0.23.10 on one thread, runs the 3 files from
   U²-Net-p's box to the cut-out, and its cut-outs score so against the same masks:

   | Marks | Turf: IoU | kept wrongly | lost wrongly | Running: IoU | kept wrongly | lost wrongly |
   |---|---:|---:|---:|---:|---:|---:|
   | U²-Net-p's box | 0.985 | 0.5 % | 1.0 % | 0.987 | 0.4 % | 0.8 % |
   | U²-Net-p's box, and a point on the tyre that isn't the subject | | | | 0.988 | 0.5 % | 0.7 % |
