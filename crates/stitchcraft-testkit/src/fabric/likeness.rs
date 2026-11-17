//! How much of a picture's detail its stitches keep (`docs/src/design/image-to-embroidery.md` › Likeness,
//! `REQ-AUTO-093`). Automatic digitizing merges specks and thin lines into their neighbours, and each merge
//! takes a jump thread or a thread change away. It can also take away what makes the picture read: an eye,
//! a nostril, a stripe. Digitizing goes by geometry and can't tell an eye from noise, so this measure checks
//! afterwards what the geometry kept.
//!
//! **The trace and the stitches.** The trace is the design as drawn, painted on the grid of
//! [`super::astray`] and [`super::unsewn`]: each pixel has the colour of the last shape over its centre, and
//! the background digitizing leaves out is bare. The grid goes round the design alone, so that every plan of
//! it is measured on the same pixels and its parts are the same. Unlike [`super::astray`], the trace is the design before
//! its thin lines merge, since what merging loses is what is measured. The stitches are painted on the same
//! grid: each pixel has the colour of the last top stitch whose thread, [`THREAD`] wide, covers its centre,
//! and no colour where none does. Underlay, travel and locks are left out: they lie under the top stitches.
//!
//! **Parts.** A colour's parts are its pieces of the trace: pixels of that colour that touch at a side or a
//! corner are one part. A part counts when it is as large as the smallest area digitizing sews by default,
//! 1 mm² (`auto_min_area_mm2`), and a disc a thread across fits in it. A smaller part is a speck, which the
//! rules merge or leave out as too small to sew, and no stitch of its own can sew a narrower one. A traced
//! photo has hundreds of such bits of texture: counted, they would rule the measure as the large patches
//! rule a share of pixels. A speck setting raised above the default makes larger parts specks, and their
//! loss shows.
//!
//! **Kept.** A pixel of a part is shown when the stitches have the part's colour within half a thread of
//! its centre. A part's core is its pixels that a disc a thread across, centred on them, fits in: the part
//! without a strip half a thread wide along its edge, where its neighbours' rows end and its own may stop
//! short. A part is kept when [`KEPT`] of its core or more is shown. So a strand 1.4 mm wide that a running
//! stitch sews along its middle is kept: its thread shows on 0.8 mm of the 1 mm of its core. A part merged
//! into its neighbour shows none of its colour, and a speck left out shows none.
//!
//! **Why parts and not area.** A share of matching pixels is ruled by the large patches. A dog's eye is a
//! fraction of a percent of the dog, so losing it moves the share by less than a change of the stitches'
//! edges does. A viewer reads a picture by its features, each by itself, so each part counts once,
//! whatever its size. Each colour counts once too, so that the 3 dark parts of a face, 2 eyes and a nose,
//! weigh as much as a colour's hundred pieces of shading. A colour's likeness is the share of its parts kept,
//! and the picture's is the mean of its colours'. A change that keeps every part scores the same, however it
//! moves the stitches. The share of each colour's area shown is kept beside it, for a colour sewn in
//! another thread, which keeps its parts' outlines but not their colour.

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Meter, Rect};
use stitchcraft_engine::design::Design;
use stitchcraft_plan::{Rgb, Role, StitchPlan};

use super::{PIXEL, Painted, THREAD, disc_steps, neighbours, stepped};

/// The share of a part's core that must be shown in its colour for the part to be kept.
pub const KEPT: f64 = 0.5;

/// The lowest likeness the standard pictures may keep, digitized (`REQ-AUTO-094`): the lowest measured on
/// them, rounded down to a twentieth (`docs/src/design/image-to-embroidery.md` › Likeness).
pub const LIKENESS: f64 = 0.75;

/// A part of a colour of the trace (see the module's docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Part {
    /// Its colour.
    pub colour: Rgb,
    /// Its area, in square millimetres.
    pub area: f64,
    /// The share of its core shown in its colour: of its pixels that a disc a thread across, centred on them,
    /// fits in.
    pub shown: f64,
    /// The box round its pixels' centres.
    pub at: Rect,
}

impl Part {
    /// Whether the stitches keep the part: [`KEPT`] of it or more is shown.
    pub fn kept(&self) -> bool {
        self.shown >= KEPT
    }
}

/// What the stitches keep of one colour of the trace (see the module's docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Colour {
    /// The colour.
    pub thread: Rgb,
    /// The area the trace paints in it, in square millimetres, its parts too narrow to count included.
    pub area: f64,
    /// The share of that area the stitches show in it, within half a thread.
    pub shown: f64,
    /// How many parts of the colour count: those as large as the smallest area sewn by default, that a disc a
    /// thread across fits in.
    pub parts: usize,
    /// How many of them the stitches keep.
    pub kept: usize,
}

impl Colour {
    /// The share of the colour's parts the stitches keep: 1 for a colour with no part that counts.
    pub fn likeness(&self) -> f64 {
        if self.parts == 0 { 1.0 } else { self.kept as f64 / self.parts as f64 }
    }
}

/// What a plan's stitches keep of its picture, colour by colour ([`likeness`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Likeness {
    /// Each colour of the trace, the largest area first, and of 2 as large the one painted first.
    pub colours: Vec<Colour>,
    /// Every part that counts, by colour in the order of `colours`, then from the top left.
    pub parts: Vec<Part>,
}

impl Likeness {
    /// The picture's likeness: the mean of its colours' (see the module's docs), 1 for a picture with no
    /// colour.
    pub fn likeness(&self) -> f64 {
        if self.colours.is_empty() {
            return 1.0;
        }
        self.colours.iter().map(Colour::likeness).sum::<f64>() / self.colours.len() as f64
    }

    /// The parts the stitches lose, in the order of `parts`.
    pub fn lost(&self) -> impl Iterator<Item = &Part> {
        self.parts.iter().filter(|part| !part.kept())
    }
}

/// What the stitches of `plan`, which lie where `design` draws its shapes, keep of the design, colour by
/// colour (see the module's docs).
///
/// # Panics
///
/// When the picture needs more work to measure than the default budget allows.
pub fn likeness(design: &Design, plan: &StitchPlan) -> Likeness {
    let mut meter = Budget::DEFAULT.meter();
    let compared = Compared::of(design, plan, &mut meter).expect("the picture is measured within the work budget");
    compared.likeness
}

/// A map of what the stitches keep, as rows of sRGB pixels from the top left, one for every `step` pixels of
/// the measure's grid each way: each kept part in its colour, each lost part red, each part too narrow to
/// count grey, and bare fabric white. With `only` a colour, the other colours are pale grey, so that a map
/// for each colour shows that colour's parts alone. The first 2 numbers are its width and height.
///
/// # Panics
///
/// As [`likeness`] does, and when `step` is 0.
pub fn map(design: &Design, plan: &StitchPlan, step: usize, only: Option<Rgb>) -> (usize, usize, Vec<[u8; 3]>) {
    assert!(step > 0, "a step of at least 1 pixel");
    let mut meter = Budget::DEFAULT.meter();
    let compared = Compared::of(design, plan, &mut meter).expect("the picture is measured within the work budget");
    let grid = &compared.picture.grid;
    let (width, height) = (grid.columns.div_ceil(step), grid.rows.div_ceil(step));
    let mut pixels = Vec::with_capacity(width * height);
    for row in (0..grid.rows).step_by(step) {
        for column in (0..grid.columns).step_by(step) {
            let at = row * grid.columns + column;
            let colour = compared.picture.colour(at);
            pixels.push(match (colour, compared.part.get(at).copied().flatten()) {
                (None, _) => [255, 255, 255],
                (Some(colour), _) if only.is_some_and(|only| only != colour) => [235, 235, 235],
                (Some(_), None) => [200, 200, 200],
                (Some(colour), Some(part)) if compared.likeness.parts.get(part).is_some_and(Part::kept) => [colour.r, colour.g, colour.b],
                (Some(_), Some(_)) => [230, 0, 0],
            });
        }
    }
    (width, height, pixels)
}

/// The trace and the stitches compared (see the module's docs).
struct Compared {
    picture: Painted,
    /// The part each pixel belongs to, by its place in `likeness.parts`; `None` for bare fabric and parts
    /// too narrow to count.
    part: Vec<Option<usize>>,
    likeness: Likeness,
}

impl Compared {
    fn of(design: &Design, plan: &StitchPlan, meter: &mut Meter) -> Result<Compared, stitchcraft_core::Exhausted> {
        // The grid goes round the design alone, so that 2 plans of it are measured on the same pixels.
        let picture = Painted::of(design, &StitchPlan { blocks: Vec::new(), elements: Vec::new() });
        let (columns, rows) = (picture.grid.columns, picture.grid.rows);
        let size = columns * rows;
        // The colour of the last top stitch over each pixel.
        let mut sewn: Vec<Option<Rgb>> = vec![None; size];
        for stitch in plan.sewn_stitches().iter().filter(|stitch| stitch.role == Role::Top && !stitch.is_lock()) {
            meter.charge(1)?;
            let colour = plan.blocks.get(stitch.block).map(|block| block.thread.color);
            picture.each([stitch.from, stitch.to], |at| sewn[at] = colour);
        }
        let disc = disc_steps(THREAD);
        let colour = |at: usize| picture.colour(at);
        // Whether the stitches show the pixel's colour within half a thread of it, and whether a disc a thread
        // across, centred on it, lies in its colour.
        let (mut shown, mut fits) = (vec![false; size], vec![false; size]);
        for at in 0..size {
            meter.charge(1)?;
            let Some(own) = colour(at) else { continue };
            let around = || disc.iter().filter_map(|&step| stepped(at, step, columns, rows));
            shown[at] = sewn[at] == Some(own) || around().any(|k| sewn.get(k).copied().flatten() == Some(own));
            fits[at] = disc.iter().all(|&step| stepped(at, step, columns, rows).is_some_and(|k| colour(k) == Some(own)));
        }
        let area = |pixels: usize| pixels as f64 * PIXEL * PIXEL;
        let smallest = crate::designs::rules().auto_min_area_mm2;
        // Each colour's area, area shown, and parts, found by flooding from the top left.
        let mut totals: BTreeMap<Rgb, (usize, usize, usize)> = BTreeMap::new();
        let mut order: Vec<Rgb> = Vec::new();
        let mut found: Vec<(Rgb, Vec<usize>)> = Vec::new();
        let mut seen = vec![false; size];
        for start in 0..size {
            meter.charge(1)?;
            let Some(own) = colour(start) else { continue };
            let total = totals.entry(own).or_insert_with(|| {
                order.push(own);
                (0, 0, picture.owner.get(start).copied().flatten().unwrap_or(0))
            });
            total.0 += 1;
            total.1 += usize::from(shown[start]);
            if seen[start] {
                continue;
            }
            seen[start] = true;
            let (mut group, mut stack) = (Vec::new(), vec![start]);
            while let Some(at) = stack.pop() {
                meter.charge(1)?;
                group.push(at);
                for next in neighbours(at, columns, rows, true) {
                    if !seen[next] && colour(next) == Some(own) {
                        seen[next] = true;
                        stack.push(next);
                    }
                }
            }
            if area(group.len()) >= smallest && group.iter().any(|&at| fits[at]) {
                found.push((own, group));
            }
        }
        // The largest colour first, and of 2 as large the one painted first.
        order.sort_by_key(|colour| totals.get(colour).map(|&(pixels, _, first)| (std::cmp::Reverse(pixels), first)));
        let place = |colour: &Rgb| order.iter().position(|known| known == colour).unwrap_or(order.len());
        found.sort_by_key(|(colour, group)| (place(colour), group.first().copied()));
        let mut part = vec![None; size];
        let mut parts = Vec::with_capacity(found.len());
        for (k, (own, group)) in found.iter().enumerate() {
            meter.charge(group.len() as u64)?;
            // The part's core: its pixels a disc a thread across, centred on them, fits in.
            let core = group.iter().filter(|&&at| fits[at]);
            let (cored, visible) = core.fold((0_usize, 0_usize), |(cored, visible), &at| (cored + 1, visible + usize::from(shown[at])));
            for &at in group {
                part[at] = Some(k);
            }
            let at = Rect::around(group.iter().map(|&at| picture.centre(at))).expect("a part has a pixel");
            parts.push(Part { colour: *own, area: area(group.len()), shown: visible as f64 / cored.max(1) as f64, at });
        }
        let colours = order
            .iter()
            .map(|&thread| {
                let (pixels, visible, _) = totals.get(&thread).copied().unwrap_or_default();
                let mine = parts.iter().filter(|part| part.colour == thread);
                let (counted, kept) = mine.fold((0, 0), |(counted, kept), part| (counted + 1, kept + usize::from(part.kept())));
                Colour { thread, area: area(pixels), shown: visible as f64 / pixels.max(1) as f64, parts: counted, kept }
            })
            .collect();
        Ok(Compared { picture, part, likeness: Likeness { colours, parts } })
    }
}
