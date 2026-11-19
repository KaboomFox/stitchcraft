//! Text in pictures from the command line (roadmap step M14.5, `REQ-CLI-020`,
//! `docs/src/design/image-to-embroidery.md` › Text in pictures): `--text` marks an area of a design whose
//! traced letters sew as lettering, and an SVG file's own markers are read.
//!
//! **Why percent.** A picture is traced into a document that `stitch` never shows, at a size the hoop
//! decides, so its millimetres are unknown to whoever types the command. The box round everything the
//! design draws is the picture itself when a picture was traced, since its background is traced too. A
//! share of that box is the same whatever the size, and can be read off the picture by eye: the marker
//! needs only to reach round the letters and round nothing else.
//!
//! **Why with the rules.** The marks travel with the settings of automatic digitizing's rules, as one more
//! key ([`KEY`]). A bug-report bundle records those and replays them, so a design marked on the command
//! line replays marked.
//!
//! The fonts are those of the folder `STITCHCRAFT_FONTS` names ([`crate::fonts`]), as for a document's texts.
//!
//! **An SVG file's text objects** are set here too (M14.7, [`lettered`]): its reader has no fonts, and gives
//! them for `stitch` to letter before the markers are looked for.

use std::collections::BTreeSet;

use stitchcraft_core::{Budget, Diagnostic, Point};
use stitchcraft_engine::design::{Design, FillRule, Path, Segment, Shape, Subpath};
use stitchcraft_svg::lettering::objects::{self, Waiting};
use stitchcraft_svg::lettering::pictures::{self, MARKER, Marker};
use stitchcraft_vectorcraft::lettering::{Fonts, found_for};

/// The key a mark of `--text` has among the rules' settings: the marker's name, which is no rule's.
pub const KEY: &str = MARKER;

/// A mark as `--text` takes it, `LEFT,TOP,WIDTH,HEIGHT:TEXT`: an area in percent of the box round the
/// design's shapes, from the box's left and top, and the text, written as a marker's name writes it. Read
/// into the setting that carries it.
pub fn mark(text: &str) -> Result<(String, String), String> {
    area(text)
        .map(|_| (KEY.to_string(), text.to_string()))
        .ok_or_else(|| format!("`{text}` isn't LEFT,TOP,WIDTH,HEIGHT:TEXT, an area in percent of the design's box, inside it, and the text in it"))
}

/// The area and the note of the mark `text`: its left, top, width and height in percent, inside the box.
fn area(text: &str) -> Option<([f64; 4], &str)> {
    let (numbers, note) = text.split_once(':')?;
    let area = share(numbers)?;
    (!note.trim().is_empty()).then_some((area, note.trim()))
}

/// The area `text` gives, `LEFT,TOP,WIDTH,HEIGHT` in percent of the box round a design's shapes, from the
/// box's left and top: 4 numbers, the area inside the box and not empty. `--text` and `--keep` both read
/// their areas so.
pub fn share(text: &str) -> Option<[f64; 4]> {
    let mut parts = text.split(',').map(|part| part.trim().parse::<f64>().ok().filter(|number| number.is_finite()));
    let area = [parts.next()??, parts.next()??, parts.next()??, parts.next()??];
    let [left, top, width, height] = area;
    let inside = left >= 0.0 && top >= 0.0 && width > 0.0 && height > 0.0 && left + width <= 100.0 && top + height <= 100.0;
    (parts.next().is_none() && inside).then_some(area)
}

/// The rectangle that `share`, an area as [`share`] reads it, marks of `design`: a closed fill in the
/// design's millimetres, from the box round its shapes.
pub fn rectangle(design: &Design, [left, top, width, height]: [f64; 4]) -> Shape {
    let bounds = design.bounds();
    let (x, y, across, down) =
        bounds.map_or((0.0, 0.0, 0.0, 0.0), |b| (b.min().x(), b.min().y(), (b.max().x() - b.min().x()) / 100.0, (b.max().y() - b.min().y()) / 100.0));
    let (x0, y0) = (x + left * across, y + top * down);
    let (x1, y1) = (x0 + width * across, y0 + height * down);
    // A share of a design's box lies inside it, so each corner is a point of the design's own range.
    let corner = |x: f64, y: f64| Point::new(x, y).unwrap_or(Point::from_tenths(0, 0));
    let outline =
        Subpath { start: corner(x0, y0), segments: [(x1, y0), (x1, y1), (x0, y1)].map(|(x, y)| Segment::Line(corner(x, y))).to_vec(), closed: true };
    Shape::Fill { path: Path { subpaths: vec![outline] }, rule: FillRule::NonZero, gradient: None }
}

/// `design` with the text of each of its own markers and of each mark of `marks` in the place of the shapes
/// inside it ([`pictures::replace`]), in the fonts of `fonts`, and what was said. The marks are numbered from
/// 1 in the order given: `text-1` is the first.
pub fn replaced(design: Design, marks: &[&str], fonts: &mut dyn Fonts) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
    let markers: Vec<Marker> = marks
        .iter()
        .filter_map(|mark| area(mark))
        .enumerate()
        .map(|(index, (area, note))| Marker { label: format!("text-{}", index + 1), shape: rectangle(&design, area), note: note.to_string() })
        .collect();
    let choices = fonts.choices();
    let mut found = |name: &str, text: &str, budget: &Budget| found_for(&mut *fonts, name, text, budget);
    pictures::replace_among(design, &markers, &mut found, &choices, &mut BTreeSet::new(), &Budget::DEFAULT)
}

/// `design` with each of the text objects `texts` of its SVG file sewn as lettering in the default font of
/// `fonts`, or a bold text in the bold font where it sews at the text's height, at its place in the stitching
/// order ([`objects::lettered_among`], `REQ-AUTO-069`, `REQ-AUTO-082`), and what was said.
pub fn lettered(design: Design, texts: &[Waiting], fonts: &mut dyn Fonts) -> Result<(Design, Vec<Diagnostic>), Diagnostic> {
    let choices = fonts.choices();
    let mut found = |name: &str, text: &str, budget: &Budget| found_for(&mut *fonts, name, text, budget);
    objects::lettered_among(design, texts, &mut found, &choices, &mut BTreeSet::new(), &Budget::DEFAULT)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Code;
    use stitchcraft_engine::design::Reading;
    use stitchcraft_testkit::temp::TempDir;

    use super::*;
    use crate::cli::ReadingArgs;
    use crate::fonts::FolderFonts;

    /// A glyph file whose `a` is a stroke 4 mm wide and 10 tall on its baseline.
    const GLYPHS: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" width="100mm" height="100mm" viewBox="0 0 100 100"><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 0,60 L 4,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"##;

    /// A folder of fonts that holds the default font, drawn here.
    fn fonts(dir: &TempDir) -> FolderFonts {
        std::fs::create_dir_all(dir.join("small_font")).unwrap();
        std::fs::write(dir.join("small_font/font.json"), r#"{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 4}"#).unwrap();
        std::fs::write(dir.join("small_font/ltr.svg"), GLYPHS).unwrap();
        FolderFonts::of(Some(dir.join(".")))
    }

    /// A picture 200 mm wide and 200 tall whose top left corner is at (10, 5): a white field, and 2 red
    /// letters 20 mm tall, at (20, 15) and at (130, 100).
    fn picture(more: &str) -> Design {
        let svg = format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="220mm" height="210mm" viewBox="0 0 220 210"><rect id="field" x="10" y="5" width="200" height="200" fill="#ffffff"/><rect id="l1" x="20" y="15" width="8" height="20" fill="#ff0000"/><rect id="l2" x="130" y="100" width="8" height="20" fill="#ff0000"/>{more}</svg>"##
        );
        stitchcraft_svg::read_with(svg.as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap().design
    }

    fn ids(design: &Design) -> Vec<&str> {
        design.elements().iter().map(|element| element.id.as_str()).collect()
    }

    fn first_point(design: &Design, id: &str) -> (f64, f64) {
        let start = design.elements().iter().find(|element| element.id.as_str() == id).unwrap().shape.path().subpaths[0].start;
        ((start.x() * 1e6).round() / 1e6, (start.y() * 1e6).round() / 1e6)
    }

    #[test]
    fn req_cli_020_a_mark_is_an_area_in_percent_of_the_designs_box_and_a_text() {
        assert_eq!(mark("10,40,80,20:MATCH"), Ok(("stitch:text".to_string(), "10,40,80,20:MATCH".to_string())));
        assert_eq!(area(" 0 , 0 ,100, 100 : \"A:B\" font=x "), Some(([0.0, 0.0, 100.0, 100.0], "\"A:B\" font=x")));
        assert_eq!(area("0.5,99,99.5,1:a"), Some(([0.5, 99.0, 99.5, 1.0], "a")));
        let refused = [
            "MATCH",
            "10,40,80:MATCH",
            "10,40,80,20,5:MATCH",
            "10,40,80,20:",
            "10,40,80,20:  ",
            "a,40,80,20:MATCH",
            "10,40,80,inf:MATCH",
            "-1,40,80,20:MATCH",
            "10,-1,80,20:MATCH",
            "10,40,0,20:MATCH",
            "10,40,80,0:MATCH",
            "20.1,40,80,20:MATCH",
            "10,80.1,80,20:MATCH",
            "10,40,80,NaN:MATCH",
            "NaN,40,80,20:MATCH",
            "10,NaN,80,20:MATCH",
            "10,40,NaN,20:MATCH",
        ];
        for text in refused {
            assert_eq!(
                mark(text),
                Err(format!("`{text}` isn't LEFT,TOP,WIDTH,HEIGHT:TEXT, an area in percent of the design's box, inside it, and the text in it")),
            );
        }
        // The marks travel with the rules' settings, every one of them, after the settings.
        let reading = ReadingArgs {
            set: vec![("auto_prefer".to_string(), "satin".to_string())],
            text: vec![mark("0,0,50,100:a").unwrap(), mark("50,0,50,100:a").unwrap()],
            ..ReadingArgs::default()
        };
        let set = |key: &str, value: &str| (key.to_string(), value.to_string());
        assert_eq!(reading.rules(), [set("auto_prefer", "satin"), set("stitch:text", "0,0,50,100:a"), set("stitch:text", "50,0,50,100:a")]);
    }

    #[test]
    fn req_cli_020_the_letters_inside_each_area_marked_sew_as_lettering() {
        let dir = TempDir::new("cli-text");
        // The top left quarter holds the first letter. The second is in the box from 55 to 65 % across and
        // 47 to 62 % down, which is from (120, 99) to (140, 129). A third mark holds nothing.
        let marks = ["0,0,50,50:a", "55,47,10,15:a", "80,0,20,10:a"];
        let (design, said) = replaced(picture(""), &marks, &mut fonts(&dir)).unwrap();
        assert_eq!(ids(&design), ["svg:field:fill", "text-1:0-svg:pa:stroke", "text-2:0-svg:pa:stroke"]);
        assert_eq!(first_point(&design, "text-1:0-svg:pa:stroke"), (20.0, 15.0));
        assert_eq!(first_point(&design, "text-2:0-svg:pa:stroke"), (130.0, 100.0));
        let codes: Vec<Code> = said.iter().map(|said| said.code).collect();
        assert_eq!(codes, [Code::TextInPictureLettered, Code::TextInPictureLettered, Code::TextMarkerNotUsed]);
        assert!(said[2].message.starts_with("The text marker `text-3` is not used: no shape lies wholly inside it."), "{}", said[2]);
        // An area that ends short of a letter leaves it, and a design that draws nothing has no box.
        let (design, said) = replaced(picture(""), &["0,0,8.9,100:a"], &mut fonts(&dir)).unwrap();
        assert_eq!((ids(&design).len(), said[0].code), (3, Code::TextMarkerNotUsed));
        let empty = Design::default();
        let (design, said) = replaced(empty, &["0,0,100,100:a"], &mut fonts(&dir)).unwrap();
        assert!(design.elements().is_empty() && said[0].message.contains("it encloses no area."), "{}", said[0]);
        // Without a folder of fonts the letters stay, and the mark says there is no font.
        let (design, said) = replaced(picture(""), &["0,0,50,100:a"], &mut FolderFonts::of(None)).unwrap();
        assert_eq!(ids(&design).len(), 3);
        assert!(said[0].message.contains("The default embroidery font is not here, and no other is."), "{}", said[0]);
    }

    #[test]
    fn req_svg_016_a_shape_labelled_stitch_text_marks_text_in_an_svg_file() {
        let dir = TempDir::new("cli-text-svg");
        let marker = r##"<rect id="m" inkscape:label="stitch:text a" x="15" y="10" width="20" height="30" fill="none" stroke="#0000ff"/>"##;
        let (design, said) = replaced(picture(marker), &[], &mut fonts(&dir)).unwrap();
        assert_eq!(ids(&design), ["svg:field:fill", "svg:m:stroke:0-svg:pa:stroke", "svg:l2:fill"]);
        assert_eq!(first_point(&design, "svg:m:stroke:0-svg:pa:stroke"), (20.0, 15.0));
        assert_eq!(said.len(), 1);
        assert!(said[0].to_string().starts_with("info SC-I1112: The text marker `svg:m:stroke`: 1 traced outlines, 20.0 mm tall,"), "{}", said[0]);
        // A file's own marker and a mark of the command line are both used.
        let (design, _) = replaced(picture(marker), &["55,47,10,15:a"], &mut fonts(&dir)).unwrap();
        assert_eq!(ids(&design), ["svg:field:fill", "svg:m:stroke:0-svg:pa:stroke", "text-1:0-svg:pa:stroke"]);
    }
}
