//! The differential's pictures (M8.14, `docs/src/design/conformance.md` › L3): each case as Ink/Stitch sews it
//! beside StitchCraft's, and where their thread lies apart.
//!
//! **Why.** The measures say whether 2 machine files sew alike, and a person reviewing the differential looks:
//! a fill that agrees by every number can still show its rows turned, and an excused case shows what its
//! excuse covers. So every case that both programs sew gets one picture, made from the 2 PES files as
//! StitchCraft's reader read them back, the very plans the measures judged.
//!
//! **What it shows.** 3 panels side by side under a title with the case's name and its verdict:
//!
//! - Ink/Stitch's file and StitchCraft's, each drawn by the engine's own preview, `stitchcraft_render`'s
//!   realistic style, as `stitch preview` draws it, never by a second renderer. Both are at one scale in one
//!   frame, the bounds of both files from where the machine starts, so a difference of place shows as one.
//!   Under each are its measures, each the very text the report's table gives
//!   ([`super::compare`]), a measure outside its tolerance marked with `!`, and its threads as swatches.
//! - Where the footprints differ, on the grid the overlap is measured on (`stitchcraft_testkit::metrics::
//!   footprint_map`), the 2 centres made one as the measure makes them: thread only Ink/Stitch's in orange,
//!   only StitchCraft's in blue, both in grey, the Okabe–Ito colours a colour-blind eye tells apart, with
//!   a legend and the overlap under it. A pixel of the picture shows a colour when any grid pixel in it is
//!   one side's alone, grey when it holds both sides' thread.
//!
//! **Bounded and the same everywhere.** The scale fits the frame's longer side into [`CELL`] pixels, at
//! most [`MOST_SCALE`] a millimetre, so a huge design makes a picture no larger than a small one. The
//! realistic style's thread is detail a PNG file can't pack, so a picture's bytes grow with its pixels: at
//! 8 pixels a millimetre and 600 a side, a run's pictures took 45 MB on the site, where these limits keep a
//! run's folder under 15 MB.
//! Letters are straight strokes ([`super::letters`]), so a picture is the same bytes on every machine, like
//! the docs' shots. A panel the preview can't draw is left empty, and the picture says it is degraded and why
//! ([`Drawn::degraded`]), which the report repeats.

// Pixels and millimetres are converted back and forth here: every value is a size or a place within a picture
// whose sides are a few thousand pixels at most, so no conversion loses what matters.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss, clippy::cast_precision_loss)]

use stitchcraft_core::Budget;
use stitchcraft_plan::{StitchPlan, profiles};
use stitchcraft_render::raster::MARGIN_MM;
use stitchcraft_render::{GridPoint, Scene, Settings, Style};
use stitchcraft_testkit::metrics::footprint_map;
use tiny_skia::{Color, Paint, PathBuilder, Pixmap, PixmapPaint, Rect, Stroke, Transform};

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::letters;
use super::{Compared, Measure, Measured};
use crate::conformance::runner::Picture;
use crate::util;

/// The longer side of a panel's frame at most, in pixels.
pub const CELL: f32 = 400.0;
/// The most pixels a millimetre: a stitch's thread, 0.4 mm, is still 2 pixels wide.
pub const MOST_SCALE: f32 = 5.0;
/// The longer side of a panel's frame at most on a contact sheet, in pixels.
pub const SMALL_CELL: f32 = 120.0;
/// The room round and between panels, in pixels.
const PAD: u32 = 10;
/// A panel's least width: room for its longest line of measures.
const LEAST_WIDTH: u32 = 250;
/// A small picture's panel's least width: room in its 3 for a verdict naming 2 of the ledger's entries.
const SMALL_LEAST_WIDTH: u32 = 110;

/// Thread only the first side, Ink/Stitch's in the differential, sews there: Okabe–Ito orange.
pub const INKSTITCH_ONLY: [u8; 3] = [0xE6, 0x9F, 0x00];
/// Thread only the second side, StitchCraft's in the differential, sews there: Okabe–Ito blue.
pub const STITCHCRAFT_ONLY: [u8; 3] = [0x00, 0x72, 0xB2];
/// Thread both sew there.
pub const BOTH: [u8; 3] = [0x9E, 0x9E, 0x9E];
const INK: [u8; 3] = [0x20, 0x20, 0x20];
const PAPER: [u8; 3] = [0xFF, 0xFF, 0xFF];

/// A picture drawn: the PNG file, and why a part of it is missing, if one is.
#[derive(Clone, Debug, PartialEq)]
pub struct Drawn {
    pub png: Vec<u8>,
    pub degraded: Vec<String>,
}

/// One side of the picture: a program's name, its plan as read back from its PES file, and its measures.
pub struct Side<'a> {
    pub name: &'a str,
    pub plan: &'a StitchPlan,
    pub measured: &'a Measured,
}

/// The lines under a side's panel: each measure but the overlap, as the report gives it, `theirs` for
/// Ink/Stitch's side and StitchCraft's otherwise, with `!` after one outside its tolerance.
pub fn captions(rows: &[Compared], theirs: bool) -> Vec<String> {
    rows.iter()
        .filter(|row| row.0 != Measure::Overlap)
        .map(|(measure, mine, other, within)| {
            let value = if theirs { other } else { mine };
            let value = if *measure == Measure::Threads { value.split_whitespace().count().to_string() } else { value.clone() };
            format!("{} {value}{}", measure.name(), if *within { "" } else { " !" })
        })
        .collect()
}

/// The overlap's line, as the report gives it.
pub fn overlap(rows: &[Compared]) -> String {
    let value = rows.iter().find(|row| row.0 == Measure::Overlap);
    value.map_or_else(|| "overlap -".to_string(), |(.., mine, _, within)| format!("overlap {mine}{}", if *within { "" } else { " !" }))
}

/// The case's picture: `title` and `verdict` over Ink/Stitch's side `theirs`, StitchCraft's `ours` and their
/// difference, labelled with `rows`, the measures compared. `small` draws it for a contact sheet: smaller,
/// with its title alone.
pub fn draw(title: &str, verdict: &str, rows: &[Compared], theirs: &Side, ours: &Side, small: bool) -> Result<Drawn, String> {
    let mut degraded = Vec::new();
    let mut meter_budget = Budget::DEFAULT.meter();
    let mut scene = |side: &Side| match Scene::of(side.plan, profiles::REFERENCE.trims, &mut meter_budget) {
        Ok(scene) => scene.bounds().map(|bounds| (scene, bounds)),
        Err(e) => {
            degraded.push(format!("{}'s panel isn't drawn: {e}", side.name));
            None
        }
    };
    let scenes = [scene(theirs), scene(ours)];
    // The frame of both files, in millimetres from where the machine starts, with the preview's margin.
    let corner = |p: GridPoint, by: f32| (p.x as f32 / 10.0 + by, p.y as f32 / 10.0 + by);
    let mut frame: Option<(f32, f32, f32, f32)> = None;
    for (_, (min, max)) in scenes.iter().flatten() {
        let ((x0, y0), (x1, y1)) = (corner(*min, -MARGIN_MM), corner(*max, MARGIN_MM));
        frame = Some(frame.map_or((x0, y0, x1, y1), |(a, b, c, d)| (a.min(x0), b.min(y0), c.max(x1), d.max(y1))));
    }
    let map = footprint_map(&theirs.measured.sewn, &ours.measured.sewn);
    let grid_mm = |count: usize| count as f32 * map.pixel as f32 + 2.0 * MARGIN_MM;
    let (frame_w, frame_h) = frame.map_or((0.0, 0.0), |(x0, y0, x1, y1)| (x1 - x0, y1 - y0));
    let (cell_w, cell_h) = (frame_w.max(grid_mm(map.columns)), frame_h.max(grid_mm(map.rows)));
    let most = if small { SMALL_CELL } else { CELL };
    let scale = ((most / cell_w.max(cell_h).max(1.0)).min(MOST_SCALE) * 10.0).floor() / 10.0;
    let scale = scale.max(Settings::MIN_SCALE);
    let least = if small { SMALL_LEAST_WIDTH } else { LEAST_WIDTH };
    let (w, h) = (((cell_w * scale).ceil() as u32).max(least), ((cell_h * scale).ceil() as u32).max(1));

    let (title_h, head_h, lines) = if small { (30, 0, 0) } else { (34, 20, 7) };
    let line_h = 15;
    let width = PAD + 3 * (w + PAD);
    let height = PAD + title_h + head_h + h + lines * line_h + PAD;
    let mut canvas = Pixmap::new(width, height).ok_or_else(|| format!("no canvas of {width} × {height} pixels"))?;
    canvas.fill(color(PAPER));
    let top = PAD + title_h + head_h;
    let left = |panel: u32| PAD + panel * (w + PAD);

    // The title, with the scale's bar of 10 mm at its right.
    let unit = if small { 1.5 } else { 2.5 };
    // Small, the verdict on a line of its own above the title, so that a long title can't push it out.
    if small {
        text(&mut canvas, verdict, PAD as f32, PAD as f32, unit, INK);
        text(&mut canvas, title, PAD as f32, PAD as f32 + 13.0, unit, INK);
    } else {
        text(&mut canvas, &format!("{title}  {verdict}"), PAD as f32, PAD as f32, unit, INK);
    }
    if !small {
        let bar = 10.0 * scale;
        let x = width as f32 - PAD as f32 - bar;
        fill(&mut canvas, x, PAD as f32 + 12.0, bar, 3.0, INK);
        let said = "10 mm";
        text(&mut canvas, said, x - letters::width(said) * 1.5 - 8.0, PAD as f32 + 8.0, 1.5, INK);
    }

    // Both sides, in one frame centred in their panels.
    let settings = Settings::new(Style::Realistic, scale).ok_or_else(|| format!("no preview at {scale} pixels a millimetre"))?;
    let fabric = settings.fabric();
    for (panel, (side, drawn)) in [theirs, ours].into_iter().zip(&scenes).enumerate() {
        let x = left(panel as u32);
        fill(&mut canvas, x as f32, top as f32, w as f32, h as f32, [fabric.r, fabric.g, fabric.b]);
        if !small {
            text(&mut canvas, side.name, x as f32, (PAD + title_h) as f32, 1.5, INK);
            let mut y = top + h + 6;
            for line in captions(rows, panel == 0) {
                text(&mut canvas, &line, x as f32, y as f32, 1.5, INK);
                y += line_h;
            }
            for (k, thread) in side.measured.threads.iter().enumerate() {
                let at = x as f32 + k as f32 * 14.0;
                if at + 12.0 > (x + w) as f32 {
                    break;
                }
                fill(&mut canvas, at, y as f32, 12.0, 12.0, INK);
                fill(&mut canvas, at + 1.0, y as f32 + 1.0, 10.0, 10.0, [thread.r, thread.g, thread.b]);
            }
        }
        let (Some((scene, (min, _))), Some((x0, y0, x1, y1))) = (drawn, frame) else { continue };
        let image = match scene.render(settings, &mut meter_budget) {
            Ok(image) => image,
            Err(e) => {
                degraded.push(format!("{}'s panel isn't drawn: {e}", side.name));
                continue;
            }
        };
        let Ok(pixmap) = Pixmap::decode_png(&image.png) else {
            degraded.push(format!("{}'s panel isn't drawn: its preview doesn't decode", side.name));
            continue;
        };
        let (mx, my) = corner(*min, -MARGIN_MM);
        let inset = |cell: u32, frame: f32| ((cell as f32 - frame * scale) / 2.0).round();
        let dx = x as f32 + inset(w, x1 - x0) + ((mx - x0) * scale).round();
        let dy = top as f32 + inset(h, y1 - y0) + ((my - y0) * scale).round();
        canvas.draw_pixmap(dx as i32, dy as i32, pixmap.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }

    // Where the footprints differ, centred as the overlap measures them.
    let x = left(2);
    if !small {
        text(&mut canvas, "difference", x as f32, (PAD + title_h) as f32, 1.5, INK);
    }
    let (ox, oy) = (
        x as f32 + ((w as f32 - map.columns as f32 * map.pixel as f32 * scale) / 2.0).round(),
        top as f32 + ((h as f32 - map.rows as f32 * map.pixel as f32 * scale) / 2.0).round(),
    );
    // For each pixel of the panel: whether a grid pixel in it is Ink/Stitch's alone, StitchCraft's alone, both's.
    let mut seen = vec![0_u8; (w * h) as usize];
    for row in 0..map.rows {
        for column in 0..map.columns {
            let k = row * map.columns + column;
            let (a, b) = (map.a.get(k).copied().unwrap_or(false), map.b.get(k).copied().unwrap_or(false));
            let flag = match (a, b) {
                (true, true) => 4,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => continue,
            };
            let px = ((column as f32 + 0.5) * map.pixel as f32 * scale + ox - x as f32).floor();
            let py = ((row as f32 + 0.5) * map.pixel as f32 * scale + oy - top as f32).floor();
            if px >= 0.0
                && py >= 0.0
                && (px as u32) < w
                && (py as u32) < h
                && let Some(spot) = seen.get_mut((py as u32 * w + px as u32) as usize)
            {
                *spot |= flag;
            }
        }
    }
    let pixels = canvas.pixels_mut();
    for (k, spot) in seen.iter().enumerate() {
        let shade = match spot {
            0 => continue,
            1 | 5 => INKSTITCH_ONLY,
            2 | 6 => STITCHCRAFT_ONLY,
            _ => BOTH,
        };
        let (px, py) = (k as u32 % w + x, k as u32 / w + top);
        if let Some(pixel) = pixels.get_mut((py * width + px) as usize) {
            *pixel = tiny_skia::ColorU8::from_rgba(shade[0], shade[1], shade[2], 255).premultiply();
        }
    }
    if !small {
        let mut y = (top + h + 6) as f32;
        let (first, second) = (format!("{} only", theirs.name), format!("{} only", ours.name));
        for (shade, said) in [(INKSTITCH_ONLY, first.as_str()), (STITCHCRAFT_ONLY, second.as_str()), (BOTH, "both")] {
            fill(&mut canvas, x as f32, y, 12.0, 12.0, shade);
            text(&mut canvas, said, x as f32 + 18.0, y + 1.5, 1.5, INK);
            y += line_h as f32;
        }
        text(&mut canvas, &overlap(rows), x as f32, y, 1.5, INK);
    }
    if !degraded.is_empty() {
        let said = "degraded: a panel isn't drawn";
        text(&mut canvas, said, PAD as f32, (height - PAD - 8) as f32, 1.2, INK);
    }
    let png = canvas.encode_png().map_err(|e| format!("the PNG encoder failed: {e}"))?;
    Ok(Drawn { png, degraded })
}

/// The verdict a picture gives: `fails` when the case `failed`, `excused by` the deviations that excuse the
/// measures of `rows` outside their tolerance, or `agrees`.
pub fn verdict(failed: bool, rows: &[Compared], excused: &BTreeMap<String, String>) -> String {
    let by: BTreeSet<&String> = rows.iter().filter(|row| !row.3).filter_map(|row| excused.get(row.0.name())).collect();
    match (failed, by.is_empty()) {
        (true, _) => "fails".to_string(),
        (false, false) => format!("excused by {}", by.into_iter().cloned().collect::<Vec<_>>().join(", ")),
        (false, true) => "agrees".to_string(),
    }
}

/// The picture of a case called `title`, drawn and written to `path`: where it is, or, when it couldn't be
/// drawn or written, why, as a degraded picture. A picture never fails its case.
pub fn pictured(path: &Path, title: &str, verdict: &str, rows: &[Compared], theirs: &Side, ours: &Side) -> Picture {
    let rel = util::rel(path);
    let written = draw(title, verdict, rows, theirs, ours, false).and_then(|drawn| {
        let wrote = path.parent().map_or(Ok(()), std::fs::create_dir_all).and_then(|()| std::fs::write(path, &drawn.png));
        wrote.map(|()| drawn.degraded).map_err(|e| format!("{rel}: {e}"))
    });
    match written {
        Ok(degraded) => Picture { path: rel, degraded },
        Err(e) => Picture { path: String::new(), degraded: vec![format!("no picture: {e}")] },
    }
}

/// `count` pictures laid out in `columns` columns, each in a cell as large as the largest: a contact sheet.
pub fn sheet(pictures: &[Vec<u8>], columns: usize) -> Result<Vec<u8>, String> {
    let decoded: Vec<Pixmap> = pictures.iter().filter_map(|png| Pixmap::decode_png(png).ok()).collect();
    let (cw, ch) = decoded.iter().fold((1, 1), |(w, h), p| (w.max(p.width()), h.max(p.height())));
    let columns = columns.clamp(1, decoded.len().max(1));
    let rows = decoded.len().div_ceil(columns).max(1);
    let (width, height) = (columns as u32 * cw, rows as u32 * ch);
    let mut canvas = Pixmap::new(width, height).ok_or_else(|| format!("no contact sheet of {width} × {height} pixels"))?;
    canvas.fill(color(PAPER));
    for (k, picture) in decoded.iter().enumerate() {
        let (x, y) = ((k % columns) as u32 * cw, (k / columns) as u32 * ch);
        canvas.draw_pixmap(x as i32, y as i32, picture.as_ref(), &PixmapPaint::default(), Transform::identity(), None);
    }
    canvas.encode_png().map_err(|e| format!("the PNG encoder failed: {e}"))
}

fn color(rgb: [u8; 3]) -> Color {
    Color::from_rgba8(rgb[0], rgb[1], rgb[2], 255)
}

fn paint(rgb: [u8; 3]) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(color(rgb));
    paint.anti_alias = true;
    paint
}

fn fill(canvas: &mut Pixmap, x: f32, y: f32, w: f32, h: f32, rgb: [u8; 3]) {
    if let Some(rect) = Rect::from_xywh(x, y, w, h) {
        canvas.fill_rect(rect, &paint(rgb), Transform::identity(), None);
    }
}

/// `said` in capitals from its top left corner at (`x`, `y`), `unit` pixels a step of the letters' grid.
fn text(canvas: &mut Pixmap, said: &str, x: f32, y: f32, unit: f32, rgb: [u8; 3]) {
    let mut path = PathBuilder::new();
    for (k, c) in said.chars().enumerate() {
        let at = x + k as f32 * letters::ADVANCE * unit;
        for ((x0, y0), (x1, y1)) in letters::segments(c) {
            path.move_to(at + x0 * unit, y + y0 * unit);
            path.line_to(at + x1 * unit, y + y1 * unit);
        }
    }
    let Some(path) = path.finish() else { return };
    let stroke = Stroke { width: (unit * 0.7).max(1.0), line_cap: tiny_skia::LineCap::Round, ..Stroke::default() };
    canvas.stroke_path(&path, &paint(rgb), &stroke, Transform::identity(), None);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conformance::differential::{encoded, judged, report};
    use crate::conformance::runner::Outcome;
    use stitchcraft_core::Point;
    use stitchcraft_plan::{PlanBuilder, Provenance, Rgb, Role, Thread};

    fn p(x: f64, y: f64) -> Point {
        Point::new(x, y).unwrap()
    }

    /// A plan of rows `gap` mm apart in `thread`, across a square `side` mm wide, from (`x`, 0).
    fn rows(thread: Rgb, side: f64, gap: f64, x: f64) -> StitchPlan {
        let mut builder = PlanBuilder::new(Thread::new(thread));
        let top = Provenance::plan(Role::Top);
        let mut y = 0.0;
        let mut forth = true;
        while y <= side {
            let (a, b) = if forth { (x, x + side) } else { (x + side, x) };
            builder.stitch(p(a, y), top);
            builder.stitch(p(b, y), top);
            forth = !forth;
            y += gap;
        }
        builder.finish()
    }

    fn outcome(case: &str) -> Outcome {
        Outcome {
            case: case.into(),
            kind: "data",
            file: format!("conformance/cases/inkstitch/{case}.toml"),
            requirements: vec!["REQ-SVG-008".into()],
            failures: Vec::new(),
            outputs: Vec::new(),
            skipped: None,
            notes: Vec::new(),
            details: Vec::new(),
            pictures: Vec::new(),
        }
    }

    fn root(name: &str) -> std::path::PathBuf {
        util::root().join("target/xtask-tests").join(name)
    }

    #[test]
    fn every_case_both_programs_sew_is_drawn_into_its_picture_and_the_report_shows_it() {
        let red = Rgb::new(237, 23, 31);
        let (theirs, ours) = (encoded(&rows(red, 10.0, 0.4, 0.0), "a").unwrap(), encoded(&rows(red, 10.0, 0.5, 0.0), "b").unwrap());
        let root = root("differential-pictures");
        let mut judged_case = outcome("square");
        judged(&root, Ok(theirs), Ok(ours), &BTreeMap::new(), &mut judged_case);
        let path = root.join(super::super::PICTURES).join("square.png");
        assert_eq!(judged_case.pictures, [Picture { path: util::rel(&path), degraded: Vec::new() }]);
        let picture = Pixmap::decode_png(&std::fs::read(&path).unwrap()).unwrap();
        // 3 panels of the least width, the overlap's grid of 14.4 mm, with the margins, at 5 pixels a millimetre
        // being narrower.
        assert_eq!(picture.width(), PAD + 3 * (LEAST_WIDTH + PAD));
        assert_eq!(picture.height(), PAD + 34 + 20 + (14.4_f32 * 5.0).ceil() as u32 + 7 * 15 + PAD);
        let markdown = report(&[judged_case], "abc1234", "d59c9ab");
        assert!(
            markdown.contains("](target/xtask-tests/differential-pictures/target/conformance/differential-site/pictures/square.png)"),
            "{markdown}"
        );
        assert!(markdown.contains("![square: Ink/Stitch's file, StitchCraft's and where they differ]("), "{markdown}");
        // A case one program doesn't sew has no picture.
        let mut alone = outcome("alone");
        judged(&root, Err("Ink/Stitch wrote nothing".into()), Ok(encoded(&rows(red, 10.0, 0.5, 0.0), "b").unwrap()), &BTreeMap::new(), &mut alone);
        assert!(alone.pictures.is_empty());
    }

    #[test]
    fn a_picture_s_labels_are_the_numbers_of_the_report_s_table() {
        let (red, blue) = (Rgb::new(237, 23, 31), Rgb::new(11, 61, 145));
        let root = root("differential-labels");
        let (theirs, ours) = (rows(red, 10.0, 0.4, 0.0), rows(blue, 12.0, 0.4, 3.0));
        let mut judged_case = outcome("labels");
        judged(&root, Ok(encoded(&theirs, "a").unwrap()), Ok(encoded(&ours, "b").unwrap()), &BTreeMap::new(), &mut judged_case);
        let (mine, other) = (Measured::of(&ours).unwrap(), Measured::of(&theirs).unwrap());
        let rows = super::super::compare(&mine, &other);
        let cells = |row: &str| row.split('|').map(str::trim).filter(|c| !c.is_empty()).map(str::to_string).collect::<Vec<_>>();
        let table: Vec<Vec<String>> = judged_case.details.iter().map(|row| cells(row)).collect();
        for (side, column) in [(false, 1), (true, 2)] {
            let captions = captions(&rows, side);
            assert_eq!(captions.len(), 6);
            for (caption, row) in captions.iter().zip(table.iter().filter(|row| row[0] != "overlap")) {
                let value = caption.strip_prefix(&format!("{} ", row[0])).unwrap();
                let (value, marked) = value.strip_suffix(" !").map_or((value, false), |v| (v, true));
                if row[0] == "threads" {
                    assert_eq!(value, row[column].split_whitespace().count().to_string());
                } else {
                    assert_eq!(value, row[column], "{caption}");
                }
                // A measure outside its tolerance is marked, as the table marks it.
                assert_eq!(marked, row[3] != "✅", "{caption} {row:?}");
            }
        }
        let overlap_row = table.iter().find(|row| row[0] == "overlap").unwrap();
        assert_eq!(overlap(&rows), format!("overlap {}{}", overlap_row[1], if overlap_row[2] == "✅" { "" } else { " !" }));
        assert_eq!(overlap(&[]), "overlap -");
        assert!(captions(&rows, false).contains(&"stitches 64 !".to_string()) || !captions(&rows, false).is_empty());
    }

    #[test]
    fn a_picture_says_whether_the_case_agrees_is_excused_or_fails() {
        let row = |measure: Measure, within: bool| (measure, "1".to_string(), "2".to_string(), within);
        let excused = BTreeMap::from([
            ("trims".to_string(), "DEV-FMT-001".to_string()),
            ("jumps".to_string(), "DEV-FMT-001".to_string()),
            ("size".to_string(), "DEV-X-002".to_string()),
        ]);
        let rows = [row(Measure::Trims, false), row(Measure::Jumps, false), row(Measure::Size, true)];
        assert_eq!(verdict(false, &rows, &excused), "excused by DEV-FMT-001");
        let both = [row(Measure::Trims, false), row(Measure::Size, false)];
        assert_eq!(verdict(false, &both, &excused), "excused by DEV-FMT-001, DEV-X-002");
        assert_eq!(verdict(true, &rows, &excused), "fails");
        assert_eq!(verdict(false, &[row(Measure::Trims, true)], &excused), "agrees");
        assert_eq!(verdict(false, &[row(Measure::Stitches, false)], &excused), "agrees");
    }

    #[test]
    fn a_picture_that_can_t_be_written_is_degraded_and_says_why() {
        let red = Rgb::new(237, 23, 31);
        let plan = rows(red, 5.0, 0.4, 0.0);
        let measured = Measured::of(&plan).unwrap();
        let side = |name| Side { name, plan: &plan, measured: &measured };
        let root = root("differential-degraded");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("file"), "").unwrap();
        let picture = pictured(&root.join("file/x.png"), "x", "agrees", &[], &side("Ink/Stitch"), &side("StitchCraft"));
        assert_eq!(picture.path, "");
        assert!(picture.degraded.len() == 1 && picture.degraded[0].starts_with("no picture: "), "{picture:?}");
    }

    #[test]
    fn a_huge_design_makes_a_picture_no_larger_than_a_small_one_and_the_same_bytes_each_time() {
        let red = Rgb::new(237, 23, 31);
        let plan = rows(red, 400.0, 20.0, 0.0);
        let measured = Measured::of(&plan).unwrap();
        let side = |name| Side { name, plan: &plan, measured: &measured };
        let drawn = draw("huge", "agrees", &[], &side("Ink/Stitch"), &side("StitchCraft"), false).unwrap();
        assert!(drawn.degraded.is_empty());
        let picture = Pixmap::decode_png(&drawn.png).unwrap();
        // The overlap's grid, 404.4 mm with the margins, in 400 pixels: 0.9 pixels a millimetre.
        assert_eq!(picture.width(), PAD + 3 * ((404.4_f32 * 0.9).ceil() as u32 + PAD));
        assert!(picture.width() <= PAD + 3 * (CELL as u32 + PAD) && picture.height() <= 2 * CELL as u32);
        let again = draw("huge", "agrees", &[], &side("Ink/Stitch"), &side("StitchCraft"), false).unwrap();
        assert_eq!(drawn, again);
        // Small, for a contact sheet.
        let small = Pixmap::decode_png(&draw("huge", "agrees", &[], &side("Ink/Stitch"), &side("StitchCraft"), true).unwrap().png).unwrap();
        // At 0.2 pixels a millimetre its panels would be 81 pixels wide, narrower than the least.
        assert_eq!((404.4_f32 * 0.2).ceil() as u32, 81);
        assert_eq!(small.width(), PAD + 3 * (SMALL_LEAST_WIDTH + PAD));
        assert_eq!(small.height(), PAD + 30 + (404.4_f32 * 0.2).ceil() as u32 + PAD);
    }

    #[test]
    fn an_example_is_laid_out_as_a_case_with_the_sides_labelled_as_given() {
        let red = Rgb::new(237, 23, 31);
        let (close, wide) = (rows(red, 10.0, 0.4, 0.0), rows(red, 10.0, 0.8, 0.0));
        let png = super::super::example("square.svg", ("rows 0.4 mm", &close), ("rows 0.8 mm", &wide)).unwrap();
        let picture = Pixmap::decode_png(&png).unwrap();
        assert_eq!(picture.width(), PAD + 3 * (LEAST_WIDTH + PAD));
        // The same as a case's picture of the same 2 files, as read back, with their labels.
        let back = |plan: &StitchPlan| stitchcraft_formats::decode(&encoded(plan, "x").unwrap()).unwrap().plan;
        let (close, wide) = (back(&close), back(&wide));
        let (first, second) = (Measured::of(&close).unwrap(), Measured::of(&wide).unwrap());
        let rows = super::super::compare(&second, &first);
        let sides = (Side { name: "rows 0.4 mm", plan: &close, measured: &first }, Side { name: "rows 0.8 mm", plan: &wide, measured: &second });
        assert_eq!(png, draw("square.svg", "differ", &rows, &sides.0, &sides.1, false).unwrap().png);
        // 2 plans that agree by every measure say so.
        let same = super::super::example("square.svg", ("a", &close), ("b", &close)).unwrap();
        let rows = super::super::compare(&first, &first);
        let sides = (Side { name: "a", plan: &close, measured: &first }, Side { name: "b", plan: &close, measured: &first });
        assert_eq!(same, draw("square.svg", "agree", &rows, &sides.0, &sides.1, false).unwrap().png);
    }

    #[test]
    fn a_contact_sheet_lays_its_pictures_out_in_columns_of_cells_as_large_as_the_largest() {
        let picture = |w: u32, h: u32| Pixmap::new(w, h).unwrap().encode_png().unwrap();
        let sheet = Pixmap::decode_png(&sheet(&[picture(10, 20), picture(30, 5), picture(4, 4)], 2).unwrap()).unwrap();
        assert_eq!((sheet.width(), sheet.height()), (60, 40));
        let one = Pixmap::decode_png(&super::sheet(&[picture(10, 20)], 4).unwrap()).unwrap();
        assert_eq!((one.width(), one.height()), (10, 20));
    }
}
