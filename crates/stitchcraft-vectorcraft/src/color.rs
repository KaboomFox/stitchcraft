//! VectorCraft's colours as thread colours.
//!
//! **Why VectorCraft's own formulas.** A thread should have the colour the drawing shows. VectorCraft draws
//! RGB colours as they are and, without colour management, CMYK as (1 − c)(1 − k), (1 − m)(1 − k),
//! (1 − y)(1 − k), and grey as 1 − k of each primary, k being the amount of ink. Lab colours are relative
//! to D50, as in ICC profiles, and reach sRGB through the matrix that adapts D50 to sRGB's white. These are
//! VectorCraft's formulas without its colour settings (`Color::to_rgb_uncalibrated` in its `crates/color`),
//! the standard ones, and the thread palette is matched later, so profiles would change nothing a machine
//! sews. The maths is `stitchcraft_core::math`'s, the same on every platform.

use stitchcraft_core::math;
use stitchcraft_plan::Rgb;

use crate::wire::{Color, Model};

/// D50's white, X and Z for Y = 1: Lab's reference white.
const D50: [f64; 2] = [0.964_22, 0.825_21];
/// CIE's ε and κ: where Lab's cube root gives way to a straight line.
const EPSILON: f64 = 216.0 / 24_389.0;
const KAPPA: f64 = 24_389.0 / 27.0;
/// XYZ relative to D50 into linear sRGB, by way of the Bradford adaptation to D65.
const XYZ_TO_SRGB: [[f64; 3]; 3] =
    [[3.133_856_1, -1.616_866_7, -0.490_614_6], [-0.978_768_4, 1.916_141_5, 0.033_454_0], [0.071_945_3, -0.228_991_4, 1.405_242_7]];

/// The 8-bit sRGB colour `color` is drawn in, or `None` when a component it needs is not a number, or its
/// model is one the adapter doesn't read.
pub(crate) fn rgb(color: &Color) -> Option<Rgb> {
    let [r, g, b] = match color.model {
        Model::Rgb => [color.r?, color.g?, color.b?],
        Model::Cmyk => {
            let (c, m, y, k) = (color.c?, color.m?, color.y?, color.k?);
            [(1.0 - c) * (1.0 - k), (1.0 - m) * (1.0 - k), (1.0 - y) * (1.0 - k)]
        }
        Model::Gray => [1.0 - color.k?; 3],
        Model::Lab => lab_to_srgb(color.l?, color.a?, color.b?),
        Model::Other => return None,
    };
    Some(Rgb::new(byte(r)?, byte(g)?, byte(b)?))
}

/// A component from 0 to 1 as a byte, as VectorCraft rounds it; `None` when it is not a number.
// A whole number from 0 to 255 after the clamp and the rounding, so the cast is exact.
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn byte(v: f64) -> Option<u8> {
    v.is_finite().then(|| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

/// CIE L*a*b* relative to D50 as sRGB components, clipped to 0 to 1.
fn lab_to_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let fy = (l + 16.0) / 116.0;
    let (fx, fz) = (fy + a / 500.0, fy - b / 200.0);
    let inverse = |f: f64| {
        let cube = f * f * f;
        if cube > EPSILON { cube } else { (116.0 * f - 16.0) / KAPPA }
    };
    let y = if l > KAPPA * EPSILON { fy * fy * fy } else { l / KAPPA };
    // The white's Y is 1, so Y needs no scaling.
    let xyz = [inverse(fx) * D50[0], y, inverse(fz) * D50[1]];
    XYZ_TO_SRGB.map(|row| encoded(row[0] * xyz[0] + row[1] * xyz[1] + row[2] * xyz[2]))
}

/// sRGB's transfer curve, from linear light to the encoded value, clipped to 0 to 1.
fn encoded(linear: f64) -> f64 {
    let v = linear.clamp(0.0, 1.0);
    if v <= 0.003_130_8 { v * 12.92 } else { 1.055 * math::pow(v, 1.0 / 2.4) - 0.055 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn color(model: Model, values: &[(&str, f64)]) -> Color {
        let mut c = Color { model, r: None, g: None, b: None, c: None, m: None, y: None, k: None, l: None, a: None };
        for (name, v) in values {
            let slot = match *name {
                "r" => &mut c.r,
                "g" => &mut c.g,
                "b" => &mut c.b,
                "c" => &mut c.c,
                "m" => &mut c.m,
                "y" => &mut c.y,
                "k" => &mut c.k,
                "l" => &mut c.l,
                _ => &mut c.a,
            };
            *slot = Some(*v);
        }
        c
    }

    #[test]
    fn each_model_gives_the_colour_vectorcraft_draws() {
        assert_eq!(rgb(&color(Model::Rgb, &[("r", 1.0), ("g", 0.5), ("b", 0.0)])), Some(Rgb::new(255, 128, 0)));
        // Out of range and clamped, as VectorCraft clamps it.
        assert_eq!(rgb(&color(Model::Rgb, &[("r", 1.5), ("g", -0.2), ("b", 0.2)])), Some(Rgb::new(255, 0, 51)));
        assert_eq!(rgb(&color(Model::Cmyk, &[("c", 0.0), ("m", 1.0), ("y", 1.0), ("k", 0.0)])), Some(Rgb::new(255, 0, 0)));
        assert_eq!(rgb(&color(Model::Cmyk, &[("c", 0.5), ("m", 0.0), ("y", 0.0), ("k", 0.5)])), Some(Rgb::new(64, 128, 128)));
        assert_eq!(rgb(&color(Model::Gray, &[("k", 0.25)])), Some(Rgb::new(191, 191, 191)));
        // D50's white and black, and the Lab coordinates of sRGB's red and of a mid grey.
        assert_eq!(rgb(&color(Model::Lab, &[("l", 100.0), ("a", 0.0), ("b", 0.0)])), Some(Rgb::new(255, 255, 255)));
        assert_eq!(rgb(&color(Model::Lab, &[("l", 0.0), ("a", 0.0), ("b", 0.0)])), Some(Rgb::new(0, 0, 0)));
        assert_eq!(rgb(&color(Model::Lab, &[("l", 54.29), ("a", 80.80), ("b", 69.89)])), Some(Rgb::new(255, 0, 0)));
        assert_eq!(rgb(&color(Model::Lab, &[("l", 53.39), ("a", 0.0), ("b", 0.0)])), Some(Rgb::new(128, 128, 128)));
        // Dark greys on the straight parts of Lab's curve, and of sRGB's for the first, and colours inside
        // sRGB's gamut, two with a* or b* on the straight part. colour-science 0.4.7's Lab to sRGB, through
        // Bradford's adaptation, gives the same bytes.
        let lab = |l: f64, a: f64, b: f64| rgb(&color(Model::Lab, &[("l", l), ("a", a), ("b", b)]));
        assert_eq!(lab(2.0, 0.0, 0.0), Some(Rgb::new(7, 7, 7)));
        assert_eq!(lab(5.0, 0.0, 0.0), Some(Rgb::new(17, 17, 17)));
        assert_eq!(lab(50.0, 20.0, -30.0), Some(Rgb::new(133, 108, 170)));
        assert_eq!(lab(60.0, 10.0, 100.0), Some(Rgb::new(183, 137, 0)));
        assert_eq!(lab(30.0, -100.0, 0.0), Some(Rgb::new(0, 95, 68)));
    }

    #[test]
    fn a_colour_without_its_components_or_model_has_none() {
        assert_eq!(rgb(&color(Model::Rgb, &[("r", 1.0), ("g", 0.5)])), None);
        assert_eq!(rgb(&color(Model::Rgb, &[("r", f64::NAN), ("g", 0.5), ("b", 0.0)])), None);
        assert_eq!(rgb(&color(Model::Cmyk, &[("c", 0.0), ("m", 1.0), ("y", 1.0)])), None);
        assert_eq!(rgb(&color(Model::Gray, &[])), None);
        assert_eq!(rgb(&color(Model::Lab, &[("l", 50.0), ("a", 0.0)])), None);
        assert_eq!(rgb(&color(Model::Other, &[("r", 1.0), ("g", 1.0), ("b", 1.0)])), None);
    }
}
