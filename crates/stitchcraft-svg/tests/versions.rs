//! `REQ-SVG-007`: a file an older Ink/Stitch saved is read as Ink/Stitch updates it. Ink/Stitch numbers the
//! files it saves (`inkstitch_svg_version`, 4 at `d59c9ab`) and updates a file that an older version saved
//! and that has something of its own: a shape with a fill that sets no longest stitch gets 3 mm, the
//! default before version 4, and a polyline whose `style` sets a stroke in a file of version 1 is a manual
//! stitch. Ink/Stitch at `d59c9ab`, run as a black box (ADR-0001, decision 6), sewed a red rectangle of a
//! version 3 file in stitches 3 mm long, and the same rectangle of a version 4 file, or of a file with
//! nothing of Ink/Stitch's, in stitches 4 mm long.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{svg, usual_stitch, warnings};
use stitchcraft_core::{Budget, Code};
use stitchcraft_engine::design::Reading;
use stitchcraft_svg::{Svg, read_with};

/// A drawing in millimetres with the namespaces Ink/Stitch's files declare, `metadata` and then `body`.
fn drawing(metadata: &str, body: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkstitch="http://inkstitch.org/namespace"
             width="100mm" height="100mm" viewBox="0 0 100 100">{metadata}{body}</svg>"#
    )
}

/// The metadata of a file of Ink/Stitch's file version `text`, as Ink/Stitch writes it.
fn version(text: &str) -> String {
    format!("<metadata><inkstitch:inkstitch_svg_version>{text}</inkstitch:inkstitch_svg_version></metadata>")
}

/// Each element's id and the value it gives `key`.
fn set<'s>(svg: &'s Svg, key: &str) -> Vec<(&'s str, Option<&'s str>)> {
    svg.design.elements().iter().map(|e| (e.id.as_str(), e.params.get(key))).collect()
}

const PATCH: &str = r#"<rect id="patch" x="10" y="10" width="50" height="30" fill="red"/>"#;

#[test]
fn req_svg_007_the_fills_of_a_file_before_version_4_sew_3_mm_stitches() {
    let old = svg(drawing(&version("3"), PATCH));
    assert_eq!(set(&old, "max_stitch_length_mm"), [("svg:patch:fill", Some("3"))]);
    assert_eq!(
        warnings(&old),
        ["info SC-I0818: The file was saved by an older Ink/Stitch (file version 3), and is read as Ink/Stitch updates it: 1 shape with a \
          fill and no longest stitch of its own sews 3 mm stitches (`max_stitch_length_mm`), the default before version 4."]
    );
    assert_eq!(usual_stitch(&old), "3.000");
    // A file of version 4, and one with nothing of Ink/Stitch's, sew the default 4 mm.
    for metadata in [version("4"), version(" 4\n"), String::new()] {
        let current = svg(drawing(&metadata, PATCH));
        assert_eq!((set(&current, "max_stitch_length_mm"), warnings(&current)), (vec![("svg:patch:fill", None)], vec![]), "{metadata}");
        assert_eq!(usual_stitch(&current), "4.000");
    }
}

#[test]
fn req_svg_007_a_shape_that_sets_its_own_longest_stitch_keeps_it() {
    // Ink/Stitch's update gives 3 mm to each shape with a fill whose own longest stitch is missing, empty,
    // not a number or 0, on the whole shape: its stroke reads it too, as a manual stitch or a satin column
    // would. A shape without a fill, or whose fill paints nothing, gets nothing.
    let body = r#"<rect id="own" x="0" y="0" width="10" height="10" fill="red" inkstitch:max_stitch_length_mm="2.5"/>
        <rect id="zero" x="0" y="20" width="10" height="10" fill="red" inkstitch:max_stitch_length_mm="0"/>
        <rect id="word" x="0" y="40" width="10" height="10" fill="red" inkstitch:max_stitch_length_mm=" three "/>
        <rect id="empty" x="0" y="60" width="10" height="10" fill="red" stroke="blue" inkstitch:max_stitch_length_mm=""/>
        <path id="line" d="M 20 0 L 90 0" fill="none" stroke="blue"/>
        <rect id="clear" x="20" y="20" width="10" height="10" fill="red" fill-opacity="0" stroke="blue"/>"#;
    for v in ["1", "2", "3"] {
        let old = svg(drawing(&version(v), body));
        assert_eq!(
            set(&old, "max_stitch_length_mm"),
            [
                ("svg:own:fill", Some("2.5")),
                ("svg:zero:fill", Some("3")),
                ("svg:word:fill", Some("3")),
                ("svg:empty:fill", Some("3")),
                ("svg:empty:stroke", Some("3")),
                ("svg:line:stroke", None),
                ("svg:clear:stroke", None),
            ],
            "version {v}"
        );
        assert!(warnings(&old)[0].starts_with(&format!("info SC-I0818: The file was saved by an older Ink/Stitch (file version {v}), ")));
        assert!(warnings(&old)[0].contains(": 3 shapes with a fill and no longest stitch of their own sew 3 mm stitches"));
    }
}

#[test]
fn req_svg_007_a_polyline_of_version_1_whose_style_sets_a_stroke_is_a_manual_stitch() {
    // Ink/Stitch's update to version 2 makes such a polyline a manual stitch: a needle point on each of its
    // points. The `style` attribute's last stroke counts; a stroke set as an attribute, or on another
    // shape, is not looked at.
    let body = r#"<polyline id="styled" points="0,0 20,5 40,0" style="fill:none;stroke:none;stroke:red"/>
        <polyline id="attribute" points="0,10 20,15 40,10" fill="none" stroke="red"/>
        <path id="path" d="M 0 30 L 20 35 L 40 30" style="fill:none;stroke:red"/>"#;
    let old = svg(drawing(&version("1"), body));
    let manual = Some("manual_stitch");
    assert_eq!(set(&old, "stroke_method"), [("svg:styled:stroke", manual), ("svg:attribute:stroke", None), ("svg:path:stroke", None)]);
    assert_eq!(
        warnings(&old),
        ["info SC-I0818: The file was saved by an older Ink/Stitch (file version 1), and is read as Ink/Stitch updates it: 1 polyline whose \
          `style` sets a stroke sews as a manual stitch (`stroke_method`), as before version 2."]
    );
    // From version 2, a polyline is sewn as any other stroke.
    let newer = svg(drawing(&version("2"), body));
    assert!(set(&newer, "stroke_method").iter().all(|(_, method)| method.is_none()));
    assert!(warnings(&newer).is_empty(), "{:?}", warnings(&newer));
    // Both updates at once.
    let both = svg(drawing(&version("1"), &format!("{body}{PATCH}<polyline points='0,40 9,40 9,45' fill='green' style='stroke:red'/>")));
    assert_eq!(
        warnings(&both),
        ["info SC-I0818: The file was saved by an older Ink/Stitch (file version 1), and is read as Ink/Stitch updates it: 2 shapes with a \
          fill and no longest stitch of their own sew 3 mm stitches (`max_stitch_length_mm`), the default before version 4, and 2 \
          polylines whose `style` sets a stroke sew as manual stitches (`stroke_method`), as before version 2."]
    );
}

#[test]
fn req_svg_007_a_shape_digitized_automatically_sews_by_the_rules() {
    // With `--auto`, a shape without Ink/Stitch's attributes is digitized by StitchCraft's own rules, and
    // the update leaves it as it is; a shape with them is updated.
    let body = format!(r#"{PATCH}<rect id="set" x="0" y="50" width="10" height="10" fill="red" inkstitch:fill_underlay="true"/>"#);
    let auto = read_with(drawing(&version("3"), &body).as_bytes(), &Budget::DEFAULT, Reading { auto: true, ..Reading::default() }).unwrap();
    assert_eq!(set(&auto, "max_stitch_length_mm"), [("svg:patch:fill", None), ("svg:set:fill", Some("3"))]);
    assert!(warnings(&auto)[0].contains(": 1 shape with a fill"), "{:?}", warnings(&auto));
}

#[test]
fn req_svg_007_where_ink_stitch_finds_the_version_and_its_own_content() {
    // The version is the text of the first element of that name anywhere, in any namespace, read as a
    // whole number. A file is Ink/Stitch's when it has an element or an attribute in Ink/Stitch's
    // namespace, the version's own element among them, or an `embroider_` attribute, as Ink/Stitch's early
    // versions wrote its settings. A file with none of them is a plain drawing, whatever version it says.
    let plain_version = "<metadata><inkstitch_svg_version>3</inkstitch_svg_version></metadata>";
    let cases = [
        (plain_version.to_string(), "", None),
        (plain_version.to_string(), r#"<rect width="1" height="1" fill="none" stroke="red" embroider_satin_column="yes"/>"#, Some("3")),
        (format!("<g>{}</g>{}", version("2"), version("4")), "", Some("3")),
        (format!("<metadata><inkstitch:inkstitch_svg_version><b>2</b></inkstitch:inkstitch_svg_version></metadata>{}", version("4")), "", Some("3")),
        (format!("<metadata><inkstitch:inkstitch_svg_version/></metadata>{}", version("2")), "", Some("3")),
    ];
    for (metadata, extra, expected) in cases {
        let read = svg(drawing(&metadata, &format!("{PATCH}{extra}")));
        assert_eq!(set(&read, "max_stitch_length_mm")[0], ("svg:patch:fill", expected), "{metadata}{extra}");
    }
}

#[test]
fn req_svg_007_a_file_without_a_version_or_from_a_newer_ink_stitch_is_read_as_it_is() {
    // Ink/Stitch asks its user what a file with its settings and no version is: an old design, which it
    // updates, or objects pasted into a new file, which it leaves. StitchCraft can't ask, and leaves it,
    // and says so; a version that does not read as a whole number counts as none.
    let unversioned = "warning SC-W0802: The file has Ink/Stitch's settings but no Ink/Stitch file version, so it is read as it is. \
                       Ink/Stitch asks first: it updates an old design, saved before Ink/Stitch numbered its files, to sew as it did, and \
                       leaves objects pasted into a new file as they are.";
    let set_here = r#"<rect id="set" x="0" y="50" width="10" height="10" fill="red" inkstitch:fill_underlay="true"/>"#;
    for metadata in [String::new(), version("3.0"), version("0"), version("")] {
        let read = svg(drawing(&metadata, &format!("{PATCH}{set_here}")));
        assert_eq!(set(&read, "max_stitch_length_mm"), [("svg:patch:fill", None), ("svg:set:fill", None)], "{metadata}");
        assert_eq!(warnings(&read), [unversioned], "{metadata}");
        assert_eq!(read.warnings[0].fix.as_ref().unwrap().describe(), "If it is an old design, open it in Ink/Stitch, choose Update, and save it.");
    }
    // A newer Ink/Stitch's file is read as version 4.
    let newer = svg(drawing(&version("5"), PATCH));
    assert_eq!(set(&newer, "max_stitch_length_mm"), [("svg:patch:fill", None)]);
    assert_eq!(
        warnings(&newer),
        ["warning SC-W0802: The file was saved by a newer Ink/Stitch (file version 5) than StitchCraft reads (up to version 4): what \
          that version changed is not known, and the file is read as version 4."]
    );
    assert_eq!(newer.warnings[0].fix.as_ref().unwrap().describe(), "Compare the stitches with the drawing.");
    assert!(newer.warnings.iter().all(|w| w.code == Code::SvgFeatureIgnored && w.element.is_none()));
}

#[test]
fn diag_sc_i0818_a_file_an_older_ink_stitch_saved_is_read_as_ink_stitch_updates_it() {
    let old = svg(drawing(&version("2"), &format!("{PATCH}<circle id='dot' cx='80' cy='80' r='5' fill='blue'/>")));
    let [note] = &old.warnings[..] else { panic!("{:?}", old.warnings) };
    assert_eq!(
        note.to_string(),
        "info SC-I0818: The file was saved by an older Ink/Stitch (file version 2), and is read as Ink/Stitch updates it: 2 shapes with a \
         fill and no longest stitch of their own sew 3 mm stitches (`max_stitch_length_mm`), the default before version 4."
    );
    assert_eq!((note.code, note.element.as_ref(), note.fix.as_ref()), (Code::SvgInkStitchUpdated, None, None));
    // Nothing to update, nothing said: a version 3 file of strokes only.
    assert!(svg(drawing(&version("3"), r#"<path d="M 0 0 L 9 0" stroke="red" fill="none"/>"#)).warnings.is_empty());
}

#[test]
fn req_svg_007_a_zigzag_stroke_of_a_file_before_version_3_halves_its_pull_compensation_for_each_side() {
    // From version 3 a zigzag stroke's pull compensation is its own setting, `stroke_pull_compensation_mm`,
    // which applies to each side. Ink/Stitch's update of a file of version 1 or 2 halves the old one into it,
    // over any the stroke has, unless it is 0 or doesn't read as a number. Other strokes keep theirs.
    let body = r#"<path id="zigzag" d="M 10 50 L 60 50" fill="none" stroke="blue" inkstitch:stroke_method="zigzag_stitch"
              inkstitch:pull_compensation_mm="1"/>
        <path id="spaced" d="M 10 55 L 60 55" fill="none" stroke="blue" inkstitch:stroke_method=" zigzag_stitch "
              inkstitch:pull_compensation_mm=" -0.5 " inkstitch:stroke_pull_compensation_mm="2"/>
        <path id="nought" d="M 10 60 L 60 60" fill="none" stroke="blue" inkstitch:stroke_method="zigzag_stitch"
              inkstitch:pull_compensation_mm="0.0"/>
        <path id="zero" d="M 10 65 L 60 65" fill="none" stroke="blue" inkstitch:stroke_method="zigzag_stitch"
              inkstitch:pull_compensation_mm="0"/>
        <path id="word" d="M 10 70 L 60 70" fill="none" stroke="blue" inkstitch:stroke_method="zigzag_stitch"
              inkstitch:pull_compensation_mm="wide"/>
        <path id="running" d="M 10 75 L 60 75" fill="none" stroke="blue" inkstitch:pull_compensation_mm="1"/>"#;
    for v in ["1", "2"] {
        let old = svg(drawing(&version(v), body));
        assert_eq!(
            set(&old, "stroke_pull_compensation_mm"),
            [
                ("svg:zigzag:stroke", Some("0.5")),
                ("svg:spaced:stroke", Some("-0.25")),
                ("svg:nought:stroke", Some("0")),
                ("svg:zero:stroke", None),
                ("svg:word:stroke", None),
                ("svg:running:stroke", None)
            ],
            "version {v}"
        );
        assert_eq!(set(&old, "pull_compensation_mm")[0], ("svg:zigzag:stroke", Some("1")));
        assert_eq!(
            warnings(&old),
            [format!(
                "info SC-I0818: The file was saved by an older Ink/Stitch (file version {v}), and is read as Ink/Stitch updates it: 3 \
                 zigzag strokes' pull compensations are halved for each side (`stroke_pull_compensation_mm`), as they apply to both \
                 sides from version 3."
            )]
        );
    }
    let newer = svg(drawing(&version("3"), body));
    assert_eq!(set(&newer, "stroke_pull_compensation_mm")[..2], [("svg:zigzag:stroke", None), ("svg:spaced:stroke", Some("2"))]);
    assert!(newer.warnings.is_empty(), "{:?}", warnings(&newer));
}

#[test]
fn req_svg_007_a_satin_column_of_a_file_before_version_3_starts_and_ends_where_it_is_drawn() {
    // Ink/Stitch's update of a file of version 1 or 2 turns a satin column's nearest points off, over its own
    // settings: it starts and ends where it is drawn, as before. A stroke that is no satin column keeps them.
    let body = r#"<path id="satin" d="M 10 70 L 60 70 M 10 74 L 60 74" fill="none" stroke="blue" inkstitch:satin_column="yes"/>
        <path id="own" d="M 10 80 L 60 80 M 10 84 L 60 84" fill="none" stroke="blue" inkstitch:satin_column=" True "
              inkstitch:start_at_nearest_point="true"/>
        <path id="line" d="M 10 90 L 60 90" fill="none" stroke="blue" inkstitch:satin_column="no" inkstitch:end_at_nearest_point="yes"/>"#;
    let old = svg(drawing(&version("2"), body));
    let off = Some("false");
    assert_eq!(set(&old, "start_at_nearest_point"), [("svg:satin:stroke", off), ("svg:own:stroke", off), ("svg:line:stroke", None)]);
    assert_eq!(set(&old, "end_at_nearest_point"), [("svg:satin:stroke", off), ("svg:own:stroke", off), ("svg:line:stroke", Some("yes"))]);
    assert_eq!(
        warnings(&old),
        ["info SC-I0818: The file was saved by an older Ink/Stitch (file version 2), and is read as Ink/Stitch updates it: 2 satin \
          columns start and end where they are drawn (`start_at_nearest_point` and `end_at_nearest_point` off), as before version 3."]
    );
    let newer = svg(drawing(&version("3"), body));
    assert_eq!(set(&newer, "start_at_nearest_point"), [("svg:satin:stroke", None), ("svg:own:stroke", Some("true")), ("svg:line:stroke", None)]);
}

#[test]
fn req_svg_007_a_fill_of_a_file_before_version_4_with_auto_fill_off_is_a_legacy_fill() {
    // Before version 4 a fill with `auto_fill` off was a legacy fill. Ink/Stitch's update of a file of version
    // 1 to 3 gives it that method, over its own, and removes `auto_fill`. A blank setting is none, which
    // leaves `auto_fill` on, its default, and any text but one of Ink/Stitch's words for yes is off. A shape
    // that sews no fill keeps its setting.
    let body = r#"<rect id="off" x="10" y="10" width="20" height="10" fill="red" inkstitch:auto_fill="false"/>
        <rect id="no" x="10" y="25" width="20" height="10" fill="red" inkstitch:auto_fill=" No " inkstitch:fill_method="contour_fill"/>
        <rect id="blank" x="10" y="40" width="20" height="10" fill="red" inkstitch:auto_fill=" "/>
        <rect id="on" x="10" y="55" width="20" height="10" fill="red" inkstitch:auto_fill="Yes"/>
        <path id="line" d="M 40 10 L 90 10" fill="none" stroke="blue" inkstitch:auto_fill="false"/>"#;
    let old = svg(drawing(&version("3"), body));
    let legacy = Some("legacy_fill");
    assert_eq!(
        set(&old, "fill_method"),
        [("svg:off:fill", legacy), ("svg:no:fill", legacy), ("svg:blank:fill", None), ("svg:on:fill", None), ("svg:line:stroke", None)]
    );
    assert_eq!(
        set(&old, "auto_fill"),
        [
            ("svg:off:fill", None),
            ("svg:no:fill", None),
            ("svg:blank:fill", Some(" ")),
            ("svg:on:fill", Some("Yes")),
            ("svg:line:stroke", Some("false"))
        ]
    );
    let updated = warnings(&old);
    assert!(updated[0].ends_with(", and 2 fills with `auto_fill` off are legacy fills (`fill_method`), as before version 4."), "{updated:?}");
    let newer = svg(drawing(&version("4"), body));
    assert_eq!(set(&newer, "fill_method")[..2], [("svg:off:fill", None), ("svg:no:fill", Some("contour_fill"))]);
    assert_eq!(set(&newer, "auto_fill")[0], ("svg:off:fill", Some("false")));
}

#[test]
fn req_svg_007_the_fill_method_auto_fill_of_a_file_before_version_4_is_a_tatami_fill() {
    // Ink/Stitch named the tatami fill `auto_fill` before version 4, and its update renames it in a file of
    // version 1 to 3. A fill whose `auto_fill` is off is a legacy fill instead.
    let body = r#"<rect id="auto" x="10" y="10" width="20" height="10" fill="red" inkstitch:fill_method=" auto_fill "
              inkstitch:max_stitch_length_mm="4"/>
        <rect id="off" x="10" y="25" width="20" height="10" fill="red" inkstitch:fill_method="auto_fill" inkstitch:auto_fill="0"
              inkstitch:max_stitch_length_mm="4"/>"#;
    let old = svg(drawing(&version("1"), body));
    assert_eq!(set(&old, "fill_method"), [("svg:auto:fill", Some("tatami_fill")), ("svg:off:fill", Some("legacy_fill"))]);
    assert_eq!(
        warnings(&old),
        ["info SC-I0818: The file was saved by an older Ink/Stitch (file version 1), and is read as Ink/Stitch updates it: 1 fill with \
          `auto_fill` off is a legacy fill (`fill_method`), as before version 4, and 1 fill set to `auto_fill`, the tatami fill's name \
          before version 4, is a tatami fill (`fill_method`)."]
    );
    let newer = svg(drawing(&version("4"), body));
    assert_eq!(set(&newer, "fill_method"), [("svg:auto:fill", Some(" auto_fill ")), ("svg:off:fill", Some("auto_fill"))]);
}

#[test]
fn req_svg_007_every_update_of_a_file_of_version_1_at_once() {
    // A shape gets each update of each version after its file's, in the order of the versions: a polyline
    // that is a zigzag stroke is a manual stitch before its pull compensation would be halved.
    let body = r#"<polyline id="poly" points="0,0 20,5 40,0" style="fill:none;stroke:red" inkstitch:stroke_method="zigzag_stitch"
              inkstitch:pull_compensation_mm="1"/>
        <path id="zigzag" d="M 10 50 L 60 50" fill="none" stroke="blue" inkstitch:stroke_method="zigzag_stitch"
              inkstitch:pull_compensation_mm="1"/>
        <path id="satin" d="M 10 70 L 60 70 M 10 74 L 60 74" fill="none" stroke="blue" inkstitch:satin_column="yes"/>
        <rect id="legacy" x="70" y="10" width="20" height="20" fill="red" inkstitch:auto_fill="false"/>
        <rect id="renamed" x="70" y="40" width="20" height="20" fill="red" inkstitch:fill_method="auto_fill"/>"#;
    let old = svg(drawing(&version("1"), body));
    assert_eq!(set(&old, "stroke_method")[0], ("svg:poly:stroke", Some("manual_stitch")));
    assert_eq!(set(&old, "stroke_pull_compensation_mm")[..2], [("svg:poly:stroke", None), ("svg:zigzag:stroke", Some("0.5"))]);
    assert_eq!(
        warnings(&old),
        ["info SC-I0818: The file was saved by an older Ink/Stitch (file version 1), and is read as Ink/Stitch updates it: 2 shapes with a \
          fill and no longest stitch of their own sew 3 mm stitches (`max_stitch_length_mm`), the default before version 4, and 1 \
          polyline whose `style` sets a stroke sews as a manual stitch (`stroke_method`), as before version 2, and 1 zigzag stroke's \
          pull compensation is halved for each side (`stroke_pull_compensation_mm`), as it applies to both sides from version 3, and 1 \
          satin column starts and ends where it is drawn (`start_at_nearest_point` and `end_at_nearest_point` off), as before version 3, \
          and 1 fill with `auto_fill` off is a legacy fill (`fill_method`), as before version 4, and 1 fill set to `auto_fill`, the \
          tatami fill's name before version 4, is a tatami fill (`fill_method`)."]
    );
}
