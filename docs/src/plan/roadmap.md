# Roadmap: small steps to the final project

Each **step** is one pull request: small, reviewable, with its tests, conformance cases and docs. Each
**milestone** ends with green gates and, where it produces something sewable, a **machine checkpoint**
(MC) on the Brother 200 × 200 mm machine with photos and measurements filed through the sew-out report
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
| M0.6 🧪 | VectorCraft "hello" plug-in: identity live effect built in CI for `wasm32-unknown-unknown`; Level A contract test against the pinned stable `vectorcraft-plugins` (v0.6.0, now v0.7.0), `release` and `main`; measure fuel per output byte | Contract test green in `compat.yml` (done: green on every track, 2026-10-08); budget numbers recorded in the integration doc (open) |
| M0.7 🧪 | Geometry spike ([ADR-0005](../design/adr/0005-geometry-stack.md) criteria) | ADR-0005 accepted or revised |
| M0.8 🧪 | Headless VectorCraft window in CI (Xvfb + Mesa) taking `ui.screenshot` twice with identical pixels; one [xa11y](https://xa11y.dev/) test that drives the same window through its accessibility tree (AT-SPI on Linux) | Decision recorded in the docs pipeline page, for both |
| M0.9 ✅ | Movable into VectorCraft ([ADR-0011](../design/adr/0011-movable-into-vectorcraft.md)): packages prefixed, manifests inheriting only what VectorCraft defines, tooling scoped to its folder and packages, `cargo xtask compat join [--nested]`, and a daily `move` job | `move` green against VectorCraft's latest release and `main` |
| M0.10 ✅ | Clean room, tightened: no document describes Ink/Stitch's code; Ink/Stitch is named only for file compatibility and in the independence notices; `cargo xtask cleanroom` scans documents too | `cleanroom` green over the whole repository |

## M1 — Stitch plan, PES/DST writers, first sew-out

Goal: sew a machine file StitchCraft wrote. No SVG yet: test sheets are generated in code.

| Step | Deliverable | Done when |
|---|---|---|
| M1.1 ✅ | `stitchcraft-core`: `Budget` and the diagnostics model (`Mm`, `Point`, `math` and `SplitMix64` with frozen reference values are already in the bootstrap) | unit tests; budget exhaustion is a diagnostic |
| M1.2 ✅ | `stitchcraft-plan`: `Stitch`, `StitchKind`, `ColorBlock`, `StitchPlan`; invariant checker (`REQ-PLAN-001..007`) | each invariant has a violating and a passing case |
| M1.3 ✅ | Machine profiles + `brother-200x200`; hoop and comfort diagnostics `SC-E0701`/`SC-W0702` with rotate hint | `REQ-PRF-001`, `REQ-PRF-002` active |
| M1.4 ✅ | Quantization (absolute, round-half-even); PEC stitch encoder with spec vectors | `REQ-FMT-002` partial; encoder unit tests |
| M1.5 ✅ | PES v1 writer: header, PEC header, palette indices, thumbnails | golden files (`REQ-FMT-001`) |
| M1.6 ✅ | Brother PEC palette table + CIEDE2000 nearest colour | `REQ-THREAD-001` spot checks |
| M1.7 ✅ | DST writer (header, ternary records, jump splitting, commands) | golden files; `REQ-FMT-007` |
| M1.8 ✅ | `stitch testsheet <TS-xx> --profile … -o file` (and `--list`), `stitch profiles`, `stitch explain`; the CLI moves to `clap` (`stitch inspect` needs the readers: M2.1) | `trycmd` examples in the docs |
| M1.9 ✅ | Conformance runner v1: requirements × cases matrix, L0 + L1 goldens, job summary | report visible on PRs |
| M1.10 ✅ | Docs: profile, format, test-sheet, diagnostics and command-line reference generated; tutorial "Your first sew-out" with real output | docs check green |
| MC-1 🧵 | Sew **TS-01** (orientation, scale), **TS-02** (colour changes, stops, both trim encodings, jumps), **TS-10A/B/C** (150 and 190 mm frames) from PES v1; same TS-01 from DST if the machine reads DST | Sew-out report filed; profile values confirmed or changed; PES v6 decision made |

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
| M3.8 | Plan assembly: order, collapse, travel, tie-off/jump/trim/tie-in (where `ties` says), stops, colour changes, commands | `REQ-ASM-001..005`, `REQ-LCK-001` |
| M3.9 | Finalize: split, merge, hoop, colour limits; `stitch plan in.svg -o out.pes --preview out.png --report out.json` | end-to-end cases |
| M3.10 | `stitch bug-report` bundle | a bundle reproduces its plan byte for byte |
| MC-2 🧵 | **TS-03** (running/bean lengths), **TS-04** (lock holding test), **TS-02** again with element-driven trims | report filed; min-stitch and lock defaults confirmed (`REQ-LCK-003`) |

## M4 — Satin column

| Step | Deliverable | Done when |
|---|---|---|
| M4.1 | Satin shape: rails/rungs recognition and validation, diagnostics `SC-E0201`, `SC-W0203`, `SC-E0204`, `SC-W0207` | `REQ-SAT-005` |
| M4.2 | Correspondence and centre-line sampling | `REQ-SAT-002` |
| M4.3 | Pull and push compensation (symmetric/asymmetric), random width | `REQ-SAT-001`, `REQ-SAT-006`, `REQ-SAT-007` |
| M4.4 | Short stitches on curves | case set on tight curves |
| M4.5 | Split stitches (simple, staggered, random) | `REQ-SAT-003` |
| M4.6 | Underlays: centre walk, contour, zigzag | `REQ-SAT-004` |
| M4.7 | Start/end nearest point; hints | `REQ-GEN-001` for satin |
| M4.8 | Single-path satin (centre line + width) | cases |
| MC-3 🧵 | **TS-05** (width ladder 1–10 mm × three densities), **TS-06** (underlay comparison) | report filed; satin defaults tuned |

## M5 — Tatami fill

| Step | Deliverable | Done when |
|---|---|---|
| M5.1 | Region normalization (fill rules, snapping, tiny rings `SC-W0303`) | degenerate corpus green |
| M5.2 | Rows: scan lines, half-open rule, gradient spacing, global grid | `REQ-FILL-TAT-002` |
| M5.3 | Needle grid and staggers | `REQ-FILL-TAT-003` |
| M5.4 | Boustrophedon cells, cell graph, ordering | `REQ-FILL-TAT-001` |
| M5.5 | Travel inside the region (visibility graph, A*), disconnected parts `SC-W0307` | `REQ-FILL-TAT-005`, `REQ-FILL-TAT-008` |
| M5.6 | Pull compensation by row extension | `REQ-FILL-TAT-006` |
| M5.7 | Underlay, gap-fill rows, tiny-region fallback `SC-W0305` | `REQ-FILL-TAT-007` |
| M5.8 | Coverage metric and performance budget | `REQ-FILL-TAT-004`, `REQ-FILL-TAT-009` |
| MC-4 🧵 | **TS-07** (fill + outline registration), **TS-08** (density ladder, angles), **TS-09** (underlay, travel) | report filed; fill defaults and pull compensation tuned |

## M6 — VectorCraft plug-in (ABI v1) and export from `.vectorcraft`

| Step | Deliverable | Done when |
|---|---|---|
| M6.1 | `stitchcraft-vectorcraft`: read `.vectorcraft` (layers, groups, paths, paints, effect records, transforms); size and depth caps | `REQ-VC-001`; fuzzed |
| M6.2 | Plug-in crate: manifests generated per family, ABI shim (`abi.rs`, audited `unsafe`) | `REQ-VC-002` (manifests valid, ≤ 64 params) |
| M6.3 | Live-effect previews with budget-aware levels | `REQ-VC-003` per level, within 5 s / fuel |
| M6.4 | Tools filter: make satin, add rung, bake preview, test sheet; Graphic Styles as presets verified | cases per tool |
| M6.5 | `stitch export design.vectorcraft`; marker conventions (`stitch:start`, `stitch:end`, `stitch:ignore`) | end-to-end cases |
| M6.6 | Compatibility gate complete: Level A + B, all four tracks, bot PR and issues, generated table | first scheduled run green |
| M6.7 | UI screenshots pipeline (from M0.8), tutorial "A patch in VectorCraft"; if the M0.8 trial held, xa11y drives VectorCraft to each screen and checks the plug-in's controls | `shots-ui` job green |
| MC-5 🧵 | **TS-12** (a design authored in VectorCraft) and **TS-10** again through the full workflow | report filed |

## M7 — More stitch types I

Zigzag stroke; E-stitch; S-stitch; zigzag satin; contour fill (three strategies, neck splitting); meander
fill with generated tiles; circular fill. One step per type with its requirements, then **MC-6 🧵**
(TS-11 sampler of the new types).

## M8 — Ink/Stitch interoperability

SVG adapter v2: every `inkstitch:*` attribute in the [compatibility contract](../design/inkstitch-compat-contract.md),
the command symbols not read yet (since M3 the adapter applies trim, stop and the ignore commands, and
never stitches commands, connectors or helper paths: `REQ-SVG-003`), clones (`<use>`), custom locks drawn
as SVG paths (the adapter reads the path for the engine; until then they sew the half stitch, with
`SC-W0503`); `stitch import-inkstitch file.svg -o file.vectorcraft`;
L3 differential conformance against a pinned Ink/Stitch with the deviations ledger. Ink/Stitch's font
library is part of that corpus: hundreds of real Ink/Stitch files, mostly satin, downloaded at a pinned
commit by `cargo xtask corpus` and never committed, using only fonts whose licence allows it (OFL, CC0,
CC-BY, CC-BY-SA; 132 of the 142 fonts on 2026-10-08).

## M9 — VectorCraft ABI v2 (upstream)

Open the [RFC](../design/rfc-vectorcraft-abi-v2.md) with ArtCraft; one PR per accepted proposal (with
tests, following their `AGENTS.md`); then plug-in v2: File › Export PES/DST, overlay previews, rich
parameter dialogs, document-level machine profile.

## M10 — More stitch types II

Guided fill, linear gradient fill, ripple stitch, tartan fill, cross stitch, legacy fill; auto-run and auto-satin
routing tools; per-layer ordering (underlay now, top later).

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
