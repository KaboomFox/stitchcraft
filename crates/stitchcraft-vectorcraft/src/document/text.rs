//! A text object: left out, or sewn in an embroidery font when it has StitchCraft's Lettering effect
//! (roadmap step M14.4, `REQ-VC-026`, `docs/src/design/vectorcraft-integration.md` › Lettering).
//!
//! **Why the text, not its outlines.** A document keeps a text object's characters, never the outlines of
//! its glyphs, and an embroidery font is digitized by hand for the needle, where the outlines of a screen
//! font are not. So the object's own font, size and spacing are not used: the Lettering effect names an
//! embroidery font and how the text is set in it, and lettering sets the characters
//! ([`stitchcraft_svg::lettering`]).
//!
//! **Where it sews.** Point type's origin is the start of its first baseline, which is where lettering puts
//! a text's origin too. The text is turned as the object's transform turns its baseline. Its size is the
//! effect's, so the transform's scale isn't used, and a mirrored or slanted text sews turned only, which is
//! said.
//!
//! **What isn't set.** Area type wraps its lines in a frame and type on a path follows the path, by the
//! screen font's widths, which an embroidery font doesn't share. Vertical type needs glyphs drawn to sew
//! downwards. Each is left out with `SC-W1111`, as is a text whose font isn't found.

use std::rc::Rc;

use serde_json::Value;
use stitchcraft_core::{Code, Diagnostic, ElementId, Mm, Point, math};
use stitchcraft_engine::design::Element;
use stitchcraft_svg::font::Font;
use stitchcraft_svg::lettering::{self, Lettering, Set};

use super::{Around, Reader, called, exhausted, quoted};
use crate::effect_name;
use crate::lettering::{FONT, LETTERING};
use crate::values::{sews, text};
use crate::wire::{Kind, Node};

/// How a text object without the Lettering effect is stitched.
const WITHOUT: &str = "Give it StitchCraft's Lettering effect to sew it in an embroidery font, or make paths of it with Type › Create Outlines.";

/// How a text that lettering doesn't set is stitched.
const AS_POINT_TYPE: &str = "Write it as point type, a line at a time, or make paths of it with Type › Create Outlines.";

/// The object replacement character, which is the text of a run that is an inline graphic.
const INLINE: char = '\u{FFFC}';

impl Reader<'_> {
    /// A text object: sewn as lettering sets its text when it has a visible Lettering effect of its own, and
    /// left out if not, or when it is a clip group's clipping object. The elements are cut by the clips
    /// `around` it, as a path's are.
    pub(super) fn text(&mut self, node: &Node, kind: &Kind, label: &str, around: &Around) -> Result<(), Diagnostic> {
        let effect = node.appearance.effects.iter().filter(|effect| effect.visible).find(|effect| effect_name(&effect.id) == Some(LETTERING));
        let Some(effect) = effect.filter(|_| !around.clipping) else {
            let message = format!("{} is not stitched.", called(node, "Text"));
            self.feature(label, message, Some(WITHOUT));
            return Ok(());
        };
        let who = called(node, "Text");
        let (asked, how) = self.lettering(label, &effect.params)?;
        let kind_name = kind.text_kind.get().map_or("point", |kind| kind.name.as_str());
        let vertical = kind.vertical.get().copied().unwrap_or(false);
        if kind_name != "point" || vertical {
            let what = match (kind_name, vertical) {
                (_, true) => "vertical type".to_string(),
                ("area", _) => "area type".to_string(),
                ("onpath", _) => "type on a path".to_string(),
                (other, _) => format!("type of the kind {}", quoted(other)),
            };
            self.unlettered(label, format!("{who} is {what}, which lettering doesn't set, and is not stitched."), AS_POINT_TYPE);
            return Ok(());
        }
        let runs = kind.runs.get().map_or(&[][..], Vec::as_slice);
        let bytes: usize = runs.iter().map(|run| run.text.len()).sum();
        self.charge(1 + bytes as u64)?;
        let whole: String = runs.iter().map(|run| run.text.as_str()).collect();
        if whole.contains(INLINE) {
            self.feature(label, format!("An inline graphic of {} is not stitched.", called(node, "text")), None);
        }
        let whole: String = whole.chars().filter(|c| *c != INLINE).collect();
        let Some(font) = self.font(label, &who, &asked) else { return Ok(()) };
        let set = match lettering::set(&font, &whole, &how, self.budget) {
            Ok(set) => set,
            Err(unset) if unset.code == Code::BudgetExhausted => return Err(exhausted()),
            Err(unset) => {
                self.unlettered(label, format!("{who} is not stitched: {}", unset.message), "Write a text with a character the font draws.");
                return Ok(());
            }
        };
        self.warnings.extend(set.warnings.iter().cloned());
        let fill = runs.iter().find(|run| !run.text.is_empty()).and_then(|run| run.style.as_ref()).and_then(|style| style.fill.as_ref());
        let thread = if one_thread(&set) { self.thread(label, fill) } else { None };
        let place = self.place(label, &who, kind.xf.get());
        let made = self.elements.len();
        for element in set.design.elements() {
            let points = element.shape.path().points().count();
            self.charge(1 + points as u64)?;
            let far = || Diagnostic::new(Code::InternalCheckFailed, format!("The VectorCraft object `vc:{label}` sews a letter no number places."));
            let moved = lettering::moved(element, |p| place.of(p).ok_or_else(far))?;
            let id = ElementId::new(format!("vc:{label}:{}", element.id))
                .map_err(|e| Diagnostic::new(Code::InternalCheckFailed, format!("The VectorCraft object `vc:{label}` makes no element id: {e}.")))?;
            let thread = thread.clone().unwrap_or(moved.thread.clone());
            self.elements.push(Element { id, name: node.name.clone(), thread, ..moved });
        }
        let count = (self.elements.len() - made) as u64;
        self.charge(count.saturating_mul(around.clips.cost))?;
        for element in self.elements.iter_mut().skip(made) {
            element.clips.extend(around.clips.list.iter().cloned());
        }
        Ok(())
    }

    /// The font a Lettering effect asks for by `asked`, and how it sets its text, from its record's `params`:
    /// each setting lettering has by its name, the others at lettering's defaults. A value that doesn't read
    /// is said, and keeps its default.
    fn lettering(&mut self, label: &str, params: &Value) -> Result<(String, Lettering), Diagnostic> {
        let (mut asked, mut how) = (String::new(), Lettering::default());
        let Value::Object(map) = params else { return Ok((asked, how)) };
        for (key, value) in map.iter().filter(|(key, _)| sews(key)) {
            self.charge(1)?;
            let read = match (key.as_str(), text(value)) {
                (FONT, Some(name)) => {
                    asked = name;
                    true
                }
                (_, Some(value)) => how.take(key, &value),
                (_, None) => false,
            };
            if !read {
                let message = format!("The parameter {} of its StitchCraft Lettering effect does not read, and keeps its default.", quoted(key));
                self.feature(label, message, Some("Set the parameter again in the effect's dialog."));
            }
        }
        Ok((asked, how))
    }

    /// The font `asked` names, with what reading it said, said once for each font. Without it, `SC-W1111`
    /// says that `who` is not stitched, and names the fonts there are.
    fn font(&mut self, label: &str, who: &str, asked: &str) -> Option<Rc<Font>> {
        let named = if asked.trim().is_empty() { crate::lettering::DEFAULT_FONT } else { asked.trim() };
        match self.fonts.font(asked, self.budget) {
            Some(Ok(font)) => {
                if self.lettered.insert(font.folder.clone()) {
                    self.warnings.extend(font.warnings.iter().cloned());
                }
                Some(font)
            }
            Some(Err(unread)) => {
                let message = format!("{who} is not stitched: its font {} does not read. {}", quoted(named), unread.message);
                self.unlettered(label, message, "Choose another font in the Lettering effect's dialog.");
                None
            }
            None => {
                let names = self.fonts.names();
                let there =
                    if names.is_empty() { "No embroidery font is here.".to_string() } else { format!("The fonts here are {}.", names.join(", ")) };
                let message = format!("{who} is not stitched: its Lettering effect names the font {}, which is not here. {there}", quoted(named));
                let hint = "Choose a font that is here in the effect's dialog. For `stitch`, set `STITCHCRAFT_FONTS` to the folder that holds the font's folder.";
                self.unlettered(label, message, hint);
                None
            }
        }
    }

    /// Where a text whose transform is `xf` sews: at its origin, turned as its baseline is. A transform that
    /// is missing or holds no number leaves the text at the document's origin, upright, and one that mirrors
    /// or slants `who` is said.
    fn place(&mut self, label: &str, who: &str, xf: Option<&[Option<f64>; 6]>) -> Place {
        let number = |index: usize| xf.and_then(|xf| xf.get(index).copied().flatten()).filter(|n| n.is_finite());
        let [a, b, c, d] = [number(0).unwrap_or(1.0), number(1).unwrap_or(0.0), number(2).unwrap_or(0.0), number(3).unwrap_or(1.0)];
        let mm = |points: Option<f64>| points.and_then(|points| Mm::from_points(points).ok()).map_or(0.0, Mm::get);
        let (along, across) = (math::hypot(a, b), math::hypot(c, d));
        let (cos, sin) = if along > 0.0 && along.is_finite() { (a / along, b / along) } else { (1.0, 0.0) };
        // Upright letters keep their second axis a quarter turn after the first.
        let upright = a * d - b * c > 0.0 && (a * c + b * d).abs() <= 1e-9 * along * across;
        if !upright {
            let message = format!("{who} is mirrored or slanted, which lettering doesn't follow: it is sewn turned as its baseline is.");
            self.feature(label, message, None);
        }
        Place { cos, sin, dx: mm(number(4)), dy: mm(number(5)) }
    }

    /// `SC-W1111` about the object `label`.
    fn unlettered(&mut self, label: &str, message: String, hint: &str) {
        self.warn(Code::TextNotLettered, Some(label), message, Some(hint));
    }
}

/// Where a text sews: turned by the angle whose cosine and sine it holds, then moved, in millimetres.
struct Place {
    cos: f64,
    sin: f64,
    dx: f64,
    dy: f64,
}

impl Place {
    /// Where the point `p` of the text as lettering set it lies in the document.
    fn of(&self, p: Point) -> Option<Point> {
        Point::new(self.cos * p.x() - self.sin * p.y() + self.dx, self.sin * p.x() + self.cos * p.y() + self.dy).ok()
    }
}

/// Whether every element of `set` sews in one thread, as a font of one colour's do: the text's own colour
/// is then the thread's.
fn one_thread(set: &Set) -> bool {
    let mut colours = set.design.elements().iter().map(|element| element.thread.color);
    colours.next().is_none_or(|first| colours.all(|colour| colour == first))
}
