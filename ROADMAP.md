# Roadmap status

The plan, with every step and its done-criteria, is in [docs/src/plan/roadmap.md](docs/src/plan/roadmap.md).
This file is the status board: update it in the same PR that completes a step.

**Where we are (2026-10-08):** M1's code and docs are done: StitchCraft writes PES and DST files, the
reference pages are generated from the code, and the MC-1 test sheets (TS-01, TS-02, TS-10A/B/C) are out
for sewing on the reference machine. M2 is under way: StitchCraft reads PES, PEC and DST files back
(`stitch inspect`), every writer/reader pair round-trips, pinned pyembroidery cross-checks every golden
file, `stitch preview` draws exactly what will sew, `stitch convert` moves designs between PES and DST,
the docs show generated pictures of every test sheet, and the readers are fuzzed for an hour every night.
Public APIs, line coverage and mutation testing now have recorded baselines that only move one way. M2
is complete. M3 is under way. The registry's declarations generate the parameters' reference pages and
their JSON Schema, and their status in the Ink/Stitch contract; every diagnostic code has a case;
StitchCraft reads SVG files into its design model, reporting whatever it leaves out; and the engine
stitches running stitches: even spacing between corners, patterns of lengths, curves followed within the
tolerance, no stitch shorter than the shortest stitch, measured straight, repeats, bean stitch and random
length, manual stitches placed by hand, and lock stitches of every shape at either end of the
stitching. It assembles a design's elements into one plan, with jumps, trims, stops, thread changes and
locks where `ties` says, and fits the plan to the machine. `stitch plan design.svg -o design.pes` takes an
SVG of strokes to a machine file, with a preview and a report. `stitch bug-report` writes one file that
reproduces a problem, and `stitch plan` writes one by itself when StitchCraft finds a bug in itself. The
conformance report shows every active requirement green. Next are the sew-out reports for MC-1, which
closes M1, and for MC-2, which closes M3. The MC-2 sheets are drawn as designs and planned by the engine.
M4's steps are done. A satin column's path is read as its rails and rungs, or made into them from its
centre line, and its underlays and top stitches are sewn as Ink/Stitch sews them. M5's steps are done too.
Tatami fills are sewn as Ink/Stitch sews them, from the region the drawing shows. The MC-3 kit sews satin
columns of every width from 1 to 10 mm, and each underlay side by side. The MC-4 kit sews tatami fills
inside outlines at 3 pull compensations, squares at 5 spacings and 3 angles, and a fill round holes with a
part apart. The reference machine is a Brother PE800 (ADR 0013), with a profile for each of its 3 hoops,
and TS-02, TS-02B and TS-10 are redrawn to fit them.
M7's first 5 steps are done, and every satin method is sewn. M13, image to embroidery, is under way, so that a
picture traced in VectorCraft sews well without settings on its shapes: with `--auto`, a shape without
settings sews by its width (M13.1). M7.6 to M7.9 follow M13.
The repository is
[KaboomFox/stitchcraft](https://github.com/KaboomFox/stitchcraft), with the docs published at
[kaboomfox.github.io/stitchcraft](https://kaboomfox.github.io/stitchcraft/) and `main` protected; the
open owner action from M0.5 is the code-of-conduct contact.

| Milestone | Scope | Status | Machine checkpoint |
|---|---|---|---|
| M0 | Foundations and guardrails | 🟡 M0.1 to M0.4 and M0.6 to M0.10 done. M0.5 partly done: the repository, owner, labels, Pages and branch protection, with the code-of-conduct contact open | none |
| M1 | Stitch plan, Brother profile, PES/DST writers, test sheets | 🟡 M1.1 to M1.10 done, and M1 closes with MC-1 | MC-1 🟡 kit out for sewing (TS-02 and TS-10 redrawn for the PE800) |
| M2 | Readers, preview renderer, fuzzing | 🟢 M2.1–M2.8 done | — |
| M3 | Running stitch family, plan assembly, SVG input | 🟡 M3.1 to M3.10 done (parameter registry, a case for every diagnostic code, SVG input, running stitch, repeats and bean stitch, manual stitch, lock stitches, plan assembly, finalize and `stitch plan`, bug-report bundles). M3 closes with MC-2. | MC-2 🟡 kit ready for sewing (TS-02B, TS-03, TS-04) |
| M4 | Satin column | 🟡 M4.1 to M4.9 done. Satin columns are sewn with their underlays and start and end at their nearest points. Their top stitches are placed, compensated, inset on curves and split, as in Ink/Stitch. A column drawn as one path is as wide as its stroke. It is sewn between rails made from its centre line, or as a stroke when too narrow to stitch across. MC-3's sew-out closes M4 | MC-3 🟡 kit ready for sewing (TS-05, TS-06) |
| M5 | Tatami fill | 🟡 M5.1 to M5.9 done. A fill's region is built as the drawing shows it under its fill rule, and parts too small to sew are left out with a warning. Its rows, and the needle points along them, lie where Ink/Stitch puts them. Tatami fills are sewn part by part, each part's underlay first, a layer for each angle, then its rows in the order Ink/Stitch routes them, with running stitches under the rows between them, as Ink/Stitch sews them. The underlay covers each part shrunk by its inset, and the top rows each part grown by `expand_mm`. The top rows are lengthened by the pull compensation, and gap-fill rows close the gaps between stretches of rows. A fill covers at least 98 % of its area at spacings from the machine's shortest stitch to 0.4 mm, and the hoop's whole field plans in about 30 ms, guarded by an instruction count. MC-4's sew-out closes M5 | MC-4 🟡 kit ready for sewing (TS-07, TS-08, TS-09) |
| M6 | VectorCraft plug-in (ABI v1), export from `.vectorcraft`, compatibility gate | 🟡 M6.1 to M6.12 done: `.vectorcraft` files read into designs, with StitchCraft's effects saying how an object sews, a group's included; the stroke, satin and fill plug-ins, whose dialogs the parameter registry makes, and whose previews show as much of the stitches as VectorCraft's budget affords; VectorCraft's own commands as the tools, and its Graphic Styles as presets; machine files from documents with `stitch export`, and names that leave objects out or mark where shapes start and end; the compatibility gate, both levels on four tracks every day, with an issue per track and a pull request adopting each new release that passes; the docs' pictures of VectorCraft, made by the pinned release, and a tutorial; the live effects following their object's stroke, given from VectorCraft 0.14, so that a satin column drawn as one path and a zigzag show their stitches; PES and DST exporters, so that VectorCraft's File › Export writes the file `stitch export` writes; dialogs that label each parameter, with its unit, help and page, in the registry's sections, and show it only where it applies; a filter, from VectorCraft 0.16, that keeps the design settings and machine with the document, which the exporters and `stitch export` plan with; and the tools, 2 filters: one that makes lines into satin columns of rails and rungs that sew as the lines did, a heavy filter of VectorCraft 0.17 (Stroke to Satin), and one that draws the test sheets drawn as designs into a document (Test Sheet). M6 closes with MC-5 | MC-5 ⚪ |
| M7 | Zigzag/E/S stitches, contour, meander, circular fills | 🟡 M7.1 to M7.5 done: a family of stitch types can declare a key again with a kind and range of its own, and zigzag strokes and every satin method, E-stitch, S-stitch and zigzag satin, are sewn as Ink/Stitch sews them. Every satin method names stitches that lean far along the column and top stitches longer than 12 mm. M7.6 to M7.9 follow M13 | MC-6 ⚪ |
| M13 | Image to embroidery: stitch types chosen by width, overlaps, specks and background, sewing order, VectorCraft's Auto effect | 🟡 M13.1 done: with `--auto`, `stitch plan` and `stitch export` choose the stitches of the shapes without settings by their width. An area is a tatami fill, a line a running stitch up to the narrowest satin stroke, a satin column as wide as the line, caps included, up to 7 mm, and wider the fill of the area it paints | MC-8 ⚪ |
| M8 | Ink/Stitch SVG interoperability, differential testing | ⚪ | — |
| M14 | Lettering in embroidery fonts, and text in pictures | ⚪ | MC-9 ⚪ |
| M10 | Guided, gradient, ripple, tartan, cross stitch; routing tools | ⚪ | — |
| M11 | EXP, JEF, VP3, XXX, U01; thread catalogues | ⚪ | — |
| M12 | 1.0: performance, docs audit, translations, release automation | ⚪ | MC-7 ⚪ |
| M9 | VectorCraft plug-in features still proposed upstream (RFC), for previews drawn as overlays, heavy live effects and the objects beside a live effect, last: after 1.0 and the work after it | ⚪ | none |

Legend: ⚪ not started · 🟡 in progress · 🟢 done (with links to the checkpoint report)
