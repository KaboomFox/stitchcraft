# Fit a design to the hoop

Each [machine profile](../reference/profiles.md) is a machine with one of its hoops, and its hoop size is
the largest area the machine sews in that hoop. The reference Brother PE800 has a profile for each of its
3 hoops. The 5 × 7 in hoop sews 130 × 180 mm, the 4 × 4 in hoop 100 × 100 mm and the small hoop
20 × 60 mm. Commands plan for the 5 × 7 in hoop unless `--profile` names another.

## Larger than the hoop

Here a border is 210 mm long, and `stitch plan` ends with an error instead of writing the file:

```console
{{#include ../reference/generated/plan-too-wide.txt}}
```

The exit status is 1. Scale the design down, or split it into parts that are sewn in separate
hoopings. When turning the design by 90° makes it fit, the error offers that instead.

## A smaller hoop

Name the hoop's profile, and StitchCraft checks the design against that hoop. Test sheet TS-10B fills the
5 × 7 in hoop, so the 4 × 4 in hoop is too small for it:

```console
{{#include ../reference/generated/testsheet-ts-10b-4x4.txt}}
```

`stitch profiles` lists every profile with its hoop.

## Larger than the comfort zone

A profile may also have a comfort zone: the part of the hoop where the fabric is taut enough for the
stitches to land where they should. A design larger than it is still written, with a warning.
`stitch explain` says more about any code:

```console
{{#include ../reference/generated/explain-sc-w0702.txt}}
```

The built-in profiles have no comfort zone yet. A sew-out that shows a hoop's edges sewing badly would
give its profile one.
