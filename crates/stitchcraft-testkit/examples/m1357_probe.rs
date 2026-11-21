//! M13.57 probe (NOT FOR THE REPOSITORY). For each .vectorcraft document given, digitized automatically, for
//! each style of PROBE_STYLES (`default`, `logo`) and each candidate of PROBE_CANDS, plans the design and
//! writes one JSON line: the plan's numbers, and for `main` what lies stacked at each spot above the cap,
//! after the cap and before it. Candidates (M13.57 brief, 3a-3f):
//!   main      main's engine
//!   a04/a02   auto_column_overlap_mm 0.4 / 0.2 (M13.55's setting)
//!   b         bound: no fill underlay on a fill that lies under a column at a spot left above the cap
//!   c / c0    centre walk only / no underlay, on every column that lies on a fill (half its spots or more)
//!   d         spacing opened evenly on a column on a fill that lays thread at a spot left above the cap
//!   e         a part's column on a fill at a spot left above the cap sews as a running line
//!   f04/f0    a column reaches 0.4 / 0 past a rail along a later satin column (REQ-AUTO-029), not the overlap
//!   af        a04 and f04 together
//!   pc0       measure only: auto_pull_compensation_mm 0
//!   nocap     measure only: the plan before the last pass and the cap
#![allow(clippy::unwrap_used, clippy::print_stdout, clippy::print_stderr, clippy::indexing_slicing, missing_docs, clippy::disallowed_methods, clippy::disallowed_types)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::Path as FsPath;
use std::time::Instant;

use stitchcraft_core::{Budget, ElementId, Point};
use stitchcraft_engine::design::{Design, Reading, Shape};
use stitchcraft_engine::probe;
use stitchcraft_plan::thickness::{Measure, Thickness};
use stitchcraft_plan::{Role, StitchPlan};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_render::{Settings, Style};
use stitchcraft_testkit::designs::in_place;
use stitchcraft_testkit::fabric;

const CAP: f64 = 3.25;
const LIMIT: f64 = 4.0;
const SIG: f64 = 0.3;
const CATS: [&str; 14] = [
    "fill_top", "fill_under", "base", "col_top", "col_under", "scol_top", "scol_under", "letter", "run", "person", "travel", "lock", "other_top", "other_under",
];

fn q(text: &str) -> String {
    format!("{text:?}")
}

fn styled(design: &Design, style: &str, extra: &[(&str, &str)]) -> Design {
    let mut pairs: Vec<(String, String)> = extra.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect();
    if style == "logo" {
        pairs.push(("auto_prefer".into(), "satin".into()));
        pairs.push(("auto_fill_uneven".into(), "false".into()));
    }
    let elements = design
        .elements()
        .iter()
        .cloned()
        .map(|mut element| {
            if element.automatic {
                for (k, v) in &pairs {
                    if element.params.get(k).is_none() {
                        element.params.set(k, v);
                    }
                }
            }
            element
        })
        .collect();
    let mut d = design.with_elements(elements).unwrap();
    d.marks.clone_from(&design.marks);
    d.guides.clone_from(&design.guides);
    d
}

/// The element kind of each stitch's element, by the probe's kinds and the design's shapes.
struct Kinds {
    of: Vec<String>,
    keys: Vec<String>,
    under: BTreeMap<usize, usize>,
}

/// The entries of `plan` that are a column's underlay, tagged top: the first `n` top entries of each.
fn under_entries(plan: &StitchPlan, kinds: &Kinds) -> BTreeSet<(usize, usize)> {
    let mut left = kinds.under.clone();
    let mut out = BTreeSet::new();
    for (bi, b) in plan.blocks.iter().enumerate() {
        for (i, e) in b.stitches.iter().enumerate() {
            if e.origin.role != Role::Top {
                continue;
            }
            if let Some(x) = e.origin.element.map(|x| x.index()) {
                if let Some(n) = left.get_mut(&x) {
                    if *n > 0 {
                        *n -= 1;
                        out.insert((bi, i));
                    }
                }
            }
        }
    }
    out
}

fn kinds(design: &Design, plan: &StitchPlan) -> Kinds {
    let table = probe::KINDS.lock().unwrap().clone();
    let strokes: BTreeSet<String> =
        design.elements().iter().filter(|e| matches!(e.shape, Shape::Stroke { .. })).map(|e| e.id.as_str().to_string()).collect();
    let unders = probe::UNDERLAY.lock().unwrap().clone();
    let mut of = Vec::new();
    let mut keys = Vec::new();
    let mut under = BTreeMap::new();
    for (x, id) in plan.elements.iter().enumerate() {
        let (kind, key) = table.get(id).cloned().unwrap_or(("none", String::new()));
        let source = id.as_str().rsplit_once('#').map_or(id.as_str(), |(s, _)| s).to_string();
        let kind = if kind == "column" && strokes.contains(&source) { "scol" } else { kind };
        if matches!(kind, "column" | "scol") {
            if let Some(n) = unders.get(id) {
                under.insert(x, *n);
            }
        }
        of.push(kind.to_string());
        keys.push(key);
    }
    Kinds { of, keys, under }
}

fn cat(kind: &str, top: bool) -> usize {
    let name = match (kind, top) {
        ("fill", true) => "fill_top",
        ("fill", false) => "fill_under",
        ("base", _) => "base",
        ("column", true) => "col_top",
        ("column", false) => "col_under",
        ("scol", true) => "scol_top",
        ("scol", false) => "scol_under",
        ("letter", _) => "letter",
        ("run", _) => "run",
        ("pfill" | "pcolumn" | "prun", _) => "person",
        (_, true) => "other_top",
        _ => "other_under",
    };
    CATS.iter().position(|c| *c == name).unwrap()
}

fn clipped(a: Point, b: Point, cx: f64, cy: f64) -> f64 {
    let (x0, y0, dx, dy) = (a.x(), a.y(), b.x() - a.x(), b.y() - a.y());
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, qq) in [(-dx, x0 - (cx - 0.5)), (dx, (cx + 0.5) - x0), (-dy, y0 - (cy - 0.5)), (dy, (cy + 0.5) - y0)] {
        if p == 0.0 {
            if qq < 0.0 {
                return 0.0;
            }
        } else {
            let r = qq / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    if t1 <= t0 { 0.0 } else { (t1 - t0) * (dx * dx + dy * dy).sqrt() }
}

#[derive(Default)]
struct Attribution {
    /// cause -> [spots >= cap, spots >= limit, sums per category at spots >= cap (CATS)..]
    causes: BTreeMap<String, Vec<f64>>,
    ratio: (f64, usize),
    /// element index -> (spots with its top >= SIG, of those with another element's fill top >= 0.5)
    on_fill: BTreeMap<usize, (usize, usize)>,
    /// element index -> worst need at a spot >= cap: (its top fills there, spot value)
    hot: BTreeMap<usize, Vec<(f64, f64)>>,
    /// fills under a column at a spot >= cap
    fills_under: BTreeSet<usize>,
    thickest: String,
}

fn attribute(plan: &StitchPlan, kinds: &Kinds, t: &Thickness) -> Attribution {
    let mut out = Attribution::default();
    let ul = under_entries(plan, kinds);
    let sewn: Vec<(Point, Point, usize, Option<usize>, bool)> = t
        .sewn()
        .iter()
        .map(|s| {
            let element = plan.blocks.get(s.block).and_then(|b| b.stitches.get(s.index)).and_then(|e| e.origin.element).map(|e| e.index());
            let kind = element.and_then(|e| kinds.of.get(e)).map_or("none", String::as_str);
            let c = if s.is_lock() {
                cat_named("lock")
            } else if s.role == Role::Travel {
                cat_named("travel")
            } else {
                cat(kind, s.role == Role::Top && !ul.contains(&(s.block, s.index)))
            };
            (t.point(s.from), t.point(s.to), c, element, s.role == Role::Top && !s.is_lock() && !ul.contains(&(s.block, s.index)))
        })
        .collect();
    let mut buckets: BTreeMap<(i64, i64), Vec<usize>> = BTreeMap::new();
    let (mut lo, mut hi) = ((f64::MAX, f64::MAX), (f64::MIN, f64::MIN));
    for (k, (a, b, ..)) in sewn.iter().enumerate() {
        let (x0, x1, y0, y1) = (a.x().min(b.x()), a.x().max(b.x()), a.y().min(b.y()), a.y().max(b.y()));
        lo = (lo.0.min(x0), lo.1.min(y0));
        hi = (hi.0.max(x1), hi.1.max(y1));
        for i in (x0.floor() as i64)..=(x1.floor() as i64) {
            for j in (y0.floor() as i64)..=(y1.floor() as i64) {
                buckets.entry((i, j)).or_default().push(k);
            }
        }
    }
    let most = t.most(Measure::Fills);
    let anchor = most.at;
    let i0 = ((lo.0 - 1.0 - anchor.x()) / 0.5).floor() as i64;
    let i1 = ((hi.0 + 1.0 - anchor.x()) / 0.5).ceil() as i64;
    let j0 = ((lo.1 - 1.0 - anchor.y()) / 0.5).floor() as i64;
    let j1 = ((hi.1 + 1.0 - anchor.y()) / 0.5).ceil() as i64;
    let mut stamp = vec![0u64; sewn.len()];
    let mut n: u64 = 0;
    for i in i0..=i1 {
        for j in j0..=j1 {
            let (cx, cy) = (anchor.x() + 0.5 * i as f64, anchor.y() + 0.5 * j as f64);
            let value = t.at(Measure::Fills, Point::new(cx, cy).unwrap());
            if value < 0.6 {
                continue;
            }
            n += 1;
            let mut cats = [0.0f64; CATS.len()];
            let mut tops: BTreeMap<usize, f64> = BTreeMap::new();
            for bi in [(cx - 0.5).floor() as i64, (cx + 0.5).floor() as i64] {
                for bj in [(cy - 0.5).floor() as i64, (cy + 0.5).floor() as i64] {
                    for &k in buckets.get(&(bi, bj)).map_or(&[][..], Vec::as_slice) {
                        if stamp[k] == n {
                            continue;
                        }
                        stamp[k] = n;
                        let (a, b, c, element, top) = sewn[k];
                        let f = clipped(a, b, cx, cy) * 0.25;
                        if f == 0.0 {
                            continue;
                        }
                        cats[c] += f;
                        if let (Some(e), true) = (element, top) {
                            *tops.entry(e).or_default() += f;
                        }
                    }
                }
            }
            let kind = |e: usize| kinds.of.get(e).map_or("none", String::as_str);
            let is_col = |e: usize| matches!(kind(e), "column" | "scol" | "letter" | "pcolumn");
            let is_fill = |e: usize| matches!(kind(e), "fill" | "pfill");
            // Columns on fills, over the whole design.
            for (&e, &f) in &tops {
                if is_col(e) && f >= SIG {
                    let under = tops.iter().any(|(&o, &g)| o != e && is_fill(o) && g >= 0.5);
                    let entry = out.on_fill.entry(e).or_default();
                    entry.0 += 1;
                    entry.1 += usize::from(under);
                }
            }
            if value < CAP {
                continue;
            }
            out.ratio.0 += cats.iter().sum::<f64>() / value;
            out.ratio.1 += 1;
            let ncol = tops.iter().filter(|(e, f)| is_col(**e) && **f >= SIG).count();
            let nfill = tops.iter().filter(|(e, f)| is_fill(**e) && **f >= SIG).count();
            let nrun = tops.iter().filter(|(e, f)| matches!(kind(**e), "run" | "prun") && **f >= SIG).count();
            let scol = tops.iter().any(|(e, f)| kind(*e) == "scol" && *f >= SIG);
            let letter = tops.iter().any(|(e, f)| kind(*e) == "letter" && *f >= SIG);
            let cause = match (ncol, nfill, nrun) {
                (0, 0, 0) => "other".to_string(),
                (0, _, 0) => format!("fills alone ({}+)", nfill.min(4)),
                (0, _, _) => "line over fill".to_string(),
                (1, 0, _) => "one column alone".to_string(),
                (1, _, _) => "one column over fill".to_string(),
                (_, 0, _) => "columns meeting, no fill".to_string(),
                (_, _, _) => "columns meeting over fill".to_string(),
            };
            let sources: BTreeSet<String> = tops
                .iter()
                .filter(|(e, f)| is_col(**e) && **f >= SIG)
                .filter_map(|(e, _)| plan.elements.get(*e))
                .map(|id| id.as_str().rsplit_once('#').map_or(id.as_str(), |(s, _)| s).to_string())
                .collect();
            let cause = if ncol >= 2 { format!("{cause}{}", if sources.len() == 1 { " (one shape)" } else { " (2+ shapes)" }) } else { cause };
            let cause = format!("{cause}{}{}", if scol { " [stroke col]" } else { "" }, if letter { " [letter]" } else { "" });
            let row = out.causes.entry(cause).or_insert_with(|| vec![0.0; 2 + CATS.len()]);
            row[0] += 1.0;
            row[1] += f64::from(u8::from(value >= LIMIT));
            for (k, c) in cats.iter().enumerate() {
                row[2 + k] += c;
            }
            for (&e, &f) in &tops {
                if is_col(e) && f >= SIG {
                    out.hot.entry(e).or_default().push((f, value));
                    if tops.iter().any(|(&o, &g)| is_fill(o) && g >= 0.5) {
                        for (&o, &g) in &tops {
                            if is_fill(o) && g >= SIG {
                                out.fills_under.insert(o);
                            }
                        }
                    }
                }
            }
            if (cx - most.at.x()).abs() < 1e-6 && (cy - most.at.y()).abs() < 1e-6 {
                let mut s = String::new();
                for (k, c) in cats.iter().enumerate() {
                    if *c >= 0.05 {
                        let _ = write!(s, "{}={:.2} ", CATS[k], c);
                    }
                }
                let _ = write!(s, "| elements: ");
                for (e, f) in &tops {
                    if *f >= 0.05 {
                        let _ = write!(s, "{}:{:.2} ", kind(*e), f);
                    }
                }
                out.thickest = s;
            }
        }
    }
    out
}

/// A plan from inside `in_place`, moved back as `in_place` moves its own.
fn moved(design: &Design, mut plan: StitchPlan) -> StitchPlan {
    if let Some(b) = design.bounds() {
        let (mx, my) = ((b.min().x() + b.max().x()) / 2.0, (b.min().y() + b.max().y()) / 2.0);
        for s in plan.blocks.iter_mut().flat_map(|b| &mut b.stitches) {
            s.at = Point::new(s.at.x() + mx, s.at.y() + my).unwrap();
        }
    }
    plan
}

fn cat_named(name: &str) -> usize {
    CATS.iter().position(|c| *c == name).unwrap()
}

struct Measured {
    line: String,
}

fn measures(design: &Design, plan: &StitchPlan, nocover: bool, light: bool) -> Measured {
    let mut meter = Budget::DEFAULT.meter();
    let mut line = String::new();
    let st = plan.stats();
    let _ = write!(line, "\"stitches\": {}, \"thread_changes\": {}, \"jumps\": {}", st.stitches, st.color_changes, st.jumps);
    if let Ok(Some(t)) = Thickness::of(plan, &mut meter) {
        let most = t.most(Measure::Fills);
        let cap = t.over(Measure::Fills, CAP, &mut meter).unwrap_or_default();
        let lim = t.over(Measure::Fills, LIMIT, &mut meter).unwrap_or_default();
        let _ = write!(line, ", \"thickest\": {:.3}, \"at\": [{:.1}, {:.1}], \"above_cap_mm2\": {:.2}, \"above_limit_mm2\": {:.2}", most.value, most.at.x(), most.at.y(), cap.area, lim.area);
    }
    if light {
        return Measured { line };
    }
    let t0 = Instant::now();
    let astray = fabric::astray(design, plan);
    let t1 = Instant::now();
    let likeness = fabric::likeness(design, plan);
    eprintln!("astray {:.1}s likeness {:.1}s", (t1 - t0).as_secs_f64(), t1.elapsed().as_secs_f64());
    let _ = write!(line, ", \"astray\": {}, \"jump_threads\": {}, \"detail_lost_percent\": {:.3}", astray.stitches.len(), astray.jumps.len(), (1.0 - likeness.likeness()) * 100.0);
    if !nocover {
        let c = fabric::cover(design, plan, fabric::GAP);
        let _ = write!(line, ", \"bare_mm2\": {:.3}, \"gaps\": {}", c.bare, c.gaps);
    }
    Measured { line }
}

fn json_causes(a: &Attribution) -> String {
    let mut s = String::from("{");
    for (k, (cause, row)) in a.causes.iter().enumerate() {
        if k > 0 {
            s.push_str(", ");
        }
        let _ = write!(s, "{}: [{}, {}", q(cause), row[0], row[1]);
        for v in &row[2..] {
            let _ = write!(s, ", {v:.3}");
        }
        s.push(']');
    }
    s.push('}');
    s
}

fn reset() {
    probe::SHORTENED.store(0, std::sync::atomic::Ordering::Relaxed);
    probe::RAIL_POINTS.store(0, std::sync::atomic::Ordering::Relaxed);
}

fn crowd() -> (usize, usize) {
    (probe::SHORTENED.load(std::sync::atomic::Ordering::Relaxed), probe::RAIL_POINTS.load(std::sync::atomic::Ordering::Relaxed))
}

fn clear() {
    probe::OVERRIDES.lock().unwrap().clear();
    probe::LINES.lock().unwrap().clear();
    probe::UNDERLAY.lock().unwrap().clear();
    *probe::NOCAP.lock().unwrap() = false;
    *probe::F.lock().unwrap() = None;
}

fn main() {
    let styles: Vec<String> = std::env::var("PROBE_STYLES").unwrap_or_else(|_| "default;logo".into()).split(';').map(str::to_string).collect();
    let cands: Vec<String> = std::env::var("PROBE_CANDS").unwrap_or_else(|_| "main".into()).split(';').map(str::to_string).collect();
    let png = std::env::var("PROBE_PNG").ok();
    let nocover = std::env::var("NOCOVER").is_ok();
    println!("{{\"cats\": {:?}}}", CATS);
    for file in std::env::args().skip(1) {
        let name = FsPath::new(&file).file_stem().unwrap().to_string_lossy().into_owned();
        let bytes = std::fs::read(&file).unwrap();
        let original = match stitchcraft_vectorcraft::read_with(&bytes, &Budget::DEFAULT, Reading { auto: true, ..Default::default() }) {
            Ok(read) => read.design,
            Err(e) => {
                println!("{{\"name\": {}, \"error\": {}}}", q(&name), q(&format!("{e:?}")));
                continue;
            }
        };
        for style in &styles {
            clear();
            let design = styled(&original, style, &[]);
            reset();
            let started = Instant::now();
            let outcome = in_place(&design);
            let secs = started.elapsed().as_secs_f64();
            let crowd_main = crowd();
            let Some(plan) = outcome.plan else {
                println!("{{\"name\": {}, \"style\": {}, \"cand\": \"main\", \"error\": \"no plan\"}}", q(&name), q(style));
                continue;
            };
            let kinds = kinds(&design, &plan);
            let pre = probe::PRE.lock().unwrap().take().map(|p| moved(&design, p));
            let mut meter = Budget::DEFAULT.meter();
            let t = Thickness::of(&plan, &mut meter).unwrap().unwrap();
            let t0 = Instant::now();
            let att = attribute(&plan, &kinds, &t);
            eprintln!("attribute {:.1}s", t0.elapsed().as_secs_f64());
            // Pass-1 sets for the candidates.
            let on_fill: BTreeSet<usize> = att.on_fill.iter().filter(|(_, (n, u))| *u * 2 >= *n && *n > 0).map(|(e, _)| *e).collect();
            let id = |e: usize| plan.elements.get(e).cloned();
            let kind = |e: usize| kinds.of.get(e).map_or("none", String::as_str);
            for cand in &cands {
                let mut line = format!("{{\"name\": {}, \"style\": {}, \"cand\": {}", q(&name), q(style), q(cand));
                let mut crowd_c = crowd_main;
                let (plan_c, design_c, secs_c) = if cand == "main" {
                    (Some(plan.clone()), design.clone(), secs)
                } else {
                    clear();
                    let mut extra: Vec<(&str, &str)> = Vec::new();
                    let mut ov: BTreeMap<ElementId, Vec<(String, String)>> = BTreeMap::new();
                    let mut skip = false;
                    match cand.as_str() {
                        "a04" => extra.push(("auto_column_overlap_mm", "0.4")),
                        "a02" => extra.push(("auto_column_overlap_mm", "0.2")),
                        "pc0" => extra.push(("auto_pull_compensation_mm", "0")),
                        "nocap" => *probe::NOCAP.lock().unwrap() = true,
                        "f04" => *probe::F.lock().unwrap() = Some(0.4),
                        "f0" => *probe::F.lock().unwrap() = Some(0.0),
                        "af" => {
                            extra.push(("auto_column_overlap_mm", "0.4"));
                            *probe::F.lock().unwrap() = Some(0.4);
                        }
                        "b" => {
                            for &e in &att.fills_under {
                                if let Some(id) = id(e) {
                                    ov.entry(id).or_default().push(("fill_underlay".into(), "false".into()));
                                }
                            }
                        }
                        "c" | "c0" => {
                            for &e in &on_fill {
                                if kind(e) == "letter" {
                                    continue;
                                }
                                if let Some(id) = id(e) {
                                    let set = ov.entry(id).or_default();
                                    set.push(("contour_underlay".into(), "false".into()));
                                    set.push(("zigzag_underlay".into(), "false".into()));
                                    set.push(("center_walk_underlay".into(), if cand == "c" { "true" } else { "false" }.into()));
                                }
                            }
                        }
                        "d" => {
                            for (&e, spots) in &att.hot {
                                if !on_fill.contains(&e) || kind(e) == "letter" {
                                    continue;
                                }
                                let need = spots.iter().map(|(f, v)| if *f <= v - CAP { 1.5 } else { f / (f - (v - CAP)) }).fold(1.0f64, f64::max).min(1.5);
                                if let Some(id) = id(e) {
                                    ov.entry(id).or_default().push(("zigzag_spacing_mm".into(), format!("{:.3}", 0.4 * need)));
                                }
                            }
                        }
                        "e" if style != "logo" => skip = true,
                        "e" => {
                            let mut lines = probe::LINES.lock().unwrap();
                            for &e in att.hot.keys() {
                                if on_fill.contains(&e) && kind(e) == "column" {
                                    if let Some(k) = kinds.keys.get(e) {
                                        lines.insert(k.clone());
                                    }
                                }
                            }
                            skip = lines.is_empty();
                        }
                        _ => skip = true,
                    }
                    let _ = write!(line, ", \"changed\": {}", ov.len() + probe::LINES.lock().unwrap().len());
                    if skip && cand != "e" {
                        println!("{line}, \"error\": \"unknown\"}}");
                        continue;
                    }
                    if skip || (ov.is_empty() && matches!(cand.as_str(), "b" | "c" | "c0" | "d")) {
                        println!("{line}, \"same\": true}}");
                        continue;
                    }
                    *probe::OVERRIDES.lock().unwrap() = ov;
                    let design_c = styled(&original, style, &extra);
                    reset();
                    let started = Instant::now();
                    let outcome = in_place(&design_c);
                    let secs = started.elapsed().as_secs_f64();
                    crowd_c = crowd();
                    let plan_c = if cand == "nocap" { probe::PRE.lock().unwrap().take().map(|p| moved(&design_c, p)).or(outcome.plan) } else { outcome.plan };
                    clear();
                    (plan_c, design_c, secs)
                };
                let _ = write!(line, ", \"secs\": {secs_c:.2}, \"shortened\": {}, \"rail_points\": {}", crowd_c.0, crowd_c.1);
                let Some(plan_c) = plan_c else {
                    println!("{line}, \"error\": \"no plan\"}}");
                    continue;
                };
                if cand != "main" && cand != "nocap" && plan_c == plan {
                    println!("{line}, \"same\": true, \"identical\": true}}");
                    continue;
                }
                let m = measures(&design_c, &plan_c, nocover, std::env::var("ALLLIGHT").is_ok() || (cand == "main" && std::env::var("MAINLIGHT").is_ok()));
                let _ = write!(line, ", {}", m.line);
                if cand == "main" {
                    let _ = write!(line, ", \"causes\": {}, \"ratio\": {:.3}, \"thickest_stack\": {}", json_causes(&att), att.ratio.0 / att.ratio.1.max(1) as f64, q(&att.thickest));
                    let _ = write!(line, ", \"columns_on_fill\": {}, \"columns\": {}", on_fill.len(), att.on_fill.len());
                    let mut roles: BTreeMap<String, usize> = BTreeMap::new();
                    for (bi, b) in plan.blocks.iter().enumerate() {
                        let _ = bi;
                        for e in &b.stitches {
                            let k = e.origin.element.and_then(|x| kinds.of.get(x.index())).map_or("none", String::as_str);
                            *roles.entry(format!("{k}/{:?}", e.origin.role)).or_default() += 1;
                        }
                    }
                    let _ = write!(line, ", \"roles\": {}", q(&format!("{roles:?}")));
                    if let Some(pre) = &pre {
                        let mut meter = Budget::DEFAULT.meter();
                        if let Ok(Some(tp)) = Thickness::of(pre, &mut meter) {
                            let a = attribute(pre, &kinds, &tp);
                            let mp = measures(&design, pre, true, true);
                            let _ = write!(line, ", \"pre\": {{{}, \"causes\": {}}}", mp.line, json_causes(&a));
                        }
                        // What the last pass and the cap took, needle points by category.
                        let count = |p: &StitchPlan| {
                            let ul = under_entries(p, &kinds);
                            let mut c = [0usize; CATS.len()];
                            let mut meter = Budget::DEFAULT.meter();
                            if let Ok(Some(tt)) = Thickness::of(p, &mut meter) {
                                for s in tt.sewn() {
                                    let element = p.blocks.get(s.block).and_then(|b| b.stitches.get(s.index)).and_then(|e| e.origin.element).map(|e| e.index());
                                    let k = element.and_then(|e| kinds.of.get(e)).map_or("none", String::as_str);
                                    let x = if s.is_lock() { cat_named("lock") } else if s.role == Role::Travel { cat_named("travel") } else { cat(k, s.role == Role::Top && !ul.contains(&(s.block, s.index))) };
                                    c[x] += 1;
                                }
                            }
                            c
                        };
                        let (cb, ca) = (count(pre), count(&plan));
                        let took: Vec<String> = (0..CATS.len()).filter(|&k| cb[k] != ca[k]).map(|k| format!("{}:{}", CATS[k], cb[k] as i64 - ca[k] as i64)).collect();
                        let _ = write!(line, ", \"took\": {}", q(&took.join(" ")));
                    }
                }
                if let Some(dir) = &png {
                    for (style_name, st) in [("real", Style::Realistic), ("thick", Style::Thickness)] {
                        if let Some(settings) = Settings::new(st, 8.0) {
                            if let Ok(image) = stitchcraft_render::preview(&plan_c, REFERENCE.trims, settings, &mut Budget::DEFAULT.meter()) {
                                std::fs::write(FsPath::new(dir).join(format!("{name}~{style}~{cand}~{style_name}.png")), image.png).unwrap();
                            }
                        }
                    }
                }
                println!("{line}}}");
            }
        }
    }
}
