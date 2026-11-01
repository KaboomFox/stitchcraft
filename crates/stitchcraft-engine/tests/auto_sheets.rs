//! MC-8's checks of the TS-13 sheets that their stitches answer (`docs/src/plan/machine-testing.md` ›
//! TS-13): where the digitized half's stitches lie against its drawing, what sews before what, and which
//! jump threads later stitches sew down. Each case measures the sheet's own plan, its stitches moved to
//! where the sheet draws its shapes (`stitchcraft_testkit::pictures::sheet`), so what it measures is what the
//! machine sews. What only the machine shows stays for the sew-out: how far the thread pulls in, whether
//! fabric or a ridge shows, the smallest letters that read, and the time.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;
use std::sync::OnceLock;

use stitchcraft_core::{Budget, Point, Rect, math};
use stitchcraft_engine::design::{Design, Element, FillRule, Path, Shape};
use stitchcraft_engine::normalize::region;
use stitchcraft_engine::normalize::stroke::distance_to_segment;
use stitchcraft_engine::testsheets::find;
use stitchcraft_plan::{StitchKind, StitchPlan};
use stitchcraft_testkit::designs::{p, rules};
use stitchcraft_testkit::metrics::{coverage, edge_coverage};
use stitchcraft_testkit::pictures;
use stitchcraft_testkit::stitches::{jump_threads, rungs, top_runs};

/// The TS-13 sheet `id`, as `pictures::sheet` plans it, planned once for every case.
fn sheet(id: &str) -> &'static (Design, StitchPlan) {
    static SHEETS: [OnceLock<(Design, StitchPlan)>; 3] = [OnceLock::new(), OnceLock::new(), OnceLock::new()];
    let index = ["TS-13A", "TS-13B", "TS-13C"].iter().position(|&sheet| sheet == id).unwrap_or_else(|| panic!("{id} is no TS-13 sheet"));
    SHEETS[index].get_or_init(|| pictures::sheet(id))
}

/// The element `id` of `design`.
fn drawn<'d>(design: &'d Design, id: &str) -> &'d Element {
    design.elements().iter().find(|element| element.id.as_str() == id).unwrap_or_else(|| panic!("no {id}"))
}

/// The points that define `element`'s path, in order.
fn corners(element: &Element) -> Vec<Point> {
    element.shape.path().points().collect()
}

/// The box round the points that define each subpath of `element`.
fn boxes(element: &Element) -> Vec<Rect> {
    element.shape.path().subpaths.iter().map(|subpath| Rect::around(Path { subpaths: vec![subpath.clone()] }.points()).unwrap()).collect()
}

/// The middle of `rect`.
fn middle(rect: Rect) -> Point {
    p((rect.min().x() + rect.max().x()) / 2.0, (rect.min().y() + rect.max().y()) / 2.0)
}

/// The needle points of the top stitches of the elements of `plan` whose ids start with `prefix`.
fn needles(plan: &StitchPlan, prefix: &str) -> Vec<Point> {
    top_runs(plan).into_iter().filter(|run| run.element.starts_with(prefix)).flat_map(|run| run.needles).collect()
}

/// The angle between the directions `a` and `b`, either way along them, in degrees: 0 to 90.
fn angle(a: (f64, f64), b: (f64, f64)) -> f64 {
    let cos = (a.0 * b.0 + a.1 * b.1).abs() / (math::hypot(a.0, a.1) * math::hypot(b.0, b.1));
    math::acos(cos.min(1.0)).to_degrees()
}

/// From `from` to `to`.
fn towards(from: Point, to: Point) -> (f64, f64) {
    (to.x() - from.x(), to.y() - from.y())
}

#[test]
fn ts13_sheets_sew_where_they_are_drawn_the_stitches_of_their_machine_files() {
    // The plan the cases measure is the sheet's own, every entry moved by the same amount.
    for id in ["TS-13A", "TS-13B", "TS-13C"] {
        let entries =
            |plan: &StitchPlan| -> Vec<(StitchKind, Point)> { plan.blocks.iter().flat_map(|b| &b.stitches).map(|s| (s.kind, s.at)).collect() };
        let (filed, drawn) = (entries(&find(id).unwrap().plan().unwrap()), entries(&sheet(id).1));
        assert_eq!(filed.len(), drawn.len(), "{id}");
        let moved = towards(filed[0].1, drawn[0].1);
        for ((kind, at), (drawn_kind, drawn_at)) in filed.iter().zip(&drawn) {
            assert_eq!(kind, drawn_kind, "{id}");
            let by = towards(*at, *drawn_at);
            assert!((by.0 - moved.0).abs() < 1e-9 && (by.1 - moved.1).abs() < 1e-9, "{id}: {at:?} to {drawn_at:?}");
        }
    }
}

#[test]
fn req_auto_007_ts13a_where_the_bar_runs_on_apart_from_the_square_the_fabric_between_stays_open() {
    // The bar meets the square's side along its top half and runs on beside it 0.5 mm away below it. There,
    // digitized, the square's rows end at its edge and the bar's at its own: neither sews into the gap.
    let (design, plan) = sheet("TS-13A");
    let bar = corners(drawn(design, "ts13a:auto:parted:bar"));
    let (side, gap, half, bottom) = (bar[0].x(), bar[4].x(), bar[4].y(), bar[3].y());
    assert!((gap - side - 0.5).abs() < 1e-9, "{bar:?}");
    // Below where they meet, clear of the corner by a row or 2.
    let beside = |prefix: &str| -> Vec<f64> {
        needles(plan, prefix).into_iter().filter(|q| q.y() > half + 0.5 && q.y() < bottom - 0.5).map(|q| q.x()).collect()
    };
    let square = beside("ts13a:auto:parted:square").into_iter().fold(f64::NEG_INFINITY, f64::max);
    let bar = beside("ts13a:auto:parted:bar").into_iter().fold(f64::INFINITY, f64::min);
    assert!((square - side).abs() < 1e-6 && (bar - gap).abs() < 1e-6, "the square's rows end at {square}, the bar's at {bar}");
}

#[test]
fn req_auto_008_ts13a_the_specks_on_the_disc_sew_as_the_disc_and_the_one_apart_sews_nothing() {
    let (design, plan) = sheet("TS-13A");
    // Digitized, no speck sews as one: the 6 on the disc's edge went into it, and the seventh, 2 mm from it,
    // is left out (`SC-I0905`).
    assert!(top_runs(plan).iter().all(|run| !run.element.starts_with("ts13a:auto:specks:specks")));
    let specks = drawn(design, "ts13a:auto:specks:specks").shape.path().subpaths.clone();
    // The disc's stitches cover where the 6 were.
    let merged = Path { subpaths: specks[..6].to_vec() };
    let covered = coverage(&region::build(&merged, FillRule::NonZero, &mut Budget::DEFAULT.meter()).unwrap().region, plan);
    assert!(covered > 0.95, "{covered}");
    // No stitch comes within 1 mm of the seventh's middle.
    let seventh = middle(Rect::around(Path { subpaths: vec![specks[6].clone()] }.points()).unwrap());
    let nearest = plan.sewn_stitches().iter().map(|s| distance_to_segment(seventh, s.from, s.to)).fold(f64::INFINITY, f64::min);
    assert!(nearest > 1.0, "{nearest}");
}

/// The first and last of `plan`'s stitches, in sewing order, of each shape: an element and the elements
/// named after it with `#2`, `#3` and so on.
fn sewn_from_to(plan: &StitchPlan) -> BTreeMap<String, (usize, usize)> {
    let mut found: BTreeMap<String, (usize, usize)> = BTreeMap::new();
    let stitches = plan.blocks.iter().flat_map(|block| &block.stitches).filter(|s| s.kind == StitchKind::Normal);
    for (at, stitch) in stitches.enumerate() {
        let Some(element) = stitch.origin.element else { continue };
        let id = plan.elements[element.index()].as_str();
        let shape = id.split_once('#').map_or(id, |(shape, _)| shape).to_string();
        found.entry(shape).and_modify(|(_, last)| *last = at).or_insert((at, at));
    }
    found
}

#[test]
fn req_auto_010_ts13_no_shape_sews_over_one_drawn_over_it() {
    // Of 2 shapes whose paint overlaps, the lower sews first, or it would cover the upper one. In the stacked
    // badge, the blue square lies on the red disc and the gold disc on the square. On the face, every line
    // lies on the pink face, the eyes' lines and pupils on the white eyes, each brow on its eye's line, and
    // the hairs on the outline.
    for (id, picture, over) in [
        ("TS-13A", "ts13a:auto:badge-stacked", &[("red", "blue"), ("blue", "gold")][..]),
        (
            "TS-13C",
            "ts13c:auto:face",
            &[
                ("face", "eyes"),
                ("face", "outline"),
                ("face", "eye-lines"),
                ("face", "pupils"),
                ("face", "brows"),
                ("face", "smile"),
                ("face", "hair"),
                ("eyes", "eye-lines"),
                ("eyes", "pupils"),
                ("eye-lines", "brows"),
                ("outline", "hair"),
            ][..],
        ),
    ] {
        let sewn = sewn_from_to(&sheet(id).1);
        for (lower, upper) in over {
            let (under, on) = (sewn[&format!("{picture}:{lower}")], sewn[&format!("{picture}:{upper}")]);
            assert!(under.1 < on.0, "{id}: {picture}:{upper}, sewn from stitch {}, before {lower} ends, at {}", on.0, under.1);
        }
    }
}

#[test]
fn req_auto_014_ts13b_a_line_traced_as_an_area_runs_to_within_its_width_of_its_ends_and_its_branch_to_the_line() {
    // Each window's line runs down it from top to bottom, with a branch across to its right side from
    // halfway down. The line along its middle ends at the middle of the last chord its end's cap grew
    // across: inside the line, and no further from its end than the line is wide.
    let (design, plan) = sheet("TS-13B");
    for width in ["0.3", "0.6", "0.9"] {
        let line = corners(drawn(design, &format!("ts13b:auto:window-{width}:line")));
        let w: f64 = width.parse().unwrap();
        let (down, across, top, right, bottom) =
            ((line[0].x() + line[1].x()) / 2.0, (line[2].y() + line[5].y()) / 2.0, line[0].y(), line[3].x(), line[6].y());
        let needles = needles(plan, &format!("ts13b:auto:window-{width}:line"));
        let upright: Vec<Point> = needles.iter().copied().filter(|q| (q.x() - down).abs() <= w / 2.0).collect();
        let level: Vec<Point> = needles.iter().copied().filter(|q| (q.y() - across).abs() <= w / 2.0).collect();
        // Every needle point lies on the line's middle, down it or across.
        assert_eq!(upright.len() + level.len(), needles.len() + upright.iter().filter(|q| level.contains(q)).count(), "{width}");
        let ys = upright.iter().map(|q| q.y());
        let (first, last) = (ys.clone().fold(f64::INFINITY, f64::min), ys.fold(f64::NEG_INFINITY, f64::max));
        let xs = level.iter().map(|q| q.x());
        let (start, farthest) = (xs.clone().fold(f64::INFINITY, f64::min), xs.fold(f64::NEG_INFINITY, f64::max));
        for (end, short) in [("top", first - top), ("bottom", bottom - last), ("right side", right - farthest)] {
            assert!(short > 0.0 && short <= w, "{width} mm: {short} mm short of its {end}");
        }
        // The branch's line starts where it meets the line down the window, within half its width of its middle.
        assert!(start - down <= w / 2.0, "{width} mm: the branch starts {} mm from the middle", start - down);
        // No knot where they meet: within 1 mm of it, no more than 3 times the needle points 1 mm round a
        // point 5 mm up the line.
        let within = |at: Point| needles.iter().filter(|q| math::hypot(q.x() - at.x(), q.y() - at.y()) <= 1.0).count();
        let (junction, along) = (within(p(down, across)), within(p(down, across - 5.0)));
        assert!(junction <= 3 * along, "{width} mm: {junction} needle points where they meet, {along} along the line");
    }
}

#[test]
fn req_auto_003_ts13c_the_smiles_column_ends_square_where_its_round_caps_end() {
    // The smile is 2 mm wide with round caps: its satin column reaches past each end of its line by half
    // its width, as far as the cap does.
    let (design, plan) = sheet("TS-13C");
    let smile = drawn(design, "ts13c:auto:face:smile");
    let Shape::Stroke { width, .. } = smile.shape else { panic!("the smile is a line") };
    let line = corners(smile);
    let needles = needles(plan, "ts13c:auto:face:smile");
    for (end, next) in [(line[0], line[1]), (line[line.len() - 1], line[line.len() - 2])] {
        let out = towards(next, end);
        let length = math::hypot(out.0, out.1);
        let past = needles.iter().map(|q| (towards(end, *q).0 * out.0 + towards(end, *q).1 * out.1) / length).fold(f64::NEG_INFINITY, f64::max);
        assert!((past - width.get() / 2.0).abs() < 0.01, "{past} mm past its end at {end:?}");
    }
}

/// The centre of the circle through `a`, `b` and `c`.
fn circumcentre(a: Point, b: Point, c: Point) -> Point {
    let d = 2.0 * (a.x() * (b.y() - c.y()) + b.x() * (c.y() - a.y()) + c.x() * (a.y() - b.y()));
    let square = |q: Point| q.x() * q.x() + q.y() * q.y();
    let x = (square(a) * (b.y() - c.y()) + square(b) * (c.y() - a.y()) + square(c) * (a.y() - b.y())) / d;
    let y = (square(a) * (c.x() - b.x()) + square(b) * (a.x() - c.x()) + square(c) * (b.x() - a.x())) / d;
    p(x, y)
}

/// The rungs of the columns of `plan` whose ids start with `prefix`, and how far each lies from square to
/// its stroke, in degrees: from the direction `square` gives at its middle; `None` leaves a rung out.
fn off_square(plan: &StitchPlan, prefix: &str, square: impl Fn(Point) -> Option<(f64, f64)>) -> Vec<(Point, f64)> {
    let runs = top_runs(plan).into_iter().filter(|run| run.element.starts_with(prefix));
    let rungs: Vec<[Point; 2]> = runs.flat_map(|run| rungs(&run.needles)).collect();
    assert!(!rungs.is_empty(), "{prefix} has no rungs");
    rungs
        .into_iter()
        .filter_map(|[a, b]| {
            let at = p((a.x() + b.x()) / 2.0, (a.y() + b.y()) / 2.0);
            square(at).map(|direction| (at, angle(towards(a, b), direction)))
        })
        .collect()
}

#[test]
fn req_auto_017_ts13_satin_columns_lie_square_to_their_strokes() {
    // Round a ring or a bend, a column's rungs point at the middle its stroke turns round; across a star's
    // point, square to the line from the star's middle to its tip. Each lies within 10° of that, along a
    // point from its tip to its cut too.
    let mut measured: Vec<(String, Vec<(Point, f64)>)> = Vec::new();
    let round = |middle: Point| move |at: Point| Some(towards(middle, at));
    let (design, plan) = sheet("TS-13C");
    let face = middle(boxes(drawn(design, "ts13c:auto:face:face"))[0]);
    let eyes: Vec<Point> = boxes(drawn(design, "ts13c:auto:face:eyes")).into_iter().map(middle).collect();
    let nearer_eye = |at: Point| {
        eyes.iter()
            .copied()
            .min_by(|a, b| math::hypot(a.x() - at.x(), a.y() - at.y()).total_cmp(&math::hypot(b.x() - at.x(), b.y() - at.y())))
            .unwrap()
    };
    let smile = corners(drawn(design, "ts13c:auto:face:smile"));
    let smiles_middle = circumcentre(smile[0], smile[smile.len() / 2], smile[smile.len() - 1]);
    measured.push(("the face's outline".into(), off_square(plan, "ts13c:auto:face:outline", round(face))));
    measured.push(("the eyes' lines".into(), off_square(plan, "ts13c:auto:face:eye-lines", |at| Some(towards(nearer_eye(at), at)))));
    measured.push(("the smile".into(), off_square(plan, "ts13c:auto:face:smile", round(smiles_middle))));
    let star = corners(drawn(design, "ts13c:auto:star:star"));
    let star_middle = p(star.iter().map(|q| q.x()).sum::<f64>() / star.len() as f64, star.iter().map(|q| q.y()).sum::<f64>() / star.len() as f64);
    let across = move |at: Point| Some((-(at.y() - star_middle.y()), at.x() - star_middle.x()));
    // The star's middle is a fill, and its points the columns named after it.
    measured.push(("the star's points".into(), off_square(plan, "ts13c:auto:star:star#", across)));
    // The C's bend, left of its middle, away from its round ends.
    let (design, plan) = sheet("TS-13B");
    for height in ["15", "10", "6"] {
        let c = boxes(drawn(design, &format!("ts13b:auto:word-{height}:c")))[0];
        let size = c.max().y() - c.min().y();
        let bend = p(c.min().x() + size / 2.0, (c.min().y() + c.max().y()) / 2.0);
        let left = move |at: Point| (at.x() < bend.x() - size / 10.0).then(|| towards(bend, at));
        measured.push((format!("the C {height} mm tall"), off_square(plan, &format!("ts13b:auto:word-{height}:c"), left)));
    }
    for (what, rungs) in measured {
        assert!(rungs.len() >= 10, "{what}: {} rungs", rungs.len());
        let worst = rungs.iter().copied().max_by(|a, b| a.1.total_cmp(&b.1)).unwrap();
        assert!(worst.1 <= 10.0, "{what}: a rung {:.1}° from square at {:?}", worst.1, worst.0);
    }
}

#[test]
fn req_auto_007_ts13c_the_columns_on_the_face_cover_the_edges_of_the_areas_under_them() {
    // The face's outline, round its edge, and each eye's line, round the eye, lie over the pink face and the
    // white eyes, which reach under them. Pulled in by the overlap, the columns' stitches still cover the
    // edges they sew along: the outline's inner one, and either side of each eye's line.
    let (design, plan) = sheet("TS-13C");
    let width = |id: &str| match drawn(design, id).shape {
        Shape::Stroke { width, .. } => width.get(),
        Shape::Fill { .. } => panic!("{id} is a line"),
    };
    let circle = |middle: Point, radius: f64| -> Vec<Point> {
        (0..=360)
            .map(|degrees| f64::from(degrees).to_radians())
            .map(|t| p(middle.x() + radius * math::cos(t), middle.y() + radius * math::sin(t)))
            .collect()
    };
    let face = boxes(drawn(design, "ts13c:auto:face:face"))[0];
    let mut edges = vec![circle(middle(face), (face.max().x() - face.min().x()) / 2.0 - width("ts13c:auto:face:outline") / 2.0)];
    for eye in boxes(drawn(design, "ts13c:auto:face:eyes")) {
        let (radius, half) = ((eye.max().x() - eye.min().x()) / 2.0, width("ts13c:auto:face:eye-lines") / 2.0);
        edges.extend([circle(middle(eye), radius - half), circle(middle(eye), radius + half)]);
    }
    let covered = edge_coverage(&edges, plan, rules().auto_overlap_mm.get());
    assert!(covered > 0.99, "{covered}");
}

#[test]
fn ts13_the_jump_threads_that_its_stitches_sew_down() {
    // MC-8 asks where a stitch sews a jump thread down: the PE800 cuts none (`stitchcraft_testkit::stitches`).
    // For each sheet, as drawn and digitized, how many jump threads it leaves and how many of them a later
    // stitch of their colour sews down, as recorded in the design (`image-to-embroidery.md` › Testing).
    // Digitized, the letters of TS-13B sew as columns with jumps between them.
    for (id, recorded) in [("TS-13A", [(8, 0), (11, 3)]), ("TS-13B", [(6, 0), (37, 16)]), ("TS-13C", [(9, 4), (6, 1)])] {
        let threads = jump_threads(&sheet(id).1);
        let half = |stops: usize| {
            let left: Vec<_> = threads.iter().filter(|thread| thread.after_stops == stops).collect();
            (left.len(), left.iter().filter(|thread| thread.sewn_down > 0).count())
        };
        assert_eq!([half(0), half(1)], recorded, "{id}");
    }
}
