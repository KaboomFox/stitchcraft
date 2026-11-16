//! Lettering along a path, back and forth, with trims and sorted colours (roadmap step M14.3, `REQ-FONT-015`
//! to `REQ-FONT-019`): text set in fonts drawn in code, checked on the glyphs' places and angles, on the
//! order the elements sew in and on their settings. No font file of anyone's is used.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used)]

use proptest::prelude::*;
use stitchcraft_core::units::{MM_PER_SVG_PX, Point};
use stitchcraft_core::{Budget, Code, Diagnostic, Fix};
use stitchcraft_engine::design::{Path, Segment, Subpath};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::{self, Along, ColourSort, Lettering, SETTINGS, Set, TextPosition, Trim, VerticalPosition};

/// A glyph file's root: 100 mm square, a user unit a millimetre.
const ROOT: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">"#;

/// A stroke `id` from (0, `top`) to (`width`, `bottom`), in a glyph file whose baseline is 70 mm down, with
/// `more` attributes.
fn stroke(id: &str, width: f64, top: f64, bottom: f64, more: &str) -> String {
    format!(r#"<path id="{id}" d="M 0,{top} L {width},{bottom}" style="fill:none;stroke:#000000;stroke-width:0.4" {more}/>"#)
}

/// The glyph layer of `label`, holding `body`.
fn layer(label: &str, body: &str) -> String {
    format!(r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}" style="display:none">{body}</g>"#)
}

/// A glyph file of `layers`, its baseline 70 mm down.
fn file(layers: &str) -> String {
    format!(
        r#"{ROOT}<sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{layers}</svg>"#
    )
}

/// The glyphs of the test font, their ids starting with `prefix`: `a` 4 mm wide and `b` 6, each 10 mm tall on
/// the baseline, and `p` 4 mm wide, from 5 mm above the baseline to 3 below it.
fn glyphs(prefix: &str) -> String {
    [
        layer("a", &stroke(&format!("{prefix}a"), 4.0, 60.0, 70.0, "")),
        layer("b", &stroke(&format!("{prefix}b"), 6.0, 60.0, 70.0, "")),
        layer("p", &stroke(&format!("{prefix}p"), 4.0, 65.0, 73.0, "")),
    ]
    .concat()
}

/// The settings of a test font, with `more` keys: 10 mm tall, sewn from 5 to 20 mm. Every glyph advances
/// 5 mm, a space 3 and a line 20.
fn json(more: &str) -> String {
    let px = |mm: f64| mm / MM_PER_SVG_PX;
    format!(
        r#"{{"name": "Test", "size": 10, "min_scale": 0.5, "max_scale": 2, "auto_satin": false, "leading": {}, "horiz_adv_x_default": {}, "horiz_adv_x_space": {}{more}}}"#,
        px(20.0),
        px(5.0),
        px(3.0)
    )
}

/// The font of `json` and the glyph files `files`, each a name and its layers.
fn font_of(json: &str, files: &[(&str, String)]) -> Font {
    let written: Vec<(&str, String)> = files.iter().map(|(name, layers)| (*name, file(layers))).collect();
    let mut given: Vec<(&str, &[u8])> = vec![("font.json", json.as_bytes())];
    given.extend(written.iter().map(|(name, text)| (*name, text.as_bytes())));
    font::read("test", &given, &Budget::DEFAULT).unwrap()
}

/// The test font, which sews left to right, with `more` keys in its settings.
fn one_way(more: &str) -> Font {
    font_of(&json(more), &[("ltr.svg", glyphs("p"))])
}

/// The test font drawn for both directions, with `more` keys in its settings: the ids of its right-to-left
/// glyphs start with `r`.
fn both_ways(more: &str) -> Font {
    font_of(&json(more), &[("ltr.svg", glyphs("p")), ("rtl.svg", glyphs("r"))])
}

/// The open path through `points`, or the closed one.
fn path(points: &[(f64, f64)], closed: bool) -> Path {
    let at = |(x, y): (f64, f64)| Point::new(x, y).unwrap();
    let segments = points[1..].iter().map(|point| Segment::Line(at(*point))).collect();
    Path { subpaths: vec![Subpath { start: at(points[0]), segments, closed }] }
}

/// The path from (10, 50) `length` millimetres to the right.
fn level(length: f64) -> Path {
    path(&[(10.0, 50.0), (10.0 + length, 50.0)], false)
}

/// `path` with a text on its baseline, from its start.
fn on(path: Path) -> Along {
    Along { path, position: TextPosition::Left, vertical: VerticalPosition::Baseline }
}

/// `text` set in `font` along `along`, which must set.
fn along(font: &Font, text: &str, along: &Along) -> Set {
    lettering::set_along(font, text, &Lettering::default(), along, &Budget::DEFAULT).unwrap()
}

/// `text` set in `font` as `how` says, which must set.
fn set(font: &Font, text: &str, how: &Lettering) -> Set {
    lettering::set(font, text, how, &Budget::DEFAULT).unwrap()
}

/// `values` are `expected`, each within a nanometre.
#[track_caller]
fn near(values: &[f64], expected: &[f64]) {
    assert_eq!(values.len(), expected.len(), "{values:?} are not {expected:?}");
    assert!(values.iter().zip(expected).all(|(a, b)| (a - b).abs() < 1e-6), "{values:?} are not {expected:?}");
}

/// Where each glyph's box's left edge meets its baseline, and how far the glyph is turned: x, y and degrees.
fn seats(set: &Set) -> Vec<f64> {
    set.glyphs.iter().flat_map(|glyph| [glyph.left, glyph.baseline, glyph.angle]).collect()
}

/// The points of the design's elements, in sewing order: x and y of each.
fn points(set: &Set) -> Vec<f64> {
    set.design.elements().iter().flat_map(|element| element.shape.path().points().flat_map(|p| [p.x(), p.y()]).collect::<Vec<_>>()).collect()
}

/// What setting said, as users read it.
fn said(set: &Set) -> Vec<String> {
    set.warnings.iter().map(ToString::to_string).collect()
}

/// The ids of the design's elements, in sewing order, each the glyph's number and the path's id: `0-pa` for
/// the reader's `0-svg:pa:stroke`.
fn ids(set: &Set) -> Vec<String> {
    let short = |id: &str| id.replacen("svg:", "", 1).rsplit_once(':').map_or(id.to_string(), |(path, _)| path.to_string());
    set.design.elements().iter().map(|element| short(element.id.as_str())).collect()
}

/// The ids of the elements a trim follows.
fn trims(set: &Set) -> Vec<String> {
    let all = ids(set);
    let trimmed = set.design.elements().iter().zip(all).filter(|(element, _)| element.params.get("trim_after") == Some("true"));
    trimmed.map(|(_, id)| id).collect()
}

#[test]
fn req_font_015_a_line_lies_from_the_paths_start_centred_on_it_or_up_to_its_end() {
    let font = one_way("");
    // `a` at 0 and `b` at 5, 6 mm wide: a line of 11 mm on a path of 100 mm from (10, 50).
    let at = |position: TextPosition| seats(&along(&font, "ab", &Along { position, ..on(level(100.0)) }));
    near(&at(TextPosition::Left), &[10.0, 50.0, 0.0, 15.0, 50.0, 0.0]);
    near(&at(TextPosition::Centre), &[54.5, 50.0, 0.0, 59.5, 50.0, 0.0]);
    near(&at(TextPosition::Right), &[99.0, 50.0, 0.0, 104.0, 50.0, 0.0]);
    // The text lies in the path's millimetres: `a`'s stroke from its top left to its baseline.
    let set = along(&font, "ab", &on(level(100.0)));
    near(&points(&set), &[10.0, 40.0, 14.0, 50.0, 15.0, 40.0, 21.0, 50.0]);
    assert_eq!((said(&set), ids(&set)), (vec![], vec!["0-pa".to_string(), "1-pb".to_string()]));
    // At twice the size the boxes are twice as wide and twice as far apart, and the path stays.
    let twice = lettering::set_along(&font, "ab", &Lettering { scale: 200.0, ..Lettering::default() }, &on(level(100.0)), &Budget::DEFAULT).unwrap();
    near(&seats(&twice), &[10.0, 50.0, 0.0, 20.0, 50.0, 0.0]);
    near(&points(&twice), &[10.0, 30.0, 18.0, 50.0, 20.0, 30.0, 32.0, 50.0]);
    near(&twice.glyphs.iter().map(|glyph| glyph.width).collect::<Vec<_>>(), &[8.0, 12.0]);
    // An aligned line keeps its glyphs' distances: its leftmost box starts the line.
    let centred = Lettering { align: lettering::Align::Centre, ..Lettering::default() };
    near(&seats(&lettering::set_along(&font, "ab", &centred, &on(level(100.0)), &Budget::DEFAULT).unwrap()), &[10.0, 50.0, 0.0, 15.0, 50.0, 0.0]);
}

#[test]
fn req_font_015_the_text_stands_on_the_path_lies_on_it_by_its_baseline_or_hangs_from_it() {
    let font = one_way("");
    let at = |text: &str, vertical: VerticalPosition| along(&font, text, &Along { vertical, ..on(level(100.0)) });
    // `p` reaches 3 mm below the baseline and `a` 10 mm above it.
    near(&seats(&at("ap", VerticalPosition::Baseline)), &[10.0, 50.0, 0.0, 15.0, 50.0, 0.0]);
    near(&seats(&at("ap", VerticalPosition::Top)), &[10.0, 47.0, 0.0, 15.0, 47.0, 0.0]);
    near(&seats(&at("ap", VerticalPosition::Bottom)), &[10.0, 60.0, 0.0, 15.0, 60.0, 0.0]);
    near(&points(&at("ap", VerticalPosition::Top)), &[10.0, 37.0, 14.0, 47.0, 15.0, 42.0, 19.0, 50.0]);
    near(&points(&at("ap", VerticalPosition::Bottom)), &[10.0, 50.0, 14.0, 60.0, 15.0, 55.0, 19.0, 63.0]);
    // Without a descender the baseline is the text's lowest line.
    near(&seats(&at("a", VerticalPosition::Top)), &[10.0, 50.0, 0.0]);
    near(&seats(&at("p", VerticalPosition::Bottom)), &[10.0, 55.0, 0.0]);
    // A second line lies a line's height further across the path: 20 mm. The text's lowest point is the
    // second line's, and its highest the first's.
    near(&seats(&at("a\np", VerticalPosition::Baseline)), &[10.0, 50.0, 0.0, 10.0, 70.0, 0.0]);
    near(&seats(&at("a\np", VerticalPosition::Top)), &[10.0, 27.0, 0.0, 10.0, 47.0, 0.0]);
    near(&seats(&at("p\na", VerticalPosition::Bottom)), &[10.0, 55.0, 0.0, 10.0, 75.0, 0.0]);
}

#[test]
fn req_font_015_a_glyph_is_turned_the_way_the_path_runs_under_its_box() {
    let font = one_way("");
    // Straight down: the glyph is turned a quarter, and what stood above its baseline lies right of the path.
    let down = along(&font, "a", &on(path(&[(50.0, 10.0), (50.0, 110.0)], false)));
    near(&seats(&down), &[50.0, 10.0, 90.0]);
    near(&points(&down), &[60.0, 10.0, 50.0, 14.0]);
    // Up and to the left, the other way about.
    let back = along(&font, "a", &on(path(&[(50.0, 50.0), (20.0, 10.0)], false)));
    near(&seats(&back), &[50.0, 50.0, -126.869_897_645_844_02]);
    near(&points(&back), &[42.0, 56.0, 47.6, 46.8]);
    // A slope of 3 to 4: `b` starts 5 mm along it.
    let slope = along(&font, "ab", &on(path(&[(0.0, 0.0), (30.0, 40.0)], false)));
    near(&seats(&slope), &[0.0, 0.0, 53.130_102_354_155_98, 3.0, 4.0, 53.130_102_354_155_98]);
    // Across a corner the glyph isn't bent: its baseline is the chord from where its box starts on the path
    // to where it ends, 3 mm along and 1 mm down the far side.
    let corner = along(&font, "ab", &on(path(&[(0.0, 0.0), (3.0, 0.0), (3.0, 10.0)], false)));
    near(&seats(&corner), &[0.0, 0.0, 18.434_948_822_922_01, 3.0, 2.0, 90.0]);
    // A curve is followed within a hundredth of a millimetre: a quarter circle of radius 10 from (0, 10) up
    // to (10, 0) is 15.708 mm long, and `b`, 6 mm wide, ends at its end.
    let quarter = Path {
        subpaths: vec![Subpath {
            start: Point::new(0.0, 10.0).unwrap(),
            segments: vec![Segment::Cubic(
                Point::new(0.0, 4.477_152_5).unwrap(),
                Point::new(4.477_152_5, 0.0).unwrap(),
                Point::new(10.0, 0.0).unwrap(),
            )],
            closed: false,
        }],
    };
    let round = along(&font, "b", &Along { position: TextPosition::Right, ..on(quarter) });
    let seat = seats(&round);
    assert!((seat[0] - 4.354).abs() < 0.01 && (seat[1] - 1.747).abs() < 0.01 && (seat[2] + 17.19).abs() < 0.05, "{seat:?}");
    assert_eq!(said(&round), Vec::<String>::new());
}

#[test]
fn req_font_016_a_stretched_line_shares_out_what_the_path_is_longer() {
    let font = one_way("");
    let stretched = |text: &str, length: f64| along(&font, text, &Along { position: TextPosition::Stretch, ..on(level(length)) });
    // `ab a` is 17 mm long: `a` at 0, `b` at 5 and the second `a` at 13. Of the 83 mm the path is longer, a
    // share lies between `a` and `b` and 2 between the words.
    near(&seats(&stretched("ab a", 100.0)), &[10.0, 50.0, 0.0, 15.0 + 83.0 / 3.0, 50.0, 0.0, 106.0, 50.0, 0.0]);
    // One word: a share between each 2 glyphs.
    near(&seats(&stretched("aab", 100.0)), &[10.0, 50.0, 0.0, 15.0 + 42.0, 50.0, 0.0, 104.0, 50.0, 0.0]);
    // One glyph has nothing to share, and starts at the path's start.
    near(&seats(&stretched("b", 100.0)), &[10.0, 50.0, 0.0]);
    assert_eq!(said(&stretched("ab a", 100.0)), Vec::<String>::new());
    // A path as long as the line shares out nothing.
    near(&seats(&stretched("ab", 11.0)), &[10.0, 50.0, 0.0, 15.0, 50.0, 0.0]);
    assert_eq!(said(&stretched("ab", 11.0)), Vec::<String>::new());
}

#[test]
fn diag_sc_w1109_a_line_longer_than_its_path_is_named_and_goes_on() {
    let font = one_way("");
    let at = |text: &str, path: Path, position: TextPosition| along(&font, text, &Along { position, ..on(path) });
    // On an open path the line goes on straight past the end, and before the start when it ends at the end.
    let past = at("ab", level(8.0), TextPosition::Left);
    near(&seats(&past), &[10.0, 50.0, 0.0, 15.0, 50.0, 0.0]);
    assert_eq!(
        said(&past),
        [
            "warning SC-W1109: A line of the text is 11.0 mm long, and its path is 8.0 mm long. Past the path's end it goes on straight, the way the path ends."
        ]
    );
    assert_eq!(past.warnings[0].fix, Some(Fix::Hint("Draw a longer path, set the text at a smaller scale, or write a shorter text.".to_string())));
    let before = at("ab", level(8.0), TextPosition::Right);
    near(&seats(&before), &[7.0, 50.0, 0.0, 12.0, 50.0, 0.0]);
    assert_eq!(said(&before).len(), 1);
    near(&seats(&at("ab", level(8.0), TextPosition::Centre)), &[8.5, 50.0, 0.0, 13.5, 50.0, 0.0]);
    // Past a corner it goes on the way the path ends: down.
    let turned = at("ab", path(&[(0.0, 0.0), (5.0, 0.0), (5.0, 2.0)], false), TextPosition::Left);
    near(&seats(&turned), &[0.0, 0.0, 0.0, 5.0, 0.0, 90.0]);
    // A line as long as its path isn't named, and one a fiftieth of a millimetre longer is, at either end.
    assert_eq!(said(&at("ab", level(11.0), TextPosition::Left)), Vec::<String>::new());
    assert_eq!(said(&at("ab", level(11.0), TextPosition::Right)), Vec::<String>::new());
    assert_eq!(said(&at("ab", level(10.98), TextPosition::Left)).len(), 1);
    assert_eq!(said(&at("ab", level(10.98), TextPosition::Right)).len(), 1);
    // The longest such line is the one named, once.
    let past = |set: &Set| said(set).into_iter().filter(|said| said.starts_with("warning SC-W1109")).collect::<Vec<_>>();
    let lines = at("ab\naab", level(8.0), TextPosition::Left);
    assert_eq!(past(&lines).len(), 1);
    assert!(past(&lines)[0].contains("is 16.0 mm long, and its path is 8.0 mm long"), "{:?}", said(&lines));
    assert!(past(&at("aab\nab", level(8.0), TextPosition::Left))[0].contains("is 16.0 mm long"));
    // A stretched line is pushed together to fit: `b` ends at the path's end.
    let squeezed = at("ab", level(8.0), TextPosition::Stretch);
    near(&seats(&squeezed), &[10.0, 50.0, 0.0, 12.0, 50.0, 0.0]);
    assert_eq!(
        said(&squeezed),
        ["warning SC-W1109: A line of the text is 11.0 mm long, and its path is 8.0 mm long. Its letters are pushed together to fit, and overlap."]
    );
    // A closed path is followed from its start, round its corners, and round again: a square 10 mm a side.
    let square = || path(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], true);
    let round = at("aaaaaaaaa", square(), TextPosition::Left);
    near(&seats(&round)[6..12], &[10.0, 0.0, 90.0, 10.0, 5.0, 90.0]);
    near(&seats(&round)[24..], &[0.0, 0.0, 0.0]);
    assert_eq!(
        said(&round),
        [
            "warning SC-W1109: A line of the text is 44.0 mm long, and its path is 40.0 mm long. It goes round the closed path again, over its own start."
        ]
    );
    // Before its start lies its end: a line that ends at the start comes up the last side.
    near(&seats(&at("a", square(), TextPosition::Right)), &[0.0, 4.0, -90.0]);
    assert_eq!(said(&at("aaaaaaaa", square(), TextPosition::Left)), Vec::<String>::new());
}

#[test]
fn req_font_016_a_path_without_length_is_refused_and_the_budget_is_charged() {
    let font = one_way("");
    let refused = |path: Path| lettering::set_along(&font, "a", &Lettering::default(), &on(path), &Budget::DEFAULT).unwrap_err().to_string();
    let message = "error SC-E1108: The path to set the text along has no length: it has no subpath, or its first doesn't move.";
    assert_eq!(refused(Path { subpaths: vec![] }), message);
    assert_eq!(refused(path(&[(1.0, 1.0)], false)), message);
    assert_eq!(refused(path(&[(1.0, 1.0), (1.0, 1.0)], false)), message);
    // Only the first subpath is followed.
    let mut two = level(100.0);
    two.subpaths.push(path(&[(0.0, 0.0), (0.0, 9.0)], false).subpaths.remove(0));
    near(&seats(&along(&font, "a", &on(two))), &[10.0, 50.0, 0.0]);
    // Every charge is one the text can run out at.
    let within =
        |work: u64| lettering::set_along(&font, "ab", &Lettering::default(), &on(level(100.0)), &Budget { max_work: work, ..Budget::DEFAULT });
    let enough = (0..200).find(|work| within(*work).is_ok()).unwrap();
    assert!(enough > 10, "{enough}");
    for work in 0..enough {
        assert_eq!(within(work).unwrap_err().code, Code::BudgetExhausted, "{work}");
    }
}

#[test]
fn req_font_017_every_other_line_sews_back_in_the_glyphs_drawn_for_it() {
    let font = both_ways("");
    // The second line takes the right-to-left glyphs and sews from its end, where the first line ended.
    let text = set(&font, "ab\nab\nab", &Lettering::default());
    assert_eq!(ids(&text), ["0-pa", "1-pb", "2-rb", "3-ra", "4-pa", "5-pb"]);
    assert_eq!(said(&text), Vec::<String>::new());
    // The glyphs lie where they lie one way.
    let one = set(&font, "ab\nab\nab", &Lettering { back_and_forth: false, ..Lettering::default() });
    assert_eq!(ids(&one), ["0-pa", "1-pb", "2-pa", "3-pb", "4-pa", "5-pb"]);
    assert_eq!(text.glyphs, one.glyphs);
    assert_eq!(said(&one), Vec::<String>::new());
    // An empty line counts: the third line sews as the first.
    assert_eq!(ids(&set(&font, "ab\n\nab", &Lettering::default())), ["0-pa", "1-pb", "2-pa", "3-pb"]);
    // The words of a line sewn back sew from the last.
    assert_eq!(ids(&set(&font, "a\nab p", &Lettering::default())), ["0-pa", "1-rp", "2-rb", "3-ra"]);
    // A font whose default direction is right to left sews its first line from the end, and the second from
    // its start in the left-to-right glyphs.
    let rtl = both_ways(r#", "default_variant": "rtl""#);
    assert_eq!(ids(&set(&rtl, "ab\nab", &Lettering::default())), ["0-rb", "1-ra", "2-pa", "3-pb"]);
    assert_eq!(ids(&set(&rtl, "ab\nab", &Lettering { back_and_forth: false, ..Lettering::default() })), ["0-rb", "1-ra", "2-rb", "3-ra"]);
    // Along a path the lines sew back and forth too, and lie as they lie one way.
    let on_path = along(&font, "ab\nab", &on(level(100.0)));
    assert_eq!(ids(&on_path), ["0-pa", "1-pb", "2-rb", "3-ra"]);
    near(&seats(&on_path), &[10.0, 50.0, 0.0, 15.0, 50.0, 0.0, 10.0, 70.0, 0.0, 15.0, 70.0, 0.0]);
}

#[test]
fn diag_sc_w1110_a_font_that_sews_one_way_or_does_not_sort_says_so() {
    // A font drawn for one direction: every line sews the same way, and that is said.
    let font = one_way("");
    let text = set(&font, "ab\nab", &Lettering::default());
    assert_eq!(ids(&text), ["0-pa", "1-pb", "2-pa", "3-pb"]);
    assert_eq!(
        said(&text),
        [
            "warning SC-W1110: The font `test` draws its glyphs for one direction, so the text's lines aren't sewn back and forth: every line sews the \
          same way, and the thread jumps from each line's end to the next line's start."
        ]
    );
    assert_eq!(
        text.warnings[0].fix,
        Some(Fix::Hint("Choose a font that sews in both directions, or cut the thread between the lines after sewing.".to_string()))
    );
    // A font that says it isn't reversible, though it draws both.
    let fixed = set(&both_ways(r#", "reversible": false"#), "ab\nab", &Lettering::default());
    assert_eq!(ids(&fixed), ["0-pa", "1-pb", "2-pa", "3-pb"]);
    assert!(
        said(&fixed)[0].starts_with("warning SC-W1110: The font `test` says in its `font.json` that it isn't reversible, so"),
        "{:?}",
        said(&fixed)
    );
    // Nothing is said of a text that has no second line to sew back, or that isn't asked to.
    assert_eq!(said(&set(&font, "ab", &Lettering::default())), Vec::<String>::new());
    assert_eq!(said(&set(&font, "ab\n\nab", &Lettering::default())), Vec::<String>::new());
    assert_eq!(said(&set(&font, "ab\n ", &Lettering::default())), Vec::<String>::new());
    assert_eq!(said(&set(&font, "ab\nab", &Lettering { back_and_forth: false, ..Lettering::default() })), Vec::<String>::new());
    // Colours asked to be sorted in a font that doesn't sort them.
    let unsorted = set(&font, "ab", &Lettering { color_sort: ColourSort::All, ..Lettering::default() });
    assert_eq!(
        said(&unsorted),
        [
            "warning SC-W1110: The font `test` doesn't sort its colours (`sortable` in its `font.json`), so each letter sews its colours before the next."
        ]
    );
    assert_eq!(unsorted.warnings[0].fix, Some(Fix::Hint("Leave the colour sort off, or choose a multicolour font that sorts.".to_string())));
    assert_eq!(unsorted.warnings[0].code, Code::LetteringSettingNotApplied);
}

/// A font of 2 colours, with `more` keys in its settings. `a` sews a red stroke numbered 2 and then a blue
/// one numbered 1. `b` sews a blue stroke numbered 1, a green one without a number and a red one numbered 2.
/// `t` is a tartan fill numbered 1, and `o` trims after its first stroke.
fn colours(more: &str) -> Font {
    let coloured = |id: &str, colour: &str, more: &str| stroke(id, 4.0, 60.0, 70.0, more).replace("#000000", colour);
    let numbered = |id: &str, colour: &str, number: i64| coloured(id, colour, &format!(r#"inkstitch:color_sort_index="{number}""#));
    let tartan = r#"<path id="t1" d="M 0,60 L 4,60 L 4,70 Z" style="fill:#0000ff;stroke:none" inkstitch:fill_method="tartan_fill" inkstitch:color_sort_index="1"/>"#;
    let layers = [
        layer("a", &[numbered("a1", "#ff0000", 2), numbered("a2", "#0000ff", 1)].concat()),
        layer("b", &[numbered("b1", "#0000ff", 1), coloured("b2", "#00ff00", ""), numbered("b3", "#ff0000", 2)].concat()),
        layer("t", tartan),
        layer("o", &[coloured("o1", "#ff0000", r#"inkstitch:trim_after="true""#), coloured("o2", "#ff0000", "")].concat()),
    ]
    .concat();
    font_of(&json(more), &[("ltr.svg", layers)])
}

#[test]
fn req_font_018_a_trim_follows_each_letter_word_or_line_as_chosen() {
    let font = colours("");
    let trimmed = |text: &str, trim: Trim| trims(&set(&font, text, &Lettering { trim, ..Lettering::default() }));
    // `ab a` and `b`: glyphs 0 and 1 are a word, glyph 2 the next, and glyph 3 the second line.
    assert_eq!(trimmed("ab a\nb", Trim::Off), Vec::<String>::new());
    assert_eq!(trimmed("ab a\nb", Trim::Glyph), ["0-a2", "1-b3", "2-a2", "3-b3"]);
    assert_eq!(trimmed("ab a\nb", Trim::Word), ["1-b3", "2-a2"]);
    assert_eq!(trimmed("ab a\nb", Trim::Line), ["2-a2"]);
    // The text's last word and last line have no trim after them, and one line has none.
    assert_eq!(trimmed("ab", Trim::Word), Vec::<String>::new());
    assert_eq!(trimmed("ab", Trim::Line), Vec::<String>::new());
    assert_eq!(trimmed("ab", Trim::Glyph), ["0-a2", "1-b3"]);
    // The same word of 2 lines is 2 words.
    assert_eq!(trimmed("a\na", Trim::Word), ["0-a2"]);
    // A glyph's own trim stays, whatever is chosen.
    assert_eq!(trimmed("o", Trim::Off), ["0-o1"]);
    assert_eq!(trimmed("o", Trim::Glyph), ["0-o1", "0-o2"]);
    // In a line sewn back the trims follow the glyphs as they sew: `b`, then `a`, then the first word's `a`.
    let both = both_ways("");
    let back = set(&both, "a\na ab", &Lettering { trim: Trim::Word, ..Lettering::default() });
    assert_eq!(
        (ids(&back), trims(&back)),
        (vec!["0-pa".into(), "1-rb".into(), "2-ra".into(), "3-ra".into()], vec!["0-pa".to_string(), "2-ra".to_string()])
    );
    // The colours' numbers are lettering's to read, and the design's elements have none, sorted or not.
    let numbered = |set: &Set| set.design.elements().iter().any(|element| element.params.get("color_sort_index").is_some());
    assert!(!numbered(&set(&font, "ab", &Lettering::default())));
    assert!(!numbered(&set(&colours(r#", "sortable": true"#), "ab", &Lettering { color_sort: ColourSort::All, ..Lettering::default() })));
}

#[test]
fn req_font_018_with_sorted_colours_each_colour_of_a_glyph_gets_its_trim() {
    let font = colours(r#", "sortable": true"#);
    let how = |trim: Trim, color_sort: ColourSort| Lettering { trim, color_sort, ..Lettering::default() };
    // Sorted, the text sews its colour 1, then its colour 2, then what has no number.
    let sorted = set(&font, "ab", &how(Trim::Glyph, ColourSort::All));
    assert_eq!(ids(&sorted), ["0-a2", "1-b1", "0-a1", "1-b3", "1-b2"]);
    assert_eq!(trims(&sorted), ["0-a2", "1-b1", "0-a1", "1-b3", "1-b2"]);
    // After a word, each colour of its last glyph.
    let word = set(&font, "ab a", &how(Trim::Word, ColourSort::All));
    assert_eq!(trims(&word), ["1-b1", "1-b3", "1-b2"]);
    // In a font that doesn't sort, the sort is off and the glyph's last element has the trim.
    let plain = set(&colours(""), "ab", &how(Trim::Glyph, ColourSort::All));
    assert_eq!(
        (ids(&plain), trims(&plain)),
        (vec!["0-a1".into(), "0-a2".into(), "1-b1".into(), "1-b2".into(), "1-b3".into()], vec!["0-a2".to_string(), "1-b3".to_string()])
    );
    // And so with the sort off in a font that sorts.
    assert_eq!(trims(&set(&font, "ab", &how(Trim::Glyph, ColourSort::Off))), ["0-a2", "1-b3"]);
}

#[test]
fn req_font_019_the_colours_of_a_font_that_sorts_sew_in_the_order_of_their_numbers() {
    let font = colours(r#", "sortable": true"#);
    let sorted = |text: &str, color_sort: ColourSort| set(&font, text, &Lettering { color_sort, ..Lettering::default() });
    // Off: each glyph sews its elements as they are drawn.
    assert_eq!(ids(&sorted("ab a\nb", ColourSort::Off)), ["0-a1", "0-a2", "1-b1", "1-b2", "1-b3", "2-a1", "2-a2", "3-b1", "3-b2", "3-b3"]);
    // The whole text: colour 1, then colour 2, then what has no number, each in the order it sewed in.
    let all = sorted("ab a\nb", ColourSort::All);
    assert_eq!(ids(&all), ["0-a2", "1-b1", "2-a2", "3-b1", "0-a1", "1-b3", "2-a1", "3-b3", "1-b2", "3-b2"]);
    let threads: Vec<_> = all.design.elements().iter().map(|element| element.thread.clone()).collect();
    assert!(threads[..4].iter().all(|thread| *thread == threads[0]) && threads[4..8].iter().all(|thread| *thread == threads[4]));
    assert!(threads[0] != threads[4] && threads[8] == threads[9] && threads[8] != threads[0] && threads[8] != threads[4]);
    assert!(all.warnings.iter().all(|said| !said.message.contains("sort")), "{:?}", said(&all));
    // Each line, and each word.
    assert_eq!(ids(&sorted("ab a\nb", ColourSort::Line)), ["0-a2", "1-b1", "2-a2", "0-a1", "1-b3", "2-a1", "1-b2", "3-b1", "3-b3", "3-b2"]);
    assert_eq!(ids(&sorted("ab a\nb", ColourSort::Word)), ["0-a2", "1-b1", "0-a1", "1-b3", "1-b2", "2-a2", "2-a1", "3-b1", "3-b3", "3-b2"]);
    // The same word of 2 lines is 2 words, and 2 words of a line are sorted apart though a line lies between
    // words of the same number.
    assert_eq!(ids(&sorted("a\na", ColourSort::Word)), ["0-a2", "0-a1", "1-a2", "1-a1"]);
    assert_eq!(ids(&sorted("a a\na", ColourSort::Line)), ["0-a2", "1-a2", "0-a1", "1-a1", "2-a2", "2-a1"]);
    // The places stay: sorting moves no glyph.
    assert_eq!(all.glyphs, sorted("ab a\nb", ColourSort::Off).glyphs);
    // A number that isn't one sorts as none.
    let odd = font_of(
        &json(r#", "sortable": true"#),
        &[(
            "ltr.svg",
            layer(
                "a",
                &[
                    stroke("x", 4.0, 60.0, 70.0, r#"inkstitch:color_sort_index="first""#),
                    stroke("y", 4.0, 60.0, 70.0, r#"inkstitch:color_sort_index=" 405 ""#),
                    stroke("z", 4.0, 60.0, 70.0, r#"inkstitch:color_sort_index="-1""#),
                ]
                .concat(),
            ),
        )],
    );
    assert_eq!(ids(&set(&odd, "a", &Lettering { color_sort: ColourSort::All, ..Lettering::default() })), ["0-z", "0-x", "0-y"]);
}

#[test]
fn req_font_019_the_elements_of_a_combined_number_become_one() {
    let font = colours(r#", "sortable": true, "combine_at_sort_indices": [1, 7]"#);
    let all = Lettering { color_sort: ColourSort::All, ..Lettering::default() };
    // Colour 1's 3 strokes become the first's, with the others' subpaths in their order. Colour 2 stays.
    let text = set(&font, "aba", &all);
    assert_eq!(ids(&text), ["0-a2", "0-a1", "1-b3", "2-a1", "1-b2"]);
    let first = text.design.elements()[0].shape.path();
    assert_eq!(first.subpaths.len(), 3);
    near(&first.points().map(|point| point.x()).collect::<Vec<_>>(), &[0.0, 4.0, 5.0, 9.0, 10.0, 14.0]);
    assert_eq!(text.design.elements()[0].params.get("stop_at_ending_point"), None);
    // Each line's are combined apart.
    assert_eq!(ids(&set(&font, "a\na", &Lettering { color_sort: ColourSort::Line, ..Lettering::default() })), ["0-a2", "0-a1", "1-a2", "1-a1"]);
    // A tartan fill combined ends where it is told to, as Ink/Stitch's lettering sets it.
    let tartan = set(&font, "tt", &all);
    assert_eq!(ids(&tartan), ["0-t1"]);
    assert_eq!(tartan.design.elements()[0].shape.path().subpaths.len(), 2);
    assert_eq!(tartan.design.elements()[0].params.get("stop_at_ending_point"), Some("true"));
    // Joining charges the budget: a text that sets with a unit to spare for it.
    let within = |work: u64| lettering::set(&font, "tt", &all, &Budget { max_work: work, ..Budget::DEFAULT });
    let enough = (0..200).find(|work| within(*work).is_ok()).unwrap();
    for work in 0..enough {
        assert_eq!(within(work).unwrap_err().code, Code::BudgetExhausted, "{work}");
    }
    let unsorted = |work: u64| lettering::set(&font, "tt", &Lettering::default(), &Budget { max_work: work, ..Budget::DEFAULT });
    assert!(unsorted(enough - 1).is_ok() && unsorted(enough - 2).is_err());
}

#[test]
fn req_font_019_auto_satin_routes_before_the_trims_and_the_sort() {
    // 2 satin columns, 2 mm wide and 10 mm tall, 6 mm apart: the route joins them, and each glyph's last
    // piece has the trim.
    let column =
        |id: &str| format!(r#"<path id="{id}" d="M 0,60 L 0,70 M 2,60 L 2,70" style="fill:none;stroke:#000000" inkstitch:satin_column="true"/>"#);
    let font = font_of(&json("").replace(r#""auto_satin": false"#, r#""auto_satin": true"#), &[("ltr.svg", layer("s", &column("ps")))]);
    let text = set(&font, "ss s", &Lettering { trim: Trim::Glyph, ..Lettering::default() });
    let (all, trimmed) = (ids(&text), trims(&text));
    assert!(all.len() > 3, "{all:?}");
    assert_eq!(trimmed.len(), 3);
    for glyph in ["0-", "1-", "2-"] {
        let last = all.iter().rposition(|id| id.starts_with(glyph)).unwrap();
        let has = text.design.elements()[last].params.get("trim_after");
        assert_eq!(has, Some("true"), "{all:?}");
    }
    let words = set(&font, "ss s", &Lettering { trim: Trim::Word, ..Lettering::default() });
    assert_eq!(trims(&words).len(), 1);
    assert!(trims(&words)[0].starts_with("1-"));
}

#[test]
fn req_font_020_the_settings_are_named_as_ink_stitch_names_them() {
    let settings: Vec<(&str, &str, &str)> = SETTINGS.iter().map(|setting| (setting.name, setting.takes, setting.default)).collect();
    assert_eq!(settings[..3], [("scale", "PERCENT", "100"), ("color_sort", "off|all|line|word", "off"), ("trim", "off|line|word|glyph", "off")]);
    assert_eq!(
        settings[7..],
        [
            ("text_position", "left|center|right|stretch", "left"),
            ("vertical_text_position", "top|baseline|bottom", "top"),
            ("back_and_forth", "true|false", "true")
        ]
    );
    // Each choice is named by its id, which the setting's help lists, and its default is the type's.
    let takes = |ids: Vec<&str>| ids.join("|");
    assert_eq!(takes(Trim::ALL.map(Trim::id).to_vec()), lettering::TRIM.takes);
    assert_eq!(takes(ColourSort::ALL.map(ColourSort::id).to_vec()), lettering::COLOR_SORT.takes);
    assert_eq!(takes(TextPosition::ALL.map(TextPosition::id).to_vec()), lettering::TEXT_POSITION.takes);
    assert_eq!(takes(VerticalPosition::ALL.map(VerticalPosition::id).to_vec()), lettering::VERTICAL_TEXT_POSITION.takes);
    assert!(Trim::ALL.into_iter().all(|trim| Trim::named(trim.id()) == Some(trim)) && Trim::named("letter").is_none());
    assert!(ColourSort::ALL.into_iter().all(|sort| ColourSort::named(sort.id()) == Some(sort)) && ColourSort::named("text").is_none());
    assert!(
        TextPosition::ALL.into_iter().all(|position| TextPosition::named(position.id()) == Some(position)) && TextPosition::named("centre").is_none()
    );
    assert!(VerticalPosition::ALL.into_iter().all(|position| VerticalPosition::named(position.id()) == Some(position)));
    assert!(VerticalPosition::named("left").is_none());
    let how = Lettering::default();
    assert_eq!((Trim::named(lettering::TRIM.default), ColourSort::named(lettering::COLOR_SORT.default)), (Some(how.trim), Some(how.color_sort)));
    assert_eq!(lettering::BACK_AND_FORTH.default.parse(), Ok(how.back_and_forth));
    assert_eq!(TextPosition::named(lettering::TEXT_POSITION.default), Some(TextPosition::default()));
    assert_eq!(VerticalPosition::named(lettering::VERTICAL_TEXT_POSITION.default), Some(VerticalPosition::default()));
    assert_eq!((Trim::default(), ColourSort::default()), (Trim::Off, ColourSort::Off));
}

/// The diagnostics a text may be refused with.
fn refusal(diagnostic: &Diagnostic) -> bool {
    matches!(diagnostic.code, Code::LetteringUnset | Code::BudgetExhausted)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn req_font_016_no_text_path_or_setting_panics(
        text in "[abp \n]{0,12}",
        points in proptest::collection::vec((-1e3..1e3f64, -1e3..1e3f64), 0..6),
        closed in any::<bool>(),
        position in 0..4usize,
        vertical in 0..3usize,
        trim in 0..4usize,
        sort in 0..4usize,
        back_and_forth in any::<bool>(),
        scale in prop_oneof![Just(f64::NAN), Just(f64::INFINITY), -1e3..1e3f64],
    ) {
        let font = colours(r#", "sortable": true, "combine_at_sort_indices": [1]"#);
        let at = |(x, y): &(f64, f64)| Point::new(*x, *y).unwrap();
        let subpaths = match points.split_first() {
            Some((start, rest)) => vec![Subpath { start: at(start), segments: rest.iter().map(|point| Segment::Line(at(point))).collect(), closed }],
            None => vec![],
        };
        let along = Along { path: Path { subpaths }, position: TextPosition::ALL[position], vertical: VerticalPosition::ALL[vertical] };
        let how = Lettering { scale, back_and_forth, trim: Trim::ALL[trim], color_sort: ColourSort::ALL[sort], ..Lettering::default() };
        match lettering::set_along(&font, &text, &how, &along, &Budget::DEFAULT) {
            Ok(set) => prop_assert!(set.glyphs.iter().all(|glyph| glyph.left.is_finite() && glyph.baseline.is_finite() && glyph.angle.is_finite())),
            Err(refused) => prop_assert!(refusal(&refused), "{refused}"),
        }
    }
}
