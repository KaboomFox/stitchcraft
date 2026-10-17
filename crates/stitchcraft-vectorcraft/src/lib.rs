//! VectorCraft adapter (layer L3): `.vectorcraft` documents to the engine's design model.
//!
//! Hosts translate their documents into a [`Design`](stitchcraft_engine::design::Design), so the engine
//! never sees VectorCraft's (`docs/src/design/architecture.md` › Hosts, ports and adapters). This crate is
//! the translation for VectorCraft: [`read`] turns a `.vectorcraft` file into a design and the warnings
//! about what it left out or simplified, as `stitchcraft-svg` does for SVG files. For the plug-in, it
//! translates what VectorCraft's plug-in ABI passes a live effect and what the effect gives back
//! ([`read_effect`], [`effect_output`], [`effect_error`]; roadmap steps M6.2 and M6.3).
//!
//! What it reads (roadmap step M6.1, `docs/src/design/vectorcraft-integration.md`): formats 1 to 4,
//! plain or compressed, with or without the data format 4 keeps after its JSON; layers, groups, paths
//! and compound paths in paint order; visibility; fills and strokes with their colours, and a stroke's
//! width and join; and StitchCraft's live effects, whose records say how an object sews.
//!
//! Its invariants:
//!
//! - **It reads VectorCraft's format, not its code.** The format as VectorCraft documents it and saves it
//!   is the contract; no VectorCraft crate is linked.
//! - **It never fails on content it can skip.** Kinds of objects, paints and colours it doesn't read,
//!   numbers that aren't numbers and objects far away are left out with a warning (`SC-W0807`,
//!   `SC-W0808`); only a file that is not a VectorCraft document, or is too large, is refused (`SC-E0806`).
//! - **Nothing is dropped silently.** What it leaves out or simplifies is a warning or a note
//!   (`SC-I0809`), unless VectorCraft does not draw it either.
//! - **Positions are exact**, points times 25.4/72, and the same on every platform.
//! - **Work is bounded** by [`MAX_BYTES`] of JSON and the budget.
#![forbid(unsafe_code)]

mod color;
mod container;
mod document;
mod effect;
mod geometry;
mod values;
mod wire;
mod writer;

pub use container::MAX_BYTES;
pub use document::{VectorCraft, read};
pub use effect::{Effect, RIBBON_MM, effect_error, effect_lines, effect_output, effect_ribbons, read_effect};

/// The namespace of StitchCraft's VectorCraft plug-ins. A live effect's id is `plugin.`, this, a dot and
/// its stitch family's id (`plugin.dev.stitchcraft.fill`), which is how a document records the effect.
pub const PLUGIN_NAMESPACE: &str = "dev.stitchcraft";

/// The parameter of StitchCraft's live effects that picks what their previews show. It is the plug-in's,
/// not the stitches': a record keeps it, the reader leaves it out of an element's settings, and a live
/// effect is given it beside its element ([`Effect::preview_level`]).
pub const PREVIEW_LEVEL: &str = "preview_level";

/// The newest `.vectorcraft` format version the adapter reads; a newer file is read best-effort
/// (`SC-W0803`).
pub const FORMAT: u64 = 4;
