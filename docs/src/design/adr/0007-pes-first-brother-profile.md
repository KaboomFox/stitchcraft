# ADR-0007: PES v1 first, for the Brother 200 × 200 mm machine

**Status:** Accepted · 2026-10-08

## Context

The first physical test machine is a Brother with a 200 × 200 mm (8 × 8 in) hoop; designs are kept to
about 150 mm (6 in). Brother machines load PES. Embroiderers report PES pitfalls with other software:
trims ignored by some Brother models, and an older Brother-family machine that hides PES v1 designs
larger than about 130 × 180 mm.

## Decision

1. The first writer is **PES v1 + PEC**; DST is second (universal fallback, independent check).
2. Built-in profile **`brother-200x200`**: hoop 200 × 200 mm (error beyond), comfort 150 × 150 mm
   (warning beyond), max stitch 12 mm, min stitch 0.3 mm, Brother PEC palette.
3. **MC-1** (end of M1) tests orientation and scale, colour changes, both trim encodings, and designs 150
   and 190 mm wide. A PES v6 writer with explicit hoop dimensions is added only if MC-1 shows v1 is
   insufficient.

## Consequences

- The earliest milestone produces something the user can sew.
- Profile values are provisional until MC-1/MC-2 records confirm them.
