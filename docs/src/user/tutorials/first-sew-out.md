# Your first sew-out on a Brother

You will sew StitchCraft's orientation and scale sheet, TS-01, on a Brother machine with a 200 × 200 mm
hoop, and check that its size and direction are exactly right. It takes about twenty minutes.

## You need

- A Brother embroidery machine that reads PES files from a USB stick, and its 200 × 200 mm hoop.
- Medium-weight woven cotton, medium tear-away stabilizer, a 75/11 embroidery needle, 40 wt polyester
  thread (black shows the lines best), white bobbin thread.
- A ruler or calipers with millimetres.

## 1. Write the file

```console
{{#include ../reference/generated/testsheet-ts-01.txt}}
```

That is real output: this page is regenerated from StitchCraft itself, so the checksum is the one your
file will have. The checksum is how a sew-out report names exactly the bytes that were sewn. Every test
sheet is in the [test sheets reference](../reference/test-sheets.md); `stitch testsheet --list` lists
them too.

## 2. Load it on the machine

Copy `TS-01.pes` to a USB stick, plug it into the machine and select the design. The machine should show
it at 120 × 120 mm, with an upright "F" in the top-left quarter. If it does not list the file at all,
stop here and report that — it is the most important thing this sheet tests.

## 3. Hoop and sew

Hoop the fabric and stabilizer drum-tight, centre the design, attach the hoop, and sew. The machine sews
the cross first, then the "F", then the four corner squares, trimming between parts if it can.

## 4. Check

- The "F" reads normally: not mirrored, not upside down, not turned.
- Each arm of the cross measures 100.0 ± 0.5 mm end to end, horizontally and vertically.
- The corner squares measure 10.0 mm on every side.
- Ticks are 10 mm apart; the long ticks mark the ends and the centre.

## 5. Tell us

File a **Sew-out report** on GitHub with photos of the front and back (a ruler in view), your measurements,
and anything odd ([machine testing](../../plan/machine-testing.md)).
