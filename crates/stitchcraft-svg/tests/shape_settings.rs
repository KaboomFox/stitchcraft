//! `REQ-SVG-009` and `REQ-SVG-010`: a shape's Ink/Stitch settings. Ink/Stitch keeps how each shape sews in
//! attributes of its namespace on the shape's own node, each named after its setting and read from there
//! alone (`lib/elements/element.py` at `d59c9ab`). Each is a parameter of every element the shape makes, and
//! `stroke_first` sews the shape's stroke before its fill (`lib/elements/utils/nodes.py`).

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{ids, svg, usual_stitch, warnings};
use stitchcraft_core::{Budget, Code};
use stitchcraft_plan::profiles;

/// A drawing in millimetres with the namespaces Ink/Stitch's files declare and its current file version,
/// as Ink/Stitch saves it (`REQ-SVG-007`), around `body`.
fn drawing(body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink"
             xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:inkstitch="http://inkstitch.org/namespace"
             width="100mm" height="100mm" viewBox="0 0 100 100">
             <metadata><inkstitch:inkstitch_svg_version>4</inkstitch:inkstitch_svg_version></metadata>{body}</svg>"#
    )
}

/// Each element's id and its parameters, as text.
fn params(svg: &stitchcraft_svg::Svg) -> Vec<(&str, Vec<(&str, &str)>)> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.params.iter().collect())).collect()
}

#[test]
fn req_svg_009_each_ink_stitch_setting_of_a_shape_is_a_parameter_of_each_of_its_elements() {
    // A rectangle with a fill and a stroke: both its elements get every setting, as they are written, white
    // space and all, which the registry strips as it reads them (`REQ-PRM-002`). Settings under another
    // prefix of Ink/Stitch's namespace are its settings too; `stroke_first` orders the elements instead.
    let read = svg(drawing(
        r#"<rect id="patch" x="10" y="10" width="30" height="20" fill="red" stroke="blue"
                 inkstitch:fill_method="contour_fill" inkstitch:row_spacing_mm=" 0.5 " inkstitch:satin_column="True"
                 inkstitch:stroke_first="false"/>
           <path id="line" d="M 10 50 L 60 50" fill="none" stroke="green" xmlns:is="http://inkstitch.org/namespace"
                 is:running_stitch_length_mm="1.5"/>"#,
    ));
    let both = vec![("fill_method", "contour_fill"), ("row_spacing_mm", " 0.5 "), ("satin_column", "True")];
    assert_eq!(
        params(&read),
        [("svg:patch:fill", both.clone()), ("svg:patch:stroke", both), ("svg:line:stroke", vec![("running_stitch_length_mm", "1.5")])]
    );
    assert!(read.warnings.is_empty(), "{:?}", warnings(&read));
}

#[test]
fn req_svg_009_a_shape_sews_as_its_settings_say() {
    // A line 60 mm long sews running stitches 1.5 mm long when it says so, and the default 2.5 mm when not.
    let line = |settings: &str| svg(drawing(&format!(r#"<path id="line" d="M 10 50 L 70 50" fill="none" stroke="green" {settings}/>"#)));
    assert_eq!(usual_stitch(&line(r#"inkstitch:running_stitch_length_mm="1.5""#)), "1.500");
    assert_eq!(usual_stitch(&line("")), "2.500");
}

#[test]
fn req_svg_009_a_setting_stitchcraft_does_not_declare_is_kept_and_named_when_sewn() {
    let read = svg(drawing(r#"<rect id="patch" x="10" y="10" width="30" height="20" fill="red" inkstitch:cutwork_needle="2"/>"#));
    assert_eq!(params(&read), [("svg:patch:fill", vec![("cutwork_needle", "2")])]);
    let outcome = stitchcraft_engine::plan(&read.design, profiles::REFERENCE, &Budget::DEFAULT);
    let unknown: Vec<String> = outcome.diagnostics.iter().filter(|d| d.code == Code::ParamUnknown).map(ToString::to_string).collect();
    assert!(unknown.len() == 1 && unknown[0].contains("cutwork_needle"), "{:?}", outcome.diagnostics);
    assert!(outcome.plan.is_some());
}

#[test]
fn req_svg_009_a_trim_command_turns_on_trim_after_over_the_shape_s_own_setting() {
    // Ink/Stitch trims after a shape with a trim command whatever its own `trim_after` says.
    let read = svg(drawing(
        r##"<defs><symbol id="inkstitch_trim"><path d="M 0 0 L 1 1"/></symbol></defs>
            <rect id="patch" x="10" y="10" width="30" height="20" fill="red" inkstitch:trim_after="false" inkstitch:stop_after="false"/>
            <use id="trim" xlink:href="#inkstitch_trim" x="50" y="50"/>
            <path id="connector" d="M 50 50 L 25 20" style="fill:none;stroke:#000" inkscape:connection-start="#trim"
                  inkscape:connection-end="#patch"/>"##,
    ));
    assert_eq!(params(&read), [("svg:patch:fill", vec![("stop_after", "false"), ("trim_after", "true")])]);
}

#[test]
fn req_svg_009_a_group_s_settings_change_nothing_and_are_named() {
    // Ink/Stitch reads a setting from a shape's own node only: the rectangle in the group sews at the
    // defaults. `ignore_object` off is a setting Ink/Stitch reads from a group, and is not named.
    let read = svg(drawing(
        r#"<g id="group" inkstitch:row_spacing_mm="1" inkstitch:fill_method="contour_fill" inkstitch:ignore_object="false">
             <rect id="patch" x="10" y="10" width="30" height="20" fill="red"/>
           </g>
           <g id="plain" inkstitch:ignore_object="false"><rect id="other" x="10" y="40" width="30" height="20" fill="red"/></g>"#,
    ));
    assert_eq!(params(&read), [("svg:patch:fill", vec![]), ("svg:other:fill", vec![])]);
    assert_eq!(
        warnings(&read),
        ["warning SC-W0802: The group `group` has Ink/Stitch settings, `row_spacing_mm`, `fill_method`, which Ink/Stitch reads only from \
          shapes: they change nothing."]
    );
    assert_eq!(read.warnings[0].fix.as_ref().unwrap().describe(), "Set them on the shapes in the group.");
}

#[test]
fn req_svg_009_underlays_first_is_a_setting_of_a_shape_and_named_on_a_group() {
    // StitchCraft's own setting, in Ink/Stitch's namespace: on 2 crossing satin columns it is a parameter of
    // each, which sews both underlays before both tops (`REQ-ASM-009`). On a group it changes nothing, and is
    // named, as every setting of a group but `ignore_object` is.
    let column = |id: &str, d: &str| {
        format!(
            r#"<path id="{id}" d="{d}" fill="none" stroke="red" inkstitch:satin_column="true" inkstitch:contour_underlay="true"
                     inkstitch:underlays_first="true"/>"#
        )
    };
    let read = svg(drawing(&format!("{}{}", column("bar", "M 10 48 L 40 48 M 10 52 L 40 52"), column("post", "M 23 35 L 23 65 M 27 35 L 27 65"))));
    let both = vec![("contour_underlay", "true"), ("satin_column", "true"), ("underlays_first", "true")];
    assert_eq!(params(&read), [("svg:bar:stroke", both.clone()), ("svg:post:stroke", both)]);
    assert!(read.warnings.is_empty(), "{:?}", warnings(&read));
    let outcome = stitchcraft_engine::plan(&read.design, profiles::REFERENCE, &Budget::DEFAULT);
    assert!(outcome.diagnostics.is_empty(), "{:?}", outcome.diagnostics);
    let roles: Vec<stitchcraft_plan::Role> = outcome.plan.unwrap().blocks.iter().flat_map(|b| b.stitches.clone()).map(|s| s.origin.role).collect();
    let underlay = |role: &stitchcraft_plan::Role| *role == stitchcraft_plan::Role::Underlay;
    let (first_top, last_underlay) = (roles.iter().position(|r| *r == stitchcraft_plan::Role::Top), roles.iter().rposition(underlay));
    assert!(last_underlay.unwrap() < first_top.unwrap(), "every underlay stitch before every top stitch");
    // On a group: named, and the rectangle in it sews without it.
    let grouped =
        svg(drawing(r#"<g id="group" inkstitch:underlays_first="true"><rect id="patch" x="10" y="10" width="30" height="20" fill="red"/></g>"#));
    assert_eq!(params(&grouped), [("svg:patch:fill", vec![])]);
    assert_eq!(
        warnings(&grouped),
        ["warning SC-W0802: The group `group` has Ink/Stitch settings, `underlays_first`, which Ink/Stitch reads only from shapes: they change \
          nothing."]
    );
}

#[test]
fn req_svg_010_stroke_first_sews_a_shape_s_stroke_before_its_fill() {
    // Ink/Stitch's words for yes turn it on, any other text leaves it off, as for its other toggles.
    let order = |value: &str| {
        let read = svg(drawing(&format!(r#"<rect id="r" width="30" height="20" fill="red" stroke="blue" inkstitch:stroke_first="{value}"/>"#)));
        ids(&read).iter().map(|id| id.rsplit(':').next().unwrap().to_string()).collect::<Vec<_>>()
    };
    for on in ["true", "True", "yes", "1"] {
        assert_eq!(order(on), ["stroke", "fill"], "{on}");
    }
    for off in ["false", "no", "on", ""] {
        assert_eq!(order(off), ["fill", "stroke"], "{off}");
    }
    // SVG's `paint-order` doesn't change the order, and says so, unless `stroke_first` asks for it.
    let painted = |settings: &str| {
        svg(drawing(&format!(r#"<rect id="r" width="30" height="20" fill="red" stroke="blue" style="paint-order:stroke" {settings}/>"#)))
    };
    let alone = painted("");
    assert_eq!(ids(&alone), ["svg:r:fill", "svg:r:stroke"]);
    assert_eq!(warnings(&alone), ["warning SC-W0802: `r` paints its stroke first (`paint-order`); its fill is sewn first, as Ink/Stitch sews them."]);
    let asked = painted(r#"inkstitch:stroke_first="true""#);
    assert_eq!(ids(&asked), ["svg:r:stroke", "svg:r:fill"]);
    assert!(asked.warnings.is_empty(), "{:?}", warnings(&asked));
}

#[test]
fn reading_a_shape_s_settings_is_charged_to_the_budget() {
    // One unit for each setting: a shape of 40 settings needs 40 units more than the same shape with none.
    let settings: String = (0..40).map(|k| format!(r#" inkstitch:setting_{k}="{k}""#)).collect();
    let used = |body: &str| {
        let text = drawing(body);
        (1..100_000).find(|&work| stitchcraft_svg::read(text.as_bytes(), &Budget { max_stitches: 1, max_work: work }).is_ok()).unwrap()
    };
    let rect = |settings: &str| format!(r#"<rect id="r" width="30" height="20" fill="red"{settings}/>"#);
    assert_eq!(used(&rect(&settings)) - used(&rect("")), 40);
    let group = |settings: &str| format!(r#"<g id="g"{settings}><rect id="r" width="30" height="20" fill="red"/></g>"#);
    assert_eq!(used(&group(&settings)) - used(&group("")), 40);
}
