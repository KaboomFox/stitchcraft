//! Clip groups (`REQ-VC-017`): what a clip group holds shows only inside its clipping object, and sews only
//! what shows.
//!
//! VectorCraft's source is cited as its tag `v0.19.0` (`bcebabe`), the compatibility gate's stable release,
//! has it. Releases from 0.13 clip alike, the area's code in `crates/doc/src/node.rs` before 0.18.
//!
//! **What VectorCraft saves.** A group or a layer whose kind has `"clip": true` clips the objects it holds
//! to its first, the clipping object (`crates/doc/src/node/kind.rs`, lines 41 to 68). Object › Clipping Mask
//! › Make wraps the objects selected in such a group, the topmost first, and that one must be a path, a
//! compound path or text. It loses its paints and effects, and a path is marked `"clipping": true`
//! (`make_clipping_path` to `clip_make`, `crates/engine/src/cmd/object/compound.rs`, lines 129 to 180). The
//! Layers panel's Make Clipping Mask clips a layer or a group the same way (`clip_toggle`,
//! `crates/engine/src/cmd/layer.rs`, line 448). A file VectorCraft imports can clip to a group or a clip
//! group too (`crates/doc/src/clipnest.rs`).
//!
//! **What it clips to.** The area of the clipping object, whatever its paints, and hidden or not
//! (`Node::clip_shapes` and `Node::clip_outline`, `crates/doc/src/node/clip.rs`, lines 9 to 71):
//!
//! - a path's by its fill rule, and a compound path's paths' subpaths together by its own rule;
//! - a clip group's, its own clipping object's;
//! - the members of another group or of a layer that show, together, but a Shaper group's sources;
//! - text's by its glyphs' outlines, an image's or a placed document's by its frame, and a blend's, an
//!   envelope's, a mesh's, a repeat's or an Intertwine object's by what it draws;
//! - a guide's and a symbol's, nothing.
//!
//! One shape keeps its rule, and several are united. A clipping object of no shape hides what the group
//! holds. The group's other objects are drawn inside the area, the clipping object's fills behind them and
//! its strokes over them, unclipped (`crates/render/src/renderer/nodes.rs`, lines 389 to 420).
//!
//! **What the reader makes of it.** A [`Clip`] for each clip group, of the clipping object's shapes in
//! millimetres, each with its rule. Every element of what the group holds, however deep, is cut to it and
//! to the clips of the clip groups around it ([`super::Clips`]), as an SVG file's shapes are cut to their
//! clip paths (`REQ-GEN-005`): a fill's area, a stroke's lines, and a satin column sewn whole (`SC-W0217`).
//! An element they leave nothing of is not sewn (`SC-W0013`). The clipping object's own elements are cut
//! only to the clips around its group: its fill lies in its own area, and its stroke is drawn over the edge.
//!
//! **What it can't read** adds nothing to the clip, and is named. Text, whose glyphs need the fonts
//! VectorCraft draws them in, is named with `SC-W0807`, as are an image, a placed document, a live object and
//! a kind the reader doesn't know. So is a path whose geometry can't be used, with `SC-W0808`. A clip left
//! with no shape leaves nothing of what it cuts, where VectorCraft shows what lies in the glyphs or the frame.
//!
//! **StitchCraft's effects on a clipping object.** VectorCraft runs a group's geometry effects on every path
//! inside it, the clipping objects of the clip groups it holds among them (`for_each_leaf`,
//! `crates/effects/src/reshape.rs`, line 731, from `evaluate_container`, `crates/effects/src/group.rs`,
//! line 522), and clips to what they return. So where StitchCraft's effects reach a clipping object, the
//! preview is cut to the stitches they draw of it, a fill's rows or a stroke's line, and what sews is cut
//! to the clipping object as it is drawn. `SC-W0807` says so.
//!
//! **Work.** Each object read for a clip costs a unit, and each subpath a unit and one more for each anchor,
//! as a shape's do. The paths are held once, shared by the elements cut to them ([`Clip::shapes`]): a path
//! is read into one clip at most, so the clips hold no more than about 5 bytes for each byte of their JSON.
//! Each element is charged a unit for each clip that cuts it and one more for each segment of the clip,
//! which the engine cuts it by ([`Reader::cut`]).

use stitchcraft_core::{Code, Diagnostic};
use stitchcraft_engine::design::{Clip, ClipShape};

use super::{EXPAND, Reader, exhausted, quoted};
use crate::geometry::{self, UNUSABLE_HINT};
use crate::wire::{Kind, Node, PathData};

/// How to clip to the shape of an image, a placed document or a mesh, which StitchCraft doesn't read.
const DRAW_A_PATH: &str = "Draw a path round it, and make the clipping mask with that.";

impl Reader {
    /// The clip of the clip group or the clipping layer `label`, `who`, whose kind is `kind`, with the number
    /// of its segments: the area of its first object. `None` when it doesn't clip, or holds nothing.
    pub(super) fn clip(&mut self, kind: &Kind, label: &str, who: &str) -> Result<Option<(Clip, u64)>, Diagnostic> {
        let Some(clipping) = kind.children.first().filter(|_| kind.clip) else { return Ok(None) };
        let mut shapes = Vec::new();
        self.clip_shapes(clipping, label, who, &mut shapes)?;
        let segments: usize = shapes.iter().flat_map(|shape| &shape.path.subpaths).map(|subpath| subpath.segments.len()).sum();
        Ok(Some((Clip { shapes: shapes.into() }, segments as u64)))
    }

    /// The shapes `node` adds to the clip of `label`, `who`, as VectorCraft clips to it, in `shapes`. What it
    /// can't read is named, and adds nothing.
    fn clip_shapes(&mut self, node: &Node, label: &str, who: &str, shapes: &mut Vec<ClipShape>) -> Result<(), Diagnostic> {
        self.charge(1)?;
        let name = node.kind.as_ref().map_or("", |kind| kind.name.as_str());
        let (what, hint) = match (name, &node.kind) {
            // A guide and a symbol add nothing, as in VectorCraft.
            ("path", Some(kind)) if kind.guide => return Ok(()),
            ("symbolinstance", _) => return Ok(()),
            ("path", Some(kind)) => return self.clip_path(&kind.path.iter().collect::<Vec<_>>(), kind.rule.as_deref(), label, who, shapes),
            ("compound", Some(kind)) => {
                // Its paths' subpaths together, each child a unit, as a compound path's are when it sews.
                let mut paths = Vec::new();
                for child in &kind.children {
                    self.charge(1)?;
                    paths.extend(child.kind.iter().filter(|k| k.name == "path").flat_map(|k| &k.path));
                }
                return self.clip_path(&paths, kind.rule.as_deref(), label, who, shapes);
            }
            ("group" | "layer", Some(kind)) => {
                // A clip group clips to its own clipping object, and a layer or another group to its members
                // that show, a Shaper group's sources left out.
                let members: Vec<&Node> = if kind.clip && name == "group" {
                    kind.children.iter().take(1).collect()
                } else {
                    kind.children.iter().skip(usize::from(node.shaper.is_some())).filter(|member| member.visible).collect()
                };
                for member in members {
                    self.clip_shapes(member, label, who, shapes)?;
                }
                return Ok(());
            }
            ("text", _) => ("text".to_string(), Some("Make paths of the text with Type › Create Outlines.")),
            ("image", _) => ("an image".to_string(), Some(DRAW_A_PATH)),
            ("placeddocument", _) => ("a placed document".to_string(), Some(DRAW_A_PATH)),
            ("mesh", _) => ("a gradient mesh".to_string(), Some(DRAW_A_PATH)),
            ("blend", _) => ("a blend".to_string(), Some(EXPAND)),
            ("envelope", _) => ("an envelope".to_string(), Some(EXPAND)),
            ("repeat", _) => ("a repeat".to_string(), Some(EXPAND)),
            ("intertwine", _) => ("an Intertwine object".to_string(), Some("Give its objects back with Object › Intertwine › Release.")),
            (other, _) => (format!("an object of the kind {}", quoted(other)), None),
        };
        let message = format!("{who} clips what it holds to {what}, which StitchCraft doesn't read, and it adds nothing to the clip.");
        self.feature(label, message, hint);
        Ok(())
    }

    /// The shape of a path or a compound path in the clip of `label`, `who`, in `shapes`: the subpaths of
    /// `data` filled by `rule`. A path that can't be used adds nothing, with `SC-W0808`.
    fn clip_path(&mut self, data: &[&PathData], rule: Option<&str>, label: &str, who: &str, shapes: &mut Vec<ClipShape>) -> Result<(), Diagnostic> {
        match geometry::path(data, &mut self.meter).map_err(|_| exhausted())? {
            Ok(path) => {
                let rule = self.rule(label, rule);
                shapes.push(ClipShape { path, rule });
            }
            Err(why) => {
                let message =
                    format!("{who} clips what it holds to a path that can't be used, which adds nothing to the clip: {}.", lower(why.why()));
                self.warn(Code::VcGeometryUnusable, Some(label), message, Some(UNUSABLE_HINT));
            }
        }
        Ok(())
    }
}

/// `text` with its first letter in lower case, to follow a colon.
fn lower(text: &str) -> String {
    let mut chars = text.chars();
    chars.next().map(|first| first.to_lowercase().chain(chars).collect()).unwrap_or_default()
}
