//! StitchCraft plug-ins for VectorCraft's WebAssembly plug-in ABI v1.
//!
//! Layout:
//! - [`manifest`]: plug-in ids and manifests, validated against VectorCraft's rules;
//! - [`effect`]: what each plug-in does with VectorCraft's JSON — safe Rust, tested natively;
//! - `abi`: the exported functions VectorCraft calls. It is the single place in StitchCraft allowed to use
//!   `unsafe`, because the ABI passes raw pointers into linear memory. This crate therefore does not carry
//!   `#![forbid(unsafe_code)]`; the workspace denies `unsafe` and only `abi.rs` opts out, which
//!   `cargo xtask unsafe-audit` verifies. The shim compiles on every target so clippy and the compiler check
//!   it everywhere, but it is only meaningful inside VectorCraft's wasm32 sandbox.
//!
//! Status: the M0.6 *hello* live effect. Design: `docs/src/design/vectorcraft-integration.md`.

pub mod effect;
pub mod manifest;

mod abi;
