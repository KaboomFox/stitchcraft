# Tartan fill

<!-- stitch-types: tartan_fill -->

A tartan fill covers an area with a plaid. Stripes of colour run across it one way, the warp, and down it
the other way, the weft, and they weave. The fill sews rows of running stitches that slant across both
ways' stripes, given to the warp and the weft in turns. Where a warp stripe crosses a weft stripe of another
colour, their rows alternate, and the 2 colours mix as woven threads do. Each colour sews in a thread of its
own. Set a fill's `fill_method` to `tartan_fill` for it, and its `tartan` setting to the sett: the order of
its stripes, their colours and their widths.

Its settings are set as every stitch type's are ([setting parameters](./#setting-parameters)).

The pictures show the square of `docs/fixtures/tartan.svg`, 30 mm a side, in 3 setts, without underlay.

<!-- shot: tartan-setts -->
| Mirrored | Repeated | Warp and weft apart |
|:-:|:-:|:-:|
| ![A square 30 mm a side sewn as a tartan fill in 3 setts, its rows slanting across the stripes so that the warp's and the weft's colours weave, Mirrored](../../images/generated/tartan-setts-1.png) | ![A square 30 mm a side sewn as a tartan fill in 3 setts, its rows slanting across the stripes so that the warp's and the weft's colours weave, Repeated](../../images/generated/tartan-setts-2.png) | ![A square 30 mm a side sewn as a tartan fill in 3 setts, its rows slanting across the stripes so that the warp's and the weft's colours weave, Warp and weft apart](../../images/generated/tartan-setts-3.png) |
<!-- /shot -->

## The sett

The sett is the `tartan` setting, as Ink/Stitch's tartan editor saves it in an SVG file: an object of
JSON. Its `palette` lists the stripes in order, each a colour in brackets and then a width in millimetres:

```json
{"palette": "(#ed171f)/4 (#f0f0f0)1.5 (#0e1f7c)3 (#e8a900)/2", "symmetry": true, "scale": 100, "rotate": 0, "offset_x": 0, "offset_y": 0, "min_stripe_width": 1}
```

- A colour is `#` and 6 or 3 hexadecimal digits, or a colour name of SVG such as `navy`.
- A `?` before the width leaves the stripe unsewn, and the fabric shows there. A `*` sews it in the colour
  of the stripe before it.
- A `|` parts the warp's stripes from the weft's. Without one, the weft is the warp.
- With `symmetry` on, the sett is mirrored at its first and last stripes, the ones Ink/Stitch's editor marks
  with a slash, and each of those is sewn once. Off, the sett repeats.
- `scale` scales every width, in percent. `rotate` turns the sett, in degrees clockwise as the canvas shows
  it, and `offset_x` and `offset_y` move it, in millimetres.
- A stripe no wider than `min_stripe_width` is sewn as a run along its middle ([below](#runs)).

Each setting the object leaves out takes the value Ink/Stitch's editor starts from. A colour that can't be
read leaves its stripe unsewn, as in Ink/Stitch, and `SC-W0328` names it. A sett that can't be read skips
the element, and `SC-E0323` says why. Ink/Stitch's editor can also write a sett as a threadcount of the
Scottish Register of Tartans or as colour letters. StitchCraft reads those from M10.7. A tartan fill
without a sett sews Ink/Stitch's default one: stripes 5 mm wide of a thread almost black, with 5 mm left
unsewn between them. `SC-W0325` says so.

The VectorCraft plug-in's fill dialog offers the tartan fill with its row angle and rows per thread, and
with its herringbone width and where its colours end. It has no field for a sett before M10.7
([VectorCraft integration](../../design/vectorcraft-integration.md#a-tartans-sett)). So a tartan fill made
in VectorCraft sews the default sett. A file from Ink/Stitch keeps its setts, and `stitch plan` sews them.

## Rows

The rows run across the stripes at `tartan_angle`, -45° by default, so that they slant across the warp and
the weft alike. They lie `row_spacing_mm` apart. Their needle points lie the longest stitch apart
(`max_stitch_length_mm`), and each row's are moved on from the row before's by `staggers`, as a tatami
fill's are. `rows_per_thread` rows go to the warp and then as many to the weft, over and over. It is 2 by
default, which weaves a twill 2 by 2.
More rows to a thread weave a coarser twill. In each stripe, a colour's rows are joined along the stripe's
edges into one zigzag, which the outline cuts.

## Runs

A stripe no wider than `min_stripe_width` is too narrow for rows. It is sewn as a run along its middle in
its own thread, after every colour's rows, and the stripes beside it share its width. A run along the warp
sews in stitches no longer than `running_stitch_length_mm`, and one along the weft in stitches no longer
than `max_stitch_length_mm`, as Ink/Stitch sews them. `bean_stitch_repeats` sews each stitch of a run back
and forth, for a bolder line. From one run to the next the needle follows the outline, in the run's colour.

<!-- shot: tartan-runs -->
| Stripes of 0.5 and 0.8 mm as runs | Turned 30°, at 150 % | 4 rows per thread |
|:-:|:-:|:-:|
| ![The square sewn as a tartan fill with stripes narrow enough to sew as runs along their middles, and with a sett turned and scaled, Stripes of 0.5 and 0.8 mm as runs](../../images/generated/tartan-runs-1.png) | ![The square sewn as a tartan fill with stripes narrow enough to sew as runs along their middles, and with a sett turned and scaled, Turned 30°, at 150 %](../../images/generated/tartan-runs-2.png) | ![The square sewn as a tartan fill with stripes narrow enough to sew as runs along their middles, and with a sett turned and scaled, 4 rows per thread](../../images/generated/tartan-runs-3.png) |
<!-- /shot -->

## Colours and their order

Before its colours, each part of the fill sews the tatami fill's underlay in the element's own thread
([tatami fill](tatami-fill.md#underlay)). Then each colour sews its rows in its own thread. The warp's
colours come first, in the order they first appear across its stripes, and then the weft's others. The
runs come last. A stripe in the element's own colour sews in the element's own thread, and the others in
threads of their colours. A colour starts and ends with its rows, and between its rows the needle follows
the outline. With
`stop_at_ending_point` on, every colour starts where the fill starts and ends where it ends instead.
Each colour is a colour block of its own, with a trim after it when the fill's `trim_after` is on. A fill
in several parts sews each part's underlay and colours, and then sorts them by colour, so that each thread
is sewn once.

`herringbone_width_mm` sews herringbones from M10.7. Until then the rows are straight, as they are at a
width of 0, and `SC-W0324` says so.

## Test sheet

TS-17B sews 5 tartans 40 mm square for the next machine checkpoint, each in setts of Brother's threads
([test sheets](../reference/test-sheets.md)).

<!-- shot: testsheet-ts-17b -->
![TS-17B as it will sew: 5 tartans 40 mm square in 2 rows, all woven in rows that slant across their stripes. In the top row, left to right, are stripes of red, white, Prussian blue and gold mirrored at their ends, stripes of green, black and white repeated, and a warp of red and black under a weft of blue and white. Below are blue and black stripes with thin white and red lines along their middles, and stripes of gold, black and tangerine turned 30°.](../../images/generated/testsheet-ts-17b.png)
<!-- /shot -->

## Parameters

The reference gives each setting's range and default:

- [tartan](../reference/params/tartan.md), the sett, the rows' angle, the rows per thread, the herringbone
  and where each colour ends
- [fill](../reference/params/fill.md), which chooses the fill method
- [tatami fill](../reference/params/tatami.md), the row spacing, staggers, `skip_last`, expanding and the
  underlay
- [path fills](../reference/params/path-fill.md), the runs' bean stitch
- [running stitch](../reference/params/running.md), the stitch length of the runs and the needle's travel
- [the settings every element shares](../reference/params/common.md), the longest stitch among them
