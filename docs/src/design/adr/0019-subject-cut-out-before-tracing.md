# ADR-0019: A photo's subject is cut out by `stitch` before VectorCraft traces it

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Proposed · 2026-11-08, spike M13.13
<!-- vale ai-tells.ColonUsage = YES -->

## Context

A picture traced in VectorCraft sews well when it is a few flat colours on a plain background
([image to embroidery](../image-to-embroidery.md)). A photo has a background of its own. Behind the owner's
dog lie turf, chair legs, a fence, a shed and 2 tractor tyres, and Image Trace traces each of them into
shapes. VectorCraft 0.21.0's command line traced the 2 photos below with the `trace` case's steps, in the
reference machine's 61 threads and 6 colours, into 7,390 paths on turf and 5,601 paths running. The
background rule (`REQ-AUTO-009`) leaves out one shape at most, the lowest, when it covers the whole picture,
and the rest is sewn. Image Trace leaves out the pixels whose alpha is under 128 (VectorCraft's
`crates/trace`, `main` at `3f8d7a0`), so a photo whose background is transparent traces as its subject
alone. The cut-outs below, cropped to the dog, traced into 206 paths and 207 paths. On 2026-11-08 the owner
asked for the cut-out in StitchCraft, as an option off by default, in the flow that already sews pictures
([ADR-0015](0015-pictures-traced-by-vectorcraft.md)). Spike M13.13 chooses where the cut-out runs, the
model, the runtime, how the model is fetched, how a user corrects a cut-out, how replays and CI do without
the model, and how a cut-out is measured. M13.14 builds what it chooses.

A prototype in Python came first. On the photo of the dog running, IS-Net found the dog and scored the 2
tyres behind it at 0.99 as part of it. OpenCV's GrabCut, told that 2 boxes over the tyres were background,
cut them away and left straight edges on the fur beside them. U²-Net-p kept a person and a shed. A cut-out
made without help is sometimes wrong with confidence, so the user must be able to correct it.

What was measured, on 2026-11-08:

- **The photos** are the owner's 2 photos of her dog, which she gave the project under the MIT licence, as
  prepared for test pictures, 1,200 px wide. In one the dog stands on turf beside chair legs and a yellow
  line. In the other it runs towards the camera with the tyres, a fence and a shed behind it, and the
  photo's left 160 px, where a person stood, are cut away.
- **The reference masks** keep the pixels that at least 2 of 3 masks keep. The 3 are SAM ViT-B and
  MobileSAM, each given a box drawn round the dog by hand, and IS-Net. Each reference was then checked by
  eye, region by region in crops at three quarters of the photo's size, the ears, the tail, the legs and
  the paws among them, and the edge beside the near tyre. Neither had to be corrected. The dog covers 12.9 %
  of the first photo and 11.3 % of the second. Where fur thins out the masks follow those 3 models, so a
  difference of 1 % or so lies within their error.
- **The measures** are a cut-out's intersection over union with the reference mask (IoU), and the share of
  the dog's area it keeps wrongly and the share it loses wrongly. Each mask counts where it is at least a
  half.
- **The computer** is an Intel Xeon at 2.1 GHz with AVX-512 and 2 cores, which the
  [technical design](../tdd.md) measures on, shared with other builds. Times are CPU time on one thread,
  which the other builds change less than the wall clock. onnxruntime 1.29.0 ran every model from Python,
  and tract 0.23.10 ran the chosen ones from Rust. Peak memory is the whole process's, with the photo
  decoded and resized.

Each model, run by onnxruntime:

| Model | Its weights' licence | Size | Multiply-adds | Time | Peak memory | IoU on turf | IoU running | Kept wrongly, running |
|---|---|---:|---:|---:|---:|---:|---:|---:|
| U²-Net-p | Apache-2.0, as its author states | 4.6 MB | 19.8 G | 0.46 to 0.56 s | 329 MB | 0.929 | 0.833 | 19.8 % |
| U²-Net | Apache-2.0, as its author states | 176.0 MB | 58.6 G | 1.12 to 1.13 s | 616 MB | 0.943 | 0.829 | 19.9 % |
| IS-Net, general use | none stated | 178.6 MB | 159.7 G | 3.09 to 3.11 s | 1,008 MB | 0.957 | 0.845 | 18.1 % |
| BiRefNet lite | MIT | 224.0 MB | 264 G or more | | out of memory at 5.9 GB | | | |
| SAM ViT-B, with U²-Net-p's box | Apache-2.0 | 375.7 MB | about 500 G | 13.7 to 14.0 s | 3,645 MB | 0.988 | 0.986 | 0.2 % |
| MobileSAM, with U²-Net-p's box | Apache-2.0 | 44.5 MB | 38.8 G | 1.57 to 1.64 s | 459 MB | 0.985 | 0.988 | 0.3 % |

U²-Net and U²-Net-p take the photo at 320 × 320 px, IS-Net and BiRefNet at 1,024 × 1,024 px, and the SAM
models at 1,024 px on its longer side. The
multiply-adds are those of the convolutions and the matrix products, counted from the shapes onnxruntime
gives their tensors. SAM ViT-B's are worked out from its layers, and BiRefNet lite's leave out the products
whose shapes are known only once it runs. A SAM model is an encoder, run once for each photo, and a decoder,
run for each prompt in 0.1 s. Its time is the encoder's, and the box adds U²-Net-p's time. U²-Net-p, U²-Net
and IS-Net each kept the near tyre with the dog. MobileSAM and SAM ViT-B cut the dog alone out of the box
U²-Net-p found round both.

How a user's marks change MobileSAM's cut-out of the running photo, against GrabCut:

| Marks | IoU | Kept wrongly | Lost wrongly |
|---|---:|---:|---:|
| U²-Net-p's box | 0.988 | 0.3 % | 0.9 % |
| U²-Net-p's box, and a point on the near tyre that isn't the subject, run by tract | 0.988 | 0.5 % | 0.7 % |
| A box drawn round the dog | 0.989 | 0.2 % | 0.9 % |
| That box, and a point on the near tyre that isn't the subject | 0.988 | 0.3 % | 0.8 % |
| A point on the dog | 0.985 | 0.8 % | 0.8 % |
| A point on the dog, and a point on the near tyre that isn't the subject | 0.664 | 0.0 % | 33.6 % |
| The middle of the photo as a point on the subject | 0.000 | 460.9 % | 100 % |
| The whole photo as the box | 0.000 | 460.4 % | 100 % |
| GrabCut (OpenCV) from IS-Net's map, boxes over both tyres as background | 0.918 | 2.5 % | 5.9 % |

The middle of the photo lies on the turf beside the dog's leg, and MobileSAM cut out the turf there, as it
did given the whole photo as the box. On the turf photo the whole photo as the box gave the turf too.
GrabCut from IS-Net's map lost the leg on the photo's left in each photo, at an IoU of 0.879 on the turf.

tract ran the chosen models in a Rust program of its own on the same 2 photos, from the JPEG file to the
PNG file of the cut-out, with one thread:

| Step | tract | onnxruntime, on the same input |
|---|---:|---|
| Decoding the photo | 0.02 s | |
| Loading and optimising the 3 models | 0.9 s | |
| U²-Net-p | 1.0 to 1.2 s | 0.46 to 0.56 s, its map within 0.0001 of tract's |
| MobileSAM's encoder | 2.5 to 2.9 s | 1.57 to 1.64 s, its output within 0.000003 of tract's |
| MobileSAM's decoder, for each prompt | 0.15 to 0.23 s | its 256 × 256 values within 0.00006 of tract's, each of the same sign |
| Scaling the map and writing the PNG files | 0.4 to 0.5 s | |
| The whole cut-out | 5.1 to 5.4 s, at most 343 MB | 2.2 s for the 3 models |

tract's cut-outs kept the dog at an IoU of 0.985 and 0.988, as onnxruntime's did. Its masks differ from
onnxruntime's in 343 and 235 pixels, 0.14 % and 0.10 % of the dog's area, most of them from the 2 programs'
own code that scales the map to the photo. The program, stripped, is 24.8 MB. tract's and the `image`
crate's 131 crates, each under a licence of `deny.toml`'s allow-list, took 13 minutes to compile with one
job on the 2.1 GHz Xeon. tract-onnx brings `rand`, `rand_distr` and `getrandom` with it, for ONNX's
random operators, and `deny.toml` admits `rand` only under proptest ([below](#decision)). None of the 3
models has a random operator.

Licences were read from each project's own repository on 2026-11-08 (references below). The weights count,
as well as the code:

- **MobileSAM.** Its repository is under the Apache-2.0 licence, and its weights, `weights/mobile_sam.pt`,
  are part of it. MobileSAM was distilled from SAM.
- **SAM and SAM 2.** Meta's repositories say the models and their checkpoints are under the Apache-2.0
  licence.
- **U²-Net and U²-Net-p.** The repository is under the Apache-2.0 licence and links the weights from its
  README. Its author wrote in issue 208, in May 2021, that "all the released codes and models in this repo
  follow the same license currently", and asked users to "check the datasets licenses, which are not
  controlled by us". The models were trained on DUTS.
- **IS-Net, general use.** The DIS repository says its code and evaluation metric are under the Apache-2.0
  licence, and states no licence for its weights. Its dataset, DIS5K, is for non-commercial research and
  education, and its terms forbid commercial use "even after copying, editing, processing or any
  operations of this database".
- **BiRefNet**'s repository is under the MIT licence. **RMBG-2.0**, BiRefNet trained on BRIA's own data,
  needs BRIA's paid agreement for commercial use. **EdgeSAM** is under the S-Lab licence, for non-commercial
  use. **FastSAM**'s repository has the AGPL-3.0 as its licence file, and its README names Apache-2.0.

## Decision

1. **An option of the picture step.** `stitch plan` and `stitch export` take `--subject` for a picture, off
   by default. Before VectorCraft's command line traces the picture, `stitch` makes the pixels that aren't
   the subject transparent, and crops the picture to the box round what it keeps. `file.place` places that
   PNG file where it placed the picture, so `--size` and the hoop fit the subject. `--cutout FILE` writes
   the cut-out beside the machine file, with the other files, all or none. `stitch cutout PICTURE -o FILE`
   writes the cut-out alone, with the same marks.
2. **No plug-in.** A VectorCraft filter can't read or replace a placed image. VectorCraft gives a filter the
   selected paths and compound paths, and lists filters on images among what plug-ins don't do in ABI v1
   (`docs/plugins.md` at `3f8d7a0`). In VectorCraft's own flow the user makes the cut-out with
   `stitch cutout` and places it instead of the photo, then traces it with Image Trace. Under M9,
   StitchCraft would propose P10, which would give a filter each selected placed image as PNG data and let
   it return a new image for each. P10 isn't raised, as a plug-in still couldn't run the models. A module may be 32 MiB,
   and the models are 49.1 MB. A heavy filter gets 1 G instructions, and MobileSAM's encoder alone does
   38.8 G multiply-adds.
3. **The models.** MobileSAM cuts the subject out of a box. Its encoder takes the photo once, at 1,024 px on
   its longer side, and its decoder takes the box and any points. U²-Net-p finds the box when the user gives
   none, round the piece of its map above a half with the most of the map in it. Both are under the
   Apache-2.0 licence, and NOTICE names them.
4. **The runtime.** tract runs both, from their ONNX files, in the app layer, with one thread. A crate of its
   own beside the command line, `stitchcraft-cutout`, does the cut-out, and only `stitch` depends on it.
   The `image` crate decodes the photo and encodes the cut-out there. The library crates of layers L0 to L3
   don't run a model or read a picture. `deny.toml` lets `rand` and `getrandom` in through `tract-onnx-opl`
   alone, as no model of the cut-out has a random operator. `stitchcraft-cutout` refuses a model that has
   one, so that no cut-out calls them, and StitchCraft asks tract to put its random operators behind a
   feature.
5. **Getting the models.** The 3 files, U²-Net-p, MobileSAM's encoder and its decoder, are never committed.
   The repository pins each one's address, size and SHA-256. A script kept beside the pins exports them once
   from the authors' checkpoints, and a StitchCraft release publishes them as assets with their licences. On
   first use `stitch` fetches them with curl, as `cargo xtask corpus` fetches the fonts, into the folder
   `STITCHCRAFT_MODELS` names, or else `stitchcraft/models` in the user's cache folder. It checks each file's
   SHA-256 before each use. A file it can't fetch, or whose sum differs, is a coded error. The error names
   each file's address, size and SHA-256 and the folder to put it in, and says that the picture plans as
   before without `--subject`. `stitch` then writes the report alone.
6. **Correcting a cut-out.** `--subject-box X0,Y0,X1,Y1` gives the box instead of U²-Net-p.
   `--subject-point X,Y` marks a point of the subject and `--not-subject X,Y` a point that isn't, as often
   as needed. Each is in the picture's pixels, as its file stores them. Points go to the decoder with the
   box, U²-Net-p's when the user gives none. Without `--subject`, each of these options is a usage error.
7. **Determinism and replay.** The models run in `stitch`, never in a library crate, and their results may
   differ in their last bits from one processor to another. So a plan is reproducible from the traced
   document, as it is for any picture, and not from the photo. A bug-report bundle of a picture contains
   the traced document. With `--subject` it also records the models' SHA-256 sums and the marks, and leaves
   the picture out, as every bundle does. A replay runs without the models and without VectorCraft.
8. **Measuring.** Cases measure a cut-out by its IoU with a mask checked by hand and the shares of the
   subject's area it keeps and loses wrongly. M13.14 commits the 2 photos, as prepared, with their masks,
   and records them in `conformance/fixtures/README.md` (Louise Fox, MIT licence). The models' outputs for
   them are fixtures too, so that the cases of the steps after the models run without them. Pictures drawn
   in code, whose masks are exact, test the marks and those steps. The models themselves are checked by a
   job of their own, on the photos and the drawn pictures, against the fixtures and the IoU bounds.

## Consequences

- Off by default, the option changes nothing for a design that doesn't ask for it, and no golden file
  changes.
- `stitch` gains tract's and the `image` crate's dependencies, 131 crates, each under a licence of
  `deny.toml`'s allow-list, and `cargo-deny` checks them as it checks every crate. The models are no crate,
  and the pins and NOTICE record their licences.
- ADR-0015 noted that StitchCraft's crates don't read PNG or JPEG files. `stitchcraft-cutout` reads them
  now. StitchCraft still doesn't trace, and VectorCraft traces the cut-out.
- With the box found by U²-Net-p, a cut-out takes 5.1 to 5.4 s of CPU time on the 2.1 GHz Xeon with
  tract, and 343 MB at most. The models alone take 3.8 to 4.0 s there with tract and 2.2 s with
  onnxruntime. Its first use downloads 49.1 MB.
- `deny.toml` admits `rand` under proptest alone, and `getrandom` under the crates that wrap it. It gains
  `tract-onnx-opl` as a wrapper of both, which the owner accepts with this ADR. The wrapper goes once
  tract's random operators sit behind a feature.
- A cut-out can still be wrong. The user sees it in the preview, or in the file `--cutout` writes, and
  corrects it with a box and points. Points without a box can lose a third of the subject, as the table
  shows. The decoder always gets a box.
- A picture whose background is transparent already, cut out by any program, traces as its subject
  without `--subject`, since Image Trace leaves its transparent pixels out.
- `stitch` is built for WASI to make the golden files. tract's own CI checks tract-onnx for `wasm32-wasip1`,
  and runs tract-core's and tract-linalg's tests there in wasmtime. M13.14 keeps the build of `stitch`
  working. With `--subject` there, `stitch` can't start curl
  to fetch the models, and says so with the coded error.
- An owner action publishes the release with the models, and M13.14's cases fetch them from it.
- Revisit this if VectorCraft gives plug-ins images and a budget the models fit, or if it cuts a subject
  out itself. Revisit the models when a smaller promptable model under a licence of the list cuts the
  photos out better, such as EfficientSAM or SAM 2.1's smallest. Revisit the runtime if tract can't run
  a model's ONNX export.

## Alternatives rejected

- **A foreground model alone,** U²-Net-p, U²-Net or IS-Net, which a user doesn't mark. Each kept the near
  tyre with the dog, 18 % to 20 % of the dog's area kept wrongly, and none takes a mark that would leave it
  out.
- **IS-Net, general use.** No licence is stated for its weights, and they were trained on DIS5K, whose terms
  forbid commercial use of anything processed from it. It is also 178.6 MB, and its box gave MobileSAM the
  same cut-outs as U²-Net-p's.
- **BiRefNet** (MIT). Its lite version, 224.0 MB, ran out of memory at 5.9 GB on the 2.1 GHz Xeon at
  1,024 × 1,024 px, and its full version is 972.7 MB.
- **SAM ViT-B.** It cut the photos out as well as MobileSAM did, in 9 times the time, with 8 times the
  memory and 8 times the size.
- **RMBG, EdgeSAM and FastSAM,** for their licences.
- **GrabCut written in Rust,** told by boxes what isn't the subject. Measured with OpenCV's, it lost a leg
  on each photo, and it left straight edges where a box met the fur. It would also need a graph cut and
  colour models of StitchCraft's own. MobileSAM takes the same marks and did better.
- **Image Trace's Ignore White,** with a colour to ignore in place of white (`imageTrace.make`'s
  `ignoreColor`). It leaves out the areas of one colour, and a photo's background has many, such as the
  turf's greens and the fence's browns.
- **A shape drawn in VectorCraft.** `stitch` doesn't read a picture from a VectorCraft document, and a filter
  can't change one (decision 2). `stitch export` already cuts a traced group to a clipping path round it
  (`REQ-VC-017`). Between the path and the subject the background still sews. A clip trims a trace, and a
  cut-out leaves the background out.
- **onnxruntime through the `ort` crate.** It ran the models in 2.2 s, where tract took 3.8 to 4.0 s. It
  is a C++ library, which `ort` downloads as a built binary when it compiles. `cargo-deny` can't check that
  binary, and the WASI build of `stitch` can't take it.
- **candle,** a pure Rust runtime under the MIT or Apache-2.0 licence, whose models include MobileSAM's
  encoder. U²-Net-p would have to be written in candle's terms. The weights would be converted to its
  format, and the 2 copies of each model kept in step. tract runs the ONNX files as they are.
- **burn,** which writes Rust code from an ONNX file. The code would be generated for each model and kept
  in the repository, and whether it covers these models' operators wasn't checked.
- **The models committed to the repository.** They are 49.1 MB, more than every other file of the
  repository together, and binary files in a repository are a cost to every clone.
- **Fetching the models in Rust.** `stitch` would gain an HTTPS client and a TLS library for 3 downloads.
  curl is part of macOS, of Windows from Windows 10 and of most Linux systems, and `cargo xtask corpus`
  fetches with it already.

## References

- VectorCraft at `3f8d7a0`: `crates/trace/src/lib.rs` and `crates/trace/src/quantize.rs` (transparent pixels
  and cut edges), `docs/plugins.md` (Input, Limits, Not in v1).
- [MobileSAM](https://github.com/ChaoningZhang/MobileSAM), its `LICENSE` and `weights/mobile_sam.pt`.
- [Segment Anything](https://github.com/facebookresearch/segment-anything) and
  [SAM 2](https://github.com/facebookresearch/sam2), their README's License sections.
- [U²-Net](https://github.com/xuebinqin/U-2-Net), its `LICENSE`, and its author's answer in
  [issue 208](https://github.com/xuebinqin/U-2-Net/issues/208).
- [DIS](https://github.com/xuebinqin/DIS), its README's License section and `DIS5K-Dataset-Terms-of-Use.pdf`.
- [BiRefNet](https://github.com/ZhengPeng7/BiRefNet), [EdgeSAM](https://github.com/chongzhou96/EdgeSAM) and
  [FastSAM](https://github.com/CASIA-IVA-Lab/FastSAM), their `LICENSE` files, and
  [rembg](https://github.com/danielgatis/rembg)'s README on RMBG-2.0.
- [tract](https://github.com/sonos/tract), its README, `.github/workflows/cross-platform.yml` and `ci/cross.sh`.
- The [subject cut-out](../image-to-embroidery.md#subject-cut-out) section of the design.
