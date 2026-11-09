//! `cargo xtask corpus`: Ink/Stitch's fonts, the corpus of the glyph differential (M8.6,
//! `docs/src/design/fonts.md`, and `docs/src/design/conformance.md` › L3).
//!
//! **Pinned, checked, never committed.** [`PIN`] names the fonts' repository, a commit and the SHA-256 of the
//! archive GitHub makes of that commit. The archive is fetched with curl into the cache, its sum checked, and
//! only then unpacked there, with tar. A copy whose sum doesn't match is fetched again, and a sum that still
//! doesn't match is an error, with nothing unpacked. The files are data under their own licences, and none is
//! ever committed: the repository holds only the pin and this code.
//!
//! **A cache outside the repository** ([`cache`]): `STITCHCRAFT_CORPUS`, or `stitchcraft/corpus` in the
//! user's cache folder. Outside the repository no gate reads the fonts as the project's own files: the
//! clean-room check would find the GPL's text in one font's licence, and the docs checks the fonts' own
//! Markdown files. `cargo clean` keeps it, and every worktree shares one copy. The archive is unpacked into a
//! folder of its own and moved into place whole ([`unpacked`]), so a folder in place was unpacked completely.
//!
//! **The fonts the corpus takes** ([`fonts`]): each font's licence file names its licence, as the font
//! reader reads it ([`Licence::of`], `stitchcraft_svg::font::licence`), and the corpus takes the fonts under
//! the OFL, CC0 or the public domain, CC BY or CC BY-SA. At `c7e3a05` that is 131 of the 142 fonts. The
//! others are listed, and their glyphs never sewn. Each font's glyph files are every SVG file in its folder,
//! `ltr.svg` and the rest, or a folder of them for a direction a large font splits, and the font reader's
//! cutter makes each glyph a design ([`stitchcraft_svg::font::each_glyph`]).

use std::path::{Path, PathBuf};
use std::process::Command;

use stitchcraft_core::Budget;
pub use stitchcraft_svg::font::licence::{self, Licence};

use crate::util;

/// The fonts' repository, commit and archive SHA-256, relative to the repository root.
pub const PIN: &str = "conformance/inkstitch/fonts.toml";
/// The variable that names the cache, when the user's cache folder isn't where it should be.
pub const CACHE: &str = "STITCHCRAFT_CORPUS";

/// The pinned corpus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pin {
    /// The repository's address on GitHub.
    pub repository: String,
    /// The commit, in full.
    pub commit: String,
    /// The SHA-256 of the archive GitHub makes of the commit, in lower-case hexadecimal.
    pub sha256: String,
}

impl Pin {
    /// The pin in `text`, [`PIN`]'s contents.
    pub fn parse(text: &str) -> Result<Pin, String> {
        let table: toml::Table = toml::from_str(text).map_err(|e| format!("{PIN}: {e}"))?;
        let field = |key: &str| table.get(key).and_then(toml::Value::as_str).map(str::to_string).ok_or(format!("{PIN}: no `{key}`"));
        let pin = Pin { repository: field("repository")?, commit: field("commit")?, sha256: field("sha256")? };
        let hex = |text: &str, len: usize| text.len() == len && text.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        if !hex(&pin.commit, 40) {
            return Err(format!("{PIN}: the commit `{}` is not 40 lower-case hexadecimal digits", pin.commit));
        }
        if !hex(&pin.sha256, 64) {
            return Err(format!("{PIN}: the sha256 `{}` is not 64 lower-case hexadecimal digits", pin.sha256));
        }
        let path = pin.repository.strip_prefix("https://github.com/").map(|path| path.split('/').collect::<Vec<_>>());
        if !path.is_some_and(|path| path.len() == 2 && path.iter().all(|part| !part.is_empty())) {
            return Err(format!("{PIN}: the repository `{}` is not one on GitHub, https://github.com/OWNER/NAME", pin.repository));
        }
        Ok(pin)
    }

    /// The pin the repository holds.
    pub fn load(root: &Path) -> Result<Pin, String> {
        Pin::parse(&util::read(&root.join(PIN))?)
    }

    /// The repository's name, the last part of its address.
    fn name(&self) -> &str {
        self.repository.rsplit('/').next().unwrap_or_default()
    }

    /// The archive's address, which GitHub redirects to the archive of the commit.
    pub fn url(&self) -> String {
        format!("{}/archive/{}.tar.gz", self.repository, self.commit)
    }

    /// The folder the archive holds, and its name in the cache: the repository's name and the commit.
    pub fn folder(&self) -> String {
        format!("{}-{}", self.name(), self.commit)
    }

    /// The commit, shortened as the docs give it.
    pub fn short(&self) -> &str {
        self.commit.get(..7).unwrap_or(&self.commit)
    }
}

/// The cache: [`CACHE`] when it is set, or else `stitchcraft/corpus` in the user's cache folder,
/// `XDG_CACHE_HOME`, `~/.cache` or, on Windows, `LOCALAPPDATA`.
pub fn cache() -> Result<PathBuf, String> {
    let var = |name: &str| std::env::var_os(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    cache_from(&var)
}

/// [`cache`], with the environment's variables given by `var`.
fn cache_from(var: &dyn Fn(&str) -> Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(dir) = var(CACHE) {
        return Ok(dir);
    }
    let base = var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|home| home.join(".cache"))).or_else(|| var("LOCALAPPDATA"));
    base.map(|base| base.join("stitchcraft").join("corpus")).ok_or(format!("no cache folder for the corpus: set {CACHE}"))
}

/// The corpus unpacked in the cache, when it is there: `None` until `cargo xtask corpus` has fetched it.
pub fn unpacked(root: &Path) -> Result<Option<PathBuf>, String> {
    let pin = Pin::load(root)?;
    let dir = cache()?.join(pin.folder());
    Ok(dir.is_dir().then_some(dir))
}

/// The corpus, fetched and checked against `pin` when the cache doesn't hold it yet, and unpacked there.
pub fn fetch(pin: &Pin, cache: &Path) -> Result<PathBuf, String> {
    let dir = cache.join(pin.folder());
    if dir.is_dir() {
        return Ok(dir);
    }
    std::fs::create_dir_all(cache).map_err(|e| format!("{}: {e}", cache.display()))?;
    let archive = cache.join(format!("{}.tar.gz", pin.folder()));
    if !archive.is_file() || sha256(&archive)? != pin.sha256 {
        let mut curl = Command::new("curl");
        curl.args(["--fail", "--silent", "--show-error", "--location", "--retry", "3", "--output"]).arg(&archive).arg(pin.url());
        util::run(curl, &format!("curl {}", pin.url()))?;
    }
    let got = sha256(&archive)?;
    if got != pin.sha256 {
        return Err(format!("{}: its SHA-256 is {got}, and {PIN} gives {}", pin.url(), pin.sha256));
    }
    // Unpacked beside the folder and moved into place whole, so that an unpacking cut short leaves nothing in
    // place.
    let partial = cache.join("unpacking");
    if partial.exists() {
        std::fs::remove_dir_all(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
    }
    std::fs::create_dir_all(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
    let mut tar = Command::new("tar");
    tar.arg("-xzf").arg(&archive).arg("-C").arg(&partial);
    util::run(tar, &format!("tar -xzf {}", archive.display()))?;
    let inside = partial.join(pin.folder());
    if !inside.is_dir() {
        return Err(format!("{} holds no folder {}", archive.display(), pin.folder()));
    }
    std::fs::rename(&inside, &dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    std::fs::remove_dir_all(&partial).map_err(|e| format!("{}: {e}", partial.display()))?;
    Ok(dir)
}

/// The SHA-256 of `file`, by `sha256sum`.
fn sha256(file: &Path) -> Result<String, String> {
    crate::compat::release::sha256(file)
}

/// A font of the corpus.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Font {
    /// Its folder's name, which names it in the reports.
    pub name: String,
    /// Its licence, as its licence file names it.
    pub licence: Licence,
    /// Its glyph files, from its folder, by path from there with `/` separators, sorted.
    pub files: Vec<String>,
    /// Its folder.
    pub dir: PathBuf,
}

/// The fonts of the corpus unpacked at `corpus`, each folder of its `src` folder, sorted by name.
pub fn fonts(corpus: &Path) -> Result<Vec<Font>, String> {
    let src = corpus.join("src");
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(&src)
        .map_err(|e| format!("{}: {e}", src.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_dir())
        .collect();
    dirs.sort();
    dirs.into_iter()
        .map(|dir| {
            let name = dir.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            let files = util::files(&dir, &["svg"])
                .iter()
                .filter_map(|file| Some(file.strip_prefix(&dir).ok()?.to_string_lossy().replace('\\', "/")))
                .collect();
            Ok(Font { name, licence: licence_of(&dir)?, files, dir })
        })
        .collect()
}

/// The licence of the font in `dir`, as its licence file names it: [`Licence::Other`] without one.
fn licence_of(dir: &Path) -> Result<Licence, String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    let mut files: Vec<PathBuf> = entries.filter_map(|entry| entry.ok().map(|entry| entry.path())).filter(|path| is_licence(path)).collect();
    files.sort();
    match files.first() {
        Some(file) => Ok(Licence::of(&String::from_utf8_lossy(&util::read_bytes(file)?))),
        None => Ok(Licence::Other),
    }
}

/// Whether `path` is a licence file, by the font reader's rule ([`licence::is_licence_file`]).
fn is_licence(path: &Path) -> bool {
    let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    path.is_file() && licence::is_licence_file(&name)
}

/// The labels of the glyphs of the glyph file `bytes`, in the order of the file.
pub fn labels(bytes: &[u8]) -> Result<Vec<String>, String> {
    let mut labels = Vec::new();
    stitchcraft_svg::font::each_glyph(bytes, &Budget::DEFAULT, &|_| false, &mut |_, cut| labels.push(cut.label)).map_err(|d| d.message)?;
    Ok(labels)
}

/// `cargo xtask corpus`: fetches the corpus where it isn't yet, and says what it holds.
pub fn run() -> Result<(), String> {
    let root = util::root();
    let pin = Pin::load(&root)?;
    let corpus = fetch(&pin, &cache()?)?;
    let fonts = fonts(&corpus)?;
    let (taken, left): (Vec<&Font>, Vec<&Font>) = fonts.iter().partition(|font| font.licence.taken());
    let files: Vec<PathBuf> = taken.iter().flat_map(|font| font.files.iter().map(|file| font.dir.join(file))).collect();
    let glyphs = files.iter().map(|file| Ok(labels(&util::read_bytes(file)?)?.len())).sum::<Result<usize, String>>()?;
    println!("corpus: {} at {} in {}", pin.repository, pin.short(), corpus.display());
    println!(
        "corpus: {} of {} fonts, with {} glyph files and {glyphs} glyphs, are under the OFL, CC0, CC BY or CC BY-SA",
        taken.len(),
        fonts.len(),
        files.len()
    );
    for licence in Licence::TAKEN {
        println!("  {:>4} {}", taken.iter().filter(|font| font.licence == licence).count(), licence.name());
    }
    println!("corpus: left out, by their licence files:");
    for font in left {
        println!("  {} ({})", font.name, font.licence.name());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const PINNED: &str = "repository = \"https://github.com/inkstitch/embroidery-fonts\"\ncommit = \"c7e3a05c3d6f5c77b881c50f257851af9a8e9cb8\"\nsha256 = \"9e8442555664d7e638de9587d1475f1983d87bc856adf28bad273adcf2eb1db0\"\n";

    #[test]
    fn the_pin_names_the_archive_of_a_commit_and_its_sum() {
        let pin = Pin::parse(PINNED).unwrap();
        assert_eq!(pin.url(), "https://github.com/inkstitch/embroidery-fonts/archive/c7e3a05c3d6f5c77b881c50f257851af9a8e9cb8.tar.gz");
        assert_eq!(pin.folder(), "embroidery-fonts-c7e3a05c3d6f5c77b881c50f257851af9a8e9cb8");
        assert_eq!(pin.short(), "c7e3a05");
        // The repository's own pin reads, and is the one the design counted.
        assert_eq!(Pin::load(&util::root()).unwrap().short(), "c7e3a05");
    }

    #[test]
    fn a_pin_must_give_a_whole_commit_and_sum_of_a_repository_on_github() {
        let with = |from: &str, to: &str| Pin::parse(&PINNED.replace(from, to));
        assert_eq!(
            with("c7e3a05c3d6f5c77b881c50f257851af9a8e9cb8", "c7e3a05").unwrap_err(),
            format!("{PIN}: the commit `c7e3a05` is not 40 lower-case hexadecimal digits")
        );
        assert!(with("9e84", "9E84").unwrap_err().starts_with(&format!("{PIN}: the sha256 `9E84")));
        assert!(with("9e84", "").unwrap_err().contains("is not 64 lower-case hexadecimal digits"));
        let not_on_github = |repository: &str| format!("{PIN}: the repository `{repository}` is not one on GitHub, https://github.com/OWNER/NAME");
        assert_eq!(with("https://github.com/", "https://example.com/").unwrap_err(), not_on_github("https://example.com/inkstitch/embroidery-fonts"));
        assert_eq!(with("inkstitch/embroidery-fonts", "").unwrap_err(), not_on_github("https://github.com/"));
        assert_eq!(with("embroidery-fonts", "embroidery-fonts/").unwrap_err(), not_on_github("https://github.com/inkstitch/embroidery-fonts/"));
        assert_eq!(with("inkstitch/", "").unwrap_err(), not_on_github("https://github.com/embroidery-fonts"));
        assert_eq!(Pin::parse("commit = \"x\"").unwrap_err(), format!("{PIN}: no `repository`"));
        assert!(Pin::parse("commit = ").unwrap_err().starts_with(PIN));
    }

    #[test]
    fn the_cache_is_the_variable_s_folder_or_the_user_s_cache() {
        let env =
            |pairs: &'static [(&'static str, &'static str)]| move |name: &str| pairs.iter().find(|(n, _)| *n == name).map(|(_, v)| PathBuf::from(v));
        assert_eq!(cache_from(&env(&[(CACHE, "/c"), ("HOME", "/h")])).unwrap(), PathBuf::from("/c"));
        assert_eq!(cache_from(&env(&[("XDG_CACHE_HOME", "/x"), ("HOME", "/h")])).unwrap(), PathBuf::from("/x/stitchcraft/corpus"));
        assert_eq!(cache_from(&env(&[("HOME", "/h"), ("LOCALAPPDATA", "/l")])).unwrap(), PathBuf::from("/h/.cache/stitchcraft/corpus"));
        assert_eq!(cache_from(&env(&[("LOCALAPPDATA", "/l")])).unwrap(), PathBuf::from("/l/stitchcraft/corpus"));
        assert_eq!(cache_from(&env(&[])).unwrap_err(), format!("no cache folder for the corpus: set {CACHE}"));
    }

    #[test]
    fn an_archive_whose_sum_differs_is_never_unpacked() {
        let cache = util::root().join("target/xtask-tests/corpus-sum");
        let _ = std::fs::remove_dir_all(&cache);
        std::fs::create_dir_all(&cache).unwrap();
        // A copy in the cache that isn't the pinned archive, and an address curl can't fetch, so that the
        // copy is what is checked.
        let pin = Pin { repository: "https://github.com/nobody/nothing".into(), commit: "0".repeat(40), sha256: "0".repeat(64) };
        std::fs::write(cache.join(format!("{}.tar.gz", pin.folder())), b"not the archive").unwrap();
        let fetched = fetch(&Pin { repository: "file:///nowhere".into(), ..pin.clone() }, &cache);
        assert!(fetched.unwrap_err().starts_with("`curl file:///nowhere/archive/"), "curl's failure is said");
        assert!(!cache.join(pin.folder()).exists() && !cache.join("unpacking").exists());
    }

    #[test]
    fn a_corpus_in_place_is_taken_as_it_is_and_its_fonts_listed_with_their_licences() {
        let cache = util::root().join("target/xtask-tests/corpus-fonts");
        let _ = std::fs::remove_dir_all(&cache);
        let pin = Pin::parse(PINNED).unwrap();
        let src = cache.join(pin.folder()).join("src");
        for (font, licence, files) in [
            ("b_font", Some(("LICENSE", "licensed under the SIL Open Font License")), &["ltr.svg", "rtl.svg"][..]),
            ("a_font", Some(("license.txt", "licensed CC-BY-NC-SA 4.0")), &["ltr/latin.svg", "ltr/greek.svg"]),
            ("c_font", None, &["ltr.svg"]),
        ] {
            for file in files {
                let path = src.join(font).join(file);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, "<svg/>").unwrap();
            }
            std::fs::write(src.join(font).join("preview.png"), "").unwrap();
            if let Some((name, text)) = licence {
                std::fs::write(src.join(font).join(name), text).unwrap();
            }
        }
        // In place, it isn't fetched: the pin's address isn't even tried.
        let corpus = fetch(&pin, &cache).unwrap();
        let listed: Vec<(String, Licence, Vec<String>)> = fonts(&corpus).unwrap().into_iter().map(|f| (f.name, f.licence, f.files)).collect();
        let strings = |files: &[&str]| files.iter().map(|f| f.to_string()).collect::<Vec<_>>();
        assert_eq!(
            listed,
            [
                ("a_font".to_string(), Licence::NonCommercial, strings(&["ltr/greek.svg", "ltr/latin.svg"])),
                ("b_font".to_string(), Licence::Ofl, strings(&["ltr.svg", "rtl.svg"])),
                ("c_font".to_string(), Licence::Other, strings(&["ltr.svg"])),
            ]
        );
        // A folder without the corpus says so with what the system says of the missing folder, in its own
        // words on each system.
        let missing = cache.join("src");
        let said = std::fs::read_dir(&missing).unwrap_err();
        assert_eq!(fonts(&cache).unwrap_err(), format!("{}: {said}", missing.display()));
    }

    #[test]
    fn licence_files_are_found_by_their_names_in_any_case() {
        let dir = util::root().join("target/xtask-tests/corpus-licence-names");
        std::fs::create_dir_all(dir.join("LICENSE.d")).unwrap();
        for (name, is) in
            [("LICENSE", true), ("licence", true), ("License.txt", true), ("LICENCE.md", true), ("LICENSE.pdf", false), ("license-x", false)]
        {
            std::fs::write(dir.join(name), "").unwrap();
            assert_eq!(is_licence(&dir.join(name)), is, "{name}");
        }
        assert!(!is_licence(&dir.join("LICENSE.d")), "a folder is no licence file");
    }
}
