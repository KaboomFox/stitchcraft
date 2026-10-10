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
is complete. M3 is under way: every parameter is declared once and its reference page, JSON Schema and
Ink/Stitch-contract status are generated from that declaration; every diagnostic code has a case;
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
M4 has begun. A satin column's path is read as its rails and rungs, and its top stitches are placed as
Ink/Stitch places them.
The repository is
[KaboomFox/stitchcraft](https://github.com/KaboomFox/stitchcraft), with the docs published at
[kaboomfox.github.io/stitchcraft](https://kaboomfox.github.io/stitchcraft/) and `main` protected; the
open owner action from M0.5 is the code-of-conduct contact.

| Milestone | Scope | Status | Machine checkpoint |
|---|---|---|---|
| M0 | Foundations and guardrails | 🟡 M0.1–M0.4, M0.9 and M0.10 done; M0.5 partly (repository, owner, labels, Pages, branch protection; code-of-conduct contact open); M0.6–M0.8 open | — |
| M1 | Stitch plan, Brother profile, PES/DST writers, test sheets | 🟡 M1.1–M1.10 done; closes with MC-1 | MC-1 🟡 kit out for sewing |
| M2 | Readers, preview renderer, fuzzing | 🟢 M2.1–M2.8 done | — |
| M3 | Running stitch family, plan assembly, SVG input | 🟡 M3.1 to M3.10 done (parameter registry, a case for every diagnostic code, SVG input, running stitch, repeats and bean stitch, manual stitch, lock stitches, plan assembly, finalize and `stitch plan`, bug-report bundles). M3 closes with MC-2. | MC-2 🟡 kit ready for sewing (TS-02B, TS-03, TS-04) |
| M4 | Satin column | 🟡 M4.1 and M4.2 done. Satin columns are sewn, their top stitches placed as in Ink/Stitch. Compensation, short stitches, split stitches and underlays are next | MC-3 ⚪ |
| M5 | Tatami fill | ⚪ | MC-4 ⚪ |
| M6 | VectorCraft plug-in (ABI v1), export from `.vectorcraft`, compatibility gate | ⚪ | MC-5 ⚪ |
| M7 | Zigzag/E/S stitches, contour, meander, circular fills | ⚪ | MC-6 ⚪ |
| M8 | Ink/Stitch SVG interoperability, differential testing | ⚪ | — |
| M9 | VectorCraft ABI v2 (upstream RFC) | ⚪ | — |
| M10 | Guided, gradient, ripple, tartan, cross stitch; routing tools | ⚪ | — |
| M11 | EXP, JEF, VP3, XXX, U01; thread catalogues | ⚪ | — |
| M12 | 1.0: performance, docs audit, translations, release automation | ⚪ | MC-7 ⚪ |

Legend: ⚪ not started · 🟡 in progress · 🟢 done (with links to the checkpoint report)
