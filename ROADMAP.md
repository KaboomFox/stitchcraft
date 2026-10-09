# Roadmap status

The plan, with every step and its done-criteria, is in [docs/src/plan/roadmap.md](docs/src/plan/roadmap.md).
This file is the status board: update it in the same PR that completes a step.

**Where we are (2026-10-08):** M1's code and docs are done: StitchCraft writes PES and DST files, the
reference pages are generated from the code, and the MC-1 test sheets (TS-01, TS-02, TS-10A/B/C) are out
for sewing on the reference machine. M2 is under way: StitchCraft reads PES, PEC and DST files back
(`stitch inspect`), every writer/reader pair round-trips, pinned pyembroidery cross-checks every golden
file, `stitch preview` draws exactly what will sew, `stitch convert` moves designs between PES and DST,
the docs show generated pictures of every test sheet, and the readers are fuzzed for an hour every night.
The conformance report shows all 17 active requirements green. Next: coverage and mutation baselines
(M2.8), and the MC-1 sew-out report, which closes M1. The repository is
[KaboomFox/inkcraft](https://github.com/KaboomFox/inkcraft) (the project inside keeps the name
StitchCraft); open owner actions from M0.5: GitHub Pages, branch protection and the code-of-conduct
contact.

| Milestone | Scope | Status | Machine checkpoint |
|---|---|---|---|
| M0 | Foundations and guardrails | 🟡 M0.1–M0.4 done; M0.5 partly (repository, owner, labels; Pages and branch protection open); M0.6–M0.8 open | — |
| M1 | Stitch plan, Brother profile, PES/DST writers, test sheets | 🟡 M1.1–M1.10 done; closes with MC-1 | MC-1 🟡 kit out for sewing |
| M2 | Readers, preview renderer, fuzzing | 🟡 M2.1–M2.7 done; M2.8 open | — |
| M3 | Running stitch family, plan assembly, SVG input | ⚪ | MC-2 ⚪ |
| M4 | Satin column | ⚪ | MC-3 ⚪ |
| M5 | Tatami fill | ⚪ | MC-4 ⚪ |
| M6 | VectorCraft plug-in (ABI v1), export from `.vectorcraft`, compatibility gate | ⚪ | MC-5 ⚪ |
| M7 | Zigzag/E/S stitches, contour, meander, circular fills | ⚪ | MC-6 ⚪ |
| M8 | Ink/Stitch SVG interoperability, differential testing | ⚪ | — |
| M9 | VectorCraft ABI v2 (upstream RFC) | ⚪ | — |
| M10 | Guided, gradient, ripple, tartan, cross stitch; routing tools | ⚪ | — |
| M11 | EXP, JEF, VP3, XXX, U01; thread catalogues | ⚪ | — |
| M12 | 1.0: performance, docs audit, translations, release automation | ⚪ | MC-7 ⚪ |

Legend: ⚪ not started · 🟡 in progress · 🟢 done (with links to the checkpoint report)
