//! `cargo xtask wasm`: the library crates and the plug-in build for `wasm32-unknown-unknown`, so the engine
//! can run inside VectorCraft's sandbox. That target has no threads, file system or clocks, and std's calls
//! for them build there but fail or panic when run: `clippy.toml` keeps library crates from calling them,
//! and this gate checks that they build.
//!
//! `cargo xtask wasm --goldens` checks that they compute there as they do natively: `stitch`, built for
//! WASI and run by wasmtime, makes every golden file of the conformance cases byte for byte
//! (`conformance::made_as_wasm`). `cargo xtask ci` runs it as the `wasm-goldens` gate.

use crate::layers::{Class, TABLE};
use crate::util;

const TARGET: &str = "wasm32-unknown-unknown";
/// The target `stitch` is built for to make the golden files as WebAssembly (`--goldens`): WASI gives it a
/// file system, and its arithmetic is the plug-in's.
const WASI: &str = "wasm32-wasip1";

/// The packages that must build for the web: every layered library, plus the plug-in.
pub fn packages() -> Vec<&'static str> {
    let mut out: Vec<&str> = TABLE.iter().filter(|(_, c)| matches!(c, Class::Layer(_))).map(|(n, _)| *n).collect();
    out.push("stitchcraft-vc-plugin");
    out
}

/// Whether `target` is installed for the active toolchain.
fn installed(target: &str) -> bool {
    std::process::Command::new("rustup")
        .args(["target", "list", "--installed"])
        .current_dir(util::root())
        .output()
        .is_ok_and(|o| String::from_utf8_lossy(&o.stdout).lines().any(|l| l.trim() == target))
}

/// Whether the wasm32 target is installed for the active toolchain.
pub fn target_installed() -> bool {
    installed(TARGET)
}

/// Whether the golden files can be made as WebAssembly here: the WASI target and wasmtime are installed.
pub fn wasi_ready() -> bool {
    installed(WASI) && util::tool_available("wasmtime", &["--version"])
}

/// What to install for `--goldens`.
pub const WASI_INSTALL: &str = "rustup target add wasm32-wasip1, and wasmtime";

/// `cargo xtask wasm --goldens`: `stitch` built for WASI makes every golden file of the conformance cases
/// under wasmtime, byte for byte (`conformance::made_as_wasm`).
pub fn goldens() -> Result<(), String> {
    if !wasi_ready() {
        return Err(format!("the golden files are made as WebAssembly with the {WASI} target and wasmtime: {WASI_INSTALL}"));
    }
    let mut cmd = util::cargo();
    cmd.args(["build", "--release", "--locked", "--target", WASI, "-p", "stitchcraft-cli"]);
    util::run(cmd, &format!("cargo build --target {WASI} -p stitchcraft-cli"))?;
    let alike = crate::conformance::made_as_wasm(&util::root().join("target").join(WASI).join("release/stitch.wasm"))?;
    println!("wasm: {alike} golden files made as WebAssembly ({WASI}, wasmtime), byte for byte");
    Ok(())
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
