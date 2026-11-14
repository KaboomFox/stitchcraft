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
//! or a corner are one gap. **The base layer lies under** a place inside the shape that one of its pieces
//! fills (`REQ-AUTO-028`). **The rim** is the band of the design that the base layer's offset leaves along
//! the design's outline, 0.6 mm wide by default: the base layer can't lie there. **A sliver** is a gap no
//! larger than the setting `auto_base_sliver_mm2` whose every place has the base layer under it or lies in
//! the rim. Over the base layer its thread shows in place of the fabric, and in the rim a sliver reads as
//! the outline. Fabric shows in every other gap, and a design passes with none ([`Cover::faults`]). **Open** is what of the design within its edge no thread at all covers: the underlays, the base
//! layer's among them (`REQ-AUTO-028`), and the travel stitches cover too. Fabric shows through an open
//! place, and a bare place that isn't open shows a thread of another colour.

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Rect};
use stitchcraft_engine::auto::{self, AutoParams};
use stitchcraft_engine::design::Design;
use stitchcraft_plan::{Rgb, Role, StitchPlan};

use super::{PIXEL, Painted, THREAD, fitting, groups, held, opened_with};

/// The id of the base layer's first piece. The others add `#2` and so on.
const BASE: &str = "stitchcraft:base";

/// How wide the disc is that finds a gap, in millimetres: 7 pixels across. A strip of bare fabric a thread
/// wide, 0.4 mm, holds 8 pixels' centres or 7 by where it lies on the grid, so this disc fits in every such
/// strip, and the disc a thread across that [`super::unsewn`] opens with, 9 pixels, fits in none.
pub const GAP: f64 = 0.35;

/// The largest share of a design without a base layer that its top stitches may leave bare (`REQ-AUTO-027`):
/// 0.15 %. A design with one goes by its setting `auto_base_bare_percent`. Measured on
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
    /// Each gap the top stitches leave: the box round its pixels' centres, its area in square millimetres, and
    /// the shape most of it lies in, by id (`between shapes` for a slit), the largest gap first.
    pub places: Vec<(Rect, f64, String)>,
    /// For each gap of `places`, in their order, how far inside the design's outline its farthest place lies
    /// that the base layer isn't under, in millimetres: 0 when the base layer lies under all of the gap, and
    /// infinite when such a place lies further than [`FAR`] from the outline.
    pub beyond: Vec<f64>,
    /// What of the design within its edge the base layer lies under, in square millimetres: 0 for a design
    /// that sews none.
    pub base: f64,
    /// The 10 shapes with the most bare fabric, by id, each with its bare area in square millimetres, the
    /// most first: `between shapes` names the slits.
    pub shapes: Vec<(String, f64)>,
}

impl Cover {
    /// The share of the design that is bare.
    pub fn bare_share(&self) -> f64 {
        if self.design > 0.0 { self.bare / self.design } else { 0.0 }
    }

    /// How wide the rim is by `rules`, in millimetres: as far as the base layer's edge lies inside the
    /// design's outline, and a pixel, since the places are measured between pixels' centres.
    fn rim(rules: &AutoParams) -> f64 {
        (-rules.auto_base_offset_mm.get()).max(0.0) + PIXEL
    }

    /// Whether the gap of `area` whose places without the base layer lie up to `beyond` inside the outline is
    /// in reach of the base layer by `rules`: the design has one, and those places lie in the rim.
    fn reached(&self, beyond: f64, rules: &AutoParams) -> bool {
        self.base > 0.0 && beyond <= Cover::rim(rules)
    }

    /// The slivers' areas, in square millimetres, the largest first: each gap no larger than
    /// `rules.auto_base_sliver_mm2` whose every place has the base layer under it or lies in the rim.
    pub fn slivers(&self, rules: &AutoParams) -> Vec<f64> {
        let sliver = |(place, beyond): (&(Rect, f64, String), &f64)| {
            (self.reached(*beyond, rules) && place.1 <= rules.auto_base_sliver_mm2 + SLACK).then_some(place.1)
        };
        self.places.iter().zip(&self.beyond).filter_map(sliver).collect()
    }

    /// How many of the [`Cover::slivers`] have a place in the rim, where no base layer lies under them.
    pub fn at_the_rim(&self, rules: &AutoParams) -> usize {
        let rim = |(place, beyond): (&(Rect, f64, String), &f64)| {
            *beyond > 0.0 && self.reached(*beyond, rules) && place.1 <= rules.auto_base_sliver_mm2 + SLACK
        };
        self.places.iter().zip(&self.beyond).filter(|(place, beyond)| rim((*place, *beyond))).count()
    }

    /// The most of the design that the top stitches may leave bare, as a share: `rules.auto_base_bare_percent`
    /// with a base layer, and [`BARE`] without.
    pub fn limit(&self, rules: &AutoParams) -> f64 {
        if self.base > 0.0 { rules.auto_base_bare_percent / 100.0 } else { BARE }
    }

    /// Why the plan doesn't cover its design by `rules`, the settings of the design's lowest automatic shape
    /// (`REQ-AUTO-027`): each gap that fabric shows in, which is one larger than a sliver or with a place
    /// inside the rim that the base layer isn't under, and the bare share when it is over its
    /// [`Cover::limit`]. Empty when the plan covers.
    pub fn faults(&self, rules: &AutoParams) -> Vec<String> {
        let mut faults = Vec::new();
        for ((place, area, shape), beyond) in self.places.iter().zip(&self.beyond) {
            let why = match (self.reached(*beyond, rules), *area <= rules.auto_base_sliver_mm2 + SLACK) {
                (true, true) => continue,
                (true, false) => format!("larger than a sliver of {} mm²", rules.auto_base_sliver_mm2),
                (false, _) => "no base layer under all of it".to_string(),
            };
            let middle = place.center();
            faults.push(format!("a gap of {area:.3} mm² at ({:.1}, {:.1}) in {shape}: {why}", middle.x(), middle.y()));
        }
        let limit = self.limit(rules);
        if self.bare_share() > limit {
            faults.push(format!(
                "{:.3} % of the design bare, {:.2} of {:.0} mm², over {:.3} %",
                100.0 * self.bare_share(),
                self.bare,
                self.design,
                100.0 * limit
            ));
        }
        faults
    }
}

/// What an area of whole pixels may be over a limit and still be within it: the sums of pixels aren't exact.
const SLACK: f64 = 1e-9;

/// How far from the design's outline a place is looked for, in millimetres: a place further inside counts as
/// infinitely far. It is the widest rim a base layer's offset can leave.
pub const FAR: f64 = 3.0;

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
    // The places the base layer lies under: inside the shape one of its pieces fills.
    let mut under = vec![false; inside.len()];
    let (elements, _) = auto::digitized(design, &Budget::DEFAULT);
    for piece in elements.iter().filter(|element| element.id.as_str().strip_prefix(BASE).is_some_and(|rest| rest.is_empty() || rest.starts_with('#')))
    {
        for at in held(&picture.grid, piece) {
            under[at] = true;
        }
    }
    let mut by_shape: BTreeMap<Option<usize>, usize> = BTreeMap::new();
    for at in (0..bare.len()).filter(|&at| bare[at]) {
        *by_shape.entry(picture.owner[at]).or_default() += 1;
    }
    let name = |place: Option<usize>| {
        place.and_then(|place| design.elements().get(place)).map_or_else(|| "between shapes".to_string(), |element| element.id.to_string())
    };
    let mut shapes: Vec<(String, f64)> = by_shape.into_iter().map(|(place, pixels)| (name(place), area(pixels))).collect();
    // How far pixel `at` lies inside the design's outline: to the nearest pixel outside the design.
    let reach = super::index((FAR / PIXEL).ceil()) as isize;
    let far = |at: usize| {
        let (column, row) = ((at % columns) as isize, (at / columns) as isize);
        let mut nearest = f64::INFINITY;
        for (dx, dy) in (-reach..=reach).flat_map(|dy| (-reach..=reach).map(move |dx| (dx, dy))) {
            let (x, y) = (column + dx, row + dy);
            let outside = x < 0 || y < 0 || x >= columns as isize || y >= rows as isize || !inside[y as usize * columns + x as usize];
            let distance = ((dx * dx + dy * dy) as f64).sqrt() * PIXEL;
            if outside && distance <= FAR {
                nearest = nearest.min(distance);
            }
        }
        nearest
    };
    let mut places: Vec<((Rect, f64, String), f64)> = gaps
        .iter()
        .filter_map(|group| {
            let mut owners: BTreeMap<Option<usize>, usize> = BTreeMap::new();
            for &at in group {
                *owners.entry(picture.owner[at]).or_default() += 1;
            }
            let most = owners.into_iter().max_by_key(|(_, pixels)| *pixels).map(|(place, _)| name(place))?;
            let place = (Rect::around(group.iter().map(|&at| picture.centre(at)))?, area(group.len()), most);
            Some((place, group.iter().filter(|&&at| !under[at]).map(|&at| far(at)).fold(0.0, f64::max)))
        })
        .collect();
    places.sort_by(|a, b| b.0.1.total_cmp(&a.0.1));
    let (places, beyond): (Vec<(Rect, f64, String)>, Vec<f64>) = places.into_iter().unzip();
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
        places,
        beyond,
        base: area(within.iter().zip(&under).filter(|(within, under)| **within && **under).count()),
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
