# Docs fixtures

Small designs for the docs' pictures (`docs/shots.toml`) and command-line examples (`docs/examples.toml`),
in SVG and in VectorCraft's format. `cargo xtask docs` and `cargo xtask shots` plan them with the engine,
so a page always shows what StitchCraft does with them today. Each one is hand-written for StitchCraft and
shows one thing, but for `round-patch.vectorcraft`, which VectorCraft writes, and `traced-pear.vectorcraft`,
which VectorCraft traced ([below](#the-traced-pear)). Test inputs live in `conformance/fixtures/`.

| Fixture | Shows | Used by |
|---|---|---|
| `badge.svg` | A red star inside a blue ring | the SVG tutorial: the `badge` shots and the `plan-badge` example; the window probe (`compat/vc-window`) |
| `traced-badge.svg` | A badge as Image Trace traces it in the Brother threads `stitch palette` writes: a red disc, a blue square and a gold disc, abutting, on white, none with settings | the picture tutorial: the `traced-badge` shots and the `plan-traced-badge` example; automatic digitizing's neighbours |
| `emblem.svg` | An emblem drawn in layers on white in 4 of those threads, each layer whole and none with settings. A navy square 50 mm wide holds a white disc with a red star on it and 3 gold bars 3 mm tall | automatic digitizing's flat-colour art: the `auto-emblem-picture` and `auto-emblem` shots |
| `traced-pear.vectorcraft` | The photo of a pear that the `auto-pear-photo` shot draws, traced 90 mm across in 4 of those threads: a light grey backdrop, and the pear in 3 bands of green | automatic digitizing's photo: the `auto-pear-traced` and `auto-pear` shots |
| `wave.svg` | A smooth wave | running stitch length |
| `zigzag.svg` | The same wave, drawn 3 mm wide | the zigzag stroke's spacing and angle, and a satin column drawn as one path |
| `ripple.svg` | An arch, half a circle 30 mm across, open at the bottom | the ripple stitch's lines and joins |
| `e-stitch.svg` | A curved band 4 mm wide between 2 rails | the E-stitch's spacing and swapped rails |
| `s-stitch.svg` | A curved band 6 mm wide between 2 rails | the S-stitch's spacing and split stitches, the zigzag satin's spacing and its look beside a satin column, and the satin column's spacing and underlays |
| `half-disc.svg` | A half disc: a straight side with 2 corners, and a curve | running stitch tolerance |
| `parallel.svg` | 14 rows 0.6 mm apart, sewn as one path | random stitch length |
| `nodes.svg` | Straight lines and one curve | manual stitch |
| `lock.svg` | An 8 mm line | lock shapes |
| `wide.svg` | A 210 mm border, wider than the reference hoop | the `plan-too-wide` example |
| `fill.svg` | A patch 32 mm by 20 mm with rounded corners and a round hole | the tatami fill's angle, staggers and underlay; the contour fill's path |
| `star.svg` | A star, its points 15 mm from its middle and its inner corners 6 mm | the contour fill's strategies and corners |
| `heart.svg` | A heart 32 mm wide and 29 mm tall | cross stitch: its grid, coverage and threads |
| `meander.svg` | A square 30 mm a side with rounded corners | the meander fill's tiles, scale, angle and line, and the circular fill's spacing and travel |
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
