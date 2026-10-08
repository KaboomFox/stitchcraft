# VectorCraft compatibility

- `vectorcraft.toml` — the VectorCraft versions pinned per track (stable, pre-release, `release` branch,
  `main`).
- `vc-contract/` — Level A contract tests: our built `.wasm` plug-ins loaded into VectorCraft's real plug-in
  host (`vectorcraft-plugins`) at the ref under test. A separate workspace, built by
  `cargo xtask compat contract`.
- `scenarios/` (M6.6) — Level B end-to-end scenarios run with the real `vectorcraft-cli`.

```sh
cargo xtask compat discover                          # latest stable, pre-release and branch heads
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown
cargo xtask compat contract --ref v0.6.0 \
  --wasm target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm
```

Design: [docs/src/design/compatibility-gate.md](../docs/src/design/compatibility-gate.md).
