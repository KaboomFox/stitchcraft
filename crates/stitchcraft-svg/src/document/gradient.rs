//! A fill's linear gradient, placed as a viewer places it, for a linear gradient fill to sew
//! (`docs/src/design/svg-input.md` › Gradients, M10.12).
//!
//! **Why as a viewer places it.** The engine sews the gradient the drawing shows: its start and end in the
//! design's millimetres, and its stops. So the reader applies what SVG says of a gradient before it hands it
//! on. A gradient takes what it doesn't say itself from the gradient its `href` names: its vector, its units
//! and its `gradientTransform` from the nearest one that says them, and its stops from the nearest one that
//! has any. Its vector is in user units with `gradientUnits="userSpaceOnUse"`, as Inkscape writes it, and
//! otherwise in shares of the shape's bounding box, SVG's default. Ink/Stitch reads neither the vector
//! another gradient gives nor the bounding box's shares (`DEV-SVG-027`).
//!
//! **The stops.** Each stop's offset is a number or a percentage, kept from 0 to 1 and never before the stop
//! before, as SVG keeps it. Its colour is its `stop-color`, black where it says nothing usable, and its
//! opacity its `stop-opacity` times its colour's alpha. A stop of opacity 0 is transparent.
//!
//! **What costs.** Each gradient followed and each stop read is a unit of the budget, and each stop's style
//! costs as an element's does.

use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_core::{Diagnostic, Point};
use stitchcraft_engine::design::{Gradient, GradientStop, within_working_limit};
use stitchcraft_plan::Rgb;
use svgtypes::{Color, Length, LengthUnit};

use super::{Axis, MAX_HOPS, Reader};
use crate::inkstitch;
use crate::path::{self, Sub};
use crate::style::{rgb, without_icc};
use crate::transform::{self, Affine, user_units};

/// The vector a linear gradient says nothing of: from its box's left side to its right, along its top.
const VECTOR: [(&str, &str, Axis); 4] = [("x1", "0%", Axis::X), ("y1", "0%", Axis::Y), ("x2", "100%", Axis::X), ("y2", "0%", Axis::Y)];

impl<'a, 'input> Reader<'_, 'a, 'input> {
    /// The linear gradient `server` paints the shape `label` with, whose outline is `subs` in its user units,
    /// which `map` takes to millimetres: its start and end placed as the module's docs say, and its stops.
    /// `None` when it places none: it has fewer than 2 stops, or its units are the shape's box and the box
    /// has no width or no height, which a viewer paints nothing with. A part of it that can't be used is named
    /// with `SC-W0804` and left as a viewer leaves it.
    pub(super) fn gradient(&mut self, server: Node<'a, 'input>, subs: &[Sub], map: Affine, label: &str) -> Result<Option<Gradient>, Diagnostic> {
        let chain = self.gradients(server)?;
        let Some(stops) = self.gradient_stops(&chain)? else { return Ok(None) };
        let said = |name: &str, linear_only: bool| {
            chain.iter().filter(|g| !linear_only || g.tag_name().name() == "linearGradient").find_map(|g| g.attribute(name))
        };
        let in_box = said("gradientUnits", false).is_none_or(|units| units.trim() != "userSpaceOnUse");
        let id = server.attribute("id").unwrap_or_default();
        let mut vector = [0.0; 4];
        for (value, (name, default, axis)) in vector.iter_mut().zip(VECTOR) {
            let text = said(name, true).unwrap_or(default);
            let length = match Length::from_str(text.trim()) {
                Ok(length) => length,
                Err(_) => {
                    let message = format!(
                        "The {name} of the gradient `{id}` that `{label}` is filled with, \"{text}\", is not a length; {default} is used, as a \
                         viewer uses it."
                    );
                    self.unusable(label, message);
                    Length::from_str(default).unwrap_or(Length::new_number(0.0))
                }
            };
            *value = if in_box && matches!(length.unit, LengthUnit::None | LengthUnit::Percent) {
                if length.unit == LengthUnit::Percent { length.number / 100.0 } else { length.number }
            } else {
                user_units(length, self.reference(axis))
            };
        }
        let own = match said("gradientTransform", false) {
            None => Affine::IDENTITY,
            Some(text) => {
                self.charge_text(text)?;
                transform::parse(text).unwrap_or_else(|reason| {
                    let message = format!(
                        "The transform of the gradient `{id}` that `{label}` is filled with cannot be used ({reason}); the gradient is placed \
                         without it."
                    );
                    self.unusable(label, message);
                    Affine::IDENTITY
                })
            }
        };
        let frame = if in_box {
            match path::bounds(subs) {
                Some((x, y, width, height)) if width > 0.0 && height > 0.0 => Affine::translate(x, y).after(Affine::scale(width, height)),
                _ => return Ok(None),
            }
        } else {
            Affine::IDENTITY
        };
        let placed = map.after(frame).after(own);
        let [x1, y1, x2, y2] = vector;
        let point = |(x, y): (f64, f64)| Point::new(x, y).ok().filter(|p| within_working_limit(*p));
        let (Some(start), Some(end)) = (point(placed.apply(x1, y1)), point(placed.apply(x2, y2))) else {
            let message = format!(
                "The gradient `{id}` that `{label}` is filled with reaches more than 10 m from the origin, or its place is no number, so the \
                 fill is sewn without it."
            );
            self.unusable(label, message);
            return Ok(None);
        };
        Ok(Some(Gradient { start, end, stops }))
    }

    /// `server` and the gradients it names in turn by their `href`s, as far as they name gradients, each once.
    fn gradients(&mut self, server: Node<'a, 'input>) -> Result<Vec<Node<'a, 'input>>, Diagnostic> {
        let mut chain = vec![server];
        let mut current = server;
        for _ in 0..MAX_HOPS {
            self.charge(1)?;
            let named = inkstitch::href(current).and_then(|h| h.trim().strip_prefix('#')).and_then(|id| self.by_id(id));
            let Some(next) = named.filter(|next| matches!(next.tag_name().name(), "linearGradient" | "radialGradient") && !chain.contains(next))
            else {
                break;
            };
            chain.push(next);
            current = next;
        }
        Ok(chain)
    }

    /// The stops of the first gradient of `chain` that has any, as the module's docs read them; `None` for
    /// fewer than 2, which paint no gradient.
    fn gradient_stops(&mut self, chain: &[Node<'a, 'input>]) -> Result<Option<Vec<GradientStop>>, Diagnostic> {
        for gradient in chain {
            let mut stops: Vec<GradientStop> = Vec::new();
            for child in gradient.children() {
                self.charge(1)?;
                if !(self.is_svg(child) && child.tag_name().name() == "stop") {
                    continue;
                }
                let before = stops.last().map_or(0.0, |stop| stop.offset);
                let offset = child.attribute("offset").and_then(share).unwrap_or(0.0).max(before);
                let (colour, alpha) = self.stop_paint(child)?;
                let opacity = self.declared(child)?.get("stop-opacity").and_then(share).unwrap_or(1.0);
                stops.push(GradientStop { offset, colour, opacity: opacity * alpha });
            }
            if !stops.is_empty() {
                return Ok((stops.len() >= 2).then_some(stops));
            }
        }
        Ok(None)
    }

    /// A stop's `stop-color`, its `color` for `currentColor`, and the colour's alpha from 0 to 1: black where
    /// it says nothing usable, as in a viewer.
    fn stop_paint(&mut self, stop: Node<'a, 'input>) -> Result<(Rgb, f64), Diagnostic> {
        let black = Color::black();
        let declared = self.declared(stop)?;
        let value = declared.get("stop-color").map(without_icc).unwrap_or("black");
        let colour = if value.trim().eq_ignore_ascii_case("currentColor") {
            let mut found = None;
            // `color` inherits: the nearest element from the stop up that sets it.
            for node in stop.ancestors() {
                self.charge(1)?;
                found = self.declared(node)?.get("color").and_then(|v| Color::from_str(without_icc(v)).ok());
                if found.is_some() {
                    break;
                }
            }
            found.unwrap_or(black)
        } else {
            Color::from_str(value).unwrap_or(black)
        };
        Ok((rgb(colour), f64::from(colour.alpha) / 255.0))
    }
}

/// A stop's offset or opacity: a number or a percentage, kept from 0 to 1; `None` when it doesn't read.
fn share(text: &str) -> Option<f64> {
    let length = Length::from_str(text.trim()).ok()?;
    let value = match length.unit {
        LengthUnit::None => length.number,
        LengthUnit::Percent => length.number / 100.0,
        _ => return None,
    };
    value.is_finite().then(|| value.clamp(0.0, 1.0))
}
