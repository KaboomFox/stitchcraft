//! StitchCraft plug-ins for VectorCraft's WebAssembly plug-in ABI v1.
//!
//! One `.wasm` module for each stitch family, so that each dialog holds only its family's parameters and
//! stays within VectorCraft's 64 (`docs/src/design/vectorcraft-integration.md`), one for the Auto effect,
//! which chooses the stitches by width (M13.7), one for each machine format, an exporter (VectorCraft
//! 0.14), and one for each filter: Design Settings (VectorCraft 0.16), Stroke to Satin, a heavy filter
//! (VectorCraft 0.17), and Test Sheet. Which plug-in a module is is chosen when it is built, by a feature:
//! `stroke`, `satin`, `fill` or `auto` for a live effect, `pes` or `dst` for an exporter, and `settings`,
//! `stroke-to-satin` or `testsheet` for a filter ([`MODULE`]). Without one it is the M0.6 *hello* effect,
//! which the contract tests measure the host's costs with. Everything a module does is ordinary code,
//! built into every module and tested natively, whichever it is.
//!
//! Layout:
//! - [`manifest`]: plug-in ids and manifests, made from the parameter registry and checked with
//!   VectorCraft's rules;
//! - [`dialog`]: how VectorCraft's dialogs show each parameter, labelled, in sections and only when it
//!   applies, from the registry;
//! - [`effect`]: what each live effect does with VectorCraft's JSON: safe Rust, tested natively;
//! - [`export`]: what each exporter does: the document sewn into a machine file as `stitch export` sews it;
//! - [`settings`]: what the Design Settings filter does: the document's settings and machine kept with it;
//! - [`stroke_to_satin`]: what the Stroke to Satin filter does: each selected line made into a satin column
//!   drawn with its rails and rungs;
//! - [`testsheet`]: what the Test Sheet filter does: a test sheet drawn into the document;
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
pub mod settings;
pub mod stroke_to_satin;
pub mod testsheet;

#[cfg(test)]
mod testing;

mod abi;

use stitchcraft_params::Family;
use stitchcraft_plan::FormatId;
use stitchcraft_vectorcraft::AUTO;
use stitchcraft_vectorcraft::settings::SETTINGS_FILTER;

/// What a module is: a stitch family's live effect, a machine format's exporter, a filter, or *hello*.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    /// The M0.6 *hello* effect, which returns the object's geometry unchanged.
    Hello,
    /// The live effect that sews an object as the family.
    Family(Family),
    /// The Auto effect, which makes the object, or each path inside the group, automatic, and previews the
    /// stitches automatic digitizing chooses (`REQ-VC-016`).
    Auto,
    /// The exporter that writes the document in the format, as `stitch export` writes it.
    Exporter(FormatId),
    /// The Design Settings filter, which keeps a document's design settings and machine with it.
    Settings,
    /// The Stroke to Satin filter, a heavy one, which makes each selected line into a satin column drawn with
    /// its rails and rungs.
    StrokeToSatin,
    /// The Test Sheet filter, which draws a test sheet into the document.
    TestSheet,
}

/// What this build of the module is, by its features: the first of `fill`, `satin`, `stroke`, `auto`, `pes`,
/// `dst`, `settings`, `stroke-to-satin` and `testsheet`, or *hello* without any.
pub const MODULE: Module = if cfg!(feature = "fill") {
    Module::Family(Family::Fill)
} else if cfg!(feature = "satin") {
    Module::Family(Family::Satin)
} else if cfg!(feature = "stroke") {
    Module::Family(Family::Stroke)
} else if cfg!(feature = "auto") {
    Module::Auto
} else if cfg!(feature = "pes") {
    Module::Exporter(FormatId::PesV1)
} else if cfg!(feature = "dst") {
    Module::Exporter(FormatId::Dst)
} else if cfg!(feature = "settings") {
    Module::Settings
} else if cfg!(feature = "stroke-to-satin") {
    Module::StrokeToSatin
} else if cfg!(feature = "testsheet") {
    Module::TestSheet
} else {
    Module::Hello
};

impl Module {
    /// Every module: *hello*, the live effect of each stitch family, the Auto effect, the exporter of each
    /// machine format, and the filters.
    pub fn all() -> Vec<Module> {
        let families = Family::ALL.iter().map(|family| Module::Family(*family));
        let exporters = FormatId::ALL.iter().map(|format| Module::Exporter(*format));
        let filters = [Module::Settings, Module::StrokeToSatin, Module::TestSheet];
        std::iter::once(Module::Hello).chain(families).chain([Module::Auto]).chain(exporters).chain(filters).collect()
    }

    /// The module's name, which is also the feature that builds it ([`MODULE`]): `hello`, the family's id,
    /// `auto`, the format's extension or the filter's.
    pub fn name(self) -> &'static str {
        match self {
            Module::Hello => "hello",
            Module::Family(family) => family.id(),
            Module::Auto => AUTO,
            Module::Exporter(format) => format.extension(),
            Module::Settings => SETTINGS_FILTER,
            Module::StrokeToSatin => stroke_to_satin::STROKE_TO_SATIN,
            Module::TestSheet => testsheet::TESTSHEET,
        }
    }

    /// The module's manifest.
    pub fn manifest(self) -> manifest::Manifest {
        match self {
            Module::Hello => manifest::hello(),
            Module::Family(family) => manifest::family(family),
            Module::Auto => manifest::auto(),
            Module::Exporter(format) => manifest::exporter(format),
            Module::Settings => manifest::settings(),
            Module::StrokeToSatin => manifest::stroke_to_satin(),
            Module::TestSheet => manifest::testsheet(),
        }
    }

    /// The module's run on `input` with `params` (the parameters JSON), as `vc_run` gives them: the JSON
    /// it returns. A live effect's input is its object, a filter's the selected objects, and an exporter's
    /// the whole document.
    pub fn run(self, input: &[u8], params: &[u8]) -> Result<Vec<u8>, effect::RunError> {
        match self {
            Module::Hello => effect::hello(input, params),
            Module::Family(family) => effect::sew(family, input, params),
            Module::Auto => effect::auto(input, params),
            Module::Exporter(format) => export::export(format, input, params),
            Module::Settings => settings::run(input, params),
            Module::StrokeToSatin => stroke_to_satin::run(input, params),
            Module::TestSheet => testsheet::run(input, params),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    #[test]
    #[cfg(not(any(
        feature = "stroke",
        feature = "satin",
        feature = "fill",
        feature = "auto",
        feature = "pes",
        feature = "dst",
        feature = "settings",
        feature = "stroke-to-satin",
        feature = "testsheet"
    )))]
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
        // The Auto effect previews a painted object's stitches.
        assert_eq!(Module::Auto.manifest(), manifest::auto());
        let rows = read(Module::Auto.run(&testing::painted(&square, true, None), b"{}"));
        assert!(rows["objects"][0]["path"]["subpaths"].as_array().is_some_and(|ribbons| ribbons.len() > 4), "{rows}");
        assert_eq!(
            Module::all().into_iter().map(Module::name).collect::<Vec<_>>(),
            ["hello", "stroke", "satin", "fill", "auto", "pes", "dst", "settings", "stroke-to-satin", "testsheet"]
        );
        // An exporter is given a whole document, and returns a machine file.
        for format in FormatId::ALL {
            assert_eq!(Module::Exporter(*format).manifest(), manifest::exporter(*format));
            let exported = read(Module::Exporter(*format).run(testing::PATCH, b"{}"));
            assert!(exported["dataBase64"].as_str().is_some_and(|data| data.len() > 1_000), "{format:?}: {exported}");
        }
    }
}
