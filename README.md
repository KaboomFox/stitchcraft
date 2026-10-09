<h1 align="center">StitchCraft</h1>

<p align="center"><b>Machine embroidery for <a href="https://github.com/storytold/vectorcraft">VectorCraft</a>, in pure Rust.</b><br>
Vector art in, machine files out — PES for Brother first, then DST and more.</p>

<p align="center">
  <img alt="Status: bootstrapped (M0)" src="https://img.shields.io/badge/status-M0%20bootstrapped-8a5cf6">
  <img alt="Pure Rust" src="https://img.shields.io/badge/pure-Rust-b83a24?logo=rust&logoColor=white">
  <img alt="License: MIT OR Apache-2.0" src="https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-555555">
</p>

StitchCraft brings machine embroidery — running and bean stitch, satin columns with underlay, tatami,
contour and meander fills, and more — to VectorCraft, ArtCraft's open-source Illustrator, as sandboxed
WebAssembly plug-ins plus a command-line tool. It reads the embroidery parameters Ink/Stitch stores in
SVG files, so designs move between the tools.

How it is built:

- **Never crashes.** Every file and parameter is untrusted input; no panics, no `unsafe`, bounded work,
  and every problem is a coded diagnostic with an explanation page.
- **Deterministic.** The same design produces the same file on Linux, macOS, Windows and the web.
- **Proven.** Behaviour is specified as numbered requirements and checked by a conformance suite on every
  pull request — and sewn on a real Brother machine at every milestone.
- **Docs that cannot drift.** Parameter, diagnostic, CLI and profile references and the pictures in the
  docs are generated from the code and regenerated in CI.
- **Survives VectorCraft releases.** The plug-in is tested daily against VectorCraft's stable releases,
  pre-releases, its `release` branch and `main`.

## Status

**M0 bootstrap committed:** the design, the roadmap and a workspace whose guardrails are already
enforced (`cargo xtask ci`). **Next:** the M0 spikes (VectorCraft hello plug-in, geometry, headless
screenshots), then **M1** — the stitch plan model and PES/DST writers, ending with the first sew-out on the
machine. See [ROADMAP.md](ROADMAP.md).

## Read more

| | |
|---|---|
| The whole design in 25 minutes | [Technical design document](docs/src/design/tdd.md) |
| How the code is organised | [Architecture](docs/src/design/architecture.md) |
| Small steps to the final project | [Roadmap](docs/src/plan/roadmap.md) |
| How we prove it works | [Conformance](docs/src/design/conformance.md) · [Machine testing](docs/src/plan/machine-testing.md) |
| How it fits VectorCraft | [Integration](docs/src/design/vectorcraft-integration.md) · [Compatibility gate](docs/src/design/compatibility-gate.md) |
| Contributing | [AGENTS.md](AGENTS.md) · [CONTRIBUTING.md](CONTRIBUTING.md) |

## Quick start (developers)

```sh
cargo xtask ci                      # all gates
cargo test --workspace              # tests only
cd docs && mdbook serve             # the documentation site, live
```

## Workspace

`crates/{stitchcraft-core, stitchcraft-params, stitchcraft-plan, stitchcraft-engine,
stitchcraft-formats, stitchcraft-render, stitchcraft-svg, stitchcraft-vectorcraft, stitchcraft-testkit}`
and `apps/{stitchcraft-cli, stitchcraft-vc-plugin}`, layered and enforced by `cargo xtask layers`.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. StitchCraft is a
clean-room implementation and contains no Ink/Stitch code; it uses Ink/Stitch's parameter names for file
interoperability ([NOTICE](NOTICE)).

<sub>StitchCraft is an independent project, not affiliated with Ink/Stitch, ArtCraft or Brother.
Brother and PES are trademarks of their respective owners.</sub>
