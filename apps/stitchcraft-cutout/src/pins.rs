//! The models' 3 files: pinned by address, size and SHA-256 in `models/models.toml`, fetched with curl on
//! first use into a folder of the user's, and checked before each use (`REQ-CLI-016`, ADR-0019 decision 5).
//!
//! **Never committed.** The files are 49 MB together, more than the rest of the repository, so only their
//! pins are: a StitchCraft release publishes them, with their licences, and `models.toml` holds each one's
//! address there. Beside it, `export.py` exports MobileSAM's 2 files from its authors' checkpoint, and says
//! where U²-Net-p's comes from.
//!
//! **The folder** is the one `STITCHCRAFT_MODELS` names, or else `stitchcraft/models` in the user's cache
//! folder: `XDG_CACHE_HOME`, `~/.cache` or, on Windows, `LOCALAPPDATA`, where `cargo xtask corpus` keeps the
//! fonts too. A user offline fetches the files elsewhere and puts them there.
//!
//! **Checked before each use.** A file is read whole, and its size and SHA-256 compared with its pin before
//! tract reads the same bytes, so a file changed in the folder is never run. A file missing, or one that
//! differs, is fetched again with curl into a temporary file beside it, checked, and only then renamed into
//! place. When a file can't be fetched, or what was fetched differs too, `SC-E1201` names each file's
//! address, size and SHA-256 and the folder, and says that the picture plans as before without `--subject`.
//! A `stitch` built without the cut-out, as the WASI one is, fetches nothing: it says so first (`SC-E1207`).
// The models are fetched and read here, in the apps' layer, and nowhere else in this crate: the steps of the
// cut-out read no file, environment or process.
#![allow(clippy::disallowed_methods)]

use std::ffi::OsString;
use std::fmt::Write as _;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::Deserialize;
use sha2::{Digest, Sha256};
use stitchcraft_core::{Code, Diagnostic, Fix};

/// The pins, as the repository holds them.
pub const PINS: &str = include_str!("../models/models.toml");
/// The variable that names the folder the models are kept in.
pub const VARIABLE: &str = "STITCHCRAFT_MODELS";

/// One model file's pin: its name, where it is published, its size and its SHA-256.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Pin {
    /// Its name in the folder.
    pub file: String,
    /// Its address, which curl fetches.
    pub address: String,
    /// Its size, in bytes.
    pub size: u64,
    /// Its SHA-256, in lower-case hexadecimal.
    pub sha256: String,
}

/// The 3 pins: U²-Net-p, MobileSAM's encoder and its decoder.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Pins {
    /// U²-Net-p, which finds the box.
    pub u2netp: Pin,
    /// MobileSAM's encoder.
    pub encoder: Pin,
    /// MobileSAM's decoder.
    pub decoder: Pin,
}

#[derive(Deserialize)]
struct Pinned {
    model: Pins,
}

impl Pins {
    /// The pins in `text`, a `models.toml`; `SC-E0009` when it doesn't hold them, which the repository's
    /// tests rule out for its own.
    pub fn parse(text: &str) -> Result<Pins, Diagnostic> {
        let pinned: Pinned = toml::from_str(text).map_err(|e| unpinned(&e.to_string()))?;
        let pins = pinned.model;
        let hex = |sum: &str| sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase());
        match pins.all().iter().find(|pin| !hex(&pin.sha256) || pin.file.contains(['/', '\\']) || pin.file.is_empty()) {
            Some(pin) => Err(unpinned(&format!("{} is pinned with the SHA-256 `{}`", pin.file, pin.sha256))),
            None => Ok(pins),
        }
    }

    /// The repository's pins.
    pub fn pinned() -> Result<Pins, Diagnostic> {
        Pins::parse(PINS)
    }

    /// The 3, in order.
    pub fn all(&self) -> [&Pin; 3] {
        [&self.u2netp, &self.encoder, &self.decoder]
    }
}

/// `SC-E0009`: the pins can't be read, and why.
fn unpinned(why: &str) -> Diagnostic {
    Diagnostic::new(Code::InternalCheckFailed, format!("The subject cut-out's models are not pinned as it reads them: {why}."))
}

/// The folder the models are kept in, from the environment ([the module](self)); `None` when it names none.
pub fn folder() -> Option<PathBuf> {
    folder_from(&|name| std::env::var_os(name))
}

/// [`folder`], with the environment's variables given by `var`.
pub fn folder_from(var: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    let var = |name: &str| var(name).filter(|value| !value.is_empty()).map(PathBuf::from);
    if let Some(folder) = var(VARIABLE) {
        return Some(folder);
    }
    let cache = var("XDG_CACHE_HOME").or_else(|| var("HOME").map(|home| home.join(".cache"))).or_else(|| var("LOCALAPPDATA"));
    cache.map(|cache| cache.join("stitchcraft").join("models"))
}

/// The 3 files' bytes, each checked against its pin.
#[derive(Clone, Debug)]
pub struct Ready {
    /// U²-Net-p's file.
    pub u2netp: Vec<u8>,
    /// MobileSAM's encoder's file.
    pub encoder: Vec<u8>,
    /// MobileSAM's decoder's file.
    pub decoder: Vec<u8>,
}

/// The files `pins` names in `folder`, each fetched first when it is missing or differs from its pin;
/// `SC-E1201` when one can't be fetched, or differs from its pin once fetched.
pub fn ready(pins: &Pins, folder: Option<&Path>) -> Result<Ready, Diagnostic> {
    let Some(folder) = folder else {
        return Err(not_ready(pins, None, &[format!("there is no folder for them: set {VARIABLE} to one")]));
    };
    let mut why = Vec::new();
    let [u2netp, encoder, decoder] = pins.all().map(|pin| checked(pin, folder).map_err(|reason| why.push(reason)).ok());
    let (Some(u2netp), Some(encoder), Some(decoder)) = (u2netp, encoder, decoder) else {
        // A folder that can't be made is one reason for the 3 files, said once.
        why.dedup();
        return Err(not_ready(pins, Some(folder), &why));
    };
    Ok(Ready { u2netp, encoder, decoder })
}

/// The bytes of the file `pin` names in `folder`, which match the pin, fetched first when they don't; why
/// not, when they still don't.
fn checked(pin: &Pin, folder: &Path) -> Result<Vec<u8>, String> {
    let path = folder.join(&pin.file);
    if let Ok(bytes) = read(&path, pin.size)
        && differs(pin, &bytes).is_none()
    {
        return Ok(bytes);
    }
    std::fs::create_dir_all(folder).map_err(|e| format!("the folder could not be made ({e})"))?;
    let partial = temporary(&path);
    let fetched = fetch(&pin.address, &partial).and_then(|()| {
        let bytes = read(&partial, pin.size).map_err(|e| format!("{} could not be read once fetched ({e})", pin.file))?;
        match differs(pin, &bytes) {
            Some(difference) => Err(format!("{} was fetched, and {difference}", pin.file)),
            None => Ok(bytes),
        }
    });
    let placed = fetched.and_then(|bytes| {
        std::fs::rename(&partial, &path).map_err(|e| format!("{} could not be put in place ({e})", pin.file))?;
        Ok(bytes)
    });
    if placed.is_err() {
        // What a failed fetch leaves is of no use: the next run fetches again.
        let _ = std::fs::remove_file(&partial);
    }
    placed
}

/// How many temporary files this process has named: the count that tells apart 2 fetches of one file at once,
/// on 2 threads.
static FETCHES: AtomicUsize = AtomicUsize::new(0);

/// The temporary file beside `path` that curl fetches into: this process's own and this fetch's, so that 2
/// runs at once don't write into one file. WASI gives a program no process id.
fn temporary(path: &Path) -> PathBuf {
    #[cfg(target_os = "wasi")]
    let process = 0;
    #[cfg(not(target_os = "wasi"))]
    let process = std::process::id();
    let mut name = path.as_os_str().to_owned();
    name.push(format!(".{process}-{}.partial", FETCHES.fetch_add(1, Ordering::Relaxed)));
    PathBuf::from(name)
}

/// Fetches `address` with curl into `into`: why not, with the end of what curl said, when it fails.
fn fetch(address: &str, into: &Path) -> Result<(), String> {
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", "--retry", "3", "--output"])
        .arg(into)
        .arg(address)
        .output()
        .map_err(|e| format!("curl could not be started to fetch {address} ({e})"))?;
    if output.status.success() {
        return Ok(());
    }
    let said = String::from_utf8_lossy(&output.stderr);
    let said: String = said.split_whitespace().collect::<Vec<_>>().join(" ").chars().filter(|c| !c.is_control()).take(300).collect();
    Err(format!("curl could not fetch {address} ({})", if said.is_empty() { output.status.to_string() } else { said }))
}

/// The bytes of `path`, read whole, when it holds at most `size` of them and 1 more, so that a file far
/// larger than its pin isn't read whole.
fn read(path: &Path, size: u64) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    std::fs::File::open(path)?.take(size.saturating_add(1)).read_to_end(&mut bytes)?;
    Ok(bytes)
}

/// How `bytes` differ from `pin`, in size or SHA-256; `None` when they don't.
fn differs(pin: &Pin, bytes: &[u8]) -> Option<String> {
    let size = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if size > pin.size {
        return Some(format!("it is larger than its {} bytes", pin.size));
    }
    if size < pin.size {
        return Some(format!("it is {size} bytes, not {}", pin.size));
    }
    let sum = sha256(bytes);
    (sum != pin.sha256).then(|| format!("its SHA-256 is {sum}, not {}", pin.sha256))
}

/// The SHA-256 of `bytes`, in lower-case hexadecimal.
pub fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().fold(String::with_capacity(64), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

/// `n` with its thousands separated, as people read a size: 4,574,861.
fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (at, digit) in digits.chars().enumerate() {
        if at > 0 && (digits.len() - at).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// `SC-E1201`: the models `pins` names aren't ready in `folder`, for each reason in `why`.
fn not_ready(pins: &Pins, folder: Option<&Path>, why: &[String]) -> Diagnostic {
    let mut message = String::from("The subject cut-out's models are not ready");
    if let Some(folder) = folder {
        let _ = write!(message, " in {}", folder.display());
    }
    let _ = write!(message, ": {}. Put these files in ", why.join("; "));
    let _ = write!(message, "{}, ", folder.map_or_else(|| "a folder".to_string(), |folder| folder.display().to_string()));
    let _ = write!(message, "or in the folder {VARIABLE} names: ");
    let files: Vec<String> =
        pins.all().iter().map(|pin| format!("{}, {} bytes, SHA-256 {}, from {}", pin.file, grouped(pin.size), pin.sha256, pin.address)).collect();
    let _ = write!(message, "{}.", files.join("; "));
    Diagnostic::new(Code::CutoutModelsNotReady, message)
        .with_fix(Fix::Hint("Fetch the files elsewhere and put them in the folder, or plan the picture without --subject, as before.".to_string()))
}

#[cfg(test)]
mod tests;
