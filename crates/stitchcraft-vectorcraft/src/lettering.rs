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
use stitchcraft_svg::lettering::pictures::Found;

/// The name of StitchCraft's Lettering effect (`plugin.dev.stitchcraft.lettering`): a text object that has
/// it sews in the embroidery font its `font` names, set as its other parameters say (`REQ-VC-026`).
pub const LETTERING: &str = "lettering";

/// The parameter of the Lettering effect that names the font.
pub const FONT: &str = "font";

/// The font a Lettering effect sews in when its `font` is left empty, by its folder's name: Ink/Stitch Small
/// Font, the first font the plug-ins hold.
pub const DEFAULT_FONT: &str = "small_font";

/// Where the embroidery fonts a document's texts name are found.
pub trait Fonts {
    /// The fonts there are, each as a person names it, for a message that says which can be chosen.
    fn names(&self) -> Vec<String>;

    /// The font `name` names, read within `budget`: `None` when there is none of that name, and what its
    /// reader said when it doesn't read.
    fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>>;
}

/// What `fonts` has of the font `name`, read within `budget`, as text in pictures asks for a font
/// ([`stitchcraft_svg::lettering::pictures::replace`], `REQ-VC-028`): the font, what its reader said, or the
/// names of the fonts there are.
pub fn found(fonts: &mut dyn Fonts, name: &str, budget: &Budget) -> Found {
    match fonts.font(name, budget) {
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

/// The fonts of folders held as bytes for as long as the program runs, as those built into a plug-in are. A
/// font is read when a text first names it, and kept. Of no folders, it has no fonts: every text that names
/// one is then left out, with `SC-W1111`.
#[derive(Debug)]
pub struct BuiltIn {
    folders: &'static [Folder],
    read: BTreeMap<usize, Rc<Font>>,
}

impl BuiltIn {
    /// The fonts of `folders`.
    pub fn new(folders: &'static [Folder]) -> Self {
        BuiltIn { folders, read: BTreeMap::new() }
    }
}

impl Fonts for BuiltIn {
    fn names(&self) -> Vec<String> {
        self.folders.iter().map(|folder| shown(folder.name, settings_of(folder).as_deref())).collect()
    }

    fn font(&mut self, name: &str, budget: &Budget) -> Option<Result<Rc<Font>, Diagnostic>> {
        let index = self.folders.iter().position(|folder| is_named(name, folder.name, settings_of(folder).as_deref()))?;
        if let Some(font) = self.read.get(&index) {
            return Some(Ok(Rc::clone(font)));
        }
        let folder = self.folders.get(index)?;
        Some(font::read(folder.name, folder.files, budget).map(|font| {
            let font = Rc::new(font);
            self.read.insert(index, Rc::clone(&font));
            font
        }))
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
}
