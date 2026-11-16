//! Builds embroidery fonts into the plug-ins (roadmap step M14.4, `REQ-FONT-021`, ADR-0020): a plug-in
//! can't read files, so the exporters hold the fonts their documents' texts sew in.
//!
//! With the build option `STITCHCRAFT_FONTS_DIR` naming a folder of fonts, such as a checkout of the font
//! repository's `fonts` folder, each font that `built-in-fonts.txt` lists is built in from its folder
//! there: its `font.json`, its glyph files and its licence file, and nothing else of it. Without the
//! option no font is built in, and everything builds and tests. A font the list names that the folder
//! lacks stops the build, since a plug-in that silently lacked a font would leave its texts out.
//!
//! The script writes `fonts.rs` into Cargo's output folder, which `src/fonts.rs` includes: a table of the
//! fonts, each file included from where it lies. No font is copied into the repository.

// A build script speaks to Cargo on standard output, reads the fonts' folder and the build option that
// names it, and stops the build when a font is missing.
#![allow(clippy::print_stdout, clippy::disallowed_methods)]

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

/// The build option that names the folder of fonts.
const OPTION: &str = "STITCHCRAFT_FONTS_DIR";

/// Whether the file `name` of a font is built in: its settings, a glyph file, or its licence.
fn wanted(name: &str) -> bool {
    let lower = name.to_lowercase();
    let stem = lower.strip_suffix(".txt").or_else(|| lower.strip_suffix(".md")).unwrap_or(&lower);
    lower == "font.json" || lower.ends_with(".svg") || stem == "license" || stem == "licence"
}

/// The files of the font folder `folder` that are built in, each by its path in the folder with `/` between
/// its parts: those directly in it and directly in the folders in it, in the order of their paths.
fn files(folder: &Path) -> std::io::Result<Vec<(String, PathBuf)>> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(folder)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if entry.file_type()?.is_dir() {
            for inner in std::fs::read_dir(entry.path())? {
                let inner = inner?;
                let inner_name = inner.file_name().to_string_lossy().into_owned();
                if inner.file_type()?.is_file() && wanted(&inner_name) {
                    files.push((format!("{name}/{inner_name}"), inner.path()));
                }
            }
        } else if wanted(&name) {
            files.push((name, entry.path()));
        }
    }
    files.sort();
    Ok(files)
}

/// The table of the fonts `listed` that `fonts` holds, as Rust, or which font it lacks or can't read.
fn table(fonts: &Path, listed: &[&str]) -> Result<String, String> {
    let mut table = String::new();
    for name in listed {
        let folder = fonts.join(name);
        let files =
            files(&folder).map_err(|error| format!("the font `{name}` of built-in-fonts.txt can't be read in {}: {error}", fonts.display()))?;
        println!("cargo:rerun-if-changed={}", folder.display());
        let _ = write!(table, "    Folder {{ name: {name:?}, files: &[");
        for (path, file) in files {
            let file = file.canonicalize().map_err(|error| format!("{}: {error}", file.display()))?;
            let _ = write!(table, "({path:?}, include_bytes!({:?})), ", file.display().to_string());
        }
        table.push_str("] },\n");
    }
    Ok(table)
}

fn main() {
    println!("cargo:rerun-if-env-changed={OPTION}");
    println!("cargo:rerun-if-changed=built-in-fonts.txt");
    println!("cargo:rerun-if-changed=build.rs");
    let list = std::fs::read_to_string("built-in-fonts.txt").unwrap_or_default();
    let listed: Vec<&str> = list.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    let fonts = std::env::var_os(OPTION).filter(|folder| !folder.is_empty()).map(PathBuf::from);
    let table = match fonts.map(|fonts| table(&fonts, &listed)) {
        None => String::new(),
        Some(Ok(table)) => table,
        Some(Err(why)) => {
            println!("cargo:warning={OPTION}: {why}");
            std::process::exit(1);
        }
    };
    let out = PathBuf::from(std::env::var_os("OUT_DIR").unwrap_or_default()).join("fonts.rs");
    let code = format!("/// The fonts built in, in the order of `built-in-fonts.txt`.\npub static FONTS: &[Folder<'static>] = &[\n{table}];\n");
    if let Err(error) = std::fs::write(&out, code) {
        println!("cargo:warning={}: {error}", out.display());
        std::process::exit(1);
    }
}
