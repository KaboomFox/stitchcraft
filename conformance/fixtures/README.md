# Fixtures

Small inputs for the conformance suite (`docs/src/design/conformance.md`), each with its origin and
licence. Fixtures written for StitchCraft are under the repository's licence (MIT OR Apache-2.0).

| Fixture | What it holds | Used by | Origin |
|---|---|---|---|
| `svg/inkscape-mm.svg` | An Inkscape-style document in millimetres: a layer moved by a transform, the basic shapes, curves, an arc, a gradient, a hidden layer | `REQ-SVG-001`; the `read_svg` fuzz corpus | Hand-written for StitchCraft |
| `svg/px-document.svg` | A document in CSS pixels without a viewBox: nested transforms, a line, a polygon, a polyline | `REQ-SVG-001`; the `read_svg` fuzz corpus | Hand-written for StitchCraft |
| `svg/unsupported.svg` | Every SVG feature StitchCraft reports instead of stitching: a style sheet, text, an image, a clone, a pattern, clipping, a nested `<svg>` | `SC-W0802`; the `read_svg` fuzz corpus | Hand-written for StitchCraft |
| `pes/pyembroidery-origin-start.pes`, `pes/pyembroidery-offset-start.pes` | Three stitches, written by pyembroidery 1.5.1 without the PEC origin field: from the origin, and from a long jump shaped like the field | `REQ-FMT-009` | Written by pyembroidery 1.5.1 (MIT) with `conformance/oracle/write_pes.py` |
| `svg/inkstitch-objects.svg` | Ink/Stitch's own objects, laid out as its extensions write them: trim, stop and ignore commands with their connectors and symbols, a connector-tool line, guide and pattern helper paths, an ignored group and an ignored layer, an origin command | `REQ-SVG-003`, `REQ-ASM-004`; the `read_svg` fuzz corpus | Hand-written for StitchCraft |
| `svg/degenerate.svg` | Degenerate arcs, a path data error, shapes that draw nothing, a shape 20 km away | `REQ-SVG-002`, `SC-W0804`; the `read_svg` fuzz corpus | Hand-written for StitchCraft |
