# Docs fixtures

Small designs for the docs' pictures (`docs/shots.toml`) and command-line examples (`docs/examples.toml`),
in SVG and in VectorCraft's format. `cargo xtask docs` and `cargo xtask shots` plan them with the engine,
so a page always shows what StitchCraft does with them today. Each one is hand-written for StitchCraft and
shows one thing, but for `round-patch.vectorcraft`, which VectorCraft writes, and the pictures VectorCraft
traced: `traced-pear.vectorcraft` ([below](#the-traced-pear)) and the owner's dog, cut out of her photos
([the traced dogs](#the-traced-dogs)). Test inputs live in `conformance/fixtures/`.

| Fixture | Shows | Used by |
|---|---|---|
| `badge.svg` | A red star inside a blue ring | the SVG tutorial: the `badge` shots and the `plan-badge` example; the window probe (`compat/vc-window`) |
| `traced-badge.svg` | A badge as Image Trace traces it in the Brother threads `stitch palette` writes: a red disc, a blue square and a gold disc, abutting, on white, none with settings | the picture tutorial: the `traced-badge` shots and the `plan-traced-badge` example; automatic digitizing's neighbours |
| `emblem.svg` | An emblem drawn in layers on white in 4 of those threads, each layer whole and none with settings. A navy square 50 mm wide holds a white disc with a red star on it and 3 gold bars 3 mm tall | automatic digitizing's flat-colour art: the `auto-emblem-picture` and `auto-emblem` shots |
| `traced-pear.vectorcraft` | The photo of a pear that the `auto-pear-photo` shot draws, traced 90 mm across in 4 of those threads: a light grey backdrop, and the pear in 3 bands of green | automatic digitizing's photo: the `auto-pear-traced` and `auto-pear` shots, and the engine's cases of what a digitized picture leaves bare and lays astray (`stitchcraft_testkit::pictures::pear`) |
| `dog-on-turf-cutout.png` | The owner's photo of her dog on artificial turf, looking up at the camera, cut out of the turf: the dog on a transparent background, 191 × 410 pixels | traced into the 2 documents below |
| `dog-running-cutout.png` | Her photo of the dog running towards the camera, cut out the same way, 183 × 372 pixels | automatic digitizing's dog: the `auto-dog-photo` shot, and traced into the 2 documents below |
| `traced-dog-on-turf-6.vectorcraft` | The dog on turf, traced 100 mm tall in 6 of the machine's threads: black, greys and white | the engine's cases of what a digitized picture leaves bare and lays astray (`stitchcraft_testkit::pictures::dogs`), and its paw alone (`dog_shapes`) |
| `traced-dog-on-turf-9.vectorcraft` | The dog on turf, traced 100 mm tall in 9 threads, a front leg tan | the same cases |
| `traced-dog-running-6.vectorcraft` | The running dog, traced 100 mm tall in 6 threads, its legs tan | the same cases, and 4 shapes of its ear alone (`dog_shapes`) |
| `traced-dog-running-9.vectorcraft` | The running dog, traced 100 mm tall in 9 threads, its mouth dark brown | the `auto-dog-traced` and `auto-dog` shots, and the same cases |
| `wave.svg` | A smooth wave | running stitch length |
| `zigzag.svg` | The same wave, drawn 3 mm wide | the zigzag stroke's spacing and angle, and a satin column drawn as one path |
| `ripple.svg` | An arch, half a circle 30 mm across, open at the bottom | the ripple stitch's lines and joins |
| `ripple-guide.svg` | A wave 12 mm long, grouped with an arch that Ink/Stitch's guide-line marker makes a guide | the ripple stitch's guide lines |
| `ripple-satin-guide.svg` | A zigzag, grouped with a satin guide: a curved band 12 mm wide between 2 rails, with 3 rungs | the ripple stitch's satin guides |
| `e-stitch.svg` | A curved band 4 mm wide between 2 rails | the E-stitch's spacing and swapped rails |
| `s-stitch.svg` | A curved band 6 mm wide between 2 rails | the S-stitch's spacing and split stitches, the zigzag satin's spacing and its look beside a satin column, and the satin column's spacing and underlays |
| `crossing.svg` | 2 bands 5 mm wide and 16 mm long between 2 rails each, a bar and a post that cross at their middles | satin columns sewn as drawn and with their underlays first |
| `half-disc.svg` | A half disc: a straight side with 2 corners, and a curve | running stitch tolerance |
| `parallel.svg` | 14 rows 0.6 mm apart, sewn as one path | random stitch length |
| `nodes.svg` | Straight lines and one curve | manual stitch |
| `lock.svg` | An 8 mm line | lock shapes |
| `wide.svg` | A 210 mm border, wider than the reference hoop | the `plan-too-wide` example |
| `fill.svg` | A patch 32 mm by 20 mm with rounded corners and a round hole | the tatami fill's angle, staggers and underlay; the contour fill's path; the linear gradient fill without a gradient |
| `star.svg` | A star, its points 15 mm from its middle and its inner corners 6 mm | the contour fill's strategies and corners |
| `heart.svg` | A heart 32 mm wide and 29 mm tall | cross stitch: its grid, coverage and threads |
| `meander.svg` | A square 30 mm a side with rounded corners | the meander fill's tiles, scale, angle and line, and the circular fill's spacing and travel |
| `tartan.svg` | A square 30 mm a side | the tartan fill's setts, runs and turned setts |
| `gradient.svg` | 3 squares 30 mm a side under linear gradients. The first is red to blue across, and the second deep gold through white to emerald green up to the right. The third is tangerine to Prussian blue down through a transparent white | the linear gradient fill's stops and row spacing |
| `widths.svg` | A line 0.6 mm wide, a letter C 3 mm wide and a disc 16 mm across, each traced as an area | automatic digitizing's stitches by width |
| `parts.svg` | A lollipop on a stick 2.4 mm wide and a balloon on a string 0.8 mm wide, each one area | automatic digitizing's shapes in parts |
| `round-patch.vectorcraft` | The VectorCraft tutorial's patch, as VectorCraft saves the `vectorcraft-patch` shot's steps, which are the tutorial's (`cargo xtask shots --vectorcraft` writes it). A disc and a star have StitchCraft Fill and Stroke, and a border of 2 rails has StitchCraft Satin | the VectorCraft tutorial: the `vectorcraft-patch-sewn` shot and the `export-round-patch` example |

## The traced pear

`traced-pear.vectorcraft` was written by VectorCraft 0.19.0's command line (MIT OR Apache-2.0, © 2026
ArtCraft Team and the VectorCraft contributors), from its Linux release archive. `stitch plan` ran it to
trace the photo that `cargo xtask shots` draws in code (`xtask/src/shots/picture.rs`), with the steps of
[a picture from the command line](../src/design/image-to-embroidery.md#pictures-from-the-command-line):

```sh
cargo xtask shots
stitch plan docs/src/images/generated/auto-pear-photo.png -o pear.pes --colours 4 --size 90 \
  --traced docs/fixtures/traced-pear.vectorcraft --vectorcraft-cli PATH/TO/vectorcraft-cli
```

The document is kept as VectorCraft saved it. The shots plan it without VectorCraft. A test in
`xtask/src/shots/picture.rs` keeps the hash of the photo it was traced from. When the photo changes, the test
fails until the new photo is traced the same way and the test keeps its hash.

## The traced dogs

The 2 photos of a dog are Louise Fox's, of her own dog. She gave them to StitchCraft on 2026-11-08 under the
MIT licence. Before they reached the repository, each was scaled down from 1536 × 2048 pixels to 1200 pixels
wide and saved as a JPEG file without metadata. The photo of the dog running first had its left 160 pixels cut
away, as they held a person. Neither photo is in the repository: only the cut-outs made from them are.

**The masks.** A mask is 255 where a photo shows the dog, made for each photo by spike M13.13. A pixel is the
dog's where at least 2 of the masks of 3 models keep it: SAM ViT-B and MobileSAM, each given a box drawn round
the dog by hand, and IS-Net. The holes were then filled, and the pieces of fewer than 2,000 pixels apart from
the dog were dropped. Each mask was checked by eye, region by region, in crops at three quarters of the
photo's size, and needed no correction. They keep a little of what shows through the fur at its edges, such
as the yellow line through the tip of the tail on turf.

**The cut-outs.** `dog-on-turf-cutout.png` and `dog-running-cutout.png` are the photos with their masks as
alpha. Each is cropped to its mask's box with a margin of 8 pixels, as far as the photo reaches, and scaled
down to a third of the photo's size: each pixel averages a block of 3 × 3 pixels of the photo, over the dog's
pixels only, so that the turf colours none of the dog's edge. A pixel is opaque where the dog fills at least
half of its block, and transparent elsewhere, where it keeps nothing of the photo. At 100 mm tall a pixel is
then 0.24 mm, finer than a thread 0.4 mm wide. At a quarter of the photo's size, the dogs traced in 4 threads
lost their eyes. Removing the background before VectorCraft traces a photo is what `--subject` will do
(M13.14), and until then the cut-outs stand in for it.

**The traces.** VectorCraft 0.22.0's command line (MIT OR Apache-2.0, © 2026 ArtCraft Team and the
VectorCraft contributors), from the Linux release archive whose SHA-256 `compat/vectorcraft.toml` pins,
wrote the 4 documents. `stitch plan` ran it to trace each cut-out 100 mm tall in 6 and in 9 threads, with
the steps of [a picture from the command line](../src/design/image-to-embroidery.md#pictures-from-the-command-line):

```sh
stitch plan docs/fixtures/dog-running-cutout.png -o dog.pes --colours 9 --size 100 \
  --traced docs/fixtures/traced-dog-running-9.vectorcraft --vectorcraft-cli PATH/TO/vectorcraft-cli
```

Image Trace leaves the transparent pixels out, so every shape of a document is the dog's: the area the
shapes cover differs from the cut-out's dog by 0.2 % of it. In 6 threads both dogs keep a middle grey where
their fur is shaded, which fewer threads lose, and the running dog has its tan legs. 9 threads are the fewest
that give the dog on turf tan legs too, which 8 trace in warm grey. VectorCraft saves a document whose
indented JSON would be larger than 1 MiB without indenting it, so each of these 4 is 236 to 265 KB. In 4 or
5 threads they would stay under that size, indented, and be 0.87 to 1 MB. The documents are kept as
VectorCraft saved them, and the shots and cases plan them without VectorCraft. A test in
`xtask/src/shots/picture.rs` keeps the hash of each cut-out. When a cut-out changes, the test fails until it
is traced again the same way and the test keeps its new hash.
