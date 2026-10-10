# Check a machine file before sewing

StitchCraft reads PES, PEC and DST files from any software. It can describe a file and draw what the
machine will sew, before you spend thread on it. The examples use test sheet TS-02, written with
`stitch testsheet TS-02 --profile brother-200x200 -o TS-02.pes`.

## Describe the file

```console
{{#include ../reference/generated/inspect-ts-02.txt}}
```

- **size** is the width and height of the stitching.
- **stitches** counts the needle points, and the jumps, trims, colour changes and stops between them.
- **extent** gives the lowest and highest x and y of the stitching, from where the machine starts.
- **lengths** gives the shortest and the longest stitch.
- **threads** lists the colour blocks in sewing order. Each colour is stored as a Brother palette colour
  in a PES file, which the line names. After a stop you sew on with the same thread.
- **profile** appears with `--profile`, with the number of stitches longer than the machine's longest
  stitch and shorter than its shortest. `stitch profiles` lists the machines.

## Look at it

```console
{{#include ../reference/generated/preview-ts-02.txt}}
```

The simple style draws each stitch as a thin line with a dot at each needle point. A red cross marks a
trim and a blue square a stop. Grey dashes show where the frame travels after a cut, and dashes in the
thread's colour show a jump thread left on the fabric.

<!-- shot: testsheet-ts-02-simple -->
![TS-02 in the simple style. A red cross marks the trim after the first dash of each red row, and a blue square marks the stop in the middle of the green line. Grey dashes show moves after a cut, and blue dashes show jump threads left in place.](../../images/generated/testsheet-ts-02-simple.png)
<!-- /shot -->

Without `--style simple`, the picture shows the finished embroidery, with loose jump threads where they
will be. The [rendering design](../../design/rendering.md#styles) has the whole legend.

## What to look for

- A design larger than the hoop, or than the machine's comfort zone: see [fit the hoop](fit-the-hoop.md).
- Jump threads where you expected trims. Each one has to be cut by hand after sewing.
- Thread colours. The machine shows each block in its palette colour, and it sews whichever thread you
  load.
