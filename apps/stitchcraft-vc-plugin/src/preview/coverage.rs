//! How much of a fill's area its preview covers (`REQ-VC-024`).
//!
//! VectorCraft paints what a live effect returns with the object's own fill, so a preview reads as solid
//! only when its geometry covers the shape. These cases measure that, as VectorCraft's renderer would fill
//! it: along 401 lines down the shape and 401 across, they add up the lengths that lie inside the shape and
//! inside what the preview returned, by the nonzero rule. The share is exact along each line, so rows a
//! quarter of a millimetre apart and gaps a fiftieth wide count for what they are.

use serde_json::{Value, json};
use stitchcraft_vectorcraft::{read_auto_effect, read_effect};

use super::*;
use crate::testing::{auto_params, input, painted, params, square};

/// The closed rings of `path`, as VectorCraft's `PathData` of straight sides, in points.
fn rings(path: &Value) -> Vec<Vec<[f64; 2]>> {
    let point = |anchor: &Value| [anchor["p"][0].as_f64().unwrap(), anchor["p"][1].as_f64().unwrap()];
    let closed = path["subpaths"].as_array().unwrap().iter().filter(|subpath| subpath["closed"].as_bool().unwrap_or(false));
    closed.map(|subpath| subpath["anchors"].as_array().unwrap().iter().map(point).collect()).collect()
}

/// Where the line at `at` on axis `axis` crosses the sides of `rings`, each with the turn it adds to the
/// winding number: `which` tells the shape's crossings from the preview's.
fn crossings(rings: &[Vec<[f64; 2]>], axis: usize, at: f64, which: usize, into: &mut Vec<(f64, usize, i32)>) {
    for ring in rings {
        for (k, a) in ring.iter().enumerate() {
            let b = ring[(k + 1) % ring.len()];
            let turn = i32::from(a[axis] <= at && at < b[axis]) - i32::from(b[axis] <= at && at < a[axis]);
            if turn != 0 {
                let t = (at - a[axis]) / (b[axis] - a[axis]);
                into.push((a[1 - axis] + t * (b[1 - axis] - a[1 - axis]), which, turn));
            }
        }
    }
}

/// The share of `shape`'s area that `preview` covers, and the area it covers outside the shape as a share
/// of the shape's: the gaps along a satin column's stitches where they reach past the edge of a small
/// shape, as its thread does, which the fill paints as hairlines.
fn shares(shape: &Value, preview: &[u8]) -> (f64, f64) {
    let drawn: Value = serde_json::from_slice(preview).unwrap();
    assert!(drawn.get("error").is_none(), "nothing drawn: {drawn}");
    let (shape, preview) = (rings(shape), rings(&drawn["objects"][0]["path"]));
    let (mut inside, mut covered, mut spilt) = (0.0, 0.0, 0.0);
    for axis in [0, 1] {
        let ends = shape.iter().flatten().map(|p| p[axis]).fold((f64::MAX, f64::MIN), |(low, high), v| (low.min(v), high.max(v)));
        for line in 0..LINES {
            let at = ends.0 + (f64::from(line) + 0.5) / f64::from(LINES) * (ends.1 - ends.0);
            let mut crossed = Vec::new();
            crossings(&shape, axis, at, 0, &mut crossed);
            crossings(&preview, axis, at, 1, &mut crossed);
            crossed.sort_by(|a, b| a.0.total_cmp(&b.0));
            let (mut winding, mut from) = ([0, 0], f64::MIN);
            for (to, which, turn) in crossed {
                let length = to - from;
                match (winding[0] != 0, winding[1] != 0) {
                    (true, true) => (inside, covered) = (inside + length, covered + length),
                    (true, false) => inside += length,
                    (false, true) => spilt += length,
                    (false, false) => {}
                }
                winding[which] += turn;
                from = to;
            }
        }
    }
    (covered / inside, spilt / inside)
}

/// The lines each way that [`shares`] measures along.
const LINES: u32 = 401;

/// A polygon through `points`, in thirds of a millimetre as the demo's drawing has them, closed.
fn polygon(points: &[(f64, f64)]) -> Value {
    let anchors: Vec<Value> = points.iter().map(|(x, y)| json!({"p": [x * 24.0 / 25.4, y * 24.0 / 25.4]})).collect();
    json!({"subpaths": [{"closed": true, "anchors": anchors}]})
}

/// A disc of radius `r` about (`x`, `y`), as a polygon of 96 sides.
fn disc(x: f64, y: f64, r: f64) -> Vec<(f64, f64)> {
    (0..96)
        .map(|k| f64::from(k) / 96.0 * std::f64::consts::TAU)
        .map(|a| (x + r * stitchcraft_core::math::cos(a), y + r * stitchcraft_core::math::sin(a)))
        .collect()
}

/// The filled shapes of the browser demo's badge, Alpine Night (`web/templates/alpine-night.svg`), and a
/// ring 2 mm wide whose hole is wound the other way.
fn badge() -> Vec<(&'static str, Value)> {
    let lake: Vec<(f64, f64)> = [(44.0, 132.0), (136.0, 132.0)]
        .into_iter()
        .chain((1..16).map(|k| f64::from(k) / 16.0).map(|t| (136.0 - 92.0 * t, 132.0 + 68.0 * t * (1.0 - t))))
        .collect();
    let mut ring = polygon(&disc(90.0, 90.0, 75.0));
    let hole: Vec<(f64, f64)> = disc(90.0, 90.0, 69.0).into_iter().rev().collect();
    ring["subpaths"].as_array_mut().unwrap().extend(polygon(&hole)["subpaths"].as_array().unwrap().clone());
    vec![
        ("border", polygon(&disc(90.0, 90.0, 75.0))),
        ("sky", polygon(&disc(90.0, 90.0, 69.0))),
        ("moon", polygon(&disc(113.0, 55.0, 12.0))),
        ("star", polygon(&[(65.0, 38.0), (67.0, 44.0), (73.0, 46.0), (67.0, 48.0), (65.0, 54.0), (63.0, 48.0), (57.0, 46.0), (63.0, 44.0)])),
        ("far ridge", polygon(&[(35.0, 113.0), (73.0, 64.0), (112.0, 113.0)])),
        ("snowcap", polygon(&[(59.0, 82.0), (73.0, 64.0), (88.0, 83.0), (77.0, 79.0), (71.0, 84.0), (67.0, 79.0)])),
        ("near ridge", polygon(&[(69.0, 127.0), (111.0, 78.0), (145.0, 127.0)])),
        ("lake", polygon(&lake)),
        ("ring", ring),
    ]
}

/// What the Auto effect shows of a filled `shape` at `level`, with what VectorCraft would let it spend.
fn auto(shape: &Value, level: &str) -> Vec<u8> {
    let (input, params) = (painted(shape, true, None), auto_params(&[("preview_level", json!(level))]));
    let effect = read_auto_effect(&input, &params, &Budget::DEFAULT).unwrap();
    show_auto(&effect, spendable(FUEL_BASE, input.len(), params.len()))
}

/// What the fill effect shows of `shape` at `level`, with what VectorCraft would let it spend.
fn fill(shape: &Value, level: &str) -> Vec<u8> {
    let (input, params) = (input(shape), params(Family::Fill, &[("preview_level", json!(level))]));
    let effect = read_effect(Family::Fill, &input, &params, &Budget::DEFAULT).unwrap();
    show(Family::Fill, &effect, spendable(FUEL_BASE, input.len(), params.len()))
}

/// Squares from 3.5 mm a side to 903 mm, far wider than any hoop.
const SIDES: [f64; 7] = [10.0, 40.0, 160.0, 640.0, 1280.0, 1600.0, 2560.0];

#[test]
fn req_vc_024_a_fill_s_preview_covers_its_shape_at_every_level_auto_picks() {
    let mut low = Vec::new();
    let mut note = |name: String, shape: &Value, preview: Vec<u8>| {
        let (covered, spilt) = shares(shape, &preview);
        println!("{name}: covers {:.1} %, spills {:.2} %", covered * 100.0, spilt * 100.0);
        if covered < 0.9 || spilt > 0.03 {
            low.push(name);
        }
    };
    for (name, shape) in badge() {
        for level in ["auto", "outline"] {
            note(format!("Auto effect, {level}, the badge's {name}"), &shape, auto(&shape, level));
        }
    }
    for side in SIDES {
        let shape = square(side);
        for level in ["auto", "outline"] {
            note(format!("Auto effect, {level}, square {side} pt"), &shape, auto(&shape, level));
        }
        for level in ["auto", "rows", "outline"] {
            // Rows cost more than a live effect has from 903 mm a side, which `SC-I0810` says.
            if (level, side) != ("rows", 2560.0) {
                note(format!("fill effect, {level}, square {side} pt"), &shape, fill(&shape, level));
            }
        }
        // Every stitch of a small fill, which is the user's to pick.
        if side <= 40.0 {
            note(format!("fill effect, exact, square {side} pt"), &shape, fill(&shape, "exact"));
        }
    }
    assert!(low.is_empty(), "previews that cover less than 90 % of their shape, or spill over 3 % of it: {low:#?}");
}

#[test]
fn req_vc_024_a_fill_s_outline_is_its_shape_whole_with_nothing_drawn_on_it() {
    // The outline lays no stitch and no row, so nothing on it may read as thread or as a gap that sews: it
    // returns the shape's own rings and covers all of it, on the badge and on squares far wider than a hoop.
    let mut marked = Vec::new();
    let mut note = |name: String, shape: &Value, preview: Vec<u8>| {
        let (covered, spilt) = shares(shape, &preview);
        println!("{name}: {} subpaths, covers {:.2} %, spills {:.2} %", subpaths(&preview), covered * 100.0, spilt * 100.0);
        // The shape's own rings, and no ribbon. Its sides are the flattened path's, so the shares differ from
        // whole by a few hundredths of a percent at a sharp corner, as the star's do.
        if subpaths(&preview) != rings(shape).len() || covered < 0.999 || spilt > 0.001 {
            marked.push(name);
        }
    };
    for (name, shape) in badge() {
        note(format!("Auto effect, the badge's {name}"), &shape, auto(&shape, "outline"));
        note(format!("fill effect, the badge's {name}"), &shape, fill(&shape, "outline"));
    }
    for side in SIDES {
        let shape = square(side);
        note(format!("Auto effect, square {side} pt"), &shape, auto(&shape, "outline"));
        note(format!("fill effect, square {side} pt"), &shape, fill(&shape, "outline"));
    }
    // Auto shows the largest square, whose rows cost too much, by the same outline.
    note("Auto effect at auto, square 2560 pt".into(), &square(2560.0), auto(&square(2560.0), "auto"));
    assert!(marked.is_empty(), "outlines with a mark on them, or that leave part of their shape uncovered: {marked:#?}");
}

/// The fill effect's element for `shape`, with its defaults.
fn element(shape: &Value) -> Element {
    read_effect(Family::Fill, &input(shape), &params(Family::Fill, &[]), &Budget::DEFAULT).unwrap().element
}

/// `count` lines a millimetre long, a hundredth of a millimetre apart, from (`x`, `y`) millimetres.
fn lines(count: u32, x: f64, y: f64) -> Vec<(Point, Point)> {
    let p = |x: f64, y: f64| Point::new(x, y).unwrap();
    (0..count).map(|k| y + f64::from(k) * 0.01).map(|y| (p(x, y), p(x + 1.0, y))).collect()
}

/// The subpaths `drawn` has.
fn subpaths(drawn: &[u8]) -> usize {
    rings(&serde_json::from_slice::<Value>(drawn).unwrap()["objects"][0]["path"]).len()
}

/// How wide the gap that is the subpath `k` of `drawn` is, in millimetres: from its first corner to its last.
fn gap(drawn: &[u8], k: usize) -> f64 {
    let ring = &rings(&serde_json::from_slice::<Value>(drawn).unwrap()["objects"][0]["path"])[k];
    stitchcraft_core::math::hypot(ring[0][0] - ring[3][0], ring[0][1] - ring[3][1]) * 25.4 / 72.0
}

#[test]
fn req_vc_024_a_fill_s_shape_is_drawn_within_its_share_or_not_at_all() {
    let small = element(&square(10.0));
    let solid = |count: u32, spend: Option<u64>| solid(&small, lines(count, 1.0, 1.0), GAP_MM, spend.map(Spend));
    // With no line to show there is nothing to draw: the object as it is shows as much.
    assert_eq!(solid(0, None), None);
    // Flattening the square takes 4 units of work, which half of 25,600 instructions pay for.
    assert_eq!((solid(2, Some(25_599)), solid(2, Some(25_600)).map(|drawn| subpaths(&drawn))), (None, Some(3)));
    // Its 5 points and 100 gaps are 405 anchors, which half of 405,000 instructions pay for.
    assert_eq!((solid(100, Some(404_999)), solid(100, Some(405_000)).map(|drawn| subpaths(&drawn))), (None, Some(101)));
    assert_eq!(solid(100, None).map(|drawn| subpaths(&drawn)), Some(101));
    // Telling which way a gap is wound in the sky's 97 sides takes a unit for one line, and 17 for 10.
    let sky = element(&badge().remove(1).1);
    let flattening = flatten(sky.shape.path(), SHAPE_TOLERANCE, &mut Budget::DEFAULT.meter()).map(|flat| anchors_of(&flat, 0)).unwrap();
    assert_eq!(flattening, 97);
    let units = |count: u32| (0..200).find(|units| super::solid(&sky, lines(count, 20.0, 20.0), GAP_MM, Some(Spend(units * 6_400))).is_some());
    let flat = units(1).unwrap() - 1;
    assert_eq!(units(10), Some(flat + 17));
}

#[test]
fn req_vc_024_a_gap_is_cut_by_the_fill_rule_and_is_as_narrow_as_a_hair() {
    // 2 squares turning the same way, one inside the other: the nonzero rule fills the inner one, where a
    // gap shows, and the even-odd rule leaves it empty, where a ribbon would be painted.
    let mut nested = square(100.0);
    let inner = json!({"closed": true, "anchors": [{"p": [30, 30]}, {"p": [70, 30]}, {"p": [70, 70]}, {"p": [30, 70]}]});
    nested["subpaths"].as_array_mut().unwrap().push(inner);
    let nonzero = element(&nested);
    let mut evenodd = nonzero.clone();
    let Shape::Fill { rule, .. } = &mut evenodd.shape else { panic!("no fill") };
    *rule = FillRule::EvenOdd;
    let flat = flatten(nonzero.shape.path(), SHAPE_TOLERANCE, &mut Budget::DEFAULT.meter()).unwrap();
    let middle = || lines(1, 17.0, 17.0);
    let cut = |element: &Element| cut(element, &flat, middle(), GAP_MM, &mut Budget::DEFAULT.meter()).unwrap().map(|drawn| subpaths(&drawn));
    assert_eq!((cut(&nonzero), cut(&evenodd)), (Some(3), Some(2)));
    // The outline's shape tests no side, so it costs no work, and has no gap: both rings only.
    assert_eq!(subpaths(&whole(&nonzero, &flat).unwrap()), 2);
    // A row's gap and a stitch's are a hundredth of a millimetre wide.
    let Shown::Drawn(Level::Rows, rows) = fill_rows(&element(&square(40.0)), Spend(FUEL_BASE)) else { panic!("no rows") };
    assert!((gap(&rows, 1) - GAP_MM).abs() < 0.001, "{}", gap(&rows, 1));
    let Shown::Drawn(Level::Exact, stitches) = exact(Family::Fill, &element(&square(40.0)), None) else { panic!("no stitches") };
    assert!(subpaths(&stitches) > subpaths(&rows) && (gap(&stitches, 1) - GAP_MM).abs() < 0.001, "{}", gap(&stitches, 1));
}
