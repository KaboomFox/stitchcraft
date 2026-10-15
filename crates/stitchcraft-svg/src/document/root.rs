//! The root element's size, viewBox and aspect ratio, which turn the drawing's user units into
//! millimetres (SVG 1.1 § 7.8), as [`transform::root_to_mm`] maps them.
//!
//! A size or viewBox that does not read could put the design at any size, so the file is refused
//! (`SC-E0801`). So is a size of 0 or less, and a viewBox with no area, in which a viewer draws nothing.
//! What a viewer ignores is ignored and named (`SC-W0804`): an empty `width`, `height` or `viewBox`, and an
//! aspect ratio that does not read, which only places the drawing in its viewport.

use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_core::{Code, Diagnostic};
use svgtypes::{AspectRatio, Length, LengthUnit, ViewBox, ViewBoxError};

use super::Reader;
use crate::text::unreadable;
use crate::transform::{self, Affine, user_units};

impl<'a, 'input> Reader<'_, 'a, 'input> {
    /// The map from the root's user units to millimetres, from its size, viewBox and aspect ratio; the
    /// root's viewport in user units is kept for percentages.
    pub(super) fn root_map(&mut self, root: Node<'a, 'input>) -> Result<Affine, Diagnostic> {
        let (width, height) = (self.root_length(root, "width")?, self.root_length(root, "height")?);
        let view_box = match self.root_attribute(root, "viewBox") {
            None => None,
            Some(v) => Some(ViewBox::from_str(v).map_err(|e| match e {
                ViewBoxError::InvalidSize => size_unusable(transform::NO_AREA),
                ViewBoxError::InvalidNumber => unreadable(format!("The root element's viewBox, \"{v}\", is not four numbers.")),
            })?),
        };
        let aspect = match self.root_attribute(root, "preserveAspectRatio") {
            None => AspectRatio::default(),
            Some(v) => AspectRatio::from_str(v.trim()).unwrap_or_else(|_| {
                let message = format!(
                    "The root element's preserveAspectRatio, \"{v}\", is not an aspect ratio SVG names; it is ignored, as a viewer ignores it."
                );
                self.warn(Code::SvgGeometryUnusable, None, message, None);
                AspectRatio::default()
            }),
        };
        // Without a viewBox or an absolute size, percentages are of a viewer's default size, 300 × 150.
        let pixels = |length: Option<Length>, default: f64| match length {
            Some(l) if l.unit != LengthUnit::Percent => user_units(l, 0.0),
            _ => default,
        };
        self.viewport = view_box.map_or_else(|| (pixels(width, 300.0), pixels(height, 150.0)), |vb| (vb.w, vb.h));
        transform::root_to_mm(width, height, view_box, aspect).map_err(|reason| size_unusable(&reason))
    }

    /// The root's `width` or `height`: `None` when it is missing, empty or `auto`.
    fn root_length(&mut self, root: Node<'a, 'input>, name: &str) -> Result<Option<Length>, Diagnostic> {
        match self.root_attribute(root, name).map(str::trim) {
            None | Some("auto") => Ok(None),
            Some(v) => Length::from_str(v).map(Some).map_err(|_| unreadable(format!("The root element's {name}, \"{v}\", is not a length."))),
        }
    }

    /// The root's attribute `name`, unless it is missing or empty. An empty one says nothing, and a viewer
    /// draws the file as if it were not there, so it is named (`SC-W0804`) and ignored.
    fn root_attribute(&mut self, root: Node<'a, 'input>, name: &str) -> Option<&'a str> {
        let value = root.attribute(name)?;
        if !value.trim().is_empty() {
            return Some(value);
        }
        self.warn(Code::SvgGeometryUnusable, None, format!("The root element's {name} is empty; it is ignored, as a viewer ignores it."), None);
        None
    }
}

/// `SC-E0801` for a drawing whose size cannot be used, and `reason` why.
fn size_unusable(reason: &str) -> Diagnostic {
    unreadable(format!("The drawing's size cannot be used: {reason}."))
}
