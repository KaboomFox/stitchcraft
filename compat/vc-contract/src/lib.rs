//! ABI contract tests between StitchCraft's plug-ins and VectorCraft's real plug-in host.
//!
//! The tests live in `tests/contract.rs`. They load the `.wasm` named by the `STITCHCRAFT_PLUGIN_WASM`
//! environment variable into `vectorcraft-plugins` — the same code VectorCraft runs — and check that it
//! installs, declares a valid manifest and behaves as specified. `cargo xtask compat contract` runs them
//! against each VectorCraft ref the compatibility gate tracks (docs/src/design/compatibility-gate.md).
