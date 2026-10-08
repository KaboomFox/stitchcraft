//! `cargo xtask api [--check]`: the public API of every library crate as a committed snapshot,
//! `crates/<crate>/public-api.txt`, so a pull request that changes what other code can call shows it in
//! its diff (`docs/src/design/guardrails.md`). Without `--check` the snapshots are rewritten.
//!
//! The listing is cargo-public-api's, read from rustdoc's JSON output. That output is still unstable in
//! rustdoc, so the pinned toolchain produces it with `RUSTC_BOOTSTRAP=1`, for documentation only, never
//! for code that ships: the snapshots then depend on `rust-toolchain.toml` and on cargo-public-api
//! [`TOOL_VERSION`] (which CI installs), not on a moving nightly. A toolchain bump may reword some lines;
//! that diff belongs in the bump's pull request.

use std::path::Path;

use crate::layers::{Class, TABLE};
use crate::util::{self, Findings};

/// One literal for the version, so the install command below cannot drift from it.
macro_rules! tool_version {
    () => {
        "0.52.0"
    };
}
/// The cargo-public-api version whose output the snapshots are; CI installs exactly this one.
pub const TOOL_VERSION: &str = tool_version!();
/// How to install it.
pub const INSTALL: &str = concat!("cargo install cargo-public-api --version ", tool_version!(), " --locked");
/// The snapshot inside each crate.
const SNAPSHOT: &str = "public-api.txt";

/// Whether cargo-public-api is installed.
pub fn available() -> bool {
    util::tool_available("cargo-public-api", &["--version"])
}

/// The library crates: every layered crate in the layering table.
fn libraries() -> impl Iterator<Item = &'static str> {
    TABLE.iter().filter(|(_, class)| matches!(class, Class::Layer(_))).map(|(name, _)| *name)
}

/// `cargo xtask api`.
pub fn run(check_only: bool) -> Result<(), String> {
    let version =
        std::process::Command::new("cargo-public-api").arg("--version").output().map_err(|e| format!("cargo-public-api: {e} ({INSTALL})"))?;
    let version = String::from_utf8_lossy(&version.stdout);
    if !version.split_whitespace().any(|word| word == TOOL_VERSION) {
        return Err(format!("the snapshots are cargo-public-api {TOOL_VERSION} output, but `{}` is installed: {INSTALL}", version.trim()));
    }
    let root = util::root();
    let mut findings = Findings::default();
    let mut count = 0;
    for crate_name in libraries() {
        count += 1;
        let path = root.join("crates").join(crate_name).join(SNAPSHOT);
        let file = util::rel(&path);
        let listing = match listing(&root, crate_name) {
            Ok(listing) => listing,
            Err(e) => {
                findings.error(format!("{crate_name}: {e}"));
                continue;
            }
        };
        let current = std::fs::read_to_string(&path).unwrap_or_default();
        if current == listing {
            continue;
        }
        if check_only {
            findings.error(format!("{file} is not the crate's public API any more: run `cargo xtask api` and review the diff"));
        } else if let Err(e) = std::fs::write(&path, &listing) {
            findings.error(format!("{file}: {e}"));
        } else {
            println!("updated {file}");
        }
    }
    let verb = if check_only { "match" } else { "written" };
    findings.finish("api", &format!("{count} public API snapshots {verb}"))
}

/// The public API of `crate_name`, one item per line, with the snapshot's header.
fn listing(root: &Path, crate_name: &str) -> Result<String, String> {
    let mut rustdoc = util::cargo();
    rustdoc
        // A target directory of its own, so the bootstrap flag never invalidates the normal build.
        .args(["rustdoc", "--quiet", "--locked", "--target-dir", "target/api", "-p", crate_name, "--lib", "--"])
        .args(["-Z", "unstable-options", "--output-format", "json"])
        .env("RUSTC_BOOTSTRAP", "1");
    util::run(rustdoc, &format!("cargo rustdoc -p {crate_name} (JSON)"))?;
    let json = root.join("target/api/doc").join(format!("{}.json", crate_name.replace('-', "_")));
    let output = util::cargo()
        .current_dir(root)
        .arg("public-api")
        .arg("--rustdoc-json")
        .arg(&json)
        .arg("-sss")
        .output()
        .map_err(|e| format!("cargo public-api: {e}"))?;
    if !output.status.success() {
        return Err(format!("cargo public-api failed: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    let items = String::from_utf8(output.stdout).map_err(|e| format!("cargo public-api: {e}"))?;
    Ok(format!(
        "# The public API of {crate_name}, as `cargo xtask api` lists it (cargo-public-api {TOOL_VERSION}, -sss).\n\
         # Generated: do not edit. A change here is a change other code can see; say why in the pull request.\n{items}"
    ))
}
