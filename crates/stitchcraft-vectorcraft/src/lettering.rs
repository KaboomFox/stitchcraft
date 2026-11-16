//! The embroidery fonts a text object of a VectorCraft document sews in (roadmap step M14.4,
//! `docs/src/design/vectorcraft-integration.md` › Lettering, `REQ-VC-026`).
//!
//! **Why a trait.** A text object with StitchCraft's Lettering effect names its font, and whoever reads the
//! document says where fonts are. The plug-ins can't read files, so their fonts are built into them
//! ([`BuiltIn`], `REQ-FONT-021`). The command line looks in folders. The reader asks for a font only when a
//! text names it, so a document without lettering reads no font, and a font is read once however many texts
//! name it.
//!
//! **Why names.** A document keeps the font's name as text, in the effect's record. A font is found by its
//! folder's name, as Ink/Stitch finds it, or by the `name` of its `font.json`, as a person calls it
//! ([`is_named`]). So the same document sews the same in the plug-in and from the command line, wherever
//! each finds the font.

use std::collections::BTreeMap;
use std::rc::Rc;

use serde_json::Value;
use stitchcraft_core::{Budget, Diagnostic};
use stitchcraft_svg::font::{self, Font};
use stitchcraft_svg::lettering::pictures::{Choice, Found};

/// The name of StitchCraft's Lettering effect (`plugin.dev.stitchcraft.lettering`): a text object that has
/// it sews in the embroidery font its `font` names, set as its other parameters say (`REQ-VC-026`).
pub const LETTERING: &str = "lettering";

/// The parameter of the Lettering effect that names the font.
pub const FONT: &str = "font";

/// The font a Lettering effect sews in when its `font` is left empty, by its folder's name: Ink/Stitch Small
/// Font, the first font the plug-ins hold.
pub const DEFAULT_FONT: &str = "small_font";

/// A font the plug-ins hold, with what a text marker that names no font is given one by (`REQ-AUTO-080`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Held {
    /// Its folder's name.
    pub folder: &'static str,
    /// How bold it is: the width of its capital letters' satin columns in heights of those letters, as
    /// [`stitchcraft_svg::lettering::pictures::weight::of_font`] measures it.
    pub weight: f64,
    /// The least and the greatest height it sews capital letters at, in millimetres: how tall it draws them,
    /// times its `min_scale` and its `max_scale`.
    pub heights: (f64, f64),
}

/// The fonts the plug-ins hold, the default font first: the fonts `built-in-fonts.txt` lists for the build.
/// The numbers are written here because a plug-in chooses a font before it reads one, and reading a font
/// to measure it costs most of what reading it to sew costs. The test `req_font_022_…` measures each font
/// built in and fails when a number here is not the font's.
pub const HELD: [Held; 2] =
    [Held { folder: DEFAULT_FONT, weight: 0.13, heights: (3.79, 16.24) }, Held { folder: "barstitch_bold", weight: 0.19, heights: (12.51, 32.52) }];

/// The fonts of [`HELD`] that `has` says are there, as text in pictures chooses among them.
pub fn choices(has: impl Fn(&str) -> bool) -> Vec<Choice> {
    let held = HELD.iter().filter(|held| has(held.folder));
    held.map(|held| Choice { font: held.folder.to_string(), weight: held.weight, heights: held.heights }).collect()
}

/// Where the embroidery fonts a document's texts name are found.
pub trait Fonts {
    /// The fonts a text marker that names none is set in, the nearest in weight to the letters it
    /// replaces: those of [`HELD`] that are here ([`choices`]). Of none, such a marker takes the default font.
    fn choices(&self) -> Vec<Choice> {
        Vec::new()
    }

    /// The fonts there are, each as a person names it, for a message that says which can be chosen.
    fn names(&self) -> Vec<String>;

    /// The font `name` names, read within `budget`: `None` when there is none of that name, and what its
    /// reader said when it doesn't read.
    fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>>;

    /// The font `name` names as far as setting `text` needs it (`REQ-FONT-023`,
    /// [`stitchcraft_svg::font::read_for`]): a host that reads a font on every run, as a plug-in does, reads
    /// only the glyphs the text can use. The font must set no other text. Unless a host says otherwise, it is
    /// the whole font.
    fn font_for(&mut self, name: &str, text: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        let _ = text;
        self.font(name, budget)
    }
}

/// What `fonts` has of the font `name`, read within `budget`, as text in pictures asks for a font
/// ([`stitchcraft_svg::lettering::pictures::replace`], `REQ-VC-028`): the font, what its reader said, or the
/// names of the fonts there are.
pub fn found(fonts: &mut dyn Fonts, name: &str, budget: &Budget) -> Found {
    found_of(fonts.font(name, budget), fonts)
}

/// As [`found`], the font read as far as setting `text` needs it ([`Fonts::font_for`]).
pub fn found_for(fonts: &mut dyn Fonts, name: &str, text: &str, budget: &Budget) -> Found {
    found_of(fonts.font_for(name, text, budget), fonts)
}

/// What a host that gave `font` has of it.
fn found_of(font: Option<Result<Rc<Font>, Diagnostic>>, fonts: &dyn Fonts) -> Found {
    match font {
        Some(Ok(font)) => Found::Font(font),
        Some(Err(unread)) => Found::Unread(unread),
        None => Found::Missing(fonts.names()),
    }
}

/// A font's folder, held as bytes: its name, and its files by their paths in it, as the font reader takes
/// them ([`font::read`]).
#[derive(Clone, Copy, Debug)]
pub struct Folder {
    /// The folder's name.
    pub name: &'static str,
    /// Its files, each by its path in the folder with `/` between its parts.
    pub files: &'static [(&'static str, &'static [u8])],
}

/// A font a host has read and keeps: the whole font, or the font as far as the texts so far need it
/// (`REQ-FONT-023`).
#[derive(Clone, Debug)]
pub struct Kept {
    /// The characters it was read for: `None` for the whole font.
    text: Option<String>,
    /// The font.
    font: Rc<Font>,
}

impl Kept {
    /// The font `kept` if it serves `text`, or all texts when `text` is `None`: a whole font serves every
    /// text, and a font read for some characters serves a text of those. Otherwise what a font that serves
    /// is read for: `None` for the whole font, or the characters kept so far with the text's.
    pub fn serving(kept: Option<&Kept>, text: Option<&str>) -> Result<Rc<Font>, Option<String>> {
        let Some(kept) = kept else { return Err(text.map(str::to_string)) };
        match (&kept.text, text) {
            (None, _) => Ok(Rc::clone(&kept.font)),
            (Some(read), Some(text)) if text.chars().all(|c| read.contains(c)) => Ok(Rc::clone(&kept.font)),
            (Some(read), Some(text)) => Err(Some(format!("{read}{text}"))),
            (Some(_), None) => Err(None),
        }
    }

    /// The font of the folder `folder`, whose files are `files`, read within `budget` for the characters
    /// `text`, or whole for `None`, to keep.
    pub fn read(folder: &str, files: &[(&str, &[u8])], text: Option<String>, budget: &Budget) -> Result<Kept, Diagnostic> {
        let font = match &text {
            Some(text) => font::read_for(folder, files, text, budget),
            None => font::read(folder, files, budget),
        }?;
        Ok(Kept { text, font: Rc::new(font) })
    }

    /// The font.
    pub fn font(&self) -> Rc<Font> {
        Rc::clone(&self.font)
    }
}

/// The fonts of folders held as bytes for as long as the program runs, as those built into a plug-in are. A
/// font is read when a text first names it, as far as the text needs it, and kept. Of no folders, it has no
/// fonts: every text that names one is then left out, with `SC-W1111`.
#[derive(Debug)]
pub struct BuiltIn {
    folders: &'static [Folder],
    read: BTreeMap<usize, Kept>,
}

impl BuiltIn {
    /// The font `name` names, for `text` or whole.
    fn get(&mut self, name: &str, text: Option<&str>, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        // A font is found by its folder's name without its `font.json` being read, which for a font with
        // many kerning pairs is long, and by the name its `font.json` gives only when no folder is so named.
        let by_folder = self.folders.iter().position(|folder| is_named(name, folder.name, None));
        let index = by_folder.or_else(|| self.folders.iter().position(|folder| is_named(name, folder.name, settings_of(folder).as_deref())))?;
        let read_for = match Kept::serving(self.read.get(&index), text) {
            Ok(font) => return Some(Ok(font)),
            Err(read_for) => read_for,
        };
        let folder = self.folders.get(index)?;
        Some(Kept::read(folder.name, folder.files, read_for, budget).map(|kept| {
            let font = kept.font();
            self.read.insert(index, kept);
            font
        }))
    }
}

impl BuiltIn {
    /// The fonts of `folders`.
    pub fn new(folders: &'static [Folder]) -> Self {
        BuiltIn { folders, read: BTreeMap::new() }
    }
}

impl Fonts for BuiltIn {
    fn choices(&self) -> Vec<Choice> {
        choices(|folder| self.folders.iter().any(|held| held.name == folder))
    }

    fn names(&self) -> Vec<String> {
        self.folders.iter().map(|folder| shown(folder.name, settings_of(folder).as_deref())).collect()
    }

    fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        self.get(name, None, budget)
    }

    fn font_for(&mut self, name: &str, text: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        self.get(name, Some(text), budget)
    }
}

/// The name the `font.json` of `folder` gives its font, if it gives one.
fn settings_of(folder: &Folder) -> Option<String> {
    let (_, bytes) = folder.files.iter().find(|(path, _)| *path == font::SETTINGS)?;
    font_name(bytes)
}

/// The `name` of the font whose `font.json` is `json`, if it has one: read alone, since reading the font
/// reads every glyph.
pub fn font_name(json: &[u8]) -> Option<String> {
    let json = json.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(json);
    let settings: Value = serde_json::from_slice(json).ok()?;
    settings.get("name")?.as_str().map(str::to_string)
}

/// Whether `asked` names the font of the folder `folder`, whose `font.json` gives it `name`: by either, in
/// any case, the spaces at its ends aside. Asked for by an empty name, the font is [`DEFAULT_FONT`].
pub fn is_named(asked: &str, folder: &str, name: Option<&str>) -> bool {
    let asked = match asked.trim() {
        "" => DEFAULT_FONT,
        asked => asked,
    };
    let same = |other: &str| other.trim().to_lowercase() == asked.to_lowercase();
    same(folder) || name.is_some_and(same)
}

/// A font as a message names it: its `name` with its folder's after it, or its folder's alone.
pub fn shown(folder: &str, name: Option<&str>) -> String {
    match name {
        Some(name) if name != folder => format!("{name} (`{folder}`)"),
        _ => format!("`{folder}`"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SETTINGS: &[u8] = b"\xEF\xBB\xBF{\"name\": \"Test Font\"}";

    /// A glyph file that draws `a`, a stroke 4 mm wide.
    const GLYPHS: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100"><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 1,60 L 5,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"#;

    #[test]
    fn req_vc_026_a_font_is_found_by_its_folder_or_its_name() {
        assert!(is_named("small_font", "small_font", None));
        assert!(is_named(" Small_Font ", "small_font", Some("Ink/Stitch Small Font")));
        assert!(is_named("ink/stitch small font", "small_font", Some("Ink/Stitch Small Font")));
        assert!(!is_named("small", "small_font", Some("Ink/Stitch Small Font")));
        assert!(!is_named("Ink/Stitch Small Font", "small_font", None));
        // Left empty, the font asked for is the default.
        assert!(is_named("", DEFAULT_FONT, None));
        assert!(is_named("  ", DEFAULT_FONT, Some("Another")));
        assert!(!is_named("", "another", Some("Another")));
        assert_eq!(font_name(SETTINGS).as_deref(), Some("Test Font"));
        assert_eq!(font_name(b"{\"name\": 3}"), None);
        assert_eq!(font_name(b"{}"), None);
        assert_eq!(font_name(b"no json"), None);
        assert_eq!(shown("test", Some("Test Font")), "Test Font (`test`)");
        assert_eq!(shown("test", Some("test")), "`test`");
        assert_eq!(shown("test", None), "`test`");
    }

    #[test]
    fn req_vc_026_fonts_held_as_bytes_are_read_when_asked_for_and_kept() {
        const FILES: [(&str, &[u8]); 2] = [("font.json", SETTINGS), ("ltr.svg", GLYPHS.as_bytes())];
        const FOLDERS: [Folder; 2] = [Folder { name: "test", files: &FILES }, Folder { name: "bare", files: &[] }];
        let mut fonts = BuiltIn::new(&FOLDERS);
        assert_eq!(fonts.names(), ["Test Font (`test`)", "`bare`"]);
        assert!(fonts.font("other", &Budget::DEFAULT).is_none());
        let first = fonts.font("Test Font", &Budget::DEFAULT).unwrap().unwrap();
        assert_eq!(first.folder, "test");
        // Asked for again, by its folder, it is the font already read.
        let again = fonts.font("TEST", &Budget::DEFAULT).unwrap().unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        // A folder that holds no font says what its reader says.
        let unread = fonts.font("bare", &Budget::DEFAULT).unwrap().unwrap_err();
        assert_eq!(unread.code, stitchcraft_core::Code::FontUnreadable);
        let mut none = BuiltIn::new(&[]);
        assert!(none.names().is_empty() && none.font("test", &Budget::DEFAULT).is_none());
    }

    #[test]
    fn req_font_023_a_font_is_read_as_far_as_the_texts_so_far_need_it() {
        /// As [`GLYPHS`], with a `b` too.
        const BOTH: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100"><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-a"><path id="pa" d="M 1,60 L 5,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g><g inkscape:groupmode="layer" inkscape:label="GlyphLayer-b"><path id="pb" d="M 1,60 L 5,70" style="fill:none;stroke:#000000;stroke-width:0.4"/></g></svg>"#;
        const FILES: [(&str, &[u8]); 2] = [("font.json", SETTINGS), ("ltr.svg", BOTH.as_bytes())];
        // The second folder holds no font: asking for the first by its folder's name never reads it.
        const FOLDERS: [Folder; 2] = [Folder { name: "test", files: &FILES }, Folder { name: "other", files: &[("font.json", b"{")] }];
        let glyphs = |font: &Font| font.variants[0].glyphs.keys().cloned().collect::<Vec<_>>();
        let mut fonts = BuiltIn::new(&FOLDERS);
        let first = fonts.font_for("test", "a", &Budget::DEFAULT).unwrap().unwrap();
        assert_eq!(glyphs(&first), ["a"]);
        // A text of the same characters is served by the font kept, in either order and any number.
        let again = fonts.font_for("TEST", "aaa", &Budget::DEFAULT).unwrap().unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        // A text with another character has the font read again for the characters of both.
        let wider = fonts.font_for("test", "b", &Budget::DEFAULT).unwrap().unwrap();
        assert_eq!(glyphs(&wider), ["a", "b"]);
        assert!(Rc::ptr_eq(&wider, &fonts.font_for("test", "ab", &Budget::DEFAULT).unwrap().unwrap()));
        assert!(Rc::ptr_eq(&wider, &fonts.font_for("Test Font", "a", &Budget::DEFAULT).unwrap().unwrap()));
        // Asked for whole, it is read whole, and then serves every text.
        let whole = fonts.font("test", &Budget::DEFAULT).unwrap().unwrap();
        assert!(!Rc::ptr_eq(&wider, &whole) && glyphs(&whole) == ["a", "b"]);
        assert!(Rc::ptr_eq(&whole, &fonts.font_for("test", "c", &Budget::DEFAULT).unwrap().unwrap()));
        // A font that doesn't read says so each time, and one that isn't there is none.
        assert_eq!(fonts.font_for("other", "a", &Budget::DEFAULT).unwrap().unwrap_err().code, stitchcraft_core::Code::FontUnreadable);
        assert!(fonts.font_for("none", "a", &Budget::DEFAULT).is_none());
        // What a kept font serves.
        let kept = Kept::read("test", &FILES, Some("a".to_string()), &Budget::DEFAULT).unwrap();
        assert_eq!(Kept::serving(None, Some("a")).unwrap_err(), Some("a".to_string()));
        assert_eq!(Kept::serving(None, None).unwrap_err(), None);
        assert!(Rc::ptr_eq(&Kept::serving(Some(&kept), Some("a")).unwrap(), &kept.font()));
        assert_eq!(Kept::serving(Some(&kept), Some("ab")).unwrap_err(), Some("aab".to_string()));
        assert_eq!(Kept::serving(Some(&kept), None).unwrap_err(), None);
        // A host that doesn't say otherwise gives the whole font for any text.
        struct Whole(BuiltIn);
        impl Fonts for Whole {
            fn names(&self) -> Vec<String> {
                self.0.names()
            }
            fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
                self.0.font(name, budget)
            }
        }
        let mut plain = Whole(BuiltIn::new(&FOLDERS));
        assert_eq!(glyphs(&plain.font_for("test", "a", &Budget::DEFAULT).unwrap().unwrap()), ["a", "b"]);
        assert!(plain.choices().is_empty());
        assert!(matches!(found_for(&mut plain, "test", "a", &Budget::DEFAULT), Found::Font(font) if glyphs(&font) == ["a", "b"]));
        assert!(matches!(found_for(&mut plain, "none", "a", &Budget::DEFAULT), Found::Missing(names) if names.len() == 2));
    }

    #[test]
    fn req_auto_081_a_host_offers_the_fonts_it_holds_of_those_the_plug_ins_hold() {
        assert_eq!(HELD[0].folder, DEFAULT_FONT);
        let all = choices(|_| true);
        assert_eq!(all.len(), HELD.len());
        for (choice, held) in all.iter().zip(HELD) {
            assert_eq!((choice.font.as_str(), choice.weight, choice.heights), (held.folder, held.weight, held.heights));
            assert!(held.weight > 0.0 && held.heights.0 > 0.0 && held.heights.0 < held.heights.1);
        }
        assert_eq!(choices(|folder| folder == "barstitch_bold").iter().map(|choice| choice.font.as_str()).collect::<Vec<_>>(), ["barstitch_bold"]);
        assert!(choices(|_| false).is_empty());
        // Fonts held as bytes offer those of them the table names.
        const FOLDERS: [Folder; 3] =
            [Folder { name: "barstitch_bold", files: &[] }, Folder { name: "test", files: &[] }, Folder { name: "small_font", files: &[] }];
        let offered = BuiltIn::new(&FOLDERS).choices();
        assert_eq!(offered.iter().map(|choice| choice.font.as_str()).collect::<Vec<_>>(), ["small_font", "barstitch_bold"]);
    }
}
