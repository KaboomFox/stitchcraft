# StitchCraft

**Machine embroidery for VectorCraft, in Rust.** StitchCraft turns vector art into machine embroidery
files — PES for Brother machines first, then DST and more — with the stitch types embroiderers know from
Ink/Stitch: running and bean stitch, satin columns, tatami, contour and meander fills, and more.

It is built to a different standard:

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

> **Status (2026-10-08):** design complete for the first milestones; repository bootstrapped with its
> guardrails (milestone M0). The first sewable output is milestone M1.
