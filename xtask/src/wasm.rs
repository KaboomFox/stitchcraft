//! `cargo xtask wasm`: the library crates and the plug-in build for `wasm32-unknown-unknown`
//! (no threads, no file system, no clocks), so the engine can run inside VectorCraft's sandbox.

use crate::layers::{Class, TABLE};
use crate::util;

const TARGET: &str = "wasm32-unknown-unknown";

/// The packages that must build for the web: every layered library, plus the plug-in.
pub fn packages() -> Vec<&'static str> {
    let mut out: Vec<&str> = TABLE.iter().filter(|(_, c)| matches!(c, Class::Layer(_))).map(|(n, _)| *n).collect();
    out.push("stitchcraft-vc-plugin");
    out
}

/// Whether the wasm32 target is installed for the active toolchain.
pub fn target_installed() -> bool {
    std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .current_dir(util::root())
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.trim() == TARGET))
}

/// `cargo xtask wasm`.
pub fn run() -> Result<(), String> {
    if !target_installed() {
        return Err(format!("the {TARGET} target is not installed: rustup target add {TARGET}"));
    }
    let mut cmd = util::cargo();
    cmd.args(["check", "--locked", "--target", TARGET]);
    for p in packages() {
        cmd.args(["-p", p]);
    }
    util::run(cmd, &format!("cargo check --target {TARGET}"))?;
    println!("wasm: {} packages build for {TARGET}", packages().len());
    Ok(())
}
