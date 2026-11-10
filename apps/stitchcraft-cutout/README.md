# stitchcraft-cutout

This crate is the subject cut-out of `stitch plan --subject` and `stitch cutout`. It cuts a photo's subject out
before VectorCraft's command line traces it, and the background is left unsewn ([ADR-0019](../../docs/src/design/adr/0019-subject-cut-out-before-tracing.md),
[image to embroidery](../../docs/src/design/image-to-embroidery.md#subject-cut-out)).

## Steps

1. It decodes the picture, a PNG or JPEG file, as its file stores the pixels (`picture`).
2. It finds the subject's box, the one `--subject-box` gives or else the one round the piece of U²-Net-p's
   map above a half with the most of the map in it (`salient`).
3. MobileSAM's encoder takes the picture, and its decoder the box and the points the user marks
   (`segment`, `marks`). Its map is each pixel's alpha.
4. The picture with that alpha, cropped to the box round the pixels of an alpha of 128 or more, is written
   as a PNG file (`cut`).

tract runs the 3 models from their ONNX files, on one thread (`run`). [`cut_out`](src/lib.rs) does every
step but the runs, which it asks of a `Models`. Some tests give it the models' outputs
for the test photos, saved as fixtures. Others give it stand-ins, tiny models with the real ones' inputs and
outputs.

## Invariants

- **Only `stitch` depends on it.** It is a library of the apps' layer (`Class::AppLibrary` in
  `xtask/src/layers.rs`): the library crates of layers L0 to L3 don't run models or read pictures, and
  `image` and `tract-onnx` are allowed here alone. It isn't built for the web.
- **Never crashing.** Every loop over the picture charges the budget (`work`), and every failure is a coded
  diagnostic, `SC-E1201` to `SC-E1205`, or `SC-E0004` when the budget runs out. A cut-out made says what it
  kept, `SC-I1206`.
- **Random operators refused.** A model with a random operator is refused before tract reads it (`SC-E1205`), so that
  `deny.toml`'s exception for `tract-onnx-opl`'s `rand` and `getrandom` stays unused.
- **Checked models.** A model is read whole and its size and SHA-256 compared with its pin before tract reads
  the same bytes (`pins`).
- **Files and processes in one module.** Only `pins` reads the environment, files and starts curl. The steps
  touch none.

## The models

The 3 files are pinned in [`models/models.toml`](models/models.toml) by address, size and SHA-256, and
StitchCraft's release `cutout-models-1` publishes them with their licences, a notice and the export record.
None is committed. `stitch` fetches them with curl on first use into the folder `STITCHCRAFT_MODELS` names,
or else `stitchcraft/models` in the user's cache folder, and checks each before each use.

| File | Model | Made |
|---|---|---|
| `u2netp.onnx` | U²-Net-p (Apache-2.0) | rembg's ONNX conversion of its author's checkpoint, released as rembg publishes it |
| `mobilesam-encoder.onnx` | MobileSAM's image encoder (Apache-2.0) | exported from its authors' checkpoint by [`models/export.py`](models/export.py) |
| `mobilesam-decoder.onnx` | MobileSAM's prompt encoder and mask decoder (Apache-2.0) | the same |

U²-Net's author publishes its checkpoint on Google Drive and Baidu only, so its file is rembg's, as
`models.toml` says.

**GitHub builds the release, and the owner publishes it** (`.github/workflows/models.yml`):

- The `build` job checks MobileSAM's repository out at the commit `models.toml` pins, and installs
  [`models/requirements.txt`](models/requirements.txt) with `--require-hashes` on CPython 3.13. It runs
  [`models/export.py`](models/export.py) twice, and the 2 exports must agree. `export.py` checks the
  checkout and the checkpoint against `models.toml`, and exports both files with PyTorch's TorchScript
  exporter.
- [`models/release.py`](models/release.py) fetches U²-Net-p's file as rembg publishes it, and stops unless
  each model has its pinned size and SHA-256. Then it assembles the release's folder, the run's artifact:
  the models, the licences and `NOTICE.txt` of `models/release/`, `export.py`, `requirements.txt`,
  `SHA256SUMS.txt`, and the export record, `models/release/EXPORT.md` with the run, its tools and the sums.
- The `models` job runs the cases that need the models. It takes them from the release when that exists,
  and from the `build` job until then.
- The `publish` job creates the release from that folder. It runs only when the workflow is started by hand
  on `main` with the release's tag typed, and refuses when the release or its tag exists. A pull request
  builds and checks, and publishes nothing.

A new set of models takes a new tag. Change the addresses and the pins in `models.toml` together, then the
texts of `models/release/`, which `release.py` checks against the pins.

## Features

`models`, on by default, is the cut-out itself: `image`, tract and the steps that use them. Without it the
crate is the marks, the pins and the budget, which `stitch`'s grammar and bundles read, so that `stitch`
builds without the cut-out (its `cutout` feature), as the WASI build that makes the golden files does.

## Tests

- `src/**`: each step's cases, and the runner on the stand-ins. The pins' cases fetch with curl from the disk.
- `tests/photos.rs`: the steps on the owner's 2 photos of her dog, from the models' outputs saved as
  fixtures. Masks checked by hand score them (`REQ-CLI-019`).
- `tests/models.rs`: the models themselves. Its cases are ignored as "models only" outside the models' job
  (`.github/workflows/models.yml`), which takes the models from the release or builds them. One picture is drawn in code, a red disc on grey
  stripes that touches a blue square, whose masks are exact.

The fixtures are in `conformance/fixtures/cutout/`, each with its row in that folder's index.

## Dependencies

| Crate | Why |
|---|---|
| `stitchcraft-core` | diagnostics, the budget, deterministic maths |
| `image` (jpeg, png) | decoding the picture and resizing it for the models, and encoding the cut-out |
| `tract-onnx` (no default features) | running the models from their ONNX files |
| `toml`, `serde` | reading the pins |
| `sha2` | checking each model against its pin |
