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
//! **Determinism.** tiny-skia is built without its SIMD feature, so its pipelines run the same scalar
//! code on every CPU. Only straight lines and circles are drawn: circles and round caps are conics,
//! which tiny-skia flattens with arithmetic and square roots (correctly rounded everywhere), so nothing
//! reaches the cubic root-finding or rotations that use the platform's trigonometry. Grid positions are
//! integers below 2^24, converted to `f32` exactly. The PNG encoder (png, with miniz_oxide) is pure
//! Rust and writes no timestamps. So a scene and its settings give the same bytes everywhere; the golden
//! PNG files check it on Linux, macOS and Windows (REQ-RND-002).

use stitchcraft_core::Meter;
use stitchcraft_plan::Rgb;
use tiny_skia::{Color, FillRule, LineCap, Paint, Path, PathBuilder, Pixmap, Rect, Stroke, StrokeDash, Transform};

use crate::error::RenderError;
use crate::scene::{Arrival, GridPoint, Mark, Scene};

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
}

impl Style {
    /// Every style.
    pub const ALL: &'static [Style] = &[Style::Realistic, Style::Simple];

    /// The style's name, as the command line and `docs/shots.toml` spell it.
    pub const fn name(self) -> &'static str {
        match self {
            Style::Realistic => "realistic",
            Style::Simple => "simple",
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
            Style::Simple => Rgb::from_hex(0xFFFFFF),
        }
    }
}

/// A style and a scale, checked.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    style: Style,
    scale: f32,
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
        (Self::MIN_SCALE..=Self::MAX_SCALE).contains(&scale).then_some(Settings { style, scale })
    }

    /// The style.
    pub const fn style(self) -> Style {
        self.style
    }

    /// Pixels per millimetre.
    pub const fn scale(self) -> f32 {
        self.scale
    }
}

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
        let (width, height) = image_size(min, max, settings.scale)?;
        let pixmap = Pixmap::new(width, height).ok_or_else(|| internal("no canvas of that size"))?;
        let mut canvas = Canvas { pixmap, min, mm: settings.scale, meter };
        canvas.pixmap.fill(color(settings.style.background()));
        match settings.style {
            Style::Realistic => canvas.realistic(self)?,
            Style::Simple => canvas.simple(self)?,
        }
        let png = canvas.pixmap.encode_png().map_err(|e| internal(&format!("the PNG encoder failed: {e}")))?;
        Ok(Image { width, height, png })
    }
}

/// The image size for a design from `min` to `max` at `scale`, or why there is none.
fn image_size(min: GridPoint, max: GridPoint, scale: f32) -> Result<(u32, u32), RenderError> {
    let millimetres = |lo: i32, hi: i32| (f64::from(hi) - f64::from(lo)) / 10.0 + 2.0 * f64::from(MARGIN_MM);
    let (across, down) = (millimetres(min.x, max.x), millimetres(min.y, max.y));
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

fn internal(what: &str) -> RenderError {
    RenderError::Internal(format!("tiny-skia refused {what}"))
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;

    use super::*;
    use crate::scene::Hole;

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
        Scene { holes, marks: Default::default() }
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

    #[test]
    fn shades_and_tints_stay_in_range() {
        assert_eq!(shade(Rgb::new(255, 100, 0), 60), Rgb::new(153, 60, 0));
        assert_eq!(tint(Rgb::new(255, 100, 0), 50), Rgb::new(255, 177, 127));
    }
}
