//! Level A of the compatibility gate: our plug-ins inside VectorCraft's real plug-in host.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Rect, shapes};
use vectorcraft_plugins::{ABI_VERSION, Kind, Limits, Plugin, effect, registry};

/// The stitch families, each a module of its own whose live effect sews an object as the family.
const FAMILIES: [&str; 3] = ["stroke", "satin", "fill"];

/// The module `name`, *hello* or a stitch family's, which `cargo xtask compat contract` built for
/// wasm32-unknown-unknown into the folder `STITCHCRAFT_PLUGINS` names.
fn module(name: &str) -> Vec<u8> {
    let dir = std::env::var("STITCHCRAFT_PLUGINS").expect("set STITCHCRAFT_PLUGINS to the folder of the built modules");
    let path = Path::new(&dir).join(format!("{name}.wasm"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// The *hello* module.
fn plugin_bytes() -> Vec<u8> {
    module("hello")
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

#[test]
fn each_family_installs_with_an_accepted_manifest() {
    for family in FAMILIES {
        let plugin = registry::install_bytes(&module(family)).unwrap_or_else(|e| panic!("VectorCraft refuses {family}: {e}"));
        assert_eq!(plugin.id(), format!("dev.stitchcraft.{family}"));
        assert!(plugin.manifest().kind == Kind::Effect, "{family} is a live effect");
        assert!(!plugin.manifest().params.is_empty(), "{family} declares its parameters");
    }
}

/// A path of straight lines through `points`, open, one subpath for each list.
fn lines(subpaths: &[&[(f64, f64)]]) -> PathData {
    let mut path = BezPath::new();
    for points in subpaths {
        for (k, (x, y)) in points.iter().enumerate() {
            if k == 0 { path.move_to(Point::new(*x, *y)) } else { path.line_to(Point::new(*x, *y)) }
        }
    }
    PathData::from_bezpath(&path)
}

/// Each family's live effect sews in VectorCraft's host, with every parameter at its default: a stroke
/// and a satin column come back as paths of stitches, a fill as one closed ribbon for each stitch. The
/// shapes are small, as a preview of all its stitches must be until M6.3's levels.
#[test]
fn each_family_sews_an_object_in_vectorcraft() {
    let bounds = Rect::new(0.0, 0.0, 20.0, 20.0);
    let square = shapes::rectangle(bounds);
    let rails = lines(&[&[(0.0, 0.0), (60.0, 0.0)], &[(0.0, 20.0), (60.0, 20.0)]]);
    for (family, path) in [("stroke", &square), ("satin", &rails), ("fill", &square)] {
        let plugin = registry::install_bytes(&module(family)).unwrap();
        let out = effect::apply(plugin.id(), &json!({}), path, bounds, FillRule::NonZero);
        let out = out.unwrap_or_else(|| panic!("{family}: {:?}", effect::last_error(plugin.id())));
        assert!(!out.subpaths.is_empty(), "{family} sews stitches");
        assert!(out.subpaths.iter().all(|subpath| subpath.closed == (family == "fill")), "{family}");
    }
}

/// When an effect cannot show an object's stitches, VectorCraft draws the object as it is and shows why
/// as the plug-in's last error: a satin column drawn as one path is as wide as its stroke, which the
/// effect is not given.
#[test]
fn what_an_effect_cannot_show_is_its_last_error() {
    let plugin = registry::install_bytes(&module("satin")).unwrap();
    let bounds = Rect::new(0.0, 0.0, 20.0, 20.0);
    let line = lines(&[&[(0.0, 0.0), (60.0, 10.0)]]);
    assert_eq!(effect::apply(plugin.id(), &json!({}), &line, bounds, FillRule::NonZero), None);
    // VectorCraft words it as the plug-in's failure: `plug-in failed: info SC-I0810: …`.
    let error = effect::last_error(plugin.id()).unwrap_or_default();
    assert!(error.contains("info SC-I0810: This satin column is drawn as one path"), "{error}");
}

/// VectorCraft refuses a value a choice does not offer before the effect runs.
#[test]
fn the_host_refuses_a_value_a_choice_does_not_offer() {
    let plugin = registry::install_bytes(&module("fill")).unwrap();
    let refused = plugin.manifest().resolve_params(&json!({"fill_method": "plaid"})).unwrap_err().to_string();
    assert!(refused.contains("fill_method"), "{refused}");
}

/// The parameters VectorCraft passes `plugin`'s `vc_run` for a record that sets `set`: every other
/// declared parameter at its default, and the effect's context.
fn default_params(plugin: &Plugin, set: &Value) -> Vec<u8> {
    let mut params = plugin.manifest().resolve_params(set).unwrap();
    params.insert("_context".into(), json!({"mode": "effect", "bounds": [0, 0, 100, 100]}));
    serde_json::to_vec(&Value::Object(params)).unwrap()
}

/// The anchors of a live effect's output.
fn anchors(output: &[u8]) -> usize {
    let output: Value = serde_json::from_slice(output).unwrap();
    let subpaths = output["objects"].as_array().into_iter().flatten().filter_map(|object| object["path"]["subpaths"].as_array()).flatten();
    subpaths.filter_map(|subpath| subpath["anchors"].as_array()).map(Vec::len).sum()
}

/// Each family's live effect at each preview level, on shapes of growing size, within VectorCraft's
/// limits: every run finishes (`REQ-VC-003`). The test prints what each run takes against what VectorCraft
/// allows it, and the anchors it writes, which the plug-in's budget (`apps/stitchcraft-vc-plugin/src/
/// preview.rs`) is measured from.
#[test]
fn req_vc_003_every_preview_level_finishes_within_vectorcraft_s_limits() {
    let square = |side: f64| shapes::rectangle(Rect::new(0.0, 0.0, side, side));
    let rails = |length: f64| lines(&[&[(0.0, 0.0), (length, 0.0)], &[(0.0, 20.0), (length, 20.0)]]);
    let squares = |sides: &[f64]| sides.iter().map(|side| (format!("square {side} pt"), square(*side))).collect::<Vec<_>>();
    let runs = [
        Runs { family: "stroke", levels: &["auto", "outline"], shapes: squares(&[20.0, 320.0, 5120.0]) },
        Runs { family: "satin", levels: &["auto", "outline"], shapes: [60.0, 960.0, 15360.0].map(|length| (format!("rails {length} pt"), rails(length))).to_vec() },
        Runs { family: "fill", levels: &["auto", "exact", "rows", "outline"], shapes: squares(&[10.0, 20.0, 40.0, 80.0, 160.0, 320.0, 640.0]) },
    ];
    for Runs { family, levels, shapes } in runs {
        let plugin = Plugin::load(&module(family), Limits::default()).unwrap();
        for level in levels {
            let params = default_params(&plugin, &json!({"preview_level": level}));
            let mut costs = Vec::new();
            for (name, path) in &shapes {
                let input = effect_input(path, json!({}));
                let allowed = Limits::default().fuel_base + Limits::default().fuel_per_byte * input.len() as u64;
                let output = plugin.run(&input, &params).unwrap_or_else(|e| panic!("{family} {level} {name}: {e}"));
                let used = fuel(&plugin, &input, &params);
                let shown = serde_json::from_slice::<Value>(&output).ok().and_then(|v| v["error"].as_str().map(|e| e.chars().take(40).collect::<String>()));
                let what = shown.map_or_else(|| format!("{} anchors", anchors(&output)), |error| format!("error {error}…"));
                costs.push(format!("{name}: {what}, {used} of {allowed}"));
            }
            println!("::notice title={family} {level}::{}", costs.join("; "));
        }
    }
}

/// A family's preview runs: the levels it shows each shape at.
struct Runs {
    family: &'static str,
    levels: &'static [&'static str],
    shapes: Vec<(String, PathData)>,
}
