//! The constrained Delaunay triangulation of a part of a region: the part cut into triangles that have
//! every edge of its outline and holes among their edges, and whose circles hold no corner they can see.
//!
//! Design: `docs/src/design/algorithms/centre-lines.md` › The triangulation. A narrow part's centre line runs
//! through the middles of the edges its triangles share ([`super::strips`], `REQ-AUTO-014`), and those edges
//! cross the part only where the triangles are about as long as the part is wide. So points are first added
//! along the part's edges, as many as it takes for no piece to be longer than a length the caller gives, the
//! pieces of an edge as long as each other. Every point is put on the grid the region's points lie on
//! ([`region::on_grid`]), so that the differences of coordinates are exact floats and the exact predicates
//! stay cheap. Then:
//!
//! 1. **Points.** A box round the part, cut into 2 triangles, takes the points one at a time, ring by ring.
//!    A point inside a triangle splits it in 3, a point on an edge splits the 2 triangles beside it in 4,
//!    and a point already there is that point. The edges round the new point are then flipped until each
//!    passes the Delaunay test: the point across the edge lies outside the circle through the triangle's
//!    corners (Lawson's method). A point is found by walking towards it from the triangle the last one was
//!    found in, which in ring order takes a step or 2.
//! 2. **Edges.** Each edge of the part that the triangles don't have yet is made: the edges it crosses are
//!    flipped until none does (Sloan 1993), and a point on it between its ends cuts it in 2. The part's
//!    edges are never flipped again.
//! 3. **Inside.** The triangles round the box's corners lie outside the part, and crossing an edge of the
//!    part goes in or out, so the triangles inside it are found by walking out from the box.
//! 4. **Delaunay again.** The edges between triangles inside the part are flipped until each passes the
//!    test. Where 4 or more points lie on one circle that holds no other, the test can't choose how to cut
//!    them into triangles, and a tie-break does, as if each point's squared distance from the origin were
//!    raised by a vanishing amount of its own (a symbolic perturbation, after Edelsbrunner and Mücke 1990).
//!    There is then one constrained Delaunay triangulation, which Lawson's flips reach from any
//!    triangulation of a polygon, so the triangles are the same whatever order the points came in.
//!
//! Every question of where a point lies is decided exactly (`stitchcraft_core::exact`): flipping ends, and
//! a part gives the same triangles on every platform. Every step costs a unit of work. A mesh that loses its
//! shape is a bug, which the caller says.

mod mesh;

use stitchcraft_core::{Exhausted, Meter, Point};

use self::mesh::{Built, Failed, Mesh};
use super::region::geom::Envelope;
use super::region::{self, Polygon};

/// How many points the box round a part adds before the part's own.
const BOX: usize = 4;

/// A part cut into triangles.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Triangulation {
    /// The points: the part's corners and the points added along its edges, on the region's grid.
    pub(crate) points: Vec<Point>,
    /// The triangles inside the part.
    pub(crate) triangles: Vec<Triangle>,
}

/// A triangle of a part.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Triangle {
    /// Its corners, counter-clockwise in y-up axes, as indices of [`Triangulation::points`].
    pub(crate) corners: [usize; 3],
    /// The triangle across each edge, by its index in [`Triangulation::triangles`]: `None` across an edge
    /// of the part's outline or holes. Edge `k` lies opposite corner `k`.
    pub(crate) across: [Option<usize>; 3],
}

impl Triangle {
    /// Its edge `k`: the corners after corner `k`, counter-clockwise.
    pub(crate) fn edge(&self, k: usize) -> (usize, usize) {
        (self.corners[(k + 1) % 3], self.corners[(k + 2) % 3])
    }
}

/// The part could not be triangulated: the mesh lost its shape, a bug.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Broken;

/// The constrained Delaunay triangulation of `part`, points added along its edges so that no piece is
/// longer than `longest` millimetres, with `meter`'s work: a unit for each point added, and for each step
/// of each stage. A part of fewer than 3 points has no triangles.
pub(crate) fn triangulate(part: &Polygon, longest: f64, meter: &mut Meter) -> Result<Result<Triangulation, Broken>, Exhausted> {
    match built(part, longest, meter) {
        Ok(triangulation) => Ok(Ok(triangulation)),
        Err(Failed::Work(exhausted)) => Err(exhausted),
        Err(Failed::Broken) => Ok(Err(Broken)),
    }
}

/// [`triangulate`], with its failures as the mesh has them.
fn built(part: &Polygon, longest: f64, meter: &mut Meter) -> Built<Triangulation> {
    let rings = refined(part, longest, meter)?;
    let all: Vec<Point> = rings.iter().flatten().copied().collect();
    if all.len() < 3 {
        return Ok(Triangulation::default());
    }
    let bounds = Envelope::of(&all);
    let corner = |x: f64, y: f64| Point::new(x, y).ok();
    let (Some(low), Some(high)) = (corner(bounds.min_x, bounds.min_y), corner(bounds.max_x, bounds.max_y)) else { return Err(Failed::Broken) };
    let mut mesh = Mesh::boxed(low, high).ok_or(Failed::Broken)?;
    let mut numbered = Vec::with_capacity(rings.len());
    // A ring of fewer than 3 points bounds nothing, and its edges would only cut the part.
    for ring in rings.iter().filter(|ring| ring.len() >= 3) {
        let mut indices = Vec::with_capacity(ring.len());
        for &p in ring {
            indices.push(mesh.insert(p, meter)?);
        }
        numbered.push(indices);
    }
    for ring in &numbered {
        for (p, q) in ring.iter().zip(ring.iter().cycle().skip(1)) {
            mesh.constrain(*p, *q, meter)?;
        }
    }
    let inside = mesh.inside(meter)?;
    mesh.settle(&inside, meter)?;
    finished(&mesh, &inside)
}

/// The rings of `part`, each open, with points added along every edge longer than `longest` so that its
/// pieces are as long as each other and none is longer, every point on the region's grid. A unit of
/// `meter`'s work for each point.
fn refined(part: &Polygon, longest: f64, meter: &mut Meter) -> Result<Vec<Vec<Point>>, Exhausted> {
    let mut rings = Vec::new();
    for ring in part.rings() {
        let mut points = Vec::with_capacity(ring.len());
        for pair in ring.windows(2) {
            let [p, q] = pair else { continue };
            meter.charge(1)?;
            points.push(region::on_grid(*p));
            let pieces = (p.distance(*q) / longest).ceil();
            // A count of pieces that isn't a number, as for a length of 0, adds nothing.
            let mut piece = 1.0;
            while piece < pieces {
                meter.charge(1)?;
                points.push(region::on_grid(p.lerp(*q, piece / pieces)));
                piece += 1.0;
            }
        }
        rings.push(points);
    }
    Ok(rings)
}

/// The triangles of `mesh` that `inside` marks, numbered afresh, and the points without the box's.
fn finished(mesh: &Mesh, inside: &[bool]) -> Built<Triangulation> {
    let mut numbers = Vec::with_capacity(mesh.tris.len());
    let mut count = 0;
    for &is_inside in inside {
        numbers.push(is_inside.then(|| {
            count += 1;
            count - 1
        }));
    }
    let mut triangles = Vec::with_capacity(count);
    for (tri, &is_inside) in mesh.tris.iter().zip(inside) {
        if !is_inside {
            continue;
        }
        let mut corners = [0; 3];
        let mut across = [None; 3];
        for k in 0..3 {
            // A box's corner is never inside the part, which lies well within the box.
            corners[k] = tri.corner(k).checked_sub(BOX).ok_or(Failed::Broken)?;
            across[k] = tri.across(k).filter(|_| !tri.is_fixed(k)).and_then(|u| numbers.get(u).copied().flatten());
        }
        triangles.push(Triangle { corners, across });
    }
    Ok(Triangulation { points: mesh.points.get(BOX..).unwrap_or_default().to_vec(), triangles })
}

#[cfg(test)]
mod tests;
