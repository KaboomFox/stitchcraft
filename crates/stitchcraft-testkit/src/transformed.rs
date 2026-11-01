//! Designs changed in ways that must change their stitches only by the same change. Mirrored or turned half
//! round, a design must sew its stitches mirrored or turned with it. Drawn the other way round, from another
//! of its points, or with points added along its straight edges, a shape must sew the same thread on the
//! same fabric. A rule that depends on which way a shape happens to be drawn, or on a tie
//! broken by its coordinates, shows as a difference. A T whose bar kept one arm and lost the other
//! (`REQ-AUTO-014`) was such a rule, and so were bugs Ink/Stitch fixed in its own stitches since 2025: flipped
//! cross stitches, a rotated grid, a transform in auto-run.
//!
//! [`Change`] makes the changed design, and [`compare`] plans both for the reference machine and measures,
//! for each element, how much of each plan's thread lies within [`TOLERANCE`] of the other's
//! ([`crate::metrics::footprint_within`]) once the first's stitches are changed too, and how many stitches
//! each sews. The tolerance overlooks needle points placed a little apart, as along a line sewn from its
//! other end, and still counts a column sewn as a line along its middle, or a branch left out.
//!
//! A design is mirrored and turned about the point (0, 0) of its coordinates, from which a fill lays out
//! its rows and their needle points (`REQ-FILL-TAT-003`), so that the rows of the changed design lie where
//! the changed rows do. Moved, a fill's rows would lie elsewhere across it, as Ink/Stitch's do, so a move is
//! no change here. A setting that is a direction turns the other way when the design is mirrored, as
//! Ink/Stitch turns a mirrored clone's fill angle: the rows of a tatami fill at 30° run at −30° in its
//! mirror image. Plans are measured from the design's origin, which the change takes with the design, so
//! a plan's points change as the design's do.

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::design::{Design, Element, Path, Segment, Shape, Subpath};
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::normalize::region::{self, Region};
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_engine::plan;
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{ElementRef, Role, StitchKind, StitchPlan};

use crate::designs::p;
use crate::metrics::footprint_within;

/// How far apart the thread of 2 plans may lie and still count as in the same place, in millimetres: the
/// width of a thread less a little, under half the narrowest satin column, 1 mm, which a line along its
/// middle sews in its place.
pub const TOLERANCE: f64 = 0.3;

/// A change to a design that must change its stitches only by the same change.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Mirrored left to right, about x = 0.
    MirrorX,
    /// Mirrored top to bottom, about y = 0.
    MirrorY,
    /// Turned half round about (0, 0).
    HalfTurn,
    /// Each shape drawn the other way round. A stroke is drawn from its end, its subpaths in the other
    /// order, so that it sews the same way between them, but a satin column keeps its rails in their order,
    /// each drawn from its other end, since the first rail is the one an E-stitch's spine runs along. A fill's
    /// rings are each drawn the other way round.
    Reverse,
    /// Each ring of a fill drawn from the point halfway through its nodes. A stroke starts to sew where it is
    /// drawn from, so it is left as it is.
    Restart,
    /// Each straight segment split in 2 at its middle.
    Split,
}

/// The settings that are directions, in degrees, which turn the other way in a mirror image: a fill's
/// rows and its underlay's.
pub const DIRECTIONS: &[&str] = &["angle", "fill_underlay_angle"];

impl Change {
    /// Every change.
    pub const ALL: [Change; 6] = [Change::MirrorX, Change::MirrorY, Change::HalfTurn, Change::Reverse, Change::Restart, Change::Split];

    /// Whether the change mirrors the design, which turns its directions the other way.
    pub const fn mirrors(self) -> bool {
        matches!(self, Change::MirrorX | Change::MirrorY)
    }

    /// Where the change takes `q`, a point of a design, or of its plan, which it measures from the design's
    /// origin and the change takes with the design.
    pub fn point(self, q: Point) -> Point {
        match self {
            Change::MirrorX => p(-q.x(), q.y()),
            Change::MirrorY => p(q.x(), -q.y()),
            Change::HalfTurn => p(-q.x(), -q.y()),
            Change::Reverse | Change::Restart | Change::Split => q,
        }
    }

    /// `design`, changed. A design without an origin gets the middle of its box as its origin first, so that
    /// both plans are measured from the same point.
    pub fn design(self, design: &Design) -> Design {
        let design = anchored(design);
        let elements = design.elements().iter().map(|element| self.element(element)).collect();
        let mut settings = design.settings;
        settings.origin = settings.origin.map(|q| self.point(q));
        settings.stop_position = settings.stop_position.map(|q| self.point(q));
        let mut changed = Design::new(elements, settings).unwrap();
        changed.sort_colours = design.sort_colours;
        changed
    }

    /// `element`, changed.
    fn element(self, element: &Element) -> Element {
        let mut changed = element.clone();
        let fill = matches!(element.shape, Shape::Fill { .. });
        let satin = element.params.iter().any(|(key, value)| key == SATIN_COLUMN && value == "true");
        let mut path = self.path(element.shape.path(), fill);
        if self == Change::Reverse && !fill && !satin {
            path.subpaths.reverse();
        }
        changed.shape = match element.shape.clone() {
            Shape::Stroke { width, join, cap, .. } => Shape::Stroke { path, width, join, cap },
            Shape::Fill { rule, .. } => Shape::Fill { path, rule },
        };
        if self.mirrors() {
            for key in DIRECTIONS {
                let Some(value) = element.params.iter().find(|(k, _)| *k == *key).map(|(_, value)| value.to_string()) else { continue };
                let turned: Vec<String> =
                    value.split_whitespace().map(|angle| angle.parse::<f64>().map_or(angle.to_string(), |a| (-a).to_string())).collect();
                changed.params.set(*key, turned.join(" ").as_str());
            }
        }
        changed
    }

    /// `path`, a `fill`'s or a stroke's, changed, each subpath in its place.
    fn path(self, path: &Path, fill: bool) -> Path {
        let point = |q: Point| self.point(q);
        let subpaths = path.subpaths.iter().map(|subpath| match self {
            Change::Reverse => reversed(subpath),
            Change::Restart if fill => restarted(subpath),
            Change::Restart => subpath.clone(),
            Change::Split => split(subpath),
            _ => Subpath {
                start: point(subpath.start),
                segments: subpath
                    .segments
                    .iter()
                    .map(|segment| match *segment {
                        Segment::Line(end) => Segment::Line(point(end)),
                        Segment::Quad(control, end) => Segment::Quad(point(control), point(end)),
                        Segment::Cubic(first, second, end) => Segment::Cubic(point(first), point(second), point(end)),
                    })
                    .collect(),
                closed: subpath.closed,
            },
        });
        Path { subpaths: subpaths.collect() }
    }
}

/// `design` with the middle of its box as its origin, when it has none.
fn anchored(design: &Design) -> Design {
    let mut anchored = design.clone();
    if anchored.settings.origin.is_none() {
        let middle = design.bounds().map_or(Point::ORIGIN, |b| p((b.min().x() + b.max().x()) / 2.0, (b.min().y() + b.max().y()) / 2.0));
        anchored.settings.origin = Some(middle);
    }
    anchored
}

/// The points `subpath` passes through, its start first: each segment's end.
fn nodes(subpath: &Subpath) -> Vec<Point> {
    std::iter::once(subpath.start).chain(subpath.segments.iter().map(|segment| segment.end())).collect()
}

/// `subpath` drawn the other way round: from its last point, each curve's control points swapped.
fn reversed(subpath: &Subpath) -> Subpath {
    let points = nodes(subpath);
    let segments = subpath
        .segments
        .iter()
        .zip(&points)
        .rev()
        .map(|(segment, &from)| match *segment {
            Segment::Line(_) => Segment::Line(from),
            Segment::Quad(control, _) => Segment::Quad(control, from),
            Segment::Cubic(first, second, _) => Segment::Cubic(second, first, from),
        })
        .collect();
    Subpath { start: points.last().copied().unwrap_or(subpath.start), segments, closed: subpath.closed }
}

/// `subpath`, when it is closed, drawn from the point halfway through its nodes and round to it again: the
/// segment that closes it drawn as a line where it doesn't end at its start.
fn restarted(subpath: &Subpath) -> Subpath {
    let points = nodes(subpath);
    if !subpath.closed || points.len() < 3 {
        return subpath.clone();
    }
    let mut segments = subpath.segments.clone();
    if points.last() != points.first() {
        segments.push(Segment::Line(subpath.start));
    }
    let half = segments.len() / 2;
    segments.rotate_left(half);
    Subpath { start: points.get(half).copied().unwrap_or(subpath.start), segments, closed: true }
}

/// `subpath` with each straight segment split in 2 at its middle.
fn split(subpath: &Subpath) -> Subpath {
    let points = nodes(subpath);
    let segments = subpath
        .segments
        .iter()
        .zip(&points)
        .flat_map(|(segment, &from)| match *segment {
            Segment::Line(end) => vec![Segment::Line(from.lerp(end, 0.5)), Segment::Line(end)],
            curve => vec![curve],
        })
        .collect();
    Subpath { start: subpath.start, segments, closed: subpath.closed }
}

/// How an element sews in a design and in the design changed.
#[derive(Clone, Debug, PartialEq)]
pub struct Compared {
    /// The element's id.
    pub element: String,
    /// The share of each plan's thread that lies within [`TOLERANCE`] of the other's, the lesser, the
    /// first's stitches changed too: 1 when they sew the same thread in the same place.
    pub shared: f64,
    /// How many stitches it sews in the design, and in the design changed.
    pub stitches: (usize, usize),
}

/// How each element of `design` sews in it and in it changed by each of `changes`, all planned for the
/// reference machine, the design once: for each change, the elements by their ids. An element that sews in
/// neither plan is left out.
pub fn compare(design: &Design, changes: &[Change]) -> Vec<(Change, Vec<Compared>)> {
    let original = anchored(design);
    let before = stitches(&original, &planned(&original));
    changes
        .iter()
        .map(|&change| {
            let changed = change.design(&original);
            (change, compared(&before, &stitches(&changed, &planned(&changed)), change))
        })
        .collect()
}

/// How the elements of a design sew in its plan, `before`, and in the plan of the design changed by `change`,
/// `after`, each element's stitches by its id.
fn compared(before: &BTreeMap<String, Vec<[Point; 2]>>, after: &BTreeMap<String, Vec<[Point; 2]>>, change: Change) -> Vec<Compared> {
    let before: BTreeMap<&String, Vec<[Point; 2]>> =
        before.iter().map(|(element, sewn)| (element, sewn.iter().map(|&[a, b]| [change.point(a), change.point(b)]).collect())).collect();
    let mut ids: Vec<&String> = before.keys().copied().chain(after.keys()).collect();
    ids.sort();
    ids.dedup();
    ids.into_iter()
        .map(|id| {
            let (a, b) = (before.get(id).map_or(&[][..], Vec::as_slice), after.get(id).map_or(&[][..], Vec::as_slice));
            let shared = footprint_within(a, b, TOLERANCE).min(footprint_within(b, a, TOLERANCE));
            Compared { element: id.clone(), shared, stitches: (a.len(), b.len()) }
        })
        .collect()
}

/// The plan of `design` for the reference machine.
fn planned(design: &Design) -> StitchPlan {
    let outcome = plan(design, REFERENCE, &Budget::DEFAULT);
    outcome.plan.unwrap_or_else(|| panic!("no plan: {:?}", outcome.diagnostics))
}

/// The stitches of `plan`, the plan of `design`, that show or hold the fabric, top stitches and underlay, from
/// needle point to needle point, by the element of the design they sew. The parts automatic digitizing makes
/// of an element (`#2`, `#3`) count as the element, since a change may number them in another order. Travel,
/// locks and a stitch from another element are left out: where a stroke starts is where it is drawn from, so
/// drawn the other way round it may lock at its other end, or be reached by a stitch from the element before
/// instead of a jump. So is a stitch of an automatic area whose middle lies outside the area, by more than
/// [`TOLERANCE`]: automatic digitizing chooses the order its lines and columns sew in, and in a shape drawn
/// symmetric 2 of them tie, so the stitch that joins them crosses the fabric wherever the tie puts it.
fn stitches(design: &Design, plan: &StitchPlan) -> BTreeMap<String, Vec<[Point; 2]>> {
    let shows = |role: Role| matches!(role, Role::Top | Role::Underlay);
    let origin = design.settings.origin.unwrap_or(Point::ORIGIN);
    // The area of each automatic area, by its id.
    let areas: BTreeMap<&str, Region> = design
        .elements()
        .iter()
        .filter(|element| element.automatic)
        .filter_map(|element| match &element.shape {
            Shape::Fill { path, rule } => Some((element.id.as_str(), region::build(path, *rule, &mut Budget::DEFAULT.meter()).unwrap().region)),
            Shape::Stroke { .. } => None,
        })
        .collect();
    let mut by_element: BTreeMap<String, Vec<[Point; 2]>> = BTreeMap::new();
    // The needle's last point that a stitch went down at, and the element and role of that stitch.
    let mut last: Option<(Point, Option<ElementRef>, Role)> = None;
    for stitch in plan.stitches() {
        match stitch.kind {
            StitchKind::Normal => {
                let (element, role) = (stitch.origin.element, stitch.origin.role);
                if let Some((from, from_element, from_role)) = last
                    && from_element == element
                    && shows(role)
                    && shows(from_role)
                {
                    let id = element.and_then(|element| plan.element(element)).map_or("", |id| id.as_str());
                    let drawn = id.split_once('#').map_or(id, |(drawn, _)| drawn);
                    let middle = p(origin.x() + (from.x() + stitch.at.x()) / 2.0, origin.y() + (from.y() + stitch.at.y()) / 2.0);
                    if areas.get(drawn).is_none_or(|area| near(area, middle, TOLERANCE)) {
                        by_element.entry(drawn.to_string()).or_default().push([from, stitch.at]);
                    }
                }
                last = Some((stitch.at, element, role));
            }
            StitchKind::Jump | StitchKind::Trim => last = None,
            StitchKind::Stop => {}
        }
    }
    by_element
}

/// Whether `q` lies inside `area`, or no farther than `reach` from its outlines and holes.
fn near(area: &Region, q: Point, reach: f64) -> bool {
    let rings: Vec<&Vec<Point>> = area.parts.iter().flat_map(|part| std::iter::once(&part.outline).chain(&part.holes)).collect();
    let edges = || rings.iter().flat_map(|ring| ring.iter().zip(ring.iter().cycle().skip(1)));
    // Inside by the even-odd rule: a ray to the right crosses the rings an odd number of times.
    let crossings =
        edges().filter(|(a, b)| (a.y() > q.y()) != (b.y() > q.y()) && q.x() < a.x() + (q.y() - a.y()) * (b.x() - a.x()) / (b.y() - a.y())).count();
    crossings % 2 == 1 || edges().any(|(a, b)| distance_to_segment(q, *a, *b) <= reach)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::designs::{RED, fill, rings};

    /// A design of one fill, an L, its origin where it is drawn from.
    fn l_shape() -> Design {
        let element = fill("l", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 3.0), (3.0, 3.0), (3.0, 8.0), (0.0, 8.0)]], &RED, &[("angle", "30")]);
        Design::new(vec![element], stitchcraft_engine::design::DesignSettings { origin: Some(p(0.0, 0.0)), ..Default::default() }).unwrap()
    }

    #[test]
    fn a_mirror_image_takes_each_point_across_the_origin_and_turns_directions_the_other_way() {
        let changed = Change::MirrorX.design(&l_shape());
        let element = &changed.elements()[0];
        let points: Vec<(f64, f64)> = element.shape.path().points().map(|q| (q.x(), q.y())).collect();
        assert_eq!(points, [(0.0, 0.0), (-10.0, 0.0), (-10.0, 3.0), (-3.0, 3.0), (-3.0, 8.0), (0.0, 8.0)]);
        assert_eq!(element.params.iter().find(|(k, _)| *k == "angle").map(|(_, v)| v), Some("-30"));
        assert_eq!(Change::HalfTurn.point(p(1.0, 2.0)), p(-1.0, -2.0));
        assert_eq!(Change::Split.point(p(1.0, 2.0)), p(1.0, 2.0));
    }

    #[test]
    fn a_subpath_drawn_the_other_way_from_another_point_or_split_passes_through_the_same_points() {
        let ring = &rings(&[&[(0.0, 0.0), (4.0, 0.0), (4.0, 2.0), (0.0, 2.0)]]).subpaths[0];
        let xy = |subpath: &Subpath| nodes(subpath).iter().map(|q| (q.x(), q.y())).collect::<Vec<_>>();
        assert_eq!(xy(&reversed(ring)), [(0.0, 2.0), (4.0, 2.0), (4.0, 0.0), (0.0, 0.0)]);
        assert_eq!(xy(&restarted(ring)), [(4.0, 2.0), (0.0, 2.0), (0.0, 0.0), (4.0, 0.0), (4.0, 2.0)]);
        assert_eq!(xy(&split(ring)), [(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (4.0, 1.0), (4.0, 2.0), (2.0, 2.0), (0.0, 2.0)]);
    }

    #[test]
    fn an_l_sews_the_same_however_it_is_changed() {
        for (change, compared) in compare(&l_shape(), &Change::ALL) {
            for element in compared {
                assert!(element.shared > 0.99, "{change:?}: {element:?}");
            }
        }
    }
}
