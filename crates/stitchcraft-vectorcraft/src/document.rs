//! Reading a VectorCraft document into a [`Design`]: which objects are stitched, how, and in which thread.
//!
//! The reader walks the layers once, bottom first, and each container's children in order: VectorCraft's
//! paint order, and so the stitching order. Every path is already in document coordinates (VectorCraft
//! moves a group's anchors when it moves the group), in points with y down, so the reader only scales
//! them into millimetres. Layers, groups and compound paths carry no transform.
//!
//! **What an object sews** (`REQ-VC-004`). An object with visible StitchCraft effects sews what they say,
//! one element for each, in their order: a `fill` effect fills its shape, a `stroke` effect stitches
//! along it, a `satin` effect makes a satin column of it, with the effect's parameters. A fill takes the
//! colour of the topmost fill VectorCraft paints over the object, and the others that of the topmost
//! stroke, whose width and join they follow, falling back on the other kind. An opaque paint comes before
//! one that lets what is under it show ([`topmost`]). An object without them sews as Ink/Stitch sews an
//! SVG shape: the fills VectorCraft draws, then its strokes as running stitch, each in its paint's colour.
//!
//! **A group's effects** (`REQ-VC-006`). VectorCraft runs a group's or a layer's geometry effects on each
//! path and compound path inside it, however deep, and paints what they return with the path's own paints.
//! A group's own paints paint the shapes it holds too, over them unless its Contents row is moved up, and
//! give an effect its thread and stroke when they are on top ([`painting`]). Applying a graphic style to a
//! group puts its effects and paints there. So the paths inside sew by the StitchCraft effects of the
//! group, as the previews show them. A path that neither its own paints nor a group's paint is invisible,
//! and is left out without a word. A clip group's first object is its clipping object. VectorCraft paints a
//! path, a compound path or text there with its own paints alone, and nothing else. The group's effects run
//! on a path or a compound path there too. A path those effects reach from more than one level, its own and
//! a group's, or 2 groups', is drawn in VectorCraft as one effect run on the stitches of another, which a
//! machine cannot sew. It sews by the nearest, and `SC-W0807` says so.
//!
//! **Ids.** A design element is `vc:<label>:<what>`. The label is the object's id, which VectorCraft keeps
//! when it saves, or `#<n>` when it has none (the n-th object read), with `~2`, `~3`, … when it repeats
//! ([`Labels`]). `<what>` is `fill` or `stroke` for a paint, `fill.2` and so on from an object's second,
//! and the family for an effect. The reader's own diagnostics name the object: `vc:<label>`.
//!
//! **Nothing is dropped silently** (`REQ-VC-005`). `SC-W0807` names what is not stitched, `SC-W0808`
//! geometry that cannot be used and `SC-I0809` the layers and guides VectorCraft does not print. What
//! VectorCraft does not draw is left out without a word: a hidden object, an object or a paint of opacity
//! 0, a paint turned off, and a stroke of width 0.
//!
//! **Work is bounded.** Besides the bytes [`crate::container`] charges, each object, appearance item,
//! effect and effect parameter costs one unit, and each subpath one unit and one more for each anchor. An
//! element costs one unit and one more for each segment of its path, which it holds a copy of, so that
//! many paints or effects on one long path cost what they make. An object with StitchCraft effects costs
//! one more for each group or layer around it that paints.

use std::cell::OnceCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::Value;
use stitchcraft_core::{Budget, Code, Diagnostic, ElementId, Fix, Labels, Meter, Mm};
use stitchcraft_engine::design::{Design, DesignSettings, Element, FillRule, Join, Path, Shape};
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_params::ParamSet;
use stitchcraft_params::stitch_type::Family;
use stitchcraft_plan::{Rgb, Thread};

use crate::container::{self, unreadable};
use crate::geometry::{self, UNUSABLE_HINT};
use crate::values::{sews, text};
use crate::wire::{self, Effect, Item, Kind, Node, Paint, PathData};
use crate::{FORMAT, PLUGIN_NAMESPACE, color};

/// A stroke's miter limit when the file leaves it out: VectorCraft's, as Illustrator's.
const MITER_LIMIT: f64 = 10.0;

/// How many characters of a name or a value a message quotes.
const QUOTED: usize = 64;

/// The thread of a paint whose colour cannot be known.
const BLACK: Rgb = Rgb::new(0, 0, 0);

/// How an object VectorCraft computes as it draws it becomes paths.
const EXPAND: &str = "Make paths of it with Object › Expand.";

/// How what one of VectorCraft's effects draws becomes paths.
const EXPAND_EFFECT: &str = "Make paths of what the effect draws with Object › Expand Appearance.";

/// What a `.vectorcraft` file holds for StitchCraft.
#[derive(Clone, Debug, PartialEq)]
pub struct VectorCraft {
    /// Every object that sews, in stitching order.
    pub design: Design,
    /// What was left out or simplified, in document order.
    pub warnings: Vec<Diagnostic>,
}

/// The design in the `.vectorcraft` file `bytes`, read within `budget`. Fails with `SC-E0806` when the file
/// cannot be read, and with `SC-E0004` when reading it needs more work than the budget allows.
pub fn read(bytes: &[u8], budget: &Budget) -> Result<VectorCraft, Diagnostic> {
    let mut meter = budget.meter();
    let json = container::json(bytes, &mut meter)?;
    let file: wire::File = serde_json::from_slice(&json).map_err(|e| unreadable(&format!("Its JSON does not read: {e}.")))?;
    let mut reader = Reader { meter, labels: Labels::default(), objects: 0, elements: Vec::new(), warnings: Vec::new() };
    reader.header(file.format.as_deref(), file.version.as_ref())?;
    let document = file.document.ok_or_else(|| unreadable("It holds no document."))?;
    for layer in &document.layers {
        reader.node(layer, &Around::default())?;
    }
    let design = Design::new(reader.elements, DesignSettings::default())?;
    Ok(VectorCraft { design, warnings: reader.warnings })
}

/// `SC-E0004` for a file that needs more work than the budget allows.
pub(crate) fn exhausted() -> Diagnostic {
    Diagnostic::new(Code::BudgetExhausted, "Reading the VectorCraft file needed more work than the budget allows.")
}

/// A StitchCraft live effect, by the stitch family its id names.
enum Ours<'a> {
    /// One of the stitch families, with the effect's parameters.
    Family(Family, &'a Value),
    /// A StitchCraft effect that names no stitch family, such as a newer version's.
    Other(&'a str),
}

/// The StitchCraft effect `effect` is, if it is one.
fn ours(effect: &Effect) -> Option<Ours<'_>> {
    let name = effect.id.strip_prefix("plugin.")?.strip_prefix(PLUGIN_NAMESPACE)?.strip_prefix('.')?;
    Some(Family::ALL.iter().find(|f| f.id() == name).map_or(Ours::Other(name), |f| Ours::Family(*f, &effect.params)))
}

/// An object's style of stroke: its width and how it turns corners.
type Style = (Mm, Join);

/// The visible StitchCraft effects of an object, or of the groups and layers around it, in their order,
/// each with its parameters.
type Effects<'a> = Vec<(Family, &'a Value)>;

/// What the groups and layers around an object give it.
#[derive(Default)]
struct Around<'a> {
    /// The StitchCraft effects of the nearest that has any, shared by everything inside it.
    effects: Rc<[(Family, &'a Value)]>,
    /// How many of them have StitchCraft effects.
    levels: u32,
    /// The paints of the groups and layers around that paint, the outermost first: none for a clip group's
    /// clipping object, which no group paints.
    painters: Rc<[Rc<Paints<'a>>]>,
    /// Whether the object is a clip group's clipping object, which VectorCraft paints only when it is a
    /// path, a compound path or text.
    clipping: bool,
}

/// A group's or a layer's own paints, which VectorCraft paints its contents with: the items before the
/// Contents row of its Appearance panel (`contents_index`) under them, and the others over them, all of
/// them by default. What is said of them is said of the group.
struct Paints<'a> {
    /// The group's label.
    label: String,
    /// The paints under its contents.
    below: Part<'a>,
    /// The paints over its contents.
    above: Part<'a>,
}

/// Paints in the order VectorCraft draws them, the last on top, and the fill and the stroke an effect
/// takes from them ([`Reader::fill_of`], [`Reader::stroke_of`]).
struct Part<'a> {
    /// The topmost fill VectorCraft draws, an opaque one before others, and whether it is opaque.
    fill: Option<(&'a Item, bool)>,
    /// The topmost stroke, the same way.
    stroke: Option<(&'a Item, bool)>,
    /// The thread of `fill`, found once.
    fill_thread: OnceCell<Option<Thread>>,
    /// The thread of `stroke`, with its width and join, found once.
    stroke_thread: OnceCell<Option<(Thread, Style)>>,
}

impl<'a> Part<'a> {
    fn new(items: &'a [Item]) -> Self {
        Part { fill: topmost(items, "fill"), stroke: topmost(items, "stroke"), fill_thread: OnceCell::new(), stroke_thread: OnceCell::new() }
    }

    /// Whether it paints anything.
    fn paints(&self) -> bool {
        self.fill.is_some() || self.stroke.is_some()
    }
}

/// The topmost of `items` of the `kind` given, `fill` or `stroke`, that VectorCraft draws: the topmost
/// opaque one if there is one, as one of opacity under 1 lets what is under it show. With whether it is
/// opaque.
fn topmost<'a>(items: &'a [Item], kind: &str) -> Option<(&'a Item, bool)> {
    let candidates = || items.iter().rev().filter(move |item| item.kind == kind && drawn(item));
    let opaque = |item: &&Item| item.opacity.is_none_or(|opacity| opacity >= 1.0);
    candidates().find(opaque).map(|item| (item, true)).or_else(|| candidates().next().map(|item| (item, false)))
}

/// The paints the StitchCraft effects on one object take their thread and stroke from, in the order
/// VectorCraft draws them, the topmost first, each with the label of the object or the group whose they
/// are ([`painting`]).
type Painting<'p, 'a> = Vec<(&'p str, &'p Part<'a>)>;

/// The paints over the object `label`, whose own are `own`, the topmost first: the paints of the groups
/// around it over their contents, the outermost first, then its own, then the groups' paints under their
/// contents, the innermost first. Each group paints what it holds after the effects of the groups around
/// it have run, and the object's own paints what its own effects draw too.
fn painting<'p, 'a>(label: &'p str, own: &'p Part<'a>, around: &'p Around<'a>) -> Painting<'p, 'a> {
    let above = |p: &'p Rc<Paints<'a>>| (p.label.as_str(), &p.above);
    let below = |p: &'p Rc<Paints<'a>>| (p.label.as_str(), &p.below);
    around.painters.iter().map(above).chain(std::iter::once((label, own))).chain(around.painters.iter().rev().map(below)).collect()
}

/// The first of `painting`'s paints whose fill or stroke, as `of` takes it, is opaque, or else the first
/// that has one.
fn first_of<'p, 'a>(painting: &Painting<'p, 'a>, of: impl Fn(&Part<'a>) -> Option<(&'a Item, bool)>) -> Option<(&'p str, &'p Part<'a>)> {
    let opaque = painting.iter().find(|(_, part)| of(part).is_some_and(|(_, opaque)| opaque));
    opaque.or_else(|| painting.iter().find(|(_, part)| of(part).is_some())).copied()
}

/// Whether VectorCraft draws nothing of `node`, a path or a compound path that the StitchCraft effects of
/// the groups `around` reach: it has none of its own, and neither its own paints nor the groups' paint it.
fn unseen(node: &Node, around: &Around) -> bool {
    node.kind.as_ref().is_some_and(|kind| !kind.guide && matches!(kind.name.as_str(), "path" | "compound"))
        && !around.effects.is_empty()
        && !node.appearance.effects.iter().any(|effect| effect.visible && matches!(ours(effect), Some(Ours::Family(..))))
        && !Part::new(&node.appearance.items).paints()
        && !around.painters.iter().any(|p| p.above.paints() || p.below.paints())
}

/// What a name that begins with `stitch:` asks of StitchCraft (`REQ-VC-009`).
#[derive(Clone, Copy)]
enum Marker {
    /// Leave the object out, with everything in it.
    Ignore,
    /// Where the shape grouped with the marker starts.
    Start,
    /// Where it ends.
    End,
    /// No marker StitchCraft reads.
    Unknown,
}

/// The marker `name` is: `stitch:` and a word, alone or followed by a space and a note, in any mix of upper
/// and lower case. `None` for a name that doesn't begin with `stitch:`.
fn marker(name: &str) -> Option<Marker> {
    const PREFIX: &str = "stitch:";
    let name = name.trim_start();
    if !name.get(..PREFIX.len())?.eq_ignore_ascii_case(PREFIX) {
        return None;
    }
    let word = name.get(PREFIX.len()..)?.split(char::is_whitespace).next().unwrap_or_default();
    Some(match word.to_ascii_lowercase().as_str() {
        "ignore" => Marker::Ignore,
        "start" => Marker::Start,
        "end" => Marker::End,
        _ => Marker::Unknown,
    })
}

struct Reader {
    meter: Meter,
    labels: Labels,
    /// How many objects the walk has met: the number of an object without an id.
    objects: u64,
    elements: Vec<Element>,
    warnings: Vec<Diagnostic>,
}

impl Reader {
    /// Checks the file's format and its version, which may be newer than the adapter reads.
    fn header(&mut self, format: Option<&str>, version: Option<&Value>) -> Result<(), Diagnostic> {
        match format {
            Some("vectorcraft" | "drawcraft") => {}
            Some(other) => return Err(unreadable(&format!("It names the format {}, where VectorCraft names `vectorcraft`.", quoted(other)))),
            None => return Err(unreadable("It names no format, where VectorCraft names `vectorcraft`.")),
        }
        let version =
            version.and_then(Value::as_u64).filter(|v| *v >= 1).ok_or_else(|| unreadable("Its format version is not a whole number from 1."))?;
        if version > FORMAT {
            let message = format!(
                "The file is in format {version}, newer than format {FORMAT}, the newest StitchCraft reads: it is read as format {FORMAT}, and what is newer may be left out."
            );
            self.warn(Code::VcNewerFormat, None, message, Some("Compare the stitches with the drawing."));
        }
        Ok(())
    }

    fn charge(&mut self, units: u64) -> Result<(), Diagnostic> {
        self.meter.charge(units).map_err(|_| exhausted())
    }

    /// One object and what is inside it, inside groups and layers whose StitchCraft effects are `around`.
    fn node<'a>(&mut self, node: &'a Node, around: &Around<'a>) -> Result<(), Diagnostic> {
        let appearance = &node.appearance;
        let effects = appearance.effects.len() + appearance.items.iter().map(|item| item.effects.len()).sum::<usize>();
        self.charge(1 + (appearance.items.len() + effects) as u64)?;
        self.objects += 1;
        let base = node.id.map_or_else(|| format!("#{}", self.objects), |id| id.to_string());
        let label = self.labels.claim(base, &mut self.meter).map_err(|_| exhausted())?;
        // VectorCraft draws nothing for these, and nothing is said.
        let clipped_away = around.clipping && !node.kind.as_ref().is_some_and(|kind| matches!(kind.name.as_str(), "path" | "compound" | "text"));
        if !node.visible || node.opacity == Some(0.0) || clipped_away || unseen(node, around) {
            return Ok(());
        }
        // Then the name, which can leave the object out or mark where a shape starts or ends.
        if let Some(marked) = node.name.as_deref().and_then(marker) {
            let what = match node.kind.as_ref().map(|kind| kind.name.as_str()) {
                Some("layer") => "The layer",
                Some("group") => "The group",
                _ => "The object",
            };
            let shown = called(node, what);
            match marked {
                Marker::Ignore => {
                    let message = format!("{shown} is named stitch:ignore, and is left out with everything in it.");
                    self.warn(Code::VcObjectIgnored, Some(&label), message, Some("Rename it to stitch it."));
                    return Ok(());
                }
                Marker::Start | Marker::End => {
                    let (verb, noun) = if matches!(marked, Marker::Start) { ("starts", "start") } else { ("ends", "end") };
                    let message = format!(
                        "{shown} marks where the shape grouped with it {verb}, and is not stitched. StitchCraft doesn't yet {noun} a shape \
                         at a marker: it chooses where."
                    );
                    self.feature(&label, message, None);
                    return Ok(());
                }
                Marker::Unknown => {
                    let message = format!("{shown} begins with stitch:, which names no marker StitchCraft reads, and is stitched as it is drawn.");
                    self.feature(&label, message, Some("StitchCraft reads stitch:ignore, stitch:start and stitch:end."));
                }
            }
        }
        if node.mask.is_some() {
            self.feature(&label, format!("{} has an opacity mask, which is not applied: it is stitched whole.", called(node, "The object")), None);
        }
        let Some(kind) = &node.kind else {
            self.feature(&label, format!("{} has no kind, and is not stitched.", called(node, "An object")), None);
            return Ok(());
        };
        match kind.name.as_str() {
            "layer" if kind.template => {
                let message =
                    format!("{} is a template, which VectorCraft shows dimmed and doesn't print, so it is not stitched.", called(node, "A layer"));
                self.warn(Code::VcNotPrinted, Some(&label), message, Some("Turn its Template option off in Layer Options to stitch it."));
            }
            "layer" if !kind.printable => {
                let message = format!("{} doesn't print, so it is not stitched.", called(node, "A layer"));
                self.warn(Code::VcNotPrinted, Some(&label), message, Some("Turn its Print option on in Layer Options to stitch it."));
            }
            "layer" | "group" => self.container(node, kind, &label, around)?,
            "path" if kind.guide => {
                let message = format!("{} is a guide, which VectorCraft doesn't print, so it is not stitched.", called(node, "A path"));
                self.warn(Code::VcNotPrinted, Some(&label), message, Some("Release the guide to stitch it."));
            }
            "path" => self.shape(node, &label, &kind.path.iter().collect::<Vec<_>>(), kind.rule.as_deref(), around)?,
            "compound" => self.compound(node, kind, &label, around)?,
            "text" => {
                let message = format!("{} is not stitched.", called(node, "Text"));
                self.feature(&label, message, Some("Make paths of it with Type › Create Outlines."));
            }
            "image" => {
                let message = format!("{} is not stitched.", called(node, "An image"));
                self.feature(&label, message, Some("Trace it, or draw over it, to stitch it."));
            }
            "symbolinstance" => {
                let message = format!("{} is not stitched.", called(node, "A symbol"));
                self.feature(&label, message, Some("Make paths of it with Break Link to Symbol, in the Symbols panel."));
            }
            "blend" | "envelope" | "mesh" | "repeat" | "placeddocument" | "intertwine" => {
                let (what, hint) = match kind.name.as_str() {
                    "blend" => ("A blend", EXPAND),
                    "envelope" => ("An envelope", EXPAND),
                    "mesh" => ("A gradient mesh", "Draw over it to stitch it."),
                    "repeat" => ("A repeat", EXPAND),
                    "placeddocument" => ("A placed document", EXPAND),
                    _ => ("An Intertwine object", "Give its objects back with Object › Intertwine › Release to stitch them."),
                };
                let message = format!("{} is not stitched: VectorCraft computes what it draws as it draws it.", called(node, what));
                self.feature(&label, message, Some(hint));
            }
            other => {
                let message =
                    format!("{} is of the kind {}, which StitchCraft doesn't read, and is not stitched.", called(node, "An object"), quoted(other));
                self.feature(&label, message, None);
            }
        }
        Ok(())
    }

    /// A layer or a group: its children in order, but a Shaper group's first, the sources it shows the
    /// shapes of. Its StitchCraft effects reach the paths inside it, which `around` adds to.
    fn container<'a>(&mut self, node: &'a Node, kind: &'a Kind, label: &str, around: &Around<'a>) -> Result<(), Diagnostic> {
        let what = if kind.name == "layer" { "A layer" } else { "A group" };
        if kind.clip {
            let message = format!("{} clips what it holds, and the clipping is not applied: what it holds is stitched whole.", called(node, what));
            self.feature(label, message, Some("Cut the art to shape with Pathfinder, then release the clipping mask."));
        }
        let appearance = &node.appearance;
        let paints = appearance.items.iter().any(drawn).then(|| {
            let message = format!(
                "{} has fills or strokes of its own, which VectorCraft paints what it holds with. They are not stitched as shapes \
                 of their own, and give their thread and stroke to the StitchCraft effects they paint.",
                called(node, what)
            );
            self.feature(label, message, Some("Make objects of them with Object › Expand Appearance."));
            let (below, above) = appearance.items.split_at(appearance.contents_index.unwrap_or(0).min(appearance.items.len()));
            Rc::new(Paints { label: label.to_string(), below: Part::new(below), above: Part::new(above) })
        });
        let unused = |id: &str| format!("{} has the effect {id}, which is not applied to what it holds.", called(node, what));
        let hint = "Apply effects to the paths inside, or make paths of what they draw with Object › Expand Appearance.";
        let effects = self.effects(node, label, unused, hint);
        let gives = !effects.is_empty();
        let inside = (gives || paints.is_some()).then(|| Around {
            effects: if gives { effects.into() } else { Rc::clone(&around.effects) },
            levels: around.levels.saturating_add(u32::from(gives)),
            painters: match paints {
                Some(paints) => around.painters.iter().cloned().chain([paints]).collect(),
                None => Rc::clone(&around.painters),
            },
            clipping: false,
        });
        let inside = inside.as_ref().unwrap_or(around);
        for (k, child) in kind.children.iter().enumerate() {
            match (k, kind.clip, node.shaper.is_some()) {
                // A clip group's first object is its clipping object. VectorCraft runs the effects of the
                // groups around it on it too, and paints it with its own paints alone.
                (0, true, _) => {
                    let clipping = Around { effects: Rc::clone(&inside.effects), levels: inside.levels, painters: Rc::default(), clipping: true };
                    self.node(child, &clipping)?;
                }
                // A Shaper group's first object holds the sources it shows the shapes of.
                (0, false, true) => {}
                _ => self.node(child, inside)?,
            }
        }
        Ok(())
    }

    /// The visible StitchCraft effects of `node`, in their order. A StitchCraft effect that names no stitch
    /// family is named, and so is each of VectorCraft's own effects: `unused` says what becomes of the one
    /// whose quoted id it is given, and `hint` how to stitch what it draws.
    fn effects<'a>(&mut self, node: &'a Node, label: &str, unused: impl Fn(&str) -> String, hint: &str) -> Effects<'a> {
        let mut effects = Vec::new();
        for effect in node.appearance.effects.iter().filter(|e| e.visible) {
            match ours(effect) {
                Some(Ours::Family(family, params)) => effects.push((family, params)),
                Some(Ours::Other(name)) => {
                    let message = format!("The StitchCraft effect {} stitches nothing, and is not used.", quoted(name));
                    self.feature(label, message, Some("Use StitchCraft's Fill, Stroke or Satin effect."));
                }
                None => self.feature(label, unused(&quoted(&effect.id)), Some(hint)),
            }
        }
        effects
    }

    /// A compound path: its paths' subpaths, one shape with the compound's appearance and fill rule.
    fn compound<'a>(&mut self, node: &'a Node, kind: &'a Kind, label: &str, around: &Around<'a>) -> Result<(), Diagnostic> {
        let mut paths = Vec::new();
        for child in &kind.children {
            self.charge(1)?;
            match child.kind.as_ref() {
                Some(Kind { name, path, .. }) if name == "path" => paths.extend(path.iter()),
                _ => {
                    let message = format!("{} holds something that isn't a path, which is left out of it.", called(node, "A compound path"));
                    self.feature(label, message, None);
                }
            }
        }
        self.shape(node, label, &paths, kind.rule.as_deref(), around)
    }

    /// A path's or a compound path's elements: by the StitchCraft effects that reach it if any do, its own
    /// or else those `around` it, and by its paints if not.
    fn shape<'a>(&mut self, node: &'a Node, label: &str, data: &[&PathData], rule: Option<&str>, around: &Around<'a>) -> Result<(), Diagnostic> {
        let Some(path) = self.path(node, label, data)? else { return Ok(()) };
        let rule = match rule {
            None | Some("NonZero") => FillRule::NonZero,
            Some("EvenOdd") => FillRule::EvenOdd,
            Some(other) => {
                self.feature(label, format!("The fill rule {} is not one StitchCraft reads, and nonzero is used.", quoted(other)), None);
                FillRule::NonZero
            }
        };
        let unused = |id: &str| format!("The effect {id} is not applied: the object is stitched as it is drawn without it.");
        let own = self.effects(node, label, unused, EXPAND_EFFECT);
        let levels = around.levels.saturating_add(u32::from(!own.is_empty()));
        if levels > 1 {
            let message = format!(
                "{} gets StitchCraft effects from more than one level, its own or those of the groups and layers around it, and \
                 VectorCraft runs each on the stitches of the one before: it is sewn as the nearest say.",
                called(node, "The object")
            );
            self.feature(label, message, Some("Keep StitchCraft's effects on the object or on one group around it."));
        }
        for effect in node.appearance.items.iter().filter(|item| drawn(item)).flat_map(|item| &item.effects).filter(|e| e.visible) {
            let (message, hint) = match ours(effect) {
                Some(_) => (
                    "A StitchCraft effect on one fill or stroke of the appearance is not used.".to_string(),
                    "Apply StitchCraft's effects to the object itself.",
                ),
                None => (format!("The effect {} on one of its fills or strokes is not applied.", quoted(&effect.id)), EXPAND_EFFECT),
            };
            self.feature(label, message, Some(hint));
        }
        let effects = if own.is_empty() { &around.effects[..] } else { own.as_slice() };
        if effects.is_empty() {
            return self.by_paints(node, label, &path, rule);
        }
        // Each group around that paints is a step more in finding the thread and the stroke.
        self.charge(around.painters.len() as u64)?;
        let paints = Part::new(&node.appearance.items);
        self.by_effects(node, label, &path, rule, effects, &painting(label, &paints, around))
    }

    /// The elements of an object without StitchCraft effects: the fills VectorCraft draws, then its strokes.
    fn by_paints(&mut self, node: &Node, label: &str, path: &Path, rule: FillRule) -> Result<(), Diagnostic> {
        for item in node.appearance.items.iter().filter(|item| !matches!(item.kind.as_str(), "fill" | "stroke")) {
            self.feature(label, format!("An appearance item of the kind {} is not stitched.", quoted(&item.kind)), None);
        }
        let mut fills = 0;
        for item in node.appearance.items.iter().filter(|item| item.kind == "fill" && drawn(item)) {
            let Some(thread) = self.thread(label, item.paint.as_ref()) else { continue };
            fills += 1;
            let shape = Shape::Fill { path: path.clone(), rule };
            self.element(node, label, &rank("fill", fills), shape, thread, ParamSet::new())?;
        }
        let mut strokes = 0;
        for item in node.appearance.items.iter().filter(|item| item.kind == "stroke" && drawn(item)) {
            let Some(thread) = self.thread(label, item.paint.as_ref()) else { continue };
            let (Some(width), join) = self.style(label, item) else {
                let message = "A stroke's width is not a number, and the stroke is left out.".to_string();
                self.warn(Code::VcGeometryUnusable, Some(label), message, None);
                continue;
            };
            if let Some(brush) = &item.brush {
                let message = format!("A stroke drawn with the brush {} is stitched along its path, without the brush's art.", quoted(brush));
                self.feature(label, message, Some("Make paths of the brush's art with Object › Expand Appearance."));
            }
            strokes += 1;
            let shape = Shape::Stroke { path: path.clone(), width, join };
            self.element(node, label, &rank("stroke", strokes), shape, thread, ParamSet::new())?;
        }
        Ok(())
    }

    /// The elements of an object with StitchCraft effects: one for each, in their order, in the thread and
    /// with the stroke they take from `painting`. A fill takes its thread from the fills first, the others
    /// from the strokes, the stroke a stroke or a satin column follows.
    fn by_effects(
        &mut self,
        node: &Node,
        label: &str,
        path: &Path,
        rule: FillRule,
        effects: &[(Family, &Value)],
        painting: &Painting,
    ) -> Result<(), Diagnostic> {
        let (mut counts, mut black): (BTreeMap<Family, u32>, _) = (BTreeMap::new(), None);
        // Found when the first effect that needs it comes, each once.
        for (family, params) in effects {
            let (thread, style) = match family {
                Family::Fill => (self.fill_of(painting).or_else(|| self.stroke_of(painting).map(|(thread, _)| thread)), None),
                Family::Stroke | Family::Satin => match self.stroke_of(painting) {
                    Some((thread, style)) => (Some(thread), Some(style)),
                    None => (self.fill_of(painting), None),
                },
            };
            // Only the object's own effects can find no paint ([`Reader::shape`]).
            let thread = thread.unwrap_or_else(|| black.get_or_insert_with(|| self.unpainted(node, label)).clone());
            let mut set = self.params(label, *family, params)?;
            let shape = match family {
                Family::Fill => Shape::Fill { path: path.clone(), rule },
                Family::Stroke | Family::Satin => {
                    set.set(SATIN_COLUMN, if *family == Family::Satin { "true" } else { "false" });
                    let (width, join) = style.unwrap_or((Mm::ZERO, join_of(None, None)));
                    Shape::Stroke { path: path.clone(), width, join }
                }
            };
            let count = counts.entry(*family).or_default();
            *count = count.saturating_add(1);
            let what = rank(family.id(), *count);
            self.element(node, label, &what, shape, thread, set)?;
        }
        Ok(())
    }

    /// Adds the element `vc:<label>:<what>`, charged for the copy of the path it holds.
    fn element(&mut self, node: &Node, label: &str, what: &str, shape: Shape, thread: Thread, params: ParamSet) -> Result<(), Diagnostic> {
        let segments: usize = shape.path().subpaths.iter().map(|sub| sub.segments.len()).sum();
        self.charge(1 + segments as u64)?;
        let id = ElementId::new(format!("vc:{label}:{what}"))
            .map_err(|e| Diagnostic::new(Code::InternalCheckFailed, format!("The VectorCraft object `vc:{label}` makes no element id: {e}.")))?;
        self.elements.push(Element { id, name: node.name.clone(), shape, thread, params });
        Ok(())
    }

    /// An effect's parameters as the element's: each value written as the parameters read it. VectorCraft's
    /// own keys and the plug-in's preview level are not the element's ([`sews`]).
    fn params(&mut self, label: &str, family: Family, params: &Value) -> Result<ParamSet, Diagnostic> {
        let mut set = ParamSet::new();
        let hint = Some("Set the parameter again in the effect's dialog.");
        match params {
            Value::Null => {}
            Value::Object(map) => {
                for (key, value) in map.iter().filter(|(key, _)| sews(key)) {
                    self.charge(1)?;
                    match text(value) {
                        Some(text) => {
                            set.set(key.clone(), text);
                        }
                        None => {
                            let message = format!(
                                "The parameter {} of its StitchCraft {} effect is not a number, a toggle, a choice or a list of them, and keeps its default.",
                                quoted(key),
                                family.id()
                            );
                            self.feature(label, message, hint);
                        }
                    }
                }
            }
            _ => {
                let message =
                    format!("The parameters of its StitchCraft {} effect are not names and values, and all keep their defaults.", family.id());
                self.feature(label, message, hint);
            }
        }
        Ok(set)
    }

    /// The thread of the topmost fill among `painting`'s paints, an opaque one before others.
    fn fill_of(&mut self, painting: &Painting) -> Option<Thread> {
        let (label, part) = first_of(painting, |part| part.fill)?;
        part.fill_thread.get_or_init(|| self.thread(label, part.fill?.0.paint.as_ref())).clone()
    }

    /// The thread of the topmost stroke among `painting`'s paints, an opaque one before others, with its
    /// width and join.
    fn stroke_of(&mut self, painting: &Painting) -> Option<(Thread, Style)> {
        let (label, part) = first_of(painting, |part| part.stroke)?;
        part.stroke_thread
            .get_or_init(|| {
                let (stroke, _) = part.stroke?;
                Some((self.thread(label, stroke.paint.as_ref())?, self.stroke_style(label, stroke)))
            })
            .clone()
    }

    /// The width and join of `stroke`, one of the paints of `label`. A width that is not a number is taken as
    /// 0, and said.
    fn stroke_style(&mut self, label: &str, stroke: &Item) -> Style {
        let (width, join) = self.style(label, stroke);
        let width = width.unwrap_or_else(|| {
            let message = "Its stroke's width is not a number, and its effects take it as 0.".to_string();
            self.warn(Code::VcGeometryUnusable, Some(label), message, None);
            Mm::ZERO
        });
        (width, join)
    }

    /// A black thread for an object whose own StitchCraft effects no paint paints, and why.
    fn unpainted(&mut self, node: &Node, label: &str) -> Thread {
        let message = format!("{} has no paint to take a thread colour from, and is sewn in black.", called(node, "The object"));
        self.feature(label, message, Some("Give it a fill or a stroke in the thread's colour."));
        Thread::new(BLACK)
    }

    /// The thread of `paint`: `None` for no paint, and for a gradient without stops, which VectorCraft
    /// draws nothing for.
    fn thread(&mut self, label: &str, paint: Option<&Paint>) -> Option<Thread> {
        let paint = paint?;
        match paint.kind.as_str() {
            "none" => None,
            "solid" => Some(match paint.color.as_ref().and_then(color::rgb) {
                Some(rgb) => Thread { color: rgb, name: paint.swatch.clone() },
                None => self.black(label, "Its colour does not read, and it is sewn in black."),
            }),
            "gradient" => {
                let stops: Vec<Option<Rgb>> = paint.gradient.iter().flat_map(|g| &g.stops).map(|s| s.color.as_ref().and_then(color::rgb)).collect();
                match stops.first()? {
                    Some(first) => {
                        if stops.iter().any(|stop| stop.as_ref() != Some(first)) {
                            self.feature(
                                label,
                                "A gradient is sewn in its first colour.".to_string(),
                                Some("Fill it with one colour, or split it into shapes of one colour each."),
                            );
                        }
                        Some(Thread::new(*first))
                    }
                    None => Some(self.black(label, "A gradient's first colour does not read, and it is sewn in black.")),
                }
            }
            "pattern" => Some(self.black(label, "A pattern is sewn in black.")),
            other => Some(self.black(label, &format!("A paint of the kind {} is sewn in black.", quoted(other)))),
        }
    }

    /// A black thread, and why.
    fn black(&mut self, label: &str, why: &str) -> Thread {
        self.feature(label, why.to_string(), Some("Give it a plain colour."));
        Thread::new(BLACK)
    }

    /// A stroke's width, `None` when it is not a number (VectorCraft writes `null` for one), and its join. A
    /// width below 0 is 0.
    fn style(&mut self, label: &str, item: &Item) -> (Option<Mm>, Join) {
        if let Some(name) = item.join.as_deref().filter(|j| !matches!(*j, "Miter" | "Round" | "Bevel")) {
            self.feature(label, format!("The stroke join {} is not one StitchCraft reads, and a miter is used.", quoted(name)), None);
        }
        let width = item.width.and_then(|w| Mm::from_points(w.max(0.0)).ok());
        (width, join_of(item.join.as_deref(), item.miter_limit))
    }

    /// The object's path in millimetres; `None`, said, when it cannot be used.
    fn path(&mut self, node: &Node, label: &str, data: &[&PathData]) -> Result<Option<Path>, Diagnostic> {
        match geometry::path(data, &mut self.meter).map_err(|_| exhausted())? {
            Ok(path) => Ok(Some(path)),
            Err(why) => {
                self.unusable(node, label, why.why());
                Ok(None)
            }
        }
    }

    /// `SC-W0808` about the object, which is left out because of `why`.
    fn unusable(&mut self, node: &Node, label: &str, why: &str) {
        let message = format!("{why}, and {} is left out.", called(node, "the object"));
        self.warn(Code::VcGeometryUnusable, Some(label), message, Some(UNUSABLE_HINT));
    }

    /// `SC-W0807` (a feature that is not stitched) about the object `label`.
    fn feature(&mut self, label: &str, message: String, hint: Option<&str>) {
        self.warn(Code::VcFeatureIgnored, Some(label), message, hint);
    }

    fn warn(&mut self, code: Code, label: Option<&str>, message: String, hint: Option<&str>) {
        let mut diagnostic = Diagnostic::new(code, message);
        if let Some(id) = label.and_then(|l| ElementId::new(format!("vc:{l}")).ok()) {
            diagnostic = diagnostic.with_element(id);
        }
        if let Some(hint) = hint {
            diagnostic = diagnostic.with_fix(Fix::Hint(hint.to_string()));
        }
        self.warnings.push(diagnostic);
    }
}

/// A stroke's join as VectorCraft draws it: a miter, with its limit, unless round or bevel.
fn join_of(join: Option<&str>, limit: Option<f64>) -> Join {
    match join {
        Some("Round") => Join::Round,
        Some("Bevel") => Join::Bevel,
        // A limit below 1 bevels every corner, as a limit of 1 does.
        _ => Join::Miter { limit: limit.filter(|l| l.is_finite()).unwrap_or(MITER_LIMIT).max(1.0) },
    }
}

/// Whether VectorCraft draws an appearance item: it is turned on and not of opacity 0, its paint is neither
/// none nor a gradient without stops, and a stroke is wider than 0. A width that is not a number is said
/// where it is used.
fn drawn(item: &Item) -> bool {
    let paints = |paint: &Paint| match paint.kind.as_str() {
        "none" => false,
        "gradient" => paint.gradient.as_ref().is_some_and(|gradient| !gradient.stops.is_empty()),
        _ => true,
    };
    item.visible
        && item.opacity != Some(0.0)
        && item.paint.as_ref().is_some_and(paints)
        && (item.kind != "stroke" || item.width.is_none_or(|w| w > 0.0))
}

/// `what` for the first of its kind, `what.n` for the n-th.
fn rank(what: &str, n: u32) -> String {
    if n > 1 { format!("{what}.{n}") } else { what.to_string() }
}

/// The object's name in quotes after `what` (`Text "Title"`), or `what` alone when it has none.
fn called(node: &Node, what: &str) -> String {
    match node.name.as_deref() {
        Some(name) => format!("{what} {}", quoted(name)),
        None => what.to_string(),
    }
}

/// `text` in quotes, cut after [`QUOTED`] characters.
fn quoted(text: &str) -> String {
    let mut chars = text.chars();
    let cut: String = chars.by_ref().take(QUOTED).collect();
    if chars.next().is_some() { format!("\"{cut}…\"") } else { format!("\"{cut}\"") }
}
