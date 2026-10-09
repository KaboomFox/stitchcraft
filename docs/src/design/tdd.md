# StitchCraft — Technical Design Document

| | |
|---|---|
| **Status** | Draft 0.1 — design approved for M0/M1, later sections refined as milestones land |
| **Date** | 2026-10-08 |
| **Scope** | A Rust machine-embroidery engine and its integration with VectorCraft |
| **Reading time** | ~25 minutes; every section links to the detailed design it summarizes |

StitchCraft turns vector art into machine embroidery. It is two things that ship together:

1. **An engine** — host-agnostic Rust crates that turn shapes plus embroidery parameters into a
   *stitch plan* and encode it as machine files (PES for Brother first, then DST, EXP, JEF, VP3 …).
2. **A VectorCraft plug-in** — the engine inside
   [VectorCraft](https://github.com/storytold/vectorcraft), ArtCraft's open-source, clean-room
   Illustrator written in Rust, so embroidery is designed where the art is drawn.

It covers the common machine-embroidery stitch types (running, bean, satin, tatami fill, contour,
meander, ripple, …) and reads the embroidery parameters Ink/Stitch stores in SVG files, so designs move
between the tools. Its quality bar: **it never crashes, it produces the same file on every platform,
and every behaviour is pinned by a conformance suite and by sew-outs on a real machine.**

---

## 1. Goals

| ID | Goal | How we know |
|---|---|---|
| G1 | Read and write every parameter in the Ink/Stitch compatibility contract, in phases | [Compatibility contract](inkstitch-compat-contract.md) coverage column reaches 100 % for phase P1–P3 rows |
| G2 | Never crash on any input | No-panic lints, fuzzing, bounded work budgets ([guardrails](guardrails.md), [robustness](#11-robustness)) |
| G3 | Correctness proven, not assumed | Every requirement has a passing conformance case; machine checkpoints signed off ([conformance](conformance.md), [machine testing](../plan/machine-testing.md)) |
| G4 | Determinism | Byte-identical files on Linux, macOS, Windows and wasm32 ([determinism](determinism.md)) |
| G5 | Host-agnostic engine | VectorCraft plug-in, CLI and (later) an in-tree VectorCraft crate share one engine ([architecture](architecture.md)) |
| G6 | Documentation that cannot drift from the code | Reference generated from code, images regenerated in CI on every PR ([docs pipeline](docs-pipeline.md)) |
| G7 | Code we are proud to show | Enforced layering, small modules, playbooks for contributors ([guardrails](guardrails.md)) |
| G8 | Survive VectorCraft releases | Plug-in tested against VectorCraft stable, pre-releases, `release` and `main` ([compatibility gate](compatibility-gate.md)) |

### Non-goals for 1.0

- An Inkscape extension.
- Lettering with a bundled embroidery-font library (post-1.0; see [roadmap](../plan/roadmap.md)).
- Automatic digitizing of photos or bitmaps.
- Sending files to machines over USB/Wi-Fi; the output is a file you copy to the machine.
- A standalone GUI. The GUI is VectorCraft; the CLI renders previews to PNG.

## 2. Target machine and the first user

The first production target is a **Brother home embroidery machine with a 200 × 200 mm (8 × 8 in)
hoop**, loading **PES** files. Designs are normally kept to **about 150 mm (6 in)**. This becomes the
built-in profile `brother-200x200`:

| Profile field | Value | Behaviour |
|---|---|---|
| Hoop (hard limit) | 200 × 200 mm | Error `SC-E0701` if the plan does not fit |
| Comfort zone | 150 × 150 mm | Warning `SC-W0702` if the design is larger |
| File format | PES v1 with PEC block | Changed only after a machine checkpoint proves another works |
| Max stitch length | 12.0 mm (split above) | Refined at MC-1 |
| Min stitch length | 0.3 mm (merged below) | Refined at MC-2 |
| Trims | PEC trim-flagged jumps | Verified at MC-1 (some models honour trims, others trim on long jumps) |
| Thread palette | Brother PEC palette (nearest colour, CIEDE2000) | — |

Profiles are data (see [data model](data-model.md#machine-profiles)); adding a machine never touches
the engine.

## 3. Background

### 3.1 File compatibility with Ink/Stitch

Ink/Stitch, a GPL-3.0 extension for Inkscape, stores its embroidery settings in SVG files as
`inkstitch:<name>` attributes. StitchCraft reads and writes the same attribute names, values and
defaults, so a design moves between the tools unchanged. Those names, types and defaults are facts
listed in the [compatibility contract](inkstitch-compat-contract.md); where StitchCraft's behaviour
differs on purpose, the deviations ledger (`conformance/deviations.toml`) says how and why. StitchCraft
is an independent project and contains no Ink/Stitch code, documentation text or data files ([ADR-0001](adr/0001-license-and-clean-room.md)).

### 3.2 VectorCraft, the host

VectorCraft (MIT OR Apache-2.0, Rust 2024, ~290,000 lines) is a clean-room Illustrator. What matters
here (details and file references in [VectorCraft integration](vectorcraft-integration.md)):

- **Plug-in ABI v1**: sandboxed WebAssembly modules run by `wasmi` with no imports. Two kinds:
  *object filters* (one undo step) and *live effects* (rewrite one object's geometry whenever it is
  drawn). Parameters come from a manifest schema (number, int, bool, choice; at most 64).
- **Live-effect parameters are stored per object** in the document as `{"id": "plugin.<id>",
  "params": {…}}` — so embroidery settings can live on the art today, with no VectorCraft change.
- Limits that shape our design: 50 M instructions per call (+4,000 per input byte), 5 s per
  live-effect run, input is geometry only (no paint), no file-format plug-ins, no panels.
- Everything is a command, reachable from the UI, a JSON control channel, an MCP server and a
  headless CLI (`vectorcraft-cli run --in f --cmd … --export out.png`), which our tests and
  screenshot pipeline drive.

### 3.3 Licence and clean room

Ink/Stitch is GPL-3.0; VectorCraft forbids GPL code in its tree. StitchCraft is **MIT OR
Apache-2.0** and is written **clean-room**: the design docs in this folder describe *behaviour*
(inputs, outputs, parameters, published algorithms); implementers work from these docs and public
references and never read or transliterate Ink/Stitch source. Parameter names are kept identical to
Ink/Stitch's SVG attributes because that is the interoperability contract. See
[ADR-0001](adr/0001-license-and-clean-room.md).

## 4. Requirements

### 4.1 Functional scope by phase

| Capability | Phase | Milestone |
|---|---|---|
| Stitch plan, Brother profile, PES + DST writers, test sheets | P1 | M1 |
| PES/DST readers, preview renderer, fuzzing | P1 | M2 |
| Running stitch (repeats, bean, random length), manual stitch, lock stitches, trims, stops, colour changes, SVG input | P1 | M3 |
| Satin column (rails/rungs, underlays, pull/push compensation, short and split stitches) | P1 | M4 |
| Tatami fill (underlay, pull compensation, staggers, gap fill, underpath travel) | P1 | M5 |
| VectorCraft plug-in (ABI v1), export from `.vectorcraft`, compatibility gate | P1 | M6 |
| Zigzag/E/S stitches, contour, meander, circular fill | P2 | M7 |
| Ink/Stitch SVG import (attributes, commands, clones) and differential testing | P2 | M8 |
| VectorCraft ABI v2 (upstream RFC): export inside VectorCraft, rich parameter panels, overlay preview | P2 | M9 |
| Guided, gradient, ripple, tartan, cross stitch; auto-run/auto-satin | P3 | M10 |
| EXP, JEF, VP3, XXX, U01 formats; thread catalogues | P3 | M11 |
| 1.0 hardening, translations, release automation | — | M12 |

The full milestone and step list is in [the roadmap](../plan/roadmap.md).

### 4.2 Non-functional requirements

| ID | Requirement |
|---|---|
| NFR-ROB-1 | No panics in shipped code; no `unsafe` outside the one audited plug-in ABI module |
| NFR-ROB-2 | All work is bounded by explicit budgets (stitches, iterations, input sizes); exceeding one is a diagnostic, never a hang |
| NFR-DET-1 | Same input and version ⇒ byte-identical output on every supported platform, including wasm32 |
| NFR-PERF-1 | A 150 × 150 mm tatami fill at 0.4 mm spacing plans in ≤ 50 ms natively (baseline set at M5, then guarded) |
| NFR-PERF-2 | Live-effect previews fit VectorCraft's v1 budget (≤ 50 M instructions, ≤ 5 s) for a 150 mm design |
| NFR-PORT-1 | Engine crates build for `wasm32-unknown-unknown` without WASI |
| NFR-DOC-1 | 100 % of parameters, diagnostics, CLI commands and formats have generated reference pages |
| NFR-TEST-1 | Every requirement in `conformance/requirements.toml` has ≥ 1 passing case before its milestone closes |
| NFR-SEC-1 | Every parser of untrusted bytes (SVG, `.vectorcraft`, PES, DST, …) is fuzzed and resource-capped |

## 5. Architecture

```text
 Hosts / adapters                   Engine                                      Outputs
┌────────────────────┐      ┌───────────────────────────────────────┐      ┌──────────────────┐
│ SVG (Ink/Stitch)   │─┐    │ normalize ─▶ validate ─▶ generate     │      │ PES / DST / …    │
│ .vectorcraft file  │─┼──▶ │   per element (StitchGroup)           │ ──▶  │ PNG preview      │
│ VectorCraft plug-in│─┘    │ ─▶ assemble plan (order, travel,      │      │ JSON report      │
└────────────────────┘      │    ties, trims, colours) ─▶ finalize  │      │ plug-in geometry │
        Design              │    (profile limits) ─▶ StitchPlan     │      └──────────────────┘
                            └───────────────────────────────────────┘
                                    + Diagnostics at every stage
```

The workspace is split into layered crates; a crate may depend only on lower layers, enforced by
`cargo xtask layers`:

| Layer | Crate | Responsibility |
|---|---|---|
| L0 | `stitchcraft-core` | Units (mm), points, deterministic math and RNG, diagnostics model |
| L0 | `stitchcraft-params` | The parameter registry: the single source of truth for every parameter |
| L1 | `stitchcraft-plan` | Stitch plan model, machine profiles, thread palettes, plan invariants |
| L2 | `stitchcraft-engine` | Design model, stitch generators, plan assembly |
| L2 | `stitchcraft-formats` | Machine file readers and writers |
| L2 | `stitchcraft-render` | Deterministic CPU preview renderer (docs, CLI, conformance) |
| L3 | `stitchcraft-svg` | SVG and Ink/Stitch-attribute adapter |
| L3 | `stitchcraft-vectorcraft` | `.vectorcraft` document adapter and effect-record mapping |
| test | `stitchcraft-testkit` | Fixtures, property-test strategies, invariant checkers |
| app | `stitchcraft-cli` (`stitch`) | Command-line tool |
| app | `stitchcraft-vc-plugin` | VectorCraft WebAssembly plug-ins |

Details, dependency rules and the reasoning are in [architecture](architecture.md).

## 6. Key decisions

| ADR | Decision |
|---|---|
| [0001](adr/0001-license-and-clean-room.md) | MIT OR Apache-2.0, clean-room from Ink/Stitch; keep its parameter names for interoperability |
| [0002](adr/0002-host-agnostic-engine-plugin-first.md) | Host-agnostic engine; VectorCraft ABI v1 plug-in first, ABI v2 upstream, in-tree crate optional |
| [0003](adr/0003-parameter-registry.md) | One parameter registry generates UI schemas, CLI, docs, SVG mapping and test strategies |
| [0004](adr/0004-determinism.md) | Determinism by construction: ordered collections, `libm`, seeded RNG, absolute quantization |
| [0005](adr/0005-geometry-stack.md) | `kurbo` for curves, `i_overlay` for polygon booleans/offsets, `petgraph` + own Eulerian walk, `rstar` |
| [0006](adr/0006-docs-mdbook-diataxis.md) | mdBook, Diátaxis structure, generated reference, CI-regenerated images |
| [0007](adr/0007-pes-first-brother-profile.md) | PES v1 first for the Brother 200 × 200 mm machine; DST second |
| [0008](adr/0008-conformance-first.md) | Conformance-first development: requirement IDs and cases before code |
| [0009](adr/0009-adopt-vectorcraft-conventions.md) | Adopt VectorCraft's proven conventions; improve the ones that drift |
| [0010](adr/0010-diagnostics-with-codes.md) | Every user-facing problem is a coded diagnostic with an explanation page |

## 7. Data model (summary)

Full types in [data model](data-model.md).

- **Units**: millimetres as `f64`, y pointing down (same as SVG and VectorCraft). Adapters convert
  (`pt × 25.4/72`, SVG user units through the viewBox). Encoders quantize absolute positions to
  0.1 mm once, so long designs never drift.
- **Design** (engine input): ordered `Element`s — `id` (stable host id), `shape` (`Region`, `Path` or
  `Satin`), `thread`, `params` (`ParamSet`), `commands` (start/end points, trim, stop, ignore) — plus
  design settings (collapse length, minimum stitch length, origin) and a machine profile.
- **StitchPlan** (engine output): colour blocks of stitches; every stitch has a position, a kind
  (`Normal`, `Jump`, `Trim`, `Stop`, `ColorChange`, `End`) and provenance (element id and role:
  underlay, top, travel, lock) used by previews, reports and diagnostics.
- **Diagnostics**: `SC-E…` errors and `SC-W…` warnings with element, location, message and a fix
  hint; each code has a generated explanation page ([diagnostics](diagnostics.md)).

## 8. Engine pipeline (summary)

Detailed in [engine pipeline](engine-pipeline.md).

1. **Normalize** each element: flatten curves to tolerance, make regions valid under their fill
   rule, drop degenerate pieces (with a diagnostic), convert units.
2. **Validate** parameters against the registry (type, range, applicability) and shapes against the
   stitch type (e.g. a satin needs two rails).
3. **Generate** a `StitchGroup` per element with its stitch type's generator. Generators are pure
   functions of (shape, typed params, seed, budget) — no globals, no I/O.
4. **Assemble** the plan: keep document order, connect groups (travel inside a region, or tie-off →
   jump/trim → tie-in), insert colour changes and stops, apply commands.
5. **Finalize** against the profile: split stitches above the maximum, merge below the minimum,
   check the hoop and colour limits.
6. **Check invariants** (the same checker the conformance suite uses) before any file is written.

## 9. Stitch generators (summary)

| Generator | Phase | Design |
|---|---|---|
| Running, bean, manual, random-length | P1 | [strokes](algorithms/strokes.md) |
| Satin column (+ E, S, zigzag variants) | P1/P2 | [satin](algorithms/satin.md) |
| Tatami fill | P1 | [fills](algorithms/fills.md#tatami-fill) |
| Contour, meander, circular | P2 | [fills](algorithms/fills.md) |
| Guided, linear gradient, tartan, cross stitch | P3 | [fills](algorithms/fills.md) |
| Ripple, zigzag stroke | P2/P3 | [strokes](algorithms/strokes.md) |
| Lock stitches, travel, ordering | P1 | [engine pipeline](engine-pipeline.md#4-plan-assembly) |

Each generator is designed from public embroidery practice and published algorithms (for example
boustrophedon cell decomposition for fill routing and Connected Fermat Spirals for spiral fills),
cited in its page.

## 10. Machine formats

PES (v1 header + PEC block) and DST first, then EXP, JEF, VP3, XXX, U01. Writers quantize once,
encode commands per the format's rules and validate against limits (displacement per record, colour
count). Readers are bounded and fuzzed. pyembroidery (MIT) is used in CI as an independent oracle:
it must read our files back to the same stitches. Details: [formats](formats.md).

## 11. Robustness

- Shipped code has no `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!`, `unimplemented!`, and no
  unchecked indexing on data; enforced by workspace lints (adopted from VectorCraft).
- Every generator receives a **work budget** (stitch count, iterations); running out returns a
  diagnostic and the partial-free result "nothing for this element", never a hang. Budgets are
  counted in work units, not wall time, so they are deterministic.
- Degradation is explicit: when a fill is too small for rows, the outline fallback is emitted *with*
  `SC-W0305`; when travel cannot stay inside a region, the plan uses tie-off + trim + tie-in and says
  so — never a silent straight line across the design.
- Parsers cap sizes and counts before allocating; files are untrusted.

## 12. Determinism

Ordered collections only (`BTreeMap`, `Vec`), transcendental functions from `libm`, a documented
seeded PRNG (SplitMix64) with seeds derived from element id and the `random_seed` parameter,
absolute quantization at encode time, and a CI job that compares output hashes across platforms.
Details: [determinism](determinism.md).

## 13. VectorCraft integration (summary)

Three phases (full design in [VectorCraft integration](vectorcraft-integration.md)):

1. **ABI v1, no upstream change (M6).** Plug-ins `dev.stitchcraft.running`, `.satin` and `.fill` are
   *live effects*: applying one stores the embroidery parameters on the object and draws a lightweight
   stitch preview. `dev.stitchcraft.tools` is an *object filter* (make satin, add rung, bake preview).
   Machine files come from `stitch export design.vectorcraft`. (The namespace is one constant, changed
   in one place once the project has a domain.)
2. **ABI v2, proposed upstream (M9).** Exporter plug-ins, rich parameter schemas (labels, units,
   groups, conditions), a display-only overlay channel for real stitch previews, budget classes and
   document-level plug-in data. Written as [an RFC](rfc-vectorcraft-abi-v2.md) for the ArtCraft team;
   every item is generic, not embroidery-specific.
3. **In-tree crate, optional.** Because the engine is MIT/Apache and host-agnostic, VectorCraft (or
   a fork) can embed it natively with panels and a simulator.

## 14. Testing and conformance (summary)

Conformance-first ([conformance](conformance.md)): requirements have IDs; cases are data files;
the runner reports a requirement × case matrix.

| Level | What | Where it runs |
|---|---|---|
| L0 | Plan invariants on every generated plan | every PR |
| L1 | Format conformance: golden bytes, round trips, pyembroidery oracle, fuzzing | PR + nightly |
| L2 | Generator properties: containment, coverage, spacing, determinism, monotonicity | every PR |
| L3 | Ink/Stitch differential metrics on a corpus (pinned Ink/Stitch as a black-box oracle) | nightly |
| L4 | Physical sew-outs on the Brother machine, recorded through an issue form | milestone gates |

Plus unit tests, property tests (`proptest`), snapshot tests (`insta`), CLI-in-docs tests
(`trycmd`), mutation testing (`cargo-mutants`) and coverage ratchets.

## 15. Documentation (summary)

mdBook site organised by Diátaxis (tutorials, how-to, reference, explanation) plus these design
docs. Reference pages are **generated from the code** and CI fails if they are stale; stitch images
are rendered by our renderer from conformance fixtures; VectorCraft UI screenshots are produced by
scripted scenarios through VectorCraft's control channel and compared on every PR. A label on the PR
regenerates them. Details: [docs pipeline](docs-pipeline.md).

## 16. Security and supply chain

`cargo-deny` (licences allow-list with GPL denied, advisories, duplicate versions), `unsafe` forbidden
outside the audited ABI module, fuzzed parsers, pinned toolchain, Dependabot for crates and Actions,
least-privilege workflow tokens.

## 17. Release and versioning

SemVer per crate; the plug-in's version and its VectorCraft ABI version are reported in its
manifest. Changelogs from conventional commit scopes (`release-plz`, M12). Plug-in `.wasm` files and
CLI binaries are attached to GitHub Releases with checksums. A release requires: green gates,
compatibility gate green for the current VectorCraft stable, and the machine checkpoint for that
milestone signed off.

## 18. Risks

| ID | Risk | Mitigation |
|---|---|---|
| R1 | PES header fields are partly undocumented; the machine could reject a file | PES v1 minimal header; pyembroidery oracle; MC-1 in the first milestone |
| R2 | ABI v1 budgets too small for full previews | Budget-aware coarse previews; full fidelity in CLI; ABI v2 overlay proposal |
| R3 | ArtCraft declines ABI v2 | Plug-in still works on v1; optional in-tree integration in a fork |
| R4 | Polygon offset/boolean robustness | Spike M0.7; input sanitizing; fuzzing; fallback to diagnostics, never panics |
| R5 | Clean-room discipline slips | Process in ADR-0001; `cargo xtask cleanroom`; review checklist |
| R6 | Cross-platform float differences | `libm`, no FMA-sensitive formulas in quantization, cross-platform hash CI |
| R7 | Scope: the compatibility contract is large | Phases; compatibility contract tracks coverage; post-1.0 items explicit |
| R8 | Brother trim behaviour varies by model | Profile flag; MC-1 tests both encodings |
| R9 | VectorCraft breaking changes | Compatibility gate on stable, pre-release, `release` and `main` |
| R10 | Headless UI screenshots need a GPU | Spike M0.8 (Xvfb + Mesa); fallback: CLI-rendered art + semantic panel tests |

## 19. Open questions

1. Exact Brother model (affects PES version and trim support) — answered at MC-1 at the latest.
2. Final project name and plug-in id namespace (working name StitchCraft; plug-in ids derive from one
   constant).
3. ArtCraft's appetite for ABI v2 and for embroidery in-tree — ask on their Discord before M9.
4. Where the repository lives on GitHub (needed for the docs site URL and badges).
