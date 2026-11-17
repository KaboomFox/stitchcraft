//! What shows of each area's own shape once a plan is sewn: whether its holes stay open, and whether its own
//! thread leaves gaps inside its outline (`docs/src/design/image-to-embroidery.md` › Measured). A letter
//! traced as an area, such as the A of a patch, has a hole, its counter, through which the field under it
//! shows, and a reader needs to see it to read the letter. Its own columns have to meet with no gap between
//! them where another colour would show.
//!
//! **What shows** at a pixel is the thread of the last top stitch that covers it, a pixel being covered as
//! [`super::unsewn`] covers it, and the fabric where none does.
//!
//! **A hole** of an area is a hole of one of its region's parts, its pixels those inside the hole's ring.
//! What should show there is the picture's colour, the shape under the area or over it, and the hole is
//! **open** where that colour's thread shows, or the fabric where the picture has none.
//!
//! **A gap** of an area is a place inside its outline where its own thread doesn't show, and a disc
//! [`super::GAP`] across fits on such pixels: a strip of another colour as wide as a thread, as
//! [`super::cover`] finds a gap. Places that touch at a side or a corner are one gap. Pixels the area owns
//! in the picture count, so that a shape drawn over the area isn't a gap of it.

use stitchcraft_core::Rect;
use stitchcraft_engine::design::{Design, Shape};
use stitchcraft_engine::normalize::region;
use stitchcraft_plan::{Rgb, Role, StitchPlan};

use super::{GAP, PIXEL, Painted, groups, opened_with};
use crate::metrics::spans;

/// What shows of one area's own shape ([`shown`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Shown {
    /// The area's id.
    pub id: String,
    /// Each hole of the area: its drawn area and what of it is open, in square millimetres.
    pub holes: Vec<(f64, f64)>,
    /// Each gap inside the area's outline: the box round its pixels' centres and its area in square
    /// millimetres, the largest first.
    pub gaps: Vec<(Rect, f64)>,
    /// What of the area's own pixels shows another thread or the fabric, in square millimetres.
    pub other: f64,
    /// The area's own pixels, in square millimetres.
    pub area: f64,
}

impl Shown {
    /// The share of the area's holes together that is open: 1 for an area without holes.
    pub fn open_share(&self) -> f64 {
        let (drawn, open) = self.holes.iter().fold((0.0, 0.0), |(drawn, open), hole| (drawn + hole.0, open + hole.1));
        if drawn > 0.0 { open / drawn } else { 1.0 }
    }

    /// The largest gap's area in square millimetres: 0 when there is none.
    pub fn largest_gap(&self) -> f64 {
        self.gaps.first().map_or(0.0, |gap| gap.1)
    }
}

/// An area of `pixels` pixels, in square millimetres.
fn area(pixels: usize) -> f64 {
    pixels as f64 * PIXEL * PIXEL
}

/// What shows of the shape of each area of `design` that `plan`, whose stitches lie where the design draws
/// its shapes, sews (see the module's docs), in the design's order.
///
/// # Panics
///
/// When an area's region can't be built, which a design that planned has built already.
pub fn shown(design: &Design, plan: &StitchPlan) -> Vec<Shown> {
    let picture = Painted::of(design, plan);
    let (columns, rows) = (picture.grid.columns, picture.grid.rows);
    // The thread that shows at each pixel: the last top stitch's that covers it.
    let mut shows: Vec<Option<Rgb>> = vec![None; columns * rows];
    for stitch in plan.sewn_stitches().iter().filter(|stitch| stitch.role == Role::Top) {
        let colour = plan.blocks.get(stitch.block).map(|block| block.thread.color);
        picture.each([stitch.from, stitch.to], |at| shows[at] = colour);
    }
    let mut all = Vec::new();
    for (place, element) in design.elements().iter().enumerate() {
        let Shape::Fill { path, rule, .. } = &element.shape else { continue };
        let colour = element.thread.color;
        let built = region::build_clipped(path, *rule, &element.clips, &mut stitchcraft_core::Budget::DEFAULT.meter()).unwrap();
        let mut holes = Vec::new();
        for hole in built.region.parts.iter().flat_map(|part| &part.holes) {
            let (mut drawn, mut open) = (0, 0);
            for row in 0..rows {
                for (from, to) in spans(&[hole.as_slice()], picture.grid.y(row)) {
                    for column in picture.grid.columns_within(from, to).filter(|&column| picture.grid.x(column) < to) {
                        let at = row * columns + column;
                        drawn += 1;
                        open += usize::from(shows[at] == picture.colour(at));
                    }
                }
            }
            holes.push((area(drawn), area(open)));
        }
        let own: Vec<bool> = picture.owner.iter().map(|owner| *owner == Some(place)).collect();
        let other: Vec<bool> = own.iter().zip(&shows).map(|(own, shows)| *own && *shows != Some(colour)).collect();
        let mut gaps: Vec<(Rect, f64)> = groups(&opened_with(&other, columns, rows, GAP, false), columns)
            .into_iter()
            .filter_map(|group| Some((Rect::around(group.iter().map(|&at| picture.centre(at)))?, area(group.len()))))
            .collect();
        gaps.sort_by(|a, b| b.1.total_cmp(&a.1));
        all.push(Shown {
            id: element.id.to_string(),
            holes,
            gaps,
            other: area(other.iter().filter(|other| **other).count()),
            area: area(own.iter().filter(|own| **own).count()),
        });
    }
    all
}
