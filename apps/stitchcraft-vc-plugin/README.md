# stitchcraft-vc-plugin

StitchCraft's plug-ins for VectorCraft, built as WebAssembly modules for VectorCraft's sandboxed plug-in
ABI v1 (no imports; JSON in linear memory).

**Status:** M0 contains the *hello* live effect for spike M0.6: it returns the object's geometry unchanged,
proving the build and the ABI contract against VectorCraft's real host (`compat/vc-contract`). The running,
satin, fill and tools plug-ins arrive in M6.

## Build

```sh
rustup target add wasm32-unknown-unknown
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown
# target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm
```

In VectorCraft: **Object › Plug-ins › Install Plug-in…** (or `File › Open` the `.wasm`), then
**Effect › Plug-ins › StitchCraft Hello**.

## Invariants

- `src/abi.rs` is the **only** place in StitchCraft allowed to use `unsafe` (raw pointers across the ABI);
  every block carries a `// SAFETY:` comment, checked by `cargo xtask unsafe-audit`. Everything else is
  safe Rust tested natively.
- Plug-in ids derive from one constant, `PLUGIN_NAMESPACE`.
- Manifests satisfy VectorCraft's rules (validated by tests, `REQ-VC-002`).
- Contains no engine logic: it adapts VectorCraft's JSON to the engine and back.
