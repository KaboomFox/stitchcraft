//! Automatic digitizing sews all of each part it sews along the middle of (`REQ-AUTO-014`, `REQ-AUTO-016`):
//! the line a narrow part runs along comes near every point of the part's outline, and the columns a part
//! sews as have all of its outline along their sides and ends. A centre line that leaves a branch out fails
//! here. The bar of a T 4 mm tall, drawn in lines 0.72 mm wide, lost an arm, so TS-13B's small T sewed as a
//! Γ, and no test noticed. Once that was fixed, this test found Ts 3 to 5 mm tall in lines 0.5 and 0.8 mm
//! wide that still lost one, their arms measured short of their caps. Checked on the TS-13 sheets'
//! automatic shapes, and on random letters T, H and E, 3 to 20 mm tall, drawn in strokes from 0.3 to 4 mm
//! wide.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use proptest::prelude::*;
use stitchcraft_core::{Budget, Point};
use stitchcraft_engine::auto::{Digitized, digitize};
use stitchcraft_engine::design::{DesignSettings, Element, Shape};
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::normalize::region::Polygon;
use stitchcraft_engine::normalize::satin::{Shape as Read, recognize};
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_engine::testsheets::find;
use stitchcraft_testkit::designs::{RED, automatic, fill, p};

/// How far a point of a narrow part's outline may lie from the part's line, in widths of the part. Across
/// the part it lies half a width away, and an end of the line stops no more than a width short of the
/// part's end (`REQ-AUTO-014`), the end's corners then about 0.7 of a width away. The end of a branch the
/// line leaves out lies farther.
const LINE_REACH: f64 = 1.5;

/// How far a point of a part's outline may lie from the outlines of its columns, in millimetres: a column's
/// rails are the part's outline on either side of it, between its ends (`REQ-AUTO-016`).
const COLUMN_REACH: f64 = 0.05;

/// The step at which a part's outline is measured, in millimetres.
const STEP: f64 = 0.1;

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// The polylines of the stroke `element`'s path: a line's branches.
fn polylines(element: &Element) -> Vec<Vec<Point>> {
    let Shape::Stroke { path, .. } = &element.shape else { panic!("`{}` is not a stroke: {:?}", element.id, element.shape) };
    path.subpaths.iter().map(|subpath| std::iter::once(subpath.start).chain(subpath.segments.iter().map(|segment| segment.end())).collect()).collect()
}

/// The outline of the satin column `element`: its first rail, then its second back to where the first starts.
fn column(element: &Element) -> Vec<Point> {
    let Shape::Stroke { path, .. } = &element.shape else { panic!("`{}` is not a stroke: {:?}", element.id, element.shape) };
    let Ok(Read::Rails(satin)) = recognize(path, &mut Budget::DEFAULT.meter()).unwrap().shape else { panic!("{}: {path:?}", element.id) };
    let [first, second] = satin.rails;
    first.iter().chain(second.iter().rev()).chain(first.first()).copied().collect()
}

/// The rings of `part`, outline and holes, each closed.
fn rings(part: &Polygon) -> Vec<Vec<Point>> {
    std::iter::once(&part.outline).chain(&part.holes).map(|ring| ring.iter().chain(ring.first()).copied().collect()).collect()
}

/// Points every [`STEP`] along the polylines `lines`, their corners among them.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // a saturating cast of a count of steps
fn along(lines: &[Vec<Point>]) -> Vec<Point> {
    let mut points = Vec::new();
    for line in lines {
        for pair in line.windows(2) {
            let (a, b) = (pair[0], pair[1]);
            let steps = (a.distance(b) / STEP).ceil().max(1.0) as usize;
            points.extend((0..steps).map(|i| {
                let t = i as f64 / steps as f64;
                p(a.x() + t * (b.x() - a.x()), a.y() + t * (b.y() - a.y()))
            }));
        }
        points.extend(line.last());
    }
    points
}

/// The distance from `point` to the nearest of `lines`' segments.
fn distance(point: Point, lines: &[Vec<Point>]) -> f64 {
    lines
        .iter()
        .flat_map(|line| line.windows(2).map(|pair| distance_to_segment(point, pair[0], pair[1])).chain(line.first().map(|q| q.distance(point))))
        .fold(f64::INFINITY, f64::min)
}

/// The points of the outlines of the parts that `out`, an automatic shape digitized, sews along the middle of
/// that lie farther from what sews them than [`LINE_REACH`] and [`COLUMN_REACH`] allow, each with how far.
/// A narrow part's width is twice the farthest its line lies from its outline, at a junction more.
fn unreached(out: &Digitized) -> Vec<(Point, f64)> {
    // Each part, with the elements that sew it.
    let mut parts: Vec<(&Polygon, Vec<&Element>)> = Vec::new();
    for (element, part) in out.elements.iter().zip(&out.parts) {
        let Some(part) = part else { continue };
        match parts.iter_mut().find(|(known, _)| *known == part) {
            Some((_, elements)) => elements.push(element),
            None => parts.push((part, vec![element])),
        }
    }
    let mut far = Vec::new();
    for (part, elements) in parts {
        let outline = rings(part);
        let (columns, lines): (Vec<&Element>, Vec<&Element>) = elements.into_iter().partition(|e| setting(e, SATIN_COLUMN) == Some("true"));
        let columns: Vec<Vec<Point>> = columns.into_iter().map(column).collect();
        // Each line, with how far a point of the outline may lie from it.
        let lines: Vec<(Vec<Vec<Point>>, f64)> = lines
            .into_iter()
            .map(polylines)
            .map(|line| {
                let width = 2.0 * line.iter().flatten().map(|&q| distance(q, &outline)).fold(0.0, f64::max);
                (line, LINE_REACH * width)
            })
            .collect();
        for point in along(&outline) {
            let to_columns = distance(point, &columns);
            let reached = to_columns <= COLUMN_REACH || lines.iter().any(|(line, reach)| distance(point, line) <= *reach);
            if !reached {
                let to_lines = lines.iter().map(|(line, _)| distance(point, line)).fold(f64::INFINITY, f64::min);
                far.push((point, to_columns.min(to_lines)));
            }
        }
    }
    far
}

/// The points that the automatic `element`, digitized in a design with `settings`, leaves too far, as
/// [`unreached`] measures them.
fn unreached_by(element: &Element, settings: &DesignSettings) -> Vec<(Point, f64)> {
    unreached(&digitize(&automatic(element.clone()), settings, &mut Budget::DEFAULT.meter()).unwrap())
}

/// The outline of the letter `letter`, T, H or E, `height` mm tall and 0.6 of that wide, drawn in strokes
/// `width` mm wide from the origin.
fn letter(letter: char, height: f64, width: f64) -> Vec<(f64, f64)> {
    let (h, w, wide) = (height, width, 0.6 * height);
    let (above, below) = (h / 2.0 - w / 2.0, h / 2.0 + w / 2.0);
    match letter {
        'T' => {
            let (left, right) = (wide / 2.0 - w / 2.0, wide / 2.0 + w / 2.0);
            vec![(0.0, 0.0), (wide, 0.0), (wide, w), (right, w), (right, h), (left, h), (left, w), (0.0, w)]
        }
        'H' => vec![
            (0.0, 0.0),
            (w, 0.0),
            (w, above),
            (wide - w, above),
            (wide - w, 0.0),
            (wide, 0.0),
            (wide, h),
            (wide - w, h),
            (wide - w, below),
            (w, below),
            (w, h),
            (0.0, h),
        ],
        'E' => {
            let middle = 0.8 * wide;
            vec![
                (0.0, 0.0),
                (wide, 0.0),
                (wide, w),
                (w, w),
                (w, above),
                (middle, above),
                (middle, below),
                (w, below),
                (w, h - w),
                (wide, h - w),
                (wide, h),
                (0.0, h),
            ]
        }
        other => panic!("no letter {other}"),
    }
}

#[test]
fn req_auto_014_a_t_reaches_the_ends_of_both_arms_of_its_bar() {
    // The Ts that lost an arm: TS-13B's, 4 mm tall, its bar 3.2 mm long, in lines 0.72 mm wide, and Ts 3, 4.8
    // and 5 mm tall, their bars 0.6 of that, in lines 0.5, 0.8 and 0.8 mm wide (`REQ-AUTO-014`).
    let small = vec![(0.0, 0.0), (3.2, 0.0), (3.2, 0.72), (1.96, 0.72), (1.96, 4.0), (1.24, 4.0), (1.24, 0.72), (0.0, 0.72)];
    for t in [small, letter('T', 3.0, 0.5), letter('T', 4.8, 0.8), letter('T', 5.0, 0.8)] {
        assert_eq!(unreached_by(&fill("t", &[&t], &RED, &[]), &DesignSettings::default()), [], "{t:?}");
    }
}

#[test]
fn req_auto_014_ts13_every_part_sewn_along_its_middle_is_reached() {
    for id in ["TS-13A", "TS-13B", "TS-13C"] {
        let design = find(id).and_then(|sheet| sheet.design()).unwrap().unwrap();
        for element in design.elements().iter().filter(|element| element.automatic) {
            let far = unreached_by(element, &design.settings);
            assert!(
                far.is_empty(),
                "{id} {}: {} points of its outline too far, the farthest {:?}",
                element.id,
                far.len(),
                far.iter().max_by(|a, b| a.1.total_cmp(&b.1))
            );
        }
    }
}

/// A letter, its height and its strokes' width in millimetres, drawn in tenths: narrow lines up to 0.9 mm,
/// and columns from 1.1 mm, no wider than a fifth of the letter's height, so that it reads as its letter.
fn sizes() -> impl Strategy<Value = (char, f64, f64)> {
    (prop::sample::select(vec!['T', 'H', 'E']), 30u32..=200)
        .prop_flat_map(|(letter, height)| {
            let widest = (height / 5).min(70);
            let widths = prop_oneof![3u32..=9, 11u32..=widest.max(11)];
            (Just(letter), Just(height), widths)
        })
        .prop_filter("a letter's strokes no wider than a fifth of its height", |(_, height, width)| width * 5 <= *height)
        .prop_map(|(letter, height, width)| (letter, f64::from(height) / 10.0, f64::from(width) / 10.0))
}

proptest! {
    #![proptest_config(stitchcraft_testkit::strategies::config(256))]

    #[test]
    fn req_auto_014_req_auto_016_random_letters_are_reached_everywhere((name, height, width) in sizes()) {
        let shape = letter(name, height, width);
        let element = fill("letter", &[&shape], &RED, &[]);
        let far = unreached_by(&element, &DesignSettings::default());
        prop_assert!(far.is_empty(), "{name} {height} mm tall, {width} mm wide: {} points too far, the farthest {:?}", far.len(), far.iter().max_by(|a, b| a.1.total_cmp(&b.1)));
    }
}
