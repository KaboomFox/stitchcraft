# stitchcraft-vc-plugin

StitchCraft's plug-ins for VectorCraft, built as WebAssembly modules for VectorCraft's sandboxed plug-in
ABI v1 (no imports; JSON in linear memory).

**Status:** one live effect for each stitch family, from M6.2: *StitchCraft Stroke*, *StitchCraft Satin*
and *StitchCraft Fill*. Each sews the object it is applied to, with the parameters set in its dialog, and
shows as much of the stitches as VectorCraft lets a live effect spend on, in place of the object's art
(M6.3, `src/preview.rs`). A stroke and a satin column show their stitches, and a fill its rows; the
outline shows when they cost too much, and the dialog's `preview_level` picks a level. M6.4 adds the
tools. Without a family the module is M0.6's *hello* effect, which returns the object's geometry
unchanged. The contract tests measure the host's costs with it
(`docs/src/design/vectorcraft-integration.md`).

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown --features fill
# target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm, the StitchCraft Fill plug-in
```

The feature is the family: `stroke`, `satin` or `fill`. Without one, the build is *hello*. Each build
writes the same file, so copy it before building the next. `cargo xtask compat contract --ref <tag>`
builds all four into `target/compat/plugins/` and loads them into VectorCraft's host.

In VectorCraft: **Object › Plug-ins › Install Plug-in…** (or `File › Open` the `.wasm`), then
**Effect › Plug-ins › StitchCraft Fill**.

## Invariants

- `src/abi.rs` is the **only** place in StitchCraft allowed to use `unsafe` (raw pointers across the ABI);
  every block carries a `// SAFETY:` comment, checked by `cargo xtask unsafe-audit`. Everything else is
  safe Rust tested natively, and every module contains all of it. The feature only picks what
  `vc_manifest` and `vc_run` serve (`MODULE`).
- Plug-in ids derive from one constant, `PLUGIN_NAMESPACE`. The VectorCraft adapter owns it, as it reads
  the effect records those ids name in documents, and the plug-in re-exports it.
- Manifests are made from the parameter registry and satisfy VectorCraft's rules (validated by tests,
  `REQ-VC-002`). A record of a manifest's defaults sews as an element without settings.
- Contains no engine logic: the VectorCraft adapter reads the object and writes the stitches, and the
  engine sews it.
- A live effect never makes an object vanish. When it can't show stitches, VectorCraft draws the object as
  it is, with the reason as the plug-in's last error.
- A preview spends at most what VectorCraft allows (`REQ-VC-003`). The plug-in prices the engine's work in
  instructions, with numbers the contract tests measure, and the engine stops before VectorCraft would. A
  fill's exact preview, which the user picks, is the exception, and VectorCraft bounds it.

## Dependencies

`serde` and `serde_json` (the ABI's JSON), `stitchcraft-params` and `stitchcraft-engine` (the registry, and
the sewing), `stitchcraft-plan` (the reference machine), `stitchcraft-core` (diagnostics and the budget),
and `stitchcraft-vectorcraft` (`PLUGIN_NAMESPACE`, and what a live effect reads and writes).
