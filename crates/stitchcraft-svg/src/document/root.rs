//! The root element's size, viewBox and aspect ratio, which turn the drawing's user units into
//! millimetres (SVG 1.1 § 7.8), as [`transform::root_to_mm`] maps them.
//!
//! A size or viewBox that does not read could put the design at any size, so the file is refused
//! (`SC-E0801`). So is a size of 0 or less, and a viewBox with no area, in which a viewer draws nothing.
//! What a viewer ignores is ignored and named (`SC-W0804`): an empty `width`, `height` or `viewBox`, and an
//! aspect ratio that does not read, which only places the drawing in its viewport.
//!
//! The font reader measures a glyph's box and its file's baseline on the same page ([`page`]), so that they
//! lie where the glyph's design does ([`crate::font`]).

use std::str::FromStr;

use roxmltree::Node;
use stitchcraft_core::{Code, Diagnostic};
use svgtypes::{AspectRatio, Length, LengthUnit, ViewBox, ViewBoxError};

use super::Reader;
use crate::text::unreadable;
use crate::transform::{self, Affine, user_units};

/// The page a file draws on: the map from its root's user units to millimetres, and its root viewport in user
/// units, which percentages are of.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Page {
    /// From user units to millimetres.
    pub map: Affine,
    /// The viewport's width and height, in user units: the viewBox's, or the size in pixels without one.
    pub viewport: (f64, f64),
}

impl<'a, 'input> Reader<'_, 'a, 'input> {
    /// The map from the root's user units to millimetres, from its size, viewBox and aspect ratio; the
    /// root's viewport in user units is kept for percentages.
    pub(super) fn root_map(&mut self, root: Node<'a, 'input>) -> Result<Affine, Diagnostic> {
        let mut notes = Vec::new();
        let page = page(root, &mut notes);
        for message in notes {
            self.warn(Code::SvgGeometryUnusable, None, message, None);
        }
        let page = page?;
        self.viewport = page.viewport;
        Ok(page.map)
    }
}

/// The page of the file whose root element is `root`, from its size, viewBox and aspect ratio. What a viewer
/// ignores goes to `notes`, for `SC-W0804`.
pub(crate) fn page(root: Node<'_, '_>, notes: &mut Vec<String>) -> Result<Page, Diagnostic> {
    let (width, height) = (root_length(root, "width", notes)?, root_length(root, "height", notes)?);
    let view_box = match root_attribute(root, "viewBox", notes) {
        None => None,
        Some(v) => Some(ViewBox::from_str(v).map_err(|e| match e {
            ViewBoxError::InvalidSize => size_unusable(transform::NO_AREA),
            ViewBoxError::InvalidNumber => unreadable(format!("The root element's viewBox, \"{v}\", is not four numbers.")),
        })?),
    };
    let aspect = match root_attribute(root, "preserveAspectRatio", notes) {
        None => AspectRatio::default(),
        Some(v) => AspectRatio::from_str(v.trim()).unwrap_or_else(|_| {
            notes.push(format!(
                "The root element's preserveAspectRatio, \"{v}\", is not an aspect ratio SVG names; it is ignored, as a viewer ignores it."
            ));
            AspectRatio::default()
        }),
    };
    // Without a viewBox or an absolute size, percentages are of a viewer's default size, 300 × 150.
    let pixels = |length: Option<Length>, default: f64| match length {
        Some(l) if l.unit != LengthUnit::Percent => user_units(l, 0.0),
        _ => default,
    };
    let viewport = view_box.map_or_else(|| (pixels(width, 300.0), pixels(height, 150.0)), |vb| (vb.w, vb.h));
    let map = transform::root_to_mm(width, height, view_box, aspect).map_err(|reason| size_unusable(&reason))?;
    Ok(Page { map, viewport })
}

/// The root's `width` or `height`: `None` when it is missing, empty or `auto`.
fn root_length(root: Node<'_, '_>, name: &str, notes: &mut Vec<String>) -> Result<Option<Length>, Diagnostic> {
    match root_attribute(root, name, notes).map(str::trim) {
        None | Some("auto") => Ok(None),
        Some(v) => Length::from_str(v).map(Some).map_err(|_| unreadable(format!("The root element's {name}, \"{v}\", is not a length."))),
    }
}

/// The root's attribute `name`, unless it is missing or empty. An empty one says nothing, and a viewer
/// draws the file as if it were not there, so it is named (`SC-W0804`) and ignored.
fn root_attribute<'a>(root: Node<'a, '_>, name: &str, notes: &mut Vec<String>) -> Option<&'a str> {
    let value = root.attribute(name)?;
    if !value.trim().is_empty() {
        return Some(value);
    }
    notes.push(format!("The root element's {name} is empty; it is ignored, as a viewer ignores it."));
    None
}

/// `SC-E0801` for a drawing whose size cannot be used, and `reason` why.
fn size_unusable(reason: &str) -> Diagnostic {
    unreadable(format!("The drawing's size cannot be used: {reason}."))
}
