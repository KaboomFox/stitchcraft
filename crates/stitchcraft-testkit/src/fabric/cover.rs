//! Whether a plan covers its design on fabric of any colour (`docs/src/design/image-to-embroidery.md` ›
//! Covering the fabric). The fabric's colour can't be chosen and may be black, so a design has to
//! cover by itself: [`super::unsewn`] passes a patch smaller than the smallest area and a slit between 2
//! shapes that neither paints, and both show on dark fabric.
//!
//! **The design** is what the picture paints ([`super`]), with the slits between its shapes that are
//! narrower than a thread: the painted pixels closed with a disc a [`THREAD`] across, so that fabric
//! belongs to the design where no such disc fits on fabric alone. A hole a thread wide or wider, as the one
//! between a dog's legs, stays fabric.
//!
//! **Its edge** is the outermost half thread of the design: the pixels on which a disc a thread across
//! doesn't fit inside it. A row of stitches starts up to a row's spacing inside its shape's outline, and the
//! fabric beyond the outline is fabric anyway, so nothing is measured in the edge: on the 3 stripes, which
//! have no gap, 7.6 mm² of the edge's pixels were bare.
//!
//! **Bare** is what of the design within its edge no top stitch covers, a pixel being covered as
//! [`super::unsewn`] covers it. **A gap** is a place where a disc [`GAP`] across fits on bare pixels, and places that touch at a side
//! or a corner are one gap. **Open** is what of the design within its edge no thread at all covers: the underlays, the base
//! layer's among them (`REQ-AUTO-028`), and the travel stitches cover too. Fabric shows through an open
//! place, and a bare place that isn't open shows a thread of another colour.

use std::collections::BTreeMap;

use stitchcraft_core::Rect;
use stitchcraft_engine::design::Design;
use stitchcraft_plan::{Rgb, Role, StitchPlan};

use super::{PIXEL, Painted, THREAD, fitting, groups, opened_with};

/// How wide the disc is that finds a gap, in millimetres: 7 pixels across. A strip of bare fabric a thread
/// wide, 0.4 mm, holds 8 pixels' centres or 7 by where it lies on the grid, so this disc fits in every such
/// strip, and the disc a thread across that [`super::unsewn`] opens with, 9 pixels, fits in none.
pub const GAP: f64 = 0.35;

/// The largest share of a design that its top stitches may leave bare (`REQ-AUTO-027`): 0.15 %. Measured on
/// 2026-11-10, the traced pear, which has no gap, leaves 0.083 % bare, and the 4 traced dogs 0.28 to 0.77 %.
pub const BARE: f64 = 0.0015;

/// How a plan covers its design ([`cover`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Cover {
    /// The design's area, in square millimetres.
    pub design: f64,
    /// What of the design the picture doesn't paint, its slits, in square millimetres.
    pub slits: f64,
    /// What of the design within its edge no top stitch covers, in square millimetres.
    pub bare: f64,
    /// How many gaps the top stitches leave.
    pub gaps: usize,
    /// The gaps' area together, in square millimetres.
    pub in_gaps: f64,
    /// The largest gap's area, in square millimetres: 0 when there is none.
    pub largest: f64,
    /// The box round the largest gap's pixels' centres; `None` when there is no gap.
    pub at: Option<Rect>,
    /// What of the design within its edge no thread covers, in square millimetres.
    pub open: f64,
    /// How many gaps the threads of every role leave.
    pub open_gaps: usize,
    /// The 10 shapes with the most bare fabric, by id, each with its bare area in square millimetres, the
    /// most first: `between shapes` names the slits.
    pub shapes: Vec<(String, f64)>,
}

impl Cover {
    /// The share of the design that is bare.
    pub fn bare_share(&self) -> f64 {
        if self.design > 0.0 { self.bare / self.design } else { 0.0 }
    }
}

/// An area of `pixels` pixels, in square millimetres.
fn area(pixels: usize) -> f64 {
    pixels as f64 * PIXEL * PIXEL
}

/// How `plan`, whose stitches lie where `design` draws its shapes, covers the design, a gap being `gap`
/// millimetres wide or wider ([`GAP`]; see the module's docs).
pub fn cover(design: &Design, plan: &StitchPlan, gap: f64) -> Cover {
    let picture = Painted::of(design, plan);
    let (columns, rows) = (picture.grid.columns, picture.grid.rows);
    let inside = inside(&picture);
    let within = fitting(&inside, columns, rows, THREAD, false);
    let (mut top, mut any) = (vec![false; inside.len()], vec![false; inside.len()]);
    for stitch in plan.sewn_stitches() {
        let on_top = stitch.role == Role::Top;
        picture.each([stitch.from, stitch.to], |at| {
            any[at] = true;
            top[at] |= on_top;
        });
    }
    let left = |covered: &[bool]| -> Vec<bool> { within.iter().zip(covered).map(|(within, covered)| *within && !covered).collect() };
    let count = |pixels: &[bool]| pixels.iter().filter(|pixel| **pixel).count();
    let (bare, open) = (left(&top), left(&any));
    let gaps = groups(&opened_with(&bare, columns, rows, gap, false), columns);
    let open_gaps = groups(&opened_with(&open, columns, rows, gap, false), columns);
    let largest = gaps.iter().max_by_key(|group| group.len());
    let mut by_shape: BTreeMap<Option<usize>, usize> = BTreeMap::new();
    for at in (0..bare.len()).filter(|&at| bare[at]) {
        *by_shape.entry(picture.owner[at]).or_default() += 1;
    }
    let name = |place: Option<usize>| {
        place.and_then(|place| design.elements().get(place)).map_or_else(|| "between shapes".to_string(), |element| element.id.to_string())
    };
    let mut shapes: Vec<(String, f64)> = by_shape.into_iter().map(|(place, pixels)| (name(place), area(pixels))).collect();
    shapes.sort_by(|a, b| b.1.total_cmp(&a.1));
    shapes.truncate(10);
    Cover {
        design: area(count(&inside)),
        slits: area(inside.iter().zip(&picture.owner).filter(|(inside, owner)| **inside && owner.is_none()).count()),
        bare: area(count(&bare)),
        gaps: gaps.len(),
        in_gaps: area(gaps.iter().map(Vec::len).sum()),
        largest: largest.map_or(0.0, |group| area(group.len())),
        at: largest.and_then(|group| Rect::around(group.iter().map(|&at| picture.centre(at)))),
        open: area(count(&open)),
        open_gaps: open_gaps.len(),
        shapes,
    }
}

/// The pixels of `picture` in the design (see the module's docs).
fn inside(picture: &Painted) -> Vec<bool> {
    let fabric: Vec<bool> = picture.owner.iter().map(Option::is_none).collect();
    // Fabric that a disc a thread across fits on stays fabric, and the rest of it is a slit of the design.
    let open = opened_with(&fabric, picture.grid.columns, picture.grid.rows, THREAD, true);
    let inside: Vec<bool> = open.iter().map(|open| !open).collect();
    inside
}

/// The area the picture of `design` paints in each colour, in square millimetres, the largest first, and
/// of 2 colours as large the one painted first. `plan` gives the grid its box, as in [`cover`].
pub fn painted(design: &Design, plan: &StitchPlan) -> Vec<(Rgb, f64)> {
    let picture = Painted::of(design, plan);
    let mut colours: Vec<(Rgb, usize)> = Vec::new();
    for colour in (0..picture.owner.len()).filter_map(|at| picture.colour(at)) {
        match colours.iter_mut().find(|(known, _)| *known == colour) {
            Some((_, pixels)) => *pixels += 1,
            None => colours.push((colour, 1)),
        }
    }
    colours.sort_by_key(|colour| std::cmp::Reverse(colour.1));
    colours.into_iter().map(|(colour, pixels)| (colour, area(pixels))).collect()
}
