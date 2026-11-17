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
use stitchcraft_vectorcraft::lettering::{Fonts, Kept, choices, font_name, is_named, shown};

use crate::commands::lettering::folder;
use crate::files;

/// The variable that names the folder of fonts.
pub const VARIABLE: &str = "STITCHCRAFT_FONTS";

/// The fonts of a folder of font folders, each read when a text first names it.
#[derive(Debug, Default)]
pub struct FolderFonts {
    /// The folder, if one is named.
    folder: Option<PathBuf>,
    /// The fonts read, by their folders' names, each as far as the texts so far need it.
    read: BTreeMap<String, Kept>,
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

    /// The folders there that hold a `font.json`, in the order of their names, without one being read.
    fn folders(&self) -> Vec<String> {
        let Some(entries) = self.folder.as_ref().and_then(|folder| std::fs::read_dir(folder).ok()) else { return Vec::new() };
        let mut folders: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.path().join(font::SETTINGS).is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        folders.sort();
        folders
    }

    /// The folder of the font `name` names: the folder so named, or else the one whose `font.json` gives
    /// that name. The settings are read only when no folder is so named, since a folder of fonts holds
    /// megabytes of them.
    fn named(&self, name: &str) -> Option<String> {
        let by_folder = self.folders().into_iter().find(|folder| is_named(name, folder, None));
        by_folder.or_else(|| self.listed().into_iter().find(|(folder, known)| is_named(name, folder, known.as_deref())).map(|(folder, _)| folder))
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

impl FolderFonts {
    /// The font `name` names, as far as setting `text` needs it, or whole: read as an exporter reads the
    /// font built into it, so that both say the same of a document (`REQ-FONT-023`).
    fn get(&mut self, name: &str, text: Option<&str>, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        let found = self.named(name)?;
        let read_for = match Kept::serving(self.read.get(&found), text) {
            Ok(font) => return Some(Ok(font)),
            Err(read_for) => read_for,
        };
        let path = self.folder.as_ref()?.join(&found);
        let files = match folder(&path) {
            Ok(files) => files,
            Err(error) => return Some(Err(Diagnostic::new(Code::FontUnreadable, format!("The font `{found}` can't be read: {error}.")))),
        };
        let given: Vec<(&str, &[u8])> = files.iter().map(|(path, bytes)| (path.as_str(), bytes.as_slice())).collect();
        Some(Kept::read(&found, &given, read_for, budget).map(|kept| {
            let font = kept.font();
            self.read.insert(found, kept);
            font
        }))
    }
}

impl Fonts for FolderFonts {
    fn choices(&self) -> Vec<Choice> {
        let folders = self.folders();
        choices(|folder| folders.iter().any(|found| found == folder))
    }

    fn names(&self) -> Vec<String> {
        self.listed().iter().map(|(folder, name)| shown(folder, name.as_deref())).collect()
    }

    fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        self.get(name, None, budget)
    }

    fn font_for(&mut self, name: &str, text: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        self.get(name, Some(text), budget)
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
        // A folder's own name comes before the name another folder's `font.json` gives.
        std::fs::create_dir_all(dir.join("plain font")).unwrap();
        std::fs::write(dir.join("plain font/font.json"), "{").unwrap();
        assert_eq!(fonts.font("Plain Font", &Budget::DEFAULT).unwrap().unwrap_err().code, Code::FontUnreadable);
        std::fs::remove_dir_all(dir.join("plain font")).unwrap();
        // Read for a text, a font has the glyphs the text can use, and is kept for texts of those characters.
        let mut fonts = FolderFonts::of(Some(dir.join(".")));
        let some = fonts.font_for("plain", "b", &Budget::DEFAULT).unwrap().unwrap();
        assert!(some.variants[0].glyphs.is_empty());
        assert!(Rc::ptr_eq(&some, &fonts.font_for("plain", "bb", &Budget::DEFAULT).unwrap().unwrap()));
        let more = fonts.font_for("plain", "a", &Budget::DEFAULT).unwrap().unwrap();
        assert_eq!(more.variants[0].glyphs.len(), 1);
        // Of the fonts the plug-ins hold, the folder offers those it has.
        assert!(fonts.choices().is_empty());
        for held in ["barstitch_bold", "small_font"] {
            std::fs::create_dir_all(dir.join(held)).unwrap();
            std::fs::write(dir.join(held).join("font.json"), "{}").unwrap();
        }
        assert_eq!(fonts.choices().iter().map(|choice| choice.font.as_str()).collect::<Vec<_>>(), ["small_font", "barstitch_bold"]);
        assert!(FolderFonts::of(None).choices().is_empty());
        // A font without a glyph file doesn't read, and its reader says so.
        assert_eq!(fonts.font("bare", &Budget::DEFAULT).unwrap().unwrap_err().code, Code::FontUnreadable);
        // Without a folder, and in one that isn't there, there are no fonts.
        for none in [None, Some(dir.join("missing"))] {
            let mut fonts = FolderFonts::of(none);
            assert!(fonts.names().is_empty() && fonts.font("plain", &Budget::DEFAULT).is_none());
        }
    }
}
