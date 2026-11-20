//! `REQ-CLI-025`: the SVG file `stitch expand` writes (`stitchcraft_svg::written`, `Params::All`) keeps every
//! setting of an expanded part, StitchCraft's own among them, its name, and what the cap holds the design
//! to, and StitchCraft's reader reads them back as they were.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use stitchcraft_core::Budget;
use stitchcraft_engine::auto::Held;
use stitchcraft_engine::design::Hints;
use stitchcraft_engine::design::{DesignSettings, Element};
use stitchcraft_params::ParamSet;
use stitchcraft_svg::written::{MARGIN, Options, Params, sewn_alike, svg};
use stitchcraft_testkit::designs::{RED, fill, p};

#[test]
fn req_cli_025_an_expanded_part_keeps_its_own_settings_its_name_and_the_design_s_cap() {
    let own = [("digitized_as", "part"), ("turn_stitches", "true"), ("underlays_first", "true"), ("angle", "30"), ("auto_overlap_mm", "1")];
    let part = Element { name: Some("Red \"fill\" (1)".to_string()), ..fill("a", &[&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0)]], &RED, &own) };
    let held = Held { cap: 3.5, ..Held::of(&ParamSet::new()).unwrap() };
    let settings = DesignSettings { cap: Some(held), ..DesignSettings::default() };
    let text = svg(std::slice::from_ref(&part), &settings, &Options::EXPANDED).unwrap();
    let read = stitchcraft_svg::read(text.as_bytes(), &Budget::DEFAULT).unwrap();
    assert!(read.warnings.is_empty(), "{:?}", read.warnings);
    let back = &read.design.elements()[0];
    // Every setting but the Auto effect's, which no stitch type reads.
    let want: Vec<(&str, &str)> = own.iter().copied().filter(|(key, _)| !key.starts_with("auto_")).collect();
    let mut got: Vec<(&str, &str)> = back.params.iter().collect();
    got.sort_unstable();
    let mut want = want;
    want.sort_unstable();
    assert_eq!(got, want);
    assert_eq!(back.name, part.name);
    assert_eq!(read.design.settings.cap, Some(held));
}

#[test]
fn the_page_reaches_past_the_design_and_the_commands_symbols_are_defined() {
    let square = fill("a", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 30.0)]], &RED, &[]);
    let text = svg(std::slice::from_ref(&square), &DesignSettings::default(), &Options::EXPANDED).unwrap();
    let (width, height) = (20.0 + MARGIN, 30.0 + MARGIN);
    assert!(text.contains(&format!("width=\"{width}mm\" height=\"{height}mm\" viewBox=\"0 0 {width} {height}\"")), "{text}");
    // A design without commands, clips or gradients has no definitions.
    assert!(!text.contains("<defs>"));
    let started = Element { hints: Hints { start: Some(p(1.0, 1.0)), ..Hints::default() }, ..square };
    let text = svg(&[started], &DesignSettings::default(), &Options::EXPANDED).unwrap();
    assert!(text.contains("<defs>\n    <symbol id=\"inkstitch_starting_point\">"), "{text}");
}

#[test]
fn the_differential_refuses_a_setting_of_stitchcraft_s_own_that_a_stitch_type_reads() {
    let first = fill("a", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 30.0)]], &RED, &[("underlays_first", "true")]);
    let differential = Options { params: Params::InkStitch, clones: &[], styled: false };
    let refused = svg(std::slice::from_ref(&first), &DesignSettings::default(), &differential).unwrap_err();
    assert_eq!(refused, "a: `underlays_first` is StitchCraft's own, and Ink/Stitch would sew the element without it");
    // An expanded design keeps it, and the differential leaves the element out of what both sew alike, but
    // not one with a setting of the Auto effect, which no stitch type reads.
    assert!(
        svg(std::slice::from_ref(&first), &DesignSettings::default(), &Options::EXPANDED).unwrap().contains("inkstitch:underlays_first=\"true\"")
    );
    let auto = fill("b", &[&[(0.0, 0.0), (20.0, 0.0), (20.0, 30.0)]], &RED, &[("auto_overlap_mm", "1")]);
    let kept: Vec<String> = sewn_alike(&[first, auto]).iter().map(|element| element.id.to_string()).collect();
    assert_eq!(kept, ["b"]);
}
