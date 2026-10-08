//! Deterministic floating-point math.
//!
//! Basic IEEE-754 arithmetic (`+ - * /`, `sqrt`) gives identical results everywhere, but `sin`, `cos`,
//! `atan2`, `exp` and friends come from the platform's C library, and those differ in the last bits
//! between operating systems. A one-ulp difference can move a stitch across a 0.1 mm rounding boundary
//! and change a machine file. Every transcendental function in StitchCraft therefore goes through this
//! module, which uses the pure-Rust `libm` crate on every target. `clippy.toml` forbids the `f64`
//! methods elsewhere (`docs/src/design/determinism.md`).

/// Sine of `x` radians.
pub fn sin(x: f64) -> f64 {
    libm::sin(x)
}

/// Cosine of `x` radians.
pub fn cos(x: f64) -> f64 {
    libm::cos(x)
}

/// Sine and cosine of `x` radians, computed together.
pub fn sin_cos(x: f64) -> (f64, f64) {
    libm::sincos(x)
}

/// Tangent of `x` radians.
pub fn tan(x: f64) -> f64 {
    libm::tan(x)
}

/// Four-quadrant arctangent of `y / x`, in radians.
pub fn atan2(y: f64, x: f64) -> f64 {
    libm::atan2(y, x)
}

/// `sqrt(x² + y²)` without intermediate overflow.
pub fn hypot(x: f64, y: f64) -> f64 {
    libm::hypot(x, y)
}

/// Converts degrees to radians.
pub fn to_radians(degrees: f64) -> f64 {
    degrees * (core::f64::consts::PI / 180.0)
}

/// Converts radians to degrees.
pub fn to_degrees(radians: f64) -> f64 {
    radians * (180.0 / core::f64::consts::PI)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference bits recorded from `libm`. If a dependency upgrade changes any of them, every machine
    /// file and golden image could change: this test makes that a deliberate, reviewed decision.
    #[test]
    fn results_are_frozen_bit_for_bit() {
        let cases: [(&str, f64, u64); 5] = [
            ("sin(1)", sin(1.0), 0x3FEA_ED54_8F09_0CEE),
            ("cos(1)", cos(1.0), 0x3FE1_4A28_0FB5_068C),
            ("tan(0.5)", tan(0.5), 0x3FE1_7B4F_5BF3_474A),
            ("atan2(1,2)", atan2(1.0, 2.0), 0x3FDD_AC67_0561_BB4F),
            ("hypot(3,4)", hypot(3.0, 4.0), 0x4014_0000_0000_0000),
        ];
        for (name, value, bits) in cases {
            assert_eq!(value.to_bits(), bits, "{name} = {value:e} changed: {:#018X}", value.to_bits());
        }
    }

    #[test]
    fn sin_cos_matches_the_separate_functions() {
        for x in [-3.0, -0.25, 0.0, 0.7, 2.0, 100.0] {
            assert_eq!(sin_cos(x), (sin(x), cos(x)));
        }
    }

    #[test]
    fn degree_conversions_round_trip() {
        for d in [-180.0, -45.0, 0.0, 30.0, 90.0, 359.0] {
            assert!((to_degrees(to_radians(d)) - d).abs() < 1e-12);
        }
    }
}
