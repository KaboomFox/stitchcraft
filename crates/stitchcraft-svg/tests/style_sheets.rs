//! `REQ-SVG-006`: style sheets set an element's properties as they do in a viewer. Illustrator and
//! CorelDRAW write their colours as classes in a `<style>` element: read without it, a file would sew black
//! fills where the drawing shows outlines, and sew what the sheet hides.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

mod common;

use common::{svg, warnings};
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::Shape;

/// A drawing 100 mm square, a user unit to the millimetre, around `body`.
fn drawing(body: &str) -> String {
    format!(r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="100mm" viewBox="0 0 100 100">{body}</svg>"#)
}

/// Each element's id and colour.
fn colours(svg: &stitchcraft_svg::Svg) -> Vec<(&str, String)> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.thread.color.to_string())).collect()
}

/// The colour of the fill of a 10 mm square with `attributes`, under the style sheet `sheet`, if it has one.
fn fill(sheet: &str, attributes: &str) -> Option<String> {
    let svg = svg(drawing(&format!(r#"<style>{sheet}</style><rect width="10" height="10" {attributes}/>"#)));
    svg.design.elements().iter().find(|e| matches!(e.shape, Shape::Fill { .. })).map(|e| e.thread.color.to_string())
}

#[test]
fn req_svg_006_classes_set_paints_and_visibility_as_illustrator_writes_them() {
    let svg = svg(drawing(
        r#"<defs><style>.cls-1{fill:none;stroke:#231f20;stroke-miterlimit:10;stroke-width:2px}.cls-2{display:none}</style></defs>
           <circle id="c" class="cls-1" cx="50" cy="50" r="10"/><rect id="r" class="cls-2" width="10" height="10"/>"#,
    ));
    assert_eq!(colours(&svg), [("svg:c:stroke", "#231f20".to_string())], "one outline, and the hidden rectangle left out");
    let Shape::Stroke { width, .. } = &svg.design.elements()[0].shape else { panic!("{:?}", svg.design.elements()[0].shape) };
    assert!((width.get() - 2.0).abs() < 1e-12, "{width:?}");
    assert_eq!(warnings(&svg), Vec::<String>::new());
}

#[test]
fn req_svg_006_the_cascade_is_css_s() {
    let (red, blue, black) = (Some("#ff0000".to_string()), Some("#0000ff".to_string()), Some("#000000".to_string()));
    // A rule wins over a presentation attribute, even a rule for every element, and the style attribute
    // wins over a rule.
    assert_eq!(fill(".a{fill:red}", r#"class="a" fill="blue""#), red);
    assert_eq!(fill("*{fill:red}", r#"fill="blue""#), red);
    assert_eq!(fill(".a{fill:red}", r#"class="a" style="fill:blue""#), blue);
    // Of 2 rules, the more specific wins, in either order: an id over classes, more classes over fewer, a
    // class over a type, a type and a class over the class alone.
    for sheet in ["#x{fill:red} .a.b{fill:blue}", ".a.b{fill:blue} #x{fill:red}"] {
        assert_eq!(fill(sheet, r#"id="x" class="a b""#), red, "{sheet}");
    }
    assert_eq!(fill(".a.b{fill:red} .b{fill:blue}", r#"class="a b""#), red);
    assert_eq!(fill(".a{fill:red} rect{fill:blue}", r#"class="a""#), red);
    assert_eq!(fill("rect.a{fill:red} .a{fill:blue}", r#"class="a""#), red);
    // Of 2 rules as specific, the later; a selector list matches what any of its selectors matches.
    assert_eq!(fill(".a{fill:blue} .b{fill:red}", "class=\" b\ta \""), red);
    assert_eq!(fill(".z, rect.a {fill:red}", r#"class="a""#), red);
    // `!important` wins over the style attribute, and an important style attribute over that.
    assert_eq!(fill(".a{fill:red !important} #x{fill:blue}", r#"id="x" class="a" style="fill:blue""#), red);
    assert_eq!(fill(".a{fill:red!important}", r#"class="a" style="fill:blue !important""#), blue);
    // A declaration that does not read gives way to the next that does, as in CSS.
    assert_eq!(fill("#x{fill:bogus} .a{fill:red}", r#"id="x" class="a""#), red);
    // Names match exactly: classes, ids and element names are case-sensitive in SVG. Every part of a
    // selector must match.
    assert_eq!(fill(".A{fill:red} #X{fill:red} RECT{fill:red}", r#"id="x" class="a""#), black);
    assert_eq!(fill(".a.b{fill:red} circle.a{fill:red} #x.b{fill:red} #x#y{fill:red}", r#"id="x" class="a""#), black);
    // And a rule can hide.
    assert_eq!(fill(".a{display:none}", r#"class="a""#), None);
}

#[test]
fn req_svg_006_rules_set_groups_and_gradient_stops_too() {
    // What a rule sets on a group, its children inherit; a gradient's stops take their colours from rules.
    let svg = svg(drawing(
        r#"<style>.group{fill:#00ff00;stroke:#0000ff;stroke-width:3} .s{stop-color:#ff00ff}</style>
           <linearGradient id="g"><stop class="s" offset="0"/><stop class="s" offset="1"/></linearGradient>
           <g class="group"><rect id="inherits" width="10" height="10"/></g>
           <rect id="graded" x="20" width="10" height="10" fill="url(#g)"/>"#,
    ));
    assert_eq!(
        colours(&svg),
        [("svg:inherits:fill", "#00ff00".to_string()), ("svg:inherits:stroke", "#0000ff".to_string()), ("svg:graded:fill", "#ff00ff".to_string()),]
    );
    assert_eq!(warnings(&svg), Vec::<String>::new(), "a gradient of one colour is that colour");
}

#[test]
fn req_svg_006_sheets_as_editors_write_them() {
    // CorelDRAW writes its sheet in CDATA, with comments. A sheet applies to elements before it, a later
    // sheet's rules come after an earlier one's, and a sheet of another type than CSS is not read, as a
    // viewer reads none.
    let svg = svg(drawing(
        r#"<rect id="before" class="fil0 str0" width="10" height="10"/>
           <style type="text/css"><![CDATA[
             /* fills */ .fil0 {fill:#2B2A29}
             .str0 {stroke:#FEFEFE;stroke-width:0.5} /* outlines; .fil0 {fill:red} */
           ]]></style>
           <style type=" TEXT/CSS; charset=utf-8">.fil0 {fill:#123456}</style>
           <style type="text/x-scss">.str0 {stroke:red}</style>"#,
    ));
    assert_eq!(colours(&svg), [("svg:before:fill", "#123456".to_string()), ("svg:before:stroke", "#fefefe".to_string())]);
    assert_eq!(warnings(&svg), Vec::<String>::new());
}

#[test]
fn diag_sc_w0802_style_sheet_rules_not_read_are_named() {
    // A rule with another kind of selector, or an at-rule, sets nothing here; the rules that are read still
    // apply. A font, or the sheet's character set, changes nothing that is sewn.
    let svg = svg(drawing(
        r#"<text id="words">before</text>
           <style>
             @charset "utf-8";
             @import url(colours.css);
             @media print { .a { fill: blue } }
             g > rect, .a { fill: red }
             rect:first-child { stroke: blue }
             [id="x"] { fill: lime }
             @font-face { font-family: x; src: url(x.woff) }
           </style>
           <rect id="x" class="a" width="10" height="10"/>"#,
    ));
    assert_eq!(colours(&svg), [("svg:x:fill", "#ff0000".to_string())]);
    // The note comes where the reader meets the sheet, after what it says of the elements before it.
    assert_eq!(
        warnings(&svg),
        [
            "warning SC-W0802: `words` is text, which is not stitched; it is left out.",
            "warning SC-W0802: StitchCraft does not read `@import`, `@media`, `g > rect` and 2 more in the file's style sheet; what they set is not used.",
        ]
    );
    let note = &svg.warnings[1];
    assert_eq!((note.code, note.element.as_ref()), (Code::SvgFeatureIgnored, None));
    assert_eq!(
        note.fix.as_ref().map(|f| f.describe()).as_deref(),
        Some("Use only type, class and id selectors, or save the file with inline styles or presentation attributes.")
    );
    // One, 2 and 3 are all named.
    let named = |sheet: &str| warnings(&svg_of(sheet));
    assert_eq!(
        named("g rect{fill:red}"),
        ["warning SC-W0802: StitchCraft does not read `g rect` in the file's style sheet; what it sets is not used."]
    );
    assert_eq!(
        named("g rect{fill:red} @media print{} a:hover{}"),
        ["warning SC-W0802: StitchCraft does not read `g rect`, `@media` and `a:hover` in the file's style sheet; what they set is not used."]
    );
}

/// A drawing with the style sheet `sheet` and nothing to sew.
fn svg_of(sheet: &str) -> stitchcraft_svg::Svg {
    svg(drawing(&format!("<style>{sheet}</style>")))
}

#[test]
fn req_svg_006_reading_a_sheet_is_charged_to_the_budget() {
    // One unit per 8 bytes of a sheet's text (here 12 bytes: 2), and for each element, one per class it has,
    // per rule tested against it and per declaration it takes (here 1 each), besides two per XML node
    // (here 4 to find, 3 to read).
    let file = r#"<svg xmlns="http://www.w3.org/2000/svg"><style>.a{fill:red}</style><rect class="a" width="1" height="1"/></svg>"#;
    let budget = |max_work| Budget { max_stitches: 1, max_work };
    assert!(stitchcraft_svg::read(file.as_bytes(), &budget(12)).is_ok());
    assert_eq!(stitchcraft_svg::read(file.as_bytes(), &budget(11)).unwrap_err().code, Code::BudgetExhausted);
    // A sheet's rules are tested only against elements that could match them: by id, class or name.
    let many = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{}</style><rect class="c7" width="1" height="1"/></svg>"#,
        (0..1000).map(|i| format!(".c{i}{{fill:red}}")).collect::<String>()
    );
    let text = many.split("<style>").nth(1).unwrap().split("</style>").next().unwrap().len() as u64;
    let units = 4 + 3 + 1 + text / 8 + 3;
    assert!(stitchcraft_svg::read(many.as_bytes(), &budget(units)).is_ok(), "{units}");
    assert!(stitchcraft_svg::read(many.as_bytes(), &budget(units - 1)).is_err());
}

#[test]
fn diag_sc_e0801_style_sheets_past_a_million_items_are_refused() {
    // Selectors, declarations and at-rules, counted by what separates them: a million is read, and more is
    // refused, as more XML nodes than the reader reads are.
    let file =
        |selectors: usize| format!(r#"<svg xmlns="http://www.w3.org/2000/svg"><style>{}.a{{fill:red}}</style></svg>"#, ".a,".repeat(selectors));
    assert!(stitchcraft_svg::read(file(999_998).as_bytes(), &Budget::DEFAULT).is_ok());
    let refused = stitchcraft_svg::read(file(999_999).as_bytes(), &Budget::DEFAULT).unwrap_err();
    assert_eq!(
        refused.to_string(),
        "error SC-E0801: The file's style sheets have more than 1000000 selectors and declarations, more than StitchCraft reads."
    );
}
