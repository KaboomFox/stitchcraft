//! Level A of the compatibility gate: our plug-ins inside VectorCraft's real plug-in host.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use serde_json::json;
use vectorcraft_geom::{FillRule, Rect, shapes};
use vectorcraft_plugins::{ABI_VERSION, Kind, effect, registry};

/// The plug-in under test, built by CI for wasm32-unknown-unknown.
fn plugin_bytes() -> Vec<u8> {
    let path = std::env::var("STITCHCRAFT_PLUGIN_WASM").expect("set STITCHCRAFT_PLUGIN_WASM to the built .wasm");
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {path}: {e}"))
}

#[test]
fn host_still_speaks_abi_v1() {
    assert_eq!(ABI_VERSION, 1, "VectorCraft's plug-in ABI changed: review apps/stitchcraft-vc-plugin/src/abi.rs");
}

#[test]
fn hello_installs_with_an_accepted_manifest() {
    let plugin = registry::install_bytes(&plugin_bytes()).expect("VectorCraft accepts the module and its manifest");
    assert_eq!(plugin.id(), "dev.stitchcraft.hello");
    assert!(plugin.manifest().kind == Kind::Effect, "hello is a live effect");
}

#[test]
fn hello_returns_the_geometry_unchanged() {
    let plugin = registry::install_bytes(&plugin_bytes()).unwrap();
    let bounds = Rect::new(10.0, 20.0, 110.0, 70.0);
    let path = shapes::rectangle(bounds);
    let out = effect::apply(plugin.id(), &json!({}), &path, bounds, FillRule::NonZero);
    assert_eq!(out.as_ref(), Some(&path), "last error: {:?}", effect::last_error(plugin.id()));
}
