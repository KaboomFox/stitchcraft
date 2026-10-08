# Your first sew-out on a Brother

> **Status: planned for M1.** This tutorial is written ahead of the code so the first milestone has a
> clear finish line. Commands below will work once `stitch testsheet` ships (roadmap step M1.8).

You will sew StitchCraft's orientation and scale sheet (TS-01) on a Brother machine with a 200 × 200 mm
hoop and check that size and direction are exactly right.

## You need

- A Brother embroidery machine that reads PES files from a USB stick, and its 200 × 200 mm hoop.
- Medium-weight woven cotton, medium tear-away stabilizer, a 75/11 embroidery needle, 40 wt polyester
  thread, white bobbin thread.
- A ruler with millimetres.

## 1. Generate the file

```console
$ stitch testsheet TS-01 --profile brother-200x200 -o TS-01.pes
```

StitchCraft prints the file's checksum, stitch count, colours and the expected sewing time, and writes
`TS-01-expected.png` showing what you should get.

## 2. Load it on the machine

Copy `TS-01.pes` to a USB stick, plug it into the machine and select the design. Check the machine shows
it at about 100 × 100 mm and the "F" reads correctly on the screen.

## 3. Hoop and sew

Hoop the fabric and stabilizer drum-tight, attach the hoop, and sew.

## 4. Check

- The "F" is not mirrored or rotated.
- The long lines measure 100.0 ± 0.5 mm in both directions.
- The corner squares are square.

## 5. Tell us

File a **Sew-out report** on GitHub with front and back photos, a ruler in view, and your measurements
([machine testing](../../plan/machine-testing.md)).
