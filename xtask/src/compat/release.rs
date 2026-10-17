//! A VectorCraft release's Linux x86_64 archive: where it is, fetching it, and checking its SHA-256.
//!
//! **Why one place.** The window probe runs the pinned stable release's app, and Level B the
//! `vectorcraft-cli` of the release a track names, which the same archive holds (VectorCraft's
//! `packaging/linux/package.sh` puts both in `bin/`). The pinned release is checked against the sum the
//! pins file records. A release the gate hasn't adopted yet is checked against the sum its own
//! `SHA256SUMS.txt` lists, and the bot PR that adopts it pins that sum. The archive is fetched with curl into
//! `target/compat/<name>/`, and fetched again only when the copy there doesn't match.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{PINS, load_pins, pinned_tag};
use crate::util;

/// The pins file's key for the SHA-256 of the stable release's Linux x86_64 archive.
pub const SHA_KEY: &str = "linux-x86_64-sha256";

/// A release's Linux x86_64 archive.
#[derive(Debug, PartialEq, Eq)]
pub struct Archive {
    /// Its file name without `.tar.gz`, which is also the folder it holds.
    pub name: String,
    /// Where it is.
    pub url: String,
    /// The release's list of SHA-256 sums.
    pub sums: String,
}

/// The archive of the release `tag`, a `v` and its version, of `repository`.
pub fn archive(repository: &str, tag: &str) -> Result<Archive, String> {
    let version = tag.strip_prefix('v').filter(|v| !v.is_empty()).ok_or(format!("the tag `{tag}` is no `v<version>`"))?;
    let name = format!("vectorcraft-{version}-linux-x86_64");
    let download = format!("{repository}/releases/download/{tag}");
    Ok(Archive { url: format!("{download}/{name}.tar.gz"), sums: format!("{download}/SHA256SUMS.txt"), name })
}

/// A release fetched and unpacked.
#[derive(Debug)]
pub struct Fetched {
    /// The folder the archive holds, with `bin/vectorcraft` and `bin/vectorcraft-cli`.
    pub root: PathBuf,
    /// The archive's SHA-256, which a bot PR pins when it adopts the release.
    pub sha256: String,
}

/// The release `tag`, fetched and unpacked, after its archive's SHA-256 is checked: against the pins
/// file's for the pinned stable release, and against the release's own list for any other.
pub fn fetch(tag: &str) -> Result<Fetched, String> {
    let pins = load_pins()?;
    let repository = pins.get("repository").and_then(toml::Value::as_str).ok_or(format!("{PINS}: no `repository`"))?;
    let archive = archive(repository, tag)?;
    let file_name = format!("{}.tar.gz", archive.name);
    let expected = match pinned_sum(&pins, tag) {
        Some(sum) => sum.to_string(),
        None => listed_sum(&download(&archive.sums)?, &file_name).ok_or(format!("{} lists no SHA-256 of {file_name}", archive.sums))?,
    };
    let dir = util::root().join("target/compat").join(&archive.name);
    std::fs::create_dir_all(&dir).map_err(|e| format!("{}: {e}", util::rel(&dir)))?;
    let file = dir.join(&file_name);
    if !file.is_file() || sha256(&file)? != expected {
        let mut curl = Command::new("curl");
        curl.args(["--fail", "--silent", "--show-error", "--location", "--retry", "3", "--output"]).arg(&file).arg(&archive.url);
        util::run(curl, &format!("curl {}", archive.url))?;
    }
    let got = sha256(&file)?;
    if got != expected {
        return Err(format!("{}: its SHA-256 is {got}, and {expected} was expected", archive.url));
    }
    let mut tar = Command::new("tar");
    tar.arg("-xzf").arg(&file).arg("-C").arg(&dir);
    util::run(tar, &format!("tar -xzf {}", util::rel(&file)))?;
    Ok(Fetched { root: dir.join(&archive.name), sha256: got })
}

/// The SHA-256 the pins file records for `tag`'s archive: only the pinned stable release has one.
fn pinned_sum<'a>(pins: &'a toml::Table, tag: &str) -> Option<&'a str> {
    let stable = pins.get("tracks").and_then(|t| t.get("stable"))?;
    (pinned_tag(pins, "stable") == tag).then(|| stable.get(SHA_KEY).and_then(toml::Value::as_str)).flatten()
}

/// The SHA-256 a `SHA256SUMS.txt` lists for `file`, as `sha256sum` writes the list: the sum, 2 spaces (or a
/// space and a `*`) and the file name, one file a line.
pub fn listed_sum(list: &str, file: &str) -> Option<String> {
    list.lines().find_map(|line| {
        let (sum, name) = line.trim_end().split_once(' ')?;
        let name = name.trim_start().trim_start_matches('*');
        (name == file && sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit())).then(|| sum.to_ascii_lowercase())
    })
}

/// The text at `url`, by curl.
fn download(url: &str) -> Result<String, String> {
    let output = Command::new("curl")
        .args(["--fail", "--silent", "--show-error", "--location", "--retry", "3", url])
        .output()
        .map_err(|e| format!("curl: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(format!("curl {url}: {}", String::from_utf8_lossy(&output.stderr).trim()))
    }
}

/// The SHA-256 of `file`, in hexadecimal, by `sha256sum`.
pub fn sha256(file: &Path) -> Result<String, String> {
    let output = Command::new("sha256sum").arg(file).output().map_err(|e| format!("sha256sum: {e}"))?;
    let text = String::from_utf8_lossy(&output.stdout);
    let sum = text.split_whitespace().next().filter(|_| output.status.success());
    sum.map(str::to_string).ok_or(format!("sha256sum {}: {}", util::rel(file), String::from_utf8_lossy(&output.stderr).trim()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_release_is_fetched_from_its_archive_and_checked_by_its_sum() {
        let pins = load_pins().unwrap();
        let tag = pinned_tag(&pins, "stable");
        let repository = pins["repository"].as_str().unwrap();
        let found = archive(repository, tag).unwrap();
        let name = format!("vectorcraft-{}-linux-x86_64", &tag[1..]);
        let download = format!("https://github.com/storytold/vectorcraft/releases/download/{tag}");
        assert_eq!(found, Archive { url: format!("{download}/{name}.tar.gz"), sums: format!("{download}/SHA256SUMS.txt"), name });
        assert!(archive(repository, "latest").unwrap_err().contains("no `v<version>`"));
        // The pinned stable release has its sum in the pins file, and every other release the one it lists.
        let sum = pinned_sum(&pins, tag).unwrap();
        assert!(sum.len() == 64 && sum.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{sum}");
        assert_eq!(pinned_sum(&pins, "v0.0.1"), None);
    }

    #[test]
    fn a_sum_is_read_from_the_release_s_list() {
        let a = "6750db109d3a253c51c82975af650f42384ba27a768f72529431edfb58508269";
        let list = format!(
            "{}  vectorcraft-0.12.0-linux-aarch64.tar.gz\n{a}  vectorcraft-0.12.0-linux-x86_64.tar.gz\n{} *vectorcraft-0.12.0.dmg\n",
            "f".repeat(64),
            "E".repeat(64)
        );
        assert_eq!(listed_sum(&list, "vectorcraft-0.12.0-linux-x86_64.tar.gz").as_deref(), Some(a));
        assert_eq!(listed_sum(&list, "vectorcraft-0.12.0.dmg"), Some("e".repeat(64)));
        // Not one of its files, or not a sum.
        assert_eq!(listed_sum(&list, "vectorcraft-0.12.0-linux-x86_64.tar"), None);
        assert_eq!(listed_sum("abc  file.tar.gz\n", "file.tar.gz"), None);
    }
}
