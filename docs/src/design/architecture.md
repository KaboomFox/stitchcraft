# Architecture

<!-- implements: crates/stitchcraft-engine/src/lib.rs, apps/stitchcraft-cli/src/*.rs, apps/stitchcraft-cli/src/commands/mod.rs, apps/stitchcraft-cli/src/commands/profiles.rs -->

This page is the map of the code: which crate owns what, which way dependencies point, and where a
new piece of code belongs. If you are about to add a file and are not sure where, the
[decision table](#where-does-my-code-go) at the end answers it.

## Principles

1. **The engine knows nothing about hosts.** It never sees SVG, VectorCraft, files or the network.
   Hosts adapt *their* documents into a `Design` and consume a `StitchPlan`.
2. **Pure core, effects at the edges.** Generators are deterministic functions of their inputs.
   File I/O, clocks, environment variables and terminals live only in apps and `xtask`.
3. **One source of truth per concept.** Parameters live in the registry, diagnostics in the
   diagnostics registry, machine profiles in profile data. Everything else (UI schemas, CLI flags,
   docs pages, test strategies) is generated from them.
4. **Layering is enforced, not suggested.** `cargo xtask layers` fails the build when a crate
   reaches up or sideways.
5. **Small modules with one job.** Files over 800 lines warn and over 1,500 fail
   (`cargo xtask filesize`); a stitch type is a module, not a branch in a big match.

## Crates and layers

```text
L3  stitchcraft-svg          stitchcraft-vectorcraft        (adapters)
      │                         │
L2  stitchcraft-engine ◀───────┘     stitchcraft-formats   stitchcraft-render
      │                                  │                     │
L1  stitchcraft-plan ◀──────────────────┴─────────────────────┘
      │
L0  stitchcraft-core ◀── stitchcraft-params

apps:  stitchcraft-cli (bin `stitch`)   stitchcraft-vc-plugin (wasm32 cdylib)
test:  stitchcraft-testkit (dev-dependency only)
tools: stitchcraft-xtask (`xtask/`, run as `cargo xtask`)
```

| Crate | Layer | Owns | Must not |
|---|---|---|---|
| `stitchcraft-core` | L0 | `Mm`, `Point`, `Vec2`, tolerances, deterministic math (`libm` wrappers), `SplitMix64` RNG, `Budget`, `Diagnostic`/`Code`/`Severity` | depend on any workspace crate |
| `stitchcraft-params` | L0 | `params!`, `ParamSpec`, `ParamSet`, value parsing and validation, the registry audit (the list of declarations is `stitchcraft_engine::registry`) | know any stitch algorithm |
| `stitchcraft-plan` | L1 | `Stitch`, `StitchKind`, `ColorBlock`, `StitchPlan`, `Thread`, palettes, `MachineProfile`, plan invariants | generate stitches; read/write files |
| `stitchcraft-engine` | L2 | `Design`/`Element`/`Shape`, normalization, generators, plan assembly | depend on formats, render or any adapter |
| `stitchcraft-formats` | L2 | readers/writers (PES/PEC, DST, …), quantization, format limits | depend on engine (it encodes *plans*) |
| `stitchcraft-render` | L2 | CPU preview images of plans (thread look, simple look, overlays) | depend on engine |
| `stitchcraft-svg` | L3 | SVG → `Design` (geometry, styles, `inkstitch:*` attributes, commands), plan → SVG | contain stitch logic |
| `stitchcraft-vectorcraft` | L3 | `.vectorcraft` → `Design`, effect records ↔ `ParamSet`, plug-in JSON objects | contain stitch logic |
| `stitchcraft-testkit` | test | fixtures, `proptest` strategies (from the registry), invariant assertions | be a normal dependency |
| `stitchcraft-cli` | app | the `stitch` command; all filesystem access | contain engine logic |
| `stitchcraft-vc-plugin` | app | VectorCraft ABI v1 shim + plug-in manifests | contain engine logic; `unsafe` outside `abi.rs` |
| `stitchcraft-xtask` | tools | gates (`ci`, `layers`, `docs`, `cleanroom`, …), generators, compatibility runs; works from its own folder, on StitchCraft's packages only ([ADR-0011](adr/0011-movable-into-vectorcraft.md)) | ship to users |

Intra-layer edges are allowed only where listed (`stitchcraft-params → stitchcraft-core`). The table
lives in `xtask/src/layers.rs` and is append-only, like VectorCraft's.

### Why this split

- **`plan` below `engine` and `formats`** lets the formats crate encode and decode plans without
  pulling in geometry and generators; a file converter (`stitch convert a.dst b.pes`) links only L0–L2
  formats code.
- **`params` at L0** because both the engine (typed views) and the adapters (attribute mapping) need
  it, and because docs/UI generation must not depend on algorithms.
- **Adapters at L3** so the engine can be tested without any host, and a new host (another editor,
  a web service) is one crate, not a refactor.
- **Two apps** because the CLI needs the filesystem and threads while the plug-in must stay inside a
  sandbox with no imports.

## External dependencies

Dependencies are added deliberately and recorded in the crate's `README` section "Dependencies".
`cargo-deny` enforces the licence allow-list (MIT, Apache-2.0, BSD-2/3, ISC, Zlib, Unicode) and denies
GPL/AGPL/LGPL.

| Need | Crate | Licence | Where |
|---|---|---|---|
| Curves, affine (strokes are flattened by the engine, for determinism) | `kurbo` 0.13 | MIT/Apache | core, engine, adapters |
| Polygon booleans, offsets | `i_overlay` 9 | MIT/Apache | engine |
| Spatial index | `rstar` 0.13 | MIT/Apache | engine |
| Graphs | `petgraph` 0.8 | MIT/Apache | engine |
| Deterministic transcendental math | `libm` 0.2 | MIT | core |
| Errors | `thiserror` 2 | MIT/Apache | all libraries |
| Serialization | `serde`, `serde_json`, `toml` | MIT/Apache | params, adapters, xtask |
| XML / SVG syntax | `roxmltree`, `svgtypes` | MIT/Apache | svg |
| Rasterizing previews | `tiny-skia` 0.12 | BSD-3-Clause | render |
| CLI parsing | `clap` 4 | MIT/Apache | cli, xtask |
| Tests | `proptest`, `insta`, `trycmd` | MIT/Apache, Apache | dev only |

Versions above are the current ones (checked 2026-10-08 with `cargo info`); `Cargo.lock` is the
truth. The geometry choices are validated by spike M0.7 ([ADR-0005](adr/0005-geometry-stack.md)).

## Hosts: ports and adapters

The engine exposes one entry point and one input type:

```rust,ignore
pub fn plan(design: &Design, profile: &MachineProfile, budget: &Budget) -> PlanOutcome;   // since M3.8

pub struct PlanOutcome {
    pub plan: Option<StitchPlan>,   // None when nothing could be sewn
    pub diagnostics: Vec<Diagnostic>,
    // per-element groups, for previews and reports, arrive with the VectorCraft plug-in (M6)
}
```

The machine profile is an input because the shortest stitch a generator may sew is the machine's, and
finalizing (M3.9) fits the plan to the machine's longest stitch and hoop.

and a per-element entry point used by live previews:

```rust,ignore
pub fn stitch_element(element: &Element, context: &ElementContext, budget: &Budget) -> ElementOutcome;
```

Adapters only translate:

| Adapter | From | To |
|---|---|---|
| `stitchcraft-svg` | SVG document (+ `inkstitch:*` attributes, command symbols) | `Design` |
| `stitchcraft-vectorcraft` | `.vectorcraft` JSON, plug-in input objects, effect records | `Design` / `Element` |
| `stitchcraft-vc-plugin` | ABI v1 call (JSON in linear memory) | `stitch_element` → preview geometry JSON |
| `stitchcraft-cli` | files and flags | `plan` → formats/render/report |

A future in-tree VectorCraft crate would be a fifth adapter calling the same two functions.

## Errors and diagnostics

- **Diagnostics** describe problems with the user's design (a satin with three rails, a fill smaller
  than its row spacing, a design larger than the hoop). They are values, collected, coded and shown;
  planning continues where it can.
- **Errors** (`Result<_, XxxError>`) describe failures to do the work (a file is truncated, a budget is
  exhausted, an I/O failure). Each crate has one `thiserror` enum; apps turn errors into diagnostics
  or exit codes.
- Nothing below the apps prints, exits or panics.
- **A bug in StitchCraft** is neither. It shows as a failed plan check (`SC-E0009`) or as a panic despite
  the lints. The command line catches it and ends with exit status 4. For `stitch plan` it also writes a
  bug-report bundle that reproduces the bug ([determinism](determinism.md#bug-reports)).

## Feature flags

Kept minimal: `stitchcraft-engine/experimental` exposes generators whose status is `experimental`
in the registry (they appear in docs with a badge); `stitchcraft-formats/<format>` flags exist only if
a format pulls a heavy dependency (none planned). No flag may change the output of a stable generator.

## WebAssembly constraints

Everything at L0–L3 must build for `wasm32-unknown-unknown` (checked by `cargo xtask wasm`): no
threads, no filesystem, no clocks, no `std::env`, no randomness from the OS. The plug-in is built
with `opt-level = "z"`, LTO and `panic = "abort"`, which is one more reason the no-panic rule is
absolute: in wasm a panic aborts the module and VectorCraft reports the plug-in as failed.

## Where does my code go?

| You are adding… | Put it in | Also touch |
|---|---|---|
| A new stitch type | `crates/stitchcraft-engine/src/generators/<name>/` | registry params, diagnostics, conformance cases, docs page — see the [playbook](../contributing/playbook-new-stitch-type.md) |
| A parameter | the generator's `params!` block | nothing else by hand: docs and schemas regenerate; a new `…Params` declaration also gets a line in `stitchcraft_engine::registry::PARAMETERS` — see the [playbook](../contributing/playbook-new-param.md) |
| A machine format | `crates/stitchcraft-formats/src/<format>/` | [format playbook](../contributing/playbook-new-format.md) |
| A machine profile | `crates/stitchcraft-plan/src/profiles/` (data) | machine-testing record |
| A diagnostic | the diagnostics registry in `stitchcraft-core` | explanation text + a case that triggers it |
| SVG/Ink/Stitch parsing | `stitchcraft-svg` | compatibility contract row |
| VectorCraft document reading | `stitchcraft-vectorcraft` | compatibility gate scenario |
| A CLI subcommand | `apps/stitchcraft-cli/src/commands/` | `trycmd` example in the docs |
| A repo check | `xtask/src/` | `cargo xtask ci` step list and the [guardrails](guardrails.md) page |
