//! Test helpers for the normalize modules: reading the geometry fixtures shapely's answers are recorded in
//! (`conformance/fixtures/geometry/`), for the modules that must answer as shapely does, and parts drawn by
//! their corners, for the modules that take a region's parts.

// Test code may unwrap (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use stitchcraft_core::Point;

use crate::normalize::region::{Polygon, geom};

/// A part with the outline `outline` and the holes `holes`, each a ring of corners, wound as a region winds
/// them: the outline clockwise in y-up axes, and the holes the other way.
pub(crate) fn part(outline: &[(f64, f64)], holes: &[&[(f64, f64)]]) -> Polygon {
    let ring = |points: &[(f64, f64)], clockwise: bool| {
        let mut ring: Vec<Point> = points.iter().map(|&(x, y)| Point::new(x, y).unwrap()).collect();
        if (geom::signed_area(&geom::closed(&ring)) < 0.0) != clockwise {
            ring.reverse();
        }
        geom::closed(&ring)
    };
    Polygon { outline: ring(outline, true), holes: holes.iter().map(|hole| ring(hole, false)).collect() }
}

/// The words of one case, read in order: numbers, counts, points and polylines (a count of points and
/// their coordinates).
pub(crate) struct Words<'a>(pub(crate) std::str::SplitWhitespace<'a>);

impl Words<'_> {
    pub(crate) fn word(&mut self) -> &str {
        self.0.next().unwrap()
    }
    pub(crate) fn number(&mut self) -> f64 {
        self.word().parse().unwrap()
    }
    pub(crate) fn count(&mut self) -> usize {
        self.word().parse().unwrap()
    }
    pub(crate) fn point(&mut self) -> Point {
        let x = self.number();
        Point::new(x, self.number()).unwrap()
    }
    pub(crate) fn polyline(&mut self) -> Vec<Point> {
        (0..self.count()).map(|_| self.point()).collect()
    }
    pub(crate) fn polylines(&mut self) -> Vec<Vec<Point>> {
        (0..self.count()).map(|_| self.polyline()).collect()
    }
}

/// The cases of a fixture: its lines other than comments, with their line numbers from 1.
pub(crate) fn cases(fixture: &str) -> impl Iterator<Item = (usize, Words<'_>)> {
    fixture.lines().enumerate().filter(|(_, line)| !line.starts_with('#')).map(|(i, line)| (i + 1, Words(line.split_whitespace())))
}
