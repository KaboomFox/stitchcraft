//! Drawing a scene: two styles, one deterministic rasterizer.
//!
//! - **Realistic** shows the finished embroidery on fabric. Each stitch is a thread 0.4 mm wide (40 wt
//!   embroidery thread) with a darker edge and a lighter crest, drawn in sewing order so that later
//!   stitches lie on earlier ones, as they do on the fabric. A jump thread the machine does not cut
//!   lies loose on top until it is cut by hand, so it is drawn too, thinner; cut travel leaves nothing.
//! - **Simple** is for checking a design. Each stitch is a thin line and each needle hole a dot; moves
//!   without sewing are dashed, in the thread's colour when the thread is carried loose and grey when it
//!   was cut. Lock stitches get a ring, trims a red cross and stops a blue square.
//!
//! Every size is in millimetres times the scale, so a design looks the same at every scale, only
//! sharper. A margin of [`MARGIN_MM`] keeps threads and marks at the edge inside the image.
//!
//! Two things that don't sew can be drawn too, so that a picture in the docs shows what the stitches
//! follow and how to read it. A scene's guides, the design's lines that aren't sewn, are drawn first in
//! every style, thin and grey, so the stitches lie on them (`REQ-RND-005`). A thickness picture can have a
//! key under it, its steps' colours and the limits where each starts ([`Settings::keyed`], `REQ-RND-006`),
//! taken from the constants the style draws with, so the key can't say other numbers than the picture uses.
//!
//! **Determinism.** tiny-skia is built without its SIMD feature, so its pipelines run the same scalar
//! code on every CPU. Only straight lines and circles are drawn: circles and round caps are conics,
//! which tiny-skia flattens with arithmetic and square roots (correctly rounded everywhere), so nothing
//! reaches the cubic root-finding or rotations that use the platform's trigonometry. Grid positions are
//! integers below 2^24, converted to `f32` exactly. The PNG encoder (png, with miniz_oxide) is pure
//! Rust and writes no timestamps. So a scene and its settings give the same bytes everywhere; the golden
//! PNG files check it on Linux, macOS and Windows (REQ-RND-002).

use stitchcraft_core::{Meter, Point};
use stitchcraft_plan::thickness::{self, Limits, Measure, Step, Thickness};
use stitchcraft_plan::{Rgb, Role, SewnStitch};
use tiny_skia::{Color, FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, StrokeDash, Transform};

use crate::error::RenderError;
use crate::key::{self, Glyph};
use crate::scene::{Arrival, GridPoint, Hole, Mark, Scene};

/// The largest preview, in pixels on a side (4096 × 4096 RGBA pixels take 64 MiB).
pub const MAX_SIDE: u32 = 4096;
/// Room around the design, in millimetres.
pub const MARGIN_MM: f32 = 2.0;
/// The width of embroidery thread on fabric (40 wt), in millimetres.
const THREAD_MM: f32 = 0.4;

/// How a preview looks (see the module documentation).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Style {
    /// The finished embroidery on fabric.
    Realistic,
    /// Lines, needle holes and marks, for checking a design.
    Simple,
    /// How thick the design sews: where thread lies, coloured by how many fills of it lie over each spot
    /// (`REQ-RND-004`).
    Thickness,
}

impl Style {
    /// Every style.
    pub const ALL: &'static [Style] = &[Style::Realistic, Style::Simple, Style::Thickness];

    /// The style's name, as the command line and `docs/shots.toml` spell it.
    pub const fn name(self) -> &'static str {
        match self {
            Style::Realistic => "realistic",
            Style::Simple => "simple",
            Style::Thickness => "thickness",
        }
    }

    /// The style called `name`.
    pub fn from_name(name: &str) -> Option<Style> {
        Style::ALL.iter().copied().find(|s| s.name() == name)
    }

    /// The colour under the design: unbleached cotton for realistic previews, white for simple ones.
    const fn background(self) -> Rgb {
        match self {
            Style::Realistic => Rgb::from_hex(0xF4EFE6),
            Style::Simple | Style::Thickness => Rgb::from_hex(0xFFFFFF),
        }
    }
}

/// A style and a scale, checked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    style: Style,
    scale: f32,
    fabric: Option<Rgb>,
    key: bool,
}

impl Settings {
    /// The smallest scale, in pixels per millimetre: at this scale a design spanning the whole machine
    /// grid (20 m) still fits [`MAX_SIDE`].
    pub const MIN_SCALE: f32 = 0.1;
    /// The largest scale: beyond 50 pixels per millimetre a preview only gets larger, not more useful.
    pub const MAX_SCALE: f32 = 50.0;
    /// The default scale, 8 pixels per millimetre (about 200 dpi): a 150 mm design is about 1,250
    /// pixels across.
    pub const DEFAULT_SCALE: f32 = 8.0;

    /// `style` at `scale` pixels per millimetre; `None` unless the scale is within
    /// [`MIN_SCALE`](Self::MIN_SCALE)..=[`MAX_SCALE`](Self::MAX_SCALE).
    pub fn new(style: Style, scale: f32) -> Option<Settings> {
        (Self::MIN_SCALE..=Self::MAX_SCALE).contains(&scale).then_some(Settings { style, scale, fabric: None, key: false })
    }

    /// The style.
    pub const fn style(self) -> Style {
        self.style
    }

    /// Pixels per millimetre.
    pub const fn scale(self) -> f32 {
        self.scale
    }

    /// These settings with the design drawn on fabric of the colour `fabric` instead of the style's own
    /// (`REQ-RND-003`): a design has to cover on fabric of any colour, and black shows what cream hides.
    #[must_use]
    pub const fn on(self, fabric: Rgb) -> Settings {
        Settings { fabric: Some(fabric), ..self }
    }

    /// These settings with a key under the picture (`REQ-RND-006`): a box in each step's colour and, under
    /// each border between 2 boxes, the number of fills where the next step starts. `None` unless the style
    /// is [`Style::Thickness`], the only style with steps to explain.
    #[must_use]
    pub const fn keyed(self) -> Option<Settings> {
        match self.style {
            Style::Thickness => Some(Settings { key: true, ..self }),
            Style::Realistic | Style::Simple => None,
        }
    }

    /// The colour of the fabric under the design: the one [`on`](Self::on) named, or else the style's own.
    pub const fn fabric(self) -> Rgb {
        match self.fabric {
            Some(fabric) => fabric,
            None => self.style.background(),
        }
    }
}

/// The colours of the thickness style: thread under 2 fills, 2 fills or more, the warning's limit or more,
/// and the failing limit or more.
const ORDINARY: Rgb = Rgb::from_hex(0xBDBDBD);
const DOUBLED: Rgb = Rgb::from_hex(0xF2CF45);
const THICK: Rgb = Rgb::from_hex(0xEE7F1E);
const FAILING: Rgb = Rgb::from_hex(0xC8161D);
/// The colours of the thickness style's steps, from the thinnest thread to the thickest, as its key shows them.
const STEPS: [Rgb; 4] = [ORDINARY, DOUBLED, THICK, FAILING];

/// The grey of the lines of a design that are not sewn (`REQ-RND-005`).
const GUIDE: Rgb = Rgb::from_hex(0xA6A6A6);
/// How wide those lines are, in millimetres: thinner than a simple picture's stitch.
const GUIDE_MM: f32 = 0.1;
/// The height of a thickness picture's key, in millimetres: 1 of room, 3 of boxes, 1 of room, 2.5 of numbers
/// and 1.5 of room.
const KEY_MM: f32 = 9.0;
/// The dark grey of the key's numbers.
const INK: Rgb = Rgb::from_hex(0x404040);

/// A drawn preview.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Image {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// The PNG file.
    pub png: Vec<u8>,
}

impl Scene {
    /// Draws the scene. One unit of work per element drawn, plus one per pixel of its length.
    pub fn render(&self, settings: Settings, meter: &mut Meter) -> Result<Image, RenderError> {
        let (min, max) = self.bounds().ok_or(RenderError::Empty)?;
        let below = if settings.key { KEY_MM } else { 0.0 };
        let (width, height) = image_size(min, max, settings.scale, below)?;
        let pixmap = Pixmap::new(width, height).ok_or_else(|| internal("no canvas of that size"))?;
        let mut canvas = Canvas { pixmap, min, mm: settings.scale, meter };
        canvas.pixmap.fill(color(settings.fabric()));
        canvas.guides(&self.guides)?;
        match settings.style {
            Style::Realistic => canvas.realistic(self)?,
            Style::Simple => canvas.simple(self)?,
            Style::Thickness => canvas.thickness(self)?,
        }
        if settings.key {
            // The design's picture keeps its size, the other styles' at this scale; the key is under it.
            let (_, design) = image_size(min, max, settings.scale, 0.0)?;
            canvas.key(design, settings.fabric())?;
        }
        let png = canvas.pixmap.encode_png().map_err(|e| internal(&format!("the PNG encoder failed: {e}")))?;
        Ok(Image { width, height, png })
    }
}

/// The image size for a design from `min` to `max` at `scale`, with `below` millimetres more under it, or why
/// there is none.
fn image_size(min: GridPoint, max: GridPoint, scale: f32, below: f32) -> Result<(u32, u32), RenderError> {
    let millimetres = |lo: i32, hi: i32| (f64::from(hi) - f64::from(lo)) / 10.0 + 2.0 * f64::from(MARGIN_MM);
    let (across, down) = (millimetres(min.x, max.x), millimetres(min.y, max.y) + f64::from(below));
    let pixels = |mm: f64| (mm * f64::from(scale)).ceil();
    let limit = f64::from(MAX_SIDE);
    let (width, height) = (pixels(across), pixels(down));
    if width > limit || height > limit {
        // Rounded down to 0.1, and one pixel short of the limit, so the suggestion always fits.
        let largest = ((limit - 1.0) / across.max(down) * 10.0).floor() / 10.0;
        // Both values are positive and far below the integer types' limits.
        #[allow(clippy::cast_possible_truncation)]
        return Err(RenderError::TooLarge { width: width as u64, height: height as u64, max: MAX_SIDE, largest_scale: largest as f32 });
    }
    // At least 2 × MARGIN_MM × MIN_SCALE > 0 and at most MAX_SIDE, so the conversion is exact.
    #[allow(clippy::cast_possible_truncation)]
    Ok((width as u32, height as u32))
}

/// A pixmap and how grid points map onto it.
struct Canvas<'m> {
    pixmap: Pixmap,
    /// The grid point at the top-left corner of the design.
    min: GridPoint,
    /// Pixels per millimetre.
    mm: f32,
    meter: &'m mut Meter,
}

/// A line's look.
#[derive(Clone, Copy)]
struct Pen {
    color: Rgb,
    /// Width in pixels.
    width: f32,
    /// Dash length in pixels (gaps are 60 % of it); `None` for a solid line with round caps.
    dash: Option<f32>,
}

impl Canvas<'_> {
    /// `p` in pixels. Differences of grid coordinates are below 2^24, so they convert to `f32` exactly.
    fn px(&self, p: GridPoint) -> (f32, f32) {
        let (dx, dy) = (p.x - self.min.x, p.y - self.min.y);
        let to_px = |units: i32| (MARGIN_MM + units as f32 / 10.0) * self.mm;
        (to_px(dx), to_px(dy))
    }

    /// The lines of the design that are not sewn, thin and grey, each as one polyline.
    fn guides(&mut self, guides: &[Vec<GridPoint>]) -> Result<(), RenderError> {
        let pen = Pen { color: GUIDE, width: (GUIDE_MM * self.mm).max(1.0), dash: None };
        for guide in guides {
            let mut points = guide.iter().map(|p| self.px(*p));
            let Some((x, y)) = points.next() else { continue };
            let (mut path, mut length, mut last) = (PathBuilder::new(), 0.0, (x, y));
            path.move_to(x, y);
            for (x, y) in points {
                path.line_to(x, y);
                length += (x - last.0).abs() + (y - last.1).abs();
                last = (x, y);
            }
            // A guide of one point has no line to draw.
            let Some(path) = path.finish() else { continue };
            self.stroke(&path, pen, length)?;
        }
        Ok(())
    }

    /// The key of a thickness picture, in the strip under the design's `design` rows of pixels, on `fabric`:
    /// a box in each step's colour between the margins, and the number of fills where each step after the
    /// first starts under the border it starts at.
    fn key(&mut self, design: u32, fabric: Rgb) -> Result<(), RenderError> {
        let (width, height, mm) = (self.pixmap.width() as f32, self.pixmap.height() as f32, self.mm);
        let top = design as f32;
        let strip = Rect::from_ltrb(0.0, top, width, height).ok_or_else(|| internal("the key's strip"))?;
        self.meter.charge(1 + work(width * (height - top)))?;
        self.pixmap.fill_rect(strip, &paint(fabric), Transform::identity(), None);
        let (left, right) = (MARGIN_MM * mm, width - MARGIN_MM * mm);
        let each = (right - left) / STEPS.len() as f32;
        for (n, colour) in STEPS.iter().enumerate() {
            let x = left + each * n as f32;
            let swatch = Rect::from_xywh(x, top + mm, each, 3.0 * mm).ok_or_else(|| internal("a box of the key"))?;
            self.meter.charge(1 + work(each * 3.0 * mm))?;
            self.pixmap.fill_rect(swatch, &paint(*colour), Transform::identity(), None);
        }
        let size = 2.5 * mm;
        let pen = Pen { color: INK, width: (0.12 * size).max(1.0), dash: None };
        for (n, text) in key_labels().iter().enumerate() {
            let glyphs = key::glyphs(text).ok_or_else(|| RenderError::Internal(format!("the key has no figures for {text}")))?;
            let advance = |glyph: &Glyph| if *glyph == Glyph::Point { 0.3 } else { 0.8 };
            let across: f32 = glyphs.iter().map(advance).sum::<f32>() - 0.25;
            let mut x = left + each * (n + 1) as f32 - across * size / 2.0;
            for glyph in &glyphs {
                let mut path = PathBuilder::new();
                for ((x0, y0), (x1, y1)) in glyph.strokes() {
                    path.move_to(x + x0 * size, top + 5.0 * mm + y0 * size);
                    path.line_to(x + x1 * size, top + 5.0 * mm + y1 * size);
                }
                let path = path.finish().ok_or_else(|| internal("a figure of the key"))?;
                self.stroke(&path, pen, 4.0 * size)?;
                x += advance(glyph) * size;
            }
        }
        Ok(())
    }

    fn realistic(&mut self, scene: &Scene) -> Result<(), RenderError> {
        let w = THREAD_MM * self.mm;
        let mut previous = None;
        for hole in &scene.holes {
            if let Some(from) = previous {
                match hole.arrival {
                    Arrival::Sewn => {
                        // Edge, body, crest: a round thread lit from the front.
                        self.line(from, hole.at, Pen { color: shade(hole.color, 60), width: w, dash: None })?;
                        self.line(from, hole.at, Pen { color: hole.color, width: w * 0.66, dash: None })?;
                        self.line(from, hole.at, Pen { color: tint(hole.color, 45), width: w * 0.22, dash: None })?;
                    }
                    Arrival::Loose => {
                        self.line(from, hole.at, Pen { color: shade(hole.color, 60), width: w * 0.5, dash: None })?;
                        self.line(from, hole.at, Pen { color: hole.color, width: w * 0.3, dash: None })?;
                    }
                    Arrival::Cut => {}
                }
            }
            previous = Some(hole.at);
        }
        Ok(())
    }

    /// Colours each pixel by the thread over the spot under its middle (`stitchcraft_plan::thickness`),
    /// measured on the stitches of `scene`, which lie on the machine's grid, at the spot nearest to it: grey
    /// under 2 fills, then
    /// yellow, orange from the warning's limit and red from the failing one, by the default limits. Fabric
    /// shows where no thread lies. A unit of work for each pixel, beside the measure's.
    fn thickness(&mut self, scene: &Scene) -> Result<(), RenderError> {
        let at = |hole: GridPoint| Point::from_tenths(hole.x, hole.y);
        let stitch = |pair: &[Hole]| match pair {
            [from, to] if to.arrival == Arrival::Sewn => {
                Some(SewnStitch { block: 0, index: 0, from: at(from.at), to: at(to.at), role: Role::Top, from_role: Role::Top })
            }
            _ => None,
        };
        let sewn: Vec<SewnStitch> = scene.holes.windows(2).filter_map(stitch).collect();
        let Some(measured) = Thickness::of_stitches(&sewn, self.meter)? else { return Ok(()) };
        let corner = at(self.min);
        let (width, mm) = (self.pixmap.width() as usize, f64::from(self.mm));
        self.meter.charge(u64::from(self.pixmap.width()) * u64::from(self.pixmap.height()))?;
        for (place, pixel) in self.pixmap.pixels_mut().iter_mut().enumerate() {
            let middle = |k: usize, corner: f64| corner + (k as f64 + 0.5) / mm - f64::from(MARGIN_MM);
            let spot = Point::new(middle(place % width, corner.x()), middle(place / width, corner.y())).unwrap_or(Point::ORIGIN);
            let colour = match Limits::DEFAULT.step(measured.at(Measure::Fills, spot)) {
                Step::Bare => continue,
                Step::Ordinary => ORDINARY,
                Step::Doubled => DOUBLED,
                Step::Thick => THICK,
                Step::Failing => FAILING,
            };
            *pixel = color(colour).premultiply().to_color_u8();
        }
        Ok(())
    }

    fn simple(&mut self, scene: &Scene) -> Result<(), RenderError> {
        let thin = (0.12 * self.mm).max(1.0);
        let dash = (1.0 * self.mm).max(3.0);
        let dot = (0.2 * self.mm).max(1.2);
        let mut previous = None;
        for hole in &scene.holes {
            if let Some(from) = previous {
                let pen = match hole.arrival {
                    Arrival::Sewn => Pen { color: hole.color, width: thin, dash: None },
                    Arrival::Loose => Pen { color: hole.color, width: thin, dash: Some(dash) },
                    Arrival::Cut => Pen { color: TRAVEL, width: thin, dash: Some(dash) },
                };
                self.line(from, hole.at, pen)?;
            }
            self.dot(hole.at, dot, shade(hole.color, 55))?;
            if hole.lock {
                self.ring(hole.at, dot * 2.5, Pen { color: LOCK, width: thin, dash: None })?;
            }
            previous = Some(hole.at);
        }
        let mark = (0.7 * self.mm).max(4.0);
        let mark_width = (0.15 * self.mm).max(1.5);
        let pen = |color| Pen { color, width: mark_width, dash: None };
        for m in &scene.marks {
            let (x, y) = self.px(m.at());
            match m {
                Mark::Stop(_) => {
                    let square = Rect::from_xywh(x - mark, y - mark, 2.0 * mark, 2.0 * mark).ok_or_else(|| internal("a stop mark"))?;
                    self.stroke(&PathBuilder::from_rect(square), pen(STOP), 2.0 * mark)?;
                }
                Mark::Trim(_) => {
                    let mut cross = PathBuilder::new();
                    cross.move_to(x - mark, y - mark);
                    cross.line_to(x + mark, y + mark);
                    cross.move_to(x - mark, y + mark);
                    cross.line_to(x + mark, y - mark);
                    let cross = cross.finish().ok_or_else(|| internal("a trim mark"))?;
                    self.stroke(&cross, pen(TRIM), 2.0 * mark)?;
                }
            }
        }
        Ok(())
    }

    /// A straight line from `from` to `to`.
    fn line(&mut self, from: GridPoint, to: GridPoint, pen: Pen) -> Result<(), RenderError> {
        let ((x0, y0), (x1, y1)) = (self.px(from), self.px(to));
        let mut path = PathBuilder::new();
        path.move_to(x0, y0);
        path.line_to(x1, y1);
        let path = path.finish().ok_or_else(|| internal("a stitch"))?;
        self.stroke(&path, pen, (x1 - x0).abs() + (y1 - y0).abs())
    }

    /// A filled circle of `radius` pixels around `at`.
    fn dot(&mut self, at: GridPoint, radius: f32, color: Rgb) -> Result<(), RenderError> {
        let (x, y) = self.px(at);
        let circle = PathBuilder::from_circle(x, y, radius).ok_or_else(|| internal("a needle hole"))?;
        self.meter.charge(1 + work(2.0 * radius))?;
        self.pixmap.fill_path(&circle, &paint(color), FillRule::Winding, Transform::identity(), None);
        Ok(())
    }

    /// A circle outline of `radius` pixels around `at`.
    fn ring(&mut self, at: GridPoint, radius: f32, pen: Pen) -> Result<(), RenderError> {
        let (x, y) = self.px(at);
        let circle = PathBuilder::from_circle(x, y, radius).ok_or_else(|| internal("a lock mark"))?;
        self.stroke(&circle, pen, 7.0 * radius)
    }

    /// Strokes `path`, which is about `length` pixels long, with `pen`.
    fn stroke(&mut self, path: &Path, pen: Pen, length: f32) -> Result<(), RenderError> {
        self.meter.charge(1 + work(length))?;
        let stroke = match pen.dash {
            None => Stroke { width: pen.width, line_cap: LineCap::Round, ..Stroke::default() },
            Some(on) => Stroke {
                width: pen.width,
                dash: Some(StrokeDash::new(vec![on, on * 0.6], 0.0).ok_or_else(|| internal("a dash pattern"))?),
                ..Stroke::default()
            },
        };
        self.pixmap.stroke_path(path, &paint(pen.color), &stroke, Transform::identity(), None);
        Ok(())
    }
}

/// Grey for travel after a cut.
const TRAVEL: Rgb = Rgb::from_hex(0x9AA0A6);
/// Orange rings around lock stitches.
const LOCK: Rgb = Rgb::from_hex(0xE07B00);
/// Red crosses at trims.
const TRIM: Rgb = Rgb::from_hex(0xD7263D);
/// Blue squares at stops.
const STOP: Rgb = Rgb::from_hex(0x1F5FBF);

/// Work units for drawing something `pixels` long: one per pixel, at least none.
fn work(pixels: f32) -> u64 {
    // Non-negative and bounded by the image size (at most a few times MAX_SIDE), so it fits.
    #[allow(clippy::cast_possible_truncation)]
    let units = pixels.max(0.0).ceil() as u64;
    units
}

/// `c` darkened to `percent` % of its brightness.
fn shade(c: Rgb, percent: u16) -> Rgb {
    let f = |v: u8| u8::try_from(u16::from(v) * percent / 100).unwrap_or(u8::MAX);
    Rgb::new(f(c.r), f(c.g), f(c.b))
}

/// `c` moved `percent` % of the way to white.
fn tint(c: Rgb, percent: u16) -> Rgb {
    let f = |v: u8| u8::try_from(u16::from(v) + (255 - u16::from(v)) * percent / 100).unwrap_or(u8::MAX);
    Rgb::new(f(c.r), f(c.g), f(c.b))
}

fn color(c: Rgb) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, 255)
}

fn paint(c: Rgb) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color(c));
    paint.anti_alias = true;
    paint
}

/// The numbers under the borders of a thickness picture's key: the fills where each step after the first
/// starts, by the limits [`Canvas::thickness`] colours with.
fn key_labels() -> [String; 3] {
    let limits = Limits::DEFAULT;
    [label(thickness::ORDINARY), label(limits.warn_fills), label(limits.fail_fills.max(limits.warn_fills))]
}

/// `fills` written as the key writes it: without a fraction when it is whole.
fn label(fills: f64) -> String {
    format!("{fills}")
}

fn internal(what: &str) -> RenderError {
    RenderError::Internal(format!("tiny-skia refused {what}"))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;

    fn g(x: i32, y: i32) -> GridPoint {
        GridPoint { x, y }
    }

    fn scene(points: &[(i32, i32)]) -> Scene {
        let holes = points
            .iter()
            .enumerate()
            .map(|(i, &(x, y))| Hole {
                at: g(x, y),
                color: Rgb::new(0, 0, 0),
                arrival: if i == 0 { Arrival::Cut } else { Arrival::Sewn },
                lock: false,
            })
            .collect();
        Scene { holes, ..Scene::default() }
    }

    #[test]
    fn styles_have_names() {
        for style in Style::ALL {
            assert_eq!(Style::from_name(style.name()), Some(*style));
        }
        assert_eq!(Style::from_name("photo"), None);
    }

    #[test]
    fn scales_are_checked() {
        assert!(Settings::new(Style::Simple, Settings::DEFAULT_SCALE).is_some());
        for bad in [0.0, 0.05, 50.5, f32::NAN, f32::INFINITY, -1.0] {
            assert_eq!(Settings::new(Style::Simple, bad), None, "{bad}");
        }
    }

    #[test]
    fn the_image_is_the_design_plus_margins() {
        let settings = Settings::new(Style::Simple, 10.0).unwrap();
        let image = scene(&[(0, 0), (100, 50)]).render(settings, &mut Budget::DEFAULT.meter()).unwrap();
        // 10 × 5 mm plus 2 mm on each side, at 10 pixels per millimetre.
        assert_eq!((image.width, image.height), (140, 90));
        assert!(image.png.starts_with(b"\x89PNG\r\n\x1a\n"));
    }

    #[test]
    fn oversized_previews_say_which_scale_fits() {
        let settings = Settings::new(Style::Realistic, 8.0).unwrap();
        let wide = scene(&[(-5000, 0), (5000, 0)]);
        let error = wide.render(settings, &mut Budget::DEFAULT.meter()).unwrap_err();
        assert_eq!(error, RenderError::TooLarge { width: 8032, height: 32, max: MAX_SIDE, largest_scale: 4.0 });
        let at_largest = Settings::new(Style::Realistic, 4.0).unwrap();
        assert!(wide.render(at_largest, &mut Budget::DEFAULT.meter()).is_ok());
        // The whole machine grid fits at the smallest scale.
        let everything = scene(&[(-100_000, -100_000), (100_000, 100_000)]);
        let smallest = Settings::new(Style::Simple, Settings::MIN_SCALE).unwrap();
        assert!(everything.render(smallest, &mut Budget::DEFAULT.meter()).is_ok());
    }

    #[test]
    fn empty_scenes_and_spent_budgets_are_errors() {
        let settings = Settings::new(Style::Simple, 8.0).unwrap();
        assert_eq!(Scene::default().render(settings, &mut Budget::DEFAULT.meter()), Err(RenderError::Empty));
        let small = Budget { max_stitches: 10, max_work: 50 };
        assert!(matches!(scene(&[(0, 0), (1000, 0)]).render(settings, &mut small.meter()), Err(RenderError::Budget(_))));
    }

    /// The colour of the pixel in column `x` and row `y` of a picture.
    fn pixel(image: &Image, x: u32, y: u32) -> Rgb {
        let colour = Pixmap::decode_png(&image.png).unwrap().pixel(x, y).unwrap();
        Rgb::new(colour.red(), colour.green(), colour.blue())
    }

    #[test]
    fn req_rnd_004_the_thickness_style_colours_the_thread_by_its_fills() {
        // Rows 0.2 mm apart are 1.25 fills. 40 mm of them, sewn again over the last 30, twice more over the
        // last 20 and once more over the last 10: 1.25, 2.5, 5 and 6.25 fills.
        let rows = |from: i32, to: i32| {
            (0..=50).flat_map(move |row| if row % 2 == 0 { [(from, row * 2), (to, row * 2)] } else { [(to, row * 2), (from, row * 2)] })
        };
        let layers = [(0, 400), (100, 400), (200, 400), (200, 400), (300, 400)];
        let points: Vec<(i32, i32)> = layers.into_iter().flat_map(|(from, to)| rows(from, to)).collect();
        let settings = Settings::new(Style::Thickness, 10.0).unwrap();
        let image = scene(&points).render(settings, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((image.width, image.height), (440, 140));
        // At 10 pixels per millimetre, with the margin of 2 mm: 5, 15, 25 and 35 mm along the rows, 5 mm down.
        // 5.5 mm down, where the edges of a spot's window lie between the rows' lanes.
        let colours = [70, 170, 270, 370].map(|x| pixel(&image, x, 75));
        assert_eq!(colours, [ORDINARY, DOUBLED, THICK, FAILING]);
        // The thread reaches a lane before the first hole, into the cell before it, which is in the window
        // of the spot half a millimetre before the hole. That spot answers for the places up to 0.75 mm
        // before the hole, and the middle of pixel 12 lies there: the spot under a pixel's middle decides.
        assert_eq!((pixel(&image, 11, 75), pixel(&image, 12, 75)), (Rgb::new(255, 255, 255), ORDINARY));
        assert_eq!((pixel(&image, 75, 11), pixel(&image, 75, 12)), (Rgb::new(255, 255, 255), ORDINARY));
        assert_eq!((pixel(&image, 5, 75), pixel(&image, 70, 135)), (Rgb::new(255, 255, 255), Rgb::new(255, 255, 255)));
        // The fabric's colour shows where no thread lies.
        let black = scene(&points).render(settings.on(Rgb::new(0, 0, 0)), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((pixel(&black, 5, 75), pixel(&black, 70, 75)), (Rgb::new(0, 0, 0), ORDINARY));
        // A scene that sews no stitch is its fabric, and the picture needs a unit of work for each pixel.
        let mut loose = scene(&[(0, 0), (100, 50)]);
        loose.holes[1].arrival = Arrival::Loose;
        let image = loose.render(settings, &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!((pixel(&image, 70, 45), image.width, image.height), (Rgb::new(255, 255, 255), 140, 90));
        let short = Budget { max_stitches: 10, max_work: 140 * 90 + 500 };
        assert!(matches!(scene(&[(0, 0), (100, 50)]).render(settings, &mut short.meter()), Err(RenderError::Budget(_))));
    }

    #[test]
    fn req_rnd_005_guides_are_drawn_thin_and_grey_under_the_stitches() {
        let bare = scene(&[(0, 0), (100, 0)]);
        // A guide 5 mm below the stitch, and one across it.
        let guided = Scene { guides: vec![vec![g(0, 50), g(100, 50)], vec![g(50, -10), g(50, 50)]], ..bare.clone() };
        for style in Style::ALL {
            let settings = Settings::new(*style, 10.0).unwrap();
            let (plain, image) =
                (bare.render(settings, &mut Budget::DEFAULT.meter()).unwrap(), guided.render(settings, &mut Budget::DEFAULT.meter()).unwrap());
            // The picture grows to hold the guides: 10 × 6 mm plus the margins, from 1 mm above the stitch.
            assert_eq!((plain.width, plain.height, image.width, image.height), (140, 40, 140, 100), "{style:?}");
            // The guide, along the 2 rows or columns of pixels its middle lies between, is drawn; on white, in grey.
            let fabric = pixel(&image, 30, 5);
            assert_eq!(fabric, settings.fabric(), "{style:?}");
            for (x, y) in [(30, 79), (30, 80), (69, 50), (70, 50)] {
                let guide = pixel(&image, x, y);
                assert_ne!(guide, fabric, "{style:?} {x},{y}");
                if *style != Style::Realistic {
                    assert!(guide.r == guide.g && guide.g == guide.b && guide.r >= GUIDE.r, "{style:?} {x},{y}: {guide:?}");
                }
            }
        }
        // Under the stitches: where the guide crosses the thread, the thread's own pixels show.
        let realistic = Settings::new(Style::Realistic, 10.0).unwrap();
        let (plain, image) =
            (bare.render(realistic, &mut Budget::DEFAULT.meter()).unwrap(), guided.render(realistic, &mut Budget::DEFAULT.meter()).unwrap());
        assert_eq!(pixel(&image, 70, 30), pixel(&plain, 70, 20));
        // A scene of a plan has none, and a scene of guides alone is still empty.
        assert!(
            Scene::of(&stitchcraft_plan::StitchPlan::default(), stitchcraft_plan::TrimSupport::None, &mut Budget::DEFAULT.meter())
                .unwrap()
                .guides
                .is_empty()
        );
        let only = Scene { guides: vec![vec![g(0, 0), g(10, 10)]], ..Scene::default() };
        let settings = Settings::new(Style::Simple, 8.0).unwrap();
        assert_eq!(only.render(settings, &mut Budget::DEFAULT.meter()), Err(RenderError::Empty));
        // Each guide line is work.
        let lines: Vec<Vec<GridPoint>> = (0..100).map(|_| vec![g(0, 0), g(100, 0)]).collect();
        let small = Budget { max_stitches: 10, max_work: 3000 };
        assert!(matches!(Scene { guides: lines, ..bare }.render(settings, &mut small.meter()), Err(RenderError::Budget(_))));
    }

    #[test]
    fn req_rnd_006_a_thickness_picture_s_key_shows_its_steps_and_their_limits() {
        assert!(Style::ALL.iter().all(|style| Settings::new(*style, 8.0).unwrap().keyed().is_some() == (*style == Style::Thickness)));
        // The numbers under the borders are where the steps start, by the limits the style draws with.
        assert_eq!(key_labels(), ["2", "4", "6"]);
        assert_eq!(label(2.5), "2.5");
        let points: Vec<(i32, i32)> = (0..=25).flat_map(|row| [(0, row * 2), (400, row * 2)]).collect();
        let settings = Settings::new(Style::Thickness, 10.0).unwrap();
        let plain = scene(&points).render(settings, &mut Budget::DEFAULT.meter()).unwrap();
        let keyed = scene(&points).render(settings.keyed().unwrap(), &mut Budget::DEFAULT.meter()).unwrap();
        // The key is a strip under the picture, as wide as it, and the picture above it doesn't change.
        assert_eq!(keyed.width, plain.width);
        assert_eq!(f64::from(keyed.height - plain.height), f64::from(KEY_MM * 10.0).ceil());
        for y in [0, 30, plain.height - 1] {
            assert_eq!(pixel(&keyed, 70, y), pixel(&plain, 70, y));
        }
        // 4 boxes from margin to margin, 100 pixels each, in the steps' colours.
        let boxes = plain.height + 20;
        assert_eq!([70, 170, 270, 370].map(|x| pixel(&keyed, x, boxes)), [ORDINARY, DOUBLED, THICK, FAILING]);
        // A number under each border between 2 boxes: its digit's middle bar, drawn in ink, and fabric beside it.
        let digits = plain.height + 62;
        for border in [120, 220, 320] {
            assert_eq!(pixel(&keyed, border, digits), INK, "{border}");
            assert_eq!(pixel(&keyed, border + 20, digits), Rgb::new(255, 255, 255), "{border}");
        }
        // The key on black fabric is on black too.
        let black = scene(&points).render(settings.keyed().unwrap().on(Rgb::new(0, 0, 0)), &mut Budget::DEFAULT.meter()).unwrap();
        assert_eq!(pixel(&black, 145, plain.height + 2), Rgb::new(0, 0, 0));
    }

    #[test]
    fn shades_and_tints_stay_in_range() {
        assert_eq!(shade(Rgb::new(255, 100, 0), 60), Rgb::new(153, 60, 0));
        assert_eq!(tint(Rgb::new(255, 100, 0), 50), Rgb::new(255, 177, 127));
    }
}
