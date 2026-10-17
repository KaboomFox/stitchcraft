//! StitchCraft plug-ins for VectorCraft's WebAssembly plug-in ABI v1.
//!
//! One `.wasm` module for each stitch family, so that each dialog holds only its family's parameters and
//! stays within VectorCraft's 64 (`docs/src/design/vectorcraft-integration.md`). Which plug-in a module is
//! is chosen when it is built, by a feature, `stroke`, `satin` or `fill` ([`MODULE`]); without one it is the
//! M0.6 *hello* effect, which the contract tests measure the host's costs with. Everything a module does
//! is ordinary code, built into every module and tested natively, whichever it is.
//!
//! Layout:
//! - [`manifest`]: plug-in ids and manifests, made from the parameter registry and checked with
//!   VectorCraft's rules;
//! - [`effect`]: what each plug-in does with VectorCraft's JSON: safe Rust, tested natively;
//! - `abi`: the exported functions VectorCraft calls. It is the single place in StitchCraft allowed to use
//!   `unsafe`, because the ABI passes raw pointers into linear memory. This crate therefore does not carry
//!   `#![forbid(unsafe_code)]`; the workspace denies `unsafe` and only `abi.rs` opts out, which
//!   `cargo xtask unsafe-audit` verifies. The shim compiles on every target so clippy and the compiler check
//!   it everywhere, but it is only meaningful inside VectorCraft's wasm32 sandbox.

pub mod effect;
pub mod manifest;
pub mod preview;

#[cfg(test)]
mod testing;

mod abi;

use stitchcraft_params::Family;

/// What a module is: a stitch family's live effect, or *hello*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    /// The M0.6 *hello* effect, which returns the object's geometry unchanged.
    Hello,
    /// The live effect that sews an object as the family.
    Family(Family),
}

/// What this build of the module is, by its features: the first of `fill`, `satin` and `stroke`, or
/// *hello* without any.
pub const MODULE: Module = if cfg!(feature = "fill") {
    Module::Family(Family::Fill)
} else if cfg!(feature = "satin") {
    Module::Family(Family::Satin)
} else if cfg!(feature = "stroke") {
    Module::Family(Family::Stroke)
} else {
    Module::Hello
};

impl Module {
    /// The module's manifest.
    pub fn manifest(self) -> manifest::Manifest {
        match self {
            Module::Hello => manifest::hello(),
            Module::Family(family) => manifest::family(family),
        }
    }

    /// The module's run on `input` (the objects JSON) with `params` (the parameters JSON), as `vc_run`
    /// gives them: the JSON it returns.
    pub fn run(self, input: &[u8], params: &[u8]) -> Result<Vec<u8>, effect::RunError> {
        match self {
            Module::Hello => effect::hello(input, params),
            Module::Family(family) => effect::sew(family, input, params),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    #[cfg(not(any(feature = "stroke", feature = "satin", feature = "fill")))]
    fn without_a_family_s_feature_the_module_is_hello() {
        assert_eq!(MODULE, Module::Hello);
    }

    #[test]
    fn each_module_serves_its_manifest_and_its_effect() {
        assert_eq!(Module::Hello.manifest(), manifest::hello());
        assert_eq!(Module::Family(Family::Satin).manifest(), manifest::family(Family::Satin));
        let square = json!({"subpaths": [{"closed": true, "anchors": [{"p": [0, 0]}, {"p": [20, 0]}, {"p": [20, 20]}, {"p": [0, 20]}]}]});
        let input =
            serde_json::to_vec(&json!({"objects": [{"type": "path", "path": square, "fillRule": "nonzero", "bounds": [0, 0, 20, 20]}]})).unwrap();
        let read = |out: Result<Vec<u8>, effect::RunError>| serde_json::from_slice::<Value>(&out.unwrap()).unwrap();
        assert_eq!(read(Module::Hello.run(&input, b"{}")), json!({"objects": [{"path": square}]}));
        let stitches = read(Module::Family(Family::Stroke).run(&input, b"{}"));
        assert!(stitches["objects"][0]["path"]["subpaths"][0]["anchors"].as_array().is_some_and(|anchors| anchors.len() > 4), "{stitches}");
    }
}
