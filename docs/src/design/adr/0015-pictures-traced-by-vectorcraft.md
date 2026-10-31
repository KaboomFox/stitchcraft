# ADR-0015: Pictures are traced by VectorCraft's command line

<!-- The status line, a label and its value, which the ADR index is checked against. -->
<!-- vale ai-tells.ColonUsage = NO -->
**Status:** Accepted · 2026-10-31, after spike M13.9
<!-- vale ai-tells.ColonUsage = YES -->

## Context

M13 sews a picture traced in VectorCraft from one command ([image to embroidery](../image-to-embroidery.md)).
The user places the picture in VectorCraft and traces it with Image Trace, in the machine's threads that
`stitch palette` writes. Then `stitch plan --auto` sews the saved document, or the Auto effect previews it.
`stitch plan picture.png` would leave VectorCraft out of the flow, and spike M13.9 chooses how a picture
given to the command line becomes shapes. There were 3 ways:

1. **VectorCraft's command line traces it.** `vectorcraft-cli run` runs VectorCraft's commands without a
   window, from standard input too (`--commands -`), and saves the document. `file.place` places a
   picture from its data, fitted to a rectangle, `imageTrace.make` and `imageTrace.expand` trace it, and
   `swatch.library.load` loads a palette. VectorCraft's `docs/mcp.md` describes them, and
   `vectorcraft-cli commands` lists each command's parameters. VectorCraft publishes `vectorcraft-cli`
   with each release, for macOS, Windows and Linux (its `docs/releasing.md`).
2. **StitchCraft reads PNG and JPEG files and traces them itself.**
3. **StitchCraft links VectorCraft's tracer as a library.**

What was checked, on 2026-10-31:

- The compatibility gate's Level B case `trace` runs the steps of way 1 on every track. It loads the
  reference machine's threads as a swatch library and places a badge. Image Trace traces the badge in
  colour with the library's colours by the Abutting method, and the paths are expanded and saved.
  StitchCraft sews the saved document, digitized automatically, in the badge's 3 threads
  ([compatibility gate](../compatibility-gate.md)). It passes on VectorCraft 0.18.0.
- VectorCraft's tracer, `crates/trace` in its repository, is 14,400 lines of Rust, 8,600 of them outside
  its tests. A harness measures its results on images whose right answer is known, and its report
  (`docs/image-trace-quality.md`) runs to 1,300 lines. It covers palettes made by median cut and k-means,
  and contours whose edges fall between pixels, fitted with curves.
- StitchCraft depends on VectorCraft only through the plug-in ABI and the documented `.vectorcraft` format
  (`AGENTS.md`).

## Decision

1. `stitch plan` and `stitch export`, given a picture, trace it with VectorCraft's command line. They run
   `vectorcraft-cli run` with the `trace` case's steps. The machine profile's threads, as `stitch palette`
   writes them, become a swatch library. The picture is placed and traced in colour with the library's
   colours by the Abutting method, then expanded and saved. Then they read the saved document with
   `--auto`.
2. They look for `vectorcraft-cli` where the user names it, and else on the `PATH`. Without it the command
   fails with a coded error, which says how to install VectorCraft or to trace the picture in VectorCraft.
3. StitchCraft doesn't trace pictures itself, and doesn't link VectorCraft's tracer.
4. M13.10 builds it, with a Level B case in which `stitch` plans a picture through VectorCraft's command
   line on every track. It settles the picture's size and how many threads the trace may use.

## Consequences

- StitchCraft's crates don't read PNG or JPEG files, and don't trace. The WebAssembly build and the
  layering stay as they are.
- A picture given to `stitch` sews as the same picture traced in VectorCraft does, in the tutorial's flow.
  A fix to VectorCraft's tracer helps both, and the user can open the saved document in VectorCraft to
  correct the trace.
- `stitch plan picture.png` needs VectorCraft installed. Without it, the user traces the picture in
  VectorCraft, as the tutorial does, or doesn't trace it from the command line at all.
- Only the `stitch` app runs VectorCraft's command line, as library crates don't start processes
  ([guardrails](../guardrails.md)).
- The commands and their parameters are VectorCraft's. The `trace` case fails on the track whose VectorCraft
  changes them, before a release does.
- Revisit this if StitchCraft must trace without VectorCraft, as a web build without it would. Revisit it
  too if VectorCraft's command line stops offering these commands.

## Alternatives rejected

- **StitchCraft traces the picture itself.** It would repeat VectorCraft's tracer and the harness that
  measures it, or trace worse. A picture would then sew one way from the command line and another from the
  tutorial's flow, and 2 tracers would need keeping in step. It also adds PNG and JPEG decoders to
  StitchCraft's crates.
- **VectorCraft's tracer as a library.** StitchCraft would depend on VectorCraft's internal crates. `AGENTS.md`
  rules that out: StitchCraft depends on the ABI and the format alone. Each VectorCraft change to the crate
  would be a change StitchCraft has to follow, without the gate's commands in between.
