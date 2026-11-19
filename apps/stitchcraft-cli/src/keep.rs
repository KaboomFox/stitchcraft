//! Marks that keep parts from the command line (roadmap step M13.39, `REQ-CLI-021`,
//! `docs/src/design/image-to-embroidery.md` › Marks that keep parts): `--keep` marks an area of a design,
//! and the parts of its automatic areas that lie wholly inside it sew by their width where automatic
//! digitizing would merge them into a neighbour (`REQ-AUTO-104`).
//!
//! The area is read as a mark of `--text` reads its own ([`crate::text::share`]), in percent of the box round
//! the design's shapes, for the reasons that module gives. The marks travel with the settings of the rules
//! under one more key ([`KEY`]), so that a bug-report bundle records them and its replay keeps the same parts.

use stitchcraft_core::ElementId;
use stitchcraft_engine::design::mark::KEEP;
use stitchcraft_engine::design::{Design, Mark};

use crate::text::{rectangle, share};

/// The key a mark of `--keep` has among the rules' settings: the mark's name, which is no rule's.
pub const KEY: &str = KEEP;

/// A mark as `--keep` takes it, `LEFT,TOP,WIDTH,HEIGHT`: an area in percent of the box round the design's
/// shapes, from the box's left and top. Read into the setting that carries it.
pub fn mark(text: &str) -> Result<(String, String), String> {
    share(text)
        .map(|_| (KEY.to_string(), text.to_string()))
        .ok_or_else(|| format!("`{text}` isn't LEFT,TOP,WIDTH,HEIGHT, an area in percent of the design's box, inside it"))
}

/// `design` with a mark for each area of `marks`, after its own, named from `keep-1` in the order given.
pub fn kept(mut design: Design, marks: &[&str]) -> Design {
    let made: Vec<Mark> = marks
        .iter()
        .filter_map(|mark| share(mark))
        .enumerate()
        .filter_map(|(index, area)| Some(Mark { id: ElementId::new(format!("keep-{}", index + 1)).ok()?, shape: rectangle(&design, area) }))
        .collect();
    design.marks.extend(made);
    design
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_engine::design::Reading;
    use stitchcraft_engine::design::mark::Area;

    use super::*;

    /// A design 60 mm square from (0, 0).
    fn design() -> Design {
        let svg = r##"<svg xmlns="http://www.w3.org/2000/svg" width="60mm" height="60mm" viewBox="0 0 60 60"><rect id="field" width="60" height="60" fill="#ffffff"/></svg>"##;
        stitchcraft_svg::read_with(svg.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design
    }

    #[test]
    fn req_cli_021_a_keep_mark_is_an_area_in_percent_of_the_designs_box() {
        assert_eq!(mark("30,30,10,10"), Ok((KEY.to_string(), "30,30,10,10".to_string())));
        assert_eq!(mark(" 0, 0 ,100,100"), Ok((KEY.to_string(), " 0, 0 ,100,100".to_string())));
        for refused in [
            "90,0,20,10",
            "0,90,10,20",
            "-1,0,10,10",
            "0,-1,10,10",
            "0,0,0,10",
            "0,0,10,0",
            "1,2,3",
            "1,2,3,4,5",
            "a,b,c,d",
            "inf,0,1,1",
            "30,30,10,10:a",
        ] {
            let refused_why = mark(refused).unwrap_err();
            assert!(refused_why.contains("isn't LEFT,TOP,WIDTH,HEIGHT"), "{refused}: {refused_why}");
        }
        let marked = kept(design(), &["30,30,10,10", "0,0,50,100", "no"]);
        let ids: Vec<&str> = marked.marks.iter().map(|mark| mark.id.as_str()).collect();
        assert_eq!(ids, ["keep-1", "keep-2"]);
        // 30 % of 60 mm from the left and the top, 6 mm across.
        let area = Area::of(&marked.marks[0].shape, &mut Budget::DEFAULT.meter()).unwrap().unwrap();
        assert!(area.holds((18.1, 18.1)) && area.holds((23.9, 23.9)) && !area.holds((17.9, 20.0)) && !area.holds((20.0, 24.1)));
    }
}
