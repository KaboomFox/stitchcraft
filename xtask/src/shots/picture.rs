//! `picture` shots: the pictures automatic digitizing starts from, which xtask draws itself
//! (`docs/src/design/docs-pipeline.md` › Images).
//!
//! - **A design's areas, flat** (`fixture`): each area of the design filled in its thread's colour, the
//!   lowest first, as a drawing program shows it. A picture of flat colours looks so, and so do the shapes
//!   Image Trace traced a photo into. The areas are filled at [`SUPERSAMPLE`] times the size without
//!   anti-aliasing, and each block of pixels is then averaged into one. 2 areas that share an edge show no
//!   seam that way, where each area anti-aliased by itself would let the white under them show along it.
//! - **A photo drawn in code** (`picture`): [`pear`], a pear lit from the top left on a light backdrop,
//!   with soft shading and a little grain, as a camera takes one. It is drawn here so that the docs show a
//!   photo with no licence to keep, and the same pixels on every platform.
//!   `docs/fixtures/traced-pear.vectorcraft` is the document VectorCraft traced it into
//!   (`docs/fixtures/README.md`). A test keeps the hash of the picture it traced, so a change to the picture
//!   fails it until the picture is traced again.
//!
//! Both are compared byte for byte, as `stitch` shots are. The photo is computed with additions,
//! multiplications, divisions, square roots and `stitchcraft_core::math::exp`, and its grain comes from
//! `SplitMix64`. The areas' curves are flattened by the engine, so that tiny-skia fills only polygons, and
//! tiny-skia writes the PNG file. So every platform makes the same bytes (`docs/src/design/determinism.md`).

use std::path::Path;

use stitchcraft_cli::commands::plan::Source;
use stitchcraft_core::rng::SplitMix64;
use stitchcraft_core::{Budget, math};
use stitchcraft_engine::design::{FillRule, Reading, Shape};
use stitchcraft_engine::normalize::stroke::{self, SHAPE_TOLERANCE};
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, Transform};

use super::Shot;
use crate::util;

/// The kind of a picture xtask draws.
pub(super) const KIND: &str = "picture";
/// How many times larger a design's areas are filled before each block of pixels becomes one.
const SUPERSAMPLE: u32 = 4;
/// The largest picture of a design's areas, in pixels on a side: filled 4 times larger, it takes 64 MiB.
const MAX_SIDE: u32 = 1024;
/// A picture drawn in code: its PNG file, or why it could not be made.
type Draw = fn() -> Result<Vec<u8>, String>;
/// The pictures drawn in code, by the names shots give them.
const DRAWN: &[(&str, Draw)] = &[("pear", pear)];

/// What is wrong with the picture shot `shot`'s declaration: it draws a design or a picture drawn in code,
/// and sets nothing of another kind.
pub(super) fn problems(shot: &Shot) -> Vec<String> {
    let mut found = Vec::new();
    match (shot.fixture.as_deref(), shot.picture.as_deref()) {
        (Some(_), None) => {}
        (None, Some(name)) if DRAWN.iter().any(|(known, _)| *known == name) => {
            if shot.scale.is_some() {
                found.push("a picture drawn in code has pixels of its own: `scale` is for a design (`fixture`)".to_string());
            }
        }
        (None, Some(name)) => found.push(format!("there is no picture `{name}` drawn in code (known: {})", names())),
        _ => found.push(format!("a picture shot draws a design (`fixture = \"…\"`) or a picture drawn in code (`picture = \"…\"`: {})", names())),
    }
    if shot.sheet.is_some() || !shot.elements.is_empty() || !shot.panels.is_empty() || shot.auto || shot.style.is_some() {
        found.push("`sheet`, `elements`, `panels`, `auto` and `style` are a stitch shot's".to_string());
    }
    found
}

/// The names of the pictures drawn in code.
fn names() -> String {
    DRAWN.iter().map(|(name, _)| format!("`{name}`")).collect::<Vec<_>>().join(", ")
}

/// The PNG file of the picture shot `shot`: its design's areas drawn flat, or its picture drawn in code.
pub(super) fn make(root: &Path, shot: &Shot) -> Result<Vec<u8>, String> {
    let found = problems(shot);
    if !found.is_empty() {
        return Err(found.join("; "));
    }
    match (shot.fixture.as_deref(), shot.picture.as_deref()) {
        (Some(fixture), _) => flat(root, fixture, shot),
        (None, Some(name)) => DRAWN.iter().find(|(known, _)| *known == name).map_or_else(|| Err(format!("no picture `{name}`")), |(_, draw)| draw()),
        (None, None) => Err("a picture shot draws a design or a picture drawn in code".to_string()),
    }
}

/// An area of a design, ready to fill: its outline's rings in millimetres, how they bound it, and its colour.
struct Area {
    rings: Vec<Vec<(f64, f64)>>,
    rule: FillRule,
    colour: [u8; 3],
}

/// The design in `fixture` with its areas filled flat, lowest first, `shot.scale` pixels to the millimetre
/// ([`super::Settings::DEFAULT_SCALE`] when it sets none), on white. The reader may report only the codes
/// `shot` allows, and the design may hold only areas without clip paths.
fn flat(root: &Path, fixture: &str, shot: &Shot) -> Result<Vec<u8>, String> {
    let bytes = util::read_bytes(&root.join(fixture))?;
    let (design, warnings) = Source::of(Path::new(fixture)).read(&bytes, Reading::default()).map_err(|d| format!("{fixture}: {d}"))?;
    let said: Vec<String> = warnings.iter().filter(|d| !shot.allow.iter().any(|code| code == d.code.id())).map(ToString::to_string).collect();
    if !said.is_empty() {
        return Err(format!("{fixture}: {}", said.join("; ")));
    }
    let mut meter = Budget::DEFAULT.meter();
    let mut areas = Vec::new();
    for element in design.elements() {
        let Shape::Fill { path, rule } = &element.shape else {
            return Err(format!("{fixture}: {} is a line: a picture shot draws areas", element.id));
        };
        if !element.clips.is_empty() {
            return Err(format!("{fixture}: {} is clipped: a picture shot draws areas as they are", element.id));
        }
        let flattened = stroke::flatten(path, SHAPE_TOLERANCE, &mut meter).map_err(|_| format!("{fixture}: {} has too many curves", element.id))?;
        let rings = flattened.pieces.iter().map(|piece| piece.points.iter().map(|p| (p.x(), p.y())).collect()).collect();
        let colour = element.thread.color;
        areas.push(Area { rings, rule: *rule, colour: [colour.r, colour.g, colour.b] });
    }
    let scale = f64::from(shot.scale.unwrap_or(super::Settings::DEFAULT_SCALE));
    fill(&areas, scale).map_err(|e| format!("{fixture}: {e}"))
}

/// `areas` filled in order on white, `scale` pixels to the millimetre, across their bounds: filled
/// [`SUPERSAMPLE`] times larger without anti-aliasing, then each block of pixels averaged into one.
fn fill(areas: &[Area], scale: f64) -> Result<Vec<u8>, String> {
    let points = || areas.iter().flat_map(|area| area.rings.iter().flatten());
    let (mut left, mut top, mut right, mut bottom) = (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for &(x, y) in points() {
        (left, top, right, bottom) = (left.min(x), top.min(y), right.max(x), bottom.max(y));
    }
    let pixels = |from: f64, to: f64| -> Result<u32, String> {
        let count = ((to - from) * scale).ceil();
        // A count from 1 to `MAX_SIDE` is a whole number that fits a u32.
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let fits = (count.is_finite() && count >= 1.0 && count <= f64::from(MAX_SIDE)).then_some(count as u32);
        fits.ok_or_else(|| format!("a picture {count} pixels across is outside 1 to {MAX_SIDE}: draw it at a smaller `scale`"))
    };
    let (width, height) = (pixels(left, right)?, pixels(top, bottom)?);
    let mut large = Pixmap::new(width * SUPERSAMPLE, height * SUPERSAMPLE).ok_or("a picture with no pixels")?;
    large.fill(Color::WHITE);
    let to_pixels = scale * f64::from(SUPERSAMPLE);
    for area in areas {
        let mut outline = PathBuilder::new();
        for ring in &area.rings {
            // Pixel positions are far below 2^24, so f32 holds each within a thousandth of a pixel.
            #[allow(clippy::cast_possible_truncation)]
            let at = |(x, y): (f64, f64)| (((x - left) * to_pixels) as f32, ((y - top) * to_pixels) as f32);
            let mut ring = ring.iter().copied().map(at);
            let Some((x, y)) = ring.next() else { continue };
            outline.move_to(x, y);
            for (x, y) in ring {
                outline.line_to(x, y);
            }
            outline.close();
        }
        let Some(outline) = outline.finish() else { continue };
        let mut paint = Paint::default();
        let [r, g, b] = area.colour;
        paint.set_color_rgba8(r, g, b, 255);
        paint.anti_alias = false;
        let rule = match area.rule {
            FillRule::NonZero => tiny_skia::FillRule::Winding,
            FillRule::EvenOdd => tiny_skia::FillRule::EvenOdd,
        };
        large.fill_path(&outline, &paint, rule, Transform::identity(), None);
    }
    shrink(&large, width, height)?.encode_png().map_err(|e| format!("the PNG encoder failed: {e}"))
}

/// `large`, [`SUPERSAMPLE`] times `width` by `height` pixels, with each block of pixels averaged into one.
fn shrink(large: &Pixmap, width: u32, height: u32) -> Result<Pixmap, String> {
    let (wide, n) = (large.width() as usize, SUPERSAMPLE as usize);
    let data = large.data();
    let mut out = Vec::with_capacity(width as usize * height as usize * 4);
    for row in 0..height as usize {
        for column in 0..width as usize {
            let mut sum = [0_u32; 4];
            for y in row * n..(row + 1) * n {
                for x in column * n..(column + 1) * n {
                    let at = (y * wide + x) * 4;
                    let pixel = data.get(at..at + 4).ok_or("a block outside the picture")?;
                    for (total, channel) in sum.iter_mut().zip(pixel) {
                        *total += u32::from(*channel);
                    }
                }
            }
            let count = SUPERSAMPLE * SUPERSAMPLE;
            // The mean of bytes is a byte.
            #[allow(clippy::cast_possible_truncation)]
            out.extend(sum.map(|total| ((total + count / 2) / count) as u8));
        }
    }
    let size = tiny_skia::IntSize::from_wh(width, height).ok_or("a picture with no pixels")?;
    Pixmap::from_vec(out, size).ok_or_else(|| "the averaged pixels make no picture".to_string())
}

/// The pear photo's side, in pixels: it is square.
const PEAR_SIDE: u32 = 320;
/// How many samples each pixel of the photo averages along each side, so that its edges are smooth.
const SAMPLES: u32 = 4;
/// The seed of the photo's grain.
const GRAIN_SEED: u64 = 0x5045_4152; // "PEAR"
/// How far the grain moves a pixel, in levels of 255: up to this much darker or lighter, its 3 channels
/// alike.
const GRAIN: f64 = 4.5;
/// Where the light comes from: the top left, in front of the pear. The viewer looks along z.
const LIGHT: [f64; 3] = [-0.45, -0.55, 0.70];

/// A photo of a pear, [`PEAR_SIDE`] pixels square, as a PNG file: a yellow-green pear with a brown stem,
/// lit from the top left, on a light backdrop with a faint shadow, and a little grain. Each pixel averages
/// [`SAMPLES`] × [`SAMPLES`] samples of the scene, and the grain is added after.
fn pear() -> Result<Vec<u8>, String> {
    let light = Light::from(LIGHT);
    let mut grain = SplitMix64::new(GRAIN_SEED);
    let mut pixels = Vec::with_capacity((PEAR_SIDE * PEAR_SIDE * 4) as usize);
    let step = 1.0 / f64::from(SAMPLES);
    for row in 0..PEAR_SIDE {
        for column in 0..PEAR_SIDE {
            let mut sum = [0.0; 3];
            for sy in 0..SAMPLES {
                for sx in 0..SAMPLES {
                    let x = f64::from(column) + (f64::from(sx) + 0.5) * step;
                    let y = f64::from(row) + (f64::from(sy) + 0.5) * step;
                    for (total, channel) in sum.iter_mut().zip(scene(x, y, &light)) {
                        *total += channel;
                    }
                }
            }
            let noise = (grain.next_f64() + grain.next_f64() + grain.next_f64() - 1.5) * (GRAIN / 1.5);
            let count = f64::from(SAMPLES * SAMPLES);
            // Each channel is clamped to a byte's range first.
            #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
            pixels.extend(sum.map(|total| (total / count + noise).round().clamp(0.0, 255.0) as u8));
            pixels.push(255);
        }
    }
    let size = tiny_skia::IntSize::from_wh(PEAR_SIDE, PEAR_SIDE).ok_or("a picture with no pixels")?;
    let pixmap = Pixmap::from_vec(pixels, size).ok_or("the pixels make no picture")?;
    pixmap.encode_png().map_err(|e| format!("the PNG encoder failed: {e}"))
}

/// The directions that light a surface, each of unit length: towards the light, and halfway between it and
/// the viewer, where a shiny surface turned that way shows a highlight.
struct Light {
    towards: [f64; 3],
    halfway: [f64; 3],
}

impl Light {
    /// The light that comes from `from`.
    fn from(from: [f64; 3]) -> Light {
        let towards = unit(from);
        let [x, y, z] = towards;
        Light { towards, halfway: unit([x, y, z + 1.0]) }
    }
}

/// `v` scaled to unit length.
fn unit(v: [f64; 3]) -> [f64; 3] {
    let [x, y, z] = v;
    let length = (x * x + y * y + z * z).sqrt();
    v.map(|c| c / length)
}

/// The colour of the scene at `(x, y)`, in pixels from the photo's top-left corner, under `light`: the stem,
/// the pear or the backdrop.
fn scene(x: f64, y: f64, light: &Light) -> [f64; 3] {
    let inside = pear_distance(x, y) < 0.0;
    match stem(x, y) {
        // The stem stands on the pear's top, and its foot sinks into the pear's shading.
        Some(colour) if !inside || y < 68.0 => colour,
        _ if inside => skin(x, y, light),
        _ => backdrop(x, y),
    }
}

/// How far `(x, y)` is outside the pear's outline, in pixels, negative inside: a large circle low and a
/// small one high, blended smoothly into one outline (a smooth minimum of their distances).
fn pear_distance(x: f64, y: f64) -> f64 {
    let body = circle_distance(x, y, (160.0, 202.0), 86.0);
    let neck = circle_distance(x, y, (160.0, 112.0), 48.0);
    let blend = 40.0;
    let h = (0.5 + 0.5 * (neck - body) / blend).clamp(0.0, 1.0);
    neck * (1.0 - h) + body * h - blend * h * (1.0 - h)
}

/// How far `(x, y)` is outside the circle round `centre` of radius `radius`, negative inside.
fn circle_distance(x: f64, y: f64, centre: (f64, f64), radius: f64) -> f64 {
    let (dx, dy) = (x - centre.0, y - centre.1);
    (dx * dx + dy * dy).sqrt() - radius
}

/// The pear's skin at `(x, y)` under `light`: yellow-green, yellower low on the right, darker away from the
/// light, with a soft highlight. The pear is shaded as an egg 200 pixels wide and 260 tall round its middle.
fn skin(x: f64, y: f64, light: &Light) -> [f64; 3] {
    let (nx, ny) = ((x - 160.0) / 100.0, (y - 172.0) / 130.0);
    let nz = (1.0 - nx * nx - ny * ny).max(0.0).sqrt();
    let facing = |[x, y, z]: [f64; 3]| (nx * x + ny * y + nz * z).clamp(0.0, 1.0);
    let diffuse = facing(light.towards);
    let shine = power(facing(light.halfway), 40);
    let yellow = smoothstep(0.2, 0.9, 0.7 * (nx + ny)) * 0.5;
    let base = mix([170.0, 200.0, 60.0], [220.0, 210.0, 70.0], yellow);
    base.map(|channel| channel * (0.40 + 0.70 * diffuse) + 255.0 * 0.30 * shine)
}

/// The stem's colour at `(x, y)`, or `None` off it: a brown band 8 pixels wide at its foot, on the pear's
/// top at y = 66, that narrows and bends right as it rises 34 pixels, lit from the left.
fn stem(x: f64, y: f64) -> Option<[f64; 3]> {
    if !(32.0..74.0).contains(&y) {
        return None;
    }
    let rise = ((66.0 - y) / 34.0).clamp(0.0, 1.0);
    let (middle, half) = (161.0 + 5.0 * rise * rise, 4.2 - 1.4 * rise);
    let across = (x - middle) / half;
    (across.abs() < 1.0).then(|| [92.0, 58.0, 30.0].map(|channel| channel * (0.75 + 0.35 * smoothstep(-1.0, 1.0, -across))))
}

/// The backdrop at `(x, y)`: a light grey, a little warmer and darker towards the bottom and the corners,
/// with the pear's faint shadow below it on the right.
fn backdrop(x: f64, y: f64) -> [f64; 3] {
    let side = f64::from(PEAR_SIDE);
    let down = y / side;
    let (dx, dy) = (x - side / 2.0, y - side / 2.0);
    let corners = 1.0 - 0.02 * (dx * dx + dy * dy) / (side * side / 2.0);
    let (sx, sy) = ((x - 178.0) / 109.2, (y - 271.44) / 29.12);
    let shadow = 1.0 - 0.06 * math::exp(-1.6 * (sx * sx + sy * sy));
    mix([242.0, 241.0, 238.0], [236.0, 234.0, 230.0], down).map(|channel| channel * corners * shadow)
}

/// `from` moved a share `t` of the way to `to`, channel by channel.
fn mix([r0, g0, b0]: [f64; 3], [r1, g1, b1]: [f64; 3], t: f64) -> [f64; 3] {
    [r0 * (1.0 - t) + r1 * t, g0 * (1.0 - t) + g1 * t, b0 * (1.0 - t) + b1 * t]
}

/// 0 at `from` and below, 1 at `to` and above, and a smooth S-curve between.
fn smoothstep(from: f64, to: f64, value: f64) -> f64 {
    let t = ((value - from) / (to - from)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// `base` to the power `exponent`, by multiplying.
fn power(base: f64, exponent: u32) -> f64 {
    (0..exponent).fold(1.0, |product, _| product * base)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The FNV-1a hash of the [`pear`] picture that `docs/fixtures/traced-pear.vectorcraft` was traced from.
    const TRACED: u64 = 0x8483_29eb_1e5d_8063;

    /// The picture shot that `toml` declares, named `probe`.
    fn shot(toml: &str) -> Shot {
        toml::from_str(&format!("id = \"probe\"\nkind = \"picture\"\nalt = \"A probe\"\n{toml}")).unwrap()
    }

    /// The pixel of `picture` at `(x, y)`, as RGBA.
    fn pixel(picture: &Pixmap, x: u32, y: u32) -> [u8; 4] {
        let at = picture.pixel(x, y).unwrap();
        [at.red(), at.green(), at.blue(), at.alpha()]
    }

    #[test]
    fn a_picture_shot_draws_a_design_or_a_picture_drawn_in_code() {
        assert_eq!(problems(&shot("fixture = \"docs/fixtures/emblem.svg\"\nscale = 2.0")), Vec::<String>::new());
        assert_eq!(problems(&shot("picture = \"pear\"")), Vec::<String>::new());
        assert_eq!(problems(&shot("picture = \"apple\"")), ["there is no picture `apple` drawn in code (known: `pear`)"]);
        let scaled = problems(&shot("picture = \"pear\"\nscale = 2.0"));
        assert_eq!(scaled, ["a picture drawn in code has pixels of its own: `scale` is for a design (`fixture`)"]);
        for neither_or_both in ["", "fixture = \"docs/fixtures/emblem.svg\"\npicture = \"pear\""] {
            let found = problems(&shot(neither_or_both));
            assert!(found.len() == 1 && found[0].starts_with("a picture shot draws a design"), "{found:?}");
        }
        let stitched = problems(&shot("fixture = \"docs/fixtures/emblem.svg\"\nauto = true\nstyle = \"simple\""));
        assert_eq!(stitched, ["`sheet`, `elements`, `panels`, `auto` and `style` are a stitch shot's"]);
    }

    #[test]
    fn areas_that_share_an_edge_are_drawn_without_the_white_between_them() {
        // A red rectangle and a blue one beside it, 4 mm tall, 2.5 pixels to the millimetre. They share the
        // edge x = 5.02 mm, at 12.55 pixels: inside pixel 12, and inside the 3rd of its 4 columns of samples,
        // which anti-aliasing would draw partly white under each rectangle.
        let rectangle =
            |from: f64, to: f64, colour| Area { rings: vec![vec![(from, 0.0), (to, 0.0), (to, 4.0), (from, 4.0)]], rule: FillRule::NonZero, colour };
        let png = fill(&[rectangle(0.0, 5.02, [255, 0, 0]), rectangle(5.02, 10.0, [0, 0, 255])], 2.5).unwrap();
        let picture = Pixmap::decode_png(&png).unwrap();
        assert_eq!((picture.width(), picture.height()), (25, 10));
        for y in 0..10 {
            for x in 0..25 {
                let expected = match x {
                    0..12 => [255, 0, 0, 255],
                    // 2 columns of samples red and 2 blue, with none of the white under them.
                    12 => [128, 0, 128, 255],
                    _ => [0, 0, 255, 255],
                };
                assert_eq!(pixel(&picture, x, y), expected, "({x}, {y})");
            }
        }
    }

    #[test]
    fn a_design_s_areas_are_drawn_lowest_first_in_their_threads_colours() {
        let png = make(&util::root(), &shot("fixture = \"docs/fixtures/emblem.svg\"\nscale = 2.0")).unwrap();
        let picture = Pixmap::decode_png(&png).unwrap();
        // The emblem is 60 mm square, 2 pixels to the millimetre.
        assert_eq!((picture.width(), picture.height()), (120, 120));
        // The white round it, the navy square on the white, the star on the disc and a bar on the navy.
        for ((x, y), colour) in [((4, 4), [240, 240, 240]), ((20, 60), [14, 31, 124]), ((60, 48), [237, 23, 31]), ((60, 85), [232, 169, 0])] {
            assert_eq!(pixel(&picture, x, y), [colour[0], colour[1], colour[2], 255], "({x}, {y})");
        }
    }

    #[test]
    fn a_highlight_faces_halfway_between_the_light_and_the_viewer() {
        let light = Light::from([0.0, -3.0, 4.0]);
        assert_eq!(light.towards, [0.0, -0.6, 0.8]);
        // Halfway between (0, -0.6, 0.8) and the viewer's (0, 0, 1): (0, -0.6, 1.8), of length 1.897….
        let expected = [0.0, -0.316_227_766_016_838, 0.948_683_298_050_514];
        assert!(light.halfway.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-12), "{:?}", light.halfway);
    }

    #[test]
    fn the_traced_pear_was_traced_from_the_pear_drawn() {
        let drawn = pear().unwrap();
        assert_eq!(
            stitchcraft_core::rng::fnv1a64(&drawn),
            TRACED,
            "the pear changed: trace it again into docs/fixtures/traced-pear.vectorcraft (docs/fixtures/README.md) and keep its hash in TRACED"
        );
    }
}
