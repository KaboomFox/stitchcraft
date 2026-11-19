//! Names of shapes: the words a person finds a shape by in VectorCraft's Layers panel, in the notices and
//! in the report (`REQ-AUTO-125` to `REQ-AUTO-127`; `docs/src/design/image-to-embroidery.md` › Names of
//! shapes).
//!
//! **Why names.** A traced photo has hundreds of shapes, and every manual control (a mark, a guide, a
//! shape's own effect) starts with finding the shape. A notice named it by its element id, a number the
//! Layers panel never shows. So planning gives each automatic shape without a name a name of its thread, how
//! it sews and its number, [`name`] makes it, and `stitchcraft_core::diag::called` writes it in every message.
//!
//! **Why the Layers panel's order.** The sewing order is chosen again at every plan, and one deleted shape
//! moves shapes far from it: a number counted in it renamed up to 3 in 4 of the other shapes of a traced
//! dog. A number counted among the shapes of one thread and one way of sewing, from the Layers panel's top
//! row, changes only for those of that thread and kind below the one deleted. The sewing place is given
//! beside the name instead, at each plan ([`Shape::place`]).
//!
//! **Why once.** A host writes the names into its document (`stitch plan picture.png --traced`), and from
//! then on they are the shapes' own names, kept as a person's are (`REQ-AUTO-126`). A name in use is never
//! given again: the number goes on to the next one free.
//!
//! **What a shape is.** The elements a host reads from one object of the design: their ids share all but
//! the last `:` part (`vc:6:fill` and `vc:6:stroke` are of the shape `vc:6`). The parts digitizing makes of
//! an element are its id with `#` or `~` and more (`vc:6:fill#2`), and are of its shape too.

use std::collections::{BTreeMap, BTreeSet};

use stitchcraft_core::{Diagnostic, ElementId, Exhausted, Meter};
use stitchcraft_params::StitchType;
use stitchcraft_plan::palette::Palette;
use stitchcraft_plan::{Rgb, Thread};

use crate::design::{Design, Element};
use crate::generate::stitch_type_of;

/// The marker a name of StitchCraft's begins with, which a name planning gives never does.
const MARKER: &str = "stitch:";

/// How a shape sews, as its name says it: by the stitches of its main part, the one that keeps its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Kind {
    /// A fill of any method.
    Fill,
    /// A satin column of any kind.
    Column,
    /// A running stitch, or another stitch along a line.
    Line,
    /// Nothing: a shape left out, as a picture's background is, or merged whole into its neighbours.
    LeftOut,
}

impl Kind {
    /// The word a name says it with.
    pub const fn label(self) -> &'static str {
        match self {
            Kind::Fill => "fill",
            Kind::Column => "column",
            Kind::Line => "line",
            Kind::LeftOut => "left out",
        }
    }

    /// How an element sewn by `stitch` sews; a fill when its stitch type doesn't read, as its settings'
    /// notice says.
    fn of(stitch: Option<StitchType>) -> Kind {
        match stitch {
            Some(StitchType::SatinColumn | StitchType::SatinZigzag | StitchType::EStitch | StitchType::SStitch | StitchType::ZigzagStitch) => {
                Kind::Column
            }
            Some(StitchType::RunningStitch | StitchType::ManualStitch | StitchType::RippleStitch) => Kind::Line,
            _ => Kind::Fill,
        }
    }
}

/// The name of the `number`-th shape of a thread named `thread` that sews as `kind`: `Red fill (3)`. The
/// number stands in brackets, so that the Layers panel's search for `fill (1)` doesn't find `fill (12)`.
pub fn name(thread: &str, kind: Kind, number: usize) -> String {
    format!("{thread} {} ({number})", kind.label())
}

/// The name a thread is called by in a shape's name: its own, or the name of the machine palette's thread of
/// its colour exactly, or else its colour's hex code. A name that begins with StitchCraft's marker is a
/// marker's, so the colour names it then.
pub fn thread_name(thread: &Thread, palette: &Palette) -> String {
    let own = thread.name.as_deref().filter(|name| !name.trim().is_empty() && !marker(name));
    let named = own.or_else(|| palette.entries.iter().find(|entry| entry.color == thread.color).map(|entry| entry.name));
    named.map_or_else(|| thread.color.to_string(), str::to_string)
}

/// Whether `name` begins with StitchCraft's marker, in any case.
fn marker(name: &str) -> bool {
    name.trim_start().get(..MARKER.len()).is_some_and(|start| start.eq_ignore_ascii_case(MARKER))
}

/// One shape of a design and what the report says of it.
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// Its name: its own, or the one planning gave it; `None` for a shape with settings and no name.
    pub name: Option<String>,
    /// The id its elements share, without their last part: `vc:6`.
    pub element: String,
    /// The colour of its lowest element's thread.
    pub thread: Rgb,
    /// How it sews.
    pub kind: Kind,
    /// Its place in the sewing order, from 1, at this plan; `None` for a shape that sews nothing.
    pub place: Option<usize>,
}

/// The shapes of a design, in the Layers panel's order, and the ids that name each.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Names {
    /// Each shape's index in `shapes`, by its id and by the id of each of its elements.
    ids: BTreeMap<String, usize>,
    /// The shapes, the Layers panel's top row first.
    shapes: Vec<Shape>,
}

/// A shape while it is named: whether it is automatic, its lowest element's thread and id, and whether its
/// kind is that of the lowest element's own part.
struct Found {
    automatic: bool,
    thread: Thread,
    lowest: ElementId,
    main: bool,
}

impl Names {
    /// The shapes of `design`, each automatic one without a name named (`REQ-AUTO-125`), with `meter`'s
    /// work: their kinds from `sewn`, the elements as they are generated, but for the base layer's pieces,
    /// `base`, which are of no shape; their threads named with `palette`'s names; and each placed by the
    /// first of the elements of `order`, the order the steps sew in, that is of it.
    pub(crate) fn given(
        design: &Design,
        (sewn, base, order): (&[Element], &[ElementId], &[&ElementId]),
        palette: &Palette,
        meter: &mut Meter,
    ) -> Result<Names, Exhausted> {
        let mut names = Names::default();
        let mut found: Vec<Found> = Vec::new();
        // The Layers panel's top row is the last object drawn.
        for element in design.elements().iter().rev() {
            meter.charge(1)?;
            let key = shape_of(element.id.as_str());
            let at = match names.ids.get(key) {
                Some(&at) => at,
                None => {
                    let at = names.shapes.len();
                    let thread = element.thread.color;
                    names.shapes.push(Shape { name: None, element: key.to_string(), thread, kind: Kind::LeftOut, place: None });
                    found.push(Found { automatic: false, thread: element.thread.clone(), lowest: element.id.clone(), main: false });
                    names.ids.insert(key.to_string(), at);
                    at
                }
            };
            names.ids.insert(element.id.as_str().to_string(), at);
            if let (Some(shape), Some(found)) = (names.shapes.get_mut(at), found.get_mut(at)) {
                // A lower element of the shape gives its thread, and the shape has its object's name.
                shape.thread = element.thread.color;
                shape.name = element.name.clone().or(shape.name.take());
                found.automatic |= element.automatic;
                found.thread = element.thread.clone();
                found.lowest = element.id.clone();
            }
        }
        // How each shape sews: by its main part, the one with its lowest element's own id, or else its first
        // part.
        for element in sewn.iter().filter(|element| !base.contains(&element.id)) {
            meter.charge(1)?;
            let Some(at) = names.at(element.id.as_str()) else { continue };
            let Some((shape, found)) = names.shapes.get_mut(at).zip(found.get_mut(at)) else { continue };
            let main = found.lowest == element.id;
            if !found.main && (main || shape.kind == Kind::LeftOut) {
                shape.kind = Kind::of(stitch_type_of(element, &design.settings));
                found.main = main;
            }
        }
        // The names in use are never given again (`REQ-AUTO-126`).
        let mut taken: BTreeSet<String> = names.shapes.iter().filter_map(|shape| shape.name.clone()).collect();
        let mut counted: BTreeMap<(String, Kind), usize> = BTreeMap::new();
        for (shape, found) in names.shapes.iter_mut().zip(&found) {
            if shape.name.is_some() || !found.automatic {
                continue;
            }
            let thread = thread_name(&found.thread, palette);
            let number = counted.entry((thread.clone(), shape.kind)).or_insert(0);
            let given = loop {
                meter.charge(1)?;
                *number = number.saturating_add(1);
                let given = name(&thread, shape.kind, *number);
                if !taken.contains(&given) {
                    break given;
                }
            };
            taken.insert(given.clone());
            shape.name = Some(given);
        }
        // The sewing order: a shape's place is that of its first step.
        let mut places = 0;
        for id in order.iter().filter(|id| !base.contains(id)) {
            meter.charge(1)?;
            let Some(shape) = names.at(id.as_str()).and_then(|at| names.shapes.get_mut(at)) else { continue };
            if shape.place.is_none() {
                places += 1;
                shape.place = Some(places);
            }
        }
        Ok(names)
    }

    /// The shape the element or the shape `id` names is, if any.
    pub fn of(&self, id: &str) -> Option<&Shape> {
        self.at(id).and_then(|at| self.shapes.get(at))
    }

    /// The name of the shape `id` is of, if it has one.
    pub fn name_of(&self, id: &str) -> Option<&str> {
        self.of(id).and_then(|shape| shape.name.as_deref())
    }

    /// The index of the shape `id` is of: an element's or a shape's own id, or a part's, which is an
    /// element's id with `#` or `~` and more after it.
    fn at(&self, id: &str) -> Option<usize> {
        if let Some(&at) = self.ids.get(id) {
            return Some(at);
        }
        id.char_indices().rev().filter(|(_, c)| matches!(c, '#' | '~')).find_map(|(i, _)| id.get(..i).and_then(|part| self.ids.get(part)).copied())
    }

    /// The shapes in the sewing order, then those that sew nothing in the Layers panel's order.
    pub fn sewing_order(&self) -> Vec<&Shape> {
        let mut shapes: Vec<&Shape> = self.shapes.iter().collect();
        shapes.sort_by_key(|shape| shape.place.unwrap_or(usize::MAX));
        shapes
    }

    /// Gives each of `diagnostics` that concerns an element of a shape with a name that name.
    pub fn tell(&self, diagnostics: &mut [Diagnostic]) {
        for diagnostic in diagnostics {
            let name = diagnostic.element.as_ref().and_then(|element| self.name_of(element.as_str()));
            if let Some(name) = name {
                diagnostic.name = Some(name.to_string());
            }
        }
    }
}

/// The shape an element's `id` is of: the id without its last `:` part, or the whole id when it has one
/// part after its host's.
fn shape_of(id: &str) -> &str {
    match id.rsplit_once(':') {
        Some((shape, _)) if shape.contains(':') => shape,
        _ => id,
    }
}

#[cfg(test)]
mod tests;
