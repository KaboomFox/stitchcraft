//! Level A of the compatibility gate: our plug-ins inside VectorCraft's real plug-in host.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use vectorcraft_doc::{Appearance, Document, Effect, Node, NodeId};
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Rect, shapes};
use vectorcraft_plugins::effect::Paints;
use vectorcraft_plugins::objects::{self, Output};
use vectorcraft_plugins::{ABI_VERSION, Budget, Error, HEAVY_FUEL, HEAVY_WALL_TIME_MS, Kind, Limits, ParamSpec, Plugin, effect, export, registry};

/// The stitch families, each a module of its own whose live effect sews an object as the family.
const FAMILIES: [&str; 3] = ["stroke", "satin", "fill"];

/// The Auto effect's module, whose live effect previews the stitches automatic digitizing chooses for an
/// object (REQ-VC-016).
const AUTO: &str = "auto";

/// The machine formats, each a module of its own whose exporter writes the document in that format, from
/// VectorCraft 0.14, and how a file of it starts.
const FORMATS: [(&str, &[u8]); 2] = [("pes", b"#PES0001"), ("dst", b"LA:")];

/// The module `name`, *hello*, a stitch family's or a machine format's, which `cargo xtask compat contract`
/// built for wasm32-unknown-unknown into the folder `STITCHCRAFT_PLUGINS` names.
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
    let out = effect::apply(plugin.id(), &json!({}), &path, bounds, FillRule::NonZero, Paints::default());
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

/// The live-effect input VectorCraft hands a plug-in for one path of an object without fills or strokes,
/// with `extra` fields beside it.
fn effect_input(path: &PathData, extra: Value) -> Vec<u8> {
    let mut object = json!({"type": "path", "path": path, "fillRule": "nonzero", "bounds": [0, 0, 100, 100], "fills": [], "strokes": []});
    if let (Some(object), Value::Object(extra)) = (object.as_object_mut(), extra) {
        object.extend(extra);
    }
    serde_json::to_vec(&json!({"objects": [object]})).unwrap()
}

/// VectorCraft's `_context` beside a live effect's parameters, for an effect on its object (not on one of
/// the object's fills or strokes), in the box [`effect_input`] gives.
fn context() -> Value {
    json!({"mode": "effect", "bounds": [0, 0, 100, 100], "item": "object"})
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
    let params = serde_json::to_vec(&json!({"_context": context()})).unwrap();
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
    // And the Auto effect, whose parameters are the settings of automatic digitizing's rules (REQ-VC-016).
    for family in FAMILIES.into_iter().chain([AUTO]) {
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
        let out = effect::apply(plugin.id(), &json!({}), path, bounds, FillRule::NonZero, Paints::default());
        let out = out.unwrap_or_else(|| panic!("{family}: {:?}", effect::last_error(plugin.id())));
        assert!(!out.subpaths.is_empty(), "{family} sews stitches");
        assert!(out.subpaths.iter().all(|subpath| subpath.closed == (family == "fill")), "{family}");
    }
}

/// From VectorCraft 0.14 a live effect is also given, read only, the fills and strokes of its object, and
/// `_context.item` says whether it sits on the object or on one of them. On the object, each family's effect
/// sews an object that has a fill and a stroke as it sews one that has none: a running stitch and a satin
/// column drawn with rails don't take a stroke's width. On its fill or its stroke, which `stitch export`
/// doesn't use, the effect leaves the object as it is (`REQ-VC-010`).
#[test]
fn each_family_sews_an_object_that_has_fills_and_strokes() {
    let bounds = Rect::new(0.0, 0.0, 20.0, 20.0);
    let square = shapes::rectangle(bounds);
    let rails = lines(&[&[(0.0, 0.0), (60.0, 0.0)], &[(0.0, 20.0), (60.0, 20.0)]]);
    // New art's appearance: a white fill under a 1 pt black stroke.
    let art = Appearance::default_art();
    for (family, path) in [("stroke", &square), ("satin", &rails), ("fill", &square)] {
        let plugin = registry::install_bytes(&module(family)).unwrap();
        let unpainted = effect::apply(plugin.id(), &json!({}), path, bounds, FillRule::NonZero, Paints::default());
        assert!(unpainted.is_some(), "{family}: {:?}", effect::last_error(plugin.id()));
        let painted = effect::apply(plugin.id(), &json!({}), path, bounds, FillRule::NonZero, Paints { appearance: Some(&art), item: None });
        assert_eq!(painted, unpainted, "{family}: {:?}", effect::last_error(plugin.id()));
        for item in [Some(0), Some(1)] {
            let on_a_paint = effect::apply(plugin.id(), &json!({}), path, bounds, FillRule::NonZero, Paints { appearance: Some(&art), item });
            let error = effect::last_error(plugin.id()).unwrap_or_default();
            assert!(
                on_a_paint.is_none() && error.contains("info SC-I0810: This effect is on one of the object's fills or strokes"),
                "{family} on {item:?}: {error}"
            );
        }
    }
}

/// A satin column drawn as one path, and a zigzag stroke, show their stitches in VectorCraft's host as wide
/// as the stroke VectorCraft gives the effect from 0.14 (`REQ-VC-010`).
#[test]
fn req_vc_010_a_satin_column_drawn_as_one_path_and_a_zigzag_show_their_stitches_as_wide_as_their_stroke() {
    let bounds = Rect::new(0.0, 0.0, 100.0, 0.0);
    let line = lines(&[&[(0.0, 0.0), (100.0, 0.0)]]);
    for (family, set) in [("satin", json!({})), ("stroke", json!({"stroke_method": "zigzag_stitch"}))] {
        let plugin = registry::install_bytes(&module(family)).unwrap();
        for mm in [1.5, 3.0, 5.0] {
            let mut art = Appearance::default_art();
            art.stroke_mut().unwrap().width = mm * 72.0 / 25.4;
            let out = effect::apply(plugin.id(), &set, &line, bounds, FillRule::NonZero, Paints { appearance: Some(&art), item: None });
            let out = out.unwrap_or_else(|| panic!("{family} {mm} mm: {:?}", effect::last_error(plugin.id())));
            let ys = || out.subpaths.iter().flat_map(|subpath| &subpath.anchors).map(|anchor| anchor.p.y);
            let across = (ys().fold(f64::NEG_INFINITY, f64::max) - ys().fold(f64::INFINITY, f64::min)) * 25.4 / 72.0;
            assert!((across - mm).abs() < 0.01, "{family} {mm} mm: its stitches lie {across} mm apart across the line");
        }
    }
}

/// When an effect cannot show an object's stitches, VectorCraft draws the object as it is and shows why
/// as the plug-in's last error: a satin column drawn as one path is as wide as its stroke, and given no
/// stroke the effect cannot tell how wide (`REQ-VC-010`).
#[test]
fn what_an_effect_cannot_show_is_its_last_error() {
    let plugin = registry::install_bytes(&module("satin")).unwrap();
    let bounds = Rect::new(0.0, 0.0, 20.0, 20.0);
    let line = lines(&[&[(0.0, 0.0), (60.0, 10.0)]]);
    assert_eq!(effect::apply(plugin.id(), &json!({}), &line, bounds, FillRule::NonZero, Paints::default()), None);
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
    params.insert("_context".into(), context());
    serde_json::to_vec(&Value::Object(params)).unwrap()
}

/// The anchors of a live effect's output.
fn anchors(output: &[u8]) -> usize {
    let output: Value = serde_json::from_slice(output).unwrap();
    let subpaths = output["objects"].as_array().into_iter().flatten().filter_map(|object| object["path"]["subpaths"].as_array()).flatten();
    subpaths.filter_map(|subpath| subpath["anchors"].as_array()).map(Vec::len).sum()
}

/// A stroke `mm` millimetres wide as VectorCraft 0.15 gives a live effect one, among its object's paints:
/// black, mitred, with butt caps.
fn stroke(mm: f64) -> Value {
    json!({"paint": {"type": "solid", "color": {"model": "rgb", "r": 0, "g": 0, "b": 0}}, "width": mm * 72.0 / 25.4, "join": "miter", "cap": "butt", "miterLimit": 10})
}

/// Each family's live effect at each preview level, on shapes of growing size, within VectorCraft's
/// limits (`REQ-VC-003`). Auto, rows and the outline finish on every shape. An exact preview has all that
/// VectorCraft allows, which a small fill's fits in, and the test notes how far the larger ones get. It
/// prints what each run takes against what VectorCraft allows it, and what it shows, which the plug-in's
/// budget (`apps/stitchcraft-vc-plugin/src/preview.rs`) is measured from. A satin column drawn as one path
/// and a zigzag stroke are given a stroke 3 mm wide, as VectorCraft gives one from 0.14 (`REQ-VC-010`). The
/// Auto effect's previews run on filled shapes, an L and a ring among them, and on lines 3 and 0.5 mm wide
/// (`REQ-VC-016`): its auto level digitizes them within a price of its work that these runs check.
#[test]
fn req_vc_003_every_preview_level_finishes_within_vectorcraft_s_limits() {
    let square = |side: f64| shapes::rectangle(Rect::new(0.0, 0.0, side, side));
    let rails = |length: f64| lines(&[&[(0.0, 0.0), (length, 0.0)], &[(0.0, 20.0), (length, 20.0)]]);
    let squares = |sides: &[f64]| sides.iter().map(|side| (format!("square {side} pt"), square(*side))).collect::<Vec<_>>();
    let fills = squares(&[10.0, 20.0, 40.0, 80.0, 160.0, 320.0, 640.0, 2560.0]);
    // Centre lines: straight, with one bend, and curves flattened to many points.
    let straight = |length: f64| (format!("line {length} pt"), lines(&[&[(0.0, 0.0), (length, 0.0)]]));
    let bent = ("bent line 102 pt".to_string(), lines(&[&[(0.0, 0.0), (60.0, 0.0), (90.0, 30.0)]]));
    let quarter: Vec<(f64, f64)> =
        (0..=16).map(|k| f64::from(k) / 16.0 * std::f64::consts::FRAC_PI_2).map(|a| (100.0 * a.cos(), 100.0 * a.sin())).collect();
    let quarter = ("quarter circle of 17 points".to_string(), lines(&[&quarter]));
    let sine: Vec<(f64, f64)> = (0..32).map(|k| (f64::from(k) * 10.0, 30.0 * (f64::from(k) * 0.4).sin())).collect();
    let sine = ("sine curve of 32 points".to_string(), lines(&[&sine]));
    // A satin column's exact preview of the first 3 fits what VectorCraft allows, and the sine curve's nearly fills it.
    let centre_lines = vec![straight(100.0), bent, quarter, sine, straight(15360.0)];
    let stroked = json!({"fills": [], "strokes": [stroke(3.0)]});
    let zigzag = |level: &str| json!({"preview_level": level, "stroke_method": "zigzag_stitch"});
    let level = |level: &str| json!({"preview_level": level});
    // The Auto effect: filled squares, an L with an arm 5 mm wide and a ring 4 mm wide, as letters are traced,
    // a line traced as an area, and lines a satin column and a running stitch.
    let red = json!({"type": "solid", "color": {"model": "rgb", "r": 1, "g": 0, "b": 0}});
    let filled = json!({"fills": [red], "strokes": []});
    let polygon = |points: &[(f64, f64)]| {
        let mut path = BezPath::new();
        for (k, (x, y)) in points.iter().enumerate() {
            if k == 0 { path.move_to(Point::new(pt(*x), pt(*y))) } else { path.line_to(Point::new(pt(*x), pt(*y))) }
        }
        path.close_path();
        PathData::from_bezpath(&path)
    };
    let mut areas = squares(&[40.0, 160.0, 640.0, 2560.0]);
    areas.push(("L 40 mm tall".to_string(), polygon(&[(0.0, 0.0), (20.0, 0.0), (20.0, 20.0), (5.0, 20.0), (5.0, 40.0), (0.0, 40.0)])));
    // TS-13B's thinnest window line: traced as an area 0.3 mm wide, 20 mm down with a branch 18 mm across. It
    // is narrow everywhere, and finding its centre lines costs digitizing the most instructions a unit.
    let (w, down, across) = (0.15, 12.0, 10.0);
    let window = [(down - w, 0.0), (down + w, 0.0), (down + w, across - w), (30.0, across - w), (30.0, across + w), (down + w, across + w)];
    areas.push(("window's line 0.3 mm wide".to_string(), polygon(&[&window[..], &[(down + w, 20.0), (down - w, 20.0)]].concat())));
    let mut ring = shapes::ellipse(Rect::new(0.0, 0.0, pt(30.0), pt(30.0)));
    ring.subpaths.extend(shapes::ellipse(Rect::new(pt(4.0), pt(4.0), pt(26.0), pt(26.0))).subpaths);
    let ring = vec![("ring 30 mm across".to_string(), ring)];
    let evenodd = json!({"fills": [red], "strokes": [], "fillRule": "evenodd"});
    let widths = |mm: f64| json!({"fills": [], "strokes": [stroke(mm)]});
    let auto_lines = vec![straight(100.0), straight(1000.0)];
    let runs = [
        Runs { family: "stroke", set: level("auto"), given: json!({}), shapes: squares(&[20.0, 320.0, 5120.0, 25600.0]), finishing: usize::MAX },
        Runs { family: "stroke", set: level("outline"), given: json!({}), shapes: squares(&[20.0, 25600.0]), finishing: usize::MAX },
        Runs { family: "stroke", set: zigzag("auto"), given: stroked.clone(), shapes: centre_lines.clone(), finishing: usize::MAX },
        Runs { family: "stroke", set: zigzag("outline"), given: stroked.clone(), shapes: centre_lines.clone(), finishing: usize::MAX },
        Runs {
            family: "satin",
            set: level("auto"),
            given: json!({}),
            shapes: [60.0, 960.0, 15360.0].map(|length| (format!("rails {length} pt"), rails(length))).to_vec(),
            finishing: usize::MAX,
        },
        Runs {
            family: "satin",
            set: level("outline"),
            given: json!({}),
            shapes: vec![("rails 15360 pt".into(), rails(15360.0))],
            finishing: usize::MAX,
        },
        Runs { family: "satin", set: level("auto"), given: stroked.clone(), shapes: centre_lines.clone(), finishing: usize::MAX },
        Runs { family: "satin", set: level("outline"), given: stroked.clone(), shapes: centre_lines.clone(), finishing: usize::MAX },
        Runs { family: "satin", set: level("exact"), given: stroked, shapes: centre_lines, finishing: 3 },
        Runs { family: "fill", set: level("auto"), given: json!({}), shapes: fills.clone(), finishing: usize::MAX },
        Runs { family: "fill", set: level("rows"), given: json!({}), shapes: fills.clone(), finishing: usize::MAX },
        Runs { family: "fill", set: level("outline"), given: json!({}), shapes: fills.clone(), finishing: usize::MAX },
        // The 3.5 mm and 7 mm squares.
        Runs { family: "fill", set: level("exact"), given: json!({}), shapes: fills, finishing: 2 },
        Runs { family: AUTO, set: level("auto"), given: filled.clone(), shapes: areas.clone(), finishing: usize::MAX },
        Runs { family: AUTO, set: level("outline"), given: filled.clone(), shapes: areas.clone(), finishing: usize::MAX },
        Runs { family: AUTO, set: level("auto"), given: evenodd.clone(), shapes: ring.clone(), finishing: usize::MAX },
        Runs { family: AUTO, set: level("outline"), given: evenodd, shapes: ring, finishing: usize::MAX },
        Runs { family: AUTO, set: level("auto"), given: widths(3.0), shapes: auto_lines.clone(), finishing: usize::MAX },
        Runs { family: AUTO, set: level("auto"), given: widths(0.5), shapes: auto_lines.clone(), finishing: usize::MAX },
        // An exact preview digitizes with all that VectorCraft allows: the line's satin column fits.
        Runs { family: AUTO, set: level("exact"), given: widths(3.0), shapes: auto_lines, finishing: 1 },
        Runs { family: AUTO, set: level("exact"), given: filled, shapes: areas, finishing: 0 },
    ];
    let mut failed = Vec::new();
    for Runs { family, set, given, shapes, finishing } in runs {
        let plugin = Plugin::load(&module(family), Limits::default()).unwrap();
        let params = default_params(&plugin, &set);
        let level =
            set.as_object().map(|set| set.iter().map(|(key, value)| format!("{key} {value}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let mut costs = Vec::new();
        for (k, (name, path)) in shapes.iter().enumerate() {
            let input = effect_input(path, given.clone());
            let allowed = Limits::default().fuel_base + Limits::default().fuel_per_byte * input.len() as u64;
            // Measured only when it finishes: a run VectorCraft stops may run on far longer without a limit.
            let what = match plugin.run(&input, &params) {
                Ok(output) => {
                    let error = serde_json::from_slice::<Value>(&output).ok().and_then(|v| v["error"].as_str().map(str::to_string));
                    let shown = error.map_or_else(
                        || format!("{} anchors", anchors(&output)),
                        |error| format!("error {}…", error.chars().take(60).collect::<String>()),
                    );
                    format!("{shown}, {} of {allowed}", fuel(&plugin, &input, &params))
                }
                Err(e) if k < finishing => {
                    failed.push(format!("{family} {level} {name}"));
                    format!("FAILED ({e})")
                }
                Err(e) => format!("stopped ({e})"),
            };
            costs.push(format!("{name}: {what}"));
        }
        println!("::notice title={family} {level}::{}", costs.join("; "));
    }
    assert!(failed.is_empty(), "past VectorCraft's limits: {failed:?}");
}

/// A family's preview runs with a record's settings, among them its preview level: what VectorCraft gives
/// beside each shape's path, the shapes, and how many of the first must finish.
struct Runs {
    family: &'static str,
    set: Value,
    given: Value,
    shapes: Vec<(String, PathData)>,
    finishing: usize,
}

#[test]
fn each_exporter_installs_as_an_exporter_of_its_format() {
    for (format, _) in FORMATS {
        let plugin = registry::install_bytes(&module(format)).unwrap_or_else(|e| panic!("VectorCraft refuses {format}: {e}"));
        assert_eq!(plugin.id(), format!("dev.stitchcraft.{format}"));
        let manifest = plugin.manifest();
        assert!(manifest.kind == Kind::Exporter, "{format} is an exporter");
        assert_eq!(manifest.extensions, [format]);
        // `stitch export`'s options, as they are when it isn't given them: the machine is the one the
        // document keeps, or else the reference machine (`REQ-VC-013`).
        let defaults = manifest.defaults();
        assert_eq!((defaults.get("auto"), defaults.get("profile")), (Some(&json!(false)), Some(&json!("document"))), "{format}");
    }
}

/// A document as VectorCraft saves it, without indentation, whose one layer holds `objects`: what an
/// exporter is given (VectorCraft's `docs/plugins.md`, Exporters).
fn document(objects: &[Value]) -> Vec<u8> {
    let layer = json!({"id": 1, "kind": {"type": "layer", "color": {"Preset": 0}, "children": objects}});
    serde_json::to_vec(
        &json!({"format": "vectorcraft", "version": 4, "document": {"version": 1, "artboards": [], "next_id": 1000, "layers": [layer]}}),
    )
    .unwrap()
}

/// The object `id`: a rectangle `width` × `height` mm from (`x`, `y`) mm, with a red fill and StitchCraft's
/// fill effect with `params`.
fn filled(id: u32, (x, y): (f64, f64), (width, height): (f64, f64), params: &Value) -> Value {
    let pt = |mm: f64| mm * 72.0 / 25.4;
    let corners = [(x, y), (x + width, y), (x + width, y + height), (x, y + height)].map(|(x, y)| json!({"p": [pt(x), pt(y)]}));
    let red = json!({"type": "solid", "color": {"model": "rgb", "r": 0.75, "g": 0.0, "b": 0.0}});
    json!({
        "id": id,
        "appearance": {"items": [{"kind": "fill", "paint": red}], "effects": [{"id": "plugin.dev.stitchcraft.fill", "params": params}]},
        "kind": {"type": "path", "path": {"subpaths": [{"anchors": corners, "closed": true}]}}
    })
}

/// `text` decoded from padded base64, as VectorCraft decodes an exporter's file.
fn base64_decoded(text: &str) -> Vec<u8> {
    let value = |c: u8| -> u32 {
        let digits = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        digits.iter().position(|&d| d == c).map_or(0, |v| v as u32)
    };
    let mut out = Vec::new();
    for chunk in text.as_bytes().chunks(4) {
        let n = chunk.iter().fold(0, |n, &c| n << 6 | value(c));
        let padding = chunk.iter().filter(|&&c| c == b'=').count();
        out.extend(n.to_be_bytes().iter().skip(1).take(3 - padding));
    }
    out
}

/// Each exporter writes a file of its format in VectorCraft's host, with the design named as VectorCraft
/// names the file, and what the export says beside it. It plans for the machine its dialog names, and
/// when anything said is an error it writes nothing, and VectorCraft says the error (`REQ-VC-011`).
#[test]
fn req_vc_011_each_exporter_writes_its_format_and_nothing_when_an_error_is_said() {
    let square = document(&[filled(2, (0.0, 0.0), (50.0, 50.0), &json!({}))]);
    for (format, start) in FORMATS {
        let plugin = registry::install_bytes(&module(format)).unwrap();
        let target = export::Target { extension: format, name: "Poster" };
        let out = export::run(&plugin, &square, &json!({}), target).unwrap_or_else(|e| panic!("{format}: {e}"));
        let file = base64_decoded(&out.data_base64);
        assert!(file.starts_with(start), "{format}");
        // The small hoop's field is 20 × 60 mm, too small for the square.
        let small = export::run(&plugin, &square, &json!({"profile": "brother-pe800-small"}), target).unwrap_err().to_string();
        assert!(small.starts_with("plug-in failed: error SC-E0701: "), "{format}: {small}");
        // A profile the dialog doesn't offer is refused before the exporter runs.
        assert!(matches!(export::run(&plugin, &square, &json!({"profile": "singer"}), target), Err(Error::Params(_))), "{format}");
        let nothing = export::run(&plugin, &document(&[]), &json!({}), target).unwrap_err().to_string();
        assert!(nothing.starts_with("plug-in failed: error SC-E0010: "), "{format}: {nothing}");
    }
    // A DST file's header holds the design's name, and DST machines cut the thread before a long jump, which
    // the export says.
    let dst = registry::install_bytes(&module("dst")).unwrap();
    let apart = document(&[filled(2, (0.0, 0.0), (20.0, 20.0), &json!({})), filled(3, (0.0, 150.0), (20.0, 20.0), &json!({}))]);
    let out = export::run(&dst, &apart, &json!({}), export::Target { extension: "dst", name: "Poster" }).unwrap();
    assert!(base64_decoded(&out.data_base64).starts_with(b"LA:Poster "));
    assert!(out.warnings.iter().any(|warning| warning.starts_with("info SC-I0605: ")), "{:?}", out.warnings);
}

/// The parameters VectorCraft passes an exporter's `vc_run` for an export to `target` that sets `set`: every
/// other parameter at its default, and the file's context, as `export::run` makes them.
fn export_params(plugin: &Plugin, set: &Value, target: export::Target) -> Vec<u8> {
    let mut params = plugin.manifest().resolve_params(set).unwrap();
    params.insert("_context".into(), json!({"mode": "export", "extension": target.extension, "name": target.name}));
    serde_json::to_vec(&Value::Object(params)).unwrap()
}

/// The instructions `plugin` takes to export `input` with `params` in VectorCraft's host, to within a
/// hundredth: the least budget it finishes within, found by halving.
fn export_fuel(plugin: &Plugin, input: &[u8], params: &[u8]) -> u64 {
    least_fuel(plugin, input, params, &export::limits(plugin))
}

/// The instructions `plugin` takes to run on `input` with `params` in VectorCraft's host, with the other
/// `limits`, to within a hundredth: the least budget it finishes within, found by halving.
fn least_fuel(plugin: &Plugin, input: &[u8], params: &[u8], limits: &Limits) -> u64 {
    let within = |fuel_base| plugin.run_limited(input, params, &Limits { fuel_base, fuel_per_byte: 0, ..limits.clone() }).is_ok();
    let (mut lo, mut hi) = (0_u64, 1 << 36);
    assert!(within(hi), "the run fails: {:?}", plugin.run_limited(input, params, limits).err());
    while (hi - lo) * 100 > hi {
        let mid = lo + (hi - lo) / 2;
        if within(mid) { hi = mid } else { lo = mid }
    }
    hi
}

/// The largest designs the reference machine sews export within what VectorCraft lets an export run
/// (`REQ-VC-011`): a fill of its 5 × 7 in hoop's whole field, and the field filled with squares, each a fill
/// of its own. It prints what each takes of what VectorCraft allows, which the integration page records.
#[test]
fn req_vc_011_a_fill_of_the_whole_hoop_exports_within_vectorcraft_s_limits() {
    let field = document(&[filled(2, (0.0, 0.0), (128.0, 178.0), &json!({}))]);
    // 63 squares 14 mm wide, in 9 rows of 7, 4 mm apart across and 5.5 mm apart down.
    let squares: Vec<Value> =
        (0..63_u32).map(|i| filled(10 + i, (f64::from(i % 7) * 18.0, f64::from(i / 7) * 19.5), (14.0, 14.0), &json!({}))).collect();
    let plugin = Plugin::load(&module("pes"), Limits::default()).unwrap();
    let target = export::Target { extension: "pes", name: "field" };
    let mut costs = Vec::new();
    for (name, input) in [("a fill of 128 × 178 mm", field), ("63 fills of 14 mm squares", document(&squares))] {
        let allowed = export::FUEL + export::limits(&plugin).fuel_per_byte * input.len() as u64;
        let start = Instant::now();
        let out = export::run(&plugin, &input, &json!({}), target);
        let seconds = start.elapsed().as_secs_f64();
        let out = out.unwrap_or_else(|e| panic!("{name}: {e}"));
        let used = export_fuel(&plugin, &input, &export_params(&plugin, &json!({}), target));
        costs.push(format!("{name}: {used} of {allowed} instructions, {seconds:.1} s, {} bytes of PES", base64_decoded(&out.data_base64).len()));
    }
    println!("::notice title=exports::{}", costs.join("; "));
}

/// The Design Settings filter, from VectorCraft 0.16, run as `plugin.run` runs a filter: it keeps its
/// dialog's settings and machine with the document and returns its objects as they are, and each exporter
/// plans the document for the machine kept, unless its dialog names another (`REQ-VC-013`).
#[test]
fn req_vc_013_the_settings_filter_keeps_the_machine_each_exporter_plans_for() {
    let plugin = registry::install_bytes(&module("settings")).unwrap_or_else(|e| panic!("VectorCraft refuses settings: {e}"));
    assert_eq!(plugin.id(), "dev.stitchcraft.settings");
    let manifest = plugin.manifest();
    assert!(manifest.kind == Kind::Filter, "settings is a filter");
    // A 50 mm square, which the small hoop's field of 20 × 60 mm can't hold, selected.
    let file: Value = serde_json::from_slice(&document(&[filled(2, (0.0, 0.0), (50.0, 50.0), &json!({}))])).unwrap();
    let mut doc: Document = serde_json::from_value(file["document"].clone()).unwrap();
    let before = serde_json::to_value(&doc).unwrap();
    let inputs = objects::filter_targets(&doc, &[NodeId(2)]);
    let dialog = manifest.resolve_params(&json!({"profile": "brother-pe800-small", "min_stitch_len_mm": 1.5})).unwrap();
    let mut params = dialog.clone();
    params.insert("_context".into(), json!({"mode": "filter", "bounds": null, "artboard": null, "documentData": null}));
    let out = plugin.run(&objects::encode_input(&doc, &inputs), &serde_json::to_vec(&params).unwrap()).unwrap();
    let Output { objects: changed, document_data } = objects::decode_output(&out).unwrap();
    // What it keeps is its dialog's values, and the document is as it was: the host refuses any effect
    // records the filter would give its objects.
    assert_eq!(document_data, Some(Some(Value::Object(dialog))));
    let refuse: &dyn Fn(&[Effect], &Node) -> Result<Vec<Effect>, String> = &|_, _| Err("the filter changes no effect".to_string());
    assert_eq!(objects::apply_output(&mut doc, &inputs, changed, None, refuse).unwrap(), inputs);
    assert_eq!(serde_json::to_value(&doc).unwrap(), before);
    doc.set_plugin_data(plugin.id(), document_data.flatten());
    let saved = serde_json::to_vec(&json!({"format": "vectorcraft", "version": 4, "document": doc})).unwrap();
    for (format, start) in FORMATS {
        let exporter = registry::install_bytes(&module(format)).unwrap();
        let target = export::Target { extension: format, name: "Poster" };
        let kept = export::run(&exporter, &saved, &json!({}), target).unwrap_err().to_string();
        assert!(kept.starts_with("plug-in failed: error SC-E0701: "), "{format}: {kept}");
        let named = export::run(&exporter, &saved, &json!({"profile": "brother-pe800-5x7"}), target).unwrap_or_else(|e| panic!("{format}: {e}"));
        assert!(base64_decoded(&named.data_base64).starts_with(start), "{format}");
    }
}

/// Millimetres in points.
fn pt(mm: f64) -> f64 {
    mm * 72.0 / 25.4
}

/// The document `doc` as VectorCraft saves it: what an exporter is given.
fn saved(doc: &Document) -> Vec<u8> {
    serde_json::to_vec(&json!({"format": "vectorcraft", "version": 4, "document": doc})).unwrap()
}

/// The document of a layer holding `objects`, as VectorCraft reads it.
fn read_document(objects: &[Value]) -> Document {
    let file: Value = serde_json::from_slice(&document(objects)).unwrap();
    serde_json::from_value(file["document"].clone()).unwrap()
}

/// Admits the effect records a filter gives object `node` as `plugin.run` admits them (`admit_effects` in
/// VectorCraft's engine, which this crate doesn't link): a record it had stays as it was, and a new one of an
/// installed live effect has its parameters resolved by the effect's manifest.
fn admit(effects: &[Effect], node: &Node) -> Result<Vec<Effect>, String> {
    let admitted = |e: &Effect| {
        if node.appearance.effects.contains(e) {
            return Ok(e.clone());
        }
        let plugin = effect::installed(&e.id).ok_or_else(|| format!("`{}`: no such effect", e.id))?;
        let params = if e.params.is_null() { json!({}) } else { e.params.clone() };
        let resolved = plugin.manifest().resolve_params(&params).map_err(|why| format!("`{}`: {why}", e.id))?;
        Ok(Effect { id: e.id.clone(), params: Value::Object(resolved), visible: e.visible })
    };
    effects.iter().map(admitted).collect()
}

/// Runs the filter `plugin` on the objects `ids` of `doc`, with its dialog's `set`, as `plugin.run` runs it
/// for a document whose first artboard is a Letter page and which keeps nothing for it, and applies what it
/// gives back to `doc`, new objects into the layer `layer`: the instructions the run took, and how many
/// VectorCraft allows it.
fn filtered(plugin: &Plugin, doc: &mut Document, ids: &[NodeId], set: &Value, layer: NodeId) -> Result<(u64, u64), String> {
    let inputs = objects::filter_targets(doc, ids);
    let input = objects::encode_input(doc, &inputs);
    let mut params = plugin.manifest().resolve_params(set).map_err(|e| e.to_string())?;
    params.insert("_context".into(), json!({"mode": "filter", "bounds": null, "artboard": [0, 0, 612, 792], "documentData": null}));
    let params = serde_json::to_vec(&Value::Object(params)).unwrap();
    let out = plugin.run(&input, &params).map_err(|e| e.to_string())?;
    let Output { objects: changed, .. } = objects::decode_output(&out).map_err(|e| e.to_string())?;
    objects::apply_output(doc, &inputs, changed, Some(layer), &admit).map_err(|e| e.to_string())?;
    let heavy = plugin.manifest().budget == Budget::Heavy;
    let limits = if heavy { Limits { wall_time_ms: HEAVY_WALL_TIME_MS, ..Limits::default() } } else { Limits::default() };
    let base = if heavy { HEAVY_FUEL } else { Limits::default().fuel_base };
    Ok((least_fuel(plugin, &input, &params, &limits), base + Limits::default().fuel_per_byte * input.len() as u64))
}

/// The lines M6.4 measured making into satin columns, in millimetres (`docs/src/design/vectorcraft-integration.md`):
/// a zigzag of 10 corners, S curves 30 and 60 mm long, and circles 10, 30 and 50 mm across.
fn lines_to_convert() -> Vec<(&'static str, PathData)> {
    let at = |x: f64, y: f64| Point::new(pt(x), pt(y));
    let mut zigzag = BezPath::new();
    zigzag.move_to(at(0.0, 0.0));
    for k in 1..12_u32 {
        zigzag.line_to(at(f64::from(k) * 5.0, if k % 2 == 0 { 0.0 } else { 4.0 }));
    }
    let s = |k: f64| {
        let mut s = BezPath::new();
        s.move_to(at(0.0, 0.0));
        s.curve_to(at(5.0 * k, -8.0 * k), at(10.0 * k, -8.0 * k), at(15.0 * k, 0.0));
        s.curve_to(at(20.0 * k, 8.0 * k), at(25.0 * k, 8.0 * k), at(30.0 * k, 0.0));
        PathData::from_bezpath(&s)
    };
    let circle = |across: f64| shapes::ellipse(Rect::new(0.0, 0.0, pt(across), pt(across)));
    vec![
        ("a zigzag of 10 corners", PathData::from_bezpath(&zigzag)),
        ("an S curve 30 mm long", s(1.0)),
        ("a circle 10 mm across", circle(10.0)),
        ("a circle 30 mm across", circle(30.0)),
        ("an S curve 60 mm long", s(2.0)),
        ("a circle 50 mm across", circle(50.0)),
    ]
}

/// The object `id`: `path` with a navy stroke 3 mm wide and StitchCraft's satin effect.
fn satin_line(id: u32, path: &PathData) -> Value {
    let navy = json!({"type": "solid", "color": {"model": "rgb", "r": 0.0, "g": 0.2, "b": 0.6}});
    let stroke = json!({"kind": "stroke", "paint": navy, "width": pt(3.0)});
    json!({"id": id, "appearance": {"items": [stroke], "effects": [{"id": "plugin.dev.stitchcraft.satin", "params": {}}]}, "kind": {"type": "path", "path": path}})
}

/// StitchCraft Stroke to Satin, from VectorCraft 0.17, is a heavy filter, which VectorCraft's host gives
/// 1 G instructions, run as `plugin.run` runs it. Each line M6.4 measured becomes a compound path of rails
/// and rungs with the satin effect, which StitchCraft's PES exporter writes as the line's file: the column
/// sews as the line did (`REQ-VC-014`). It prints what each takes of what VectorCraft allows, from which the
/// plug-in prices its work (`apps/stitchcraft-vc-plugin/src/stroke_to_satin.rs`).
#[test]
fn req_vc_014_stroke_to_satin_makes_columns_that_export_as_the_lines_did() {
    let plugin = registry::install_bytes(&module("stroke-to-satin")).unwrap_or_else(|e| panic!("VectorCraft refuses stroke-to-satin: {e}"));
    assert_eq!(plugin.id(), "dev.stitchcraft.stroke-to-satin");
    assert!(plugin.manifest().kind == Kind::Filter && plugin.manifest().budget == Budget::Heavy, "a heavy filter");
    assert_eq!(HEAVY_FUEL, 1_000_000_000, "VectorCraft's heavy budget changed: review the plug-in's HEAVY_FUEL");
    // VectorCraft admits the satin effect's records only while StitchCraft Satin is installed.
    registry::install_bytes(&module("satin")).unwrap();
    let pes = registry::install_bytes(&module("pes")).unwrap();
    let target = export::Target { extension: "pes", name: "Column" };
    let mut costs = Vec::new();
    for (k, (name, path)) in lines_to_convert().into_iter().enumerate() {
        let id = 2 + k as u32;
        let mut doc = read_document(&[satin_line(id, &path)]);
        let line = export::run(&pes, &saved(&doc), &json!({}), target).unwrap_or_else(|e| panic!("{name}: {e}"));
        let (used, allowed) = filtered(&plugin, &mut doc, &[NodeId(u64::from(id))], &json!({}), NodeId(1)).unwrap_or_else(|e| panic!("{name}: {e}"));
        let column = doc.node(NodeId(u64::from(id))).unwrap();
        let effects: Vec<&str> = column.appearance.effects.iter().map(|e| e.id.as_str()).collect();
        assert_eq!(effects, ["plugin.dev.stitchcraft.satin"], "{name}");
        let made = export::run(&pes, &saved(&doc), &json!({}), target).unwrap_or_else(|e| panic!("{name}: {e}"));
        assert!(made.data_base64 == line.data_base64, "{name}: the column exports otherwise than the line");
        costs.push(format!("{name}: {used} of {allowed}"));
    }
    println!("::notice title=stroke to satin::{}", costs.join("; "));
}

/// StitchCraft Test Sheet draws each sheet its dialog offers into a new document as `plugin.run` runs it,
/// within a light filter's budget, each object with its StitchCraft effect, and StitchCraft's PES exporter
/// writes the document within an export's budget (`REQ-VC-015`). It prints what each takes of what
/// VectorCraft allows.
#[test]
fn req_vc_015_each_test_sheet_draws_into_a_document_that_exports_within_vectorcraft_s_limits() {
    let plugin = registry::install_bytes(&module("testsheet")).unwrap_or_else(|e| panic!("VectorCraft refuses testsheet: {e}"));
    assert_eq!(plugin.id(), "dev.stitchcraft.testsheet");
    assert!(plugin.manifest().kind == Kind::Filter && plugin.manifest().budget == Budget::Light, "a light filter");
    let sheets = match plugin.manifest().params.iter().find(|(key, _)| key == "sheet").map(|(_, param)| &param.spec) {
        Some(ParamSpec::Choice { options, .. }) => options.clone(),
        other => panic!("no choice of sheets: {other:?}"),
    };
    assert_eq!(sheets, ["TS-02B", "TS-03", "TS-04", "TS-05", "TS-06", "TS-07", "TS-08", "TS-09", "TS-13A", "TS-13B", "TS-13C", "TS-13D"]);
    // VectorCraft admits StitchCraft's effect records only while their plug-ins are installed: the families',
    // and the Auto effect's, which the TS-13 sheets' shapes digitized automatically have.
    for family in FAMILIES.into_iter().chain([AUTO]) {
        registry::install_bytes(&module(family)).unwrap();
    }
    let pes = Plugin::load(&module("pes"), Limits::default()).unwrap();
    let mut costs = Vec::new();
    for sheet in &sheets {
        let mut doc = read_document(&[]);
        let (used, allowed) = filtered(&plugin, &mut doc, &[], &json!({ "sheet": sheet }), NodeId(1)).unwrap_or_else(|e| panic!("{sheet}: {e}"));
        let input = saved(&doc);
        let target = export::Target { extension: "pes", name: sheet };
        let out = export::run(&pes, &input, &json!({}), target).unwrap_or_else(|e| panic!("{sheet}: {e}"));
        assert!(base64_decoded(&out.data_base64).starts_with(b"#PES0001"), "{sheet}");
        let exported = export_fuel(&pes, &input, &export_params(&pes, &json!({}), target));
        let export_allowed = export::FUEL + export::limits(&pes).fuel_per_byte * input.len() as u64;
        costs.push(format!("{sheet}: drawn in {used} of {allowed} instructions, exported in {exported} of {export_allowed}"));
    }
    println!("::notice title=test sheets::{}", costs.join("; "));
}
