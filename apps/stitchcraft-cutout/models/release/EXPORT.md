# The export record of StitchCraft's cut-out models

<!-- build: release.py puts the release, the run that built it, its tools and the files' sums here -->

## Sources

**MobileSAM.** Its authors' repository, https://github.com/ChaoningZhang/MobileSAM, at commit
`f706ad9c4eb7f219c00d9050e46328518ffb65d2` (2026-05-05). The checkpoint `weights/mobile_sam.pt` is 40,728,226
bytes, SHA-256 `6dbb90523a35330fedd7f1d3dfc66f995213d81b29a5ca8108dbcdd4e37d6c2f`. MobileSAM's `LICENSE`, the
Apache License 2.0, is `LICENSE-MobileSAM.txt` here.

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

The decoder's export has the same bytes on every machine that has made it. The encoder's export is
27,969,741 bytes on each of them, and 2 SHA-256 sums have been seen:

| SHA-256 | Where | When |
|---|---|---|
| `047b16d8c829fb9f313acb9943bafd4d9f98b7fbd8f3d19149f221cfeefe0327`, the pin | The first export, made by hand. GitHub's runner image `ubuntu-24.04` at version 20261111.127.1, in 4 runs | 2026-11-08 and 2026-11-15 |
| `71268b41f17ab47538384f408a7e1d25c7f7451f4e8f5cba31b34a047aeff7ff` | GitHub's runner image `ubuntu-24.04` at version 20261101.360.1, in 1 run, and at version 20261111.370.1, in 1 run on `main` at commit `ecdaaeb9` | 2026-11-10 and 2026-11-15 |

Each run gave its sum twice. The cause isn't known. On 2026-11-15 the images 20261111.127.1 and 20261111.370.1
gave different sums, and the runs so far didn't record their processors. The build job
prints the processor from now on, and the table above names the image and the processor of this build. The
outputs saved in `conformance/fixtures/cutout/` came from the first row's export, and its sum is the pin.
Those outputs haven't been compared with the second row's export in any run so far.

A build can't count on either sum, and the encoder's sum doesn't stop it. The build checks the size of
each file and the sums of the decoder and of U²-Net-p, and stops when one differs. When the encoder alone has
another sum, the build warns with the sum, the image and the processor, and leaves the folder unassembled.
The models' job then runs tract on that encoder and compares its outputs with the saved ones, each map
within 0.001 and each logit within 0.01. `stitch` itself runs the pinned file alone. For such an encoder that job leaves
`stitch plan --subject` end to end out, and warns that it did.

A release contains pinned files alone. The owner publishes with these steps:

1. Start the workflow `models` on `main` and type the release's tag in `tag`.
2. When the run's encoder has the pinned sum, the run publishes the release.
3. When it has another sum, the run refuses, publishes nothing, and says what to type. Find a finished run
   of the workflow whose build job's summary is headed "The cut-out's models, each as its pin says".
4. Start the workflow again with the tag and that run's id in `encoder_run`. The id is the number in the
   run's address.

The build then takes the encoder from that run's artifact and checks its size and sum against the pin. It
refuses when the artifact is absent or its encoder isn't the pinned file. The record above then names the
run the encoder came from. A run keeps its artifact for 14 days, and can be named until then. Once the release exists its files are fixed, and the pin stays the sum of the released file.

The first export was made by hand on 2026-11-08 with this `export.py` and this `requirements.txt`, on Linux
x86_64 (an Intel Xeon at 2.1 GHz, 2 cores) with CPython 3.13.18. The checks below were made on its files.
Every build makes checks 1 and 3 again on its own files.

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
