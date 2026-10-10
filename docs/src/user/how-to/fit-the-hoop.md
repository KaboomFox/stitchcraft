# Fit a design to the hoop

Each [machine profile](../reference/profiles.md) has 2 sizes. The hoop is the largest area the machine
sews. The comfort zone is the part of it where the fabric is held firmly enough for the stitches to land
where they should. On the reference Brother the hoop is 200 × 200 mm and the comfort zone 150 × 150 mm.

## Larger than the comfort zone

StitchCraft writes the file and warns. Test sheet TS-10B is 190 × 150 mm on purpose:

```console
{{#include ../reference/generated/testsheet-ts-10b.txt}}
```

The design will sew, but near the hoop's edge the fabric can shift and pucker. `stitch explain` says
more about any code:

```console
{{#include ../reference/generated/explain-sc-w0702.txt}}
```

Hoop the fabric drum-tight with a firm stabilizer, or scale the design down. When turning the design by
90° brings it inside the comfort zone, the warning offers that instead.

## Larger than the hoop

Here a border is 210 mm long, and `stitch plan` ends with an error instead of writing the file:

```console
{{#include ../reference/generated/plan-too-wide.txt}}
```

The exit status is 1. Scale the design down, or split it into parts that are sewn in separate
hoopings. When turning the design by 90° makes it fit, the error offers that
instead.
