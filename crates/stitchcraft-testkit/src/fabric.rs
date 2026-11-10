//! What a plan leaves of a picture: the fabric its thread leaves bare where the picture has a colour, and
//! the thread that shows where the picture has another colour or none
//! (`docs/src/design/image-to-embroidery.md` › Measured). Both measures take any design and a plan of it
//! whose stitches lie where the design draws its shapes ([`crate::designs::in_place`]), so that a photo
//! traced into shapes is measured as the test pictures are.
//!
//! **The picture** is the design drawn on a grid of square pixels [`PIXEL`] wide, as a viewer paints it: each
//! pixel has the colour of the last shape drawn over its centre, its element's thread. A fill's gradient is
//! left out, which is right as long as automatic digitizing never makes a linear gradient fill. An area holds
//! the pixels inside it by its fill rule. A line holds those within half its width of its path, its corners
//! round, and past an open end as far as its cap reaches. Each shape is cut to its clips. Digitizing leaves
//! out the picture's background (`SC-W0906`) but for what shows of it inside the picture. So the background's
//! pixels that the grid's edge reaches without crossing another shape are bare fabric, as are the pixels no
//! shape holds.
//!
//! **Unsewn fabric** ([`unsewn`]) is what of the picture's colours no top stitch covers. A pixel is covered
//! when its centre lies within half of [`THREAD`] of a top stitch, as [`crate::metrics`] counts coverage. A
//! line narrower than a satin column runs along its middle, and a sliver of it beside the thread, narrower
//! than a thread, is no fabric that shows. So the uncovered pixels are opened with a disc a thread across: a
//! pixel is in a patch when it lies in such a disc, centred on a pixel, all of whose pixels are uncovered.
//! Pixels in patches that touch, at a side or a corner, are one patch. The measure is the largest patch's
//! area and the share of the picture's colours in patches.
//!
//! **Thread astray** ([`astray`]) is thread that shows where it should not: where the picture has another
//! colour or none, as a running stitch does that travels on top of a fill of another colour sewn before it,
//! and anywhere for an underlay or a travel stitch, which is meant to lie under the top stitches. Each
//! stitch's centre line is followed in steps of about a [`PIXEL`]. A step shows where no later stitch's
//! thread covers the pixel under it. A step of a top stitch is astray where it shows and neither that pixel
//! nor a pixel beside it has the colour of the stitch's thread: a row may run along its shape's edge. A
//! satin column reaches past its rails by its pull compensation, 0.4 mm, so a stitch is astray when a
//! stretch of it [`ASTRAY`] long or longer is. Locks are left out, as they are meant to be covered. A stitch
//! from one element's needle point to the next element's is a move, which the plan sews where the 2 lie
//! nearer than its collapse length rather than jump (`REQ-ASM-002`). Moves astray are counted apart from the
//! elements' own stitches, and so are the jump threads that a machine cutting no jump leaves
//! ([`jump_threads`]).

use stitchcraft_core::{Budget, Code, Point, Rect};
use stitchcraft_engine::auto;
use stitchcraft_engine::design::{Cap, Clip, Design, Element, Join, Shape};
use stitchcraft_engine::normalize::{region, stroke};
use stitchcraft_plan::{Rgb, Role, SewnStitch, StitchKind, StitchPlan};

use crate::metrics::{Grid, band, crossing, disc, index, spans};
pub use crate::metrics::{PIXEL, THREAD};
use crate::stitches::{JumpThread, jump_threads};

mod cover;
pub use cover::{BARE, Cover, GAP, cover, painted};

/// How long a stretch of a stitch, in millimetres, may lie where the picture has another colour before the
/// stitch is astray: past a satin column's rail by its pull compensation, 0.4 mm, and twice that.
pub const ASTRAY: f64 = 1.0;

/// The fabric a plan leaves bare where its picture has a colour ([`unsewn`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Unsewn {
    /// The largest patch's area, in square millimetres: 0 when there is none.
    pub largest: f64,
    /// The box round the largest patch's pixels' centres; `None` when there is no patch.
    pub at: Option<Rect>,
    /// How many patches there are.
    pub patches: usize,
    /// The share of the picture's colours in patches.
    pub share: f64,
}

/// The thread a plan lays where it shows and should not, and the jump threads it leaves ([`astray`]).
#[derive(Clone, Debug, PartialEq)]
pub struct Astray {
    /// The elements' own stitches with a stretch [`ASTRAY`] long or longer astray, each from its needle point
    /// to the next.
    pub stitches: Vec<[Point; 2]>,
    /// The longest stretch of an element's own stitch astray, in millimetres.
    pub longest: f64,
    /// The stitch with that stretch; `None` when no such stitch has a step astray.
    pub at: Option<[Point; 2]>,
    /// The moves from one element to the next with a stretch [`ASTRAY`] long or longer astray.
    pub moves: Vec<[Point; 2]>,
    /// The jump threads the reference machine leaves, which cuts no jump.
    pub jumps: Vec<JumpThread>,
}

/// The fabric `plan`, whose stitches lie where `design` draws its shapes, leaves bare where the picture has a
/// colour (see the module's docs).
pub fn unsewn(design: &Design, plan: &StitchPlan) -> Unsewn {
    let picture = Painted::of(design, plan);
    let grid = &picture.grid;
    let mut covered = vec![false; grid.columns * grid.rows];
    for stitch in plan.sewn_stitches().iter().filter(|stitch| stitch.role == Role::Top) {
        picture.each([stitch.from, stitch.to], |at| covered[at] = true);
    }
    let bare: Vec<bool> = picture.owner.iter().zip(&covered).map(|(owner, covered)| owner.is_some() && !covered).collect();
    let patches = opened(&bare, grid.columns, grid.rows);
    let groups = groups(&patches, grid.columns);
    let area = |pixels: usize| pixels as f64 * PIXEL * PIXEL;
    let largest = groups.iter().max_by_key(|group| group.len());
    let coloured = picture.owner.iter().filter(|owner| owner.is_some()).count();
    let in_patches: usize = groups.iter().map(Vec::len).sum();
    Unsewn {
        largest: largest.map_or(0.0, |group| area(group.len())),
        at: largest.and_then(|group| Rect::around(group.iter().map(|&at| picture.centre(at)))),
        patches: groups.len(),
        share: if coloured == 0 { 0.0 } else { in_patches as f64 / coloured as f64 },
    }
}

/// The thread `plan`, whose stitches lie where `design` draws its shapes, lays where it shows and should not,
/// and the jump threads it leaves (see the module's docs).
pub fn astray(design: &Design, plan: &StitchPlan) -> Astray {
    let picture = Painted::of(design, plan);
    let grid = &picture.grid;
    let sewn: Vec<_> = plan.sewn_stitches().into_iter().filter(|stitch| !stitch.is_lock()).collect();
    // The last stitch whose thread covers each pixel, by its place among `sewn`, plus 1: 0 for none.
    let mut last = vec![0_usize; grid.columns * grid.rows];
    for (k, stitch) in sewn.iter().enumerate() {
        picture.each([stitch.from, stitch.to], |at| last[at] = k + 1);
    }
    let (mut stitches, mut moves, mut longest, mut worst) = (Vec::new(), Vec::new(), 0.0_f64, None);
    for (k, stitch) in sewn.iter().enumerate() {
        let colour = plan.blocks.get(stitch.block).map(|block| block.thread.color);
        // An underlay or travel stitch is meant to lie under the top stitches: it is astray wherever it shows.
        let under = matches!(stitch.role, Role::Underlay | Role::Travel);
        let length = stitch.from.distance(stitch.to);
        let steps = (length / PIXEL).ceil().max(1.0);
        let (mut run, mut most) = (0.0_f64, 0.0_f64);
        let mut step = 0.0;
        while step < steps {
            let at = stitch.from.lerp(stitch.to, (step + 0.5) / steps);
            let pixel = grid.holding(at).map(|(column, row)| row * grid.columns + column);
            let shows = pixel.is_none_or(|at| last.get(at).is_none_or(|&last| last <= k + 1));
            // A pixel beside one of the stitch's colour is its shape's edge, where a row may run.
            let elsewhere = pixel
                .is_none_or(|at| std::iter::once(at).chain(neighbours(at, grid.columns, grid.rows, true)).all(|at| picture.colour(at) != colour));
            run = if shows && (elsewhere || under) { run + length / steps } else { 0.0 };
            most = most.max(run);
            step += 1.0;
        }
        if moved(plan, stitch) {
            if most >= ASTRAY {
                moves.push([stitch.from, stitch.to]);
            }
            continue;
        }
        if most > longest {
            (longest, worst) = (most, Some([stitch.from, stitch.to]));
        }
        if most >= ASTRAY {
            stitches.push([stitch.from, stitch.to]);
        }
    }
    Astray { stitches, longest, at: worst, moves, jumps: jump_threads(plan) }
}

/// Whether `stitch` of `plan` moves from one element to the next: its needle points belong to 2 elements.
fn moved(plan: &StitchPlan, stitch: &SewnStitch) -> bool {
    let Some(block) = plan.blocks.get(stitch.block) else { return false };
    let to = block.stitches.get(stitch.index).map(|entry| entry.origin.element);
    let before = block.stitches.get(..stitch.index).unwrap_or_default();
    let from = before.iter().rev().find(|entry| entry.kind == StitchKind::Normal).map(|entry| entry.origin.element);
    from.is_some() && from != to
}

/// A design drawn on a grid as a viewer paints it: the shape whose colour each pixel has.
struct Painted {
    grid: Grid,
    /// The place in the design of the shape whose colour each pixel has, by row then column; `None` for bare
    /// fabric.
    owner: Vec<Option<usize>>,
    /// The colour of each shape's thread, by its place in the design.
    colours: Vec<Rgb>,
}

impl Painted {
    /// `design` drawn on a grid round its shapes and `plan`'s stitches (see the module's docs).
    fn of(design: &Design, plan: &StitchPlan) -> Painted {
        let elements = design.elements();
        let reach = elements.iter().map(|element| reach(&element.shape)).fold(THREAD, f64::max);
        let around = design.bounds().into_iter().chain(plan.bounds()).reduce(|a, b| a.include(b.min()).include(b.max()));
        let bounds = around.map_or_else(|| Rect::around([Point::ORIGIN]).unwrap(), |rect| grown(rect, reach + 2.0 * PIXEL));
        let grid = Grid::new(bounds, PIXEL);
        let mut owner = vec![None; grid.columns * grid.rows];
        for (place, element) in elements.iter().enumerate() {
            for at in held(&grid, element) {
                owner[at] = Some(place);
            }
        }
        if background(design) {
            outside_of_picture(&mut owner, grid.columns, grid.rows);
        }
        Painted { grid, owner, colours: elements.iter().map(|element| element.thread.color).collect() }
    }

    /// The colour of the shape pixel `at` has; `None` for bare fabric.
    fn colour(&self, at: usize) -> Option<Rgb> {
        self.owner.get(at).copied().flatten().and_then(|place| self.colours.get(place).copied())
    }

    /// The centre of pixel `at`.
    fn centre(&self, at: usize) -> Point {
        Point::new(self.grid.x(at % self.grid.columns), self.grid.y(at / self.grid.columns)).unwrap()
    }

    /// Calls `mark` with each pixel whose centre lies within half of [`THREAD`] of the stitch `[from, to]`.
    fn each(&self, [from, to]: [Point; 2], mut mark: impl FnMut(usize)) {
        let reach = THREAD / 2.0;
        for row in self.grid.rows_within(from.y().min(to.y()) - reach, from.y().max(to.y()) + reach) {
            let Some((low, high)) = crossing(from, to, reach, self.grid.y(row)) else { continue };
            for column in self.grid.columns_within(low, high) {
                mark(row * self.grid.columns + column);
            }
        }
    }
}

/// How far past its path's points a shape paints: half a line's width times √2, which holds a square cap's
/// corners, and a miter's as a satin column's rails clip it; nothing for an area.
fn reach(shape: &Shape) -> f64 {
    match shape {
        Shape::Stroke { width, join: Join::Miter { limit }, .. } => width.get() / 2.0 * (limit * limit + 1.0).sqrt(),
        Shape::Stroke { width, .. } => width.get() / 2.0 * std::f64::consts::SQRT_2,
        Shape::Fill { .. } => 0.0,
    }
}

/// `rect` grown by `by` on every side.
fn grown(rect: Rect, by: f64) -> Rect {
    let at = |x: f64, y: f64| Point::new(x, y).unwrap();
    Rect::around([at(rect.min().x() - by, rect.min().y() - by), at(rect.max().x() + by, rect.max().y() + by)]).unwrap()
}

/// Whether digitizing leaves out `design`'s lowest shape as the picture's background (`SC-W0906`): the
/// warning names it. One whose verdict needed more work than the budget allows, and is sewn, is named too,
/// and taken as left out.
fn background(design: &Design) -> bool {
    let Some(lowest) = design.elements().first() else { return false };
    let (_, said) = auto::digitized(design, &Budget::DEFAULT);
    said.iter().any(|d| d.code == Code::AutoBackground && d.element.as_ref() == Some(&lowest.id))
}

/// The pixels of `grid` that `element` paints: an area's by its fill rule and a line's by its width, joins
/// and caps, each cut to its clips.
fn held(grid: &Grid, element: &Element) -> Vec<usize> {
    let pixels = match &element.shape {
        Shape::Fill { path, rule, .. } => {
            let built = region::build_clipped(path, *rule, &element.clips, &mut Budget::DEFAULT.meter()).unwrap();
            filled(grid, &built.region)
        }
        Shape::Stroke { path, width, cap, .. } => {
            let flat = stroke::flatten(path, stroke::SHAPE_TOLERANCE, &mut Budget::DEFAULT.meter()).unwrap();
            let lined = lined(grid, &flat.pieces, width.get() / 2.0, *cap);
            clipped(grid, lined, &element.clips)
        }
    };
    pixels.into_iter().enumerate().filter(|(_, held)| *held).map(|(at, _)| at).collect()
}

/// The pixels of `grid` inside `region`, by the even-odd rule over its rings: a region's parts don't overlap.
fn filled(grid: &Grid, region: &region::Region) -> Vec<bool> {
    let mut held = vec![false; grid.columns * grid.rows];
    let rings: Vec<&[Point]> =
        region.parts.iter().flat_map(|part| std::iter::once(part.outline.as_slice()).chain(part.holes.iter().map(Vec::as_slice))).collect();
    for row in 0..grid.rows {
        for (from, to) in spans(&rings, grid.y(row)) {
            // Half-open, so that 2 stretches meeting at a pixel's centre hold it once.
            for column in grid.columns_within(from, to).filter(|&column| grid.x(column) < to) {
                held[row * grid.columns + column] = true;
            }
        }
    }
    held
}

/// The pixels of `grid` that a line along `pieces`, `half` its width either side, paints: a band along each
/// segment, a disc at each corner, and at an open end as far past it as `cap` reaches. A piece of one point
/// is a dot, which a round cap draws as a disc and a square one as a square.
fn lined(grid: &Grid, pieces: &[stroke::Piece], half: f64, cap: Cap) -> Vec<bool> {
    let mut held = vec![false; grid.columns * grid.rows];
    let mut mark = |stretch: Option<(f64, f64)>, row: usize| {
        let Some((low, high)) = stretch else { return };
        for column in grid.columns_within(low, high) {
            held[row * grid.columns + column] = true;
        }
    };
    for piece in pieces {
        let points = &piece.points;
        let closed = points.len() > 2 && points.first() == points.last();
        let (Some(&first), Some(&last)) = (points.first(), points.last()) else { continue };
        // Past each open end, along the way the line ends in.
        let past = |end: Point, before: Option<Point>| {
            let (dx, dy) = before.map_or((1.0, 0.0), |b| (end.x() - b.x(), end.y() - b.y()));
            let length = stitchcraft_core::math::hypot(dx, dy);
            let (ux, uy) = if length > 0.0 { (dx / length, dy / length) } else { (1.0, 0.0) };
            Point::new(end.x() + ux * half, end.y() + uy * half).unwrap()
        };
        let ends = [(first, points.get(1).copied()), (last, points.len().checked_sub(2).and_then(|k| points.get(k)).copied())];
        let (low, high) = points.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), p| (low.min(p.y()), high.max(p.y())));
        for row in grid.rows_within(low - 2.0 * half, high + 2.0 * half) {
            let y = grid.y(row);
            for pair in points.windows(2) {
                mark(band(pair[0], pair[1], half, y), row);
            }
            // The corners: every point of a closed line, and those between an open line's ends.
            let corners = if closed { points.as_slice() } else { points.get(1..points.len().saturating_sub(1)).unwrap_or_default() };
            for &corner in corners {
                mark(disc(corner, half, y), row);
            }
            if closed {
                continue;
            }
            for &(end, before) in &ends {
                match cap {
                    Cap::Butt => {}
                    Cap::Round => mark(disc(end, half, y), row),
                    Cap::Square if points.len() == 1 => {
                        let side = |dx: f64| Point::new(end.x() + dx, end.y()).unwrap();
                        mark(band(side(-half), side(half), half, y), row);
                    }
                    Cap::Square => mark(band(end, past(end, before), half, y), row),
                }
            }
        }
    }
    held
}

/// `held`, pixels of `grid`, cut to `clips`: kept where each clip holds them, a clip holding the pixels any
/// of its shapes does.
fn clipped(grid: &Grid, mut held: Vec<bool>, clips: &[Clip]) -> Vec<bool> {
    for clip in clips {
        let mut inside = vec![false; held.len()];
        for shape in clip.shapes.iter() {
            let built = region::build(&shape.path, shape.rule, &mut Budget::DEFAULT.meter()).unwrap();
            for (at, held) in filled(grid, &built.region).into_iter().enumerate() {
                inside[at] |= held;
            }
        }
        for (held, inside) in held.iter_mut().zip(inside) {
            *held &= inside;
        }
    }
    held
}

/// Makes bare fabric of the background's pixels, the lowest shape's, that the grid's edge reaches through
/// pixels of bare fabric or of the background: what shows of it inside the picture is sewn.
fn outside_of_picture(owner: &mut [Option<usize>], columns: usize, rows: usize) {
    let open = |owner: &[Option<usize>], at: usize| owner.get(at).is_some_and(|owner| matches!(owner, None | Some(0)));
    let mut reached = vec![false; owner.len()];
    let edge = (0..columns).flat_map(|column| [column, (rows.saturating_sub(1)) * columns + column]);
    let sides = (0..rows).flat_map(|row| [row * columns, row * columns + columns.saturating_sub(1)]);
    let mut stack: Vec<usize> = edge.chain(sides).filter(|&at| open(owner, at)).collect();
    while let Some(at) = stack.pop() {
        if reached.get(at) != Some(&false) {
            continue;
        }
        reached[at] = true;
        stack.extend(neighbours(at, columns, rows, false).filter(|&next| open(owner, next)));
    }
    for (owner, reached) in owner.iter_mut().zip(reached) {
        if reached {
            *owner = None;
        }
    }
}

/// The pixels beside pixel `at` of a grid `columns` wide and `rows` tall: those at its sides, and at its
/// corners too when `corners`.
fn neighbours(at: usize, columns: usize, rows: usize, corners: bool) -> impl Iterator<Item = usize> {
    let (column, row) = ((at % columns) as isize, (at / columns) as isize);
    let steps: &[(isize, isize)] =
        if corners { &[(-1, -1), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] } else { &[(0, -1), (-1, 0), (1, 0), (0, 1)] };
    steps.iter().filter_map(move |&(dx, dy)| {
        let (x, y) = (column + dx, row + dy);
        (x >= 0 && y >= 0 && (x as usize) < columns && (y as usize) < rows).then(|| y as usize * columns + x as usize)
    })
}

/// `bare`, pixels of a grid `columns` wide and `rows` tall, opened with a disc a thread across: those in a disc
/// centred on a pixel whose pixels are all bare.
fn opened(bare: &[bool], columns: usize, rows: usize) -> Vec<bool> {
    opened_with(bare, columns, rows, THREAD, false)
}

/// `bare` opened with a disc `across` millimetres across, the pixels beyond the grid taken as bare or not as
/// `beyond` says.
fn opened_with(bare: &[bool], columns: usize, rows: usize, across: f64, beyond: bool) -> Vec<bool> {
    let disc = disc_steps(across);
    let mut opened = vec![false; bare.len()];
    for (centre, _) in fitting(bare, columns, rows, across, beyond).iter().enumerate().filter(|(_, fits)| **fits) {
        for k in disc.iter().filter_map(|&step| stepped(centre, step, columns, rows)) {
            opened[k] = true;
        }
    }
    opened
}

/// The pixels of `bare` that a disc `across` millimetres across, centred on them, fits on: `bare` without
/// its edge. The pixels beyond the grid are taken as bare or not as `beyond` says.
fn fitting(bare: &[bool], columns: usize, rows: usize, across: f64, beyond: bool) -> Vec<bool> {
    let disc = disc_steps(across);
    let fits = |centre: usize| disc.iter().all(|&step| stepped(centre, step, columns, rows).map_or(beyond, |k| bare[k]));
    (0..bare.len()).map(|centre| bare[centre] && fits(centre)).collect()
}

/// The steps from a pixel to each pixel of the disc `across` millimetres across that is centred on it.
fn disc_steps(across: f64) -> Vec<(isize, isize)> {
    let radius = across / 2.0 / PIXEL;
    let span = index(radius.floor()) as isize;
    (-span..=span).flat_map(|dy| (-span..=span).map(move |dx| (dx, dy))).filter(|&(dx, dy)| ((dx * dx + dy * dy) as f64) <= radius * radius).collect()
}

/// The pixel `(dx, dy)` away from pixel `at` of a grid `columns` wide and `rows` tall; `None` beyond the grid.
fn stepped(at: usize, (dx, dy): (isize, isize), columns: usize, rows: usize) -> Option<usize> {
    let (x, y) = ((at % columns) as isize + dx, (at / columns) as isize + dy);
    (x >= 0 && y >= 0 && (x as usize) < columns && (y as usize) < rows).then(|| y as usize * columns + x as usize)
}

/// The groups of `pixels`, of a grid `columns` wide, that touch at a side or a corner, each as its pixels.
fn groups(pixels: &[bool], columns: usize) -> Vec<Vec<usize>> {
    let rows = pixels.len() / columns.max(1);
    let mut seen = vec![false; pixels.len()];
    let mut groups = Vec::new();
    for start in 0..pixels.len() {
        if !pixels[start] || seen[start] {
            continue;
        }
        seen[start] = true;
        let (mut group, mut stack) = (Vec::new(), vec![start]);
        while let Some(at) = stack.pop() {
            group.push(at);
            for next in neighbours(at, columns, rows, true) {
                if pixels[next] && !seen[next] {
                    seen[next] = true;
                    stack.push(next);
                }
            }
        }
        groups.push(group);
    }
    groups
}

#[cfg(test)]
mod tests;
