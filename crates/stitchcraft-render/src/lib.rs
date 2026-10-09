//! Deterministic CPU previews of stitch plans (layer L2).
//!
//! A preview shows what the machine will sew, not what the plan meant. Every position is rounded to the
//! 0.1 mm grid with the writers' own rounding before anything is drawn, so a stitch never "moves"
//! between the preview and the sew-out (REQ-RND-001). Drawing is two
//! steps, each tested on its own:
//!
//! 1. [`Scene::of`] walks the plan once and lists what the fabric will show — needle holes, how the
//!    thread got to each (sewn, carried loose, or cut), trims and stops — in machine units
//!    ([`scene`]). Tests compare scenes, which says *what* differs, instead of pixels.
//! 2. [`Scene::render`] draws a scene in one of two [`Style`]s and encodes it as PNG ([`raster`]),
//!    byte-identical on every platform (REQ-RND-002), so documentation images are compared byte for
//!    byte (`cargo xtask shots --check`).
//!
//! [`preview`] does both. Every loop charges the caller's [`Meter`], and every failure is a
//! [`RenderError`] with a registered diagnostic code. Design: `docs/src/design/rendering.md`.
#![forbid(unsafe_code)]

pub mod error;
pub mod raster;
pub mod scene;

pub use error::RenderError;
pub use raster::{Image, MAX_SIDE, Settings, Style};
pub use scene::{Arrival, GridPoint, Hole, Mark, Scene};
use stitchcraft_core::Meter;
use stitchcraft_plan::StitchPlan;

/// The preview of `plan`: [`Scene::of`], then [`Scene::render`].
pub fn preview(plan: &StitchPlan, settings: Settings, meter: &mut Meter) -> Result<Image, RenderError> {
    Scene::of(plan, meter)?.render(settings, meter)
}
