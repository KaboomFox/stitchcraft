//! The embroidery fonts built into the plug-ins (roadmap step M14.4, `REQ-FONT-021`, ADR-0020).
//!
//! **Why built in.** A plug-in runs in VectorCraft's sandbox and reads no file, so an exporter holds the
//! fonts that the texts of its documents sew in. The build script builds them in from the folder the build
//! option `STITCHCRAFT_FONTS_DIR` names, those that `built-in-fonts.txt` lists, and none without the option.
//! No font is in this repository.
//!
//! **Why only some.** A published plug-in shares the fonts in it, so a font is listed only when its licence
//! lets a copy be shared with the licence's text beside it. Each font's licence file is built in with it, and
//! whoever publishes the modules publishes it beside them, as the browser demo's build does. A font adds its glyph files' size to each
//! exporter: the linker leaves the fonts out of the modules that don't sew a document.

use stitchcraft_vectorcraft::lettering::{BuiltIn, Folder};

include!(concat!(env!("OUT_DIR"), "/fonts.rs"));

/// The fonts built in, for a document's reader.
pub fn built_in() -> BuiltIn {
    BuiltIn::new(FONTS)
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_svg::font::{self, Licence};
    use stitchcraft_vectorcraft::lettering::Fonts;

    use super::*;

    #[test]
    fn req_font_021_each_font_built_in_reads_and_may_be_shared_with_its_licence_beside_it() {
        if FONTS.is_empty() {
            // Built without STITCHCRAFT_FONTS_DIR: no font is built in, and there is nothing to check.
            assert!(built_in().names().is_empty());
            eprintln!("skipped: no font is built in (STITCHCRAFT_FONTS_DIR is not set)");
            return;
        }
        let listed: Vec<&str> =
            include_str!("../built-in-fonts.txt").lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
        assert_eq!(FONTS.iter().map(|folder| folder.name).collect::<Vec<_>>(), listed);
        for folder in FONTS {
            let font = font::read(folder.name, folder.files, &Budget::DEFAULT).unwrap();
            let licence = font.licence.as_ref().map(|file| file.licence);
            assert!(matches!(licence, Some(Licence::Ofl | Licence::PublicDomain)), "{}: {licence:?}", folder.name);
            assert!(folder.files.iter().all(|(path, _)| path.ends_with(".svg") || *path == font::SETTINGS || font::licence::is_licence_file(path)));
        }
        // The first is the default font, which a Lettering effect without a font sews in.
        assert!(built_in().font("", &Budget::DEFAULT).is_some_and(|font| font.is_ok()));
    }

    #[test]
    fn req_font_022_the_fonts_held_are_those_listed_with_the_weight_and_the_heights_each_measures() {
        use stitchcraft_svg::lettering::pictures::weight;
        use stitchcraft_vectorcraft::lettering::HELD;

        // The list the build reads and the table a font is chosen by name the same fonts, in one order.
        let listed: Vec<&str> =
            include_str!("../built-in-fonts.txt").lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
        assert_eq!(HELD.iter().map(|held| held.folder).collect::<Vec<_>>(), listed);
        if FONTS.is_empty() {
            assert!(built_in().choices().is_empty());
            eprintln!("skipped: no font is built in (STITCHCRAFT_FONTS_DIR is not set)");
            return;
        }
        assert_eq!(built_in().choices().iter().map(|choice| choice.font.as_str()).collect::<Vec<_>>(), listed);
        for (folder, held) in FONTS.iter().zip(HELD) {
            let font = font::read(folder.name, folder.files, &Budget::DEFAULT).unwrap();
            let measured = weight::of_font(&font, &Budget::DEFAULT).unwrap().unwrap();
            let (least, most) = (measured.height * font.settings.min_scale, measured.height * font.settings.max_scale);
            // The table writes the weight to 3 places and the heights to 2.
            assert!((measured.ratio() - held.weight).abs() <= 0.0005, "{}: {}", held.folder, measured.ratio());
            assert!((least - held.heights.0).abs() <= 0.005 && (most - held.heights.1).abs() <= 0.005, "{}: {least} to {most}", held.folder);
        }
    }

    /// A document of one text object, `Hi` at (72, 72) points, with the Lettering effect and its `params`.
    // A case writes the parameters in place.
    #[allow(clippy::needless_pass_by_value)]
    fn lettered(params: serde_json::Value) -> Vec<u8> {
        let effect = serde_json::json!({"id": stitchcraft_vectorcraft::effect_id("lettering"), "params": params, "visible": true});
        let kind = serde_json::json!({"type": "text", "kind": {"kind": "point"}, "xf": [1, 0, 0, 1, 72, 72], "runs": [{"text": "Hi"}]});
        let text = serde_json::json!({"id": 2, "appearance": {"effects": [effect]}, "kind": kind});
        let layer = serde_json::json!({"id": 1, "kind": {"type": "layer", "children": [text]}});
        serde_json::to_vec(&serde_json::json!({"format": "vectorcraft", "version": 4, "document": {"layers": [layer]}})).unwrap()
    }

    #[test]
    fn req_vc_027_an_exporter_sews_a_text_in_a_font_built_in_and_names_them_for_one_that_is_not() {
        let export = |params| {
            let out = crate::Module::Exporter(stitchcraft_plan::FormatId::PesV1).run(&lettered(params), b"{}").unwrap();
            serde_json::from_slice::<serde_json::Value>(&out).unwrap()
        };
        // A font no build holds: the text is left out, which leaves nothing to sew, and the export says why.
        let other = export(serde_json::json!({"font": "no such font"})).to_string();
        assert!(other.starts_with("{\"error\":\"error SC-E0010: ") && other.contains(" A text was left out: warning SC-W1111"), "{other}");
        assert!(other.contains("names the font \\\"no such font\\\", which is not here."), "{other}");
        let default = export(serde_json::json!({}));
        if FONTS.is_empty() {
            assert!(default.to_string().contains("No embroidery font is here."), "{default}");
            eprintln!("skipped: no font is built in (STITCHCRAFT_FONTS_DIR is not set)");
            return;
        }
        assert!(other.contains("The fonts here are "), "{other}");
        // The default font sews the text: the file is the one the document's reader and the planner make.
        let read = stitchcraft_vectorcraft::read_in(&lettered(serde_json::json!({})), &Budget::DEFAULT, Default::default(), &mut built_in()).unwrap();
        assert!(read.design.elements().len() >= 2, "{}", read.design.elements().len());
        assert!(read.warnings.iter().all(|warning| warning.code != stitchcraft_core::Code::TextNotLettered));
        let box_mm = read.design.bounds().unwrap();
        // The text starts an inch right of the origin, and stands on its baseline an inch down.
        assert!((25.0..27.0).contains(&box_mm.min().x()) && (24.0..27.5).contains(&box_mm.max().y()), "{box_mm:?}");
        assert!(default["dataBase64"].as_str().is_some_and(|data| data.len() > 1_000), "{default}");
    }
}
