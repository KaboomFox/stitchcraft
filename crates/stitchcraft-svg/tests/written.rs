//! `REQ-CLI-025`: the SVG file `stitch expand` writes (`stitchcraft_svg::written`, `Params::All`) keeps every
//! setting of an expanded part, StitchCraft's own among them, its name, and what the cap holds the design
//! to, and StitchCraft's reader reads them back as they were.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::indexing_slicing)]

use stitchcraft_core::Budget;
use stitchcraft_engine::auto::Held;
use stitchcraft_engine::design::{DesignSettings, Element};
use stitchcraft_params::ParamSet;
use stitchcraft_svg::written::{Options, svg};
use stitchcraft_testkit::designs::{RED, fill};

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
