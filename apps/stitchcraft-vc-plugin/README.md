# stitchcraft-vc-plugin

StitchCraft's plug-ins for VectorCraft, built as WebAssembly modules for VectorCraft's sandboxed plug-in
ABI v1 (no imports; JSON in linear memory).

**Status:** one live effect for each stitch family, from M6.2: *StitchCraft Stroke*, *StitchCraft Satin*
and *StitchCraft Fill*. Each sews the object it is applied to, with the parameters set in its dialog, and
shows as much of the stitches as VectorCraft lets a live effect spend on, in place of the object's art
(M6.3, `src/preview.rs`). A stroke and a satin column show their stitches, and a fill its rows; the
outline shows when they cost too much, and the dialog's `preview_level` picks a level. VectorCraft's own
commands are the tools, and its Graphic Styles keep the effects' settings as presets (M6.4,
`docs/src/user/how-to/vectorcraft-columns-and-presets.md`). From VectorCraft 0.14 a stroke or a satin effect
follows its object's stroke, so a satin column drawn as one path and a zigzag stroke show their stitches as
wide as it (M6.8, `REQ-VC-010`). From M6.9, 2 exporters too, *PES embroidery (StitchCraft)* and *DST
embroidery (StitchCraft)*, which write the file `stitch export` writes from VectorCraft 0.14's File ›
Export › Export As (`src/export.rs`, `REQ-VC-011`). From M6.10 the dialogs label each parameter, with its
unit, help and help page, in the registry's groups as sections, and show it only for the methods it
applies to (`src/dialog.rs`, `REQ-VC-012`). VectorCraft reads those keys from 0.14, and older releases
ignore them. From M6.11 a filter, *StitchCraft Design Settings*, keeps the design settings and the machine
with the document as VectorCraft 0.16 keeps a filter's data, and the exporters plan for the machine kept
(`src/settings.rs`, `REQ-VC-013`). An older VectorCraft would drop that data, and there the filter
declines with `SC-E0813`.
Without a feature the module is M0.6's *hello* effect, which returns the object's geometry unchanged. The
contract tests measure the host's costs with it (`docs/src/design/vectorcraft-integration.md`).

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown --features fill
# target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm, the StitchCraft Fill plug-in
```

The feature is the family, `stroke`, `satin` or `fill`, the machine format, `pes` or `dst`, or `settings`
for the Design Settings filter. Without one, the build is *hello*. Each build writes the same file, so copy
it before building the next. `cargo xtask compat contract --ref <tag>` builds all seven into
`target/compat/plugins/` and loads them into VectorCraft's host.

In VectorCraft: **Object › Plug-ins › Install Plug-in…** (or `File › Open` the `.wasm`), then
**Effect › Plug-ins › StitchCraft Fill**, **File › Export › Export As** with an exporter, or
**Object › Plug-ins › StitchCraft Design Settings**.

## Invariants

- `src/abi.rs` is the **only** place in StitchCraft allowed to use `unsafe` (raw pointers across the ABI);
  every block carries a `// SAFETY:` comment, checked by `cargo xtask unsafe-audit`. Everything else is
  safe Rust tested natively, and every module contains all of it. The feature only picks what
  `vc_manifest` and `vc_run` serve (`MODULE`).
- Plug-in ids come from one function, `plugin_id`. The VectorCraft adapter owns it, with the effect ids
  documents record, as it reads those records, and the plug-in re-exports it.
- Manifests are made from the parameter registry and satisfy VectorCraft's rules (validated by tests,
  `REQ-VC-002`). A record of a manifest's defaults sews as an element without settings.
- What a dialog shows comes from the registry too, and never changes what sews: VectorCraft gives the
  plug-in the value of a row it hides. Each help page is a page of the book, which
  `cargo xtask docs --check` finds (`REQ-VC-012`).
- Contains no engine logic: the VectorCraft adapter reads the object and writes the stitches, and the
  engine sews it. An exporter calls the reader, the engine and the format writer as `stitch export` does,
  with its budget, and Level B of the compatibility gate checks that the files are the same.
- A live effect never makes an object vanish. When it can't show stitches, VectorCraft draws the object as
  it is, with the reason as the plug-in's last error. The Design Settings filter returns each object it is
  given as it is, as VectorCraft deletes one a filter leaves out.
- The design settings are read and written in one place, `stitchcraft_vectorcraft::settings`, by the
  names, labels and help of `stitchcraft_engine::design::SETTINGS`, which the SVG adapter reads
  Ink/Stitch's with.
- A preview spends at most what VectorCraft allows (`REQ-VC-003`). The plug-in prices the engine's work in
  instructions, with numbers the contract tests measure, and the engine stops before VectorCraft would. A
  fill's exact preview, which the user picks, is the exception, and VectorCraft bounds it.

## Dependencies

`serde` and `serde_json` (the ABI's JSON), `stitchcraft-params` and `stitchcraft-engine` (the registry, and
the sewing), `stitchcraft-plan` (the machine profiles and formats), `stitchcraft-core` (diagnostics and the
budget), `stitchcraft-formats` (the exporters' files) and `stitchcraft-vectorcraft` (`plugin_id`, what a
live effect reads and writes, and the documents an exporter reads).
