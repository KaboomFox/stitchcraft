//! The embroidery fonts built into the plug-ins (roadmap step M14.4, `REQ-FONT-021`, ADR-0020).
//!
//! **Why built in.** A plug-in runs in VectorCraft's sandbox and reads no file, so an exporter holds the
//! fonts that the texts of its documents sew in. The build script builds them in from the folder the build
//! option `STITCHCRAFT_FONTS_DIR` names, those that `built-in-fonts.txt` lists, and none without the option.
//! No font is in this repository.
//!
//! **Why only some.** A published plug-in shares the fonts in it, so a font is listed only when its licence
//! lets a copy be shared with the licence's text beside it. Each font's licence file is built in with it, and
//! [`licences`] gives them to whoever publishes the modules. A font adds its glyph files' size to each
//! exporter: the linker leaves the fonts out of the modules that don't sew a document.

use stitchcraft_vectorcraft::lettering::{BuiltIn, Folder, Fonts};

include!(concat!(env!("OUT_DIR"), "/fonts.rs"));

/// The fonts built in, for a document's reader.
pub fn built_in() -> BuiltIn<'static> {
    BuiltIn::new(FONTS)
}

/// The fonts built in, each as a person names it.
pub fn names() -> Vec<String> {
    built_in().names()
}

/// Each font built in that has a licence file: its folder's name, and the file's name and bytes.
pub fn licences() -> Vec<(&'static str, &'static str, &'static [u8])> {
    let licence = |folder: &'static Folder<'static>| {
        let (path, bytes) = folder.files.iter().find(|(path, _)| stitchcraft_svg::font::licence::is_licence_file(path))?;
        Some((folder.name, *path, *bytes))
    };
    FONTS.iter().filter_map(licence).collect()
}

/// The bytes the fonts built in take, which is what they add to an exporter's module.
pub fn bytes() -> usize {
    FONTS.iter().flat_map(|folder| folder.files).map(|(_, bytes)| bytes.len()).sum()
}

#[cfg(test)]
mod tests {
    use stitchcraft_core::Budget;
    use stitchcraft_svg::font::{self, Licence};

    use super::*;

    #[test]
    fn req_font_021_each_font_built_in_reads_and_may_be_shared_with_its_licence_beside_it() {
        if FONTS.is_empty() {
            // Built without STITCHCRAFT_FONTS_DIR: no font is built in, and there is nothing to check.
            assert!(names().is_empty() && licences().is_empty() && bytes() == 0);
            eprintln!("skipped: no font is built in (STITCHCRAFT_FONTS_DIR is not set)");
            return;
        }
        let listed: Vec<&str> =
            include_str!("../built-in-fonts.txt").lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
        assert_eq!(FONTS.iter().map(|folder| folder.name).collect::<Vec<_>>(), listed);
        assert_eq!(licences().len(), FONTS.len());
        assert!(bytes() > 0);
        for folder in FONTS {
            let font = font::read(folder.name, folder.files, &Budget::DEFAULT).unwrap();
            let licence = font.licence.as_ref().map(|file| file.licence);
            assert!(matches!(licence, Some(Licence::Ofl | Licence::PublicDomain)), "{}: {licence:?}", folder.name);
            assert!(folder.files.iter().all(|(path, _)| path.ends_with(".svg") || *path == font::SETTINGS || font::licence::is_licence_file(path)));
        }
        // The first is the default font, which a Lettering effect without a font sews in.
        assert!(built_in().font("", &Budget::DEFAULT).is_some_and(|font| font.is_ok()));
    }

    /// A document of one text object, `Hi` at (72, 72) points, with the Lettering effect and its `params`.
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
