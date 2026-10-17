//! Level A of the compatibility gate: our plug-ins inside VectorCraft's real plug-in host.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::time::Instant;

use serde_json::{Value, json};
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Rect, shapes};
use vectorcraft_plugins::{ABI_VERSION, Kind, Limits, Plugin, effect, registry};

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

/// The least instruction budget within which VectorCraft's host lets `plugin` finish `vc_run` on `input`.
fn fuel(plugin: &Plugin, input: &[u8], params: &[u8]) -> u64 {
    let within = |fuel_base| plugin.run_limited(input, params, &Limits { fuel_base, fuel_per_byte: 0, ..Limits::default() }).is_ok();
    let (mut lo, mut hi) = (0, 1 << 36);
    assert!(within(hi), "the run fails: {:?}", plugin.run(input, params).err());
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if within(mid) { hi = mid } else { lo = mid }
    }
    hi
}

/// A path of `n` anchors joined by straight lines, as a stitch preview returns, at coordinates with as
/// many digits as computed ones have.
fn polyline(n: u32) -> PathData {
    let mut path = BezPath::new();
    for k in 0..n {
        let t = f64::from(k);
        let at = Point::new(10.0 + 0.37 * t + f64::from(k % 7) / 3.0, 20.0 + 7.5 * (0.29 * t).fract());
        if k == 0 { path.move_to(at) } else { path.line_to(at) }
    }
    PathData::from_bezpath(&path)
}

/// The live-effect input VectorCraft hands a plug-in for one path, with `extra` fields beside it.
fn effect_input(path: &PathData, extra: Value) -> Vec<u8> {
    let mut object = json!({"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 100, 100]});
    if let (Some(object), Value::Object(extra)) = (object.as_object_mut(), extra) {
        object.extend(extra);
    }
    serde_json::to_vec(&json!({"objects": [object]})).unwrap()
}

/// The instructions VectorCraft's host counted for each byte of JSON the hello plug-in read, and for each
/// byte it wrote, when spike M0.6 measured them (docs/src/design/vectorcraft-integration.md, live-effect
/// previews). M6.3 sizes its preview levels from them.
const READ_PER_BYTE: f64 = 512.0;
const WRITTEN_PER_BYTE: f64 = 822.0;
/// How far above them a later host, or a later build of the plug-in, may go before the gate says so.
const HEADROOM: f64 = 1.25;
/// The 2 sizes of path measured, in anchors. The costs are the differences between them, so that what every
/// run costs anyway cancels out.
const ANCHORS: [u32; 2] = [1_000, 4_000];

/// What a live effect spends on its JSON: the instructions VectorCraft's host counts for each byte the hello
/// plug-in reads and writes, how many bytes a path's anchor takes, and how many instructions the host runs a
/// second here. Printed on every run; the costs stay within a quarter of what M0.6 recorded.
#[test]
fn hello_reads_and_writes_json_within_the_recorded_budget() {
    let plugin = Plugin::load(&plugin_bytes(), Limits::default()).unwrap();
    let params = serde_json::to_vec(&json!({"_context": {"mode": "effect", "bounds": [0, 0, 100, 100]}})).unwrap();
    let paths = ANCHORS.map(polyline);
    // Echoed: read and written. Carried beside a path of 2 anchors: read, and dropped.
    let echoed = paths.each_ref().map(|path| effect_input(path, json!({})));
    let carried = paths.each_ref().map(|path| effect_input(&polyline(2), json!({"pad": path})));
    let written = echoed.each_ref().map(|input| plugin.run(input, &params).unwrap().len());
    let [echo_small, echo_large] = echoed.each_ref().map(|input| fuel(&plugin, input, &params));
    let [carry_small, carry_large] = carried.each_ref().map(|input| fuel(&plugin, input, &params));
    let per_byte_read = (carry_large - carry_small) as f64 / (carried[1].len() - carried[0].len()) as f64;
    let read = per_byte_read * (echoed[1].len() - echoed[0].len()) as f64;
    let per_byte_written = ((echo_large - echo_small) as f64 - read) / (written[1] - written[0]) as f64;
    let per_anchor = (written[1] - written[0]) as f64 / f64::from(ANCHORS[1] - ANCHORS[0]);
    let start = Instant::now();
    plugin.run(&echoed[1], &params).unwrap();
    let per_second = echo_large as f64 / start.elapsed().as_secs_f64();
    println!(
        "::notice title=plug-in budget::reading JSON {per_byte_read:.0} instructions a byte, writing it {per_byte_written:.0} a byte; \
         {per_anchor:.1} bytes an anchor; a run of {} bytes in and {} out {echo_large} instructions; {:.0} million instructions a second",
        echoed[1].len(),
        written[1],
        per_second / 1e6,
    );
    assert!(per_byte_read > 0.0 && per_byte_written > 0.0, "{per_byte_read} {per_byte_written}");
    assert!(
        per_byte_read <= READ_PER_BYTE * HEADROOM && per_byte_written <= WRITTEN_PER_BYTE * HEADROOM,
        "JSON costs {per_byte_read:.0} instructions a byte to read and {per_byte_written:.0} to write, more than a quarter above \
         the {READ_PER_BYTE} and {WRITTEN_PER_BYTE} recorded: measure the budget again (vectorcraft-integration.md, M6.3's levels)"
    );
}
