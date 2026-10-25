//! Level A of the compatibility gate: our plug-ins inside VectorCraft's real plug-in host.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;
use std::time::Instant;

use serde_json::{Value, json};
use vectorcraft_doc::Appearance;
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Rect, shapes};
use vectorcraft_plugins::effect::Paints;
use vectorcraft_plugins::{ABI_VERSION, Error, Kind, Limits, Plugin, effect, export, registry};

/// The stitch families, each a module of its own whose live effect sews an object as the family.
const FAMILIES: [&str; 3] = ["stroke", "satin", "fill"];

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
            assert!(on_a_paint.is_none() && error.contains("info SC-I0810: This effect is on one of the object's fills or strokes"), "{family} on {item:?}: {error}");
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
/// and a zigzag stroke are given a stroke 3 mm wide, as VectorCraft gives one from 0.14 (`REQ-VC-010`).
#[test]
fn req_vc_003_every_preview_level_finishes_within_vectorcraft_s_limits() {
    let square = |side: f64| shapes::rectangle(Rect::new(0.0, 0.0, side, side));
    let rails = |length: f64| lines(&[&[(0.0, 0.0), (length, 0.0)], &[(0.0, 20.0), (length, 20.0)]]);
    let squares = |sides: &[f64]| sides.iter().map(|side| (format!("square {side} pt"), square(*side))).collect::<Vec<_>>();
    let fills = squares(&[10.0, 20.0, 40.0, 80.0, 160.0, 320.0, 640.0, 2560.0]);
    // Centre lines: straight, with one bend, and curves flattened to many points.
    let straight = |length: f64| (format!("line {length} pt"), lines(&[&[(0.0, 0.0), (length, 0.0)]]));
    let bent = ("bent line 102 pt".to_string(), lines(&[&[(0.0, 0.0), (60.0, 0.0), (90.0, 30.0)]]));
    let quarter: Vec<(f64, f64)> = (0..=16).map(|k| f64::from(k) / 16.0 * std::f64::consts::FRAC_PI_2).map(|a| (100.0 * a.cos(), 100.0 * a.sin())).collect();
    let quarter = ("quarter circle of 17 points".to_string(), lines(&[&quarter]));
    let sine: Vec<(f64, f64)> = (0..32).map(|k| (f64::from(k) * 10.0, 30.0 * (f64::from(k) * 0.4).sin())).collect();
    let sine = ("sine curve of 32 points".to_string(), lines(&[&sine]));
    // A satin column's exact preview of the first 3 fits what VectorCraft allows, and the sine curve's nearly fills it.
    let centre_lines = vec![straight(100.0), bent, quarter, sine, straight(15360.0)];
    let stroked = json!({"fills": [], "strokes": [stroke(3.0)]});
    let zigzag = |level: &str| json!({"preview_level": level, "stroke_method": "zigzag_stitch"});
    let level = |level: &str| json!({"preview_level": level});
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
        Runs { family: "satin", set: level("outline"), given: json!({}), shapes: vec![("rails 15360 pt".into(), rails(15360.0))], finishing: usize::MAX },
        Runs { family: "satin", set: level("auto"), given: stroked.clone(), shapes: centre_lines.clone(), finishing: usize::MAX },
        Runs { family: "satin", set: level("outline"), given: stroked.clone(), shapes: centre_lines.clone(), finishing: usize::MAX },
        Runs { family: "satin", set: level("exact"), given: stroked, shapes: centre_lines, finishing: 3 },
        Runs { family: "fill", set: level("auto"), given: json!({}), shapes: fills.clone(), finishing: usize::MAX },
        Runs { family: "fill", set: level("rows"), given: json!({}), shapes: fills.clone(), finishing: usize::MAX },
        Runs { family: "fill", set: level("outline"), given: json!({}), shapes: fills.clone(), finishing: usize::MAX },
        // The 3.5 mm and 7 mm squares.
        Runs { family: "fill", set: level("exact"), given: json!({}), shapes: fills, finishing: 2 },
    ];
    let mut failed = Vec::new();
    for Runs { family, set, given, shapes, finishing } in runs {
        let plugin = Plugin::load(&module(family), Limits::default()).unwrap();
        let params = default_params(&plugin, &set);
        let level = set.as_object().map(|set| set.iter().map(|(key, value)| format!("{key} {value}")).collect::<Vec<_>>().join(", ")).unwrap_or_default();
        let mut costs = Vec::new();
        for (k, (name, path)) in shapes.iter().enumerate() {
            let input = effect_input(path, given.clone());
            let allowed = Limits::default().fuel_base + Limits::default().fuel_per_byte * input.len() as u64;
            // Measured only when it finishes: a run VectorCraft stops may run on far longer without a limit.
            let what = match plugin.run(&input, &params) {
                Ok(output) => {
                    let error = serde_json::from_slice::<Value>(&output).ok().and_then(|v| v["error"].as_str().map(str::to_string));
                    let shown = error.map_or_else(|| format!("{} anchors", anchors(&output)), |error| format!("error {}…", error.chars().take(60).collect::<String>()));
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
        // `stitch export`'s options, as they are when it isn't given them.
        let defaults = manifest.defaults();
        assert_eq!((defaults.get("auto"), defaults.get("profile")), (Some(&json!(false)), Some(&json!("brother-pe800-5x7"))), "{format}");
    }
}

/// A document as VectorCraft saves it, without indentation, whose one layer holds `objects`: what an
/// exporter is given (VectorCraft's `docs/plugins.md`, Exporters).
fn document(objects: &[Value]) -> Vec<u8> {
    let layer = json!({"id": 1, "kind": {"type": "layer", "color": {"Preset": 0}, "children": objects}});
    serde_json::to_vec(&json!({"format": "vectorcraft", "version": 4, "document": {"version": 1, "artboards": [], "next_id": 1000, "layers": [layer]}}))
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
    let within = |fuel_base| plugin.run_limited(input, params, &Limits { fuel_base, fuel_per_byte: 0, ..export::limits(plugin) }).is_ok();
    let (mut lo, mut hi) = (0_u64, 1 << 36);
    assert!(within(hi), "the export fails: {:?}", plugin.run_limited(input, params, &export::limits(plugin)).err());
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
    let squares: Vec<Value> = (0..63_u32).map(|i| filled(10 + i, (f64::from(i % 7) * 18.0, f64::from(i / 7) * 19.5), (14.0, 14.0), &json!({}))).collect();
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
