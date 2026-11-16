//! The embroidery fonts `stitch` finds for the texts of a VectorCraft document (roadmap step M14.4,
//! `REQ-FONT-021`, ADR-0020).
//!
//! **Where.** In the folder `STITCHCRAFT_FONTS` names: each folder in it that has a `font.json` is a font, as
//! a checkout of the font repository's `fonts` folder lays them out, and as Ink/Stitch's own `fonts` folder
//! does. A font is found by its folder's name or by the `name` of its `font.json`
//! ([`stitchcraft_vectorcraft::lettering::is_named`]), which is how a Lettering effect's record names it,
//! so a document sews from the command line as it does from VectorCraft's exporters.
//!
//! **Why lazily.** A folder of fonts holds hundreds of megabytes. Only the `font.json` of each is read to
//! find a font, and a font's glyph files only when a text names it.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::rc::Rc;

use stitchcraft_core::{Budget, Code, Diagnostic};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::pictures::Choice;
use stitchcraft_vectorcraft::lettering::{Fonts, choices, font_name, is_named, shown};

use crate::commands::lettering::folder;
use crate::files;

/// The variable that names the folder of fonts.
pub const VARIABLE: &str = "STITCHCRAFT_FONTS";

/// The fonts of a folder of font folders, each read when a text first names it.
#[derive(Debug, Default)]
pub struct FolderFonts {
    /// The folder, if one is named.
    folder: Option<PathBuf>,
    /// The fonts read, by their folders' names.
    read: BTreeMap<String, Rc<Font>>,
}

impl FolderFonts {
    /// The fonts of the folder [`VARIABLE`] names, or none when it names none.
    pub fn from_env() -> Self {
        FolderFonts::of(std::env::var_os(VARIABLE).filter(|folder| !folder.is_empty()).map(PathBuf::from))
    }

    /// The fonts of `folder`.
    pub fn of(folder: Option<PathBuf>) -> Self {
        FolderFonts { folder, read: BTreeMap::new() }
    }

    /// Each font there: its folder's name and the `name` of its `font.json`, in the order of the folders'
    /// names. A folder without a `font.json`, or one that can't be listed, is no font.
    fn listed(&self) -> Vec<(String, Option<String>)> {
        let Some(entries) = self.folder.as_ref().and_then(|folder| std::fs::read_dir(folder).ok()) else { return Vec::new() };
        let mut fonts: Vec<(String, Option<String>)> = entries
            .flatten()
            .filter_map(|entry| {
                let settings = files::read_capped(&entry.path().join(font::SETTINGS)).ok()?;
                Some((entry.file_name().to_string_lossy().into_owned(), font_name(&settings)))
            })
            .collect();
        fonts.sort();
        fonts
    }
}

impl Fonts for FolderFonts {
    fn choices(&self) -> Vec<Choice> {
        let listed = self.listed();
        choices(|folder| listed.iter().any(|(found, _)| found == folder))
    }

    fn names(&self) -> Vec<String> {
        self.listed().iter().map(|(folder, name)| shown(folder, name.as_deref())).collect()
    }

    fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        let (found, _) = self.listed().into_iter().find(|(folder, known)| is_named(name, folder, known.as_deref()))?;
        if let Some(font) = self.read.get(&found) {
            return Some(Ok(Rc::clone(font)));
        }
        let path = self.folder.as_ref()?.join(&found);
        let files = match folder(&path) {
            Ok(files) => files,
            Err(error) => return Some(Err(Diagnostic::new(Code::FontUnreadable, format!("The font `{found}` can't be read: {error}.")))),
        };
        let given: Vec<(&str, &[u8])> = files.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
        Some(font::read(&found, &given, budget).map(|font| {
            let font = Rc::new(font);
            self.read.insert(found, Rc::clone(&font));
            font
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A glyph file that draws `a`.
    const GLYPHS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100"><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 1,60 L 5,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"#;

    #[test]
    fn req_font_021_stitch_finds_a_font_in_the_folder_of_fonts_by_its_folder_or_its_name() {
        let dir = stitchcraft_testkit::temp::TempDir::new("cli-fonts");
        for (font, settings) in [("plain", r#"{"name": "Plain Font"}"#), ("bare", "{}")] {
            std::fs::create_dir_all(dir.join(font)).unwrap();
            std::fs::write(dir.join(font).join("font.json"), settings).unwrap();
        }
        std::fs::write(dir.join("plain/ltr.svg"), GLYPHS).unwrap();
        std::fs::create_dir_all(dir.join("no-font")).unwrap();
        std::fs::write(dir.join("README.md"), "fonts").unwrap();
        let mut fonts = FolderFonts::of(Some(dir.join(".")));
        assert_eq!(fonts.names(), ["`bare`", "Plain Font (`plain`)"]);
        assert!(fonts.font("no-font", &Budget::DEFAULT).is_none());
        let first = fonts.font("plain font", &Budget::DEFAULT).unwrap().unwrap();
        assert_eq!((first.folder.as_str(), first.settings.name.as_str()), ("plain", "Plain Font"));
        let again = fonts.font("PLAIN", &Budget::DEFAULT).unwrap().unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        // A font without a glyph file doesn't read, and its reader says so.
        assert_eq!(fonts.font("bare", &Budget::DEFAULT).unwrap().unwrap_err().code, Code::FontUnreadable);
        // Without a folder, and in one that isn't there, there are no fonts.
        for none in [None, Some(dir.join("missing"))] {
            let mut fonts = FolderFonts::of(none);
            assert!(fonts.names().is_empty() && fonts.font("plain", &Budget::DEFAULT).is_none());
        }
    }
}
