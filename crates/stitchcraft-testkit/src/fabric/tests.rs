use std::f64::consts::PI;

use stitchcraft_core::ElementId;
use stitchcraft_engine::design::{Cap, DesignSettings};
use stitchcraft_plan::{ColorBlock, ElementRef, Provenance, StitchKind, Thread};

use super::*;
use crate::designs::{BLUE, RED, automatic, fill, line, p, widened};

/// A run of needle points, each sewn, or `None` for a jump to the next.
type Run = Vec<Option<(f64, f64)>>;

/// A plan of `blocks`, each a thread and its run.
fn plan(blocks: &[(&Thread, Run)]) -> StitchPlan {
    let origin = Provenance::new(ElementRef::from_index(0).unwrap(), Role::Top);
    let blocks = blocks
        .iter()
        .map(|(thread, points)| {
            let mut stitches = Vec::new();
            let mut jump = false;
            for point in points {
                match point {
                    None => jump = true,
                    Some((x, y)) => {
                        let kind = if std::mem::take(&mut jump) { StitchKind::Jump } else { StitchKind::Normal };
                        stitches.push(stitchcraft_plan::Stitch { at: p(*x, *y), kind, origin });
                        if kind == StitchKind::Jump {
                            stitches.push(stitchcraft_plan::Stitch { at: p(*x, *y), kind: StitchKind::Normal, origin });
                        }
                    }
                }
            }
            ColorBlock { thread: (*thread).clone(), stitches }
        })
        .collect();
    StitchPlan { blocks, elements: vec![ElementId::new("a").unwrap()] }
}

/// Rows 0.25 mm apart from y = `top` to `bottom`, each from x = `left` to `right` and back the next, as the
/// needle points of one run.
fn rows(left: f64, right: f64, top: f64, bottom: f64) -> Run {
    let count = crate::metrics::index(((bottom - top) / 0.25).round());
    (0..=count)
        .flat_map(|k| {
            let y = top + 0.25 * k as f64;
            if k % 2 == 0 { [Some((left, y)), Some((right, y))] } else { [Some((right, y)), Some((left, y))] }
        })
        .collect()
}

/// A design of `elements`, its origin where the coordinates are.
fn design(elements: Vec<Element>) -> Design {
    Design::new(elements, DesignSettings { origin: Some(p(0.0, 0.0)), ..DesignSettings::default() }).unwrap()
}

/// A square `side` millimetres wide from (`x`, `y`), a fill sewn in `thread`.
fn square(id: &str, (x, y): (f64, f64), side: f64, thread: &Thread) -> Element {
    fill(id, &[&[(x, y), (x + side, y), (x + side, y + side), (x, y + side)]], thread, &[])
}

#[test]
fn rows_that_cover_a_square_leave_no_fabric_and_a_gap_in_them_leaves_a_patch() {
    let square = design(vec![square("square", (0.0, 0.0), 10.0, &RED)]);
    let whole = unsewn(&square, &plan(&[(&RED, rows(0.0, 10.0, 0.0, 10.0))]));
    assert_eq!((whole.largest, whole.patches, whole.share, whole.at), (0.0, 0, 0.0, None));
    // The rows from y = 4.25 to 5.75 jump from x = 3 to x = 7: the thread leaves 3.6 by 1.6 mm bare, its
    // corners rounded by the disc a thread across.
    let mut gapped = Vec::new();
    for pair in rows(0.0, 10.0, 0.0, 10.0).chunks(2) {
        let (Some(a), Some(b)) = (pair[0], pair[1]) else { continue };
        if (4.2..5.8).contains(&a.1) {
            let (from, to) = if a.0 < b.0 { (3.0, 7.0) } else { (7.0, 3.0) };
            gapped.extend([Some(a), Some((from, a.1)), None, Some((to, a.1)), Some(b)]);
        } else {
            gapped.extend([Some(a), Some(b)]);
        }
    }
    let gap = unsewn(&square, &plan(&[(&RED, gapped)]));
    assert_eq!(gap.patches, 1);
    assert!((gap.largest - (3.6 * 1.6 - (4.0 - PI) * 0.04)).abs() < 0.15, "{gap:?}");
    assert!((gap.share - gap.largest / 100.0).abs() < 0.002, "{gap:?}");
    let at = gap.at.unwrap();
    assert!((at.min().x() - 3.2).abs() < 0.06 && (at.max().y() - 5.8).abs() < 0.06, "{at:?}");
    // Nothing sewn, the whole square is one patch.
    let bare = unsewn(&square, &plan(&[(&RED, Vec::new())]));
    assert!((bare.largest - 100.0).abs() < 0.5 && bare.share > 0.99, "{bare:?}");
}

#[test]
fn a_sliver_narrower_than_a_thread_beside_a_running_stitch_is_no_patch() {
    // A line 0.9 mm wide with a round cap, run along its middle: 0.25 mm of it is bare either side, and the
    // caps beyond its ends, narrower than a thread.
    let running = plan(&[(&RED, vec![Some((0.0, 0.0)), Some((10.0, 0.0))])]);
    let narrow = design(vec![widened(line("line", (0.0, 0.0), 10.0, &RED, &[]), 0.9)]);
    assert_eq!(unsewn(&narrow, &running).patches, 0);
    // 1.5 mm wide, 0.55 mm either side is bare: a patch along each side.
    let wide = design(vec![widened(line("line", (0.0, 0.0), 10.0, &RED, &[]), 1.5)]);
    let sides = unsewn(&wide, &running);
    assert_eq!(sides.patches, 2, "{sides:?}");
    assert!((sides.largest - 0.55 * 10.0).abs() < 0.6, "{sides:?}");
}

#[test]
fn a_line_paints_its_caps_and_corners_as_drawn() {
    // Nothing sewn: the line's whole area is bare, a band 1 mm wide and 10 mm long, past its ends as its cap
    // reaches, a half disc each with a round cap and half a square with a square one.
    let bare = |cap: Cap| {
        let element = widened(line("line", (0.0, 0.0), 10.0, &RED, &[]), 1.0);
        let Shape::Stroke { path, width, join, .. } = element.shape else { panic!() };
        unsewn(&design(vec![Element { shape: Shape::Stroke { path, width, join, cap }, ..element }]), &plan(&[(&RED, Vec::new())])).largest
    };
    // The disc a thread across rounds a butt or a square end's corners, (4 - π) 0.2² mm² each.
    let corners = 4.0 * (4.0 - PI) * 0.04;
    assert!((bare(Cap::Butt) - (10.0 - corners)).abs() < 0.1, "{}", bare(Cap::Butt));
    assert!((bare(Cap::Round) - (10.0 + PI / 4.0)).abs() < 0.1, "{}", bare(Cap::Round));
    assert!((bare(Cap::Square) - (11.0 - corners)).abs() < 0.1, "{}", bare(Cap::Square));
    // A path that turns a corner is round there, and a closed one at each corner.
    let corner = Element {
        shape: Shape::Stroke {
            path: crate::designs::polylines(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]]),
            width: stitchcraft_core::Mm::new(1.0).unwrap(),
            join: Join::Round,
            cap: Cap::Butt,
        },
        ..line("corner", (0.0, 0.0), 1.0, &RED, &[])
    };
    // 2 bands 10 mm by 1 mm that share a square of 0.5 by 0.5 mm, and the quarter of the round corner's disc
    // that lies outside both.
    let turned = unsewn(&design(vec![corner.clone()]), &plan(&[(&RED, Vec::new())])).largest;
    assert!((turned - (20.0 - 0.25 + PI / 16.0 - corners)).abs() < 0.15, "{turned}");
    // Round a rectangle 10 mm by 6 mm: a band 1 mm wide, its outer corners round and its inner ones square,
    // the rectangle grown by half a millimetre less the rectangle shrunk by as much.
    let Shape::Stroke { width, join, cap, .. } = corner.shape else { panic!() };
    let rectangle = crate::designs::rings(&[&[(0.0, 0.0), (10.0, 0.0), (10.0, 6.0), (0.0, 6.0)]]);
    let ring = Element { shape: Shape::Stroke { path: rectangle, width, join, cap }, ..corner };
    let round = unsewn(&design(vec![ring]), &plan(&[(&RED, Vec::new())])).largest;
    let band = (60.0 + 32.0 * 0.5 + PI * 0.25) - 9.0 * 5.0;
    assert!((round - band).abs() < 0.15, "{round} {band}");
}

#[test]
fn the_background_is_bare_fabric_but_for_what_shows_of_it_inside_the_picture() {
    // A blue square 40 mm wide under a red ring round (20, 20), 20 mm across round a hole 10 mm across, all
    // automatic: the square is the picture's background, and the ring's hole shows it inside the picture.
    let disc = |r: f64| {
        (0..72)
            .map(|k| {
                (20.0 + r * stitchcraft_core::math::cos(f64::from(k) * PI / 36.0), 20.0 + r * stitchcraft_core::math::sin(f64::from(k) * PI / 36.0))
            })
            .collect::<Vec<_>>()
    };
    let (outer, mut inner) = (disc(10.0), disc(5.0));
    inner.reverse();
    let ring = automatic(fill("ring", &[&outer, &inner], &RED, &[]));
    let picture = design(vec![automatic(square("backdrop", (0.0, 0.0), 40.0, &BLUE)), ring.clone()]);
    // Nothing sewn: the ring and its hole are bare, but not the backdrop round the ring.
    let bare = unsewn(&picture, &plan(&[(&RED, Vec::new())]));
    assert!((bare.largest - PI * 100.0).abs() < 2.0, "{bare:?}");
    // Without the backdrop, the hole shows no colour.
    let alone = unsewn(&design(vec![ring.clone()]), &plan(&[(&RED, Vec::new())]));
    assert!((alone.largest - PI * 75.0).abs() < 2.0, "{alone:?}");
    // Sewn with settings, the backdrop is no background: all of it is bare.
    let drawn = design(vec![square("backdrop", (0.0, 0.0), 40.0, &BLUE), Element { automatic: false, ..ring }]);
    let all = unsewn(&drawn, &plan(&[(&RED, Vec::new())]));
    assert!((all.largest - 1600.0).abs() < 2.0, "{all:?}");
}

#[test]
fn a_stitch_that_shows_on_another_colour_is_astray() {
    // A red square and a blue one beside it, each sewn in rows.
    let squares = design(vec![square("red", (0.0, 0.0), 10.0, &RED), square("blue", (10.0, 0.0), 10.0, &BLUE)]);
    let (red, blue) = (rows(0.0, 10.0, 0.125, 9.875), rows(10.0, 20.0, 0.125, 9.875));
    // The rows only step along the edge where they meet, a quarter of a millimetre at a time.
    let neat = astray(&squares, &plan(&[(&RED, red.clone()), (&BLUE, blue.clone())]));
    assert!(neat.stitches.is_empty() && neat.longest <= 0.3 && neat.jumps.is_empty(), "{neat:?}");
    // A red running stitch across the blue square after it is sewn shows on it for 5 mm, and a red stitch
    // 0.45 mm into it, as a satin column's reaches past its rail, doesn't count.
    let mut over = red;
    over.extend([None, Some((9.0, 5.1)), Some((10.45, 5.1)), None, Some((5.0, 5.1)), Some((15.0, 5.1))]);
    let travel = astray(&squares, &plan(&[(&BLUE, blue.clone()), (&RED, over.clone())]));
    assert_eq!(travel.stitches, [[p(5.0, 5.1), p(15.0, 5.1)]]);
    assert!((travel.longest - 5.0).abs() < 0.06, "{travel:?}");
    // The jumps to its stitches leave threads on a machine that cuts none.
    assert_eq!(travel.jumps.len(), 2);
    // Sewn before the blue rows, the blue covers it.
    let hidden = astray(&squares, &plan(&[(&RED, over), (&BLUE, blue)]));
    assert!(hidden.stitches.is_empty() && hidden.longest < 0.6, "{hidden:?}");
    // Past the squares, on bare fabric, it shows too: 2 mm of it is astray, and half a millimetre isn't.
    let out = |to: f64| astray(&squares, &plan(&[(&RED, vec![Some((5.0, 5.0)), Some((to, 5.0))])])).stitches.len();
    assert_eq!((out(-2.0), out(-0.5)), (1, 0));
}

/// The needle points `points` of the element at place `element` in the plan, sewn one after another in the
/// role `role`.
fn sewn(points: &[(f64, f64)], element: usize, role: Role) -> Vec<stitchcraft_plan::Stitch> {
    let origin = Provenance::new(ElementRef::from_index(element).unwrap(), role);
    points.iter().map(|&(x, y)| stitchcraft_plan::Stitch { at: p(x, y), kind: StitchKind::Normal, origin }).collect()
}

/// A plan of `blocks`, each a thread and its stitches, of elements with the ids `ids`.
fn blocks(blocks: Vec<(&Thread, Vec<stitchcraft_plan::Stitch>)>, ids: &[&str]) -> StitchPlan {
    let blocks = blocks.into_iter().map(|(thread, stitches)| ColorBlock { thread: thread.clone(), stitches }).collect();
    StitchPlan { blocks, elements: ids.iter().map(|id| ElementId::new(*id).unwrap()).collect() }
}

/// Rows across the square `side` millimetres wide from x = `left`, y = 0, inset by half a row, as needle
/// points.
fn across(left: f64, side: f64) -> Vec<(f64, f64)> {
    rows(left, left + side, 0.125, side - 0.125).into_iter().flatten().collect()
}

#[test]
fn a_move_from_one_element_to_the_next_is_counted_apart_from_their_stitches() {
    // Red squares either side of a blue one, the blue sewn first. The red thread sews the left square's rows,
    // then on to the right square's, across the blue: a move.
    let squares =
        design(vec![square("left", (0.0, 0.0), 10.0, &RED), square("middle", (10.0, 0.0), 10.0, &BLUE), square("right", (20.0, 0.0), 10.0, &RED)]);
    let red = [sewn(&across(0.0, 10.0), 0, Role::Top), sewn(&across(20.0, 10.0), 2, Role::Top)].concat();
    let moved = astray(&squares, &blocks(vec![(&BLUE, sewn(&across(10.0, 10.0), 1, Role::Top)), (&RED, red)], &["left", "middle", "right"]));
    assert_eq!(moved.moves, [[p(0.0, 9.875), p(20.0, 0.125)]]);
    assert!(moved.stitches.is_empty() && moved.longest <= 0.3, "{moved:?}");
}

#[test]
fn an_underlay_stitch_is_astray_wherever_it_shows() {
    // A red square's rows, and an underlay row across its middle: sewn after the rows it shows, on its own
    // colour, and so does the stitch to it.
    let red = design(vec![square("red", (0.0, 0.0), 10.0, &RED)]);
    let (top, under) = (sewn(&across(0.0, 10.0), 0, Role::Top), sewn(&[(1.0, 5.0), (9.0, 5.0)], 0, Role::Underlay));
    let shown = astray(&red, &blocks(vec![(&RED, [top.clone(), under.clone()].concat())], &["red"]));
    assert_eq!(shown.stitches, [[p(0.0, 9.875), p(1.0, 5.0)], [p(1.0, 5.0), p(9.0, 5.0)]]);
    // Sewn before them, the rows cover it.
    let hidden = astray(&red, &blocks(vec![(&RED, [under, top].concat())], &["red"]));
    assert!(hidden.stitches.is_empty() && hidden.moves.is_empty(), "{hidden:?}");
}
