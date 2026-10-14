//! Names the commit `stitch` is built from, when it is built from a git checkout, as `STITCHCRAFT_COMMIT`
//! (empty otherwise): `stitch --version` shows it, and a bug-report bundle records it, since every build of
//! the workspace says the version it will be released as (`REQ-CLI-001`).

// A build script speaks to Cargo on standard output.
#![allow(clippy::print_stdout)]

use std::process::Command;

/// What `git` prints for `args`, trimmed, when it runs and succeeds.
fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok().filter(|output| output.status.success())?;
    String::from_utf8(output.stdout).ok().map(|text| text.trim().to_string())
}

fn main() {
    println!("cargo:rustc-env=STITCHCRAFT_COMMIT={}", git(&["rev-parse", "--short=12", "HEAD"]).unwrap_or_default());
    // Built again when the checkout moves to another commit: HEAD moves, or the branch it names does.
    for path in [git(&["rev-parse", "--git-path", "HEAD"]), git(&["symbolic-ref", "-q", "HEAD"]).and_then(|r| git(&["rev-parse", "--git-path", &r]))]
        .into_iter()
        .flatten()
    {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-changed=build.rs");
}
