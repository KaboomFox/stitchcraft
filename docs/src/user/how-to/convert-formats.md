# Convert between machine formats

`stitch convert` reads a PES, PEC or DST file and writes it as PES or DST. The output file's extension
picks the format, or `--format` names it. Each format records some things the other cannot, and the
command says what changed.

## PES to DST

DST is the format most commercial machines read. Here test sheet TS-02 is converted and the new file
read back:

```console
{{#include ../reference/generated/convert-ts-02-dst.txt}}
```

- **Colours.** A DST file records where the colour changes are, without the colours. The machine pauses
  at each colour change, and you load the next thread yourself. Note the thread list before you convert.
- **Stops.** DST has one pause command, for colour changes and stops alike. The stop in TS-02 reads back
  as a third colour change.
- **Trims.** DST has no trim command either. StitchCraft writes a trim as 3 short jumps that end where
  they started, and a machine set to cut at 3 jumps in a row cuts there.
- **Long jumps.** A DST record moves the frame at most 12.1 mm, and a longer jump takes several records.
  A jump longer than 24.2 mm takes 3 or more, and a machine set to 3 cuts the thread there too.
  `SC-I0605` counts these extra cuts. After a tie-off they save you cutting the jump thread by hand.
  Without one, the end of the stitching can pull out.

## DST to PES

A PES file needs a colour for each thread, and a DST file has none to give. Each thread becomes a black
placeholder, and `SC-W0604` says so. The machine shows every colour block as black, and it pauses
between them as the DST file did.

The [machine formats design](../../design/formats.md) describes how each format is written and read.
