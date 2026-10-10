# StitchCraft

**Machine embroidery for VectorCraft, in Rust.** StitchCraft turns vector art into machine embroidery
files — PES for Brother machines first, then DST and more — with running and bean stitch, satin columns,
tatami, contour and meander fills, and more.

How it is built:

- **It never crashes.** Every input is untrusted, every loop is bounded, every problem is a coded,
  explained diagnostic.
- **It is deterministic.** The same design gives the same file on every computer, so bugs reproduce and
  previews match the sew-out.
- **It is proven.** Behaviour is specified as requirements, checked by a conformance suite on every
  change, and sewn on a real machine at every milestone.
- **Its docs cannot drift.** Reference pages and pictures are generated from the code and regenerated on
  every pull request.

## Where to go

- **Embroidering?** Start with the [user guide](user/README.md) and [embroidery basics](user/explanation/embroidery-basics.md).
- **Building StitchCraft?** Read the [technical design](design/tdd.md), then the [roadmap](plan/roadmap.md).
- **Contributing?** See [contributing](contributing/README.md).

> **Status:** StitchCraft sews the strokes of an SVG design and writes PES and DST files. A stroke is
> sewn in running stitch or manual stitch, with bean stitch where it is set. Milestone M4 is under way:
> the engine sews a satin column's underlays and top stitches, and `stitch plan` sews the satins of
> an SVG from M8, when it reads Ink/Stitch's settings. Fills come in M5. The
> [roadmap](plan/roadmap.md) has every step.
