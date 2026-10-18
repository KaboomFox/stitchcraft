# Roadmap: small steps to the final project

Each **step** is one pull request: small, reviewable, with its tests, conformance cases and docs. Each
**milestone** ends with green gates and, where it produces something sewable, a **machine checkpoint**
(MC) on the reference Brother PE800 with photos and measurements filed through the sew-out report
form ([machine testing](machine-testing.md)).

Step ids are stable (`M3.4`); PR titles start with them. Status lives in `ROADMAP.md` at the repository
root and is updated in the same PR that completes a step.

**Legend:** ✅ done in the bootstrap commit · 🧪 spike (answers a question; may produce a decision instead
of code) · 🧵 machine checkpoint · 👤 owner action

## M0 — Foundations and guardrails

Goal: a repository where the rules are enforced before the first line of engine code.

| Step | Deliverable | Done when |
|---|---|---|
| M0.1 ✅ | Workspace, crates skeleton, licences, toolchain pin, lints, `rustfmt`/`clippy`/`deny`/`typos` configs | `cargo build` and `cargo test` pass |
| M0.2 ✅ | `cargo xtask` gates: `ci`, `layers`, `docs --check`, `shots --check`, `cleanroom`, `unsafe-audit`, `filesize`, `wasm`, `conformance --check`, `compat discover`/`contract` | `cargo xtask ci` passes locally |
| M0.3 ✅ | Workflows: `ci.yml`, `docs.yml`, `compat.yml` (discovery), `nightly.yml`; templates; Dependabot | First PR runs green on GitHub |
| M0.4 ✅ | mdBook site, design docs, ADRs, this roadmap, machine-testing protocol | `mdbook build` and the docs check pass |
| M0.5 👤 (partly ✅) | Create the GitHub repository, push, enable Pages (from the `gh-pages` branch) and branch protection with required checks `ci` and `docs`; replace `OWNER` in `CODEOWNERS` and `Cargo.toml` and add a contact to `CODE_OF_CONDUCT.md`; create the labels `golden-change`, `docs:refresh-shots`, `upstream-canary`, `upstream-prerelease`, `sewout`, `sewout:pass`, `sewout:tune`, `sewout:bug`, `good first issue` | A test PR cannot merge while a check is red |
| M0.6 ✅ 🧪 | VectorCraft "hello" plug-in: an identity live effect built in CI for `wasm32-unknown-unknown`, a Level A contract test against the pinned stable `vectorcraft-plugins` (v0.6.0, now v0.7.0), `release` and `main`, and the fuel it spends per byte of output | The contract test is green in `compat.yml` on every track (2026-10-08), and the budget is recorded in the integration doc: 512 instructions a byte read and 822 written (2026-10-17) |
| M0.7 ✅ 🧪 | Geometry spike ([ADR-0005](../design/adr/0005-geometry-stack.md) criteria) | ADR-0005 accepted with the engine's own geometry, and the golden files made as WebAssembly in CI (2026-10-17) |
| M0.8 ✅ 🧪 | Headless VectorCraft window in CI (Xvfb + Mesa) taking `ui.screenshot` twice with identical pixels; one [xa11y](https://xa11y.dev/) test that drives the same window through its accessibility tree (AT-SPI on Linux) | Decision recorded in the docs pipeline page, for both (2026-10-17). Screenshots match above the status bar, whose frame timings change. xa11y chooses menu rows by AT-SPI's press and by clicks at the tree's places |
| M0.9 ✅ | Movable into VectorCraft ([ADR-0011](../design/adr/0011-movable-into-vectorcraft.md)): packages prefixed, manifests inheriting only what VectorCraft defines, tooling scoped to its folder and packages, `cargo xtask compat join [--nested]`, and a daily `move` job | `move` green against VectorCraft's latest release and `main` |
| M0.10 ✅ | Clean room, tightened: no document describes Ink/Stitch's code; Ink/Stitch is named only for file compatibility and in the independence notices; `cargo xtask cleanroom` scans documents too | `cleanroom` green over the whole repository |

## M1 — Stitch plan, PES/DST writers, first sew-out

Goal: sew a machine file StitchCraft wrote. No SVG yet: test sheets are generated in code.

| Step | Deliverable | Done when |
|---|---|---|
| M1.1 ✅ | `stitchcraft-core`: `Budget` and the diagnostics model (`Mm`, `Point`, `math` and `SplitMix64` with frozen reference values are already in the bootstrap) | unit tests; budget exhaustion is a diagnostic |
| M1.2 ✅ | `stitchcraft-plan`: `Stitch`, `StitchKind`, `ColorBlock`, `StitchPlan`; invariant checker (`REQ-PLAN-001` to `003`, `005` to `007`) | each invariant has a violating and a passing case |
| M1.3 ✅ | Machine profiles + `brother-200x200` (since replaced by the PE800's, [ADR 0013](../design/adr/0013-brother-pe800-reference-machine.md)); hoop and comfort diagnostics `SC-E0701`/`SC-W0702` with rotate hint | `REQ-PRF-001`, `REQ-PRF-002` active |
| M1.4 ✅ | Quantization (absolute, round-half-even); PEC stitch encoder with spec vectors | `REQ-FMT-002` partial; encoder unit tests |
| M1.5 ✅ | PES v1 writer: header, PEC header, palette indices, thumbnails | golden files (`REQ-FMT-001`) |
| M1.6 ✅ | Brother PEC palette table + CIEDE2000 nearest colour | `REQ-THREAD-001` spot checks |
| M1.7 ✅ | DST writer (header, ternary records, jump splitting, commands) | golden files; `REQ-FMT-007` |
| M1.8 ✅ | `stitch testsheet <TS-xx> --profile … -o file` (and `--list`), `stitch profiles`, `stitch explain`; the CLI moves to `clap` (`stitch inspect` needs the readers: M2.1) | `trycmd` examples in the docs |
| M1.9 ✅ | Conformance runner v1: requirements × cases matrix, L0 + L1 goldens, job summary | report visible on PRs |
| M1.10 ✅ | Docs: profile, format, test-sheet, diagnostics and command-line reference generated; tutorial "Your first sew-out" with real output | docs check green |
| MC-1 🧵 | Sew **TS-01** (orientation, scale), **TS-02** (colour changes, stops, jumps with and without trims), **TS-10A/B/C** (a frame filling each of the PE800's 3 hoops) from PES v1; same TS-01 from DST if the machine reads DST | Sew-out report filed; profile values confirmed or changed; PES v6 decision made |

## M2 — Readers, preview renderer, fuzzing

| Step | Deliverable | Done when |
|---|---|---|
| M2.1 ✅ | PES/PEC reader (bounded, typed errors); `stitch inspect file` (counts, bounds, colours, stitch lengths, profile check) | empty files `REQ-FMT-004`; every truncation and byte flip handled |
| M2.2 ✅ | DST reader (trims inferred from cancelling jump runs) | same |
| M2.3 ✅ | Command round trip for every writer/reader pair; machine-equivalence oracle; fresh-seed nightly | `REQ-FMT-002`, `REQ-FMT-003` active |
| M2.4 ✅ | pyembroidery oracle (pinned by hash) as a conformance case in the gates job | `REQ-FMT-005` active |
| M2.5 ✅ | `cargo-fuzz` targets for both readers; nightly job with persisted corpus | `REQ-FMT-006`; 1 h fuzzing clean |
| M2.6 ✅ | `stitchcraft-render`: realistic and simple styles from the quantized plan | `REQ-RND-001`; golden PNG files |
| M2.7 ✅ | `stitch preview`, `stitch convert`; the `stitch` shot generator behind `cargo xtask shots`; the `docs-refresh.yml` label workflow | first generated docs images, byte-compared on every PR |
| M2.8 ✅ | Coverage ratchet and mutation-testing baseline; public-API snapshots | thresholds recorded |

## M3 — Running stitch family and plan assembly

| Step | Deliverable | Done when |
|---|---|---|
| M3.1 ✅ | Parameter registry (`params!`, `ParamSet`, validation, generated reference + JSON Schema) | `REQ-PRM-001`, `REQ-PRM-002`; `docs --check` covers params |
| M3.2 ✅ | Diagnostics registry, `stitch explain`, generated diagnostics index | every registered code has a page and a case |
| M3.3 ✅ | `Design` model; SVG adapter v1: paths, groups, transforms, viewBox units, colours, visibility (no `inkstitch:*` yet) | `REQ-SVG-001`, `REQ-SVG-002` (fuzzed path data) |
| M3.4 ✅ | Running stitch: corners, even spacing, patterns, tolerance; stitch lengths measured straight | `REQ-RUN-001..003` |
| M3.5 ✅ | Repeats, bean stitch, random length | `REQ-RUN-004`, `REQ-RUN-005`, `REQ-RUN-007` |
| M3.6 ✅ | Manual stitch | `REQ-RUN-006`, `REQ-RUN-008` |
| M3.7 ✅ | Lock stitches: every shape, its size and its place at either end of the stitching | `REQ-LCK-002`, `REQ-LCK-004` |
| M3.8 ✅ | Plan assembly: order, collapse, tie-off/jump/trim/tie-in (where `ties` says), stops, colour changes, the origin and stop position; `stitchcraft_engine::plan` sends each element to its generator | `REQ-ASM-001..003`, `REQ-ASM-005`, `REQ-LCK-001`, `REQ-GEN-002` |
| M3.9 ✅ | Finalize: split, merge, hoop, colour limits, the plan check; `stitch plan in.svg -o out.pes --preview out.png --report out.json` | `REQ-FIN-001..003`; the `strokes` plan case, SVG to machine file |
| M3.10 ✅ | `stitch bug-report` bundle; `stitch plan` writes one for a failed plan check or a panic | a bundle reproduces its plan byte for byte (`REQ-CLI-001`, `REQ-CLI-002`) |
| MC-2 🧵 | **TS-03** (running/bean lengths, the shortest stitch), **TS-04** (lock holding test), **TS-02B** (TS-02 with element-driven trims); kit: `stitch testsheet` for each | report filed; min-stitch and lock defaults confirmed (`REQ-LCK-003`) |

## M4 — Satin column

| Step | Deliverable | Done when |
|---|---|---|
| M4.1 ✅ | Satin shape: rails and rungs told apart as Ink/Stitch tells them, diagnostics `SC-E0201`, `SC-W0202`, `SC-W0203`, `SC-W0205`, `SC-W0207` | `REQ-SAT-005` |
| M4.2 ✅ | Correspondence and stitch placement as in Ink/Stitch (rails turned, sections at the rungs or the nodes), `SC-W0210`. Satin columns are sewn | `REQ-SAT-002` |
| M4.3 ✅ | Pull and push compensation (symmetric/asymmetric), random width and spacing, `SC-W0211` | `REQ-SAT-001`, `REQ-SAT-006` to `REQ-SAT-008` |
| M4.4 ✅ | Short stitches on curves | `REQ-SAT-009`, a case set on tight curves |
| M4.5 ✅ | Split stitches (simple, staggered, random) | `REQ-SAT-003` |
| M4.6 ✅ | Underlays: centre walk, contour, zigzag | `REQ-SAT-004`, `REQ-SAT-010` to `REQ-SAT-012` |
| M4.7 ✅ | Start/end nearest point; generators see their neighbours (ADR 0014) | `REQ-GEN-003`, `REQ-SAT-013`, `REQ-SAT-014` |
| M4.8 ✅ | A stroke's width and join, and Ink/Stitch's design settings, read from SVG as Ink/Stitch reads them. A satin column drawn as one path and too narrow to stitch across is sewn as a stroke, `SC-W0212` | `REQ-SVG-004`, `REQ-SVG-005`, `REQ-SAT-015` |
| M4.9 ✅ | Single-path satin. Its centre line is offset by half the stroke's width to each side, with the stroke's join, into 2 rails, as shapely offsets it, with rungs beside its corners and at its nodes, as Ink/Stitch places them | `REQ-SAT-016`, `REQ-SAT-017` |
| MC-3 🧵 | **TS-05** (width ladder 1 to 10 mm × 3 densities), **TS-06** (underlay comparison); kit: `stitch testsheet` for each, sewable from M4.6 | report filed; satin defaults tuned |

## M5 — Tatami fill

| Step | Deliverable | Done when |
|---|---|---|
| M5.1 ✅ | A fill's region as the drawing shows it under its fill rule, in parts with holes, its subpaths cut exactly where they meet. Parts too small to sew are left out with `SC-W0303`, as Ink/Stitch leaves them out. Small fills (`SC-W0304`), the fill rule (`SC-I0306`) and fills in parts (`SC-W0307`) are pointed out | `REQ-FILL-001`, `REQ-FILL-002` |
| M5.2 ✅ | Rows as Ink/Stitch lays them, at the angle and a whole number of row spacings from the origin, graded towards the end spacing. They are cut where they meet the outline, as GEOS cuts them | `REQ-FILL-TAT-002`, `REQ-FILL-TAT-010` |
| M5.3 ✅ | Needle points as Ink/Stitch places them, on the stagger grid from the origin, with `skip_last` and random stitch length. A fill's longest stitch defaults to 4 mm, so a parameter can have a default for each family | `REQ-FILL-TAT-003` |
| M5.4 ✅ | Routing as Ink/Stitch routes. Every segment sewn once, back and forth, from the needle to the next element, with running stitches along the outline between. Parts sewn one at a time, nearest the needle first, each ending nearest the next. A part too thin for any row sewn round its outline, `SC-W0305`. `fill_method` registered. Fills are sewn | `REQ-FILL-TAT-001`, `REQ-FILL-TAT-008` |
| M5.5 ✅ | Travel under the rows (`underpath`), off the rows already sewn: the cheapest way along 3 gratings of lines, smoothed and kept inside the part. Where no line leads, along rings and rows (`DEV-FILL-007`) | `REQ-FILL-TAT-005` |
| M5.6 ✅ | Underlay as Ink/Stitch sews it: before its top rows each part sews a layer for each angle, each from the point nearest the needle back to it. `fill_underlay` and its settings registered, with a list of angles as a kind of parameter. A layer too thin for a row is sewn round the outline, `SC-I0308` | `REQ-FILL-TAT-011` |
| M5.7 ✅ | The underlay on the part shrunk by `fill_underlay_inset_mm`, and the top rows on the part grown by `expand_mm`. A part is buffered as GEOS buffers a polygon, and checked against shapely. Pieces a shrinking cuts a part into sewn one after another (`DEV-FILL-008`), and a part a shrinking wears away said, `SC-W0309`, `SC-W0310`. A needle inside the part starts at the nearest node of the lines under the rows | `REQ-FILL-TAT-012` |
| M5.8 ✅ | Pull compensation at the rows' ends, holes and parts kept (`DEV-FILL-009`), and below 0 read as 0, `SC-W0312`; gap-fill rows from the grid, inside the region (`DEV-FILL-010`). The pull compensation settings are shared with satins | `REQ-FILL-TAT-006`, `REQ-FILL-TAT-007` |
| M5.9 ✅ | Coverage metric (`stitchcraft_testkit::metrics`), at least 98 % from the machine's shortest stitch to 0.4 mm; the hoop's whole field plans, its instructions guarded by `cargo xtask perf` | `REQ-FILL-TAT-004`, `REQ-FILL-TAT-009` |
| MC-4 🧵 | **TS-07** (fill + outline registration), **TS-08** (density ladder, angles), **TS-09** (underlay, travel); kit: `stitch testsheet` for each, sewable from M5.9 | report filed; fill defaults and pull compensation tuned |

## M6 — VectorCraft plug-in (ABI v1) and export from `.vectorcraft`

| Step | Deliverable | Done when |
|---|---|---|
| M6.1 ✅ | `stitchcraft-vectorcraft`: read `.vectorcraft` files of formats 1 to 4, plain or compressed: layers, groups, paths and compound paths, paints as threads, and StitchCraft's effect records, which say how an object sews. VectorCraft keeps paths in document coordinates, with no transforms to apply. What isn't stitched is named (`SC-W0807`, `SC-W0808`, `SC-I0809`). Caps: 64 MiB of JSON and the JSON reader's depth | `REQ-VC-001`, `REQ-VC-004`, `REQ-VC-005`; fuzzed (`read_vectorcraft`) |
| M6.2 ✅ | The plug-in crate, with manifests generated per family and the ABI shim (`abi.rs`, whose `unsafe` is audited). One module per family, whose live effect shows the object's stitches, or says why it shows none (`SC-I0810`) | `REQ-VC-002` (manifests valid, ≤ 64 params), and the module's size with the engine recorded ([ADR-0005](../design/adr/0005-geometry-stack.md)) |
| M6.3 ✅ | Live-effect previews with budget-aware levels: exact, a fill's rows, and the outline, picked by `preview_level` or by what the budget affords, the engine's work priced in instructions measured in VectorCraft's host | `REQ-VC-003` per level, within 5 s / fuel |
| M6.4 ✅ | Tools as VectorCraft's own commands, checked in its source. Make Compound Path joins a satin column's 2 paths and adds rungs. Expand Appearance bakes a preview, and Graphic Styles are presets. A group's StitchCraft effects sew the paths inside it, as VectorCraft draws them. The tools filter is left for later: making a line into a satin column took 51 to 162 million instructions in VectorCraft's host against about 52 million, and a filter cannot give the objects it draws effects | `REQ-VC-006`, `REQ-VC-007` |
| M6.5 ✅ | `stitch export design.vectorcraft`, another name for `stitch plan`, which reads a VectorCraft document as it reads an SVG file. The commands are written in names. `stitch:ignore` leaves an object or a layer out, and `stitch:start` and `stitch:end` mark where a shape starts and ends, never stitched and named until M8 applies such hints | `REQ-VC-008`, `REQ-VC-009`; the end-to-end case `vectorcraft-patch` |
| M6.6 ✅ | The compatibility gate, complete. Level A and Level B run on all four tracks. In Level B, VectorCraft's own command line makes documents with the plug-ins' effects, and StitchCraft sews what it saved. An issue per track, a bot PR adopting each new release that passes, and the supported-versions page generated from the record that PR writes | first scheduled run green |
| M6.7 ✅ | UI screenshots pipeline on M0.8's window probe (`cargo xtask compat window`), each picture cropped to its menu or dialog; tutorial "A patch in VectorCraft"; xa11y drives VectorCraft to each screen and checks the plug-in's controls | `shots-ui` job green |
| MC-5 🧵 | **TS-12** (a design authored in VectorCraft) and **TS-10** again through the full workflow | report filed |

## M7 — More stitch types I

Zigzag stroke; E-stitch; S-stitch; zigzag satin; contour fill (three strategies, neck splitting); meander
fill with generated tiles; circular fill. One step per type with its requirements, then **MC-6 🧵**
(TS-11 sampler of the new types). With the satin methods come the warnings they all share: `SC-W0208`
for stitches turned more than 45° from the column, and `SC-W0209` for a column wider than 12 mm. The
zigzag stroke's and the meander fill's settings need a key with a kind or a range of a family's own, which
the registry gains first ([parameter registry](../design/params.md#naming-rules), rule 7).

Before contour fill relies on it, the offsetter meets the third criterion of
[ADR-0005](../design/adr/0005-geometry-stack.md): 300 insets of a region 150 mm across, of 1,000 points, in
under 200 ms. At M0.7 they took 218 to 448 ms, where GEOS took 104 to 149 ms.

## M8 — Ink/Stitch interoperability

SVG adapter v2:

- every `inkstitch:*` attribute in the [compatibility contract](../design/inkstitch-compat-contract.md)
- the command symbols not read yet, such as start and end points (`REQ-GEN-001`). The origin and stop
  position commands go into the design settings. Since M3 the adapter applies trim, stop and the ignore
  commands, and it never stitches commands, connectors or helper paths (`REQ-SVG-003`).
- clones (`<use>`)
- clip paths, as Ink/Stitch applies them: strokes and fills cut to the clip paths of the element and of
  the groups around it, with the region code from M5.1. Until then the adapter sews the whole shape, and
  `SC-W0802` says so.
- custom locks drawn as SVG paths, which the adapter reads for the engine. Until then they sew the half
  stitch, with `SC-W0503`.
- `stitch import-inkstitch file.svg -o file.vectorcraft`
- L3 differential conformance against a pinned Ink/Stitch with the deviations ledger

Ink/Stitch's font
library is part of that corpus: hundreds of real Ink/Stitch files, mostly satin, downloaded at a pinned
commit by `cargo xtask corpus` and never committed, using only fonts whose licence allows it (OFL, CC0,
CC-BY, CC-BY-SA; 132 of the 142 fonts on 2026-10-08).

## M10 — More stitch types II

Guided fill, linear gradient fill, ripple stitch, tartan fill, cross stitch, legacy fill; auto-run and auto-satin
routing tools; per-layer ordering (underlay now, top later). VectorCraft's tools filter with Convert Line to
Satin, once a column's centre line converts within ABI v1's budget, or ABI v2 gives heavy plug-ins more
([tools](../design/vectorcraft-integration.md#tools-vectorcrafts-own-commands)).

## M11 — Formats II

EXP, JEF, VP3, XXX, U01 writers and readers with goldens and oracle checks; thread catalogues
(Madeira, Isacord, Robison-Anton, …) with sourced tables; PES v6 thread lists if not done in M1.

## M12 — 1.0

Performance pass and budgets; docs completeness audit (every reference page has examples and images);
translations (UI Fluent catalogues, docs gettext); release automation (`release-plz`, signed binaries,
checksums); 24 h fuzz soak; **MC-7 🧵** full regression set on the machine.

## After 1.0

Lettering with fonts from a separate font repository (VectorCraft keeps fonts out of its tree too): our
own, Ink/Stitch's fonts whose licence allows it (OFL, CC0, CC-BY, CC-BY-SA, each with its licence file and
an `ASSETS.md` row, never NC, GPL or unclear terms; ADR-0001 is amended in that step), and fonts a user has
installed with Ink/Stitch, read in their own format; appliqué workflow, design splitting for small hoops, colour-change reordering, print worksheets, an
in-tree VectorCraft crate if wanted.

## M9 (VectorCraft ABI v2, upstream)

M9 comes last, after 1.0 and the work after it. 1.0 has the ABI v1 plug-in from M6, and nothing before M9
waits on ArtCraft's answer to the RFC. Its id stays M9, as every step id does.

Open the [RFC](../design/rfc-vectorcraft-abi-v2.md) with ArtCraft; one PR per accepted proposal (with
tests, following their `AGENTS.md`); then plug-in v2, with export to PES and DST from File › Export,
overlay previews, rich parameter dialogs and a machine profile per document.
