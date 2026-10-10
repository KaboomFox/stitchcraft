<h1 align="center">StitchCraft</h1>

<p align="center"><b>Machine embroidery for <a href="https://github.com/storytold/vectorcraft">VectorCraft</a>, in pure Rust.</b><br>
Vector art in, machine files out — PES for Brother first, then DST and more.</p>

<p align="center">
  <a href="https://kaboomfox.github.io/stitchcraft/dev/"><img alt="Documentation" src="https://img.shields.io/badge/docs-user%20guide-8a5cf6"></a>
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

StitchCraft sews the strokes of an SVG design in running stitch, bean stitch or manual stitch, with lock
stitches, trims and stops, and writes PES and DST files. It reads, draws and converts PES, PEC and DST
files from other software. The engine sews satin columns, their underlays first, then their top stitches
with compensation, short stitches and split stitches. The rest of milestone M4 decides where a column
starts and ends, and sews columns drawn as a centre line. An SVG's satin columns are sewn once `stitch
plan` reads Ink/Stitch's settings, in M8. Fills come in M5.
[ROADMAP.md](ROADMAP.md) is the status board.

## Use it

Build the `stitch` command from source ([install](docs/src/user/install.md)), then plan a design:

```sh
stitch plan design.svg -o design.pes --preview design.png
```

The [user guide](https://kaboomfox.github.io/stitchcraft/dev/user/) has tutorials, how-to guides and a
page for each stitch type.

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

Dual-licensed under [MIT](LICENSE-MIT) or [Apache-2.0](LICENSE-APACHE), at your option. StitchCraft is an
independent implementation and contains no Ink/Stitch code; it uses Ink/Stitch's parameter names for file
interoperability ([NOTICE](NOTICE)).

<sub>StitchCraft is an independent project, not affiliated with Ink/Stitch, ArtCraft or Brother.
Brother and PES are trademarks of their respective owners.</sub>
