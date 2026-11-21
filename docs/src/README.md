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

## Try it in your browser

[Open the browser demo](demo/index.html) with the Alpine Night drawing to turn into stitches, or read the
[browser demo guide](browser-demo.md).

## Where to go

- **Embroidering?** Start with the [user guide](user/) and [embroidery basics](user/explanation/embroidery-basics.md).
- **Building StitchCraft?** Read the [technical design](design/tdd.md), then the [roadmap](plan/roadmap.md).
- **Contributing?** See [contributing](contributing/).

> **Status:** before a first release. StitchCraft sews strokes, satin columns and fills from SVG files,
> Ink/Stitch's files and VectorCraft documents, chooses the stitches of a picture traced in VectorCraft, sets
> text in Ink/Stitch's embroidery fonts, and writes PES and DST files. The machine checkpoints that check
> it on fabric are out for sewing. The [roadmap](plan/roadmap.md) has every step and where each stands.
