//! A VectorCraft release's Linux x86_64 archive: where it is, fetching it, and checking its SHA-256.
//!
//! **Why one place.** The window probe runs the pinned stable release's app, and Level B the
//! `vectorcraft-cli` of the release a track names, which the same archive holds (VectorCraft's
//! `packaging/linux/package.sh` puts both in `bin/`). The pinned release is checked against the sum the
//! pins file records. A release the gate hasn't adopted yet is checked against the sum its own
//! `SHA256SUMS.txt` lists, and the bot PR that adopts it pins that sum. The archive is fetched with curl into
//! `target/compat/<name>/`, and fetched again only when the copy there doesn't match.
//!
//! **Why a download is checked before it takes the archive's place.** CI keeps the pinned release's archive
//! in the Actions cache (`.github/actions/vectorcraft-archive`, which names the same path), and a job saves
//! the file at that path once a run has fetched it. So curl writes a download beside the archive's path, and
//! the download is moved there only once its sum matches. The cache then never keeps an archive whose sum
//! differs, which every later run would fetch again. A copy the cache gives is checked like any other.
//!
//! **Why curl tries again.** On 8 November 2026 GitHub answered the release's downloads with 504s and reset
//! connections for an hour. curl tries a download again after any error, every [`RETRY_DELAY`] seconds for
//! [`RETRY_FOR`] seconds, and gives up a try that takes [`STALL`] seconds to connect, or then receives
//! nothing for as long. So a short outage passes, and a file that isn't there still fails within 2 minutes.

use std::path::{Path, PathBuf};
use std::process::Command;

use super::{load_pins, pinned_tag, repository};
use crate::util;

/// The pins file's key for the SHA-256 of the stable release's Linux x86_64 archive.
pub const SHA_KEY: &str = "linux-x86_64-sha256";

/// How long curl goes on trying a download again, in seconds from its first try.
const RETRY_FOR: u32 = 60;
/// How long curl waits before each new try, in seconds.
const RETRY_DELAY: u32 = 10;
/// How long a try may take to connect, and then go without receiving anything, in seconds.
const STALL: u32 = 20;
// The last try starts within RETRY_FOR and RETRY_DELAY of the first, and gives up after STALL seconds without
// a connection and STALL more without data.
const _: () = assert!(RETRY_FOR + RETRY_DELAY + 2 * STALL <= 120, "a download that can't be made fails within 2 minutes");

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
    let archive = archive(repository(&pins)?, tag)?;
    let file_name = format!("{}.tar.gz", archive.name);
    let expected = match pinned_sum(&pins, tag) {
        Some(sum) => sum.to_string(),
        None => listed_sum(&download(&archive.sums)?, &file_name).ok_or(format!("{} lists no SHA-256 of {file_name}", archive.sums))?,
    };
    let file = util::root().join(kept(&archive.name));
    let Some(dir) = file.parent() else { return Err(format!("{}: no folder", util::rel(&file))) };
    std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", util::rel(dir)))?;
    fetch_archive(&archive.url, &expected, &file)?;
    let mut tar = Command::new("tar");
    tar.arg("-xzf").arg(&file).arg("-C").arg(dir);
    util::run(tar, &format!("tar -xzf {}", util::rel(&file)))?;
    Ok(Fetched { root: dir.join(&archive.name), sha256: expected })
}

/// Where the archive named `name` is kept, from the repository's root: in the folder it unpacks into. CI's
/// cache restores the pinned release's archive to the same path (`.github/actions/vectorcraft-archive`), as a
/// test checks.
fn kept(name: &str) -> String {
    format!("target/compat/{name}/{name}.tar.gz")
}

/// The archive at `url` in `file`, whose SHA-256 must be `expected`. A copy already there is kept when its
/// sum is that one. Otherwise curl fetches the archive to a file beside `file`, which takes `file`'s place
/// only once its sum matches. So `file` never holds an archive whose sum differs.
fn fetch_archive(url: &str, expected: &str, file: &Path) -> Result<(), String> {
    if file.is_file() && sha256(file)? == expected {
        return Ok(());
    }
    let mut part = file.as_os_str().to_owned();
    part.push(".part");
    let part = PathBuf::from(part);
    // Without the progress meter, and with curl's word on each new try, which the log keeps.
    let mut curl = curl();
    curl.arg("--no-progress-meter").arg("--output").arg(&part).arg(url);
    util::run(curl, &format!("curl {url}"))?;
    let got = sha256(&part)?;
    if got != expected {
        return Err(format!("{url}: its SHA-256 is {got}, and {expected} was expected"));
    }
    std::fs::rename(&part, file).map_err(|e| format!("{}: {e}", util::rel(file)))
}

/// curl, with the options of every download from GitHub: it follows redirects and fails on an HTTP error.
/// It tries again after any error, a 504 and a reset connection among them, every [`RETRY_DELAY`] seconds
/// for [`RETRY_FOR`] seconds, and gives up a try that takes [`STALL`] seconds to connect, or then receives
/// nothing for as long. CI's download of wasmtime (`.github/workflows/ci.yml`) takes the same options, as a
/// test checks.
fn curl() -> Command {
    let mut curl = Command::new("curl");
    curl.args(["--fail", "--location", "--retry-all-errors"]);
    let options = [
        ("--retry", RETRY_FOR / RETRY_DELAY),
        ("--retry-delay", RETRY_DELAY),
        ("--retry-max-time", RETRY_FOR),
        ("--connect-timeout", STALL),
        ("--speed-time", STALL),
    ];
    for (option, value) in options {
        curl.arg(option).arg(value.to_string());
    }
    curl
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

/// The text at `url`, by curl, which says only its last error.
fn download(url: &str) -> Result<String, String> {
    let output = curl().args(["--silent", "--show-error", url]).output().map_err(|e| format!("curl: {e}"))?;
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

    #[test]
    fn ci_s_cache_restores_the_pinned_archive_where_it_is_kept() {
        // The action's Python names the archive and its path from the pinned tag's version and writes the key
        // from the pinned sum. Its f-strings have the formulas of `archive` and `kept`, placeholders and all.
        let action = util::read(&util::root().join(".github/actions/vectorcraft-archive/action.yml")).unwrap();
        let name = archive("https://github.com/storytold/vectorcraft", "v{version}").unwrap().name;
        assert_eq!(name, "vectorcraft-{version}-linux-x86_64");
        for formula in [
            "[\"tracks\"][\"stable\"]".to_string(),
            "version = stable[\"tag\"].removeprefix(\"v\")".to_string(),
            format!("name = f\"{name}\""),
            format!("print(f\"path={}\")", kept("{name}")),
            format!("print(f\"key={{name}}-{{stable['{SHA_KEY}']}}\")"),
        ] {
            assert!(action.contains(&formula), "the action's Python must have `{formula}`");
        }
    }

    /// The arguments of `command`, as text.
    fn args(command: &Command) -> Vec<String> {
        command.get_args().map(|arg| arg.to_string_lossy().into_owned()).collect()
    }

    #[test]
    fn curl_tries_again_after_any_error_every_10_seconds_for_a_minute() {
        let curl = curl();
        assert_eq!(curl.get_program(), "curl");
        assert_eq!(
            args(&curl),
            [
                "--fail",
                "--location",
                "--retry-all-errors",
                "--retry",
                "6",
                "--retry-delay",
                "10",
                "--retry-max-time",
                "60",
                "--connect-timeout",
                "20",
                "--speed-time",
                "20"
            ]
        );
        // CI fetches wasmtime from GitHub's releases too, and tries again as often.
        let workflow = util::read(&util::root().join(".github/workflows/ci.yml")).unwrap();
        let options = args(&curl).join(" ");
        assert!(workflow.contains(&format!("curl {options} ")), "ci.yml's curl for wasmtime takes `{options}`");
    }

    #[cfg(unix)]
    #[test]
    fn a_download_takes_the_archive_s_place_only_once_its_sum_matches() {
        let dir = util::root().join("target/xtask-tests/release-download");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("source.tar.gz");
        std::fs::write(&source, b"an archive").unwrap();
        let sum = sha256(&source).unwrap();
        let url = format!("file://{}", source.display());
        let file = dir.join("vectorcraft.tar.gz");
        // A download whose sum differs is refused, and the archive's path, which CI's cache keeps, stays empty.
        let other = "0".repeat(64);
        assert_eq!(fetch_archive(&url, &other, &file).unwrap_err(), format!("{url}: its SHA-256 is {sum}, and {other} was expected"));
        assert!(!file.exists());
        // One whose sum matches takes its place.
        fetch_archive(&url, &sum, &file).unwrap();
        assert_eq!(std::fs::read(&file).unwrap(), b"an archive");
        // A copy in place whose sum matches is kept: nothing is fetched from an address curl couldn't read.
        fetch_archive("file:///nowhere", &sum, &file).unwrap();
        // The text of a release's list of sums comes with the same options.
        let list = dir.join("SHA256SUMS.txt");
        std::fs::write(&list, format!("{sum}  vectorcraft.tar.gz\n")).unwrap();
        assert_eq!(listed_sum(&download(&format!("file://{}", list.display())).unwrap(), "vectorcraft.tar.gz"), Some(sum));
    }
}
