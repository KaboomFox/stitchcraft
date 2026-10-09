//! Affine maps: SVG's `transform` attributes, and the root's mapping from user units to millimetres.
//!
//! svgtypes parses the transform lists, but the matrices are built here: its own `Transform` uses the
//! platform's sine, cosine and tangent, which differ in the last bits between operating systems, and
//! StitchCraft's output must be the same everywhere (`docs/src/design/determinism.md`). Rotations and
//! skews therefore use `stitchcraft_core::math`.

use stitchcraft_core::math;
use stitchcraft_core::units::{MM_PER_INCH, MM_PER_SVG_PX};
use svgtypes::{Align, AspectRatio, Length, LengthUnit, TransformListParser, TransformListToken, ViewBox};

/// `x' = a·x + c·y + e`, `y' = b·x + d·y + f`: SVG's `matrix(a b c d e f)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Affine {
    /// The map that changes nothing.
    pub const IDENTITY: Affine = Affine { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    /// A move by (`tx`, `ty`).
    pub const fn translate(tx: f64, ty: f64) -> Affine {
        Affine { e: tx, f: ty, ..Affine::IDENTITY }
    }

    /// A scale by `sx` across and `sy` down.
    pub const fn scale(sx: f64, sy: f64) -> Affine {
        Affine { a: sx, d: sy, ..Affine::IDENTITY }
    }

    /// A rotation by `degrees`, clockwise on screen (y points down).
    pub fn rotate(degrees: f64) -> Affine {
        let (sin, cos) = math::sin_cos(math::to_radians(degrees));
        Affine { a: cos, b: sin, c: -sin, d: cos, e: 0.0, f: 0.0 }
    }

    /// A skew along x by `degrees`.
    pub fn skew_x(degrees: f64) -> Affine {
        Affine { c: math::tan(math::to_radians(degrees)), ..Affine::IDENTITY }
    }

    /// A skew along y by `degrees`.
    pub fn skew_y(degrees: f64) -> Affine {
        Affine { b: math::tan(math::to_radians(degrees)), ..Affine::IDENTITY }
    }

    /// This map applied after `inner`: `(self × inner)(p) = self(inner(p))`.
    pub fn after(self, inner: Affine) -> Affine {
        let (l, r) = (self, inner);
        Affine {
            a: l.a * r.a + l.c * r.b,
            b: l.b * r.a + l.d * r.b,
            c: l.a * r.c + l.c * r.d,
            d: l.b * r.c + l.d * r.d,
            e: l.a * r.e + l.c * r.f + l.e,
            f: l.b * r.e + l.d * r.f + l.f,
        }
    }

    /// Where the map takes (`x`, `y`).
    pub fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    /// Whether every coefficient is a finite number.
    pub fn is_finite(self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f].iter().all(|v| v.is_finite())
    }
}

/// The transform list in `text`, as one map; the reason when `text` is not a transform list.
pub fn parse(text: &str) -> Result<Affine, String> {
    let mut map = Affine::IDENTITY;
    for token in TransformListParser::from(text) {
        let next = match token.map_err(|e| e.to_string())? {
            TransformListToken::Matrix { a, b, c, d, e, f } => Affine { a, b, c, d, e, f },
            TransformListToken::Translate { tx, ty } => Affine::translate(tx, ty),
            TransformListToken::Scale { sx, sy } => Affine::scale(sx, sy),
            TransformListToken::Rotate { angle } => Affine::rotate(angle),
            TransformListToken::SkewX { angle } => Affine::skew_x(angle),
            TransformListToken::SkewY { angle } => Affine::skew_y(angle),
        };
        map = map.after(next);
    }
    if map.is_finite() { Ok(map) } else { Err("its numbers are too large".to_string()) }
}

/// A length in user units (CSS pixels), for lengths written with a unit: `10mm` is 37.8 user units.
/// Percentages are of `reference`; `em` and `ex` assume CSS's default 16-pixel font.
pub fn user_units(length: Length, reference: f64) -> f64 {
    let px_per_mm = 1.0 / MM_PER_SVG_PX;
    let factor = match length.unit {
        LengthUnit::None | LengthUnit::Px => 1.0,
        LengthUnit::Mm => px_per_mm,
        LengthUnit::Cm => 10.0 * px_per_mm,
        LengthUnit::In => MM_PER_INCH * px_per_mm,
        LengthUnit::Pt => MM_PER_INCH / 72.0 * px_per_mm,
        LengthUnit::Pc => MM_PER_INCH / 6.0 * px_per_mm,
        LengthUnit::Em => 16.0,
        LengthUnit::Ex => 8.0,
        LengthUnit::Percent => reference / 100.0,
    };
    length.number * factor
}

/// The map from the root element's user units to millimetres, from its `width`, `height`, `viewBox` and
/// `preserveAspectRatio` (SVG 1.1 § 7.8). Without a viewBox a user unit is a CSS pixel, 1/96 inch.
pub fn root_to_mm(width: Option<Length>, height: Option<Length>, view_box: Option<ViewBox>, aspect: AspectRatio) -> Result<Affine, String> {
    // A zero size turns rendering off, and a negative one is an error (SVG 1.1 § 7.7).
    for (name, length) in [("width", width), ("height", height)] {
        if let Some(l) = length
            && l.unit != LengthUnit::Percent
            && (l.number <= 0.0 || l.number.is_nan())
        {
            return Err(format!("its {name} is zero or negative"));
        }
    }
    let px = Affine::scale(MM_PER_SVG_PX, MM_PER_SVG_PX);
    let Some(vb) = view_box else { return Ok(px) };
    if !(vb.w > 0.0 && vb.h > 0.0) {
        return Err("its viewBox has no area".to_string());
    }
    // The viewport in millimetres; a missing or relative size is the viewBox's, in pixels.
    let size = |length: Option<Length>, fallback: f64| match length {
        Some(l) if l.unit != LengthUnit::Percent => user_units(l, 0.0) * MM_PER_SVG_PX,
        _ => fallback * MM_PER_SVG_PX,
    };
    let (width, height) = (size(width, vb.w), size(height, vb.h));
    let (sx, sy) = (width / vb.w, height / vb.h);
    let (fx, fy) = match aspect.align {
        Align::None => return Ok(Affine::translate(-vb.x * sx, -vb.y * sy).after(Affine::scale(sx, sy))),
        Align::XMinYMin => (0.0, 0.0),
        Align::XMidYMin => (0.5, 0.0),
        Align::XMaxYMin => (1.0, 0.0),
        Align::XMinYMid => (0.0, 0.5),
        Align::XMidYMid => (0.5, 0.5),
        Align::XMaxYMid => (1.0, 0.5),
        Align::XMinYMax => (0.0, 1.0),
        Align::XMidYMax => (0.5, 1.0),
        Align::XMaxYMax => (1.0, 1.0),
    };
    let s = if aspect.slice { sx.max(sy) } else { sx.min(sy) };
    let (tx, ty) = ((width - vb.w * s) * fx - vb.x * s, (height - vb.h * s) * fy - vb.y * s);
    let map = Affine::translate(tx, ty).after(Affine::scale(s, s));
    if map.is_finite() { Ok(map) } else { Err("its size and viewBox give no usable scale".to_string()) }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn close(a: (f64, f64), b: (f64, f64)) -> bool {
        (a.0 - b.0).abs() < 1e-9 && (a.1 - b.1).abs() < 1e-9
    }

    #[test]
    fn transform_lists_apply_right_to_left() {
        // Scale first, then move: (1, 1) → (2, 2) → (12, 2).
        let map = parse("translate(10) scale(2)").unwrap();
        assert!(close(map.apply(1.0, 1.0), (12.0, 2.0)));
        // A quarter turn clockwise on screen takes +x to +y, and +y to −x.
        assert!(close(parse("rotate(90)").unwrap().apply(1.0, 0.0), (0.0, 1.0)));
        assert!(close(parse("rotate(90)").unwrap().apply(0.0, 1.0), (-1.0, 0.0)));
        // Composition multiplies out every coefficient: matrix(1 2 3 4 5 6) × matrix(7 8 9 10 11 12) is
        // matrix(31 46 39 58 52 76).
        let product = parse("matrix(1 2 3 4 5 6) matrix(7 8 9 10 11 12)").unwrap();
        assert!(close(product.apply(0.0, 0.0), (52.0, 76.0)));
        assert!(close(product.apply(1.0, 0.0), (83.0, 122.0)));
        assert!(close(product.apply(0.0, 1.0), (91.0, 134.0)));
        // Rotation about a centre.
        assert!(close(parse("rotate(180 5 5)").unwrap().apply(0.0, 0.0), (10.0, 10.0)));
        assert!(close(parse("skewX(45)").unwrap().apply(0.0, 1.0), (1.0, 1.0)));
        assert!(close(parse("skewY(45)").unwrap().apply(1.0, 0.0), (1.0, 1.0)));
        assert!(close(parse("matrix(1 2 3 4 5 6)").unwrap().apply(1.0, 1.0), (9.0, 12.0)));
        assert_eq!(parse("").unwrap(), Affine::IDENTITY);
        assert!(parse("rotate(").is_err());
        assert!(parse("scale(1e308) scale(1e308)").is_err());
    }

    #[test]
    fn lengths_convert_to_user_units() {
        let px = |text: &str| user_units(Length::from_str(text).unwrap(), 200.0);
        assert!((px("25.4mm") - 96.0).abs() < 1e-9);
        assert!((px("1in") - 96.0).abs() < 1e-9);
        assert!((px("72pt") - 96.0).abs() < 1e-9);
        assert!((px("6pc") - 96.0).abs() < 1e-9);
        assert!((px("2.54cm") - 96.0).abs() < 1e-9);
        assert_eq!((px("3"), px("3px"), px("1em"), px("1ex"), px("50%")), (3.0, 3.0, 16.0, 8.0, 100.0));
    }

    #[test]
    fn the_root_maps_user_units_to_millimetres() {
        let length = |t: &str| Some(Length::from_str(t).unwrap());
        let view_box = |t: &str| Some(ViewBox::from_str(t).unwrap());
        let aspect = |t: &str| AspectRatio::from_str(t).unwrap();
        // No viewBox: user units are CSS pixels.
        assert!(close(root_to_mm(length("100mm"), length("50mm"), None, aspect("xMidYMid")).unwrap().apply(96.0, 0.0), (25.4, 0.0)));
        // A viewBox in millimetres: Inkscape's usual document.
        let mm = root_to_mm(length("210mm"), length("297mm"), view_box("0 0 210 297"), aspect("xMidYMid")).unwrap();
        assert!(close(mm.apply(105.0, 148.5), (105.0, 148.5)));
        // A viewBox with an origin, scaled to fit and centred ("meet").
        let meet = root_to_mm(length("100mm"), length("50mm"), view_box("10 10 50 50"), aspect("xMidYMid")).unwrap();
        assert!(close(meet.apply(10.0, 10.0), (25.0, 0.0)));
        assert!(close(meet.apply(60.0, 60.0), (75.0, 50.0)));
        let slice = root_to_mm(length("100mm"), length("50mm"), view_box("0 0 50 50"), aspect("xMinYMin slice")).unwrap();
        assert!(close(slice.apply(50.0, 50.0), (100.0, 100.0)));
        let stretch = root_to_mm(length("100mm"), length("50mm"), view_box("0 0 50 50"), aspect("none")).unwrap();
        assert!(close(stretch.apply(50.0, 50.0), (100.0, 50.0)));
        // No size: the viewBox's own size in pixels.
        assert!(close(root_to_mm(None, None, view_box("0 0 96 96"), aspect("xMidYMid")).unwrap().apply(96.0, 96.0), (25.4, 25.4)));
        let flat = Some(ViewBox::new(0.0, 0.0, 0.0, 10.0));
        assert_eq!(root_to_mm(length("10mm"), length("10mm"), flat, aspect("xMidYMid")), Err("its viewBox has no area".to_string()));
        // A viewBox with an origin and a scale other than 1, under each kind of alignment.
        let origin = |align: &str| root_to_mm(length("100mm"), length("100mm"), view_box("10 20 200 100"), aspect(align)).unwrap();
        assert!(close(origin("xMidYMid").apply(10.0, 20.0), (0.0, 25.0)));
        assert!(close(origin("xMidYMid").apply(210.0, 120.0), (100.0, 75.0)));
        assert!(close(origin("none").apply(10.0, 20.0), (0.0, 0.0)));
        assert!(close(origin("none").apply(210.0, 120.0), (100.0, 100.0)));
        assert_eq!(root_to_mm(length("0"), None, None, aspect("xMidYMid")), Err("its width is zero or negative".to_string()));
        assert!(root_to_mm(None, length("-1mm"), view_box("0 0 10 10"), aspect("xMidYMid")).is_err());
        // A relative size is the viewBox's own, in pixels.
        assert!(close(root_to_mm(length("50%"), None, view_box("0 0 96 96"), aspect("xMidYMid")).unwrap().apply(96.0, 0.0), (25.4, 0.0)));
    }
}
