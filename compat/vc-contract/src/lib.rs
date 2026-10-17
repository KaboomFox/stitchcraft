//! ABI contract tests between StitchCraft's plug-ins and VectorCraft's real plug-in host.
//!
//! The tests live in `tests/contract.rs`. They load the modules in the folder the `STITCHCRAFT_PLUGINS`
//! environment variable names, *hello* and one for each stitch family, into `vectorcraft-plugins` — the
//! same code VectorCraft runs — and check that each installs, declares a valid manifest and behaves as
//! specified. `cargo xtask compat contract` builds the modules and runs the tests against each VectorCraft
//! ref the compatibility gate tracks (docs/src/design/compatibility-gate.md).
#![forbid(unsafe_code)]
