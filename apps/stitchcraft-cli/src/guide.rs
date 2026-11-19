//! Guides from the command line (roadmap step M13.24, `REQ-CLI-022`, `docs/src/design/image-to-embroidery.md`
//! › A guide on one shape): `--guide` draws a line over a design, and the filled parts of its automatic areas
//! that the line crosses sew as guided fills whose rows follow it (`REQ-AUTO-109`).
//!
//! A picture traced by the command line never opens in an editor where a guide could be drawn, so the line
//! is given as points in percent of the box round the design's shapes, as `--keep` reads its area
//! ([`crate::text::numbers`]): they can be read off the picture whatever size it sews at. The points are
//! joined by straight lines, since a guided fill smooths its rows. The guides travel with the settings of
//! the rules under one more key ([`KEY`]), so that a bug-report bundle records them and its replay steers the
//! same parts.

use stitchcraft_core::ElementId;
use stitchcraft_engine::design::mark::{GUIDE, Mark};
use stitchcraft_engine::design::{Design, Path, Segment, Shape, Subpath};

use crate::text::{at, numbers};

/// The key a guide of `--guide` has among the rules' settings: the guide's name, which is no rule's.
pub const KEY: &str = GUIDE;

/// A guide as `--guide` takes it, `X,Y,X,Y…`: 2 points or more in percent of the box round the design's
/// shapes, from the box's left and top. Read into the setting that carries it.
pub fn guide(text: &str) -> Result<(String, String), String> {
    points(text)
        .map(|_| (KEY.to_string(), text.to_string()))
        .ok_or_else(|| format!("`{text}` isn't X,Y,X,Y…, 2 points or more in percent of the design's box"))
}

/// The points of the guide `text`, as `X,Y` pairs: `None` for fewer than 2, an odd count of numbers, or a
/// number that doesn't read.
fn points(text: &str) -> Option<Vec<[f64; 2]>> {
    let numbers = numbers(text)?;
    let (points, odd) = numbers.as_chunks::<2>();
    (odd.is_empty() && points.len() >= 2).then(|| points.to_vec())
}

/// `design` with a guide for each line of `guides`, after its own, named from `guide-1` in the order given.
/// A guide with a point outside the range a design's points may have is left out.
pub fn guided(mut design: Design, guides: &[&str]) -> Design {
    let made: Vec<Mark> = guides
        .iter()
        .filter_map(|guide| points(guide))
        .enumerate()
        .filter_map(|(index, points)| {
            let mut placed = points.iter().map(|&[x, y]| at(&design, x, y));
            let start = placed.next()??;
            let segments = placed.map(|point| point.map(Segment::Line)).collect::<Option<Vec<_>>>()?;
            let line = Path { subpaths: vec![Subpath { start, segments, closed: false }] };
            Some(Mark { id: ElementId::new(format!("guide-{}", index + 1)).ok()?, shape: Shape::stroke(line) })
        })
        .collect();
    design.guides.extend(made);
    design
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_engine::design::Reading;

    use super::*;

    /// A design 60 mm square from (0, 0).
    fn design() -> Design {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="60mm" height="60mm" viewBox="0 0 60 60"><rect id="field" width="60" height="60" fill="#ffffff"/></svg>"##;
        stitchcraft_svg::read_with(svg.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design
    }

    #[test]
    fn req_cli_022_a_guide_is_points_in_percent_of_the_designs_box() {
        assert_eq!(guide("10,50,90,50"), Ok((KEY.to_string(), "10,50,90,50".to_string())));
        assert_eq!(guide(" 0, 0 ,100,100, -5,120"), Ok((KEY.to_string(), " 0, 0 ,100,100, -5,120".to_string())));
        for refused in ["10,50", "10,50,90", "10,50,x,40", "", "1,2,3,4,5", "inf,0,1,1"] {
            let refused_why = guide(refused).unwrap_err();
            assert!(refused_why.contains("isn't X,Y,X,Y"), "{refused}: {refused_why}");
        }
        let guided = guided(design(), &["10,50,50,40,90,50", "no", "0,0,100,100"]);
        let ids: Vec<&str> = guided.guides.iter().map(|guide| guide.id.as_str()).collect();
        assert_eq!(ids, ["guide-1", "guide-2"]);
        // 10 % of 60 mm from the left and 50 % from the top, then straight lines through the other points.
        let line = &guided.guides[0].shape.path().subpaths[0];
        let points: Vec<(f64, f64)> = std::iter::once(line.start)
            .chain(line.segments.iter().filter_map(|segment| if let Segment::Line(p) = segment { Some(*p) } else { None }))
            .map(|p| (p.x(), p.y()))
            .collect();
        assert_eq!(points, [(6.0, 30.0), (30.0, 24.0), (54.0, 30.0)]);
        assert!(!line.closed);
        // A point far outside the range a design's points may have leaves its guide out.
        assert!(super::guided(design(), &["0,0,1e300,0"]).guides.is_empty());
    }
}
