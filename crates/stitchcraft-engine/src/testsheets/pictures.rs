//! The test pictures of automatic digitizing: designs drawn in code that look like a picture of flat colours
//! once traced, or like a cartoon drawn with lines. The TS-13 sheets sew them beside the same shapes sewn in
//! paint order (`ts13`), and the engine's cases measure them (`stitchcraft_testkit::pictures`), so what is
//! measured is what the machine sews. No logo or picture of anyone else's is used
//! (`docs/src/design/image-to-embroidery.md` › Testing).
//!
//! Every shape is automatic: a shape without settings, as `--auto` reads it. A shape traced from a picture
//! is an area filled by the nonzero rule, so a hole runs the other way round from its outline, and a line
//! drawn as a line is a stroke. A picture of areas is traced one of 2 ways ([`Trace`]), and each sews the
//! same areas once digitized: abutting, as VectorCraft's Abutting method traces it, each area with a hole
//! where the next one is; or stacked, as its Overlapping method and Inkscape's Stack option trace it, each
//! colour a whole shape over the ones under it. Each picture is drawn where it is asked to be, so that a
//! sheet can draw it twice and a case where its numbers were measured.
//!
//! - [`badge`] and [`badges`]: areas that meet along straight edges and round a circle, the row of badges
//!   drawn badge by badge, as VectorCraft's Auto Grouping draws them (M13.2, M13.3).
//! - [`specks`]: specks round a disc. [`parted`]: a shape that runs on beside another without touching it
//!   (M13.2).
//! - [`word`]: block letters. [`window`]: a line traced as an area between fills, with a junction (M13.4,
//!   M13.5).
//! - [`face`]: a cartoon face drawn with lines 1, 2 and 4 mm wide, one with round caps, and small rings and
//!   hairpins (M13.1).
//! - [`star`] and [`balloon`]: shapes broad in places and narrow in others (M13.6).

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use stitchcraft_core::math::{self, sin_cos};
use stitchcraft_core::{ElementId, Mm, Point};
use stitchcraft_params::ParamSet;
use stitchcraft_plan::Thread;

use super::designed::{open, polygon, round};
use super::sketch::SheetError;
use super::{BLACK, BLUE, DEEP_GOLD, FLESH_PINK, RED, WHITE, thread};
use crate::design::{Cap, Element, FillRule, Join, Path, Segment, Shape, Subpath};

/// A ring of a shape: its corners, in order.
type Ring = Vec<(f64, f64)>;

/// How a picture was traced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trace {
    /// Each area with a hole where the next one is.
    Abutting,
    /// Each colour a whole shape over the ones under it.
    Stacked,
}

/// A test picture.
#[derive(Clone, Debug)]
pub struct Picture {
    /// Its name, which a sheet puts in the ids of its shapes (`badge-abutting`).
    pub name: String,
    /// Its shapes, the lowest first, each automatic.
    pub elements: Vec<Element>,
    /// The edges where 2 of its areas meet, as polylines: where fabric shows when they only meet. Only the
    /// pictures whose edges the cases measure have them.
    pub edges: Vec<Vec<Point>>,
}

/// A badge `scale` times 50 mm across round `centre`: a red disc, a blue square 20 mm wide on it, and a gold
/// disc 10 mm across on the square, traced as `trace` says. Its edges are the square's sides, level and
/// upright, and the gold disc's circle, at every angle to a fill's rows.
pub fn badge(centre: (f64, f64), scale: f64, trace: Trace) -> Result<Picture, SheetError> {
    let (elements, edges) = badge_at("", centre, scale, trace)?;
    let name = match trace {
        Trace::Abutting => "badge-abutting",
        Trace::Stacked => "badge-stacked",
    };
    Ok(Picture { name: name.into(), elements, edges })
}

/// 3 badges in a row round `centre`, each the badge of [`badge`] at `scale`, their red discs touching,
/// traced as `trace` says with VectorCraft's Auto Grouping, which draws each shape with the shapes inside
/// it: badge by badge, so that the threads go red, blue, gold 3 times over in paint order.
pub fn badges((cx, cy): (f64, f64), scale: f64, trace: Trace) -> Result<Picture, SheetError> {
    let (mut elements, mut edges) = (Vec::new(), Vec::new());
    for n in [1_u8, 2, 3] {
        let x = cx + 50.0 * scale * (f64::from(n) - 2.0);
        let (shapes, meet) = badge_at(&format!("-{n}"), (x, cy), scale, trace)?;
        elements.extend(shapes);
        edges.extend(meet);
    }
    let name = match trace {
        Trace::Abutting => "badges-abutting",
        Trace::Stacked => "badges-stacked",
    };
    Ok(Picture { name: name.into(), elements, edges })
}

/// A badge round `(cx, cy)`, `scale` times 50 mm across, traced as `trace` says, its shapes named `red`,
/// `blue` and `gold` followed by `suffix`; and the edges where they meet.
fn badge_at(suffix: &str, (cx, cy): (f64, f64), scale: f64, trace: Trace) -> Result<(Vec<Element>, Vec<Vec<Point>>), SheetError> {
    let (half, dot) = (10.0 * scale, 5.0 * scale);
    let (outer, square, inner) =
        (circle((cx, cy), 25.0 * scale, 96), rectangle((cx - half, cy - half), (cx + half, cy + half)), circle((cx, cy), dot, 48));
    // Abutting, each lower area has a hole where the next one lies, drawn the other way round so that the
    // nonzero rule leaves it empty.
    let (square_hole, inner_hole) = (reversed(&square), reversed(&inner));
    let [red, blue]: [Vec<&[(f64, f64)]>; 2] = match trace {
        Trace::Stacked => [vec![&outer], vec![&square]],
        Trace::Abutting => [vec![&outer, &square_hole], vec![&square, &inner_hole]],
    };
    let elements = vec![
        area(&format!("red{suffix}"), &red, &thread(RED)?)?,
        area(&format!("blue{suffix}"), &blue, &thread(BLUE)?)?,
        area(&format!("gold{suffix}"), &[&inner], &thread(DEEP_GOLD)?)?,
    ];
    Ok((elements, vec![closed(&square)?, closed(&inner)?]))
}

/// A blue disc 24 mm across round `centre` with 6 red specks round it, as noise in a picture leaves them,
/// each smaller than 1 mm² and sharing an edge with the disc, and a seventh speck 2 mm from it, sharing
/// none. The specks are one shape, as a trace draws a colour.
pub fn specks((cx, cy): (f64, f64)) -> Result<Picture, SheetError> {
    let disc = circle((cx, cy), 12.0, 96);
    let rim = circle((cx, cy), 13.0, 96);
    // Each speck is the disc's edge between 2 of its corners, 1 mm out: 0.82 mm².
    let mut specks: Vec<Ring> = (0..6)
        .filter_map(|k: usize| {
            let (i, j) = (16 * k, 16 * k + 1);
            Some(vec![*disc.get(i)?, *rim.get(i)?, *rim.get(j)?, *disc.get(j)?])
        })
        .collect();
    let (sin, cos) = sin_cos(7.0 * PI / 6.0);
    let (x, y) = (cx + 14.0 * cos, cy + 14.0 * sin);
    specks.push(rectangle((x - 0.4, y - 0.4), (x + 0.4, y + 0.4)));
    let rings: Vec<&[(f64, f64)]> = specks.iter().map(Vec::as_slice).collect();
    let elements = vec![area("disc", &[&disc], &thread(BLUE)?)?, area("specks", &rings, &thread(RED)?)?];
    Ok(Picture { name: "specks".into(), elements, edges: Vec::new() })
}

/// A gold square 18 × 24 mm from `(x, y)`, its top-left corner, and a blue bar 10 mm wide to its right that
/// meets its side along the top half and runs on beside the bottom half 0.5 mm from it, without touching it.
/// Its edge is where they meet.
pub fn parted((x, y): (f64, f64)) -> Result<Picture, SheetError> {
    let (side, half, bottom) = (x + 18.0, y + 12.0, y + 24.0);
    let square = rectangle((x, y), (side, bottom));
    let bar = vec![(side, y), (side + 10.0, y), (side + 10.0, bottom), (side + 0.5, bottom), (side + 0.5, half), (side, half)];
    let elements = vec![area("square", &[&square], &thread(DEEP_GOLD)?)?, area("bar", &[&bar], &thread(BLUE)?)?];
    Ok(Picture { name: "parted".into(), elements, edges: vec![polyline(&[(side, y), (side, half)])?] })
}

/// The word MATCH in black block letters `height` mm tall, from `(x, y)`, the top-left corner of its first
/// letter, each letter a shape of its own. Its strokes are 18 % of its height wide. The M's middle strokes
/// meet at sharp angles, the A has a hole and a crossbar between its legs, the T a stem under its bar, the C
/// a bend ending round, and the H a crossbar between its stems.
pub fn word((x, y): (f64, f64), height: f64) -> Result<Picture, SheetError> {
    let black = thread(BLACK)?;
    let mut elements = Vec::new();
    let mut left = 0.0;
    for (name, rings, width) in letters() {
        let placed: Vec<Ring> = rings.iter().map(|ring| ring.iter().map(|&(u, v)| (x + (left + u) * height, y + v * height)).collect()).collect();
        let placed: Vec<&[(f64, f64)]> = placed.iter().map(Vec::as_slice).collect();
        elements.push(area(name, &placed, &black)?);
        left += width + LETTER_GAP;
    }
    Ok(Picture { name: format!("word-{height}"), elements, edges: Vec::new() })
}

/// The space between 2 letters of [`word`], in heights.
const LETTER_GAP: f64 = 0.14;

/// How wide a stroke of [`word`]'s letters is, in heights.
const LETTER_STROKE: f64 = 0.18;

/// The letters of [`word`], each its name, its rings in heights from its top-left corner, the y axis down,
/// its holes the other way round, and how wide it is.
fn letters() -> [(&'static str, Vec<Ring>, f64); 5] {
    let s = LETTER_STROKE;
    // The M's middle strokes run from its top corners to its baseline, their sides 18° from upright, and
    // meet in a V that ends flat on the baseline.
    let m = vec![
        (0.0, 1.0),
        (0.0, 0.0),
        (0.22, 0.0),
        (0.46, 0.70),
        (0.70, 0.0),
        (0.92, 0.0),
        (0.92, 1.0),
        (0.74, 1.0),
        (0.74, 0.4385),
        (0.5475, 1.0),
        (0.3725, 1.0),
        (0.18, 0.4385),
        (0.18, 1.0),
    ];
    // The A's legs run from a flat top 0.16 wide to its feet, each as wide as a stroke across it, with a
    // crossbar from 0.6 to 0.78 and a hole above it.
    let a = vec![(0.0, 1.0), (0.35, 0.0), (0.51, 0.0), (0.86, 1.0), (0.6693, 1.0), (0.5923, 0.78), (0.2677, 0.78), (0.1907, 1.0)];
    let a_hole = vec![(0.43, 0.3163), (0.3307, 0.6), (0.5293, 0.6)];
    let t = vec![(0.0, 0.0), (0.8, 0.0), (0.8, s), (0.49, s), (0.49, 1.0), (0.31, 1.0), (0.31, s), (0.0, s)];
    let h = vec![
        (0.0, 0.0),
        (s, 0.0),
        (s, 0.41),
        (0.62, 0.41),
        (0.62, 0.0),
        (0.8, 0.0),
        (0.8, 1.0),
        (0.62, 1.0),
        (0.62, 0.59),
        (s, 0.59),
        (s, 1.0),
        (0.0, 1.0),
    ];
    [("m", vec![m], 0.92), ("a", vec![a, a_hole], 0.86), ("t", vec![t], 0.8), ("c", vec![c_letter()], 0.88), ("h", vec![h], 0.8)]
}

/// The C of [`word`]: a ring 1 high and a stroke wide round its middle, open on the right between 45° above
/// and below it, each end round, in heights.
fn c_letter() -> Ring {
    let (centre, outer, inner) = (0.5, 0.5, 0.5 - LETTER_STROKE);
    let at = |angle: f64, radius: f64| {
        let (sin, cos) = sin_cos(angle);
        (centre + radius * cos, centre - radius * sin)
    };
    let (from, to) = (PI / 4.0, 7.0 * PI / 4.0);
    let steps = 54_u32;
    let mut ring: Ring = (0..=steps).map(|k| at(from + (to - from) * f64::from(k) / f64::from(steps), outer)).collect();
    ring.extend(end(to, 1.0, &at));
    ring.extend((0..=steps).map(|k| at(to - (to - from) * f64::from(k) / f64::from(steps), inner)));
    ring.extend(end(from, -1.0, &at));
    ring
}

/// The points between the outer and the inner corner of the round end of [`c_letter`] at `angle`, a half
/// circle a stroke across that bulges on along the C, the way `way` says: 1 where the C runs on to larger
/// angles, -1 where it runs on to smaller ones.
fn end(angle: f64, way: f64, at: &impl Fn(f64, f64) -> (f64, f64)) -> impl Iterator<Item = (f64, f64)> {
    let (sin, cos) = sin_cos(angle);
    let (middle, r) = (at(angle, 0.5 - LETTER_STROKE / 2.0), LETTER_STROKE / 2.0);
    // Out from the middle of the C, and along it the way it would run on, the y axis down.
    let (out, on) = ((cos, -sin), (-sin * way, -cos * way));
    let steps = 8_u32;
    (1..steps).map(move |k| {
        let (s, c) = sin_cos(if way > 0.0 { PI * f64::from(k) / f64::from(steps) } else { PI - PI * f64::from(k) / f64::from(steps) });
        (middle.0 + r * (c * out.0 + s * on.0), middle.1 + r * (c * out.1 + s * on.1))
    })
}

/// A window 30 × 20 mm from `(x, y)`, its top-left corner: a black line `width` mm wide, traced as an area,
/// down it 12 mm from its left side, and a branch of it across to its right side halfway down, which ends at
/// the line it meets; and the red, blue and gold areas between them, each abutting the line.
pub fn window((x, y): (f64, f64), width: f64) -> Result<Picture, SheetError> {
    let (w, right, bottom) = (width / 2.0, x + 30.0, y + 20.0);
    let (down, across) = (x + 12.0, y + 10.0);
    let line = vec![
        (down - w, y),
        (down + w, y),
        (down + w, across - w),
        (right, across - w),
        (right, across + w),
        (down + w, across + w),
        (down + w, bottom),
        (down - w, bottom),
    ];
    let elements = vec![
        area("red", &[&rectangle((x, y), (down - w, bottom))], &thread(RED)?)?,
        area("blue", &[&rectangle((down + w, y), (right, across - w))], &thread(BLUE)?)?,
        area("gold", &[&rectangle((down + w, across + w), (right, bottom))], &thread(DEEP_GOLD)?)?,
        area("line", &[&line], &thread(BLACK)?)?,
    ];
    Ok(Picture { name: format!("window-{width}"), elements, edges: Vec::new() })
}

/// A cartoon face 40 mm across round `centre`, drawn with lines: its outline 4 mm wide, its eyes' 2 mm and its
/// brows 1 mm, a smile 2 mm wide with round caps, pupils that are small rings, and 3 hairs, each a hairpin.
/// The pupils and hairs turn more tightly than they are wide, which no satin column can follow. The face is
/// pink and the eyes white, under the black lines.
pub fn face((cx, cy): (f64, f64)) -> Result<Picture, SheetError> {
    let black = thread(BLACK)?;
    let eyes = [(cx - 8.0, cy - 5.0), (cx + 8.0, cy - 5.0)];
    let smile: Vec<(f64, f64)> = (0..=28)
        .map(|k: u32| {
            let (sin, cos) = sin_cos(PI / 7.0 + 5.0 * PI / 7.0 * f64::from(k) / 28.0);
            (cx + 10.0 * cos, cy + 1.0 + 10.0 * sin)
        })
        .collect();
    let brow = |(ex, ey): (f64, f64)| -> Result<Subpath, SheetError> {
        Ok(Subpath {
            start: Point::new(ex - 3.5, ey - 5.0)?,
            segments: vec![Segment::Quad(Point::new(ex, ey - 8.0)?, Point::new(ex + 3.5, ey - 5.0)?)],
            closed: false,
        })
    };
    let hair = |left: f64| {
        open(&[
            (left, cy - 18.0),
            (left, cy - 24.0),
            (left + 0.3, cy - 24.5),
            (left + 0.9, cy - 24.5),
            (left + 1.2, cy - 24.0),
            (left + 1.2, cy - 18.0),
        ])
    };
    let [one, other] = eyes;
    let elements = vec![
        area_of("face", vec![round((cx, cy), 20.0)?], &thread(FLESH_PINK)?)?,
        area_of("eyes", vec![round(one, 5.0)?, round(other, 5.0)?], &thread(WHITE)?)?,
        line("outline", vec![round((cx, cy), 20.0)?], 4.0, Cap::Butt, &black)?,
        line("eye-lines", vec![round(one, 5.0)?, round(other, 5.0)?], 2.0, Cap::Butt, &black)?,
        line("pupils", vec![round(one, 0.9)?, round(other, 0.9)?], 2.0, Cap::Butt, &black)?,
        line("brows", vec![brow(one)?, brow(other)?], 1.0, Cap::Round, &black)?,
        line("smile", vec![open(&smile)?], 2.0, Cap::Round, &black)?,
        line("hair", vec![hair(cx - 4.0)?, hair(cx - 0.6)?, hair(cx + 2.8)?], 2.0, Cap::Round, &black)?,
    ];
    Ok(Picture { name: "face".into(), elements, edges: Vec::new() })
}

/// A gold star of 5 points round `centre`, its points `radius` mm from it and the corners between them 0.4
/// times as far: its middle broad and its points narrow, tapering to a tip.
pub fn star((cx, cy): (f64, f64), radius: f64) -> Result<Picture, SheetError> {
    let ring: Ring = (0..10_u32)
        .map(|k| {
            let r = if k % 2 == 0 { radius } else { 0.4 * radius };
            let (sin, cos) = sin_cos(-FRAC_PI_2 + PI * f64::from(k) / 5.0);
            (cx + r * cos, cy + r * sin)
        })
        .collect();
    Ok(Picture { name: "star".into(), elements: vec![area("star", &[&ring], &thread(DEEP_GOLD)?)?], edges: Vec::new() })
}

/// A red balloon 18 mm wide and 22 mm tall round `centre`, on a string 1.4 mm wide of the same red that waves
/// 26 mm down from it: one shape, broad above and narrow below, its outline one ring, as a trace draws it.
pub fn balloon((cx, cy): (f64, f64)) -> Result<Picture, SheetError> {
    let (wide, tall, half) = (9.0, 11.0, 0.7);
    // The string meets the balloon where the balloon is as wide as the string, either side of its bottom.
    let meet = math::atan2(half, (wide * wide - half * half).sqrt());
    let around = |t: f64| {
        let (sin, cos) = sin_cos(t);
        (cx + wide * cos, cy + tall * sin)
    };
    let (top, length, steps) = (around(FRAC_PI_2 - meet).1, 26.0, 24_u32);
    // The string's middle, a whole wave 2 mm either side, from where it meets the balloon.
    let middle = |k: u32| {
        let along = f64::from(k) / f64::from(steps);
        (cx + 2.0 * sin_cos(TAU * along).0, top + length * along)
    };
    // Clockwise as the y axis points down: round the balloon from where the string meets it on the left, then
    // down the string's right side and up its left one.
    let mut outline: Ring = (0..=90_u32).map(|k| around(FRAC_PI_2 + meet + (TAU - 2.0 * meet) * f64::from(k) / 90.0)).collect();
    outline.extend((1..=steps).map(middle).map(|(x, y)| (x + half, y)));
    outline.extend((1..steps).rev().map(middle).map(|(x, y)| (x - half, y)));
    let shape = area("balloon", &[&outline], &thread(RED)?)?;
    Ok(Picture { name: "balloon".into(), elements: vec![shape], edges: Vec::new() })
}

/// An automatic area named `id`, bounded by `rings` under the nonzero rule, sewn with `thread`.
fn area(id: &str, rings: &[&[(f64, f64)]], thread: &Thread) -> Result<Element, SheetError> {
    let subpaths = rings.iter().map(|ring| polygon(ring)).collect::<Result<_, _>>()?;
    area_of(id, subpaths, thread)
}

/// An automatic area named `id`, bounded by the closed `subpaths` under the nonzero rule, sewn with `thread`.
fn area_of(id: &str, subpaths: Vec<Subpath>, thread: &Thread) -> Result<Element, SheetError> {
    automatic(id, Shape::Fill { path: Path { subpaths }, rule: FillRule::NonZero }, thread)
}

/// An automatic line named `id` along `subpaths`, `width` mm wide, ending with `cap`, its corners round, sewn
/// with `thread`.
fn line(id: &str, subpaths: Vec<Subpath>, width: f64, cap: Cap, thread: &Thread) -> Result<Element, SheetError> {
    automatic(id, Shape::Stroke { path: Path { subpaths }, width: Mm::new(width)?, join: Join::Round, cap }, thread)
}

/// An automatic element named `id`, of `shape`, sewn with `thread`.
fn automatic(id: &str, shape: Shape, thread: &Thread) -> Result<Element, SheetError> {
    Ok(Element { id: ElementId::new(id)?, name: None, shape, thread: thread.clone(), params: ParamSet::default(), automatic: true })
}

/// The rectangle from `(x0, y0)` to `(x1, y1)`, its corners in order.
fn rectangle((x0, y0): (f64, f64), (x1, y1): (f64, f64)) -> Ring {
    vec![(x0, y0), (x1, y0), (x1, y1), (x0, y1)]
}

/// A circle round `centre` of radius `r`, as `n` points.
fn circle((cx, cy): (f64, f64), r: f64, n: u32) -> Ring {
    (0..n)
        .map(|k| {
            let (sin, cos) = sin_cos(TAU * f64::from(k) / f64::from(n));
            (cx + r * cos, cy + r * sin)
        })
        .collect()
}

/// `ring` the other way round.
fn reversed(ring: &[(f64, f64)]) -> Ring {
    ring.iter().rev().copied().collect()
}

/// `ring` as a polyline back to its first point.
fn closed(ring: &[(f64, f64)]) -> Result<Vec<Point>, SheetError> {
    polyline(&[ring, ring.get(..1).unwrap_or_default()].concat())
}

/// The polyline through `points`.
fn polyline(points: &[(f64, f64)]) -> Result<Vec<Point>, SheetError> {
    Ok(points.iter().map(|&(x, y)| Point::new(x, y)).collect::<Result<_, _>>()?)
}
