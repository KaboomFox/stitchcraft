//! The standard pictures of the before and after sheet, planned and measured by this revision
//! (`docs/src/design/before-after-sheet.md`). `sheet DIR` writes a picture of what each one sews
//! (`DIR/NAME.png`) and the numbers of all of them (`DIR/numbers.json`). `web/sheet.py` runs it, built in
//! release, for a pull request and for main, and puts the 2 results side by side.
//!
//! Why it is an example of the testkit: the sheet's numbers must be the ones the cases assert
//! (`tests/auto_photos.rs`, `tests/auto_fabric.rs`), so they come from the same functions, [`fabric::astray`],
//! [`fabric::cover`], [`fabric::thickness`] and the plan's own counts, and no script measures a plan a second
//! way.
//!
//! The pictures are the traced photos the cases measure ([`pictures::dogs`], [`pictures::pear`]), a lettered
//! patch drawn here ([`patch`]), and the digitized half of every test sheet that has one. A picture that
//! does not plan, or whose measuring fails, is written with the reason in the place of its numbers: the
//! sheet shows it, and the other pictures are still measured.

// A binary prints what it did, which library code never may (clippy.toml).
#![allow(clippy::print_stdout, clippy::print_stderr)]

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;
use std::process::ExitCode;

use stitchcraft_core::Budget;
use stitchcraft_engine::design::{Design, DesignSettings, Element};
use stitchcraft_engine::testsheets::{self, SHEETS, TestSheet};
use stitchcraft_plan::profiles::REFERENCE;
use stitchcraft_plan::{Rgb, StitchPlan, Thread};
use stitchcraft_render::{Settings, Style};
use stitchcraft_testkit::designs::{automatic, fill, in_place, rules};
use stitchcraft_testkit::fabric::{self, GAP, Limits};
use stitchcraft_testkit::pictures;

/// The pictures' scale, in pixels per millimetre: a design 100 mm tall is 500 pixels tall, which shows its
/// stitches and keeps a preview's 2 sets of pictures to a few megabytes.
const SCALE: f32 = 5.0;

/// What the sheet's table shows of a picture, in its order: the key in `numbers.json`, the column's heading,
/// and whether more of it is worse. More stitches are neither better nor worse.
const MEASURES: [(&str, &str, bool); 10] = [
    ("stitches", "Stitches", false),
    ("thread_changes", "Thread changes", true),
    ("jumps", "Jumps", true),
    ("jump_threads", "Jump threads left to cut", true),
    ("astray", "Stitches astray", true),
    ("slivers", "Slivers", true),
    ("bare_percent", "Bare, % of the design", true),
    ("thickest_fills", "Thickest spot, fills", true),
    ("thick_mm2", "At 3 fills or more, mm²", true),
    ("needle_points", "Most needle points per mm²", true),
];

/// What `fault` says, for a picture's reason.
fn said(fault: &impl std::fmt::Debug) -> String {
    format!("{fault:?}")
}

/// What `work` gives, or what it said when it panicked: the testkit's functions panic as tests do.
fn caught<T>(work: impl FnOnce() -> T) -> Result<T, String> {
    catch_unwind(AssertUnwindSafe(work)).map_err(|payload| {
        let text = payload.downcast_ref::<String>().cloned().or_else(|| payload.downcast_ref::<&str>().map(ToString::to_string));
        text.unwrap_or_else(|| "it panicked without a message".to_string())
    })
}

/// A lettered patch drawn in code: the word MATCH in block letters 10 mm tall, each a shape of its own
/// (`testsheets::pictures::word`), on a cream field with a red border, both with cut corners. Every shape is
/// automatic, and the field and the border lie whole under what is drawn on them, as a patch drawn in an
/// editor has them.
fn patch() -> Result<Design, String> {
    let word = testsheets::pictures::word((0.0, 0.0), 10.0).map_err(|fault| said(&fault))?;
    let letters = Design::new(word.elements.clone(), DesignSettings::default()).map_err(|fault| said(&fault))?;
    let bounds = letters.bounds().ok_or("the word has no shapes")?;
    let plate = |id: &str, margin: f64, thread: &Thread| {
        let (left, top) = (bounds.min().x() - margin, bounds.min().y() - margin);
        let (right, bottom) = (bounds.max().x() + margin, bounds.max().y() + margin);
        let cut = margin / 2.0;
        let ring = [
            (left + cut, top),
            (right - cut, top),
            (right, top + cut),
            (right, bottom - cut),
            (right - cut, bottom),
            (left + cut, bottom),
            (left, bottom - cut),
            (left, top + cut),
        ];
        automatic(fill(id, &[&ring], thread, &[]))
    };
    let border = plate("patch-border", 9.0, &Thread::new(Rgb::new(200, 0, 0)));
    let field = plate("patch-field", 6.0, &Thread::new(Rgb::new(236, 226, 198)));
    let shapes: Vec<Element> = [border, field].into_iter().chain(word.elements).collect();
    Design::new(shapes, DesignSettings::default()).map_err(|fault| said(&fault))
}

/// The digitized half of `sheet`: its automatic shapes alone, in their paint order, with its settings.
fn digitized(sheet: &TestSheet) -> Result<Design, String> {
    let design = sheet.design().ok_or("the sheet is not drawn as a design")?.map_err(|fault| said(&fault))?;
    let shapes: Vec<Element> = design.elements().iter().filter(|element| element.automatic).cloned().collect();
    Design::new(shapes, design.settings).map_err(|fault| said(&fault))
}

/// The standard pictures by name, each as a design or the reason there is none.
fn standard() -> Vec<(String, Result<Design, String>)> {
    let mut all: Vec<(String, Result<Design, String>)> = Vec::new();
    match caught(pictures::dogs) {
        Ok(dogs) => all.extend(dogs.into_iter().map(|(name, design)| (name, Ok(design)))),
        Err(why) => all.push(("dogs".to_string(), Err(why))),
    }
    all.push(("pear".to_string(), caught(pictures::pear)));
    all.push(("lettered-patch".to_string(), patch()));
    for sheet in SHEETS.iter().filter(|sheet| sheet.digitized()) {
        let name = format!("{}-digitized", sheet.id.to_lowercase());
        all.push((name, digitized(sheet)));
    }
    all
}

/// The plan of `design` with its stitches where it draws its shapes, or what was said of a design that
/// does not plan.
fn planned(design: &Design) -> Result<StitchPlan, String> {
    let outcome = caught(|| in_place(design))?;
    outcome.plan.ok_or_else(|| format!("it does not plan: {}", said(&outcome.diagnostics)))
}

/// The numbers of `plan`, by the keys of [`MEASURES`] and in their order, as the cases measure them.
fn measured(design: &Design, plan: &StitchPlan) -> [f64; 10] {
    let stats = plan.stats();
    let astray = fabric::astray(design, plan);
    let cover = fabric::cover(design, plan, GAP);
    let thickness = fabric::thickness(plan, &Limits::DEFAULT);
    let count = |count: usize| count as f64;
    [
        count(stats.stitches),
        count(stats.color_changes),
        count(stats.jumps),
        count(astray.jumps.len()),
        count(astray.stitches.len()),
        count(cover.slivers(&rules()).len()),
        cover.bare_share() * 100.0,
        thickness.fills.value,
        thickness.thick.area,
        thickness.points.value,
    ]
}

/// The picture of what `plan` sews, as a PNG file.
fn drawn(plan: &StitchPlan) -> Result<Vec<u8>, String> {
    let settings = Settings::new(Style::Realistic, SCALE).ok_or("the pictures' scale is out of range")?;
    let image = stitchcraft_render::preview(plan, REFERENCE.trims, settings, &mut Budget::DEFAULT.meter()).map_err(|fault| said(&fault))?;
    Ok(image.png)
}

/// `text` as a JSON string.
fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for letter in text.chars() {
        match letter {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            letter if u32::from(letter) < 0x20 => out.push(' '),
            letter => out.push(letter),
        }
    }
    out.push('"');
    out
}

/// A picture's fields in `numbers.json`: its picture's file, written to `folder`, and its numbers.
fn fields(name: &str, design: &Design, folder: &Path) -> Result<String, String> {
    let plan = planned(design)?;
    let numbers = caught(|| measured(design, &plan))?;
    let file = format!("{name}.png");
    write(&folder.join(&file), &drawn(&plan)?)?;
    let mut fields = format!("\"picture\": {}", quoted(&file));
    for ((key, _, _), number) in MEASURES.iter().zip(numbers) {
        // Counts are whole, and the share is kept to a thousandth of a percent, as the cases record it.
        let number = if number.is_finite() { ((number * 1000.0).round() / 1000.0).to_string() } else { "null".to_string() };
        fields.push_str(&format!(", {}: {number}", quoted(key)));
    }
    Ok(fields)
}

/// One picture's entry in `numbers.json`: its fields, or the reason it has none.
fn entry(name: &str, design: Result<Design, String>, folder: &Path) -> String {
    match design.and_then(|design| fields(name, &design, folder)) {
        Ok(fields) => format!("  {{\"name\": {}, {fields}}}", quoted(name)),
        Err(why) => format!("  {{\"name\": {}, \"error\": {}}}", quoted(name), quoted(&why)),
    }
}

/// Writes `bytes` to `path`.
// An example program writes its results.
#[allow(clippy::disallowed_methods)]
fn write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    std::fs::write(path, bytes).map_err(|fault| format!("{}: {fault}", path.display()))
}

// An example program reads its arguments and makes its folder.
#[allow(clippy::disallowed_methods)]
fn main() -> ExitCode {
    let Some(folder) = std::env::args().nth(1) else {
        eprintln!("usage: sheet DIR, the folder for the pictures and numbers.json");
        return ExitCode::FAILURE;
    };
    let folder = Path::new(&folder);
    if let Err(fault) = std::fs::create_dir_all(folder) {
        eprintln!("{}: {fault}", folder.display());
        return ExitCode::FAILURE;
    }
    // A panic's own message would go to the log once for each picture that fails: the entry says it.
    std::panic::set_hook(Box::new(|_| {}));
    let measures: Vec<String> = MEASURES
        .iter()
        .map(|(key, label, worse)| format!("  {{\"key\": {}, \"label\": {}, \"more_is_worse\": {worse}}}", quoted(key), quoted(label)))
        .collect();
    let mut entries = Vec::new();
    for (name, design) in standard() {
        let line = entry(&name, design, folder);
        println!("{}", line.trim_start());
        entries.push(line);
    }
    let json = format!("{{\n \"measures\": [\n{}\n ],\n \"pictures\": [\n{}\n ]\n}}\n", measures.join(",\n"), entries.join(",\n"));
    match write(&folder.join("numbers.json"), json.as_bytes()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(fault) => {
            eprintln!("{fault}");
            ExitCode::FAILURE
        }
    }
}
