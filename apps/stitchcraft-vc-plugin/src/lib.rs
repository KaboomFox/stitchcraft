//! StitchCraft plug-ins for VectorCraft's WebAssembly plug-in ABI v1.
//!
//! One `.wasm` module for each stitch family, so that each dialog holds only its family's parameters and
//! stays within VectorCraft's 64 (`docs/src/design/vectorcraft-integration.md`), and one for each machine
//! format, an exporter (VectorCraft 0.14). Which plug-in a module is is chosen when it is built, by a
//! feature: `stroke`, `satin` or `fill` for a family's live effect, `pes` or `dst` for an exporter
//! ([`MODULE`]). Without one it is the M0.6 *hello* effect, which the contract tests measure the host's
//! costs with. Everything a module does is ordinary code, built into every module and tested natively,
//! whichever it is.
//!
//! Layout:
//! - [`manifest`]: plug-in ids and manifests, made from the parameter registry and checked with
//!   VectorCraft's rules;
//! - [`dialog`]: how VectorCraft's dialogs show each parameter, labelled, in sections and only when it
//!   applies, from the registry;
//! - [`effect`]: what each live effect does with VectorCraft's JSON: safe Rust, tested natively;
//! - [`export`]: what each exporter does: the document sewn into a machine file as `stitch export` sews it;
//! - `abi`: the exported functions VectorCraft calls. It is the single place in StitchCraft allowed to use
//!   `unsafe`, because the ABI passes raw pointers into linear memory. This crate therefore does not carry
//!   `#![forbid(unsafe_code)]`; the workspace denies `unsafe` and only `abi.rs` opts out, which
//!   `cargo xtask unsafe-audit` verifies. The shim compiles on every target so clippy and the compiler check
//!   it everywhere, but it is only meaningful inside VectorCraft's wasm32 sandbox.

pub mod dialog;
pub mod effect;
pub mod export;
pub mod manifest;
pub mod preview;

#[cfg(test)]
mod testing;

mod abi;

use stitchcraft_params::Family;
use stitchcraft_plan::FormatId;

/// What a module is: a stitch family's live effect, a machine format's exporter, or *hello*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    /// The M0.6 *hello* effect, which returns the object's geometry unchanged.
    Hello,
    /// The live effect that sews an object as the family.
    Family(Family),
    /// The exporter that writes the document in the format, as `stitch export` writes it.
    Exporter(FormatId),
}

/// What this build of the module is, by its features: the first of `fill`, `satin`, `stroke`, `pes` and
/// `dst`, or *hello* without any.
pub const MODULE: Module = if cfg!(feature = "fill") {
    Module::Family(Family::Fill)
} else if cfg!(feature = "satin") {
    Module::Family(Family::Satin)
} else if cfg!(feature = "stroke") {
    Module::Family(Family::Stroke)
} else if cfg!(feature = "pes") {
    Module::Exporter(FormatId::PesV1)
} else if cfg!(feature = "dst") {
    Module::Exporter(FormatId::Dst)
} else {
    Module::Hello
};

impl Module {
    /// Every module: *hello*, the live effect of each stitch family and the exporter of each machine
    /// format.
    pub fn all() -> Vec<Module> {
        let families = Family::ALL.iter().map(|family| Module::Family(*family));
        let exporters = FormatId::ALL.iter().map(|format| Module::Exporter(*format));
        std::iter::once(Module::Hello).chain(families).chain(exporters).collect()
    }

    /// The module's name, which is also the feature that builds it ([`MODULE`]): `hello`, the family's id or
    /// the format's extension.
    pub fn name(self) -> &'static str {
        match self {
            Module::Hello => "hello",
            Module::Family(family) => family.id(),
            Module::Exporter(format) => format.extension(),
        }
    }

    /// The module's manifest.
    pub fn manifest(self) -> manifest::Manifest {
        match self {
            Module::Hello => manifest::hello(),
            Module::Family(family) => manifest::family(family),
            Module::Exporter(format) => manifest::exporter(format),
        }
    }

    /// The module's run on `input` with `params` (the parameters JSON), as `vc_run` gives them: the JSON
    /// it returns. A live effect's input is its object, and an exporter's the whole document.
    pub fn run(self, input: &[u8], params: &[u8]) -> Result<Vec<u8>, effect::RunError> {
        match self {
            Module::Hello => effect::hello(input, params),
            Module::Family(family) => effect::sew(family, input, params),
            Module::Exporter(format) => export::export(format, input, params),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    #[cfg(not(any(feature = "stroke", feature = "satin", feature = "fill", feature = "pes", feature = "dst")))]
    fn without_a_feature_the_module_is_hello() {
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
        assert_eq!(Module::all().into_iter().map(Module::name).collect::<Vec<_>>(), ["hello", "stroke", "satin", "fill", "pes", "dst"]);
        // An exporter is given a whole document, and returns a machine file.
        for format in FormatId::ALL {
            assert_eq!(Module::Exporter(*format).manifest(), manifest::exporter(*format));
            let exported = read(Module::Exporter(*format).run(testing::PATCH, b"{}"));
            assert!(exported["dataBase64"].as_str().is_some_and(|data| data.len() > 1_000), "{format:?}: {exported}");
        }
    }
}
