# Docs fixtures

Small designs for the docs' pictures (`docs/shots.toml`) and command-line examples (`docs/examples.toml`),
in SVG and in VectorCraft's format. `cargo xtask docs` and `cargo xtask shots` plan them with the engine,
so a page always shows what StitchCraft does with them today. Each one is hand-written for StitchCraft and
shows one thing, but for `round-patch.vectorcraft`, which VectorCraft writes. Test inputs live in
`conformance/fixtures/`.

| Fixture | Shows | Used by |
|---|---|---|
| `badge.svg` | A red star inside a blue ring | the SVG tutorial: the `badge` shots and the `plan-badge` example; the window probe (`compat/vc-window`) |
| `traced-badge.svg` | A badge as Image Trace traces it in the Brother threads `stitch palette` writes: a red disc, a blue square and a gold disc, abutting, on white, none with settings | the picture tutorial: the `traced-badge` shots and the `plan-traced-badge` example; automatic digitizing's neighbours |
| `wave.svg` | A smooth wave | running stitch length |
| `zigzag.svg` | The same wave, drawn 3 mm wide | the zigzag stroke's spacing and angle, and a satin column drawn as one path |
| `e-stitch.svg` | A curved band 4 mm wide between 2 rails | the E-stitch's spacing and swapped rails |
| `s-stitch.svg` | A curved band 6 mm wide between 2 rails | the S-stitch's spacing and split stitches, the zigzag satin's spacing and its look beside a satin column, and the satin column's spacing and underlays |
| `half-disc.svg` | A half disc: a straight side with 2 corners, and a curve | running stitch tolerance |
| `parallel.svg` | 14 rows 0.6 mm apart, sewn as one path | random stitch length |
| `nodes.svg` | Straight lines and one curve | manual stitch |
| `lock.svg` | An 8 mm line | lock shapes |
| `wide.svg` | A 210 mm border, wider than the reference hoop | the `plan-too-wide` example |
| `fill.svg` | A patch 32 mm by 20 mm with rounded corners and a round hole | the tatami fill's angle, staggers and underlay |
| `widths.svg` | A line 0.6 mm wide, a letter C 3 mm wide and a disc 16 mm across, each traced as an area | automatic digitizing's stitches by width |
| `parts.svg` | A lollipop on a stick 2.4 mm wide and a balloon on a string 0.8 mm wide, each one area | automatic digitizing's shapes in parts |
| `round-patch.vectorcraft` | The VectorCraft tutorial's patch, as VectorCraft saves the `vectorcraft-patch` shot's steps, which are the tutorial's (`cargo xtask shots --vectorcraft` writes it). A disc and a star have StitchCraft Fill and Stroke, and a border of 2 rails has StitchCraft Satin | the VectorCraft tutorial: the `vectorcraft-patch-sewn` shot and the `export-round-patch` example |
