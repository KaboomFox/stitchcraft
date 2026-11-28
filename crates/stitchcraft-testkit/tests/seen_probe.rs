//! line-seen PROBE (from M13.63's), never committed: one ROW (JSON) per design of the set, with its threads, passes and
//! measures. SC_P_ORDER (the engine's dump) is moved to M1363_OUT/{name}.order.jsonl after each design.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::print_stdout, missing_docs, clippy::disallowed_methods)]

use std::collections::BTreeMap;

use stitchcraft_core::{Budget, Mm, Point, Size};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Reading};
use stitchcraft_engine::testsheets::SHEETS;
use stitchcraft_testkit::designs::{in_place, rules};
use stitchcraft_testkit::fabric::{self, GAP, Limits, likeness};
use stitchcraft_testkit::pictures::{self, Trace};

fn read(path: &str) -> Design {
    let bytes = std::fs::read(path).unwrap();
    let reading = Reading { auto: true, ..Reading::default() };
    if path.ends_with(".svg") {
        stitchcraft_svg::read_with(&bytes, &Budget::DEFAULT, reading).unwrap().design
    } else {
        stitchcraft_vectorcraft::read_with(&bytes, &Budget::DEFAULT, reading).unwrap().design
    }
}

fn designs() -> Vec<(String, Design, bool)> {
    let mut designs: Vec<(String, Design, bool)> = Vec::new();
    if std::env::var("M1363_REPO").is_ok() {
        for picture in pictures::all()
            .into_iter()
            .chain([Trace::Abutting, Trace::Stacked].into_iter().flat_map(|trace| [pictures::emblem(trace), pictures::button(trace)]))
        {
            designs.push((picture.name, Design::new(picture.elements, DesignSettings::default()).unwrap(), false));
        }
        designs.push(("pear".to_string(), pictures::pear(), false));
        designs.push(("lettered-patch".to_string(), pictures::lettered_patch(), false));
        for sheet in SHEETS.iter().filter(|sheet| sheet.digitized()) {
            let design = sheet.design().unwrap().unwrap();
            designs.push((sheet.id.to_lowercase(), design.clone(), true));
            let shapes: Vec<Element> = design.elements().iter().filter(|element| element.automatic).cloned().collect();
            designs.push((format!("{}-auto", sheet.id.to_lowercase()), Design::new(shapes, design.settings).unwrap(), false));
        }
    }
    if std::env::var("M1363_DOGS").is_ok() {
        designs.extend(pictures::dogs().into_iter().map(|(name, design)| (name, design, false)));
    }
    if let Ok(set) = std::env::var("M1363_SET") {
        designs = designs
            .into_iter()
            .map(|(name, design, file)| {
                let mut elements = design.elements().to_vec();
                for e in elements.iter_mut().filter(|e| e.automatic) {
                    for kv in set.split(',') {
                        let (k, v) = kv.split_once('=').unwrap();
                        e.params.set(k, v);
                    }
                }
                let mut d = Design::new(elements, design.settings).unwrap();
                d.marks.clone_from(&design.marks);
                (name, d, file)
            })
            .collect();
    }
    if let Ok(files) = std::env::var("M1363_FILES") {
        for f in files.split(':').filter(|f| !f.is_empty()) {
            let mut name = std::path::Path::new(f).file_stem().unwrap().to_string_lossy().to_string();
            if f.contains("review-set-work") && name == "emblem" {
                name.push_str("-drawing");
            }
            designs.push((name, read(f), true));
        }
    }
    designs
}

/// The design planned in a hoop 2 m square, its origin at the middle of its shapes.
fn big(design: &Design) -> stitchcraft_engine::PlanOutcome {
    let profile = stitchcraft_plan::MachineProfile {
        hoop: Size::new(Mm::from_tenths(20000), Mm::from_tenths(20000)),
        comfort: None,
        ..stitchcraft_plan::profiles::REFERENCE.clone()
    };
    let b = design.bounds().unwrap();
    let middle = Point::new((b.min().x() + b.max().x()) / 2.0, (b.min().y() + b.max().y()) / 2.0).unwrap();
    let mut centred = Design::new(design.elements().to_vec(), DesignSettings { origin: Some(middle), ..design.settings }).unwrap();
    centred.marks.clone_from(&design.marks);
    centred.guides.clone_from(&design.guides);
    let mut outcome = stitchcraft_engine::plan(&centred, &profile, &Budget::DEFAULT);
    for stitch in outcome.plan.iter_mut().flat_map(|plan| &mut plan.blocks).flat_map(|block| &mut block.stitches) {
        stitch.at = Point::new(stitch.at.x() + middle.x(), stitch.at.y() + middle.y()).unwrap();
    }
    outcome
}

#[test]
fn probe() {
    let only = std::env::var("SC_P_ONLY").ok();
    let fast = std::env::var("M1363_FAST").is_ok();
    let out = std::env::var("M1363_OUT").unwrap_or_else(|_| "/tmp".to_string());
    let dump = std::env::var("SC_P_ORDER").ok();
    let rules = rules();
    for (name, design, file) in designs() {
        if only.as_ref().is_some_and(|only| !only.split(',').any(|o| o == name)) {
            continue;
        }
        if let Some(dump) = &dump {
            let _ = std::fs::remove_file(dump);
        }
        let pairs_file = std::env::var("SC_P_PAIRS").ok();
        if let Some(f) = &pairs_file {
            let _ = std::fs::remove_file(f);
        }
        eprintln!("M1363 {name}");
        let t0 = std::time::Instant::now();
        let outcome = if file { big(&design) } else { in_place(&design) };
        let ms = t0.elapsed().as_millis();
        if let Some(dump) = &dump {
            let _ = std::fs::rename(dump, format!("{out}/{name}.order.jsonl"));
        }
        let Some(plan) = outcome.plan else {
            println!("ROW {{\"name\":{name:?},\"error\":{:?}}}", outcome.diagnostics.iter().filter(|d| d.code.to_string().contains("-E")).map(|d| format!("{}: {}", d.code, d.message)).collect::<Vec<_>>().join(" | "));
            continue;
        };
        if std::env::var("M1363_PNG").is_ok_and(|names| names.split(',').any(|n| n == name || n == "all")) {
            let pts: Vec<Point> = plan.stitches().filter(|st| st.kind == stitchcraft_plan::StitchKind::Normal).map(|st| st.at).collect();
            let b = pts.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| [b[0].min(p.x()), b[1].min(p.y()), b[2].max(p.x()), b[3].max(p.y())]);
            println!("BOUNDS {name} {:.3} {:.3} {:.3} {:.3}", b[0], b[1], b[2], b[3]);
            for (style, tag) in [(stitchcraft_render::Style::Realistic, "real"), (stitchcraft_render::Style::Simple, "simple")] {
                let settings = stitchcraft_render::Settings::new(style, 12.0).unwrap();
                let mut meter = Budget::DEFAULT.meter();
                if let Ok(image) = stitchcraft_render::preview(&plan, stitchcraft_plan::TrimSupport::None, settings, &mut meter) {
                    std::fs::write(format!("{out}/{name}-{tag}.png"), &image.png).unwrap();
                }
            }
        }
        let colours: Vec<String> = plan.blocks.iter().map(|b| format!("{:02x}{:02x}{:02x}", b.thread.color.r, b.thread.color.g, b.thread.color.b)).collect();
        let mut passes: BTreeMap<&str, usize> = BTreeMap::new();
        for (at, c) in colours.iter().enumerate() {
            if at == 0 || colours.get(at - 1) != Some(c) {
                *passes.entry(c.as_str()).or_default() += 1;
            }
        }
        let stats = plan.stats();
        let columns = fabric::columns(&design);
        let (thickest, colfills) = fabric::thickest(&plan, &columns);
        let report = fabric::thickness(&plan, &Limits::DEFAULT);
        let cap = fabric::thickness(&plan, &Limits { warn_fills: 3.25, ..Limits::DEFAULT });
        let three = fabric::thickness(&plan, &Limits { warn_fills: 3.0, ..Limits::DEFAULT });
        let t_dig = std::time::Instant::now();
        let dig = if fast { Vec::new() } else { stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT).0.into_owned() };
        if std::env::var("SEEN_TIME").is_ok() { eprintln!("TIME {name} plan {:.1} digitized {:.1}", ms as f64 / 1000.0, t_dig.elapsed().as_secs_f64()); }
        let fill_pc = {
            dig.iter().filter(|e| matches!(e.shape, stitchcraft_engine::design::Shape::Fill { .. }) && e.params.get("pull_compensation_mm").is_some_and(|v| v.split_whitespace().any(|x| x.parse::<f64>().is_ok_and(|x| x != 0.0)))).count()
        };
        let note: String = outcome.diagnostics.iter().filter(|d| d.code.to_string() == "SC-I1119").map(|d| d.message.to_string()).collect();
        let codes: Vec<String> = outcome.diagnostics.iter().map(|d| d.code.to_string()).collect();
        let w0913 = codes.iter().filter(|c| c.as_str() == "SC-W0913").count();
        let i0907 = codes.iter().filter(|c| c.as_str() == "SC-I0907").count();
        let mut row = format!(
            "{{\"name\":{name:?},\"blocks\":{:?},\"passes\":{:?},\"changes\":{},\"jumps\":{},\"trims\":{},\"stitches\":{},\"thickest\":{thickest:.3},\"colfills\":{colfills:.3},\"over4\":{:.2},\"overcap\":{:.2},\"w0913\":{w0913},\"i0907\":{i0907},\"ms\":{ms}",
            colours,
            passes.iter().map(|(c, n)| format!("{c}:{n}")).collect::<Vec<_>>(),
            stats.color_changes,
            stats.jumps,
            stats.trims,
            stats.stitches,
            report.thick.area,
            cap.thick.area
        );
        row.push_str(&format!(",\"over3\":{:.2},\"fill_pc\":{fill_pc},\"note\":{note:?}", three.thick.area));
        let tick = std::time::Instant::now();
        let mark = |what: &str| if std::env::var("SEEN_TIME").is_ok() { eprintln!("TIME {name} {what} {:.1}", tick.elapsed().as_secs_f64()) };
        mark("plan+thickness");
        if !fast {
            let (shares, lost, seen) = lines(&design, &dig, &plan);
            mark("lines");
            let worst = shares.iter().map(|(_, share)| *share).fold(1.0_f64, f64::min);
            row.push_str(&format!(",\"lines\":{},\"lines_half\":{},\"lines_min\":{worst:.3}", shares.len(), lost.len()));
            // Lines sewn before a fill of another thread they border (the engine's dump), as the plan sews them.
            let first: BTreeMap<String, usize> = {
                let mut m = BTreeMap::new();
                for (b, block) in plan.blocks.iter().enumerate() {
                    for st in &block.stitches {
                        if let Some(id) = st.origin.element.and_then(|e| plan.elements.get(e.index())) {
                            m.entry(id.to_string()).or_insert(b);
                        }
                    }
                }
                m
            };
            let mut by_line: BTreeMap<String, (String, String, f64, f64, Vec<String>)> = BTreeMap::new();
            if let Some(text) = pairs_file.as_ref().and_then(|f| std::fs::read_to_string(f).ok()) {
                for l in text.lines() {
                    let v: Vec<&str> = l.split('\t').collect();
                    if v.len() < 8 { continue; }
                    let (lid, fid) = (v[1].to_string(), v[2].to_string());
                    let of = |id: &str| first.iter().filter(|(k, _)| k.as_str() == id || k.strip_prefix(id).is_some_and(|r| r.starts_with('#'))).map(|(_, b)| *b).min();
                    let before = matches!((first.get(&lid), of(&fid)), (Some(a), Some(b)) if *a < b);
                    if !before { continue; }
                    let e = by_line.entry(lid).or_insert((v[3].to_string(), v[4].to_string(), v[5].parse().unwrap_or(0.0), v[6].parse().unwrap_or(0.0), Vec::new()));
                    if !e.4.contains(&fid) { e.4.push(fid); }
                }
            }
            let mut pseen = Vec::new();
            let mut pcov = Vec::new();
            for (lid, (kind, satin, width, length, fills)) in &by_line {
                let Some((cells, shown, owners)) = seen.get(lid) else {
                    if std::env::var("SEEN_LINES").is_ok() { println!("LINE {name} {lid} {kind} satin={satin} w={width:.2} len={length:.1} seen=-1 covw=0 fills={}", fills.join(",")); }
                    continue
                };
                let share = *shown as f64 / (*cells).max(1) as f64;
                let covered: usize = owners.iter().filter(|(o, _)| fills.iter().any(|f| *o == f || o.strip_prefix(f.as_str()).is_some_and(|r| r.starts_with('#')))).map(|(_, n)| *n).sum();
                let cov_w = covered as f64 * 0.0025 / length.max(0.01);
                pseen.push(share);
                pcov.push(cov_w);
                if std::env::var("SEEN_LINES").is_ok() {
                    println!("LINE {name} {lid} {kind} satin={satin} w={width:.2} len={length:.1} seen={share:.3} covw={cov_w:.3} fills={} owners={:?}", fills.join(","), owners.iter().filter(|(_, n)| **n * 20 > *cells).collect::<Vec<_>>());
                }
            }
            pseen.sort_by(f64::total_cmp);
            let med = pseen.get(pseen.len() / 2).copied().unwrap_or(1.0);
            let half = pseen.iter().filter(|s| **s < 0.5).count();
            let mean_cov = if pcov.is_empty() { 0.0 } else { pcov.iter().sum::<f64>() / pcov.len() as f64 };
            row.push_str(&format!(",\"pairs\":{},\"pairs_half\":{half},\"pairs_med\":{med:.3},\"pairs_cov\":{mean_cov:.3}", by_line.len()));
            mark("pairs");
            let skip = std::env::var("SEEN_SKIP").is_ok();
            let astray = if skip { fabric::Astray { stitches: Vec::new(), longest: 0.0, at: None, moves: Vec::new(), jumps: Vec::new() } } else { fabric::astray(&design, &plan) };
            mark("astray");
            let cover = fabric::cover(&design, &plan, GAP);
            mark("cover");
            let slivers = cover.slivers(&rules);
            let kept = if skip { None } else { Some(likeness::likeness(&design, &plan)) };
            mark("likeness");
            let pulled = fabric::cover_with(&design, &plan, GAP, 0.4);
            mark("pulled");
            for (place, area, shape) in pulled.places.iter().filter(|p| p.1 >= 0.1) {
                println!("PGAP {name} {area:.3} ({:.1}, {:.1}) {shape}", place.center().x(), place.center().y());
            }
            row.push_str(&format!(",\"pgaps\":{},\"pgap_mm2\":{:.3},\"pbare\":{:.3}", pulled.gaps, pulled.in_gaps, pulled.bare));
            for fault in cover.faults(&rules) {
                println!("FAULT {name} {fault}");
            }
            row.push_str(&format!(
                ",\"astray\":{},\"moves\":{},\"jumpthreads\":{},\"slivers\":{},\"sliver_mm2\":{:.3},\"bare\":{:.6},\"faults\":{},\"likeness\":{:.3},\"gaps\":{},\"gap_mm2\":{:.3},\"bare_mm2\":{:.3}",
                astray.stitches.len(),
                astray.moves.len(),
                astray.jumps.len(),
                slivers.len(),
                slivers.iter().sum::<f64>(),
                cover.bare_share(),
                cover.faults(&rules).len(),
                kept.map_or(-1.0, |k| k.likeness()),
                cover.gaps,
                cover.in_gaps,
                cover.bare
            ));
            let astray_at: Vec<String> = astray
                .stitches
                .iter()
                .map(|[a, b]| {
                    let sewn = plan.sewn_stitches().into_iter().find(|s| s.from == *a && s.to == *b);
                    let el = sewn
                        .and_then(|s| plan.blocks.get(s.block)?.stitches.get(s.index)?.origin.element)
                        .and_then(|e| plan.element(e))
                        .map(ToString::to_string)
                        .unwrap_or_default();
                    format!("{el}@{:.1},{:.1}", a.x(), a.y())
                })
                .collect();
            row.push_str(&format!(",\"astray_at\":{astray_at:?}"));
        }
        if let Ok(ids) = std::env::var("M1363_BOX") {
            let (mut x0, mut y0) = (f64::INFINITY, f64::INFINITY);
            for st in plan.stitches() {
                x0 = x0.min(st.at.x());
                y0 = y0.min(st.at.y());
            }
            for id in ids.split(',') {
                let pts: Vec<_> = plan
                    .stitches()
                    .filter(|st| st.origin.element.and_then(|e| plan.elements.get(e.index())).is_some_and(|e| e.to_string() == id))
                    .map(|st| st.at)
                    .collect();
                let bx = pts.iter().fold([f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY], |b, p| [b[0].min(p.x()), b[1].min(p.y()), b[2].max(p.x()), b[3].max(p.y())]);
                println!("BOX {name} {id} min ({:.2},{:.2}) box {:.2} {:.2} {:.2} {:.2} n {}", x0, y0, bx[0], bx[1], bx[2], bx[3], pts.len());
            }
        }
        if let Ok(spec) = std::env::var("M1363_NEAR") {
            let (elements, _) = stitchcraft_engine::auto::digitized(&design, &Budget::DEFAULT);
            let kinds: BTreeMap<String, String> = elements
                .iter()
                .map(|e| (e.id.to_string(), format!("{:?}", stitchcraft_engine::generate::stitch_type_of(e, &design.settings))))
                .collect();
            for xy in spec.split(';') {
                let v: Vec<f64> = xy.split(',').map(|t| t.parse().unwrap()).collect();
                let at = Point::new(v[0], v[1]).unwrap();
                for (b, block) in plan.blocks.iter().enumerate() {
                    let mut last: Option<&stitchcraft_plan::Stitch> = None;
                    for (i, st) in block.stitches.iter().enumerate() {
                        if st.kind != stitchcraft_plan::StitchKind::Normal {
                            last = if st.kind == stitchcraft_plan::StitchKind::Jump { Some(st) } else { None };
                            continue;
                        }
                        if let Some(prev) = last {
                            let d = side(at, prev.at, st.at);
                            if d <= 0.7 {
                                let id = st.origin.element.and_then(|e| plan.elements.get(e.index())).map(|e| e.to_string()).unwrap_or_default();
                                println!(
                                    "NEAR {name} {xy} b{b} {:02x}{:02x}{:02x} i{i} {id} {} {:?} ({:.2},{:.2})->({:.2},{:.2}) len {:.2} d {:.2}",
                                    block.thread.color.r, block.thread.color.g, block.thread.color.b,
                                    kinds.get(&id).map(String::as_str).unwrap_or("-"), st.origin.role,
                                    prev.at.x(), prev.at.y(), st.at.x(), st.at.y(), prev.at.distance(st.at), d
                                );
                            }
                        }
                        last = Some(st);
                    }
                }
            }
        }
        if let Ok(spec) = std::env::var("M1363_AT") {
            let mut meter = Budget::DEFAULT.meter();
            let measured = stitchcraft_plan::thickness::Thickness::of(&plan, &mut meter).unwrap().unwrap();
            for xy in spec.split(';') {
                let v: Vec<f64> = xy.split(',').map(|t| t.parse().unwrap()).collect();
                let at = Point::new(v[0], v[1]).unwrap();
                let layers: Vec<String> = measured
                    .stack(&plan, at, &mut meter)
                    .unwrap()
                    .iter()
                    .map(|l| {
                        let id = l.element.and_then(|e| plan.elements.get(e.index())).map(|e| e.to_string()).unwrap_or_default();
                        format!("{}{}:{}:{:.2}+{:.2}", if columns.contains(id.as_str()) { "C:" } else { "" }, l.block, id, l.top, l.under)
                    })
                    .collect();
                println!("AT {name} {xy} {:.3} {}", measured.at(stitchcraft_plan::thickness::Measure::Fills, at), layers.join(" "));
            }
        }
        let layers: Vec<String> = report
            .stack
            .iter()
            .map(|l| {
                let id = l.element.and_then(|e| plan.elements.get(e.index())).map(|e| e.to_string()).unwrap_or_default();
                format!("{}{}:{}:{:.2}+{:.2}", if columns.contains(id.as_str()) { "C:" } else { "" }, l.block, id, l.top, l.under)
            })
            .collect();
        row.push_str(&format!(",\"stack_at\":[{:.2},{:.2}],\"stack\":{layers:?}", report.fills.at.x(), report.fills.at.y()));
        row.push('}');
        println!("ROW {row}");
    }
}

/// For each automatic element sewn as a running stitch, the share of its top stitches' thread (0.4 mm wide)
/// that no later top stitch of another element covers, on a grid of 0.05 mm; and those under 0.5.
#[allow(clippy::type_complexity)]
fn lines(design: &Design, elements: &[Element], plan: &stitchcraft_plan::StitchPlan) -> (Vec<(String, f64)>, Vec<(String, f64)>, BTreeMap<String, (usize, usize, BTreeMap<String, usize>)>) {
    use stitchcraft_params::StitchType;
    let running: std::collections::BTreeSet<String> = elements
        .iter()
        .filter(|e| stitchcraft_engine::generate::stitch_type_of(e, &design.settings) == Some(StitchType::RunningStitch))
        .map(|e| e.id.to_string())
        .collect();
    let fills: std::collections::BTreeSet<String> = elements
        .iter()
        .filter(|e| stitchcraft_engine::generate::stitch_type_of(e, &design.settings).is_none_or(|t| t.family() == stitchcraft_params::Family::Fill))
        .map(|e| e.id.to_string())
        .collect();
    let sewn = plan.sewn_stitches();
    let tops: Vec<(usize, Point, Point)> = sewn
        .iter()
        .filter(|s| s.role == stitchcraft_plan::Role::Top)
        .filter_map(|s| {
            let el = plan.blocks.get(s.block)?.stitches.get(s.index)?.origin.element?;
            Some((el.index(), s.from, s.to))
        })
        .collect();
    let (mut x0, mut y0, mut x1, mut y1) = (f64::MAX, f64::MAX, f64::MIN, f64::MIN);
    for (_, a, b) in &tops {
        for p in [a, b] {
            x0 = x0.min(p.x()); y0 = y0.min(p.y()); x1 = x1.max(p.x()); y1 = y1.max(p.y());
        }
    }
    if tops.is_empty() { return (Vec::new(), Vec::new(), BTreeMap::new()) }
    let step = 0.05; let half = 0.2;
    let (cols, rows) = (((x1 - x0 + 1.0) / step) as usize + 1, ((y1 - y0 + 1.0) / step) as usize + 1);
    let mut top = vec![usize::MAX; cols * rows];
    let mut own: std::collections::BTreeMap<usize, Vec<usize>> = Default::default();
    let is_line = |el: usize| plan.elements.get(el).is_some_and(|id| !fills.contains(id.as_str()) && !id.as_str().starts_with("stitchcraft:base"));
    for &(el, a, b) in &tops {
        let (ax, ay, bx, by) = (a.x(), a.y(), b.x(), b.y());
        let (lo_x, hi_x) = (ax.min(bx) - half, ax.max(bx) + half);
        let (lo_y, hi_y) = (ay.min(by) - half, ay.max(by) + half);
        let (dx, dy) = (bx - ax, by - ay); let len2 = dx * dx + dy * dy;
        let c0 = ((lo_x - x0 + 0.5) / step).max(0.0) as usize; let c1 = (((hi_x - x0 + 0.5) / step) as usize).min(cols - 1);
        let r0 = ((lo_y - y0 + 0.5) / step).max(0.0) as usize; let r1 = (((hi_y - y0 + 0.5) / step) as usize).min(rows - 1);
        let line = is_line(el);
        for r in r0..=r1 {
            for c in c0..=c1 {
                let (px, py) = (x0 - 0.5 + c as f64 * step, y0 - 0.5 + r as f64 * step);
                let t = if len2 > 0.0 { (((px - ax) * dx + (py - ay) * dy) / len2).clamp(0.0, 1.0) } else { 0.0 };
                let (qx, qy) = (ax + t * dx - px, ay + t * dy - py);
                if qx * qx + qy * qy <= half * half {
                    let cell = r * cols + c;
                    top[cell] = el;
                    if line { own.entry(el).or_default().push(cell); }
                }
            }
        }
    }
    let mut shares = Vec::new(); let mut lost = Vec::new(); let mut seen = BTreeMap::new();
    for (el, cells) in own {
        let cells: std::collections::BTreeSet<usize> = cells.into_iter().collect();
        let shown = cells.iter().filter(|&&c| top[c] == el).count();
        let share = shown as f64 / cells.len().max(1) as f64;
        let id = plan.elements.get(el).map(ToString::to_string).unwrap_or_default();
        let mut owners: BTreeMap<String, usize> = BTreeMap::new();
        for &c in &cells {
            if top[c] != el {
                *owners.entry(plan.elements.get(top[c]).map(ToString::to_string).unwrap_or_default()).or_default() += 1;
            }
        }
        if running.contains(&id) || std::env::var("SEEN_DEBUG").is_ok() {
            if share < 0.5 { lost.push((id.clone(), (share * 1000.0).round() / 1000.0)); }
            shares.push((id.clone(), share));
        }
        seen.insert(id, (cells.len(), shown, owners));
    }
    (shares, lost, seen)
}

/// How far `p` lies from the segment `a`-`b`.
fn side(p: Point, a: Point, b: Point) -> f64 {
    let (dx, dy) = (b.x() - a.x(), b.y() - a.y());
    let l = dx * dx + dy * dy;
    let t = if l > 0.0 { (((p.x() - a.x()) * dx + (p.y() - a.y()) * dy) / l).clamp(0.0, 1.0) } else { 0.0 };
    ((a.x() + t * dx - p.x()).powi(2) + (a.y() + t * dy - p.y()).powi(2)).sqrt()
}
