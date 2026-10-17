# VectorCraft compatibility

- `vectorcraft.toml` — the VectorCraft versions pinned per track (stable, pre-release, `release` branch,
  `main`), and the SHA-256 of the stable release's Linux x86_64 archive.
- `vc-contract/` — Level A contract tests: our built `.wasm` plug-ins loaded into VectorCraft's real plug-in
  host (`vectorcraft-plugins`) at the ref under test. A separate workspace, built by
  `cargo xtask compat contract`.
- `vc-window/` holds the window probe. It runs the stable release's app without a screen and compares its
  screenshots, and xa11y drives the app through its accessibility tree. A separate workspace, run by
  `cargo xtask compat window` in a desktop of its own (Xvfb, D-Bus and AT-SPI, on Linux).
- `scenarios/` (M6.6) — Level B end-to-end scenarios run with the real `vectorcraft-cli`.

```sh
cargo xtask compat discover                          # latest stable, pre-release and branch heads
cargo build -p stitchcraft-vc-plugin --release --target wasm32-unknown-unknown
cargo xtask compat contract --ref v0.7.0 \
  --wasm target/wasm32-unknown-unknown/release/stitchcraft_vc_plugin.wasm
cargo xtask compat window                            # needs xvfb, dbus and at-spi2-core
```

Design: [docs/src/design/compatibility-gate.md](../docs/src/design/compatibility-gate.md), and for the
window probe [docs/src/design/docs-pipeline.md](../docs/src/design/docs-pipeline.md).
