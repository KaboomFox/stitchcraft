//! `REQ-SVG-005`: Ink/Stitch's design settings. Ink/Stitch keeps settings for the whole design in the
//! file's first `<metadata>`, each an element of its namespace named after the setting, its text the
//! value. 3 of them, `collapse_len_mm`, `min_stitch_len_mm` and `min_satin_stroke_width_mm`, become the
//! design's settings, and the fourth, `rotate_on_export`, is the design's turn on export.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic)]

mod common;

use common::{svg, warnings};
use stitchcraft_core::Mm;
use stitchcraft_engine::design::{DesignSettings, Turn};

/// A document with `metadata` before a line to sew, of Ink/Stitch's current file version, which a second
/// `<metadata>` holds (`REQ-SVG-007`).
fn document(metadata: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100">
          {metadata}<metadata><inkstitch:inkstitch_svg_version>4</inkstitch:inkstitch_svg_version></metadata>
          <path d="M 10 10 L 90 10" stroke="black" fill="none"/></svg>"#
    )
}

/// `name`'s setting written as Ink/Stitch writes it, its text `value`.
fn setting(name: &str, value: &str) -> String {
    format!("<inkstitch:{name}>{value}</inkstitch:{name}>")
}

/// The settings of the design `metadata` makes, and the warnings.
fn read(metadata: &str) -> (DesignSettings, Vec<String>) {
    let svg = svg(document(metadata));
    (svg.design.settings, warnings(&svg))
}

#[test]
fn req_svg_005_ink_stitch_s_design_settings_are_the_design_s() {
    let all = [setting("collapse_len_mm", "5"), setting("min_stitch_len_mm", "0.25"), setting("min_satin_stroke_width_mm", "2.5")].concat();
    let (settings, warned) = read(&format!("<metadata>{all}</metadata>"));
    let expected = DesignSettings {
        collapse_len: Mm::new(5.0).unwrap(),
        min_stitch_len: Some(Mm::new(0.25).unwrap()),
        min_satin_stroke_width: Mm::new(2.5).unwrap(),
        ..DesignSettings::default()
    };
    assert_eq!((settings, warned), (expected, vec![]));
    // A file that sets none has the defaults, and so does one with no metadata.
    assert_eq!(read("<metadata/>"), (DesignSettings::default(), vec![]));
    assert_eq!(read(""), (DesignSettings::default(), vec![]));
    // Numbers as JSON writes them: a fraction, an exponent, spaces around; 0 is a setting too.
    for (text, mm) in [("3.0", 3.0), ("2e-1", 0.2), (" 4 ", 4.0), ("0", 0.0)] {
        let (settings, warned) = read(&format!("<metadata>{}</metadata>", setting("collapse_len_mm", text)));
        assert_eq!((settings.collapse_len.get(), warned), (mm, vec![]), "{text}");
    }
}

#[test]
fn req_svg_005_the_first_of_each_in_the_first_metadata() {
    let first = setting("collapse_len_mm", "5");
    let second = setting("collapse_len_mm", "7");
    // Of 2 elements with one name, the first counts.
    assert_eq!(read(&format!("<metadata>{first}{second}</metadata>")).0.collapse_len.get(), 5.0);
    // Only the first `<metadata>` of the file is read, wherever it is, and only its own children.
    assert_eq!(read(&format!("<g><metadata/></g><metadata>{first}</metadata>")).0.collapse_len.get(), 3.0);
    assert_eq!(read(&format!("<metadata><g>{first}</g></metadata>")).0.collapse_len.get(), 3.0);
    // Other elements there, Inkscape's description of the work among them, are not settings.
    let others = r#"<rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"/><inkstitch:inkstitch_svg_version>3</inkstitch:inkstitch_svg_version><collapse_len_mm>9</collapse_len_mm>"#;
    assert_eq!(read(&format!("<metadata>{others}</metadata>")), (DesignSettings::default(), vec![]));
}

#[test]
fn req_svg_005_a_value_that_does_not_read_keeps_the_default() {
    for bad in ["-1", "three", "", "\"3\"", "true", "inf", "NaN", "1e400"] {
        let (settings, warned) = read(&format!("<metadata>{}</metadata>", setting("min_satin_stroke_width_mm", bad)));
        assert_eq!(settings, DesignSettings::default(), "{bad}");
        let message = format!(
            "warning SC-W0802: Ink/Stitch's design setting `min_satin_stroke_width_mm`, \"{bad}\", is not a number of 0 or more; it is \
             ignored, and the design keeps its default."
        );
        assert_eq!(warned, [message]);
    }
    // The first element of the name counts, even when it does not read.
    let (settings, _) = read(&format!("<metadata>{}{}</metadata>", setting("collapse_len_mm", "x"), setting("collapse_len_mm", "5")));
    assert_eq!(settings.collapse_len, DesignSettings::COLLAPSE_LEN);
}

#[test]
fn req_svg_005_rotate_on_export_turns_the_design() {
    // Ink/Stitch's fourth design setting turns the design about the hoop's centre as it writes it into a
    // machine file, by any number of degrees, right for a positive one (`lib/extensions/output.py` at
    // d59c9ab, and its writer's matrix); its preferences offer 90 either way.
    for (value, degrees) in [("90", 90.0), ("-90", -90.0), (" 90.0 ", 90.0), ("45", 45.0), ("180", 180.0), ("0", 0.0), ("-0", 0.0)] {
        let (settings, warned) = read(&format!("<metadata>{}</metadata>", setting("rotate_on_export", value)));
        let expected = DesignSettings { rotate_on_export: Turn::new(degrees).unwrap(), ..DesignSettings::default() };
        assert_eq!((settings, warned), (expected, vec![]), "{value}");
    }
    // Text that isn't a number turns nothing, as in Ink/Stitch, which reads text that isn't JSON as no turn.
    for value in ["", "right", "inf"] {
        let (settings, warned) = read(&format!("<metadata>{}</metadata>", setting("rotate_on_export", value)));
        let message = format!(
            "warning SC-W0802: Ink/Stitch's design setting `rotate_on_export`, \"{value}\", is not a number of degrees; it is ignored, and the \
             design isn't turned."
        );
        assert_eq!((settings, warned), (DesignSettings::default(), vec![message]), "{value}");
    }
}

#[test]
fn req_svg_005_ink_stitch_s_other_metadata_is_named_unless_it_changes_nothing_sewn() {
    // The thread palette names the threads in Ink/Stitch's own lists and print-outs, which StitchCraft does
    // not write, and the file version is read with the file (`REQ-SVG-007`). Any other element of
    // Ink/Stitch's there is named once, the first of its name.
    let others = [setting("thread-palette", "\"Madeira Polyneon\""), setting("sparkle", "1"), setting("sparkle", "2")].concat();
    let (settings, warned) = read(&format!("<metadata>{others}</metadata>"));
    assert_eq!(settings, DesignSettings::default());
    assert_eq!(
        warned,
        ["warning SC-W0802: The file's metadata has Ink/Stitch's `sparkle`, which is not a design setting StitchCraft knows; it is ignored."]
    );
}
