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
| `stitchcraft-engine` | L2 | `Design`/`Element`/`Shape`, automatic digitizing, normalization, generators, plan assembly, finalizing for the machine | depend on formats, render or any adapter |
| `stitchcraft-formats` | L2 | readers/writers (PES/PEC, DST, …), quantization, format limits | depend on engine (it encodes *plans*) |
| `stitchcraft-render` | L2 | CPU preview images of plans (thread look, simple look, overlays) | depend on engine |
| `stitchcraft-svg` | L3 | SVG → `Design` (geometry, styles, `inkstitch:*` attributes, commands), plan → SVG | contain stitch logic |
| `stitchcraft-vectorcraft` | L3 | `.vectorcraft` → `Design` (commands in names included), effect records ↔ `ParamSet`, plug-in JSON objects | contain stitch logic |
| `stitchcraft-testkit` | test | fixtures, `proptest` strategies (from the registry), invariant assertions | be a normal dependency |
| `stitchcraft-cli` | app | the `stitch` command; all filesystem access, and the one program StitchCraft starts, VectorCraft's command line, which traces a picture ([ADR-0015](adr/0015-pictures-traced-by-vectorcraft.md)) | contain engine logic |
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
| Deterministic transcendental math | `libm` 0.2 | MIT | core |
| Errors | `thiserror` 2 | MIT/Apache | all libraries |
| XML / SVG syntax | `roxmltree`, `svgtypes` | MIT/Apache | svg |
| `.vectorcraft` documents: JSON, its coordinates read exactly, unpacked from gzip by a Rust backend | `serde`, `serde_json`, `flate2` 1 | MIT/Apache | vectorcraft |
| Rasterizing previews, and reading VectorCraft's pictures to compare them | `tiny-skia` 0.11 | BSD-3-Clause | render, xtask |
| JSON reports, and the plug-in's JSON | `serde_json` | MIT/Apache | cli, vc-plugin |
| Content hashes of bug-report bundles | `sha2` 0.11 | MIT/Apache | cli |
| CLI parsing | `clap` 4 | MIT/Apache | cli, xtask |
| Tooling's TOML and JSON | `toml`, `toml_edit`, `serde_json` | MIT/Apache | xtask |
| Tests | `proptest`, `insta` | MIT/Apache, Apache | dev only |

The engine's geometry is its own, with no crate for curves, regions, offsets, nearest points or graphs:
spike M0.7 accepted that ([ADR-0005](adr/0005-geometry-stack.md)). Versions above are those
`Cargo.toml` asks for (2026-10-17), and `Cargo.lock` is the truth.

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

and a per-element entry point, which `plan` calls for each element and live previews call for one:

```rust,ignore
pub fn generate(element: &Element, settings: &DesignSettings, profile: &MachineProfile,
                neighbours: &Neighbours, budget: &Budget) -> Generation;   // since M3.8

pub struct Generation {
    pub generated: Option<Generated>,   // the element's stitch groups; None when it is skipped
    pub diagnostics: Vec<Diagnostic>,
}
```

A live preview sees one object, so it has no neighbours, and it sews on the reference machine.

Adapters only translate:

| Adapter | From | To |
|---|---|---|
| `stitchcraft-svg` | SVG document (+ `inkstitch:*` attributes, command symbols) | `Design` |
| `stitchcraft-vectorcraft` | `.vectorcraft` JSON (+ commands in names), plug-in input objects, effect records | `Design` / `Element` |
| `stitchcraft-vc-plugin` | ABI v1 call (JSON in linear memory) | `generate` → preview geometry JSON |
| `stitchcraft-cli` | files and flags, a picture traced by VectorCraft's command line | `plan` → formats/render/report |

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
  bug-report bundle that reproduces the bug ([determinism](determinism.md#bug-reports)). A VectorCraft
  exporter declines with it, so VectorCraft doesn't write the file.
- **The command line's own promises.** A command refuses outputs that name the same file as each other,
  or as a file it reads (`REQ-CLI-007`), and writes each file under a temporary name of its own, created
  new and synced before it is renamed into place (`REQ-CLI-006`). Built for WASI, as
  `cargo xtask wasm --goldens` runs it, a program has no process id, so there the names count from 0. What it prints of a file it wrote is the
  file read back (`REQ-CLI-008`, `stitchcraft_formats::read_back`, which the VectorCraft exporters call too). A closed standard output keeps a command's exit status, any other error
  writing it ends with 3 (`REQ-CLI-009`), and help and errors are coloured only on a terminal
  (`REQ-CLI-010`).

## Feature flags

Kept minimal: `stitchcraft-engine/experimental` exposes generators whose status is `experimental`
in the registry (they appear in docs with a badge); `stitchcraft-formats/<format>` flags exist only if
a format pulls a heavy dependency (none planned). No flag may change the output of a stable generator.

## WebAssembly constraints

Everything in layers L0 to L3 must build for `wasm32-unknown-unknown`, which `cargo xtask wasm` checks. That target
has no threads, file system, clocks, environment, processes or randomness from the OS, and std's calls
for them build there but fail or panic when run, so `clippy.toml` keeps library crates from calling
them. The plug-in is built
in release with the workspace's profile (thin LTO, one codegen unit), and on `wasm32-unknown-unknown`
a panic aborts. That is one more reason the no-panic rule is absolute: the module stops, and
VectorCraft reports the plug-in as failed.

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
