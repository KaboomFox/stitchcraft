//! Colour difference for thread matching: sRGB → CIELAB and CIEDE2000.
//!
//! Formats that store palette indices need "the palette colour that looks most like this one". Distance
//! in RGB is a poor answer (it overrates differences between blues and underrates them between greens),
//! so StitchCraft uses CIEDE2000 (CIE 142-2001), the colour-difference formula built to match what people
//! see, in CIELAB with the D65 white point.
//!
//! The implementation follows Sharma, Wu & Dalal, "The CIEDE2000 Color-Difference Formula:
//! Implementation Notes, Supplementary Test Data, and Mathematical Observations" (Color Research &
//! Application 30(1), 2005), and is tested against that paper's 34 reference pairs. Every
//! transcendental function goes through `stitchcraft_core::math`, so matches are identical on every
//! platform (REQ-THREAD-001).

use stitchcraft_core::math;

use crate::thread::Rgb;

/// A colour in CIELAB: lightness `l` (0–100), green–red `a`, blue–yellow `b`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lab {
    /// Lightness.
    pub l: f64,
    /// Green (negative) to red (positive).
    pub a: f64,
    /// Blue (negative) to yellow (positive).
    pub b: f64,
}

/// The D65 reference white (CIE 1931 2° observer), Y normalized to 1.
const WHITE: [f64; 3] = [0.950_47, 1.0, 1.088_83];

impl Lab {
    /// The CIELAB coordinates of an sRGB colour (IEC 61966-2-1 transfer function, D65).
    pub fn from_rgb(color: Rgb) -> Lab {
        let [r, g, b] = [color.r, color.g, color.b].map(linear);
        let x = 0.412_456_4 * r + 0.357_576_1 * g + 0.180_437_5 * b;
        let y = 0.212_672_9 * r + 0.715_152_2 * g + 0.072_175_0 * b;
        let z = 0.019_333_9 * r + 0.119_192_0 * g + 0.950_304_1 * b;
        let [fx, fy, fz] = [x / WHITE[0], y / WHITE[1], z / WHITE[2]].map(lab_f);
        Lab { l: 116.0 * fy - 16.0, a: 500.0 * (fx - fy), b: 200.0 * (fy - fz) }
    }
}

/// sRGB channel (0–255) to linear light (0–1).
fn linear(channel: u8) -> f64 {
    let c = f64::from(channel) / 255.0;
    if c <= 0.040_45 { c / 12.92 } else { math::pow((c + 0.055) / 1.055, 2.4) }
}

/// The CIELAB companding function.
fn lab_f(t: f64) -> f64 {
    const DELTA: f64 = 6.0 / 29.0;
    if t > DELTA * DELTA * DELTA { math::cbrt(t) } else { t / (3.0 * DELTA * DELTA) + 4.0 / 29.0 }
}

/// `x⁷` by multiplication (no `powf`, so no platform differences).
fn pow7(x: f64) -> f64 {
    let x2 = x * x;
    x2 * x2 * x2 * x
}

/// Hue angle in degrees, 0–360; 0 for a neutral colour.
fn hue(b: f64, a: f64) -> f64 {
    if a == 0.0 && b == 0.0 {
        return 0.0;
    }
    let h = math::to_degrees(math::atan2(b, a));
    if h < 0.0 { h + 360.0 } else { h }
}

/// The CIEDE2000 colour difference between two colours (kL = kC = kH = 1). About 1 is a just-noticeable
/// difference; thread palettes are typically 5–20 apart.
pub fn ciede2000(x: Lab, y: Lab) -> f64 {
    let c_mean = (math::hypot(x.a, x.b) + math::hypot(y.a, y.b)) / 2.0;
    let c7 = pow7(c_mean);
    let g = 0.5 * (1.0 - (c7 / (c7 + pow7(25.0))).sqrt());
    let (a1, a2) = ((1.0 + g) * x.a, (1.0 + g) * y.a);
    let (c1, c2) = (math::hypot(a1, x.b), math::hypot(a2, y.b));
    let (h1, h2) = (hue(x.b, a1), hue(y.b, a2));

    let dl = y.l - x.l;
    let dc = c2 - c1;
    let dh_angle = if c1 * c2 == 0.0 {
        0.0
    } else {
        let d = h2 - h1;
        if d.abs() <= 180.0 {
            d
        } else if d > 180.0 {
            d - 360.0
        } else {
            d + 360.0
        }
    };
    let dh = 2.0 * (c1 * c2).sqrt() * math::sin(math::to_radians(dh_angle / 2.0));

    let l_mean = (x.l + y.l) / 2.0;
    let c_mean_p = (c1 + c2) / 2.0;
    let h_mean = if c1 * c2 == 0.0 {
        h1 + h2
    } else if (h1 - h2).abs() <= 180.0 {
        (h1 + h2) / 2.0
    } else if h1 + h2 < 360.0 {
        (h1 + h2 + 360.0) / 2.0
    } else {
        (h1 + h2 - 360.0) / 2.0
    };
    let cos_deg = |degrees: f64| math::cos(math::to_radians(degrees));
    let t =
        1.0 - 0.17 * cos_deg(h_mean - 30.0) + 0.24 * cos_deg(2.0 * h_mean) + 0.32 * cos_deg(3.0 * h_mean + 6.0) - 0.20 * cos_deg(4.0 * h_mean - 63.0);
    let d_theta = 30.0 * math::exp(-((h_mean - 275.0) / 25.0) * ((h_mean - 275.0) / 25.0));
    let c7p = pow7(c_mean_p);
    let r_c = 2.0 * (c7p / (c7p + pow7(25.0))).sqrt();
    let l50 = (l_mean - 50.0) * (l_mean - 50.0);
    let s_l = 1.0 + 0.015 * l50 / (20.0 + l50).sqrt();
    let s_c = 1.0 + 0.045 * c_mean_p;
    let s_h = 1.0 + 0.015 * c_mean_p * t;
    let r_t = -math::sin(math::to_radians(2.0 * d_theta)) * r_c;

    let (tl, tc, th) = (dl / s_l, dc / s_c, dh / s_h);
    (tl * tl + tc * tc + th * th + r_t * tc * th).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lab(l: f64, a: f64, b: f64) -> Lab {
        Lab { l, a, b }
    }

    /// The 34 reference pairs of Sharma, Wu & Dalal (2005), Table 1, with ΔE00 to four decimals.
    #[test]
    fn matches_the_sharma_reference_data() {
        #[rustfmt::skip]
        let pairs: [([f64; 3], [f64; 3], f64); 34] = [
            ([50.0, 2.6772, -79.7751], [50.0, 0.0, -82.7485], 2.0425),
            ([50.0, 3.1571, -77.2803], [50.0, 0.0, -82.7485], 2.8615),
            ([50.0, 2.8361, -74.0200], [50.0, 0.0, -82.7485], 3.4412),
            ([50.0, -1.3802, -84.2814], [50.0, 0.0, -82.7485], 1.0000),
            ([50.0, -1.1848, -84.8006], [50.0, 0.0, -82.7485], 1.0000),
            ([50.0, -0.9009, -85.5211], [50.0, 0.0, -82.7485], 1.0000),
            ([50.0, 0.0, 0.0], [50.0, -1.0, 2.0], 2.3669),
            ([50.0, -1.0, 2.0], [50.0, 0.0, 0.0], 2.3669),
            ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0009], 7.1792),
            ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0010], 7.1792),
            ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0011], 7.2195),
            ([50.0, 2.4900, -0.0010], [50.0, -2.4900, 0.0012], 7.2195),
            ([50.0, -0.0010, 2.4900], [50.0, 0.0009, -2.4900], 4.8045),
            ([50.0, -0.0010, 2.4900], [50.0, 0.0010, -2.4900], 4.8045),
            ([50.0, -0.0010, 2.4900], [50.0, 0.0011, -2.4900], 4.7461),
            ([50.0, 2.5, 0.0], [50.0, 0.0, -2.5], 4.3065),
            ([50.0, 2.5, 0.0], [73.0, 25.0, -18.0], 27.1492),
            ([50.0, 2.5, 0.0], [61.0, -5.0, 29.0], 22.8977),
            ([50.0, 2.5, 0.0], [56.0, -27.0, -3.0], 31.9030),
            ([50.0, 2.5, 0.0], [58.0, 24.0, 15.0], 19.4535),
            ([50.0, 2.5, 0.0], [50.0, 3.1736, 0.5854], 1.0000),
            ([50.0, 2.5, 0.0], [50.0, 3.2972, 0.0], 1.0000),
            ([50.0, 2.5, 0.0], [50.0, 1.8634, 0.5757], 1.0000),
            ([50.0, 2.5, 0.0], [50.0, 3.2592, 0.3350], 1.0000),
            ([60.2574, -34.0099, 36.2677], [60.4626, -34.1751, 39.4387], 1.2644),
            ([63.0109, -31.0961, -5.8663], [62.8187, -29.7946, -4.0864], 1.2630),
            ([61.2901, 3.7196, -5.3901], [61.4292, 2.2480, -4.9620], 1.8731),
            ([35.0831, -44.1164, 3.7933], [35.0232, -40.0716, 1.5901], 1.8645),
            ([22.7233, 20.0904, -46.6940], [23.0331, 14.9730, -42.5619], 2.0373),
            ([36.4612, 47.8580, 18.3852], [36.2715, 50.5065, 21.2231], 1.4146),
            ([90.8027, -2.0831, 1.4410], [91.1528, -1.6435, 0.0447], 1.4441),
            ([90.9257, -0.5406, -0.9208], [88.6381, -0.8985, -0.7239], 1.5381),
            ([6.7747, -0.2908, -2.4247], [5.8714, -0.0985, -2.2286], 0.6377),
            ([2.0776, 0.0795, -1.1350], [0.9033, -0.0636, -0.5514], 0.9082),
        ];
        for (i, (x, y, expected)) in pairs.iter().enumerate() {
            let d = ciede2000(lab(x[0], x[1], x[2]), lab(y[0], y[1], y[2]));
            assert!((d - expected).abs() < 5e-5, "pair {}: got {d:.6}, expected {expected}", i + 1);
            let back = ciede2000(lab(y[0], y[1], y[2]), lab(x[0], x[1], x[2]));
            assert!((back - d).abs() < 1e-9, "pair {}: not symmetric", i + 1);
        }
    }

    #[test]
    fn srgb_white_black_and_primaries_land_where_cielab_puts_them() {
        let white = Lab::from_rgb(Rgb::new(255, 255, 255));
        assert!((white.l - 100.0).abs() < 1e-3 && white.a.abs() < 1e-2 && white.b.abs() < 1e-2, "{white:?}");
        assert_eq!(Lab::from_rgb(Rgb::new(0, 0, 0)), Lab { l: 0.0, a: 0.0, b: 0.0 });
        // Published sRGB red in CIELAB (D65): L 53.24, a 80.09, b 67.20.
        let red = Lab::from_rgb(Rgb::new(255, 0, 0));
        assert!((red.l - 53.24).abs() < 0.01 && (red.a - 80.09).abs() < 0.02 && (red.b - 67.20).abs() < 0.02, "{red:?}");
    }

    #[test]
    fn identical_colours_have_no_difference() {
        let c = Lab::from_rgb(Rgb::new(10, 85, 163));
        assert_eq!(ciede2000(c, c), 0.0);
    }
}
