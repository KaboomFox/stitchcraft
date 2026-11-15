//! Which stitch a shape gets (`REQ-AUTO-030` to `REQ-AUTO-036`): automatic digitizing chooses the stitches of
//! each part of a shape from the part's geometry, by the first rule of one table that it matches
//! (`auto::rules`), and each shape says which rule that was (`SC-I0912`).
//!
//! The rules ask nothing about what a picture shows, so each case draws its shape in code, or takes a few
//! shapes of a traced picture: a blob, a branching patch and the nose of the owner's traced dog, the traced
//! pear, the letters and the face of the TS-13 sheets, and an ink drawing made of brush strokes.

// Test code may unwrap and index (clippy.toml allows it in tests).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeMap;

use stitchcraft_core::math::{self, sin_cos};
use stitchcraft_core::{Budget, Code, Mm};
use stitchcraft_engine::auto::rules::{PRESETS, RULES};
use stitchcraft_engine::auto::{Digitized, LONGEST_SATIN_STITCH, Stitches, digitize, digitized};
use stitchcraft_engine::design::{Design, DesignSettings, Element, Shape};
use stitchcraft_engine::generators::passes::BEAN_STITCH_REPEATS;
use stitchcraft_engine::generators::satin::SATIN_COLUMN;
use stitchcraft_engine::generators::tatami::ANGLE;
use stitchcraft_plan::Role;
use stitchcraft_testkit::designs::{BLUE, RED, along, automatic, fill, in_place, messages, planned, polylines, rules, rules_said, unbased, widened};
use stitchcraft_testkit::pictures;

/// What the automatic `element` sews as, in a design with the default settings.
fn made(element: &Element) -> Digitized {
    digitize(&automatic(element.clone()), &DesignSettings::default(), &mut Budget::DEFAULT.meter()).unwrap()
}

/// A red fill `id` of the outline `outline`, with the settings `params`.
fn shape(id: &str, outline: &[(f64, f64)], params: &[(&str, &str)]) -> Element {
    fill(id, &[outline], &RED, params)
}

/// A rectangle `wide` by `tall` from the origin.
fn rectangle(wide: f64, tall: f64) -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (wide, 0.0), (wide, tall), (0.0, tall)]
}

/// An ellipse round `(cx, cy)` with the radii `(rx, ry)`, in `n` corners.
fn ellipse((cx, cy): (f64, f64), (rx, ry): (f64, f64), n: u32) -> Vec<(f64, f64)> {
    (0..n)
        .map(|i| {
            let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(i) / f64::from(n));
            (cx + rx * cos, cy + ry * sin)
        })
        .collect()
}

/// `ring` the other way round, as a hole is drawn.
fn hole(mut ring: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    ring.reverse();
    ring
}

/// A leaf `long` by `wide` with its tips on the x axis, round `(cx, cy)`: 2 arcs of a circle.
fn leaf((cx, cy): (f64, f64), long: f64, wide: f64) -> Vec<(f64, f64)> {
    let (half, sag) = (long / 2.0, wide / 2.0);
    let radius = (half * half + sag * sag) / (2.0 * sag);
    let reach = math::atan2(half, radius - sag);
    let arc = |sign: f64| {
        (0..=24).map(move |i| {
            let (sin, cos) = sin_cos(-reach + 2.0 * reach * f64::from(i) / 24.0);
            (cx + radius * sin, cy + sign * (radius * cos - (radius - sag)))
        })
    };
    arc(1.0).chain(arc(-1.0).collect::<Vec<_>>().into_iter().rev().skip(1).take(23)).collect()
}

/// How each of `elements` sews: `fill`, `satin` or `run`.
fn kinds(elements: &[Element]) -> Vec<&'static str> {
    let kind = |element: &Element| match &element.shape {
        Shape::Fill { .. } => "fill",
        Shape::Stroke { .. } if setting(element, SATIN_COLUMN) == Some("true") => "satin",
        Shape::Stroke { .. } => "run",
    };
    elements.iter().map(kind).collect()
}

/// The setting `key` of `element`.
fn setting<'e>(element: &'e Element, key: &str) -> Option<&'e str> {
    element.params.iter().find(|(k, _)| *k == key).map(|(_, value)| value)
}

/// What `SC-I0912` says of the shape `made` was digitized from.
fn rule(made: &Digitized) -> String {
    let said: Vec<&str> = made.diagnostics.iter().filter(|said| said.code == Code::AutoRule).map(|said| said.message.as_str()).collect();
    assert_eq!(said.len(), 1, "{:?}", made.diagnostics);
    said[0].strip_prefix("This shape sews by the rule ").unwrap_or(said[0]).trim_end_matches('.').to_string()
}

/// How many parts of `design` sewed by each rule, by the rule's word, as `SC-I0912` says of its shapes.
fn fired(design: &Design) -> BTreeMap<&'static str, usize> {
    let (_, said) = digitized(design, &Budget::DEFAULT);
    let mut counts = BTreeMap::new();
    for said in said.iter().filter(|said| said.code == Code::AutoRule) {
        // A rule's word follows "rule " for a shape of one part, and ": " for each part of several that is
        // listed. The parts after them are counted by rule, as in "and 4 more: 1 speck, 3 column."
        let (listed, counted) = said.message.split_once(" more: ").unwrap_or((&said.message, ""));
        for rule in RULES {
            let key = rule.id.key();
            let more =
                counted.trim_end_matches('.').split(", ").filter_map(|each| each.strip_suffix(key)?.trim().parse::<usize>().ok()).sum::<usize>();
            let count = listed.matches(&format!("rule {key}")).count() + listed.matches(&format!(": {key}")).count() + more;
            if count > 0 {
                *counts.entry(key).or_insert(0) += count;
            }
        }
    }
    counts
}

/// The longest top stitch of each element of `design` that sews as a satin column, planned in place, in
/// millimetres, by the element's id.
fn longest_satin_stitches(design: &Design) -> Vec<(String, f64)> {
    let (elements, _) = digitized(design, &Budget::DEFAULT);
    let satin: Vec<String> = elements.iter().filter(|element| kinds(std::slice::from_ref(element)) == ["satin"]).map(|e| e.id.to_string()).collect();
    let plan = in_place(design).plan.unwrap();
    let mut longest: BTreeMap<String, f64> = BTreeMap::new();
    for stitch in plan.sewn_stitches().into_iter().filter(|stitch| stitch.role == Role::Top && stitch.from_role == Role::Top) {
        let element = plan.blocks[stitch.block].stitches[stitch.index].origin.element.and_then(|element| plan.element(element));
        let Some(id) = element.map(ToString::to_string).filter(|id| satin.contains(id)) else { continue };
        let length = longest.entry(id).or_insert(0.0);
        *length = length.max(stitch.from.distance(stitch.to));
    }
    longest.into_iter().collect()
}

/// A blob 3 mm square with 2 tails 0.6 mm wide and 8 mm long: a part of several centre lines whose width varies.
fn blob_with_tails() -> Vec<(f64, f64)> {
    vec![
        (0.0, 0.0),
        (3.0, 0.0),
        (3.0, 1.2),
        (11.0, 1.2),
        (11.0, 1.8),
        (3.0, 1.8),
        (3.0, 3.0),
        (1.8, 3.0),
        (1.8, 11.0),
        (1.2, 11.0),
        (1.2, 3.0),
        (0.0, 3.0),
    ]
}

/// A body 12 mm square with an arm `wide` mm wide and 24 mm long along its bottom.
fn body_with_arm(wide: f64) -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (12.0, 0.0), (12.0, 12.0), (36.0, 12.0), (36.0, 12.0 + wide), (0.0, 12.0 + wide)]
}

/// A T whose bar is 4 mm wide and whose stem is 1.5 mm wide: 2 even strokes of unlike widths.
fn thick_and_thin() -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (20.0, 0.0), (20.0, 4.0), (10.75, 4.0), (10.75, 16.0), (9.25, 16.0), (9.25, 4.0), (0.0, 4.0)]
}

#[test]
fn req_auto_031_a_blob_is_filled() {
    // A rectangle 4.5 by 3 mm has no way to run: a column along its middle would sew its stitches into its
    // corners. It is compact, 1.7 times its width squared, and filled.
    let blob = made(&shape("blob", &rectangle(4.5, 3.0), &[]));
    assert_eq!((kinds(&blob.elements), blob.stitches), (vec!["fill"], Some(Stitches::Fill)));
    assert_eq!(rule(&blob), "compact (2.8 mm wide, slenderness 1.7, width varying by 0.11, 1 centre line)");
    // An L 4.5 mm long each way, its arms 2.5 mm wide, has 2 centre lines, and is as compact.
    let ell = made(&shape("ell", &[(0.0, 0.0), (4.5, 0.0), (4.5, 2.5), (2.5, 2.5), (2.5, 4.5), (0.0, 4.5)], &[]));
    assert_eq!(kinds(&ell.elements), ["fill"]);
    assert!(rule(&ell).starts_with("compact ("), "{}", rule(&ell));
}

#[test]
fn req_auto_031_a_dot_sews_a_few_stitches_and_no_underlay() {
    // A disc 1.5 mm across is the smallest compact part. It sews as a fill of a few stitches, none of them
    // underlay, with the base layer and without.
    let dot = shape("dot", &ellipse((0.0, 0.0), (0.75, 0.75), 32), &[]);
    assert!(rule(&made(&dot)).starts_with("compact ("), "{}", rule(&made(&dot)));
    for elements in [vec![automatic(dot.clone())], unbased(vec![automatic(dot)])] {
        let plan = planned(elements).plan.unwrap();
        let roles: Vec<Role> = plan.sewn_stitches().iter().map(|stitch| stitch.role).collect();
        let top = roles.iter().filter(|role| **role == Role::Top).count();
        assert!(top > 0 && top < 20 && roles.iter().all(|role| *role != Role::Underlay), "{roles:?}");
    }
}

#[test]
fn req_auto_031_the_least_slenderness_is_a_setting_and_the_rules_lean_by_it() {
    // A rectangle 3 by 8 mm is 2.7 times its width squared: compact by default, and a column for a shape
    // whose least slenderness is 2, or that prefers satin columns, which divides the bound by 1.5.
    let short = |params: &[(&str, &str)]| kinds(&made(&shape("short", &rectangle(8.0, 3.0), params)).elements);
    assert_eq!([short(&[]), short(&[("auto_min_slenderness", "2")]), short(&[("auto_prefer", "satin")])], [["fill"], ["satin"], ["satin"]]);
    // One 3 by 12 mm is a column by default, and compact for a shape that prefers fills, whose bound is 4.5.
    let long = |params: &[(&str, &str)]| kinds(&made(&shape("long", &rectangle(12.0, 3.0), params)).elements);
    assert_eq!([long(&[]), long(&[("auto_prefer", "fill")]), long(&[("auto_min_slenderness", "4.5")])], [["satin"], ["fill"], ["fill"]]);
}

#[test]
fn req_auto_032_a_branching_patch_is_filled() {
    // A patch of the dog on turf traced in 9 threads, 8 by 6 mm with a tail and 5 branches: its width varies
    // by more than 0.3 of itself along its centre lines, and it is filled, not sewn as columns that fan where
    // they meet.
    let patch = pictures::dog_shapes("dog-on-turf-9", &["vc:103:fill"]);
    let (elements, said) = digitized(&patch, &Budget::DEFAULT);
    let said: Vec<&str> = said.iter().filter(|said| said.code == Code::AutoRule).map(|said| said.message.as_str()).collect();
    assert!(matches!(said.as_slice(), [one] if one.contains("uneven (")), "{said:?}");
    let sewn: Vec<&Element> = elements.iter().filter(|element| element.id.as_str().starts_with("vc:103:")).collect();
    assert!(sewn.iter().all(|element| setting(element, SATIN_COLUMN).is_none()), "{sewn:?}");
    assert!(matches!(sewn[0].shape, Shape::Fill { .. }), "{:?}", sewn[0].shape);
}

#[test]
fn req_auto_032_an_uneven_part_s_tails_leave_its_fill_as_lines() {
    // A blob 3 mm square with 2 tails 0.6 mm wide: uneven, so the blob is filled, and each tail, narrower
    // than the narrowest satin stroke, leaves the fill as a running stitch, tripled from 0.5 mm wide.
    let out = made(&shape("blob", &blob_with_tails(), &[]));
    assert_eq!((kinds(&out.elements), out.stitches), (vec!["fill", "run", "run"], Some(Stitches::Fill)));
    assert!(out.elements[1..].iter().all(|tail| setting(tail, BEAN_STITCH_REPEATS) == Some("1")), "{:?}", out.elements);
    let said = rule(&out);
    assert!(said.starts_with("This shape's 3 parts sew by these rules, part 1: uneven ("), "{said}");
    assert!(said.ends_with("2 centre lines), part 2: line (0.6 mm wide), part 3: line (0.6 mm wide)"), "{said}");
}

#[test]
fn req_auto_032_filling_uneven_parts_is_a_switch_and_its_bound_a_setting() {
    // A T of 2 even strokes of unlike widths, 4 mm and 1.5 mm, is uneven as a whole, its width varying by 0.4
    // of itself, and it is filled.
    let default = made(&shape("t", &thick_and_thin(), &[]));
    assert_eq!(
        (kinds(&default.elements), rule(&default)),
        (vec!["fill"], "uneven (3 mm wide, slenderness 10.9, width varying by 0.40, 2 centre lines)".to_string())
    );
    // With `auto_fill_uneven` off, or with a bound that its unevenness is within, each stroke sews as a satin
    // column, as width alone had it.
    for params in [[("auto_fill_uneven", "false")], [("auto_max_unevenness", "0.5")]] {
        let out = made(&shape("t", &thick_and_thin(), &params));
        assert_eq!((kinds(&out.elements), rule(&out).starts_with("column (")), (vec!["satin", "satin"], true), "{params:?}");
    }
    // A design drawn in strokes keeps them with the logo style, and the photo style is the rules' defaults.
    let logo = PRESETS.iter().find(|preset| preset.name == "logo").unwrap();
    let drawn = made(&shape("t", &thick_and_thin(), logo.values));
    assert_eq!((kinds(&drawn.elements), rule(&drawn).starts_with("column (")), (vec!["satin", "satin"], true));
    let photo = PRESETS.iter().find(|preset| preset.name == "photo").unwrap();
    assert_eq!(kinds(&made(&shape("t", &thick_and_thin(), photo.values)).elements), kinds(&default.elements));
}

#[test]
fn req_auto_032_even_strokes_a_taper_a_leaf_and_a_ring_sew_as_columns() {
    // A Y of 3 arms 2 mm wide: 2 centre lines of even width, 2 satin columns.
    let y = [(24.0, 16.0), (26.0, 16.0), (26.0, 23.0), (31.0, 28.0), (29.6, 29.4), (25.0, 24.8), (20.4, 29.4), (19.0, 28.0), (24.0, 23.0)];
    let y = made(&shape("y", &y, &[]));
    assert_eq!(kinds(&y.elements), ["satin", "satin"]);
    assert!(rule(&y).contains("2 centre lines"), "{}", rule(&y));
    // A stroke that tapers from 3 mm to a point over 18 mm, and a leaf 4 by 12 mm: each has one centre line,
    // so its width may vary as it likes, by 0.49 and by more than 0.3 of itself, and it sews as one column.
    for (name, outline) in [("taper", vec![(36.0, 17.0), (39.0, 17.0), (37.5, 35.0)]), ("leaf", leaf((0.0, 0.0), 12.0, 4.0))] {
        let out = made(&shape(name, &outline, &[]));
        assert_eq!(kinds(&out.elements), ["satin"], "{name}");
        let said = rule(&out);
        let varying: f64 = said.split("width varying by ").nth(1).unwrap().split(',').next().unwrap().parse().unwrap();
        assert!(said.starts_with("column (") && said.ends_with("1 centre line)") && varying > 0.3, "{name}: {said}");
    }
    // A ring 10 mm across in a stroke 2 mm wide: one column round its hole, which stays open.
    let ring = made(&fill("ring", &[&ellipse((0.0, 0.0), (5.0, 5.0), 64), &hole(ellipse((0.0, 0.0), (3.0, 3.0), 48))], &RED, &[]));
    assert_eq!(
        (kinds(&ring.elements), rule(&ring)),
        (vec!["satin"], "column (2 mm wide, slenderness 12.6, width varying by 0.00, 1 centre line)".to_string())
    );
}

#[test]
fn req_auto_032_lettering_keeps_its_columns() {
    // TS-13B's word MATCH, traced as areas 15, 10, 6 and 4 mm tall, and its windows' lines: no letter is
    // compact or uneven. Each word is one shape of 5 parts: those from 6 mm tall sew as columns, 15 parts
    // and 3 more of the windows, and the 4 mm word and the windows' lines as lines.
    let (design, _) = pictures::sheet("TS-13B");
    assert_eq!(fired(&design), BTreeMap::from([("area", 9), ("column", 18), ("line", 5)]));
}

#[test]
fn req_auto_033_a_nose_is_filled_under_its_nostrils() {
    // The nose of the running dog traced in 9 threads, 7.2 by 5.8 mm, and its 2 nostrils, which fill its
    // holes. The nose is uneven, and each nostril compact: the nose is filled whole, its holes closed, and
    // each nostril sews on it as a fill, after it. Width alone sewed the nose as 4 columns round
    // the nostrils.
    let nose = pictures::dog_shapes("dog-running-9", &["vc:14:fill", "vc:95:fill", "vc:96:fill"]);
    let (elements, said) = digitized(&nose, &Budget::DEFAULT);
    let sewn: Vec<(&str, &str, usize)> = elements
        .iter()
        .filter(|element| element.id.as_str().starts_with("vc:"))
        .map(|element| (element.id.as_str(), kinds(std::slice::from_ref(element))[0], element.shape.path().subpaths.len()))
        .collect();
    assert_eq!(sewn, [("vc:14:fill", "fill", 1), ("vc:95:fill", "fill", 1), ("vc:96:fill", "fill", 1)]);
    let rules: Vec<&str> = said.iter().filter(|said| said.code == Code::AutoRule).map(|said| said.message.as_str()).collect();
    assert!(
        matches!(rules.as_slice(), [nose, left, right] if nose.contains("rule uneven") && left.contains("rule compact") && right.contains("rule compact")),
        "{rules:?}"
    );
    assert!(said.iter().any(|said| said.code == Code::AutoLayers && said.message.contains("1 area is digitized whole")), "{:?}", messages_of(&said));
}

/// `said` as people read it.
fn messages_of(said: &[stitchcraft_core::Diagnostic]) -> Vec<String> {
    said.iter().map(ToString::to_string).collect()
}

#[test]
fn req_auto_033_a_filled_part_keeps_a_hole_that_nothing_fills_and_closes_one_that_a_shape_fills() {
    // A drawn nose, 7.2 by 5.8 mm with 2 holes 2.2 mm across that nothing fills: uneven, and filled round
    // its holes, which stay open.
    let nostril = |x: f64| hole(ellipse((x, 20.3), (1.1, 1.1), 24));
    let nose = fill("nose", &[&ellipse((9.0, 20.0), (3.6, 2.9), 64), &nostril(7.4), &nostril(10.6)], &RED, &[]);
    let alone = made(&nose);
    assert_eq!((kinds(&alone.elements), alone.elements[0].shape.path().subpaths.len()), (vec!["fill"], 3));
    assert!(rule(&alone).starts_with("uneven ("), "{}", rule(&alone));
    // With a disc in each hole, compact and so filled, the holes close: the nose's stitches are chosen for it
    // whole, 5.6 mm wide and compact, and it sews before the discs.
    let disc = |id: &str, x: f64| automatic(fill(id, &[&ellipse((x, 20.3), (1.1, 1.1), 24)], &BLUE, &[]));
    let outcome = planned(unbased(vec![automatic(nose), disc("left", 7.4), disc("right", 10.6)]));
    let whole = "info SC-I0908: 1 area is digitized whole under the areas that fill its holes, which sew after it.";
    assert!(messages(&outcome).iter().any(|said| said == whole), "{:?}", messages(&outcome));
    let said = rules_said(&outcome);
    assert_eq!(
        said[0],
        (
            "nose".to_string(),
            "This shape sews by the rule compact (5.61 mm wide, slenderness 1.0, width varying by 0.03, 1 centre line).".to_string()
        )
    );
    assert_eq!(said.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["nose", "left", "right"]);
    // A line 0.6 mm wide in a slit of an uneven part fills the slit as a detail does: the slit closes, and
    // the part, whole, is compact.
    let slit = [(4.0, 1.2), (4.0, 1.8), (9.0, 1.8), (9.0, 1.2)];
    let slitted = fill("blob", &[&blob_with_a_tail(), &slit], &RED, &[]);
    assert!(rule(&made(&slitted)).starts_with("This shape's 2 parts sew by these rules, part 1: uneven ("), "{}", rule(&made(&slitted)));
    let line = automatic(fill("line", &[&hole(slit.to_vec())], &BLUE, &[]));
    let outcome = planned(unbased(vec![automatic(slitted), line]));
    assert!(messages(&outcome).iter().any(|said| said == whole), "{:?}", messages(&outcome));
    let said = rules_said(&outcome);
    assert_eq!(said[1], ("line".to_string(), "This shape sews by the rule line (0.6 mm wide).".to_string()));
}

/// A blob 3 mm tall and 10 mm long with a tail 0.6 mm wide up from it: uneven, and room for a slit.
fn blob_with_a_tail() -> Vec<(f64, f64)> {
    vec![(0.0, 0.0), (10.0, 0.0), (10.0, 3.0), (1.8, 3.0), (1.8, 11.0), (1.2, 11.0), (1.2, 3.0), (0.0, 3.0)]
}

#[test]
fn req_auto_034_a_piece_leaves_a_fill_only_narrower_than_4_mm() {
    // A body 12 mm square with an arm 24 mm long. An arm 6 mm wide stays in the fill, where a disc 7 mm
    // across, which found the broad parts alone before, cut it off as a satin column 6 mm wide. An arm 3 mm
    // wide leaves the fill and sews as a column.
    let wide = made(&shape("wide", &body_with_arm(6.0), &[]));
    assert_eq!((kinds(&wide.elements), rule(&wide)), (vec!["fill"], "area".to_string()));
    let narrow = made(&shape("narrow", &body_with_arm(3.0), &[]));
    assert_eq!(kinds(&narrow.elements), ["fill", "satin"]);
    assert_eq!(
        rule(&narrow),
        "This shape's 2 parts sew by these rules, part 1: area, part 2: column (3 mm wide, slenderness 8.0, width varying by 0.00, 1 centre line)"
    );
    // The narrowest piece of a fill is a setting: at 7 mm the arm 6 mm wide leaves the fill, and at 2 mm the
    // arm 3 mm wide stays.
    assert_eq!(kinds(&made(&shape("wide", &body_with_arm(6.0), &[("auto_min_fill_width_mm", "7")])).elements), ["fill", "satin"]);
    assert_eq!(kinds(&made(&shape("narrow", &body_with_arm(3.0), &[("auto_min_fill_width_mm", "2")])).elements), ["fill"]);
    // And the rules lean by it: a shape that prefers satin columns keeps a piece from 6 mm wide in its fill,
    // so an arm 5 mm wide leaves, which stays by default.
    let arm = |params: &[(&str, &str)]| kinds(&made(&shape("arm", &body_with_arm(5.0), params)).elements);
    assert_eq!([arm(&[]), arm(&[("auto_prefer", "satin")])], [vec!["fill"], vec!["fill", "satin"]]);
}

#[test]
fn req_auto_034_the_blaze_is_filled_down_to_the_nose() {
    // The white blaze of the running dog traced in 9 threads is 12 mm wide above the nose. A disc 7 mm
    // across can't pass its corner, 6.7 mm from the band over the nose, and the wing beyond sewed as a column
    // of stitches up to 7.05 mm long. The fill now goes on as far as a disc 4 mm across fits: no column of
    // the blaze is wider than 4 mm.
    let blaze = pictures::dog_shapes("dog-running-9", &["vc:79:fill"]);
    let (elements, _) = digitized(&blaze, &Budget::DEFAULT);
    let widths: Vec<f64> = elements
        .iter()
        .filter(|element| setting(element, SATIN_COLUMN) == Some("true"))
        .filter_map(|element| if let Shape::Stroke { width, .. } = element.shape { Some(width.get()) } else { None })
        .collect();
    assert!(!widths.is_empty() && widths.iter().all(|width| *width < 4.0), "{widths:?}");
    assert!(matches!(elements.iter().find(|element| element.id.as_str() == "vc:79:fill").map(|fill| &fill.shape), Some(Shape::Fill { .. })));
}

/// A square `side` mm across with a round hole half as wide in its middle: a ring whose column sews its
/// longest stitches at the square's corners.
fn square_with_a_round_hole(side: f64) -> Element {
    let half = side / 2.0;
    // The lowest shape of its design, it is sewn though it covers the design's whole box, as a background does.
    let sewn = [("auto_ignore_background", "false")];
    fill(
        "ring",
        &[&[(-half, -half), (half, -half), (half, half), (-half, half)], &hole(ellipse((0.0, 0.0), (side / 4.0, side / 4.0), 48))],
        &RED,
        &sewn,
    )
}

#[test]
fn req_auto_035_no_satin_stitch_is_longer_than_the_widest_column() {
    // A square 16 mm across with a round hole 8 mm across is no wider than a satin column, 4.6 mm on
    // average, but a column round the hole would sew a stitch of 7.31 mm between its rails at each corner.
    // It is refused, and filled.
    let refused = made(&square_with_a_round_hole(16.0));
    assert_eq!((kinds(&refused.elements), refused.stitches), (vec!["fill"], Some(Stitches::Fill)));
    assert!(rule(&refused).starts_with("refused ("), "{}", rule(&refused));
    // One 12 mm across sews as a column: its longest stitch is 5.49 mm between its rails, and its pull
    // compensation longer on either side as sewn.
    let design = Design::new(unbased(vec![automatic(square_with_a_round_hole(12.0))]), DesignSettings::default()).unwrap();
    let longest = longest_satin_stitches(&design);
    let limit = LONGEST_SATIN_STITCH + 2.0 * rules().auto_pull_compensation_mm.get();
    let longest: Vec<f64> = longest.into_iter().map(|(_, length)| length).collect();
    assert!(!longest.is_empty() && longest.iter().all(|length| *length > 5.49 && *length < 6.4), "{longest:?}");
    // The traced pear sewed stitches of 7.65 and 7.91 mm in 2 columns under 7 mm wide on average. No column
    // of it now sews a stitch longer than 7 mm and its pull compensation on either side.
    let pear = longest_satin_stitches(&pictures::pear());
    assert!(!pear.is_empty() && pear.iter().all(|(_, length)| *length <= limit), "{pear:?}");
}

#[test]
fn diag_sc_i0902_a_column_that_would_sew_a_stitch_too_long_is_filled() {
    let refused = made(&square_with_a_round_hole(16.0));
    let said: Vec<String> = refused.diagnostics.iter().filter(|said| said.code == Code::AutoSatinFilled).map(ToString::to_string).collect();
    assert_eq!(
        said,
        ["info SC-I0902: A satin column along a part of this area would sew a stitch 7.31 mm long between its rails, longer than the widest satin \
          column, 7 mm, and such a stitch lies loose, so the part is filled."]
    );
    assert_eq!(refused.diagnostics[0].element.as_ref().map(|id| id.as_str()), Some("ring"));
    // In a hole of it, a disc is compact, and so filled: the square is then filled whole under the disc, and
    // nothing is refused.
    let disc = automatic(fill("disc", &[&ellipse((0.0, 0.0), (4.0, 4.0), 48)], &BLUE, &[]));
    let outcome = planned(unbased(vec![automatic(square_with_a_round_hole(16.0)), disc]));
    assert!(messages(&outcome).iter().all(|said| !said.contains("SC-I0902")), "{:?}", messages(&outcome));
    assert!(rules_said(&outcome).contains(&("ring".to_string(), "This shape sews by the rule area.".to_string())), "{:?}", rules_said(&outcome));
}

#[test]
fn req_auto_035_a_shape_may_lower_the_widest_satin_column_and_never_raise_it() {
    // A bar 7.5 mm wide is an area, and filled: a satin column that wide, with its stitches split, is a later
    // step's. A bar 5 mm wide is a column, and filled for a shape whose widest satin column is 4 mm.
    assert_eq!(kinds(&made(&shape("bar", &rectangle(7.5, 30.0), &[])).elements), ["fill"]);
    assert_eq!(kinds(&made(&shape("bar", &rectangle(5.0, 30.0), &[])).elements), ["satin"]);
    assert_eq!(kinds(&made(&shape("bar", &rectangle(5.0, 30.0), &[("auto_max_satin_width_mm", "4")])).elements), ["fill"]);
    // 7 mm is the longest satin stitch a machine sews well, so a shape that asks for 9 mm is read as 7 mm,
    // which is said (SC-W0102), and a bar 8 mm wide is filled all the same. So is a line that wide.
    let raised = made(&shape("bar", &rectangle(8.0, 30.0), &[("auto_max_satin_width_mm", "9")]));
    assert_eq!(kinds(&raised.elements), ["fill"]);
    assert_eq!(raised.diagnostics.first().map(|said| said.code), Some(Code::ParamClamped));
    let line = widened(along("line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), &RED, &[("auto_max_satin_width_mm", "9")]), 8.0);
    assert_eq!(made(&line).stitches, Some(Stitches::FilledStroke));
}

#[test]
fn diag_sc_i0912_each_shape_says_the_rule_its_parts_sewed_by() {
    // A blob, a line 0.5 mm wide, a line 3 mm wide and a body with an arm, planned: each automatic shape says
    // its rule, with its id, and a shape with settings says none.
    let thin = widened(along("thin", polylines(&[&[(0.0, 40.0), (20.0, 40.0)]]), &RED, &[]), 0.5);
    let column = widened(along("column", polylines(&[&[(0.0, 50.0), (20.0, 50.0)]]), &RED, &[]), 3.0);
    let set = widened(along("set", polylines(&[&[(0.0, 60.0), (20.0, 60.0)]]), &RED, &[]), 3.0);
    let blob = automatic(shape("blob", &at(&rectangle(4.5, 3.0), (50.0, 0.0)), &[]));
    let outcome = planned(unbased(vec![blob, automatic(thin), automatic(column), set, automatic(shape("arm", &body_with_arm(3.0), &[]))]));
    assert_eq!(
        rules_said(&outcome),
        [
            ("blob", "This shape sews by the rule compact (2.8 mm wide, slenderness 1.7, width varying by 0.11, 1 centre line)."),
            ("thin", "This shape sews by the rule line (0.5 mm wide)."),
            ("column", "This shape sews by the rule column (3 mm wide)."),
            (
                "arm",
                "This shape's 2 parts sew by these rules, part 1: area, part 2: column (3 mm wide, slenderness 8.0, width varying by 0.00, 1 centre \
                 line)."
            ),
        ]
        .map(|(id, said)| (id.to_string(), said.to_string()))
    );
    let said = outcome.diagnostics.iter().find(|said| said.code == Code::AutoRule).unwrap();
    assert_eq!((Code::AutoRule.id(), said.severity()), ("SC-I0912", stitchcraft_core::Severity::Info));
    // The other messages leave the rules out.
    assert!(messages(&outcome).iter().all(|said| !said.contains("SC-I0912")));
}

/// `outline` moved by `(dx, dy)`.
fn at(outline: &[(f64, f64)], (dx, dy): (f64, f64)) -> Vec<(f64, f64)> {
    outline.iter().map(|&(x, y)| (x + dx, y + dy)).collect()
}

#[test]
fn req_auto_036_auto_stitch_overrides_the_table() {
    // A blob that asks for satin columns sews as one, and a bar 3 by 30 mm that asks for a fill, or to run,
    // sews so: each says that it sews as asked.
    let blob = made(&shape("blob", &rectangle(4.5, 3.0), &[("auto_stitch", "satin")]));
    assert_eq!((kinds(&blob.elements), blob.stitches), (vec!["satin"], Some(Stitches::Satin)));
    assert!(rule(&blob).starts_with("asked ("), "{}", rule(&blob));
    let bar = |stitch: &str| made(&shape("bar", &rectangle(3.0, 30.0), &[("auto_stitch", stitch)]));
    assert_eq!([kinds(&bar("auto").elements), kinds(&bar("fill").elements), kinds(&bar("run").elements)], [["satin"], ["fill"], ["run"]]);
    assert_eq!((rule(&bar("fill")), rule(&bar("run"))), ("asked".to_string(), "asked (3 mm wide)".to_string()));
    assert_eq!((bar("run").stitches, setting(&bar("run").elements[0], BEAN_STITCH_REPEATS)), (Some(Stitches::Running), Some("1")));
    // A part that can't sew as asked goes by the table: a bar 0.6 mm wide that asks for satin columns runs,
    // and a body with an arm that asks for them is filled, with its arm a column. One that asks to be filled
    // is one fill, its arm too.
    let thin = made(&shape("thin", &rectangle(0.6, 30.0), &[("auto_stitch", "satin")]));
    assert_eq!((kinds(&thin.elements), rule(&thin)), (vec!["run"], "line (0.6 mm wide)".to_string()));
    let arm = |stitch: &str| kinds(&made(&shape("arm", &body_with_arm(3.0), &[("auto_stitch", stitch)])).elements);
    assert_eq!([arm("satin"), arm("fill")], [vec!["fill", "satin"], vec!["fill"]]);
    // A line goes by it too: 3 mm wide, it runs or fills the area it paints as asked, and 10 mm wide it can't
    // be a satin column, and is filled as the table says.
    let line =
        |width: f64, stitch: &str| made(&widened(along("line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), &RED, &[("auto_stitch", stitch)]), width));
    let stitches =
        [line(3.0, "auto"), line(3.0, "run"), line(3.0, "fill"), line(0.5, "satin"), line(10.0, "satin")].map(|made| made.stitches.unwrap());
    assert_eq!(stitches, [Stitches::Satin, Stitches::Running, Stitches::FilledStroke, Stitches::Running, Stitches::FilledStroke]);
    assert_eq!((rule(&line(3.0, "run")), rule(&line(10.0, "satin"))), ("asked (3 mm wide)".to_string(), "area (10 mm wide)".to_string()));
}

#[test]
fn req_auto_036_every_fill_sews_at_the_design_s_one_angle() {
    // By default a fill keeps the tatami fill's own angle, which is the rules' default, and its settings name
    // none. A shape that sets `auto_fill_angle` gives it to each fill made of it: of an area, of a blob, and
    // of a line too wide for a satin column.
    let angle = |element: &Element| made(element).elements.iter().map(|made| setting(made, ANGLE).map(str::to_string)).collect::<Vec<_>>();
    assert_eq!(angle(&shape("blob", &rectangle(4.5, 3.0), &[])), [None]);
    let turned = [("auto_fill_angle", "45")];
    assert_eq!(angle(&shape("blob", &rectangle(4.5, 3.0), &turned)), [Some("45".to_string())]);
    assert_eq!(angle(&shape("arm", &body_with_arm(3.0), &turned)), [Some("45".to_string()), None]);
    assert_eq!(angle(&widened(along("line", polylines(&[&[(0.0, 0.0), (30.0, 0.0)]]), &RED, &turned), 10.0)), [Some("45".to_string())]);
    // Planned, the blob's rows lie at that angle: few of its stitches run along x, where most did.
    let rows = |params: &[(&str, &str)]| {
        let plan = planned(unbased(vec![automatic(shape("blob", &rectangle(9.0, 6.0), params))])).plan.unwrap();
        let level = |stitch: &stitchcraft_plan::SewnStitch| (stitch.to.y() - stitch.from.y()).abs() < 0.05 && stitch.from.distance(stitch.to) > 1.0;
        plan.sewn_stitches().iter().filter(|stitch| stitch.role == Role::Top && level(stitch)).count()
    };
    assert!(rows(&[]) > 50 && rows(&turned) < rows(&[]) / 4, "{} {}", rows(&[]), rows(&turned));
}

#[test]
fn req_auto_030_a_design_s_settings_reach_the_shapes_that_set_none() {
    // `stitch plan --set` and `--style` set a setting on every automatic shape that doesn't set it itself:
    // a T keeps its columns, and one that fills its uneven parts by its own setting is filled. A shape with
    // settings takes none.
    // A T of a thick and a thin stroke is uneven: filled by default, and 2 columns with `auto_fill_uneven` off.
    let own = shape("own", &at(&thick_and_thin(), (30.0, 0.0)), &[("auto_fill_uneven", "true")]);
    let set = shape("set", &at(&thick_and_thin(), (60.0, 0.0)), &[]);
    let design = Design::new(vec![automatic(shape("t", &thick_and_thin(), &[])), automatic(own), set], DesignSettings::default()).unwrap();
    let even = [("auto_fill_uneven".to_string(), "false".to_string())];
    let styled = design.clone().automatic_with(&even);
    let params = |design: &Design, id: &str| design.elements().iter().find(|e| e.id.as_str() == id).unwrap().params.iter().count();
    assert_eq!([params(&styled, "t"), params(&styled, "own"), params(&styled, "set")], [1, 1, 0]);
    let filled = |design: &Design| {
        let (elements, _) = digitized(design, &Budget::DEFAULT);
        let fills = |id: &str| elements.iter().any(|e| e.id.as_str() == id && matches!(e.shape, Shape::Fill { .. }));
        [fills("t"), fills("own")]
    };
    assert_eq!([filled(&design), filled(&styled)], [[true, true], [false, true]]);
    assert_eq!(design.clone().automatic_with(&[]), design);
}

/// An ink drawing of a flower, drawn in code as a pen would: a stem that tapers, 2 leaves and 5 petals as
/// brush strokes, a round centre, hatching of hairlines, a blot with 2 runs of ink, the ground, and a frame
/// drawn with a broad nib. Each shape is an area, as a tracer draws a stroke.
fn ink_drawing() -> Design {
    let mut shapes = vec![
        shape("stem", &[(29.0, 40.0), (31.0, 40.0), (30.4, 70.0), (29.6, 70.0)], &[]),
        shape("leaf-1", &leaf((38.0, 58.0), 14.0, 4.0), &[]),
        shape("leaf-2", &leaf((22.0, 64.0), 12.0, 3.5), &[]),
        fill("centre", &[&ellipse((30.0, 30.0), (2.5, 2.5), 32)], &BLUE, &[]),
        shape("ground", &[(5.0, 72.0), (55.0, 72.0), (55.0, 84.0), (5.0, 84.0)], &[]),
        shape("blot", &blob_with_tails().iter().map(|&(x, y)| (x + 44.0, y + 40.0)).collect::<Vec<_>>(), &[]),
    ];
    for petal in 0..5_u32 {
        let (sin, cos) = sin_cos(std::f64::consts::TAU * f64::from(petal) / 5.0);
        let turned: Vec<(f64, f64)> =
            leaf((9.0, 0.0), 11.0, 4.5).into_iter().map(|(x, y)| (30.0 + x * cos - y * sin, 30.0 + x * sin + y * cos)).collect();
        shapes.push(shape(&format!("petal-{petal}"), &turned, &[]));
    }
    for hatch in 0..4_u32 {
        let x = 8.0 + 2.0 * f64::from(hatch);
        shapes.push(shape(&format!("hatch-{hatch}"), &[(x, 45.0), (x + 0.4, 45.0), (x + 6.4, 60.0), (x + 6.0, 60.0)], &[]));
    }
    let frame = [(0.0, 0.0), (60.0, 0.0), (60.0, 90.0), (0.0, 90.0)];
    shapes.push(fill("frame", &[&frame, &[(4.6, 4.6), (4.6, 85.4), (55.4, 85.4), (55.4, 4.6)]], &BLUE, &[]));
    Design::new(unbased(shapes.into_iter().map(automatic).collect()), DesignSettings::default()).unwrap()
}

#[test]
fn req_auto_030_unlike_subjects_sew_by_the_same_rules() {
    // The rules ask about geometry alone, so one table sorts a traced photo of an animal, a traced photo of
    // a fruit, drawings and an ink drawing. How many parts of each sewed by each rule, as recorded.
    let dog = pictures::dog_shapes(
        "dog-running-9",
        &["vc:14:fill", "vc:95:fill", "vc:96:fill", "vc:79:fill", "vc:10:fill", "vc:34:fill", "vc:27:fill", "vc:3:fill", "vc:11:fill"],
    );
    let subjects = [
        ("the running dog's nose, blaze and fur", fired(&dog)),
        ("the traced pear", fired(&pictures::pear())),
        ("TS-13C's face, star and balloon", fired(&pictures::sheet("TS-13C").0)),
        ("TS-13D's emblems and buttons", fired(&pictures::sheet("TS-13D").0)),
        ("an ink drawing", fired(&ink_drawing())),
    ];
    let recorded = [
        BTreeMap::from([("area", 2), ("column", 2), ("compact", 2), ("line", 1), ("uneven", 4)]),
        BTreeMap::from([("area", 3), ("column", 3), ("uneven", 2)]),
        BTreeMap::from([("area", 5), ("column", 11), ("line", 1)]),
        BTreeMap::from([("area", 6), ("column", 10), ("compact", 14), ("refused", 2)]),
        BTreeMap::from([("area", 1), ("column", 9), ("compact", 1), ("line", 6), ("uneven", 1)]),
    ];
    let found: Vec<(&str, &BTreeMap<&str, usize>)> = subjects.iter().map(|(name, fired)| (*name, fired)).collect();
    assert_eq!(found, subjects.iter().map(|(name, _)| *name).zip(&recorded).collect::<Vec<_>>());
    // Each rule that sorts a part by its shape fires on 3 unlike subjects or more. A column refused is a
    // matter of its rails, which the emblems' white rounds show here, and a square with a round hole in a
    // case of its own.
    for rule in ["line", "area", "compact", "uneven", "column"] {
        let on: Vec<&str> = subjects.iter().filter(|(_, fired)| fired.contains_key(rule)).map(|(name, _)| *name).collect();
        assert!(on.len() >= 3, "{rule}: {on:?}");
    }
    // The settings by tier each change what sews, which the cases of this file show for each, and the
    // narrowest satin stroke is the design's: at 2 mm the ink drawing's stem, 2 mm wide at its foot, runs.
    let wider =
        Design::new(ink_drawing().elements().to_vec(), DesignSettings { min_satin_stroke_width: Mm::new(2.0).unwrap(), ..DesignSettings::default() });
    assert!(fired(&wider.unwrap())["line"] > fired(&ink_drawing())["line"]);
}
