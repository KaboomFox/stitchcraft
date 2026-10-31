//! VectorCraft adapter (layer L3): `.vectorcraft` documents to the engine's design model.
//!
//! Hosts translate their documents into a [`Design`](stitchcraft_engine::design::Design), so the engine
//! never sees VectorCraft's (`docs/src/design/architecture.md` › Hosts, ports and adapters). This crate is
//! the translation for VectorCraft: [`read`] turns a `.vectorcraft` file into a design and the warnings
//! about what it left out or simplified, as `stitchcraft-svg` does for SVG files. For the plug-in, it
//! translates what VectorCraft's plug-in ABI passes a live effect and what the effect gives back
//! ([`read_effect`], [`effect_output`], [`effect_error`]; roadmap steps M6.2 and M6.3), the Auto effect's
//! too ([`read_auto_effect`], M13.7), and what it passes a filter and the objects a filter gives back
//! ([`filter`], M6.12).
//!
//! What it reads (roadmap step M6.1, `docs/src/design/vectorcraft-integration.md`): formats 1 to 4,
//! plain or compressed, with or without the data format 4 keeps after its JSON; layers, groups, paths
//! and compound paths in paint order; visibility; fills and strokes with their colours, and a stroke's
//! width and join; and StitchCraft's live effects, whose records say how an object sews, or, the Auto
//! effect's (M13.7), that StitchCraft chooses its stitches. From M6.5 it reads Ink/Stitch's commands in
//! names: `stitch:ignore`, `stitch:start` and `stitch:end` (`REQ-VC-009`).
//! [`read_part`] is the part of a file it reads, which a bug-report bundle keeps.
//!
//! Its invariants:
//!
//! - **It reads VectorCraft's format, not its code.** The format as VectorCraft documents it and saves it
//!   is the contract; no VectorCraft crate is linked.
//! - **It never fails on content it can skip.** Kinds of objects, paints and colours it doesn't read,
//!   numbers that aren't numbers and objects far away are left out with a warning (`SC-W0807`,
//!   `SC-W0808`); only a file that is not a VectorCraft document, or is too large, is refused (`SC-E0806`).
//! - **Nothing is dropped silently.** What it leaves out or simplifies is a warning or a note
//!   (`SC-I0809`, and `SC-I0811` for what a name leaves out), unless VectorCraft does not draw it either.
//! - **Positions are exact**, points times 25.4/72, and the same on every platform.
//! - **Work is bounded** by [`MAX_BYTES`] of JSON and the budget.
#![forbid(unsafe_code)]

mod color;
mod container;
mod document;
mod effect;
pub mod filter;
mod geometry;
pub mod settings;
mod values;
mod wire;
mod writer;

pub use container::{MAX_BYTES, read_part};
pub use document::{VectorCraft, machine, read, read_with};
pub use effect::{
    AutoEffect, Effect, RIBBON_MM, effect_drawn, effect_error, effect_lines, effect_output, effect_ribbons, read_auto_effect, read_effect,
};
pub use values::text as param_text;

/// The namespace of StitchCraft's VectorCraft plug-ins. A live effect's id is `plugin.`, this, a dot and
/// its stitch family's id (`plugin.dev.stitchcraft.fill`), or [`AUTO`], which is how a document records the
/// effect.
pub const PLUGIN_NAMESPACE: &str = "dev.stitchcraft";

/// The name of StitchCraft's Auto effect (`plugin.dev.stitchcraft.auto`), which makes the object it is
/// applied to, and each path inside the group or the layer, an automatic element, its parameters the
/// settings of the rules (`REQ-VC-016`).
pub const AUTO: &str = "auto";

/// What VectorCraft puts before an installed plug-in's id to name its live effect (`effect.list` in its
/// engine's `cmd/effectcmd.rs`).
const EFFECT: &str = "plugin.";

/// The id of the plug-in `name`, *hello* or a stitch family's module: `dev.stitchcraft.fill`.
pub fn plugin_id(name: &str) -> String {
    format!("{PLUGIN_NAMESPACE}.{name}")
}

/// The id of the plug-in `name`'s live effect, by which a document records it: `plugin.dev.stitchcraft.fill`.
pub fn effect_id(name: &str) -> String {
    format!("{EFFECT}{}", plugin_id(name))
}

/// The plug-in name in the live effect id `id`: a stitch family's id, [`AUTO`], or another from a newer
/// version; `None` when the effect isn't StitchCraft's.
pub fn effect_name(id: &str) -> Option<&str> {
    id.strip_prefix(EFFECT)?.strip_prefix(PLUGIN_NAMESPACE)?.strip_prefix('.')
}

/// The parameter of StitchCraft's live effects that picks what their previews show. It is the plug-in's,
/// not the stitches': a record keeps it, the reader leaves it out of an element's settings, and a live
/// effect is given it beside its element ([`Effect::preview_level`]).
pub const PREVIEW_LEVEL: &str = "preview_level";

/// The newest `.vectorcraft` format version the adapter reads; a newer file is read best-effort
/// (`SC-W0803`).
pub const FORMAT: u64 = 4;

/// The file name extensions of VectorCraft's documents: its own, `drawcraft` from before VectorCraft was
/// renamed, which it still opens (`EXTENSION` and `LEGACY_EXTENSION` in its `crates/format`), and
/// `vctemplate`, a template, which it saves in the same format (its format table, `crates/engine/src/cmd/
/// fileio`).
pub const EXTENSIONS: &[&str] = &["vectorcraft", "drawcraft", "vctemplate"];

#[cfg(test)]
mod tests {
    use stitchcraft_params::Family;

    use super::*;

    #[test]
    fn an_effect_s_id_names_its_plug_in() {
        assert_eq!(plugin_id("hello"), "dev.stitchcraft.hello");
        assert_eq!(effect_id("fill"), "plugin.dev.stitchcraft.fill");
        for name in Family::ALL.iter().map(|family| family.id()).chain([AUTO]) {
            assert_eq!(effect_name(&effect_id(name)), Some(name));
        }
        // Another plug-in's effect, one of VectorCraft's own, and a plug-in's id that isn't an effect's.
        for other in ["plugin.dev.stitchcraftx.fill", "plugin.dev.stitchcraft", "stylize.dropShadow", "dev.stitchcraft.fill"] {
            assert_eq!(effect_name(other), None, "{other}");
        }
    }
}
