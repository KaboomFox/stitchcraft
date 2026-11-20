//! `stitch lettering` on fonts drawn in code (`REQ-FONT-014`): no font file of anyone's is used.

use std::path::{Path, PathBuf};

use stitchcraft_testkit::temp::TempDir;

use super::*;
use crate::cli::Format;
use stitchcraft_svg::lettering::{Align, ColourSort, TextPosition, Trim, VerticalPosition};

/// A glyph of one satin column, 2 mm wide and 10 mm tall, standing on the baseline with its left rail at
/// `left`.
fn glyph(label: &str, left: f64) -> String {
    format!(
        r#"<g inkscape:groupmode="layer" inkscape:label="GlyphLayer-{label}"><path id="p{label}" d="M {left},60 L {left},70 M {r},60 L {r},70" style="fill:none;stroke:#000000" inkstitch:satin_column="true"/></g>"#,
        r = left + 2.0
    )
}

/// A font folder in `dir` with the glyphs `H` and `i`, each advancing 4 mm, a space 3 and a line 15, with
/// `settings` more in its `font.json`. Its glyph file lies in a folder `ltr`.
fn font_in(dir: &TempDir, settings: &str) -> PathBuf {
    let folder = dir.join("testfont");
    std::fs::create_dir_all(folder.join("ltr")).unwrap();
    let px = |mm: f64| mm * 96.0 / 25.4;
    let json = format!(
        r#"{{"name": "Test Font", "auto_satin": true, "horiz_adv_x_default": {}, "horiz_adv_x_space": {}, "leading": {}{settings}}}"#,
        px(4.0),
        px(3.0),
        px(15.0)
    );
    std::fs::write(folder.join("font.json"), json).unwrap();
    let file = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" xmlns:sodipodi="http://sodipodi.sourceforge.net/DTD/sodipodi-0.dtd" xmlns:inkstitch="http://inkstitch.org/namespace" width="100mm" height="100mm" viewBox="0 0 100 100"><metadata><inkstitch:inkstitch_svg_version>3</inkstitch:inkstitch_svg_version></metadata><sodipodi:namedview id="base"><sodipodi:guide position="0,30" orientation="0,1" inkscape:label="baseline"/></sodipodi:namedview>{}{}</svg>"#,
        glyph("H", 1.0),
        glyph("i", 0.0)
    );
    std::fs::write(folder.join("ltr").join("glyphs.svg"), file).unwrap();
    std::fs::write(folder.join("LICENSE"), "SIL Open Font License").unwrap();
    folder
}

/// The arguments that set `text` in `font` and write `output`, everything else at its default.
fn args(text: &str, font: &Path, output: PathBuf) -> LetteringArgs {
    LetteringArgs {
        text: text.to_string(),
        font: font.to_path_buf(),
        output,
        profile: None,
        format: None,
        preview: None,
        scale: None,
        height: None,
        align: Align::Left,
        letter_spacing: 0.0,
        word_spacing: 0.0,
        line_height: 0.0,
        trim: Trim::Off,
        color_sort: ColourSort::Off,
        back_and_forth: true,
        widen_percent: 0.0,
        path: None,
        text_position: TextPosition::Left,
        vertical_text_position: VerticalPosition::Top,
    }
}

/// Whether `stderr` has no warning and no error: the planner may note needle points it left out.
fn quiet(stderr: &str) -> bool {
    stderr.lines().all(|line| line.starts_with("info ") || line.starts_with("  "))
}

/// The size of the design in the machine file at `path`, in millimetres, and its stitches.
fn sewn(path: &Path) -> (f64, f64, usize) {
    let decoded = stitchcraft_formats::decode(&std::fs::read(path).unwrap()).unwrap();
    let bounds = decoded.plan.bounds().unwrap();
    (bounds.width(), bounds.height(), decoded.plan.stats().stitches)
}

#[test]
fn req_font_014_a_word_sews_for_the_pe800_inside_its_hoop() {
    let dir = TempDir::new("lettering");
    let font = font_in(&dir, r#", "size": 10, "min_scale": 0.5, "max_scale": 3"#);
    let (output, preview) = (dir.join("hi.pes"), dir.join("hi.png"));
    let out = run(&LetteringArgs { preview: Some(preview.clone()), ..args("Hi", &font, output.clone()) });
    assert!(out.status == Status::Done && quiet(&out.stderr), "{}", out.stderr);
    let lines: Vec<&str> = out.stdout.lines().collect();
    assert_eq!(lines[0], "Hi · Test Font (testfont)");
    assert!(lines[1].starts_with(&format!("  file      {} (PES", output.display())), "{}", out.stdout);
    assert_eq!(lines[3], format!("  profile   {} ({})", profiles::REFERENCE.id, profiles::REFERENCE.name));
    assert_eq!(lines[4], "  scale     100 % (10.0 mm tall)");
    assert!(out.stdout.contains(&format!("  preview   {} (", preview.display())), "{}", out.stdout);
    assert!(std::fs::read(&preview).unwrap().starts_with(b"\x89PNG"));
    // `H` from 0 to 2 mm and `i` from 3 to 5, 10 mm tall: inside the PE800's 130 × 180 mm, with the satin's pull.
    let (width, height, stitches) = sewn(&output);
    assert!((4.5..6.0).contains(&width) && (9.5..11.0).contains(&height) && stitches > 40, "{width} × {height} mm, {stitches} stitches");
    assert!(width <= profiles::REFERENCE.hoop.width.get() && height <= profiles::REFERENCE.hoop.height.get());
    // `\n` ends a line, 15 mm above the next, and --height gives the scale over the font's size: twice as tall.
    let tall = run(&LetteringArgs { height: Some(20.0), back_and_forth: false, ..args("H\\ni", &font, output.clone()) });
    assert!(tall.status == Status::Done && quiet(&tall.stderr), "{}", tall.stderr);
    assert_eq!(tall.stdout.lines().next(), Some("H\\ni · Test Font (testfont)"));
    assert!(tall.stdout.contains("\n  scale     200 % (20.0 mm tall)\n"), "{}", tall.stdout);
    let (width, height, _) = sewn(&output);
    assert!((3.5..5.5).contains(&width) && (49.0..51.5).contains(&height), "{width} × {height} mm");
    // --scale, the spacing and the alignment reach the text, and a DST file is written by its format.
    let dst = dir.join("hi.bin");
    let spaced = LetteringArgs {
        scale: Some(50.0),
        letter_spacing: 2.0,
        word_spacing: 1.0,
        line_height: -5.0,
        align: Align::Right,
        format: Some(Format::Dst),
        back_and_forth: false,
        ..args("Hi i\\ni", &font, dst.clone())
    };
    let spaced = run(&spaced);
    assert!(spaced.status == Status::Done && quiet(&spaced.stderr), "{}", spaced.stderr);
    assert!(spaced.stdout.contains("\n  scale     50 % (5.0 mm tall)\n") && spaced.stdout.contains("(DST, "), "{}", spaced.stdout);
    // `H` 0 to 2, `i` 5 to 7 and, after a 3 + 1 mm space, 13 to 15 mm, at half the size, over 2 lines 10 mm apart.
    let (width, height, _) = sewn(&dst);
    assert!((7.0..8.5).contains(&width) && (9.5..11.0).contains(&height), "{width} × {height} mm");
}

#[test]
fn req_font_014_what_is_said_is_printed_and_an_error_writes_nothing() {
    let dir = TempDir::new("lettering");
    let font = font_in(&dir, r#", "size": 10, "min_scale": 0.5, "max_scale": 3"#);
    let output = dir.join("out.pes");
    // A warning of the scale and one of a missing character are printed, and the file is written.
    let warned = run(&LetteringArgs { scale: Some(400.0), ..args("Hx", &font, output.clone()) });
    assert_eq!(warned.status, Status::Done);
    let codes: Vec<&str> = warned.stderr.lines().filter(|line| line.starts_with("warning ")).map(|line| line.split(':').next().unwrap()).collect();
    assert_eq!(codes, ["warning SC-W1106", "warning SC-W1107"]);
    assert!(warned.stdout.contains("\n  scale     300 % (30.0 mm tall)\n") && output.exists(), "{}", warned.stdout);
    std::fs::remove_file(&output).unwrap();
    // A text wider than the hoop is refused as any design is.
    let wide = run(&LetteringArgs { letter_spacing: 150.0, ..args("Hi", &font, output.clone()) });
    assert_eq!(wide.status, Status::DesignErrors);
    assert!(
        wide.stderr.contains("error SC-E0701: The design is 155.0 × 10.0 mm") && wide.stderr.ends_with("nothing was written\n"),
        "{}",
        wide.stderr
    );
    // A text of no glyph, after what reading the font said.
    let empty = run(&args(" ", &font, output.clone()));
    assert_eq!(empty.status, Status::DesignErrors);
    assert!(empty.stderr.starts_with("error SC-E1108: None of the text's characters sews in the font `testfont`"), "{}", empty.stderr);
    assert!(!output.exists());
    // A folder that is no font, and one that isn't there.
    let bare = dir.join("bare");
    std::fs::create_dir_all(&bare).unwrap();
    let unread = run(&args("Hi", &bare, output.clone()));
    assert_eq!(unread.status, Status::DesignErrors);
    assert!(unread.stderr.starts_with("error SC-E1101: The font `bare` has no `font.json`"), "{}", unread.stderr);
    let missing = dir.join("missing");
    let gone = run(&args("Hi", &missing, output.clone()));
    assert_eq!(gone.status, Status::Io);
    assert!(gone.stderr.starts_with(&format!("stitch: cannot read {}: ", missing.display())), "{}", gone.stderr);
    assert!(!output.exists());
}

#[test]
fn req_font_014_the_options_are_checked_before_the_font_is_read() {
    let dir = TempDir::new("lettering");
    // A font without a size: its scale is named in percent alone, its folder's name when it has no other.
    let font = font_in(&dir, "");
    let output = dir.join("out.pes");
    let plain = run(&args("i", &font, output.clone()));
    assert_eq!(plain.status, Status::Done, "{}", plain.stderr);
    assert!(plain.stdout.contains("\n  scale     100 %\n"), "{}", plain.stdout);
    let tall = run(&LetteringArgs { height: Some(12.0), ..args("i", &font, output.clone()) });
    assert_eq!(tall.status, Status::Usage);
    assert_eq!(tall.stderr, "stitch: the font `testfont` gives no size in its font.json, so --height can't be told: use --scale\n");
    let missing = dir.join("missing");
    assert_eq!(run(&args("Hi", &missing, dir.join("out.png"))).status, Status::Usage, "a format no file has");
    assert_eq!(run(&LetteringArgs { profile: Some("no-such-machine".to_string()), ..args("Hi", &missing, output.clone()) }).status, Status::Usage);
    let same = run(&LetteringArgs { preview: Some(output.clone()), ..args("Hi", &missing, output.clone()) });
    assert_eq!(same.status, Status::Usage, "{}", same.stderr);
    // A font named as its folder is named once.
    let folder = dir.join("named");
    std::fs::create_dir_all(&folder).unwrap();
    for file in ["font.json", "ltr/glyphs.svg"] {
        std::fs::create_dir_all(folder.join(file).parent().unwrap()).unwrap();
        let text = std::fs::read_to_string(font.join(file)).unwrap().replace("Test Font", "named");
        std::fs::write(folder.join(file), text).unwrap();
    }
    assert_eq!(run(&args("i", &folder, output)).stdout.lines().next(), Some("i · named"));
}

/// An SVG file `name` in `dir`, 100 mm square, holding `body`.
fn svg_in(dir: &TempDir, name: &str, body: &str) -> PathBuf {
    let file = dir.join(name);
    let root = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" width="100mm" height="100mm" viewBox="0 0 100 100">"#;
    std::fs::write(&file, format!("{root}{body}</svg>")).unwrap();
    file
}

#[test]
fn req_font_020_a_text_is_set_along_a_path_of_an_svg_file() {
    let dir = TempDir::new("lettering-path");
    let font = font_in(&dir, r#", "size": 10, "min_scale": 0.5, "max_scale": 3"#);
    let output = dir.join("out.pes");
    let line = |id: &str, d: &str, more: &str| format!(r#"<path id="{id}" d="{d}" style="fill:none;stroke:#000000" {more}/>"#);
    // The path labelled `batch lettering` is the one, wherever it lies in the file: it runs down, and so
    // does the text, in the path's place on the page.
    let labelled = [line("across", "M 10,10 L 90,10", ""), line("down", "M 50,20 L 50,90", r#"inkscape:label="batch lettering""#)].concat();
    let file = svg_in(&dir, "labelled.svg", &labelled);
    let down = run(&LetteringArgs { path: Some(file.clone()), ..args("HiH", &font, output.clone()) });
    assert_eq!(down.status, Status::Done, "{}", down.stderr);
    assert!(down.stdout.contains(&format!("\n  path      the path labelled `batch lettering` of {}\n", file.display())), "{}", down.stdout);
    let (width, height, _) = sewn(&output);
    assert!((width - 10.0).abs() < 1.0 && (height - 10.0).abs() < 1.0, "{width} × {height}");
    // Without a label, the file's first path: the text runs across, from the path's start.
    let file = svg_in(&dir, "first.svg", &[line("across", "M 10,10 L 90,10", ""), line("down", "M 50,20 L 50,90", "")].concat());
    let across = run(&LetteringArgs { path: Some(file.clone()), text_position: TextPosition::Stretch, ..args("HiH", &font, output.clone()) });
    assert_eq!(across.status, Status::Done, "{}", across.stderr);
    assert!(across.stdout.contains(&format!("\n  path      the first path, `svg:across:stroke` of {}\n", file.display())), "{}", across.stdout);
    let (width, height, _) = sewn(&output);
    assert!((width - 80.0).abs() < 1.0 && (height - 10.0).abs() < 1.0, "{width} × {height}");
    // Hanging from the path, the text is the same size, and what the path's file and the text said is printed.
    let short = svg_in(&dir, "short.svg", &line("short", "M 10,10 L 12,10", ""));
    let hung = run(&LetteringArgs { path: Some(short), vertical_text_position: VerticalPosition::Bottom, ..args("HiH", &font, output.clone()) });
    assert_eq!(hung.status, Status::Done, "{}", hung.stderr);
    assert!(hung.stderr.contains("warning SC-W1109: A line of the text is 10.0 mm long, and its path is 2.0 mm long."), "{}", hung.stderr);
    std::fs::remove_file(&output).unwrap();
    // A file without a path that paints, one that is no SVG file, one that isn't there, and a path without
    // length: nothing is written.
    let empty = run(&LetteringArgs { path: Some(svg_in(&dir, "empty.svg", "")), ..args("HiH", &font, output.clone()) });
    assert_eq!(empty.status, Status::Usage);
    assert!(empty.stderr.ends_with("empty.svg has no path that paints to set the text along: give one a stroke\n"), "{}", empty.stderr);
    let bad = dir.join("bad.svg");
    std::fs::write(&bad, "<svg><path").unwrap();
    let unread = run(&LetteringArgs { path: Some(bad), ..args("HiH", &font, output.clone()) });
    assert_eq!(unread.status, Status::DesignErrors);
    assert!(unread.stderr.starts_with("error SC-E0801") && unread.stderr.ends_with("nothing was written\n"), "{}", unread.stderr);
    let missing = run(&LetteringArgs { path: Some(dir.join("none.svg")), ..args("HiH", &font, output.clone()) });
    assert_eq!(missing.status, Status::Io);
    let still = svg_in(&dir, "still.svg", &line("still", "M 10,10 L 10,10 M 0,0 L 5,5", ""));
    let unset = run(&LetteringArgs { path: Some(still), ..args("HiH", &font, output.clone()) });
    assert_eq!(unset.status, Status::DesignErrors);
    assert!(unset.stderr.contains("error SC-E1108: The path to set the text along has no length"), "{}", unset.stderr);
    assert!(!output.exists());
}

#[test]
fn req_font_020_the_trims_the_sort_and_back_and_forth_are_passed_on() {
    let dir = TempDir::new("lettering-order");
    let font = font_in(&dir, r#", "size": 10, "min_scale": 0.5, "max_scale": 3"#);
    let output = dir.join("out.pes");
    let trims = |args: &LetteringArgs| {
        let outcome = run(args);
        assert_eq!(outcome.status, Status::Done, "{}", outcome.stderr);
        (stitchcraft_formats::decode(&std::fs::read(&output).unwrap()).unwrap().plan.stats().trims, outcome.stderr)
    };
    // The font's letters are joined by the route: a trim after each cuts the joins.
    let (joined, _) = trims(&args("Hi", &font, output.clone()));
    let (cut, _) = trims(&LetteringArgs { trim: Trim::Glyph, ..args("Hi", &font, output.clone()) });
    assert_eq!((joined, cut), (0, 1));
    // The font sews one way and doesn't sort: asked for both, it says so twice, and not when neither is asked.
    let (_, said) = trims(&LetteringArgs { color_sort: ColourSort::All, ..args("Hi\\nHi", &font, output.clone()) });
    assert_eq!(said.matches("warning SC-W1110").count(), 2, "{said}");
    let (_, said) = trims(&LetteringArgs { back_and_forth: false, ..args("Hi\\nHi", &font, output.clone()) });
    assert_eq!(said.matches("warning SC-W1110").count(), 0, "{said}");
}
