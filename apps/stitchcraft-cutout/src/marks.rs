//! What a user marks on a picture to correct its cut-out: the subject's box, points of the subject and
//! points of what isn't it (`REQ-CLI-015`, ADR-0019 decision 6).
//!
//! **In the picture's pixels.** Each is measured from the picture's left and top edges, in its pixels as
//! its file stores them, so the numbers can be read off the picture in any program that shows a pointer's
//! position. A pixel spans 1 unit, so the box of a whole picture `w` × `h` px is `0,0,w,h`, and the point at
//! a pixel's middle is half a unit in from its left and top.
//!
//! **Checked against the picture.** A box with no area, or reaching past the picture, and a point outside
//! it are refused before any model runs: they are the user's mistakes, which `stitch` says as usage errors.

use std::fmt;

/// A point in a picture's pixels: `x` from its left edge, `y` from its top edge.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    /// Across, from the left edge.
    pub x: f64,
    /// Down, from the top edge.
    pub y: f64,
}

/// A box in a picture's pixels: from its left and top edges at `x0` and `y0` to its right and bottom ones
/// at `x1` and `y1`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PixelBox {
    /// Its left edge.
    pub x0: f64,
    /// Its top edge.
    pub y0: f64,
    /// Its right edge.
    pub x1: f64,
    /// Its bottom edge.
    pub y1: f64,
}

/// The finite numbers in `text`, separated by commas, when there are `count` of them.
fn numbers(text: &str, count: usize, form: &str) -> Result<Vec<f64>, String> {
    let values: Vec<f64> = text.split(',').filter_map(|part| part.trim().parse::<f64>().ok().filter(|value| value.is_finite())).collect();
    if values.len() == count && text.split(',').count() == count {
        Ok(values)
    } else {
        Err(format!("`{text}` isn't {form}, {count} numbers separated by commas"))
    }
}

impl Point {
    /// The point `X,Y` gives, as `--subject-point` and `--not-subject` take it.
    pub fn parse(text: &str) -> Result<Point, String> {
        match numbers(text, 2, "X,Y")?.as_slice() {
            &[x, y] => Ok(Point { x, y }),
            _ => Err(format!("`{text}` isn't X,Y")),
        }
    }

    /// Whether it lies in a picture `width` × `height` px: why not, for a usage error, when it doesn't.
    pub fn check(&self, width: u32, height: u32) -> Result<(), String> {
        let inside = (0.0..f64::from(width)).contains(&self.x) && (0.0..f64::from(height)).contains(&self.y);
        if inside { Ok(()) } else { Err(format!("the point {self} lies outside the picture, which is {width} × {height} px")) }
    }
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{}", self.x, self.y)
    }
}

impl PixelBox {
    /// The box `X0,Y0,X1,Y1` gives, as `--subject-box` takes it.
    pub fn parse(text: &str) -> Result<PixelBox, String> {
        match numbers(text, 4, "X0,Y0,X1,Y1")?.as_slice() {
            &[x0, y0, x1, y1] => Ok(PixelBox { x0, y0, x1, y1 }),
            _ => Err(format!("`{text}` isn't X0,Y0,X1,Y1")),
        }
    }

    /// Whether it has an area and lies within a picture `width` × `height` px: why not, for a usage error,
    /// when it doesn't.
    pub fn check(&self, width: u32, height: u32) -> Result<(), String> {
        if self.x1 <= self.x0 || self.y1 <= self.y0 {
            return Err(format!("the box {self} has no area: its right edge must lie right of its left one, and its bottom below its top"));
        }
        if self.x0 < 0.0 || self.y0 < 0.0 || self.x1 > f64::from(width) || self.y1 > f64::from(height) {
            return Err(format!("the box {self} reaches past the picture, which is {width} × {height} px"));
        }
        Ok(())
    }

    /// Its width, in pixels.
    pub fn width(&self) -> f64 {
        self.x1 - self.x0
    }

    /// Its height, in pixels.
    pub fn height(&self) -> f64 {
        self.y1 - self.y0
    }
}

impl fmt::Display for PixelBox {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{},{},{},{}", self.x0, self.y0, self.x1, self.y1)
    }
}

/// The marks a cut-out takes: the subject's box, or none for U²-Net-p to find, and the points of the
/// subject and of what isn't it, in the order given.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marks {
    /// `--subject-box`.
    pub subject_box: Option<PixelBox>,
    /// Each `--subject-point`.
    pub subject: Vec<Point>,
    /// Each `--not-subject`.
    pub not_subject: Vec<Point>,
}

impl Marks {
    /// Whether every mark fits a picture `width` × `height` px: the first one that doesn't, and why, for a
    /// usage error.
    pub fn check(&self, width: u32, height: u32) -> Result<(), String> {
        if let Some(subject_box) = &self.subject_box {
            subject_box.check(width, height).map_err(|why| format!("--subject-box: {why}"))?;
        }
        for point in &self.subject {
            point.check(width, height).map_err(|why| format!("--subject-point: {why}"))?;
        }
        for point in &self.not_subject {
            point.check(width, height).map_err(|why| format!("--not-subject: {why}"))?;
        }
        Ok(())
    }

    /// Whether any is given.
    pub fn given(&self) -> bool {
        self.subject_box.is_some() || !self.subject.is_empty() || !self.not_subject.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn req_cli_015_marks_are_read_as_numbers_in_the_pictures_pixels() {
        assert_eq!(PixelBox::parse("353,0,881,1192"), Ok(PixelBox { x0: 353.0, y0: 0.0, x1: 881.0, y1: 1192.0 }));
        assert_eq!(PixelBox::parse(" 352.5, 0 ,881.25,1190 "), Ok(PixelBox { x0: 352.5, y0: 0.0, x1: 881.25, y1: 1190.0 }));
        assert_eq!(Point::parse("640,720"), Ok(Point { x: 640.0, y: 720.0 }));
        // As many numbers as the form says, finite and separated by commas.
        for wrong in ["353,0,881", "353,0,881,1192,4", "353;0;881;1192", "a,b,c,d", "1,2,3,inf", "1,2,,3,4", ""] {
            assert_eq!(PixelBox::parse(wrong), Err(format!("`{wrong}` isn't X0,Y0,X1,Y1, 4 numbers separated by commas")), "{wrong}");
        }
        for wrong in ["640", "640,720,1", "640,NaN", "640 720"] {
            assert_eq!(Point::parse(wrong), Err(format!("`{wrong}` isn't X,Y, 2 numbers separated by commas")), "{wrong}");
        }
        // Written back as given.
        assert_eq!(PixelBox::parse(" 352.5, 0 ,881.25,1190 ").unwrap().to_string(), "352.5,0,881.25,1190");
        assert_eq!(Point::parse("640.5,720").unwrap().to_string(), "640.5,720");
    }

    #[test]
    fn req_cli_015_a_box_without_area_or_past_the_picture_and_a_point_outside_it_are_refused() {
        let (width, height) = (1200, 1786);
        let parsed = |text: &str| PixelBox::parse(text).unwrap();
        assert_eq!(parsed("0,0,1200,1786").check(width, height), Ok(()), "the whole picture");
        assert_eq!(parsed("160,215,680,1300").check(width, height), Ok(()));
        for flat in ["10,10,10,20", "10,10,20,10", "20,10,10,20"] {
            let why = parsed(flat).check(width, height).unwrap_err();
            assert!(why.ends_with("has no area: its right edge must lie right of its left one, and its bottom below its top"), "{why}");
        }
        for past in ["-1,0,100,100", "0,-0.5,100,100", "0,0,1200.5,100", "0,0,100,1787"] {
            assert_eq!(parsed(past).check(width, height), Err(format!("the box {past} reaches past the picture, which is 1200 × 1786 px")));
        }
        // A point lies in the picture from its left and top edges to just before its right and bottom ones.
        for inside in ["0,0", "1199.9,1785.9", "600,893"] {
            assert_eq!(Point::parse(inside).unwrap().check(width, height), Ok(()), "{inside}");
        }
        for outside in ["1200,10", "10,1786", "-0.1,10", "10,-3"] {
            let why = Point::parse(outside).unwrap().check(width, height).unwrap_err();
            assert_eq!(why, format!("the point {outside} lies outside the picture, which is 1200 × 1786 px"));
        }
        assert_eq!((parsed("160,215,680,1300").width(), parsed("160,215,680,1300").height()), (520.0, 1085.0));
    }

    #[test]
    fn req_cli_015_the_first_mark_that_does_not_fit_is_named_by_its_option() {
        let point = |x, y| Point { x, y };
        let marks = Marks { subject_box: None, subject: vec![point(10.0, 10.0)], not_subject: vec![point(5.0, 5.0), point(500.0, 5.0)] };
        assert_eq!(marks.check(100, 100), Err("--not-subject: the point 500,5 lies outside the picture, which is 100 × 100 px".to_string()));
        let marks = Marks { subject: vec![point(100.0, 10.0)], ..marks };
        assert!(marks.check(100, 100).unwrap_err().starts_with("--subject-point: the point 100,10 lies outside"));
        let marks = Marks { subject_box: Some(PixelBox { x0: 0.0, y0: 0.0, x1: 0.0, y1: 1.0 }), ..marks };
        assert!(marks.check(100, 100).unwrap_err().starts_with("--subject-box: the box 0,0,0,1 has no area"));
        assert!(marks.given() && !Marks::default().given());
        assert_eq!(Marks::default().check(1, 1), Ok(()));
        assert!(Marks { not_subject: vec![point(0.0, 0.0)], ..Marks::default() }.given());
        assert!(Marks { subject: vec![point(0.0, 0.0)], ..Marks::default() }.given());
    }
}
